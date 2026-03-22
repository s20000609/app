use serde_json::{json, Map, Value};
use std::collections::HashMap;

use tauri::AppHandle;

use crate::chat_manager::storage::resolve_provider_credential_for_model;
use crate::utils::{emit_toast, log_info, log_warn};

use super::types::{Character, Model, ProviderCredential, Session, Settings};

const FALLBACK_TEMPERATURE: f64 = 0.7;
const FALLBACK_TOP_P: f64 = 1.0;
const FALLBACK_MAX_OUTPUT_TOKENS: u32 = 4096;
const DEFAULT_LLAMA_SAMPLER_PROFILE: &str = "balanced";

#[derive(Clone, Copy)]
struct LlamaSamplerProfileDefaults {
    name: &'static str,
    temperature: f64,
    top_p: f64,
    top_k: Option<u32>,
    min_p: Option<f64>,
    typical_p: Option<f64>,
    frequency_penalty: Option<f64>,
    presence_penalty: Option<f64>,
}

fn is_llama_cpp_model(model: &Model) -> bool {
    model.provider_id.eq_ignore_ascii_case("llamacpp")
}

fn normalize_llama_sampler_profile(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "balanced" | "creative" | "stable" | "reasoning" => Some(normalized),
        _ => None,
    }
}

fn llama_sampler_profile_defaults(profile: Option<&str>) -> LlamaSamplerProfileDefaults {
    match profile.unwrap_or(DEFAULT_LLAMA_SAMPLER_PROFILE) {
        "creative" => LlamaSamplerProfileDefaults {
            name: "creative",
            temperature: 0.95,
            top_p: 0.98,
            top_k: Some(80),
            min_p: Some(0.02),
            typical_p: None,
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.25),
        },
        "stable" => LlamaSamplerProfileDefaults {
            name: "stable",
            temperature: 0.55,
            top_p: 0.90,
            top_k: Some(32),
            min_p: Some(0.08),
            typical_p: Some(0.97),
            frequency_penalty: Some(0.2),
            presence_penalty: Some(0.0),
        },
        "reasoning" => LlamaSamplerProfileDefaults {
            name: "reasoning",
            temperature: 0.35,
            top_p: 0.90,
            top_k: Some(24),
            min_p: None,
            typical_p: Some(0.95),
            frequency_penalty: Some(0.1),
            presence_penalty: Some(0.0),
        },
        _ => LlamaSamplerProfileDefaults {
            name: "balanced",
            temperature: 0.8,
            top_p: 0.95,
            top_k: Some(40),
            min_p: Some(0.05),
            typical_p: None,
            frequency_penalty: Some(0.15),
            presence_penalty: Some(0.0),
        },
    }
}

fn resolve_llama_sampler_profile(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_sampler_profile.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_sampler_profile.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_sampler_profile
                .clone()
        })
        .and_then(|value| normalize_llama_sampler_profile(&value))
}

fn resolve_temperature(session: &Session, model: &Model, settings: &Settings) -> f64 {
    let configured = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.temperature)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.temperature)
        })
        .or(settings.advanced_model_settings.temperature);
    if let Some(value) = configured {
        return value;
    }
    if is_llama_cpp_model(model) {
        return llama_sampler_profile_defaults(
            resolve_llama_sampler_profile(session, model, settings).as_deref(),
        )
        .temperature;
    }
    FALLBACK_TEMPERATURE
}

fn resolve_top_p(session: &Session, model: &Model, settings: &Settings) -> f64 {
    let configured = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.top_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.top_p)
        })
        .or(settings.advanced_model_settings.top_p);
    if let Some(value) = configured {
        return value;
    }
    if is_llama_cpp_model(model) {
        return llama_sampler_profile_defaults(
            resolve_llama_sampler_profile(session, model, settings).as_deref(),
        )
        .top_p;
    }
    FALLBACK_TOP_P
}

fn resolve_max_tokens(session: &Session, model: &Model, settings: &Settings) -> u32 {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.max_output_tokens)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.max_output_tokens)
        })
        .or(settings.advanced_model_settings.max_output_tokens)
        .unwrap_or(FALLBACK_MAX_OUTPUT_TOKENS)
}

fn resolve_context_length(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.context_length)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.context_length)
        })
        .or(settings.advanced_model_settings.context_length)
        .filter(|v| *v > 0)
}

