use crate::palette::{BLUE, CYAN, FOREGROUND, GREEN, RED, Rgb, YELLOW};
use std::io::{self, IsTerminal, Write};
use std::time::SystemTime;

pub(crate) const WIDTH: usize = 96;
const H: char = '\u{2501}';
const CHECK: char = '\u{2713}';
const CROSS: char = '\u{2715}';
pub(crate) struct Term {
    tty: bool,
    color: bool,
}

impl Term {
    pub(crate) fn stderr(no_color: bool) -> Self {
        let tty = io::stderr().is_terminal();
        Self {
            tty,
            color: tty && !no_color,
        }
    }

    pub(crate) fn stdout(no_color: bool) -> Self {
        let tty = io::stdout().is_terminal();
        Self {
            tty,
            color: tty && !no_color,
        }
    }

    pub(crate) fn is_tty(&self) -> bool {
        self.tty
    }

    pub(crate) fn paint(&self, text: &str, color: Rgb) -> String {
        if self.color {
            format!(
                "\x1b[38;2;{};{};{}m{text}\x1b[0m",
                color.0, color.1, color.2
            )
        } else {
            text.to_string()
        }
    }

    pub(crate) fn rule(&self, out: &mut impl Write, label: &str) {
        let fill_len = WIDTH.saturating_sub(label.chars().count() + 8);
        let prefix = format!("{H}{H}");
        let fill: String = std::iter::repeat_n(H, fill_len).collect();
        let _ = writeln!(
            out,
            "  {}  {}  {}",
            self.paint(&prefix, BLUE),
            self.paint(label, FOREGROUND),
            self.paint(&fill, BLUE),
        );
    }

    pub(crate) fn group(&self, out: &mut impl Write, name: &str) {
        let _ = writeln!(out);
        let _ = writeln!(out, "  {}", self.paint(name, FOREGROUND));
    }

    pub(crate) fn row(&self, out: &mut impl Write, label: &str, value: &str, color: Rgb) {
        let padded = pad(label, 10);
        let _ = writeln!(
            out,
            "  {}{}",
            self.paint(&padded, CYAN),
            self.paint(value, color)
        );
    }

    pub(crate) fn done(&self, out: &mut impl Write, label: &str, text: &str) {
        let padded = pad(label, 10);
        let msg = format!("{CHECK}  {text}");
        let _ = writeln!(
            out,
            "  {}{}",
            self.paint(&padded, CYAN),
            self.paint(&msg, GREEN)
        );
    }

    pub(crate) fn error(
        &self,
        out: &mut impl Write,
        title: &str,
        detail: Option<&str>,
        hint: Option<&str>,
    ) {
        let header = format!("  {CROSS}  {title}");
        let _ = writeln!(out, "{}", self.paint(&header, RED));
        if let Some(d) = detail {
            let line = format!("     {d}");
            let _ = writeln!(out, "{}", self.paint(&line, FOREGROUND));
        }
        if let Some(h) = hint {
            let line = format!("     {h}");
            let _ = writeln!(out, "{}", self.paint(&line, CYAN));
        }
    }

    pub(crate) fn container_header(&self, out: &mut impl Write) {
        let line = format!("  {}{}{}", pad("name", 14), pad("state", 10), "detail");
        let _ = writeln!(out, "{}", self.paint(&line, CYAN));
    }

    pub(crate) fn container_row(
        &self,
        out: &mut impl Write,
        name: &str,
        state: &str,
        detail: &str,
    ) {
        let state_color = match state {
            "running" => GREEN,
            "paused" | "restarting" => YELLOW,
            _ => RED,
        };
        let _ = writeln!(
            out,
            "  {}{}{}",
            self.paint(&pad(name, 14), FOREGROUND),
            self.paint(&pad(state, 10), state_color),
            self.paint(detail, CYAN),
        );
    }

    pub(crate) fn container_change(&self, out: &mut impl Write, added: bool, name: &str) {
        let (mark, color) = if added {
            ("+", GREEN)
        } else {
            ("\u{2212}", RED)
        };
        let _ = writeln!(
            out,
            "  {}{}",
            self.paint(&pad(mark, 3), color),
            self.paint(name, FOREGROUND),
        );
    }

    pub(crate) fn event(&self, out: &mut impl Write, scope: &str, msg: &str) {
        let ts = utc_timestamp();
        let _ = writeln!(out, "{ts}  {scope:<10} {msg}");
    }
}

pub(crate) fn pad(text: &str, width: usize) -> String {
    format!("{text:<width$}")
}

pub(crate) fn wrap(text: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_chars = 0;
    for word in text.split_whitespace() {
        let word_chars = word.chars().count();
        if current.is_empty() {
            current.push_str(word);
            current_chars = word_chars;
        } else if current_chars + 1 + word_chars <= max {
            current.push(' ');
            current.push_str(word);
            current_chars += 1 + word_chars;
        } else {
            lines.push(current);
            current = word.to_string();
            current_chars = word_chars;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn utc_timestamp() -> String {
    let now: chrono::DateTime<chrono::Utc> = SystemTime::now().into();
    now.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
