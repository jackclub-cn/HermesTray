import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Message {
  role: "user" | "assistant" | "system";
  content: string;
  id: string;
}

export function ChatPanel() {
  const [messages, setMessages] = useState<Message[]>([
    { role: "system", content: "Press Enter to send, Shift+Enter for newline.", id: "sys-0" },
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

    // Add user message
    const userMsg: Message = { role: "user", content: text, id: `u-${Date.now()}` };
    setMessages((prev) => [...prev, userMsg]);

    try {
      // Get or create session
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
        { role: "assistant", content: `Error: ${e}`, id: `e-${Date.now()}` },
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
      { role: "system", content: "New session started. Press Enter to send.", id: `sys-${Date.now()}` },
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
        {sending && <div className="message assistant loading">Thinking...</div>}
        <div ref={messagesEndRef} />
      </div>
      <div className="chat-input-bar">
        <textarea
          className="chat-input"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a message..."
          rows={1}
          disabled={sending}
        />
        <button className="btn btn-primary" onClick={newSession} title="New session" style={{ padding: "8px 10px" }}>
          ✕
        </button>
        <button className="btn btn-primary" onClick={sendMessage} disabled={!input.trim() || sending}>
          Send
        </button>
      </div>
    </>
  );
}