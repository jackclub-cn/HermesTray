# HermesTray

<p align="center">
  <img src="screenshots/icon.png" width="128" height="128" alt="HermesTray icon">
</p>

🌐 [English](README.md) · [中文](README.zh.md)

Cross-platform desktop tray client for [Hermes Agent](https://hermes-agent.nousresearch.com/).

Monitor your Hermes Agent status and chat with it — right from your system tray.

## Features

- **System tray icon** — shows H letter on colored circle at a glance:
  - 🟢 Green = idle (ready)
  - 🟡 Orange = busy (processing)
  - ⚪ Gray = disconnected
- **Global shortcuts** — toggle window from anywhere:
  - `Alt+Space` (configurable) — show/hide chat window
  - `Ctrl+Alt+Shift+C` — quick input
- **Chat interface** — send messages and receive responses
- **Session management** — auto-creates sessions, start fresh anytime
- **Configurable** — API URL, key, polling interval, hotkey, language all editable
- **Chinese / English UI** — switch language from the header button or Settings
- **Cross-platform** — Windows, macOS, Linux

## Download

| Platform | Installer | Notes |
|---|---|---|
| Windows | [Download .msi](https://github.com/jackclub-cn/HermesTray/releases/latest) | Windows 10+ 64-bit |
| Windows | [Download .exe](https://github.com/jackclub-cn/HermesTray/releases/latest) | NSIS installer |
| macOS | [Download .dmg](https://github.com/jackclub-cn/HermesTray/releases/latest) | macOS 11+ Intel/Apple Silicon |

👉 [All releases](https://github.com/jackclub-cn/HermesTray/releases)

## Prerequisites

- **System tray icon** — shows H letter on colored circle at a glance:
  - 🟢 Green = idle (ready)
  - 🟡 Orange = busy (processing)
  - ⚪ Gray = disconnected
- **Global shortcuts** — toggle window from anywhere:
  - `Alt+Space` (configurable) — show/hide chat window
  - `Ctrl+Alt+Shift+C` — quick input
- **Chat interface** — send messages and receive responses
- **Session management** — auto-creates sessions, start fresh anytime
- **Configurable** — API URL, key, polling interval, hotkey, language all editable
- **Chinese / English UI** — switch language from the header button or Settings
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

## Build from Source

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

## Configuring Hermes Agent for HermesTray

HermesTray communicates with Hermes Agent through its **API Server** (OpenAI-compatible REST API on port 8642). You need to enable it on your Hermes instance.

### 1. Enable the API Server

Add to your Hermes `.env` file (typically `~/.hermes/.env`):

```bash
# Enable the API Server adapter
API_SERVER_ENABLED=true
API_SERVER_HOST=0.0.0.0
API_SERVER_PORT=8642

# Set an API key (required for /health/detailed and other endpoints)
API_SERVER_KEY=your_secret_key_here
```

Or set them directly:

```bash
hermes config set api_server.enabled true
```

### 2. Verify it's running

```bash
# Check the API Server is listening
curl http://localhost:8642/health

# Should return: {"status":"ok","platform":"hermes-agent","version":"0.18.2"}
```

### 3. Start HermesTray

Launch the app. In Settings, set:

| Setting | Value |
|---|---|
| **API URL** | `http://localhost:8642` (or your remote server address) |
| **API Key** | `your_secret_key_here` (the same as `API_SERVER_KEY`) |

Click **Test Connection** to verify. The tray icon should turn green (idle).

### Remote access

If Hermes runs on a different machine:

- Expose port 8642 via SSH tunnel: `ssh -L 8642:localhost:8642 your-server`
- Or use a reverse proxy (Caddy/Nginx) with TLS in front
- Set the API URL to `http://your-server:8642` or `https://your-domain.com/hermes`

### How status polling works

HermesTray polls `GET /health/detailed` every N seconds (configurable, default 3s). The endpoint returns:

```json
{
  "status": "ok",
  "gateway_state": "running",
  "active_agents": 0,
  "gateway_busy": false
}
```

- `active_agents > 0` or `gateway_busy == true` → **busy** (orange)
- `gateway_state == "running"` → **idle** (green)
- any error / timeout → **disconnected** (gray)

When chatting, HermesTray also self-reports **busy** while waiting for a response, since synchronous API chat sessions don't increment `active_agents`.

## Configuration

Settings are accessible from the tray menu (right-click → Settings) or the Settings tab:

| Setting | Default | Description |
|---|---|---|
| API URL | `http://localhost:8642` | Hermes API Server address |
| API Key | *(empty)* | Bearer token for authentication |
| Poll Interval | 3s | How often to check status |
| Toggle Hotkey | `Alt+Space` | Global shortcut to show/hide window |
| Language | English | UI language (English / 中文) |

Config is saved to `~/HermesTray/config.json`.

## Tech Stack

- **Tauri 2.0** — native desktop shell
- **Rust** — system tray, global shortcuts, HTTP polling, tray icon rendering
- **React + TypeScript** — chat UI with Chinese/English i18n
- **Vite** — frontend bundling

## 中文版

[中文文档](README.zh.md)

## English Version

[English Documentation](README.md)

## TODO

### CLI Status Monitoring

Currently HermesTray can only detect busy state from gateway-initiated agents (Telegram, Mattermost, etc.).
CLI sessions are independent processes and not reflected in the API.

**Idea:** write a plugin that hooks into `pre_llm_call` / `post_llm_call` and pushes status through the Hermes API Server,
so HermesTray can poll it remotely without any reverse connectivity.

See [issue discussion](https://github.com/jackclub-cn/HermesTray/issues) for proposed approaches.

- [ ] CLI status → API endpoint (plugin writes, API Server serves)
- [ ] Real-time busy/idle for remote Hermes CLI sessions

## License

MIT