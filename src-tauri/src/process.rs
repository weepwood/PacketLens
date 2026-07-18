use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::{Duration, Instant},
};

use crate::model::PacketSummary;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct TransportFlowKey {
    local_ip: IpAddr,
    local_port: u16,
    remote_ip: IpAddr,
    remote_port: u16,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct EndpointKey {
    local_ip: IpAddr,
    local_port: u16,
}

#[derive(Debug, Clone)]
struct ProcessInfo {
    pid: u32,
    name: Option<String>,
}

#[derive(Default)]
struct OwnerTables {
    tcp: HashMap<TransportFlowKey, ProcessInfo>,
    udp: HashMap<EndpointKey, ProcessInfo>,
}

pub struct ProcessResolver {
    last_refresh: Instant,
    tables: OwnerTables,
}

impl ProcessResolver {
    pub fn new() -> Self {
        Self {
            last_refresh: Instant::now() - Duration::from_secs(5),
            tables: OwnerTables::default(),
        }
    }

    pub fn enrich(&mut self, packet: &mut PacketSummary) {
        let transport = match packet.protocol.as_str() {
            "TCP" => TransportKind::Tcp,
            "UDP" | "DNS" => TransportKind::Udp,
            _ => return,
        };

        if self.last_refresh.elapsed() >= Duration::from_millis(750) {
            self.tables = platform::owner_tables();
            self.last_refresh = Instant::now();
        }

        let (Ok(source_ip), Ok(destination_ip), Some(source_port), Some(destination_port)) = (
            packet.source.parse::<IpAddr>(),
            packet.destination.parse::<IpAddr>(),
            packet.source_port,
            packet.destination_port,
        ) else {
            return;
        };

        match transport {
            TransportKind::Tcp => {
                let forward = TransportFlowKey {
                    local_ip: source_ip,
                    local_port: source_port,
                    remote_ip: destination_ip,
                    remote_port: destination_port,
                };
                let reverse = TransportFlowKey {
                    local_ip: destination_ip,
                    local_port: destination_port,
                    remote_ip: source_ip,
                    remote_port: source_port,
                };

                if let Some(process) = self.tables.tcp.get(&forward) {
                    apply_process(packet, process, "outbound");
                } else if let Some(process) = self.tables.tcp.get(&reverse) {
                    apply_process(packet, process, "inbound");
                }
            }
            TransportKind::Udp => {
                let source = EndpointKey {
                    local_ip: source_ip,
                    local_port: source_port,
                };
                let destination = EndpointKey {
                    local_ip: destination_ip,
                    local_port: destination_port,
                };

                if let Some(process) = lookup_udp_owner(&self.tables.udp, &source) {
                    apply_process(packet, process, "outbound");
                } else if let Some(process) = lookup_udp_owner(&self.tables.udp, &destination) {
                    apply_process(packet, process, "inbound");
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum TransportKind {
    Tcp,
    Udp,
}

fn apply_process(packet: &mut PacketSummary, process: &ProcessInfo, direction: &str) {
    packet.process_id = Some(process.pid);
    packet.process_name = process.name.clone();
    packet.direction = direction.to_string();
}

fn lookup_udp_owner<'a>(
    entries: &'a HashMap<EndpointKey, ProcessInfo>,
    endpoint: &EndpointKey,
) -> Option<&'a ProcessInfo> {
    entries.get(endpoint).or_else(|| {
        let wildcard = EndpointKey {
            local_ip: match endpoint.local_ip {
                IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            },
            local_port: endpoint.local_port,
        };
        entries.get(&wildcard)
    })
}

#[cfg(windows)]
mod platform {
    use std::{
        collections::HashMap,
        ffi::{c_void, OsString},
        mem::size_of,
        net::{IpAddr, Ipv4Addr, Ipv6Addr},
        os::windows::ffi::OsStringExt,
        path::Path,
        ptr,
    };

    use super::{EndpointKey, OwnerTables, ProcessInfo, TransportFlowKey};

    const AF_INET: u32 = 2;
    const AF_INET6: u32 = 23;
    const TCP_TABLE_OWNER_PID_ALL: u32 = 5;
    const UDP_TABLE_OWNER_PID: u32 = 1;
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

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MibTcp6RowOwnerPid {
        local_addr: [u8; 16],
        _local_scope_id: u32,
        local_port: u32,
        remote_addr: [u8; 16],
        _remote_scope_id: u32,
        remote_port: u32,
        _state: u32,
        owning_pid: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MibUdpRowOwnerPid {
        local_addr: u32,
        local_port: u32,
        owning_pid: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MibUdp6RowOwnerPid {
        local_addr: [u8; 16],
        _local_scope_id: u32,
        local_port: u32,
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
        fn GetExtendedUdpTable(
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

    pub(super) fn owner_tables() -> OwnerTables {
        let mut name_cache = HashMap::new();
        let mut tables = OwnerTables::default();
        tables.tcp.extend(tcp4_owner_table(&mut name_cache));
        tables.tcp.extend(tcp6_owner_table(&mut name_cache));
        tables.udp.extend(udp4_owner_table(&mut name_cache));
        tables.udp.extend(udp6_owner_table(&mut name_cache));
        tables
    }

    fn tcp4_owner_table(
        name_cache: &mut HashMap<u32, Option<String>>,
    ) -> HashMap<TransportFlowKey, ProcessInfo> {
        let Some(storage) = query_tcp_table(AF_INET) else {
            return HashMap::new();
        };
        unsafe {
            let count = storage[0] as usize;
            let rows = storage.as_ptr().add(1).cast::<MibTcpRowOwnerPid>();
            let mut output = HashMap::with_capacity(count);
            for index in 0..count {
                let row = *rows.add(index);
                output.insert(
                    TransportFlowKey {
                        local_ip: IpAddr::V4(Ipv4Addr::from(row.local_addr.to_ne_bytes())),
                        local_port: network_port(row.local_port),
                        remote_ip: IpAddr::V4(Ipv4Addr::from(row.remote_addr.to_ne_bytes())),
                        remote_port: network_port(row.remote_port),
                    },
                    process_info(row.owning_pid, name_cache),
                );
            }
            output
        }
    }

    fn tcp6_owner_table(
        name_cache: &mut HashMap<u32, Option<String>>,
    ) -> HashMap<TransportFlowKey, ProcessInfo> {
        let Some(storage) = query_tcp_table(AF_INET6) else {
            return HashMap::new();
        };
        unsafe {
            let count = storage[0] as usize;
            let rows = storage.as_ptr().add(1).cast::<MibTcp6RowOwnerPid>();
            let mut output = HashMap::with_capacity(count);
            for index in 0..count {
                let row = *rows.add(index);
                output.insert(
                    TransportFlowKey {
                        local_ip: IpAddr::V6(Ipv6Addr::from(row.local_addr)),
                        local_port: network_port(row.local_port),
                        remote_ip: IpAddr::V6(Ipv6Addr::from(row.remote_addr)),
                        remote_port: network_port(row.remote_port),
                    },
                    process_info(row.owning_pid, name_cache),
                );
            }
            output
        }
    }

    fn udp4_owner_table(
        name_cache: &mut HashMap<u32, Option<String>>,
    ) -> HashMap<EndpointKey, ProcessInfo> {
        let Some(storage) = query_udp_table(AF_INET) else {
            return HashMap::new();
        };
        unsafe {
            let count = storage[0] as usize;
            let rows = storage.as_ptr().add(1).cast::<MibUdpRowOwnerPid>();
            let mut output = HashMap::with_capacity(count);
            for index in 0..count {
                let row = *rows.add(index);
                output.insert(
                    EndpointKey {
                        local_ip: IpAddr::V4(Ipv4Addr::from(row.local_addr.to_ne_bytes())),
                        local_port: network_port(row.local_port),
                    },
                    process_info(row.owning_pid, name_cache),
                );
            }
            output
        }
    }

    fn udp6_owner_table(
        name_cache: &mut HashMap<u32, Option<String>>,
    ) -> HashMap<EndpointKey, ProcessInfo> {
        let Some(storage) = query_udp_table(AF_INET6) else {
            return HashMap::new();
        };
        unsafe {
            let count = storage[0] as usize;
            let rows = storage.as_ptr().add(1).cast::<MibUdp6RowOwnerPid>();
            let mut output = HashMap::with_capacity(count);
            for index in 0..count {
                let row = *rows.add(index);
                output.insert(
                    EndpointKey {
                        local_ip: IpAddr::V6(Ipv6Addr::from(row.local_addr)),
                        local_port: network_port(row.local_port),
                    },
                    process_info(row.owning_pid, name_cache),
                );
            }
            output
        }
    }

    fn query_tcp_table(address_family: u32) -> Option<Vec<u32>> {
        query_table(|table, size| unsafe {
            GetExtendedTcpTable(table, size, 0, address_family, TCP_TABLE_OWNER_PID_ALL, 0)
        })
    }

    fn query_udp_table(address_family: u32) -> Option<Vec<u32>> {
        query_table(|table, size| unsafe {
            GetExtendedUdpTable(table, size, 0, address_family, UDP_TABLE_OWNER_PID, 0)
        })
    }

    fn query_table(call: impl Fn(*mut c_void, *mut u32) -> u32) -> Option<Vec<u32>> {
        let mut byte_size = 0u32;
        let first = call(ptr::null_mut(), &mut byte_size);
        if first != ERROR_INSUFFICIENT_BUFFER || byte_size < 4 {
            return None;
        }

        let word_count = byte_size.div_ceil(size_of::<u32>() as u32) as usize;
        let mut storage = vec![0u32; word_count];
        if call(storage.as_mut_ptr().cast(), &mut byte_size) != NO_ERROR {
            return None;
        }
        Some(storage)
    }

    fn network_port(value: u32) -> u16 {
        u16::from_be(value as u16)
    }

    fn process_info(pid: u32, name_cache: &mut HashMap<u32, Option<String>>) -> ProcessInfo {
        let name = name_cache
            .entry(pid)
            .or_insert_with(|| process_name(pid))
            .clone();
        ProcessInfo { pid, name }
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
    use super::OwnerTables;

    pub(super) fn owner_tables() -> OwnerTables {
        OwnerTables::default()
    }
}
