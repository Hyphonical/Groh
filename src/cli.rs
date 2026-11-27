use crate::palette::{CYAN, FOREGROUND};
use crate::term::{Term, WIDTH, pad, wrap};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::time::Duration;

pub(crate) struct Config {
    pub(crate) bind: IpAddr,
    pub(crate) port: u16,
    pub(crate) no_color: bool,
    pub(crate) config: PathBuf,
    pub(crate) widgets_dir: PathBuf,
    pub(crate) pool_size: usize,
    pub(crate) script_timeout: Duration,
}

pub(crate) enum Action {
    Run(Config),
    Exit(i32),
}

const DEFAULT_BIND: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 3000;
const DEFAULT_CONFIG: &str = "./groh.toml";
const DEFAULT_WIDGETS_DIR: &str = "./widgets";
const DEFAULT_POOL_SIZE: usize = 4;
const DEFAULT_SCRIPT_TIMEOUT_SECS: u64 = 5;

struct FlagRow {
    flag: &'static str,
    alias: &'static str,
    desc: &'static str,
    kind: &'static str,
    default: String,
}

fn flags() -> Vec<FlagRow> {
    vec![
        FlagRow {
            flag: "--bind",
            alias: "",
            desc: "Listen address",
            kind: "text",
            default: DEFAULT_BIND.to_string(),
        },
        FlagRow {
            flag: "--port",
            alias: "",
            desc: "Port to listen on",
            kind: "int",
            default: DEFAULT_PORT.to_string(),
        },
        FlagRow {
            flag: "--no-color",
            alias: "",
            desc: "Plain output without color",
            kind: "bool",
            default: String::new(),
        },
        FlagRow {
            flag: "--config",
            alias: "",
            desc: "Layout file",
            kind: "text",
            default: DEFAULT_CONFIG.to_string(),
        },
        FlagRow {
            flag: "--widgets-dir",
            alias: "",
            desc: "User widget directory",
            kind: "text",
            default: DEFAULT_WIDGETS_DIR.to_string(),
        },
        FlagRow {
            flag: "--pool-size",
            alias: "",
            desc: "Script worker threads",
            kind: "int",
            default: DEFAULT_POOL_SIZE.to_string(),
        },
        FlagRow {
            flag: "--script-timeout",
            alias: "",
            desc: "Max seconds per script run",
            kind: "int",
            default: DEFAULT_SCRIPT_TIMEOUT_SECS.to_string(),
        },
        FlagRow {
            flag: "--help",
            alias: "-h",
            desc: "Show help",
            kind: "bool",
            default: String::new(),
        },
        FlagRow {
            flag: "--version",
            alias: "-V",
            desc: "Show the version",
            kind: "bool",
            default: String::new(),
        },
    ]
}

const EXAMPLES: &[(&str, &str)] = &[
    ("Groh", "the dashboard, on :3000"),
    ("Groh --port 8080", "the dashboard, on :8080"),
    ("Groh --bind 127.0.0.1", "only this machine"),
];

struct ParseError {
    title: String,
    detail: String,
}

pub(crate) fn parse() -> Action {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let no_color = args.iter().any(|arg| arg == "--no-color");
    match parse_args(&args) {
        Ok(action) => action,
        Err(err) => fail_arg(no_color, &err.title, &err.detail, Some("see groh --help")),
    }
}

