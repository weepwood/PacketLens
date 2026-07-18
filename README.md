# PacketLens

PacketLens 是一款 **Windows 优先**的实时网络观察与数据包分析工具，使用 **Rust + Tauri 2 + React** 构建。

它把传统抓包工具的“数据包视角”和任务管理器式的“进程视角”结合起来，用于回答：

- 当前哪些程序正在联网？
- 程序连接了哪些 IP、域名和端口？
- 实时上传、下载和数据包速率是多少？
- 捕获数据如何保存、回放和分页查询？
- 哪些连接存在超时、重传或失败？

> 当前版本为 **PacketLens 0.3**：已完成 Windows 实时抓包、本地持久化、Flow Table 和按进程流量聚合。ETW 短连接补全、完整协议树和应用层诊断仍按 Roadmap 推进。

## 技术栈

- Windows 10/11 x64
- Rust
- Tauri 2
- React 19 + TypeScript + Vite
- Npcap（运行时动态加载 `wpcap.dll`）
- Windows IP Helper API
- etherparse
- rusqlite + SQLite
- Zustand
- TanStack Virtual
- CSS Variables（不使用 TailwindCSS）

## 已落地能力

### 实时捕获

- Windows Npcap 安装状态检测与网卡枚举
- 启动、停止捕获和 BPF 捕获过滤器
- Ethernet / ARP / IPv4 / IPv6 / TCP / UDP / ICMP 基础解析
- Rust 后端每 100ms 批量推送摘要，避免逐包刷新 React
- 最多保留最近 10,000 条摘要的虚拟数据包列表
- 实时吞吐量、PPS、累计流量和界面/存储背压丢包统计

### Flow 与进程视角

- TCP / UDP / DNS 统一 Flow Table
- IPv4 / IPv6 TCP、UDP owner-PID 映射
- 入站、出站与未知方向流量分别统计
- 15 秒活跃窗口和 5 分钟近期连接保留
- 最多跟踪 20,000 个 Flow，前端最多接收最近 5,000 条
- 每 500ms 批量发送连接快照
- “网络连接”页面：进程、端点、协议、状态和方向流量
- “联网进程”页面：连接数、活跃连接、上传、下载和未知流量排行

### 本地持久化

- 原始数据写入标准 pcapng 文件
- SQLite 保存捕获会话、数据包索引、协议、五元组、PID 和文件偏移
- 捕获历史、分页浏览、会话删除和异常中断标记
- 每 256 条或 500ms 批量提交索引
- pcapng 每 512 MiB 或 30 分钟滚动
- 10 GiB 默认磁盘配额和最旧非运行会话清理

### 工程保障

- 前端 TypeScript 检查与 Vite 构建
- Windows Rust fmt、Clippy `-D warnings` 和单元测试
- Dependabot、CODEOWNERS、Issue/PR 模板
- 架构、存储、Flow、安全和 Windows 开发文档

## Windows 开发环境

### 1. 安装基础工具

- Node.js 22.12+
- Rust stable（MSVC toolchain）
- Visual Studio Build Tools，包含“使用 C++ 的桌面开发”
- WebView2 Runtime（Windows 10/11 通常已包含）
- Npcap

安装 Rust MSVC 工具链：

```powershell
rustup default stable-x86_64-pc-windows-msvc
```

### 2. 安装 Npcap

从 Npcap 官方网站下载安装。开发阶段可以按实际需要选择：

- Install Npcap in WinPcap API-compatible Mode
- Support raw 802.11 traffic

PacketLens 不把 Npcap 安装程序直接放入仓库或默认重新分发。

### 3. 启动项目

```powershell
git clone https://github.com/weepwood/PacketLens.git
cd PacketLens
npm install
npm run tauri dev
```

仅运行前端界面：

```powershell
npm run dev
```

### 4. 构建检查

```powershell
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## 架构

```text
Npcap / wpcap.dll
        ↓
独立 Rust 抓包线程
        ↓
协议摘要 + Windows PID/方向映射
        ↓
┌────────────────────────────┬─────────────────────────────┐
│ 有界实时摘要队列           │ 有界存储队列                │
│ 100ms Tauri 批量事件       │ pcapng 分段 + SQLite 索引   │
└────────────────────────────┴─────────────────────────────┘
        ↓
有界 Flow Tracker
        ↓
500ms FlowSnapshot
        ↓
React 虚拟列表、连接页和进程页
```

关键约束：

1. React 不接触原始抓包缓冲区。
2. 不为每个数据包触发一次 IPC。
3. 实时、存储和 Flow 数据均有明确上限。
4. 无法可靠识别方向的流量保留为 `unknown`，不猜测为上传或下载。
5. SQLite 只保存索引，不重复保存完整 payload。
6. Npcap 使用动态加载，仓库不依赖 Npcap SDK 的 `.lib` 文件。

## 本地数据

每次捕获创建一个独立会话目录：

```text
<app-data>/
├─ packetlens.sqlite3
└─ captures/
   └─ <session-id>/
      ├─ segment-0000.pcapng
      └─ segment-0001.pcapng
```

“捕获历史”页面可以查看会话、存储路径、包数、字节数、分段和分页索引。pcapng 分段可交给 Wireshark 继续分析。

## 当前边界

- IP Helper 是周期性快照，生命周期极短的连接仍可能在刷新间隔内消失；Issue #3 保持开启，用 ETW 补全短连接和 PID 生命周期。
- UDP 通配监听和端口复用可能无法唯一归属到进程，因此进程映射用于开发诊断，不应直接作为审计证据。
- pcapng 当前固定使用 Ethernet LinkType。
- 单包协议树、Hex/ASCII 查看器、DNS/TLS 深度解析尚未完成。
- CI 已验证编译、静态检查和单元测试，但真实 Windows + Npcap 网络环境仍需要现场验证。

## 隐私与安全

抓包文件可能包含 IP、域名、Cookie、Token 或业务数据。PacketLens 默认：

- 不上传捕获数据；
- 不启用 HTTPS 中间人代理；
- 不自动解密 TLS；
- 不无限保存 payload；
- 删除会话时同时删除对应本地 pcapng 目录和索引。

请只捕获你拥有或被授权分析的设备和网络。

## 文档

- [架构说明](docs/ARCHITECTURE.md)
- [捕获存储](docs/STORAGE.md)
- [Flow 与进程归属](docs/FLOW-TRACKING.md)
- [Windows 开发环境](docs/WINDOWS-DEVELOPMENT.md)
- [路线图](ROADMAP.md)
- [贡献指南](CONTRIBUTING.md)
- [安全策略](SECURITY.md)

## License

MIT
