use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr},
    time::{Duration, Instant},
};

use crate::model::PacketSummary;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct TcpFlowKey {
    local_ip: Ipv4Addr,
    local_port: u16,
    remote_ip: Ipv4Addr,
    remote_port: u16,
}

#[derive(Debug, Clone)]
struct ProcessInfo {
    pid: u32,
    name: Option<String>,
}

pub struct ProcessResolver {
    last_refresh: Instant,
    entries: HashMap<TcpFlowKey, ProcessInfo>,
}

impl ProcessResolver {
    pub fn new() -> Self {
        Self {
            last_refresh: Instant::now() - Duration::from_secs(5),
            entries: HashMap::new(),
        }
    }

    pub fn enrich(&mut self, packet: &mut PacketSummary) {
        if packet.protocol != "TCP" {
            return;
        }

        if self.last_refresh.elapsed() >= Duration::from_millis(750) {
            self.entries = platform::tcp_owner_table();
            self.last_refresh = Instant::now();
        }

        let (
            Ok(IpAddr::V4(source_ip)),
            Ok(IpAddr::V4(destination_ip)),
            Some(source_port),
            Some(destination_port),
        ) = (
            packet.source.parse::<IpAddr>(),
            packet.destination.parse::<IpAddr>(),
            packet.source_port,
            packet.destination_port,
        )
        else {
            return;
        };

        let forward = TcpFlowKey {
            local_ip: source_ip,
            local_port: source_port,
            remote_ip: destination_ip,
            remote_port: destination_port,
        };
        let reverse = TcpFlowKey {
            local_ip: destination_ip,
            local_port: destination_port,
            remote_ip: source_ip,
            remote_port: source_port,
        };

        if let Some(process) = self
            .entries
            .get(&forward)
            .or_else(|| self.entries.get(&reverse))
        {
            packet.process_id = Some(process.pid);
            packet.process_name = process.name.clone();
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::{
        collections::HashMap,
        ffi::{c_void, OsString},
        mem::size_of,
        net::Ipv4Addr,
        os::windows::ffi::OsStringExt,
        path::Path,
        ptr,
    };

    use super::{ProcessInfo, TcpFlowKey};

    const AF_INET: u32 = 2;
    const TCP_TABLE_OWNER_PID_ALL: u32 = 5;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
    const NO_ERROR: u32 = 0;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    type Handle = *mut c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MibTcpRowOwnerPid {
        _state: u32,
        local_addr: u32,
        local_port: u32,
        remote_addr: u32,
        remote_port: u32,
        owning_pid: u32,
    }

    #[link(name = "iphlpapi")]
    extern "system" {
        fn GetExtendedTcpTable(
            table: *mut c_void,
            size: *mut u32,
            order: i32,
            address_family: u32,
            table_class: u32,
            reserved: u32,
        ) -> u32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn QueryFullProcessImageNameW(
            process: Handle,
            flags: u32,
            executable_name: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    pub(super) fn tcp_owner_table() -> HashMap<TcpFlowKey, ProcessInfo> {
        unsafe {
            let mut byte_size = 0u32;
            let first = GetExtendedTcpTable(
                ptr::null_mut(),
                &mut byte_size,
                0,
                AF_INET,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            );
            if first != ERROR_INSUFFICIENT_BUFFER || byte_size < 4 {
                return HashMap::new();
            }

            let word_count = byte_size.div_ceil(size_of::<u32>() as u32) as usize;
            let mut storage = vec![0u32; word_count];
            let result = GetExtendedTcpTable(
                storage.as_mut_ptr().cast(),
                &mut byte_size,
                0,
                AF_INET,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            );
            if result != NO_ERROR {
                return HashMap::new();
            }

            let count = storage[0] as usize;
            let rows = storage.as_ptr().add(1).cast::<MibTcpRowOwnerPid>();
            let mut output = HashMap::with_capacity(count);

            for index in 0..count {
                let row = *rows.add(index);
                let key = TcpFlowKey {
                    local_ip: Ipv4Addr::from(row.local_addr.to_ne_bytes()),
                    local_port: network_port(row.local_port),
                    remote_ip: Ipv4Addr::from(row.remote_addr.to_ne_bytes()),
                    remote_port: network_port(row.remote_port),
                };
                output.insert(
                    key,
                    ProcessInfo {
                        pid: row.owning_pid,
                        name: process_name(row.owning_pid),
                    },
                );
            }

            output
        }
    }

    fn network_port(value: u32) -> u16 {
        u16::from_be(value as u16)
    }

    fn process_name(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }

            let mut buffer = vec![0u16; 32_768];
            let mut length = buffer.len() as u32;
            let success =
                QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) != 0;
            let _ = CloseHandle(handle);

            if !success || length == 0 {
                return None;
            }

            let path = OsString::from_wide(&buffer[..length as usize]);
            Path::new(&path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::collections::HashMap;

    use super::{ProcessInfo, TcpFlowKey};

    pub(super) fn tcp_owner_table() -> HashMap<TcpFlowKey, ProcessInfo> {
        HashMap::new()
    }
}
