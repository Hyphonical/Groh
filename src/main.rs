mod cli;
mod config;
mod docker;
mod palette;
mod script;
mod term;
mod view;
mod widgets;

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
use docker::{ContainerDelta, ContainerInfo, ContainerState};
use futures_util::Stream;
use palette::FOREGROUND;
use rust_embed::RustEmbed;
use std::convert::Infallible;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use term::Term;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::{RwLock, mpsc, watch};

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
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(path)),
    );
    response
}

struct AppState {
    runtime: Arc<RwLock<widgets::Runtime>>,
    version: watch::Sender<u64>,
}

async fn events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let version = state.version.subscribe();
    let stream = futures_util::stream::unfold(
        (true, state, version),
        |(first, state, mut version)| async move {
            if !first && version.changed().await.is_err() {
                return None;
            }
            let payload = {
                let guard = state.runtime.read().await;
                widgets::payload(&guard)
            };
            let Ok(json) = serde_json::to_string(&payload) else {
                return None;
            };
            Some((Ok(Event::default().data(json)), (false, state, version)))
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn startup_failed(term: &Term, message: &str) -> ! {
    let mut stderr = io::stderr().lock();
    term.error(&mut stderr, "startup failed", Some(message), None);
    std::process::exit(1);
}

fn report_error(term: &Term, title: &str, detail: &str) {
    let mut stderr = io::stderr().lock();
    if term.is_tty() {
        term.rule(&mut stderr, "Error");
        term.error(&mut stderr, title, Some(detail), None);
    } else {
        term.event(&mut stderr, "error", &format!("{title}: {detail}"));
    }
}

fn print_startup(term: &Term, url: &str) {
    let mut stderr = io::stderr().lock();
    if term.is_tty() {
        term.rule(&mut stderr, "Startup");
        term.row(&mut stderr, "url", url, FOREGROUND);
        term.done(&mut stderr, "serve", "listening");
    } else {
        term.event(&mut stderr, "startup", &format!("listening on {url}"));
    }
}

fn print_initial_snapshot(term: &Term, list: &[ContainerInfo]) {
    let mut stderr = io::stderr().lock();
    if term.is_tty() {
        let _ = io::Write::write_all(&mut stderr, b"\n");
        term.rule(&mut stderr, "Containers");
        term.container_header(&mut stderr);
        for c in list {
            term.container_row(&mut stderr, &c.name, c.state.as_str(), &c.detail);
        }
        let _ = io::Write::write_all(&mut stderr, b"\n");
        term.rule(&mut stderr, "Live");
    } else {
        let running = list
            .iter()
            .filter(|c| c.state == ContainerState::Running)
            .count();
        let other = list.len() - running;
        term.event(
            &mut stderr,
            "containers",
            &format!("{running} running, {other} other"),
        );
    }
}

fn print_delta(term: &Term, delta: &ContainerDelta) {
    let mut stderr = io::stderr().lock();
    match delta {
        ContainerDelta::Added(name) => {
            if term.is_tty() {
                term.container_change(&mut stderr, true, name);
            } else {
                term.event(&mut stderr, "containers", &format!("{name} added"));
            }
        }
        ContainerDelta::Removed(name) => {
            if term.is_tty() {
                term.container_change(&mut stderr, false, name);
            } else {
                term.event(&mut stderr, "containers", &format!("{name} removed"));
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let config = match cli::parse() {
        Action::Run(c) => c,
        Action::Exit(code) => std::process::exit(code),
    };

    let term = Term::stderr(config.no_color);
    let loaded = match config::load(&config.config) {
        Ok(loaded) => loaded,
        Err(message) => startup_failed(&term, &message),
    };

    let (snapshot_tx, mut cli_snapshot_rx) = watch::channel(Vec::new());
    let running = match widgets::start(
        &loaded,
        &config.widgets_dir,
        config.pool_size,
        config.script_timeout,
        snapshot_tx.subscribe(),
    ) {
        Ok(running) => running,
        Err(message) => startup_failed(&term, &message),
    };

    let app_state = Arc::new(AppState {
        runtime: Arc::clone(&running.runtime),
        version: running.version.clone(),
    });
    let app = Router::new()
        .route("/api/events", get(events))
        .with_state(app_state)
        .fallback(get(serve_asset));

    let addr = SocketAddr::new(config.bind, config.port);
    let url = format!("http://{addr}");
    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            report_error(&term, &format!("failed to bind {addr}"), &err.to_string());
            std::process::exit(1);
        }
    };

    print_startup(&term, &url);

    let term = Arc::new(term);
    let (delta_tx, mut delta_rx) = mpsc::unbounded_channel();

    let watcher_term = Arc::clone(&term);
    tokio::spawn(async move {
        if let Err(err) = docker::watch(snapshot_tx, delta_tx).await {
            report_error(&watcher_term, "docker connection failed", &err.to_string());
        }
    });

    let cli_term = Arc::clone(&term);
    tokio::spawn(async move {
        if cli_snapshot_rx.changed().await.is_ok() {
            let list = cli_snapshot_rx.borrow_and_update().clone();
            print_initial_snapshot(&cli_term, &list);
        }

        while let Some(delta) = delta_rx.recv().await {
            print_delta(&cli_term, &delta);
        }
    });

    if let Err(err) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        report_error(&term, "server error", &err.to_string());
        std::process::exit(1);
    }

    let mut stderr = io::stderr().lock();
    if term.is_tty() {
        term.rule(&mut stderr, "Shutdown");
        term.done(&mut stderr, "serve", "shutting down");
    } else {
        term.event(&mut stderr, "shutdown", "shutting down");
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
