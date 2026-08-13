# Main 分支修复 Plan

> 依据：`doc/reviews/main-merge-0b34dca.md`（已复核，全部属实）
> 目标分支：`main`（当前 tip `6252a13`，相对审查基线仅 changeset/history/web 有改动，本 plan 涉及的 ops.go / app.go / cmd/xwiki/main.go / maintenance 未变，行号有效）
> 原则：先保数据安全（备份/恢复），再堵并发写库，最后收敛架构；每步独立可提交、可回退。

## 优先级总览

| 优先级 | 项 | 问题 | 交付物 |
|--------|----|------|--------|
| P0 | 1 | 旧备份无法恢复（格式不兼容） | ops 旧格式兼容层 + 单测 |
| P0 | 2 | 数据目录锁覆盖不完整 | 锁移回 New..Close + 并发测试 |
| P1 | 3 | 原地恢复与失败回滚被删 | restore `--replace` + 回滚 + 单测 |
| P1 | 4 | CI Go 版本与 go.mod 不一致 | ci.yml 升 1.26 |
| P2 | 5 | Git 封装约定被绕过 | 建 internal/gitrepo 或改文档 |
| P2 | 6 | maintenance/ops 双实现 | 收敛为单一实现 |

---

## P0-1 旧备份恢复兼容

### 现状（已核实）
- 合并前 `51f3e8a` 的 CLI（`agentdocs backup/restore`）→ `maintenance.Backup/Restore`：tar 内 `manifest.json`（小写）+ `format_version` 字段 + `data/` 前缀条目。
- 合并后 CLI（`xwiki backup create/restore`）→ `ops.BackupCreate/BackupRestore`（cmd/xwiki/main.go:69,84）：只认 `MANIFEST.json`（大写）+ `format` + 根级条目。
- 旧备份送入 `ops.BackupRestore` → `validateRestored`（ops.go:385）找不到 `MANIFEST.json` → `backup manifest missing`。
- `internal/maintenance` 合并后零调用者（死代码），是实现兼容的现成参考。

### 方案（推荐：兼容层，不迁移 CLI 接线）
在 `ops.BackupRestore` 的提取阶段做格式探测，新旧格式走同一套校验/激活：

1. `extractBackup`（ops.go:331）解包时若发现 `manifest.json`（小写）→ 记录 `oldFormat = true`，并把 `data/` 前缀条目**剥离前缀**写到 tmp 根（对齐新格式布局）；`MANIFEST.json` → 原样。
2. 旧格式的 manifest 校验：`format_version` 必须等于 1（对照 maintenance/backup.go:20 `backupFormatVersion`）。
3. `validateRestored` 保持不动（新旧布局在 tmp 中已统一为根级 + MANIFEST.json，若旧格式则用旧 manifest 的 entries 校验文件）。

具体改动点：
- `ops.go`：`extractBackup` 增加旧格式分支（~15 行）；`validateRestored` 增加旧 manifest 解析分支（~10 行）。
- 旧 manifest 无文件清单（maintenance 的 backupManifest 只有 format_version/created_at，见 backup.go:22-25）→ 校验降级为"manifest 存在且 format_version=1"，文件完整性由 `checkSQLite`/`checkRepositories` 兜底（restore 后可跑 `doctor`）。

### 测试
- 新增 `ops_test.go`：用 maintenance 的布局手工构造旧格式 tar.gz（`manifest.json` + `data/xwiki.db` + `data/repos/...`），`BackupRestore` 成功；错误断言 `backup manifest missing` 不再触发。
- 回归：新格式备份 create→restore 往返不变。

### 备选（不推荐）
CLI 回接 maintenance —— 等于让双实现同时活跃，放大 P2-6 问题。

---

## P0-2 数据目录锁覆盖

### 现状（已核实）
- `app.New`（app.go:39）只开 DB；`AcquireDataLock` 在 `Run`（app.go:94）才获取。
- `reindex`（cmd/xwiki/main.go:108）、`admin create`（cmd/xwiki/main.go:171）只调 `app.New`，无锁 → 可与运行中的服务并发写同一 `xwiki.db`。
- 父分支 `51f3e8a` 在 `New` 内取锁、`Close` 释放（锁覆盖 New..Close 全程）。

### 方案（对齐父分支：锁移回 New）
1. `App` 增加 `dataLock *ops.DataLock` 字段；`New` 末尾 `ops.AcquireDataLock(cfg.DataDir)`，失败则关 DB 返回错误。
2. `Run` 删除自己的 `AcquireDataLock`（避免双重获取），直接用 `a.dataLock`。
3. `Close` 释放锁（先关 DB 再放锁）。
4. `ops.AcquireDataLock` 会 `MkdirAll(dataDir)` —— 对 `admin create`（初始化场景）正好可用，无副作用。

### 测试
- 集成测试：服务 `Run` 期间调用 `reindex` / `admin create` → 断言返回 `data directory is locked`。
- 现有 `app_test.go` 回归（`New` 行为变化：数据目录不可写时 New 直接失败）。

