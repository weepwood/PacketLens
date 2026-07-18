import { useMemo, useState } from "react";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  Boxes,
  CircleDot,
  Search,
} from "lucide-react";
import type { FlowSummary, ProcessTrafficSummary } from "../types/network";

function formatBytes(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(2)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

function formatEndpoint(address: string, port: number | null): string {
  const wrapped = address.includes(":") ? `[${address}]` : address;
  return port === null ? wrapped : `${wrapped}:${port}`;
}

function formatAge(timestampMicros: number): string {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000 - timestampMicros / 1_000_000));
  if (seconds < 2) return "刚刚";
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  return `${Math.floor(minutes / 60)}h`;
}

interface ConnectionsPanelProps {
  flows: FlowSummary[];
  trackedFlowCount: number;
  capturing: boolean;
}

export function ConnectionsPanel({
  flows,
  trackedFlowCount,
  capturing,
}: ConnectionsPanelProps) {
  const [query, setQuery] = useState("");
  const [stateFilter, setStateFilter] = useState<"all" | "active" | "recent">(
    "all",
  );

  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return flows.filter((flow) => {
      if (stateFilter !== "all" && flow.state !== stateFilter) return false;
      if (!normalized) return true;
      return [
        flow.protocol,
        flow.localAddress,
        flow.remoteAddress,
        flow.localPort,
        flow.remotePort,
        flow.processId,
        flow.processName,
      ]
        .filter((value) => value !== null && value !== undefined)
        .some((value) => String(value).toLowerCase().includes(normalized));
    });
  }, [flows, query, stateFilter]);

  const activeCount = flows.filter((flow) => flow.state === "active").length;
  const upload = flows.reduce((total, flow) => total + flow.uploadBytes, 0);
  const download = flows.reduce((total, flow) => total + flow.downloadBytes, 0);

  return (
    <section className="flow-page">
      <div className="flow-summary-cards">
        <article>
          <CircleDot size={17} />
          <span>活跃连接</span>
          <strong>{activeCount.toLocaleString()}</strong>
          <small>{capturing ? "实时更新" : "最近一次捕获"}</small>
        </article>
        <article>
          <Boxes size={17} />
          <span>跟踪 Flow</span>
          <strong>{trackedFlowCount.toLocaleString()}</strong>
          <small>最多展示 {flows.length.toLocaleString()}</small>
        </article>
        <article>
          <ArrowUp size={17} />
          <span>已识别上传</span>
          <strong>{formatBytes(upload)}</strong>
          <small>按本机端点方向聚合</small>
        </article>
        <article>
          <ArrowDown size={17} />
          <span>已识别下载</span>
          <strong>{formatBytes(download)}</strong>
          <small>未知方向单独保留</small>
        </article>
      </div>

      <article className="panel flow-table-panel">
        <div className="panel-heading flow-toolbar">
          <div>
            <span>FLOW TABLE</span>
            <h2>TCP / UDP 实时连接</h2>
          </div>
          <div className="flow-filters">
            <div className="compact-search">
              <Search size={14} />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="进程、IP、端口"
              />
            </div>
            <select
              value={stateFilter}
              onChange={(event) =>
                setStateFilter(event.target.value as "all" | "active" | "recent")
              }
            >
              <option value="all">全部状态</option>
              <option value="active">活跃</option>
              <option value="recent">近期</option>
            </select>
          </div>
        </div>
        <div className="flow-grid flow-grid-header">
          <span>State</span>
          <span>Protocol</span>
          <span>Process</span>
          <span>Local</span>
          <span>Remote</span>
          <span>Upload</span>
          <span>Download</span>
          <span>Last Seen</span>
        </div>
        <div className="flow-table-body">
          {filtered.length === 0 ? (
            <div className="empty-state">暂无符合条件的连接</div>
          ) : (
            filtered.map((flow) => (
              <div className="flow-grid flow-row" key={flow.id}>
                <span>
                  <i className={`flow-state flow-state-${flow.state}`}>
                    {flow.state}
                  </i>
                </span>
                <span>
                  <i className={`protocol protocol-${flow.protocol.toLowerCase()}`}>
                    {flow.protocol}
                  </i>
                </span>
                <span title={flow.processName ?? "未识别进程"}>
                  <strong>{flow.processName ?? "未识别进程"}</strong>
                  <small>{flow.processId === null ? "PID —" : `PID ${flow.processId}`}</small>
                </span>
                <span title={formatEndpoint(flow.localAddress, flow.localPort)}>
                  {formatEndpoint(flow.localAddress, flow.localPort)}
                </span>
                <span title={formatEndpoint(flow.remoteAddress, flow.remotePort)}>
                  {formatEndpoint(flow.remoteAddress, flow.remotePort)}
                </span>
                <span>{formatBytes(flow.uploadBytes)}</span>
                <span>{formatBytes(flow.downloadBytes)}</span>
                <span>{formatAge(flow.lastSeenMicros)}</span>
              </div>
            ))
          )}
        </div>
      </article>
    </section>
  );
}

