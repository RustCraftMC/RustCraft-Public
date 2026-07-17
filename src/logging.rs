use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_FILTER: &str = "warn,rustcraft=debug";
static LOG_FILE_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

struct TeeWriter {
    stderr: io::Stderr,
    file: File,
    ansi_state: AnsiState,
}

#[derive(Clone, Copy, Default)]
enum AnsiState {
    #[default]
    Text,
    Escape,
    Csi,
}

impl Write for TeeWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stderr.write_all(buffer)?;
        let plain = strip_ansi(&mut self.ansi_state, buffer);
        self.file.write_all(&plain)?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stderr.flush()?;
        self.file.flush()
    }
}

pub fn init() {
    let env = env_logger::Env::default()
        .filter_or("RUST_LOG", DEFAULT_FILTER)
        .write_style_or("RUST_LOG_STYLE", "always");
    let mut builder = env_logger::Builder::from_env(env);
    let log_file = create_log_file();
    match &log_file {
        Ok((_, file)) => {
            builder.target(env_logger::Target::Pipe(Box::new(TeeWriter {
                stderr: io::stderr(),
                file: file
                    .try_clone()
                    .expect("newly-created log file should be cloneable"),
                ansi_state: AnsiState::Text,
            })));
        }
        Err(_) => {
            builder.target(env_logger::Target::Stderr);
        }
    }
    builder.format(|buffer, record| {
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("unnamed");
        let module = record.module_path().unwrap_or(record.target());
        let target = record.target();
        let file = record.file().unwrap_or("unknown");
        let line = record.line().unwrap_or(0);
        let level_style = match record.level() {
            log::Level::Trace => env_logger::fmt::style::AnsiColor::BrightBlack.on_default(),
            log::Level::Debug => env_logger::fmt::style::AnsiColor::Blue.on_default(),
            log::Level::Info => env_logger::fmt::style::AnsiColor::Green.on_default(),
            log::Level::Warn => env_logger::fmt::style::AnsiColor::Yellow.on_default(),
            log::Level::Error => env_logger::fmt::style::AnsiColor::Red
                .on_default()
                .effects(env_logger::fmt::style::Effects::BOLD),
        };

        writeln!(
            buffer,
            "{} {level_style}{:<5}{level_style:#} [{}] [{}] [{}] {}:{} | {}",
            buffer.timestamp_millis(),
            record.level(),
            thread_name,
            target,
            module,
            file,
            line,
            record.args()
        )
    });

    let initialised = match builder.try_init() {
        Ok(()) => true,
        Err(error) => {
            eprintln!("[RustCraft] Failed to initialise logger: {error}");
            false
        }
    };

    let path = log_file.as_ref().ok().map(|(path, _)| path.clone());
    let _ = LOG_FILE_PATH.set(path);
    if initialised {
        match log_file {
            Ok((path, _)) => log::info!("client log file: {}", path.display()),
            Err(error) => log::warn!("file logging unavailable: {error}; using stderr only"),
        }
    }

    install_panic_hook();
}

fn strip_ansi(state: &mut AnsiState, buffer: &[u8]) -> Vec<u8> {
    let mut plain = Vec::with_capacity(buffer.len());
    for &byte in buffer {
        match *state {
            AnsiState::Text if byte == 0x1b => *state = AnsiState::Escape,
            AnsiState::Text => plain.push(byte),
            AnsiState::Escape if byte == b'[' => *state = AnsiState::Csi,
            AnsiState::Escape if byte == 0x1b => {}
            AnsiState::Escape => {
                *state = AnsiState::Text;
                plain.push(byte);
            }
            AnsiState::Csi if (0x40..=0x7e).contains(&byte) => *state = AnsiState::Text,
            AnsiState::Csi => {}
        }
    }
    plain
}

fn create_log_file() -> Result<(PathBuf, File), String> {
    let log_dir = log_directory();
    std::fs::create_dir_all(&log_dir).map_err(|error| {
        format!(
            "failed to create log directory '{}': {error}",
            log_dir.display()
        )
    })?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = log_dir.join(format!("rustcraft-{timestamp}.log"));
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("failed to create log file '{}': {error}", path.display()))?;
    Ok((path, file))
}

