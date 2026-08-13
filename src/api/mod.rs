//! XWiki HTTP API client (spec: Rust 客户端 `api` 层).
//!
//! Thin typed client over the Go server's `/api/v1`. Cookie jar is managed
//! by reqwest (session cookie), errors are unwrapped from the uniform
//! `{error:{code,message,request_id}}` envelope.

#[cfg(test)]
mod e2e_test;

#[cfg(test)]
mod tests {
    use super::de_null_default;
    use super::dto;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Wrap {
        #[serde(default, deserialize_with = "de_null_default")]
        items: Vec<String>,
    }

    #[test]
    fn null_slices_become_empty() {
        let w: Wrap = serde_json::from_str(r#"{"items": null}"#).unwrap();
        assert!(w.items.is_empty());
        let w: Wrap = serde_json::from_str(r#"{"items": ["a"]}"#).unwrap();
        assert_eq!(w.items, vec!["a"]);
    }

    #[test]
    fn error_envelope_decodes() {
        let e: dto::ApiErrorBody = serde_json::from_str(
            r#"{"code":"revision_conflict","message":"stale","request_id":"req_x"}"#,
        )
        .unwrap();
        assert_eq!(e.code, "revision_conflict");
        assert_eq!(e.request_id.as_deref(), Some("req_x"));
    }

    #[test]
    fn error_envelope_without_request_id_still_decodes() {
        // The Go server always sends request_id today, but an envelope
        // missing it must not fail the whole decode (that would mask the
        // real error code — e.g. revision_conflict — as http_error).
        let e: dto::ApiErrorBody =
            serde_json::from_str(r#"{"code":"revision_conflict","message":"stale"}"#).unwrap();
        assert_eq!(e.code, "revision_conflict");
        assert_eq!(e.request_id, None);
    }

    #[test]
    fn project_dto_decodes_go_null_fields() {
        let p: dto::Project = serde_json::from_str(
            r#"{"id":"prj_1","name":"docs","repo_dir":"r","archived":false,"created_at":"2026-08-05T00:00:00Z","updated_at":"2026-08-05T00:00:00Z"}"#,
        )
        .unwrap();
        assert_eq!(p.name, "docs");
        assert_eq!(p.description, "");
    }

    #[test]
    fn commit_dto_roundtrip() {
        let c: dto::CommitDetail = serde_json::from_str(
            r#"{"sha":"abc","message":"m","author":"a","date":"d","files":[{"status":"A","path":"docs/x.md"}]}"#,
        )
        .unwrap();
        assert_eq!(c.files.len(), 1);
        assert_eq!(c.files[0].status, "A");
    }
}

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use std::future::Future;
use std::sync::{Arc, OnceLock};

/// Keep binary responses bounded before the GUI writes them to disk. Exports
/// can be large, but never need to occupy an unbounded amount of memory.
pub(crate) const MAX_BUFFERED_RESPONSE_BYTES: usize = 128 << 20;

/// reqwest needs a tokio reactor, but the GUI runs on gpui's own executor.
/// Every request is therefore dispatched onto a process-wide tokio runtime
/// (spawned from any thread; the task itself runs inside the runtime).
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("tokio runtime"))
}

async fn tokio_spawn<F, T>(fut: F) -> Result<T, ApiError>
where
    F: Future<Output = Result<T, ApiError>> + Send + 'static,
    T: Send + 'static,
{
    runtime().handle().spawn(fut).await.map_err(|e| ApiError {
        code: "join_error".into(),
        message: e.to_string(),
        request_id: None,
        status: 0,
    })?
}

/// API error mapped from the server's uniform error envelope.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub request_id: Option<String>,
    pub status: u16,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.request_id {
            Some(id) => write!(
                f,
                "{} ({}): {} [{}]",
                self.code, self.status, self.message, id
            ),
            None => write!(f, "{} ({}): {}", self.code, self.status, self.message),
        }
    }
}

impl std::error::Error for ApiError {}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: dto::ApiErrorBody,
}

/// Client for one XWiki server; keeps the session cookie jar.
#[derive(Clone)]
pub struct Client {
    base: String,
    http: reqwest::Client,
}

impl Client {
    pub fn new(server: &str) -> Self {
        Self::with_credentials(server, None, None)
    }

    pub fn with_token(server: &str, token: Option<String>) -> Self {
        Self::with_credentials(server, token, None)
    }

    /// Rehydrates a client with the server-issued cookie saved by the GUI.
    /// Passwords are never needed for session restoration.
    pub fn with_session(server: &str, cookie: Option<String>) -> Self {
        Self::with_credentials(server, None, cookie)
    }

