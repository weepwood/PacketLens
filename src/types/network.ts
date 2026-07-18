export interface NetworkInterface {
  name: string;
  description: string | null;
  addresses: string[];
  loopback: boolean;
}

export interface CaptureRequest {
  deviceName: string;
  filter: string | null;
  promiscuous: boolean;
  snapshotLength: number;
}

export interface PacketSummary {
  id: number;
  timestampMicros: number;
  source: string;
  destination: string;
  sourcePort: number | null;
  destinationPort: number | null;
  protocol: string;
  length: number;
  info: string;
  processId: number | null;
  processName: string | null;
}

export interface CaptureStatistics {
  capturedPackets: number;
  capturedBytes: number;
  droppedPackets: number;
  packetsPerSecond: number;
  bytesPerSecond: number;
}

export interface PacketBatch {
  packets: PacketSummary[];
  statistics: CaptureStatistics;
}

export interface CaptureStatus {
  running: boolean;
  deviceName: string | null;
  startedAtUnixMs: number | null;
  npcapAvailable: boolean;
  lastError: string | null;
}
