use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use base64::{engine::general_purpose::STANDARD, Engine as _};

static TOAST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn close_old_toasts(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with("toast-") {
            let _ = window.close();
        }
    }
}

fn escape_js_string(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('\'', "\\'")
     .replace('\n', "\\n")
     .replace('\r', "\\r")
     .replace('\t', "\\t")
}

pub fn show_toast_on_main(
    app: &AppHandle,
    thumb_data: &Option<Vec<u8>>,
    game_name: &str,
    file_path: &str,
    mode_str: &str,
    duration_ms: u64,
) {
    let thumb_b64 = match thumb_data {
        Some(bytes) => STANDARD.encode(bytes),
        None => String::new(),
    };

    let file_name = PathBuf::from(file_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "completed".to_string());

    show_toast(app, mode_str, game_name, &file_name, &thumb_b64, duration_ms);
}

/// Shows an error toast with the raw Rust error message so failures (window
/// lost, encode timeout, save failure, disk full, ...) are visible to the user.
pub fn show_error_toast(app: &AppHandle, message: &str) {
    show_toast(app, "ERROR", "CAPTURE FAILED", message, "", 5000);
}

fn show_toast(
    app: &AppHandle,
    mode_str: &str,
    game_name: &str,
    file_name: &str,
    thumb_b64: &str,
    duration_ms: u64,
) {
    close_old_toasts(app);

    let id = format!("toast-{}", TOAST_COUNTER.fetch_add(1, Ordering::SeqCst));

    let (pos_x, pos_y) = match app.primary_monitor() {
        Ok(Some(monitor)) => {
            let size = monitor.size();
            let scale = monitor.scale_factor();
            let x = ((size.width as f64) / scale) - 340.0;
            (x, 20.0)
        }
        _ => (100.0, 20.0),
    };

    let builder = WebviewWindowBuilder::new(
        app,
        &id,
        WebviewUrl::App("toast.html".into())
    )
    .title("")
    .inner_size(320.0, 100.0)
    .position(pos_x, pos_y)
    .decorations(false)
    .shadow(false)
    .transparent(true)
    .always_on_top(true)
    .resizable(false)
    .skip_taskbar(true)
    .focused(false)
    .visible(true);

    let mode = mode_str.to_string();
    let game = game_name.to_string();
    let fname = file_name.to_string();
    let thumb = thumb_b64.to_string();

    if let Ok(win) = builder.build() {
        let eval_script = format!(
            "__setToastData('{}','{}','{}','{}')",
            escape_js_string(&mode),
            escape_js_string(&game),
            escape_js_string(&fname),
            escape_js_string(&thumb),
        );

        let win_for_eval = win.clone();
        let app_for_close = app.clone();
        let win_id = id.clone();

        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            let _ = win_for_eval.eval(&eval_script);

            std::thread::sleep(Duration::from_millis(duration_ms + 600));
            let app_for_find = app_for_close.clone();
            let _ = app_for_close.run_on_main_thread(move || {
                if let Some(w) = app_for_find.get_webview_window(&win_id) {
                    let _ = w.close();
                }
            });
        });
    }
}
