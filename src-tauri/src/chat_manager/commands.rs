use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::api::{api_request, ApiRequest};
use crate::chat_manager::storage::{get_base_prompt, get_base_prompt_entries, PromptType};
use crate::image_generator::types::ImageGenerationRequest;
use crate::storage_manager::media::{storage_load_avatar, storage_read_image_data};
use crate::utils::{log_info, now_millis};

use super::attachments::{cleanup_attachments, persist_attachments};
use super::execution::{find_model_and_credential, prepare_sampling_request};
use super::prompt_engine;
use super::prompts;
use super::request::extract_text;
use super::service::{resolve_api_key, ChatContext};
use crate::usage::tracking::UsageOperationType;

use super::storage::{
    default_character_rules, recent_messages, resolve_provider_credential_for_model,
};
use super::turn_builder::{
    partition_prompt_entries, role_swap_enabled, should_insert_in_chat_prompt_entry,
    swap_role_for_api, swapped_prompt_entities,
};
use super::types::{
    Character, ChatAddMessageAttachmentArgs, ChatCompletionArgs, ChatContinueArgs,
    ChatGenerateSceneImageArgs, ChatGenerateScenePromptArgs, ChatRegenerateArgs, ChatTurnResult,
    ContinueResult, ImageAttachment, Model, Persona, PromptEntryPosition, PromptScope,
    ProviderCredential, RegenerateResult, Session, Settings, StoredMessage, SystemPromptEntry,
    SystemPromptTemplate,
};
use crate::storage_manager::sessions::{messages_upsert_batch_typed, session_upsert_meta_typed};
fn resolve_persona_id<'a>(session: &'a Session, explicit: Option<&'a str>) -> Option<&'a str> {
    if explicit.is_some() {
        return explicit;
    }
    if session.persona_disabled {
        Some("")
    } else {
        session.persona_id.as_deref()
    }
}

#[allow(dead_code)]
fn has_image_generation_model(settings: &Settings) -> bool {
    settings.models.iter().any(|m| {
        m.output_scopes
            .iter()
            .any(|s| s.eq_ignore_ascii_case("image"))
    })
}

pub(crate) fn take_aborted_request(app: &AppHandle, request_id: Option<&str>) -> bool {
    let Some(request_id) = request_id else {
        return false;
    };

    let registry = app.state::<crate::abort_manager::AbortRegistry>();
    registry.take_aborted(request_id)
}

fn help_me_reply_participant_names<'a>(
    prompt_character: &'a Character,
    prompt_persona: Option<&'a Persona>,
) -> (&'a str, &'a str) {
    let effective_user_name = prompt_persona.map(|p| p.title.as_str()).unwrap_or("User");
    let effective_assistant_name = prompt_character.name.as_str();
    (effective_user_name, effective_assistant_name)
}

#[tauri::command]
pub async fn chat_completion(
    app: AppHandle,
    args: ChatCompletionArgs,
) -> Result<ChatTurnResult, String> {
    super::flows::completion::CompletionFlow::new(app)
        .execute(args)
        .await
}

#[tauri::command]
pub async fn chat_regenerate(
    app: AppHandle,
    args: ChatRegenerateArgs,
) -> Result<RegenerateResult, String> {
    super::flows::regenerate::RegenerateFlow::new(app)
        .execute(args)
        .await
}

#[tauri::command]
pub async fn chat_continue(
    app: AppHandle,
    args: ChatContinueArgs,
) -> Result<ContinueResult, String> {
    super::flows::continuation::ContinueFlow::new(app)
        .execute(args)
        .await
}

#[tauri::command]
pub fn get_default_character_rules(pure_mode_level: String) -> Vec<String> {
    default_character_rules(&pure_mode_level)
}

#[tauri::command]
pub fn get_default_system_prompt_template() -> String {
    get_base_prompt(PromptType::SystemPrompt)
}

// ==================== Prompt Template Commands ====================

#[tauri::command]
pub fn list_prompt_templates(app: AppHandle) -> Result<Vec<SystemPromptTemplate>, String> {
    prompts::load_templates(&app)
}

#[tauri::command]
pub fn create_prompt_template(
    app: AppHandle,
    name: String,
    scope: PromptScope,
    target_ids: Vec<String>,
    content: String,
    entries: Option<Vec<SystemPromptEntry>>,
    condense_prompt_entries: Option<bool>,
) -> Result<SystemPromptTemplate, String> {
    prompts::create_template(
        &app,
        name,
        scope,
        target_ids,
        content,
        entries,
        condense_prompt_entries,
    )
}

#[tauri::command]
pub fn update_prompt_template(
    app: AppHandle,
    id: String,
    name: Option<String>,
    scope: Option<PromptScope>,
    target_ids: Option<Vec<String>>,
    content: Option<String>,
    entries: Option<Vec<SystemPromptEntry>>,
    condense_prompt_entries: Option<bool>,
) -> Result<SystemPromptTemplate, String> {
    prompts::update_template(
        &app,
        id,
        name,
        scope,
        target_ids,
        content,
        entries,
        condense_prompt_entries,
    )
}

#[tauri::command]
pub fn delete_prompt_template(app: AppHandle, id: String) -> Result<(), String> {
    prompts::delete_template(&app, id)
}

#[tauri::command]
pub fn get_prompt_template(
    app: AppHandle,
    id: String,
) -> Result<Option<SystemPromptTemplate>, String> {
    prompts::get_template(&app, &id)
}

#[tauri::command]
pub fn export_prompt_template_as_usc(app: AppHandle, id: String) -> Result<String, String> {
    let template =
        prompts::get_template(&app, &id)?.ok_or_else(|| format!("Template not found: {}", id))?;
    let card = crate::storage_manager::system_cards::create_system_prompt_template_usc(&template);
    serde_json::to_string_pretty(&card).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize USC prompt template export: {}", e),
        )
    })
}

