# iOS 參考程式碼（魔改用）

- **PromptEngine.swift**：與 `src/core/prompts/PromptEngine.ts` 對齊的 Swift 版。  
  - 用法：拖進 Xcode 專案，在需要組 prompt 的地方呼叫 `PromptEngine.renderWithContext(...)` 或 `PromptEngine.buildSystemPromptEntries(...)`，再用 `PromptEngine.systemPromptString(from: entries)` 得到給 API 的 system 字串。
- **OpenRouterClient.swift**：打 OpenRouter（OpenAI 相容）API。  
  - `send(...)` 非串流、`sendStream(...)` 串流；把 PromptEngine 產出的 system 字串與 messages 傳入即可。
- 詳細步驟見 **docs/ios-migration-roadmap.md**。
