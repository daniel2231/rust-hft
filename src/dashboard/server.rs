use super::SharedState;
use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws::{WebSocket, Message}},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::info;

pub async fn run_dashboard(state: Arc<SharedState>, port: u16) {
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(ws_handler))
        .route("/halt", get(halt_handler))
        .route("/resume", get(resume_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("Dashboard running at http://localhost:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn halt_handler() -> impl IntoResponse {
    std::fs::write("HALT", "").ok();
    "OK"
}

async fn resume_handler() -> impl IntoResponse {
    std::fs::remove_file("HALT").ok();
    "OK"
}

async fn index_handler() -> impl IntoResponse {
    Html(include_str!("../../static/dashboard.html"))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<SharedState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<SharedState>) {
    let mut ticker = interval(Duration::from_secs(1));
    loop {
        ticker.tick().await;
        let dashboard = state.to_dashboard_state();
        let json = match serde_json::to_string(&dashboard) {
            Ok(j) => j,
            Err(_) => continue,
        };
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
}
