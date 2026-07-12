use crate::types::{DepthSnapshot, DepthUpdate, MarketEvent, Trade};
use crate::watchdog::Watchdog;
use anyhow::Result;
use crossbeam_channel::Sender;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
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
    resync_needed: Arc<AtomicBool>,
) -> Result<()> {
    run_ingestion_with_rest(
        ws_url,
        "https://fapi.binance.com".to_string(),
        symbol,
        tx,
        watchdog,
        cancel,
        resync_needed,
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
    resync_needed: Arc<AtomicBool>,
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

                // Fetch initial snapshot
                let rest_url_clone = rest_url.clone();
                let symbol_clone = symbol.clone();
                let tx_snap = tx.clone();
                let resync_clone = Arc::clone(&resync_needed);
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    resync_clone.store(false, Ordering::Relaxed);
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

                // Resync poller: checks flag every 200ms and re-fetches snapshot if needed
                let resync_poll = Arc::clone(&resync_needed);
                let rest_url_resync = rest_url.clone();
                let symbol_resync = symbol.clone();
                let tx_resync = tx.clone();
                let cancel_resync = cancel.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = cancel_resync.cancelled() => break,
                            _ = tokio::time::sleep(tokio::time::Duration::from_millis(200)) => {
                                if resync_poll.compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
                                    warn!("Resync requested — re-fetching snapshot");
                                    // Wait briefly so the orderbook thread has buffered some new events
                                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                                    match fetch_snapshot(&rest_url_resync, &symbol_resync).await {
                                        Ok(snap) => {
                                            info!("Resync snapshot fetched, lastUpdateId={}", snap.last_update_id);
                                            let _ = tx_resync.send(MarketEvent::DepthSnapshot(snap));
                                        }
                                        Err(e) => {
                                            error!("Resync snapshot fetch failed: {}", e);
                                            // Set flag again so we retry
                                            resync_poll.store(true, Ordering::Relaxed);
                                        }
                                    }
                                }
                            }
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

                                    // Parse straight into the typed structs — no intermediate
                                    // serde_json::Value tree. depthUpdate is tried first since
                                    // it dominates message volume; an aggTrade fails that parse
                                    // immediately on its numeric "a" field.
                                    if let Ok(update) = serde_json::from_str::<DepthUpdate>(raw) {
                                        if update.event_type == "depthUpdate" {
                                            watchdog.touch();
                                            let _ = tx.send(MarketEvent::DepthUpdate(update));
                                            continue;
                                        }
                                    }
                                    if let Ok(trade) = serde_json::from_str::<Trade>(raw) {
                                        if trade.event_type == "aggTrade" {
                                            watchdog.touch();
                                            let _ = tx.send(MarketEvent::Trade(trade));
                                            continue;
                                        }
                                    }
                                    info!("Unrecognized WS message: {}", &raw[..raw.len().min(120)]);
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
