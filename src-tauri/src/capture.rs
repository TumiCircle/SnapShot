use crate::config::AppConfig;
use chrono::Local;
use image::{DynamicImage, RgbaImage, ImageFormat as ImgFmt, codecs::gif::Repeat};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};
use uuid::Uuid;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetForegroundWindow, GetWindow, GetWindowRect, GetWindowTextW,
    GW_OWNER, IsWindowVisible, FindWindowW,
};
use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    encoder::{
        AudioSettingsBuilder, AudioSettingsSubType, ContainerSettingsBuilder, ContainerSettingsSubType,
        VideoEncoder, VideoSettingsBuilder, VideoSettingsSubType,
    },
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
    window::Window,
};

const TICKS_PER_SECOND: i64 = 10_000_000;
const VIDEO_BITRATE: u32 = 8_000_000;
const AUDIO_BUF_CAPACITY: usize = 65536;
const CAPTURE_TIMEOUT_SECS: u64 = 120;
const POLL_INTERVAL_MS: u64 = 20;
const IMAGE_POST_CAPTURE_WAIT_MS: u64 = 150;
const GIF_POST_CAPTURE_WAIT_MS: u64 = 300;
const FIRST_FRAME_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, Clone)]
pub enum CaptureTarget {
    ForegroundWindow,
    MainWindow,
    FullScreen,
}

pub struct CapturePreview {
    pub game_name: String,
    pub thumbnail_data: Option<Vec<u8>>,
    pub saved_path: String,
    pub mode_label: &'static str,
}

pub enum CaptureEvent {
    Started(bool),
    CaptureComplete(CapturePreview),
    SaveComplete,
    Error(String),
}

#[derive(Debug, Clone)]
struct WindowInfo {
    hwnd: HWND,
    window: Window,
    title: String,
    width: i32,
    height: i32,
}

#[derive(Clone)]
struct CaptureParams {
    window: Window,
    thumb_path: PathBuf,
    file_path: PathBuf,
    fps: u32,
    duration_secs: u64,
    thumb_size: u32,
    crop_x: u32,
    crop_y: u32,
    crop_w: u32,
    crop_h: u32,
    save_thumbnail: bool,
    record_audio: bool,
    audio_bitrate: u32,
    result: Arc<Mutex<Option<CaptureResultInternal>>>,
}

#[derive(Clone)]
struct MonitorCaptureParams {
    monitor: Monitor,
    thumb_path: PathBuf,
    file_path: PathBuf,
    fps: u32,
    duration_secs: u64,
    thumb_size: u32,
    width: u32,
    height: u32,
    save_thumbnail: bool,
    record_audio: bool,
    audio_bitrate: u32,
    result: Arc<Mutex<Option<CaptureResultInternal>>>,
}

enum CaptureResultInternal {
    Image { img: RgbaImage, thumbnail: Option<Vec<u8>> },
    Gif { frames: Vec<RgbaImage>, thumbnail: Option<Vec<u8>> },
    VideoDone { thumbnail: Option<Vec<u8>> },
    Error(String),
}

enum SaveData {
    Image { img: RgbaImage, format: String, quality: u8 },
    Gif { frames: Vec<RgbaImage>, fps: u32 },
    Video { file_path: PathBuf },
}

type CaptureResult = Arc<Mutex<Option<CaptureResultInternal>>>;

fn make_result() -> CaptureResult {
    Arc::new(Mutex::new(None))
}

fn is_result_set(result: &CaptureResult) -> bool {
    result.lock().unwrap().is_some()
}

fn set_error(result: &CaptureResult, msg: String) {
    let mut res = result.lock().unwrap();
    if res.is_none() {
        *res = Some(CaptureResultInternal::Error(msg));
    }
}

fn should_exit(result: &CaptureResult, finished: bool, stop_flag: Option<&AtomicBool>) -> bool {
    if finished {
        return true;
    }
    if let Some(flag) = stop_flag {
        if flag.load(Ordering::SeqCst) {
            return true;
        }
    }
    is_result_set(result)
}

fn write_thumbnail(thumb_data: &Option<Vec<u8>>, thumb_path: &PathBuf, save_thumbnail: bool) {
    if save_thumbnail {
        if let Some(data) = thumb_data {
            let _ = fs::write(thumb_path, data);
        }
    }
}

unsafe fn get_window_title(hwnd: HWND) -> String {
    let mut title = vec![0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut title) };
    if len == 0 {
        return "Unknown".to_string();
    }
    String::from_utf16_lossy(&title[..len as usize]).trim().to_string()
}

unsafe fn is_our_window(hwnd: HWND) -> bool {
    let title = unsafe { get_window_title(hwnd) };
    title.contains("PixelSnap") || title.contains("pixelsnap")
}

unsafe fn is_real_top_level_window(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return false;
    }
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return false;
    }
    if unsafe { is_our_window(hwnd) } {
        return false;
    }
    if let Ok(owner) = unsafe { GetWindow(hwnd, GW_OWNER) } {
        if !owner.is_invalid() {
            return false;
        }
    }
    let title = unsafe { get_window_title(hwnd) };
    if title.is_empty() || title == "Program Manager" || title == "Windows Input Experience" {
        return false;
    }
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w < 100 || h < 100 {
            return false;
        }
    }
    let mut client_rect = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut client_rect) }.is_ok() {
        let cw = client_rect.right - client_rect.left;
        let ch = client_rect.bottom - client_rect.top;
        if cw < 50 || ch < 50 {
            return false;
        }
    }
    true
}

unsafe fn get_dwm_client_area_bounds(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
    let mut rect = RECT::default();
    let hr = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
    };
    if hr.is_ok() {
        Some((rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top))
    } else {
        None
    }
}

unsafe fn get_foreground_window_info() -> Option<WindowInfo> {
    let hwnd = unsafe { GetForegroundWindow() };
    if !unsafe { is_real_top_level_window(hwnd) } {
        return None;
    }

    let window = Window::from_raw_hwnd(hwnd.0 as *mut std::ffi::c_void);
    if !window.is_valid() {
        return None;
    }

    let title = window.title().ok()?;
    let safe_title = sanitize_filename(&title);
    if safe_title.is_empty() || safe_title == "Unknown" {
        return None;
    }

    let mut client_rect = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut client_rect) }.is_err() {
        return None;
    }

    let width = client_rect.right - client_rect.left;
    let height = client_rect.bottom - client_rect.top;
    if width <= 0 || height <= 0 {
        return None;
    }

    Some(WindowInfo { hwnd, window, title: safe_title, width, height })
}

fn get_primary_monitor() -> Option<(Monitor, u32, u32)> {
    let monitor = Monitor::primary().ok()?;
    let width = monitor.width().ok()?;
    let height = monitor.height().ok()?;
    Some((monitor, width, height))
}

unsafe fn get_main_window_info() -> Option<WindowInfo> {
    let title_wide: Vec<u16> = "PixelSnap\0".encode_utf16().collect();
    let hwnd = match unsafe { FindWindowW(None, windows::core::PCWSTR(title_wide.as_ptr())) } {
        Ok(h) => h,
        Err(_) => return None,
    };
    if hwnd.is_invalid() {
        return None;
    }

    let window = Window::from_raw_hwnd(hwnd.0 as *mut std::ffi::c_void);
    if !window.is_valid() {
        return None;
    }

    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return None;
    }

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return None;
    }

    Some(WindowInfo {
        hwnd,
        window,
        title: "PixelSnap".to_string(),
        width,
        height,
    })
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn build_filename(prefix: &str, kind: &str, timestamp: &str, unique: &str, ext: &str) -> String {
    let pfx = if prefix.is_empty() { String::new() } else { format!("{}_", prefix) };
    format!("{}{}_{}_{}.{}", pfx, kind, timestamp, unique, ext)
}

