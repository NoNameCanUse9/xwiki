# 全功能验收测试 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 对 xwiki-server 全部功能做端到端验收，产出「能/不能用」清单与修复建议。

**Architecture:** 两层验证：① 自动化基线（web 111 测试 + go test ./... 全量）；② 通过正在运行的 dev server（后端 :9090，Vite :5173）用 curl 走真实 HTTP 全链路冒烟（登录→建项目→写文档→版本→Token→Git→搜索→审计→用户管理→附件→锁→导出/导入→分享→API 文档→健康检查）。

**Tech Stack:** curl / jq、Go test、Vitest、Git 客户端、dev server（agentdocs :9090 + Vite :5173，WSL 环境，浏览器在 Windows）

**环境事实：**
- 后端 API 前缀 `/api/v1`，session cookie 认证；Agent Token 用 `Bearer ad_...`；Git 用 `Basic x:<secret>`
- 测试用户：admin / 密码见 `internal/server/` 测试助手（`admin123` 类，先 `POST /login` 探明）
- cookie 文件：`/tmp/ad_cookie.txt`；验证结果记录到 `docs/superpowers/plans/2026-08-09-full-verification-results.md`

---

### Task 1: 环境准备与基线

**Files:**
- Modify: `docs/superpowers/plans/2026-08-09-full-verification-results.md`（新建结果记录）

- [ ] **Step 1: 确认服务在线**

Run:
```bash
curl -s http://127.0.0.1:9090/healthz; echo
curl -s http://127.0.0.1:9090/readyz; echo
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:5173/
```
Expected: `ok` / `ok` / `200`

- [ ] **Step 2: 自动化测试基线**

Run:
```bash
cd web && pnpm test 2>&1 | tail -3
cd .. && go test ./... 2>&1 | grep -cE '^ok' && go test ./... 2>&1 | grep -E 'FAIL'
```
Expected: 25 files / 111 tests passed；`ok` 包数与 FAIL 0 行

- [ ] **Step 3: 记录基线到结果文件**（表格：功能 / 验证方式 / 结果 / 备注）

---

### Task 2: 认证与账号

**Files:** 无（纯 API 冒烟，记录到结果文件）

- [ ] **Step 1: 登录并保存 cookie**

```bash
curl -s -c /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"admin123"}' | jq . 2>/dev/null || true
```
若 401，改试 `admin`/`admin` 或从 `internal/server/auth_flow_test.go` 读取种子密码；登录失败也记入结果并继续。

- [ ] **Step 2: /me 与会话**

```bash
curl -s -b /tmp/ad_cookie.txt http://127.0.0.1:9090/api/v1/auth/me | jq .
```
Expected: 返回当前用户 JSON

- [ ] **Step 3: 修改密码 → 改回**

```bash
curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/auth/password \
  -H 'Content-Type: application/json' -d '{"current_password":"admin123","new_password":"admin123"}' | jq .
```
Expected: `{"ok":true}`（同密码重置也验证校验逻辑不报错）

- [ ] **Step 4: 登出**

```bash
curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/auth/logout | jq .
curl -s -b /tmp/ad_cookie.txt http://127.0.0.1:9090/api/v1/auth/me | jq .error.code
```
Expected: logout ok，随后 /me 返回 `unauthorized`

- [ ] **Step 5: 重新登录（恢复会话）** 与 Step 1 相同命令

---

### Task 3: 用户管理（settings/users）

- [ ] **Step 1: 列表**

```bash
curl -s -b /tmp/ad_cookie.txt http://127.0.0.1:9090/api/v1/users | jq '.users | length, .users[0]'
```

- [ ] **Step 2: 创建 → 禁用 → 启用 → 删除**

```bash
ID=$(curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/users \
  -H 'Content-Type: application/json' \
  -d '{"username":"qa-tmp","password":"pass1234","display_name":"QA"}' | jq -r '.user.id')
curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/users/$ID/disable | jq .
curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/users/$ID/enable | jq .
```
Expected: 全 `{"ok":true}`；用新用户登录验证禁用后 403（若接口无删除，记「无删除入口」）

---

### Task 4: 项目生命周期（首页）

- [ ] **Step 1: 列表 + 筛选字段**

```bash
curl -s -b /tmp/ad_cookie.txt http://127.0.0.1:9090/api/v1/projects | jq '.projects | length'
```

- [ ] **Step 2: 创建 → 重命名 → 归档 → 恢复 → 删除**

