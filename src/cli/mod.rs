//! agentdocs-client CLI (spec: `cli` 层).
//!
//! Exit codes: 0 ok · 2 usage · 3 auth/permission · 4 not found · 5 revision
//! or lock conflict · 6 network/server error. Data on stdout, progress and
//! errors on stderr. `--json` switches list outputs to stable JSON.

use crate::api::{ApiError, Client};
use crate::config;

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

/// Builds a client from --server / saved config, with a bearer token from
/// AGENTDOCS_TOKEN when set.
fn client(args: &[String]) -> Client {
    Client::with_token(
        &server_from_args(args),
        std::env::var("AGENTDOCS_TOKEN").ok(),
    )
}

/// --prefix values (repeatable); defaults to ["docs"] so a write token can
/// touch the conventional docs tree without extra flags.
fn token_prefixes(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--prefix" {
            if let Some(p) = args.get(i + 1) {
                out.push(p.clone());
                i += 2;
                continue;
            }
        }
        i += 1;
    }
    if out.is_empty() {
        out.push("docs".into());
    }
    out
}

fn wants_json(args: &[String]) -> bool {
    args.iter().any(|a| a == "--json")
}

fn exit_code(err: &ApiError) -> i32 {
    match err.code.as_str() {
        "authentication_required"
        | "invalid_credentials"
        | "invalid_token"
        | "account_disabled"
        | "agent_forbidden"
        | "admin_required" => 3,
        "not_found" | "project_not_found" | "doc_not_found" | "commit_not_found"
        | "token_not_found" | "user_not_found" => 4,
        "revision_conflict" | "page_locked" | "idempotency_conflict" | "revert_conflict" => 5,
        "network_error" => 6,
        _ => 6,
    }
}

fn fail(err: &ApiError) -> i32 {
    eprintln!("{err}");
    exit_code(err)
}

fn usage() -> i32 {
    eprintln!(
        "usage: agentdocs-client <command> [args]

  server status|info          service meta & capabilities
  config show|set-server <url>
  login [--username U] [--password P]
  whoami
  project list|create|show|archive|restore|rename|delete <args>
  doc tree|get|create|update|delete|move <args>
  search <query>
  history list|show|diff <args>
  lock status|acquire|release <project> <path>
  token list|create|revoke
  user list|create|enable|disable
  audit list <project>
  gui                          launch the desktop app

Global: --server <url>  --json   Env: AGENTDOCS_TOKEN (bearer auth)"
    );
    2
}

pub fn run(args: Vec<String>) -> i32 {
    if args.is_empty() {
        return usage();
    }
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("runtime error: {e}");
            return 6;
        }
    };
    let cmd = args[0].as_str();
    let rest = &args[1..];
    rt.block_on(async move {
        match cmd {
            "server" => cmd_server(rest).await,
            "config" => cmd_config(rest),
            "login" => cmd_login(rest).await,
            "whoami" => cmd_whoami(rest).await,
            "project" => cmd_project(rest).await,
            "doc" => cmd_doc(rest).await,
            "search" => cmd_search(rest).await,
            "history" => cmd_history(rest).await,
            "lock" => cmd_lock(rest).await,
            "token" => cmd_token(rest).await,
            "user" => cmd_user(rest).await,
            "audit" => cmd_audit(rest).await,
            _ => usage(),
        }
    })
}

// ----- server -----

