# Capture storage

PacketLens 0.2 将实时窗口和长期捕获数据分开处理：

- React 只保留最近 10,000 条摘要。
- Rust 使用独立有界队列写入原始数据。
- 原始数据写入标准 pcapng 文件。
- SQLite 只保存会话元数据和数据包索引，不重复保存 payload。

## 数据位置

数据保存在 Tauri 的应用数据目录中：

```text
<app-data>/
├─ packetlens.sqlite3
└─ captures/
   └─ <session-id>/
      ├─ segment-0000.pcapng
      ├─ segment-0001.pcapng
      └─ ...
```

Windows 默认位于当前用户的应用数据目录下，实际路径可以在“捕获历史”页面查看。

## SQLite 模型

`capture_sessions` 保存：

- 会话 ID、开始和结束时间
- 网卡与 BPF 过滤器
- 状态、包数、字节数和存储丢弃数
- pcapng 分段数量与会话目录
- 异常退出或写入失败信息

`packet_index` 保存：

- 会话 ID 与包序号
- 时间戳、协议和五元组
- PID 与进程名称
- pcapng 分段编号和文件偏移
- 捕获长度、原始长度与摘要

## 写入和恢复策略

- 存储队列容量为 4,096；过载时不会阻塞抓包线程，而是增加独立的存储丢弃计数。
- 索引每 256 条或 500ms 批量提交。
- pcapng 每 512 MiB 或 30 分钟滚动为新分段。
- 默认磁盘配额为 10 GiB；超限时从最旧的非运行会话开始清理，直到回落到约 90%。
- 应用再次开始捕获时，未正常结束的 `running` 会话会标记为 `interrupted`。
- pcapng 使用 Section Header、Interface Description 和 Enhanced Packet Block，分段文件可由 Wireshark 打开。

## 当前边界

- 当前 pcapng 接口类型固定为 Ethernet（LINKTYPE_ETHERNET）。
- 数据包详情仍只提供摘要；按文件偏移读取协议树和 Hex 内容属于后续任务。
- SQLite 与 pcapng 的提交不是跨文件事务。异常退出时，pcapng 可能包含少量尚未写入索引的尾部数据，但已完成的块仍然可恢复。
- 默认配额和滚动参数目前是编译期常量，后续会进入设置页面。
