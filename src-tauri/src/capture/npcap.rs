#[cfg(windows)]
mod windows {
    use std::{
        ffi::{c_char, c_int, c_void, CStr, CString},
        path::PathBuf,
        ptr, slice,
    };

    use libloading::Library;

    use crate::model::{CaptureRequest, CapturedPacket, NetworkInterface};

    const PCAP_ERRBUF_SIZE: usize = 256;
    const PCAP_IF_LOOPBACK: u32 = 0x0000_0001;

    #[repr(C)]
    struct PcapHandle {
        _private: [u8; 0],
    }

    #[repr(C)]
    #[allow(dead_code)]
    struct PcapIf {
        next: *mut PcapIf,
        name: *mut c_char,
        description: *mut c_char,
        _addresses: *mut c_void,
        flags: u32,
    }

    #[repr(C)]
    struct TimeVal {
        tv_sec: i32,
        tv_usec: i32,
    }

    #[repr(C)]
    struct PcapPacketHeader {
        timestamp: TimeVal,
        captured_length: u32,
        original_length: u32,
    }

    #[repr(C)]
    #[allow(dead_code)]
    struct BpfProgram {
        length: u32,
        instructions: *mut c_void,
    }

    type FindAllDevices = unsafe extern "C" fn(*mut *mut PcapIf, *mut c_char) -> c_int;
    type FreeAllDevices = unsafe extern "C" fn(*mut PcapIf);
    type OpenLive = unsafe extern "C" fn(
        *const c_char,
        c_int,
        c_int,
        c_int,
        *mut c_char,
    ) -> *mut PcapHandle;
    type NextPacket = unsafe extern "C" fn(
        *mut PcapHandle,
        *mut *const PcapPacketHeader,
        *mut *const u8,
    ) -> c_int;
    type CloseCapture = unsafe extern "C" fn(*mut PcapHandle);
    type CompileFilter = unsafe extern "C" fn(
        *mut PcapHandle,
        *mut BpfProgram,
        *const c_char,
        c_int,
        u32,
    ) -> c_int;
    type SetFilter = unsafe extern "C" fn(*mut PcapHandle, *mut BpfProgram) -> c_int;
    type FreeFilter = unsafe extern "C" fn(*mut BpfProgram);
    type GetError = unsafe extern "C" fn(*mut PcapHandle) -> *const c_char;

    struct NpcapApi {
        _library: Library,
        find_all_devices: FindAllDevices,
        free_all_devices: FreeAllDevices,
        open_live: OpenLive,
        next_packet: NextPacket,
        close_capture: CloseCapture,
        compile_filter: CompileFilter,
        set_filter: SetFilter,
        free_filter: FreeFilter,
        get_error: GetError,
    }

    impl NpcapApi {
        fn load() -> Result<Self, String> {
            let mut candidates = vec![PathBuf::from("wpcap.dll")];
            if let Some(system_root) = std::env::var_os("SystemRoot") {
                candidates.push(
                    PathBuf::from(system_root)
                        .join("System32")
                        .join("Npcap")
                        .join("wpcap.dll"),
                );
            }

            let mut last_error = None;
            for candidate in candidates {
                let library = match unsafe { Library::new(&candidate) } {
                    Ok(library) => library,
                    Err(error) => {
                        last_error = Some(format!("{}: {error}", candidate.display()));
                        continue;
                    }
                };

                unsafe {
                    let find_all_devices = *library
                        .get::<FindAllDevices>(b"pcap_findalldevs\0")
                        .map_err(|error| error.to_string())?;
                    let free_all_devices = *library
                        .get::<FreeAllDevices>(b"pcap_freealldevs\0")
                        .map_err(|error| error.to_string())?;
                    let open_live = *library
                        .get::<OpenLive>(b"pcap_open_live\0")
                        .map_err(|error| error.to_string())?;
                    let next_packet = *library
                        .get::<NextPacket>(b"pcap_next_ex\0")
                        .map_err(|error| error.to_string())?;
                    let close_capture = *library
                        .get::<CloseCapture>(b"pcap_close\0")
                        .map_err(|error| error.to_string())?;
                    let compile_filter = *library
                        .get::<CompileFilter>(b"pcap_compile\0")
                        .map_err(|error| error.to_string())?;
                    let set_filter = *library
                        .get::<SetFilter>(b"pcap_setfilter\0")
                        .map_err(|error| error.to_string())?;
                    let free_filter = *library
                        .get::<FreeFilter>(b"pcap_freecode\0")
                        .map_err(|error| error.to_string())?;
                    let get_error = *library
                        .get::<GetError>(b"pcap_geterr\0")
                        .map_err(|error| error.to_string())?;

                    return Ok(Self {
                        _library: library,
                        find_all_devices,
                        free_all_devices,
                        open_live,
                        next_packet,
                        close_capture,
                        compile_filter,
                        set_filter,
                        free_filter,
                        get_error,
                    });
                }
            }

            Err(format!(
                "未找到 Npcap 的 wpcap.dll。请先安装 Npcap。{}",
                last_error
                    .map(|error| format!(" 最后一次加载错误：{error}"))
                    .unwrap_or_default()
            ))
        }

        fn capture_error(&self, handle: *mut PcapHandle) -> String {
            let pointer = unsafe { (self.get_error)(handle) };
            if pointer.is_null() {
                "Npcap 返回未知错误".to_string()
            } else {
                unsafe { CStr::from_ptr(pointer) }
                    .to_string_lossy()
                    .into_owned()
            }
        }
    }

    pub fn available() -> bool {
        NpcapApi::load().is_ok()
    }

