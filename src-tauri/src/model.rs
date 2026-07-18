use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterface {
    pub name: String,
    pub description: Option<String>,
    pub addresses: Vec<String>,
    pub loopback: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRequest {
    pub device_name: String,
    pub filter: Option<String>,
    pub promiscuous: bool,
    pub snapshot_length: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacketSummary {
    pub id: u64,
    pub timestamp_micros: u64,
    pub source: String,
    pub destination: String,
    pub source_port: Option<u16>,
    pub destination_port: Option<u16>,
    pub protocol: String,
    pub direction: String,
    pub length: u32,
    pub info: String,
    pub process_id: Option<u32>,
    pub process_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatistics {
    pub captured_packets: u64,
    pub captured_bytes: u64,
    pub dropped_packets: u64,
    pub storage_dropped_packets: u64,
    pub packets_per_second: u64,
    pub bytes_per_second: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacketBatch {
    pub packets: Vec<PacketSummary>,
    pub statistics: CaptureStatistics,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub running: bool,
    pub device_name: Option<String>,
    pub session_id: Option<String>,
    pub started_at_unix_ms: Option<u64>,
    pub npcap_available: bool,
    pub last_error: Option<String>,
}

impl Default for CaptureStatus {
    fn default() -> Self {
        Self {
            running: false,
            device_name: None,
            session_id: None,
            started_at_unix_ms: None,
            npcap_available: crate::capture::npcap_available(),
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowSummary {
    pub id: String,
    pub protocol: String,
    pub local_address: String,
    pub local_port: Option<u16>,
    pub remote_address: String,
    pub remote_port: Option<u16>,
    pub process_id: Option<u32>,
    pub process_name: Option<String>,
    pub first_seen_micros: u64,
    pub last_seen_micros: u64,
    pub state: String,
    pub upload_packets: u64,
    pub download_packets: u64,
    pub unknown_packets: u64,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub unknown_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessTrafficSummary {
    pub process_id: Option<u32>,
    pub process_name: String,
    pub connection_count: u64,
    pub active_connection_count: u64,
    pub upload_packets: u64,
    pub download_packets: u64,
    pub unknown_packets: u64,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub unknown_bytes: u64,
    pub last_seen_micros: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowSnapshot {
    pub generated_at_micros: u64,
    pub tracked_flow_count: u64,
    pub flows: Vec<FlowSummary>,
    pub processes: Vec<ProcessTrafficSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSessionSummary {
    pub id: String,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub device_name: String,
    pub filter: Option<String>,
    pub status: String,
    pub packet_count: u64,
    pub byte_count: u64,
    pub storage_dropped: u64,
    pub segment_count: u32,
    pub directory_path: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredPacketSummary {
    pub id: u64,
    pub timestamp_micros: u64,
    pub segment_index: u32,
    pub file_offset: u64,
    pub captured_length: u32,
    pub source: String,
    pub destination: String,
    pub source_port: Option<u16>,
    pub destination_port: Option<u16>,
    pub protocol: String,
    pub length: u32,
    pub info: String,
    pub process_id: Option<u32>,
    pub process_name: Option<String>,
}

#[derive(Debug)]
pub struct CapturedPacket {
    pub timestamp_micros: u64,
    pub original_length: u32,
    pub data: Vec<u8>,
}
