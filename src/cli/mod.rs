//! xwiki CLI (spec: `cli` 层).
//!
//! Exit codes: 0 ok · 2 usage · 3 auth/permission · 4 not found · 5 revision
//! or lock conflict · 6 network/server error. Data on stdout, progress and
//! errors on stderr. `--json` switches list outputs to stable JSON.

use crate::api::{ApiError, Client, dto};
use crate::config;
use base64::Engine;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn read_password(prompt: &str) -> String {
    print!("{prompt}");
    std::io::Write::flush(&mut std::io::stdout()).ok();
    #[cfg(unix)]
    let _ = std::process::Command::new("stty").arg("-echo").status();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("stty").arg("echo").status();
        println!();
    }
    s.trim().to_string()
}

#[cfg(test)]
mod tests;

fn server_from_args(args: &[String]) -> String {
    for (i, a) in args.iter().enumerate() {
        if a == "--server" && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }
    config::load_server()
}

/// Builds a client from --server / saved config: a bearer token from
/// XWIKI_TOKEN wins, otherwise the session cookie saved by `login` (same
/// mechanism as the GUI) is restored, so CLI sessions survive across
/// processes and admin-only operations work without exporting a token.
fn client(args: &[String]) -> Client {
    let server = server_from_args(args);
    if let Ok(token) = std::env::var("XWIKI_TOKEN")
        && !token.trim().is_empty()
    {
        return Client::with_token(&server, Some(token));
    }
    if let Some(session) = config::load_session() {
        return Client::with_session(&server, Some(session.cookie));
    }
    Client::new(&server)
}

/// --project values (repeatable). An explicit project scope prevents tokens
/// from accidentally receiving access to every project.
fn token_projects(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--project"
            && let Some(p) = args.get(i + 1)
        {
            out.push(p.clone());
            i += 2;
            continue;
        }
        if let Some(p) = args[i].strip_prefix("--project=") {
            out.push(p.to_string());
        }
        i += 1;
    }
    out
}

fn wants_json(args: &[String]) -> bool {
    args.iter().any(|a| a == "--json")
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    for (index, arg) in args.iter().enumerate() {
        if arg == name {
            return args.get(index + 1).cloned();
        }
        if let Some(value) = arg.strip_prefix(&format!("{name}=")) {
            return Some(value.to_string());
        }
    }
    None
}

/// Positional arguments with all `--option value` pairs removed. The CLI is
/// intentionally dependency-free, but keeping this small parser shared makes
/// options safe to place before or after positional arguments.
fn positional_args(args: &[String]) -> Vec<String> {
    const VALUE_OPTIONS: &[&str] = &[
        "--server",
        "--project",
        "--message",
        "--file",
        "--base-revision",
        "--idempotency-key",
        "--limit",
        "--offset",
        "--query",
        "--q",
        "--status",
        "--format",
        "--output",
        "--at",
        "--description",
        "--name",
        "--scope",
        "--username",
        "--password",
        "--current-password",
        "--new-password",
        "--display-name",
        "--editor",
        "--token",
    ];
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--json" || arg == "--dry-run" || arg == "--admin" {
            i += 1;
        } else if VALUE_OPTIONS.contains(&arg.as_str()) {
            i += 2;
        } else if arg.starts_with("--") {
            i += 1;
        } else {
            out.push(arg.clone());
            i += 1;
        }
    }
    out
}

fn parse_u32_option(args: &[String], name: &str) -> Result<Option<u32>, String> {
    option_value(args, name)
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| format!("{name} must be a non-negative integer"))
        })
        .transpose()
}

fn print_json<T: Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("CLI response is serializable")
    );
}

fn write_binary_output(
    bytes: &[u8],
    output: Option<&str>,
    default_name: &str,
) -> Result<Option<PathBuf>, String> {
    match output {
        Some("-") => {
            use std::io::Write;
            std::io::stdout()
                .write_all(bytes)
                .and_then(|_| std::io::stdout().flush())
                .map_err(|e| format!("cannot write stdout: {e}"))?;
            Ok(None)
        }
        Some(path) => {
            std::fs::write(path, bytes).map_err(|e| format!("cannot write {path}: {e}"))?;
            Ok(Some(PathBuf::from(path)))
        }
        None => {
            let path = PathBuf::from(default_name);
            std::fs::write(&path, bytes)
                .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            Ok(Some(path))
        }
    }
}

