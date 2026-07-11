import { useState, useEffect, useCallback, useRef } from "react";
import "./App.css";
import { ChatPanel } from "./components/ChatPanel";
import { SettingsPanel } from "./components/SettingsPanel";

type Tab = "chat" | "settings";
type Status = "disconnected" | "idle" | "busy";

export type ConfigFile = {
  api_url: string;
  api_key: string;
  poll_interval_secs: number;
  toggle_hotkey: string;
  quick_input_hotkey: string;
};

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
      emit: (event: string, payload?: unknown) => Promise<void>;
      listen: (event: string, handler: (event: { payload: unknown }) => void) => Promise<() => void>;
    };
  }
}

function App() {
  const [tab, setTab] = useState<Tab>("chat");
  const [status, setStatus] = useState<Status>("disconnected");
  const [config, setConfig] = useState<ConfigFile>({
    api_url: "http://localhost:8642",
    api_key: "",
    poll_interval_secs: 3,
    toggle_hotkey: "Alt+Space",
    quick_input_hotkey: "Ctrl+Alt+Shift+C",
  });
  const [connected, setConnected] = useState(false);
  const unlistenRef = useRef<(() => void)[]>([]);

  useEffect(() => {
    const tauri = window.__TAURI_INTERNALS__;
    if (!tauri) return;

    // Load config from Rust backend
    tauri.invoke("get_config").then((cfg) => {
      setConfig(cfg as ConfigFile);
    });

    // Listen for status events from Rust (polling)
    tauri.listen("hermes-status", (event) => {
      const payload = event.payload as { status: Status };
      setStatus(payload.status);
      setConnected(payload.status !== "disconnected");
    });

    // Listen for quick-input event
    const unlisten = tauri.listen("quick-input", () => {
      setTab("chat");
    });

    // Listen for navigate event
    const unlisten2 = tauri.listen("navigate", (event) => {
      const page = event.payload as string;
      if (page === "settings") setTab("settings");
    });

    return () => {
      unlisten.then((fn) => fn());
      unlisten2.then((fn) => fn());
    };
  }, []);

  const handleSaveConfig = useCallback(async (newConfig: ConfigFile) => {
    const tauri = window.__TAURI_INTERNALS__;
    if (!tauri) return false;
    try {
      await tauri.invoke("update_config", { newConfig });
      setConfig(newConfig);
      return true;
    } catch (e) {
      console.error("Failed to save config", e);
      return false;
    }
  }, []);

  const handleTestConnection = useCallback(async () => {
    const tauri = window.__TAURI_INTERNALS__;
    if (!tauri) return "Cannot reach backend";
    try {
      const result = await tauri.invoke("test_connection");
      return result as string;
    } catch (e) {
      return String(e);
    }
  }, []);

  return (
    <div className="app">
      {/* Header with status */}
      <div className="header">
        <div className={`status-dot ${status}`} />
        <span className="header-title">HermesTray</span>
        <span className="header-status">{status}</span>
      </div>

      {/* Tab bar */}
      <div className="tab-bar">
        <div
          className={`tab ${tab === "chat" ? "active" : ""}`}
          onClick={() => setTab("chat")}
        >
          Chat
        </div>
        <div
          className={`tab ${tab === "settings" ? "active" : ""}`}
          onClick={() => setTab("settings")}
        >
          Settings
        </div>
      </div>

      {/* Content */}
      <div className="content">
        {tab === "chat" && <ChatPanel />}
        {tab === "settings" && (
          <SettingsPanel
            config={config}
            onSave={handleSaveConfig}
            onTest={handleTestConnection}
            connected={connected}
            status={status}
          />
        )}
      </div>
    </div>
  );
}

export default App;
