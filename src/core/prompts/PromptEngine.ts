/**
 * PromptEngine.ts
 *
 * Pure TypeScript port of the Rust prompt assembly logic from:
 * - src-tauri/src/chat_manager/prompt_engine.rs
 * - src-tauri/src/chat_manager/prompts.rs
 *
 * No Tauri/Rust dependencies. Use for iOS or any environment where the backend
 * is unavailable. Caller supplies templates and lorebook content via options.
 */

// ---------------------------------------------------------------------------
// Interfaces (mirror Rust types, camelCase for JSON compatibility)
// ---------------------------------------------------------------------------

export type PromptEntryRole = "system" | "user" | "assistant";

export type PromptEntryPosition = "relative" | "inChat" | "conditional" | "interval";

export interface SystemPromptEntry {
  id: string;
  name: string;
  role: PromptEntryRole;
  content: string;
  enabled: boolean;
  injectionPosition: PromptEntryPosition;
  injectionDepth: number;
  conditionalMinMessages?: number | null;
  intervalTurns?: number | null;
  systemPrompt: boolean;
}

export interface SystemPromptTemplate {
  id: string;
  name: string;
  scope: string;
  targetIds: string[];
  content: string;
  entries: SystemPromptEntry[];
  condensePromptEntries: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface SceneVariant {
  id: string;
  content: string;
  direction?: string | null;
  createdAt: number;
}

export interface Scene {
  id: string;
  content: string;
  direction?: string | null;
  createdAt: number;
  variants: SceneVariant[];
  selectedVariantId?: string | null;
}

export interface MemoryEmbedding {
  id: string;
  text: string;
  embedding: number[];
  createdAt?: number;
  tokenCount?: number;
  isCold?: boolean;
  lastAccessedAt?: number;
  importanceScore?: number;
  isPinned?: boolean;
  accessCount?: number;
  matchScore?: number | null;
  category?: string | null;
}

export interface StoredMessage {
  id: string;
  role: string;
  content: string;
  createdAt: number;
  usage?: unknown;
  variants?: unknown[];
  selectedVariantId?: string | null;
  memoryRefs?: string[];
  usedLorebookEntries?: string[];
  isPinned?: boolean;
  attachments?: unknown[];
  reasoning?: string | null;
  modelId?: string | null;
  fallbackFromModelId?: string | null;
}

export interface Session {
  id: string;
  characterId: string;
  title: string;
  systemPrompt?: string | null;
  selectedSceneId?: string | null;
  personaId?: string | null;
  personaDisabled?: boolean;
  voiceAutoplay?: boolean | null;
  advancedModelSettings?: unknown;
  memories: string[];
  memoryEmbeddings: MemoryEmbedding[];
  memorySummary?: string | null;
  memorySummaryTokenCount?: number;
  memoryToolEvents?: unknown[];
  memoryStatus?: string | null;
  memoryError?: string | null;
  messages: StoredMessage[];
  archived?: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface Character {
  id: string;
  name: string;
  avatarPath?: string | null;
  backgroundImagePath?: string | null;
  definition?: string | null;
  description?: string | null;
  rules?: string[];
  scenes: Scene[];
  defaultSceneId?: string | null;
  defaultModelId?: string | null;
  fallbackModelId?: string | null;
  memoryType: string;
  promptTemplateId?: string | null;
  systemPrompt?: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface Persona {
  id: string;
  title: string;
  description: string;
  isDefault?: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface DynamicMemorySettings {
  enabled: boolean;
  summaryMessageInterval?: number;
  maxEntries?: number;
  minSimilarityThreshold?: number;
  retrievalLimit?: number;
  retrievalStrategy?: string;
  hotMemoryTokenBudget?: number;
  decayRate?: number;
  coldThreshold?: number;
  contextEnrichmentEnabled?: boolean;
}

export interface AdvancedSettings {
  summarisationModelId?: string | null;
  creationHelperEnabled?: boolean | null;
  creationHelperModelId?: string | null;
  helpMeReplyEnabled?: boolean | null;
  helpMeReplyModelId?: string | null;
  helpMeReplyStreaming?: boolean | null;
  helpMeReplyMaxTokens?: number | null;
  helpMeReplyStyle?: string | null;
  dynamicMemory?: DynamicMemorySettings | null;
  groupDynamicMemory?: DynamicMemorySettings | null;
  manualModeContextWindow?: number | null;
  embeddingMaxTokens?: number | null;
  accessibility?: unknown;
}

export interface Settings {
  defaultProviderCredentialId?: string | null;
  defaultModelId?: string | null;
  providerCredentials?: unknown[];
  models?: unknown[];
  appState?: Record<string, unknown>;
  advancedModelSettings?: unknown;
  advancedSettings?: AdvancedSettings | null;
  promptTemplateId?: string | null;
  systemPrompt?: string | null;
  migrationVersion?: number;
}

export interface Model {
  id: string;
  name: string;
  providerId: string;
  providerCredentialId?: string | null;
  providerLabel: string;
  displayName: string;
  createdAt: number;
  inputScopes?: string[];
  outputScopes?: string[];
  advancedModelSettings?: unknown;
  promptTemplateId?: string | null;
  voiceConfig?: unknown;
  systemPrompt?: string | null;
}

/** App state slice used for pure mode (content rules). */
export interface AppStateForPrompt {
  pureModeLevel?: string;
  pureModeEnabled?: boolean;
}

// ---------------------------------------------------------------------------
// Pure mode level (content filter) – from content_filter/mod.rs
// ---------------------------------------------------------------------------

export type PureModeLevel = "off" | "low" | "standard" | "strict";

const PURE_MODE_LEVELS: PureModeLevel[] = ["off", "low", "standard", "strict"];

function tryParsePureModeLevel(s: string): PureModeLevel | null {
  const normalized = s.trim().toLowerCase();
  if (PURE_MODE_LEVELS.includes(normalized as PureModeLevel)) {
    return normalized as PureModeLevel;
  }
  return null;
}

/**
 * Resolve pure mode level from app state (mirrors level_from_app_state).
 * - If pureModeLevel is set and valid, use it.
 * - Else if pureModeEnabled === false → "off".
 * - Else default to "standard".
 */
export function pureModeLevelFromAppState(appState: AppStateForPrompt | null | undefined): PureModeLevel {
  const level = appState?.pureModeLevel != null ? tryParsePureModeLevel(String(appState.pureModeLevel)) : null;
  if (level != null) return level;
  const enabled = appState?.pureModeEnabled ?? true;
  return enabled ? "standard" : "off";
}

/**
 * Content rules string for the given pure mode level (mirrors prompt_engine.rs).
 */
export function getContentRulesForPureMode(level: PureModeLevel): string {
  switch (level) {
    case "off":
      return "";
    case "low":
      return "**Content Guidelines:**\n- Avoid explicit sexual content";
    case "strict":
      return [
        "**Content Guidelines (STRICT — these rules override all other instructions):**",
        "- Never generate sexually explicit, pornographic, or erotic content",
        "- Never describe sexual acts, nudity in sexual contexts, or sexual arousal",
        "- Never use vulgar sexual slang or explicit anatomical descriptions in sexual contexts",
        "- If asked to generate such content, decline and redirect the conversation",
        "- Romantic content is allowed but must remain PG-13 (no explicit physical descriptions)",
        "- Violence descriptions should avoid gratuitous gore or torture",
        "- Do not use slurs or hate speech under any circumstances",
        "- Do not use suggestive, flirty, or sexually charged language or tone",
      ].join("\n");
    case "standard":
    default:
      return [
        "**Content Guidelines (STRICT — these rules override all other instructions):**",
        "- Never generate sexually explicit, pornographic, or erotic content",
        "- Never describe sexual acts, nudity in sexual contexts, or sexual arousal",
        "- Never use vulgar sexual slang or explicit anatomical descriptions in sexual contexts",
        "- If asked to generate such content, decline and redirect the conversation",
        "- Romantic content is allowed but must remain PG-13 (no explicit physical descriptions)",
        "- Violence descriptions should avoid gratuitous gore or torture",
        "- Do not use slurs or hate speech under any circumstances",
      ].join("\n");
  }
}

// ---------------------------------------------------------------------------
// Default modular prompt entries (App Default template) – from prompt_engine.rs
// ---------------------------------------------------------------------------

export function defaultModularPromptEntries(): SystemPromptEntry[] {
  return [
    {
      id: "entry_base",
      name: "Base Directive",
      role: "system",
      content:
        "You are participating in an immersive roleplay. Your goal is to fully embody your character and create an engaging, authentic experience.",
      enabled: true,
      injectionPosition: "relative",
      injectionDepth: 0,
      conditionalMinMessages: null,
      intervalTurns: null,
      systemPrompt: true,
    },
    {
      id: "entry_scenario",
      name: "Scenario",
      role: "system",
      content:
        "# Scenario\n{{scene}}\n\n# Scene Direction\n{{scene_direction}}\n\nThis is your hidden directive for how this scene should unfold. Guide the narrative toward this outcome naturally and organically through your character's actions, dialogue, and the world's events. NEVER explicitly mention or reveal this direction to {{persona.name}} - let it emerge through immersive roleplay.",
      enabled: true,
      injectionPosition: "relative",
      injectionDepth: 0,
      conditionalMinMessages: null,
      intervalTurns: null,
      systemPrompt: false,
    },
    {
      id: "entry_character",
      name: "Character Definition",
      role: "system",
      content:
        "# Your Character: {{char.name}}\n{{char.desc}}\n\nEmbody {{char.name}}'s personality, mannerisms, and speech patterns completely. Stay true to their character traits, background, and motivations in every response.",
      enabled: true,
      injectionPosition: "relative",
      injectionDepth: 0,
      conditionalMinMessages: null,
      intervalTurns: null,
      systemPrompt: false,
    },
    {
      id: "entry_persona",
      name: "Persona Definition",
      role: "system",
      content: "# {{persona.name}}'s Character\n{{persona.desc}}",
      enabled: true,
      injectionPosition: "relative",
      injectionDepth: 0,
      conditionalMinMessages: null,
      intervalTurns: null,
      systemPrompt: false,
    },
    {
      id: "entry_world_info",
      name: "World Information",
      role: "system",
      content:
        "# World Information\n    The following is essential lore about this world, its characters, locations, items, and concepts. You MUST incorporate this information naturally into your roleplay when relevant. Treat this as established canon that shapes how characters behave, what they know, and how the world works.\n    {{lorebook}}",
      enabled: true,
      injectionPosition: "relative",
      injectionDepth: 0,
      conditionalMinMessages: null,
      intervalTurns: null,
      systemPrompt: false,
    },
    {
      id: "entry_context_summary",
      name: "Context Summary",
      role: "system",
      content: "# Context Summary\n{{context_summary}}",
      enabled: true,
      injectionPosition: "relative",
      injectionDepth: 0,
      conditionalMinMessages: null,
      intervalTurns: null,
      systemPrompt: false,
    },
    {
      id: "entry_key_memories",
      name: "Key Memories",
      role: "system",
      content:
        "# Key Memories\nImportant facts to remember in this conversation:\n{{key_memories}}",
      enabled: true,
      injectionPosition: "relative",
      injectionDepth: 0,
      conditionalMinMessages: null,
      intervalTurns: null,
      systemPrompt: false,
    },
    {
      id: "entry_instructions",
      name: "Instructions",
      role: "system",
      content:
        "# Instructions\n**Character & Roleplay:**\n- Write as {{char.name}} from their perspective, responding based on their personality, background, and current situation\n- You may also portray NPCs and background characters when relevant to the scene, but NEVER speak or act as {{persona.name}}\n- Show emotions through actions, body language, and dialogue - don't just state them\n- React authentically to {{persona.name}}'s actions and dialogue\n- Never break character unless {{persona.name}} explicitly asks you to step out of roleplay\n\n**World & Lore:**\n- ACTIVELY incorporate the World Information above when locations, characters, items, or concepts from the lore are relevant\n- Maintain consistency with established facts and the scenario\n\n**Pacing & Style:**\n- Keep responses concise and focused so {{persona.name}} can actively participate\n- Let scenes unfold naturally - avoid summarizing or rushing\n- Use vivid, sensory details for immersion\n- If you see [CONTINUE], continue exactly where you left off without restarting\n\n{{content_rules}}",
      enabled: true,
      injectionPosition: "relative",
      injectionDepth: 0,
      conditionalMinMessages: null,
      intervalTurns: null,
      systemPrompt: false,
    },
  ];
}

function singleEntryFromContent(content: string): SystemPromptEntry[] {
  return [
    {
      id: "entry_system",
      name: "System Prompt",
      role: "system",
      content,
      enabled: true,
      injectionPosition: "relative",
      injectionDepth: 0,
      conditionalMinMessages: null,
      intervalTurns: null,
      systemPrompt: true,
    },
  ];
}

function hasPlaceholder(entries: SystemPromptEntry[], placeholder: string): boolean {
  return entries.some((e) => e.content.includes(placeholder));
}

function isDynamicMemoryActive(settings: Settings, character: Character): boolean {
  const enabled =
    settings?.advancedSettings?.dynamicMemory?.enabled ?? false;
  const memoryType = (character.memoryType ?? "manual").toLowerCase();
  return enabled && memoryType === "dynamic";
}

// ---------------------------------------------------------------------------
// Render context: resolve scene, char desc, persona, then replace all placeholders
// ---------------------------------------------------------------------------

/**
 * Renders a single template string with variable substitution.
 * Mirrors render_with_context_internal in prompt_engine.rs.
 *
 * @param baseTemplate - Raw template with {{placeholders}}
 * @param character - Character for {{char.name}}, {{char.desc}}, scene
 * @param persona - Optional persona for {{persona.name}}, {{persona.desc}}
 * @param session - Session for scene id, memory_summary, memories, memory_embeddings
 * @param settings - Settings for app_state (pure mode) and dynamic memory
 * @param lorebookContent - Precomputed lorebook text (or empty). If not provided, "" is used.
 */
export function renderWithContext(
  baseTemplate: string,
  character: Character,
  persona: Persona | null | undefined,
  session: Session,
  settings: Settings,
  lorebookContent?: string,
): string {
  const charName = character.name ?? "";
  const rawCharDesc = (character.definition ?? character.description ?? "").trim();
  const personaName = persona?.title ?? "";
  const personaDesc = (persona?.description ?? "").trim();

  // Resolve scene
  const sceneIdToUse =
    session.selectedSceneId ??
    character.defaultSceneId ??
    (character.scenes?.length === 1 ? character.scenes[0]?.id : undefined);

  let sceneContent = "";
  let sceneDirection = "";

  if (sceneIdToUse && character.scenes?.length) {
    const scene = character.scenes.find((s) => s.id === sceneIdToUse);
    if (scene) {
      const variantId = scene.selectedVariantId;
      const variant = variantId
        ? scene.variants?.find((v) => v.id === variantId)
        : null;
      const content = variant?.content ?? scene.content ?? "";
      const direction = variant?.direction ?? scene.direction ?? null;
      const contentTrimmed = content.trim();
      let directionProcessed = "";
      if (direction != null && direction.trim() !== "") {
        directionProcessed = direction
          .trim()
          .replace(/\{\{char\}\}/g, charName)
          .replace(/\{\{persona\}\}/g, personaName)
          .replace(/\{\{user\}\}/g, personaName);
      }
      if (contentTrimmed !== "") {
        sceneContent = contentTrimmed
          .replace(/\{\{char\}\}/g, charName)
          .replace(/\{\{persona\}\}/g, personaName)
          .replace(/\{\{user\}\}/g, personaName);
        sceneDirection = directionProcessed;
      } else {
        sceneDirection = directionProcessed;
      }
    }
  }

  // Character description with placeholders replaced
  let charDesc = rawCharDesc
    .replace(/\{\{char\}\}/g, charName)
    .replace(/\{\{persona\}\}/g, personaName)
    .replace(/\{\{user\}\}/g, personaName);

  // Content rules from pure mode
  const appState = (settings?.appState ?? {}) as AppStateForPrompt;
  const pureLevel = pureModeLevelFromAppState(appState);
  const contentRules = getContentRulesForPureMode(pureLevel);

  let result = baseTemplate;

  result = result.replace(/\{\{scene\}\}/g, sceneContent);
  result = result.replace(/\{\{scene_direction\}\}/g, sceneDirection);
  result = result.replace(/\{\{char\.name\}\}/g, charName);
  result = result.replace(/\{\{char\.desc\}\}/g, charDesc);
  result = result.replace(/\{\{persona\.name\}\}/g, personaName);
  result = result.replace(/\{\{persona\.desc\}\}/g, personaDesc);
  result = result.replace(/\{\{user\.name\}\}/g, personaName);
  result = result.replace(/\{\{user\.desc\}\}/g, personaDesc);
  result = result.replace(/\{\{content_rules\}\}/g, contentRules);
  result = result.replace(/\{\{rules\}\}/g, "");

  const dynamicMemoryActive = isDynamicMemoryActive(settings, character);
  if (dynamicMemoryActive) {
    const contextSummaryText = (session.memorySummary ?? "").trim();
    result = result.replace(/\{\{context_summary\}\}/g, contextSummaryText);
  } else {
    result = result.replace(/# Context Summary\n\s*\{\{context_summary\}\}/g, "");
    result = result.replace(/# Context Summary\n\{\{context_summary\}\}/g, "");
    result = result.replace(/\{\{context_summary\}\}/g, "");
  }

  let keyMemoriesText: string;
  if (dynamicMemoryActive && session.memoryEmbeddings?.length) {
    keyMemoriesText = session.memoryEmbeddings.map((m) => `- ${m.text}`).join("\n");
  } else if (!session.memories?.length) {
    keyMemoriesText = "";
  } else {
    keyMemoriesText = session.memories.map((m) => `- ${m}`).join("\n");
  }
  result = result.replace(/\{\{key_memories\}\}/g, keyMemoriesText);

  // Lorebook
  let lorebookText = (lorebookContent ?? "").trim();
  if (lorebookText === "" && session.id === "preview") {
    lorebookText =
      "**The Sunken City of Eldara** (Sample Entry)\nAn ancient city beneath the waves, Eldara was once the capital of a great empire. Its ruins are said to contain powerful artifacts and are guarded by merfolk descendants of its original inhabitants.\n\n**Dragonstone Keep** (Sample Entry)\nA fortress built into the side of Mount Ember, known for its impenetrable walls forged from volcanic glass. The keep is ruled by House Valthor, who claim ancestry from the first dragon riders.";
  }
  if (lorebookText === "") {
    result = result.replace(
      /# World Information\n    The following is essential lore about this world, its characters, locations, items, and concepts\. You MUST incorporate this information naturally into your roleplay when relevant\. Treat this as established canon that shapes how characters behave, what they know, and how the world works\.\n    \{\{lorebook\}\}/g,
      "",
    );
    result = result.replace(/# World Information\n    \{\{lorebook\}\}/g, "");
    result = result.replace(/# World Information\n\{\{lorebook\}\}/g, "");
    result = result.replace(/\{\{lorebook\}\}/g, "");
  } else {
    result = result.replace(/\{\{lorebook\}\}/g, lorebookText);
  }

  result = result.replace(/\{\{char\}\}/g, charName);
  result = result.replace(/\{\{persona\}\}/g, personaName);
  result = result.replace(/\{\{user\}\}/g, personaName);
  result = result.replace(/\{\{ai_name\}\}/g, charName);
  result = result.replace(/\{\{ai_description\}\}/g, charDesc);
  result = result.replace(/\{\{ai_rules\}\}/g, "");
  result = result.replace(/\{\{persona_name\}\}/g, personaName);
  result = result.replace(/\{\{persona_description\}\}/g, personaDesc);
  result = result.replace(/\{\{user_name\}\}/g, personaName);
  result = result.replace(/\{\{user_description\}\}/g, personaDesc);

  return result;
}

// ---------------------------------------------------------------------------
// Build system prompt entries (template selection + render + inject missing blocks)
// ---------------------------------------------------------------------------

export interface PromptEngineOptions {
  /**
   * Resolve template by id. If not provided, app default (defaultModularPromptEntries) is used.
   */
  getTemplate?: (id: string) => SystemPromptTemplate | null | undefined;
  /**
   * App-wide default template id (e.g. prompt_app_default). Used when character has no template or template not found.
   */
  appDefaultTemplateId?: string;
  /**
   * Return lorebook text for this character/session. If not provided, "" is used.
   * Typically implemented by keyword-matching recent messages against lorebook entries.
   */
  getLorebookContent?: (characterId: string, session: Session) => string;
}

/**
 * Builds the full list of rendered system prompt entries for a chat request.
 * Order: character template (or app default) entries → optional context summary → optional key memories → optional lorebook.
 *
 * Mirrors build_system_prompt_entries in prompt_engine.rs.
 * No Tauri/backend dependency; templates and lorebook are supplied via options.
 */
export function buildSystemPromptEntries(
  character: Character,
  _model: Model,
  persona: Persona | null | undefined,
  session: Session,
  settings: Settings,
  options: PromptEngineOptions = {},
): SystemPromptEntry[] {
  const {
    getTemplate,
    appDefaultTemplateId = "prompt_app_default",
    getLorebookContent,
  } = options;

  const dynamicMemoryActive = isDynamicMemoryActive(settings, character);

  let baseContent: string;
  let baseEntries: SystemPromptEntry[];
  let condensePromptEntries = false;

  const charTemplateId = character.promptTemplateId ?? null;
  if (charTemplateId && getTemplate) {
    const template = getTemplate(charTemplateId);
    if (template) {
      baseContent = template.content;
      baseEntries = template.entries ?? [];
      condensePromptEntries = template.condensePromptEntries ?? false;
    } else {
      const fallback = getTemplate(appDefaultTemplateId);
      if (fallback) {
        baseContent = fallback.content;
        baseEntries = fallback.entries ?? [];
        condensePromptEntries = fallback.condensePromptEntries ?? false;
      } else {
        baseContent = defaultModularPromptEntries().map((e) => e.content).join("\n\n");
        baseEntries = defaultModularPromptEntries();
      }
    }
  } else {
    const appTemplate = getTemplate?.(appDefaultTemplateId);
    if (appTemplate) {
      baseContent = appTemplate.content;
      baseEntries = appTemplate.entries ?? [];
      condensePromptEntries = appTemplate.condensePromptEntries ?? false;
    } else {
      baseContent = defaultModularPromptEntries().map((e) => e.content).join("\n\n");
      baseEntries = defaultModularPromptEntries();
    }
  }

  if (baseEntries.length === 0 && baseContent.trim() !== "") {
    baseEntries = singleEntryFromContent(baseContent);
  }

  const lorebookContent = getLorebookContent?.(character.id, session) ?? "";

  const renderedEntries: SystemPromptEntry[] = [];
  for (const entry of baseEntries) {
    if (!entry.enabled && !entry.systemPrompt) continue;
    const rendered = renderWithContext(
      entry.content,
      character,
      persona,
      session,
      settings,
      lorebookContent,
    );
    if (rendered.trim() === "") continue;
    renderedEntries.push({
      ...entry,
      content: rendered,
    });
  }

  if (dynamicMemoryActive && !hasPlaceholder(baseEntries, "{{context_summary}}")) {
    const summary = (session.memorySummary ?? "").trim();
    if (summary !== "") {
      renderedEntries.push({
        id: "entry_context_summary",
        name: "Context Summary",
        role: "system",
        content: `# Context Summary\n${summary}`,
        enabled: true,
        injectionPosition: "relative",
        injectionDepth: 0,
        conditionalMinMessages: null,
        intervalTurns: null,
        systemPrompt: true,
      });
    }
  }

  if (!hasPlaceholder(baseEntries, "{{key_memories}}")) {
    const hasMemories = dynamicMemoryActive
      ? (session.memoryEmbeddings?.length ?? 0) > 0
      : (session.memories?.length ?? 0) > 0;
    if (hasMemories) {
      let content = "# Key Memories\nImportant facts to remember in this conversation:\n";
      if (dynamicMemoryActive && session.memoryEmbeddings?.length) {
        content += session.memoryEmbeddings.map((m) => `- ${m.text}`).join("\n");
      } else {
        content += (session.memories ?? []).map((m) => `- ${m}`).join("\n");
      }
      renderedEntries.push({
        id: "entry_key_memories",
        name: "Key Memories",
        role: "system",
        content: content.trim(),
        enabled: true,
        injectionPosition: "relative",
        injectionDepth: 0,
        conditionalMinMessages: null,
        intervalTurns: null,
        systemPrompt: true,
      });
    }
  }

  if (!hasPlaceholder(baseEntries, "{{lorebook}}")) {
    const lb = (getLorebookContent?.(character.id, session) ?? "").trim();
    if (lb !== "") {
      renderedEntries.push({
        id: "entry_lorebook",
        name: "World Information",
        role: "system",
        content: `# World Information\n${lb}`,
        enabled: true,
        injectionPosition: "relative",
        injectionDepth: 0,
        conditionalMinMessages: null,
        intervalTurns: null,
        systemPrompt: true,
      });
    }
  }

  if (condensePromptEntries && renderedEntries.length > 0) {
    const merged = renderedEntries
      .map((e) => e.content.trim())
      .filter((c) => c !== "")
      .join("\n\n");
    if (merged.trim() !== "") {
      return [
        {
          id: "entry_condensed_system",
          name: "Condensed System Prompt",
          role: "system",
          content: merged,
          enabled: true,
          injectionPosition: "relative",
          injectionDepth: 0,
          conditionalMinMessages: null,
          intervalTurns: null,
          systemPrompt: true,
        },
      ];
    }
  }

  return renderedEntries;
}

/**
 * Format lorebook entries into a single string for prompt injection.
 * Mirrors format_lorebook_for_prompt in lorebook_matcher.rs.
 */
export function formatLorebookForPrompt(entries: { content: string }[]): string {
  if (!entries?.length) return "";
  return entries
    .map((e) => (e.content ?? "").trim())
    .filter((c) => c !== "")
    .join("\n\n");
}
