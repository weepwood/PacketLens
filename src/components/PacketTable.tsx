import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { PacketSummary } from "../types/network";

interface PacketTableProps {
  packets: PacketSummary[];
  selectedPacketId: number | null;
  onSelect: (id: number) => void;
}

function formatTime(timestampMicros: number): string {
  const date = new Date(Math.floor(timestampMicros / 1000));
  return date.toLocaleTimeString("zh-CN", { hour12: false }) + `.${String(timestampMicros % 1_000_000).padStart(6, "0").slice(0, 3)}`;
}

function endpoint(address: string, port: number | null): string {
  return port === null ? address : `${address}:${port}`;
}

export function PacketTable({ packets, selectedPacketId, onSelect }: PacketTableProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: packets.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 35,
    overscan: 12,
  });

  return (
    <div className="packet-table-shell">
      <div className="packet-table-header packet-grid">
        <span>No.</span>
        <span>Time</span>
        <span>Process</span>
        <span>Source</span>
        <span>Destination</span>
        <span>Protocol</span>
        <span>Length</span>
        <span>Info</span>
      </div>
      <div ref={parentRef} className="packet-table-body">
        {packets.length === 0 ? (
          <div className="empty-state">尚未捕获数据包。选择网卡并点击“开始捕获”。</div>
        ) : (
          <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const packet = packets[virtualRow.index];
              return (
                <button
                  type="button"
                  key={packet.id}
                  className={`packet-row packet-grid ${selectedPacketId === packet.id ? "selected" : ""}`}
                  style={{ transform: `translateY(${virtualRow.start}px)` }}
                  onClick={() => onSelect(packet.id)}
                >
                  <span>{packet.id}</span>
                  <span>{formatTime(packet.timestampMicros)}</span>
                  <span>{packet.processName ?? "—"}</span>
                  <span title={endpoint(packet.source, packet.sourcePort)}>{endpoint(packet.source, packet.sourcePort)}</span>
                  <span title={endpoint(packet.destination, packet.destinationPort)}>{endpoint(packet.destination, packet.destinationPort)}</span>
                  <span><b className={`protocol protocol-${packet.protocol.toLowerCase()}`}>{packet.protocol}</b></span>
                  <span>{packet.length}</span>
                  <span title={packet.info}>{packet.info}</span>
                </button>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