#[tauri::command]
pub fn chat_template_export_as_usc(template_json: String) -> Result<String, String> {
    let value: Value = serde_json::from_str(&template_json).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Invalid chat template JSON for export: {}", e),
        )
    })?;

    let id = value
        .get("id")
        .and_then(|item| item.as_str())
        .ok_or_else(|| {
            crate::utils::err_msg(module_path!(), line!(), "Chat template id is required")
        })?
        .to_string();
    let name = value
        .get("name")
        .and_then(|item| item.as_str())
        .ok_or_else(|| {
            crate::utils::err_msg(module_path!(), line!(), "Chat template name is required")
        })?
        .to_string();
    let scene_id = value
        .get("sceneId")
        .and_then(|item| item.as_str())
        .map(|item| item.to_string());
    let prompt_template_id = value
        .get("promptTemplateId")
        .and_then(|item| item.as_str())
        .map(|item| item.to_string());
    let created_at = value
        .get("createdAt")
        .and_then(|item| item.as_i64())
        .unwrap_or_else(|| now_millis().unwrap_or(0) as i64);

    let template = crate::sync::models::ChatTemplate {
        id: id.clone(),
        character_id: String::new(),
        name,
        scene_id,
        prompt_template_id,
        created_at,
    };

    let messages = value
        .get("messages")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(idx, message)| crate::sync::models::ChatTemplateMessage {
            id: message
                .get("id")
                .and_then(|item| item.as_str())
                .unwrap_or_default()
                .to_string(),
            template_id: id.clone(),
            idx: idx as i64,
            role: message
                .get("role")
                .and_then(|item| item.as_str())
                .unwrap_or("assistant")
                .to_string(),
            content: message
                .get("content")
                .and_then(|item| item.as_str())
                .unwrap_or_default()
                .to_string(),
        })
        .collect::<Vec<_>>();

    let card = crate::storage_manager::system_cards::create_chat_template_usc(&template, &messages);
    serde_json::to_string_pretty(&card).map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Failed to serialize USC chat template export: {}", e),
        )
    })
}

#[tauri::command]
pub fn get_app_default_template_id() -> String {
    prompts::APP_DEFAULT_TEMPLATE_ID.to_string()
}

#[tauri::command]
pub fn is_app_default_template(id: String) -> bool {
    prompts::is_app_default_template(&id)
}

#[tauri::command]
pub fn reset_app_default_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_app_default_template(&app)
}

#[tauri::command]
pub fn reset_dynamic_summary_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_dynamic_summary_template(&app)
}

#[tauri::command]
pub fn reset_dynamic_memory_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_dynamic_memory_template(&app)
}

#[tauri::command]
pub fn reset_help_me_reply_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_help_me_reply_template(&app)
}

#[tauri::command]
pub fn reset_help_me_reply_conversational_template(
    app: AppHandle,
) -> Result<SystemPromptTemplate, String> {
    prompts::reset_help_me_reply_conversational_template(&app)
}

#[tauri::command]
pub fn reset_avatar_generation_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_avatar_generation_template(&app)
}

#[tauri::command]
pub fn reset_avatar_edit_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_avatar_edit_template(&app)
}

#[tauri::command]
pub fn reset_scene_generation_template(app: AppHandle) -> Result<SystemPromptTemplate, String> {
    prompts::reset_scene_generation_template(&app)
}

#[tauri::command]
pub fn get_required_template_variables(template_id: String) -> Vec<String> {
    prompts::get_required_variables(&template_id)
}

#[tauri::command]
pub fn validate_template_variables(
    template_id: String,
    content: String,
    entries: Option<Vec<SystemPromptEntry>>,
) -> Result<(), String> {
    let validation_text = if let Some(entries) = entries {
        if entries.is_empty() {
            content
        } else {
            entries
                .iter()
                .map(|entry| entry.content.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        content
    };
    prompts::validate_required_variables(&template_id, &validation_text)
        .map_err(|missing| format!("Missing required variables: {}", missing.join(", ")))
}

// Deprecated: get_applicable_prompts_for_* commands removed in favor of global list on client

// ==================== Prompt Preview Command ====================

#[tauri::command]
pub fn render_prompt_preview(
    app: AppHandle,
    content: String,
    character_id: String,
    session_id: Option<String>,
    persona_id: Option<String>,
) -> Result<String, String> {
    let context = super::service::ChatContext::initialize(app.clone())?;
    let settings = &context.settings;

    let character = context.find_character(&character_id)?;

    // Load session if provided, otherwise synthesize a minimal one
    let session: Session = if let Some(sid) = session_id.as_ref() {
        context
            .load_session(sid)
            .and_then(|opt| opt.ok_or_else(|| "Session not found".to_string()))?
    } else {
        // Minimal ephemeral session for preview
        let now = now_millis()?;
        Session {
            id: "preview".to_string(),
            character_id: character.id.clone(),
            title: "Preview".to_string(),
            system_prompt: None,
            selected_scene_id: None,
            prompt_template_id: None,
            persona_id: None,
            persona_disabled: false,
            voice_autoplay: None,
            advanced_model_settings: None,
            messages: vec![],
            archived: false,
            created_at: now,
            updated_at: now,
            memory_status: None,
            memory_error: None,
            memories: vec![
                "Memory 1 (Preview): The user prefers direct communication.".to_string(),
                "Memory 2 (Preview): We met in the tavern last night.".to_string(),
            ],
            memory_embeddings: vec![],
            memory_summary: Some("This is a placeholder for the context summary that will be generated by the AI based on your conversation history.".to_string()),
            memory_summary_token_count: 0,
            memory_tool_events: vec![],
        }
    };

    let effective_persona_id = resolve_persona_id(&session, persona_id.as_deref());
    let persona = context.choose_persona(effective_persona_id);

    let rendered =
        prompt_engine::render_with_context(&app, &content, &character, persona, &session, settings);
    Ok(rendered)
}

#[tauri::command]
pub async fn retry_dynamic_memory(
    app: AppHandle,
    session_id: String,
    model_id: Option<String>,
    update_default: Option<bool>,
) -> Result<(), String> {
    super::memory::flow::retry_dynamic_memory(app, session_id, model_id, update_default).await
}

#[tauri::command]
pub async fn trigger_dynamic_memory(app: AppHandle, session_id: String) -> Result<(), String> {
    super::memory::flow::trigger_dynamic_memory(app, session_id).await
}

#[tauri::command]
pub fn abort_dynamic_memory(app: AppHandle, session_id: String) -> Result<(), String> {
    super::memory::flow::abort_dynamic_memory(app, session_id)
}

fn resolve_image_generation_target<'a>(
    settings: &'a Settings,
    preferred_model_id: Option<&str>,
) -> Result<(&'a Model, &'a ProviderCredential), String> {
    if let Some(model_id) = preferred_model_id.filter(|id| !id.trim().is_empty()) {
        let (model, provider_cred) = find_model_and_credential(settings, model_id)
            .ok_or_else(|| "Configured scene generation model could not be resolved".to_string())?;
        let supports_image_output = model
            .output_scopes
            .iter()
            .any(|scope| scope.eq_ignore_ascii_case("image"));
        if !supports_image_output {
            return Err(
                "Configured scene generation model does not support image output".to_string(),
            );
        }
        return Ok((model, provider_cred));
    }

    settings
        .models
        .iter()
        .find_map(|model| {
            let supports_image_output = model
                .output_scopes
                .iter()
                .any(|scope| scope.eq_ignore_ascii_case("image"));
            if !supports_image_output {
                return None;
            }
            let provider_cred = resolve_provider_credential_for_model(settings, model)?;
            Some((model, provider_cred))
        })
        .ok_or_else(|| "No image generation model is configured".to_string())
}

fn scene_generation_enabled(settings: &Settings) -> bool {
    settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.scene_generation_enabled)
        .unwrap_or(true)
}

