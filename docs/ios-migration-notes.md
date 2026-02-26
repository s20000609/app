# iOS 遷移與動態記憶限制說明

## iOS 適配度總覽（一句話版）

| 情境 | 適配度 | 說明 |
|------|--------|------|
| **Tauri 編成 iOS app，不開動態記憶** | ✅ 理論可行 | 需在 **Mac 上**編譯；聊天、角色、Session、世界書、Prompt 都會跑，**只有動態記憶被關掉**（ONNX 在 iOS 未啟用）。 |
| **Tauri iOS + 要動態記憶（用 ONNX）** | ❌ 不建議 | ONNX 在 iOS 上沒載入路徑（`ort_runtime.rs` 排除 ios），且沙盒／體積約 1GB，實務上不可行。 |
| **要 iOS 又有動態記憶** | 🟡 走 CoreML 路線，**程式已鋪好、剩模型與接線** | 見下方「我們已為 iOS 做了什麼」與「還缺什麼」。 |

---

### 我們已為 iOS 做了什麼（純 TS，無 Tauri 依賴）

這些都在 **`src/core/`** 裡，可在 React Native 或原生 iOS 專案裡直接沿用或對照重寫：

1. **Prompt 組裝**：`prompts/PromptEngine.ts` — 把 system prompt 組好（含 `{{key_memories}}`、`{{context_summary}}`），只吃「已檢索好的 key memories」。
2. **記憶檢索**：`memory/MemoryRetrieval.ts` — 餘弦相似度 + 門檻 + category 多樣性，`retrieveRelevantMemories(queryEmbedding, memories, limit, minSimilarity)`。
3. **一鍵接線**：`memory/embeddingProvider.ts` — `getKeyMemoriesForRequest(sessionId, queryText, { getSessionMemories, embeddingProvider, options })`，發送前呼叫就能拿到要灌進 prompt 的 key memories。
4. **抽象**：`EmbeddingProvider`（之後 iOS 接 CoreML）、`GetSessionMemories`（之後 iOS 接本地儲存）、`stubEmbeddingProvider`（未接 CoreML 前先不崩潰）。

桌面端若要「用 TS 做檢索」：`storage/repo.ts` 有 `getSessionMemoriesFromTauri`、`tauriEmbeddingProvider`，可和上面接在一起用。

---

### 已接好（Tauri iOS 同一 repo）

| 項目 | 說明 |
|------|------|
| **儲存** | 在 Tauri iOS 上仍用後端 SQLite；前端用 **getSessionMemoriesFromTauri(sessionId)** 讀出該 session 的 memory_embeddings（`storage/repo.ts`）。 |
| **發送前接線** | 在 **iOS** 上發送一般訊息時，前端會先呼叫 **getKeyMemoriesForRequest(sessionId, message, { getSessionMemories: getSessionMemoriesFromTauri, embeddingProvider: stubEmbeddingProvider })**，把回傳的 key memories 傳給後端；後端 **chat_completion** 接受可選參數 **keyMemoriesJson**，有值就用它當 relevant memories、不跑 ONNX。見 `chat/manager.ts`、`useChatController.ts`、Rust `ChatCompletionArgs.key_memories_json`。 |

### 還缺什麼（才能「在 iOS 上跑起來且有動態記憶」）

| 項目 | 誰來做 | 說明 |
|------|--------|------|
| **iOS 建置／入口** | 你 | 在 **Mac 上**用 Tauri 建出 iOS IPA（或開 RN／原生專案）。 |
| **CoreML 嵌入（最後一步）** | 你 | 把 ONNX 嵌入模型轉成 CoreML，在 iOS 上實作 **EmbeddingProvider**（文字 → CoreML → `number[]`），在發送前接線處改用此 provider 取代 **stubEmbeddingProvider**，動態記憶就完整（目前用 stub 時 key memories 為空，但不會崩潰）。 |

---

### 總結一句

- **現在**：專案對 iOS 的「適配」= **Tauri 可編 iOS，但動態記憶在 iOS 上不會跑**（ONNX 被關掉）。
- **我們多做的**：把 **prompt 組裝 + 記憶檢索 + 一鍵接線** 都做成 **純 TypeScript**，讓你在 **不依賴 Tauri 的 iOS 專案** 裡也能用同一套邏輯；你只要在 iOS 端補 **儲存**、**發送前接線**、最後 **CoreML**，就能在 iOS 上跑出動態記憶。

---

## 原開發者對 iOS 版的說明（節錄）