fn parse_args(raw: &[String]) -> Result<Action, ParseError> {
    let mut bind = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    let mut port: u16 = DEFAULT_PORT;
    let mut no_color = false;
    let mut config = PathBuf::from(DEFAULT_CONFIG);
    let mut widgets_dir = PathBuf::from(DEFAULT_WIDGETS_DIR);
    let mut pool_size: usize = DEFAULT_POOL_SIZE;
    let mut script_timeout = Duration::from_secs(DEFAULT_SCRIPT_TIMEOUT_SECS);
    let mut show_help = false;
    let mut show_version = false;

    let mut args = raw.iter().peekable();
    while let Some(arg) = args.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (arg.as_str(), None),
        };
        match flag {
            "-h" | "--help" if inline.is_none() => show_help = true,
            "-V" | "--version" if inline.is_none() => show_version = true,
            "--no-color" if inline.is_none() => no_color = true,
            "--bind" => {
                let val = next_value(flag, inline, &mut args, "expected an IP address")?;
                bind = val.parse::<IpAddr>().map_err(|_| ParseError {
                    title: "invalid value for '--bind'".to_string(),
                    detail: format!("expected an IP address, got \"{val}\""),
                })?;
            }
            "--port" => {
                let val = next_value(
                    flag,
                    inline,
                    &mut args,
                    "expected a port number between 1 and 65535",
                )?;
                port = val
                    .parse::<u16>()
                    .ok()
                    .filter(|&p| p >= 1)
                    .ok_or_else(|| ParseError {
                        title: "invalid value for '--port'".to_string(),
                        detail: format!(
                            "expected a port number between 1 and 65535, got \"{val}\""
                        ),
                    })?;
            }
            "--config" => {
                let val = next_value(flag, inline, &mut args, "expected a path")?;
                config = PathBuf::from(val);
            }
            "--widgets-dir" => {
                let val = next_value(flag, inline, &mut args, "expected a path")?;
                widgets_dir = PathBuf::from(val);
            }
            "--pool-size" => {
                let val = next_value(flag, inline, &mut args, "expected a number")?;
                pool_size = val
                    .parse::<usize>()
                    .ok()
                    .filter(|&n| n >= 1)
                    .ok_or_else(|| ParseError {
                        title: "invalid value for '--pool-size'".to_string(),
                        detail: format!("expected a positive integer, got \"{val}\""),
                    })?;
            }
            "--script-timeout" => {
                let val = next_value(
                    flag,
                    inline,
                    &mut args,
                    "expected a positive duration or seconds",
                )?;
                script_timeout = parse_timeout(val).ok_or_else(|| ParseError {
                    title: "invalid value for '--script-timeout'".to_string(),
                    detail: format!("expected positive seconds (e.g. 5 or 5s), got \"{val}\""),
                })?;
            }
            _ => {
                return Err(ParseError {
                    title: format!("unexpected argument '{arg}'"),
                    detail: "unknown option".to_string(),
                });
            }
        }
    }

    if show_help {
        print_help(no_color);
        return Ok(Action::Exit(0));
    }

    if show_version {
        println!("Groh {}", env!("CARGO_PKG_VERSION"));
        return Ok(Action::Exit(0));
    }

    Ok(Action::Run(Config {
        bind,
        port,
        no_color,
        config,
        widgets_dir,
        pool_size,
        script_timeout,
    }))
}

fn next_value<'a>(
    flag: &str,
    inline: Option<&'a str>,
    args: &mut std::iter::Peekable<std::slice::Iter<'a, String>>,
    expected: &str,
) -> Result<&'a str, ParseError> {
    if let Some(val) = inline {
        if val.is_empty() {
            return Err(ParseError {
                title: format!("missing value for '{flag}'"),
                detail: expected.to_string(),
            });
        }
        return Ok(val);
    }
    args.next().map(String::as_str).ok_or_else(|| ParseError {
        title: format!("missing value for '{flag}'"),
        detail: expected.to_string(),
    })
}

fn parse_timeout(raw: &str) -> Option<Duration> {
    let trimmed = raw.trim();
    if let Ok(seconds) = trimmed.parse::<u64>() {
        return (seconds > 0).then(|| Duration::from_secs(seconds));
    }
    crate::script::parse_interval(trimmed).ok()
}

fn fail_arg(no_color: bool, title: &str, detail: &str, hint: Option<&str>) -> Action {
    let term = Term::stderr(no_color);
    let mut err = io::stderr().lock();
    term.error(&mut err, title, Some(detail), hint);
    Action::Exit(2)
}

fn print_help(no_color: bool) {
    let term = Term::stdout(no_color);
    let mut out = io::stdout().lock();

    let header = format!("Groh (v{})", env!("CARGO_PKG_VERSION"));
    term.rule(&mut out, &header);

    for line in wrap(env!("CARGO_PKG_DESCRIPTION"), WIDTH - 2) {
        let _ = writeln!(out, "  {}", term.paint(&line, CYAN));
    }

    term.group(&mut out, "Usage");
    let _ = writeln!(out, "    {}", term.paint("Groh [options]", FOREGROUND));

    term.group(&mut out, "Options");
    let flags = flags();
    let w_flag = flags.iter().map(|r| r.flag.len()).max().unwrap_or(0) + 2;
    let w_alias = flags.iter().map(|r| r.alias.len()).max().unwrap_or(0) + 2;
    let w_desc = flags.iter().map(|r| r.desc.len()).max().unwrap_or(0) + 2;
    let w_kind = flags.iter().map(|r| r.kind.len()).max().unwrap_or(0) + 2;

    for r in &flags {
        let left = format!("    {}{}", pad(r.flag, w_flag), pad(r.alias, w_alias));
        let right = format!(
            "{}{}{}",
            pad(r.desc, w_desc),
            pad(r.kind, w_kind),
            r.default
        );
        let _ = writeln!(
            out,
            "{}{}",
            term.paint(&left, FOREGROUND),
            term.paint(&right, CYAN)
        );
    }

    term.group(&mut out, "Examples");
    let w_cmd = EXAMPLES.iter().map(|(c, _)| c.len()).max().unwrap_or(0) + 2;
    for (cmd, desc) in EXAMPLES {
        let left = pad(&format!("    {cmd}"), w_cmd + 4);
        let _ = writeln!(
            out,
            "{}{}",
            term.paint(&left, FOREGROUND),
            term.paint(desc, CYAN)
        );
    }
}
