import { useEffect, useMemo, useState } from "react";
import {
  Activity,
  Boxes,
  CircleAlert,
  Database,
  EthernetPort,
  Gauge,
  Globe2,
  LockKeyhole,
  Network,
  Pause,
  Play,
  Radio,
  RotateCcw,
  Search,
  Settings,
  ShieldCheck,
  Square,
} from "lucide-react";
import { ConnectionsPanel, ProcessesPanel } from "./components/FlowPanels";
import { PacketTable } from "./components/PacketTable";
import { SessionsPanel } from "./components/SessionsPanel";
import { TrafficChart } from "./components/TrafficChart";
import { networkApi } from "./lib/tauri";
import { useCaptureStore } from "./store/capture-store";

const navigation = [
  { id: "dashboard", label: "实时总览", icon: Gauge },
  { id: "packets", label: "数据包", icon: Boxes },
  { id: "connections", label: "网络连接", icon: Network },
  { id: "processes", label: "联网进程", icon: Activity },
  { id: "dns", label: "DNS 查询", icon: Globe2 },
  { id: "tls", label: "TLS 会话", icon: LockKeyhole },
  { id: "diagnostics", label: "网络诊断", icon: ShieldCheck },
  { id: "sessions", label: "捕获历史", icon: Database },
  { id: "settings", label: "系统设置", icon: Settings },
] as const;

function formatRate(bytes: number): string {
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(2)} MB/s`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB/s`;
  return `${bytes} B/s`;
}

