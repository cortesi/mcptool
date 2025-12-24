//! Output formatting and display utilities.

/// Call tool result display.
pub mod calltool;
/// Completion result display.
pub mod complete;
/// General output formatting.
pub mod formatter;
/// Prompt result display.
pub mod getprompt;
/// Initialization result display.
pub mod initresult;
/// Prompt list display.
pub mod listprompts;
/// Resource list display.
pub mod listresources;
/// Resource template list display.
pub mod listresourcetemplates;
/// Tool list display.
pub mod listtools;
/// Resource read result display.
pub mod readresource;

use std::{
    fmt,
    io::{self, Write},
    result,
    str::FromStr,
    sync::{Arc, Mutex},
};

use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    parsing::SyntaxSet,
    util::{LinesWithEndings, as_24_bit_terminal_escaped},
};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use textwrap::{Options, wrap};
use tracing::{Event, Level, Subscriber, field};
use tracing_subscriber::{
    EnvFilter,
    layer::{Context, Layer, SubscriberExt},
    registry::LookupSpan,
    util::SubscriberInitExt,
};

use crate::Result;

/// Log level configuration for the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    /// Convert to tracing Level
    pub fn to_tracing_level(&self) -> Level {
        match self {
            Self::Error => Level::ERROR,
            Self::Warn => Level::WARN,
            Self::Info => Level::INFO,
            Self::Debug => Level::DEBUG,
            Self::Trace => Level::TRACE,
        }
    }

    /// Get the level as a string for env filter
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

impl FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            "trace" => Ok(Self::Trace),
            _ => Err(format!("Invalid log level: {s}")),
        }
    }
}

/// Solarized Dark color scheme
#[allow(dead_code)]
struct SolarizedDark;

#[allow(dead_code, clippy::missing_docs_in_private_items)]
impl SolarizedDark {
    // Background tones
    const BASE03: Color = Color::Rgb(0, 43, 54); // darkest background
    const BASE02: Color = Color::Rgb(7, 54, 66); // dark background
    const BASE01: Color = Color::Rgb(88, 110, 117); // darker content
    const BASE00: Color = Color::Rgb(101, 123, 131); // dark content

    // Content tones
    const BASE0: Color = Color::Rgb(131, 148, 150); // light content
    const BASE1: Color = Color::Rgb(147, 161, 161); // lighter content
    const BASE2: Color = Color::Rgb(238, 232, 213); // light background
    const BASE3: Color = Color::Rgb(253, 246, 227); // lightest background

    // Accent colors
    const YELLOW: Color = Color::Rgb(181, 137, 0);
    const ORANGE: Color = Color::Rgb(203, 75, 22);
    const RED: Color = Color::Rgb(220, 50, 47);
    const MAGENTA: Color = Color::Rgb(211, 54, 130);
    const VIOLET: Color = Color::Rgb(108, 113, 196);
    const BLUE: Color = Color::Rgb(38, 139, 210);
    const CYAN: Color = Color::Rgb(42, 161, 152);
    const GREEN: Color = Color::Rgb(133, 153, 0);
}

/// Handles all output formatting for the application.
///
/// This struct provides a unified interface for outputting text to the console,
/// with support for both human-readable formatted output and machine-readable JSON output.
/// It uses the Solarized Dark color scheme for styled terminal output and can switch
/// between colored text mode and JSON mode based on the `json` flag.
///
/// The struct is `Clone` and thread-safe, allowing it to be shared across different
/// parts of the application.
#[derive(Clone)]
pub struct Output {
    /// The standard output stream for writing to the terminal.
    stdout: Arc<Mutex<StandardStream>>,
    /// Whether to output in JSON format.
    pub json: bool,
    /// Whether to suppress non-essential output.
    pub quiet: bool,
    /// Whether to use colored output.
    color: bool,
    /// The terminal width for text wrapping.
    width: usize,
    /// The current indentation level.
    indent: usize,
}

impl Output {
    pub fn new(color: bool, width: usize) -> Self {
        let color_choice = if color {
            ColorChoice::Always
        } else {
            ColorChoice::Never
        };

        Self {
            stdout: Arc::new(Mutex::new(StandardStream::stdout(color_choice))),
            json: false,
            quiet: false,
            color,
            width,
            indent: 0,
        }
    }

