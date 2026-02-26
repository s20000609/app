# iOS 魔改路線圖（今日可完成範圍）

目標：**回家前** 有「可跑的 iOS 專案骨架 + Prompt 組裝 + 呼叫 API」，其餘（動態記憶 CoreML、完整 UI）之後補。

---

## 今日範圍（Phase 1 + Phase 2 起手）

### Phase 1：專案與 Prompt（約 1–2 小時）

- [ ] **在 Mac 上開新 Xcode 專案**  
  - File → New → Project → **App**（SwiftUI, iOS 16+）。  
  - 專案名例如 `LettuceAI`，Bundle ID 自訂（需與 p12 對應）。
- [ ] **把本 repo 的 Swift 參考碼加進專案**  
  - 複製 **`ios-reference/PromptEngine.swift`** 與 **`ios-reference/OpenRouterClient.swift`** 到 Xcode 專案（拖進專案或 Add Files）。  
  - 確認 target 有勾選，能編過。
- [ ] **驗證 Prompt 組裝**  
  - 在任意 View 或 App 啟動時呼叫 `PromptEngine.renderWithContext(...)` 與 `PromptEngine.buildSystemPromptEntries(...)`，用假資料，把結果印在 console 或顯示在畫面上。  
  - 確認 `{{char.name}}`、`{{context_summary}}`、`{{key_memories}}` 等有被替換。

### Phase 2：呼叫 LLM API（今日至少打通）

- [ ] **新增 API 模組**  
  - 本 repo 已提供 **`ios-reference/OpenRouterClient.swift`**，可直接加入 Xcode：  
    - `send(...)`：非串流，`async throws -> String`。  
    - `sendStream(...)`：串流，`onDelta` 回呼每個 delta。  
  - 打 `https://openrouter.ai/api/v1/chat/completions`，body 用 OpenAI 格式。  
  - 若串流不穩，可先只用 `send(...)` 非串流。
- [ ] **接到 PromptEngine 的輸出**  
  - 用 `buildSystemPromptEntries` 得到 `[SystemPromptEntry]`，把 `content` 合併成一個 `systemPrompt` 字串。  
  - 再加上 `[.user("你好")]` 之類的對話，傳給 `OpenRouterClient.send(...)`。  
  - 在畫面上顯示回傳內容（或 streaming 的累積文字）。
- [ ] **API Key**  
  - 先寫死在開發用 config（之後改 Keychain 或設定頁）。

完成以上 = **今日可帶回家的狀態**：iOS app 能組 prompt、能打 API、能收到（或串流）回覆。

---

## 之後補上（Phase 3–5）

| Phase | 內容 | 備註 |
|-------|------|------|
| **3** | 本地儲存（角色、session、對話紀錄） | SQLite / SwiftData / Core Data 擇一，schema 對齊現有 Tauri 的 sessions + messages。 |
| **4** | 動態記憶（CoreML） | 在 Mac 上把現有 `.onnx` 轉 CoreML；iOS 用 CoreML 跑 embedding，cosine 相似度 + 檢索邏輯用 Swift 重寫（可對照 `src-tauri/.../dynamic_memory.rs`）。 |
| **5** | UI 與功能對齊 | 角色列表、聊天畫面、設定（API key、模型選擇）、lorebook／世界書 等，依需求逐步做。 |

---

## 本 repo 提供的參考

- **`ios-reference/PromptEngine.swift`**  
  - 與 `src/core/prompts/PromptEngine.ts` 對齊的 Swift 版：型別、`renderWithContext`、`buildSystemPromptEntries`、預設條目、pure mode 規則。  
  - 可直接放進 Xcode 專案使用。
- **`ios-reference/OpenRouterClient.swift`**  
  - 打 OpenRouter API 的範例：`send(...)` 非串流、`sendStream(...)` 串流，與 PromptEngine 輸出接在一起即可。
- **`docs/ios-migration-notes.md`**  
  - 原開發者說法、1GB／動態記憶／側載、技術因果。
- **`src/core/prompts/PromptEngine.ts`**  
  - TypeScript 版邏輯與變數替換，可對照 Swift 或之後加單元測試。

---

## 今日檢查清單（回家前打勾）

1. [ ] Xcode 專案建好、可跑在模擬器或真機。  
2. [ ] `PromptEngine.swift` 已加入並編過。  
3. [ ] 假資料呼叫 `renderWithContext` / `buildSystemPromptEntries`，輸出正確。  
4. [ ] `OpenRouterClient` 能打 API 並收到回覆（或 SSE 串流）。  
5. [ ] 畫面上至少能看到「一則自己發的 + 一則 API 回傳」的對話。

做到這裡就算「魔改起手式」完成，之後再補儲存、CoreML、完整 UI。
