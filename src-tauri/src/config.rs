use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use dirs::config_dir;

#[derive(Debug, Clone)]
pub struct AppConfig {
    // ---- Mode ----
    pub mode: String,
    pub image_format: String,
    pub jpeg_quality: u8,
    pub video_bitrate: u32,
    pub video_duration: u32,
    pub video_fps: u32,
    pub motion_duration: u32,
    pub motion_fps: u32,
    // ---- Output ----
    pub save_dir: String,
    pub filename_prefix: String,
    pub thumbnail_size: u32,
    pub save_thumbnail: bool,
    // ---- Hotkeys ----
    pub hotkey_image: String,
    pub hotkey_video: String,
    pub hotkey_motion: String,
    // ---- Recording ----
    pub record_system_audio: bool,
    pub audio_bitrate: u32,
    // ---- Appearance ----
    pub window_transparency: u32,
    pub ui_transparency: u32,
    pub starfield_density: u32,
    pub star_twinkle_speed: u32,
    pub meteor_rate: u32,
    pub language: String,
    // ---- Behavior ----
    pub sound_enabled: bool,
    pub toast_duration: u64,
    pub show_toast: bool,
    pub auto_open_folder: bool,
    pub start_minimized: bool,
    pub hide_on_capture: bool,
    pub close_to_tray: bool,
}

fn default_mode() -> String { "image".to_string() }
fn default_image_format() -> String { "png".to_string() }
fn default_quality() -> u8 { 90 }
fn default_vid_dur() -> u32 { 3 }
fn default_vid_fps() -> u32 { 30 }
fn default_video_bitrate() -> u32 { 8_000_000 }
fn default_mot_dur() -> u32 { 3 }
fn default_mot_fps() -> u32 { 15 }
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_toast_dur() -> u64 { 2500 }
fn default_thumb_size() -> u32 { 128 }
fn default_filename_prefix() -> String { "snap".to_string() }
fn default_close_behavior() -> bool { true }
fn default_audio_bitrate() -> u32 { 192_000 }
fn default_window_transparency() -> u32 { 100 }
fn default_ui_transparency() -> u32 { 100 }
fn default_starfield_density() -> u32 { 50 }
fn default_star_twinkle_speed() -> u32 { 50 }
fn default_meteor_rate() -> u32 { 50 }
fn default_language() -> String { "en".to_string() }
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
            image_format: default_image_format(),
            jpeg_quality: default_quality(),
            video_bitrate: default_video_bitrate(),
            video_duration: default_vid_dur(),
            video_fps: default_vid_fps(),
            motion_duration: default_mot_dur(),
            motion_fps: default_mot_fps(),
            save_dir: default_save_dir(),
            filename_prefix: default_filename_prefix(),
            thumbnail_size: default_thumb_size(),
            save_thumbnail: default_false(),
            hotkey_image: default_hotkey_image(),
            hotkey_video: default_hotkey_video(),
            hotkey_motion: default_hotkey_motion(),
            record_system_audio: default_true(),
            audio_bitrate: default_audio_bitrate(),
            window_transparency: default_window_transparency(),
            ui_transparency: default_ui_transparency(),
            starfield_density: default_starfield_density(),
            star_twinkle_speed: default_star_twinkle_speed(),
            meteor_rate: default_meteor_rate(),
            language: default_language(),
            sound_enabled: default_true(),
            toast_duration: default_toast_dur(),
            show_toast: default_true(),
            auto_open_folder: default_false(),
            start_minimized: default_false(),
            hide_on_capture: default_true(),
            close_to_tray: default_close_behavior(),
        }
    }
}

fn get_value<'a>(value: &'a Value, group: &str, keys: &[&str]) -> Option<&'a Value> {
    for key in keys {
        if let Some(v) = value.get(key) {
            if !v.is_object() && !v.is_array() {
                return Some(v);
            }
        }
    }
    if let Some(group_value) = value.get(group) {
        for key in keys {
            if let Some(v) = group_value.get(key) {
                return Some(v);
            }
        }
    }
    None
}

fn as_str(v: &Value, default: &str) -> String {
    v.as_str().unwrap_or(default).to_string()
}

fn as_u32(v: &Value, default: u32) -> u32 {
    v.as_u64().map(|n| n as u32).unwrap_or(default)
}

fn as_u64(v: &Value, default: u64) -> u64 {
    v.as_u64().unwrap_or(default)
}

fn as_bool(v: &Value, default: bool) -> bool {
    v.as_bool().unwrap_or(default)
}