fn effective_thumb_size(cfg: &AppConfig) -> u32 {
    cfg.thumbnail_size.clamp(60, 256)
}

fn create_thumbnail(img: &RgbaImage, max_size: u32) -> DynamicImage {
    let dyn_img = DynamicImage::ImageRgba8(img.clone());
    let (w, h) = img.dimensions();
    let ratio = (max_size as f32) / (w.max(h) as f32);
    let new_w = ((w as f32 * ratio) as u32).max(1);
    let new_h = ((h as f32 * ratio) as u32).max(1);
    dyn_img.resize(new_w, new_h, image::imageops::FilterType::Nearest)
}

fn bgra_to_rgba(raw: &[u8], w: u32, h: u32) -> Option<RgbaImage> {
    let expected_len = (w * h * 4) as usize;
    if raw.len() < expected_len {
        return None;
    }
    let mut rgba = Vec::with_capacity(expected_len);
    for chunk in raw[..expected_len].chunks_exact(4) {
        rgba.push(chunk[2]);
        rgba.push(chunk[1]);
        rgba.push(chunk[0]);
        rgba.push(chunk[3]);
    }
    RgbaImage::from_raw(w, h, rgba)
}

/// Flip BGRA buffer vertically (top-down -> bottom-up).
/// Windows Media Foundation expects bottom-up layout for raw BGRA buffers (send_frame_buffer path).
fn flip_bgra_vertical(raw: &[u8], w: u32, h: u32) -> Vec<u8> {
    let stride = (w * 4) as usize;
    let mut flipped = Vec::with_capacity(raw.len().min(stride * h as usize));
    for row in (0..h as usize).rev() {
        let start = row * stride;
        let end = start + stride;
        flipped.extend_from_slice(&raw[start..end.min(raw.len())]);
    }
    flipped
}

fn encode_thumbnail_png(img: &RgbaImage, max_size: u32) -> Option<Vec<u8>> {
    let thumb = create_thumbnail(img, max_size);
    let mut buf = Vec::new();
    if thumb.write_to(&mut Cursor::new(&mut buf), ImgFmt::Png).is_ok() {
        Some(buf)
    } else {
        None
    }
}

fn get_raw_bgra_frame(frame: &mut Frame, crop_x: u32, crop_y: u32, crop_w: u32, crop_h: u32) -> Option<(Vec<u8>, u32, u32)> {
    let mut raw_buf = Vec::new();
    let buffer = if crop_x > 0 || crop_y > 0 {
        frame.buffer_crop(crop_x, crop_y, crop_x + crop_w, crop_y + crop_h).ok()?
    } else {
        frame.buffer().ok()?
    };
    let raw_data = buffer.as_nopadding_buffer(&mut raw_buf);
    Some((raw_data.to_vec(), buffer.width(), buffer.height()))
}

fn process_frame_to_rgba(
    frame: &mut Frame,
    crop_x: u32, crop_y: u32, crop_w: u32, crop_h: u32,
) -> Option<(RgbaImage, u32, u32)> {
    let (raw_bgra, w, h) = get_raw_bgra_frame(frame, crop_x, crop_y, crop_w, crop_h)?;
    // Windows Graphics Capture returns top-down Bgra8 buffer; no vertical flip needed.
    let rgba = bgra_to_rgba(&raw_bgra, w, h)?;
    Some((rgba, w, h))
}

// ============ Window-based capture handlers ============

fn run_image_capture(params: CaptureParams) -> Result<CaptureResultInternal, String> {
    struct Handler {
        params: CaptureParams,
    }

    impl GraphicsCaptureApiHandler for Handler {
        type Flags = CaptureParams;
        type Error = Box<dyn std::error::Error + Send + Sync>;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Self { params: ctx.flags })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            if is_result_set(&self.params.result) {
                return Ok(());
            }

            let p = &self.params;
            match process_frame_to_rgba(frame, p.crop_x, p.crop_y, p.crop_w, p.crop_h) {
                Some((img, _, _)) => {
                    let thumb_data = encode_thumbnail_png(&img, p.thumb_size);
                    write_thumbnail(&thumb_data, &p.thumb_path, p.save_thumbnail);
                    *p.result.lock().unwrap() = Some(CaptureResultInternal::Image { img, thumbnail: thumb_data });
                }
                None => {
                    set_error(&p.result, "Failed to get/convert frame buffer".to_string());
                }
            }

            capture_control.stop();
            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            set_error(&self.params.result, "Window closed during capture".to_string());
            Ok(())
        }
    }

    let result = params.result.clone();
    let window = params.window.clone();
    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        params,
    );
    let _ctrl = Handler::start_free_threaded(settings)
        .map_err(|e| format!("Failed to start capture: {:?}", e))?;

    loop {
        {
            let res = result.lock().unwrap();
            if let Some(r) = res.as_ref() {
                match r {
                    CaptureResultInternal::Error(e) => return Err(e.clone()),
                    _ => break,
                }
            }
        }
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }
    std::thread::sleep(Duration::from_millis(IMAGE_POST_CAPTURE_WAIT_MS));

    let mut res = result.lock().unwrap();
    res.take().ok_or_else(|| "Capture finished without result".to_string())
}

// ============ Shared video encoding thread ============

struct VideoShared {
    latest_frame: StdMutex<Option<Vec<u8>>>,
    thumbnail: StdMutex<Option<Vec<u8>>>,
    first_frame_ready: AtomicBool,
    stop_requested: Arc<AtomicBool>,
    result: CaptureResult,
}

