import {
  renderWithContext,
  type Character,
  type Session,
  type Settings,
} from "./PromptEngine";

// 1. 模擬你的角色與世界觀（符合 PromptEngine 的 Character / Session 介面）
const fakeCharacter: Character = {
  id: "c1",
  name: "D&D 法師",
  description: "脾氣暴躁，認為所有科技都是黑魔法。",
  scenes: [],
  memoryType: "dynamic", // 設為 dynamic 才會替換 {{context_summary}}
  createdAt: 0,
  updatedAt: 0,
};
const fakeSession: Session = {
  id: "s1",
  characterId: "c1",
  title: "Test",
  selectedSceneId: "scene_01",
  memorySummary: "現代工程師剛剛拿出了一台發光的扁平石板 (iPad Pro)。",
  memories: [],
  memoryEmbeddings: [],
  messages: [],
  createdAt: 0,
  updatedAt: 0,
};

// 啟用動態記憶，{{context_summary}} 才會被替換成 session.memorySummary
const fakeSettings: Settings = {
  advancedSettings: {
    dynamicMemory: { enabled: true },
  },
};

// 2. 模擬原系統的模板
const fakeTemplate = `
場景：{{scene}}
角色：{{char.name}} - {{char.desc}}
前情提要：{{context_summary}}
`;

// 3. 執行替換引擎
const result = renderWithContext(
  fakeTemplate,
  fakeCharacter,
  null,
  fakeSession,
  fakeSettings,
);

console.log("=== 生成的 System Prompt ===");
console.log(result);
console.log("");

// 4. 簡單驗證：替換後的文字應包含角色名與前情提要，且不應殘留 {{}}
const hasCharName = result.includes("D&D 法師");
const hasSummary = result.includes("扁平石板");
const noPlaceholders = !result.includes("{{");
if (hasCharName && hasSummary && noPlaceholders) {
  console.log("✅ PromptEngine 大腦跑起來了！替換邏輯正常。");
} else {
  console.log("❌ 請檢查：", { hasCharName, hasSummary, noPlaceholders });
  throw new Error("PromptEngine 驗證未通過");
}