//! ACowork-hosted FigCraft process (ADR-116 generic runtime).
//!
//! Speaks the Gateway generic-supervisor CLI contract (`--host` / `--port` /
//! `--port-file` / `--gateway-health-url` / `--log-level`) and serves:
//! - `GET /health` — supervisor liveness
//! - `GET /ui/*` — static web UI (Trunk build of `figcraft-web`, or placeholder)
//! - `POST /mcp` — empty tools catalog (`auto_inject = false` in app.toml)
//!
//! The UI directory is resolved as: `--ui-dir` → sibling `../ui` of this
//! binary (package layout) → `./ui`.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use axum::Router;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use clap::Parser;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Parser, Debug)]
#[command(
    name = "figcraft-acowork",
    about = "FigCraft host process for ACowork (loopback HTTP, ADR-116)"
)]
struct Cli {
    /// Loopback bind address (Gateway always passes 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Preferred listen port; on conflict, try up to +20
    #[arg(long, default_value = "18090")]
    port: u16,

    /// Write the bound port here for the Gateway supervisor handshake
    #[arg(long)]
    port_file: Option<PathBuf>,

    /// Gateway /health URL — self-exit after sustained failure (ADR-018)
    #[arg(long)]
    gateway_health_url: Option<String>,

    #[arg(long, default_value = "10000")]
    gateway_health_interval_ms: u64,

    #[arg(long, default_value = "300000")]
    gateway_health_timeout_ms: u64,

    /// Override the directory that backs `/ui/`
    #[arg(long)]
    ui_dir: Option<PathBuf>,

    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_logging(&cli.log_level);

    let ui_dir = resolve_ui_dir(cli.ui_dir.as_deref());
    ensure_placeholder_ui(&ui_dir);
    log::info!("serving FigCraft UI from {}", ui_dir.display());

    if let Some(url) = cli.gateway_health_url.clone() {
        spawn_gateway_health_watchdog(
            url,
            Duration::from_millis(cli.gateway_health_interval_ms),
            Duration::from_millis(cli.gateway_health_timeout_ms),
        );
    }

    let index = ui_dir.join("index.html");
    let ui_service = ServeDir::new(&ui_dir)
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(index));

    let app = Router::new()
        .route("/health", get(health))
        .route("/mcp", post(mcp_stub))
        .nest_service("/ui", ui_service)
        .layer(axum::middleware::from_fn(wasm_content_type));

    let listener = match bind_with_fallback(&cli.host, cli.port).await {
        Ok(l) => l,
        Err(e) => {
            log::error!("bind failed: {e}");
            std::process::exit(1);
        }
    };
    let addr = match listener.local_addr() {
        Ok(a) => a,
        Err(e) => {
            log::error!("local_addr failed: {e}");
            std::process::exit(1);
        }
    };
    log::info!("figcraft-acowork listening on {addr}");

    if let Some(path) = &cli.port_file {
        if let Err(e) = std::fs::write(path, addr.port().to_string()) {
            log::error!("failed to write port file {}: {e}", path.display());
            std::process::exit(1);
        }
        log::info!("port file written to {} ({})", path.display(), addr.port());
    }

    if let Err(e) = axum::serve(listener, app).await {
        log::error!("server exited: {e}");
        std::process::exit(1);
    }
}

fn init_logging(level: &str) {
    let mut builder = env_logger::Builder::new();
    builder.parse_filters(level);
    builder.format_timestamp_secs();
    let _ = builder.try_init();
}

fn resolve_ui_dir(explicit: Option<&Path>) -> PathBuf {
    if let Some(p) = explicit {
        return p.to_path_buf();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            let sibling = bin_dir.join("..").join("ui");
            if let Ok(canon) = sibling.canonicalize() {
                if canon.is_dir() {
                    return canon;
                }
            }
            if sibling.is_dir() {
                return sibling;
            }
        }
    }
    PathBuf::from("ui")
}