fn spawn_video_encoding_thread(
    mut encoder: VideoEncoder,
    shared: Arc<VideoShared>,
    audio_format: Option<crate::audio::AudioFormat>,
    record_audio: bool,
    audio_bitrate: u32,
    fps: u32,
    duration_secs: u64,
    thumb_path: PathBuf,
    save_thumbnail: bool,
    file_path: PathBuf,
) -> std::thread::JoinHandle<()> {
    let frame_interval_ticks = TICKS_PER_SECOND / fps.max(1) as i64;
    let duration_ticks = duration_secs as i64 * TICKS_PER_SECOND;

    std::thread::spawn(move || {
        // Wait for first frame
        while !shared.first_frame_ready.load(Ordering::SeqCst) {
            if shared.stop_requested.load(Ordering::SeqCst) {
                let _ = encoder.finish();
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }

        let start_instant = Instant::now();
        eprintln!("[VIDEO] Encoding thread started at wall-clock, fps={}, dur={}s", fps, duration_secs);

        // Start audio capture
        let mut audio_capture: Option<crate::audio::AudioCapture> = None;
        let audio_rx: Option<crossbeam_channel::Receiver<Vec<u8>>> = if record_audio {
            match crate::audio::AudioCapture::start() {
                Ok((cap, rx)) => {
                    eprintln!("[AUDIO] Capture started");
                    audio_capture = Some(cap);
                    Some(rx)
                }
                Err(e) => {
                    eprintln!("[AUDIO] Failed to start: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let mut audio_buf: Vec<u8> = Vec::with_capacity(AUDIO_BUF_CAPACITY);
        let mut last_sent_ticks: i64 = 0;

        // Encoding loop: drive frames at exact wall-clock intervals
        loop {
            if shared.stop_requested.load(Ordering::SeqCst) {
                break;
            }

            let elapsed_ticks = (start_instant.elapsed().as_nanos() / 100) as i64;
            if elapsed_ticks >= duration_ticks {
                break;
            }

            // Send all frames due up to current wall-clock time
            let target_ticks = elapsed_ticks.min(duration_ticks.saturating_sub(1));
            while last_sent_ticks + frame_interval_ticks <= target_ticks {
                last_sent_ticks += frame_interval_ticks;

                // Drain audio
                if let Some(ref rx) = audio_rx {
                    while let Ok(chunk) = rx.try_recv() {
                        audio_buf.extend_from_slice(&chunk);
                    }
                }

                // Send video frame
                {
                    let frame_guard = shared.latest_frame.lock().unwrap();
                    if let Some(ref frame_data) = *frame_guard {
                        let _ = encoder.send_frame_buffer(frame_data, last_sent_ticks);
                    }
                }

                // Flush audio (encoder uses monotonic audio clock; timestamp is ignored)
                if !audio_buf.is_empty() {
                    let _ = encoder.send_audio_buffer(&audio_buf, last_sent_ticks);
                    audio_buf.clear();
                }
            }

            // Sleep a bit to avoid busy-waiting, but wake up frequently enough to hit frame deadlines
            let next_frame_due_ticks = last_sent_ticks + frame_interval_ticks;
            let until_next = (next_frame_due_ticks - elapsed_ticks).max(0);
            let sleep_ms = (until_next as u64 / 10_000).saturating_sub(1).min(5); // wake 1ms early, max 5ms sleep
            std::thread::sleep(Duration::from_millis(if sleep_ms > 0 { sleep_ms } else { 1 }));
        }

        // Final drain of any remaining audio
        if let Some(ref rx) = audio_rx {
            while let Ok(chunk) = rx.try_recv() {
                audio_buf.extend_from_slice(&chunk);
            }
        }
        if !audio_buf.is_empty() {
            let _ = encoder.send_audio_buffer(&audio_buf, duration_ticks);
            audio_buf.clear();
        }

        // Stop audio capture
        audio_capture = None;

        // Finish encoding (blocks until file is finalized)
        eprintln!("[VIDEO] Finalizing encoder, sent {} frames (target ~{} frames)",
            last_sent_ticks / frame_interval_ticks, duration_ticks / frame_interval_ticks);
        let _ = encoder.finish();
        eprintln!("[VIDEO] Encoder finished");

        // Write thumbnail and set result
        let thumb_data = shared.thumbnail.lock().unwrap().clone();
        write_thumbnail(&thumb_data, &thumb_path, save_thumbnail);

        let mut res = shared.result.lock().unwrap();
        if res.is_none() {
            if file_path.exists() {
                *res = Some(CaptureResultInternal::VideoDone { thumbnail: thumb_data });
            } else {
                *res = Some(CaptureResultInternal::Error("Video file was not created".to_string()));
            }
        }
    })
}

fn run_video_capture(params: CaptureParams) -> Result<CaptureResultInternal, String> {
    struct VideoFlags {
        shared: Arc<VideoShared>,
        crop: (u32, u32, u32, u32),
        thumb_size: u32,
    }

    struct VideoHandler {
        shared: Arc<VideoShared>,
        crop: (u32, u32, u32, u32),
        thumb_size: u32,
    }

    impl GraphicsCaptureApiHandler for VideoHandler {
        type Flags = VideoFlags;
        type Error = Box<dyn std::error::Error + Send + Sync>;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            let f = ctx.flags;
            Ok(Self {
                shared: f.shared,
                crop: f.crop,
                thumb_size: f.thumb_size,
            })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            if self.shared.stop_requested.load(Ordering::SeqCst) {
                if is_result_set(&self.shared.result) {
                    capture_control.stop();
                }
                return Ok(());
            }

            let (cx, cy, cw, ch) = self.crop;
            if let Some((raw_bgra, w, h)) = get_raw_bgra_frame(frame, cx, cy, cw, ch) {
                // Thumbnail from first frame
                {
                    let mut thumb_lock = self.shared.thumbnail.lock().unwrap();
                    if thumb_lock.is_none() {
                        if let Some(rgba) = bgra_to_rgba(&raw_bgra, w, h) {
                            *thumb_lock = encode_thumbnail_png(&rgba, self.thumb_size);
                        }
                    }
                }

                // Flip for video encoder (bottom-up BGRA) and cache
                let flipped = flip_bgra_vertical(&raw_bgra, w, h);
                {
                    let mut frame_lock = self.shared.latest_frame.lock().unwrap();
                    *frame_lock = Some(flipped);
                }

                // Signal that first frame is ready
                self.shared.first_frame_ready.store(true, Ordering::SeqCst);
            }

            // Check if we should stop
            if self.shared.stop_requested.load(Ordering::SeqCst) {
                // Give encoding thread a moment to finish before stopping capture
                if is_result_set(&self.shared.result) {
                    capture_control.stop();
                }
            }

            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            // If capture closed before result is set (e.g., window closed), signal stop
            if !is_result_set(&self.shared.result) {
                self.shared.stop_requested.store(true, Ordering::SeqCst);
                // Wait a bit for encoding thread to finish and set result
                let start = Instant::now();
                while !is_result_set(&self.shared.result) && start.elapsed() < Duration::from_millis(500) {
                    std::thread::sleep(Duration::from_millis(10));
                }
                if !is_result_set(&self.shared.result) {
                    set_error(&self.shared.result, "Capture closed unexpectedly".to_string());
                }
            }
            Ok(())
        }
    }

    let result = params.result.clone();
    let stop_requested = Arc::new(AtomicBool::new(false));

    let audio_format = if params.record_audio {
        match crate::audio::AudioCapture::init_format() {
            Ok(fmt) => {
                eprintln!("[AUDIO] Format detected: {}Hz, {}ch", fmt.sample_rate, fmt.channels);
                Some(fmt)
            }
            Err(e) => {
                eprintln!("[AUDIO] Failed to detect format (video-only): {}", e);
                None
            }
        }
    } else {
        None
    };

    let audio_settings = if params.record_audio {
        match audio_format {
            Some(fmt) => AudioSettingsBuilder::new()
                .bitrate(params.audio_bitrate)
                .channel_count(fmt.channels as u32)
                .sample_rate(fmt.sample_rate)
                .bit_per_sample(16)
                .sub_type(AudioSettingsSubType::AAC),
            None => {
                eprintln!("[AUDIO] No audio format available, recording video only");
                AudioSettingsBuilder::new().disabled(true)
            }
        }
    } else {
        AudioSettingsBuilder::new().disabled(true)
    };

    let encoder = VideoEncoder::new(
        VideoSettingsBuilder::new(params.crop_w, params.crop_h)
            .sub_type(VideoSettingsSubType::H264)
            .bitrate(VIDEO_BITRATE)
            .frame_rate(params.fps),
        audio_settings,
        ContainerSettingsBuilder::new().sub_type(ContainerSettingsSubType::MPEG4),
        &params.file_path,
    ).map_err(|e| format!("Failed to create video encoder: {:?}", e))?;

    let shared = Arc::new(VideoShared {
        latest_frame: StdMutex::new(None),
        thumbnail: StdMutex::new(None),
        first_frame_ready: AtomicBool::new(false),
        stop_requested: stop_requested.clone(),
        result: result.clone(),
    });

    // Spawn the encoding thread (owns the encoder)
    let encode_thread = spawn_video_encoding_thread(
        encoder,
        shared.clone(),
        audio_format,
        params.record_audio,
        params.audio_bitrate,
        params.fps,
        params.duration_secs,
        params.thumb_path.clone(),
        params.save_thumbnail,
        params.file_path.clone(),
    );

    let window = params.window.clone();
    let target_fps = params.fps.max(10).min(60);
    let min_interval = Duration::from_micros(1_000_000 / target_fps as u64);
    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Custom(min_interval),
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        VideoFlags {
            shared: shared.clone(),
            crop: (params.crop_x, params.crop_y, params.crop_w, params.crop_h),
            thumb_size: params.thumb_size,
        },
    );

    let ctrl = VideoHandler::start_free_threaded(settings)
        .map_err(|e| format!("Failed to start video capture: {:?}", e))?;

    let duration = Duration::from_secs(params.duration_secs.max(1).min(60));
    let timeout_duration = Duration::from_secs(CAPTURE_TIMEOUT_SECS);
    let wait_start = Instant::now();

    loop {
        {
            let res = result.lock().unwrap();
            if let Some(r) = res.as_ref() {
                match r {
                    CaptureResultInternal::Error(e) => {
                        stop_requested.store(true, Ordering::SeqCst);
                        ctrl.stop().ok();
                        let _ = encode_thread.join();
                        return Err(e.clone());
                    }
                    CaptureResultInternal::VideoDone { .. } => break,
                    _ => {}
                }
            }
        }
        if wait_start.elapsed() >= duration + Duration::from_millis(500) {
            // Duration reached, signal stop and wait for encoding thread to finish
            stop_requested.store(true, Ordering::SeqCst);
            // Give the encoding thread time to finish (it breaks on stop_requested or elapsed >= duration)
            let finish_wait_start = Instant::now();
            while finish_wait_start.elapsed() < Duration::from_secs(5) {
                if is_result_set(&result) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            break;
        }
        if wait_start.elapsed() > timeout_duration {
            stop_requested.store(true, Ordering::SeqCst);
            ctrl.stop().ok();
            let _ = encode_thread.join();
            return Err("Capture timed out".to_string());
        }
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }

    // Ensure stop is signaled and capture is stopped
    stop_requested.store(true, Ordering::SeqCst);
    ctrl.stop().ok();
    let _ = encode_thread.join();
    std::thread::sleep(Duration::from_millis(100));

    let mut res = result.lock().unwrap();
    res.take().ok_or_else(|| "Capture finished without result".to_string())
}

fn run_gif_capture(params: CaptureParams) -> Result<CaptureResultInternal, String> {
    enum GifMsg {
        NewFrame(RgbaImage),
        Stop,
    }

    struct GifHandlerData {
        tx: mpsc::Sender<GifMsg>,
        result: CaptureResult,
        stop_flag: Arc<AtomicBool>,
        thumb_size: u32,
        thumbnail: Mutex<Option<Vec<u8>>>,
        crop: (u32, u32, u32, u32),
    }

    struct GifHandler {
        data: Arc<GifHandlerData>,
    }

    impl GraphicsCaptureApiHandler for GifHandler {
        type Flags = (CaptureParams, Arc<GifHandlerData>);
        type Error = Box<dyn std::error::Error + Send + Sync>;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Self { data: ctx.flags.1 })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            _capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            let d = &self.data;
            if should_exit(&d.result, false, Some(&d.stop_flag)) {
                return Ok(());
            }
            let (cx, cy, cw, ch) = d.crop;
            if let Some((rgba, _, _)) = process_frame_to_rgba(frame, cx, cy, cw, ch) {
                let mut thumb_lock = d.thumbnail.lock().unwrap();
                if thumb_lock.is_none() {
                    *thumb_lock = encode_thumbnail_png(&rgba, d.thumb_size);
                }
                drop(thumb_lock);
                let _ = d.tx.send(GifMsg::NewFrame(rgba));
            }
            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            self.data.stop_flag.store(true, Ordering::SeqCst);
            let _ = self.data.tx.send(GifMsg::Stop);
            Ok(())
        }
    }

    let (tx, rx) = mpsc::channel::<GifMsg>();
    let stop_flag = Arc::new(AtomicBool::new(false));

    let handler_data = Arc::new(GifHandlerData {
        tx: tx.clone(),
        result: params.result.clone(),
        stop_flag: stop_flag.clone(),
        thumb_size: params.thumb_size,
        thumbnail: Mutex::new(None),
        crop: (params.crop_x, params.crop_y, params.crop_w, params.crop_h),
    });

    let result_collect = params.result.clone();
    let stop_collect = stop_flag.clone();
    let thumb_path = params.thumb_path.clone();
    let save_thumbnail = params.save_thumbnail;
    let handler_data_collect = handler_data.clone();
    let fps = params.fps;
    let duration_secs = params.duration_secs;

    let collect_thread = std::thread::spawn(move || {
        let mut last_rgba: Option<RgbaImage> = None;
        let mut frames: Vec<RgbaImage> = Vec::new();
        let mut collect_start: Option<Instant> = None;
        let thread_start = Instant::now();
        let total_frames = duration_secs * fps as u64;
        let nanos_per_frame = 1_000_000_000u64 / fps as u64;
        let first_frame_timeout = Duration::from_secs(FIRST_FRAME_TIMEOUT_SECS);

        for frame_idx in 0..total_frames {
            'inner: loop {
                if stop_collect.load(Ordering::SeqCst) {
                    break 'inner;
                }
                if is_result_set(&result_collect) {
                    break 'inner;
                }

                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        GifMsg::NewFrame(rgba) => {
                            if collect_start.is_none() {
                                collect_start = Some(Instant::now());
                            }
                            last_rgba = Some(rgba);
                        }
                        GifMsg::Stop => stop_collect.store(true, Ordering::SeqCst),
                    }
                }

                if last_rgba.is_none() {
                    if thread_start.elapsed() > first_frame_timeout {
                        set_error(&result_collect, "Timed out waiting for first GIF frame".to_string());
                        break 'inner;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }

                if let Some(frame) = &last_rgba {
                    frames.push(frame.clone());
                    break 'inner;
                }

                std::thread::sleep(Duration::from_millis(1));
            }

            if stop_collect.load(Ordering::SeqCst) {
                break;
            }
            if is_result_set(&result_collect) {
                break;
            }

            if frame_idx < total_frames - 1 {
                if let Some(start) = collect_start {
                    let target = start + Duration::from_nanos((frame_idx as u64 + 1) * nanos_per_frame);
                    if let Some(wait) = target.checked_duration_since(Instant::now()) {
                        std::thread::sleep(wait);
                    }
                }
            }
        }

        let thumb_data = handler_data_collect.thumbnail.lock().unwrap().clone();
        write_thumbnail(&thumb_data, &thumb_path, save_thumbnail);

        let mut res = result_collect.lock().unwrap();
        if res.is_none() {
            if frames.len() >= 2 {
                *res = Some(CaptureResultInternal::Gif { frames, thumbnail: thumb_data });
            } else {
                *res = Some(CaptureResultInternal::Error("Not enough frames captured".to_string()));
            }
        }
        stop_collect.store(true, Ordering::SeqCst);
    });

    let result = params.result.clone();
    let window = params.window.clone();
    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        (params, handler_data),
    );
    let ctrl = GifHandler::start_free_threaded(settings)
        .map_err(|e| format!("Failed to start GIF capture: {:?}", e))?;

    loop {
        {
            let res = result.lock().unwrap();
            if let Some(r) = res.as_ref() {
                match r {
                    CaptureResultInternal::Error(e) => {
                        stop_flag.store(true, Ordering::SeqCst);
                        let _ = tx.send(GifMsg::Stop);
                        ctrl.stop().ok();
                        let _ = collect_thread.join();
                        return Err(e.clone());
                    }
                    CaptureResultInternal::Gif { frames, .. } if frames.len() >= 2 => break,
                    CaptureResultInternal::Gif { .. } => {
                        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                        continue;
                    }
                    _ => break,
                }
            }
        }
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }

    stop_flag.store(true, Ordering::SeqCst);
    let _ = tx.send(GifMsg::Stop);
    ctrl.stop().ok();
    let _ = collect_thread.join();
    std::thread::sleep(Duration::from_millis(GIF_POST_CAPTURE_WAIT_MS));

    let mut res = result.lock().unwrap();
    match res.take() {
        Some(CaptureResultInternal::Gif { frames, thumbnail }) if frames.len() >= 2 => {
            Ok(CaptureResultInternal::Gif { frames, thumbnail })
        }
        Some(CaptureResultInternal::Error(e)) => Err(e),
        _ => Err("GIF capture failed: not enough frames".to_string()),
    }
}

