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
    // Central once-per-second sampler: computes events/sec and appends to the
    // PnL history. Doing this here (not per WebSocket client) keeps the
    // numbers correct when several browser tabs are connected.
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let mut tick = interval(Duration::from_secs(1));
            loop {
                tick.tick().await;
                state.sample_second();
            }
        });
    }

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/chart.js", get(chart_js_handler))
        .route("/ws", get(ws_handler))
        .route("/halt", get(halt_handler))
        .route("/resume", get(resume_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("Dashboard running at http://localhost:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn halt_handler(State(state): State<Arc<SharedState>>) -> impl IntoResponse {
    std::fs::write("HALT", "").ok();
    // Set the flag directly so the risk checker reacts immediately,
    // without waiting for the next file-poll tick.
    state.halt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
    "OK"
}

async fn resume_handler(State(state): State<Arc<SharedState>>) -> impl IntoResponse {
    std::fs::remove_file("HALT").ok();
    state.halt_flag.store(false, std::sync::atomic::Ordering::Relaxed);
    "OK"
}

async fn index_handler() -> impl IntoResponse {
    Html(include_str!("../../static/dashboard.html"))
}

// Chart.js is embedded in the binary and served locally so the dashboard
// works without internet access (no CDN dependency).
async fn chart_js_handler() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../static/chart.umd.min.js"),
    )
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