fn resolve_frequency_penalty(session: &Session, model: &Model, settings: &Settings) -> Option<f64> {
    let configured = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.frequency_penalty)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.frequency_penalty)
        });
    if configured.is_some() {
        return configured;
    }
    if is_llama_cpp_model(model) {
        return llama_sampler_profile_defaults(
            resolve_llama_sampler_profile(session, model, settings).as_deref(),
        )
        .frequency_penalty;
    }
    None
}

fn resolve_presence_penalty(session: &Session, model: &Model, settings: &Settings) -> Option<f64> {
    let configured = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.presence_penalty)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.presence_penalty)
        });
    if configured.is_some() {
        return configured;
    }
    if is_llama_cpp_model(model) {
        return llama_sampler_profile_defaults(
            resolve_llama_sampler_profile(session, model, settings).as_deref(),
        )
        .presence_penalty;
    }
    None
}

fn resolve_top_k(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    let configured = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.top_k)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.top_k)
        });
    if configured.is_some() {
        return configured;
    }
    if is_llama_cpp_model(model) {
        return llama_sampler_profile_defaults(
            resolve_llama_sampler_profile(session, model, settings).as_deref(),
        )
        .top_k;
    }
    None
}

fn resolve_llama_gpu_layers(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_gpu_layers)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_gpu_layers)
        })
        .or(settings.advanced_model_settings.llama_gpu_layers)
}

fn resolve_llama_threads(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_threads)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_threads)
        })
        .or(settings.advanced_model_settings.llama_threads)
}

fn resolve_llama_threads_batch(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_threads_batch)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_threads_batch)
        })
        .or(settings.advanced_model_settings.llama_threads_batch)
}

fn resolve_llama_seed(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_seed)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_seed)
        })
        .or(settings.advanced_model_settings.llama_seed)
}

fn resolve_llama_rope_freq_base(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_rope_freq_base)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_rope_freq_base)
        })
        .or(settings.advanced_model_settings.llama_rope_freq_base)
}

fn resolve_llama_rope_freq_scale(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_rope_freq_scale)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_rope_freq_scale)
        })
        .or(settings.advanced_model_settings.llama_rope_freq_scale)
}

fn resolve_llama_offload_kqv(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<bool> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_offload_kqv)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_offload_kqv)
        })
        .or(settings.advanced_model_settings.llama_offload_kqv)
}

fn resolve_llama_batch_size(session: &Session, model: &Model, settings: &Settings) -> Option<u32> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_batch_size)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_batch_size)
        })
        .or(settings.advanced_model_settings.llama_batch_size)
        .filter(|v| *v > 0)
}

fn resolve_llama_kv_type(session: &Session, model: &Model, settings: &Settings) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_kv_type.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_kv_type.clone())
        })
        .or_else(|| settings.advanced_model_settings.llama_kv_type.clone())
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
}

fn resolve_llama_flash_attention(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_flash_attention.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_flash_attention.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_flash_attention
                .clone()
        })
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
}

fn resolve_llama_chat_template_override(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_chat_template_override.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_chat_template_override.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_chat_template_override
                .clone()
        })
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn resolve_llama_chat_template_preset(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_chat_template_preset.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_chat_template_preset.clone())
        })
        .or_else(|| {
            settings
                .advanced_model_settings
                .llama_chat_template_preset
                .clone()
        })
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn resolve_llama_raw_completion_fallback(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<bool> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_raw_completion_fallback)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_raw_completion_fallback)
        })
        .or(settings
            .advanced_model_settings
            .llama_raw_completion_fallback)
}

fn resolve_llama_profile_min_p(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    if let Some(value) = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_min_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_min_p)
        })
        .or(settings.advanced_model_settings.llama_min_p)
    {
        return Some(value);
    }
    if !is_llama_cpp_model(model) {
        return None;
    }
    llama_sampler_profile_defaults(
        resolve_llama_sampler_profile(session, model, settings).as_deref(),
    )
    .min_p
}

fn resolve_llama_profile_typical_p(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<f64> {
    if let Some(value) = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.llama_typical_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.llama_typical_p)
        })
        .or(settings.advanced_model_settings.llama_typical_p)
    {
        return Some(value);
    }
    if !is_llama_cpp_model(model) {
        return None;
    }
    llama_sampler_profile_defaults(
        resolve_llama_sampler_profile(session, model, settings).as_deref(),
    )
    .typical_p
}

