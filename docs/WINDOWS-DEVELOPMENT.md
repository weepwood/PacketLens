# Windows Development

## 必要环境

| 工具 | 要求 |
|---|---|
| Windows | Windows 10/11 x64 |
| Node.js | 22.12+ |
| Rust | stable MSVC |
| Visual Studio Build Tools | Desktop development with C++ |
| WebView2 | Evergreen Runtime |
| Npcap | 最新稳定版 |

## 安装 Rust

```powershell
winget install Rustlang.Rustup
rustup default stable-x86_64-pc-windows-msvc
rustup component add rustfmt clippy
```

## 安装 Node.js

```powershell
winget install OpenJS.NodeJS.LTS
node --version
npm --version
```

## 安装 Npcap

Npcap 需要从官方安装程序安装。PacketLens 只动态加载本机的 `wpcap.dll`。

验证：

```powershell
Test-Path "$env:SystemRoot\System32\Npcap\wpcap.dll"
```

## 运行

```powershell
git clone https://github.com/weepwood/PacketLens.git
cd PacketLens
npm install
npm run tauri dev
```

如果界面显示“未检测到 Npcap”：

1. 确认 Npcap 已安装；
2. 确认应用与 Npcap 架构一致，优先使用 x64；
3. 重启终端或系统；
4. 检查安全软件是否拦截 DLL；
5. 确认 `wpcap.dll` 位于系统搜索路径或 Npcap 目录。

## 调试建议

- 先用 `tcp or udp or icmp` 过滤，减少广播噪声。
- 使用 `Npcap Loopback Adapter` 观察 localhost。
- 抓包线程是阻塞线程，不要迁移到 React 或主线程。
- 调试崩溃前先关闭抓包，保证 worker 正常退出。
- 不要将包含真实业务数据的 `.pcapng` 提交到 Git。

## 发布前工作

当前 `bundle.active=false`。正式打包前需要：

1. 生成图标；
2. 设置 `bundle.active=true`；
3. 配置 NSIS/MSI；
4. 配置 Windows 代码签名；
5. 明确 Npcap 安装检测和许可策略；
6. 测试安装、升级和卸载不会删除用户抓包文件。