- Tauri 可編譯成 iOS / macOS，**實際測試可跑**。
- 若要**分發**，需要 Apple Developer Account（約 100 USD/年）、上架 App Store 體驗差、還要向 Apple 申請檔案／相機／Wi‑Fi 等權限，可能被拒或審核近一個月 → 原開發者 **TL;DR: not yet**。
- **後續計畫**：原開發者說 *"i will add some of the Apple-compatibility stuff that i removed originally"* — 會把**先前拿掉的 Apple 相容相關改動**再補回去，之後 fork 或上游的 iOS/macOS 支援可能會好一點，但動態記憶與側載等限制仍如上述。
- **關於動態記憶**：原開發者原話 — *"Dynamic Memory will not be able to run in iOS unless i make the app size like 1 gb or smth"*  
  → 意思不是「超過 1GB 所以功能壞掉」，而是：**若要在 iOS 上支援動態記憶（沿用目前 ONNX 做法），app 體積就得做到約 1GB，因此選擇不在 iOS 版提供動態記憶。**

## Fork / 自編與側載的現實（原開發者補充）

- **可以 fork**，但有一串前提與限制：
  - **一定要有 macOS**：編譯、跑 dev、或丟 TestFlight 都**必須在 Apple Mac/MacBook 上**，無法在 Windows/Linux 完成 iOS 建置。
  - **側載 (sideload) 不友善**：原開發者用 **iPhone 16 Pro** 實測，**iOS 會在幾天後隨機刪除側載的 app**（可能與免費開發者憑證 7 天效期或系統清理有關）。整體來說「讓 iOS 吃進側載 app 很麻煩」(*getting iOS to sideload it is pain*)。

若你打算用 p12 自簽、不上架：需有 Mac 才能產出 IPA，且側載後要預期**可能被系統自動刪除**，需定期重裝或考慮付費開發者帳號延長憑證效期。

## 若你接受 1GB 且已有 p12：可以直接編譯嗎？

- **分發／簽名方面**：你接受 1GB、又有 p12（應為付費開發者帳號），原開發者提到的「分發麻煩」「側載很痛」對你影響較小：自簽安裝、ad-hoc 或 TestFlight 都可做，憑證效期也較長，不必每 7 天重裝。
- **能不能「直接編譯」**：
  - **一定要在 Mac 上**編譯／跑 dev／產 IPA，這點無法繞過。
  - 專案已設好 **iOS target**（`capabilities/mobile.json` 含 iOS、Cargo 有 `target_os = "ios"` 條件），所以**理論上可以試著建置**。
  - **但**：目前程式碼裡 **ONNX Runtime 在 iOS 上沒有載入路徑**——`ort_runtime.rs` 裡下載／設定 `ORT_DYLIB_PATH` 與 `preload_dylib` 都是 `#[cfg(not(ios))]`，**iOS 不會執行**。也就是說 **動態記憶在現有 iOS 建置下很可能根本跑不起來**（一用就錯或 init 失敗），除非有人補上「在 iOS 上打包或下載 ONNX Runtime」的邏輯（正是原開發者說要補的 "Apple-compatibility stuff" 之一）。
- **建議**：
  1. **先試著在 Mac 上編譯**：`npm run tauri build`（或 Tauri 2 的 iOS 建置指令），看能否產出 IPA。若編譯就失敗，可能是原開發者拿掉的 Apple 相容程式還沒補回。
  2. **若編過但動態記憶不能用**：等原開發者補回 Apple 相容（含 iOS 的 ONNX 載入），或自己改 Rust 在 iOS 上 bundle／下載 ORT。
  3. **若你要完整控制體積與體驗**：走「魔改」路線，改成 **iOS 原生（Swift + CoreML）**，動態記憶用 CoreML 取代 ONNX，不再依賴 Tauri 建置。

## 其他功能本來就對 iOS 有支援嗎？

**對，目前真正大的問題只有動態記憶。**

- **Tauri 本身**：可編譯 iOS / macOS，原開發者已測過可跑；專案裡 `capabilities/mobile.json` 含 iOS、Cargo 有 `target_os = "ios"` 條件編譯。
- **聊天、呼叫 LLM、角色、Session、世界書 (lorebook)、Prompt 組裝**：都是同一套 Rust 後端 + 前端，在 Tauri 的 iOS 建置下會一起跑，**沒有被特別關掉**。
- **唯一例外**：**動態記憶** 依賴 ONNX Runtime + 約 86MB 嵌入模型；目前程式碼在 iOS 上**沒有載入 ONNX 的路徑**（`ort_runtime.rs` 裡相關邏輯是 `#[cfg(not(ios))]`），所以現成 iOS 建置要嘛不包含動態記憶、要嘛一用就掛。若補上 iOS 的 ONNX 支援，體積就會到約 1GB。