fn resolve_avatar_reference_data(
    app: &AppHandle,
    entity_prefix: &str,
    entity_id: &str,
    avatar_path: Option<&str>,
) -> Option<String> {
    let prefixed_entity_id = format!("{}-{}", entity_prefix, entity_id);

    storage_load_avatar(
        app.clone(),
        prefixed_entity_id.clone(),
        "avatar_base.webp".to_string(),
    )
    .ok()
    .or_else(|| {
        let filename = avatar_path?.trim();
        if filename.is_empty() || filename.eq_ignore_ascii_case("avatar_base.webp") {
            return None;
        }
        storage_load_avatar(app.clone(), prefixed_entity_id, filename.to_string()).ok()
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SceneReferenceSource {
    None,
    AvatarFallback,
    DesignImages,
}

struct SceneReferenceImages {
    character_images: Vec<String>,
    character_reference_count: usize,
    character_reference_source: SceneReferenceSource,
    persona_images: Vec<String>,
    persona_reference_count: usize,
    persona_reference_source: SceneReferenceSource,
}

fn resolve_design_reference_images(app: &AppHandle, image_ids: &[String]) -> Vec<String> {
    image_ids
        .iter()
        .filter_map(|image_id| {
            let image_id = image_id.trim();
            if image_id.is_empty() {
                return None;
            }
            storage_read_image_data(app, image_id).ok()
        })
        .collect()
}

fn build_scene_reference_images(
    app: &AppHandle,
    character: &Character,
    persona: Option<&Persona>,
) -> SceneReferenceImages {
    let character_design_images =
        resolve_design_reference_images(app, &character.design_reference_image_ids);
    let (character_images, character_reference_count, character_reference_source) =
        if !character_design_images.is_empty() {
            let count = character_design_images.len();
            (
                character_design_images,
                count,
                SceneReferenceSource::DesignImages,
            )
        } else if let Some(character_image) = resolve_avatar_reference_data(
            app,
            "character",
            &character.id,
            character.avatar_path.as_deref(),
        ) {
            (
                vec![character_image],
                1,
                SceneReferenceSource::AvatarFallback,
            )
        } else {
            (Vec::new(), 0, SceneReferenceSource::None)
        };

    let mut persona_images = Vec::new();
    let mut persona_reference_count = 0;
    let mut persona_reference_source = SceneReferenceSource::None;
    if let Some(persona) = persona {
        let persona_design_images =
            resolve_design_reference_images(app, &persona.design_reference_image_ids);
        if !persona_design_images.is_empty() {
            persona_reference_count = persona_design_images.len();
            persona_reference_source = SceneReferenceSource::DesignImages;
            persona_images = persona_design_images;
        } else if let Some(persona_image) = resolve_avatar_reference_data(
            app,
            "persona",
            &persona.id,
            persona.avatar_path.as_deref(),
        ) {
            persona_images.push(persona_image);
            persona_reference_count = 1;
            persona_reference_source = SceneReferenceSource::AvatarFallback;
        }
    }

    SceneReferenceImages {
        character_images,
        character_reference_count,
        character_reference_source,
        persona_images,
        persona_reference_count,
        persona_reference_source,
    }
}

fn format_scene_reference_range(start_index: usize, count: usize) -> String {
    if count <= 1 {
        format!("attached image {}", start_index)
    } else {
        format!(
            "attached images {}-{}",
            start_index,
            start_index + count - 1
        )
    }
}

fn persona_scene_name(persona: Option<&Persona>) -> String {
    persona
        .and_then(|value| value.nickname.as_deref())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| persona.map(|value| value.title.as_str()))
        .unwrap_or("the persona")
        .to_string()
}

fn build_scene_prompt_reference_hint(
    entity_name: &str,
    reference_count: usize,
    reference_source: SceneReferenceSource,
) -> String {
    match reference_source {
        SceneReferenceSource::DesignImages if reference_count > 0 => format!(
            "The image model will receive {} saved design reference image{} for {}.",
            reference_count,
            if reference_count == 1 { "" } else { "s" },
            entity_name
        ),
        SceneReferenceSource::AvatarFallback => {
            format!(
                "The image model will receive {}'s base avatar as a visual reference.",
                entity_name
            )
        }
        SceneReferenceSource::None => String::new(),
        SceneReferenceSource::DesignImages => String::new(),
    }
}

fn build_scene_prompt_reference_text(
    entity_name: &str,
    design_description: Option<&str>,
    reference_count: usize,
    reference_source: SceneReferenceSource,
) -> String {
    let mut sections = Vec::new();

    if let Some(description) = design_description
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sections.push(format!(
            "# {} Reference Notes\n{}",
            entity_name, description
        ));
    }

    let reference_hint =
        build_scene_prompt_reference_hint(entity_name, reference_count, reference_source);
    if !reference_hint.is_empty() {
        sections.push(reference_hint);
    }

    sections.join("\n\n")
}

fn build_scene_prompt_image_parts(images: &[String]) -> Vec<Value> {
    images
        .iter()
        .filter(|image| !image.trim().is_empty())
        .map(|image| {
            json!({
                "type": "image_url",
                "image_url": {
                    "url": image,
                    "detail": "auto"
                }
            })
        })
        .collect()
}

fn build_scene_prompt_content_with_images(
    content: &str,
    reference_images: &SceneReferenceImages,
) -> Option<Value> {
    const CHARACTER_TOKEN: &str = "{{image[character]}}";
    const PERSONA_TOKEN: &str = "{{image[persona]}}";

    if !content.contains(CHARACTER_TOKEN) && !content.contains(PERSONA_TOKEN) {
        return None;
    }

    let mut parts: Vec<Value> = Vec::new();
    if content.contains(CHARACTER_TOKEN) {
        parts.extend(build_scene_prompt_image_parts(
            &reference_images.character_images,
        ));
    }
    if content.contains(PERSONA_TOKEN) {
        parts.extend(build_scene_prompt_image_parts(
            &reference_images.persona_images,
        ));
    }

    if parts.is_empty() {
        None
    } else {
        Some(Value::Array(parts))
    }
}

fn build_scene_generation_request(
    scene_prompt: &str,
    model: &Model,
    provider_cred: &ProviderCredential,
    character: &Character,
    persona: Option<&Persona>,
    reference_images: SceneReferenceImages,
) -> ImageGenerationRequest {
    let SceneReferenceImages {
        character_images,
        character_reference_count,
        character_reference_source,
        persona_images,
        persona_reference_count,
        persona_reference_source,
    } = reference_images;
    let mut prompt_sections = Vec::new();
    let mut input_images = character_images.clone();
    input_images.extend(persona_images.clone());
    let has_character_reference = character_reference_count > 0;
    let has_persona_reference = persona_reference_count > 0;

    if let Some(design_description) = character
        .design_description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        prompt_sections.push(format!(
            "Character design notes for {}:\n{}",
            character.name, design_description
        ));
    }

    if let Some(persona) = persona {
        if let Some(design_description) = persona
            .design_description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let persona_name = persona_scene_name(Some(persona));
            prompt_sections.push(format!(
                "Persona design notes for {}:\n{}",
                persona_name, design_description
            ));
        }
    }

    if has_character_reference || has_persona_reference {
        let mut reference_lines = Vec::new();
        let mut next_image_index = 1;
        if has_character_reference {
            let range_label =
                format_scene_reference_range(next_image_index, character_reference_count);
            let source_label = match character_reference_source {
                SceneReferenceSource::DesignImages => "saved character design reference",
                SceneReferenceSource::AvatarFallback => "base avatar reference",
                SceneReferenceSource::None => "character reference",
            };
            reference_lines.push(format!(
                "The {} is the {} for {}. Use it only for {}'s identity, face, body, outfit cues, and signature styling.",
                range_label, source_label, character.name, character.name
            ));
            next_image_index += character_reference_count;
        }
        if has_persona_reference {
            let persona_name = persona_scene_name(persona);
            let range_label =
                format_scene_reference_range(next_image_index, persona_reference_count);
            let source_label = match persona_reference_source {
                SceneReferenceSource::DesignImages => "saved persona design reference",
                SceneReferenceSource::AvatarFallback => "base avatar reference",
                SceneReferenceSource::None => "persona reference",
            };
            reference_lines.push(format!(
                "The {} is the {} for {}. Use it only for {}'s identity, face, body, outfit cues, and signature styling.",
                range_label, source_label, persona_name, persona_name
            ));
        }
        reference_lines.push(
            "Do not swap, merge, or borrow identity-defining features between reference images."
                .to_string(),
        );
        match (has_character_reference, has_persona_reference) {
            (true, false) => reference_lines.push(format!(
                "Only {} has a reference image attached. Do not invent {} from {}'s appearance.",
                character.name,
                persona
                    .and_then(|value| value.nickname.as_deref())
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| persona
                        .map(|value| value.title.as_str())
                        .unwrap_or("the persona")),
                character.name
            )),
            (false, true) => reference_lines.push(format!(
                "Only {} has a reference image attached. Do not invent {} from {}'s appearance.",
                persona
                    .and_then(|value| value.nickname.as_deref())
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| persona
                        .map(|value| value.title.as_str())
                        .unwrap_or("the persona")),
                character.name,
                persona
                    .and_then(|value| value.nickname.as_deref())
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| persona
                        .map(|value| value.title.as_str())
                        .unwrap_or("the persona"))
            )),
            _ => {}
        }
        prompt_sections.push(reference_lines.join("\n"));
    }

    prompt_sections.push(scene_prompt.trim().to_string());

    ImageGenerationRequest {
        prompt: prompt_sections.join("\n\n"),
        model: model.name.clone(),
        provider_id: model.provider_id.clone(),
        credential_id: provider_cred.id.clone(),
        input_images: if input_images.is_empty() {
            None
        } else {
            Some(input_images)
        },
        size: Some("1024x1024".to_string()),
        quality: None,
        style: None,
        n: Some(1),
    }
}

