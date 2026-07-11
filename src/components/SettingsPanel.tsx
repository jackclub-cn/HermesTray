import { useState, useEffect } from "react";
import type { ConfigFile } from "../App";

interface Props {
  config: ConfigFile;
  onSave: (cfg: ConfigFile) => Promise<boolean>;
  onTest: () => Promise<string>;
  connected: boolean;
  status: string;
}

export function SettingsPanel({ config, onSave, onTest, connected, status }: Props) {
  const [apiUrl, setApiUrl] = useState(config.api_url);
  const [apiKey, setApiKey] = useState(config.api_key);
  const [pollInterval, setPollInterval] = useState(config.poll_interval_secs);
  const [toggleHotkey, setToggleHotkey] = useState(config.toggle_hotkey);
  const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    setApiUrl(config.api_url);
    setApiKey(config.api_key);
    setPollInterval(config.poll_interval_secs);
    setToggleHotkey(config.toggle_hotkey);
  }, [config]);

  const handleSave = async () => {
    const newCfg: ConfigFile = {
      api_url: apiUrl,
      api_key: apiKey,
      poll_interval_secs: pollInterval,
      toggle_hotkey: toggleHotkey,
      quick_input_hotkey: "Ctrl+Alt+Shift+C",
    };
    const ok = await onSave(newCfg);
    if (ok) {
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    }
  };

  const handleTest = async () => {
    setTestResult(null);
    // Save first, then test with the new values
    const newCfg: ConfigFile = {
      api_url: apiUrl,
      api_key: apiKey,
      poll_interval_secs: pollInterval,
      toggle_hotkey: toggleHotkey,
      quick_input_hotkey: "Ctrl+Alt+Shift+C",
    };
    const saved = await onSave(newCfg);
    if (!saved) {
      setTestResult({ ok: false, msg: "Failed to save settings before testing" });
      return;
    }
    const result = await onTest();
    try {
      const parsed = JSON.parse(result);
      if (parsed === "idle" || parsed === "busy") {
        setTestResult({ ok: true, msg: `Connected — status: ${parsed}` });
      } else {
        setTestResult({ ok: false, msg: `Unexpected status: ${result}` });
      }
    } catch {
      setTestResult({ ok: false, msg: result });
    }
  };

  return (
    <div className="settings">
      <div className="settings-section">🔗 API Connection</div>

      <div className="settings-group">
        <label htmlFor="api-url">API URL</label>
        <input
          id="api-url"
          className="settings-input"
          value={apiUrl}
          onChange={(e) => setApiUrl(e.target.value)}
          placeholder="http://localhost:8642"
        />
        <span className="settings-hint">
          Hermes API Server address (e.g. http://your-server:8642)
        </span>
      </div>

      <div className="settings-group">
        <label htmlFor="api-key">API Key</label>
        <input
          id="api-key"
          className="settings-input"
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="sk-... or leave empty"
        />
        <span className="settings-hint">
          Leave empty if the server doesn't require authentication
        </span>
      </div>

      <div className="settings-group">
        <label htmlFor="poll-interval">Poll Interval (seconds)</label>
        <input
          id="poll-interval"
          className="settings-input"
          type="number"
          min={1}
          max={60}
          value={pollInterval}
          onChange={(e) => setPollInterval(Number(e.target.value))}
        />
        <span className="settings-hint">How often to check Hermes status</span>
      </div>

      <div className="settings-section">⌨️ Shortcuts</div>

      <div className="settings-group">
        <label htmlFor="toggle-hotkey">Toggle Window</label>
        <input
          id="toggle-hotkey"
          className="settings-input"
          value={toggleHotkey}
          onChange={(e) => setToggleHotkey(e.target.value)}
          placeholder="Alt+Space"
        />
        <span className="settings-hint">
          Global shortcut to show/hide the window (e.g. Alt+Space, Ctrl+Alt+H)
        </span>
      </div>

      <div className="settings-hint" style={{ margin: "4px 0" }}>
        Quick input: <strong>Ctrl+Alt+Shift+C</strong> (fixed)
      </div>

      <div className="settings-actions">
        <button className="btn btn-primary" onClick={handleSave}>
          {saved ? "✓ Saved" : "Save"}
        </button>
        <button className="btn" onClick={handleTest} style={{ border: "1px solid var(--border)", background: "var(--bg-tertiary)", color: "var(--text-primary)" }}>
          Test Connection
        </button>
      </div>

      {testResult && (
        <div className={`settings-status ${testResult.ok ? "ok" : "err"}`}>
          {testResult.msg}
        </div>
      )}

      {saved && (
        <div className="settings-status ok">Settings saved successfully</div>
      )}

      <div className="settings-section" style={{ marginTop: 16 }}>📡 Live Status</div>
      <div className="settings-hint">
        Current: <strong>{status}</strong>
        {connected ? " 🟢 Online" : " ⚪ Offline"}
      </div>
    </div>
  );
}