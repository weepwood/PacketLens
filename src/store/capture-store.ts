import { create } from "zustand";
import type {
  CaptureStatistics,
  CaptureStatus,
  NetworkInterface,
  PacketBatch,
  PacketSummary,
} from "../types/network";

const EMPTY_STATISTICS: CaptureStatistics = {
  capturedPackets: 0,
  capturedBytes: 0,
  droppedPackets: 0,
  packetsPerSecond: 0,
  bytesPerSecond: 0,
};

interface TrafficPoint {
  timestamp: number;
  bytesPerSecond: number;
  packetsPerSecond: number;
}

interface CaptureStore {
  interfaces: NetworkInterface[];
  selectedInterface: string;
  filter: string;
  status: CaptureStatus;
  statistics: CaptureStatistics;
  packets: PacketSummary[];
  trafficHistory: TrafficPoint[];
  selectedPacketId: number | null;
  loading: boolean;
  error: string | null;
  setInterfaces: (interfaces: NetworkInterface[]) => void;
  setSelectedInterface: (name: string) => void;
  setFilter: (filter: string) => void;
  setStatus: (status: CaptureStatus) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  appendBatch: (batch: PacketBatch) => void;
  selectPacket: (id: number) => void;
  clearPackets: () => void;
}

const INITIAL_STATUS: CaptureStatus = {
  running: false,
  deviceName: null,
  startedAtUnixMs: null,
  npcapAvailable: false,
  lastError: null,
};

export const useCaptureStore = create<CaptureStore>((set) => ({
  interfaces: [],
  selectedInterface: "",
  filter: "tcp or udp or icmp",
  status: INITIAL_STATUS,
  statistics: EMPTY_STATISTICS,
  packets: [],
  trafficHistory: [],
  selectedPacketId: null,
  loading: false,
  error: null,
  setInterfaces: (interfaces) =>
    set((state) => ({
      interfaces,
      selectedInterface:
        state.selectedInterface || interfaces.find((item) => !item.loopback)?.name || interfaces[0]?.name || "",
    })),
  setSelectedInterface: (selectedInterface) => set({ selectedInterface }),
  setFilter: (filter) => set({ filter }),
  setStatus: (status) => set({ status }),
  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
  appendBatch: (batch) =>
    set((state) => {
      const packets = [...state.packets, ...batch.packets].slice(-10_000);
      const trafficHistory = [
        ...state.trafficHistory,
        {
          timestamp: Date.now(),
          bytesPerSecond: batch.statistics.bytesPerSecond,
          packetsPerSecond: batch.statistics.packetsPerSecond,
        },
      ].slice(-60);
      return { packets, statistics: batch.statistics, trafficHistory };
    }),
  selectPacket: (selectedPacketId) => set({ selectedPacketId }),
  clearPackets: () =>
    set({
      packets: [],
      selectedPacketId: null,
      trafficHistory: [],
      statistics: EMPTY_STATISTICS,
    }),
}));