// ============ Monitor-based capture handlers (for fullscreen) ============

fn run_monitor_image_capture(params: MonitorCaptureParams) -> Result<CaptureResultInternal, String> {
    struct Handler {
        params: MonitorCaptureParams,
    }

    impl GraphicsCaptureApiHandler for Handler {
        type Flags = MonitorCaptureParams;
        type Error = Box<dyn std::error::Error + Send + Sync>;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Self { params: ctx.flags })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            if is_result_set(&self.params.result) {
                return Ok(());
            }

            let p = &self.params;
            match process_frame_to_rgba(frame, 0, 0, p.width, p.height) {
                Some((img, _, _)) => {
                    let thumb_data = encode_thumbnail_png(&img, p.thumb_size);
                    write_thumbnail(&thumb_data, &p.thumb_path, p.save_thumbnail);
                    *p.result.lock().unwrap() = Some(CaptureResultInternal::Image { img, thumbnail: thumb_data });
                }
                None => {
                    set_error(&p.result, "Failed to get/convert frame buffer".to_string());
                }
            }

            capture_control.stop();
            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            set_error(&self.params.result, "Capture session closed".to_string());
            Ok(())
        }
    }

    let result = params.result.clone();
    let monitor = params.monitor.clone();
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        params,
    );
    let _ctrl = Handler::start_free_threaded(settings)
        .map_err(|e| format!("Failed to start monitor capture: {:?}", e))?;

    loop {
        {
            let res = result.lock().unwrap();
            if let Some(r) = res.as_ref() {
                match r {
                    CaptureResultInternal::Error(e) => return Err(e.clone()),
                    _ => break,
                }
            }
        }
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }
    std::thread::sleep(Duration::from_millis(IMAGE_POST_CAPTURE_WAIT_MS));

    let mut res = result.lock().unwrap();
    res.take().ok_or_else(|| "Capture finished without result".to_string())
}

