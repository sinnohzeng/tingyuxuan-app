use crate::state::AppState;
use std::sync::Arc;
use tauri::State;
use tingyuxuan_core::config::AppConfig;
use tingyuxuan_core::history::TranscriptRecord;
use tokio::sync::Mutex;

type AppStateHandle = Arc<Mutex<AppState>>;

#[tauri::command]
pub async fn start_recording(
    mode: String,
    state: State<'_, AppStateHandle>,
) -> Result<String, String> {
    tracing::info!("Starting recording in mode: {}", mode);
    // TODO: Implement with Pipeline in Step 6
    Ok(format!("Recording started in {} mode", mode))
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppStateHandle>) -> Result<String, String> {
    tracing::info!("Stopping recording");
    // TODO: Implement with Pipeline in Step 6
    Ok("Recording stopped".to_string())
}

#[tauri::command]
pub async fn cancel_recording(state: State<'_, AppStateHandle>) -> Result<(), String> {
    tracing::info!("Cancelling recording");
    // TODO: Implement with Pipeline in Step 6
    Ok(())
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppStateHandle>) -> Result<AppConfig, String> {
    let state = state.lock().await;
    Ok(state.config.clone())
}

#[tauri::command]
pub async fn save_config(
    config: AppConfig,
    state: State<'_, AppStateHandle>,
) -> Result<(), String> {
    let mut state = state.lock().await;
    state.config = config.clone();
    config.save().map_err(|e| e.to_string())?;
    tracing::info!("Configuration saved");
    Ok(())
}

#[tauri::command]
pub async fn test_stt_connection(state: State<'_, AppStateHandle>) -> Result<bool, String> {
    // TODO: Implement with STT providers in Step 4
    Ok(false)
}

#[tauri::command]
pub async fn test_llm_connection(state: State<'_, AppStateHandle>) -> Result<bool, String> {
    // TODO: Implement with LLM providers in Step 5
    Ok(false)
}

#[tauri::command]
pub async fn get_recent_history(
    limit: u32,
    state: State<'_, AppStateHandle>,
) -> Result<Vec<TranscriptRecord>, String> {
    let state = state.lock().await;
    state
        .history
        .get_recent(limit)
        .map_err(|e| e.to_string())
}
