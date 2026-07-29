import { useState, useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { i18n, type Lang, type TKey } from "../i18n";

interface Message {
  role: "user" | "assistant" | "system";
  content: string;
  images?: string[]; // base64 data URLs
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
  const [attachedImages, setAttachedImages] = useState<
    { id: string; dataUrl: string }[]
  >([]);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const autoResize = useCallback(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = Math.min(ta.scrollHeight, 90) + "px";
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  useEffect(() => {
    autoResize();
  }, [input, autoResize]);

  // ── Image processing ──

  const fileToDataUrl = (file: File): Promise<string> => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = reject;
      reader.readAsDataURL(file);
    });
  };

  const addImagesFromFiles = async (files: FileList | File[]) => {
    const newImages: { id: string; dataUrl: string }[] = [];
    for (const file of Array.from(files)) {
      if (!file.type.startsWith("image/")) continue;
      // Limit to reasonable size (20MB)
      if (file.size > 20 * 1024 * 1024) continue;
      const dataUrl = await fileToDataUrl(file);
      newImages.push({ id: `img-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`, dataUrl });
    }
    if (newImages.length > 0) {
      setAttachedImages((prev) => [...prev, ...newImages]);
    }
  };

  const handlePaste = async (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const items = e.clipboardData.items;
    const imageFiles: File[] = [];
    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      if (item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (file) imageFiles.push(file);
      }
    }
    if (imageFiles.length > 0) {
      e.preventDefault();
      await addImagesFromFiles(imageFiles);
    }
  };

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (files && files.length > 0) {
      await addImagesFromFiles(files);
    }
    // Reset so the same file can be selected again
    e.target.value = "";
  };

  const removeImage = (id: string) => {
    setAttachedImages((prev) => prev.filter((img) => img.id !== id));
  };

  const triggerFilePicker = () => {
    fileInputRef.current?.click();
  };

  // ── Sending messages ──

  const sendMessage = async () => {
    const text = input.trim();
    const hasImages = attachedImages.length > 0;
    if ((!text && !hasImages) || sending) return;

    setInput("");
    setAttachedImages([]);
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
    setSending(true);

    const userMsg: Message = {
      role: "user",
      content: text,
      images: hasImages ? attachedImages.map((img) => img.dataUrl) : undefined,
      id: `u-${Date.now()}`,
    };
    setMessages((prev) => [...prev, userMsg]);

    try {
      let sid = sessionId;
      if (!sid) {
        sid = await invoke<string>("create_session");
        setSessionId(sid);
      }

      // Build message payload – OpenAI multimodal format when images are present
      let messagePayload: string;
      if (hasImages) {
        const content: unknown[] = [];
        if (text) {
          content.push({ type: "text", text });
        }
        for (const img of attachedImages) {
          content.push({
            type: "image_url",
            image_url: { url: img.dataUrl },
          });
        }
        messagePayload = JSON.stringify(content);
      } else {
        messagePayload = text;
      }

      const response = await invoke<string>("send_chat", {
        sessionId: sid,
        message: messagePayload,
      });

      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: response, id: `a-${Date.now()}` },
      ]);
    } catch (e) {
      setMessages((prev) => [
        ...prev,
        {
          role: "assistant",
          content: `${t("chat_error")}: ${e}`,
          id: `e-${Date.now()}`,
        },
      ]);
    } finally {
      setSending(false);
      setTimeout(() => textareaRef.current?.focus(), 0);
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
    setAttachedImages([]);
    setMessages([
      {
        role: "system",
        content: t("chat_new_session_started"),
        id: `sys-${Date.now()}`,
      },
    ]);
  };

  // ── Render helpers ──

  const renderMessageContent = (msg: Message) => {
    return (
      <>
        {msg.images && msg.images.length > 0 && (
          <div className="message-images">
            {msg.images.map((src, i) => (
              <img
                key={i}
                src={src}
                alt={`Image ${i + 1}`}
                className="message-image"
                onClick={() => window.open(src, "_blank")}
              />
            ))}
          </div>
        )}
        {msg.content && <div className="message-text">{msg.content}</div>}
      </>
    );
  };

  return (
    <>
      <div className="chat-messages">
        {messages.map((msg) => (
          <div key={msg.id} className={`message ${msg.role}`}>
            {renderMessageContent(msg)}
          </div>
        ))}
        {sending && (
          <div className="message assistant loading">{t("chat_thinking")}</div>
        )}
        <div ref={messagesEndRef} />
      </div>

      <div className="chat-input-bar">
        {/* Image previews */}
        {attachedImages.length > 0 && (
          <div className="chat-images-preview">
            {attachedImages.map((img) => (
              <div key={img.id} className="chat-image-preview">
                <img src={img.dataUrl} alt="preview" />
                <button
                  className="chat-image-remove"
                  onClick={() => removeImage(img.id)}
                  title={lang === "zh" ? "移除" : "Remove"}
                >
                  ×
                </button>
              </div>
            ))}
          </div>
        )}

        <textarea
          ref={textareaRef}
          className="chat-input"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          placeholder={
            attachedImages.length > 0
              ? lang === "zh"
                ? "添加描述或直接发送..."
                : "Add a description or send directly..."
              : t("chat_placeholder")
          }
          rows={1}
          disabled={sending}
        />

        <div className="chat-actions">
          <div className="chat-actions-left">
            <button
              className="btn btn-new"
              onClick={newSession}
            >
              {t("chat_new_session")}
            </button>
          </div>
          <div className="chat-actions-right">
            {/* Image attach button */}
            <button
              className="btn btn-attach"
              onClick={triggerFilePicker}
              disabled={sending}
              title={lang === "zh" ? "上传图片" : "Upload image"}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
                <circle cx="8.5" cy="8.5" r="1.5"/>
                <polyline points="21,15 16,10 5,21"/>
              </svg>
            </button>
            <button
              className="btn btn-send"
              onClick={sendMessage}
              disabled={(!input.trim() && attachedImages.length === 0) || sending}
            >
              {t("chat_send")}
            </button>
          </div>
        </div>
      </div>

      {/* Hidden file input */}
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        multiple
        style={{ display: "none" }}
        onChange={handleFileSelect}
      />
    </>
  );
}