fn run_monitor_video_capture(params: MonitorCaptureParams) -> Result<CaptureResultInternal, String> {
    struct MonitorVideoFlags {
        shared: Arc<VideoShared>,
        dims: (u32, u32),
        thumb_size: u32,
    }

    struct MonitorVideoHandler {
        shared: Arc<VideoShared>,
        dims: (u32, u32),
        thumb_size: u32,
    }

    impl GraphicsCaptureApiHandler for MonitorVideoHandler {
        type Flags = MonitorVideoFlags;
        type Error = Box<dyn std::error::Error + Send + Sync>;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            let f = ctx.flags;
            Ok(Self {
                shared: f.shared,
                dims: f.dims,
                thumb_size: f.thumb_size,
            })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            if self.shared.stop_requested.load(Ordering::SeqCst) {
                if is_result_set(&self.shared.result) {
                    capture_control.stop();
                }
                return Ok(());
            }

            let (w, h) = self.dims;
            if let Some((raw_bgra, fw, fh)) = get_raw_bgra_frame(frame, 0, 0, w, h) {
                // Thumbnail from first frame
                {
                    let mut thumb_lock = self.shared.thumbnail.lock().unwrap();
                    if thumb_lock.is_none() {
                        if let Some(rgba) = bgra_to_rgba(&raw_bgra, fw, fh) {
                            *thumb_lock = encode_thumbnail_png(&rgba, self.thumb_size);
                        }
                    }
                }

                // Flip for video encoder (bottom-up BGRA) and cache
                let flipped = flip_bgra_vertical(&raw_bgra, w, h);
                {
                    let mut frame_lock = self.shared.latest_frame.lock().unwrap();
                    *frame_lock = Some(flipped);
                }

                // Signal first frame ready
                self.shared.first_frame_ready.store(true, Ordering::SeqCst);
            }

            if self.shared.stop_requested.load(Ordering::SeqCst) {
                if is_result_set(&self.shared.result) {
                    capture_control.stop();
                }
            }

            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            if !is_result_set(&self.shared.result) {
                self.shared.stop_requested.store(true, Ordering::SeqCst);
                let start = Instant::now();
                while !is_result_set(&self.shared.result) && start.elapsed() < Duration::from_millis(500) {
                    std::thread::sleep(Duration::from_millis(10));
                }
                if !is_result_set(&self.shared.result) {
                    set_error(&self.shared.result, "Capture closed unexpectedly".to_string());
                }
            }
            Ok(())
        }
    }

    let result = params.result.clone();
    let stop_requested = Arc::new(AtomicBool::new(false));

    let audio_format = if params.record_audio {
        match crate::audio::AudioCapture::init_format() {
            Ok(fmt) => {
                eprintln!("[AUDIO] Monitor format: {}Hz, {}ch", fmt.sample_rate, fmt.channels);
                Some(fmt)
            }
            Err(e) => {
                eprintln!("[AUDIO] Monitor failed to detect format: {}", e);
                None
            }
        }
    } else {
        None
    };

    let audio_settings = if params.record_audio {
        match audio_format {
            Some(fmt) => AudioSettingsBuilder::new()
                .bitrate(params.audio_bitrate)
                .channel_count(fmt.channels as u32)
                .sample_rate(fmt.sample_rate)
                .bit_per_sample(16)
                .sub_type(AudioSettingsSubType::AAC),
            None => AudioSettingsBuilder::new().disabled(true),
        }
    } else {
        AudioSettingsBuilder::new().disabled(true)
    };

    let encoder = VideoEncoder::new(
        VideoSettingsBuilder::new(params.width, params.height)
            .sub_type(VideoSettingsSubType::H264)
            .bitrate(VIDEO_BITRATE)
            .frame_rate(params.fps),
        audio_settings,
        ContainerSettingsBuilder::new().sub_type(ContainerSettingsSubType::MPEG4),
        &params.file_path,
    ).map_err(|e| format!("Failed to create video encoder: {:?}", e))?;

    let shared = Arc::new(VideoShared {
        latest_frame: StdMutex::new(None),
        thumbnail: StdMutex::new(None),
        first_frame_ready: AtomicBool::new(false),
        stop_requested: stop_requested.clone(),
        result: result.clone(),
    });

    let encode_thread = spawn_video_encoding_thread(
        encoder,
        shared.clone(),
        audio_format,
        params.record_audio,
        params.audio_bitrate,
        params.fps,
        params.duration_secs,
        params.thumb_path.clone(),
        params.save_thumbnail,
        params.file_path.clone(),
    );

    let monitor = params.monitor.clone();
    let target_fps = params.fps.max(10).min(60);
    let min_interval = Duration::from_micros(1_000_000 / target_fps as u64);
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Custom(min_interval),
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        MonitorVideoFlags {
            shared: shared.clone(),
            dims: (params.width, params.height),
            thumb_size: params.thumb_size,
        },
    );
    let ctrl = MonitorVideoHandler::start_free_threaded(settings)
        .map_err(|e| format!("Failed to start monitor video capture: {:?}", e))?;

    let duration = Duration::from_secs(params.duration_secs.max(1).min(60));
    let timeout_duration = Duration::from_secs(CAPTURE_TIMEOUT_SECS);
    let wait_start = Instant::now();

    loop {
        {
            let res = result.lock().unwrap();
            if let Some(r) = res.as_ref() {
                match r {
                    CaptureResultInternal::Error(e) => {
                        stop_requested.store(true, Ordering::SeqCst);
                        ctrl.stop().ok();
                        let _ = encode_thread.join();
                        return Err(e.clone());
                    }
                    CaptureResultInternal::VideoDone { .. } => break,
                    _ => {}
                }
            }
        }
        if wait_start.elapsed() >= duration + Duration::from_millis(500) {
            stop_requested.store(true, Ordering::SeqCst);
            let finish_wait_start = Instant::now();
            while finish_wait_start.elapsed() < Duration::from_secs(5) {
                if is_result_set(&result) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            break;
        }
        if wait_start.elapsed() > timeout_duration {
            stop_requested.store(true, Ordering::SeqCst);
            ctrl.stop().ok();
            let _ = encode_thread.join();
            return Err("Capture timed out".to_string());
        }
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }

    stop_requested.store(true, Ordering::SeqCst);
    ctrl.stop().ok();
    let _ = encode_thread.join();
    std::thread::sleep(Duration::from_millis(100));

    let mut res = result.lock().unwrap();
    res.take().ok_or_else(|| "Capture finished without result".to_string())
}