fn log_directory() -> PathBuf {
    if let Some(path) = std::env::var_os("RUSTCRAFT_LOG_DIR") {
        return PathBuf::from(path);
    }

    let current = std::env::current_dir().ok();
    if let Some(path) = current.as_ref().filter(|path| has_assets(path)) {
        return path.join("logs");
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest.is_dir() {
        return manifest.join("logs");
    }

    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or(current)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("logs")
}

fn has_assets(path: &Path) -> bool {
    path.join("assets/minecraft").is_dir()
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("unnamed");
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "unknown".to_string());
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        let backtrace = std::backtrace::Backtrace::force_capture();

        log::error!(
            target: "rustcraft::panic",
            "panic on thread '{thread_name}' at {location}: {payload}\n{backtrace}"
        );
        log::logger().flush();
        default_hook(info);
    }));
}

pub fn log_startup_context() {
    let current_dir = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|error| format!("unavailable ({error})"));
    let executable = std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|error| format!("unavailable ({error})"));
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_FILTER.to_string());
    let log_file = LOG_FILE_PATH
        .get()
        .and_then(|path| path.as_ref())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "stderr only".to_string());

    log::info!(
        target: "rustcraft::startup",
        "RustCraft {} starting (pid={}, profile={}, os={}, arch={})",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        if cfg!(debug_assertions) { "debug" } else { "release" },
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    log::info!(target: "rustcraft::startup", "executable={executable}");
    log::info!(target: "rustcraft::startup", "runtime_directory={current_dir}");
    log::info!(target: "rustcraft::startup", "log_destination={log_file}");
    log::info!(target: "rustcraft::startup", "log_filter={rust_log}");
}

const MAX_EVENT_TEXT_CHARS: usize = 4_096;

/// Converts untrusted server/player text into a single safe log line.
pub fn event_text(input: &str) -> String {
    let mut output = String::with_capacity(input.len().min(MAX_EVENT_TEXT_CHARS));
    let mut chars = input.chars();
    let mut written = 0;

    while let Some(ch) = chars.next() {
        if ch == '\u{00a7}' {
            // Minecraft formatting is a section sign followed by one format code.
            chars.next();
            continue;
        }
        if written >= MAX_EVENT_TEXT_CHARS {
            output.push_str("...[truncated]");
            break;
        }
        match ch {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{001b}' => output.push_str("\\x1b"),
            ch if ch.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(output, "\\u{{{:x}}}", ch as u32);
            }
            ch => output.push(ch),
        }
        written += 1;
    }

    output
}

pub fn outbound_chat_text(message: &str) -> String {
    let command = message.split_whitespace().next().unwrap_or_default();
    let sensitive = matches!(
        command.to_ascii_lowercase().as_str(),
        "/login"
            | "/l"
            | "/register"
            | "/reg"
            | "/changepassword"
            | "/changepass"
            | "/password"
            | "/pin"
            | "/auth"
    );
    if sensitive {
        format!("{command} [REDACTED]")
    } else {
        event_text(message)
    }
}

#[cfg(test)]
mod tests {
    use super::{event_text, outbound_chat_text, strip_ansi, AnsiState};

    #[test]
    fn file_output_removes_ansi_styles_across_write_boundaries() {
        let mut state = AnsiState::Text;
        let mut output = strip_ansi(&mut state, b"plain \x1b[3");
        output.extend(strip_ansi(&mut state, b"2mgreen\x1b[0m text"));

        assert_eq!(output, b"plain green text");
    }

    #[test]
    fn event_text_is_single_line_and_removes_minecraft_formatting() {
        assert_eq!(
            event_text("\u{00a7}aHello\nworld\u{001b}[31m"),
            "Hello\\nworld\\x1b[31m"
        );
    }

    #[test]
    fn authentication_commands_are_redacted() {
        assert_eq!(outbound_chat_text("/login secret"), "/login [REDACTED]");
        assert_eq!(outbound_chat_text("/MSG Alex hello"), "/MSG Alex hello");
    }
}
