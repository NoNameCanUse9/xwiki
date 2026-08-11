//! End-to-end smoke test against a running XWiki server.
//!
//! Requires a server on XWIKI_TEST_SERVER (default http://127.0.0.1:9090)
//! with the seeded admin user (admin/secret123). Run with:
//! cargo test -- --ignored

use super::Client;

#[test]
#[ignore]
fn e2e_login_and_projects() {
    let server =
        std::env::var("XWIKI_TEST_SERVER").unwrap_or_else(|_| "http://127.0.0.1:9090".into());
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        let client = Client::new(&server);
        let user = client.login("admin", "secret123").await.expect("login");
        assert_eq!(user.username, "admin");
        let projects = client.projects().await.expect("projects");
        println!("projects: {}", projects.len());
        let meta = client.meta().await.expect("meta");
        assert_eq!(meta.api_version, "1");
    });
}
