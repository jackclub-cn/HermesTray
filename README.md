# HermesTray

🎛️ Cross-platform desktop tray client for [Hermes Agent](https://hermes-agent.nousresearch.com/).

Monitor your Hermes Agent status and chat with it — right from your system tray.

![screenshot](screenshots/screenshot.png)

## Features

- **System tray icon** — shows Hermes status at a glance:
  - 🟢 Green = idle (ready)
  - 🟡 Orange = busy (processing)
  - ⚪ Gray = disconnected
- **Global shortcuts** — toggle window from anywhere:
  - `Alt+Space` (configurable) — show/hide chat window
  - `Ctrl+Alt+Shift+C` — quick input
  - `Ctrl+Q` — quit app
- **Chat interface** — send messages and receive responses
- **Session management** — auto-creates sessions, start fresh anytime
- **Configurable** — API URL, key, polling interval all editable
- **Cross-platform** — Windows, macOS, Linux

## Prerequisites

- [Rust](https://rustup.rs/) (1.77+)
- [Node.js](https://nodejs.org/) (18+)

### Linux

```bash
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev libayatana-appindicator3-dev librsvg2-dev
```

### macOS

Xcode Command Line Tools: `xcode-select --install`

### Windows

No additional dependencies — WebView2 is built into Windows 10+.

## Quick Start

```bash
git clone https://github.com/jackclub-cn/HermesTray.git
cd HermesTray

npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

The bundled app will be in `src-tauri/target/release/bundle/`.

## Configuration

Settings are accessible from the tray menu (right-click → Settings) or the Settings tab:

| Setting | Default | Description |
|---|---|---|
| API URL | `http://localhost:8642` | Hermes API Server address |
| API Key | *(empty)* | Bearer token for authentication |
| Poll Interval | 3s | How often to check status |
| Toggle Hotkey | `Alt+Space` | Global shortcut to show/hide window |

Config is stored at `~/.config/HermesTray/config.json`.

## Connecting to a Remote Hermes

1. Ensure your Hermes instance has the API Server enabled (port 8642)
2. If remote, set up a tunnel or expose the port
3. In HermesTray Settings, set API URL to `http://your-server:8642`
4. Add API key if required
5. Click "Test Connection"

## Tech Stack

- **Tauri 2.0** — native desktop shell
- **Rust** — system tray, global shortcuts, HTTP polling
- **React + TypeScript** — chat UI
- **Vite** — frontend bundling

## License

MIT
