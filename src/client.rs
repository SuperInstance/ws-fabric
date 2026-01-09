//! WebSocket client with connection management

use crate::{
    backpressure::BackpressureController, config::ClientConfig, error::{Error, Result},
    heartbeat::HeartbeatManager, message::Message, metrics::MetricsCollector,
    reconnect::ReconnectState,
};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as TungsteniteMessage, WebSocketStream};

/// WebSocket client
#[derive(Debug, Clone)]
pub struct WebSocketClient {
    #[allow(dead_code)]
    config: ClientConfig,
    state: Arc<Mutex<ClientState>>,
    tx: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
    metrics: Arc<MetricsCollector>,
    heartbeat: Arc<HeartbeatManager>,
    backpressure: Arc<BackpressureController>,
    #[allow(dead_code)]
    reconnect: Arc<Mutex<ReconnectState>>,
}

#[derive(Debug)]
struct ClientState {
    connected: bool,
    shutdown: bool,
}

impl WebSocketClient {
    /// Connect to a WebSocket server
    pub async fn connect(url: impl Into<String>) -> Result<Self> {
        Self::connect_with_config(ClientConfig::new(url)).await
    }

    /// Connect with custom configuration
    pub async fn connect_with_config(config: ClientConfig) -> Result<Self> {
        let url = config.url.clone();
        let (ws_stream, _) = connect_async(&url).await.map_err(|e| {
            Error::connection_failed(format!("Failed to connect to {}: {}", url, e))
        })?;

        let (tx, rx) = mpsc::channel(config.backpressure_config.max_buffer_size);

        let client = Self {
            config: config.clone(),
            state: Arc::new(Mutex::new(ClientState {
                connected: true,
                shutdown: false,
            })),
            tx: Arc::new(Mutex::new(Some(tx))),
            metrics: Arc::new(MetricsCollector::new()),
            heartbeat: Arc::new(HeartbeatManager::new(config.heartbeat_config)),
            backpressure: Arc::new(BackpressureController::new(config.backpressure_config)),
            reconnect: Arc::new(Mutex::new(ReconnectState::new(config.reconnect_config))),
        };

        client.metrics.record_connection();

        // Spawn background task for message handling
        let client_clone = client.clone();
        tokio::spawn(async move {
            if let Err(e) = client_clone.run(ws_stream, rx).await {
                tracing::error!("WebSocket task error: {}", e);
            }
        });

        // Spawn heartbeat task
        let client_clone = client.clone();
        tokio::spawn(async move {
            if let Err(e) = client_clone.run_heartbeat().await {
                tracing::error!("Heartbeat error: {}", e);
            }
        });

        Ok(client)
    }

    /// Send a text message
    pub async fn send_text(&self, text: &str) -> Result<()> {
        self.send(Message::text(text)).await
    }

    /// Send a binary message
    pub async fn send_binary(&self, data: &[u8]) -> Result<()> {
        self.send(Message::binary_from_slice(data)).await
    }

    /// Send a message
    pub async fn send(&self, msg: Message) -> Result<()> {
        // Check backpressure
        if !self.backpressure.can_send() {
            return Err(Error::BufferFull);
        }

        // Get sender
        let tx_opt: Option<mpsc::Sender<Message>> = {
            let tx_guard = self.tx.lock();
            tx_guard.clone()
        };

        let tx = match tx_opt {
            Some(tx) => tx,
            None => return Err(Error::ConnectionClosed),
        };

        // Send message
        let size = msg.len();
        tx.send(msg)
            .await
            .map_err(|_| Error::ConnectionClosed)?;

        // Record metrics
        self.metrics.record_message_sent(size);
        self.backpressure.record_send(size);

        Ok(())
    }

    /// Receive a message (blocking)
    pub async fn recv(&self) -> Result<Option<Message>> {
        // This is a simplified implementation
        // In real implementation, this would use a receiver channel
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        Ok(None)
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        let state = self.state.lock();
        state.connected && !state.shutdown
    }

    /// Close the connection
    pub async fn close(&self, reason: Option<String>) -> Result<()> {
        let msg = if let Some(reason) = reason {
            Message::close(Some(1000), Some(reason))
        } else {
            Message::close(None, None)
        };

        self.send(msg).await?;

        // Mark as shutdown
        let mut state = self.state.lock();
        state.shutdown = true;
        state.connected = false;

        self.metrics.record_disconnection();

        Ok(())
    }

    /// Get metrics
    pub fn metrics(&self) -> &MetricsCollector {
        &self.metrics
    }

