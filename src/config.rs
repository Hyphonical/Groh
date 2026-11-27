use crate::script::{ConfigKind, Meta};
use crate::view::Size;
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsStr;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use toml::Value as TomlValue;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    AsIs(PathBuf),
    WidgetRelative(PathBuf),
}

impl Source {
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        if raw.trim().is_empty() {
            return Err("widget source cannot be empty".to_string());
        }
        let path = PathBuf::from(raw);
        if path.is_absolute()
            || path.components().next() == Some(Component::Normal(OsStr::new("widgets")))
        {
            Ok(Self::AsIs(path))
        } else {
            Ok(Self::WidgetRelative(path))
        }
    }

    pub(crate) fn resolve(&self, widgets_dir: &Path) -> PathBuf {
        match self {
            Self::AsIs(path) => path.clone(),
            Self::WidgetRelative(path) => widgets_dir.join(path),
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AsIs(path) | Self::WidgetRelative(path) => write!(f, "{}", path.display()),
        }
    }
}

impl<'de> Deserialize<'de> for Source {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Placement {
    pub(crate) id: String,
    pub(crate) source: Source,
    pub(crate) size: Option<Size>,
    pub(crate) show_summary: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct LoadedConfig {
    pub(crate) columns: Vec<Vec<Placement>>,
    pub(crate) widget_configs: BTreeMap<String, BTreeMap<String, TomlValue>>,
}

impl LoadedConfig {
    pub(crate) fn placements(&self) -> impl Iterator<Item = &Placement> {
        self.columns.iter().flatten()
    }

    pub(crate) fn widget_config(&self, id: &str) -> Option<&BTreeMap<String, TomlValue>> {
        self.widget_configs.get(id)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfigFile {
    page: RawPage,
    #[serde(default)]
    widgets: BTreeMap<String, RawWidgetEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPage {
    columns: Vec<RawColumn>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawColumn {
    #[serde(default)]
    widgets: Vec<RawPlacement>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlacement {
    id: String,
    source: Source,
    size: Option<Size>,
    #[serde(default = "default_show_summary")]
    show_summary: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWidgetEntry {
    #[serde(default)]
    config: BTreeMap<String, TomlValue>,
}

fn default_show_summary() -> bool {
    true
}

pub(crate) fn load(path: &Path) -> Result<LoadedConfig, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    let raw: RawConfigFile =
        toml::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))?;

    let mut columns = Vec::with_capacity(raw.page.columns.len());
    let mut ids = HashSet::new();
    for column in raw.page.columns {
        let mut placements = Vec::with_capacity(column.widgets.len());
        for entry in column.widgets {
            if entry.id.trim().is_empty() {
                return Err(format!("{}: widget entry has empty id", path.display()));
            }
            if !ids.insert(entry.id.clone()) {
                return Err(format!(
                    "{}: duplicate widget id {}",
                    path.display(),
                    entry.id
                ));
            }
            placements.push(Placement {
                id: entry.id,
                source: entry.source,
                size: entry.size,
                show_summary: entry.show_summary,
            });
        }
        columns.push(placements);
    }

    let mut widget_configs = BTreeMap::new();
    for (id, entry) in raw.widgets {
        if !ids.contains(&id) {
            return Err(format!(
                "{}: orphan config table [widgets.{id}.config]",
                path.display()
            ));
        }
        widget_configs.insert(id, entry.config);
    }

    Ok(LoadedConfig {
        columns,
        widget_configs,
    })
}

pub(crate) fn validate_instance(
    id: &str,
    meta: &Meta,
    values: Option<&BTreeMap<String, TomlValue>>,
) -> Result<serde_json::Value, String> {
    let empty = BTreeMap::new();
    let values = values.unwrap_or(&empty);

    for key in values.keys() {
        if !meta.config.contains_key(key) {
            return Err(format!("widget {id}: unknown config key {key}"));
        }
    }

    let mut resolved = serde_json::Map::new();
    for (key, spec) in &meta.config {
        let provided = values.get(key);
        if provided.is_none() && spec.required {
            return Err(format!("widget {id}: missing required config key {key}"));
        }
        if let Some(value) = provided {
            check_type(id, key, spec.kind, value)?;
            let json = serde_json::to_value(value)
                .map_err(|err| format!("widget {id}: config key {key}: {err}"))?;
            resolved.insert(key.clone(), json);
        } else if let Some(fallback) = &spec.fallback {
            resolved.insert(key.clone(), fallback.clone());
        }
    }
    Ok(serde_json::Value::Object(resolved))
}

fn check_type(id: &str, key: &str, kind: ConfigKind, value: &TomlValue) -> Result<(), String> {
    let ok = match kind {
        ConfigKind::String => value.is_str(),
        ConfigKind::Number => value.as_float().is_some() || value.as_integer().is_some(),
        ConfigKind::Bool => value.is_bool(),
        ConfigKind::List => value
            .as_array()
            .is_some_and(|items| items.iter().all(TomlValue::is_str)),
    };
    if ok {
        Ok(())
    } else {
        Err(format!("widget {id}: config key {key} should be a {kind}"))
    }
}