fn run_monitor_gif_capture(params: MonitorCaptureParams) -> Result<CaptureResultInternal, String> {
    enum GifMsg {
        NewFrame(RgbaImage),
        Stop,
    }

    struct GifHandlerData {
        tx: mpsc::Sender<GifMsg>,
        result: CaptureResult,
        stop_flag: Arc<AtomicBool>,
        thumb_size: u32,
        thumbnail: Mutex<Option<Vec<u8>>>,
        dims: (u32, u32),
    }

    struct GifHandler {
        data: Arc<GifHandlerData>,
    }

    impl GraphicsCaptureApiHandler for GifHandler {
        type Flags = (MonitorCaptureParams, Arc<GifHandlerData>);
        type Error = Box<dyn std::error::Error + Send + Sync>;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Self { data: ctx.flags.1 })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            _capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            let d = &self.data;
            if should_exit(&d.result, false, Some(&d.stop_flag)) {
                return Ok(());
            }
            let (w, h) = d.dims;
            if let Some((rgba, _, _)) = process_frame_to_rgba(frame, 0, 0, w, h) {
                let mut thumb_lock = d.thumbnail.lock().unwrap();
                if thumb_lock.is_none() {
                    *thumb_lock = encode_thumbnail_png(&rgba, d.thumb_size);
                }
                drop(thumb_lock);
                let _ = d.tx.send(GifMsg::NewFrame(rgba));
            }
            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            self.data.stop_flag.store(true, Ordering::SeqCst);
            let _ = self.data.tx.send(GifMsg::Stop);
            Ok(())
        }
    }

    let (tx, rx) = mpsc::channel::<GifMsg>();
    let stop_flag = Arc::new(AtomicBool::new(false));

    let handler_data = Arc::new(GifHandlerData {
        tx: tx.clone(),
        result: params.result.clone(),
        stop_flag: stop_flag.clone(),
        thumb_size: params.thumb_size,
        thumbnail: Mutex::new(None),
        dims: (params.width, params.height),
    });

    let result_collect = params.result.clone();
    let stop_collect = stop_flag.clone();
    let thumb_path = params.thumb_path.clone();
    let save_thumbnail = params.save_thumbnail;
    let handler_data_collect = handler_data.clone();
    let fps = params.fps;
    let duration_secs = params.duration_secs;

    let collect_thread = std::thread::spawn(move || {
        let mut last_rgba: Option<RgbaImage> = None;
        let mut frames: Vec<RgbaImage> = Vec::new();
        let mut collect_start: Option<Instant> = None;
        let thread_start = Instant::now();
        let total_frames = duration_secs * fps as u64;
        let nanos_per_frame = 1_000_000_000u64 / fps as u64;
        let first_frame_timeout = Duration::from_secs(FIRST_FRAME_TIMEOUT_SECS);

        for frame_idx in 0..total_frames {
            'inner: loop {
                if stop_collect.load(Ordering::SeqCst) {
                    break 'inner;
                }
                if is_result_set(&result_collect) {
                    break 'inner;
                }

                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        GifMsg::NewFrame(rgba) => {
                            if collect_start.is_none() {
                                collect_start = Some(Instant::now());
                            }
                            last_rgba = Some(rgba);
                        }
                        GifMsg::Stop => stop_collect.store(true, Ordering::SeqCst),
                    }
                }

                if last_rgba.is_none() {
                    if thread_start.elapsed() > first_frame_timeout {
                        set_error(&result_collect, "Timed out waiting for first GIF frame".to_string());
                        break 'inner;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }

                if let Some(frame) = &last_rgba {
                    frames.push(frame.clone());
                    break 'inner;
                }

                std::thread::sleep(Duration::from_millis(1));
            }

            if stop_collect.load(Ordering::SeqCst) {
                break;
            }
            if is_result_set(&result_collect) {
                break;
            }

            if frame_idx < total_frames - 1 {
                if let Some(start) = collect_start {
                    let target = start + Duration::from_nanos((frame_idx as u64 + 1) * nanos_per_frame);
                    if let Some(wait) = target.checked_duration_since(Instant::now()) {
                        std::thread::sleep(wait);
                    }
                }
            }
        }

        let thumb_data = handler_data_collect.thumbnail.lock().unwrap().clone();
        write_thumbnail(&thumb_data, &thumb_path, save_thumbnail);

        let mut res = result_collect.lock().unwrap();
        if res.is_none() {
            if frames.len() >= 2 {
                *res = Some(CaptureResultInternal::Gif { frames, thumbnail: thumb_data });
            } else {
                *res = Some(CaptureResultInternal::Error("Not enough frames captured".to_string()));
            }
        }
        stop_collect.store(true, Ordering::SeqCst);
    });

    let result = params.result.clone();
    let monitor = params.monitor.clone();
    let settings = Settings::new(
        monitor,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        (params, handler_data),
    );
    let ctrl = GifHandler::start_free_threaded(settings)
        .map_err(|e| format!("Failed to start monitor GIF capture: {:?}", e))?;

    loop {
        {
            let res = result.lock().unwrap();
            if let Some(r) = res.as_ref() {
                match r {
                    CaptureResultInternal::Error(e) => {
                        stop_flag.store(true, Ordering::SeqCst);
                        let _ = tx.send(GifMsg::Stop);
                        ctrl.stop().ok();
                        let _ = collect_thread.join();
                        return Err(e.clone());
                    }
                    CaptureResultInternal::Gif { frames, .. } if frames.len() >= 2 => break,
                    CaptureResultInternal::Gif { .. } => {
                        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                        continue;
                    }
                    _ => break,
                }
            }
        }
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }

    stop_flag.store(true, Ordering::SeqCst);
    let _ = tx.send(GifMsg::Stop);
    ctrl.stop().ok();
    let _ = collect_thread.join();
    std::thread::sleep(Duration::from_millis(GIF_POST_CAPTURE_WAIT_MS));

    let mut res = result.lock().unwrap();
    match res.take() {
        Some(CaptureResultInternal::Gif { frames, thumbnail }) if frames.len() >= 2 => {
            Ok(CaptureResultInternal::Gif { frames, thumbnail })
        }
        Some(CaptureResultInternal::Error(e)) => Err(e),
        _ => Err("GIF capture failed: not enough frames".to_string()),
    }
}