    /// Output JSON with syntax highlighting if color is enabled
    fn output_json(&self, json_str: &str) -> io::Result<()> {
        if self.color {
            // Load syntax highlighting assets
            let ps = SyntaxSet::load_defaults_newlines();
            let ts = ThemeSet::load_defaults();

            let syntax = ps.find_syntax_by_extension("json").unwrap();
            let theme = &ts.themes["Solarized (dark)"];
            let mut h = HighlightLines::new(syntax, theme);
            let mut stdout = self.stdout.lock().unwrap();
            for line in LinesWithEndings::from(json_str) {
                let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();
                let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
                write!(stdout, "{escaped}")?;
            }
            stdout.reset()?;
            stdout.flush()
        } else {
            self.raw(json_str)
        }
    }

    /// Output a JSON value with syntax highlighting if appropriate
    pub fn json_value<T: serde::Serialize>(&self, value: &T) -> Result<()> {
        if self.quiet {
            return Ok(());
        }
        let json_str = serde_json::to_string_pretty(value)?;
        self.output_json(&json_str)?;
        Ok(())
    }

    /// Set JSON output mode.
    pub fn with_json(mut self, json: bool) -> Self {
        self.json = json;
        self
    }

    /// Enable logging with the specified log level and return self.
    pub fn with_logging(self, level: Option<LogLevel>) -> Result<Self> {
        if let Some(log_level) = level {
            let env_filter = EnvFilter::try_new(log_level.as_str()).unwrap_or_default();
            let output_layer = OutputLayer::new(self.clone());

            tracing_subscriber::registry()
                .with(env_filter)
                .with(output_layer)
                .init();
        }

        Ok(self)
    }

    /// Set quiet output mode.
    pub fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Return a copy of this Output with indent incremented by 4 spaces.
    pub fn indent(&self) -> Self {
        let mut output = self.clone();
        output.indent += 4;
        output
    }

    /// Helper to wrap text with a specific indentation
    fn wrap_text(
        &self,
        text: &str,
        available_width: usize,
        initial_indent: &str,
        subsequent_indent: &str,
    ) -> Vec<String> {
        if available_width < 10 {
            // If width is too small, just return the lines as-is
            text.lines().map(|s| s.to_string()).collect()
        } else {
            let options = Options::new(available_width)
                .initial_indent(initial_indent)
                .subsequent_indent(subsequent_indent);
            wrap(text, &options)
                .into_iter()
                .map(|cow| cow.into_owned())
                .collect()
        }
    }

    /// Helper method to write a line with proper indentation and text wrapping.
    fn write_block(&self, message: &str) -> io::Result<()> {
        self.write_block_with_color(message, &ColorSpec::new())
    }

    /// Helper method to write a line with proper indentation, text wrapping, and color.
    fn write_block_with_color(&self, message: &str, color_spec: &ColorSpec) -> io::Result<()> {
        let mut stdout = self.stdout.lock().unwrap();
        let indent_str = " ".repeat(self.indent);
        let available_width = self.width.saturating_sub(self.indent);

        let wrapped_lines = self.wrap_text(message, available_width, "", "");
        let has_color = color_spec != &ColorSpec::new();

        for line in wrapped_lines {
            if has_color {
                stdout.set_color(color_spec)?;
            }
            write!(stdout, "{indent_str}{line}")?;
            if has_color {
                stdout.reset()?;
            }
            writeln!(stdout)?;
        }

        stdout.flush()
    }

    /// Raw output that is not affected by output settings
    fn raw(&self, message: impl Into<String>) -> io::Result<()> {
        if self.quiet {
            return Ok(());
        }
        let message = message.into();
        self.write_block(&message)
    }

    pub fn text(&self, message: impl Into<String>) -> io::Result<()> {
        if self.json || self.quiet {
            return Ok(());
        }

        let message = message.into();

        // Use SOLARIZED_BASE0 for regular text
        let color_spec = ColorSpec::new().set_fg(Some(SolarizedDark::BASE0)).clone();

        self.write_block_with_color(&message, &color_spec)
    }

