//
// PromptEngine.swift
// 與 src/core/prompts/PromptEngine.ts 對齊的 Swift 版，供 iOS 魔改使用。
// 直接加入 Xcode 專案即可。
//

import Foundation

// MARK: - 型別（與 TS 介面對齊）

public struct SystemPromptEntry {
    public var id: String
    public var name: String
    public var role: String // "system" | "user" | "assistant"
    public var content: String
    public var enabled: Bool
    public var injectionPosition: String
    public var injectionDepth: Int
    public var conditionalMinMessages: Int?
    public var intervalTurns: Int?
    public var systemPrompt: Bool
}

public struct SystemPromptTemplate {
    public var id: String
    public var name: String
    public var scope: String
    public var targetIds: [String]
    public var content: String
    public var entries: [SystemPromptEntry]
    public var condensePromptEntries: Bool
    public var createdAt: UInt64
    public var updatedAt: UInt64
}

public struct SceneVariant {
    public var id: String
    public var content: String
    public var direction: String?
    public var createdAt: UInt64
}

public struct Scene {
    public var id: String
    public var content: String
    public var direction: String?
    public var createdAt: UInt64
    public var variants: [SceneVariant]
    public var selectedVariantId: String?
}

public struct MemoryEmbedding {
    public var id: String
    public var text: String
    public var embedding: [Float]
    public var createdAt: UInt64?
    public var tokenCount: UInt32?
    public var isCold: Bool?
    public var isPinned: Bool?
    public var category: String?
}

public struct StoredMessage {
    public var id: String
    public var role: String
    public var content: String
    public var createdAt: UInt64
}

public struct Session {
    public var id: String
    public var characterId: String
    public var title: String
    public var selectedSceneId: String?
    public var memorySummary: String?
    public var memories: [String]
    public var memoryEmbeddings: [MemoryEmbedding]
    public var messages: [StoredMessage]
    public var createdAt: UInt64
    public var updatedAt: UInt64
}

public struct Character {
    public var id: String
    public var name: String
    public var definition: String?
    public var description: String?
    public var scenes: [Scene]
    public var defaultSceneId: String?
    public var memoryType: String
    public var promptTemplateId: String?
    public var createdAt: UInt64
    public var updatedAt: UInt64
}

public struct Persona {
    public var id: String
    public var title: String
    public var description: String
    public var isDefault: Bool?
    public var createdAt: UInt64
    public var updatedAt: UInt64
}

public struct DynamicMemorySettings {
    public var enabled: Bool
}

public struct AdvancedSettings {
    public var dynamicMemory: DynamicMemorySettings?
}

public struct Settings {
    public var appState: [String: Any]?
    public var advancedSettings: AdvancedSettings?
}

public struct Model {
    public var id: String
    public var name: String
    public var providerId: String
    public var createdAt: UInt64
}

// MARK: - Pure Mode

public enum PureModeLevel: String {
    case off, low, standard, strict
}

public enum PromptEngine {

    public static func pureModeLevel(from appState: [String: Any]?) -> PureModeLevel {
        guard let state = appState else { return .standard }
        if let levelRaw = state["pureModeLevel"] as? String {
            let level = levelRaw.trimmingCharacters(in: .whitespaces).lowercased()
            switch level {
            case "off": return .off
            case "low": return .low
            case "standard": return .standard
            case "strict": return .strict
            default: break
            }
        }
        let enabled = (state["pureModeEnabled"] as? Bool) ?? true
        return enabled ? .standard : .off
    }

