import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ConfigFile } from "../App";
import { i18n, type Lang, type TKey } from "../i18n";

/** Format seconds as HH:MM:SS (or MM:SS under an hour). */
function formatDuration(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(r)}` : `${m}:${pad(r)}`;
}

interface Props {
  config: ConfigFile;
  onSave: (cfg: ConfigFile) => Promise<boolean>;
  onTest: () => Promise<string>;
  onCheckUpdate: () => Promise<string>;
  connected: boolean;
  status: string;
  lang: Lang;
}

export function SettingsPanel({ config, onSave, onTest, onCheckUpdate, connected, status, lang }: Props) {
  const t = (key: TKey) => i18n[lang][key] || i18n.en[key] || key;

  const [apiUrl, setApiUrl] = useState(config.api_url);
  const [apiKey, setApiKey] = useState(config.api_key);
  const [pollInterval, setPollInterval] = useState(config.poll_interval_secs);
  const [toggleHotkey, setToggleHotkey] = useState(config.toggle_hotkey);
  const [quickInputHotkey, setQuickInputHotkey] = useState(config.quick_input_hotkey);
  const [capturing, setCapturing] = useState(false);
  const [capturingQuick, setCapturingQuick] = useState(false);
  const [language, setLanguage] = useState(config.language || "en");
  const [theme, setTheme] = useState(config.theme || "dark");
  const [autostart, setAutostart] = useState(config.autostart);
  const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [updateState, setUpdateState] = useState<"idle" | "checking" | "done" | "error">("idle");
  const [updateMsg, setUpdateMsg] = useState("");
  const [stats, setStats] = useState<{ uptime_secs: number; session_count: number } | null>(null);

  useEffect(() => {
    setApiUrl(config.api_url);
    setApiKey(config.api_key);
    setPollInterval(config.poll_interval_secs);
    setToggleHotkey(config.toggle_hotkey);
    setQuickInputHotkey(config.quick_input_hotkey);
    setLanguage(config.language || "en");
    setTheme(config.theme || "dark");
    setAutostart(config.autostart);
  }, [config]);

  // Live stats: uptime + session count, refreshed every 5s
  useEffect(() => {
    let cancelled = false;
    const tick = async () => {
      try {
        const s = await invoke<{ uptime_secs: number; session_count: number }>("get_stats");
        if (!cancelled) setStats(s);
      } catch {
        /* ignore */
      }
    };
    tick();
    const timer = window.setInterval(tick, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  const buildConfig = (): ConfigFile => ({
    api_url: apiUrl,
    api_key: apiKey,
    poll_interval_secs: pollInterval,
    toggle_hotkey: toggleHotkey,
    quick_input_hotkey: quickInputHotkey,
    autostart,
    language,
    theme,
  });

  const handleSave = async () => {
    setSaveError(null);
    const ok = await onSave(buildConfig());
    if (ok) {
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } else {
      setSaveError(
        lang === "zh"
          ? "保存失败：快捷键可能被占用或按键无效"
          : "Save failed: hotkey may be in use or the key is unsupported"
      );
    }
  };

  // Shared hotkey capture: Escape cancels, at least one modifier required (except F-keys)
  const handleHotkeyKeyDown = (
    e: React.KeyboardEvent<HTMLInputElement>,
    setter: (v: string) => void,
    setCapturing: (v: boolean) => void
  ) => {
    e.preventDefault();
    e.stopPropagation();

    if (e.key === "Escape") {
      setCapturing(false);
      e.currentTarget.blur();
      return;
    }

    const keyName = (() => {
      const code = e.code;
      if (code.startsWith("Key")) return code.slice(3); // KeyA -> A
      if (code.startsWith("Digit")) return code.slice(5); // Digit1 -> 1
      if (code.startsWith("F") && /^F(?:[1-9]|1[0-2])$/.test(code)) return code;
      switch (code) {
        case "Space": return "Space";
        case "Enter": return "Enter";
        case "ArrowUp": return "ArrowUp";
        case "ArrowDown": return "ArrowDown";
        case "ArrowLeft": return "ArrowLeft";
        case "ArrowRight": return "ArrowRight";
        default: return null;
      }
    })();

    if (!keyName) return; // unsupported key, keep capturing

    const mods: string[] = [];
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push("Super");

    if (mods.length === 0 && !/^F/.test(keyName)) return;

    setter([...mods, keyName].join("+"));
    setCapturing(false);
    e.currentTarget.blur();
  };

  const handleCheckUpdate = async () => {
    setUpdateState("checking");
    const msg = await onCheckUpdate();
    setUpdateMsg(msg);
    setUpdateState(msg.startsWith(t("settings_update_available")) ? "done" : "error");
  };

  const handleTest = async () => {
    setTestResult(null);
    // Save first, then test with the new values
    const savedOk = await onSave(buildConfig());
    if (!savedOk) {
      setTestResult({ ok: false, msg: lang === "zh" ? "保存设置失败" : "Failed to save settings before testing" });
      return;
    }
    const result = await onTest();
    try {
      const parsed = JSON.parse(result);
      if (parsed === "idle" || parsed === "busy") {
        setTestResult({ ok: true, msg: lang === "zh" ? `已连接 — 状态: ${parsed}` : `Connected — status: ${parsed}` });
      } else {
        setTestResult({ ok: false, msg: `${result}` });
      }
    } catch {
      setTestResult({ ok: false, msg: result });
    }
  };

  return (
    <div className="settings">
      <div className="settings-section">{lang === "zh" ? "🔗 API 连接" : "🔗 API Connection"}</div>

      <div className="settings-group">
        <label htmlFor="api-url">{t("settings_api_url")}</label>
        <input
          id="api-url"
          className="settings-input"
          value={apiUrl}
          onChange={(e) => setApiUrl(e.target.value)}
          placeholder="http://localhost:8642"
        />
        <span className="settings-hint">{t("settings_api_url_hint")}</span>
      </div>

      <div className="settings-group">
        <label htmlFor="api-key">{t("settings_api_key")}</label>
        <input
          id="api-key"
          className="settings-input"
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="sk-..."
        />
        <span className="settings-hint">{t("settings_api_key_hint")}</span>
      </div>

      <div className="settings-group">
        <label htmlFor="poll-interval">{t("settings_poll_interval")}</label>
        <input
          id="poll-interval"
          className="settings-input"
          type="number"
          min={1}
          max={60}
          value={pollInterval}
          onChange={(e) => setPollInterval(Number(e.target.value))}
        />
        <span className="settings-hint">{t("settings_poll_hint")}</span>
      </div>

      <div className="settings-section">{t("settings_shortcuts")}</div>

      <div className="settings-group">
        <label htmlFor="toggle-hotkey">{t("settings_toggle_hotkey")}</label>
        <input
          id="toggle-hotkey"
          className={`settings-input hotkey-input${capturing ? " capturing" : ""}`}
          value={capturing ? "" : toggleHotkey}
          readOnly
          onFocus={() => setCapturing(true)}
          onBlur={() => setCapturing(false)}
          onKeyDown={(e) => handleHotkeyKeyDown(e, setToggleHotkey, setCapturing)}
          placeholder={capturing ? (lang === "zh" ? "按下快捷键..." : "Press shortcut...") : "Alt+Space"}
        />
        <span className="settings-hint">{t("settings_toggle_hint")}</span>
      </div>

      <div className="settings-group">
        <label htmlFor="quick-input-hotkey">{t("settings_quick_input_hotkey")}</label>
        <input
          id="quick-input-hotkey"
          className={`settings-input hotkey-input${capturingQuick ? " capturing" : ""}`}
          value={capturingQuick ? "" : quickInputHotkey}
          readOnly
          onFocus={() => setCapturingQuick(true)}
          onBlur={() => setCapturingQuick(false)}
          onKeyDown={(e) => handleHotkeyKeyDown(e, setQuickInputHotkey, setCapturingQuick)}
          placeholder={capturingQuick ? (lang === "zh" ? "按下快捷键..." : "Press shortcut...") : "Ctrl+Alt+Shift+C"}
        />
        <span className="settings-hint">{t("settings_quick_input_hint")}</span>
      </div>

      <div className="settings-section">{t("settings_system")}</div>
      <div className="settings-group">
        <label htmlFor="autostart" style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}>
          <input
            id="autostart"
            type="checkbox"
            checked={autostart}
            onChange={(e) => setAutostart(e.target.checked)}
            style={{ accentColor: "var(--accent)" }}
          />
          {t("settings_autostart")}
        </label>
        <span className="settings-hint">{t("settings_autostart_hint")}</span>
      </div>

      <div className="settings-group">
        <label htmlFor="theme">{t("settings_theme")}</label>
        <select
          id="theme"
          className="settings-input"
          value={theme}
          onChange={(e) => setTheme(e.target.value)}
        >
          <option value="dark">{lang === "zh" ? "深色" : "Dark"}</option>
          <option value="light">{lang === "zh" ? "浅色" : "Light"}</option>
        </select>
        <span className="settings-hint">{t("settings_theme_hint")}</span>
      </div>

      <div className="settings-section">{t("settings_language")}</div>
      <div className="settings-group">
        <label htmlFor="language">{t("settings_language")}</label>
        <select
          id="language"
          className="settings-input"
          value={language}
          onChange={(e) => setLanguage(e.target.value)}
        >
          <option value="en">English</option>
          <option value="zh">中文</option>
        </select>
        <span className="settings-hint">{t("settings_language_hint")}</span>
      </div>

      <div className="settings-actions">
        <button className="btn btn-primary" onClick={handleSave}>
          {saved ? t("settings_saved") : t("settings_save")}
        </button>
        <button className="btn" onClick={handleTest} style={{ border: "1px solid var(--border)", background: "var(--bg-tertiary)", color: "var(--text-primary)" }}>
          {t("settings_test")}
        </button>
        <button
          className="btn"
          onClick={handleCheckUpdate}
          disabled={updateState === "checking"}
          style={{ border: "1px solid var(--border)", background: "var(--bg-tertiary)", color: "var(--text-primary)" }}
        >
          {updateState === "checking" ? t("settings_update_checking") : t("settings_check_update")}
        </button>
      </div>

      {testResult && (
        <div className={`settings-status ${testResult.ok ? "ok" : "err"}`}>
          {testResult.msg}
        </div>
      )}

      {saveError && <div className="settings-status err">{saveError}</div>}

      {saved && (
        <div className="settings-status ok">{t("settings_saved_ok")}</div>
      )}

      {updateState !== "idle" && (
        <div className={`settings-status ${updateState === "done" ? "ok" : updateState === "error" ? "err" : ""}`}>
          {updateState === "checking" ? t("settings_update_checking") : updateMsg}
        </div>
      )}

      <div className="settings-section" style={{ marginTop: 16 }}>{t("settings_live_status")}</div>
      <div className="settings-hint">
        {lang === "zh" ? "当前" : "Current"}: <strong>{t(("status_" + (status as string)) as TKey)}</strong>
        {connected ? ` 🟢 ${t("settings_online")}` : ` ⚪ ${t("settings_offline")}`}
      </div>
      {stats && (
        <div className="settings-hint">
          {t("settings_stats_uptime")}: <strong>{formatDuration(stats.uptime_secs)}</strong> · {t("settings_stats_sessions")}: <strong>{stats.session_count}</strong>
        </div>
      )}
    </div>
  );
}