    pub fn h1(&self, message: impl Into<String>) -> io::Result<()> {
        if self.json || self.quiet {
            return Ok(());
        }

        let message = message.into();

        // Create left-aligned header with padding to fill the FULL width (including indent)
        // The header background should span the entire terminal width
        let message_with_spaces = format!(" {message} ");
        let indent_str = " ".repeat(self.indent);
        let total_content_length = self.indent + message_with_spaces.len();
        let padding = self.width.saturating_sub(total_content_length);
        let header = format!(
            "{}{}{}",
            indent_str,
            message_with_spaces,
            " ".repeat(padding)
        );

        // Set lighter content text on dark background for better readability
        let color_spec = ColorSpec::new()
            .set_fg(Some(SolarizedDark::BASE0))
            .set_bg(Some(SolarizedDark::BASE03))
            .set_bold(true)
            .clone();

        // Write directly to stdout with color, bypassing write_block to avoid double indentation
        let mut stdout = self.stdout.lock().unwrap();
        stdout.set_color(&color_spec)?;
        write!(stdout, "{header}")?;
        stdout.reset()?;
        writeln!(stdout)?;
        stdout.flush()
    }

    pub fn h2(&self, message: impl Into<String>) -> io::Result<()> {
        if self.json || self.quiet {
            return Ok(());
        }

        let message = message.into();

        // Use highlighted foreground color without background
        let color_spec = ColorSpec::new()
            .set_fg(Some(SolarizedDark::BLUE))
            .set_bold(true)
            .clone();

        self.write_block_with_color(&message, &color_spec)
    }

    pub fn h3(&self, message: impl Into<String>) -> io::Result<()> {
        if self.json || self.quiet {
            return Ok(());
        }

        let message = message.into();

        // Just bold text, no color change
        let color_spec = ColorSpec::new().set_bold(true).clone();

        self.write_block_with_color(&message, &color_spec)
    }

    pub fn trace_warn(&self, message: impl Into<String>) -> io::Result<()> {
        self.status(message, "[WARNING]", SolarizedDark::YELLOW, false)
    }

    pub fn trace_error(&self, message: impl Into<String>) -> io::Result<()> {
        self.status(message, "[ERROR]", SolarizedDark::RED, true)
    }

    pub fn trace_success(&self, message: impl Into<String>) -> io::Result<()> {
        self.status(message, "[OK]", SolarizedDark::GREEN, false)
    }

    pub fn trace_debug(&self, message: impl Into<String>) -> io::Result<()> {
        self.status(message, "[DEBUG]", SolarizedDark::MAGENTA, false)
    }

    pub fn trace_info(&self, message: impl Into<String>) -> io::Result<()> {
        self.status(message, "[INFO]", SolarizedDark::BLUE, false)
    }

    pub fn success(&self, message: impl Into<String>) -> io::Result<()> {
        if self.json || self.quiet {
            return Ok(());
        }

        let message = message.into();

        // Use SOLARIZED_YELLOW for notes
        let color_spec = ColorSpec::new().set_fg(Some(SolarizedDark::GREEN)).clone();

        self.write_block_with_color(&message, &color_spec)
    }

    pub fn note(&self, message: impl Into<String>) -> io::Result<()> {
        if self.json || self.quiet {
            return Ok(());
        }

        let message = message.into();

        // Use SOLARIZED_YELLOW for notes
        let color_spec = ColorSpec::new().set_fg(Some(SolarizedDark::YELLOW)).clone();

        self.write_block_with_color(&message, &color_spec)
    }

    /// Helper method to write status messages with consistent formatting.
    fn status(
        &self,
        message: impl Into<String>,
        prefix: &str,
        color: Color,
        bold: bool,
    ) -> io::Result<()> {
        if self.json || self.quiet {
            return Ok(());
        }

        let message = message.into();
        let formatted_message = format!("{prefix} {message}");

        let mut color_spec = ColorSpec::new();
        color_spec.set_fg(Some(color));
        if bold {
            color_spec.set_bold(true);
        }

        self.write_block_with_color(&formatted_message, &color_spec)
    }

