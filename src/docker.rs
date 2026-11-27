use bollard::Docker;
use bollard::query_parameters::{
    EventsOptions, EventsOptionsBuilder, ListContainersOptionsBuilder, StatsOptionsBuilder,
};
use futures_util::StreamExt;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ContainerState {
    Created,
    Running,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
    Unknown(String),
}

impl ContainerState {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Created => "created",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Restarting => "restarting",
            Self::Removing => "removing",
            Self::Exited => "exited",
            Self::Dead => "dead",
            Self::Unknown(raw) => raw.as_str(),
        }
    }

    pub(crate) fn parse(raw: &str) -> Self {
        match raw.to_ascii_lowercase().as_str() {
            "created" => Self::Created,
            "running" => Self::Running,
            "paused" => Self::Paused,
            "restarting" => Self::Restarting,
            "removing" => Self::Removing,
            "exited" => Self::Exited,
            "dead" => Self::Dead,
            "" => Self::Unknown("unknown".to_string()),
            other => Self::Unknown(other.to_string()),
        }
    }
}

impl fmt::Display for ContainerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ContainerState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct ContainerInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) state: ContainerState,
    pub(crate) detail: String,
    pub(crate) image: String,
    pub(crate) description: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) started_epoch: Option<i64>,
    pub(crate) cpu_percent: Option<f64>,
    pub(crate) mem_usage: Option<u64>,
    pub(crate) mem_limit: Option<u64>,
    pub(crate) children: Vec<ContainerInfo>,
}

struct RawEntry {
    info: ContainerInfo,
    container_name: String,
    groh_id: Option<String>,
    compose_service: Option<String>,
    parent: Option<String>,
    child_entries: Vec<RawEntry>,
}

