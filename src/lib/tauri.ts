import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CaptureRequest,
  CaptureSessionSummary,
  CaptureStatus,
  NetworkInterface,
  PacketBatch,
  StoredPacketSummary,
} from "../types/network";

export const networkApi = {
  listInterfaces: () => invoke<NetworkInterface[]>("list_interfaces"),
  getCaptureStatus: () => invoke<CaptureStatus>("get_capture_status"),
  startCapture: (request: CaptureRequest) =>
    invoke<CaptureStatus>("start_capture", { request }),
  stopCapture: () => invoke<CaptureStatus>("stop_capture"),
  listCaptureSessions: (limit = 100, offset = 0) =>
    invoke<CaptureSessionSummary[]>("list_capture_sessions", { limit, offset }),
  listSessionPackets: (sessionId: string, limit = 250, offset = 0) =>
    invoke<StoredPacketSummary[]>("list_session_packets", {
      sessionId,
      limit,
      offset,
    }),
  deleteCaptureSession: (sessionId: string) =>
    invoke<void>("delete_capture_session", { sessionId }),
  onPacketBatch: (handler: (batch: PacketBatch) => void): Promise<UnlistenFn> =>
    listen<PacketBatch>("packet-batch", (event) => handler(event.payload)),
};