async fn cmd_server(args: &[String]) -> i32 {
    let verb = args.first().map(String::as_str).unwrap_or("status");
    match verb {
        "status" | "info" => {
            let c = client(args);
            match c.meta().await {
                Ok(m) => {
                    println!("version {} · api v{}", m.version, m.api_version);
                    println!(
                        "limits: doc ≤ {} MiB · import ≤ {} MiB · diff ≤ {} MiB",
                        m.limits.max_doc_bytes / (1 << 20),
                        m.limits.max_import_bytes / (1 << 20),
                        m.limits.max_diff_bytes / (1 << 20)
                    );
                    println!("capabilities: {}", m.capabilities.join(", "));
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
    let verb = args.first().map(String::as_str).unwrap_or("show");
    match verb {
        "show" => {
            println!("server: {}", config::load_server());
            0
        }
        "set-server" => {
            let Some(url) = args.get(1) else {
                return usage();
            };
            config::save_server(url);
            println!("server: {url}");
            0
        }
        _ => usage(),
    }
}

// ----- auth -----

async fn cmd_login(args: &[String]) -> i32 {
    let c = client(args);
    let (mut username, mut password) = (None, None);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--username" => {
                username = args.get(i + 1).cloned();
                i += 2;
            }
            "--password" => {
                password = args.get(i + 1).cloned();
                i += 2;
            }
            _ => i += 1,
        }
    }
    let username = username.unwrap_or_else(|| {
        print!("username: ");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let mut s = String::new();
        std::io::stdin().read_line(&mut s).ok();
        s.trim().to_string()
    });
    let password = password.unwrap_or_else(|| {
        print!("password: ");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        let mut s = String::new();
        std::io::stdin().read_line(&mut s).ok();
        s.trim().to_string()
    });
    match c.login(&username, &password).await {
        Ok(u) => {
            println!("logged in as {} (admin: {})", u.username, u.is_admin);
            // CLI sessions don't persist across processes: mint a bearer
            // token right away so the user can export it.
            let ids: Vec<String> = c
                .projects()
                .await
                .map(|ps| ps.into_iter().map(|p| p.id).collect())
                .unwrap_or_default();
            if ids.is_empty() {
                eprintln!("note: no projects yet — create one first, then re-run login");
            }
            let prefixes = token_prefixes(args);
            match c.create_token("cli-login", "write", ids, prefixes).await {
                Ok((_, secret)) => {
                    println!("export AGENTDOCS_TOKEN={secret}");
                }
                Err(e) => eprintln!("token mint failed: {e}"),
            }
            0
        }
        Err(e) => fail(&e),
    }
}

async fn cmd_whoami(args: &[String]) -> i32 {
    let c = client(args);
    match c.me().await {
        Ok(u) => {
            println!(
                "{} ({}) admin={} disabled={}",
                u.username, u.id, u.is_admin, u.disabled
            );
            0
        }
        Err(e) => fail(&e),
    }
}

// ----- project -----

async fn cmd_project(args: &[String]) -> i32 {
    let c = client(args);
    let verb = args.first().map(String::as_str).unwrap_or("list");
    let json = wants_json(args);
    match verb {
        "list" => match c.projects().await {
            Ok(ps) => {
                if json {
                    println!("{}", serde_json::to_string_pretty(&ps).unwrap());
                } else {
                    for p in ps {
                        let state = if p.archived { "archived" } else { "active" };
                        println!("{:<28} {:<24} {}", p.id, p.name, state);
                    }
                }
                0
            }
            Err(e) => fail(&e),
        },
        "create" => {
            let Some(name) = args.get(1) else {
                return usage();
            };
            let desc = args.get(2).cloned().unwrap_or_default();
            match c.create_project(name, &desc).await {
                Ok(p) => {
                    println!("created {} ({})", p.name, p.id);
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "show" => {
            let Some(id) = args.get(1) else {
                return usage();
            };
            match c.project(id).await {
                Ok(p) => {
                    println!("{} · {} · archived={}", p.id, p.name, p.archived);
                    if !p.description.is_empty() {
                        println!("{}", p.description);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "archive" | "restore" => {
            let Some(id) = args.get(1) else {
                return usage();
            };
            match c.set_archived(id, verb == "archive").await {
                Ok(p) => {
                    println!("{} archived={}", p.name, p.archived);
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "rename" => {
            let Some(id) = args.get(1) else {
                return usage();
            };
            let Some(new_name) = args.get(2) else {
                return usage();
            };
            match c.rename_project(id, new_name).await {
                Ok(p) => {
                    println!("renamed to {} ({})", p.name, p.id);
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "delete" => {
            let Some(id) = args.get(1) else {
                return usage();
            };
            // Destructive: require interactive username + password
            // confirmation. The credentials are verified against the server
            // (/auth/login) before the project is removed.
            let username = {
                print!("confirm username: ");
                std::io::Write::flush(&mut std::io::stdout()).ok();
                let mut s = String::new();
                std::io::stdin().read_line(&mut s).ok();
                s.trim().to_string()
            };
            let password = {
                print!("confirm password: ");
                std::io::Write::flush(&mut std::io::stdout()).ok();
                let mut s = String::new();
                std::io::stdin().read_line(&mut s).ok();
                s.trim().to_string()
            };
            if username.is_empty() || password.is_empty() {
                eprintln!("aborted: empty credentials");
                return 2;
            }
            match c.login(&username, &password).await {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("aborted: password verification failed: {e}");
                    return 2;
                }
            }
            match c.delete_project(id).await {
                Ok(()) => {
                    println!("deleted {id}");
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => usage(),
    }
}

// ----- doc -----

async fn cmd_doc(args: &[String]) -> i32 {
    let c = client(args);
    let verb = args.first().map(String::as_str).unwrap_or("tree");
    let json = wants_json(args);
    match verb {
        "tree" => {
            let Some(project) = args.get(1) else {
                return usage();
            };
            // First non-flag argument after the project is the directory.
            let path = args
                .iter()
                .skip(2)
                .find(|a| !a.starts_with('-'))
                .cloned()
                .unwrap_or_default();
            match c.tree(project, &path).await {
                Ok(entries) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&entries).unwrap());
                    } else {
                        for e in entries {
                            let icon = if e.r#type == "tree" { "d" } else { "f" };
                            println!("{icon} {}", e.path);
                        }
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "get" => {
            let (Some(project), Some(path)) = (args.get(1), args.get(2)) else {
                return usage();
            };
            match c.page(project, path).await {
                Ok(p) => {
                    print!("{}", p.content);
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "create" | "update" => {
            let (Some(project), Some(path)) = (args.get(1), args.get(2)) else {
                return usage();
            };
            let message = args
                .iter()
                .position(|a| a == "--message")
                .and_then(|i| args.get(i + 1))
                .cloned()
                .unwrap_or_else(|| format!("{verb} {path}"));
            let content = read_stdin_or_file(args);
            let op = if verb == "create" { "create" } else { "update" };
            match c.edit_doc(project, path, op, &content, &message).await {
                Ok(r) => {
                    println!("committed {} ({})", r.commit, r.revision);
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "delete" => {
            let (Some(project), Some(path)) = (args.get(1), args.get(2)) else {
                return usage();
            };
            let message = format!("delete {path}");
            match c.delete_doc(project, path, &message).await {
                Ok(r) => {
                    println!("deleted ({})", r.commit);
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "move" => {
            let (Some(project), Some(from), Some(to)) = (args.get(1), args.get(2), args.get(3))
            else {
                return usage();
            };
            match c
                .move_doc(project, from, to, &format!("move {from} → {to}"))
                .await
            {
                Ok(r) => {
                    println!("moved ({})", r.commit);
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => usage(),
    }
}

/// Reads doc content from --file <path>, or stdin when piped.
fn read_stdin_or_file(args: &[String]) -> String {
    if let Some(i) = args.iter().position(|a| a == "--file") {
        if let Some(path) = args.get(i + 1) {
            return std::fs::read_to_string(path).unwrap_or_default();
        }
    }
    let mut buf = String::new();
    let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf);
    buf
}

// ----- search -----

async fn cmd_search(args: &[String]) -> i32 {
    let (Some(project), Some(q)) = (args.first(), args.get(1)) else {
        return usage();
    };
    let c = client(args);
    match c.search(project, q).await {
        Ok(results) => {
            for r in results {
                println!("{}", r.path);
                println!("    {}", r.snippet);
            }
            0
        }
        Err(e) => fail(&e),
    }
}

// ----- history -----

async fn cmd_history(args: &[String]) -> i32 {
    let c = client(args);
    let verb = args.first().map(String::as_str).unwrap_or("list");
    match verb {
        "list" => {
            let Some(project) = args.get(1) else {
                return usage();
            };
            match c.commits(project, 50).await {
                Ok(cs) => {
                    for c in cs {
                        let short: String = c.sha.chars().take(7).collect();
                        println!("{short}  {}  {}", c.author, c.message);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "show" => {
            let (Some(project), Some(sha)) = (args.get(1), args.get(2)) else {
                return usage();
            };
            match c.commit_detail(project, sha).await {
                Ok(d) => {
                    println!("{}  {}  {}", d.sha, d.author, d.date);
                    println!("{}", d.message);
                    for f in &d.files {
                        println!("  {}  {}", f.status, f.path);
                    }
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "diff" => {
            let (Some(project), Some(sha)) = (args.get(1), args.get(2)) else {
                return usage();
            };
            match c.diff_stats(project, sha).await {
                Ok(stats) => {
                    for s in stats {
                        println!("{:>5} {:>5}  {}", s.added, s.deleted, s.path);
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
    let c = client(args);
    let verb = args.first().map(String::as_str).unwrap_or("status");
    let (Some(project), Some(path)) = (args.get(1), args.get(2)) else {
        return usage();
    };
    match verb {
        "status" => match c.acquire_lock(project, path).await {
            // status uses the same shape; acquire is idempotent-ish for the holder
            Ok(l) => {
                println!(
                    "{} · held by {} · expires {}",
                    l.path, l.username, l.expires_at
                );
                0
            }
            Err(e) => fail(&e),
        },
        "acquire" => match c.acquire_lock(project, path).await {
            Ok(l) => {
                println!("locked {} until {}", l.path, l.expires_at);
                0
            }
            Err(e) => fail(&e),
        },
        "release" => match c.release_lock(project, path).await {
            Ok(_) => {
                println!("released");
                0
            }
            Err(e) => fail(&e),
        },
        _ => usage(),
    }
}

// ----- token -----

async fn cmd_token(args: &[String]) -> i32 {
    let c = client(args);
    let verb = args.first().map(String::as_str).unwrap_or("list");
    match verb {
        "list" => match c.tokens().await {
            Ok(ts) => {
                for t in ts {
                    println!("{:<24} {:<8} {}", t.id, t.scope, t.name);
                }
                0
            }
            Err(e) => fail(&e),
        },
        "create" => {
            let name = args.get(1).cloned().unwrap_or_else(|| "cli".into());
            let scope = args.get(2).cloned().unwrap_or_else(|| "read".into());
            let ids = c
                .projects()
                .await
                .map(|ps| ps.into_iter().map(|p| p.id).collect())
                .unwrap_or_default();
            let prefixes = token_prefixes(args);
            match c.create_token(&name, &scope, ids, prefixes).await {
                Ok((t, secret)) => {
                    println!("token {} · {}", t.id, secret);
                    println!("export AGENTDOCS_TOKEN={secret}");
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "revoke" => {
            let Some(id) = args.get(1) else {
                return usage();
            };
            match c.revoke_token(id).await {
                Ok(_) => {
                    println!("revoked {id}");
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
    let c = client(args);
    let verb = args.first().map(String::as_str).unwrap_or("list");
    match verb {
        "list" => match c.users().await {
            Ok(us) => {
                for u in us {
                    println!(
                        "{:<24} {:<16} admin={} disabled={}",
                        u.id, u.username, u.is_admin, u.disabled
                    );
                }
                0
            }
            Err(e) => fail(&e),
        },
        "create" => {
            let (Some(username), Some(password)) = (args.get(1), args.get(2)) else {
                return usage();
            };
            match c.create_user(username, password).await {
                Ok(u) => {
                    println!("created {} ({})", u.username, u.id);
                    0
                }
                Err(e) => fail(&e),
            }
        }
        "enable" | "disable" => {
            let Some(id) = args.get(1) else {
                return usage();
            };
            match c.set_user_enabled(id, verb == "enable").await {
                Ok(u) => {
                    println!("{} disabled={}", u.username, u.disabled);
                    0
                }
                Err(e) => fail(&e),
            }
        }
        _ => usage(),
    }
}

// ----- audit -----

async fn cmd_audit(args: &[String]) -> i32 {
    let Some(project) = args.first() else {
        return usage();
    };
    let c = client(args);
    match c.audit(project).await {
        Ok(entries) => {
            for e in entries {
                println!(
                    "{}  {:<10} {} {}",
                    e.created_at.split('T').next().unwrap_or(""),
                    e.action,
                    e.actor_id,
                    e.path
                );
            }
            0
        }
        Err(e) => fail(&e),
    }
}