fn normalize_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn collect_upload_files(root: &Path) -> Result<Vec<dto::UploadFile>, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }

    fn visit(root: &Path, current: &Path, out: &mut Vec<dto::UploadFile>) -> Result<(), String> {
        let entries = std::fs::read_dir(current)
            .map_err(|e| format!("cannot read {}: {e}", current.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("cannot read directory entry: {e}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("cannot inspect {}: {e}", path.display()))?;
            if file_type.is_dir() {
                visit(root, &path, out)?;
            } else if file_type.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| format!("cannot make {} relative", path.display()))?;
                out.push(dto::UploadFile {
                    path: normalize_relative_path(relative),
                    content: std::fs::read(&path)
                        .map_err(|e| format!("cannot read {}: {e}", path.display()))?,
                });
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn collect_import_files(root: &Path) -> Result<Vec<dto::ImportFile>, String> {
    collect_upload_files(root).map(|files| {
        files
            .into_iter()
            .map(|file| dto::ImportFile {
                path: file.path,
                content: base64::engine::general_purpose::STANDARD.encode(file.content),
            })
            .collect()
    })
}

fn temporary_editor_path(path: &str) -> PathBuf {
    let stamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    std::env::temp_dir().join(format!(
        "xwiki-edit-{}-{stamp}.{}",
        std::process::id(),
        path.replace('/', "-")
    ))
}

fn run_editor(path: &Path, configured: Option<String>) -> Result<(), String> {
    let editor = configured
        .or_else(|| std::env::var("VISUAL").ok())
        .or_else(|| std::env::var("EDITOR").ok())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "notepad".into()
            } else {
                "vi".into()
            }
        });
    let mut parts = editor.split_whitespace();
    let Some(program) = parts.next() else {
        return Err("editor command is empty".into());
    };
    let status = std::process::Command::new(program)
        .args(parts)
        .arg(path)
        .status()
        .map_err(|e| format!("cannot start editor {editor}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("editor exited with {status}"))
    }
}

fn history_json(page: &crate::api::dto::CommitListResponse) -> String {
    serde_json::to_string_pretty(page).expect("history page is serializable")
}

fn exit_code(err: &ApiError) -> i32 {
    match err.code.as_str() {
        "authentication_required"
        | "invalid_credentials"
        | "invalid_token"
        | "account_disabled"
        | "agent_forbidden"
        | "admin_required"
        | "session_required" => 3,
        "not_found" | "project_not_found" | "doc_not_found" | "commit_not_found"
        | "revision_not_found" | "token_not_found" | "user_not_found" => 4,
        "revision_conflict"
        | "page_locked"
        | "lock_lost"
        | "not_lock_holder"
        | "idempotency_conflict"
        | "revert_conflict"
        | "project_archived"
        | "project_not_deleted" => 5,
        "validation_failed"
        | "invalid_project_name"
        | "invalid_doc_path"
        | "invalid_format"
        | "invalid_query"
        | "invalid_changeset"
        | "invalid_import"
        | "invalid_upload"
        | "invalid_lock_path"
        | "invalid_token_input"
        | "invalid_username"
        | "invalid_password"
        | "invalid_reset_token" => 2,
        "network_error" => 6,
        _ => match err.status {
            400 => 2,
            401 | 403 => 3,
            404 => 4,
            409 | 410 => 5,
            _ => 6,
        },
    }
}

fn fail(err: &ApiError) -> i32 {
    eprintln!("{err}");
    exit_code(err)
}

fn usage() -> i32 {
    eprintln!(
        "usage: xwiki <command> [args]

  server status|info          service meta & capabilities
  config show|set-server <url>
  login|logout [--username U] [--password P]
  password change|forgot|reset <args>
  whoami
  project list|show|create|archive|restore|restore-deleted|rename|delete
          import-folder|import-repo|import-bundle|export|purge|purge-deleted <args>
  doc tree|home|get|create|update|edit|delete|move|import <args>
  search <project> <query>
  backlinks <project> <path>       share <project> <path>
  attachment list|upload|download|delete <args>
  history list|show|diff|file|revert|restore <args>
  lock status|acquire|heartbeat|release|force-release <project> <path>
  token list|create|revoke
  user list|create|enable|disable|delete
  audit list <project>           openapi export [--output FILE]
  gui                             launch the desktop app

Global: --server <url>  --json   Env: XWIKI_TOKEN (bearer auth)"
    );
    2
}

pub fn run(args: Vec<String>) -> i32 {
    if args.is_empty() {
        return usage();
    }

    // Accept global flags before the command as well as after it. Keep them
    // at the end of the handler argument list so each command can still use
    // its first positional argument as the verb.
    let mut command_index = 0;
    while command_index < args.len() {
        match args[command_index].as_str() {
            "--json" => command_index += 1,
            "--server" if command_index + 1 < args.len() => command_index += 2,
            _ => break,
        }
    }
    if command_index >= args.len() {
        return usage();
    }
    let cmd = args[command_index].clone();
    let mut rest = args[command_index + 1..].to_vec();
    rest.extend(args[..command_index].iter().cloned());

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("runtime error: {e}");
            return 6;
        }
    };
    rt.block_on(async move {
        match cmd.as_str() {
            "server" => cmd_server(&rest).await,
            "config" => cmd_config(&rest),
            "login" => cmd_login(&rest).await,
            "logout" => cmd_logout(&rest).await,
            "password" => cmd_password(&rest).await,
            "whoami" => cmd_whoami(&rest).await,
            "project" => cmd_project(&rest).await,
            "doc" => cmd_doc(&rest).await,
            "search" => cmd_search(&rest).await,
            "backlinks" => cmd_backlinks(&rest).await,
            "share" => cmd_share(&rest).await,
            "attachment" => cmd_attachment(&rest).await,
            "history" => cmd_history(&rest).await,
            "lock" => cmd_lock(&rest).await,
            "token" => cmd_token(&rest).await,
            "user" => cmd_user(&rest).await,
            "audit" => cmd_audit(&rest).await,
            "openapi" => cmd_openapi(&rest).await,
            _ => usage(),
        }
    })
}

// ----- server -----

