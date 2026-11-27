use rhai::{Dynamic, Engine};
use serde::{Deserialize, Serialize};

pub(crate) const MAX_NODES: usize = 2000;
pub(crate) const MAX_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Color {
    #[default]
    Foreground,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    LightBlack,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    LightWhite,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Size {
    Small,
    #[default]
    Normal,
    Large,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tone {
    Running,
    Warn,
    Stopped,
    #[default]
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SeparatorKind {
    #[default]
    Dotted,
    Dashed,
    Solid,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct TextOptions {
    #[serde(default)]
    pub(crate) size: Size,
    #[serde(default)]
    pub(crate) color: Color,
    #[serde(default)]
    pub(crate) bold: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ContainerOptions {
    #[serde(default = "default_gap")]
    pub(crate) gap: u8,
}

impl Default for ContainerOptions {
    fn default() -> Self {
        Self { gap: default_gap() }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct BarOptions {
    #[serde(default)]
    pub(crate) color: Color,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct SeparatorOptions {
    #[serde(default)]
    pub(crate) kind: SeparatorKind,
    #[serde(default)]
    pub(crate) orientation: Orientation,
}

fn default_gap() -> u8 {
    8
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub(crate) enum Node {
    Text {
        value: String,
        size: Size,
        color: Color,
        bold: bool,
    },
    Box {
        children: Vec<Node>,
    },
    Row {
        gap: u8,
        children: Vec<Node>,
    },
    Column {
        gap: u8,
        children: Vec<Node>,
    },
    Separator {
        kind: SeparatorKind,
        orientation: Orientation,
    },
    Bar {
        percent: f64,
        color: Color,
    },
    StatusDot {
        tone: Tone,
    },
    Link {
        href: String,
        label: String,
    },
    Image {
        src: String,
        alt: String,
    },
    Icon {
        src: String,
        alt: String,
    },
    Window {
        title: String,
        children: Vec<Node>,
    },
}

impl Node {
    fn children(&self) -> &[Node] {
        match self {
            Node::Box { children }
            | Node::Window { children, .. }
            | Node::Row { children, .. }
            | Node::Column { children, .. } => children,
            Node::Text { .. }
            | Node::Separator { .. }
            | Node::Bar { .. }
            | Node::StatusDot { .. }
            | Node::Link { .. }
            | Node::Image { .. }
            | Node::Icon { .. } => &[],
        }
    }
}

pub(crate) fn validate(node: &Node) -> Result<(), String> {
    let mut stack = vec![(node, 1usize)];
    let mut count = 0usize;
    while let Some((current, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err(format!("tree exceeds max depth of {MAX_DEPTH}"));
        }
        count += 1;
        if count > MAX_NODES {
            return Err(format!("tree has more than {MAX_NODES} nodes"));
        }
        for child in current.children() {
            stack.push((child, depth + 1));
        }
    }
    Ok(())
}

pub(crate) fn native_error(message: String) -> Box<rhai::EvalAltResult> {
    Box::new(rhai::EvalAltResult::ErrorRuntime(
        message.into(),
        rhai::Position::NONE,
    ))
}

fn nodes_of(array: rhai::Array) -> Result<Vec<Node>, Box<rhai::EvalAltResult>> {
    array
        .into_iter()
        .map(|item| {
            let type_name = item.type_name().to_string();
            item.try_cast::<Node>()
                .ok_or_else(|| native_error(format!("expected Node, got {type_name}")))
        })
        .collect()
}

fn text_of(value: &Dynamic) -> Result<String, Box<rhai::EvalAltResult>> {
    if value.is_unit() {
        return Ok(String::new());
    }
    if value.is_string() {
        return value
            .clone()
            .into_string()
            .map_err(|err| native_error(err.to_string()));
    }
    if let Ok(int) = value.as_int() {
        return Ok(int.to_string());
    }
    if let Ok(float) = value.as_float() {
        return Ok(float.to_string());
    }
    if let Ok(flag) = value.as_bool() {
        return Ok(flag.to_string());
    }
    Err(native_error(format!(
        "expected text, got {}",
        value.type_name()
    )))
}

fn percent_of(value: &Dynamic) -> Result<f64, Box<rhai::EvalAltResult>> {
    let raw = if let Ok(float) = value.as_float() {
        float
    } else if let Ok(int) = value.as_int() {
        int as f64
    } else if value.is_unit() {
        0.0
    } else {
        return Err(native_error(format!(
            "expected number, got {}",
            value.type_name()
        )));
    };
    if !raw.is_finite() {
        return Ok(0.0);
    }
    Ok(raw.clamp(0.0, 100.0))
}

fn options_of<T>(value: &Dynamic) -> Result<T, Box<rhai::EvalAltResult>>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if value.is_unit() {
        return Ok(T::default());
    }
    rhai::serde::from_dynamic(value).map_err(|err| native_error(err.to_string()))
}

fn text_node(value: &Dynamic, options: &Dynamic) -> Result<Node, Box<rhai::EvalAltResult>> {
    let options: TextOptions = options_of(options)?;
    Ok(Node::Text {
        value: text_of(value)?,
        size: options.size,
        color: options.color,
        bold: options.bold,
    })
}

pub(crate) fn register(engine: &mut Engine) {
    engine.register_type_with_name::<Node>("Node");

    engine.register_fn("text", |value: Dynamic| text_node(&value, &Dynamic::UNIT));
    engine.register_fn("text", |value: Dynamic, options: Dynamic| {
        text_node(&value, &options)
    });
    engine.register_fn(
        "box",
        |children: rhai::Array| -> Result<Node, Box<rhai::EvalAltResult>> {
            Ok(Node::Box {
                children: nodes_of(children)?,
            })
        },
    );
    engine.register_fn(
        "row",
        |children: rhai::Array| -> Result<Node, Box<rhai::EvalAltResult>> {
            Ok(Node::Row {
                gap: default_gap(),
                children: nodes_of(children)?,
            })
        },
    );
    engine.register_fn(
        "row",
        |children: rhai::Array, options: Dynamic| -> Result<Node, Box<rhai::EvalAltResult>> {
            let options: ContainerOptions = options_of(&options)?;
            Ok(Node::Row {
                gap: options.gap,
                children: nodes_of(children)?,
            })
        },
    );
    engine.register_fn(
        "column",
        |children: rhai::Array| -> Result<Node, Box<rhai::EvalAltResult>> {
            Ok(Node::Column {
                gap: default_gap(),
                children: nodes_of(children)?,
            })
        },
    );
    engine.register_fn(
        "column",
        |children: rhai::Array, options: Dynamic| -> Result<Node, Box<rhai::EvalAltResult>> {
            let options: ContainerOptions = options_of(&options)?;
            Ok(Node::Column {
                gap: options.gap,
                children: nodes_of(children)?,
            })
        },
    );
    engine.register_fn("separator", || -> Node {
        Node::Separator {
            kind: SeparatorKind::Dotted,
            orientation: Orientation::Horizontal,
        }
    });
    engine.register_fn(
        "separator",
        |options: Dynamic| -> Result<Node, Box<rhai::EvalAltResult>> {
            let options: SeparatorOptions = options_of(&options)?;
            Ok(Node::Separator {
                kind: options.kind,
                orientation: options.orientation,
            })
        },
    );
    engine.register_fn(
        "bar",
        |percent: Dynamic| -> Result<Node, Box<rhai::EvalAltResult>> {
            Ok(Node::Bar {
                percent: percent_of(&percent)?,
                color: Color::LightGreen,
            })
        },
    );
    engine.register_fn(
        "bar",
        |percent: Dynamic, options: Dynamic| -> Result<Node, Box<rhai::EvalAltResult>> {
            let options: BarOptions = options_of(&options)?;
            Ok(Node::Bar {
                percent: percent_of(&percent)?,
                color: options.color,
            })
        },
    );
    engine.register_fn(
        "status_dot",
        |tone: Dynamic| -> Result<Node, Box<rhai::EvalAltResult>> {
            let tone: Tone =
                rhai::serde::from_dynamic(&tone).map_err(|err| native_error(err.to_string()))?;
            Ok(Node::StatusDot { tone })
        },
    );
    engine.register_fn(
        "link",
        |label: Dynamic, href: Dynamic| -> Result<Node, Box<rhai::EvalAltResult>> {
            Ok(Node::Link {
                href: text_of(&href)?,
                label: text_of(&label)?,
            })
        },
    );
    engine.register_fn(
        "image",
        |src: Dynamic| -> Result<Node, Box<rhai::EvalAltResult>> {
            Ok(Node::Image {
                src: text_of(&src)?,
                alt: String::new(),
            })
        },
    );
    engine.register_fn(
        "image",
        |src: Dynamic, alt: Dynamic| -> Result<Node, Box<rhai::EvalAltResult>> {
            Ok(Node::Image {
                src: text_of(&src)?,
                alt: text_of(&alt)?,
            })
        },
    );
    engine.register_fn(
        "icon",
        |src: Dynamic| -> Result<Node, Box<rhai::EvalAltResult>> {
            Ok(Node::Icon {
                src: text_of(&src)?,
                alt: String::new(),
            })
        },
    );
    engine.register_fn(
        "icon",
        |src: Dynamic, alt: Dynamic| -> Result<Node, Box<rhai::EvalAltResult>> {
            Ok(Node::Icon {
                src: text_of(&src)?,
                alt: text_of(&alt)?,
            })
        },
    );
    engine.register_fn(
        "window",
        |title: Dynamic, children: rhai::Array| -> Result<Node, Box<rhai::EvalAltResult>> {
            Ok(Node::Window {
                title: text_of(&title)?,
                children: nodes_of(children)?,
            })
        },
    );
}

pub(crate) fn from_dynamic(value: Dynamic) -> Result<Node, String> {
    let type_name = value.type_name().to_string();
    value
        .try_cast::<Node>()
        .ok_or_else(|| format!("widget must return a Node, got {type_name}"))
}
