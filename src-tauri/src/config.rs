use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use dirs::config_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_capture_target")]
    pub capture_target: String,
    #[serde(default = "default_true", alias = "hide_capture")]
    pub hide_on_capture: bool,
    #[serde(default = "default_image_format")]
    pub image_format: String,
    #[serde(default = "default_quality")]
    pub jpeg_quality: u8,
    #[serde(default = "default_vid_dur")]
    pub video_duration: u32,
    #[serde(default = "default_vid_fps")]
    pub video_fps: u32,
    #[serde(default = "default_video_format")]
    pub video_format: String,
    #[serde(default = "default_mot_dur")]
    pub motion_duration: u32,
    #[serde(default = "default_mot_fps")]
    pub motion_fps: u32,
    #[serde(default = "default_motion_format")]
    pub motion_format: String,
    #[serde(default = "default_save_dir")]
    pub save_dir: String,
    #[serde(default = "default_hotkey_image")]
    pub hotkey_image: String,
    #[serde(default = "default_hotkey_video")]
    pub hotkey_video: String,
    #[serde(default = "default_hotkey_motion")]
    pub hotkey_motion: String,
    #[serde(default = "default_true", alias = "sound_on")]
    pub sound_enabled: bool,
    #[serde(default = "default_toast_dur")]
    pub toast_duration: u64,
    #[serde(default = "default_thumb_size")]
    pub thumbnail_size: u32,
    #[serde(default = "default_false", alias = "auto_open")]
    #[serde(alias = "auto_open_folder")]
    pub auto_open_folder: bool,
    #[serde(default = "default_false", alias = "start_tray")]
    #[serde(alias = "start_to_tray")]
    pub start_minimized: bool,
    #[serde(default = "default_rec_position", alias = "rec_corner")]
    pub rec_position: String,
    #[serde(default = "default_filename_prefix", alias = "file_prefix")]
    pub filename_prefix: String,
    #[serde(default = "default_true", alias = "toast_on")]
    pub show_toast: bool,
    #[serde(default = "default_close_behavior")]
    pub close_to_tray: bool,
    #[serde(default = "default_false")]
    pub save_thumbnail: bool,
    #[serde(default = "default_true", alias = "record_audio")]
    pub record_system_audio: bool,
    #[serde(default = "default_audio_bitrate")]
    pub audio_bitrate: u32,
    #[serde(default = "default_ui_opacity")]
    pub ui_opacity: u32,
}

fn default_mode() -> String { "image".to_string() }
fn default_capture_target() -> String { "auto".to_string() }
fn default_image_format() -> String { "png".to_string() }
fn default_quality() -> u8 { 90 }
fn default_vid_dur() -> u32 { 3 }
fn default_vid_fps() -> u32 { 30 }
fn default_video_format() -> String { "mp4".to_string() }
fn default_mot_dur() -> u32 { 3 }
fn default_mot_fps() -> u32 { 15 }
fn default_motion_format() -> String { "gif".to_string() }
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_toast_dur() -> u64 { 2500 }
fn default_thumb_size() -> u32 { 128 }
fn default_rec_position() -> String { "top-left".to_string() }
fn default_filename_prefix() -> String { "snap".to_string() }
fn default_close_behavior() -> bool { true }
fn default_audio_bitrate() -> u32 { 192_000 }
fn default_ui_opacity() -> u32 { 100 }
fn default_hotkey_image() -> String { "CommandOrControl+Shift+S".to_string() }
fn default_hotkey_video() -> String { "CommandOrControl+Shift+V".to_string() }
fn default_hotkey_motion() -> String { "CommandOrControl+Shift+M".to_string() }
pub fn default_save_dir() -> String {
    if let Some(pic) = dirs::picture_dir() {
        pic.join("PixelSnap").to_string_lossy().to_string()
    } else if let Some(home) = dirs::home_dir() {
        home.join("Pictures").join("PixelSnap").to_string_lossy().to_string()
    } else {
        "C:\\PixelSnap".to_string()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            capture_target: default_capture_target(),
            hide_on_capture: default_true(),
            image_format: default_image_format(),
            jpeg_quality: default_quality(),
            video_duration: default_vid_dur(),
            video_fps: default_vid_fps(),
            video_format: default_video_format(),
            motion_duration: default_mot_dur(),
            motion_fps: default_mot_fps(),
            motion_format: default_motion_format(),
            save_dir: default_save_dir(),
            hotkey_image: default_hotkey_image(),
            hotkey_video: default_hotkey_video(),
            hotkey_motion: default_hotkey_motion(),
            sound_enabled: default_true(),
            toast_duration: default_toast_dur(),
            thumbnail_size: default_thumb_size(),
            auto_open_folder: default_false(),
            start_minimized: default_false(),
            rec_position: default_rec_position(),
            filename_prefix: default_filename_prefix(),
            show_toast: default_true(),
            close_to_tray: default_close_behavior(),
            save_thumbnail: default_false(),
            record_system_audio: default_true(),
            audio_bitrate: default_audio_bitrate(),
            ui_opacity: default_ui_opacity(),
        }
    }
}

impl AppConfig {
    fn config_path() -> Option<PathBuf> {
        config_dir().map(|p| p.join("PixelSnap").join("config.json"))
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::config_path().ok_or("No config dir")?;
        let mut cfg = if path.exists() {
            let content = fs::read_to_string(&path)?;
            match serde_json::from_str::<AppConfig>(&content) {
                Ok(cfg) => cfg,
                Err(_) => Self::default(),
            }
        } else {
            Self::default()
        };
        // Normalize after loading
        if cfg.save_dir.trim().is_empty() {
            cfg.save_dir = default_save_dir();
        }
        cfg.image_format = match cfg.image_format.to_lowercase().as_str() {
            "jpg" | "jpeg" => "jpg".to_string(),
            _ => "png".to_string(),
        };
        cfg.jpeg_quality = cfg.jpeg_quality.clamp(1, 100);
        Ok(cfg)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path().ok_or("No config dir")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn reset() -> Result<Self, Box<dyn std::error::Error>> {
        let cfg = Self::default();
        cfg.save()?;
        Ok(cfg)
    }
}
