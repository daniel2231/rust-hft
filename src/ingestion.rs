use crate::types::{DepthUpdate, MarketEvent, Trade};
use anyhow::Result;
use crossbeam_channel::Sender;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

pub async fn run_ingestion(ws_url: String, symbol: String, tx: Sender<MarketEvent>) -> Result<()> {
    let stream = format!("{}/{}@depth@100ms/{}@aggTrade", ws_url, symbol.to_lowercase(), symbol.to_lowercase());
    info!("Connecting to {}", stream);

    loop {
        match connect_async(&stream).await {
            Ok((ws_stream, _)) => {
                info!("WebSocket connected");
                let (mut write, mut read) = ws_stream.split();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            let raw = text.as_str();
                            tracing::debug!("RAW: {}", raw);

                            if let Ok(v) = serde_json::from_str::<Value>(raw) {
                                let event_type = v.get("e").and_then(|e| e.as_str()).unwrap_or("");
                                match event_type {
                                    "depthUpdate" => {
                                        if let Ok(update) = serde_json::from_value::<DepthUpdate>(v) {
                                            let _ = tx.send(MarketEvent::DepthUpdate(update));
                                        }
                                    }
                                    "aggTrade" => {
                                        if let Ok(trade) = serde_json::from_value::<Trade>(v) {
                                            let _ = tx.send(MarketEvent::Trade(trade));
                                        }
                                    }
                                    _ => {
                                        info!("Unknown event type: {}", event_type);
                                    }
                                }
                            }
                        }
                        Ok(Message::Ping(data)) => {
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        Ok(Message::Close(_)) => {
                            warn!("WebSocket closed by server");
                            break;
                        }
                        Err(e) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                error!("Connection failed: {}", e);
            }
        }

        let _ = tx.send(MarketEvent::Reconnect);
        warn!("Reconnecting in 3s...");
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }
}