async fn generate_scene_image_with_retry(
    app: &AppHandle,
    request: ImageGenerationRequest,
    max_attempts: usize,
) -> Result<crate::image_generator::types::ImageGenerationResponse, String> {
    let mut last_error: Option<String> = None;

    for attempt in 1..=max_attempts.max(1) {
        match crate::image_generator::commands::generate_image(app.clone(), request.clone()).await {
            Ok(response) if !response.images.is_empty() => return Ok(response),
            Ok(_) => {
                let error = "No images found in response".to_string();
                if attempt >= max_attempts {
                    return Err(error);
                }
                last_error = Some(error);
            }
            Err(error) => {
                if !error.to_ascii_lowercase().contains("no image") || attempt >= max_attempts {
                    return Err(error);
                }
                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "No images found in response".to_string()))
}

fn build_scene_prompt_context_messages(
    session: &Session,
    message_id: &str,
) -> Result<String, String> {
    let target_index = session
        .messages
        .iter()
        .position(|message| message.id == message_id)
        .ok_or_else(|| "Message not found in loaded session window".to_string())?;

    let start_index = target_index.saturating_sub(2);
    let context_slice = &session.messages[start_index..=target_index];

    let context = context_slice
        .iter()
        .filter(|message| {
            matches!(message.role.as_str(), "user" | "assistant" | "scene")
                && !message.content.trim().is_empty()
        })
        .map(|message| {
            let role = match message.role.as_str() {
                "assistant" => "Assistant",
                "scene" => "Scene",
                _ => "User",
            };
            format!("{}: {}", role, message.content.trim())
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    if context.trim().is_empty() {
        return Err("No conversation context available for scene prompt generation".to_string());
    }

    Ok(context)
}

fn condense_prompt_whitespace(input: String) -> String {
    let mut output = input;
    while output.contains("\n\n\n") {
        output = output.replace("\n\n\n", "\n\n");
    }
    output.trim().to_string()
}

fn render_scene_generation_prompt_content(
    template_content: &str,
    character: &Character,
    persona: Option<&Persona>,
    recent_messages_text: &str,
) -> String {
    let mut prompt = template_content.to_string();
    let char_name = character.name.as_str();
    let mut char_desc_parts = Vec::new();
    if let Some(value) = character
        .definition
        .as_deref()
        .or(character.description.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        char_desc_parts.push(value.to_string());
    }
    if let Some(value) = character
        .design_description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        char_desc_parts.push(format!("Visual design notes: {}", value));
    }
    let char_desc = char_desc_parts.join("\n\n");
    let persona_name = persona.map(|value| value.title.as_str()).unwrap_or("User");
    let mut persona_desc_parts = Vec::new();
    if let Some(value) = persona
        .map(|value| value.description.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        persona_desc_parts.push(value.to_string());
    }
    if let Some(value) = persona
        .and_then(|value| value.design_description.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        persona_desc_parts.push(format!("Visual design notes: {}", value));
    }
    let persona_desc = persona_desc_parts.join("\n\n");
    let character_reference_text = build_scene_prompt_reference_text(
        char_name,
        character.design_description.as_deref(),
        character.design_reference_image_ids.len(),
        if !character.design_reference_image_ids.is_empty() {
            SceneReferenceSource::DesignImages
        } else if character.avatar_path.is_some() {
            SceneReferenceSource::AvatarFallback
        } else {
            SceneReferenceSource::None
        },
    );
    let persona_reference_text = if let Some(persona) = persona {
        build_scene_prompt_reference_text(
            &persona_scene_name(Some(persona)),
            persona.design_description.as_deref(),
            persona.design_reference_image_ids.len(),
            if !persona.design_reference_image_ids.is_empty() {
                SceneReferenceSource::DesignImages
            } else if persona.avatar_path.is_some() {
                SceneReferenceSource::AvatarFallback
            } else {
                SceneReferenceSource::None
            },
        )
    } else {
        String::new()
    };
    prompt = prompt.replace("{{char.name}}", char_name);
    prompt = prompt.replace("{{char}}", char_name);
    prompt = prompt.replace("{{user}}", persona_name);
    prompt = prompt.replace("{{persona}}", persona_name);
    prompt = prompt.replace("{{char.desc}}", &char_desc);
    prompt = prompt.replace("{{persona.name}}", persona_name);
    prompt = prompt.replace("{{persona.desc}}", &persona_desc);
    prompt = prompt.replace("{{recent_messages}}", recent_messages_text);
    let scene_request = if let Some(persona) = persona {
        format!(
            "Create one polished scene image prompt for the visual moment described by the recent messages. Focus on the currently active beat involving {} and {}. Keep {} and {} visually distinct, and make the result immediately usable for image generation.",
            character.name, persona.title, character.name, persona.title
        )
    } else {
        format!(
            "Create one polished scene image prompt for the visual moment described by the recent messages. Focus on the currently active beat involving {}. Make the result immediately usable for image generation.",
            character.name
        )
    };
    prompt = prompt.replace("{{scene_request}}", &scene_request);
    prompt = prompt.replace("{{reference[character]}}", &character_reference_text);
    prompt = prompt.replace("{{reference[persona]}}", &persona_reference_text);

    condense_prompt_whitespace(prompt)
}

fn condense_scene_generation_entries(entries: Vec<SystemPromptEntry>) -> Vec<SystemPromptEntry> {
    let merged = entries
        .into_iter()
        .filter_map(|entry| {
            let trimmed = entry.content.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    if merged.trim().is_empty() {
        return Vec::new();
    }

    vec![SystemPromptEntry {
        id: "scene_gen_condensed_system".to_string(),
        name: "Condensed Scene Generation Prompt".to_string(),
        role: super::types::PromptEntryRole::System,
        content: merged,
        enabled: true,
        injection_position: PromptEntryPosition::Relative,
        injection_depth: 0,
        conditional_min_messages: None,
        interval_turns: None,
        system_prompt: true,
    }]
}

fn load_scene_generation_prompt_entries(app: &AppHandle) -> (Vec<SystemPromptEntry>, bool) {
    match prompts::get_template(app, prompts::APP_SCENE_GENERATION_TEMPLATE_ID) {
        Ok(Some(template)) => {
            if !template.entries.is_empty() {
                (template.entries, template.condense_prompt_entries)
            } else if !template.content.trim().is_empty() {
                (
                    vec![SystemPromptEntry {
                        id: "scene_gen_single_entry".to_string(),
                        name: "Scene Generation Prompt".to_string(),
                        role: super::types::PromptEntryRole::System,
                        content: template.content,
                        enabled: true,
                        injection_position: PromptEntryPosition::Relative,
                        injection_depth: 0,
                        conditional_min_messages: None,
                        interval_turns: None,
                        system_prompt: true,
                    }],
                    template.condense_prompt_entries,
                )
            } else {
                (
                    get_base_prompt_entries(PromptType::SceneGenerationPrompt),
                    false,
                )
            }
        }
        _ => (
            get_base_prompt_entries(PromptType::SceneGenerationPrompt),
            false,
        ),
    }
}

fn render_scene_generation_prompt_entries(
    app: &AppHandle,
    character: &Character,
    persona: Option<&Persona>,
    recent_messages_text: &str,
) -> Vec<SystemPromptEntry> {
    let (template_entries, condense_prompt_entries) = load_scene_generation_prompt_entries(app);
    let mut rendered_entries = Vec::new();

    for entry in template_entries {
        if !entry.enabled && !entry.system_prompt {
            continue;
        }
        let rendered = render_scene_generation_prompt_content(
            &entry.content,
            character,
            persona,
            recent_messages_text,
        );
        if rendered.trim().is_empty() {
            continue;
        }
        let mut next_entry = entry.clone();
        next_entry.content = rendered;
        rendered_entries.push(next_entry);
    }

    if condense_prompt_entries {
        condense_scene_generation_entries(rendered_entries)
    } else {
        rendered_entries
    }
}

fn scene_prompt_entry_to_message(
    entry: &SystemPromptEntry,
    system_role: &str,
    reference_images: &SceneReferenceImages,
    character: &Character,
    persona: Option<&Persona>,
) -> Option<Value> {
    if let Some(content) = build_scene_prompt_content_with_images(&entry.content, reference_images)
    {
        return Some(json!({ "role": "user", "content": content }));
    }

    let role = match entry.role {
        super::types::PromptEntryRole::System => system_role,
        super::types::PromptEntryRole::User => "user",
        super::types::PromptEntryRole::Assistant => "assistant",
    };

    let content = if role == system_role {
        Value::String(content_with_scene_image_hints(
            &entry.content,
            reference_images,
            character,
            persona,
        ))
    } else {
        Value::String(entry.content.clone())
    };

    Some(json!({ "role": role, "content": content }))
}

fn content_with_scene_image_hints(
    content: &str,
    reference_images: &SceneReferenceImages,
    character: &Character,
    persona: Option<&Persona>,
) -> String {
    content
        .replace(
            "{{image[character]}}",
            &build_scene_prompt_reference_hint(
                character.name.as_str(),
                reference_images.character_reference_count,
                reference_images.character_reference_source,
            ),
        )
        .replace(
            "{{image[persona]}}",
            &build_scene_prompt_reference_hint(
                &persona_scene_name(persona),
                reference_images.persona_reference_count,
                reference_images.persona_reference_source,
            ),
        )
}

fn insert_scene_in_chat_prompt_entries(
    messages: &mut Vec<Value>,
    system_role: &str,
    entries: &[SystemPromptEntry],
    reference_images: &SceneReferenceImages,
    character: &Character,
    persona: Option<&Persona>,
) {
    if entries.is_empty() {
        return;
    }
    let base_len = messages.len();
    let turn_count = base_len;
    let mut inserts: Vec<(usize, usize, &SystemPromptEntry)> = entries
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| {
            if !should_insert_in_chat_prompt_entry(entry, turn_count) {
                return None;
            }
            let depth = entry.injection_depth as usize;
            let pos = base_len.saturating_sub(depth);
            Some((pos, idx, entry))
        })
        .collect();
    inserts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    for (offset, (pos, _, entry)) in inserts.into_iter().enumerate() {
        let insert_at = (pos + offset).min(messages.len());
        if let Some(message) =
            scene_prompt_entry_to_message(entry, system_role, reference_images, character, persona)
        {
            messages.insert(insert_at, message);
        }
    }
}

#[tauri::command]
pub async fn chat_add_message_attachment(
    app: AppHandle,
    args: ChatAddMessageAttachmentArgs,
) -> Result<StoredMessage, String> {
    let ChatAddMessageAttachmentArgs {
        session_id,
        character_id,
        message_id,
        role,
        attachment_id,
        base64_data,
        mime_type,
        filename,
        width,
        height,
    } = args;

    if base64_data.trim().is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "base64Data cannot be empty",
        ));
    }

    let mut session = super::storage::load_session(&app, &session_id)?
        .ok_or_else(|| "Session not found".to_string())?;

    let target_index = session
        .messages
        .iter()
        .position(|m| m.id == message_id)
        .ok_or_else(|| "Message not found in loaded session window".to_string())?;

    let new_attachment = persist_attachments(
        &app,
        &character_id,
        &session_id,
        &message_id,
        &role,
        vec![ImageAttachment {
            id: attachment_id,
            data: base64_data,
            mime_type,
            filename,
            width,
            height,
            storage_path: None,
        }],
    )?
    .into_iter()
    .next()
    .ok_or_else(|| "Failed to persist attachment".to_string())?;

    let updated_message = {
        let target = &mut session.messages[target_index];
        if let Some(existing) = target
            .attachments
            .iter_mut()
            .find(|att| att.id == new_attachment.id)
        {
            *existing = new_attachment;
        } else {
            target.attachments.push(new_attachment);
        }
        target.clone()
    };

    session.updated_at = now_millis()?;

    // Persist meta + the updated message (even if it's not the last message).
    let mut meta = session.clone();
    meta.messages = Vec::new();
    session_upsert_meta_typed(&app, &meta)?;
    messages_upsert_batch_typed(&app, &session_id, std::slice::from_ref(&updated_message))?;

    Ok(updated_message)
}