fn ensure_placeholder_ui(ui_dir: &Path) {
    let index = ui_dir.join("index.html");
    if index.is_file() {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(ui_dir) {
        log::warn!("could not create ui dir {}: {e}", ui_dir.display());
        return;
    }
    if let Err(e) = std::fs::write(&index, PLACEHOLDER_HTML) {
        log::warn!("could not write placeholder {}: {e}", index.display());
    }
}

async fn bind_with_fallback(host: &str, start: u16) -> Result<tokio::net::TcpListener, String> {
    let ip: std::net::IpAddr = host
        .parse()
        .map_err(|e| format!("invalid --host {host:?}: {e}"))?;
    let max = start.saturating_add(20);
    let mut port = start;
    loop {
        let addr = SocketAddr::new(ip, port);
        match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => return Ok(l),
            Err(e) if port < max => {
                log::warn!("port {port} occupied ({e}) — trying next");
                port += 1;
            }
            Err(e) => return Err(format!("could not bind {start}..={max}: {e}")),
        }
    }
}

async fn health() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": "ok",
        "app": "figcraft",
        "service": "figcraft-acowork",
    }))
}

/// Minimal Streamable-HTTP MCP stub. FigCraft is UI-first; tools come later.
async fn mcp_stub(body: axum::body::Bytes) -> Response {
    let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let method = req
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("");

    let result = match method {
        "initialize" => serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "figcraft", "version": env!("CARGO_PKG_VERSION") },
        }),
        "notifications/initialized" => {
            return StatusCode::ACCEPTED.into_response();
        }
        "tools/list" => serde_json::json!({ "tools": [] }),
        "ping" => serde_json::json!({}),
        _ => {
            let err = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {method}") },
            });
            return (StatusCode::OK, axum::Json(err)).into_response();
        }
    };

    (
        StatusCode::OK,
        axum::Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        })),
    )
        .into_response()
}

/// Ensure `.wasm` responses advertise `application/wasm` (Trunk hashed assets).
async fn wasm_content_type(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let is_wasm = req
        .uri()
        .path()
        .rsplit('.')
        .next()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("wasm"));
    let mut response = next.run(req).await;
    if is_wasm {
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/wasm"),
        );
    }
    response
}

const PLACEHOLDER_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>FigCraft — ACowork</title>
  <style>
    body { margin: 0; min-height: 100vh; display: grid; place-items: center;
      font: 15px/1.5 system-ui, sans-serif; background: #0d0e10; color: #e8eaed; }
    main { max-width: 36rem; padding: 2rem; }
    h1 { font-size: 1.4rem; font-weight: 600; margin: 0 0 .5rem; }
    p { color: #9aa3b2; margin: 0 0 .75rem; }
    code { color: #f0a92e; }
  </style>
</head>
<body>
  <main>
    <h1>FigCraft host is running</h1>
    <p>This process answers <code>/health</code> and <code>/ui/</code> for ACowork.
       Pack the Trunk build of <code>figcraft-web</code> into the package <code>ui/</code>
       directory to replace this placeholder.</p>
  </main>
</body>
</html>
"#;

fn spawn_gateway_health_watchdog(health_url: String, interval: Duration, timeout: Duration) {
    tokio::spawn(async move {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .no_proxy()
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                log::error!("gateway health client: {e}");
                return;
            }
        };
        let mut last_success = Instant::now();
        loop {
            tokio::time::sleep(interval).await;
            let healthy = match client.get(&health_url).send().await {
                Ok(resp) => resp.status().is_success(),
                Err(_) => false,
            };
            if healthy {
                last_success = Instant::now();
            } else if last_success.elapsed() >= timeout {
                log::error!(
                    "Gateway unreachable for {}s (limit {}s) — self-exiting (ADR-018)",
                    last_success.elapsed().as_secs(),
                    timeout.as_secs()
                );
                std::process::exit(0);
            }
        }
    });
}