// ============ Public API ============

unsafe fn is_desktop_window(hwnd: HWND) -> bool {
    let class_name = unsafe { get_window_class(hwnd) };
    class_name == "Progman" || class_name == "WorkerW" || class_name == "Shell_TrayWnd"
}

unsafe fn get_window_class(hwnd: HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
}

unsafe fn auto_detect_target() -> CaptureTarget {
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_invalid() {
        return CaptureTarget::FullScreen;
    }
    if unsafe { is_our_window(fg) } {
        return CaptureTarget::MainWindow;
    }
    if unsafe { is_desktop_window(fg) } {
        return CaptureTarget::FullScreen;
    }
    if !unsafe { is_real_top_level_window(fg) } {
        return CaptureTarget::FullScreen;
    }
    CaptureTarget::ForegroundWindow
}

pub fn start_capture<F: Fn(CaptureEvent) + Send + 'static>(
    cfg: &AppConfig,
    mode_override: Option<&str>,
    on_event: F,
) {
    let cfg = cfg.clone();
    let mode = mode_override.unwrap_or(&cfg.mode).to_string();
    std::thread::spawn(move || {
        let target = unsafe { auto_detect_target() };
        eprintln!("[CAPTURE-DEBUG] Capture thread started for mode: {}, target: {:?}", mode, target);

        enum CaptureSource {
            Window(WindowInfo, String),
            Monitor(Monitor, u32, u32, String),
        }

        let source = unsafe {
            match target {
                CaptureTarget::MainWindow => {
                    match get_main_window_info() {
                        Some(info) => {
                            let title = info.title.clone();
                            CaptureSource::Window(info, title)
                        }
                        None => {
                            on_event(CaptureEvent::Error("Could not find main window".to_string()));
                            return;
                        }
                    }
                }
                CaptureTarget::FullScreen => {
                    match get_primary_monitor() {
                        Some((mon, w, h)) => {
                            CaptureSource::Monitor(mon, w, h, "FullScreen".to_string())
                        }
                        None => {
                            on_event(CaptureEvent::Error("Could not get primary monitor".to_string()));
                            return;
                        }
                    }
                }
                CaptureTarget::ForegroundWindow => {
                    match get_foreground_window_info() {
                        Some(w) => {
                            let title = w.title.clone();
                            CaptureSource::Window(w, title)
                        }
                        None => {
                            on_event(CaptureEvent::Error("No valid foreground window found. Click a window first.".to_string()));
                            return;
                        }
                    }
                }
            }
        };

        let (game_name, is_monitor) = match &source {
            CaptureSource::Window(_info, name) => (name.clone(), false),
            CaptureSource::Monitor(_, _, _, name) => (name.clone(), true),
        };

        let base_dir = PathBuf::from(&cfg.save_dir).join(&game_name);
        if let Err(e) = fs::create_dir_all(&base_dir) {
            on_event(CaptureEvent::Error(format!("Failed to create dir: {}", e)));
            return;
        }

        let thumbs_dir = base_dir.join(".thumbs");
        if cfg.save_thumbnail {
            let _ = fs::create_dir_all(&thumbs_dir);
        }

        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let unique = Uuid::new_v4().to_string()[..8].to_string();

        let thumb_size = effective_thumb_size(&cfg);

        let build_paths = |kind: &str, ext: &str| -> (PathBuf, PathBuf) {
            let file = base_dir.join(build_filename(&cfg.filename_prefix, kind, &timestamp, &unique, ext));
            let thumb = thumbs_dir.join(build_filename(&cfg.filename_prefix, "thumb", &timestamp, &unique, "png"));
            (file, thumb)
        };

        let result = if is_monitor {
            let (monitor, mw, mh) = match source {
                CaptureSource::Monitor(m, w, h, _) => (m, w, h),
                _ => unreachable!(),
            };
            match mode.as_str() {
                "image" => {
                    on_event(CaptureEvent::Started(false));
                    let ext = match cfg.image_format.to_lowercase().as_str() {
                        "jpg" | "jpeg" => "jpg",
                        _ => "png",
                    };
                    let (fp, tp) = build_paths("snap", ext);
                    run_monitor_image_capture(MonitorCaptureParams {
                        monitor,
                        thumb_path: tp,
                        file_path: fp,
                        fps: 0,
                        duration_secs: 0,
                        thumb_size,
                        width: mw,
                        height: mh,
                        save_thumbnail: cfg.save_thumbnail,
                        record_audio: false,
                        audio_bitrate: 0,
                        result: make_result(),
                    })
                }
                "motion" => {
                    on_event(CaptureEvent::Started(true));
                    let fps = cfg.motion_fps.max(5).min(20);
                    let dur = cfg.motion_duration.max(1).min(30) as u64;
                    let (fp, tp) = build_paths("motion", "gif");
                    run_monitor_gif_capture(MonitorCaptureParams {
                        monitor,
                        thumb_path: tp,
                        file_path: fp,
                        fps,
                        duration_secs: dur,
                        thumb_size,
                        width: mw,
                        height: mh,
                        save_thumbnail: cfg.save_thumbnail,
                        record_audio: false,
                        audio_bitrate: 0,
                        result: make_result(),
                    })
                }
                "video" => {
                    on_event(CaptureEvent::Started(true));
                    let fps = cfg.video_fps.max(10).min(60);
                    let dur = cfg.video_duration.max(1).min(60) as u64;
                    let (fp, tp) = build_paths("video", "mp4");
                    run_monitor_video_capture(MonitorCaptureParams {
                        monitor,
                        thumb_path: tp,
                        file_path: fp,
                        fps,
                        duration_secs: dur,
                        thumb_size,
                        width: mw,
                        height: mh,
                        save_thumbnail: cfg.save_thumbnail,
                        record_audio: cfg.record_system_audio,
                        audio_bitrate: cfg.audio_bitrate,
                        result: make_result(),
                    })
                }
                _ => {
                    on_event(CaptureEvent::Error(format!("Unknown mode: {}", mode)));
                    return;
                }
            }
        } else {
            let win_info = match source {
                CaptureSource::Window(w, _) => w,
                _ => unreachable!(),
            };

            let (mut crop_x, mut crop_y, mut crop_w, mut crop_h) = (0u32, 0u32, win_info.width as u32, win_info.height as u32);
            if !matches!(target, CaptureTarget::MainWindow) {
                unsafe {
                    if let Some((_, _, dwm_w, dwm_h)) = get_dwm_client_area_bounds(win_info.hwnd) {
                        if let Ok(win_rect) = win_info.window.rect() {
                            let win_w = (win_rect.right - win_rect.left) as u32;
                            let win_h = (win_rect.bottom - win_rect.top) as u32;
                            let cx = win_w.saturating_sub(dwm_w as u32) / 2;
                            crop_x = cx;
                            crop_y = win_h.saturating_sub(dwm_h as u32) - cx;
                            crop_w = dwm_w as u32;
                            crop_h = dwm_h as u32;
                        }
                    }
                }
            }

            let build_params = |fps: u32, dur: u64, rec_audio: bool, (fp, tp): (PathBuf, PathBuf)| -> CaptureParams {
                CaptureParams {
                    window: win_info.window.clone(),
                    thumb_path: tp,
                    file_path: fp,
                    fps,
                    duration_secs: dur,
                    thumb_size,
                    crop_x, crop_y, crop_w, crop_h,
                    save_thumbnail: cfg.save_thumbnail,
                    record_audio: rec_audio,
                    audio_bitrate: cfg.audio_bitrate,
                    result: make_result(),
                }
            };

            match mode.as_str() {
                "image" => {
                    on_event(CaptureEvent::Started(false));
                    let ext = match cfg.image_format.to_lowercase().as_str() {
                        "jpg" | "jpeg" => "jpg",
                        _ => "png",
                    };
                    run_image_capture(build_params(0, 0, false, build_paths("snap", ext)))
                }
                "motion" => {
                    on_event(CaptureEvent::Started(true));
                    let fps = cfg.motion_fps.max(5).min(20);
                    let dur = cfg.motion_duration.max(1).min(30) as u64;
                    run_gif_capture(build_params(fps, dur, false, build_paths("motion", "gif")))
                }
                "video" => {
                    on_event(CaptureEvent::Started(true));
                    let fps = cfg.video_fps.max(10).min(60);
                    let dur = cfg.video_duration.max(1).min(60) as u64;
                    run_video_capture(build_params(fps, dur, cfg.record_system_audio, build_paths("video", "mp4")))
                }
                _ => {
                    on_event(CaptureEvent::Error(format!("Unknown mode: {}", mode)));
                    return;
                }
            }
        };

        match result {
            Ok(cap_data) => {
                let (mode_label, save_data, final_path, thumb_data) = match cap_data {
                    CaptureResultInternal::Image { img, thumbnail } => {
                        let ext = match cfg.image_format.to_lowercase().as_str() {
                            "jpg" | "jpeg" => "jpg",
                            _ => "png",
                        };
                        eprintln!("[CAPTURE] Image captured, cfg.image_format='{}', ext='{}'", cfg.image_format, ext);
                        let fp = base_dir.join(build_filename(&cfg.filename_prefix, "snap", &timestamp, &unique, ext));
                        ("IMAGE", SaveData::Image {
                            img, format: cfg.image_format.clone(), quality: cfg.jpeg_quality,
                        }, fp, thumbnail)
                    }
                    CaptureResultInternal::Gif { frames, thumbnail } => {
                        let fp = base_dir.join(build_filename(&cfg.filename_prefix, "motion", &timestamp, &unique, "gif"));
                        ("MOTION", SaveData::Gif {
                            frames, fps: cfg.motion_fps.max(5).min(20),
                        }, fp, thumbnail)
                    }
                    CaptureResultInternal::VideoDone { thumbnail } => {
                        let fp = base_dir.join(build_filename(&cfg.filename_prefix, "video", &timestamp, &unique, "mp4"));
                        ("VIDEO", SaveData::Video { file_path: fp.clone() }, fp, thumbnail)
                    }
                    CaptureResultInternal::Error(e) => {
                        on_event(CaptureEvent::Error(e));
                        return;
                    }
                };

                let preview = CapturePreview {
                    game_name: game_name.clone(),
                    thumbnail_data: thumb_data,
                    saved_path: final_path.to_string_lossy().to_string(),
                    mode_label,
                };
                on_event(CaptureEvent::CaptureComplete(preview));

                let save_result = match save_data {
                    SaveData::Image { img, format, quality } => save_image(&img, &final_path, &format, quality),
                    SaveData::Gif { frames, fps } => save_gif_fast(&frames, &final_path, fps),
                    SaveData::Video { file_path } => {
                        if file_path.exists() { Ok(()) }
                        else { Err("Video file was not created".to_string()) }
                    }
                };

                match save_result {
                    Ok(()) => on_event(CaptureEvent::SaveComplete),
                    Err(e) => on_event(CaptureEvent::Error(format!("Save failed: {}", e))),
                }
            }
            Err(e) => on_event(CaptureEvent::Error(e)),
        }
    });
}