    pub fn trace(&self, message: impl Into<String>, level: Level) -> io::Result<()> {
        if self.quiet {
            return Ok(());
        }
        let message = message.into();
        let formatted_message = format!("trace: {message}");

        let mut color_spec = ColorSpec::new();
        match level {
            Level::ERROR => {
                color_spec.set_fg(Some(SolarizedDark::RED)).set_bold(true);
            }
            Level::WARN => {
                color_spec.set_fg(Some(SolarizedDark::YELLOW));
            }
            Level::INFO => {
                color_spec.set_fg(Some(SolarizedDark::BASE0)); // Light content color
            }
            Level::DEBUG => {
                color_spec.set_fg(Some(SolarizedDark::MAGENTA));
            }
            Level::TRACE => {
                color_spec.set_fg(Some(SolarizedDark::BASE01)); // Darker content for trace
            }
        };

        self.write_block_with_color(&formatted_message, &color_spec)
    }

    pub fn kv(&self, key: impl Into<String>, value: impl Into<String>) -> io::Result<()> {
        if self.json || self.quiet {
            return Ok(());
        }

        let key = key.into();
        let value = value.into();

        let mut stdout = self.stdout.lock().unwrap();
        let indent_str = " ".repeat(self.indent);

        // Write key with color
        let key_color_spec = ColorSpec::new()
            .set_fg(Some(SolarizedDark::CYAN))
            .set_bold(true)
            .clone();

        stdout.set_color(&key_color_spec)?;
        write!(stdout, "{indent_str}{key}: ")?;
        stdout.reset()?;

        // Calculate indentation for wrapped value lines
        let key_prefix_len = self.indent + key.len() + 2; // +2 for ": "
        let available_width = self.width.saturating_sub(key_prefix_len);
        let value_indent = " ".repeat(key_prefix_len);
        let value_color_spec = ColorSpec::new().set_fg(Some(SolarizedDark::BASE0)).clone();

        // Handle simple single-line case
        if value.len() <= available_width && !value.contains('\n') {
            stdout.set_color(&value_color_spec)?;
            write!(stdout, "{value}")?;
            stdout.reset()?;
            writeln!(stdout)?;
        } else {
            // Multi-line or long value
            let lines: Vec<&str> = value.lines().collect();

            for (idx, line) in lines.iter().enumerate() {
                let wrapped = if idx == 0 {
                    // First line - no initial indent since it follows the key
                    self.wrap_text(line, available_width, "", &value_indent)
                } else {
                    // Subsequent lines - indent to align under the value
                    self.wrap_text(line, available_width, &value_indent, &value_indent)
                };

                for wrapped_line in wrapped {
                    stdout.set_color(&value_color_spec)?;
                    write!(stdout, "{wrapped_line}")?;
                    stdout.reset()?;
                    writeln!(stdout)?;
                }
            }
        }

        stdout.flush()
    }

    pub fn ping(&self) -> Result<()> {
        if self.json {
            self.output_json("{}")?;
        } else {
            self.trace_success("Ping successful!")?;
        }
        Ok(())
    }
}

impl Default for Output {
    fn default() -> Self {
        // Default to color detection based on TTY
        let color = atty::is(atty::Stream::Stdout);
        Self::new(color, 80)
    }
}

/// A tracing subscriber layer that forwards log messages to an Output instance.
///
/// This struct implements the `tracing_subscriber::Layer` trait to integrate with
/// the tracing ecosystem. It captures log events and forwards them to the Output
/// struct for consistent formatting. This allows application logs to respect the
/// same formatting rules (including JSON mode) as regular output.
pub struct OutputLayer {
    /// The output instance to forward logs to.
    output: Output,
}

impl OutputLayer {
    pub fn new(output: Output) -> Self {
        Self { output }
    }
}

impl<S> Layer<S> for OutputLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level = event.metadata().level();
        let mut visitor = MessageVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);
        let message = visitor.message;

        self.output.trace(&message, *level).ok();
    }
}

/// A visitor for extracting message content from tracing events.
///
/// This struct implements the `tracing::field::Visit` trait to extract the message
/// field from tracing events. It's used internally by `OutputLayer` to get the
/// actual log message text that needs to be formatted and displayed.
struct MessageVisitor {
    /// The extracted message text.
    message: String,
}

impl field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &field::Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        }
    }

    fn record_str(&mut self, field: &field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}
