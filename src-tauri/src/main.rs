#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod config;
mod toast;
mod audio;

use std::path::PathBuf;
use std::sync::{Mutex, Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Instant, Duration};
use tauri::{Manager, Emitter, Position, PhysicalPosition};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use config::AppConfig;

static CAPTURE_LOCK: AtomicBool = AtomicBool::new(false);
static LAST_CAPTURE_TIME: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);

struct AppState {
    config: Arc<Mutex<AppConfig>>,
    tray_icon: Mutex<Option<tauri::tray::TrayIcon>>,
}

#[tauri::command]
fn load_config(state: tauri::State<AppState>) -> Result<AppConfig, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    Ok(cfg.clone())
}

#[tauri::command]
fn save_config(app: tauri::AppHandle, state: tauri::State<AppState>, mut config: AppConfig) -> Result<(), String> {
    // Validate save_dir: if empty, use default
    if config.save_dir.trim().is_empty() {
        config.save_dir = crate::config::default_save_dir();
    }
    // Normalize image_format
    config.image_format = match config.image_format.to_lowercase().as_str() {
        "jpg" | "jpeg" => "jpg".to_string(),
        _ => "png".to_string(),
    };
    // Clamp jpeg_quality to valid range
    config.jpeg_quality = config.jpeg_quality.clamp(1, 100);

    let (old_hk_img, old_hk_vid, old_hk_mot) = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        (cfg.hotkey_image.clone(), cfg.hotkey_video.clone(), cfg.hotkey_motion.clone())
    };

    let new_hk_img = config.hotkey_image.clone();
    let new_hk_vid = config.hotkey_video.clone();
    let new_hk_mot = config.hotkey_motion.clone();

    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        *cfg = config.clone();
        cfg.save().map_err(|e| e.to_string())?;
    }

    let need_reregister = old_hk_img != new_hk_img || old_hk_vid != new_hk_vid || old_hk_mot != new_hk_mot;
    if need_reregister {
        let _ = app.global_shortcut().unregister_all();
        register_all_shortcuts(&app);
    }

    Ok(())
}

#[tauri::command]
fn reset_config(app: tauri::AppHandle, state: tauri::State<AppState>) -> Result<AppConfig, String> {
    let cfg = AppConfig::reset().map_err(|e| e.to_string())?;
    {
        let mut state_cfg = state.config.lock().map_err(|e| e.to_string())?;
        *state_cfg = cfg.clone();
    }
    let _ = app.global_shortcut().unregister_all();
    register_all_shortcuts(&app);
    Ok(cfg)
}

