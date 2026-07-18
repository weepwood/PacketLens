# Contributing

## 分支与提交

- 功能：`feat/<name>`
- 修复：`fix/<name>`
- 文档：`docs/<name>`
- 提交信息遵循 Conventional Commits，例如 `feat(capture): add pcapng writer`

所有变更通过 Pull Request 合入 `main`。

## 本地检查

```powershell
npm install
npm run check
npm run build

cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

实时抓包测试必须在 Windows 10/11、已安装 Npcap 的环境进行。

## 代码边界

- 高频抓包、解析、Flow 和存储全部在 Rust。
- React 只接收批量摘要和按需详情。
- 不允许无界队列。
- 不允许在数据包回调中直接更新 UI。
- 不默认上传捕获数据。
- 新协议解析器必须包含最小合法、截断和畸形输入测试。

## Pull Request 检查清单

- [ ] 说明了变更目的和风险
- [ ] 添加或更新测试
- [ ] 未提交 pcap、数据库或真实敏感流量
- [ ] 前端类型检查通过
- [ ] Rust fmt、clippy 和 test 通过
- [ ] 对性能和内存影响进行了说明
