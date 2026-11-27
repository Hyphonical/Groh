use crate::config::{self, LoadedConfig, Placement};
use crate::docker::ContainerInfo;
use crate::script::{self, Meta, RunContext};
use crate::view::{Node, Size};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Semaphore, watch};
use tokio::task::{JoinError, JoinHandle, JoinSet};

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Slot {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) size: Size,
    pub(crate) tree: Option<Node>,
    pub(crate) updated_at: Option<u64>,
    pub(crate) show_summary: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Payload {
    pub(crate) columns: Vec<Vec<Slot>>,
}

pub(crate) struct Runtime {
    cache: HashMap<String, Slot>,
    columns: Vec<Vec<String>>,
    version: u64,
}

pub(crate) struct Running {
    pub(crate) runtime: Arc<RwLock<Runtime>>,
    pub(crate) version: watch::Sender<u64>,
    supervisor: JoinHandle<()>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.supervisor.abort();
    }
}

struct Prepared {
    placement: Placement,
    meta: Meta,
    config: serde_json::Value,
    secrets: Vec<String>,
    source: String,
}

struct Done {
    idx: usize,
    id: String,
    result: Result<Node, String>,
    secrets: Vec<String>,
}

pub(crate) fn start(
    config: &LoadedConfig,
    widgets_dir: &Path,
    pool_size: usize,
    script_timeout: Duration,
    snapshot_rx: watch::Receiver<Vec<ContainerInfo>>,
) -> Result<Running, String> {
    let http_agent = script::host::http::build_agent();
    let http_cache: Arc<Mutex<HashMap<String, (Instant, String)>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let mut prepared = Vec::new();
    let mut cache = HashMap::new();

    for placement in config.placements() {
        let path = placement.source.resolve(widgets_dir);
        let source = std::fs::read_to_string(&path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        let label = placement.source.to_string();
        let meta = script::extract_meta(&source, &label)?;
        let values = config.widget_config(&placement.id);
        let resolved = config::validate_instance(&placement.id, &meta, values)?;
        let secrets = collect_secrets(&meta, &resolved);
        let size = placement.size.unwrap_or(meta.size);

        cache.insert(
            placement.id.clone(),
            Slot {
                id: placement.id.clone(),
                title: meta.title.clone(),
                size,
                tree: None,
                updated_at: None,
                show_summary: placement.show_summary,
            },
        );
        prepared.push(Arc::new(Prepared {
            placement: placement.clone(),
            meta,
            config: resolved,
            secrets,
            source,
        }));
    }

    let columns = config
        .columns
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|placement| placement.id.clone())
                .collect()
        })
        .collect();
    let runtime = Arc::new(RwLock::new(Runtime {
        cache,
        columns,
        version: 0,
    }));
    let (version, _) = watch::channel(0_u64);

    let supervisor_runtime = Arc::clone(&runtime);
    let supervisor_version = version.clone();
    let supervisor_handle = Supervisor {
        prepared,
        runtime: supervisor_runtime,
        snapshot_rx,
        http_agent,
        http_cache,
        pool_size: pool_size.max(1),
        script_timeout,
        version: supervisor_version,
    };
    let supervisor = tokio::spawn(supervisor_handle.run());

    Ok(Running {
        runtime,
        version,
        supervisor,
    })
}

pub(crate) fn payload(runtime: &Runtime) -> Payload {
    let columns = runtime
        .columns
        .iter()
        .map(|column| {
            column
                .iter()
                .filter_map(|id| runtime.cache.get(id).cloned())
                .collect()
        })
        .collect();
    Payload { columns }
}

struct Supervisor {
    prepared: Vec<Arc<Prepared>>,
    runtime: Arc<RwLock<Runtime>>,
    snapshot_rx: watch::Receiver<Vec<ContainerInfo>>,
    http_agent: ureq::Agent,
    http_cache: Arc<Mutex<HashMap<String, (Instant, String)>>>,
    pool_size: usize,
    script_timeout: Duration,
    version: watch::Sender<u64>,
}

