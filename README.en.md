<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="PixelSnap Logo" width="128" height="128">
</p>

<h1 align="center">PixelSnap · Pixel Snapshot</h1>

<p align="center">
  A pixel-CRT styled screenshot, screen recording and GIF capture tool for Windows<br>
  (Independently developed personal project)
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Windows-10%2F11-blue?style=flat-square&logo=windows" alt="Windows">
  <img src="https://img.shields.io/badge/Tauri-2.0-24c8db?style=flat-square&logo=tauri" alt="Tauri">
  <img src="https://img.shields.io/badge/Rust-1.85+-dea584?style=flat-square&logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="MIT">
</p>

<p align="center"><a href="README.md">中文</a> | English</p>

---

## ❤️ Core Innovation

- No more manual **box selection** before capturing
- **Auto-locates** the foreground window, excludes the Windows **title bar** (the white bar with the process name and window controls) and captures only the window **client area**
- Creates a **folder named after the application** to organize saved files

> Author's note:
> I originally wanted to make a tool for capturing animated screenshots of galgames (because I wanted to record audio too). After researching, JPG and HEIF animations - which work well on mobile - are not easy to implement on Windows, so I switched to short video capture + GIF animation.
> In the AI era, I don't write a single line of HTML or Rust, but that doesn't matter - as long as you know how to prompt agents.

## ✨ Features

- 🖼️ **Screenshot** - PNG/JPG formats, configurable JPG quality
- 🎬 **Video recording** - H.264 hardware-accelerated MP4, optional system audio capture
- 🎞️ **GIF capture** - configurable duration and frame rate, streamed while recording
- ⌨️ **Global hotkeys** - three independent, customizable hotkeys
- 🎨 **CRT retro UI** - pixel art style, animated starfield background, scanline effect
- 📁 **Custom save path** - pick the output folder via a browse dialog
- 🪟 **System tray** - minimize to tray, quick capture from the tray menu
- 🔴 **Recording toast** - REC and completion states share the same toast; no separate REC window
- 🖼️ **Thumbnails** - optional, with adjustable size
- 🔔 **Toast notifications** - non-intrusive completion popups
- 🎵 **Sound effects** - optional capture completion sound
- 🔧 **Highly configurable** - format, quality, duration, frame rate, bitrate, transparency, starfield and language settings

## 🖥️ UI Preview

**PixelSnap** features a retro CRT pixel art style with mint/pink/yellow colors, a dynamic starfield background and scanline effects.

<img src="images/UI.png" alt="UI preview" width="600">

## 🚀 Quick Start

### Download the prebuilt version

Download the latest `PixelSnap.exe` from the [Releases](../../releases) page.

### Build from source

**Prerequisites:**
- Windows 10/11
- [Rust](https://www.rust-lang.org/tools/install) (1.85+)
- MSVC build tools (via Visual Studio Installer)
- Windows 10/11 SDK (for the Windows Graphics Capture API)
- [Node.js](https://nodejs.org/) (development only, not needed for release builds)

```powershell
# Clone the repository
git clone https://github.com/YOUR_USERNAME/PixelSnap.git
cd PixelSnap

# Build the release version
cd src-tauri
cargo build --release

# The generated executable is at:
# src-tauri\target\release\pixelsnap.exe
```

## ⌨️ Default Hotkeys

| Mode | Default hotkey |
|------|----------------|
| Screenshot | `Ctrl+Shift+S` |
| Video recording | `Ctrl+Shift+V` |
| GIF capture | `Ctrl+Shift+M` |

Hotkeys can be customized in the settings.

## 📁 Output Formats

- **Images**: `PNG` (lossless) + `JPG` (quality 1-100)
- **Video**: `MP4` (H.264 video + AAC audio, hardware accelerated via Windows Media Foundation)
- **Animation**: `GIF` (streamed while recording, 256-color global palette)

## ⚙️ Configuration

All settings can be adjusted in the UI. The configuration file is stored at:
```
%APPDATA%\PixelSnap\config.json
```

**Main settings include:**
- Save directory
- Filename prefix (may be empty)
- Image format and quality
- Video duration, frame rate and bitrate
- GIF duration and frame rate
- Window / UI transparency, starfield density, twinkle speed, meteor rate
- Startup behavior (minimize to tray)
- Auto-open folder after capture
- Toast notification toggle
- System audio recording toggle
- Language (English / 中文)

## 🧩 Tech Stack

- **[Tauri 2](https://tauri.app/)** - desktop application framework
- **[Rust](https://www.rust-lang.org/)** - capture, encoding and system integration
- **[Windows Graphics Capture API](https://learn.microsoft.com/en-us/windows/uwp/audio-video-camera/screen-capture)** - high performance screen/window capture
- **[Windows Media Foundation](https://learn.microsoft.com/en-us/windows/win32/medfound/microsoft-media-foundation-sdk)** - H.264 hardware video encoding
- **WASAPI** - system audio capture
- **HTML/CSS/JS** - frontend UI (zero dependencies, vanilla JS)
- **[image](https://crates.io/crates/image)** - image encoding (PNG/JPG/GIF/ICO)
- **[windows-rs](https://github.com/microsoft/windows-rs)** - Windows API bindings
- **[windows-capture](https://github.com/phantom-software-ak/windows-capture)** - WGC Rust bindings

## 📂 Project Structure

```
PixelSnap/
├── dist/                      # Frontend static files (HTML/CSS/JS)
│   ├── index.html             # Main settings UI
│   ├── app.js                 # Frontend logic
│   └── toast.html             # Toast notification window
├── src-tauri/
│   ├── src/
│   │   ├── main.rs            # Entry point, tray, window management, IPC commands
│   │   ├── capture.rs         # Core capture logic (image/video/GIF)
│   │   ├── config.rs          # Configuration management
│   │   ├── audio.rs           # Audio capture (WASAPI)
│   │   ├── toast.rs           # Toast window helper
│   │   └── log.rs             # Logging to %APPDATA%\PixelSnap\logs
│   ├── icons/                 # App icons
│   ├── capabilities/          # Tauri permission configuration
│   ├── Cargo.toml             # Rust dependencies
│   ├── Cargo.lock             # Rust dependency lock file
│   ├── build.rs               # Build script (icon generation)
│   └── tauri.conf.json        # Tauri app configuration
├── .gitignore
├── LICENSE                    # MIT open source license
└── README.md                  # Project documentation
```

## 🤝 Contributing

Contributions are welcome! You can:

1. Fork this repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes
4. Push to the branch
5. Open a Pull Request

## 📜 License

This project is licensed under the [MIT License](LICENSE).

## ⚠️ Windows Only

This application uses Windows-specific APIs (Windows Graphics Capture, Media Foundation, WASAPI) and is designed exclusively for Windows 10 1903+ and Windows 11. There are currently no cross-platform plans.

## 💖 Thanks

- [Trae](https://www.trae.ai/) - developed with Trae AI SOLO assistance
- [Tauri](https://tauri.app/) - excellent desktop application framework
- [windows-rs](https://github.com/microsoft/windows-rs) - official Microsoft Windows bindings
- [windows-capture](https://github.com/phantom-software-ak/windows-capture) - WGC Rust bindings
