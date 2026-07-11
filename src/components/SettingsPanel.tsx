import { useState, useEffect } from "react";
import type { ConfigFile } from "../App";
import { i18n, type Lang, type TKey } from "../i18n";

interface Props {
  config: ConfigFile;
  onSave: (cfg: ConfigFile) => Promise<boolean>;
  onTest: () => Promise<string>;
  connected: boolean;
  status: string;
  lang: Lang;
}

export function SettingsPanel({ config, onSave, onTest, connected, status, lang }: Props) {
  const t = (key: TKey) => i18n[lang][key] || i18n.en[key] || key;

  const [apiUrl, setApiUrl] = useState(config.api_url);
  const [apiKey, setApiKey] = useState(config.api_key);
  const [pollInterval, setPollInterval] = useState(config.poll_interval_secs);
  const [toggleHotkey, setToggleHotkey] = useState(config.toggle_hotkey);
  const [language, setLanguage] = useState(config.language || "en");
  const [testResult, setTestResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    setApiUrl(config.api_url);
    setApiKey(config.api_key);
    setPollInterval(config.poll_interval_secs);
    setToggleHotkey(config.toggle_hotkey);
    setLanguage(config.language || "en");
  }, [config]);

  const handleSave = async () => {
    const newCfg: ConfigFile = {
      api_url: apiUrl,
      api_key: apiKey,
      poll_interval_secs: pollInterval,
      toggle_hotkey: toggleHotkey,
      quick_input_hotkey: "Ctrl+Alt+Shift+C",
      language,
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
      language,
    };
    const savedOk = await onSave(newCfg);
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
          className="settings-input"
          value={toggleHotkey}
          onChange={(e) => setToggleHotkey(e.target.value)}
          placeholder="Alt+Space"
        />
        <span className="settings-hint">{t("settings_toggle_hint")}</span>
      </div>

      <div className="settings-hint" style={{ margin: "4px 0" }}>
        {t("settings_quick_input")}: <strong>{t("settings_quick_input_fixed")}</strong>
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
      </div>

      {testResult && (
        <div className={`settings-status ${testResult.ok ? "ok" : "err"}`}>
          {testResult.msg}
        </div>
      )}

      {saved && (
        <div className="settings-status ok">{t("settings_saved_ok")}</div>
      )}

      <div className="settings-section" style={{ marginTop: 16 }}>{t("settings_live_status")}</div>
      <div className="settings-hint">
        {lang === "zh" ? "当前" : "Current"}: <strong>{t(("status_" + (status as string)) as TKey)}</strong>
        {connected ? ` 🟢 ${t("settings_online")}` : ` ⚪ ${t("settings_offline")}`}
      </div>
    </div>
  );
}