fn do_take_screenshot(app: &tauri::AppHandle, mode: Option<String>, hide_window: bool) -> Result<String, String> {
    if CAPTURE_LOCK.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err("Already capturing".to_string());
    }

    {
        let mut last = LAST_CAPTURE_TIME.lock().unwrap();
        let now = Instant::now();
        if let Some(t) = *last {
            if now.duration_since(t).as_millis() < 500 {
                CAPTURE_LOCK.store(false, Ordering::SeqCst);
                return Err("Cooldown active".to_string());
            }
        }
        *last = Some(now);
    }

    let cfg = app.state::<AppState>().config.lock().map_err(|e| e.to_string())?.clone();
    let capture_mode = mode.unwrap_or_else(|| cfg.mode.clone());

    if hide_window && cfg.hide_on_capture {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
        std::thread::sleep(Duration::from_millis(150));
    }

    let config_arc = app.state::<AppState>().config.clone();
    let app_owned = app.clone();
    let completed = Arc::new(AtomicBool::new(false));

    capture::start_capture(&cfg, Some(&capture_mode), move |event| {
        let a = app_owned.clone();
        let cfg_arc = config_arc.clone();
        let comp = completed.clone();
        let for_thread = a.clone();
        let _ = a.run_on_main_thread(move || {
            let a_inner = for_thread;
            let main_window = a_inner.get_webview_window("main");
            let rec_window = a_inner.get_webview_window("rec-indicator");

            match event {
                capture::CaptureEvent::Started(is_dynamic) => {
                    if let Some(w) = &main_window {
                        let _ = w.emit("capture-started", is_dynamic);
                    }
                    if is_dynamic {
                        let cfg_now = cfg_arc.lock().unwrap();
                        position_rec_window(&a_inner, &cfg_now);
                        drop(cfg_now);
                        if let Some(rw) = &rec_window {
                            let _ = rw.show();
                        }
                    }
                }
                capture::CaptureEvent::CaptureComplete(preview) => {
                    if comp.swap(true, Ordering::SeqCst) {
                        return;
                    }
                    if let Some(w) = &main_window {
                        let _ = w.show();
                        let _ = w.emit("capture-completed", ());
                    }
                    if let Some(rw) = &rec_window {
                        let _ = rw.hide();
                    }

                    let (play_sound, do_toast, auto_open) = {
                        let c = cfg_arc.lock().unwrap();
                        (c.sound_enabled, c.show_toast, c.auto_open_folder)
                    };

                    if play_sound {
                        play_shutter_sound();
                    }
                    if do_toast {
                        let toast_dur = {
                            let c = cfg_arc.lock().unwrap();
                            c.toast_duration
                        };
                        toast::show_toast_on_main(&a_inner, &preview.thumbnail_data, &preview.game_name, &preview.saved_path, preview.mode_label, toast_dur);
                    }
                    if auto_open {
                        let folder = PathBuf::from(&preview.saved_path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| preview.saved_path.clone());
                        let _ = std::process::Command::new("explorer").arg(&folder).spawn();
                    }
                }
                capture::CaptureEvent::SaveComplete => {
                    CAPTURE_LOCK.store(false, Ordering::SeqCst);
                }
                capture::CaptureEvent::Error(_e) => {
                    if comp.swap(true, Ordering::SeqCst) {
                        return;
                    }
                    CAPTURE_LOCK.store(false, Ordering::SeqCst);
                    if let Some(w) = &main_window {
                        let _ = w.show();
                        let _ = w.emit("capture-error", _e.clone());
                    }
                    if let Some(rw) = &rec_window {
                        let _ = rw.hide();
                    }
                }
            }
        });
    });

    Ok("started".to_string())
}

#[tauri::command]
fn take_screenshot(app: tauri::AppHandle, mode: Option<String>, hide_window: Option<bool>) -> Result<String, String> {
    do_take_screenshot(&app, mode, hide_window.unwrap_or(true))
}

#[tauri::command]
fn minimize_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        w.minimize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn close_window(app: tauri::AppHandle) -> Result<(), String> {
    let close_to_tray = {
        let state = app.state::<AppState>();
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.close_to_tray
    };
    if close_to_tray {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
    } else {
        app.exit(0);
    }
    Ok(())
}

#[tauri::command]
fn open_save_folder(state: tauri::State<AppState>) -> Result<(), String> {
    let dir = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.save_dir.clone()
    };
    let path_buf = PathBuf::from(&dir);
    let target = if path_buf.exists() {
        path_buf
    } else {
        path_buf.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("C:\\"))
    };
    let _ = std::process::Command::new("explorer").arg(&target).spawn();
    Ok(())
}

#[tauri::command]
fn browse_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog().file().pick_folder(move |path| {
        let _ = tx.send(path.map(|p| p.to_string()));
    });
    rx.recv().map_err(|e| e.to_string())
}

fn play_shutter_sound() {
    std::thread::spawn(|| {
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC};
        let sound_name: Vec<u16> = "SystemExclamation\0".encode_utf16().collect();
        unsafe {
            let _ = PlaySoundW(windows::core::PCWSTR(sound_name.as_ptr()), None, SND_ALIAS | SND_ASYNC);
        }
    });
}

type Shortcut = tauri_plugin_global_shortcut::Shortcut;

fn parse_hotkey(s: &str) -> Option<Shortcut> {
    match s.parse::<Shortcut>() {
        Ok(accel) => Some(accel),
        Err(e) => {
            eprintln!("Failed to parse hotkey '{}': {:?}", s, e);
            None
        }
    }
}

