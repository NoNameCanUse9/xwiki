# XWiki 实施计划索引

> 本目录按 `doc/spec.md` §27 的 MVP 阶段划分，为每个阶段保存一份独立执行计划。
> 每个计划都可独立执行（含 TDD 步骤、完整代码、提交点），完成后产出可运行、可测试的软件。
> 文档语言为中文（与 spec 保持一致）；代码、命令、提交信息为英文。

## 计划清单

| # | 计划文件 | 对应 spec 阶段 | 目标 | 依赖 | 状态 |
|---|---------|--------------|------|------|------|
| 01 | `2026-08-02-xwiki-phase-01-skeleton.md` | 阶段一：项目骨架 | Go 服务骨架、SQLite 迁移、管理员 CLI、登录/会话 API、React+shadcn/ui 登录页、静态资源嵌入、Docker | — | ✅ 已完成 |
| 02 | `2026-08-02-xwiki-phase-02-projects.md` | 阶段二：项目和 Git 仓库 | 创建项目、一项目一 Git 仓库、README 初始化、项目列表/详情/归档 | 01 | ✅ 已完成 |
| 03 | `2026-08-02-xwiki-phase-03-docs-read.md` | 阶段三：文档读取 | Git Tree、Markdown 读取与渲染、文档树、面包屑、项目首页 | 02 | ✅ 已完成 |
| E1 | `2026-08-02-xwiki-editor-enhancement.md` | 编辑器增强 | Tiptap Notion 式编辑器、嵌入组件、wiki 链接、文件操作 | 03 | ✅ 已完成 |
| E2 | `2026-08-02-xwiki-otterwiki-align.md` | OtterWiki 对齐 | 附件、Git HTTP、自定义侧栏 | 03 | ✅ 已完成 |
| 04 | `2026-08-02-xwiki-phase-04-changesets.md` | 阶段四：ChangeSet 写入 | 项目锁、临时 Worktree、create/update/delete/move、dry-run、原子 update-ref、409 冲突 | 03 | ✅ 已完成 |
| 05 | `2026-08-02-xwiki-phase-05-history.md` | 阶段五：历史和 Diff | Commit 列表、文件历史、机器可读 Diff、Revert | 04 | ✅ 已完成 |
| 06 | `2026-08-02-xwiki-phase-06-tokens.md` | 阶段六：Agent Token | Token、Scope、项目/路径限制、幂等键、审计日志 | 05 | ✅ 已完成 |
| 07 | `2026-08-02-xwiki-phase-07-search.md` | 阶段七：搜索 | SQLite FTS5、增量索引、reindex CLI | 05 | ✅ 已完成 |
| 08 | `2026-08-02-xwiki-phase-08-openapi-transfer.md` | 阶段八：OpenAPI 与导入导出 | Scalar 预览、ZIP/Bundle 导入导出、图片与附件 | 05 | ✅ 已完成 |

## 全局验收（spec §29，全部阶段完成后统一核对）

最终交付：可运行 Go 后端、React+shadcn/ui 前端、SQLite Migration、Git 仓库服务、REST API、OpenAPI 文档、Dockerfile、docker-compose.yml、测试套件、README、架构说明、示例项目。

## 编号规则

- 文件名：`YYYY-MM-DD-xwiki-phase-NN-<slug>.md`
- 每完成一个阶段计划，更新本表状态，并在对应计划的「验收清单」中勾选 spec 验收项。
