use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

static TOAST_SEQ: AtomicU32 = AtomicU32::new(0);
const STATUS_TOAST_LABEL: &str = "status-toast";

fn toast_position(app: &AppHandle) -> (f64, f64) {
    match app.primary_monitor() {
        Ok(Some(monitor)) => {
            let size = monitor.size();
            let scale = monitor.scale_factor();
            let x = ((size.width as f64) / scale) - 340.0;
            (x, 20.0)
        }
        _ => (100.0, 20.0),
    }
}

fn build_status_toast(app: &AppHandle) -> Result<tauri::WebviewWindow, tauri::Error> {
    let (pos_x, pos_y) = toast_position(app);

    WebviewWindowBuilder::new(
        app,
        STATUS_TOAST_LABEL,
        WebviewUrl::App("toast.html".into()),
    )
    .title("")
    .inner_size(320.0, 80.0)
    .position(pos_x, pos_y)
    .decorations(false)
    .shadow(false)
    .transparent(true)
    .always_on_top(true)
    .resizable(false)
    .skip_taskbar(true)
    .focused(false)
    .visible(true)
    .build()
}

fn get_status_toast(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(win) = app.get_webview_window(STATUS_TOAST_LABEL) {
        return Some(win);
    }
    build_status_toast(app).ok()
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

    show_toast(
        app,
        mode_str,
        game_name,
        &file_name,
        &thumb_b64,
        duration_ms,
        "completed",
    );
}

/// Shows an error toast with the raw Rust error message so failures (window
/// lost, encode timeout, save failure, disk full, ...) are visible to the user.
pub fn show_error_toast(app: &AppHandle, message: &str) {
    show_toast(app, "ERROR", "CAPTURE FAILED", message, "", 5000, "error");
}

/// Shows the REC state in the same reusable toast. It stays visible until the
/// capture finishes and the completion/error state replaces it in-place.
pub fn show_rec_toast(app: &AppHandle, mode_str: &str) {
    show_toast(app, mode_str, "", "", "", 120_000, "rec");
}

/// Image mode: give instant feedback by showing the completion-style toast
/// right away; the real completion toast (with the file name) replaces it.
pub fn show_immediate_complete_toast(app: &AppHandle, mode_str: &str) {
    show_toast(app, mode_str, "", "", "", 120_000, "completed");
}

fn show_toast(
    app: &AppHandle,
    mode_str: &str,
    game_name: &str,
    file_name: &str,
    thumb_b64: &str,
    duration_ms: u64,
    status: &str,
) {
    let Some(win) = get_status_toast(app) else {
        return;
    };
    let seq = TOAST_SEQ.fetch_add(1, Ordering::SeqCst).wrapping_add(1);

    let mode = mode_str.to_string();
    let game = game_name.to_string();
    let fname = file_name.to_string();
    let thumb = thumb_b64.to_string();
    let status = status.to_string();
    let lang = app
        .state::<crate::AppState>()
        .config
        .lock()
        .map(|c| c.language.clone())
        .unwrap_or_else(|_| "en".to_string());
    let (win_alpha, ui_opacity) = app
        .state::<crate::AppState>()
        .config
        .lock()
        .map(|c| {
            (
                1.0 - c.window_transparency as f64 / 100.0,
                1.0 - c.ui_transparency as f64 / 100.0,
            )
        })
        .unwrap_or((1.0, 1.0));

    let eval_script = format!(
        "__setToastData('{}','{}','{}','{}','{}','{}','{:.3}','{:.3}')",
        escape_js_string(&mode),
        escape_js_string(&game),
        escape_js_string(&fname),
        escape_js_string(&thumb),
        escape_js_string(&lang),
        escape_js_string(&status),
        win_alpha,
        ui_opacity,
    );

    let win_for_eval = win.clone();
    let _ = win.show();
    // Give a freshly created WebView time to finish loading before injecting
    // the toast data. Reusing the window makes later toasts just as fast.
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        let _ = win_for_eval.eval(&eval_script);
    });

    let app_for_close = app.clone();
    let win_label = STATUS_TOAST_LABEL.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(duration_ms + 600));
        // A newer toast (e.g. REC -> completed) supersedes this timer.
        if TOAST_SEQ.load(Ordering::SeqCst) != seq {
            return;
        }
        let app_for_hide = app_for_close.clone();
        let _ = app_for_close.run_on_main_thread(move || {
            if let Some(w) = app_for_hide.get_webview_window(&win_label) {
                let _ = w.hide();
            }
        });
    });
}
