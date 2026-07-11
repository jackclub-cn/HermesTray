      </div>
      <div className="chat-input-bar">
        <textarea
          ref={textareaRef}
          className="chat-input"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t("chat_placeholder")}
          rows={1}
          disabled={sending}
        />
        <div className="chat-actions">
          <button className="btn btn-send" onClick={sendMessage} disabled={!input.trim() || sending}>
            {t("chat_send")}
          </button>
          <button className="btn btn-new" onClick={newSession} title={t("chat_new_session")}>
            +
          </button>
        </div>
      </div>
    </>
  );
}