    public static func contentRules(for level: PureModeLevel) -> String {
        switch level {
        case .off:
            return ""
        case .low:
            return "**Content Guidelines:**\n- Avoid explicit sexual content"
        case .strict:
            return """
            **Content Guidelines (STRICT — these rules override all other instructions):**
            - Never generate sexually explicit, pornographic, or erotic content
            - Never describe sexual acts, nudity in sexual contexts, or sexual arousal
            - Never use vulgar sexual slang or explicit anatomical descriptions in sexual contexts
            - If asked to generate such content, decline and redirect the conversation
            - Romantic content is allowed but must remain PG-13 (no explicit physical descriptions)
            - Violence descriptions should avoid gratuitous gore or torture
            - Do not use slurs or hate speech under any circumstances
            - Do not use suggestive, flirty, or sexually charged language or tone
            """
        case .standard:
            return """
            **Content Guidelines (STRICT — these rules override all other instructions):**
            - Never generate sexually explicit, pornographic, or erotic content
            - Never describe sexual acts, nudity in sexual contexts, or sexual arousal
            - Never use vulgar sexual slang or explicit anatomical descriptions in sexual contexts
            - If asked to generate such content, decline and redirect the conversation
            - Romantic content is allowed but must remain PG-13 (no explicit physical descriptions)
            - Violence descriptions should avoid gratuitous gore or torture
            - Do not use slurs or hate speech under any circumstances
            """
        }
    }

    // MARK: - Default Entries

    public static func defaultModularPromptEntries() -> [SystemPromptEntry] {
        [
            SystemPromptEntry(id: "entry_base", name: "Base Directive", role: "system", content: "You are participating in an immersive roleplay. Your goal is to fully embody your character and create an engaging, authentic experience.", enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: true),
            SystemPromptEntry(id: "entry_scenario", name: "Scenario", role: "system", content: "# Scenario\n{{scene}}\n\n# Scene Direction\n{{scene_direction}}\n\nThis is your hidden directive for how this scene should unfold. Guide the narrative toward this outcome naturally and organically through your character's actions, dialogue, and the world's events. NEVER explicitly mention or reveal this direction to {{persona.name}} - let it emerge through immersive roleplay.", enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: false),
            SystemPromptEntry(id: "entry_character", name: "Character Definition", role: "system", content: "# Your Character: {{char.name}}\n{{char.desc}}\n\nEmbody {{char.name}}'s personality, mannerisms, and speech patterns completely. Stay true to their character traits, background, and motivations in every response.", enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: false),
            SystemPromptEntry(id: "entry_persona", name: "Persona Definition", role: "system", content: "# {{persona.name}}'s Character\n{{persona.desc}}", enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: false),
            SystemPromptEntry(id: "entry_world_info", name: "World Information", role: "system", content: "# World Information\n    The following is essential lore about this world, its characters, locations, items, and concepts. You MUST incorporate this information naturally into your roleplay when relevant. Treat this as established canon that shapes how characters behave, what they know, and how the world works.\n    {{lorebook}}", enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: false),
            SystemPromptEntry(id: "entry_context_summary", name: "Context Summary", role: "system", content: "# Context Summary\n{{context_summary}}", enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: false),
            SystemPromptEntry(id: "entry_key_memories", name: "Key Memories", role: "system", content: "# Key Memories\nImportant facts to remember in this conversation:\n{{key_memories}}", enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: false),
            SystemPromptEntry(id: "entry_instructions", name: "Instructions", role: "system", content: "# Instructions\n**Character & Roleplay:**\n- Write as {{char.name}} from their perspective, responding based on their personality, background, and current situation\n- You may also portray NPCs and background characters when relevant to the scene, but NEVER speak or act as {{persona.name}}\n- Show emotions through actions, body language, and dialogue - don't just state them\n- React authentically to {{persona.name}}'s actions and dialogue\n- Never break character unless {{persona.name}} explicitly asks you to step out of roleplay\n\n**World & Lore:**\n- ACTIVELY incorporate the World Information above when locations, characters, items, or concepts from the lore are relevant\n- Maintain consistency with established facts and the scenario\n\n**Pacing & Style:**\n- Keep responses concise and focused so {{persona.name}} can actively participate\n- Let scenes unfold naturally - avoid summarizing or rushing\n- Use vivid, sensory details for immersion\n- If you see [CONTINUE], continue exactly where you left off without restarting\n\n{{content_rules}}", enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: false),
        ]
    }