function formatBytes(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(2)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

function App() {
  const [page, setPage] = useState<(typeof navigation)[number]["id"]>("dashboard");
  const store = useCaptureStore();

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    const initialize = async () => {
      store.setLoading(true);
      try {
        const [interfaces, status, flowSnapshot] = await Promise.all([
          networkApi.listInterfaces(),
          networkApi.getCaptureStatus(),
          networkApi.getFlowSnapshot(),
        ]);
        store.setInterfaces(interfaces);
        store.setStatus(status);
        store.setFlowSnapshot(flowSnapshot);
        unlisteners.push(await networkApi.onPacketBatch(store.appendBatch));
        unlisteners.push(await networkApi.onFlowSnapshot(store.setFlowSnapshot));
      } catch (error) {
        store.setError(error instanceof Error ? error.message : String(error));
      } finally {
        store.setLoading(false);
      }
    };
    void initialize();
    return () => unlisteners.forEach((unlisten) => unlisten());
  }, []);

  const selectedPacket = useMemo(
    () => store.packets.find((packet) => packet.id === store.selectedPacketId) ?? null,
    [store.packets, store.selectedPacketId],
  );

  const startCapture = async () => {
    if (!store.selectedInterface) return;
    store.setLoading(true);
    store.setError(null);
    try {
      const status = await networkApi.startCapture({
        deviceName: store.selectedInterface,
        filter: store.filter.trim() || null,
        promiscuous: true,
        snapshotLength: 65_535,
      });
      store.setStatus(status);
      store.setFlowSnapshot(await networkApi.getFlowSnapshot());
    } catch (error) {
      store.setError(error instanceof Error ? error.message : String(error));
    } finally {
      store.setLoading(false);
    }
  };

  const stopCapture = async () => {
    store.setLoading(true);
    try {
      store.setStatus(await networkApi.stopCapture());
      store.setFlowSnapshot(await networkApi.getFlowSnapshot());
    } catch (error) {
      store.setError(error instanceof Error ? error.message : String(error));
    } finally {
      store.setLoading(false);
    }
  };

  const pageLabel = navigation.find((item) => item.id === page)?.label ?? "PacketLens";

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">
            <Radio size={22} />
          </div>
          <div>
            <strong>PacketLens</strong>
            <span>Network Inspector</span>
          </div>
        </div>
        <nav>
          {navigation.map((item) => {
            const Icon = item.icon;
            return (
              <button
                key={item.id}
                className={page === item.id ? "active" : ""}
                onClick={() => setPage(item.id)}
              >
                <Icon size={17} />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>
        <div className="sidebar-status">
          <span
            className={`status-dot ${store.status.npcapAvailable ? "online" : "offline"}`}
          />
          <div>
            <strong>Npcap</strong>
            <span>{store.status.npcapAvailable ? "驱动可用" : "未检测到"}</span>
          </div>
        </div>
      </aside>

      <main className="workspace">
        <header className="topbar">
          <div>
            <p>PACKETLENS / {page.toUpperCase()}</p>
            <h1>{pageLabel}</h1>
          </div>
          <div className="capture-controls">
            <select
              value={store.selectedInterface}
              onChange={(event) => store.setSelectedInterface(event.target.value)}
              disabled={store.status.running}
            >
              {store.interfaces.length === 0 ? (
                <option value="">未发现网络接口</option>
              ) : null}
              {store.interfaces.map((item) => (
                <option key={item.name} value={item.name}>
                  {item.description || item.name}
                </option>
              ))}
            </select>
            <div className="filter-input">
              <Search size={15} />
              <input
                value={store.filter}
                onChange={(event) => store.setFilter(event.target.value)}
                placeholder="BPF: tcp port 443"
                disabled={store.status.running}
              />
            </div>
            {store.status.running ? (
              <button
                className="capture-button stop"
                onClick={stopCapture}
                disabled={store.loading}
              >
                <Square size={15} fill="currentColor" />停止
              </button>
            ) : (
              <button
                className="capture-button"
                onClick={startCapture}
                disabled={
                  store.loading ||
                  !store.selectedInterface ||
                  !store.status.npcapAvailable
                }
              >
                <Play size={16} fill="currentColor" />开始捕获
              </button>
            )}
          </div>
        </header>

        {store.error || store.status.lastError ? (
          <div className="error-banner">
            <CircleAlert size={17} />
            <span>{store.error || store.status.lastError}</span>
          </div>
        ) : null}

        {page === "dashboard" || page === "packets" ? (
          <>
            <section className="stats-grid">
              <article>
                <div className="stat-icon">
                  <EthernetPort size={19} />
                </div>
                <span>实时吞吐量</span>
                <strong>{formatRate(store.statistics.bytesPerSecond)}</strong>
                <small>{formatBytes(store.statistics.capturedBytes)} 累计</small>
              </article>
              <article>
                <div className="stat-icon">
                  <Activity size={19} />
                </div>
                <span>数据包速率</span>
                <strong>{store.statistics.packetsPerSecond.toLocaleString()} pps</strong>
                <small>{store.statistics.capturedPackets.toLocaleString()} packets</small>
              </article>
              <article>
                <div className="stat-icon">
                  <ShieldCheck size={19} />
                </div>
                <span>捕获状态</span>
                <strong>{store.status.running ? "Capturing" : "Idle"}</strong>
                <small>{store.status.sessionId ?? store.status.deviceName ?? "等待选择网卡"}</small>
              </article>
              <article>
                <div className="stat-icon warning">
                  <CircleAlert size={19} />
                </div>
                <span>背压丢弃</span>
                <strong>
                  {store.statistics.droppedPackets.toLocaleString()} /{" "}
                  {store.statistics.storageDroppedPackets.toLocaleString()}
                </strong>
                <small>界面队列 / 存储队列</small>
              </article>
            </section>

            <section className="overview-grid">
              <article className="panel chart-panel">
                <div className="panel-heading">
                  <div>
                    <span>LIVE TRAFFIC</span>
                    <h2>实时网络吞吐量</h2>
                  </div>
                  <div className="live-pill">
                    <i />100ms 批量刷新
                  </div>
                </div>
                <TrafficChart points={store.trafficHistory} />
              </article>
              <article className="panel capture-panel">
                <div className="panel-heading">
                  <div>
                    <span>CAPTURE ENGINE</span>
                    <h2>捕获引擎</h2>
                  </div>
                </div>
                <dl>
                  <div>
                    <dt>驱动</dt>
                    <dd>Npcap / wpcap.dll</dd>
                  </div>
                  <div>
                    <dt>接口</dt>
                    <dd>{store.status.deviceName ?? "未启动"}</dd>
                  </div>
                  <div>
                    <dt>过滤器</dt>
                    <dd>{store.filter || "无"}</dd>
                  </div>
                  <div>
                    <dt>Flow</dt>
                    <dd>{store.trackedFlowCount.toLocaleString()} tracked</dd>
                  </div>
                  <div>
                    <dt>本地会话</dt>
                    <dd>{store.status.sessionId ?? "未创建"}</dd>
                  </div>
                </dl>
                <button className="secondary-button" onClick={store.clearPackets}>
                  <RotateCcw size={15} />清空当前窗口
                </button>
              </article>
            </section>

            <section className="panel packets-panel">
              <div className="panel-heading">
                <div>
                  <span>PACKET STREAM</span>
                  <h2>实时数据包</h2>
                </div>
                <div className="table-actions">
                  <span>{store.packets.length.toLocaleString()} / 10,000</span>
                  <button title="暂停界面刷新（后续实现）">
                    <Pause size={15} />
                  </button>
                </div>
              </div>
              <PacketTable
                packets={store.packets}
                selectedPacketId={store.selectedPacketId}
                onSelect={store.selectPacket}
              />
              {selectedPacket ? (
                <div className="packet-inspector">
                  <div>
                    <span>选中数据包 #{selectedPacket.id}</span>
                    <strong>
                      {selectedPacket.protocol} · {selectedPacket.info}
                    </strong>
                  </div>
                  <code>
                    {selectedPacket.direction} · {selectedPacket.source}:
                    {selectedPacket.sourcePort ?? "*"} → {selectedPacket.destination}:
                    {selectedPacket.destinationPort ?? "*"}
                  </code>
                </div>
              ) : null}
            </section>
          </>
        ) : page === "connections" ? (
          <ConnectionsPanel
            flows={store.flows}
            trackedFlowCount={store.trackedFlowCount}
            capturing={store.status.running}
          />
        ) : page === "processes" ? (
          <ProcessesPanel processes={store.processes} />
        ) : page === "sessions" ? (
          <SessionsPanel />
        ) : (
          <section className="panel module-placeholder">
            <div className="placeholder-icon">
              <Network size={28} />
            </div>
            <h2>{pageLabel}</h2>
            <p>该模块将在后续里程碑接入 DNS、TLS 或诊断数据。</p>
          </section>
        )}
      </main>
    </div>
  );
}

export default App;
