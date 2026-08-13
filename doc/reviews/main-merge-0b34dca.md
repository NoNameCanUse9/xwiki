# Main 分支 merge 0b34dca 审查复核报告

> 复核日期：2026-08-13
> 审查对象：merge commit `0b34dca`（`merge: integrate origin/main with safety history and recovery workflows`）
> 父提交：`51f3e8a`（feat(main): complete safety history and recovery workflows）+ `decf70f`（origin/main）
> 复核方式：逐项比对两父分支与合并结果的实际代码（`git show <rev>:<path>`），全部结论可复现。

## 结论总览

| # | 严重度 | 发现 | 复核结果 |
|---|--------|------|----------|
| 1 | 高 | 旧备份无法恢复（备份格式不兼容） | 属实 |
| 2 | 中 | 数据目录锁覆盖不完整 | 属实 |
| 3 | 中 | 原地恢复与失败回滚被删 | 属实 |
| 4 | 中低 | CI 工具链与 go.mod 版本不一致 | 属实 |
| 5 | 硬性违规 | history.go 绕过 Git 封装约定 | 属实，但为全仓通病（约定未落地） |
| 6 | 设计问题 | maintenance 与 ops 双实现并存 | 属实 |

---

## 1. [高] 旧备份无法恢复

### 现象
合并前（父分支 `51f3e8a`）创建的备份，合并后用当前 CLI 恢复会直接报错 `backup manifest missing`。

### 证据：两套格式对比

| 维度 | 父分支 51f3e8a（`internal/maintenance`） | 合并后 main（`internal/ops`） |
|------|------------------------------------------|-------------------------------|
| CLI 命令 | `agentdocs backup/restore`（cmd/agentdocs/main.go） | `xwiki backup create/restore`（cmd/xwiki/main.go） |
| Manifest 文件名 | `manifest.json`（小写） | `MANIFEST.json`（大写） |
| 版本字段 | `format_version`（int） | `format`（int，`backupFormat = 1`） |
| 数据条目路径 | `data/` 前缀（`addDataTree`，maintenance/backup.go:164） | 根级相对路径（`collectFiles`，ops.go:266） |
| 恢复校验 | `extractBackup` 读 `manifest.json` + `data/` 前缀（backup.go:221-231） | `validateRestored` 读 `MANIFEST.json`（ops.go:384-391） |

### 故障路径
合并后 CLI 只接线 `ops`：
- `cmd/xwiki/main.go:69` → `ops.BackupCreate`
- `cmd/xwiki/main.go:84` → `ops.BackupRestore`

旧备份（`manifest.json`/`format_version`/`data/` 结构）送入 `ops.BackupRestore` → `extractBackup` 解包后 `validateRestored`（ops.go:385）找不到 `MANIFEST.json` → `backup manifest missing`（ops.go:387）。**格式完全不兼容，报错路径与审查描述一致。**

### 附带事实
`internal/maintenance` 包在合并后仍完整保留（backup.go/restore/doctor/lock 全部在），但 **main 全仓已无任何调用者**（`git grep "maintenance\."` 仅命中 ops.go 注释）——是死代码。这也意味着修复兼容性的现成实现就在仓库里。

### 修复建议（二选一）
1. **推荐**：以 `ops` 为规范实现，给 `ops.BackupRestore` 增加旧格式读取兼容（检测 `manifest.json` + `data/` 前缀，映射为当前结构）。
2. CLI 回接 `maintenance` 实现 —— 不推荐，等于保留两套活跃实现（见 #6）。

---

## 2. [中] 数据目录锁覆盖不完整

### 现象
`reindex`、`admin create` 可在服务运行期间绕过数据目录锁，并发修改同一个 SQLite 数据库。

### 证据
父分支 `51f3e8a` 的 `internal/app/app.go`：
- `New()` 内即 `maintenance.AcquireDataLock(cfg.DataDir)`（app.go:42），`Close()` 释放（app.go:76-78）→ **锁覆盖 New..Close 全程**。

合并后 main 的 `internal/app/app.go`：
- `New()`（app.go:39-56）只 `sqlite.Open(cfg.DataDir)`，**不取锁**；
- `AcquireDataLock` 移到 `Run()`（app.go:94）→ 仅 HTTP 服务持有锁。

而 CLI 命令：
- `reindex`（cmd/xwiki/main.go:108-122）：`app.New(cfg)` 后直接 `SearchSvc().ReindexProject/ReindexAll`，无锁。
- `admin create`（cmd/xwiki/main.go:171-194）：`app.New` + `CreateAdmin`，无锁。

→ 服务运行中执行 `reindex` 或 `admin create` 会打开并写同一个 `xwiki.db`，绕过 `.xwiki.lock`。SQLite 层面有 WAL 兜底不会立即损坏，但违反"离线维护命令不得与服务并发"的锁设计意图，且 `reindex` 会重建索引表，存在与在线写入的竞态窗口。

### 修复建议
把 `AcquireDataLock` 移回 `New()`（持锁到 `Close`），或给 `reindex`/`admin` 命令显式加锁（复用 `ops.AcquireDataLock`）。注意 `ops.AcquireDataLock` 会 `MkdirAll(dataDir)`，对 `admin create`（初始化场景）恰好可用。