impl Supervisor {
    async fn run(self) {
        if self.prepared.is_empty() {
            return;
        }

        let script_timeout = self.script_timeout;
        let mut in_flight = vec![false; self.prepared.len()];
        let mut next_due = vec![tokio::time::Instant::now(); self.prepared.len()];
        let mut join_set: JoinSet<Done> = JoinSet::new();
        let semaphore = Arc::new(Semaphore::new(self.pool_size));
        let timeout_budget = script_timeout + Duration::from_millis(250);

        loop {
            let now = tokio::time::Instant::now();
            for (idx, item) in self.prepared.iter().enumerate() {
                if in_flight[idx] || now < next_due[idx] {
                    continue;
                }
                in_flight[idx] = true;
                let item = Arc::clone(item);
                let semaphore = Arc::clone(&semaphore);
                let containers = self.snapshot_rx.borrow().clone();
                let http_agent = self.http_agent.clone();
                let http_cache = Arc::clone(&self.http_cache);

                join_set.spawn(async move {
                    let Ok(_permit) = semaphore.acquire_owned().await else {
                        return Done {
                            idx,
                            id: item.placement.id.clone(),
                            result: Err("widget semaphore closed".to_string()),
                            secrets: item.secrets.clone(),
                        };
                    };
                    let run_item = Arc::clone(&item);
                    let task = tokio::task::spawn_blocking(move || {
                        let run_context = RunContext {
                            http_agent,
                            http_cache,
                            containers,
                        };
                        script::run_script(
                            &run_item.source,
                            &run_item.meta,
                            &run_item.config,
                            &run_context,
                            script_timeout,
                        )
                    });
                    let result = match tokio::time::timeout(timeout_budget, task).await {
                        Ok(Ok(outcome)) => outcome,
                        Ok(Err(err)) => Err(format!("worker panicked: {err}")),
                        Err(_) => Err(format!(
                            "script timed out after {}s",
                            script_timeout.as_secs()
                        )),
                    };
                    Done {
                        idx,
                        id: item.placement.id.clone(),
                        result,
                        secrets: item.secrets.clone(),
                    }
                });
            }

            let next_wakeup = next_due
                .iter()
                .zip(&in_flight)
                .filter(|(_, busy)| !**busy)
                .map(|(due, _)| *due)
                .min();

            match next_wakeup {
                Some(wakeup) => {
                    tokio::select! {
                        Some(joined) = join_set.join_next() => {
                            handle_joined(
                                joined,
                                &self.prepared,
                                &mut in_flight,
                                &mut next_due,
                                &self.runtime,
                                &self.version,
                            ).await;
                        }
                        () = tokio::time::sleep_until(wakeup) => {}
                    }
                }
                None => {
                    if let Some(joined) = join_set.join_next().await {
                        handle_joined(
                            joined,
                            &self.prepared,
                            &mut in_flight,
                            &mut next_due,
                            &self.runtime,
                            &self.version,
                        )
                        .await;
                    }
                }
            }
        }
    }
}

async fn handle_joined(
    joined: Result<Done, JoinError>,
    prepared: &[Arc<Prepared>],
    in_flight: &mut [bool],
    next_due: &mut [tokio::time::Instant],
    runtime: &Arc<RwLock<Runtime>>,
    version: &watch::Sender<u64>,
) {
    let Ok(done) = joined else {
        return;
    };
    in_flight[done.idx] = false;
    next_due[done.idx] = tokio::time::Instant::now() + prepared[done.idx].meta.interval;
    if let Some(next) = apply_done(runtime, &done).await {
        let _ = version.send(next);
    }
}

