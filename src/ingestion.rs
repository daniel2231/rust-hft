use crate::types::{DepthSnapshot, DepthUpdate, MarketEvent, Trade};
use crate::watchdog::Watchdog;
use anyhow::Result;
use crossbeam_channel::Sender;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

#[derive(Debug, Deserialize)]
struct SnapshotResponse {
    #[serde(rename = "lastUpdateId")]
    last_update_id: u64,
    bids: Vec<[String; 2]>,
    asks: Vec<[String; 2]>,
}

async fn fetch_snapshot(rest_url: &str, symbol: &str) -> Result<DepthSnapshot> {
    let url = format!("{}/fapi/v1/depth?symbol={}&limit=1000", rest_url, symbol);
    info!("Fetching depth snapshot: {}", url);
    let resp = reqwest::get(&url).await?.json::<SnapshotResponse>().await?;
    Ok(DepthSnapshot {
        last_update_id: resp.last_update_id,
        bids: resp.bids,
        asks: resp.asks,
    })
}

pub async fn run_ingestion(
    ws_url: String,
    symbol: String,
    tx: Sender<MarketEvent>,
    watchdog: Arc<Watchdog>,
    cancel: CancellationToken,
) -> Result<()> {
    run_ingestion_with_rest(
        ws_url,
        "https://fapi.binance.com".to_string(),
        symbol,
        tx,
        watchdog,
        cancel,
    )
    .await
}

pub async fn run_ingestion_with_rest(
    ws_url: String,
    rest_url: String,
    symbol: String,
    tx: Sender<MarketEvent>,
    watchdog: Arc<Watchdog>,
    cancel: CancellationToken,
) -> Result<()> {
    let stream = format!(
        "{}/{}@depth@100ms/{}@aggTrade",
        ws_url,
        symbol.to_lowercase(),
        symbol.to_lowercase()
    );
    info!("Connecting to {}", stream);

    loop {
        if cancel.is_cancelled() {
            info!("Ingestion cancelled");
            return Ok(());
        }

        match connect_async(&stream).await {
            Ok((ws_stream, _)) => {
                info!("WebSocket connected");
                let (mut write, mut read) = ws_stream.split();

                let rest_url_clone = rest_url.clone();
                let symbol_clone = symbol.clone();
                let tx_snap = tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    match fetch_snapshot(&rest_url_clone, &symbol_clone).await {
                        Ok(snap) => {
                            info!("Snapshot fetched, lastUpdateId={}", snap.last_update_id);
                            let _ = tx_snap.send(MarketEvent::DepthSnapshot(snap));
                        }
                        Err(e) => {
                            error!("Failed to fetch snapshot: {}", e);
                        }
                    }
                });

                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => {
                            info!("Ingestion cancelled, closing WebSocket");
                            return Ok(());
                        }
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    let raw = text.as_str();
                                    tracing::debug!("RAW: {}", raw);

                                    if let Ok(v) = serde_json::from_str::<Value>(raw) {
                                        let event_type =
                                            v.get("e").and_then(|e| e.as_str()).unwrap_or("");
                                        match event_type {
                                            "depthUpdate" => {
                                                if let Ok(update) =
                                                    serde_json::from_value::<DepthUpdate>(v)
                                                {
                                                    watchdog.touch();
                                                    let _ = tx.send(MarketEvent::DepthUpdate(update));
                                                }
                                            }
                                            "aggTrade" => {
                                                if let Ok(trade) =
                                                    serde_json::from_value::<Trade>(v)
                                                {
                                                    watchdog.touch();
                                                    let _ = tx.send(MarketEvent::Trade(trade));
                                                }
                                            }
                                            _ => {
                                                info!("Unknown event type: {}", event_type);
                                            }
                                        }
                                    }
                                }
                                Some(Ok(Message::Ping(data))) => {
                                    let _ = write.send(Message::Pong(data)).await;
                                }
                                Some(Ok(Message::Close(_))) => {
                                    warn!("WebSocket closed by server");
                                    break;
                                }
                                Some(Err(e)) => {
                                    error!("WebSocket error: {}", e);
                                    break;
                                }
                                None => break,
                                _ => {}
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Connection failed: {}", e);
            }
        }

        let _ = tx.send(MarketEvent::Reconnect);
        warn!("Reconnecting in 3s...");
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Ingestion cancelled during reconnect wait");
                return Ok(());
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(3)) => {}
        }
    }
}
