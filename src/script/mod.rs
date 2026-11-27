pub(crate) mod host;

use crate::docker::ContainerInfo;
use crate::script::host::HostContext;
use crate::view::Size;
use rhai::{Engine, OptimizationLevel, Scope};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ConfigKind {
    String,
    Number,
    Bool,
    List,
}

impl fmt::Display for ConfigKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Bool => "bool",
            Self::List => "list",
        };
        f.write_str(label)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub(crate) enum Need {
    #[serde(rename = "net:http")]
    NetHttp,
    #[serde(rename = "docker:containers")]
    DockerContainers,
    #[serde(rename = "time:now")]
    TimeNow,
}

#[derive(Clone, Debug)]
pub(crate) struct ConfigKey {
    pub(crate) kind: ConfigKind,
    pub(crate) required: bool,
    pub(crate) secret: bool,
    pub(crate) fallback: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub(crate) struct Meta {
    pub(crate) title: String,
    pub(crate) interval: Duration,
    pub(crate) size: Size,
    pub(crate) needs: Vec<Need>,
    pub(crate) hosts: Vec<String>,
    pub(crate) config: BTreeMap<String, ConfigKey>,
}

#[derive(Clone, Debug)]
pub(crate) struct RunContext {
    pub(crate) http_agent: ureq::Agent,
    pub(crate) http_cache: Arc<Mutex<HashMap<String, (Instant, String)>>>,
    pub(crate) containers: Vec<ContainerInfo>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMeta {
    title: String,
    interval: String,
    size: Size,
    #[serde(default)]
    needs: Vec<Need>,
    #[serde(default)]
    hosts: Vec<String>,
    #[serde(default)]
    config: BTreeMap<String, RawConfigKey>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfigKey {
    #[serde(rename = "type")]
    kind: ConfigKind,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    secret: bool,
    #[serde(default)]
    fallback: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    help: Option<String>,
}

pub(crate) fn extract_meta(source: &str, label: &str) -> Result<Meta, String> {
    let mut bare = Engine::new();
    bare.set_optimization_level(OptimizationLevel::Simple);
    let ast = bare
        .compile(source)
        .map_err(|err| format!("{label}: {err}"))?;
    let meta_dynamic = ast
        .iter_literal_variables(true, false)
        .find(|(name, _, _)| *name == "META")
        .map(|(_, _, value)| value)
        .ok_or_else(|| format!("{label}: no const META found"))?;
    let raw: RawMeta = rhai::serde::from_dynamic(&meta_dynamic)
        .map_err(|err| format!("{label}: invalid META: {err}"))?;
    build_meta(raw, label)
}

fn build_meta(raw: RawMeta, label: &str) -> Result<Meta, String> {
    if raw.title.trim().is_empty() {
        return Err(format!("{label}: META title cannot be empty"));
    }
    let interval = parse_interval(&raw.interval).map_err(|err| format!("{label}: {err}"))?;
    let mut config = BTreeMap::new();
    for (key, entry) in raw.config {
        let fallback = entry.fallback.filter(|v| !v.is_null());
        if entry.required && fallback.is_some() {
            return Err(format!(
                "{label}: config key {key}: required and fallback cannot combine"
            ));
        }
        if let Some(fb) = &fallback {
            validate_fallback(label, &key, entry.kind, fb)?;
        }
        config.insert(
            key,
            ConfigKey {
                kind: entry.kind,
                required: entry.required,
                secret: entry.secret,
                fallback,
            },
        );
    }
    Ok(Meta {
        title: raw.title,
        interval,
        size: raw.size,
        needs: raw.needs,
        hosts: raw.hosts,
        config,
    })
}

fn validate_fallback(
    label: &str,
    key: &str,
    kind: ConfigKind,
    value: &serde_json::Value,
) -> Result<(), String> {
    let ok = match kind {
        ConfigKind::String => value.is_string(),
        ConfigKind::Number => value.is_number(),
        ConfigKind::Bool => value.is_boolean(),
        ConfigKind::List => value
            .as_array()
            .is_some_and(|items| items.iter().all(serde_json::Value::is_string)),
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{label}: config key {key}: fallback does not match type {kind}"
        ))
    }
}

pub(crate) fn parse_interval(text: &str) -> Result<Duration, String> {
    let trimmed = text.trim();
    let (digits, multiplier) = if let Some(hours) = trimmed.strip_suffix('h') {
        (hours, 3600)
    } else if let Some(minutes) = trimmed.strip_suffix('m') {
        (minutes, 60)
    } else if let Some(seconds) = trimmed.strip_suffix('s') {
        (seconds, 1)
    } else {
        return Err(format!("bad interval '{text}': expected suffix s, m, or h"));
    };
    let count = digits
        .parse::<u64>()
        .map_err(|_| format!("bad interval '{text}'"))?;
    let seconds = count
        .checked_mul(multiplier)
        .filter(|&s| s > 0)
        .ok_or_else(|| format!("bad interval '{text}'"))?;
    Ok(Duration::from_secs(seconds))
}

pub(crate) fn run_script(
    source: &str,
    meta: &Meta,
    config: &serde_json::Value,
    context: &RunContext,
    timeout: Duration,
) -> Result<crate::view::Node, String> {
    let deadline = Instant::now() + timeout;
    let engine = build_engine(meta, context, deadline);
    let mut scope = Scope::new();
    let config_dynamic = rhai::serde::to_dynamic(config)
        .map_err(|err| format!("config serialization failed: {err}"))?;
    scope.push("config", config_dynamic);
    let ast = engine
        .compile_with_scope(&scope, source)
        .map_err(|err| err.to_string())?;
    let result = engine
        .eval_ast_with_scope::<rhai::Dynamic>(&mut scope, &ast)
        .map_err(|err| err.to_string())?;
    let tree = crate::view::from_dynamic(result)?;
    crate::view::validate(&tree)?;
    Ok(tree)
}

fn build_engine(meta: &Meta, context: &RunContext, deadline: Instant) -> Engine {
    let mut engine = Engine::new();
    engine.set_optimization_level(OptimizationLevel::Simple);
    engine.set_max_operations(1_000_000);
    engine.set_max_string_size(1_000_000);
    engine.set_max_array_size(10_000);
    engine.set_max_map_size(10_000);
    engine.set_max_variables(10_000);
    engine.on_progress(move |_ops| {
        if Instant::now() > deadline {
            Some("script deadline exceeded".into())
        } else {
            None
        }
    });
    let host_context = HostContext {
        hosts: meta.hosts.clone(),
        deadline,
        http_agent: context.http_agent.clone(),
        http_cache: Arc::clone(&context.http_cache),
        containers: context.containers.clone(),
    };
    crate::view::register(&mut engine);
    for need in &meta.needs {
        match need {
            Need::NetHttp => {
                engine.register_static_module("http", host::http::module(host_context.clone()));
            }
            Need::DockerContainers => {
                engine.register_static_module("docker", host::docker::module(host_context.clone()));
            }
            Need::TimeNow => {
                engine.register_static_module("time", host::time::module());
            }
        }
    }
    engine
}

pub(crate) fn redact(message: &str, secrets: &[String]) -> String {
    let mut redacted = message.to_string();
    for secret in secrets {
        if !secret.is_empty() {
            redacted = redacted.replace(secret.as_str(), "***");
        }
    }
    redacted
}