interface ProcessesPanelProps {
  processes: ProcessTrafficSummary[];
}

export function ProcessesPanel({ processes }: ProcessesPanelProps) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return processes;
    return processes.filter((process) =>
      `${process.processName} ${process.processId ?? ""}`
        .toLowerCase()
        .includes(normalized),
    );
  }, [processes, query]);

  const identified = processes.filter((process) => process.processId !== null).length;
  const activeConnections = processes.reduce(
    (total, process) => total + process.activeConnectionCount,
    0,
  );

  return (
    <section className="flow-page">
      <div className="flow-summary-cards process-summary-cards">
        <article>
          <Activity size={17} />
          <span>联网进程</span>
          <strong>{identified.toLocaleString()}</strong>
          <small>Windows IP Helper 映射</small>
        </article>
        <article>
          <CircleDot size={17} />
          <span>活跃连接</span>
          <strong>{activeConnections.toLocaleString()}</strong>
          <small>15 秒活动窗口</small>
        </article>
      </div>

      <article className="panel process-table-panel">
        <div className="panel-heading flow-toolbar">
          <div>
            <span>PROCESS TRAFFIC</span>
            <h2>按进程聚合流量</h2>
          </div>
          <div className="compact-search">
            <Search size={14} />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="进程名或 PID"
            />
          </div>
        </div>
        <div className="process-grid process-grid-header">
          <span>Process</span>
          <span>Connections</span>
          <span>Active</span>
          <span>Upload</span>
          <span>Download</span>
          <span>Unknown</span>
          <span>Packets</span>
          <span>Last Seen</span>
        </div>
        <div className="process-table-body">
          {filtered.length === 0 ? (
            <div className="empty-state">尚未识别到联网进程</div>
          ) : (
            filtered.map((process) => (
              <div
                className="process-grid process-row"
                key={`${process.processId ?? "unknown"}-${process.processName}`}
              >
                <span>
                  <strong>{process.processName}</strong>
                  <small>
                    {process.processId === null ? "PID —" : `PID ${process.processId}`}
                  </small>
                </span>
                <span>{process.connectionCount.toLocaleString()}</span>
                <span>{process.activeConnectionCount.toLocaleString()}</span>
                <span className="traffic-up">{formatBytes(process.uploadBytes)}</span>
                <span className="traffic-down">{formatBytes(process.downloadBytes)}</span>
                <span>{formatBytes(process.unknownBytes)}</span>
                <span>
                  {(
                    process.uploadPackets +
                    process.downloadPackets +
                    process.unknownPackets
                  ).toLocaleString()}
                </span>
                <span>{formatAge(process.lastSeenMicros)}</span>
              </div>
            ))
          )}
        </div>
      </article>
    </section>
  );
}
