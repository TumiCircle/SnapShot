use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Opens (or creates) `%APPDATA%\PixelSnap\logs\pixelsnap.log` for append.
/// Called once at startup; no extra dependencies are introduced.
pub fn init() {
    let dir = log_dir();
    if let Some(d) = &dir {
        let _ = fs::create_dir_all(d);
    }
    if let Some(p) = dir.map(|d| d.join("pixelsnap.log")) {
        if let Ok(f) = OpenOptions::new().create(true).append(true).open(p) {
            if let Ok(mut guard) = LOG_FILE.lock() {
                *guard = Some(f);
            }
        }
    }
}

/// Writes a timestamped line to the log file and to stderr.
pub fn log_line(line: &str) {
    let line = format!("[{}] {}", timestamp(), line);
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(f) = guard.as_mut() {
            let _ = writeln!(f, "{}", line);
            let _ = f.flush();
        }
    }
    eprintln!("{}", line);
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

fn log_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("PixelSnap").join("logs"))
}

/// Logging macro usable from any module: `log_line!("msg {}", x);`
#[macro_export]
macro_rules! log_line {
    ($($arg:tt)*) => {{
        let line = format!($($arg)*);
        $crate::log::log_line(&line);
    }};
}
