/**
 * embeddingProvider.ts
 *
 * Embedding 抽象層與「query 文字 → 檢索」流程，供 iOS / 純 TS 動態記憶使用。
 * 桌面端可注入 Tauri compute_embedding；iOS 端之後注入 CoreML。
 * 對應 Rust：dynamic_memory.rs 的 FALLBACK_* 與 retrieval limit/min_similarity。
 */

import type { MemoryEmbedding } from "./MemoryRetrieval";
import { retrieveRelevantMemories } from "./MemoryRetrieval";

// ---------------------------------------------------------------------------
// 預設參數（對應 Rust FALLBACK_RETRIEVAL_LIMIT、FALLBACK_MIN_SIMILARITY）
// ---------------------------------------------------------------------------

export const DEFAULT_RETRIEVAL_LIMIT = 5;
export const DEFAULT_MIN_SIMILARITY = 0.35;

// ---------------------------------------------------------------------------
// Embedding 提供者介面（桌面 = Tauri invoke，iOS = CoreML）
// ---------------------------------------------------------------------------

/**
 * 由呼叫端注入：輸入文字，回傳該文字的 embedding 向量。
 * 桌面：storageBridge.computeEmbedding(text)
 * iOS：之後實作 CoreML 推理後注入。
 */
export interface EmbeddingProvider {
  computeEmbedding(text: string): Promise<number[]>;
}

/**
 * 當 embedding 尚未就緒（例如 iOS 尚未接 CoreML）時使用，回傳空向量，檢索結果為 []。
 */
export const stubEmbeddingProvider: EmbeddingProvider = {
  computeEmbedding: async () => [],
};

// ---------------------------------------------------------------------------
// 儲存抽象：取得某 session 的完整 memory_embeddings（由呼叫端實作）
// ---------------------------------------------------------------------------

/**
 * 由平台注入：給 sessionId 回傳該 session 的 memory_embeddings。
 * 桌面：可從 Tauri 讀出 session 後取 session.memoryEmbeddings。
 * iOS：從 AsyncStorage / SQLite 等讀出對應 JSON。
 */
export type GetSessionMemories = (sessionId: string) => Promise<MemoryEmbedding[]>;

// ---------------------------------------------------------------------------
// 檢索選項（可從 app 設定讀取，或使用預設）
// ---------------------------------------------------------------------------

export interface DynamicMemoryRetrievalOptions {
  limit?: number;
  minSimilarity?: number;
}

function applyDefaults(options?: DynamicMemoryRetrievalOptions): {
  limit: number;
  minSimilarity: number;
} {
  return {
    limit: options?.limit ?? DEFAULT_RETRIEVAL_LIMIT,
    minSimilarity: options?.minSimilarity ?? DEFAULT_MIN_SIMILARITY,
  };
}

// ---------------------------------------------------------------------------
// 流程：query 文字 → embedding → retrieveRelevantMemories
// ---------------------------------------------------------------------------

/**
 * 依「當前 query 文字」從 memories 中檢索要注入 prompt 的 key memories。
 * 使用注入的 EmbeddingProvider 取得 query embedding，再呼叫 retrieveRelevantMemories。
 * 發送訊息前呼叫此函數即可取得 key memories，再傳入 PromptEngine 的 session.memoryEmbeddings。
 *
 * @param queryText - 通常為最後一則用戶訊息或近期對話摘要
 * @param memories - 該 session 的完整 memory_embeddings（含 embedding 向量）
 * @param embeddingProvider - 由平台注入（Tauri / CoreML）
 * @param options - limit、minSimilarity，不傳則用預設
 * @returns 檢索後的 MemoryEmbedding[]，可直接當作 session.memoryEmbeddings 傳給 buildSystemPromptEntries / render
 */
export async function retrieveKeyMemoriesForQuery(
  queryText: string,
  memories: MemoryEmbedding[],
  embeddingProvider: EmbeddingProvider,
  options?: DynamicMemoryRetrievalOptions,
): Promise<MemoryEmbedding[]> {
  const { limit, minSimilarity } = applyDefaults(options);

  if (memories.length === 0) {
    return [];
  }

  const queryEmbedding = await embeddingProvider.computeEmbedding(queryText);
  if (!queryEmbedding.length) {
    return [];
  }

  return retrieveRelevantMemories(queryEmbedding, memories, limit, minSimilarity);
}

// ---------------------------------------------------------------------------
// 一鍵接線：sessionId + queryText → key memories（供 iOS / 純 TS 發送前呼叫）
// ---------------------------------------------------------------------------

export interface GetKeyMemoriesForRequestParams {
  getSessionMemories: GetSessionMemories;
  embeddingProvider: EmbeddingProvider;
  options?: DynamicMemoryRetrievalOptions;
}

/**
 * 發送訊息前呼叫：依 sessionId 取得該 session 的 memories，再依 queryText 檢索出 key memories。
 * iOS 端實作 getSessionMemories（從本地儲存讀出）、embeddingProvider（之後接 CoreML），
 * 即可與 PromptEngine 接線（回傳值當作 session.memoryEmbeddings 傳入 buildSystemPromptEntries / render）。
 *
 * @param sessionId - 當前 session id
 * @param queryText - 當前 query（通常為最後一則用戶訊息或摘要）
 * @param params - getSessionMemories、embeddingProvider、options
 * @returns 檢索後的 key memories，可直接賦值給 session.memoryEmbeddings
 */
export async function getKeyMemoriesForRequest(
  sessionId: string,
  queryText: string,
  params: GetKeyMemoriesForRequestParams,
): Promise<MemoryEmbedding[]> {
  const { getSessionMemories, embeddingProvider, options } = params;
  const memories = await getSessionMemories(sessionId);
  return retrieveKeyMemoriesForQuery(queryText, memories, embeddingProvider, options);
}