impl Serialize for AppConfig {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        json!({
            "mode": {
                "mode": self.mode,
                "image_format": self.image_format,
                "jpeg_quality": self.jpeg_quality,
                "video_bitrate": self.video_bitrate,
                "video_duration": self.video_duration,
                "video_fps": self.video_fps,
                "motion_duration": self.motion_duration,
                "motion_fps": self.motion_fps,
            },
            "output": {
                "save_dir": self.save_dir,
                "filename_prefix": self.filename_prefix,
                "thumbnail_size": self.thumbnail_size,
                "save_thumbnail": self.save_thumbnail,
            },
            "hotkeys": {
                "hotkey_image": self.hotkey_image,
                "hotkey_video": self.hotkey_video,
                "hotkey_motion": self.hotkey_motion,
            },
            "recording": {
                "record_system_audio": self.record_system_audio,
                "audio_bitrate": self.audio_bitrate,
            },
            "appearance": {
                "window_transparency": self.window_transparency,
                "ui_transparency": self.ui_transparency,
                "starfield_density": self.starfield_density,
                "star_twinkle_speed": self.star_twinkle_speed,
                "meteor_rate": self.meteor_rate,
                "language": self.language,
            },
            "behavior": {
                "sound_enabled": self.sound_enabled,
                "toast_duration": self.toast_duration,
                "show_toast": self.show_toast,
                "auto_open_folder": self.auto_open_folder,
                "start_minimized": self.start_minimized,
                "hide_on_capture": self.hide_on_capture,
                "close_to_tray": self.close_to_tray,
            },
        })
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AppConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let mut cfg = AppConfig::default();

        if let Some(v) = get_value(&value, "mode", &["mode"]) {
            cfg.mode = as_str(v, "image");
        }
        if let Some(v) = get_value(&value, "mode", &["image_format"]) {
            cfg.image_format = as_str(v, "png");
        }
        if let Some(v) = get_value(&value, "mode", &["jpeg_quality"]) {
            cfg.jpeg_quality = as_u32(v, 90) as u8;
        }
        if let Some(v) = get_value(&value, "mode", &["video_bitrate"]) {
            cfg.video_bitrate = as_u32(v, 8_000_000);
        }
        if let Some(v) = get_value(&value, "mode", &["video_duration"]) {
            cfg.video_duration = as_u32(v, 3);
        }
        if let Some(v) = get_value(&value, "mode", &["video_fps"]) {
            cfg.video_fps = as_u32(v, 30);
        }
        if let Some(v) = get_value(&value, "mode", &["motion_duration"]) {
            cfg.motion_duration = as_u32(v, 3);
        }
        if let Some(v) = get_value(&value, "mode", &["motion_fps"]) {
            cfg.motion_fps = as_u32(v, 15);
        }

        if let Some(v) = get_value(&value, "output", &["save_dir"]) {
            cfg.save_dir = as_str(v, &default_save_dir());
        }
        if let Some(v) = get_value(&value, "output", &["filename_prefix", "file_prefix"]) {
            cfg.filename_prefix = as_str(v, "snap");
        }
        if let Some(v) = get_value(&value, "output", &["thumbnail_size"]) {
            cfg.thumbnail_size = as_u32(v, 128);
        }
        if let Some(v) = get_value(&value, "output", &["save_thumbnail"]) {
            cfg.save_thumbnail = as_bool(v, false);
        }

        if let Some(v) = get_value(&value, "hotkeys", &["hotkey_image"]) {
            cfg.hotkey_image = as_str(v, "CommandOrControl+Shift+S");
        }
        if let Some(v) = get_value(&value, "hotkeys", &["hotkey_video"]) {
            cfg.hotkey_video = as_str(v, "CommandOrControl+Shift+V");
        }
        if let Some(v) = get_value(&value, "hotkeys", &["hotkey_motion"]) {
            cfg.hotkey_motion = as_str(v, "CommandOrControl+Shift+M");
        }

        if let Some(v) = get_value(&value, "recording", &["record_system_audio", "record_audio"]) {
            cfg.record_system_audio = as_bool(v, true);
        }
        if let Some(v) = get_value(&value, "recording", &["audio_bitrate"]) {
            cfg.audio_bitrate = as_u32(v, 192_000);
        }

        if let Some(v) = get_value(&value, "appearance", &["window_transparency"]) {
            cfg.window_transparency = as_u32(v, 100);
        }
        if let Some(v) = get_value(&value, "appearance", &["ui_transparency"]) {
            cfg.ui_transparency = as_u32(v, 100);
        }
        if let Some(v) = get_value(&value, "appearance", &["starfield_density"]) {
            cfg.starfield_density = as_u32(v, 50);
        }
        if let Some(v) = get_value(&value, "appearance", &["star_twinkle_speed"]) {
            cfg.star_twinkle_speed = as_u32(v, 50);
        }
        if let Some(v) = get_value(&value, "appearance", &["meteor_rate"]) {
            cfg.meteor_rate = as_u32(v, 50);
        }
        if let Some(v) = get_value(&value, "appearance", &["language"]) {
            cfg.language = as_str(v, "en");
        }

