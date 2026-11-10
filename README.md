<div align="center">

<img src="icon.png" alt="rDM icon" width="96">

# rDM

A desktop download manager built in Rust with GPUI.

[![Windows x64](https://img.shields.io/badge/Windows-x64-0078d4)](#windows-x64)
[![Linux x86_64](https://img.shields.io/badge/Linux-x86__64-fcc624?logo=linux&logoColor=black)](#linux-x86_64)
[![Chrome and Firefox](https://img.shields.io/badge/Extension-Chrome%20%7C%20Firefox-5e81ac)](#browser-integration)

</div>

## Scope

rDM brings downloads from direct links and the browser into a desktop app where you can manage transfers independently of the tabs that started them. The command-line client uses the same Rust engine for downloads from a terminal or script.

The engine uses Tokio for network and disk work while GPUI renders the desktop interface on the GPU. When a server supports HTTP range requests and reports a file size, rDM can download multiple segments in parallel. Other servers use a single connection.

## Features

### Download Engine

- **Segmented downloads.** Choose the number of connections per file and how many downloads run at once.
- **Pause and resume.** Continue supported transfers from partial data and retry temporary connection failures.
- **Transfer controls.** Set a speed limit and choose the default download folder.

### Desktop App

- **Live progress.** See each transfer's speed, remaining time, and status.
- **Dashboard and history.** View active transfers and browse completed downloads in a calendar.
- **Appearance.** Choose a light or dark theme and customize the accent color.

### Browser and Media

- **Download handoff.** Send links to rDM from the browser context menu or let the extension intercept eligible downloads automatically.
- **YouTube formats.** Use the extension to select a video or audio format. Combining separate video and audio streams or converting audio to MP3 requires FFmpeg. rDM looks for it in the app's data directory under `bin` and on `PATH`.

## Installation

### Windows x64

When Windows packages are available, run `rDM-Setup-v*-x64.exe` or extract `rDM-v*-portable-x64.zip` and open `rDM.exe`. Both packages include the desktop app and `rdm-cli.exe`. [GitHub Releases](https://github.com/moiCR/rDM/releases)

To build from source, use the stable Rust MSVC toolchain with the Visual Studio C++ build tools and a Windows SDK.

```powershell
git clone https://github.com/moiCR/rDM.git
cd rDM
cargo build --release --workspace
.\target\release\rDM.exe
```

### Linux x86_64

The Linux release workflow builds a `.deb`, an `.rpm`, and an AppImage and uploads them to the release. Download the format for your distribution when the assets are available. [GitHub Releases](https://github.com/moiCR/rDM/releases)

The `.deb` and `.rpm` add rDM to the application menu and install `rdm-cli` on `PATH`. The desktop app runs in a Wayland or X11 session with a working Vulkan driver. [GPUI Kit's Linux requirements](https://gpui-kit.com/docs/installation/#platform-requirements)

## Browser Integration

Download `rdm-chrome.zip` or `rdm-firefox.zip` from the release assets. [GitHub Releases](https://github.com/moiCR/rDM/releases)

- **Chrome.** Extract `rdm-chrome.zip`. Open `chrome://extensions`, enable **Developer mode**, choose **Load unpacked**, and select the folder containing `manifest.json`. [Chrome extension guide](https://developer.chrome.com/docs/extensions/get-started/tutorial/hello-world#load-unpacked)
- **Firefox.** Open `about:debugging#/runtime/this-firefox`, choose **Load Temporary Add-on**, and select `rdm-firefox.zip`. Repeat after restarting Firefox. [Firefox extension guide](https://extensionworkshop.com/documentation/develop/temporary-installation-in-firefox/)

Keep the desktop app running while using the extension. The extension connects to `127.0.0.1:9654` by default. Right-click a link and choose **Descargar con rDM**, or enable automatic interception in the extension popup.

## Command Line

With `rdm-cli` on `PATH`, run these commands directly. From a source build, use `.\target\release\rdm-cli.exe` on Windows or `./target/release/rdm-cli` on Linux.

```text
rdm-cli download https://example.com/file.zip -o file.zip -c 8
rdm-cli info https://example.com/file.zip
rdm-cli list
rdm-cli resume <TASK_ID>
rdm-cli --help
```

Use `info` to inspect a URL before downloading and `list` to find the ID of a paused or interrupted transfer.