fn register_all_shortcuts(app: &tauri::AppHandle) {
    let (hk_img, hk_vid, hk_mot) = {
        let state = app.state::<AppState>();
        let cfg = state.config.lock().unwrap();
        (cfg.hotkey_image.clone(), cfg.hotkey_video.clone(), cfg.hotkey_motion.clone())
    };

    let gs = app.global_shortcut();
    for (hk_str, mode_label) in [(&hk_img, "image"), (&hk_vid, "video"), (&hk_mot, "motion")] {
        if let Some(accel) = parse_hotkey(hk_str) {
            match gs.register(accel) {
                Ok(_) => eprintln!("Registered {} hotkey: {}", mode_label, hk_str),
                Err(e) => eprintln!("Failed to register {} hotkey '{}': {:?}", mode_label, hk_str, e),
            }
        }
    }
}

fn position_rec_window(app: &tauri::AppHandle, cfg: &AppConfig) {
    if let Some(rw) = app.get_webview_window("rec-indicator") {
        let w = 200.0_f64;
        let h = 56.0_f64;
        let margin = 12.0_f64;
        if let Ok(Some(monitor)) = app.primary_monitor() {
            let size = monitor.size();
            let sf = monitor.scale_factor();
            let scr_w = (size.width as f64) / sf;
            let scr_h = (size.height as f64) / sf;
            let (x, y) = match cfg.rec_position.as_str() {
                "top-right" => (scr_w - w - margin, margin),
                "bottom-left" => (margin, scr_h - h - margin - 40.0),
                "bottom-right" => (scr_w - w - margin, scr_h - h - margin - 40.0),
                _ => (margin, margin),
            };
            let _ = rw.set_position(Position::Physical(PhysicalPosition {
                x: (x * sf) as i32,
                y: (y * sf) as i32,
            }));
        }
    }
}

fn create_rec_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    let rec_window = WebviewWindowBuilder::new(app, "rec-indicator", WebviewUrl::App("rec.html".into()))
        .inner_size(200.0, 56.0)
        .position(0.0, 0.0)
        .decorations(false)
        .shadow(false)
        .transparent(true)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .visible(false)
        .build()?;

    // Exclude REC indicator from screen capture so it doesn't appear in recordings,
    // but remains visible to the user (WDA_EXCLUDEFROMCAPTURE, Windows 10 2004+).
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE};
        if let Ok(hwnd_val) = rec_window.hwnd() {
            // tauri uses windows 0.61, our code uses windows 0.62.
            // HWND is #[repr(transparent)] around *mut c_void in both versions, so transmute is safe.
            let hwnd: windows::Win32::Foundation::HWND = unsafe { std::mem::transmute(hwnd_val) };
            unsafe {
                let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
            }
        }
    }

    Ok(())
}

