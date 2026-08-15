# Handoff:服务端 move 语义验证 + e2e 契约测试

> 交接对象:服务端开发者(或负责 e2e 验证的同学)
> 交接人:客户端(本仓库)
> 日期:2026-08-15
> 关联提交:`5de3bb7 feat: add safe document move action`、`205067b fix: keep viewed doc open after move and harden path checks`

## 1. 背景

客户端(本仓库)已实现文档/目录移动与重命名功能,通过 **changeset 的 `op: "move"`** 完成,而不是"删除旧路径 + 新建新路径"。

客户端侧已完成并验证:`cargo check`、`cargo test`(29 passed,含路径映射/校验单元测试)、LSP 干净。

**剩余工作全部依赖服务端(不在本仓库)**,需要你:

1. 确认服务端对 `op: "move"` 的语义(见 §3 问题清单);
2. 把 §4 的 e2e 用例补进 `src/api/e2e_test.rs`(或服务端测试套件)并跑通;
3. 根据验证结果决定是否启用 §5 的 dry-run 冲突预检。

## 2. 客户端已确认的契约(服务端应以此为准)

### 请求形状

`POST /api/v1/projects/{project_id}/changesets`

```json
{
  "base_revision": "<GET /api/v1/projects/{id}/revision 拿到的当前 revision>",
  "message": "移动文档",
  "changes": [
    { "op": "move", "path": "docs/guide.md", "new_path": "archive/guide.md" }
  ]
}
```

- `new_path` 仅在 move 时携带;`content` 为 null(序列化时省略)。
- 移动目录时 `path` 是目录路径(如 `docs`),客户端语义是**整棵子树跟随**。
- dry-run 预检:`?dry_run=true` 查询参数(`apply_changeset_with_options` 已支持,见 §5)。

### 客户端响应解析

`ChangesetResult { commit: String, revision: String, preview?: Option<Value> }`。

### 客户端校验(移动前,纯本地)

- 拒绝:空路径、绝对路径、尾斜杠、反斜杠、`.`/`..` 段、与源相同、移动到自身子路径。
- 目标路径是否已存在:**客户端不检查**,依赖服务端语义(见 §3-2)。

## 3. 需要服务端确认的问题清单

按优先级排序。请逐条给出"服务端当前行为 + 是否与客户端预期一致"。

| # | 问题 | 客户端预期 | 影响 |
| --- | --- | --- | --- |
| 1 | `op: "move"` 是否被接受且**原子执行**(失败不产生部分状态) | 是 | 移动失败后客户端只显示错误,不重试;若服务端非原子,客户端需要额外补偿逻辑 |
| 2 | 目标路径**已存在**时的行为 | **拒绝**(409 + 明确错误码,如 `path_exists`),**绝不覆盖** | 决定 §5 dry-run 预检是否必须;覆盖 = 数据丢失,不可接受 |
| 3 | 移动**目录**是否递归移动整棵子树 | 是(客户端把目录 move 当子树移动) | 目录移动是主要用例 |
| 4 | 目标**父目录不存在**时(如 `archive/x` 而 `archive` 不存在) | 报错即可(客户端不自动建目录);若服务端自动创建父目录也可接受,但需告知 | 影响错误提示文案 |
| 5 | **历史连续性**:移动后新路径能否查到旧提交历史?旧路径历史是否保留? | 新路径历史连续(依赖 git rename detection 或服务端追踪);旧路径应 404 或重定向 | 客户端"历史"面板按新路径请求;若历史断裂,需服务端方案 |
| 6 | **并发/过期 base revision** 与 `update` 是否一致(409 `revision_conflict`) | 是 | e2e 可直接复用现有 stale 测试模式 |
| 7 | 权限:read-only token 提交 move → 403 `agent_forbidden`? | 是(与 update 一致) | e2e 可复用现有 token 测试模式 |
| 8 | 边界:move 到根(`new_path` 为空或 `/`)、move 根目录、path 不存在 | 返回明确错误码 | 防呆 |

## 4. e2e 契约用例(建议补进 `src/api/e2e_test.rs`)