    fn with_credentials(server: &str, token: Option<String>, cookie: Option<String>) -> Self {
        let base = server.trim_end_matches('/').to_string();
        let mut builder = reqwest::Client::builder()
            .cookie_store(true)
            // Guard against a hung/half-open server leaving every request
            // pending forever (no cancel path on the GUI side).
            .timeout(std::time::Duration::from_secs(60));
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(t) = token {
            // An invalid token (e.g. control chars from the environment)
            // must not panic startup — skip it and let the server reject
            // the anonymous request instead.
            if let Ok(value) = reqwest::header::HeaderValue::from_str(&format!("Bearer {t}")) {
                headers.insert(reqwest::header::AUTHORIZATION, value);
            }
        }
        if let Some(c) = cookie
            && let Ok(value) = reqwest::header::HeaderValue::from_str(&c)
        {
            headers.insert(reqwest::header::COOKIE, value);
        }
        if !headers.is_empty() {
            builder = builder.default_headers(headers);
        }
        Self {
            base,
            http: builder.build().expect("reqwest client"),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    /// Turns a non-success response into the uniform `ApiError`, extracting
    /// the server envelope when it decodes and falling back to a plain HTTP
    /// status otherwise.
    async fn error_from_resp(resp: reqwest::Response) -> ApiError {
        let status = resp.status().as_u16();
        let body: Result<ErrorEnvelope, _> = resp.json().await;
        match body {
            Ok(env) => ApiError {
                code: env.error.code,
                message: env.error.message,
                request_id: env.error.request_id,
                status,
            },
            Err(_) => ApiError {
                code: "http_error".into(),
                message: format!("HTTP {status}"),
                request_id: None,
                status,
            },
        }
    }

    async fn send<T: DeserializeOwned + Send + 'static>(
        req: reqwest::RequestBuilder,
    ) -> Result<T, ApiError> {
        tokio_spawn(async move {
            let resp = req.send().await.map_err(network_error)?;
            let status = resp.status().as_u16();
            if !resp.status().is_success() {
                return Err(Self::error_from_resp(resp).await);
            }
            resp.json().await.map_err(|e| ApiError {
                code: "decode_error".into(),
                message: e.to_string(),
                request_id: None,
                status,
            })
        })
        .await
    }

    /// Send a request whose successful response is an opaque byte stream.
    /// This is used for ZIP/Bundle exports and attachment downloads; keeping
    /// it separate from `send` avoids accidentally decoding binary data as
    /// JSON or buffering it in a string.
    async fn send_bytes(req: reqwest::RequestBuilder) -> Result<Vec<u8>, ApiError> {
        tokio_spawn(async move {
            let mut resp = req.send().await.map_err(network_error)?;
            let status = resp.status().as_u16();
            if !resp.status().is_success() {
                return Err(Self::error_from_resp(resp).await);
            }
            if resp
                .content_length()
                .is_some_and(|length| length > MAX_BUFFERED_RESPONSE_BYTES as u64)
            {
                return Err(ApiError {
                    code: "response_too_large".into(),
                    message: format!(
                        "响应超过 {} MiB 内存缓冲限制",
                        MAX_BUFFERED_RESPONSE_BYTES / (1 << 20)
                    ),
                    request_id: None,
                    status,
                });
            }
            let mut bytes = Vec::with_capacity(
                resp.content_length()
                    .unwrap_or_default()
                    .min(MAX_BUFFERED_RESPONSE_BYTES as u64) as usize,
            );
            while let Some(chunk) = resp.chunk().await.map_err(|e| ApiError {
                code: "decode_error".into(),
                message: e.to_string(),
                request_id: None,
                status,
            })? {
                if bytes.len().saturating_add(chunk.len()) > MAX_BUFFERED_RESPONSE_BYTES {
                    return Err(ApiError {
                        code: "response_too_large".into(),
                        message: format!(
                            "响应超过 {} MiB 内存缓冲限制",
                            MAX_BUFFERED_RESPONSE_BYTES / (1 << 20)
                        ),
                        request_id: None,
                        status,
                    });
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(bytes)
        })
        .await
    }

    async fn send_with_session<T: DeserializeOwned + Send + 'static>(
        req: reqwest::RequestBuilder,
    ) -> Result<(T, Option<String>), ApiError> {
        tokio_spawn(async move {
            let resp = req.send().await.map_err(network_error)?;
            let status = resp.status().as_u16();
            let session_cookie = response_session_cookie(&resp);
            if !resp.status().is_success() {
                return Err(Self::error_from_resp(resp).await);
            }
            let value = resp.json().await.map_err(|e| ApiError {
                code: "decode_error".into(),
                message: e.to_string(),
                request_id: None,
                status,
            })?;
            Ok((value, session_cookie))
        })
        .await
    }

    pub async fn meta(&self) -> Result<dto::Meta, ApiError> {
        Self::send(self.http.get(self.url("/api/v1/meta"))).await
    }

    /// Fetch the latest public desktop release from GitHub. This uses a
    /// separate client so the XWiki session cookie/token is never sent to
    /// GitHub.
    pub async fn latest_github_release(
        owner: &str,
        repo: &str,
    ) -> Result<dto::GithubRelease, ApiError> {
        let http = reqwest::Client::builder()
            .user_agent("xwiki")
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|error| ApiError {
                code: "client_error".into(),
                message: error.to_string(),
                request_id: None,
                status: 0,
            })?;
        let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
        Self::send(
            http.get(url)
                .header(reqwest::header::ACCEPT, "application/vnd.github+json"),
        )
        .await
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<dto::User, ApiError> {
        self.login_with_session(username, password)
            .await
            .map(|(user, _)| user)
    }

    /// Login and return the server-issued cookie so the GUI can persist it.
    pub async fn login_with_session(
        &self,
        username: &str,
        password: &str,
    ) -> Result<(dto::User, Option<String>), ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            username: &'a str,
            password: &'a str,
        }
        let (resp, cookie): (dto::UserResponse, Option<String>) = Self::send_with_session(
            self.http
                .post(self.url("/api/v1/auth/login"))
                .json(&Body { username, password }),
        )
        .await?;
        Ok((resp.user, cookie))
    }