#[tauri::command]
pub async fn chat_generate_scene_image(
    app: AppHandle,
    args: ChatGenerateSceneImageArgs,
) -> Result<StoredMessage, String> {
    let ChatGenerateSceneImageArgs {
        session_id,
        message_id,
        attachment_id,
        scene_prompt,
    } = args;

    if scene_prompt.trim().is_empty() {
        return Err("scenePrompt cannot be empty".to_string());
    }

    let mut session = super::storage::load_session(&app, &session_id)?
        .ok_or_else(|| "Session not found".to_string())?;

    let target_index = session
        .messages
        .iter()
        .position(|message| message.id == message_id)
        .ok_or_else(|| "Message not found in loaded session window".to_string())?;

    let settings = super::storage::load_settings(&app)?;
    if !scene_generation_enabled(&settings) {
        return Err("Scene generation is disabled in settings".to_string());
    }
    let (model, provider_cred) = resolve_image_generation_target(
        &settings,
        settings
            .advanced_settings
            .as_ref()
            .and_then(|advanced| advanced.scene_generation_model_id.as_deref()),
    )?;

    let characters = super::storage::load_characters(&app)?;
    let character = characters
        .iter()
        .find(|value| value.id == session.character_id)
        .ok_or_else(|| "Session character not found".to_string())?;

    let personas = super::storage::load_personas(&app)?;
    let persona = if session.persona_disabled {
        None
    } else {
        session
            .persona_id
            .as_deref()
            .and_then(|persona_id| personas.iter().find(|value| value.id == persona_id))
    };

    let reference_images = build_scene_reference_images(&app, character, persona);
    let request = build_scene_generation_request(
        &scene_prompt,
        model,
        provider_cred,
        character,
        persona,
        reference_images,
    );
    let response = generate_scene_image_with_retry(&app, request, 3).await?;
    let generated = response
        .images
        .into_iter()
        .next()
        .ok_or_else(|| "No images found in response".to_string())?;
    let generated_data = storage_read_image_data(&app, &generated.asset_id)?;

    let persisted_attachments = persist_attachments(
        &app,
        &session.character_id,
        &session.id,
        &message_id,
        "assistant",
        vec![ImageAttachment {
            id: attachment_id.clone(),
            data: generated_data,
            mime_type: generated.mime_type,
            filename: Some(scene_prompt),
            width: generated.width,
            height: generated.height,
            storage_path: None,
        }],
    )?;

    let persisted_attachment = persisted_attachments
        .into_iter()
        .next()
        .ok_or_else(|| "Failed to persist generated scene attachment".to_string())?;
    let cleanup_attachment = persisted_attachment.clone();

    let updated_message = {
        let target = &mut session.messages[target_index];
        if let Some(existing) = target
            .attachments
            .iter_mut()
            .find(|attachment| attachment.id == attachment_id)
        {
            *existing = persisted_attachment;
        } else {
            target.attachments.push(persisted_attachment);
        }
        target.clone()
    };

    session.updated_at = now_millis()?;

    let mut meta = session.clone();
    meta.messages = Vec::new();
    if let Err(err) = session_upsert_meta_typed(&app, &meta) {
        cleanup_attachments(
            &app,
            std::slice::from_ref(&cleanup_attachment),
            "chat_generate_scene_image",
        );
        return Err(err);
    }

    if let Err(err) =
        messages_upsert_batch_typed(&app, &session_id, std::slice::from_ref(&updated_message))
    {
        cleanup_attachments(
            &app,
            std::slice::from_ref(&cleanup_attachment),
            "chat_generate_scene_image",
        );
        return Err(err);
    }

    Ok(updated_message)
}

