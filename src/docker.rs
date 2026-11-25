use bollard::Docker;
use bollard::query_parameters::{
    EventsOptionsBuilder, ListContainersOptionsBuilder, StatsOptionsBuilder,
};
use futures_util::StreamExt;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct ContainerInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) state: String,
    pub(crate) detail: String,
    pub(crate) image: String,
    pub(crate) description: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) category: Option<String>,
    pub(crate) started_at: Option<String>,
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
}

#[derive(Clone, Debug)]
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
    let mut filters = HashMap::new();
    filters.insert("type", vec!["container"]);
    let opts = EventsOptionsBuilder::default()
        .filters(&filters)
        .build();
    let mut stream = docker.events(Some(opts));
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    // Skip the immediate tick so the loop waits for the first interval.
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
                let action = ev.action.as_deref().unwrap_or("");
                if !matches!(
                    action,
                    "create" | "destroy" | "start" | "stop" | "die" | "rename" | "pause"
                        | "unpause"
                ) {
                    continue;
                }
                let Ok(next) = fetch_all(&docker, &mut prev_cpu).await else {
                    continue;
                };
                let prev_names: HashSet<&str> =
                    prev.iter().map(|c| c.name.as_str()).collect();
                let next_names: HashSet<&str> =
                    next.iter().map(|c| c.name.as_str()).collect();
                for name in &next_names {
                    if !prev_names.contains(name) {
                        let _ = delta_tx.send(ContainerDelta::Added((*name).to_string()));
                    }
                }
                for name in &prev_names {
                    if !next_names.contains(name) {
                        let _ = delta_tx.send(ContainerDelta::Removed((*name).to_string()));
                    }
                }
                let _ = snapshot_tx.send(next.clone());
                prev = next;
            }
            _ = tick.tick() => {
                let Ok(next) = fetch_all(&docker, &mut prev_cpu).await else {
                    continue;
                };
                let _ = snapshot_tx.send(next.clone());
                prev = next;
            }
        }
    }

    Ok(())
}

async fn fetch_all(
    docker: &Docker,
    prev_cpu: &mut HashMap<String, PrevCpuSample>,
) -> Result<Vec<ContainerInfo>, bollard::errors::Error> {
    let opts = ListContainersOptionsBuilder::default()
        .all(true)
        .build();
    let list = docker.list_containers(Some(opts)).await?;
    let mut infos = parse_and_group(list);
    enrich(docker, &mut infos, prev_cpu).await;
    Ok(infos)
}

async fn enrich(
    docker: &Docker,
    infos: &mut [ContainerInfo],
    prev_cpu: &mut HashMap<String, PrevCpuSample>,
) {
    for info in infos.iter_mut() {
        let is_running = matches!(info.state.as_str(), "running" | "paused")
            && !info.id.is_empty();
        if is_running {
            if let Ok(inspect) = docker.inspect_container(&info.id, None).await
                && let Some(started) = inspect.state.and_then(|s| s.started_at)
                && !started.is_empty()
                && started != "0001-01-01T00:00:00Z"
            {
                info.started_at = Some(started);
            }
            let stats_opts = StatsOptionsBuilder::default()
                .stream(false)
                .one_shot(true)
                .build();
            if let Some(Ok(sample)) = docker.stats(&info.id, Some(stats_opts)).next().await {
                if let Some(memory) = sample.memory_stats {
                    info.mem_usage = memory.usage;
                    info.mem_limit = memory.limit.filter(|limit| *limit > 0);
                }
                if let Some(cpu) = sample.cpu_stats {
                    let total = cpu.cpu_usage.and_then(|usage| usage.total_usage);
                    let system = cpu.system_cpu_usage;
                    if let (Some(total), Some(system)) = (total, system) {
                        let online = cpu.online_cpus.unwrap_or(1).max(1);
                        if let Some(prev_sample) = prev_cpu.get(&info.id)
                            && system > prev_sample.system_total
                            && total >= prev_sample.cpu_total
                        {
                            let cpu_delta = (total - prev_sample.cpu_total) as f64;
                            let system_delta = (system - prev_sample.system_total) as f64;
                            if system_delta > 0.0 {
                                info.cpu_percent =
                                    Some(cpu_delta / system_delta * f64::from(online) * 100.0);
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
            }
        }
        Box::pin(enrich(docker, &mut info.children, prev_cpu)).await;
    }
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

    for child in pending_children {
        let Some(target) = child.parent.as_deref() else {
            continue;
        };
        if let Some(parent_idx) = find_parent_index(&top_level, target) {
            top_level[parent_idx].info.children.push(child.info);
        } else {
            top_level.push(child);
        }
    }

    top_level.into_iter().map(|e| e.info).collect()
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
        .map(|s| s.to_string().to_lowercase())
        .unwrap_or_else(|| "unknown".to_string());
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
            started_at: None,
            cpu_percent: None,
            mem_usage: None,
            mem_limit: None,
            children: Vec::new(),
        },
        container_name,
        groh_id,
        compose_service,
        parent,
    })
}

fn find_parent_index(parents: &[RawEntry], target: &str) -> Option<usize> {
    if let Some(idx) = parents
        .iter()
        .position(|p| p.groh_id.as_deref() == Some(target))
    {
        return Some(idx);
    }
    if let Some(idx) = parents
        .iter()
        .position(|p| p.container_name.eq_ignore_ascii_case(target))
    {
        return Some(idx);
    }
    parents
        .iter()
        .position(|p| p.compose_service.as_deref() == Some(target))
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
            make_container(
                "db",
                &[("groh.name", "DB"), ("groh.parent", "custom-app")],
            ),
            make_container(
                "worker",
                &[("groh.name", "Worker"), ("groh.parent", "APP-MAIN")],
            ),
            make_container(
                "redis",
                &[("groh.name", "Redis"), ("groh.parent", "web")],
            ),
            make_container(
                "orphan",
                &[("groh.name", "Orphan"), ("groh.parent", "missing")],
            ),
        ];
        let result = parse_and_group(list);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "App");
        assert_eq!(result[0].children.len(), 3);
        assert_eq!(result[1].name, "Orphan");
    }
}