```bash
PID=$(curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/projects \
  -H 'Content-Type: application/json' -d '{"name":"qa-verify"}' | jq -r '.project.id')
curl -s -b /tmp/ad_cookie.txt -X PATCH http://127.0.0.1:9090/api/v1/projects/$PID \
  -H 'Content-Type: application/json' -d '{"name":"qa-verify-2"}' | jq '.project.name'
curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/projects/$PID/archive | jq '.project.archived'
curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/projects/$PID/unarchive | jq '.project.archived'
```
Expected: 名称变化、archived true→false；最后保留项目供 Task 5 用，全部完成后 DELETE 收尾

- [ ] **Step 3: 重名冲突**

```bash
curl -s -b /tmp/ad_cookie.txt -X PATCH http://127.0.0.1:9090/api/v1/projects/$PID \
  -H 'Content-Type: application/json' -d "{\"name\":\"$(已有项目名)\"}" | jq '.error.code'
```
Expected: `project_name_conflict`（若后端支持）

---

### Task 5: 文档读写与版本（docs-viewer）

- [ ] **Step 1: 初始 tree/home**

```bash
curl -s -b /tmp/ad_cookie.txt "http://127.0.0.1:9090/api/v1/projects/$PID/docs/tree?path=" | jq .
curl -s -b /tmp/ad_cookie.txt "http://127.0.0.1:9090/api/v1/projects/$PID/docs/home" | jq .
```

- [ ] **Step 2: 变更集创建/更新/移动/删除**

```bash
REV=$(curl -s -b /tmp/ad_cookie.txt http://127.0.0.1:9090/api/v1/projects/$PID/revision | jq -r '.revision')
curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/projects/$PID/changesets \
  -H 'Content-Type: application/json' \
  -d "{\"base_revision\":\"$REV\",\"message\":\"qa create\",\"changes\":[{\"op\":\"create\",\"path\":\"docs/qa.md\",\"content\":\"# QA\\n\\nhello\"}]}" | jq '.commit'
```
Expected: commit hash；再取新 revision 做 update/move/delete 各一次

- [ ] **Step 3: 读取页面**

```bash
curl -s -b /tmp/ad_cookie.txt "http://127.0.0.1:9090/api/v1/projects/$PID/docs/pages/docs/qa.md" | jq .
```

- [ ] **Step 4: 历史 + diff + 旧版本内容**

```bash
curl -s -b /tmp/ad_cookie.txt "http://127.0.0.1:9090/api/v1/projects/$PID/history?path=docs/qa.md" | jq '.commits | length'
curl -s -b /tmp/ad_cookie.txt "http://127.0.0.1:9090/api/v1/projects/$PID/diff?from=<旧rev>&to=<新rev>" | jq .
curl -s -b /tmp/ad_cookie.txt "http://127.0.0.1:9090/api/v1/projects/$PID/revision/<旧rev>/docs/pages/docs/qa.md" | jq .
```

- [ ] **Step 5: 冲突（stale base_revision）**

```bash
# 用 Task5 Step2 的旧 REV 再提交一次
curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/projects/$PID/changesets \
  -H 'Content-Type: application/json' \
  -d "{\"base_revision\":\"$旧REV\",\"message\":\"qa stale\",\"changes\":[{\"op\":\"create\",\"path\":\"docs/stale.md\",\"content\":\"x\"}]}" | jq '.error.code'
```
Expected: `revision_conflict`

---

### Task 6: 编辑器高级能力（Markdown 特性渲染）

- [ ] **Step 1: 写入含代码块/表格/任务列表/Katex/Mermaid 的页面**

```bash
CONTENT='# Rich

\`\`\`ts
const x = 1;
\`\`\`

| a | b |
|---|---|
| 1 | 2 |

- [x] done
- [ ] todo

$e = mc^2$

\`\`\`mermaid
graph LR; A-->B
\`\`\`
'
# base64 或 jq -Rs 编码后走 changesets create docs/rich.md
```
Expected: 201/200；随后 GET page 内容与写入一致

- [ ] **Step 2: 附件上传 + 下载**

```bash
echo 'hello-attachment' > /tmp/qa.txt
curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/projects/$PID/attachments/docs \
  -F 'file=@/tmp/qa.txt' | jq .
curl -s -b /tmp/ad_cookie.txt "http://127.0.0.1:9090/api/v1/projects/$PID/attachments/docs/qa.txt"
```
Expected: 上传返回路径，下载内容为 `hello-attachment`

- [ ] **Step 3: 编辑锁**

