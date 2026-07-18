import { useEffect, useMemo, useState } from "react";
import {
  ChevronLeft,
  ChevronRight,
  Database,
  FolderOpen,
  HardDrive,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { networkApi } from "../lib/tauri";
import type {
  CaptureSessionSummary,
  StoredPacketSummary,
} from "../types/network";

const PAGE_SIZE = 250;

function formatBytes(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(2)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

function formatDate(timestamp: number | null): string {
  if (timestamp === null) return "仍在运行";
  return new Date(timestamp).toLocaleString("zh-CN", { hour12: false });
}

function formatDuration(session: CaptureSessionSummary): string {
  const end = session.endedAtMs ?? Date.now();
  const seconds = Math.max(0, Math.floor((end - session.startedAtMs) / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remaining = seconds % 60;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${remaining}s`;
  return `${remaining}s`;
}

function protocolClass(protocol: string): string {
  return `protocol protocol-${protocol.toLowerCase()}`;
}

export function SessionsPanel() {
  const [sessions, setSessions] = useState<CaptureSessionSummary[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [packets, setPackets] = useState<StoredPacketSummary[]>([]);
  const [packetOffset, setPacketOffset] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedSession = useMemo(
    () => sessions.find((session) => session.id === selectedSessionId) ?? null,
    [selectedSessionId, sessions],
  );

  const loadPackets = async (sessionId: string, offset: number) => {
    const result = await networkApi.listSessionPackets(
      sessionId,
      PAGE_SIZE,
      offset,
    );
    setPackets(result);
    setPacketOffset(offset);
  };

  const refresh = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await networkApi.listCaptureSessions();
      setSessions(result);
      const nextSelected =
        result.find((session) => session.id === selectedSessionId)?.id ??
        result[0]?.id ??
        null;
      setSelectedSessionId(nextSelected);
      if (nextSelected) {
        await loadPackets(nextSelected, 0);
      } else {
        setPackets([]);
        setPacketOffset(0);
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const selectSession = async (sessionId: string) => {
    setSelectedSessionId(sessionId);
    setLoading(true);
    setError(null);
    try {
      await loadPackets(sessionId, 0);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  };

  const changePage = async (offset: number) => {
    if (!selectedSessionId) return;
    setLoading(true);
    setError(null);
    try {
      await loadPackets(selectedSessionId, Math.max(0, offset));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  };

  const removeSession = async (session: CaptureSessionSummary) => {
    if (
      session.status === "running" ||
      !window.confirm(`删除捕获会话 ${session.id} 及其全部 pcapng 文件？`)
    ) {
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await networkApi.deleteCaptureSession(session.id);
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setLoading(false);
    }
  };

  return (
    <section className="sessions-layout">
      <article className="panel sessions-list-panel">
        <div className="panel-heading">
          <div>
            <span>CAPTURE SESSIONS</span>
            <h2>本地捕获会话</h2>
          </div>
          <button
            className="icon-button"
            onClick={() => void refresh()}
            disabled={loading}
            title="刷新捕获历史"
          >
            <RefreshCw size={15} />
          </button>
        </div>

        {error ? <div className="sessions-error">{error}</div> : null}
        <div className="session-list">
          {sessions.length === 0 ? (
            <div className="session-empty">
              <Database size={25} />
              <span>还没有本地捕获会话</span>
            </div>
          ) : (
            sessions.map((session) => (
              <button
                key={session.id}
                className={`session-card ${
                  selectedSessionId === session.id ? "selected" : ""
                }`}
                onClick={() => void selectSession(session.id)}
              >
                <div className="session-card-header">
                  <span className={`session-state state-${session.status}`}>
                    {session.status}
                  </span>
                  <time>{formatDate(session.startedAtMs)}</time>
                </div>
                <strong>{session.deviceName}</strong>
                <small>{session.filter || "无捕获过滤器"}</small>
                <div className="session-metrics">
                  <span>{session.packetCount.toLocaleString()} packets</span>
                  <span>{formatBytes(session.byteCount)}</span>
                  <span>{formatDuration(session)}</span>
                </div>
              </button>
            ))
          )}
        </div>
      </article>

      <article className="panel session-detail-panel">
        {selectedSession ? (
          <>
            <div className="panel-heading session-detail-heading">
              <div>
                <span>SESSION DETAIL</span>
                <h2>{selectedSession.id}</h2>
              </div>
              <button
                className="icon-button danger-button"
                disabled={loading || selectedSession.status === "running"}
                onClick={() => void removeSession(selectedSession)}
                title="删除会话"
              >
                <Trash2 size={15} />
              </button>
            </div>

            <div className="session-summary-grid">
              <div>
                <HardDrive size={15} />
                <span>原始数据</span>
                <strong>{formatBytes(selectedSession.byteCount)}</strong>
              </div>
              <div>
                <Database size={15} />
                <span>索引包数</span>
                <strong>{selectedSession.packetCount.toLocaleString()}</strong>
              </div>
              <div>
                <FolderOpen size={15} />
                <span>pcapng 分段</span>
                <strong>{selectedSession.segmentCount}</strong>
              </div>
              <div>
                <RefreshCw size={15} />
                <span>存储丢弃</span>
                <strong>{selectedSession.storageDropped.toLocaleString()}</strong>
              </div>
            </div>

            <dl className="session-metadata">
              <div>
                <dt>开始</dt>
                <dd>{formatDate(selectedSession.startedAtMs)}</dd>
              </div>
              <div>
                <dt>结束</dt>
                <dd>{formatDate(selectedSession.endedAtMs)}</dd>
              </div>
              <div>
                <dt>目录</dt>
                <dd title={selectedSession.directoryPath}>
                  {selectedSession.directoryPath}
                </dd>
              </div>
              {selectedSession.lastError ? (
                <div className="metadata-error">
                  <dt>错误</dt>
                  <dd>{selectedSession.lastError}</dd>
                </div>
              ) : null}
            </dl>

            <div className="stored-packet-table">
              <div className="stored-packet-header stored-packet-grid">
                <span>No.</span>
                <span>Protocol</span>
                <span>Source</span>
                <span>Destination</span>
                <span>Length</span>
                <span>Segment / Offset</span>
                <span>Info</span>
              </div>
              <div className="stored-packet-body">
                {packets.length === 0 ? (
                  <div className="empty-state">该页没有数据包索引</div>
                ) : (
                  packets.map((packet) => (
                    <div className="stored-packet-grid stored-packet-row" key={packet.id}>
                      <span>{packet.id}</span>
                      <span>
                        <i className={protocolClass(packet.protocol)}>
                          {packet.protocol}
                        </i>
                      </span>
                      <span>
                        {packet.source}:{packet.sourcePort ?? "*"}
                      </span>
                      <span>
                        {packet.destination}:{packet.destinationPort ?? "*"}
                      </span>
                      <span>{packet.length}</span>
                      <span>
                        {packet.segmentIndex.toString().padStart(4, "0")} / {packet.fileOffset}
                      </span>
                      <span title={packet.info}>{packet.info}</span>
                    </div>
                  ))
                )}
              </div>
            </div>

            <div className="session-pagination">
              <span>
                {packetOffset + 1}–
                {Math.min(packetOffset + packets.length, selectedSession.packetCount)} /{" "}
                {selectedSession.packetCount.toLocaleString()}
              </span>
              <div>
                <button
                  className="icon-button"
                  disabled={loading || packetOffset === 0}
                  onClick={() => void changePage(packetOffset - PAGE_SIZE)}
                >
                  <ChevronLeft size={15} />
                </button>
                <button
                  className="icon-button"
                  disabled={
                    loading || packetOffset + packets.length >= selectedSession.packetCount
                  }
                  onClick={() => void changePage(packetOffset + PAGE_SIZE)}
                >
                  <ChevronRight size={15} />
                </button>
              </div>
            </div>
          </>
        ) : (
          <div className="session-empty detail-empty">
            <Database size={30} />
            <span>选择一个捕获会话查看索引</span>
          </div>
        )}
      </article>
    </section>
  );
}