fn build_llama_extra_fields(
    session: &Session,
    model: &Model,
    settings: &Settings,
) -> Option<HashMap<String, Value>> {
    let mut extra = HashMap::new();
    let sampler_profile = if is_llama_cpp_model(model) {
        Some(
            llama_sampler_profile_defaults(
                resolve_llama_sampler_profile(session, model, settings).as_deref(),
            )
            .name
            .to_string(),
        )
    } else {
        None
    };
    if let Some(v) = resolve_llama_gpu_layers(session, model, settings) {
        extra.insert("llamaGpuLayers".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_threads(session, model, settings) {
        extra.insert("llamaThreads".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_threads_batch(session, model, settings) {
        extra.insert("llamaThreadsBatch".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_seed(session, model, settings) {
        extra.insert("llamaSeed".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_rope_freq_base(session, model, settings) {
        extra.insert("llamaRopeFreqBase".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_rope_freq_scale(session, model, settings) {
        extra.insert("llamaRopeFreqScale".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_offload_kqv(session, model, settings) {
        extra.insert("llamaOffloadKqv".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_batch_size(session, model, settings) {
        extra.insert("llamaBatchSize".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_kv_type(session, model, settings) {
        extra.insert("llamaKvType".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_flash_attention(session, model, settings) {
        extra.insert("llamaFlashAttentionPolicy".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_chat_template_override(session, model, settings) {
        extra.insert("llamaChatTemplateOverride".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_chat_template_preset(session, model, settings) {
        extra.insert("llamaChatTemplatePreset".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_raw_completion_fallback(session, model, settings) {
        extra.insert("llamaRawCompletionFallback".to_string(), json!(v));
    }
    if let Some(v) = sampler_profile {
        extra.insert("llamaSamplerProfile".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_profile_min_p(session, model, settings) {
        extra.insert("llamaMinP".to_string(), json!(v));
    }
    if let Some(v) = resolve_llama_profile_typical_p(session, model, settings) {
        extra.insert("llamaTypicalP".to_string(), json!(v));
    }

    if extra.is_empty() {
        None
    } else {
        Some(extra)
    }
}

fn build_ollama_extra_fields(
    session: &Session,
    model: &Model,
    settings: &Settings,
    request_settings: &RequestSettings,
) -> Option<HashMap<String, Value>> {
    let mut options = Map::new();

    let num_ctx = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_ctx)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_ctx)
        })
        .or(settings.advanced_model_settings.ollama_num_ctx)
        .or(request_settings.context_length);
    let num_predict = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_predict)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_predict)
        })
        .or(settings.advanced_model_settings.ollama_num_predict)
        .or(Some(request_settings.max_tokens));
    let num_keep = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_keep)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_keep)
        })
        .or(settings.advanced_model_settings.ollama_num_keep);
    let num_batch = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_batch)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_batch)
        })
        .or(settings.advanced_model_settings.ollama_num_batch);
    let num_gpu = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_gpu)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_gpu)
        })
        .or(settings.advanced_model_settings.ollama_num_gpu);
    let num_thread = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_num_thread)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_num_thread)
        })
        .or(settings.advanced_model_settings.ollama_num_thread);
    let tfs_z = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_tfs_z)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_tfs_z)
        })
        .or(settings.advanced_model_settings.ollama_tfs_z);
    let typical_p = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_typical_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_typical_p)
        })
        .or(settings.advanced_model_settings.ollama_typical_p);
    let min_p = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_min_p)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_min_p)
        })
        .or(settings.advanced_model_settings.ollama_min_p);
    let mirostat = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_mirostat)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_mirostat)
        })
        .or(settings.advanced_model_settings.ollama_mirostat);
    let mirostat_tau = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_mirostat_tau)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_mirostat_tau)
        })
        .or(settings.advanced_model_settings.ollama_mirostat_tau);
    let mirostat_eta = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_mirostat_eta)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_mirostat_eta)
        })
        .or(settings.advanced_model_settings.ollama_mirostat_eta);
    let repeat_penalty = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_repeat_penalty)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_repeat_penalty)
        })
        .or(settings.advanced_model_settings.ollama_repeat_penalty);
    let seed = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_seed)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_seed)
        })
        .or(settings.advanced_model_settings.ollama_seed);
    let stop = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.ollama_stop.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.ollama_stop.clone())
        })
        .or(settings.advanced_model_settings.ollama_stop.clone());

    options.insert("temperature".into(), json!(request_settings.temperature));
    options.insert("top_p".into(), json!(request_settings.top_p));
    if let Some(v) = request_settings.top_k {
        options.insert("top_k".into(), json!(v));
    }
    if let Some(v) = request_settings.frequency_penalty {
        options.insert("frequency_penalty".into(), json!(v));
    }
    if let Some(v) = request_settings.presence_penalty {
        options.insert("presence_penalty".into(), json!(v));
    }
    if let Some(v) = num_ctx {
        options.insert("num_ctx".into(), json!(v));
    }
    if let Some(v) = num_predict {
        options.insert("num_predict".into(), json!(v));
    }
    if let Some(v) = num_keep {
        options.insert("num_keep".into(), json!(v));
    }
    if let Some(v) = num_batch {
        options.insert("num_batch".into(), json!(v));
    }
    if let Some(v) = num_gpu {
        options.insert("num_gpu".into(), json!(v));
    }
    if let Some(v) = num_thread {
        options.insert("num_thread".into(), json!(v));
    }
    if let Some(v) = tfs_z {
        options.insert("tfs_z".into(), json!(v));
    }
    if let Some(v) = typical_p {
        options.insert("typical_p".into(), json!(v));
    }
    if let Some(v) = min_p {
        options.insert("min_p".into(), json!(v));
    }
    if let Some(v) = mirostat {
        options.insert("mirostat".into(), json!(v));
    }
    if let Some(v) = mirostat_tau {
        options.insert("mirostat_tau".into(), json!(v));
    }
    if let Some(v) = mirostat_eta {
        options.insert("mirostat_eta".into(), json!(v));
    }
    if let Some(v) = repeat_penalty {
        options.insert("repeat_penalty".into(), json!(v));
    }
    if let Some(v) = seed {
        options.insert("seed".into(), json!(v));
    }
    if let Some(v) = stop {
        options.insert("stop".into(), json!(v));
    }

    let mut extra = HashMap::new();
    extra.insert("options".to_string(), Value::Object(options));
    Some(extra)
}

