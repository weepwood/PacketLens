# AGENTS.md

本文件是 AI Agent 在 PacketLens 仓库中工作的强制入口。PacketLens 是 Windows 优先的实时网络观察与数据包分析工具，使用 Rust、Tauri 2、React、Npcap、Windows IP Helper、pcapng 与 SQLite。抓包数据可能包含 IP、域名、Cookie、Token 和业务内容，任何修改都必须优先考虑授权、隐私、数据上限和真实 Windows 行为。

## 1. 修改前必读

按顺序读取：

1. 当前 Issue、PR 描述、验收标准和明确非目标；
2. `README.md`、`ROADMAP.md`、`CONTRIBUTING.md` 和 `SECURITY.md`；
3. `docs/ARCHITECTURE.md`、`docs/STORAGE.md`、`docs/FLOW-TRACKING.md`、`docs/WINDOWS-DEVELOPMENT.md`；
4. 本文件；
5. 相关 Rust、Tauri、React、SQLite、pcapng 和测试实现。

涉及 Npcap、ETW、进程归属、协议解析、存储格式或会话删除时，先确认现有边界和未完成 Roadmap，不得把近似结果表述为可靠审计证据。

## 2. 核心架构不变量

- React 不接触原始抓包缓冲区；
- 不为每个数据包触发一次 IPC；
- 实时摘要、存储队列、Flow Tracker 和前端列表必须有明确上限；
- 无法可靠识别方向的流量保留为 `unknown`，不得猜测为上传或下载；
- SQLite 只保存会话和数据包索引，不重复保存完整 payload；
- 原始数据写入标准 pcapng，保持可由 Wireshark 等工具读取；
- Npcap 通过运行时动态加载，不把 Npcap SDK `.lib` 或安装程序直接提交到仓库；
- 真实 Windows + Npcap 行为不能仅由 Linux CI、Mock 或静态分析证明。

## 3. 风险等级

- **P0**：未授权抓包、敏感数据泄露、任意文件访问、捕获文件损坏、无限磁盘占用；
- **P1**：Npcap/ETW、抓包线程、BPF、pcapng、SQLite 迁移、会话删除、磁盘配额、进程归属；
- **P2**：协议解析、Flow、批量 IPC、性能、Windows 兼容和大型 UI 改造；
- **P3**：低风险 UI、文档、测试与内部整理。

P0/P1 必须有关联 Issue、失败路径测试、独立审查和可执行回滚。

## 4. 强制工作流

### 4.1 调查

先只读并确认：

- Npcap 抓包线程、解析、摘要、存储和 Flow 调用链；
- Windows PID/方向映射的数据来源与可靠性；
- pcapng 分段、SQLite 索引、文件偏移和会话生命周期；
- 队列、批次、时间窗口、记录数和磁盘配额；
- 前端虚拟列表、Tauri 事件和状态管理影响；
- 已确认事实、推断、未知项与最小修改范围。

### 4.2 实现

- 一个 PR 只解决一个主要问题；
- 不混合功能、无关重构、依赖升级和全局格式化；
- 优先复用现有抓包、解析、Flow、存储和 UI 管线；
- 不建立第二套会话库、payload 存储、进程映射或事件通道；
- 不为未确认的未来协议提前扩张抽象；
- 提交说明使用中文；
- 不直接推送 `main`，默认 squash merge。

### 4.3 测试先行

以下改动优先先写失败测试：

- 包长度、截断、畸形头、IPv4/IPv6、TCP/UDP/ICMP/ARP 解析；
- BPF 过滤、Npcap 加载失败和网卡变化；
- 有界队列、背压、丢包统计和批量刷新；
- Flow 过期、容量、方向、PID 复用和无法归属场景；
- pcapng 分段、文件偏移、异常中断、会话恢复和分页；
- SQLite 事务、配额清理和删除失败恢复；
- 路径、权限、文件删除和敏感信息输出；
- 前端大列表、排序、筛选和错误状态。

协议解析依赖升级必须增加与变更协议相关的回归测试，不得只依赖 Dependabot 兼容分数。

## 5. 隐私与授权

- 只帮助分析用户拥有或已获授权的设备和网络；
- 不增加 HTTPS 中间人代理、TLS 自动解密、凭据提取或数据外传能力；
- 抓包文件、截图、测试夹具、Issue、PR 和 Artifact 不得包含真实 Cookie、Token、账号或业务 payload；
- 默认不上传捕获数据；
- 日志和错误不得输出完整 payload、敏感请求头或本地隐私路径；
- 示例数据必须人工构造或充分脱敏；
- 涉及数据导出或分享时必须明确用户确认、范围和风险。

## 6. 实时与性能边界

- 不逐包刷新 React；
- IPC、SQLite 写入和 FlowSnapshot 必须批量、限频并可观测；
- 队列满时应记录丢弃和背压，不得无限扩容；
- Flow、前端摘要、捕获分段和磁盘占用必须保持上限；
- 性能优化必须给出数据规模、采样方法和前后对比；
- 不用“本机看起来流畅”替代高包速率和长时间捕获验证。

## 7. 存储与删除

- pcapng 与 SQLite 索引必须保持一致或能够识别异常会话；
- 文件滚动、索引批量提交和异常退出不得破坏已完成分段；
- 删除会话时同时处理索引和对应 pcapng 目录；
- 删除失败必须返回可恢复状态，不得静默留下部分删除；
- 配额清理只删除符合策略的最旧非运行会话；
- 不对正在捕获的会话执行清理或覆盖；
- 路径必须由受控会话 ID 派生，拒绝任意路径输入。

## 8. Windows 与依赖

- Npcap、IP Helper、ETW 和进程归属必须在真实 Windows 环境验证；
- UDP 端口复用、短连接和 PID 生命周期存在不确定性时必须保留说明；
- 不将进程映射直接宣传为法律或安全审计证据；
- Npcap 许可证、安装和再分发边界必须保持清晰；
- Rust 抓包与解析依赖的 minor/major 升级需要阅读上游变更并人工审查；
- 不因开发便利提交系统 DLL、抓包驱动、真实 pcap 或第三方安装程序。

## 9. 完成验证

完成前必须提供实际证据：

```powershell
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

并根据改动补充：

- 真实 Windows + Npcap 抓包验证；
- 长时间或高包速率下的队列、内存、磁盘和 UI 验证；
- pcapng 可被 Wireshark 打开；
- 异常中断、配额清理和会话删除验证；
- 未验证平台行为和剩余风险。

CI 通过不能替代真实驱动、网卡、Windows API 和现场网络验证。