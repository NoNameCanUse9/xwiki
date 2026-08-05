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
        write!(f, "{} ({}): {}", self.code, self.status, self.message)
    }
}

impl std::error::Error for ApiError {}

#[derive(Deserialize)]
struct Envelope<T> {
    #[serde(flatten)]
    data: T,
}

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
pub mod dto {
    use serde::Deserialize;

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
