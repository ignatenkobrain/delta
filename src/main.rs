extern crate structopt;

mod parse;

use std::io::{self, BufRead, ErrorKind};
use std::process;

use console::strip_ansi_codes;
use structopt::StructOpt;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

pub const DELTA_THEME_DEFAULT: &str = "base16-mocha.dark";

const GREEN: Color = Color {
    r: 0x01,
    g: 0x18,
    b: 0x00,
    a: 0x00,
};

const RED: Color = Color {
    r: 0x24,
    g: 0x00,
    b: 0x01,
    a: 0x00,
};

#[derive(StructOpt, Debug)]
#[structopt(name = "delta")]
struct Opt {
    /// Use diff highlighting colors appropriate for a light terminal
    /// background
    #[structopt(long = "light")]
    light: bool,

    /// Use diff highlighting colors appropriate for a dark terminal
    /// background
    #[structopt(long = "dark")]
    dark: bool,

    /// The width (in characters) of the diff highlighting. By
    /// default, the highlighting extends to the last character on
    /// each line
    #[structopt(short = "-w", long = "width")]
    width: Option<u16>,
}

#[derive(PartialEq)]
enum State {
    Commit,
    DiffMeta,
    DiffHunk,
    Unknown,
}

fn main() {
    match delta() {
        Err(error) => {
            match error.kind() {
                ErrorKind::BrokenPipe => process::exit(0),
                _ => eprintln!("{}", error),
            }
        }
        _ => (),
    }
}

fn delta() -> std::io::Result<()> {
    use std::io::Write;
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();
    let theme = &theme_set.themes[DELTA_THEME_DEFAULT];
    let mut output = String::new();
    let mut state = State::Unknown;
    let mut syntax: Option<&SyntaxReference> = None;
    let mut did_emit_line: bool;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let opt = Opt::from_args();

    for _line in stdin.lock().lines() {
        let raw_line = _line?;
        let mut line = strip_ansi_codes(&raw_line).to_string();
        did_emit_line = false;
        if line.starts_with("diff --") {
            state = State::DiffMeta;
            syntax = match parse::get_file_extension_from_diff_line(&line) {
                Some(extension) => syntax_set.find_syntax_by_extension(extension),
                None => None,
            };
        } else if line.starts_with("commit") {
            state = State::Commit;
        } else if line.starts_with("@@") {
            state = State::DiffHunk;
        } else if state == State::DiffHunk {
            match syntax {
                Some(syntax) => {
                    let mut highlighter = HighlightLines::new(syntax, theme);
                    let first_char = line.chars().next();
                    let background_color = match first_char {
                        Some('+') => Some(GREEN),
                        Some('-') => Some(RED),
                        _ => None,
                    };
                    if first_char == Some('+') || first_char == Some('-') {
                        line = line[1..].to_string();
                        output.push_str(" ");
                    }
                    if line.len() < 100 {
                        line = format!("{}{}", line, " ".repeat(100 - line.len()));
                    }
                    let ranges: Vec<(Style, &str)> = highlighter.highlight(&line, &syntax_set);
                    paint_ranges(&ranges[..], background_color, &mut output);
                    writeln!(stdout, "{}", output)?;
                    output.truncate(0);
                    did_emit_line = true;
                }
                None => (),
            }
        }
        if !did_emit_line {
            writeln!(stdout, "{}", raw_line)?;
        }
    }
    Ok(())
}

/// Based on as_24_bit_terminal_escaped from syntect
fn paint_ranges(
    foreground_style_ranges: &[(Style, &str)],
    background_color: Option<Color>,
    buf: &mut String,
) -> () {
    for &(ref style, text) in foreground_style_ranges.iter() {
        paint(text, Some(style.foreground), background_color, false, buf);
    }
    buf.push_str("\x1b[0m");
}

/// Write text to buffer with color escape codes applied.
fn paint(
    text: &str,
    foreground_color: Option<Color>,
    background_color: Option<Color>,
    reset_color: bool,
    buf: &mut String,
) -> () {
    use std::fmt::Write;
    match background_color {
        Some(background_color) => {
            write!(
                buf,
                "\x1b[48;2;{};{};{}m",
                background_color.r,
                background_color.g,
                background_color.b
            ).unwrap();
            if reset_color {
                buf.push_str("\x1b[0m");
            }
        }
        None => (),
    }
    match foreground_color {
        Some(foreground_color) => {
            write!(
                buf,
                "\x1b[38;2;{};{};{}m{}",
                foreground_color.r,
                foreground_color.g,
                foreground_color.b,
                text
            ).unwrap();
            if reset_color {
                buf.push_str("\x1b[0m");
            }
        }
        None => {
            write!(buf, "{}", text).unwrap();
        }
    }
}