#[tauri::command]
pub async fn chat_generate_scene_prompt(
    app: AppHandle,
    args: ChatGenerateScenePromptArgs,
) -> Result<String, String> {
    let ChatGenerateScenePromptArgs {
        session_id,
        message_id,
    } = args;

    let context = ChatContext::initialize(app.clone())?;
    let settings = &context.settings;
    if !scene_generation_enabled(settings) {
        return Err("Scene generation is disabled in settings".to_string());
    }
    let session = context
        .load_session(&session_id)?
        .ok_or_else(|| "Session not found".to_string())?;
    let character = context.find_character(&session.character_id)?;
    let persona = context.choose_persona(resolve_persona_id(&session, None));
    let (model, provider_cred) = context.select_model(&character)?;
    let api_key = resolve_api_key(&app, provider_cred, "scene_prompt")?;

    let recent_messages_text = build_scene_prompt_context_messages(&session, &message_id)?;
    let reference_images = build_scene_reference_images(&app, &character, persona);
    let prompt_entries =
        render_scene_generation_prompt_entries(&app, &character, persona, &recent_messages_text);
    if prompt_entries.is_empty() {
        return Err("Scene generation prompt template rendered no usable entries".to_string());
    }

    let (relative_entries, in_chat_entries) = partition_prompt_entries(prompt_entries);
    let mut messages_for_api: Vec<Value> = relative_entries
        .iter()
        .filter_map(|entry| {
            scene_prompt_entry_to_message(entry, "system", &reference_images, &character, persona)
        })
        .collect();
    insert_scene_in_chat_prompt_entries(
        &mut messages_for_api,
        "system",
        &in_chat_entries,
        &reference_images,
        &character,
        persona,
    );

    let (request_settings, extra_body_fields) = prepare_sampling_request(
        &provider_cred.provider_id,
        &session,
        model,
        settings,
        1280,
        0.7,
        1.0,
        None,
        None,
        None,
    );

    let built = super::request_builder::build_chat_request(
        provider_cred,
        &api_key,
        &model.name,
        &messages_for_api,
        None,
        request_settings.temperature,
        request_settings.top_p,
        request_settings.max_tokens,
        request_settings.context_length,
        false,
        None,
        None,
        None,
        None,
        None,
        request_settings.reasoning_enabled,
        request_settings.reasoning_effort.clone(),
        request_settings.reasoning_budget,
        extra_body_fields,
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(60_000),
        stream: Some(false),
        request_id: None,
        provider_id: Some(provider_cred.provider_id.clone()),
    };

    let api_response = api_request(app.clone(), api_request_payload).await?;
    if !api_response.ok {
        return Err(format!(
            "API request failed with status {}",
            api_response.status
        ));
    }

    let generated_text = extract_text(&api_response.data, Some(&provider_cred.provider_id))
        .ok_or_else(|| "Failed to extract text from response".to_string())?;

    let cleaned = condense_prompt_whitespace(
        generated_text
            .trim()
            .trim_matches('"')
            .trim()
            .trim_start_matches("<img>")
            .trim_end_matches("</img>")
            .trim_end_matches("[CONTINUE]")
            .trim_end_matches("[continue]")
            .trim_end_matches("[/continue]")
            .trim()
            .to_string(),
    );

    let usage = super::sse::usage_from_value(&api_response.data);
    super::service::record_usage_if_available(
        &context,
        &usage,
        &session,
        &character,
        model,
        provider_cred,
        &api_key,
        now_millis().unwrap_or(0),
        UsageOperationType::ReplyHelper,
        "scene_prompt",
    )
    .await;

    if cleaned.is_empty() {
        return Err("Scene prompt generation returned an empty result".to_string());
    }

    Ok(cleaned)
}

