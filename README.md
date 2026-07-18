# PacketLens

PacketLens 是一款 **Windows 优先**的实时网络观察与数据包分析工具，使用 **Rust + Tauri 2 + React** 构建。

它希望把传统抓包工具的“数据包视角”和任务管理器式的“进程视角”结合起来，让开发者能够回答：

- 当前哪些程序正在联网？
- 程序连接了哪些 IP、域名和端口？
- 实时上传、下载和数据包速率是多少？
- DNS、TCP、TLS 请求发生了什么？
- 哪些连接存在超时、重传或失败？

> 当前版本为可运行的工程骨架与 Windows 抓包 MVP。协议覆盖、进程关联和持久化会按 Roadmap 继续推进。

## 技术栈

- Windows 10/11 x64
- Rust + Tokio
- Tauri 2
- React 19 + TypeScript + Vite
- Npcap（运行时动态加载 `wpcap.dll`）
- etherparse
- Zustand
- TanStack Virtual
- SCSS/CSS Variables（不使用 TailwindCSS）

## 已落地能力

- Tauri + React 桌面应用骨架
- Windows Npcap 安装状态检测
- 网卡枚举
- 启动、停止实时捕获
- BPF 捕获过滤器
- Ethernet / IPv4 / IPv6 / TCP / UDP / ICMP 基础解析
- Rust 后端每 100ms 批量推送数据，避免逐包刷新 React
- 前端虚拟数据包列表，最多保留最近 10,000 条摘要
- 实时吞吐量、数据包速率和丢包统计
- Windows TCP 连接与 PID 映射基础接口
- CI、依赖更新、安全策略和贡献指南

## Windows 开发环境

### 1. 安装基础工具

- Node.js 20.19+ 或 22.12+
- Rust stable（MSVC toolchain）
- Visual Studio Build Tools，包含“使用 C++ 的桌面开发”
- WebView2 Runtime（Windows 10/11 通常已包含）
- Npcap

安装 Rust MSVC 工具链：

```powershell
rustup default stable-x86_64-pc-windows-msvc
```

### 2. 安装 Npcap

从 Npcap 官方网站下载安装。开发阶段建议勾选：

- Install Npcap in WinPcap API-compatible Mode（可选）
- Support raw 802.11 traffic（仅确有无线需求时）

PacketLens 不会把 Npcap 安装程序直接打包进仓库，以避免重新分发许可问题。

### 3. 启动项目

```powershell
npm install
npm run tauri dev
```

前端单独运行：

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
WindowsCaptureBackend（独立阻塞线程）
        ↓
有界同步队列
        ↓
etherparse 基础协议解析
        ↓
PacketSummary / FlowKey
        ↓
100ms 批量聚合
        ↓
Tauri Event
        ↓
React + TanStack Virtual
```

关键约束：

1. React 不接触原始抓包缓冲区。
2. 不为每个数据包触发一次 IPC。
3. 内部队列有上限，过载时显式统计丢包。
4. 前端只保留有限实时窗口；长期数据后续写入 pcapng + SQLite。
5. Npcap 使用动态加载，仓库不依赖 Npcap SDK 的 `.lib` 文件。

## 隐私与安全

抓包文件可能包含 IP、域名、Cookie、Token 或业务数据。PacketLens 默认：

- 不上传任何捕获数据；
- 不启用 HTTPS 中间人代理；
- 不自动解密 TLS；
- 不无限保存 payload；
- 导出前提示隐私风险。

请只捕获你拥有或被授权分析的设备和网络。

## 文档

- [架构说明](docs/ARCHITECTURE.md)
- [Windows 开发环境](docs/WINDOWS-DEVELOPMENT.md)
- [路线图](ROADMAP.md)
- [贡献指南](CONTRIBUTING.md)
- [安全策略](SECURITY.md)

## License

MIT
