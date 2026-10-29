import { useState, useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { sendNotification } from "@tauri-apps/plugin-notification";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { codeToHtml } from "shiki";
import { i18n, type Lang, type TKey } from "../i18n";

interface Message {
  role: "user" | "assistant" | "system";
  content: string;
  images?: string[]; // base64 data URLs
  id: string;
}

interface SessionInfo {
  id: string;
  title: string;
  created?: string;
}

/** Local chat history file shape (~/HermesTray/history.json) */
interface HistoryFile {
  active_session: string | null;
  titles: Record<string, string>;
  messages: Record<string, Message[]>;
}

interface Props {
  lang: Lang;
}

/** Lazily-highlighted code block (shiki), debounced for streaming. */
function HighlightedCode({ code, lang }: { code: string; lang: string }) {
  const [html, setHtml] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setHtml(null);
    const timer = window.setTimeout(() => {
      codeToHtml(code, { lang: lang || "text", theme: "github-dark" })
        .then((h) => {
          if (!cancelled) setHtml(h);
        })
        .catch(() => {
          if (!cancelled) setHtml(null);
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [code, lang]);

  if (html) {
    return <div className="shiki-wrap" dangerouslySetInnerHTML={{ __html: html }} />;
  }
  return (
    <pre className="md-plain-pre">
      <code>{code}</code>
    </pre>
  );
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
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [editingSession, setEditingSession] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Refs that survive the event-listener closure
  const sessionIdRef = useRef<string | null>(null);
  const currentAssistantRef = useRef<string | null>(null);
  const sendingRef = useRef(false);
  const lastUserTextRef = useRef("");
  // Input history (↑/↓)
  const historyRef = useRef<string[]>([]);
  const histIdxRef = useRef(-1);
  // Local chat history store
  const storeRef = useRef<HistoryFile>({ active_session: null, titles: {}, messages: {} });
  const storeLoadedRef = useRef(false);
  const saveTimerRef = useRef<number | null>(null);
  const lastSavedRef = useRef<string | null>(null);
  const newSessionRef = useRef<() => void>(() => {});

  const autoResize = useCallback(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    ta.style.height = "auto";
    ta.style.height = Math.min(ta.scrollHeight, 90) + "px";
  }, []);

  useEffect(() => {
    sessionIdRef.current = sessionId;
  }, [sessionId]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  useEffect(() => {
    autoResize();
  }, [input, autoResize]);

  // ── Local persistence: load on mount ──
  useEffect(() => {
    (async () => {
      try {
        const hist = await invoke<HistoryFile>("load_history");
        if (hist && typeof hist === "object") {
          storeRef.current = {
            active_session: hist.active_session ?? null,
            titles: hist.titles ?? {},
            messages: hist.messages ?? {},
          };
          const sid = storeRef.current.active_session;
          const saved = sid ? storeRef.current.messages[sid] : undefined;
          if (sid && saved && saved.length > 0) {
            setSessionId(sid);
            setMessages([
              { role: "system", content: t("chat_restored_session"), id: `sys-${Date.now()}` },
              ...saved,
            ]);
          }
        }
      } catch {
        /* no history yet */
      }
      storeLoadedRef.current = true;
    })();
    refreshSessions();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Local persistence: debounced save on messages/session change ──
  useEffect(() => {
    if (!storeLoadedRef.current) return;
    if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
    saveTimerRef.current = window.setTimeout(() => {
      persistHistory();
    }, 600);
    return () => {
      if (saveTimerRef.current) window.clearTimeout(saveTimerRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [messages, sessionId]);

  // Persist the history object, skipping the IPC + disk write when the serialized
  // content is unchanged since the last save (rename/auto-title/switch paths can
  // produce identical consecutive payloads).
  const saveHistory = (h: HistoryFile) => {
    const snapshot = JSON.stringify(h);
    if (lastSavedRef.current === snapshot) return;
    lastSavedRef.current = snapshot;
    invoke("save_history", { history: h }).catch(() => {});
  };

  const persistHistory = () => {
    const sid = sessionIdRef.current;
    if (!sid) return;
    const h = storeRef.current;
    // Strip base64 images before persisting: each image can be tens of MB, so
    // writing them into history.json on every debounced save would bloat the
    // file and slow startup restore. Text is persisted; images stay in memory
    // only for the current view.
    const msgs = messages
      .filter((m) => m.role !== "system")
      .map((m) => ({ role: m.role, content: m.content, id: m.id }))
      .slice(-200);
    if (msgs.length > 0) {
      h.messages[sid] = msgs;
    }
    h.active_session = sid;
    // Auto-title from the first user message
    if (!h.titles[sid]) {
      const firstUser = msgs.find((m) => m.role === "user");
      if (firstUser?.content) {
        h.titles[sid] =
          firstUser.content.replace(/\s+/g, " ").trim().slice(0, 24) || "…";
      }
    }
    // Cap the number of kept sessions (oldest dropped)
    const keys = Object.keys(h.messages);
    if (keys.length > 30) {
      const drop = keys.slice(0, keys.length - 30);
      for (const k of drop) {
        delete h.messages[k];
        delete h.titles[k];
      }
    }
    saveHistory(h);
  };

  // ── Streaming events from Rust ──
  useEffect(() => {
    const unlisten = listen<{ session_id: string; delta: string; done: boolean }>(
      "chat-stream",
      (event) => {
        const { session_id, delta, done } = event.payload;
        if (done) {
          sendingRef.current = false;
          setSending(false);
          currentAssistantRef.current = null;
          return;
        }
        if (session_id !== sessionIdRef.current || !delta) return;

        setMessages((prev) => {
          let id = currentAssistantRef.current;
          if (!id) {
            id = `a-${Date.now()}`;
            currentAssistantRef.current = id;
            return [...prev, { role: "assistant", content: delta, id }];
          }
          return prev.map((m) =>
            m.id === id ? { ...m, content: m.content + delta } : m
          );
        });
      }
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // ── Session list ──
  const refreshSessions = useCallback(async () => {
    try {
      const list = await invoke<unknown[]>("list_sessions");
      const mapped = (list ?? [])
        .map((s) => {
          const rec = s as Record<string, unknown>;
          const id = String(rec?.id ?? "");
          const title = String(rec?.title ?? rec?.name ?? "");
          const created = rec?.created_at ?? rec?.created;
          return {
            id,
            title,
            created: created ? String(created) : undefined,
          };
        })
        .filter((s) => s.id);
      setSessions(mapped);
    } catch {
      /* server may not support listing; ignore */
    }
  }, []);

  useEffect(() => {
    refreshSessions();
  }, [refreshSessions]);

  // ── Focus input when quick-input hotkey pressed ──
  useEffect(() => {
    const unlisten = listen("quick-input", () => {
      setTimeout(() => textareaRef.current?.focus(), 80);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // ── Tray menu "New Chat" ──
  useEffect(() => {
    const unlisten = listen("new-session", () => {
      newSessionRef.current();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

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

  // ── Copy helper ──
  const copyText = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // clipboard may be unavailable; ignore
    }
  };

  // ── Sending messages (streaming) ──

  const sendMessage = async (textOverride?: string) => {
    const text = (textOverride ?? input).trim();
    const hasImages = attachedImages.length > 0;
    if ((!text && !hasImages) || sendingRef.current) return;

    if (!textOverride) {
      setInput("");
      setAttachedImages([]);
      if (textareaRef.current) {
        textareaRef.current.style.height = "auto";
      }
    }

    sendingRef.current = true;
    setSending(true);

    // Input history (only record the user-typed variant)
    if (!textOverride) {
      historyRef.current.push(text);
      histIdxRef.current = -1;
    }
    lastUserTextRef.current = text;

    const userMsg: Message = {
      role: "user",
      content: text,
      images: hasImages ? attachedImages.map((img) => img.dataUrl) : undefined,
      id: `u-${Date.now()}`,
    };
    setMessages((prev) => [...prev, userMsg]);

    try {
      let sid = sessionIdRef.current;
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

      await invoke<string>("send_chat_stream", {
        sessionId: sid,
        message: messagePayload,
      });

      refreshSessions();

      // Notify if the window is hidden while the reply completes
      try {
        const win = getCurrentWindow();
        if (!(await win.isVisible())) {
          sendNotification({
            title: "HermesTray",
            body: t("chat_reply_notification"),
          });
        }
      } catch {
        /* window API unavailable; ignore */
      }
    } catch (e) {
      sendingRef.current = false;
      setSending(false);
      currentAssistantRef.current = null;
      setMessages((prev) => [
        ...prev,
        {
          role: "assistant",
          content: `${t("chat_error")}: ${e}`,
          id: `e-${Date.now()}`,
        },
      ]);
    } finally {
      setTimeout(() => textareaRef.current?.focus(), 0);
    }
  };

  const cancelChat = async () => {
    if (sessionIdRef.current) {
      try {
        await invoke("cancel_chat", { sessionId: sessionIdRef.current });
      } catch {
        /* ignore */
      }
    }
  };

  const regenerate = () => {
    if (sendingRef.current || !lastUserTextRef.current) return;
    sendMessage(lastUserTextRef.current);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    } else if (e.key === "ArrowUp" && !e.shiftKey && (input === "" || histIdxRef.current >= 0)) {
      e.preventDefault();
      const h = historyRef.current;
      if (h.length === 0) return;
      const idx = Math.max(-1, histIdxRef.current - 1);
      histIdxRef.current = idx;
      setInput(idx === -1 ? "" : h[idx]);
      setTimeout(autoResize, 0);
    } else if (e.key === "ArrowDown" && histIdxRef.current >= 0) {
      e.preventDefault();
      const h = historyRef.current;
      const idx = Math.min(h.length - 1, histIdxRef.current + 1);
      histIdxRef.current = idx;
      setInput(h[idx]);
      setTimeout(autoResize, 0);
    }
  };

  // ── Session management ──

  const newSession = () => {
    setSessionId(null);
    setAttachedImages([]);
    setMessages([
      {
        role: "system",
        content: t("chat_new_session_started"),
        id: `sys-${Date.now()}`,
      },
    ]);
    refreshSessions();
  };
  newSessionRef.current = newSession;

  const resumeSession = async (id: string) => {
    if (id === sessionIdRef.current) return;
    setSessionId(id);
    setAttachedImages([]);
    // Prefer locally persisted messages for instant display
    const local = storeRef.current.messages[id];
    setMessages([
      { role: "system", content: t("chat_resumed_session"), id: `sys-${Date.now()}` },
      ...(local ?? []),
    ]);
    // Then try to load the full history from the server
    try {
      const msgs = await invoke<unknown[]>("get_session_messages", {
        sessionId: id,
      });
      const mapped: Message[] = (msgs ?? [])
        .map((m, i) => toMessage(m, i))
        .filter((m): m is Message => m !== null);
      if (mapped.length > 0) {
        setMessages(mapped);
      }
    } catch {
      /* history endpoint may be unsupported; keep local copy */
    }
  };

  const deleteSession = async (id: string) => {
    try {
      await invoke("delete_session", { sessionId: id });
      setSessions((prev) => prev.filter((s) => s.id !== id));
      if (sessionIdRef.current === id) {
        newSession();
      }
    } catch {
      /* ignore */
    }
  };

  // ── Rename (double-click in sidebar) ──
  const titleForId = (id: string): string => {
    const local = storeRef.current.titles[id];
    if (local) return local;
    const s = sessions.find((x) => x.id === id);
    if (s?.title) return s.title;
    return id.length > 16 ? `${id.slice(0, 8)}…${id.slice(-6)}` : id;
  };

  const startRename = (id: string) => {
    setEditingSession(id);
    setEditTitle(titleForId(id));
  };

  const commitRename = () => {
    const id = editingSession;
    setEditingSession(null);
    if (id && editTitle.trim()) {
      storeRef.current.titles[id] = editTitle.trim().slice(0, 40);
      saveHistory(storeRef.current);
    }
  };

  // ── Export current chat to Markdown ──
  const exportChat = async () => {
    if (messages.length === 0) return;
    const sid = sessionIdRef.current;
    const base =
      (sid && storeRef.current.titles[sid]) ||
      `hermestray-chat-${new Date().toISOString().slice(0, 10)}`;
    const safe = base.replace(/[\\/:*?"<>|]/g, "_").slice(0, 40);
    try {
      const path = await saveDialog({
        title: t("chat_export"),
        defaultPath: `${safe}.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return; // cancelled
      const lines: string[] = [];
      lines.push(`# ${base}`);
      lines.push("");
      lines.push(`*Exported ${new Date().toLocaleString()} · HermesTray*`);
      lines.push("");
      for (const m of messages) {
        if (m.role === "system") continue;
        lines.push(`## ${m.role === "user" ? "User" : "Assistant"}`);
        lines.push("");
        if (m.images && m.images.length > 0) {
          lines.push(`*(attached ${m.images.length} image${m.images.length > 1 ? "s" : ""})*`);
          lines.push("");
        }
        if (m.content) lines.push(m.content);
        lines.push("");
      }
      await writeTextFile(path, lines.join("\n"));
    } catch {
      /* user cancelled or write failed */
    }
  };

  const toMessage = (m: unknown, i: number): Message | null => {
    const rec = m as Record<string, unknown>;
    const role = rec?.role === "user" || rec?.role === "assistant" ? rec.role : null;
    if (!role) return null;
    const content = rec?.content;
    let text = "";
    const images: string[] = [];
    if (typeof content === "string") {
      text = content;
    } else if (Array.isArray(content)) {
      for (const part of content as unknown[]) {
        const p = part as Record<string, unknown>;
        if (p?.type === "text") {
          text += (text ? "\n" : "") + String(p.text ?? "");
        } else if (p?.type === "image_url") {
          const imgUrl = (p.image_url as Record<string, unknown>)?.url;
          if (typeof imgUrl === "string") images.push(imgUrl);
        }
      }
    }
    return {
      role,
      content: text,
      images: images.length > 0 ? images : undefined,
      id: `h-${i}-${role}`,
    };
  };

  // ── Markdown ──
  const extractText = (node: unknown): string => {
    if (node == null || typeof node === "boolean") return "";
    if (typeof node === "string" || typeof node === "number") return String(node);
    if (Array.isArray(node)) return node.map(extractText).join("");
    if (typeof node === "object" && "props" in (node as object)) {
      return extractText((node as { props: { children: unknown } }).props.children);
    }
    return "";
  };

  const extractLang = (node: unknown): string => {
    if (Array.isArray(node)) {
      for (const n of node) {
        const l = extractLang(n);
        if (l) return l;
      }
      return "";
    }
    if (node && typeof node === "object" && "props" in (node as object)) {
      const props = (node as { props: { className?: string; children: unknown } }).props;
      if (props.className) {
        const m = /language-([\w-]+)/.exec(props.className);
        if (m) return m[1];
      }
      return extractLang(props.children);
    }
    return "";
  };

  const Markdown = ({ text }: { text: string }) => (
    <div className="md">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ href, children }) => (
            <a
              href={href}
              onClick={(e) => {
                e.preventDefault();
                if (href) window.open(href, "_blank");
              }}
            >
              {children}
            </a>
          ),
          pre: ({ children }) => {
            const codeText = extractText(children);
            const lang = extractLang(children);
            return (
              <div className="md-pre">
                <button
                  className="md-copy"
                  onClick={() => copyText(codeText)}
                >
                  {t("chat_copy")}
                </button>
                <HighlightedCode code={codeText} lang={lang} />
              </div>
            );
          },
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );

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
        {msg.content &&
          (msg.role === "assistant" ? (
            <Markdown text={msg.content} />
          ) : (
            <div className="message-text">{msg.content}</div>
          ))}
      </>
    );
  };

  return (
    <div className="chat-wrap">
      {/* ── Session sidebar ── */}
      <div className="session-sidebar">
        <div className="session-sidebar-title">{t("chat_sessions")}</div>
        <div className="session-list">
          {sessions.length === 0 && (
            <div className="session-empty">{t("chat_empty_sessions")}</div>
          )}
          {sessions.map((s) => (
            <div
              key={s.id}
              className={`session-item${sessionId === s.id ? " active" : ""}`}
              onClick={() => resumeSession(s.id)}
              onDoubleClick={() => startRename(s.id)}
              title={editingSession === s.id ? t("chat_rename_session") : s.id}
            >
              {editingSession === s.id ? (
                <input
                  className="session-rename-input"
                  value={editTitle}
                  autoFocus
                  onChange={(e) => setEditTitle(e.target.value)}
                  onClick={(e) => e.stopPropagation()}
                  onDoubleClick={(e) => e.stopPropagation()}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") commitRename();
                    if (e.key === "Escape") setEditingSession(null);
                  }}
                  onBlur={commitRename}
                />
              ) : (
                <span className="session-item-title">{titleForId(s.id)}</span>
              )}
              <button
                className="session-item-del"
                onClick={(e) => {
                  e.stopPropagation();
                  deleteSession(s.id);
                }}
                title={t("chat_delete_session")}
              >
                ×
              </button>
            </div>
          ))}
        </div>
      </div>

      {/* ── Chat main ── */}
      <div className="chat-main">
        <div className="chat-messages">
          {messages.map((msg) => (
            <div key={msg.id} className={`message ${msg.role}`}>
              {renderMessageContent(msg)}
              {msg.role === "assistant" && msg.content && !sending && (
                <div className="message-actions">
                  <button
                    className="msg-action"
                    onClick={() => copyText(msg.content)}
                    title={t("chat_copy")}
                  >
                    ⧉
                  </button>
                  <button
                    className="msg-action"
                    onClick={regenerate}
                    title={t("chat_regenerate")}
                    disabled={sendingRef.current}
                  >
                    ↻
                  </button>
                </div>
              )}
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
              <button className="btn btn-new" onClick={newSession}>
                {t("chat_new_session")}
              </button>
              <button className="btn btn-new" onClick={exportChat} title={t("chat_export")}>
                {t("chat_export")}
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
              {sending ? (
                <button className="btn btn-stop" onClick={cancelChat}>
                  {t("chat_stop")}
                </button>
              ) : (
                <button
                  className="btn btn-send"
                  onClick={() => sendMessage()}
                  disabled={(!input.trim() && attachedImages.length === 0) || sending}
                >
                  {t("chat_send")}
                </button>
              )}
            </div>
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
    </div>
  );
}