fn resolve_reasoning_enabled(session: &Session, model: &Model, _settings: &Settings) -> bool {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.reasoning_enabled)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.reasoning_enabled)
        })
        .unwrap_or(false)
}

fn resolve_reasoning_effort(
    session: &Session,
    model: &Model,
    _settings: &Settings,
) -> Option<String> {
    session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.reasoning_effort.clone())
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.reasoning_effort.clone())
        })
}

fn resolve_reasoning_budget(
    session: &Session,
    model: &Model,
    _settings: &Settings,
    reasoning_effort: Option<&str>,
) -> Option<u32> {
    let explicit_budget = session
        .advanced_model_settings
        .as_ref()
        .and_then(|cfg| cfg.reasoning_budget_tokens)
        .or_else(|| {
            model
                .advanced_model_settings
                .as_ref()
                .and_then(|cfg| cfg.reasoning_budget_tokens)
        });

    if explicit_budget.is_some() {
        return explicit_budget;
    }

    reasoning_effort.map(|effort| match effort {
        "low" => 2048,
        "medium" => 8192,
        "high" => 16384,
        _ => 4096,
    })
}

#[derive(Clone, Debug)]
pub(crate) struct RequestSettings {
    pub(crate) temperature: f64,
    pub(crate) top_p: f64,
    pub(crate) max_tokens: u32,
    pub(crate) context_length: Option<u32>,
    pub(crate) frequency_penalty: Option<f64>,
    pub(crate) presence_penalty: Option<f64>,
    pub(crate) top_k: Option<u32>,
    pub(crate) reasoning_enabled: bool,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) reasoning_budget: Option<u32>,
}

impl RequestSettings {
    pub(crate) fn resolve(session: &Session, model: &Model, settings: &Settings) -> Self {
        let reasoning_effort = resolve_reasoning_effort(session, model, settings);
        Self {
            temperature: resolve_temperature(session, model, settings),
            top_p: resolve_top_p(session, model, settings),
            max_tokens: resolve_max_tokens(session, model, settings),
            context_length: resolve_context_length(session, model, settings),
            frequency_penalty: resolve_frequency_penalty(session, model, settings),
            presence_penalty: resolve_presence_penalty(session, model, settings),
            top_k: resolve_top_k(session, model, settings),
            reasoning_enabled: resolve_reasoning_enabled(session, model, settings),
            reasoning_budget: resolve_reasoning_budget(
                session,
                model,
                settings,
                reasoning_effort.as_deref(),
            ),
            reasoning_effort,
        }
    }

    pub(crate) fn for_sampling(
        context_length: Option<u32>,
        max_tokens: u32,
        temperature: f64,
        top_p: f64,
        top_k: Option<u32>,
        frequency_penalty: Option<f64>,
        presence_penalty: Option<f64>,
    ) -> Self {
        Self {
            temperature,
            top_p,
            max_tokens,
            context_length,
            frequency_penalty,
            presence_penalty,
            top_k,
            reasoning_enabled: false,
            reasoning_effort: None,
            reasoning_budget: None,
        }
    }
}