现有套件:`cargo test e2e_contracts -- --ignored`,服务端默认 `http://127.0.0.1:9090`,种子账号 `admin/secret123`。helper:`unique()`(唯一前缀)、`update()`(构造 update change)。

建议在 `e2e_contracts` 主流程中追加(沿用同一项目 `pid`,或新建项目避免污染):

```rust
// ---- move:文档 ----
let base = admin.revision(&pid).await.expect("revision");
let mv = admin
    .apply_changeset(&pid, &base, "move doc", vec![dto::Change {
        op: "move".into(),
        path: "guide.md".into(),
        new_path: Some("moved.md".into()),
        content: None,
    }])
    .await
    .expect("move doc");
assert!(admin.page(&pid, "moved.md").await.is_ok(), "new path readable");
assert!(
    admin.page(&pid, "guide.md").await.is_err(),
    "old path must disappear"
);
let tree = admin.tree(&pid, "").await.expect("tree");
assert!(
    tree.iter().any(|e| e.path == "moved.md"),
    "tree reflects the move"
);
```

然后按 §3 逐条补断言,重点:

- **冲突**:先创建 `target.md`,再 move `moved.md` → `target.md`,记录服务端返回的 status/code(预期 409 + 明确 code),断言内容未被覆盖;
- **目录**:建 `dir/a.md`、`dir/sub/b.md`,move `dir` → `archive`,断言 `archive/a.md`、`archive/sub/b.md` 可读,旧路径 404;
- **历史**:move 后 `commits_page` / `commits_search_page` 查新路径相关提交(确认历史是否连续;若服务端依赖 git rename detection,断言方式可能需调整);
- **dry_run**:先跑一次 `dry_run=true` 的 move,断言不产生 commit(revision 不变),再正式 move;若服务端不支持 dry_run,记录并反馈(影响 §5);
- **权限**:read-only token 提交 move → 403 `agent_forbidden`。

> 注意:e2e 套件是跨分支契约测试(服务端 main 构建、测试从 app checkout 跑)。如果服务端当前**不**接受 `op: "move"`,请先把服务端实现补齐再跑这套用例 —— 这正是本 handoff 的核心目的。

## 5. dry-run 冲突预检(客户端侧待接线,依赖 §3-2/§4 结果)

若服务端确认:dry_run 对 move 有效,且冲突目标会返回明确错误 → 客户端启用预检:

- API 层:`src/api/mod.rs::changeset_one` 增加 dry_run 变体(复用 `apply_changeset_with_options(dry_run=true)`),新增 `Client::move_doc_dry_run`(或给 `move_doc` 加参数);
- UI 层:`src/app/mod.rs::confirm_move_doc` 提交前先 dry-run,失败则在弹窗内 toast 报错、不提交。

若服务端在正式提交时已拒绝冲突(§3-2 成立),dry-run 属于 UX 优化(提前提示),可延后;若服务端会覆盖,则 dry-run 是**必须**的防护。

## 6. 验收标准

1. §3 问题清单逐条有结论(服务端行为 + 错误码);
2. §4 e2e 用例在真实服务端跑通(若服务端需改动,改动合入后跑通);
3. 若启用 §5:客户端移动弹窗在目标冲突时给出中文错误提示,不产生提交;
4. 客户端 `cargo test` 保持全绿。

## 7. 相关代码位置(客户端)

| 关注点 | 位置 |
| --- | --- |
| 移动弹窗 + 路径校验 | `src/app/mod.rs::confirm_move_doc`、`move_target_error`、`mapped_path` |
| 移动执行 + 状态恢复 | `src/app/mod.rs::move_tree_path` |
| API 请求 | `src/api/mod.rs::move_doc`(852)、`changeset_one`(794)、`apply_changeset_with_options`(546) |
| 请求/响应 DTO | `src/api/mod.rs` dto 模块:`Change`、`ChangesetRequest`、`ChangesetResult` |
| e2e 契约套件 | `src/api/e2e_test.rs::e2e_contracts` |
