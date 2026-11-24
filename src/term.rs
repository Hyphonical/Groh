use std::io::{self, IsTerminal, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const WIDTH: usize = 96;
const H: char = '\u{2501}';
const CHECK: char = '\u{2713}';
const CROSS: char = '\u{2715}';

#[derive(Clone, Copy)]
pub(crate) struct Rgb(u8, u8, u8);

pub(crate) const HEAD: Rgb = Rgb(0xff, 0xff, 0xd4);
pub(crate) const FG: Rgb = Rgb(0xff, 0xff, 0xd4);
pub(crate) const DIM: Rgb = Rgb(0x77, 0x83, 0x85);
pub(crate) const MUTED: Rgb = Rgb(0x44, 0x47, 0x4a);
pub(crate) const OK: Rgb = Rgb(0xaf, 0xb9, 0x7a);
pub(crate) const FAIL: Rgb = Rgb(0xc0, 0x6d, 0x44);

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
            self.paint(&prefix, MUTED),
            self.paint(label, HEAD),
            self.paint(&fill, MUTED),
        );
    }

    pub(crate) fn group(&self, out: &mut impl Write, name: &str) {
        let _ = writeln!(out);
        let _ = writeln!(out, "  {}", self.paint(name, HEAD));
    }

    pub(crate) fn row(&self, out: &mut impl Write, label: &str, value: &str, color: Rgb) {
        let padded = pad(label, 10);
        let _ = writeln!(
            out,
            "  {}{}",
            self.paint(&padded, DIM),
            self.paint(value, color)
        );
    }

    pub(crate) fn done(&self, out: &mut impl Write, label: &str, text: &str) {
        let padded = pad(label, 10);
        let msg = format!("{CHECK}  {text}");
        let _ = writeln!(
            out,
            "  {}{}",
            self.paint(&padded, DIM),
            self.paint(&msg, OK)
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
        let _ = writeln!(out, "{}", self.paint(&header, FAIL));
        if let Some(d) = detail {
            let line = format!("     {d}");
            let _ = writeln!(out, "{}", self.paint(&line, FG));
        }
        if let Some(h) = hint {
            let line = format!("     {h}");
            let _ = writeln!(out, "{}", self.paint(&line, DIM));
        }
    }

    pub(crate) fn container_header(&self, out: &mut impl Write) {
        let line = format!("  {}{}{}", pad("name", 14), pad("state", 10), "detail");
        let _ = writeln!(out, "{}", self.paint(&line, DIM));
    }

    pub(crate) fn container_row(
        &self,
        out: &mut impl Write,
        name: &str,
        state: &str,
        detail: &str,
    ) {
        let state_color = if state == "running" { OK } else { FAIL };
        let _ = writeln!(
            out,
            "  {}{}{}",
            self.paint(&pad(name, 14), FG),
            self.paint(&pad(state, 10), state_color),
            self.paint(detail, DIM),
        );
    }

    pub(crate) fn container_change(&self, out: &mut impl Write, added: bool, name: &str) {
        let (mark, color) = if added { ("+", OK) } else { ("\u{2212}", FAIL) };
        let _ = writeln!(
            out,
            "  {}{}",
            self.paint(&pad(mark, 3), color),
            self.paint(name, FG),
        );
    }

    pub(crate) fn event(&self, out: &mut impl Write, scope: &str, msg: &str) {
        let ts = utc_timestamp();
        let _ = writeln!(out, "{ts}  {scope:<10} {msg}");
    }
}

pub(crate) fn pad(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(width - len))
    }
}

pub(crate) fn wrap(text: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub(crate) fn max_width() -> usize {
    WIDTH
}

fn utc_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u32, d as u32)
}