        if let Some(v) = get_value(&value, "behavior", &["sound_enabled", "sound_on"]) {
            cfg.sound_enabled = as_bool(v, true);
        }
        if let Some(v) = get_value(&value, "behavior", &["toast_duration"]) {
            cfg.toast_duration = as_u64(v, 2500);
        }
        if let Some(v) = get_value(&value, "behavior", &["show_toast", "toast_on"]) {
            cfg.show_toast = as_bool(v, true);
        }
        if let Some(v) = get_value(&value, "behavior", &["auto_open_folder", "auto_open"]) {
            cfg.auto_open_folder = as_bool(v, false);
        }
        if let Some(v) = get_value(
            &value,
            "behavior",
            &["start_minimized", "start_tray", "start_to_tray"],
        ) {
            cfg.start_minimized = as_bool(v, false);
        }
        if let Some(v) = get_value(&value, "behavior", &["hide_on_capture", "hide_capture"]) {
            cfg.hide_on_capture = as_bool(v, true);
        }
        if let Some(v) = get_value(&value, "behavior", &["close_to_tray"]) {
            cfg.close_to_tray = as_bool(v, true);
        }

        Ok(cfg)
    }
}

impl AppConfig {
    /// Flat representation used by the frontend. The on-disk config stays
    /// grouped by category, but the UI expects the original flat field names.
    pub fn to_flat_value(&self) -> Value {
        json!({
            "mode": self.mode,
            "image_format": self.image_format,
            "jpeg_quality": self.jpeg_quality,
            "video_bitrate": self.video_bitrate,
            "video_duration": self.video_duration,
            "video_fps": self.video_fps,
            "motion_duration": self.motion_duration,
            "motion_fps": self.motion_fps,
            "save_dir": self.save_dir,
            "filename_prefix": self.filename_prefix,
            "thumbnail_size": self.thumbnail_size,
            "save_thumbnail": self.save_thumbnail,
            "hotkey_image": self.hotkey_image,
            "hotkey_video": self.hotkey_video,
            "hotkey_motion": self.hotkey_motion,
            "record_system_audio": self.record_system_audio,
            "audio_bitrate": self.audio_bitrate,
            "window_transparency": self.window_transparency,
            "ui_transparency": self.ui_transparency,
            "starfield_density": self.starfield_density,
            "star_twinkle_speed": self.star_twinkle_speed,
            "meteor_rate": self.meteor_rate,
            "language": self.language,
            "sound_enabled": self.sound_enabled,
            "toast_duration": self.toast_duration,
            "show_toast": self.show_toast,
            "auto_open_folder": self.auto_open_folder,
            "start_minimized": self.start_minimized,
            "hide_on_capture": self.hide_on_capture,
            "close_to_tray": self.close_to_tray,
        })
    }

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
        cfg.filename_prefix = cfg.filename_prefix.trim().to_string();
        cfg.image_format = match cfg.image_format.to_lowercase().as_str() {
            "jpg" | "jpeg" => "jpg".to_string(),
            _ => "png".to_string(),
        };
        cfg.jpeg_quality = cfg.jpeg_quality.clamp(1, 100);
        cfg.video_bitrate = cfg.video_bitrate.clamp(1_000_000, 20_000_000);
        cfg.window_transparency = cfg.window_transparency.clamp(0, 100);
        cfg.ui_transparency = cfg.ui_transparency.clamp(0, 100);
        cfg.starfield_density = cfg.starfield_density.clamp(0, 100);
        cfg.star_twinkle_speed = cfg.star_twinkle_speed.clamp(0, 100);
        cfg.meteor_rate = cfg.meteor_rate.clamp(0, 100);
        cfg.language = match cfg.language.as_str() {
            "zh" => "zh".to_string(),
            _ => "en".to_string(),
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_json_fields_are_grouped_by_category() {
        let json = serde_json::to_value(&AppConfig::default()).unwrap();
        let obj = json.as_object().unwrap();
        for group in ["mode", "output", "hotkeys", "recording", "appearance", "behavior"] {
            assert!(obj.contains_key(group), "missing group: {}", group);
        }
        assert_eq!(obj["mode"]["mode"], "image");
        assert_eq!(obj["mode"]["video_fps"], 30);
        assert!(obj["output"].get("save_dir").is_some());
        assert!(obj["hotkeys"].get("hotkey_image").is_some());
        assert!(obj["recording"].get("record_system_audio").is_some());
        assert!(obj["appearance"].get("window_transparency").is_some());
        assert!(obj["behavior"].get("close_to_tray").is_some());
    }

    #[test]
    fn config_accepts_nested_and_flat() {
        let nested = serde_json::json!({
            "mode": { "mode": "video", "video_fps": 60 },
            "behavior": { "close_to_tray": false }
        });
        let cfg: AppConfig = serde_json::from_value(nested).unwrap();
        assert_eq!(cfg.mode, "video");
        assert_eq!(cfg.video_fps, 60);
        assert!(!cfg.close_to_tray);

        let flat = serde_json::json!({
            "mode": "image",
            "video_fps": 45,
            "close_to_tray": false
        });
        let cfg: AppConfig = serde_json::from_value(flat).unwrap();
        assert_eq!(cfg.mode, "image");
        assert_eq!(cfg.video_fps, 45);
        assert!(!cfg.close_to_tray);
    }
}