fn generate_tray_icon() -> tauri::image::Image<'static> {
    #[derive(Clone, Copy)]
    struct Color(u8, u8, u8, u8);
    const MINT: Color = Color(127, 255, 212, 255);
    const MINT_LIGHT: Color = Color(184, 255, 232, 255);
    const MINT_DARK: Color = Color(45, 168, 138, 255);
    const PINK: Color = Color(255, 110, 180, 255);
    const PINK_LIGHT: Color = Color(255, 157, 207, 255);
    const PINK_DARK: Color = Color(184, 61, 122, 255);
    const YELLOW: Color = Color(255, 238, 120, 255);
    const YELLOW_LIGHT: Color = Color(255, 246, 176, 255);
    const YELLOW_DARK: Color = Color(197, 168, 32, 255);
    const WHITE: Color = Color(255, 255, 255, 255);
    const RED: Color = Color(255, 77, 109, 255);
    const RED_LIGHT: Color = Color(255, 143, 165, 255);

    let w = 32usize;
    let h = 32usize;
    let mut pixels = vec![0u8; w * h * 4];

    fn set(buf: &mut [u8], width: usize, x: usize, y: usize, c: Color) {
        if x < width && y < 32 {
            let idx = (y * width + x) * 4;
            buf[idx] = c.0;
            buf[idx + 1] = c.1;
            buf[idx + 2] = c.2;
            buf[idx + 3] = c.3;
        }
    }

    fn rect(buf: &mut [u8], width: usize, x: usize, y: usize, rw: usize, rh: usize, c: Color) {
        for dy in 0..rh {
            for dx in 0..rw {
                set(buf, width, x + dx, y + dy, c);
            }
        }
    }

    // Draw in exact SVG order (matching hero camera logo)
    rect(&mut pixels, w, 10, 4, 10, 3, MINT);              // viewfinder base
    rect(&mut pixels, w, 10, 4, 10, 1, MINT_LIGHT);        // viewfinder top highlight
    rect(&mut pixels, w, 10, 6, 1, 1, MINT_LIGHT);         // viewfinder left highlight
    rect(&mut pixels, w, 19, 6, 1, 1, MINT_DARK);          // viewfinder right shadow
    rect(&mut pixels, w, 2, 7, 28, 20, MINT);              // body base
    rect(&mut pixels, w, 2, 7, 28, 1, MINT_LIGHT);         // body top highlight
    rect(&mut pixels, w, 2, 26, 28, 1, MINT_DARK);         // body bottom shadow
    rect(&mut pixels, w, 2, 8, 1, 18, MINT_LIGHT);         // body left highlight
    rect(&mut pixels, w, 29, 8, 1, 18, MINT_DARK);         // body right shadow
    rect(&mut pixels, w, 27, 10, 3, 8, MINT);              // grip base
    rect(&mut pixels, w, 27, 10, 1, 1, MINT_LIGHT);        // grip highlight
    rect(&mut pixels, w, 29, 17, 1, 1, MINT_DARK);         // grip shadow
    rect(&mut pixels, w, 9, 11, 14, 12, PINK);             // lens outer base
    rect(&mut pixels, w, 9, 11, 14, 1, PINK_LIGHT);        // lens outer top highlight
    rect(&mut pixels, w, 9, 22, 14, 1, PINK_DARK);         // lens outer bottom shadow
    rect(&mut pixels, w, 9, 12, 1, 10, PINK_LIGHT);        // lens outer left highlight
    rect(&mut pixels, w, 22, 12, 1, 10, PINK_DARK);        // lens outer right shadow
    rect(&mut pixels, w, 12, 14, 8, 6, YELLOW);            // lens inner base
    rect(&mut pixels, w, 12, 14, 8, 1, YELLOW_LIGHT);      // lens inner top highlight
    rect(&mut pixels, w, 12, 19, 8, 1, YELLOW_DARK);       // lens inner bottom shadow
    rect(&mut pixels, w, 12, 15, 1, 4, YELLOW_LIGHT);      // lens inner left highlight
    rect(&mut pixels, w, 19, 15, 1, 4, YELLOW_DARK);       // lens inner right shadow
    rect(&mut pixels, w, 14, 15, 3, 3, WHITE);             // center white highlight
    rect(&mut pixels, w, 24, 12, 2, 2, RED);               // shutter button base
    rect(&mut pixels, w, 24, 12, 1, 1, RED_LIGHT);         // shutter button highlight
    rect(&mut pixels, w, 5, 27, 4, 2, MINT_DARK);          // left foot
    rect(&mut pixels, w, 23, 27, 4, 2, MINT_DARK);         // right foot

    tauri::image::Image::new_owned(pixels, w as u32, h as u32)
}