async fn cmd_server(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let verb = positional.first().map(String::as_str).unwrap_or("status");
    match verb {
        "status" | "info" => {
            let c = client(args);
            match c.meta().await {
                Ok(meta) => {
                    if wants_json(args) {
                        print_json(&meta);
                    } else {
                        println!("version {} · api v{}", meta.version, meta.api_version);
                        println!(
                            "limits: doc ≤ {} MiB · import ≤ {} MiB · diff ≤ {} MiB",
                            meta.limits.max_doc_bytes / (1 << 20),
                            meta.limits.max_import_bytes / (1 << 20),
                            meta.limits.max_diff_bytes / (1 << 20)
                        );
                        println!("capabilities: {}", meta.capabilities.join(", "));
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => usage(),
    }
}

// ----- config -----

fn cmd_config(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let verb = positional.first().map(String::as_str).unwrap_or("show");
    match verb {
        "show" => {
            if wants_json(args) {
                print_json(&serde_json::json!({"server": config::load_server()}));
            } else {
                println!("server: {}", config::load_server());
            }
            0
        }
        "set-server" => {
            let Some(url) = positional.get(1) else {
                return usage();
            };
            match config::save_server(url) {
                Ok(()) => {
                    if wants_json(args) {
                        print_json(&serde_json::json!({"server": url}));
                    } else {
                        println!("server: {url}");
                    }
                    0
                }
                Err(e) => {
                    eprintln!("failed to save server: {e}");
                    6
                }
            }
        }
        _ => usage(),
    }
}

// ----- auth -----

async fn cmd_login(args: &[String]) -> i32 {
    let c = client(args);
    match c.meta().await {
        Ok(meta) if meta.api_version == "1" => {}
        Ok(meta) => {
            eprintln!(
                "服务器 API 版本 {} 不受支持，请升级客户端",
                meta.api_version
            );
            return 2;
        }
        Err(e) => {
            eprintln!("无法检查服务器版本: {e}");
            return fail(&e);
        }
    }
    let username = option_value(args, "--username").unwrap_or_else(|| {
        print!("username: ");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let mut s = String::new();
        std::io::stdin().read_line(&mut s).ok();
        s.trim().to_string()
    });
    let password = option_value(args, "--password").unwrap_or_else(|| read_password("password: "));
    match c.login_with_session(&username, &password).await {
        Ok((u, cookie)) => {
            if !wants_json(args) {
                println!("logged in as {} (admin: {})", u.username, u.is_admin);
            }
            // Persist the session cookie like the GUI does, so subsequent
            // CLI calls keep admin powers across processes. Also mint a
            // bearer token for env-based automation (XWIKI_TOKEN).
            if let Some(cookie) = cookie {
                config::save_session(&server_from_args(args), &u.username, &cookie);
            }
            let ids: Vec<String> = c
                .projects()
                .await
                .map(|ps| ps.into_iter().map(|p| p.id).collect())
                .unwrap_or_default();
            if ids.is_empty() {
                eprintln!("note: no projects yet — create one first, then re-run login");
            }
            let scoped = token_projects(args);
            let ids = if scoped.is_empty() { ids } else { scoped };
            if ids.is_empty() {
                eprintln!("token creation requires at least one --project");
                return 2;
            }
            match c.create_token("cli-login", "write", ids).await {
                Ok((token, secret)) => {
                    if wants_json(args) {
                        print_json(
                            &serde_json::json!({"user": u, "token": token, "secret": secret}),
                        );
                    } else {
                        println!("export XWIKI_TOKEN={secret}");
                    }
                    0
                }
                Err(e) => {
                    eprintln!("token mint failed: {e}");
                    6
                }
            }
        }
        Err(e) => fail(&e),
    }
}

async fn cmd_logout(args: &[String]) -> i32 {
    let c = client(args);
    match c.logout().await {
        Ok(()) => {
            config::clear_session();
            if wants_json(args) {
                print_json(&serde_json::json!({"ok": true}));
            } else {
                println!("logged out");
            }
            0
        }
        Err(e) => fail(&e),
    }
}

async fn cmd_password(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let verb = positional.first().map(String::as_str).unwrap_or("change");
    let json = wants_json(args);
    let c = client(args);
    match verb {
        "change" => {
            let current = option_value(args, "--current-password").or_else(|| {
                if positional.get(1).is_none() {
                    Some(read_password("current password: "))
                } else {
                    positional.get(1).cloned()
                }
            });
            let new_password = option_value(args, "--new-password").or_else(|| {
                positional
                    .get(2)
                    .cloned()
                    .or_else(|| Some(read_password("new password: ")))
            });
            let (Some(current), Some(new_password)) = (current, new_password) else {
                return usage();
            };
            match c.change_password(&current, &new_password).await {
                Ok(()) => {
                    if json {
                        print_json(&serde_json::json!({"ok": true}));
                    } else {
                        println!("password changed");
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "forgot" => {
            let Some(username) =
                option_value(args, "--username").or_else(|| positional.get(1).cloned())
            else {
                return usage();
            };
            match c.forgot_password(&username).await {
                Ok(ok) => {
                    if json {
                        print_json(&serde_json::json!({"ok": ok}));
                    } else {
                        println!("reset request accepted");
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "reset" => {
            let Some(token) = option_value(args, "--token").or_else(|| positional.get(1).cloned())
            else {
                return usage();
            };
            let Some(new_password) =
                option_value(args, "--new-password").or_else(|| positional.get(2).cloned())
            else {
                return usage();
            };
            match c.reset_password(&token, &new_password).await {
                Ok(ok) => {
                    if json {
                        print_json(&serde_json::json!({"ok": ok}));
                    } else {
                        println!("password reset");
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => usage(),
    }
}

async fn cmd_whoami(args: &[String]) -> i32 {
    let c = client(args);
    match c.me().await {
        Ok(user) => {
            if wants_json(args) {
                print_json(&user);
            } else {
                println!(
                    "{} ({}) admin={} disabled={}",
                    user.username, user.id, user.is_admin, user.disabled
                );
            }
            0
        }
        Err(e) => fail(&e),
    }
}

// ----- project -----

async fn cmd_project(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let verb = positional.first().map(String::as_str).unwrap_or("list");
    let json = wants_json(args);
    let c = client(args);
    match verb {
        "list" => {
            let requested_status = option_value(args, "--status");
            let status = if has_flag(args, "--deleted") {
                Some("deleted")
            } else {
                requested_status.as_deref()
            };
            if status.is_some_and(|value| value != "deleted") {
                return usage();
            }
            match c.projects_status(status).await {
                Ok(projects) => {
                    if json {
                        print_json(&projects);
                    } else {
                        for project in projects {
                            let state = if project.deleted {
                                "deleted"
                            } else if project.archived {
                                "archived"
                            } else {
                                "active"
                            };
                            println!("{:<28} {:<24} {}", project.id, project.name, state);
                        }
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "create" => {
            let Some(name) = positional.get(1) else {
                return usage();
            };
            let description = option_value(args, "--description")
                .or_else(|| positional.get(2).cloned())
                .unwrap_or_default();
            match c.create_project(name, &description).await {
                Ok(project) => {
                    if json {
                        print_json(&project);
                    } else {
                        println!("created {} ({})", project.name, project.id);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "show" => {
            let Some(id) = positional.get(1) else {
                return usage();
            };
            match c.project(id).await {
                Ok(project) => {
                    if json {
                        print_json(&project);
                    } else {
                        println!(
                            "{} · {} · archived={} deleted={}",
                            project.id, project.name, project.archived, project.deleted
                        );
                        if !project.description.is_empty() {
                            println!("{}", project.description);
                        }
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "archive" | "restore" => {
            let Some(id) = positional.get(1) else {
                return usage();
            };
            match c.set_archived(id, verb == "archive").await {
                Ok(project) => {
                    if json {
                        print_json(&project);
                    } else {
                        println!("{} archived={}", project.name, project.archived);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "restore-deleted" | "restore-trash" => {
            let Some(id) = positional.get(1) else {
                return usage();
            };
            match c.restore_deleted_project(id).await {
                Ok(project) => {
                    if json {
                        print_json(&project);
                    } else {
                        println!("restored {} ({})", project.name, project.id);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "rename" => {
            let Some(id) = positional.get(1) else {
                return usage();
            };
            let Some(name) = option_value(args, "--name").or_else(|| positional.get(2).cloned())
            else {
                return usage();
            };
            match c.rename_project(id, &name).await {
                Ok(project) => {
                    if json {
                        print_json(&project);
                    } else {
                        println!("renamed to {} ({})", project.name, project.id);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "delete" => {
            let Some(id) = positional.get(1) else {
                return usage();
            };
            // Destructive: require interactive username + password
            // confirmation. The credentials are verified against the server
            // (/auth/login) before the project is removed.
            let username = option_value(args, "--username").unwrap_or_else(|| {
                print!("confirm username: ");
                std::io::Write::flush(&mut std::io::stdout()).ok();
                let mut value = String::new();
                std::io::stdin().read_line(&mut value).ok();
                value.trim().to_string()
            });
            let password = option_value(args, "--password")
                .unwrap_or_else(|| read_password("confirm password: "));
            if username.is_empty() || password.is_empty() {
                eprintln!("aborted: empty credentials");
                return 2;
            }
            let session_client = Client::new(&server_from_args(args));
            match session_client.login(&username, &password).await {
                Ok(user) if user.username == username => {}
                Ok(_) => {
                    eprintln!("aborted: username does not match current account");
                    return 2;
                }
                Err(e) => {
                    eprintln!("aborted: password verification failed: {e}");
                    return 2;
                }
            }
            match session_client.delete_project(id).await {
                Ok(()) => {
                    if json {
                        print_json(&serde_json::json!({"deleted": true, "id": id}));
                    } else {
                        println!("deleted {id}");
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "purge-deleted" => {
            let Some(id) = positional.get(1) else {
                return usage();
            };
            match c.purge_deleted_project(id).await {
                Ok(()) => {
                    if json {
                        print_json(&serde_json::json!({"purged": true, "id": id}));
                    } else {
                        println!("purged {id}");
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "purge" => {
            let Some(id) = positional.get(1) else {
                return usage();
            };
            let paths: Vec<String> = positional.iter().skip(2).cloned().collect();
            if paths.is_empty() {
                eprintln!("project purge requires at least one path");
                return 2;
            }
            let message = option_value(args, "--message")
                .unwrap_or_else(|| format!("purge {}", paths.join(", ")));
            match c.purge_paths(id, &paths, &message).await {
                Ok(()) => {
                    if json {
                        print_json(&serde_json::json!({"purged": true, "paths": paths}));
                    } else {
                        println!("purged {} path(s)", paths.len());
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "import-folder" => {
            let (Some(name), Some(directory)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            let files = match collect_upload_files(Path::new(directory)) {
                Ok(files) if !files.is_empty() => files,
                Ok(_) => {
                    eprintln!("import folder contains no files");
                    return 2;
                }
                Err(error) => {
                    eprintln!("{error}");
                    return 2;
                }
            };
            let description = option_value(args, "--description").unwrap_or_default();
            match c.import_folder(name, &description, Arc::new(files)).await {
                Ok(result) => {
                    if json {
                        print_json(&result);
                    } else {
                        println!(
                            "imported {} ({}, {} commits)",
                            result.project.name, result.project.id, result.commits
                        );
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "import-repo" => {
            let (Some(name), Some(url)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            match c.import_repo(name, url).await {
                Ok(result) => {
                    if json {
                        print_json(&result);
                    } else {
                        println!(
                            "imported {} ({}, {} commits)",
                            result.project.name, result.project.id, result.commits
                        );
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "import-bundle" => {
            let (Some(name), Some(file)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            let bytes = match std::fs::read(file) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("cannot read {file}: {e}");
                    return 2;
                }
            };
            match c.import_bundle(name, Arc::new(bytes)).await {
                Ok(result) => {
                    if json {
                        print_json(&result);
                    } else {
                        println!(
                            "imported {} ({}, {} commits)",
                            result.project.name, result.project.id, result.commits
                        );
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "export" => {
            let Some(project) = positional.get(1) else {
                return usage();
            };
            let format = option_value(args, "--format")
                .or_else(|| positional.get(2).cloned())
                .unwrap_or_else(|| "zip".into());
            let (bytes, extension) = match format.as_str() {
                "zip" => match c.export_zip(project).await {
                    Ok(bytes) => (bytes, "zip"),
                    Err(e) => return fail(&e),
                },
                "bundle" => match c.export_bundle(project).await {
                    Ok(bytes) => (bytes, "bundle"),
                    Err(e) => return fail(&e),
                },
                _ => return usage(),
            };
            let default_name = format!("{project}.{extension}");
            match write_binary_output(
                &bytes,
                option_value(args, "--output").as_deref(),
                &default_name,
            ) {
                Ok(Some(path)) => {
                    if json {
                        print_json(
                            &serde_json::json!({"path": path, "format": format, "bytes": bytes.len()}),
                        );
                    } else {
                        println!("exported {} ({} bytes)", path.display(), bytes.len());
                    }
                    0
                }
                Ok(None) => 0,
                Err(error) => {
                    eprintln!("{error}");
                    2
                }
            }
        }
        _ => usage(),
    }
}

// ----- doc -----

async fn cmd_doc(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let verb = positional.first().map(String::as_str).unwrap_or("tree");
    let json = wants_json(args);
    let c = client(args);
    match verb {
        "tree" => {
            let Some(project) = positional.get(1) else {
                return usage();
            };
            let path = positional.get(2).map(String::as_str).unwrap_or("");
            match c.tree(project, path).await {
                Ok(entries) => {
                    if json {
                        print_json(&entries);
                    } else {
                        for entry in entries {
                            let icon = if entry.r#type == "tree" { "d" } else { "f" };
                            println!("{icon} {}", entry.path);
                        }
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "home" => {
            let Some(project) = positional.get(1) else {
                return usage();
            };
            match c.home(project).await {
                Ok(page) => {
                    if json {
                        print_json(&page);
                    } else {
                        print!("{}", page.content);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "get" => {
            let (Some(project), Some(path)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            let format = option_value(args, "--format").unwrap_or_else(|| "raw".into());
            if !["raw", "html", "base64"].contains(&format.as_str()) {
                return usage();
            }
            let page = match option_value(args, "--at") {
                Some(revision) => {
                    c.page_with_format_at(project, path, &format, Some(&revision))
                        .await
                }
                None => c.page_with_format(project, path, &format).await,
            };
            match page {
                Ok(page) => {
                    if json {
                        print_json(&page);
                    } else {
                        print!("{}", page.content);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "create" | "update" => {
            let (Some(project), Some(path)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            let content = match read_stdin_or_file(args) {
                Ok(content) => content,
                Err(error) => {
                    eprintln!("error: {error}");
                    return 2;
                }
            };
            let base = match option_value(args, "--base-revision") {
                Some(revision) => revision,
                None => match c.revision(project).await {
                    Ok(revision) => revision,
                    Err(e) => return fail(&e),
                },
            };
            let message =
                option_value(args, "--message").unwrap_or_else(|| format!("{verb} {path}"));
            let idempotency_key = option_value(args, "--idempotency-key");
            let result = c
                .apply_changeset_with_options(
                    project,
                    &base,
                    &message,
                    vec![dto::Change {
                        op: verb.into(),
                        path: path.into(),
                        new_path: None,
                        content: Some(content),
                    }],
                    has_flag(args, "--dry-run"),
                    idempotency_key.as_deref(),
                )
                .await;
            match result {
                Ok(result) => {
                    if json {
                        print_json(&result);
                    } else if has_flag(args, "--dry-run") {
                        println!("dry-run: {}", result.preview.unwrap_or_default());
                    } else {
                        println!("committed {} ({})", result.commit, result.revision);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "edit" => {
            let (Some(project), Some(path)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            let page = match c.page(project, path).await {
                Ok(page) => page,
                Err(e) => return fail(&e),
            };
            if let Err(e) = c.acquire_lock(project, path).await {
                return fail(&e);
            }
            let temporary = temporary_editor_path(path);
            let edit_result = (|| -> Result<String, String> {
                std::fs::write(&temporary, &page.content)
                    .map_err(|e| format!("cannot create draft: {e}"))?;
                run_editor(&temporary, option_value(args, "--editor"))?;
                std::fs::read_to_string(&temporary)
                    .map_err(|e| format!("cannot read edited draft: {e}"))
            })();
            let _ = std::fs::remove_file(&temporary);
            let mut code = 0;
            match edit_result {
                Ok(content) if content == page.content => {
                    println!("no changes");
                }
                Ok(content) => {
                    let base = match option_value(args, "--base-revision") {
                        Some(revision) => revision,
                        None => match c.revision(project).await {
                            Ok(revision) => revision,
                            Err(e) => {
                                let _ = c.release_lock(project, path).await;
                                return fail(&e);
                            }
                        },
                    };
                    let message =
                        option_value(args, "--message").unwrap_or_else(|| format!("update {path}"));
                    let key = option_value(args, "--idempotency-key");
                    match c
                        .apply_changeset_with_options(
                            project,
                            &base,
                            &message,
                            vec![dto::Change {
                                op: "update".into(),
                                path: path.into(),
                                new_path: None,
                                content: Some(content),
                            }],
                            has_flag(args, "--dry-run"),
                            key.as_deref(),
                        )
                        .await
                    {
                        Ok(result) => {
                            if json {
                                print_json(&result);
                            } else {
                                println!("committed {} ({})", result.commit, result.revision);
                            }
                        }
                        Err(e) => code = fail(&e),
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    code = 6;
                }
            }
            if let Err(error) = c.release_lock(project, path).await {
                eprintln!("warning: could not release lock: {error}");
                if code == 0 {
                    code = fail(&error);
                }
            }
            code
        }
        "delete" => {
            let (Some(project), Some(path)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            let base = match option_value(args, "--base-revision") {
                Some(revision) => revision,
                None => match c.revision(project).await {
                    Ok(revision) => revision,
                    Err(e) => return fail(&e),
                },
            };
            let message =
                option_value(args, "--message").unwrap_or_else(|| format!("delete {path}"));
            let key = option_value(args, "--idempotency-key");
            match c
                .apply_changeset_with_options(
                    project,
                    &base,
                    &message,
                    vec![dto::Change {
                        op: "delete".into(),
                        path: path.into(),
                        new_path: None,
                        content: None,
                    }],
                    has_flag(args, "--dry-run"),
                    key.as_deref(),
                )
                .await
            {
                Ok(result) => {
                    if json {
                        print_json(&result);
                    } else {
                        println!("deleted {} ({})", path, result.commit);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "move" => {
            let (Some(project), Some(from), Some(to)) =
                (positional.get(1), positional.get(2), positional.get(3))
            else {
                return usage();
            };
            let base = match option_value(args, "--base-revision") {
                Some(revision) => revision,
                None => match c.revision(project).await {
                    Ok(revision) => revision,
                    Err(e) => return fail(&e),
                },
            };
            let message =
                option_value(args, "--message").unwrap_or_else(|| format!("move {from} → {to}"));
            let key = option_value(args, "--idempotency-key");
            match c
                .apply_changeset_with_options(
                    project,
                    &base,
                    &message,
                    vec![dto::Change {
                        op: "move".into(),
                        path: from.into(),
                        new_path: Some(to.into()),
                        content: None,
                    }],
                    has_flag(args, "--dry-run"),
                    key.as_deref(),
                )
                .await
            {
                Ok(result) => {
                    if json {
                        print_json(&result);
                    } else {
                        println!("moved {} → {} ({})", from, to, result.commit);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "import" => {
            let Some(project) = positional.get(1) else {
                return usage();
            };
            let source_or_target = positional.get(2).cloned();
            let file_option = option_value(args, "--file");
            let files = match (source_or_target.as_deref(), file_option.as_deref()) {
                (Some(source), None) if Path::new(source).is_dir() => {
                    match collect_import_files(Path::new(source)) {
                        Ok(files) if !files.is_empty() => files,
                        Ok(_) => {
                            eprintln!("import directory contains no files");
                            return 2;
                        }
                        Err(error) => {
                            eprintln!("{error}");
                            return 2;
                        }
                    }
                }
                (Some(source), None) if Path::new(source).is_file() => {
                    let content = match std::fs::read(source) {
                        Ok(content) => content,
                        Err(error) => {
                            eprintln!("cannot read {source}: {error}");
                            return 2;
                        }
                    };
                    let path = Path::new(source)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("imported-file")
                        .to_string();
                    vec![dto::ImportFile {
                        path,
                        content: base64::engine::general_purpose::STANDARD.encode(content),
                    }]
                }
                (Some(target), Some(local_file)) => {
                    let content = match std::fs::read(local_file) {
                        Ok(content) => content,
                        Err(error) => {
                            eprintln!("cannot read {local_file}: {error}");
                            return 2;
                        }
                    };
                    vec![dto::ImportFile {
                        path: target.to_string(),
                        content: base64::engine::general_purpose::STANDARD.encode(content),
                    }]
                }
                (Some(target), None) => {
                    let content = match read_stdin_bytes() {
                        Ok(content) => content,
                        Err(error) => {
                            eprintln!("cannot read stdin: {error}");
                            return 2;
                        }
                    };
                    vec![dto::ImportFile {
                        path: target.to_string(),
                        content: base64::engine::general_purpose::STANDARD.encode(content),
                    }]
                }
                (None, Some(local_file)) => {
                    let content = match std::fs::read(local_file) {
                        Ok(content) => content,
                        Err(error) => {
                            eprintln!("cannot read {local_file}: {error}");
                            return 2;
                        }
                    };
                    let path = Path::new(local_file)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("imported-file")
                        .to_string();
                    vec![dto::ImportFile {
                        path,
                        content: base64::engine::general_purpose::STANDARD.encode(content),
                    }]
                }
                (None, None) => return usage(),
            };
            let base = match option_value(args, "--base-revision") {
                Some(revision) => revision,
                None => match c.revision(project).await {
                    Ok(revision) => revision,
                    Err(e) => return fail(&e),
                },
            };
            let message = option_value(args, "--message")
                .unwrap_or_else(|| format!("import {} file(s)", files.len()));
            if has_flag(args, "--dry-run") {
                if json {
                    print_json(&serde_json::json!({
                        "dry_run": true,
                        "base_revision": base,
                        "message": message,
                        "files": files,
                    }));
                } else {
                    println!("dry-run: {} file(s), base {}", files.len(), base);
                }
                return 0;
            }
            match c
                .import_files_with_options(
                    project,
                    &base,
                    &message,
                    files,
                    option_value(args, "--idempotency-key").as_deref(),
                )
                .await
            {
                Ok(result) => {
                    if json {
                        print_json(&result);
                    } else {
                        println!("imported {} file(s) ({})", result.imported, result.commit);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => usage(),
    }
}

/// Reads doc content from `--file <path>`, or stdin when piped. A `--file`
/// that cannot be read is an error — silently falling back to empty content
/// would let `doc update` wipe the page.
fn read_stdin_or_file(args: &[String]) -> Result<String, String> {
    if let Some(i) = args.iter().position(|a| a == "--file")
        && let Some(path) = args.get(i + 1)
    {
        return std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read --file {path}: {e}"));
    }
    let mut buf = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
        .map_err(|e| format!("cannot read stdin: {e}"))?;
    Ok(buf)
}

fn read_stdin_bytes() -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::stdin(), &mut bytes)
        .map_err(|e| format!("cannot read stdin: {e}"))?;
    Ok(bytes)
}

// ----- search -----

async fn cmd_search(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let (Some(project), Some(query)) = (positional.first(), positional.get(1)) else {
        return usage();
    };
    let limit = match parse_u32_option(args, "--limit") {
        Ok(limit) => limit,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    if limit.is_some_and(|value| value == 0 || value > 50) {
        eprintln!("--limit must be between 1 and 50");
        return 2;
    }
    let c = client(args);
    match c.search_detail_with_limit(project, query, limit).await {
        Ok(response) => {
            if wants_json(args) {
                print_json(&response);
            } else {
                for result in response.results {
                    println!("{}", result.path);
                    println!("    {}", result.snippet);
                }
            }
            0
        }
        Err(e) => fail(&e),
    }
}

async fn cmd_backlinks(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let (Some(project), Some(path)) = (positional.first(), positional.get(1)) else {
        return usage();
    };
    let c = client(args);
    match c.backlinks_detail(project, path).await {
        Ok(response) => {
            if wants_json(args) {
                print_json(&response);
            } else {
                for item in response.backlinks {
                    println!("{}\n    {}", item.source, item.snippet);
                }
            }
            0
        }
        Err(e) => fail(&e),
    }
}

async fn cmd_share(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let (Some(project), Some(path)) = (positional.first(), positional.get(1)) else {
        return usage();
    };
    let c = client(args);
    match c.create_share(project, path).await {
        Ok(share) => {
            if wants_json(args) {
                print_json(&share);
            } else {
                println!("{}", share.url);
            }
            0
        }
        Err(e) => fail(&e),
    }
}

async fn cmd_attachment(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let verb = positional.first().map(String::as_str).unwrap_or("list");
    let json = wants_json(args);
    let c = client(args);
    match verb {
        "list" => {
            let Some(project) = positional.get(1) else {
                return usage();
            };
            let path = positional
                .get(2)
                .map(String::as_str)
                .unwrap_or("attachments");
            match c.tree(project, path).await {
                Ok(entries) => {
                    let entries: Vec<_> = entries
                        .into_iter()
                        .filter(|entry| entry.r#type == "blob")
                        .collect();
                    if json {
                        print_json(&entries);
                    } else {
                        for entry in entries {
                            println!("{}", entry.path);
                        }
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "upload" => {
            let (Some(project), Some(local)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            let local_path = Path::new(local);
            let default_name = local_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("attachment");
            let remote = positional
                .get(3)
                .cloned()
                .unwrap_or_else(|| format!("attachments/{default_name}"));
            let remote = if remote.starts_with("attachments/") {
                remote
            } else {
                format!("attachments/{remote}")
            };
            let content = match std::fs::read(local_path) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("cannot read {local}: {e}");
                    return 2;
                }
            };
            let base = match option_value(args, "--base-revision") {
                Some(revision) => revision,
                None => match c.revision(project).await {
                    Ok(revision) => revision,
                    Err(e) => return fail(&e),
                },
            };
            let message =
                option_value(args, "--message").unwrap_or_else(|| format!("upload {remote}"));
            let encoded = base64::engine::general_purpose::STANDARD.encode(content);
            match c
                .upload_attachment_with_options(
                    project,
                    &base,
                    &remote,
                    &encoded,
                    &message,
                    option_value(args, "--idempotency-key").as_deref(),
                )
                .await
            {
                Ok(result) => {
                    if json {
                        print_json(&result);
                    } else {
                        println!("uploaded {} ({})", remote, result.commit);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "download" => {
            // `<project> <remote> [local]`: the optional third positional is
            // the local target, mirroring `upload <project> <local> [remote]`;
            // `--output` overrides it. `-` writes to stdout.
            let (Some(project), Some(remote)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            let local = positional.get(3).map(String::as_str);
            match c.download_attachment(project, remote).await {
                Ok(bytes) => {
                    let default_name = Path::new(remote)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("attachment");
                    match write_binary_output(
                        &bytes,
                        option_value(args, "--output").as_deref().or(local),
                        default_name,
                    ) {
                        Ok(Some(path)) => {
                            if json {
                                print_json(
                                    &serde_json::json!({"path": path, "bytes": bytes.len()}),
                                );
                            } else {
                                println!("downloaded {} ({} bytes)", path.display(), bytes.len());
                            }
                            0
                        }
                        Ok(None) => 0,
                        Err(error) => {
                            eprintln!("{error}");
                            2
                        }
                    }
                }
                Err(e) => fail(&e),
            }
        }
        "delete" => {
            let (Some(project), Some(path)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            let base = match option_value(args, "--base-revision") {
                Some(revision) => revision,
                None => match c.revision(project).await {
                    Ok(revision) => revision,
                    Err(e) => return fail(&e),
                },
            };
            match c
                .delete_attachment_with_options(
                    project,
                    &base,
                    path,
                    option_value(args, "--idempotency-key").as_deref(),
                )
                .await
            {
                Ok(result) => {
                    if json {
                        print_json(&result);
                    } else {
                        println!("deleted {} ({})", path, result.commit);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => usage(),
    }
}

async fn cmd_openapi(args: &[String]) -> i32 {
    let positional = positional_args(args);
    if positional.first().map(String::as_str).unwrap_or("export") != "export" {
        return usage();
    }
    let c = client(args);
    match c.openapi().await {
        Ok(spec) => {
            let rendered = match serde_json::to_string_pretty(&spec) {
                Ok(rendered) => rendered,
                Err(error) => {
                    eprintln!("cannot serialize OpenAPI document: {error}");
                    return 6;
                }
            };
            match option_value(args, "--output") {
                None => {
                    println!("{rendered}");
                    0
                }
                Some(path) if path == "-" => {
                    println!("{rendered}");
                    0
                }
                Some(path) => match std::fs::write(&path, format!("{rendered}\n")) {
                    Ok(()) => {
                        println!("exported {path}");
                        0
                    }
                    Err(e) => {
                        eprintln!("cannot write {path}: {e}");
                        2
                    }
                },
            }
        }
        Err(e) => fail(&e),
    }
}

// ----- history -----

async fn cmd_history(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let verb = positional.first().map(String::as_str).unwrap_or("list");
    let json = wants_json(args);
    let c = client(args);
    match verb {
        "list" => {
            let Some(project) = positional.get(1) else {
                return usage();
            };
            let query = option_value(args, "--query")
                .or_else(|| option_value(args, "--q"))
                .unwrap_or_default();
            let limit = match parse_u32_option(args, "--limit") {
                Ok(Some(limit)) if (1..=100).contains(&limit) => limit,
                Ok(None) => 20,
                Ok(Some(_)) => {
                    eprintln!("--limit must be between 1 and 100");
                    return 2;
                }
                Err(error) => {
                    eprintln!("{error}");
                    return 2;
                }
            };
            let offset = match parse_u32_option(args, "--offset") {
                Ok(Some(offset)) => offset,
                Ok(None) => 0,
                Err(error) => {
                    eprintln!("{error}");
                    return 2;
                }
            };
            match c.commits_search_page(project, &query, limit, offset).await {
                Ok(page) => {
                    if json {
                        println!("{}", history_json(&page));
                    } else {
                        for commit in page.commits {
                            let short: String = commit.sha.chars().take(7).collect();
                            println!("{short}  {}  {}", commit.author, commit.message);
                        }
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "show" => {
            let (Some(project), Some(sha)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            match c.commit_detail(project, sha).await {
                Ok(detail) => {
                    if json {
                        print_json(&detail);
                    } else {
                        println!("{}  {}  {}", detail.sha, detail.author, detail.date);
                        println!("{}", detail.message);
                        for file in &detail.files {
                            println!("  {}  {}", file.status, file.path);
                        }
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "diff" => {
            let (Some(project), Some(sha)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            let format = option_value(args, "--format").unwrap_or_else(|| "numstat".into());
            match format.as_str() {
                "numstat" => match c.diff_stats_detail(project, sha).await {
                    Ok(detail) => {
                        if json {
                            print_json(&detail);
                        } else {
                            for stat in detail.stats {
                                println!("{:>5} {:>5}  {}", stat.added, stat.deleted, stat.path);
                            }
                        }
                        0
                    }
                    Err(e) => fail(&e),
                },
                "patch" => match c.commit_patch(project, sha).await {
                    Ok(patch) => {
                        if json {
                            print_json(&patch);
                        } else {
                            print!("{}", patch.patch);
                        }
                        0
                    }
                    Err(e) => fail(&e),
                },
                _ => usage(),
            }
        }
        "file" => {
            let (Some(project), Some(path)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            match c.file_history(project, path).await {
                Ok(commits) => {
                    if json {
                        print_json(&serde_json::json!({"path": path, "commits": commits}));
                    } else {
                        for commit in commits {
                            let short: String = commit.sha.chars().take(7).collect();
                            println!("{short}  {}  {}", commit.author, commit.message);
                        }
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "revert" => {
            let (Some(project), Some(sha)) = (positional.get(1), positional.get(2)) else {
                return usage();
            };
            let message = option_value(args, "--message").unwrap_or_default();
            match c.revert_commit(project, sha, &message).await {
                Ok(commit) => {
                    if json {
                        print_json(&commit);
                    } else {
                        println!("reverted {} ({})", sha, commit.sha);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "restore" => {
            let (Some(project), Some(path), Some(revision)) =
                (positional.get(1), positional.get(2), positional.get(3))
            else {
                return usage();
            };
            let page = match c.page_at(project, path, revision).await {
                Ok(page) => page,
                Err(e) => return fail(&e),
            };
            let base = match option_value(args, "--base-revision") {
                Some(base) => base,
                None => match c.revision(project).await {
                    Ok(base) => base,
                    Err(e) => return fail(&e),
                },
            };
            let message = option_value(args, "--message")
                .unwrap_or_else(|| format!("restore {path} from {revision}"));
            let key = option_value(args, "--idempotency-key");
            match c
                .apply_changeset_with_options(
                    project,
                    &base,
                    &message,
                    vec![dto::Change {
                        op: "update".into(),
                        path: path.into(),
                        new_path: None,
                        content: Some(page.content),
                    }],
                    has_flag(args, "--dry-run"),
                    key.as_deref(),
                )
                .await
            {
                Ok(result) => {
                    if json {
                        print_json(&result);
                    } else {
                        println!("restored {} ({})", path, result.commit);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => usage(),
    }
}

// ----- lock -----

async fn cmd_lock(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let verb = positional.first().map(String::as_str).unwrap_or("status");
    let (Some(project), Some(path)) = (positional.get(1), positional.get(2)) else {
        return usage();
    };
    let json = wants_json(args);
    let c = client(args);
    match verb {
        "status" => match c.lock_status(project, path).await {
            Ok(lock) => {
                if json {
                    print_json(&serde_json::json!({"lock": lock}));
                } else if let Some(lock) = lock {
                    println!(
                        "{} · held by {} · expires {}",
                        lock.path, lock.username, lock.expires_at
                    );
                } else {
                    println!("unlocked");
                }
                0
            }
            Err(e) => fail(&e),
        },
        "acquire" => match c.acquire_lock(project, path).await {
            Ok(lock) => {
                if json {
                    print_json(&lock);
                } else {
                    println!("locked {} until {}", lock.path, lock.expires_at);
                }
                0
            }
            Err(e) => fail(&e),
        },
        "heartbeat" | "renew" => match c.heartbeat_lock(project, path).await {
            Ok(lock) => {
                if json {
                    print_json(&lock);
                } else {
                    println!("renewed {} until {}", lock.path, lock.expires_at);
                }
                0
            }
            Err(e) => fail(&e),
        },
        "release" => match c.release_lock(project, path).await {
            Ok(released) => {
                if json {
                    print_json(&serde_json::json!({"released": released}));
                } else if released {
                    println!("released");
                } else {
                    println!("not locked");
                }
                0
            }
            Err(e) => fail(&e),
        },
        "force-release" | "force" => match c.force_release_lock(project, path).await {
            Ok(released) => {
                if json {
                    print_json(&serde_json::json!({"released": released}));
                } else {
                    println!(
                        "{}",
                        if released {
                            "force-released"
                        } else {
                            "not locked"
                        }
                    );
                }
                0
            }
            Err(e) => fail(&e),
        },
        _ => usage(),
    }
}

// ----- token -----

async fn cmd_token(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let verb = positional.first().map(String::as_str).unwrap_or("list");
    let json = wants_json(args);
    let c = client(args);
    match verb {
        "list" => match c.tokens().await {
            Ok(tokens) => {
                if json {
                    print_json(&tokens);
                } else {
                    for token in tokens {
                        println!("{:<24} {:<8} {}", token.id, token.scope, token.name);
                    }
                }
                0
            }
            Err(e) => fail(&e),
        },
        "create" => {
            let name = option_value(args, "--name")
                .or_else(|| positional.get(1).cloned())
                .unwrap_or_else(|| "cli".into());
            let scope = option_value(args, "--scope")
                .or_else(|| positional.get(2).cloned())
                .unwrap_or_else(|| "read".into());
            if !["read", "write"].contains(&scope.as_str()) {
                eprintln!("--scope must be read or write");
                return 2;
            }
            let ids = if token_projects(args).is_empty() {
                match c.projects().await {
                    Ok(projects) => projects.into_iter().map(|project| project.id).collect(),
                    Err(e) => return fail(&e),
                }
            } else {
                token_projects(args)
            };
            if ids.is_empty() {
                eprintln!("token creation requires at least one --project");
                return 2;
            }
            match c.create_token(&name, &scope, ids).await {
                Ok((token, secret)) => {
                    if json {
                        print_json(&serde_json::json!({"token": token, "secret": secret}));
                    } else {
                        println!("token {} · {}", token.id, secret);
                        println!("export XWIKI_TOKEN={secret}");
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "revoke" => {
            let Some(id) = positional.get(1) else {
                return usage();
            };
            match c.revoke_token(id).await {
                Ok(()) => {
                    if json {
                        print_json(&serde_json::json!({"ok": true, "id": id}));
                    } else {
                        println!("revoked {id}");
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => usage(),
    }
}

// ----- user -----

async fn cmd_user(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let verb = positional.first().map(String::as_str).unwrap_or("list");
    let json = wants_json(args);
    let c = client(args);
    match verb {
        "list" => match c.users().await {
            Ok(users) => {
                if json {
                    print_json(&users);
                } else {
                    for user in users {
                        println!(
                            "{:<24} {:<16} admin={} disabled={}",
                            user.id, user.username, user.is_admin, user.disabled
                        );
                    }
                }
                0
            }
            Err(e) => fail(&e),
        },
        "create" => {
            let Some(username) =
                option_value(args, "--username").or_else(|| positional.get(1).cloned())
            else {
                return usage();
            };
            let Some(password) =
                option_value(args, "--password").or_else(|| positional.get(2).cloned())
            else {
                return usage();
            };
            let display_name = option_value(args, "--display-name").unwrap_or_default();
            match c
                .create_user_with_options(
                    &username,
                    &password,
                    &display_name,
                    has_flag(args, "--admin"),
                )
                .await
            {
                Ok(user) => {
                    if json {
                        print_json(&user);
                    } else {
                        println!("created {} ({})", user.username, user.id);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "enable" | "disable" => {
            let Some(id) = positional.get(1) else {
                return usage();
            };
            let id = match resolve_user_id(&c, id).await {
                Ok(id) => id,
                Err(e) => return fail(&e),
            };
            match c.set_user_enabled(&id, verb == "enable").await {
                Ok(user) => {
                    if json {
                        print_json(&user);
                    } else {
                        println!("{} disabled={}", user.username, user.disabled);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "delete" => {
            let Some(id) = positional.get(1) else {
                return usage();
            };
            let id = match resolve_user_id(&c, id).await {
                Ok(id) => id,
                Err(e) => return fail(&e),
            };
            match c.delete_user(&id).await {
                Ok(()) => {
                    if json {
                        print_json(&serde_json::json!({"ok": true, "id": id}));
                    } else {
                        println!("deleted {id}");
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => usage(),
    }
}

/// `user enable|disable|delete` accept an id or a username; the server only
/// knows ids, so usernames are resolved through the users list.
async fn resolve_user_id(c: &Client, id_or_username: &str) -> Result<String, ApiError> {
    if id_or_username.starts_with("usr_") {
        return Ok(id_or_username.to_string());
    }
    let users = c.users().await?;
    users
        .into_iter()
        .find(|user| user.username == id_or_username)
        .map(|user| user.id)
        .ok_or_else(|| ApiError {
            code: "user_not_found".into(),
            message: format!("no user named {id_or_username}"),
            request_id: None,
            status: 404,
        })
}

// ----- audit -----

async fn cmd_audit(args: &[String]) -> i32 {
    let positional = positional_args(args);
    let project = if positional.first().map(String::as_str) == Some("list") {
        positional.get(1)
    } else {
        positional.first()
    };
    let Some(project) = project else {
        return usage();
    };
    let limit = match parse_u32_option(args, "--limit") {
        Ok(Some(limit)) if (1..=100).contains(&limit) => limit,
        Ok(None) => 20,
        Ok(Some(_)) => {
            eprintln!("--limit must be between 1 and 100");
            return 2;
        }
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let offset = match parse_u32_option(args, "--offset") {
        Ok(Some(offset)) => offset,
        Ok(None) => 0,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let json = wants_json(args);
    let c = client(args);
    match c.audit_page(project, limit, offset).await {
        Ok(page) => {
            if json {
                print_json(&page);
            } else {
                for entry in &page.entries {
                    println!(
                        "{}  {:<10} {} {}",
                        entry.created_at.split('T').next().unwrap_or(""),
                        entry.action,
                        entry.actor_id,
                        entry.path
                    );
                }
                if page.has_more {
                    eprintln!(
                        "…还有更多（已显示 {} 条，用 --limit/--offset 翻页）",
                        page.entries.len()
                    );
                }
            }
            0
        }
        Err(e) => fail(&e),
    }
}
