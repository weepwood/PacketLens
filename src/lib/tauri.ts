import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CaptureRequest,
  CaptureStatus,
  NetworkInterface,
  PacketBatch,
} from "../types/network";

export const networkApi = {
  listInterfaces: () => invoke<NetworkInterface[]>("list_interfaces"),
  getCaptureStatus: () => invoke<CaptureStatus>("get_capture_status"),
  startCapture: (request: CaptureRequest) =>
    invoke<CaptureStatus>("start_capture", { request }),
  stopCapture: () => invoke<CaptureStatus>("stop_capture"),
  onPacketBatch: (handler: (batch: PacketBatch) => void): Promise<UnlistenFn> =>
    listen<PacketBatch>("packet-batch", (event) => handler(event.payload)),
};
