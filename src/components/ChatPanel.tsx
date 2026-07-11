import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { i18n, type Lang, type TKey } from "../i18n";

interface Message {
  role: "user" | "assistant" | "system";
  content: string;
  id: string;
}

interface Props {
  lang: Lang;
}

export function ChatPanel({ lang }: Props) {
  const t = (key: TKey) => i18n[lang][key] || i18n.en[key] || key;

  const [messages, setMessages] = useState<Message[]>([
    { role: "system", content: t("chat_hint"), id: "sys-0" },
  ]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const sendMessage = async () => {
    const text = input.trim();
    if (!text || sending) return;
    setInput("");
    setSending(true);

    const userMsg: Message = { role: "user", content: text, id: `u-${Date.now()}` };
    setMessages((prev) => [...prev, userMsg]);

    try {
      let sid = sessionId;
      if (!sid) {
        sid = await invoke<string>("create_session");
        setSessionId(sid);
      }

      const response = await invoke<string>("send_chat", {
        sessionId: sid,
        message: text,
      });

      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: response, id: `a-${Date.now()}` },
      ]);
    } catch (e) {
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `${t("chat_error")}: ${e}`, id: `e-${Date.now()}` },
      ]);
    } finally {
      setSending(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  };

  const newSession = async () => {
    setSessionId(null);
    setMessages([
      { role: "system", content: t("chat_new_session_started"), id: `sys-${Date.now()}` },
    ]);
  };

  return (
    <>
      <div className="chat-messages">
        {messages.map((msg) => (
          <div key={msg.id} className={`message ${msg.role}`}>
            {msg.content}
          </div>
        ))}
        {sending && <div className="message assistant loading">{t("chat_thinking")}</div>}
        <div ref={messagesEndRef} />
      </div>
      <div className="chat-input-bar">
        <textarea
          className="chat-input"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t("chat_placeholder")}
          rows={1}
          disabled={sending}
        />
        <button className="btn btn-primary" onClick={newSession} title={t("chat_new_session")} style={{ padding: "8px 10px" }}>
          +
        </button>
        <button className="btn btn-primary" onClick={sendMessage} disabled={!input.trim() || sending}>
          {t("chat_send")}
        </button>
      </div>
    </>
  );
}