---

## P1-3 原地恢复 + 失败回滚

### 现状（已核实）
- 父分支：`restore --replace` + `maintenance.Restore` 保留旧目录为 `<dir>.pre-restore-<ts>`，激活失败 rename 回滚（backup.go:125-144）。
- 合并后：`ops.BackupRestore` 非空目录报 `restore target must be empty`（ops.go:169）；CLI 无 `--replace`。

### 方案（移植 maintenance.Restore 的替换语义到 ops）
1. `ops.BackupRestore` 增加 `replace bool` 参数（内部函数签名变更，CLI 是唯一调用方）。
2. 非空目录（仅 `.xwiki.lock` 除外）时：`replace=false` → 保持现有报错；`replace=true` → 先 `os.Rename(dataDir, dataDir+".pre-restore-"+ts)`，激活（`os.Rename(tmp, dataDir)`）失败则 rename 回旧目录。
3. CLI `backup restore` 增加 `-replace` 标志（cmd/xwiki/main.go:74-87），usage 文案同步（main.go:207-208）。

### 测试
- 单测：非空目录 + `replace=true` 成功替换，旧目录内容仍在 `.pre-restore-*`；激活失败路径（构造 rename 目标不可达，如父目录被替换为文件）回滚成功。
- 回归：空目录恢复、非空目录无 `-replace` 报错不变。

---

## P1-4 CI Go 版本对齐

### 现状（已核实）
- `go.mod:3` → `go 1.26.5`；`.github/workflows/ci.yml:11` 与 `:34` → `go-version: '1.24'`。
- 仅 `GOTOOLCHAIN=auto` 隐式下载才能构建；`local` 直接失败。

### 方案
- `ci.yml` 两处 `go-version: '1.24'` → `'1.26'`（与 go.mod 对齐）。不降 go.mod（1.26.5 的语义以 go.mod 为准，且 CI 用 1.24 是历史遗留）。

### 验证
- 本地 `go build ./... && go test ./...` 用本机 go1.26.5 已通过；CI 绿即可。

---

## P2-5 Git 封装约定

### 现状（已核实）
- `development.md:28` 要求"所有 Git 命令必须经由 internal/gitrepo 封装（阶段二引入）"，但 `internal/gitrepo` 从未实现。
- 全仓直接 `exec.Command git` 20+ 处：`internal/project/repo.go`（9）、`internal/ops/ops.go`（3）、`internal/project/history.go`（1）、`internal/project/transfer.go`（1）、`internal/maintenance/doctor.go`（3）等。

### 方案
- **立即（低成本）**：更新 `development.md:28`，改为如实描述"Git 命令直接 exec，封装待落地"，避免文档与代码持续背离。
- **后续（独立任务）**：建 `internal/gitrepo`，先封装高频操作（`gitOutput`：带超时、安全参数、错误归一化），按 repo.go → history.go → ops.go 顺序迁移；每个文件迁移单独提交。不阻塞 P0/P1。

---

## P2-6 双实现收敛

### 现状（已核实）
- `internal/maintenance` 与 `internal/ops` 并存；锁协议不同（maintenance `.data.lock` 在 dataDir 旁 flock，ops `.xwiki.lock` 在 dataDir 内 O_EXCL+pid），互不排斥。
- maintenance 零调用者；`git grep "maintenance\."` 仅命中 ops.go 注释。

### 方案
- 在 P0-1 兼容层落地、确认旧备份可恢复后：删除 `internal/maintenance/` 全部文件（backup.go、restore 相关、doctor.go、lock*.go 及测试），连同 ops.go:26 注释中提及 maintenance 的措辞更新。
- 删除前跑 `go build ./... && go test ./...` 确认无引用残留（预计零阻碍，maintenance 无外部调用者）。

---

## 提交序列与依赖

| 顺序 | 提交内容 | 依赖 |
|------|----------|------|
| 1 | P1-4 CI go 版本（独立、零风险） | — |
| 2 | P0-1 旧格式兼容 + 单测 | — |
| 3 | P1-3 restore --replace + 回滚 + 单测 | 与 2 同文件，建议合并为一个提交或紧随 |
| 4 | P0-2 锁移回 New..Close + 并发测试 | — |
| 5 | P2-5 development.md 更新 | — |
| 6 | P2-6 删除 maintenance | 2 之后（兼容层就位） |

每步提交后跑 `go test ./... && go vet ./...`（main CI 同款）。最终整体验证：契约 CI（main + app 双分支 job）绿。

## 风险

- **P0-1**：旧备份无文件清单（只有 format_version）→ 完整性校验弱于新格式，靠 `doctor` 兜底；可接受，旧备份本就是过渡产物。
- **P0-2**：`New` 持锁后，若未来有"只开库不持锁"的需求（如只读工具），需要额外入口；当前 CLI 全部命令都该持锁，无此需求。
- **P1-3**：`.pre-restore-*` 旧目录是原子恢复的代价，恢复成功后需用户手动清理（或 `doctor` 提示）。