結論：**直接編出現有 Tauri 的 iOS 版來用，是可行的**；只是「動態記憶」在現狀下不能用，要嘛接受沒有這功能，要嘛等／自己補 ONNX（1GB），要嘛之後魔改用 CoreML 做動態記憶。

## 已知問題：Tauri 編譯成 IPA 時

- **現象**：以目前 Tauri 專案編譯成 iOS IPA 時，**若包含動態記憶（ONNX Runtime + 嵌入模型），整體體積會到約 1GB**。
- **影響**：原開發者**選擇不在 iOS 版啟用動態記憶**，避免推出 1GB+ 的 app；若自行編譯並啟用，則會得到體積約 1GB 的 IPA，且 ONNX 在 iOS 上仍有 RAM／穩定性風險。
- **分發方式**：本應用**不上架 App Store**，以 **p12 證書自簽**後自行安裝使用（sideload / ad-hoc）。

## 因果關係整理

| 說法 | 說明 |
|------|------|
| **「要動態記憶 → app 會變約 1GB」** | 正確。在 iOS 上用現有做法（ONNX Runtime + 約 86MB 嵌入模型），打包後 IPA 體積會到約 1GB，所以原開發者說 "unless i make the app size like 1 gb"。 |
| **「超過 1GB → 動態記憶失效」** | 不是「系統因 1GB 關閉功能」。而是：(1) 原開發者**不願**推出 1GB 的 iOS 版，所以**不提供**動態記憶；(2) 若自己編譯並啟用，除了體積大，ONNX 在真機上也可能有 **RAM/OOM 或閃退** 等穩定性問題。 |

## 技術原因（對應現有程式碼）

1. **ONNX 向量模型**  
   - 動態記憶使用約 **86MB** 的 `.onnx` 嵌入模型（見 `src-tauri/src/embedding_model/specs.rs`、`download.rs`）。  
   - 首次啟動時下載，並用 **ONNX Runtime** 做語義嵌入與向量檢索。

2. **ONNX Runtime 在 iOS 上的成本**  
   - ONNX Runtime 在 iOS 上體積與記憶體占用都偏大。  
   - 與 Tauri 的 WebView、Rust 依賴、前端資源等一起打包進 IPA，容易把總體積推到 **1GB+**，且增加記憶體壓力（OOM 風險）。

因此：**「Tauri 編譯成 IPA → 超過 1GB → 動態記憶無法實用」** 是同一條因果鏈。

## 遷移方向（與目前進度的關係）

| 項目 | 說明 |
|------|------|
| **目標** | 放棄 Tauri 架構，改為 **iOS 原生**（例如 SwiftUI + CoreML），以縮小體積並正常使用動態記憶。 |
| **嵌入模型** | 在 Mac 上將現有 `.onnx` 轉成 **CoreML**，在 iOS 上用 CoreML 跑嵌入與向量檢索，取代 ONNX Runtime。 |
| **Prompt 組裝** | 已將 Rust 的 prompt 邏輯移植到純 TypeScript（`src/core/prompts/PromptEngine.ts`），無 Tauri 依賴。  
   iOS 版可依同邏輯用 **Swift** 重寫，或暫時用輕量 JS 引擎跑同一套 TS，以保持行為一致。 |
| **LLM 與串流** | 在 iOS 上改為用 **URLSession**（或類似）呼叫 OpenRouter/OpenAI 相容 API，自行處理 SSE 串流。 |

## 小結

- **目前狀況**：在 iOS 上用 Tauri + ONNX 做動態記憶會讓 IPA 約 1GB，原開發者因此不在 iOS 版提供該功能；你以 p12 自簽、不上架，若自編並啟用則會面對體積與可能的 RAM 問題。
- **對應做法**：改走 iOS 原生 + CoreML 嵌入 + 沿用/重寫 Prompt 組裝邏輯，以控制體積並在自用版保留動態記憶功能。

此文件可隨遷移進度更新。

---

## iOS 動態記憶遷移進度（要能在 iOS 跑起來且有動態記憶）

### 已完成