    pub fn interfaces() -> Result<Vec<NetworkInterface>, String> {
        let api = NpcapApi::load()?;
        let mut devices: *mut PcapIf = ptr::null_mut();
        let mut error_buffer = [0 as c_char; PCAP_ERRBUF_SIZE];

        let result =
            unsafe { (api.find_all_devices)(&mut devices, error_buffer.as_mut_ptr()) };
        if result != 0 {
            return Err(error_buffer_to_string(&error_buffer));
        }

        let mut output = Vec::new();
        let mut current = devices;
        while !current.is_null() {
            unsafe {
                let device = &*current;
                let name = nullable_string(device.name).unwrap_or_default();
                if !name.is_empty() {
                    output.push(NetworkInterface {
                        name,
                        description: nullable_string(device.description),
                        addresses: Vec::new(),
                        loopback: device.flags & PCAP_IF_LOOPBACK != 0,
                    });
                }
                current = device.next;
            }
        }

        unsafe { (api.free_all_devices)(devices) };
        Ok(output)
    }

    pub struct CaptureSession {
        api: NpcapApi,
        handle: *mut PcapHandle,
    }

    unsafe impl Send for CaptureSession {}

    impl CaptureSession {
        pub fn open(request: &CaptureRequest) -> Result<Self, String> {
            let api = NpcapApi::load()?;
            let device = CString::new(request.device_name.as_str())
                .map_err(|_| "网卡名称包含无效的 NUL 字符".to_string())?;
            let mut error_buffer = [0 as c_char; PCAP_ERRBUF_SIZE];

            let handle = unsafe {
                (api.open_live)(
                    device.as_ptr(),
                    request.snapshot_length.clamp(64, 65_535),
                    if request.promiscuous { 1 } else { 0 },
                    200,
                    error_buffer.as_mut_ptr(),
                )
            };

            if handle.is_null() {
                return Err(error_buffer_to_string(&error_buffer));
            }

            let session = Self { api, handle };
            if let Some(filter) = request
                .filter
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                session.apply_filter(filter)?;
            }
            Ok(session)
        }

        fn apply_filter(&self, filter: &str) -> Result<(), String> {
            let filter = CString::new(filter)
                .map_err(|_| "捕获过滤器包含无效的 NUL 字符".to_string())?;
            let mut program = BpfProgram {
                length: 0,
                instructions: ptr::null_mut(),
            };

            let compile_result = unsafe {
                (self.api.compile_filter)(
                    self.handle,
                    &mut program,
                    filter.as_ptr(),
                    1,
                    u32::MAX,
                )
            };
            if compile_result != 0 {
                return Err(format!(
                    "BPF 过滤器编译失败：{}",
                    self.api.capture_error(self.handle)
                ));
            }

            let set_result = unsafe { (self.api.set_filter)(self.handle, &mut program) };
            unsafe { (self.api.free_filter)(&mut program) };
            if set_result != 0 {
                return Err(format!(
                    "BPF 过滤器应用失败：{}",
                    self.api.capture_error(self.handle)
                ));
            }
            Ok(())
        }

        pub fn next_packet(&mut self) -> Result<Option<CapturedPacket>, String> {
            let mut header: *const PcapPacketHeader = ptr::null();
            let mut data: *const u8 = ptr::null();
            let result =
                unsafe { (self.api.next_packet)(self.handle, &mut header, &mut data) };

            match result {
                1 if !header.is_null() && !data.is_null() => {
                    let header = unsafe { &*header };
                    let bytes = unsafe {
                        slice::from_raw_parts(data, header.captured_length as usize)
                    };
                    let seconds = header.timestamp.tv_sec.max(0) as u64;
                    let micros = header.timestamp.tv_usec.max(0) as u64;
                    Ok(Some(CapturedPacket {
                        timestamp_micros: seconds
                            .saturating_mul(1_000_000)
                            .saturating_add(micros),
                        original_length: header.original_length,
                        data: bytes.to_vec(),
                    }))
                }
                0 => Ok(None),
                -2 => Ok(None),
                _ => Err(self.api.capture_error(self.handle)),
            }
        }
    }

    impl Drop for CaptureSession {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe { (self.api.close_capture)(self.handle) };
                self.handle = ptr::null_mut();
            }
        }
    }

    fn nullable_string(value: *const c_char) -> Option<String> {
        if value.is_null() {
            None
        } else {
            Some(
                unsafe { CStr::from_ptr(value) }
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    }

    fn error_buffer_to_string(buffer: &[c_char; PCAP_ERRBUF_SIZE]) -> String {
        let bytes: Vec<u8> = buffer
            .iter()
            .copied()
            .take_while(|value| *value != 0)
            .map(|value| value as u8)
            .collect();
        let message = String::from_utf8_lossy(&bytes).trim().to_string();
        if message.is_empty() {
            "Npcap 操作失败".to_string()
        } else {
            message
        }
    }
}

#[cfg(not(windows))]
mod windows {
    use crate::model::{CaptureRequest, CapturedPacket, NetworkInterface};

    pub fn available() -> bool {
        false
    }

    pub fn interfaces() -> Result<Vec<NetworkInterface>, String> {
        Err("PacketLens 当前只支持 Windows 实时抓包".to_string())
    }

    pub struct CaptureSession;

    impl CaptureSession {
        pub fn open(_request: &CaptureRequest) -> Result<Self, String> {
            Err("PacketLens 当前只支持 Windows 实时抓包".to_string())
        }

        pub fn next_packet(&mut self) -> Result<Option<CapturedPacket>, String> {
            Ok(None)
        }
    }
}

pub use windows::CaptureSession;

pub fn npcap_available() -> bool {
    windows::available()
}

pub fn list_interfaces() -> Result<Vec<crate::model::NetworkInterface>, String> {
    windows::interfaces()
}
