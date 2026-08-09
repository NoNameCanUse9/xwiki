# 全功能验收测试结果（2026-08-09）

执行：`docs/superpowers/plans/2026-08-09-full-verification.md`，Inline 模式
环境：dev server（agentdocs :9090 + Vite :5173，WSL）、临时新代码实例（:19090，已销毁）

## 总览

| 功能域 | 结果 |
|---|---|
| 自动化基线（web 111 / Go 15 包） | ✅ 全过 |
| 认证与账号 | ✅ 6/6 |
| 用户管理 | ✅ 3/3（无删除接口，属设计） |
| 项目生命周期 | ✅ 7/7 |
| 文档读写与版本 | ✅ 10/10 |
| 富文本/附件/编辑锁 | ✅ 8/8 |
| Agent Token | ✅ 6/6 |
| Git HTTP / 搜索 / 审计 / 分享 / 导出导入 / API 文档 | ✅ 12/12 |

**未发现功能性 bug。** 测试中发现并已确认的差异均为环境或预期行为。

## 明细

### Task 2 认证 ✅
- 登录 200、`/me` 返回 admin、改密 `{ok:true}`、登出 `{ok:true}`、登出后 `/me` 返回 `authentication_required`、重登成功
- 测试账号：admin / secret123（生产库种子密码，非代码问题）

### Task 3 用户管理 ✅
- 列表、创建（qa-tmp）、禁用后登录 → `account_disabled`、启用后登录恢复 200
- ⚠️ 无删除用户 API（仅禁用/启用）—— 设计如此，非 bug

### Task 4 项目生命周期 ✅
- 创建 → 重命名（`qa-verify`→`qa-verify-2`）→ 重名冲突 `project_name_conflict`（改名为其他项目名时触发）→ 归档 `archived:true` → 恢复 `archived:false` → 删除 200
- 改回自己的名字不触发冲突（合理）

### Task 5 文档读写与版本 ✅
- 创建/更新/移动 changesets 全部返回 commit
- 读取 `content` 正确、tree/home 正常
- commits 列表（含初始化/重命名记录）、file history、diff（numstat）正常
- stale base → `revision_conflict` ✓、revert 生成 Revert commit ✓

### Task 6 富文本/附件/锁 ✅
- 服务端 HTML 渲染：代码块（language-ts）、表格、任务列表（checkbox）正常
- KaTeX（`$e=mc^2$`）与 Mermaid 服务端**原样输出**，由前端渲染 —— 预期行为（依赖 katex/mermaid 已装、docs-viewer 使用）
- 附件：无独立上传端点，二进制走 Git 仓库提交；下载 `attachments/docs/qa.bin` 200 且字节一致
- 编辑锁：acquire → status → 二次 acquire `page_locked`（"该页面正被 admin 编辑"）→ force-release → 再 acquire → heartbeat → release 全部正常

### Task 7 Agent Token ✅
- 创建（secret 返回一次）、列表、项目内读、跨项目读 403 `agent_forbidden`、写 changesets、撤销 `{ok:true}`、撤销后 `invalid_token`
- **新代码验证**（临时 :19090 实例）：Token 写 `docs/qa.md` 与根目录 `README.md` 均成功（路径前缀限制已移除）；Token JSON 无 `path_prefixes`；不带 `path_prefixes` 创建成功
- ⚠️ 环境注意：用户当前 dev server（:9090）仍是旧二进制（15:55 启动），仍强制路径前缀。**重启后生效**，不影响代码正确性

### Task 8 集成 ✅
- Git smart HTTP：token 认证 clone、push 成功，API 立即看到新 commit
- 搜索：`?q=QA` 返回 README 与 docs/qa-moved.md 命中（含 snippet）；`/search` 全局端点不存在（404，仅项目内搜索，正确）
- 反向链接：无链接时返回空（正常）
- 审计：entries 含 change/token 操作记录
- 分享：创建幂等（同页同 token）、公开 `/share/{token}` 无 cookie 200、渲染标题 `qa-moved`/`<h1>QA v2</h1>`
- 导出：export.zip（5 文件）、export.bundle（2.9KB）
- 导入：`/import/bundle?name=` 成功（11 commits 全历史）；`/{id}/import` JSON base64 成功（imported:5）
- openapi.json 200；Vite 各路由（/api-docs、/settings/tokens、/settings/users、/settings/audit）200

## 发现/建议（非阻塞）

1. **dev server 未重启**：:9090 跑的是提交前旧代码（Token 路径限制仍在）。功能不受影响（旧代码兼容新前端），但如需体验新 Token 行为需重启。
2. 无删除用户 API —— 如需可后续加 `DELETE /users/{id}`。
3. 附件上传 UX：目前只能通过 Git 提交二进制，前端附件面板实际走 changesets（需确认前端是否有对应入口，未发现独立上传端点属预期架构）。

## 测试数据清理

- 删除：qa-verify、qa-import-target、qa-import-json、qa-imported-bundle（均已删除）
- 禁用：qa-tmp（无删除 API）
- 临时文件与临时实例已销毁；原 3 个项目（team-wiki/meeting-notes/engineering-ops）未动