| 項目 | 說明 |
|------|------|
| **Prompt 組裝** | `src/core/prompts/PromptEngine.ts`：純 TypeScript，無 Tauri。可接收已檢索好的 `session.memoryEmbeddings` 並替換 `{{key_memories}}`、`{{context_summary}}`。 |
| **記憶檢索邏輯** | `src/core/memory/MemoryRetrieval.ts`：餘弦相似度、minSimilarity 過濾、category 多樣性（每類最多 2 筆）。API：`retrieveRelevantMemories(queryEmbedding, memories, limit, minSimilarity)`。 |
| **iOS 參考** | `ios-reference/`：Swift 版 PromptEngine、OpenRouter 客戶端，供原生 iOS 或 RN 參考。 |
| **Embedding 抽象與流程** | `src/core/memory/embeddingProvider.ts`：**EmbeddingProvider** 介面（`computeEmbedding(text)`）、**retrieveKeyMemoriesForQuery**、**getKeyMemoriesForRequest(sessionId, queryText, { getSessionMemories, embeddingProvider, options })**（一鍵接線）、**stubEmbeddingProvider**（embedding 未就緒時回傳 []）、**GetSessionMemories** 型別（儲存抽象）。桌面端可注入 Tauri；iOS 端實作 getSessionMemories（讀本地儲存）+ 之後注入 CoreML，**轉 CoreML 留最後**。 |
| **桌面端 Tauri 接線** | `src/core/storage/repo.ts`：**getSessionMemoriesFromTauri(sessionId)**、**tauriEmbeddingProvider**。桌面端若要走 TS 檢索路徑（例如預覽或測試），可 `getKeyMemoriesForRequest(sessionId, queryText, { getSessionMemories: getSessionMemoriesFromTauri, embeddingProvider: tauriEmbeddingProvider })` 取得 key memories。 |

### 決策：不接外部 API 做動態記憶

動態記憶的 **embedding 一律在裝置端產生**，不呼叫遠端 Embedding API。  
因此 iOS 上的 embedding 來源只能是：**在裝置上跑嵌入模型**。實務上僅 **CoreML** 可行（見下）。

**為何不選 ONNX on iOS**：除了體積約 1GB 外，**iOS 沙盒與 App 限制**會影響 ONNX Runtime 的載入與執行（例如動態庫載入路徑、檔案存取、權限等），現有 `ort_runtime.rs` 也以 `#[cfg(not(ios))]` 排除 iOS，因此 **在 iOS 上依賴 ONNX 不可行**；動態記憶的裝置端嵌入應以 **CoreML** 實作。

---

### 尚未完成（缺了才能「在 iOS 上跑起來且有動態記憶」）

| 項目 | 說明 |
|------|------|
| **1. Embedding 來源（裝置端）** | 桌面版用 Rust ONNX；**iOS 上須改用 CoreML**（ONNX 因沙盒／載入限制不適用）。做法：將現有 ONNX 嵌入模型轉成 **CoreML**，在 iOS 上用 CoreML 跑推理，產出 query 與每筆記憶的 embedding。 |
| **2. 儲存** | Session 與 `memory_embeddings`（含 `embedding: number[]`）在 iOS 上的讀寫。若走 Tauri iOS：沿用現有 SQLite；若走 React Native / 原生：需實作對應儲存（例如 AsyncStorage + JSON 或原生 SQLite）。 |
| **3. 流程接線** | 發送訊息前：用「當前 query」在**裝置上**以 CoreML 算出 **query embedding** → 從儲存讀出該 session 的 **memory_embeddings** → 呼叫 `retrieveRelevantMemories(queryEmbedding, memories, limit, minSimilarity)` → 將回傳結果當成 **key memories** 傳入 PromptEngine（或 Swift 版）組 prompt → 再呼叫 LLM。 |
| **4. iOS 建置與入口** | Tauri iOS 建置或 React Native / 原生 app，皆需接上 **CoreML 嵌入** + 上述儲存與流程接線。 |

### 建議下一步

1. **在 Mac 上建出 Tauri iOS**：編譯產出 IPA，確認一般聊天與發送流程可跑；目前 iOS 上 key memories 會用 stub（空陣列），不會崩潰。  
2. **轉 CoreML**：將 ONNX 嵌入模型轉成 CoreML，在 iOS 上實作 **EmbeddingProvider**（輸入文字 → CoreML 推理 → 回傳 `number[]`），在 `useChatController` 的 iOS 分支裡改用此 provider 取代 **stubEmbeddingProvider**，即完成動態記憶。