fn into_info(entry: RawEntry) -> ContainerInfo {
    let mut info = entry.info;
    info.children = entry.child_entries.into_iter().map(into_info).collect();
    info
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ContainerDelta {
    Added(String),
    Removed(String),
}

#[derive(Clone, Copy)]
struct PrevCpuSample {
    cpu_total: u64,
    system_total: u64,
}

pub(crate) async fn watch(
    snapshot_tx: watch::Sender<Vec<ContainerInfo>>,
    delta_tx: mpsc::UnboundedSender<ContainerDelta>,
) -> Result<(), bollard::errors::Error> {
    let docker = Docker::connect_with_socket_defaults()?;
    let mut prev_cpu: HashMap<String, PrevCpuSample> = HashMap::new();
    let mut prev = fetch_all(&docker, &mut prev_cpu).await?;
    let _ = snapshot_tx.send(prev.clone());

    let mut stream = docker.events(Some(events_options()));
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let _ = tick.tick().await;

    loop {
        tokio::select! {
            item = stream.next() => {
                let Some(item) = item else {
                    break;
                };
                let Ok(ev) = item else {
                    continue;
                };
                if !ev.action.as_deref().is_some_and(is_relevant_event) {
                    continue;
                }
                if let Ok(next) = fetch_all(&docker, &mut prev_cpu).await {
                    publish_snapshot(&mut prev, next, &snapshot_tx, &delta_tx);
                }
            }
            _ = tick.tick() => {
                if let Ok(next) = fetch_all(&docker, &mut prev_cpu).await {
                    publish_snapshot(&mut prev, next, &snapshot_tx, &delta_tx);
                }
            }
        }
    }

    Ok(())
}

fn events_options() -> EventsOptions {
    let mut filters = HashMap::new();
    filters.insert("type", vec!["container"]);
    EventsOptionsBuilder::default().filters(&filters).build()
}

fn is_relevant_event(action: &str) -> bool {
    matches!(
        action,
        "create" | "destroy" | "start" | "stop" | "die" | "rename" | "pause" | "unpause"
    )
}

fn publish_snapshot(
    prev: &mut Vec<ContainerInfo>,
    next: Vec<ContainerInfo>,
    snapshot_tx: &watch::Sender<Vec<ContainerInfo>>,
    delta_tx: &mpsc::UnboundedSender<ContainerDelta>,
) {
    for delta in compute_delta(prev, &next) {
        let _ = delta_tx.send(delta);
    }
    *prev = next.clone();
    snapshot_tx.send_if_modified(|current| {
        if *current == next {
            false
        } else {
            *current = next;
            true
        }
    });
}

pub(crate) fn compute_delta(prev: &[ContainerInfo], next: &[ContainerInfo]) -> Vec<ContainerDelta> {
    let prev_names: BTreeSet<&str> = prev.iter().map(|c| c.name.as_str()).collect();
    let next_names: BTreeSet<&str> = next.iter().map(|c| c.name.as_str()).collect();

    let added = next_names
        .difference(&prev_names)
        .map(|name| ContainerDelta::Added((*name).to_string()));
    let removed = prev_names
        .difference(&next_names)
        .map(|name| ContainerDelta::Removed((*name).to_string()));

    added.chain(removed).collect()
}

async fn fetch_all(
    docker: &Docker,
    prev_cpu: &mut HashMap<String, PrevCpuSample>,
) -> Result<Vec<ContainerInfo>, bollard::errors::Error> {
    let opts = ListContainersOptionsBuilder::default().all(true).build();
    let list = docker.list_containers(Some(opts)).await?;
    let mut infos = parse_and_group(list);
    enrich(docker, &mut infos, prev_cpu).await;
    Ok(infos)
}
struct Fetched {
    started_epoch: Option<i64>,
    mem_usage: Option<u64>,
    mem_limit: Option<u64>,
    cpu: Option<(u64, u64, u32)>,
}

fn is_running(info: &ContainerInfo) -> bool {
    matches!(info.state, ContainerState::Running | ContainerState::Paused) && !info.id.is_empty()
}

fn collect_running(infos: &[ContainerInfo], ids: &mut Vec<String>) {
    for info in infos {
        if is_running(info) {
            ids.push(info.id.clone());
        }
        collect_running(&info.children, ids);
    }
}

async fn fetch_one(docker: &Docker, id: String) -> (String, Fetched) {
    let mut fetched = Fetched {
        started_epoch: None,
        mem_usage: None,
        mem_limit: None,
        cpu: None,
    };
    if let Ok(inspect) = docker.inspect_container(&id, None).await
        && let Some(started) = inspect.state.and_then(|state| state.started_at)
        && !started.is_empty()
        && started != "0001-01-01T00:00:00Z"
    {
        fetched.started_epoch = iso_to_epoch(&started);
    }
    let stats_opts = StatsOptionsBuilder::default()
        .stream(false)
        .one_shot(true)
        .build();
    if let Some(Ok(sample)) = docker.stats(&id, Some(stats_opts)).next().await {
        if let Some(memory) = sample.memory_stats {
            fetched.mem_usage = memory.usage;
            fetched.mem_limit = memory.limit.filter(|limit| *limit > 0);
        }
        if let Some(cpu) = sample.cpu_stats {
            let total = cpu.cpu_usage.and_then(|usage| usage.total_usage);
            let system = cpu.system_cpu_usage;
            if let (Some(total), Some(system)) = (total, system) {
                fetched.cpu = Some((total, system, cpu.online_cpus.unwrap_or(1).max(1)));
            }
        }
    }
    (id, fetched)
}

fn apply_fetched(
    info: &mut ContainerInfo,
    fetched: &HashMap<String, Fetched>,
    prev_cpu: &mut HashMap<String, PrevCpuSample>,
) {
    if let Some(fetched) = fetched.get(&info.id) {
        info.started_epoch = fetched.started_epoch;
        info.mem_usage = fetched.mem_usage;
        info.mem_limit = fetched.mem_limit;
        if let Some((total, system, online)) = fetched.cpu {
            if let Some(prev_sample) = prev_cpu.get(&info.id)
                && system > prev_sample.system_total
                && total >= prev_sample.cpu_total
            {
                let cpu_delta = (total - prev_sample.cpu_total) as f64;
                let system_delta = (system - prev_sample.system_total) as f64;
                if system_delta > 0.0 {
                    let percent = cpu_delta / system_delta * f64::from(online) * 100.0;
                    if percent.is_finite() {
                        info.cpu_percent = Some(percent);
                    }
                }
            }
            prev_cpu.insert(
                info.id.clone(),
                PrevCpuSample {
                    cpu_total: total,
                    system_total: system,
                },
            );
        }
    }
    for child in &mut info.children {
        apply_fetched(child, fetched, prev_cpu);
    }
}

async fn enrich(
    docker: &Docker,
    infos: &mut [ContainerInfo],
    prev_cpu: &mut HashMap<String, PrevCpuSample>,
) {
    let mut ids = Vec::new();
    collect_running(infos, &mut ids);
    let fetched: HashMap<String, Fetched> = futures_util::stream::iter(ids)
        .map(|id| fetch_one(docker, id))
        .buffer_unordered(8)
        .collect()
        .await;
    prev_cpu.retain(|id, _| fetched.contains_key(id));
    for info in infos.iter_mut() {
        apply_fetched(info, &fetched, prev_cpu);
    }
}

struct TreeIndex {
    by_id: HashMap<String, Vec<usize>>,
    by_name: HashMap<String, Vec<usize>>,
    by_service: HashMap<String, Vec<usize>>,
}

fn index_tree(entries: &[RawEntry], prefix: &[usize], index: &mut TreeIndex) {
    for (idx, entry) in entries.iter().enumerate() {
        let mut path = prefix.to_vec();
        path.push(idx);
        if let Some(groh_id) = &entry.groh_id {
            index
                .by_id
                .entry(groh_id.clone())
                .or_insert_with(|| path.clone());
        }
        index
            .by_name
            .entry(entry.container_name.to_lowercase())
            .or_insert_with(|| path.clone());
        if let Some(service) = &entry.compose_service {
            index
                .by_service
                .entry(service.clone())
                .or_insert_with(|| path.clone());
        }
        index_tree(&entry.child_entries, &path, index);
    }
}

fn tree_index(entries: &[RawEntry]) -> TreeIndex {
    let mut index = TreeIndex {
        by_id: HashMap::new(),
        by_name: HashMap::new(),
        by_service: HashMap::new(),
    };
    index_tree(entries, &[], &mut index);
    index
}

fn find_path(index: &TreeIndex, target: &str) -> Option<Vec<usize>> {
    index
        .by_id
        .get(target)
        .or_else(|| index.by_name.get(&target.to_lowercase()))
        .or_else(|| index.by_service.get(target))
        .cloned()
}

fn attach_at(entries: &mut [RawEntry], path: &[usize], child: RawEntry) {
    let mut node = &mut entries[path[0]];
    for &idx in &path[1..] {
        node = &mut node.child_entries[idx];
    }
    node.child_entries.push(child);
}

fn parse_and_group(list: Vec<bollard::models::ContainerSummary>) -> Vec<ContainerInfo> {
    let mut entries: Vec<RawEntry> = list.into_iter().filter_map(parse_entry).collect();
    entries.sort_by(|a, b| a.info.name.cmp(&b.info.name));

    let mut top_level: Vec<RawEntry> = Vec::with_capacity(entries.len());
    let mut pending_children: Vec<RawEntry> = Vec::new();

    for entry in entries {
        if entry.parent.is_some() {
            pending_children.push(entry);
        } else {
            top_level.push(entry);
        }
    }

    let mut pending = pending_children;
    loop {
        if pending.is_empty() {
            break;
        }
        let top_index = tree_index(&top_level);
        let mut remaining: Vec<RawEntry> = Vec::new();
        let mut progress = false;
        for child in pending {
            let target = child.parent.clone();
            let top_path = target
                .as_deref()
                .and_then(|target| find_path(&top_index, target));
            if let Some(path) = top_path {
                attach_at(&mut top_level, &path, child);
                progress = true;
                continue;
            }
            let pending_index = tree_index(&remaining);
            let pending_path = target
                .as_deref()
                .and_then(|target| find_path(&pending_index, target));
            if let Some(path) = pending_path {
                attach_at(&mut remaining, &path, child);
                progress = true;
                continue;
            }
            remaining.push(child);
        }
        if !progress {
            top_level.extend(remaining);
            break;
        }
        pending = remaining;
    }

    top_level.into_iter().map(into_info).collect()
}

fn parse_entry(c: bollard::models::ContainerSummary) -> Option<RawEntry> {
    let labels = c.labels.unwrap_or_default();
    let has_groh_label = labels.keys().any(|k| k.starts_with("groh."));
    if !has_groh_label {
        return None;
    }
    if labels.get("groh.hide").map(String::as_str) == Some("true") {
        return None;
    }

    let id = c.id.unwrap_or_default();
    let container_name = c
        .names
        .and_then(|mut n| n.drain(..).next())
        .unwrap_or_else(|| id.chars().take(12).collect())
        .trim_start_matches('/')
        .to_string();
    let name = labels
        .get("groh.name")
        .cloned()
        .unwrap_or_else(|| container_name.clone());
    let state = c
        .state
        .map(|s| ContainerState::parse(s.as_ref()))
        .unwrap_or(ContainerState::Unknown("unknown".to_string()));
    let detail = c.status.unwrap_or_default();
    let image = c.image.unwrap_or_default();

    let description = labels.get("groh.description").cloned();
    let url = labels.get("groh.url").cloned();
    let icon = labels.get("groh.icon").cloned();
    let category = labels.get("groh.category").cloned();
    let groh_id = labels.get("groh.id").cloned();
    let compose_service = labels.get("com.docker.compose.service").cloned();
    let parent = labels.get("groh.parent").cloned();

    Some(RawEntry {
        info: ContainerInfo {
            id,
            name,
            state,
            detail,
            image,
            description,
            url,
            icon,
            category,
            started_epoch: None,
            cpu_percent: None,
            mem_usage: None,
            mem_limit: None,
            children: Vec::new(),
        },
        container_name,
        groh_id,
        compose_service,
        parent,
        child_entries: Vec::new(),
    })
}

fn iso_to_epoch(text: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(text)
        .map(|parsed| parsed.timestamp())
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::models::ContainerSummary;

    fn make_container(name: &str, labels: &[(&str, &str)]) -> ContainerSummary {
        let mut map = HashMap::new();
        for (k, v) in labels {
            map.insert((*k).to_string(), (*v).to_string());
        }
        ContainerSummary {
            id: Some(format!("id-{name}")),
            names: Some(vec![format!("/{name}")]),
            labels: Some(map),
            ..Default::default()
        }
    }

    #[test]
    fn filters_opt_in_and_hide() {
        let list = vec![
            make_container("plain", &[]),
            make_container("hidden", &[("groh.name", "Hidden"), ("groh.hide", "true")]),
            make_container("shown", &[("groh.url", "https://example.com")]),
        ];
        let result = parse_and_group(list);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "shown");
        assert_eq!(result[0].url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn attaches_children_by_id_name_and_compose_service() {
        let list = vec![
            make_container(
                "app-main",
                &[
                    ("groh.id", "custom-app"),
                    ("groh.name", "App"),
                    ("com.docker.compose.service", "web"),
                ],
            ),
            make_container("db", &[("groh.name", "DB"), ("groh.parent", "custom-app")]),
            make_container("leaf", &[("groh.name", "Leaf"), ("groh.parent", "db")]),
            make_container(
                "worker",
                &[("groh.name", "Worker"), ("groh.parent", "APP-MAIN")],
            ),
            make_container("redis", &[("groh.name", "Redis"), ("groh.parent", "web")]),
            make_container(
                "orphan",
                &[("groh.name", "Orphan"), ("groh.parent", "missing")],
            ),
        ];
        let result = parse_and_group(list);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "App");
        assert_eq!(result[0].children.len(), 3);
        let db = result[0]
            .children
            .iter()
            .find(|child| child.name == "DB")
            .unwrap_or_else(|| panic!("DB child"));
        assert_eq!(db.children.len(), 1);
        assert_eq!(db.children[0].name, "Leaf");
        assert_eq!(result[1].name, "Orphan");
    }
}
