//! AgentDocs HTTP API client (spec: Rust 客户端 `api` 层).
//!
//! Thin typed client over the Go server's `/api/v1`. Cookie jar is managed
//! by reqwest (session cookie), errors are unwrapped from the uniform
//! `{error:{code,message,request_id}}` envelope.

#[cfg(test)]
mod e2e_test;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

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
            Some(id) => write!(f, "{} ({}): {} [{}]", self.code, self.status, self.message, id),
            None => write!(f, "{} ({}): {}", self.code, self.status, self.message),
        }
    }
}

impl std::error::Error for ApiError {}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: dto::ApiErrorBody,
}

/// Client for one AgentDocs server; keeps the session cookie jar.
#[derive(Clone)]
pub struct Client {
    base: String,
    http: reqwest::Client,
}

impl Client {
    pub fn new(server: &str) -> Self {
        let base = server.trim_end_matches('/').to_string();
        Self {
            base,
            http: reqwest::Client::builder()
                .cookie_store(true)
                .build()
                .expect("reqwest client"),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    async fn send<T: DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<T, ApiError> {
        let resp = req.send().await.map_err(network_error)?;
        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body: Result<ErrorEnvelope, _> = resp.json().await;
            let (code, message, request_id) = match body {
                Ok(env) => (env.error.code, env.error.message, Some(env.error.request_id)),
                Err(_) => ("http_error".into(), format!("HTTP {}", status), None),
            };
            return Err(ApiError { code, message, request_id, status });
        }
        resp.json().await.map_err(|e| ApiError {
            code: "decode_error".into(),
            message: e.to_string(),
            request_id: None,
            status,
        })
    }

    pub async fn meta(&self) -> Result<dto::Meta, ApiError> {
        self.send(self.http.get(self.url("/api/v1/meta"))).await
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<dto::User, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            username: &'a str,
            password: &'a str,
        }
        let resp: dto::UserResponse = self
            .send(
                self.http
                    .post(self.url("/api/v1/auth/login"))
                    .json(&Body { username, password }),
            )
            .await?;
        Ok(resp.user)
    }

    #[allow(dead_code)] // used by the CLI milestone
    pub async fn me(&self) -> Result<dto::User, ApiError> {
        let resp: dto::UserResponse = self
            .send(self.http.get(self.url("/api/v1/auth/me")))
            .await?;
        Ok(resp.user)
    }

    pub async fn projects(&self) -> Result<Vec<dto::Project>, ApiError> {
        let resp: dto::ProjectsResponse = self
            .send(self.http.get(self.url("/api/v1/projects")))
            .await?;
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
        let resp: dto::ProjectResponse = self
            .send(
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
        let resp: dto::TreeResponse = self
            .send(
                self.http
                    .get(self.url(&format!(
                        "/api/v1/projects/{}/docs/tree",
                        project_id
                    )))
                    .query(&[("path", path)]),
            )
            .await?;
        Ok(resp.tree)
    }

    /// Reads a doc as markdown source (format=raw).
    pub async fn page(
        &self,
        project_id: &str,
        path: &str,
    ) -> Result<dto::DocPage, ApiError> {
        self.send(
            self.http
                .get(self.url(&format!(
                    "/api/v1/projects/{}/docs/pages/{}",
                    project_id, path
                )))
                .query(&[("format", "raw")]),
        )
        .await
    }

    pub async fn revision(&self, project_id: &str) -> Result<String, ApiError> {
        let resp: dto::RevisionResponse = self
            .send(
                self.http
                    .get(self.url(&format!(
                        "/api/v1/projects/{}/revision",
                        project_id
                    ))),
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
        self.send(
            self.http
                .post(self.url(&format!(
                    "/api/v1/projects/{}/changesets",
                    project_id
                )))
                .json(&body),
        )
        .await
    }

    pub async fn acquire_lock(&self, project_id: &str, path: &str) -> Result<dto::Lock, ApiError> {
        self.send(
            self.http
                .post(self.url(&format!(
                    "/api/v1/projects/{}/locks",
                    project_id
                )))
                .query(&[("path", path)]),
        )
        .await
    }

    pub async fn heartbeat_lock(
        &self,
        project_id: &str,
        path: &str,
    ) -> Result<dto::Lock, ApiError> {
        self.send(
            self.http
                .post(self.url(&format!(
                    "/api/v1/projects/{}/locks/heartbeat",
                    project_id
                )))
                .query(&[("path", path)]),
        )
        .await
    }

    pub async fn release_lock(&self, project_id: &str, path: &str) -> Result<bool, ApiError> {
        let resp: dto::ReleasedResponse = self
            .send(
                self.http
                    .delete(self.url(&format!(
                        "/api/v1/projects/{}/locks",
                        project_id
                    )))
                    .query(&[("path", path)]),
            )
            .await?;
        Ok(resp.released)
    }

    pub async fn commits(
        &self,
        project_id: &str,
        limit: u32,
    ) -> Result<Vec<dto::Commit>, ApiError> {
        let resp: dto::CommitListResponse = self
            .send(
                self.http
                    .get(self.url(&format!(
                        "/api/v1/projects/{}/commits",
                        project_id
                    )))
                    .query(&[("limit", limit.to_string())]),
            )
            .await?;
        Ok(resp.commits)
    }

    pub async fn commit_detail(
        &self,
        project_id: &str,
        sha: &str,
    ) -> Result<dto::CommitDetail, ApiError> {
        let resp: dto::CommitDetailResponse = self
            .send(
                self.http
                    .get(self.url(&format!(
                        "/api/v1/projects/{}/commits/{}",
                        project_id, sha
                    ))),
            )
            .await?;
        Ok(resp.commit)
    }

    pub async fn diff_stats(
        &self,
        project_id: &str,
        sha: &str,
    ) -> Result<Vec<dto::DiffStat>, ApiError> {
        let resp: dto::DiffStatsResponse = self
            .send(
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
        pub request_id: String,
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

    #[derive(Debug, Deserialize)]
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

    #[derive(Debug, Deserialize)]
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
    pub struct ReleasedResponse {
        pub released: bool,
    }

    #[derive(Debug, Deserialize)]
    pub struct Commit {
        pub sha: String,
        pub message: String,
        pub author: String,
        pub date: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct CommitListResponse {
        #[serde(default, deserialize_with = "crate::api::de_null_default")]
        pub commits: Vec<Commit>,
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

    #[derive(Debug, Deserialize)]
    pub struct Meta {
        pub version: String,
        pub api_version: String,
        pub limits: MetaLimits,
        pub capabilities: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct MetaLimits {
        pub max_doc_bytes: u64,
        pub max_import_bytes: u64,
        pub max_diff_bytes: u64,
        pub max_changes_per_request: u64,
    }
}
