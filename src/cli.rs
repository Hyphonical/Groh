use crate::term::{DIM, FG, Term, max_width, pad, wrap};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr};

pub(crate) struct Config {
    pub(crate) bind: IpAddr,
    pub(crate) port: u16,
    pub(crate) no_color: bool,
}

pub(crate) enum Action {
    Run(Config),
    Exit(i32),
}

struct FlagRow {
    flag: &'static str,
    alias: &'static str,
    desc: &'static str,
    kind: &'static str,
    default: &'static str,
}

const FLAGS: &[FlagRow] = &[
    FlagRow {
        flag: "--bind",
        alias: "",
        desc: "Listen address",
        kind: "text",
        default: "0.0.0.0",
    },
    FlagRow {
        flag: "--port",
        alias: "",
        desc: "Port to listen on",
        kind: "int",
        default: "3000",
    },
    FlagRow {
        flag: "--no-color",
        alias: "",
        desc: "Plain output without color",
        kind: "bool",
        default: "",
    },
    FlagRow {
        flag: "--help",
        alias: "-h",
        desc: "Show help",
        kind: "bool",
        default: "",
    },
    FlagRow {
        flag: "--version",
        alias: "-V",
        desc: "Show the version",
        kind: "bool",
        default: "",
    },
];

const EXAMPLES: &[(&str, &str)] = &[
    ("Groh", "the dashboard, on :3000"),
    ("Groh --port 8080", "the dashboard, on :8080"),
    ("Groh --bind 127.0.0.1", "only this machine"),
];

pub(crate) fn parse() -> Action {
    let mut bind = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    let mut port: u16 = 3000;
    let mut no_color = false;
    let mut show_help = false;
    let mut show_version = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => show_help = true,
            "-V" | "--version" => show_version = true,
            "--no-color" => no_color = true,
            "--bind" => {
                let Some(val) = args.next() else {
                    return fail_arg(
                        no_color,
                        "missing value for '--bind'",
                        "expected an IP address",
                        Some("see Groh --help"),
                    );
                };
                match val.parse::<IpAddr>() {
                    Ok(ip) => bind = ip,
                    Err(_) => {
                        let detail = format!("expected an IP address, got \"{val}\"");
                        return fail_arg(
                            no_color,
                            "invalid value for '--bind'",
                            &detail,
                            Some("see Groh --help"),
                        );
                    }
                }
            }
            "--port" => {
                let Some(val) = args.next() else {
                    return fail_arg(
                        no_color,
                        "missing value for '--port'",
                        "expected a port number between 1 and 65535",
                        Some("see Groh --help"),
                    );
                };
                match val.parse::<u16>() {
                    Ok(p) if p >= 1 => port = p,
                    _ => {
                        let detail =
                            format!("expected a port number between 1 and 65535, got \"{val}\"");
                        return fail_arg(
                            no_color,
                            "invalid value for '--port'",
                            &detail,
                            Some("see Groh --help"),
                        );
                    }
                }
            }
            other => {
                let title = format!("unexpected argument '{other}'");
                return fail_arg(no_color, &title, "unknown option", Some("see Groh --help"));
            }
        }
    }

    if show_help {
        print_help(no_color);
        return Action::Exit(0);
    }

    if show_version {
        println!("Groh {}", env!("CARGO_PKG_VERSION"));
        return Action::Exit(0);
    }

    Action::Run(Config {
        bind,
        port,
        no_color,
    })
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

    for line in wrap(env!("CARGO_PKG_DESCRIPTION"), max_width() - 2) {
        let _ = writeln!(out, "  {}", term.paint(&line, DIM));
    }

    term.group(&mut out, "Usage");
    let _ = writeln!(out, "    {}", term.paint("Groh [options]", FG));

    term.group(&mut out, "Options");
    let w_flag = FLAGS.iter().map(|r| r.flag.len()).max().unwrap_or(0) + 2;
    let w_alias = FLAGS.iter().map(|r| r.alias.len()).max().unwrap_or(0) + 2;
    let w_desc = FLAGS.iter().map(|r| r.desc.len()).max().unwrap_or(0) + 2;
    let w_kind = FLAGS.iter().map(|r| r.kind.len()).max().unwrap_or(0) + 2;

    for r in FLAGS {
        let left = format!("    {}{}", pad(r.flag, w_flag), pad(r.alias, w_alias));
        let right = format!(
            "{}{}{}",
            pad(r.desc, w_desc),
            pad(r.kind, w_kind),
            r.default
        );
        let _ = writeln!(out, "{}{}", term.paint(&left, FG), term.paint(&right, DIM));
    }

    term.group(&mut out, "Examples");
    let w_cmd = EXAMPLES.iter().map(|(c, _)| c.len()).max().unwrap_or(0) + 2;
    for (cmd, desc) in EXAMPLES {
        let left = pad(&format!("    {cmd}"), w_cmd + 4);
        let _ = writeln!(out, "{}{}", term.paint(&left, FG), term.paint(desc, DIM));
    }
}