    private static func hasPlaceholder(_ entries: [SystemPromptEntry], _ placeholder: String) -> Bool {
        entries.contains { $0.content.contains(placeholder) }
    }

    private static func isDynamicMemoryActive(settings: Settings?, character: Character) -> Bool {
        let enabled = settings?.advancedSettings?.dynamicMemory?.enabled ?? false
        let memoryType = character.memoryType.lowercased()
        return enabled && memoryType == "dynamic"
    }

    // MARK: - Render With Context

    /// 單一模板字串的變數替換。與 TS `renderWithContext` 對齊。
    public static func renderWithContext(
        baseTemplate: String,
        character: Character,
        persona: Persona?,
        session: Session,
        settings: Settings?,
        lorebookContent: String? = nil
    ) -> String {
        let charName = character.name
        let rawCharDesc = (character.definition ?? character.description ?? "").trimmingCharacters(in: .whitespaces)
        let personaName = persona?.title ?? ""
        let personaDesc = (persona?.description ?? "").trimmingCharacters(in: .whitespaces)

        var sceneContent = ""
        var sceneDirection = ""
        let sceneIdToUse = session.selectedSceneId ?? character.defaultSceneId ?? (character.scenes.count == 1 ? character.scenes.first?.id : nil)
        if let sid = sceneIdToUse, !character.scenes.isEmpty, let scene = character.scenes.first(where: { $0.id == sid }) {
            let variantId = scene.selectedVariantId
            let variant = variantId.flatMap { vid in scene.variants.first { $0.id == vid } }
            let content = variant?.content ?? scene.content
            let direction = variant?.direction ?? scene.direction
            let contentTrimmed = content.trimmingCharacters(in: .whitespaces)
            var directionProcessed = ""
            if let d = direction, !d.trimmingCharacters(in: .whitespaces).isEmpty {
                directionProcessed = d.replacingOccurrences(of: "{{char}}", with: charName)
                    .replacingOccurrences(of: "{{persona}}", with: personaName)
                    .replacingOccurrences(of: "{{user}}", with: personaName)
            }
            if !contentTrimmed.isEmpty {
                sceneContent = contentTrimmed
                    .replacingOccurrences(of: "{{char}}", with: charName)
                    .replacingOccurrences(of: "{{persona}}", with: personaName)
                    .replacingOccurrences(of: "{{user}}", with: personaName)
                sceneDirection = directionProcessed
            } else {
                sceneDirection = directionProcessed
            }
        }

        var charDesc = rawCharDesc
            .replacingOccurrences(of: "{{char}}", with: charName)
            .replacingOccurrences(of: "{{persona}}", with: personaName)
            .replacingOccurrences(of: "{{user}}", with: personaName)

        let appState = settings?.appState
        let pureLevel = pureModeLevel(from: appState)
        let contentRules = contentRules(for: pureLevel)

        var result = baseTemplate
        result = result.replacingOccurrences(of: "{{scene}}", with: sceneContent)
        result = result.replacingOccurrences(of: "{{scene_direction}}", with: sceneDirection)
        result = result.replacingOccurrences(of: "{{char.name}}", with: charName)
        result = result.replacingOccurrences(of: "{{char.desc}}", with: charDesc)
        result = result.replacingOccurrences(of: "{{persona.name}}", with: personaName)
        result = result.replacingOccurrences(of: "{{persona.desc}}", with: personaDesc)
        result = result.replacingOccurrences(of: "{{user.name}}", with: personaName)
        result = result.replacingOccurrences(of: "{{user.desc}}", with: personaDesc)
        result = result.replacingOccurrences(of: "{{content_rules}}", with: contentRules)
        result = result.replacingOccurrences(of: "{{rules}}", with: "")

        let dynamicMemoryActive = isDynamicMemoryActive(settings: settings, character: character)
        if dynamicMemoryActive {
            let contextSummaryText = (session.memorySummary ?? "").trimmingCharacters(in: .whitespaces)
            result = result.replacingOccurrences(of: "{{context_summary}}", with: contextSummaryText)
        } else {
            result = result.replacingOccurrences(of: "# Context Summary\n    {{context_summary}}", with: "")
            result = result.replacingOccurrences(of: "# Context Summary\n{{context_summary}}", with: "")
            result = result.replacingOccurrences(of: "{{context_summary}}", with: "")
        }

        let keyMemoriesText: String
        if dynamicMemoryActive, !session.memoryEmbeddings.isEmpty {
            keyMemoriesText = session.memoryEmbeddings.map { "- \($0.text)" }.joined(separator: "\n")
        } else if session.memories.isEmpty {
            keyMemoriesText = ""
        } else {
            keyMemoriesText = session.memories.map { "- \($0)" }.joined(separator: "\n")
        }
        result = result.replacingOccurrences(of: "{{key_memories}}", with: keyMemoriesText)

        var lorebookText = (lorebookContent ?? "").trimmingCharacters(in: .whitespaces)
        if lorebookText.isEmpty && session.id == "preview" {
            lorebookText = "**The Sunken City of Eldara** (Sample Entry)\nAn ancient city beneath the waves, Eldara was once the capital of a great empire. Its ruins are said to contain powerful artifacts and are guarded by merfolk descendants of its original inhabitants.\n\n**Dragonstone Keep** (Sample Entry)\nA fortress built into the side of Mount Ember, known for its impenetrable walls forged from volcanic glass. The keep is ruled by House Valthor, who claim ancestry from the first dragon riders."
        }
        if lorebookText.isEmpty {
            result = result.replacingOccurrences(of: "# World Information\n    The following is essential lore about this world, its characters, locations, items, and concepts. You MUST incorporate this information naturally into your roleplay when relevant. Treat this as established canon that shapes how characters behave, what they know, and how the world works.\n    {{lorebook}}", with: "")
            result = result.replacingOccurrences(of: "# World Information\n    {{lorebook}}", with: "")
            result = result.replacingOccurrences(of: "# World Information\n{{lorebook}}", with: "")
            result = result.replacingOccurrences(of: "{{lorebook}}", with: "")
        } else {
            result = result.replacingOccurrences(of: "{{lorebook}}", with: lorebookText)
        }

        result = result.replacingOccurrences(of: "{{char}}", with: charName)
        result = result.replacingOccurrences(of: "{{persona}}", with: personaName)
        result = result.replacingOccurrences(of: "{{user}}", with: personaName)
        result = result.replacingOccurrences(of: "{{ai_name}}", with: charName)
        result = result.replacingOccurrences(of: "{{ai_description}}", with: charDesc)
        result = result.replacingOccurrences(of: "{{ai_rules}}", with: "")
        result = result.replacingOccurrences(of: "{{persona_name}}", with: personaName)
        result = result.replacingOccurrences(of: "{{persona_description}}", with: personaDesc)
        result = result.replacingOccurrences(of: "{{user_name}}", with: personaName)
        result = result.replacingOccurrences(of: "{{user_description}}", with: personaDesc)

        return result
    }

