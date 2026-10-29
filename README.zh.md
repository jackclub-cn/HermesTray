# HermesTray

<p align="center">
  <img src="screenshots/icon.png" width="128" height="128" alt="HermesTray icon">
</p>

🌐 [English](README.md) · [中文](README.zh.md)

🎛️ 跨平台桌面托盘客户端，用于 [Hermes Agent](https://hermes-agent.nousresearch.com/)。

在系统托盘中监控你的 Hermes Agent 状态并与之聊天。

## 功能特性

- **系统托盘图标** — H 字母 + 状态圆形背景，一目了然：
  - 🟢 绿色 = 待命（就绪）
  - 🟡 橙色 = 忙碌（处理中，含你自己的流式聊天）
  - ⚪ 灰色 = 断连（无法连接）
- **全局快捷键** — 均可配置：
  - `Alt+Space`（可配置）— 显示/隐藏聊天窗口
  - `Ctrl+Alt+Shift+C`（可配置）— 快速输入：呼出窗口并聚焦输入框
- **流式聊天** — 回复逐字渲染，带**停止**按钮可随时中断
- **会话管理** — 自动创建会话，随时开始新会话；**会话侧栏**可列出、恢复、删除历史会话；会话自动生成标题（双击可重命名）
- **本地历史** — 聊天记录持久化到 `~/HermesTray/history.json`，启动时自动恢复
- **Markdown 渲染** — 回复以 Markdown 呈现（代码块、表格、列表），**语法高亮**，代码块一键复制
- **消息操作** — 悬停回复可复制或重新生成
- **导出** — 任意会话可导出为 Markdown 文件
- **输入历史** — ↑/↓ 键翻看之前输入的内容
- **回复与断连通知** — 窗口隐藏时收到回复、以及连接断开时都会弹系统通知
- **图片支持** — 粘贴或上传图片（多模态）
- **明暗主题** — 深色/浅色可切换
- **检查更新** — 通过 GitHub Releases 检测新版本（托盘菜单 & 设置页）
- **完全可配置** — API 地址、密钥、轮询间隔、快捷键、开机自启、语言均可自由设置；窗口大小/位置会被记住；**Esc** 一键隐藏窗口

## 下载安装

| 平台 | 安装包 | 说明 |
|---|---|---|
| Windows | [HermesTray_x64-setup.exe](https://github.com/jackclub-cn/HermesTray/releases/latest) | NSIS 安装程序 |
| Windows | [HermesTray_x64_en-US.msi](https://github.com/jackclub-cn/HermesTray/releases/latest) | Windows 10+ 64 位 |
| macOS | [HermesTray_aarch64.dmg](https://github.com/jackclub-cn/HermesTray/releases/latest) | macOS 11+ Apple Silicon |

👉 [所有版本](https://github.com/jackclub-cn/HermesTray/releases)

## 环境要求

- [Rust](https://rustup.rs/)（1.77+）
- [Node.js](https://nodejs.org/)（18+）

### Linux

```bash
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev libayatana-appindicator3-dev librsvg2-dev
```

### macOS

安装 Xcode 命令行工具：`xcode-select --install`

### Windows

无需额外依赖 — WebView2 已内置于 Windows 10+。

## 从源码构建

```bash
git clone https://github.com/jackclub-cn/HermesTray.git
cd HermesTray

npm install
npm run tauri dev
```

## 构建

```bash
npm run tauri build
```

构建产物在 `src-tauri/target/release/bundle/` 目录下。

## 配置 Hermes Agent

HermesTray 通过 Hermes Agent 的 **API Server**（OpenAI 兼容 REST API，端口 8642）进行通信。需要在你的 Hermes 实例上启用它。

### 1. 启用 API Server

在 Hermes 的 `.env` 文件中添加（通常在 `~/.hermes/.env`）：

```bash
# 启用 API Server 适配器
API_SERVER_ENABLED=true
API_SERVER_HOST=0.0.0.0
API_SERVER_PORT=8642

# 设置 API 密钥（/health/detailed 等接口需要）
API_SERVER_KEY=你的密钥
```

或在命令行中设置：

```bash
hermes config set api_server.enabled true
```

### 2. 验证是否正常运行

```bash
# 检查 API Server 是否在监听
curl http://localhost:8642/health

# 应返回：{"status":"ok","platform":"hermes-agent","version":"0.18.2"}
```

### 3. 启动 HermesTray

启动应用后在设置中填入：

| 设置项 | 值 |
|---|---|
| **API 地址** | `http://localhost:8642`（或远程服务器地址）|
| **API 密钥** | 你的密钥（与 `API_SERVER_KEY` 保持一致）|

点击**测试连接**验证。托盘图标应变为绿色（待命）。

### 远程连接

如果 Hermes 运行在另一台机器上：

- 通过 SSH 隧道暴露端口：`ssh -L 8642:localhost:8642 your-server`
- 或用反向代理（Caddy/Nginx）在前面加 TLS
- 设置 API 地址为 `http://your-server:8642` 或 `https://your-domain.com/hermes`

### 状态轮询原理

HermesTray 每隔 N 秒（可配置，默认 3 秒）轮询 `GET /health/detailed`。该接口返回：

```json
{
  "status": "ok",
  "gateway_state": "running",
  "active_agents": 0,
  "gateway_busy": false
}
```

- `active_agents > 0` 或 `gateway_busy == true` → **忙碌**（橙色）
- `gateway_state == "running"` → **待命**（绿色）
- 任何错误或超时 → **断连**（灰色）

聊天时，HermesTray 也会在流式回复进行期间自报**忙碌**状态，因为 API 聊天会话不会增加 `active_agents` 计数。断连期间轮询会指数退避，避免对失效服务器高频请求。

#### 忙碌计数器过期自愈

`active_agents` 持久化在 gateway 的 `gateway_state.json` 里，Hermes 只在 turn 边界才重写它。如果某个 turn 在最后一次写回前被杀掉，这个计数器就可能一直停在非零，`/health/detailed` 也就一直回 `gateway_busy: true`。为避免图标被这种残留计数卡在橙色，HermesTray 会给每个忙碌声明计时：

- 有实时工作佐证（`api_server.active_runs > 0`，或 `readiness.checks.background_queues.*` 任一 > 0）→ **忙碌**
- 报告的 `active_agents` 发生变化 → **忙碌**，计时重新开始
- 否则，未经佐证且数值一直不变的声明在 **15 分钟**后过期 → **待命**，并输出一行日志（`[status] active_agents=N unchanged for Ns with no live work; treating the counter as stale`）

取舍：真正持续超过 15 分钟且没有实时佐证的 turn（例如非 API 平台上很长的 turn）在计数再次变化前会显示为待命。

## 设置说明

通过托盘右键菜单 → 设置，或点击设置标签页可进行配置：

| 设置项 | 默认值 | 说明 |
|---|---|---|
| API 地址 | `http://localhost:8642` | Hermes API 服务地址 |
| API 密钥 | 空 | Bearer 认证令牌 |
| 轮询间隔 | 3 秒 | 检查 Hermes 状态的频率 |
| 切换窗口快捷键 | `Alt+Space` | 显示/隐藏窗口的全局快捷键 |
| 快速输入快捷键 | `Ctrl+Alt+Shift+C` | 呼出窗口并聚焦输入框的全局快捷键 |
| 开机自启 | 关 | 登录系统时自动启动 |
| 主题 | 深色 | 界面主题（深色 / 浅色）|
| 语言 | English | 界面语言（English / 中文）|

配置文件保存在 `~/HermesTray/config.json`。

## 技术栈

- **Tauri 2.0** — 原生桌面壳
- **Rust** — 系统托盘、全局快捷键、HTTP 轮询与 SSE 流式、托盘图标渲染
- **React + TypeScript** — 聊天界面，Markdown 渲染，中英文国际化
- **Vite** — 前端打包

## English Version

[English Documentation](README.md)

## TODO

### CLI 状态监听

当前 HermesTray 只能通过 gateway 平台的 agent 检测繁忙状态（Telegram、Mattermost 等）。
CLI 会话是独立进程，API 无法感知其工作状态。

**思路：** 编写一个插件，在 `pre_llm_call` / `post_llm_call` 钩子中通过 Hermes API Server 推送状态，
使 HermesTray 可通过远程 API 轮询，无需反向连接。

参见 [issue 讨论](https://github.com/jackclub-cn/HermesTray/issues) 了解各方案。

- [ ] CLI 状态写入 API 端点（插件写入，API Server 提供读取）
- [ ] 远程 Hermes CLI 会话的实时忙碌/空闲检测

## 许可证

MIT
