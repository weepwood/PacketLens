# Architecture

## 1. 目标

PacketLens 不是简单复制 Wireshark，而是将四种视角统一：

- Packet：单个数据包
- Flow：连接和会话
- Process：发起网络活动的程序
- Diagnostic：延迟、重传和失败原因

## 2. 当前数据通路

```text
wpcap.dll
  │ pcap_next_ex（200ms timeout）
  ▼
packetlens-capture thread
  │ etherparse + ProcessResolver
  ▼
sync_channel<PacketSummary>(8192)
  │ 满载时丢弃摘要并累计 droppedPackets
  ▼
packetlens-batch thread
  │ 每 100ms 最多合并 2048 条
  ▼
Tauri event: packet-batch
  ▼
Zustand store
  │ 最近 10,000 条
  ▼
TanStack Virtual
```

原始数据包不会通过 Tauri IPC 发送到 React。

## 3. 为什么动态加载 Npcap

直接链接 Npcap SDK 会要求开发者和 CI 配置 `Packet.lib`、`wpcap.lib` 和 SDK 路径。PacketLens 使用 `libloading` 在运行时加载：

1. Windows DLL 搜索路径中的 `wpcap.dll`
2. `%SystemRoot%\System32\Npcap\wpcap.dll`

这样：

- 编译和静态检查不依赖 Npcap SDK；
- 用户仍需自行安装 Npcap；
- 仓库不重新分发 Npcap 二进制；
- 缺少驱动时能返回可读错误，而不是程序无法启动。

## 4. 背压

高流量环境中，采集速度可能超过 UI 消费速度。当前策略：

- 队列容量固定为 8192；
- 队列满时保留捕获线程实时性，丢弃 UI 摘要；
- `droppedPackets` 明确显示；
- 前端窗口最多 10,000 条；
- 后续 pcapng 写入使用独立有界队列。

这里的 dropped 目前是 PacketLens 应用层队列丢弃，不等同于 Npcap 内核丢包。后续会分别展示两种指标。

## 5. 进程关联

当前 Windows IPv4 TCP 映射使用 `GetExtendedTcpTable(TCP_TABLE_OWNER_PID_ALL)`：

```text
local IP + local port + remote IP + remote port
                         ↓
                        PID
                         ↓
           QueryFullProcessImageNameW
```

系统表每 750ms 刷新一次。短连接可能在轮询间隔内消失，因此 v0.2 会引入 ETW。

## 6. 下一阶段模块

```text
capture
├── npcap
├── pcapng_writer
└── capture_statistics

flow
├── flow_table
├── tcp_state
└── stream_reassembly

protocol
├── dns
├── http1
├── tls
└── registry

windows
├── ip_helper
├── etw
└── process_metadata

storage
├── pcapng
├── sqlite_index
└── retention
```

## 7. 权限演进

早期版本可由用户以管理员权限启动。发布版本将拆成：

```text
PacketLens.exe（普通权限）
       │ 受限 Named Pipe
       ▼
PacketLens.CaptureService.exe（最小必要权限）
       │
       ▼
Npcap
```