async fn apply_done(runtime: &Arc<RwLock<Runtime>>, done: &Done) -> Option<u64> {
    match &done.result {
        Ok(tree) => {
            let mut guard = runtime.write().await;
            let slot = guard.cache.get_mut(&done.id)?;
            slot.tree = Some(tree.clone());
            slot.updated_at = Some(epoch_seconds());
            guard.version += 1;
            Some(guard.version)
        }
        Err(message) => {
            eprintln!(
                "widget {}: {}",
                done.id,
                script::redact(message, &done.secrets)
            );
            None
        }
    }
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

fn collect_secrets(meta: &Meta, resolved: &serde_json::Value) -> Vec<String> {
    meta.config
        .iter()
        .filter(|(_, spec)| spec.secret)
        .filter_map(|(key, _)| {
            resolved
                .get(key)
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker::ContainerState;

    fn sample(id: &str, state: ContainerState, category: Option<&str>) -> ContainerInfo {
        ContainerInfo {
            id: id.to_string(),
            name: format!("/{id}"),
            state,
            detail: String::new(),
            image: "img:1".to_string(),
            description: None,
            url: None,
            icon: None,
            category: category.map(str::to_string),
            started_epoch: None,
            cpu_percent: None,
            mem_usage: None,
            mem_limit: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn containers_script_groups_orders_and_colors() {
        let mut hot = sample("hot", ContainerState::Running, Some("B"));
        hot.cpu_percent = Some(90.0);
        hot.mem_usage = Some(90);
        hot.mem_limit = Some(100);
        let mut warm = sample("warm", ContainerState::Running, Some("A"));
        warm.cpu_percent = Some(65.0);
        let mut bad = sample("bad", ContainerState::Running, None);
        bad.detail = "UNHEALTHY (health: unhealthy)".to_string();
        let child = sample("kid", ContainerState::Running, None);
        hot.children.push(child);
        let dead = sample("dead", ContainerState::Exited, Some("A"));
        let containers = vec![hot, warm, bad, dead];
        let source = std::fs::read_to_string("widgets/Containers.groh")
            .unwrap_or_else(|err| panic!("{err}"));
        let meta =
            script::extract_meta(&source, "Containers.groh").unwrap_or_else(|err| panic!("{err}"));
        let context = RunContext {
            http_agent: script::host::http::build_agent(),
            http_cache: Arc::new(Mutex::new(HashMap::new())),
            containers,
        };
        let tree = script::run_script(
            &source,
            &meta,
            &serde_json::Value::Object(Default::default()),
            &context,
            Duration::from_secs(5),
        )
        .unwrap_or_else(|err| panic!("{err}"));
        let text = serde_json::to_string(&tree).unwrap_or_default();
        assert!(
            text.contains("3 running · 1 exited · 1 unhealthy"),
            "tree: {text}"
        );
        assert!(
            text.contains("\"type\":\"Window\""),
            "missing window: {text}"
        );
        assert!(text.contains("\"type\":\"Box\""), "missing box: {text}");
        assert!(
            text.contains("\"color\":\"light-red\""),
            "expected red bar: {text}"
        );
        assert!(
            text.contains("\"color\":\"light-yellow\""),
            "expected yellow bar: {text}"
        );
        assert!(text.contains("\"value\":\"A\""), "missing group A: {text}");
        assert!(text.contains("\"value\":\"B\""), "missing group B: {text}");
    }

    #[test]
    fn bookmarks_script_builds_favicon_row_with_vertical_separators() {
        let source = std::fs::read_to_string("widgets/Bookmarks.groh")
            .unwrap_or_else(|err| panic!("{err}"));
        let meta =
            script::extract_meta(&source, "Bookmarks.groh").unwrap_or_else(|err| panic!("{err}"));
        let context = RunContext {
            http_agent: script::host::http::build_agent(),
            http_cache: Arc::new(Mutex::new(HashMap::new())),
            containers: Vec::new(),
        };
        let config = serde_json::json!({
            "links": ["Google | https://www.google.com", "Example | https://example.com/path"]
        });
        let tree = script::run_script(
            &source,
            &meta,
            &config,
            &context,
            Duration::from_secs(5),
        )
        .unwrap_or_else(|err| panic!("{err}"));
        let text = serde_json::to_string(&tree).unwrap_or_default();
        assert!(
            text.contains("https://www.google.com/favicon.ico"),
            "tree: {text}"
        );
        assert!(
            text.contains("https://example.com/favicon.ico"),
            "tree: {text}"
        );
        assert!(
            text.contains("\"orientation\":\"vertical\""),
            "missing vertical separator: {text}"
        );
        assert!(text.contains("\"label\":\"Google\""), "tree: {text}");
    }
}