fn init_dpi_awareness() {
    use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

fn main() {
    init_dpi_awareness();

    let config = AppConfig::load().unwrap_or_default();
    let start_min = config.start_minimized;

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, shortcut, event| {
                if event.state() != ShortcutState::Pressed {
                    return;
                }

                let hk_img;
                let hk_vid;
                let hk_mot;
                {
                    let state = app.state::<AppState>();
                    let cfg_guard = state.config.lock().unwrap();
                    hk_img = cfg_guard.hotkey_image.clone();
                    hk_vid = cfg_guard.hotkey_video.clone();
                    hk_mot = cfg_guard.hotkey_motion.clone();
                }

                let mode = if let Some(accel_img) = parse_hotkey(&hk_img) {
                    if &accel_img == shortcut { Some("image") } else { None }
                } else { None };

                let mode = mode.or_else(|| {
                    if let Some(accel_vid) = parse_hotkey(&hk_vid) {
                        if &accel_vid == shortcut { Some("video") } else { None }
                    } else { None }
                });

                let mode = mode.or_else(|| {
                    if let Some(accel_mot) = parse_hotkey(&hk_mot) {
                        if &accel_mot == shortcut { Some("motion") } else { None }
                    } else { None }
                });

                if let Some(m) = mode {
                    let _ = do_take_screenshot(app, Some(m.to_string()), false);
                }
            })
            .build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            config: Arc::new(Mutex::new(config)),
            tray_icon: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            reset_config,
            take_screenshot,
            minimize_window,
            close_window,
            open_save_folder,
            browse_folder,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            register_all_shortcuts(&app_handle);
            let _ = create_rec_window(&app_handle);

            if start_min {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

            let tray_icon = {
                use tauri::tray::TrayIconBuilder;
                use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};

                let icon = generate_tray_icon();

                let snap_item = MenuItemBuilder::with_id("snap_image", "\u{2605} SNAP! (IMAGE)").build(app)?;
                let snap_video = MenuItemBuilder::with_id("snap_video", "\u{25CF} SNAP VIDEO").build(app)?;
                let snap_motion = MenuItemBuilder::with_id("snap_motion", "\u{25C6} SNAP MOTION").build(app)?;
                let separator1 = PredefinedMenuItem::separator(app)?;
                let show_item = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
                let hide_item = MenuItemBuilder::with_id("hide", "Hide Window").build(app)?;
                let separator2 = PredefinedMenuItem::separator(app)?;
                let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

                let menu = MenuBuilder::new(app)
                    .item(&snap_item)
                    .item(&snap_video)
                    .item(&snap_motion)
                    .item(&separator1)
                    .item(&show_item)
                    .item(&hide_item)
                    .item(&separator2)
                    .item(&quit_item)
                    .build()?;

                let app_tray = app.handle().clone();
                TrayIconBuilder::new()
                    .icon(icon)
                    .tooltip("PixelSnap")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(move |app, event| {
                        match event.id().as_ref() {
                            "snap_image" => {
                                let _ = do_take_screenshot(app, Some("image".to_string()), false);
                            }
                            "snap_video" => {
                                let _ = do_take_screenshot(app, Some("video".to_string()), false);
                            }
                            "snap_motion" => {
                                let _ = do_take_screenshot(app, Some("motion".to_string()), false);
                            }
                            "show" => {
                                if let Some(w) = app.get_webview_window("main") {
                                    let _ = w.show();
                                    let _ = w.unminimize();
                                    let _ = w.set_focus();
                                }
                            }
                            "hide" => {
                                if let Some(w) = app.get_webview_window("main") {
                                    let _ = w.hide();
                                }
                            }
                            "quit" => {
                                for (label, win) in app.webview_windows() {
                                    if label != "main" {
                                        let _ = win.close();
                                    }
                                }
                                if let Some(w) = app.get_webview_window("main") {
                                    let _ = w.close();
                                }
                                app.exit(0);
                            }
                            _ => {}
                        }
                    })
                    .on_tray_icon_event(move |_tray, event| {
                        use tauri::tray::TrayIconEvent;
                        if let TrayIconEvent::DoubleClick { .. } = event {
                            if let Some(w) = app_tray.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.unminimize();
                                let _ = w.set_focus();
                            }
                        }
                    })
                    .build(app)?
            };

            if let Some(w) = app.get_webview_window("main") {
                let app_exit = app.handle().clone();
                let app_close = app.handle().clone();
                w.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let close_to_tray = {
                            let state = app_close.state::<AppState>();
                            let cfg = state.config.lock();
                            match cfg {
                                Ok(c) => c.close_to_tray,
                                Err(_) => true,
                            }
                        };
                        if close_to_tray {
                            api.prevent_close();
                            if let Some(win) = app_close.get_webview_window("main") {
                                let _ = win.hide();
                            }
                        } else {
                            // Close all auxiliary windows and exit the app
                            for (label, win) in app_close.webview_windows() {
                                if label != "main" {
                                    let _ = win.close();
                                }
                            }
                            app_exit.exit(0);
                        }
                    }
                });
            }

            // Store tray icon in app state to ensure proper cleanup on exit
            {
                let state = app.state::<AppState>();
                if let Ok(mut tray_guard) = state.tray_icon.lock() {
                    *tray_guard = Some(tray_icon);
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
