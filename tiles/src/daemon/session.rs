//! High level apis for sessions

use std::sync::Arc;

use axum::{Router, extract::State, response::IntoResponse, routing::post};
use serde::Serialize;

use crate::core::agent::pi::{self};

#[derive(Serialize)]
struct Session {
    id: String,
}
use crate::{
    daemon::{ApiResponse, AppError, AppState, agent::get_agent_start_params},
    utils::config::PY_PORT,
};

pub fn session_router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/tilekit/session/new", post(create_session))
}

// Creates a new session or starts the agent
async fn create_session(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, AppError> {
    let mut agent = state.agent.lock().await;
    if agent.is_some() {
        log::info!("Pi session already there, create a new session..");
        // agent already there, so lets create a new session
        let agent = agent.as_mut().ok_or(AppError::InternalServerError(
            "Failed to get a mutable agent instance".to_string(),
        ))?;

        let agent_state = agent
            .reader
            .create_new_session(&mut agent.writer)
            .await
            .map_err(|e| AppError::CannotProcess(e.to_string()))?;

        let session_data = Session {
            id: agent_state.session_id,
        };
        Ok(ApiResponse::success(session_data))
    } else {
        log::info!("Pi agent not started, so starting..");
        let (modelname, system_prompt) = get_agent_start_params()?;

        let pi_agent = pi::new(&modelname, &system_prompt, PY_PORT)
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        *agent = Some(pi_agent);
        let agent = agent.as_mut().ok_or(AppError::InternalServerError(
            "No agent instance available, start one first".to_string(),
        ))?;

        let state = agent
            .reader
            .get_pi_state(&mut agent.writer)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        Ok(ApiResponse::success(Session {
            id: state.session_id,
        }))
    }
}
