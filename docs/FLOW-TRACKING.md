# Flow and process tracking

PacketLens 0.3 在数据包摘要之上增加了连接与进程视角。

## FlowKey

一个 Flow 由以下字段构成：

- 协议：TCP、UDP 或 DNS
- 本机 IP 和端口
- 远端 IP 和端口
- PID（能够识别时）

Windows 进程归属来自 IP Helper API：

- `GetExtendedTcpTable`
- `GetExtendedUdpTable`
- IPv4 与 IPv6 owner-PID 表

当数据包与 owner 表匹配时，PacketLens 同时得到进程和方向：

- source 端点属于本机进程：`outbound`
- destination 端点属于本机进程：`inbound`
- 无法可靠匹配：`unknown`

未知方向不会被猜测成上传或下载，而是单独累积。

## 生命周期

- 最近 15 秒有数据包的 Flow 标记为 `active`。
- 15 秒后保留为 `recent`。
- 5 分钟无数据后从内存中清理。
- 最多跟踪 20,000 个 Flow；超限时清理最旧数据至约 90%。
- 单次发给前端的快照最多包含最近 5,000 个 Flow。
- 快照每 500ms 批量发送，不逐包更新 React。

## 进程流量

Flow 按 PID 和进程名聚合：

- 连接数、活跃连接数
- 上传/下载/未知方向字节数
- 上传/下载/未知方向包数
- 最近活动时间

进程名称查询会在每次 owner 表刷新中按 PID 缓存，避免同一轮为每条连接重复打开进程句柄。

## 当前边界

IP Helper 表是周期性快照。生命周期极短、在刷新间隔内创建并消失的连接仍可能没有 PID。这部分需要 ETW 网络事件补充，Issue #3 保持开放直到 ETW 短连接追踪与 PID 生命周期处理完成。

UDP 端点可能使用通配地址或端口复用。PacketLens 会尝试匹配 `0.0.0.0` / `::` 监听端点，但在多个进程共享同一 UDP 端口时不能保证唯一归属，因此应把结果视为诊断线索，而不是审计证据。