    pub async fn me(&self) -> Result<dto::User, ApiError> {
        let resp: dto::UserResponse =
            Self::send(self.http.get(self.url("/api/v1/auth/me"))).await?;
        Ok(resp.user)
    }

    pub async fn projects(&self) -> Result<Vec<dto::Project>, ApiError> {
        let resp: dto::ProjectsResponse =
            Self::send(self.http.get(self.url("/api/v1/projects"))).await?;
        Ok(resp.projects)
    }

    pub async fn create_project(
        &self,
        name: &str,
        description: &str,
    ) -> Result<dto::Project, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            #[serde(skip_serializing_if = "str::is_empty")]
            description: &'a str,
        }
        let resp: dto::ProjectResponse = Self::send(
            self.http
                .post(self.url("/api/v1/projects"))
                .json(&Body { name, description }),
        )
        .await?;
        Ok(resp.project)
    }

    pub async fn tree(
        &self,
        project_id: &str,
        path: &str,
    ) -> Result<Vec<dto::TreeEntry>, ApiError> {
        let resp: dto::TreeResponse = Self::send(
            self.http
                .get(self.url(&format!("/api/v1/projects/{}/docs/tree", project_id)))
                .query(&[("path", path)]),
        )
        .await?;
        Ok(resp.tree)
    }

    /// Reads a doc as markdown source (format=raw).
    pub async fn page(&self, project_id: &str, path: &str) -> Result<dto::DocPage, ApiError> {
        Self::send(
            self.http
                .get(self.url(&format!(
                    "/api/v1/projects/{}/docs/pages/{}",
                    project_id, path
                )))
                .query(&[("format", "raw")]),
        )
        .await
    }

    pub async fn page_at(
        &self,
        project_id: &str,
        path: &str,
        revision: &str,
    ) -> Result<dto::DocPage, ApiError> {
        Self::send(
            self.http
                .get(self.url(&format!(
                    "/api/v1/projects/{}/docs/pages/{}",
                    project_id, path
                )))
                .query(&[("format", "raw"), ("at", revision)]),
        )
        .await
    }

    pub async fn revision(&self, project_id: &str) -> Result<String, ApiError> {
        let resp: dto::RevisionResponse = Self::send(
            self.http
                .get(self.url(&format!("/api/v1/projects/{}/revision", project_id))),
        )
        .await?;
        Ok(resp.revision)
    }

    pub async fn apply_changeset(
        &self,
        project_id: &str,
        base_revision: &str,
        message: &str,
        changes: Vec<dto::Change>,
    ) -> Result<dto::ChangesetResult, ApiError> {
        let body = dto::ChangesetRequest {
            base_revision: base_revision.to_string(),
            message: message.to_string(),
            changes,
        };
        Self::send(
            self.http
                .post(self.url(&format!("/api/v1/projects/{}/changesets", project_id)))
                .json(&body),
        )
        .await
    }

    pub async fn acquire_lock(&self, project_id: &str, path: &str) -> Result<dto::Lock, ApiError> {
        let resp: dto::LockResponse = Self::send(
            self.http
                .post(self.url(&format!("/api/v1/projects/{}/locks", project_id)))
                .query(&[("path", path)]),
        )
        .await?;
        resp.lock.ok_or_else(|| ApiError {
            code: "lock_missing".into(),
            message: "server returned no lock".into(),
            request_id: None,
            status: 500,
        })
    }

    pub async fn heartbeat_lock(
        &self,
        project_id: &str,
        path: &str,
    ) -> Result<dto::Lock, ApiError> {
        let resp: dto::LockResponse = Self::send(
            self.http
                .post(self.url(&format!("/api/v1/projects/{}/locks/heartbeat", project_id)))
                .query(&[("path", path)]),
        )
        .await?;
        resp.lock.ok_or_else(|| ApiError {
            code: "lock_missing".into(),
            message: "server returned no lock".into(),
            request_id: None,
            status: 500,
        })
    }

    pub async fn release_lock(&self, project_id: &str, path: &str) -> Result<bool, ApiError> {
        let resp: dto::ReleasedResponse = Self::send(
            self.http
                .delete(self.url(&format!("/api/v1/projects/{}/locks", project_id)))
                .query(&[("path", path)]),
        )
        .await?;
        Ok(resp.released)
    }

    pub async fn lock_status(
        &self,
        project_id: &str,
        path: &str,
    ) -> Result<Option<dto::Lock>, ApiError> {
        let resp: dto::LockResponse = Self::send(
            self.http
                .get(self.url(&format!("/api/v1/projects/{}/locks", project_id)))
                .query(&[("path", path)]),
        )
        .await?;
        Ok(resp.lock)
    }

    pub async fn commits(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<dto::Commit>, ApiError> {
        self.commits_page(project_id, limit, 0).await
    }

    pub async fn commits_page(
        &self,
        project_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<dto::Commit>, ApiError> {
        Ok(self
            .commits_search_page(project_id, "", limit, offset)
            .await?
            .commits)
    }

    pub async fn commits_search_page(
        &self,
        project_id: &str,
        query: &str,
        limit: u32,
        offset: u32,
    ) -> Result<dto::CommitListResponse, ApiError> {
        let resp: dto::CommitListResponse = Self::send(
            self.http
                .get(self.url(&format!("/api/v1/projects/{}/commits", project_id)))
                .query(&[
                    ("q", query.to_string()),
                    ("limit", limit.to_string()),
                    ("offset", offset.to_string()),
                ]),
        )
        .await?;
        Ok(resp)
    }

    pub async fn commit_detail(
        &self,
        project_id: &str,
        sha: &str,
    ) -> Result<dto::CommitDetail, ApiError> {
        let resp: dto::CommitDetailResponse = Self::send(
            self.http
                .get(self.url(&format!("/api/v1/projects/{}/commits/{}", project_id, sha))),
        )
        .await?;
        Ok(resp.commit)
    }

    pub async fn project(&self, project_id: &str) -> Result<dto::Project, ApiError> {
        let resp: dto::ProjectResponse = Self::send(
            self.http
                .get(self.url(&format!("/api/v1/projects/{project_id}"))),
        )
        .await?;
        Ok(resp.project)
    }

    pub async fn set_archived(
        &self,
        project_id: &str,
        archived: bool,
    ) -> Result<dto::Project, ApiError> {
        let path = if archived { "archive" } else { "unarchive" };
        let resp: dto::ProjectResponse = Self::send(
            self.http
                .post(self.url(&format!("/api/v1/projects/{project_id}/{path}"))),
        )
        .await?;
        Ok(resp.project)
    }

    /// Rename a project. The server updates metadata and refreshes the
    /// repository README headline in one call.
    pub async fn rename_project(
        &self,
        project_id: &str,
        name: &str,
    ) -> Result<dto::Project, ApiError> {
        #[derive(Serialize)]
        struct RenameBody<'a> {
            name: &'a str,
        }
        let resp: dto::ProjectResponse = Self::send(
            self.http
                .patch(self.url(&format!("/api/v1/projects/{project_id}")))
                .json(&RenameBody { name }),
        )
        .await?;
        Ok(resp.project)
    }

    /// Delete a project completely: metadata + repository (irreversible).
    pub async fn delete_project(&self, project_id: &str) -> Result<(), ApiError> {
        let _: serde_json::Value = Self::send(
            self.http
                .delete(self.url(&format!("/api/v1/projects/{project_id}"))),
        )
        .await?;
        Ok(())
    }

    /// Hard-delete paths from the project's git history (rewrites history,
    /// irreversible).
    pub async fn purge_paths(
        &self,
        project_id: &str,
        paths: &[String],
        message: &str,
    ) -> Result<(), ApiError> {
        #[derive(Serialize)]
        struct PurgeBody<'a> {
            paths: &'a [String],
            message: &'a str,
        }
        let _: serde_json::Value = Self::send(
            self.http
                .post(self.url(&format!("/api/v1/projects/{project_id}/purge")))
                .json(&PurgeBody { paths, message }),
        )
        .await?;
        Ok(())
    }

    async fn changeset_one(
        &self,
        project_id: &str,
        path: &str,
        op: &str,
        content: Option<String>,
        new_path: Option<String>,
        message: &str,
    ) -> Result<dto::ChangesetResult, ApiError> {
        let resp: dto::RevisionResponse = Self::send(
            self.http
                .get(self.url(&format!("/api/v1/projects/{project_id}/revision"))),
        )
        .await?;
        self.apply_changeset(
            project_id,
            &resp.revision,
            message,
            vec![dto::Change {
                op: op.into(),
                path: path.into(),
                new_path,
                content,
            }],
        )
        .await
    }

    pub async fn edit_doc(
        &self,
        project_id: &str,
        path: &str,
        op: &str,
        content: &str,
        message: &str,
    ) -> Result<dto::ChangesetResult, ApiError> {
        self.changeset_one(
            project_id,
            path,
            op,
            Some(content.to_string()),
            None,
            message,
        )
        .await
    }

    pub async fn delete_doc(
        &self,
        project_id: &str,
        path: &str,
        message: &str,
    ) -> Result<dto::ChangesetResult, ApiError> {
        self.changeset_one(project_id, path, "delete", None, None, message)
            .await
    }

    pub async fn move_doc(
        &self,
        project_id: &str,
        from: &str,
        to: &str,
        message: &str,
    ) -> Result<dto::ChangesetResult, ApiError> {
        self.changeset_one(
            project_id,
            from,
            "move",
            None,
            Some(to.to_string()),
            message,
        )
        .await
    }

    pub async fn search(
        &self,
        project_id: &str,
        q: &str,
    ) -> Result<Vec<dto::SearchResult>, ApiError> {
        let resp: dto::SearchResponse = Self::send(
            self.http
                .get(self.url(&format!("/api/v1/projects/{project_id}/search")))
                .query(&[("q", q)]),
        )
        .await?;
        Ok(resp.results)
    }

    /// Create (or reuse) the public share for one document.
    pub async fn create_share(&self, project_id: &str, path: &str) -> Result<dto::Share, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            path: &'a str,
        }
        Self::send(
            self.http
                .post(self.url(&format!("/api/v1/projects/{project_id}/shares")))
                .json(&Body { path }),
        )
        .await
    }

    pub async fn backlinks(
        &self,
        project_id: &str,
        path: &str,
    ) -> Result<Vec<dto::Backlink>, ApiError> {
        let resp: dto::BacklinksResponse = Self::send(
            self.http
                .get(self.url(&format!("/api/v1/projects/{project_id}/backlinks")))
                .query(&[("path", path)]),
        )
        .await?;
        Ok(resp.backlinks)
    }

    pub async fn file_history(
        &self,
        project_id: &str,
        path: &str,
    ) -> Result<Vec<dto::Commit>, ApiError> {
        let resp: dto::FileHistoryResponse = Self::send(self.http.get(self.url(&format!(
            "/api/v1/projects/{project_id}/files/history/{path}"
        ))))
        .await?;
        Ok(resp.commits)
    }

    pub async fn export_zip(&self, project_id: &str) -> Result<Vec<u8>, ApiError> {
        Self::send_bytes(
            self.http
                .get(self.url(&format!("/api/v1/projects/{project_id}/export.zip"))),
        )
        .await
    }

    pub async fn export_bundle(&self, project_id: &str) -> Result<Vec<u8>, ApiError> {
        Self::send_bytes(
            self.http
                .get(self.url(&format!("/api/v1/projects/{project_id}/export.bundle"))),
        )
        .await
    }

    pub async fn download_attachment(
        &self,
        project_id: &str,
        path: &str,
    ) -> Result<Vec<u8>, ApiError> {
        Self::send_bytes(
            self.http
                .get(self.url(&format!("/api/v1/projects/{project_id}/attachments/{path}"))),
        )
        .await
    }

    pub async fn openapi(&self) -> Result<serde_json::Value, ApiError> {
        Self::send(self.http.get(self.url("/api/openapi.json"))).await
    }

    pub async fn import_files(
        &self,
        project_id: &str,
        base_revision: &str,
        message: &str,
        files: Vec<dto::ImportFile>,
    ) -> Result<dto::ImportResult, ApiError> {
        let body = dto::ImportRequest {
            base_revision: base_revision.to_string(),
            message: message.to_string(),
            files,
        };
        Self::send(
            self.http
                .post(self.url(&format!("/api/v1/projects/{project_id}/import")))
                .json(&body),
        )
        .await
    }

    /// Backwards-compatible name for callers that used the original import
    /// endpoint wrapper before it supported arbitrary project files.
    #[allow(dead_code)]
    pub async fn import_repo(
        &self,
        name: &str,
        url: &str,
    ) -> Result<dto::ImportProjectResult, ApiError> {
        Self::send(
            self.http
                .post(self.url("/api/v1/import/repo"))
                .query(&[("name", name), ("url", url)]),
        )
        .await
    }

    pub async fn import_bundle(
        &self,
        name: &str,
        bundle: Arc<Vec<u8>>,
    ) -> Result<dto::ImportProjectResult, ApiError> {
        let part =
            reqwest::multipart::Part::bytes(bundle.as_ref().clone()).file_name("project.bundle");
        let form = reqwest::multipart::Form::new()
            .text("name", name.to_string())
            .part("file", part);
        Self::send(
            self.http
                .post(self.url("/api/v1/import/bundle"))
                .query(&[("name", name)])
                .multipart(form),
        )
        .await
    }

    pub async fn import_folder(
        &self,
        name: &str,
        description: &str,
        files: Arc<Vec<dto::UploadFile>>,
    ) -> Result<dto::ImportProjectResult, ApiError> {
        let mut form = reqwest::multipart::Form::new()
            .text("name", name.to_string())
            .text("description", description.to_string());
        for file in files.iter() {
            let path = file.path.clone();
            form = form.text("paths", path.clone()).part(
                "files",
                reqwest::multipart::Part::bytes(file.content.clone()).file_name(path),
            );
        }
        Self::send(
            self.http
                .post(self.url("/api/v1/projects/import-folder"))
                .multipart(form),
        )
        .await
    }

    /// Upload an attachment through the same atomic changeset endpoint used
    /// by the web client. The server stores the content as base64 in Git.
    pub async fn upload_attachment(
        &self,
        project_id: &str,
        base_revision: &str,
        path: &str,
        content_base64: &str,
    ) -> Result<dto::ChangesetResult, ApiError> {
        #[derive(Serialize)]
        struct EncodedChange<'a> {
            op: &'a str,
            path: &'a str,
            content: &'a str,
            encoding: &'a str,
        }
        #[derive(Serialize)]
        struct Body<'a> {
            base_revision: &'a str,
            message: &'a str,
            changes: Vec<EncodedChange<'a>>,
        }
        Self::send(
            self.http
                .post(self.url(&format!("/api/v1/projects/{project_id}/changesets")))
                .json(&Body {
                    base_revision,
                    message: "",
                    changes: vec![EncodedChange {
                        op: "create",
                        path,
                        content: content_base64,
                        encoding: "base64",
                    }],
                }),
        )
        .await
    }

    pub async fn delete_attachment(
        &self,
        project_id: &str,
        base_revision: &str,
        path: &str,
    ) -> Result<dto::ChangesetResult, ApiError> {
        self.apply_changeset(
            project_id,
            base_revision,
            "删除附件",
            vec![dto::Change {
                op: "delete".into(),
                path: path.into(),
                new_path: None,
                content: None,
            }],
        )
        .await
    }

    pub async fn tokens(&self) -> Result<Vec<dto::Token>, ApiError> {
        let resp: dto::TokenListResponse =
            Self::send(self.http.get(self.url("/api/v1/tokens"))).await?;
        Ok(resp.tokens)
    }

    pub async fn create_token(
        &self,
        name: &str,
        scope: &str,
        project_ids: Vec<String>,
    ) -> Result<(dto::Token, String), ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            name: &'a str,
            scope: &'a str,
            project_ids: Vec<String>,
        }
        let resp: dto::TokenCreateResponse =
            Self::send(self.http.post(self.url("/api/v1/tokens")).json(&Body {
                name,
                scope,
                project_ids,
            }))
            .await?;
        Ok((resp.token, resp.secret))
    }

    pub async fn revoke_token(&self, id: &str) -> Result<(), ApiError> {
        Self::send(self.http.delete(self.url(&format!("/api/v1/tokens/{id}")))).await
    }

    pub async fn users(&self) -> Result<Vec<dto::User>, ApiError> {
        let resp: dto::UsersResponse = Self::send(self.http.get(self.url("/api/v1/users"))).await?;
        Ok(resp.users)
    }

    pub async fn create_user(&self, username: &str, password: &str) -> Result<dto::User, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            username: &'a str,
            password: &'a str,
        }
        let resp: dto::UserResponse = Self::send(
            self.http
                .post(self.url("/api/v1/users"))
                .json(&Body { username, password }),
        )
        .await?;
        Ok(resp.user)
    }

    pub async fn set_user_enabled(&self, id: &str, enabled: bool) -> Result<dto::User, ApiError> {
        let action = if enabled { "enable" } else { "disable" };
        let resp: dto::UserResponse = Self::send(
            self.http
                .post(self.url(&format!("/api/v1/users/{id}/{action}"))),
        )
        .await?;
        Ok(resp.user)
    }

    pub async fn audit(&self, project_id: &str) -> Result<Vec<dto::AuditEntry>, ApiError> {
        let resp: dto::AuditResponse = Self::send(
            self.http
                .get(self.url(&format!("/api/v1/projects/{project_id}/audit"))),
        )
        .await?;
        Ok(resp.entries)
    }

    pub async fn diff_stats(
        &self,
        project_id: &str,
        sha: &str,
    ) -> Result<Vec<dto::DiffStat>, ApiError> {
        let resp: dto::DiffStatsResponse = Self::send(
            self.http
                .get(self.url(&format!(
                    "/api/v1/projects/{}/commits/{}/diff",
                    project_id, sha
                )))
                .query(&[("format", "numstat")]),
        )
        .await?;
        Ok(resp.stats)
    }

    /// Full patch diff for a commit (file-level unified diff lines).
    pub async fn commit_patch(
        &self,
        project_id: &str,
        sha: &str,
    ) -> Result<dto::CommitPatch, ApiError> {
        let resp: dto::CommitPatchResponse = Self::send(
            self.http
                .get(self.url(&format!(
                    "/api/v1/projects/{}/commits/{}/diff",
                    project_id, sha
                )))
                .query(&[("format", "patch")]),
        )
        .await?;
        Ok(dto::CommitPatch {
            sha: resp.sha,
            format: resp.format,
            patch: resp.patch,
        })
    }

    /// Revert a commit with a new changeset commit (server-side).
    pub async fn revert_commit(
        &self,
        project_id: &str,
        sha: &str,
        message: &str,
    ) -> Result<dto::Commit, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(skip_serializing_if = "str::is_empty")]
            message: &'a str,
        }
        let resp: dto::CommitResponse = Self::send(
            self.http
                .post(self.url(&format!(
                    "/api/v1/projects/{}/commits/{}/revert",
                    project_id, sha
                )))
                .json(&Body { message }),
        )
        .await?;
        Ok(resp.commit)
    }

    /// Request a password-reset token (self-hosted: token goes to the
    /// server log; response is identical for unknown accounts).
    pub async fn forgot_password(&self, username: &str) -> Result<bool, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            username: &'a str,
        }
        let resp: serde_json::Value = Self::send(
            self.http
                .post(self.url("/api/v1/auth/forgot-password"))
                .json(&Body { username }),
        )
        .await?;
        Ok(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false))
    }

    /// Reset a password with a one-time token.
    pub async fn reset_password(&self, token: &str, new_password: &str) -> Result<bool, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            token: &'a str,
            new_password: &'a str,
        }
        let resp: serde_json::Value = Self::send(
            self.http
                .post(self.url("/api/v1/auth/reset-password"))
                .json(&Body {
                    token,
                    new_password,
                }),
        )
        .await?;
        Ok(resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false))
    }
}

