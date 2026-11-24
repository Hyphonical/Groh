mod cli;
mod docker;
mod term;

use axum::{
    Router,
    extract::{OriginalUri, State},
    http::{HeaderValue, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::get,
};
use cli::Action;
use docker::{ContainerDelta, ContainerInfo};
use futures_util::Stream;
use rust_embed::RustEmbed;
use std::convert::Infallible;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use term::{FG, Term};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::{mpsc, watch};
#[derive(RustEmbed)]
#[folder = "frontend/build"]
struct Assets;

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

async fn serve_asset(OriginalUri(uri): OriginalUri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    let Some(file) = Assets::get(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let mut response = (StatusCode::OK, file.data.into_owned()).into_response();
    if let Ok(value) = HeaderValue::from_str(content_type(path)) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    response
}

async fn events(
    State(rx): State<watch::Receiver<Vec<ContainerInfo>>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = futures_util::stream::unfold((true, rx), |(first, mut rx)| async move {
        if !first && rx.changed().await.is_err() {
            return None;
        }
        let json = serde_json::to_string(&*rx.borrow_and_update()).unwrap_or_default();
        Some((Ok(Event::default().data(json)), (false, rx)))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[tokio::main]
async fn main() {
    let config = match cli::parse() {
        Action::Run(c) => c,
        Action::Exit(code) => std::process::exit(code),
    };

    let term = Term::stderr(config.no_color);
    let (snapshot_tx, mut snapshot_rx) = watch::channel(Vec::new());
    let sse_rx = snapshot_rx.clone();
    let app = Router::new()
        .route("/api/events", get(events))
        .with_state(sse_rx)
        .fallback(get(serve_asset));
    let addr = SocketAddr::new(config.bind, config.port);
    let url = format!("http://{addr}");

    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(err) => {
            let mut stderr = io::stderr().lock();
            if term.is_tty() {
                term.rule(&mut stderr, "Error");
                let title = format!("failed to bind {addr}");
                let detail = err.to_string();
                term.error(&mut stderr, &title, Some(&detail), None);
            } else {
                term.event(
                    &mut stderr,
                    "error",
                    &format!("failed to bind {addr}: {err}"),
                );
            }
            std::process::exit(1);
        }
    };

    {
        let mut stderr = io::stderr().lock();
        if term.is_tty() {
            term.rule(&mut stderr, "Startup");
            term.row(&mut stderr, "url", &url, FG);
            term.done(&mut stderr, "serve", "listening");
        } else {
            term.event(&mut stderr, "startup", &format!("listening on {url}"));
        }
    }

    let term = Arc::new(term);
    let (delta_tx, mut delta_rx) = mpsc::unbounded_channel();

    let watcher_term = Arc::clone(&term);
    tokio::spawn(async move {
        if let Err(err) = docker::watch(snapshot_tx, delta_tx).await {
            let mut stderr = io::stderr().lock();
            if watcher_term.is_tty() {
                watcher_term.rule(&mut stderr, "Error");
                let detail = err.to_string();
                watcher_term.error(&mut stderr, "docker connection failed", Some(&detail), None);
            } else {
                watcher_term.event(
                    &mut stderr,
                    "error",
                    &format!("docker connection failed: {err}"),
                );
            }
        }
    });

    let cli_term = Arc::clone(&term);
    tokio::spawn(async move {
        if snapshot_rx.changed().await.is_ok() {
            let list = snapshot_rx.borrow().clone();
            let mut stderr = io::stderr().lock();
            if cli_term.is_tty() {
                let _ = io::Write::write_all(&mut stderr, b"\n");
                cli_term.rule(&mut stderr, "Containers");
                cli_term.container_header(&mut stderr);
                for c in &list {
                    cli_term.container_row(&mut stderr, &c.name, &c.state, &c.detail);
                }
                let _ = io::Write::write_all(&mut stderr, b"\n");
                cli_term.rule(&mut stderr, "Live");
            } else {
                let running = list.iter().filter(|c| c.state == "running").count();
                let other = list.len() - running;
                cli_term.event(
                    &mut stderr,
                    "containers",
                    &format!("{running} running, {other} other"),
                );
            }
        }

        while let Some(delta) = delta_rx.recv().await {
            let mut stderr = io::stderr().lock();
            match delta {
                ContainerDelta::Added(name) => {
                    if cli_term.is_tty() {
                        cli_term.container_change(&mut stderr, true, &name);
                    } else {
                        cli_term.event(&mut stderr, "containers", &format!("{name} added"));
                    }
                }
                ContainerDelta::Removed(name) => {
                    if cli_term.is_tty() {
                        cli_term.container_change(&mut stderr, false, &name);
                    } else {
                        cli_term.event(&mut stderr, "containers", &format!("{name} removed"));
                    }
                }
            }
        }
    });

    if let Err(err) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        let mut stderr = io::stderr().lock();
        if term.is_tty() {
            term.rule(&mut stderr, "Error");
            let detail = err.to_string();
            term.error(&mut stderr, "server error", Some(&detail), None);
        } else {
            term.event(&mut stderr, "error", &format!("server error: {err}"));
        }
        std::process::exit(1);
    }

    {
        let mut stderr = io::stderr().lock();
        if term.is_tty() {
            term.rule(&mut stderr, "Shutdown");
            term.done(&mut stderr, "serve", "shutting down");
        } else {
            term.event(&mut stderr, "shutdown", "shutting down");
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            sig.recv().await;
        } else {
            std::future::pending::<()>().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
