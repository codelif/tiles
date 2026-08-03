//! APIs for communication with Agent harness (Pi)

use std::sync::Arc;

use axum::{Router, extract::State, routing::get};
use axum_macros::debug_handler;
use reqwest::StatusCode;

use crate::{
    core::agent::pi::{self, PiAgent},
    daemon::AppState,
    repl::get_default_modelfile,
    utils::config::PY_PORT,
};

pub fn agent_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/agent/start", get(start_agent))
        .route("/agent/stop", get(stop_agent))
        .route("/agent/status", get(agent_status))
}

async fn start_agent(State(state): State<Arc<AppState>>) -> Result<String, StatusCode> {
    let modelfile_path =
        get_default_modelfile(false).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let default_modelfile = tilekit::modelfile::parse_from_file(
        modelfile_path
            .to_str()
            .expect("default_modelfile_path: Failed PathBuf to str"),
    )
    .map_err(|_| StatusCode::INSUFFICIENT_STORAGE)?;

    let modelname = default_modelfile
        .from
        .clone()
        .ok_or_else(|| StatusCode::INTERNAL_SERVER_ERROR)?;

    let system_prompt = default_modelfile.system.clone().unwrap_or("".to_owned());

    let pi_agent = pi::new(&modelname, &system_prompt, PY_PORT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut agent = state.agent.lock().await;
    if agent.is_some() {
        Ok(String::from("agent already up"))
    } else {
        *agent = Some(pi_agent);
        Ok(String::from("Started agent"))
    }
}

#[debug_handler]
async fn stop_agent(State(state): State<Arc<AppState>>) -> Result<String, StatusCode> {
    let mut agent = state.agent.lock().await;
    let agent = agent.as_mut().unwrap();
    pi::handle_graceful_exit(&mut agent.writer)
        .await
        .map_err(|_| StatusCode::METHOD_NOT_ALLOWED)?;
    Ok(String::from("Stopped agent"))
}

async fn agent_status(State(state): State<Arc<AppState>>) -> Result<String, StatusCode> {
    let mut agent = state.agent.lock().await;
    let agent = agent.as_mut().unwrap();

    let state = agent
        .reader
        .get_pi_state(&mut agent.writer)
        .await
        .map_err(|_| StatusCode::FAILED_DEPENDENCY)?;

    let state_str = format!("{}", serde_json::to_string(&state).unwrap());

    Ok(state_str)
}
