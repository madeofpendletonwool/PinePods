//! Global AI configuration (the `AISettings` singleton row) and helpers that resolve it into
//! the per-request specs sent to the stateless `pinepods-ai` sidecar (#790).
//!
//! Admins pick the transcription (whisper) model and the LLM backend used for ad detection —
//! either the bundled local GGUF or a remote OpenAI-compatible endpoint — from the AI Settings
//! page. The remote API key is stored encrypted (Fernet, same as other secrets) and never
//! returned to clients.

use crate::database::DatabasePool;
use crate::services::ai_client::LlmSpec;
use serde::{Deserialize, Serialize};
use sqlx::Row;

/// AI config as shown to (and updated from) the AI Settings page. Never carries the API key —
/// only whether one is set.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AiSettings {
    pub transcription_model: String,
    pub llm_backend: String, // "local" | "remote"
    pub llm_model: Option<String>,
    pub llm_url: Option<String>,
    pub has_api_key: bool,
    pub whisper_device: String,
    pub whisper_compute_type: String,
}

/// Update payload from the settings page. `llm_api_key` is only written when `Some(non-empty)`,
/// so leaving the field blank preserves the stored key; `clear_api_key` removes it.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct AiSettingsUpdate {
    pub transcription_model: String,
    pub llm_backend: String,
    pub llm_model: Option<String>,
    pub llm_url: Option<String>,
    pub llm_api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    pub whisper_device: Option<String>,
    pub whisper_compute_type: Option<String>,
}

/// Raw row incl. the still-encrypted API key (internal only).
struct RawAiSettings {
    transcription_model: String,
    llm_backend: String,
    llm_model: Option<String>,
    llm_url: Option<String>,
    llm_api_key: Option<String>,
    whisper_device: String,
    whisper_compute_type: String,
}

impl Default for RawAiSettings {
    fn default() -> Self {
        Self {
            transcription_model: "base".to_string(),
            llm_backend: "local".to_string(),
            llm_model: None,
            llm_url: None,
            llm_api_key: None,
            whisper_device: "cpu".to_string(),
            whisper_compute_type: "int8".to_string(),
        }
    }
}