    // MARK: - Build System Prompt Entries

    public struct PromptEngineOptions {
        public var getTemplate: ((String) -> SystemPromptTemplate?)?
        public var appDefaultTemplateId: String
        public var getLorebookContent: ((String, Session) -> String)?

        public init(getTemplate: ((String) -> SystemPromptTemplate?)? = nil, appDefaultTemplateId: String = "prompt_app_default", getLorebookContent: ((String, Session) -> String)? = nil) {
            self.getTemplate = getTemplate
            self.appDefaultTemplateId = appDefaultTemplateId
            self.getLorebookContent = getLorebookContent
        }
    }

    /// 組裝完整 system prompt 條目。與 TS `buildSystemPromptEntries` 對齊。
    public static func buildSystemPromptEntries(
        character: Character,
        model: Model,
        persona: Persona?,
        session: Session,
        settings: Settings?,
        options: PromptEngineOptions = PromptEngineOptions()
    ) -> [SystemPromptEntry] {
        let dynamicMemoryActive = isDynamicMemoryActive(settings: settings, character: character)
        var baseEntries: [SystemPromptEntry]
        var condensePromptEntries = false

        if let templateId = character.promptTemplateId, let getTemplate = options.getTemplate, let template = getTemplate(templateId) {
            baseEntries = template.entries.isEmpty ? defaultModularPromptEntries() : template.entries
            condensePromptEntries = template.condensePromptEntries
        } else if let getTemplate = options.getTemplate, let template = getTemplate(options.appDefaultTemplateId) {
            baseEntries = template.entries.isEmpty ? defaultModularPromptEntries() : template.entries
            condensePromptEntries = template.condensePromptEntries
        } else {
            baseEntries = defaultModularPromptEntries()
        }

        let lorebookContent = options.getLorebookContent?(character.id, session) ?? ""

        var renderedEntries: [SystemPromptEntry] = []
        for entry in baseEntries {
            if !entry.enabled && !entry.systemPrompt { continue }
            let rendered = renderWithContext(entry.content, character: character, persona: persona, session: session, settings: settings, lorebookContent: lorebookContent)
            if rendered.trimmingCharacters(in: .whitespaces).isEmpty { continue }
            var out = entry
            out.content = rendered
            renderedEntries.append(out)
        }

        if dynamicMemoryActive, !hasPlaceholder(baseEntries, "{{context_summary}}") {
            let summary = (session.memorySummary ?? "").trimmingCharacters(in: .whitespaces)
            if !summary.isEmpty {
                renderedEntries.append(SystemPromptEntry(id: "entry_context_summary", name: "Context Summary", role: "system", content: "# Context Summary\n\(summary)", enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: true))
            }
        }

        if !hasPlaceholder(baseEntries, "{{key_memories}}") {
            let hasMemories = dynamicMemoryActive ? !session.memoryEmbeddings.isEmpty : !session.memories.isEmpty
            if hasMemories {
                var content = "# Key Memories\nImportant facts to remember in this conversation:\n"
                if dynamicMemoryActive, !session.memoryEmbeddings.isEmpty {
                    content += session.memoryEmbeddings.map { "- \($0.text)" }.joined(separator: "\n")
                } else {
                    content += session.memories.map { "- \($0)" }.joined(separator: "\n")
                }
                renderedEntries.append(SystemPromptEntry(id: "entry_key_memories", name: "Key Memories", role: "system", content: content.trimmingCharacters(in: .whitespaces), enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: true))
            }
        }

        if !hasPlaceholder(baseEntries, "{{lorebook}}") {
            let lb = (options.getLorebookContent?(character.id, session) ?? "").trimmingCharacters(in: .whitespaces)
            if !lb.isEmpty {
                renderedEntries.append(SystemPromptEntry(id: "entry_lorebook", name: "World Information", role: "system", content: "# World Information\n\(lb)", enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: true))
            }
        }

        if condensePromptEntries, !renderedEntries.isEmpty {
            let merged = renderedEntries.map { $0.content.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }.joined(separator: "\n\n")
            if !merged.trimmingCharacters(in: .whitespaces).isEmpty {
                return [SystemPromptEntry(id: "entry_condensed_system", name: "Condensed System Prompt", role: "system", content: merged, enabled: true, injectionPosition: "relative", injectionDepth: 0, conditionalMinMessages: nil, intervalTurns: nil, systemPrompt: true)]
            }
        }

        return renderedEntries
    }

    /// 將條目合併成單一 system 字串（給 API 用）
    public static func systemPromptString(from entries: [SystemPromptEntry]) -> String {
        entries.map { $0.content }.filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }.joined(separator: "\n\n")
    }

    public static func formatLorebookForPrompt(entries: [(content: String)]) -> String {
        entries.map { $0.content.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }.joined(separator: "\n\n")
    }
}