```bash
curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/projects/$PID/locks \
  -H 'Content-Type: application/json' -d '{"path":"docs/qa.md"}' | jq .
curl -s -b /tmp/ad_cookie.txt http://127.0.0.1:9090/api/v1/projects/$PID/locks | jq .
curl -s -b /tmp/ad_cookie.txt -X DELETE http://127.0.0.1:9090/api/v1/projects/$PID/locks \
  -H 'Content-Type: application/json' -d '{"path":"docs/qa.md"}' | jq .
```
Expected: acquired → status 含锁 → released；另测 force-release

---

### Task 7: Agent Token（settings/tokens）

- [ ] **Step 1: 创建（项目绑定）→ 列表 → 撤销**

```bash
SECRET=$(curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/tokens \
  -H 'Content-Type: application/json' \
  -d "{\"name\":\"qa-bot\",\"scope\":\"write\",\"project_ids\":[\"$PID\"]}" | jq -r '.secret')
curl -s -b /tmp/ad_cookie.txt http://127.0.0.1:9090/api/v1/tokens | jq '.tokens | length'
```

- [ ] **Step 2: Token 读/写项目**

```bash
curl -s -H "Authorization: Bearer $SECRET" "http://127.0.0.1:9090/api/v1/projects/$PID/docs/tree?path=" | jq '.error // .entries | length'
# 写 changeset（scope write）
# 跨项目读 → 403
```

- [ ] **Step 3: 撤销后失效**

```bash
TID=$(curl -s -b /tmp/ad_cookie.txt http://127.0.0.1:9090/api/v1/tokens | jq -r '.tokens[] | select(.name=="qa-bot") | .id')
curl -s -b /tmp/ad_cookie.txt -X DELETE http://127.0.0.1:9090/api/v1/tokens/$TID | jq .
curl -s -H "Authorization: Bearer $SECRET" "http://127.0.0.1:9090/api/v1/projects/$PID/docs/tree?path=" | jq '.error.code'
```
Expected: `token_revoked` / 401

---

### Task 8: Git HTTP、搜索、审计、分享、导出导入

- [ ] **Step 1: Git clone + push（Token Basic 认证）**

```bash
git clone http://x:$SECRET@127.0.0.1:9090/git/$PID /tmp/qa-clone
cd /tmp/qa-clone && echo 'via git' >> docs/qa.md && git add -A
git -c user.name=QA -c user.email=qa@local commit -m 'git push'
git push origin main
```
Expected: push 成功，API 能看到新 commit

- [ ] **Step 2: 搜索 + 反向链接**

```bash
curl -s -b /tmp/ad_cookie.txt "http://127.0.0.1:9090/api/v1/projects/$PID/search?q=hello" | jq '.results | length'
curl -s -b /tmp/ad_cookie.txt "http://127.0.0.1:9090/api/v1/projects/$PID/backlinks?path=docs/qa.md" | jq .
```

- [ ] **Step 3: 审计日志**

```bash
curl -s -b /tmp/ad_cookie.txt http://127.0.0.1:9090/api/v1/projects/$PID/audit | jq '.entries[0:3]'
```

- [ ] **Step 4: 分享创建/访问**

```bash
SID=$(curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/projects/$PID/shares \
  -H 'Content-Type: application/json' -d '{"path":"docs/qa.md","ttl_hours":24}' | jq -r '.share.id // .share.token')
# 无 cookie 访问分享链接（路径见 openapi：/s/{token} 或 share 端点）
```

- [ ] **Step 5: 导出 zip/bundle + 重新导入**

```bash
curl -s -b /tmp/ad_cookie.txt -o /tmp/qa.zip "http://127.0.0.1:9090/api/v1/projects/$PID/export.zip"
unzip -l /tmp/qa.zip | head -8
curl -s -b /tmp/ad_cookie.txt -X POST http://127.0.0.1:9090/api/v1/projects/import \
  -F 'file=@/tmp/qa.zip' -F 'name=qa-imported' | jq '.project.name'
```

- [ ] **Step 6: API 文档 + 健康**

```bash
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:9090/api/openapi.json
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:5173/api-docs
```

---

### Task 9: 汇总报告

**Files:**
- Create: `docs/superpowers/plans/2026-08-09-full-verification-results.md`（若 Task 1 未建）

- [ ] **Step 1: 整理结果表**：每个 Task 的每步 → ✅/❌/⚠️，❌ 附错误信息
- [ ] **Step 2: 清理测试数据**：删除 qa-verify 项目（DELETE + purge）、qa-tmp 用户（若可删）、/tmp/qa-*
- [ ] **Step 3: 输出结论**：按严重度列出不可用功能与建议修复，报告给用户
