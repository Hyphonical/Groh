use bollard::Docker;
use bollard::container::ListContainersOptions;
use bollard::system::EventsOptions;
use futures_util::StreamExt;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tokio::sync::{mpsc, watch};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ContainerInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) state: String,
    pub(crate) detail: String,
    pub(crate) image: String,
}

#[derive(Clone, Debug)]
pub(crate) enum ContainerDelta {
    Added(String),
    Removed(String),
}

pub(crate) async fn watch(
    snapshot_tx: watch::Sender<Vec<ContainerInfo>>,
    delta_tx: mpsc::UnboundedSender<ContainerDelta>,
) -> Result<(), bollard::errors::Error> {
    let docker = Docker::connect_with_socket_defaults()?;
    let mut prev = fetch_all(&docker).await?;
    let _ = snapshot_tx.send(prev.clone());
    let mut filters = HashMap::new();
    filters.insert("type", vec!["container"]);
    let opts = EventsOptions {
        filters,
        ..Default::default()
    };
    let mut stream = docker.events(Some(opts));

    while let Some(item) = stream.next().await {
        let Ok(ev) = item else {
            continue;
        };
        let action = ev.action.as_deref().unwrap_or("");
        if !matches!(
            action,
            "create" | "destroy" | "start" | "stop" | "die" | "rename" | "pause" | "unpause"
        ) {
            continue;
        }

        let Ok(next) = fetch_all(&docker).await else {
            continue;
        };

        let prev_names: HashSet<&str> = prev.iter().map(|c| c.name.as_str()).collect();
        let next_names: HashSet<&str> = next.iter().map(|c| c.name.as_str()).collect();

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

    Ok(())
}

async fn fetch_all(docker: &Docker) -> Result<Vec<ContainerInfo>, bollard::errors::Error> {
    let opts = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };
    let list = docker.list_containers(Some(opts)).await?;
    let mut containers: Vec<ContainerInfo> = list
        .into_iter()
        .map(|c| {
            let id = c.id.unwrap_or_default();
            let name = c
                .names
                .and_then(|mut n| n.drain(..).next())
                .unwrap_or_else(|| id.chars().take(12).collect())
                .trim_start_matches('/')
                .to_string();
            let state = c
                .state
                .map(|s| s.to_string().to_lowercase())
                .unwrap_or_else(|| "unknown".to_string());
            let detail = c.status.unwrap_or_default();
            let image = c.image.unwrap_or_default();
            ContainerInfo {
                id,
                name,
                state,
                detail,
                image,
            }
        })
        .collect();
    containers.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(containers)
}
