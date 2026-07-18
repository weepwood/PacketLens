use std::{
    cmp::Reverse,
    collections::HashMap,
    hash::{Hash, Hasher},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::model::{FlowSnapshot, FlowSummary, PacketSummary, ProcessTrafficSummary};

const ACTIVE_WINDOW_MICROS: u64 = 15 * 1_000_000;
const RETENTION_WINDOW_MICROS: u64 = 5 * 60 * 1_000_000;
const MAX_TRACKED_FLOWS: usize = 20_000;
const MAX_SNAPSHOT_FLOWS: usize = 5_000;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct FlowKey {
    protocol: String,
    local_address: String,
    local_port: Option<u16>,
    remote_address: String,
    remote_port: Option<u16>,
    process_id: Option<u32>,
}

#[derive(Debug, Clone)]
struct FlowEntry {
    key: FlowKey,
    process_name: Option<String>,
    first_seen_micros: u64,
    last_seen_micros: u64,
    upload_packets: u64,
    download_packets: u64,
    unknown_packets: u64,
    upload_bytes: u64,
    download_bytes: u64,
    unknown_bytes: u64,
}

#[derive(Debug, Default)]
pub struct FlowTracker {
    entries: HashMap<FlowKey, FlowEntry>,
}

impl FlowTracker {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn observe(&mut self, packet: &PacketSummary) {
        if !matches!(packet.protocol.as_str(), "TCP" | "UDP" | "DNS") {
            return;
        }

        let (local_address, local_port, remote_address, remote_port) =
            match packet.direction.as_str() {
                "outbound" => (
                    packet.source.clone(),
                    packet.source_port,
                    packet.destination.clone(),
                    packet.destination_port,
                ),
                "inbound" => (
                    packet.destination.clone(),
                    packet.destination_port,
                    packet.source.clone(),
                    packet.source_port,
                ),
                _ => canonical_endpoints(packet),
            };

        let key = FlowKey {
            protocol: packet.protocol.clone(),
            local_address,
            local_port,
            remote_address,
            remote_port,
            process_id: packet.process_id,
        };
        let entry = self
            .entries
            .entry(key.clone())
            .or_insert_with(|| FlowEntry {
                key,
                process_name: packet.process_name.clone(),
                first_seen_micros: packet.timestamp_micros,
                last_seen_micros: packet.timestamp_micros,
                upload_packets: 0,
                download_packets: 0,
                unknown_packets: 0,
                upload_bytes: 0,
                download_bytes: 0,
                unknown_bytes: 0,
            });

        entry.last_seen_micros = entry.last_seen_micros.max(packet.timestamp_micros);
        if entry.process_name.is_none() && packet.process_name.is_some() {
            entry.process_name = packet.process_name.clone();
        }

        match packet.direction.as_str() {
            "outbound" => {
                entry.upload_packets = entry.upload_packets.saturating_add(1);
                entry.upload_bytes = entry.upload_bytes.saturating_add(packet.length as u64);
            }
            "inbound" => {
                entry.download_packets = entry.download_packets.saturating_add(1);
                entry.download_bytes = entry.download_bytes.saturating_add(packet.length as u64);
            }
            _ => {
                entry.unknown_packets = entry.unknown_packets.saturating_add(1);
                entry.unknown_bytes = entry.unknown_bytes.saturating_add(packet.length as u64);
            }
        }

        if self.entries.len() > MAX_TRACKED_FLOWS {
            self.compact(packet.timestamp_micros);
        }
    }

    pub fn snapshot(&mut self) -> FlowSnapshot {
        let generated_at_micros = unix_time_micros();
        self.prune(generated_at_micros);

        let mut flows = self
            .entries
            .values()
            .map(|entry| entry.to_summary(generated_at_micros))
            .collect::<Vec<_>>();
        flows.sort_by_key(|flow| Reverse(flow.last_seen_micros));
        flows.truncate(MAX_SNAPSHOT_FLOWS);

        let mut process_map: HashMap<(Option<u32>, String), ProcessTrafficSummary> = HashMap::new();
        for flow in &flows {
            let process_name = flow
                .process_name
                .clone()
                .unwrap_or_else(|| "未识别进程".to_string());
            let process = process_map
                .entry((flow.process_id, process_name.clone()))
                .or_insert_with(|| ProcessTrafficSummary {
                    process_id: flow.process_id,
                    process_name,
                    connection_count: 0,
                    active_connection_count: 0,
                    upload_packets: 0,
                    download_packets: 0,
                    unknown_packets: 0,
                    upload_bytes: 0,
                    download_bytes: 0,
                    unknown_bytes: 0,
                    last_seen_micros: 0,
                });
            process.connection_count = process.connection_count.saturating_add(1);
            if flow.state == "active" {
                process.active_connection_count = process.active_connection_count.saturating_add(1);
            }
            process.upload_packets = process.upload_packets.saturating_add(flow.upload_packets);
            process.download_packets = process
                .download_packets
                .saturating_add(flow.download_packets);
            process.unknown_packets = process.unknown_packets.saturating_add(flow.unknown_packets);
            process.upload_bytes = process.upload_bytes.saturating_add(flow.upload_bytes);
            process.download_bytes = process.download_bytes.saturating_add(flow.download_bytes);
            process.unknown_bytes = process.unknown_bytes.saturating_add(flow.unknown_bytes);
            process.last_seen_micros = process.last_seen_micros.max(flow.last_seen_micros);
        }

        let mut processes = process_map.into_values().collect::<Vec<_>>();
        processes.sort_by_key(|process| {
            Reverse(
                process
                    .upload_bytes
                    .saturating_add(process.download_bytes)
                    .saturating_add(process.unknown_bytes),
            )
        });

        FlowSnapshot {
            generated_at_micros,
            tracked_flow_count: self.entries.len() as u64,
            flows,
            processes,
        }
    }

    fn compact(&mut self, now_micros: u64) {
        self.prune(now_micros);
        if self.entries.len() <= MAX_TRACKED_FLOWS {
            return;
        }

        let mut oldest = self
            .entries
            .iter()
            .map(|(key, entry)| (key.clone(), entry.last_seen_micros))
            .collect::<Vec<_>>();
        oldest.sort_by_key(|(_, last_seen)| *last_seen);
        let remove_count = self
            .entries
            .len()
            .saturating_sub(MAX_TRACKED_FLOWS * 9 / 10);
        for (key, _) in oldest.into_iter().take(remove_count) {
            self.entries.remove(&key);
        }
    }

    fn prune(&mut self, now_micros: u64) {
        self.entries.retain(|_, entry| {
            now_micros.saturating_sub(entry.last_seen_micros) <= RETENTION_WINDOW_MICROS
        });
    }
}

impl FlowEntry {
    fn to_summary(&self, now_micros: u64) -> FlowSummary {
        FlowSummary {
            id: stable_flow_id(&self.key),
            protocol: self.key.protocol.clone(),
            local_address: self.key.local_address.clone(),
            local_port: self.key.local_port,
            remote_address: self.key.remote_address.clone(),
            remote_port: self.key.remote_port,
            process_id: self.key.process_id,
            process_name: self.process_name.clone(),
            first_seen_micros: self.first_seen_micros,
            last_seen_micros: self.last_seen_micros,
            state: if now_micros.saturating_sub(self.last_seen_micros) <= ACTIVE_WINDOW_MICROS {
                "active".to_string()
            } else {
                "recent".to_string()
            },
            upload_packets: self.upload_packets,
            download_packets: self.download_packets,
            unknown_packets: self.unknown_packets,
            upload_bytes: self.upload_bytes,
            download_bytes: self.download_bytes,
            unknown_bytes: self.unknown_bytes,
        }
    }
}

fn canonical_endpoints(packet: &PacketSummary) -> (String, Option<u16>, String, Option<u16>) {
    let source = (
        packet.source.as_str(),
        packet.source_port.unwrap_or_default(),
    );
    let destination = (
        packet.destination.as_str(),
        packet.destination_port.unwrap_or_default(),
    );
    if source <= destination {
        (
            packet.source.clone(),
            packet.source_port,
            packet.destination.clone(),
            packet.destination_port,
        )
    } else {
        (
            packet.destination.clone(),
            packet.destination_port,
            packet.source.clone(),
            packet.source_port,
        )
    }
}

fn stable_flow_id(key: &FlowKey) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn unix_time_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_micros() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(direction: &str, length: u32) -> PacketSummary {
        PacketSummary {
            id: 1,
            timestamp_micros: unix_time_micros(),
            source: "10.0.0.2".to_string(),
            destination: "1.1.1.1".to_string(),
            source_port: Some(50_000),
            destination_port: Some(443),
            protocol: "TCP".to_string(),
            direction: direction.to_string(),
            length,
            info: String::new(),
            process_id: Some(42),
            process_name: Some("browser.exe".to_string()),
        }
    }

    #[test]
    fn aggregates_process_and_directional_traffic() {
        let mut tracker = FlowTracker::default();
        tracker.observe(&packet("outbound", 100));
        tracker.observe(&packet("inbound", 200));
        let snapshot = tracker.snapshot();

        assert_eq!(snapshot.flows.len(), 1);
        assert_eq!(snapshot.flows[0].upload_bytes, 100);
        assert_eq!(snapshot.flows[0].download_bytes, 200);
        assert_eq!(snapshot.processes[0].process_id, Some(42));
    }
}
