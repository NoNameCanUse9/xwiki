# agentdocs-client (xwiki-app)

AgentDocs 桌面端与 CLI(Rust / GPUI)。通过 HTTP API 对接 Go 服务端
(`main` 分支的 `agentdocs`),不直接访问服务端 SQLite/Git。

## 构建

```bash
cargo build --release
```

系统依赖(Linux):`libfontconfig1-dev libx11-dev libxkbcommon-dev libwayland-dev
libxkbcommon-x11-dev libgl1-mesa-dev libegl1-mesa-dev`(建议同时装
`fonts-jetbrains-mono fonts-inter` 以获得完整 Cobalt 排版)。

## 运行

```bash
# 桌面端(GUI, WSLg 下可直接显示)
cargo run            # 或 ./target/release/xwiki-app

# CLI(子命令模式)
./target/release/xwiki-app server status
./target/release/xwiki-app login --username admin --password secret123
export AGENTDOCS_TOKEN=ad_xxx          # login 会铸造一个 write token
./target/release/xwiki-app project list
./target/release/xwiki-app doc tree <project-id>
./target/release/xwiki-app history list <project-id>
```

退出码:0 成功 · 2 用法 · 3 认证/权限 · 4 不存在 · 5 revision/锁冲突 · 6 网络/服务端。
全局参数:`--server <url>`(或 `config set-server`)、`--json`、环境变量 `AGENTDOCS_TOKEN`。

## 快捷键(桌面端)

- `⌘K` / `Ctrl+K` — 命令面板
- `⌘⇧T` / `Ctrl+Shift+T` — 切换浅色/深色 Cobalt 主题(持久化到
  `~/.config/agentdocs-client/theme`)

## 测试

```bash
cargo test                       # 单元测试
cargo test -- --ignored          # e2e(需要运行中的服务端,默认 127.0.0.1:9090)
```

## 架构

- `src/api` — HTTP 客户端(cookie jar / Bearer、统一错误信封、DTO)
- `src/domain`-ish 逻辑在 `src/app`(GPUI 状态 + 视图)
- `src/cli` — 子命令分发、退出码、表格/JSON 输出
- `src/config` — 主题与服务地址持久化(凭据不落盘)