fn response_session_cookie(response: &reqwest::Response) -> Option<String> {
    let cookies: Vec<String> = response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    (!cookies.is_empty()).then(|| cookies.join("; "))
}

fn network_error(e: reqwest::Error) -> ApiError {
    ApiError {
        code: "network_error".into(),
        message: format!("无法连接服务: {e}"),
        request_id: None,
        status: 0,
    }
}

/// Deserializes `null` as the field's default (Go servers emit null for
/// nil slices/pointers).
pub(crate) fn de_null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// Shared types for the api layer. Mirrors `doc/api.md` response shapes.
/// DTO fields are API contract — the UI reads a subset and serde fills the
/// rest, so field-level dead-code warnings are silenced.
#[allow(dead_code)]
pub mod dto {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize)]
    pub struct ApiErrorBody {
        pub code: String,
        pub message: String,
        #[serde(default)]
        pub request_id: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct User {
        pub id: String,
        pub username: String,
        #[serde(default)]
        pub display_name: String,
        #[serde(default)]
        pub is_admin: bool,
        #[serde(default)]
        pub disabled: bool,
    }

    #[derive(Debug, Deserialize)]
    pub struct UserResponse {
        pub user: User,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct Project {
        pub id: String,
        pub name: String,
        #[serde(default)]
        pub description: String,
        #[serde(default)]
        pub archived: bool,
        #[serde(default)]
        pub created_at: String,
        #[serde(default)]
        pub updated_at: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct ProjectsResponse {
        // Go marshals nil slices as null; treat it as an empty list.
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub projects: Vec<Project>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ProjectResponse {
        pub project: Project,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct TreeEntry {
        pub name: String,
        pub r#type: String,
        pub path: String,
        #[serde(default)]
        pub sha: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct TreeResponse {
        pub path: String,
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub tree: Vec<TreeEntry>,
    }

    #[derive(Debug, Deserialize)]
    pub struct DocPage {
        pub path: String,
        pub format: String,
        #[serde(default)]
        pub content: String,
        #[serde(default)]
        pub revision: String,
        #[serde(default)]
        pub encoding: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct RevisionResponse {
        pub revision: String,
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct Change {
        pub op: String,
        pub path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub new_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub content: Option<String>,
    }

    #[derive(Debug, Serialize)]
    pub struct ChangesetRequest {
        pub base_revision: String,
        pub message: String,
        pub changes: Vec<Change>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ChangesetResult {
        pub commit: String,
        pub revision: String,
        #[serde(default)]
        pub preview: Option<serde_json::Value>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Lock {
        pub path: String,
        pub user_id: String,
        pub username: String,
        pub acquired_at: String,
        pub expires_at: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct LockResponse {
        pub lock: Option<Lock>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ReleasedResponse {
        pub released: bool,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct Commit {
        pub sha: String,
        pub message: String,
        pub author: String,
        pub date: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct CommitListResponse {
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub commits: Vec<Commit>,
        #[serde(default)]
        pub has_more: bool,
    }

    #[derive(Debug, Deserialize)]
    pub struct CommitFile {
        pub status: String,
        pub path: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct CommitDetail {
        pub sha: String,
        pub message: String,
        pub author: String,
        pub date: String,
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub files: Vec<CommitFile>,
    }

    #[derive(Debug, Deserialize)]
    pub struct CommitDetailResponse {
        pub commit: CommitDetail,
    }

    #[derive(Debug, Deserialize)]
    pub struct DiffStat {
        pub path: String,
        pub added: u32,
        pub deleted: u32,
    }

    #[derive(Debug, Deserialize)]
    pub struct DiffStatsResponse {
        pub sha: String,
        pub format: String,
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub stats: Vec<DiffStat>,
    }

    /// Patch diff for one commit (`format=patch`): server returns the
    /// unified diff as a single text block.
    #[derive(Debug, Deserialize)]
    pub struct CommitPatchResponse {
        pub sha: String,
        pub format: String,
        #[serde(default)]
        pub patch: String,
    }

    #[derive(Debug, Clone)]
    pub struct CommitPatch {
        pub sha: String,
        pub format: String,
        pub patch: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct CommitResponse {
        pub commit: Commit,
    }

    #[derive(Debug, Deserialize)]
    pub struct SearchResult {
        pub path: String,
        pub snippet: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct SearchResponse {
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub results: Vec<SearchResult>,
    }

    #[derive(Debug, Deserialize, Clone)]
    pub struct Share {
        pub token: String,
        pub url: String,
    }

    #[derive(Debug, Deserialize, Clone)]
    pub struct Backlink {
        pub source: String,
        pub snippet: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct BacklinksResponse {
        #[serde(default)]
        pub path: String,
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub backlinks: Vec<Backlink>,
    }

    #[derive(Debug, Deserialize)]
    pub struct FileHistoryResponse {
        pub path: String,
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub commits: Vec<Commit>,
    }

    #[derive(Debug, Clone, Serialize)]
    pub struct ImportFile {
        pub path: String,
        pub content: String,
    }

    #[derive(Debug, Serialize)]
    pub struct ImportRequest {
        pub base_revision: String,
        pub message: String,
        pub files: Vec<ImportFile>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ImportResult {
        pub commit: String,
        pub revision: String,
        pub imported: u32,
    }

    #[derive(Debug, Deserialize)]
    pub struct ImportProjectResult {
        pub project: Project,
        pub commits: u32,
    }

    #[derive(Debug, Clone)]
    pub struct UploadFile {
        pub path: String,
        pub content: Vec<u8>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Token {
        pub id: String,
        pub name: String,
        pub scope: String,
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub project_ids: Vec<String>,
        #[serde(default)]
        pub created_at: String,
        #[serde(default)]
        pub revoked_at: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct TokenListResponse {
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub tokens: Vec<Token>,
    }

    #[derive(Debug, Deserialize)]
    pub struct TokenCreateResponse {
        pub token: Token,
        pub secret: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct UsersResponse {
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub users: Vec<User>,
    }

    #[derive(Debug, Deserialize)]
    pub struct AuditEntry {
        pub id: String,
        pub actor_type: String,
        pub actor_id: String,
        #[serde(default)]
        pub action: String,
        #[serde(default)]
        pub path: String,
        #[serde(default)]
        pub created_at: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct AuditResponse {
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub entries: Vec<AuditEntry>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Meta {
        pub version: String,
        pub api_version: String,
        pub limits: MetaLimits,
        pub capabilities: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct GithubRelease {
        pub tag_name: String,
        #[serde(default)]
        pub name: String,
        pub html_url: String,
        #[serde(default)]
        pub published_at: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct MetaLimits {
        pub max_doc_bytes: u64,
        pub max_import_bytes: u64,
        pub max_diff_bytes: u64,
        pub max_changes_per_request: u64,
    }
}