    /// Get heartbeat manager
    pub fn heartbeat(&self) -> &HeartbeatManager {
        &self.heartbeat
    }

    /// Get backpressure controller
    pub fn backpressure(&self) -> &BackpressureController {
        &self.backpressure
    }

    // Internal methods

    async fn run(
        &self,
        mut ws_stream: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        mut rx: mpsc::Receiver<Message>,
    ) -> Result<()> {
        loop {
            tokio::select! {
                // Handle outgoing messages
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => {
                            // Convert to tungstenite message
                            let tungstenite_msg = match msg.msg_type {
                                crate::MessageType::Text => {
                                    TungsteniteMessage::Text(msg.as_text()?.into())
                                }
                                crate::MessageType::Binary => {
                                    TungsteniteMessage::Binary(msg.payload.clone())
                                }
                                crate::MessageType::Ping => {
                                    TungsteniteMessage::Ping(msg.payload.clone())
                                }
                                crate::MessageType::Pong => {
                                    TungsteniteMessage::Pong(msg.payload.clone())
                                }
                                crate::MessageType::Close => {
                                    TungsteniteMessage::Close(None)
                                }
                                crate::MessageType::Continuation => {
                                    TungsteniteMessage::Binary(msg.payload.clone())
                                }
                            };

                            ws_stream.send(tungstenite_msg).await.map_err(|e| {
                                Error::connection_failed(format!("Send error: {}", e))
                            })?;

                            self.backpressure.record_send(msg.len());
                        }
                        None => {
                            // Channel closed
                            return Ok(());
                        }
                    }
                }

                // Handle incoming messages
                Some(msg_result) = ws_stream.next() => {
                    match msg_result {
                        Ok(tungstenite_msg) => {
                            let msg = match tungstenite_msg {
                                TungsteniteMessage::Text(text) => {
                                    Message::text(text.to_string())
                                }
                                TungsteniteMessage::Binary(data) => Message::binary(data),
                                TungsteniteMessage::Ping(data) => {
                                    // Respond to ping with pong
                                    let pong = self.heartbeat.handle_ping(&data);
                                    let _ = ws_stream.send(TungsteniteMessage::Pong(pong.payload().to_vec().into())).await;
                                    continue;
                                }
                                TungsteniteMessage::Pong(data) => {
                                    // Handle pong
                                    if let Err(e) = self.heartbeat.handle_pong(&data) {
                                        tracing::warn!("Pong error: {}", e);
                                    }
                                    continue;
                                }
                                TungsteniteMessage::Close(_) => {
                                    tracing::info!("Connection closed by server");
                                    self.metrics.record_disconnection();
                                    return Ok(());
                                }
                                _ => continue,
                            };

                            let size = msg.len();
                            self.metrics.record_message_received(size);
                            self.backpressure.record_recv(size);
                        }
                        Err(e) => {
                            tracing::error!("WebSocket error: {}", e);
                            self.metrics.record_receive_error();
                            return Err(Error::WebSocket(e.to_string()));
                        }
                    }
                }
            }
        }
    }

    async fn run_heartbeat(&self) -> Result<()> {
        if !self.heartbeat.is_enabled() {
            return Ok(());
        }

        let mut interval = self.heartbeat.ping_interval()?;

        loop {
            interval.tick().await;

            if !self.is_connected() {
                return Ok(());
            }

            // Send ping
            let ping = self.heartbeat.ping();

            if let Err(e) = self.send(ping).await {
                tracing::warn!("Failed to send ping: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_builder() {
        let config = ClientConfig::new("ws://localhost:8080")
            .with_max_message_size(2048)
            .with_auto_reconnect(false);

        assert_eq!(config.url, "ws://localhost:8080");
        assert_eq!(config.max_message_size, 2048);
        assert!(!config.auto_reconnect);
    }

    #[test]
    fn test_client_not_connected() {
        let config = ClientConfig::new("ws://localhost:8080");
        let client = WebSocketClient {
            config: config.clone(),
            state: Arc::new(Mutex::new(ClientState {
                connected: false,
                shutdown: false,
            })),
            tx: Arc::new(Mutex::new(None)),
            metrics: Arc::new(MetricsCollector::new()),
            heartbeat: Arc::new(HeartbeatManager::new(config.heartbeat_config)),
            backpressure: Arc::new(BackpressureController::new(config.backpressure_config)),
            reconnect: Arc::new(Mutex::new(ReconnectState::new(config.reconnect_config))),
        };

        assert!(!client.is_connected());
    }
}