#[tauri::command]
pub async fn search_messages(
    app: AppHandle,
    session_id: String,
    query: String,
) -> Result<Vec<super::types::MessageSearchResult>, String> {
    let context = ChatContext::initialize(app.clone())?;

    let session = match context.load_session(&session_id)? {
        Some(s) => s,
        None => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Session not found",
            ))
        }
    };

    let query_lower = query.to_lowercase();
    let results: Vec<super::types::MessageSearchResult> = session
        .messages
        .iter()
        .filter(|msg| {
            msg.content.to_lowercase().contains(&query_lower)
                && (msg.role == "user" || msg.role == "assistant")
        })
        .map(|msg| super::types::MessageSearchResult {
            message_id: msg.id.clone(),
            content: msg.content.clone(),
            created_at: msg.created_at,
            role: msg.role.clone(),
        })
        .collect();

    Ok(results)
}

#[tauri::command]
pub async fn chat_generate_user_reply(
    app: AppHandle,
    session_id: String,
    current_draft: Option<String>,
    request_id: Option<String>,
    swap_places: Option<bool>,
) -> Result<String, String> {
    let swap_places = role_swap_enabled(swap_places);
    log_info(
        &app,
        "help_me_reply",
        format!(
            "Generating user reply for session={}, has_draft={}, swap_places={}",
            &session_id,
            current_draft.is_some(),
            swap_places
        ),
    );
    let context = ChatContext::initialize(app.clone())?;
    let settings = &context.settings;

    // Check if help me reply is enabled
    if let Some(advanced) = &settings.advanced_settings {
        if advanced.help_me_reply_enabled == Some(false) {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Help Me Reply is disabled in settings",
            ));
        }
    }

    let session = match context.load_session(&session_id)? {
        Some(s) => s,
        None => {
            return Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Session not found",
            ))
        }
    };

    let character = context.find_character(&session.character_id)?;
    let persona = context.choose_persona(resolve_persona_id(&session, None));
    let (prompt_character, prompt_persona) = if swap_places {
        swapped_prompt_entities(&character, persona)
    } else {
        (character.clone(), persona.cloned())
    };

    let recent_msgs = recent_messages(&session, 10);

    if recent_msgs.is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "No conversation history to base reply on",
        ));
    }

    // Use help me reply model if configured, otherwise fall back to default
    let model_id = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_model_id.as_ref())
        .or(settings.default_model_id.as_ref())
        .ok_or_else(|| "No model configured for Help Me Reply".to_string())?;

    let model = settings
        .models
        .iter()
        .find(|m| &m.id == model_id)
        .ok_or_else(|| "Help Me Reply model not found".to_string())?;

    let provider_cred = resolve_provider_credential_for_model(settings, model)
        .ok_or_else(|| "Provider credential not found".to_string())?;

    let api_key = resolve_api_key(&app, provider_cred, "help_me_reply")?;

    // Get reply style from settings (default to roleplay)
    let reply_style = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_style.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("roleplay");

    let base_prompt = prompts::get_help_me_reply_prompt(&app, reply_style);

    // Get max tokens from settings (default to 150)
    let max_tokens = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_max_tokens)
        .unwrap_or(150) as u32;

    // Get streaming setting (default to true)
    let streaming_enabled = settings
        .advanced_settings
        .as_ref()
        .and_then(|advanced| advanced.help_me_reply_streaming)
        .unwrap_or(true);

    let char_name = &prompt_character.name;
    let char_desc = prompt_character
        .definition
        .as_deref()
        .or(prompt_character.description.as_deref())
        .unwrap_or("");
    let persona_name = prompt_persona
        .as_ref()
        .map(|p| p.title.as_str())
        .unwrap_or("User");
    let persona_desc = prompt_persona
        .as_ref()
        .map(|p| p.description.as_str())
        .unwrap_or("");

    let mut system_prompt = base_prompt;
    system_prompt = system_prompt.replace("{{char.name}}", char_name);
    system_prompt = system_prompt.replace("{{char.desc}}", char_desc);
    system_prompt = system_prompt.replace("{{persona.name}}", persona_name);
    system_prompt = system_prompt.replace("{{persona.desc}}", persona_desc);
    system_prompt = system_prompt.replace("{{user.name}}", persona_name);
    system_prompt = system_prompt.replace("{{user.desc}}", persona_desc);
    let draft_str = current_draft.as_deref().unwrap_or("");
    system_prompt = system_prompt.replace("{{current_draft}}", draft_str);
    // Legacy placeholders
    system_prompt = system_prompt.replace("{{char}}", char_name);
    system_prompt = system_prompt.replace("{{persona}}", persona_name);
    system_prompt = system_prompt.replace("{{user}}", persona_name);

    if let Some(ref draft) = current_draft {
        if !draft.trim().is_empty() {
            system_prompt = system_prompt.replace("{{#if current_draft}}", "");
            system_prompt = system_prompt.replace("{{current_draft}}", draft);
            if let Some(else_start) = system_prompt.find("{{else}}") {
                if let Some(endif_start) = system_prompt[else_start..].find("{{/if}}") {
                    system_prompt.replace_range(else_start..(else_start + endif_start + 7), "");
                }
            }
            system_prompt = system_prompt.replace("{{/if}}", "");
        } else {
            remove_if_block(&mut system_prompt);
        }
    } else {
        remove_if_block(&mut system_prompt);
    }

    let (effective_user_name, effective_assistant_name) =
        help_me_reply_participant_names(&prompt_character, prompt_persona.as_ref());

    let conversation_context = recent_msgs
        .iter()
        .map(|msg| {
            let effective_role = if swap_places {
                swap_role_for_api(msg.role.as_str())
            } else {
                msg.role.as_str()
            };
            let role_label = if effective_role == "user" {
                effective_user_name
            } else {
                effective_assistant_name
            };
            format!("{}: {}", role_label, msg.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let user_prompt = format!(
        "Here is the recent conversation:\n\n{}\n\nGenerate a reply for {} to say next.",
        conversation_context, effective_user_name
    );

    let messages_for_api: Vec<Value> = vec![
        json!({ "role": "system", "content": system_prompt }),
        json!({ "role": "user", "content": user_prompt }),
    ];

    let (request_settings, extra_body_fields) = prepare_sampling_request(
        &provider_cred.provider_id,
        &session,
        model,
        settings,
        max_tokens,
        0.8,
        1.0,
        None,
        None,
        None,
    );
    let built = super::request_builder::build_chat_request(
        provider_cred,
        &api_key,
        &model.name,
        &messages_for_api,
        None,
        request_settings.temperature,
        request_settings.top_p,
        request_settings.max_tokens,
        request_settings.context_length,
        streaming_enabled,
        request_id.clone(),
        request_settings.frequency_penalty,
        request_settings.presence_penalty,
        request_settings.top_k,
        None,
        request_settings.reasoning_enabled,
        request_settings.reasoning_effort.clone(),
        request_settings.reasoning_budget,
        extra_body_fields,
    );

    log_info(
        &app,
        "help_me_reply",
        format!("Sending request to {}", built.url),
    );

    let api_request_payload = ApiRequest {
        url: built.url,
        method: Some("POST".into()),
        headers: Some(built.headers),
        query: None,
        body: Some(built.body),
        timeout_ms: Some(60_000),
        stream: Some(streaming_enabled),
        request_id: request_id.clone(),
        provider_id: Some(provider_cred.provider_id.clone()),
    };

    let api_response = api_request(app.clone(), api_request_payload).await?;

    if !api_response.ok {
        return Err(format!(
            "API request failed with status {}",
            api_response.status
        ));
    }

    let generated_text = extract_text(&api_response.data, Some(&provider_cred.provider_id))
        .ok_or_else(|| "Failed to extract text from response".to_string())?;

    let cleaned = generated_text
        .trim()
        .trim_matches('"')
        .trim_start_matches(&format!("{}:", effective_user_name))
        .trim()
        .to_string();

    log_info(
        &app,
        "help_me_reply",
        format!("Generated reply: {} chars", cleaned.len()),
    );

    let usage = super::sse::usage_from_value(&api_response.data);
    super::service::record_usage_if_available(
        &context,
        &usage,
        &session,
        &prompt_character,
        &model,
        &provider_cred,
        &api_key,
        now_millis().unwrap_or(0),
        UsageOperationType::ReplyHelper,
        "help_me_reply",
    )
    .await;

    Ok(cleaned)
}

/// Helper to remove {{#if current_draft}}...{{else}}...{{/if}} and keep else content
fn remove_if_block(prompt: &mut String) {
    if let Some(if_start) = prompt.find("{{#if current_draft}}") {
        if let Some(else_pos) = prompt.find("{{else}}") {
            prompt.replace_range(if_start..(else_pos + 8), "");
        }
    }
    *prompt = prompt.replace("{{/if}}", "");
}

#[cfg(test)]
mod tests {
    use super::{help_me_reply_participant_names, swapped_prompt_entities};
    use crate::chat_manager::types::{Character, Persona};

    fn make_character() -> Character {
        Character {
            id: "char-1".to_string(),
            name: "Astra".to_string(),
            avatar_path: None,
            design_description: None,
            design_reference_image_ids: Vec::new(),
            background_image_path: None,
            definition: Some("A starship captain".to_string()),
            description: Some("Commanding and curious".to_string()),
            rules: Vec::new(),
            scenes: Vec::new(),
            default_scene_id: None,
            default_model_id: None,
            fallback_model_id: None,
            memory_type: "manual".to_string(),
            prompt_template_id: None,
            system_prompt: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn make_persona() -> Persona {
        Persona {
            id: "persona-1".to_string(),
            title: "Milo".to_string(),
            description: "A reckless smuggler".to_string(),
            nickname: None,
            avatar_path: None,
            design_description: None,
            design_reference_image_ids: Vec::new(),
            is_default: false,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn help_me_reply_names_match_unswapped_prompt_entities() {
        let character = make_character();
        let persona = make_persona();

        let (effective_user_name, effective_assistant_name) =
            help_me_reply_participant_names(&character, Some(&persona));

        assert_eq!(effective_user_name, "Milo");
        assert_eq!(effective_assistant_name, "Astra");
    }

    #[test]
    fn help_me_reply_names_follow_swapped_prompt_entities() {
        let character = make_character();
        let persona = make_persona();
        let (prompt_character, prompt_persona) =
            swapped_prompt_entities(&character, Some(&persona));

        let (effective_user_name, effective_assistant_name) =
            help_me_reply_participant_names(&prompt_character, prompt_persona.as_ref());

        assert_eq!(effective_user_name, "Astra");
        assert_eq!(effective_assistant_name, "Milo");
    }
}
