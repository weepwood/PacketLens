use etherparse::{NetSlice, SlicedPacket, TransportSlice};

use crate::model::{CapturedPacket, PacketSummary};

pub fn summarize(id: u64, packet: &CapturedPacket) -> PacketSummary {
    let mut summary = PacketSummary {
        id,
        timestamp_micros: packet.timestamp_micros,
        source: "—".to_string(),
        destination: "—".to_string(),
        source_port: None,
        destination_port: None,
        protocol: "RAW".to_string(),
        direction: "unknown".to_string(),
        length: packet.original_length,
        info: format!("{} captured bytes", packet.data.len()),
        process_id: None,
        process_name: None,
    };

    let Ok(sliced) = SlicedPacket::from_ethernet(&packet.data) else {
        return summary;
    };

    match sliced.net.as_ref() {
        Some(NetSlice::Ipv4(ipv4)) => {
            summary.source = ipv4.header().source_addr().to_string();
            summary.destination = ipv4.header().destination_addr().to_string();
            summary.protocol = "IPv4".to_string();
        }
        Some(NetSlice::Ipv6(ipv6)) => {
            summary.source = ipv6.header().source_addr().to_string();
            summary.destination = ipv6.header().destination_addr().to_string();
            summary.protocol = "IPv6".to_string();
        }
        Some(NetSlice::Arp(_)) => {
            summary.protocol = "ARP".to_string();
            summary.info = "Address Resolution Protocol".to_string();
            return summary;
        }
        None => return summary,
    }

    match sliced.transport.as_ref() {
        Some(TransportSlice::Tcp(tcp)) => {
            summary.source_port = Some(tcp.source_port());
            summary.destination_port = Some(tcp.destination_port());
            summary.protocol = "TCP".to_string();

            let mut flags = Vec::with_capacity(4);
            if tcp.syn() {
                flags.push("SYN");
            }
            if tcp.ack() {
                flags.push("ACK");
            }
            if tcp.fin() {
                flags.push("FIN");
            }
            if tcp.rst() {
                flags.push("RST");
            }
            if tcp.psh() {
                flags.push("PSH");
            }

            let flag_text = if flags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", flags.join(", "))
            };
            summary.info = format!(
                "{} → {}{} Seq={} Ack={} Win={}",
                tcp.source_port(),
                tcp.destination_port(),
                flag_text,
                tcp.sequence_number(),
                tcp.acknowledgment_number(),
                tcp.window_size()
            );
        }
        Some(TransportSlice::Udp(udp)) => {
            summary.source_port = Some(udp.source_port());
            summary.destination_port = Some(udp.destination_port());
            let is_dns = udp.source_port() == 53 || udp.destination_port() == 53;
            summary.protocol = if is_dns { "DNS" } else { "UDP" }.to_string();
            summary.info = format!(
                "{} → {} Len={}",
                udp.source_port(),
                udp.destination_port(),
                udp.length()
            );
        }
        Some(TransportSlice::Icmpv4(_)) => {
            summary.protocol = "ICMP".to_string();
            summary.info = "Internet Control Message Protocol".to_string();
        }
        Some(TransportSlice::Icmpv6(_)) => {
            summary.protocol = "ICMPv6".to_string();
            summary.info = "Internet Control Message Protocol v6".to_string();
        }
        None => {}
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_packet_is_reported_as_raw() {
        let packet = CapturedPacket {
            timestamp_micros: 42,
            original_length: 3,
            data: vec![1, 2, 3],
        };
        let summary = summarize(1, &packet);

        assert_eq!(summary.protocol, "RAW");
        assert_eq!(summary.direction, "unknown");
        assert_eq!(summary.id, 1);
    }
}