fn save_image(img: &RgbaImage, path: &PathBuf, format: &str, quality: u8) -> Result<(), String> {
    let ext_format = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let normalized_format = match (format.to_lowercase().as_str(), ext_format.as_str()) {
        ("jpg" | "jpeg", "jpg" | "jpeg") | (_, "jpg" | "jpeg") => "jpg",
        ("png", "png") | (_, "png") => "png",
        ("jpg" | "jpeg", _) => "jpg",
        _ => "png",
    };

    eprintln!(
        "[SAVE_IMAGE] format='{}' ext='{}' using='{}' path='{}'",
        format, ext_format, normalized_format, path.display()
    );

    let file = fs::File::create(path).map_err(|e| format!("Failed to create file: {}", e))?;
    let mut writer = std::io::BufWriter::new(file);
    match normalized_format {
        "jpg" => {
            let rgb = DynamicImage::ImageRgba8(img.clone()).to_rgb8();
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
            enc.encode(&rgb, rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
                .map_err(|e| format!("JPEG encode error: {}", e))?;
        }
        "png" => {
            use image::ImageEncoder;
            let enc = image::codecs::png::PngEncoder::new(&mut writer);
            enc.write_image(
                img.as_raw(),
                img.width(),
                img.height(),
                image::ExtendedColorType::Rgba8,
            ).map_err(|e| format!("PNG encode error: {}", e))?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn save_gif_fast(frames: &[RgbaImage], path: &PathBuf, fps: u32) -> Result<(), String> {
    use image::Delay;
    use image::codecs::gif::GifEncoder;

    if frames.is_empty() {
        return Err("No frames to encode".to_string());
    }

    let file = fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = GifEncoder::new(file);
    encoder.set_repeat(Repeat::Infinite).map_err(|e| e.to_string())?;
    let frame_delay_ms = (1000 / fps.max(1)) as u32;

    for frame in frames {
        let gif_frame = image::Frame::from_parts(
            frame.clone().into(),
            0, 0,
            Delay::from_numer_denom_ms(frame_delay_ms, 1),
        );
        encoder.encode_frame(gif_frame).map_err(|e| e.to_string())?;
    }
    Ok(())
}