---

## 3. [中] 原地恢复与失败回滚被删

### 现象
父分支支持 `restore --replace`（原地替换非空目录），并在激活失败时自动回滚；合并结果只允许恢复到空目录，现有部署无法执行完整恢复流程。

### 证据
父分支 `51f3e8a`：
- CLI：`restore [--data-dir DIR] [--replace] <backup.tar.gz>`（cmd/agentdocs/main.go:69-83）。
- `maintenance.Restore(ctx, archive, dataDir, replace)`（backup.go:103）：
  - `replace=false` 且目录存在 → 报错 `data directory already exists; pass --replace`（backup.go:125-126）；
  - 替换前把旧目录改名保留为 `<dir>.pre-restore-<时间戳>`（backup.go:131-137）；
  - 激活失败时把旧目录 rename 回原位（backup.go:138-142）→ **失败可回滚**。

合并后 main：
- CLI：`xwiki backup restore -input <file> -data-dir <dir>`（cmd/xwiki/main.go:74-87），**无 `--replace` 标志**。
- `ops.BackupRestore`：目标目录非空（除 `.xwiki.lock`）直接报 `restore target must be empty`（ops.go:168-170）；删除空目录后 `os.Rename(tmp, dataDir)`（ops.go:198），无保留旧目录、无回滚。

`maintenance.Restore` 代码仍存在但不可达（见 #1）。

### 修复建议
给 `ops.BackupRestore` 恢复 `--replace` 语义：非空目录时先 rename 为 `.pre-restore-*`，`Rename(tmp, dataDir)` 失败则 rename 回去。`maintenance.Restore` 的既有逻辑可直接移植。

---

## 4. [中低] CI 工具链与 go.mod 版本不一致

### 现象
`go.mod` 要求 `go 1.26.5`，CI 两处固定 `go-version: '1.24'`。

### 证据
- `go.mod:3` → `go 1.26.5`
- `.github/workflows/ci.yml:11` 与 `:34`（main 分支）→ `go-version: '1.24'`

### 影响
- `GOTOOLCHAIN=auto`（默认）：`go build` 时隐式下载 go1.26.5 toolchain，CI 依赖隐式网络下载，且每次构建多一步；
- `GOTOOLCHAIN=local`：直接报 `requires go >= 1.26.5` 失败。

### 修复建议
二选一：CI 升级到 `1.26`（推荐，与 go.mod 对齐）；或把 go.mod 降到 CI 支持的版本（需确认实际用到的语言特性）。

---

## 5. [硬性违规] history.go 绕过 Git 封装约定

### 现象
`internal/project/history.go` 直接 `exec.CommandContext("git", ...)`，违反 `doc/development.md` 约定。

### 证据
- `internal/project/history.go:79`：`exec.CommandContext(ctx, "git", "--git-dir", repo.Dir, "log", "--format=...", "--all")`
- `doc/development.md:28`：`所有 Git 命令必须经由 internal/gitrepo 封装（阶段二引入），业务层禁止直接 exec.Command`

### 复核修正（重要）
**这是全仓通病，不是单点违规**：
- main 上不存在 `internal/gitrepo`（`git ls-tree main --name-only internal/` 无此目录）——约定标注"阶段二引入"，但阶段二从未落地；
- 全仓直接 `exec.Command git` 共 20+ 处：`internal/project/repo.go` 9 处、`internal/ops/ops.go` 3 处、`internal/maintenance/doctor.go` 3 处、`internal/project/transfer.go` 1 处等。

单点指认 history.go 字面成立，但根因是约定未实现、全仓无封装层。

### 修复建议
二选一：真正落地 `internal/gitrepo` 封装并迁移全部调用（工作量大但符合 development.md 方向）；或把 development.md 的约定改为现状描述（若认为封装无必要）。

---

## 6. [设计问题] maintenance 与 ops 双实现并存

### 现象
`internal/maintenance` 与 `internal/ops` 同时保留 backup/restore/doctor/lock 两套实现，锁文件协议不同。

### 证据
- `internal/maintenance/lock.go:30`：锁文件在 dataDir **旁**：`.<base>.lock`（如 `.data.lock`），flock 协议；
- `internal/ops/ops.go:38`：锁文件在 dataDir **内**：`.xwiki.lock`，O_EXCL + pid 协议。

两把锁路径不同、协议不同、**互不排斥**——若 maintenance 代码被重新接线，可与 ops 的锁同时持有，形同虚设。当前 maintenance 无调用者，属潜伏风险。

### 修复建议
收敛为单一实现：删除 `internal/maintenance`（及其测试），或将其降级为 ops 的兼容层（见 #1）。

---

## 验证情况
- 复核时 main 工作区 `git status` 干净，无残留冲突标记；
- 上述所有行号与代码均来自 `git show main:<path>` / `git show 51f3e8a:<path>` 实际输出，可复现。

## 修复优先级
1. **#1 备份兼容**（数据安全，用户已有旧备份则直接受损）
2. **#3 原地恢复/回滚**（与 #1 同属恢复流程完整性，可一并修）
3. **#2 锁覆盖**（并发写库风险）
4. **#4 Go 版本**（一行配置）
5. **#5/#6**（Standards 收敛，建议在功能修复后处理）
