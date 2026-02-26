/**
 * MemoryRetrieval.ts
 *
 * 純 TypeScript 實作：動態記憶檢索邏輯（餘弦相似度 + 過濾 + category 多樣性）。
 * 對應 Rust：src-tauri/src/chat_manager/dynamic_memory.rs 與 types.rs。
 * 無 Tauri、無外部 NPM，供 React Native / iOS 等環境使用。
 */

// ---------------------------------------------------------------------------
// 資料結構（對應 types.rs MemoryEmbedding，camelCase）
// ---------------------------------------------------------------------------

export interface MemoryEmbedding {
  id: string;
  text: string;
  embedding: number[];
  createdAt?: number;
  tokenCount?: number;
  /** 若為 true 表示在 cold storage，預設不注入；但 isPinned 時仍可檢索 */
  isCold?: boolean;
  lastAccessedAt?: number;
  importanceScore?: number;
  isPinned?: boolean;
  accessCount?: number;
  /** 檢索後可填寫的暫時相似度分數 */
  matchScore?: number | null;
  /** 分類標籤，用於 category 多樣性（如 character_trait, relationship, plot_event） */
  category?: string | null;
}

// ---------------------------------------------------------------------------
// 數學核心：餘弦相似度（對應 dynamic_memory.rs cosine_similarity）
// ---------------------------------------------------------------------------

/**
 * 餘弦相似度：dot(a,b) / (||a|| * ||b||)。維度不同或空陣列回傳 0。
 */
export function cosineSimilarity(a: number[], b: number[]): number {
  if (a.length !== b.length || a.length === 0) {
    return 0;
  }
  let dot = 0;
  let sumA = 0;
  let sumB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    sumA += a[i] * a[i];
    sumB += b[i] * b[i];
  }
  const normA = Math.sqrt(sumA);
  const normB = Math.sqrt(sumB);
  const denom = normA * normB;
  if (denom === 0) {
    return 0;
  }
  return dot / denom;
}

// ---------------------------------------------------------------------------
// 檢索輔助：取得 embedding、是否可納入檢索（hot 或 pinned）
// ---------------------------------------------------------------------------

function getEmbedding(m: MemoryEmbedding): number[] {
  return m.embedding ?? [];
}

function isCold(m: MemoryEmbedding): boolean {
  return m.isCold ?? false;
}

function isPinned(m: MemoryEmbedding): boolean {
  return m.isPinned ?? false;
}

function getCategory(m: MemoryEmbedding): string {
  return m.category?.trim() || "other";
}

/** 是否參與檢索：有 embedding 且（非 cold 或 已 pin） */
function isRetrievable(m: MemoryEmbedding): boolean {
  const emb = getEmbedding(m);
  return emb.length > 0 && (!isCold(m) || isPinned(m));
}

// ---------------------------------------------------------------------------
// 純餘弦排序：對應 select_top_cosine_memory_indices（無 category 多樣性）
// ---------------------------------------------------------------------------

/**
 * 依餘弦相似度由高到低取前 limit 個，且 score >= minSimilarity。
 * 不回傳 matchScore，若要帶分數請用 retrieveRelevantMemories 或自行對結果再算一次。
 */
export function selectTopCosineMemoryIndices(
  queryEmbedding: number[],
  memories: MemoryEmbedding[],
  limit: number,
  minSimilarity: number,
): MemoryEmbedding[] {
  const scored: { score: number; index: number }[] = [];

  for (let i = 0; i < memories.length; i++) {
    const m = memories[i];
    if (!isRetrievable(m)) continue;
    const score = cosineSimilarity(queryEmbedding, getEmbedding(m));
    if (score >= minSimilarity) {
      scored.push({ score, index: i });
    }
  }

  scored.sort((a, b) => b.score - a.score);

  const taken = scored.slice(0, limit);
  return taken.map(({ score, index }) => {
    const mem = { ...memories[index] };
    mem.matchScore = score;
    return mem;
  });
}

// ---------------------------------------------------------------------------
// 帶 category 多樣性的檢索：對應 select_relevant_memory_indices
// ---------------------------------------------------------------------------

/**
 * 先依餘弦相似度排序並過濾 minSimilarity，再套用 category 多樣性：
 * 每個 category 最多 2 個名額，剩餘名額用其餘高分項補滿。
 * 回傳的每個項目會帶 matchScore。
 */
export function selectRelevantMemoryIndices(
  queryEmbedding: number[],
  memories: MemoryEmbedding[],
  limit: number,
  minSimilarity: number,
): { index: number; score: number }[] {
  const scored: { score: number; index: number }[] = [];

  for (let i = 0; i < memories.length; i++) {
    const m = memories[i];
    if (!isRetrievable(m)) continue;
    const score = cosineSimilarity(queryEmbedding, getEmbedding(m));
    if (score >= minSimilarity) {
      scored.push({ score, index: i });
    }
  }

  scored.sort((a, b) => b.score - a.score);

  const categoryCounts = new Map<string, number>();
  const result: { index: number; score: number }[] = [];
  const remaining: { score: number; index: number }[] = [];

  for (const { score, index } of scored) {
    const cat = getCategory(memories[index]);
    const count = categoryCounts.get(cat) ?? 0;
    if (count < 2 && result.length < limit) {
      categoryCounts.set(cat, count + 1);
      result.push({ index, score });
    } else {
      remaining.push({ score, index });
    }
  }

  for (const { score, index } of remaining) {
    if (result.length >= limit) break;
    result.push({ index, score });
  }

  return result;
}

// ---------------------------------------------------------------------------
// 對外主 API：retrieveRelevantMemories
// ---------------------------------------------------------------------------

/**
 * 依 query 的 embedding 從 memories 中檢索最相關的項目，數量上限 limit，相似度門檻 minSimilarity。
 * 邏輯照搬 Rust：餘弦相似度 → 過濾 → 排序 → category 多樣性（每類最多 2 個，其餘名額用高分補滿）。
 * 回傳的每個 MemoryEmbedding 會帶上 matchScore（相似度分數）。
 */
export function retrieveRelevantMemories(
  queryEmbedding: number[],
  memories: MemoryEmbedding[],
  limit: number,
  minSimilarity: number,
): MemoryEmbedding[] {
  const indicesWithScores = selectRelevantMemoryIndices(
    queryEmbedding,
    memories,
    limit,
    minSimilarity,
  );

  return indicesWithScores.map(({ index, score }) => {
    const mem = { ...memories[index] };
    mem.matchScore = score;
    return mem;
  });
}
