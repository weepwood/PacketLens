# PacketLens Roadmap

## v0.1 — Windows 抓包闭环

- [x] Tauri 2 + React 工程
- [x] Npcap 动态检测与网卡枚举
- [x] 启动、停止和 BPF 捕获过滤
- [x] Ethernet / IPv4 / IPv6 / TCP / UDP / ICMP 摘要
- [x] 100ms 批量 IPC
- [x] 虚拟数据包列表
- [x] IPv4 TCP 五元组到 PID 的基础映射
- [ ] pcapng 原始数据写入
- [ ] 单包协议树与 Hex 查看器

## v0.2 — Flow 与进程视角

- [ ] TCP/UDP Flow Table
- [ ] IPv6 TCP/UDP PID 映射
- [ ] ETW 捕获短连接和进程生命周期
- [ ] 进程流量统计
- [ ] 连接页面与进程页面
- [ ] DNS 请求、响应和域名-IP 缓存
- [ ] TCP RTT、重传和 Reset 分析

## v0.3 — 应用层协议

- [ ] DNS
- [ ] DHCP
- [ ] HTTP/1.1
- [ ] TLS ClientHello / ServerHello
- [ ] SNI、ALPN、证书和握手耗时
- [ ] TCP 流重组
- [ ] Follow TCP Stream

## v0.4 — 持久化和诊断

- [ ] pcapng + SQLite 索引
- [ ] 捕获会话管理
- [ ] 大文件滚动与磁盘配额
- [ ] 显示过滤器表达式
- [ ] DNS 超时、SYN 超时和 TLS 失败诊断
- [ ] 隐私脱敏导出

## v1.0 — 可发布 Windows 工具

- [ ] 特权 Capture Service + 普通权限 UI
- [ ] NSIS/MSI 安装包
- [ ] 代码签名
- [ ] 自动更新
- [ ] 崩溃报告（默认不上传抓包数据）
- [ ] 性能压测和模糊测试
- [ ] 中英文界面
