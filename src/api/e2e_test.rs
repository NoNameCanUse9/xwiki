//! Cross-branch contract tests against a running XWiki server.
//!
//! Requires a server on XWIKI_TEST_SERVER (default http://127.0.0.1:9090)
//! with the seeded admin user (admin/secret123). Run with:
//! cargo test e2e_contracts -- --ignored
//!
//! The same suite runs in CI on both branches: the server is built from
//! `main`, the tests run from the `app` checkout.

use super::{Client, dto};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn server_url() -> String {
    std::env::var("XWIKI_TEST_SERVER").unwrap_or_else(|_| "http://127.0.0.1:9090".into())
}

fn unique(prefix: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis();
    format!("{prefix}-{ts}")
}

fn update(path: &str, content: &str) -> dto::Change {
    dto::Change {
        op: "update".into(),
        path: path.into(),
        new_path: None,
        content: Some(content.into()),
    }
}

#[test]
#[ignore]
fn e2e_contracts() {
    let server = server_url();
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async move {
        // ---- 登录 / meta ----
        let admin = Client::new(&server);
        let user = admin.login("admin", "secret123").await.expect("login");
        assert_eq!(user.username, "admin");
        assert!(user.is_admin, "seeded admin must be admin");
        let meta = admin.meta().await.expect("meta");
        assert_eq!(meta.api_version, "1");

        // ---- 项目创建 + 文档写入 ----
        let pid = admin
            .create_project(&unique("contract"), "contract test")
            .await
            .expect("create project")
            .id;
        let base1 = admin.revision(&pid).await.expect("initial revision");
        assert!(!base1.is_empty());
        let first = admin
            .apply_changeset(
                &pid,
                &base1,
                "add page",
                vec![update("guide.md", "# Guide\n\nhello")],
            )
            .await
            .expect("first changeset");
        assert!(!first.commit.is_empty());

        // ---- DocPage revision ----
        let page = admin.page(&pid, "guide.md").await.expect("read page");
        assert!(
            !page.revision.is_empty(),
            "DocPage.revision must be set on reads"
        );
        assert_eq!(
            page.revision, first.revision,
            "page revision must track the writing commit"
        );
        assert!(page.content.contains("hello"));

        // 再写几个 commit,制造分页与搜索语料
        let mut base = first.revision.clone();
        for i in 0..3 {
            let res = admin
                .apply_changeset(
                    &pid,
                    &base,
                    &format!("edit number {i}"),
                    vec![update("guide.md", &format!("# Guide\n\nedit {i}"))],
                )
                .await
                .expect("follow-up changeset");
            base = res.revision;
        }

        // ---- commit 搜索: q + has_more ----
        let one = admin
            .commits_search_page(&pid, "", 1, 0)
            .await
            .expect("commits page");
        assert_eq!(one.commits.len(), 1);
        assert!(one.has_more, "a page below the total must set has_more");
        let hits = admin
            .commits_search_page(&pid, "edit number", 20, 0)
            .await
            .expect("message search");
        assert_eq!(hits.commits.len(), 3, "message substring search");
        let by_sha = admin
            .commits_search_page(&pid, &base[..7], 20, 0)
            .await
            .expect("sha search");
        assert!(!by_sha.commits.is_empty(), "sha prefix search");
        let none = admin
            .commits_search_page(&pid, "zzz-no-such-commit", 20, 0)
            .await
            .expect("empty search");
        assert!(none.commits.is_empty());

        // ---- 锁: 未锁定为 null, 加锁/解锁往返 ----
        assert!(
            admin
                .lock_status(&pid, "guide.md")
                .await
                .expect("unlocked status")
                .is_none(),
            "unlocked path must report no lock"
        );
        let lock = admin
            .acquire_lock(&pid, "guide.md")
            .await
            .expect("acquire lock");
        assert_eq!(lock.path, "guide.md");
        assert!(
            admin
                .lock_status(&pid, "guide.md")
                .await
                .expect("locked status")
                .is_some()
        );
        assert!(
            admin
                .release_lock(&pid, "guide.md")
                .await
                .expect("release lock")
        );
        assert!(
            admin
                .lock_status(&pid, "guide.md")
                .await
                .expect("unlocked again")
                .is_none()
        );

        // ---- changeset 409: 过期 base revision ----
        let stale = admin
            .apply_changeset(
                &pid,
                &base1,
                "stale write",
                vec![update("guide.md", "clobber")],
            )
            .await;
        let err = stale.expect_err("stale base revision must be rejected");
        assert_eq!(err.status, 409, "stale write must be a conflict");
        assert_eq!(err.code, "revision_conflict");

        // ---- Token JSON + 权限矩阵 ----
        let (tok, secret) = admin
            .create_token("ro", "read", vec![pid.clone()])
            .await
            .expect("create read token");
        assert_eq!(tok.scope, "read");
        assert_eq!(tok.project_ids, vec![pid.clone()]);
        assert!(!secret.is_empty());
        let listed = admin.tokens().await.expect("token list");
        assert!(
            listed.iter().any(|t| t.id == tok.id),
            "created token appears in list"
        );

        let ro = Client::with_token(&server, Some(secret.clone()));
        let visible = ro.projects().await.expect("ro project list");
        assert!(
            visible.iter().any(|p| p.id == pid),
            "read token must see its bound project"
        );
        assert!(
            visible.iter().all(|p| p.id == pid),
            "read token must only see its bound project"
        );
        let ro_write = ro
            .apply_changeset(&pid, &base, "token write", vec![update("guide.md", "nope")])
            .await;
        let err = ro_write.expect_err("read token must not write");
        assert_eq!(err.status, 403);
        assert_eq!(err.code, "agent_forbidden");

        // 跨项目: write token 绑定 pid, 写第二个项目 → 403
        let p2 = admin
            .create_project(&unique("other"), "other")
            .await
            .expect("create second project")
            .id;
        let (_, wo_secret) = admin
            .create_token("wo", "write", vec![pid.clone()])
            .await
            .expect("create write token");
        let wo = Client::with_token(&server, Some(wo_secret));
        let cross = wo
            .apply_changeset(&p2, &base, "cross project", vec![update("x.md", "x")])
            .await;
        let err = cross.expect_err("cross-project write must be forbidden");
        assert_eq!(err.status, 403);
        assert_eq!(err.code, "agent_forbidden");

        // 删除是 Session-only: token 即使绑定该项目也被拒
        let del = wo.delete_project(&pid).await;
        let err = del.expect_err("delete must reject bearer tokens");
        assert_eq!(err.status, 403, "delete must be session-only");

        // ---- Bundle 导出 → 导入 ----
        let bundle = admin.export_bundle(&pid).await.expect("export bundle");
        assert!(!bundle.is_empty());
        let imported = admin
            .import_bundle(&unique("bundle"), Arc::new(bundle))
            .await
            .expect("import bundle");
        let ipage = admin
            .page(&imported.project.id, "guide.md")
            .await
            .expect("bundle doc");
        assert!(
            ipage.content.contains("edit 2"),
            "imported bundle must carry the full git history"
        );
        println!("contract suite ok: {pid}");
    });
}