pub(crate) fn build_provider_extra_fields(
    provider_id: &str,
    session: &Session,
    model: &Model,
    settings: &Settings,
    request_settings: &RequestSettings,
) -> Option<HashMap<String, Value>> {
    if provider_id == "llamacpp" {
        build_llama_extra_fields(session, model, settings)
    } else if provider_id == "ollama" {
        build_ollama_extra_fields(session, model, settings, request_settings)
    } else {
        None
    }
}

pub(crate) fn prepare_sampling_request(
    provider_id: &str,
    session: &Session,
    model: &Model,
    settings: &Settings,
    max_tokens: u32,
    temperature: f64,
    top_p: f64,
    top_k: Option<u32>,
    frequency_penalty: Option<f64>,
    presence_penalty: Option<f64>,
) -> (RequestSettings, Option<HashMap<String, Value>>) {
    let model_request_settings = RequestSettings::resolve(session, model, settings);
    let request_settings = RequestSettings::for_sampling(
        model_request_settings.context_length,
        max_tokens,
        temperature,
        top_p,
        top_k,
        frequency_penalty,
        presence_penalty,
    );
    let extra_body_fields =
        build_provider_extra_fields(provider_id, session, model, settings, &request_settings);

    (request_settings, extra_body_fields)
}

pub(crate) fn prepare_default_sampling_request(
    provider_id: &str,
    session: &Session,
    model: &Model,
    settings: &Settings,
    temperature: f64,
    top_p: f64,
    top_k: Option<u32>,
    frequency_penalty: Option<f64>,
    presence_penalty: Option<f64>,
) -> (RequestSettings, Option<HashMap<String, Value>>) {
    let model_request_settings = RequestSettings::resolve(session, model, settings);
    prepare_sampling_request(
        provider_id,
        session,
        model,
        settings,
        model_request_settings.max_tokens,
        temperature,
        top_p,
        top_k,
        frequency_penalty,
        presence_penalty,
    )
}

pub(crate) fn find_model_and_credential<'a>(
    settings: &'a Settings,
    model_id: &str,
) -> Option<(&'a Model, &'a ProviderCredential)> {
    let model = settings.models.iter().find(|m| m.id == model_id)?;
    let provider_cred = resolve_provider_credential_for_model(settings, model)?;
    Some((model, provider_cred))
}

pub(crate) fn build_model_attempts<'a>(
    app: &AppHandle,
    settings: &'a Settings,
    character: &Character,
    primary_model: &'a Model,
    primary_provider_cred: &'a ProviderCredential,
    log_scope: &str,
) -> Vec<(&'a Model, &'a ProviderCredential, bool)> {
    let explicit_fallback_candidate = character
        .fallback_model_id
        .as_ref()
        .filter(|fallback_id| *fallback_id != &primary_model.id)
        .and_then(|fallback_id| find_model_and_credential(settings, fallback_id));

    let app_default_fallback_candidate = settings
        .default_model_id
        .as_ref()
        .filter(|default_id| *default_id != &primary_model.id)
        .and_then(|default_id| find_model_and_credential(settings, default_id));

    let mut attempts: Vec<(&Model, &ProviderCredential, bool)> =
        vec![(primary_model, primary_provider_cred, false)];
    if let Some((fallback_model, fallback_cred)) = explicit_fallback_candidate {
        attempts.push((fallback_model, fallback_cred, true));
    } else if character
        .fallback_model_id
        .as_ref()
        .is_some_and(|id| id != &primary_model.id)
    {
        log_warn(
            app,
            log_scope,
            format!(
                "configured character fallback model id {} could not be resolved",
                character.fallback_model_id.as_deref().unwrap_or("")
            ),
        );
        if let Some((fallback_model, fallback_cred)) = app_default_fallback_candidate {
            log_info(
                app,
                log_scope,
                format!(
                    "using app default model {} as fallback candidate",
                    fallback_model.name
                ),
            );
            attempts.push((fallback_model, fallback_cred, true));
        }
    }

    attempts
}

pub(crate) fn emit_fallback_retry_toast(app: &AppHandle, shown: &mut bool) {
    if *shown {
        return;
    }
    emit_toast(
        app,
        "warning",
        "Primary model failed",
        Some("Retrying with fallback model.".to_string()),
    );
    *shown = true;
}
