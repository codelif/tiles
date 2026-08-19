//! APIs for communication with Agent harness (Pi)

use crate::{
    core::agent::pi::{self},
    daemon::{ApiResponse, AppError, AppState},
    repl::{get_default_modelfile, model_spec},
    utils::config::PY_PORT,
};
use axum::{
    Json, Router,
    extract::State,
    response::{IntoResponse, Sse, sse::Event},
    routing::{get, post},
};
use axum_macros::debug_handler;
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Deserialize)]
struct promptRequest {
    message: String,
}

struct SseEvent {
    event: String,
    data: String,
}

pub fn agent_router() -> Router<Arc<AppState>> {
    Router::new()
        // TODO: start should be a POST request
        .route("/v1/tilekit/agent/start", get(start_agent))
        // TODO: end_session should be a POST req
        .route("/v1/tilekit/agent/end_session", get(end_current_session))
        .route("/v1/tilekit/agent/state", get(agent_state))
        .route("/v1/tilekit/agent/prompt", post(process_chat_prompt))
}

async fn start_agent(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, AppError> {
    let modelfile_path =
        get_default_modelfile().map_err(|e| AppError::ModelFileNotFound(e.to_string()))?;
    let default_modelfile = tilekit::modelfile::parse_from_file(&modelfile_path.to_string_lossy())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let modelname =
        model_spec(&default_modelfile).map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let system_prompt = default_modelfile.system.clone().unwrap_or("".to_owned());

    let mut agent = state.agent.lock().await;
    if agent.is_some() {
        Ok(ApiResponse::success(
            json!({"message": "Agent already started"}),
        ))
    } else {
        let pi_agent = pi::new(&modelname, &system_prompt, PY_PORT)
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
        *agent = Some(pi_agent);
        Ok(ApiResponse::success(json!({"message": "started agent"})))
    }
}

#[debug_handler]
async fn end_current_session(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let mut agent = state.agent.lock().await;
    let agent = agent.as_mut().ok_or(AppError::InternalServerError(
        "Failed to get a mutable agent instance".to_string(),
    ))?;
    pi::handle_graceful_exit(&mut agent.writer)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(ApiResponse::success(
        json!({"message": "Successfully ended current session"}),
    ))
}

// TODO: Could we have explicity tell in return type we are sending
// GetStateData?
async fn agent_state(State(state): State<Arc<AppState>>) -> Result<impl IntoResponse, AppError> {
    let mut agent = state.agent.lock().await;
    let agent = agent.as_mut().ok_or(AppError::InternalServerError(
        "Failed to get a mutable agent instance".to_string(),
    ))?;

    let state = agent
        .reader
        .get_pi_state(&mut agent.writer)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(ApiResponse::success(serde_json::to_value(state).unwrap()))
}

//TODO: understand the way of signature here
#[debug_handler]
async fn process_chat_prompt(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<promptRequest>,
) -> Result<impl IntoResponse, AppError> {
    let t_state = state.clone();

    // So the current idea is we create a mpsc channel
    // send the pi events as strings to receiver
    // then receiver will return sse
    //

    // why the error type in infallible, and why channel even is Result?
    let (tx, rx) = mpsc::channel::<SseEvent>(32);

    tokio::spawn(async move {
        let mut agent = t_state.agent.lock().await;
        let agent = agent
            .as_mut()
            .ok_or(AppError::InternalServerError(
                "Failed to get a mutable agent instance".to_string(),
            ))
            .unwrap();
        let payload = json!({
            "type": "prompt",
            "message": payload.message
        });
        agent.writer.send_to_pi(payload).await.unwrap();

        while let Ok(Some(line)) = agent.reader.next_line().await {
            let json_event: serde_json::Value = serde_json::from_str(&line).unwrap();

            let sse_event = SseEvent {
                event: json_event["type"].as_str().unwrap().to_string(),
                data: line.clone(),
            };
            let _ = tx.send(sse_event).await;
        }
    });

    println!("Stream start.");

    let sse_stream = ReceiverStream::new(rx)
        .map(|msg| Ok::<_, Infallible>(Event::default().event(msg.event).data(msg.data)));

    println!("Stream completed.");
    Ok(Sse::new(sse_stream))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::{body::Body, http::Request};
    use reqwest::StatusCode;
    use serde_json::json;
    use tokio_stream::StreamExt;
    use tower::ServiceExt;

    use crate::daemon::{AppState, agent::agent_router};

    #[tokio::test]
    async fn test_process_chat_prompt_success() {
        let state = AppState {
            shutdown_sender: Mutex::new(None),
            vsn: env!("CARGO_PKG_VERSION").to_owned(),
            remote_ticket: Mutex::new(None),
            remote_shutdown_sender: Mutex::new(None),
            remote_running: Mutex::new(false),
            agent: None.into(),
        };
        let body = json!({
            "message": "hello"
        })
        .to_string();
        let agent_app = agent_router();
        let response = agent_app
            .with_state(state.into())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .header("content-type", "application/json")
                    .uri("/v1/tilekit/agent/prompt")
                    .body(Body::new(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/event-stream"
        );
        let mut body_stream = response.into_body().into_data_stream();

        let first_chunk = body_stream.next().await.unwrap().unwrap();

        println!("{:?}", first_chunk);

        // let body = response.body().to_owned();
        // let stream = body;

        // println!("{:?}", response.body().into_data_stream());
        // assert_eq!(response.status(), StatusCode::OK);
    }
}

// curl -X POST "http://127.0.0.1:1729/v1/tilekit/agent/prompt" \
//   -H "Content-Type: application/json" \
//   -d '{"message":"hello"}'