async fn read_row(db_pool: &DatabasePool) -> Result<RawAiSettings, String> {
    // The two SELECTs differ only by table-name quoting; mapping happens per-arm because the
    // Postgres and MySQL row types are distinct.
    match db_pool {
        DatabasePool::Postgres(pool) => {
            let row = sqlx::query(r#"
                SELECT TranscriptionModel AS transcription_model, LlmBackend AS llm_backend,
                       LlmModel AS llm_model, LlmUrl AS llm_url, LlmApiKey AS llm_api_key,
                       WhisperDevice AS whisper_device, WhisperComputeType AS whisper_compute_type
                FROM "AISettings" WHERE AISettingsID = 1
            "#)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
            let Some(r) = row else { return Ok(RawAiSettings::default()) };
            Ok(RawAiSettings {
                transcription_model: r.try_get("transcription_model").unwrap_or_else(|_| "base".into()),
                llm_backend: r.try_get("llm_backend").unwrap_or_else(|_| "local".into()),
                llm_model: r.try_get("llm_model").ok().flatten(),
                llm_url: r.try_get("llm_url").ok().flatten(),
                llm_api_key: r.try_get("llm_api_key").ok().flatten(),
                whisper_device: r.try_get("whisper_device").unwrap_or_else(|_| "cpu".into()),
                whisper_compute_type: r.try_get("whisper_compute_type").unwrap_or_else(|_| "int8".into()),
            })
        }
        DatabasePool::MySQL(pool) => {
            let row = sqlx::query(r#"
                SELECT TranscriptionModel AS transcription_model, LlmBackend AS llm_backend,
                       LlmModel AS llm_model, LlmUrl AS llm_url, LlmApiKey AS llm_api_key,
                       WhisperDevice AS whisper_device, WhisperComputeType AS whisper_compute_type
                FROM AISettings WHERE AISettingsID = 1
            "#)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
            let Some(r) = row else { return Ok(RawAiSettings::default()) };
            Ok(RawAiSettings {
                transcription_model: r.try_get("transcription_model").unwrap_or_else(|_| "base".into()),
                llm_backend: r.try_get("llm_backend").unwrap_or_else(|_| "local".into()),
                llm_model: r.try_get("llm_model").ok().flatten(),
                llm_url: r.try_get("llm_url").ok().flatten(),
                llm_api_key: r.try_get("llm_api_key").ok().flatten(),
                whisper_device: r.try_get("whisper_device").unwrap_or_else(|_| "cpu".into()),
                whisper_compute_type: r.try_get("whisper_compute_type").unwrap_or_else(|_| "int8".into()),
            })
        }
    }
}

/// AI config for the settings page (API key redacted).
pub async fn get_ai_settings(db_pool: &DatabasePool) -> Result<AiSettings, String> {
    let raw = read_row(db_pool).await?;
    Ok(AiSettings {
        transcription_model: raw.transcription_model,
        llm_backend: raw.llm_backend,
        llm_model: raw.llm_model,
        llm_url: raw.llm_url,
        has_api_key: raw.llm_api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
        whisper_device: raw.whisper_device,
        whisper_compute_type: raw.whisper_compute_type,
    })
}

/// The whisper model to transcribe with (admin-configured, default `base`).
pub async fn transcription_model(db_pool: &DatabasePool) -> String {
    read_row(db_pool).await.map(|r| r.transcription_model).unwrap_or_else(|_| "base".to_string())
}

/// Resolve the configured LLM backend into a per-request `LlmSpec`, decrypting the API key.
pub async fn resolve_llm_spec(db_pool: &DatabasePool) -> Result<LlmSpec, String> {
    let raw = read_row(db_pool).await?;
    let api_key = match raw.llm_api_key {
        Some(enc) if !enc.is_empty() => db_pool.decrypt_password(&enc).await.ok(),
        _ => None,
    };
    Ok(LlmSpec {
        backend: raw.llm_backend,
        model: raw.llm_model,
        url: raw.llm_url,
        api_key,
    })
}

/// Persist AI settings from the settings page. Encrypts the API key when a new one is supplied.
pub async fn set_ai_settings(db_pool: &DatabasePool, update: AiSettingsUpdate) -> Result<(), String> {
    // Decide the API-key value to store: new (encrypted) / cleared / unchanged.
    let new_key: Option<Option<String>> = if update.clear_api_key {
        Some(None)
    } else if let Some(k) = update.llm_api_key.as_deref().filter(|k| !k.is_empty()) {
        Some(Some(db_pool.encrypt_password(k).await.map_err(|e| e.to_string())?))
    } else {
        None // leave existing key untouched
    };

    let device = update.whisper_device.unwrap_or_else(|| "cpu".to_string());
    let compute = update.whisper_compute_type.unwrap_or_else(|| "int8".to_string());

    match db_pool {
        DatabasePool::Postgres(pool) => {
            match &new_key {
                Some(key) => {
                    sqlx::query(r#"
                        UPDATE "AISettings" SET TranscriptionModel=$1, LlmBackend=$2, LlmModel=$3,
                            LlmUrl=$4, LlmApiKey=$5, WhisperDevice=$6, WhisperComputeType=$7,
                            UpdatedAt=CURRENT_TIMESTAMP WHERE AISettingsID = 1
                    "#)
                    .bind(&update.transcription_model).bind(&update.llm_backend).bind(&update.llm_model)
                    .bind(&update.llm_url).bind(key).bind(&device).bind(&compute)
                    .execute(pool).await.map_err(|e| e.to_string())?;
                }
                None => {
                    sqlx::query(r#"
                        UPDATE "AISettings" SET TranscriptionModel=$1, LlmBackend=$2, LlmModel=$3,
                            LlmUrl=$4, WhisperDevice=$5, WhisperComputeType=$6,
                            UpdatedAt=CURRENT_TIMESTAMP WHERE AISettingsID = 1
                    "#)
                    .bind(&update.transcription_model).bind(&update.llm_backend).bind(&update.llm_model)
                    .bind(&update.llm_url).bind(&device).bind(&compute)
                    .execute(pool).await.map_err(|e| e.to_string())?;
                }
            }
        }
        DatabasePool::MySQL(pool) => {
            match &new_key {
                Some(key) => {
                    sqlx::query(r#"
                        UPDATE AISettings SET TranscriptionModel=?, LlmBackend=?, LlmModel=?,
                            LlmUrl=?, LlmApiKey=?, WhisperDevice=?, WhisperComputeType=?,
                            UpdatedAt=CURRENT_TIMESTAMP WHERE AISettingsID = 1
                    "#)
                    .bind(&update.transcription_model).bind(&update.llm_backend).bind(&update.llm_model)
                    .bind(&update.llm_url).bind(key).bind(&device).bind(&compute)
                    .execute(pool).await.map_err(|e| e.to_string())?;
                }
                None => {
                    sqlx::query(r#"
                        UPDATE AISettings SET TranscriptionModel=?, LlmBackend=?, LlmModel=?,
                            LlmUrl=?, WhisperDevice=?, WhisperComputeType=?,
                            UpdatedAt=CURRENT_TIMESTAMP WHERE AISettingsID = 1
                    "#)
                    .bind(&update.transcription_model).bind(&update.llm_backend).bind(&update.llm_model)
                    .bind(&update.llm_url).bind(&device).bind(&compute)
                    .execute(pool).await.map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}
