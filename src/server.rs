//! WebSocket server with connection management

use crate::{
    backpressure::BackpressureController, config::ServerConfig, error::{Error, Result},
    heartbeat::HeartbeatManager, message::Message, metrics::MetricsCollector,
};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::handshake::server::{Request, Response},
    WebSocketStream,
};
use uuid::Uuid;

/// WebSocket server
#[derive(Debug, Clone)]
pub struct WebSocketServer {
    config: ServerConfig,
    state: Arc<Mutex<ServerState>>,
    metrics: Arc<MetricsCollector>,
    heartbeat: Arc<HeartbeatManager>,
    backpressure: Arc<BackpressureController>,
}

#[derive(Debug)]
struct ServerState {
    running: bool,
    connections: HashMap<Uuid, Arc<ConnectedClient>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

/// Connected client
#[derive(Debug, Clone)]
pub struct ConnectedClient {
    id: Uuid,
    tx: mpsc::Sender<Message>,
    remote_addr: String,
}

impl ConnectedClient {
    /// Get client ID
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Get remote address
    pub fn remote_addr(&self) -> &str {
        &self.remote_addr
    }

    /// Send a message to this client
    pub async fn send(&self, msg: Message) -> Result<()> {
        self.tx
            .send(msg)
            .await
            .map_err(|_| Error::ConnectionClosed)
    }

    /// Send a text message to this client
    pub async fn send_text(&self, text: &str) -> Result<()> {
        self.send(Message::text(text)).await
    }

    /// Send a binary message to this client
    pub async fn send_binary(&self, data: &[u8]) -> Result<()> {
        self.send(Message::binary_from_slice(data)).await
    }
}

impl WebSocketServer {
    /// Create a new WebSocket server
    pub fn new(config: ServerConfig) -> Self {
        let heartbeat_config = crate::config::HeartbeatConfig {
            enabled: true,
            ping_interval: config.ping_interval,
            ping_timeout: config.ping_timeout,
        };

        let backpressure_config = crate::config::BackpressureConfig {
            enabled: true,
            max_buffer_size: config.buffer_size,
            backpressure_threshold: 0.8,
            recovery_threshold: 0.6,
        };

        Self {
            config,
            state: Arc::new(Mutex::new(ServerState {
                running: false,
                connections: HashMap::new(),
                shutdown_tx: None,
            })),
            metrics: Arc::new(MetricsCollector::new()),
            heartbeat: Arc::new(HeartbeatManager::new(heartbeat_config)),
            backpressure: Arc::new(BackpressureController::new(backpressure_config)),
        }
    }

    /// Create a new server with a bind address
    pub fn bind(bind_address: impl Into<String>) -> Self {
        Self::new(ServerConfig::new(bind_address))
    }

    /// Start the server and accept connections
    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.config.bind_address)
            .await
            .map_err(|e| Error::connection_failed(format!("Failed to bind to {}: {}", self.config.bind_address, e)))?;

        tracing::info!("WebSocket server listening on {}", self.config.bind_address);

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        // Store shutdown channel
        {
            let mut state = self.state.lock();
            state.running = true;
            state.shutdown_tx = Some(shutdown_tx);
        }

        // Spawn heartbeat task
        let server_clone = self.clone();
        tokio::spawn(async move {
            if let Err(e) = server_clone.run_heartbeat().await {
                tracing::error!("Server heartbeat error: {}", e);
            }
        });

        // Accept connections
        loop {
            tokio::select! {
                // Accept new connection
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            // Check connection limit
                            {
                                let state = self.state.lock();
                                if state.connections.len() >= self.config.max_connections {
                                    tracing::warn!("Connection limit reached, rejecting: {}", addr);
                                    self.metrics.record_connection_error();
                                    continue;
                                }
                            }

                            // Spawn connection handler
                            let server_clone = self.clone();
                            tokio::spawn(async move {
                                if let Err(e) = server_clone.handle_connection(stream, addr.to_string()).await {
                                    tracing::error!("Connection handler error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to accept connection: {}", e);
                            self.metrics.record_connection_error();
                        }
                    }
                }

                // Handle shutdown
                _ = shutdown_rx.recv() => {
                    tracing::info!("Server shutting down...");
                    self.shutdown().await;
                    return Ok(());
                }
            }
        }
    }

    /// Accept a single incoming connection
    pub async fn accept(&self) -> Result<ConnectedClient> {
        // This is a blocking accept - for non-blocking use start() instead
        Err(Error::not_supported(
            "Use start() for async connection accepting. accept() is not yet implemented.",
        ))
    }

    /// Broadcast a message to all connected clients
    pub async fn broadcast(&self, msg: Message) -> Result<usize> {
        let clients: Vec<Arc<ConnectedClient>> = {
            let state = self.state.lock();
            state.connections.values().cloned().collect()
        };

        if clients.is_empty() {
            return Ok(0);
        }

        let mut sent_count = 0;
        let msg_size = msg.len();

        for client in clients {
            if let Err(e) = client.send(msg.clone()).await {
                tracing::warn!("Failed to send to client {}: {}", client.id, e);
            } else {
                sent_count += 1;
                self.metrics.record_message_sent(msg_size);
            }
        }

        Ok(sent_count)
    }

    /// Broadcast a text message to all connected clients
    pub async fn broadcast_text(&self, text: &str) -> Result<usize> {
        self.broadcast(Message::text(text)).await
    }

    /// Broadcast a binary message to all connected clients
    pub async fn broadcast_binary(&self, data: &[u8]) -> Result<usize> {
        self.broadcast(Message::binary_from_slice(data)).await
    }

    /// Get number of connected clients
    pub fn connection_count(&self) -> usize {
        let state = self.state.lock();
        state.connections.len()
    }

    /// Get a connected client by ID
    pub fn get_client(&self, id: Uuid) -> Option<Arc<ConnectedClient>> {
        let state = self.state.lock();
        state.connections.get(&id).cloned()
    }

    /// Get all connected clients
    pub fn clients(&self) -> Vec<Arc<ConnectedClient>> {
        let state = self.state.lock();
        state.connections.values().cloned().collect()
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        let state = self.state.lock();
        state.running
    }

    /// Shutdown the server gracefully
    pub async fn shutdown(&self) {
        tracing::info!("Shutting down WebSocket server...");

        // Close all connections
        let clients: Vec<Arc<ConnectedClient>> = {
            let mut state = self.state.lock();
            let clients: Vec<Arc<ConnectedClient>> = state.connections.values().cloned().collect();
            state.connections.clear();
            state.running = false;
            clients
        };

        // Send close message to all clients
        for client in clients {
            let _ = client
                .send(Message::close(Some(1000), Some("Server shutdown".to_string())))
                .await;
        }

        tracing::info!("Server shutdown complete");
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

    async fn handle_connection(&self, stream: TcpStream, remote_addr: String) -> Result<()> {
        // Accept WebSocket connection with callback
        let ws_stream = accept_hdr_async(stream, |_req: &Request, resp: Response| {
            // Log connection attempt
            tracing::info!("Connection from: {}", remote_addr);

            // Could add custom headers here
            Ok(resp)
        })
        .await
        .map_err(|e| Error::handshake_failed(format!("Failed to accept WebSocket: {}", e)))?;

        // Generate unique client ID
        let client_id = Uuid::new_v4();

        // Create channels for this client
        let (tx, rx) = mpsc::channel(self.config.buffer_size);

        // Create connected client
        let client = Arc::new(ConnectedClient {
            id: client_id,
            tx,
            remote_addr: remote_addr.clone(),
        });

        // Add to connections
        {
            let mut state = self.state.lock();
            state.connections.insert(client_id, client.clone());
        }

        self.metrics.record_connection();
        tracing::info!("Client {} connected from {}", client_id, remote_addr);

        // Spawn task to handle incoming messages from this client
        let server_clone = self.clone();
        tokio::spawn(async move {
            if let Err(e) = server_clone.handle_client_messages(ws_stream, client_id, rx).await {
                tracing::error!("Client {} message handler error: {}", client_id, e);
            }
        });

        Ok(())
    }

    async fn handle_client_messages(
        &self,
        mut ws_stream: WebSocketStream<TcpStream>,
        client_id: Uuid,
        mut rx: mpsc::Receiver<Message>,
    ) -> Result<()> {
        loop {
            tokio::select! {
                // Handle outgoing messages to client
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => {
                            // Convert to tungstenite message
                            let tungstenite_msg = match msg.msg_type {
                                crate::MessageType::Text => {
                                    tokio_tungstenite::tungstenite::protocol::Message::Text(msg.as_text()?.into())
                                }
                                crate::MessageType::Binary => {
                                    tokio_tungstenite::tungstenite::protocol::Message::Binary(msg.payload.clone())
                                }
                                crate::MessageType::Ping => {
                                    tokio_tungstenite::tungstenite::protocol::Message::Ping(msg.payload.clone())
                                }
                                crate::MessageType::Pong => {
                                    tokio_tungstenite::tungstenite::protocol::Message::Pong(msg.payload.clone())
                                }
                                crate::MessageType::Close => {
                                    tokio_tungstenite::tungstenite::protocol::Message::Close(None)
                                }
                                crate::MessageType::Continuation => {
                                    tokio_tungstenite::tungstenite::protocol::Message::Binary(msg.payload.clone())
                                }
                            };

                            ws_stream.send(tungstenite_msg).await.map_err(|e| {
                                Error::connection_failed(format!("Send error: {}", e))
                            })?;

                            self.backpressure.record_send(msg.len());
                        }
                        None => {
                            // Channel closed
                            tracing::info!("Client {} channel closed", client_id);
                            break;
                        }
                    }
                }

                // Handle incoming messages from client
                Some(msg_result) = ws_stream.next() => {
                    match msg_result {
                        Ok(tungstenite_msg) => {
                            match tungstenite_msg {
                                tokio_tungstenite::tungstenite::protocol::Message::Text(text) => {
                                    let size = text.len();
                                    self.metrics.record_message_received(size);
                                    self.backpressure.record_recv(size);
                                    // Could handle incoming text messages here
                                }
                                tokio_tungstenite::tungstenite::protocol::Message::Binary(data) => {
                                    let size = data.len();
                                    self.metrics.record_message_received(size);
                                    self.backpressure.record_recv(size);
                                    // Could handle incoming binary messages here
                                }
                                tokio_tungstenite::tungstenite::protocol::Message::Ping(data) => {
                                    // Respond to ping with pong
                                    let pong = self.heartbeat.handle_ping(&data);
                                    let _ = ws_stream.send(
                                        tokio_tungstenite::tungstenite::protocol::Message::Pong(pong.payload().to_vec().into())
                                    ).await;
                                }
                                tokio_tungstenite::tungstenite::protocol::Message::Pong(data) => {
                                    // Handle pong
                                    if let Err(e) = self.heartbeat.handle_pong(&data) {
                                        tracing::warn!("Pong error from client {}: {}", client_id, e);
                                    }
                                }
                                tokio_tungstenite::tungstenite::protocol::Message::Close(_) => {
                                    tracing::info!("Client {} closed connection", client_id);
                                    break;
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            tracing::error!("WebSocket error from client {}: {}", client_id, e);
                            self.metrics.record_receive_error();
                            break;
                        }
                    }
                }
            }
        }

        // Remove client from connections
        {
            let mut state = self.state.lock();
            state.connections.remove(&client_id);
        }

        self.metrics.record_disconnection();
        tracing::info!("Client {} disconnected", client_id);

        Ok(())
    }

    async fn run_heartbeat(&self) -> Result<()> {
        if !self.heartbeat.is_enabled() {
            return Ok(());
        }

        let mut interval = self.heartbeat.ping_interval()?;

        loop {
            interval.tick().await;

            if !self.is_running() {
                return Ok(());
            }

            // Check all clients for liveness
            let clients = self.clients();

            for client in clients {
                if !self.heartbeat.is_alive() {
                    tracing::warn!("Client {} appears dead, may need disconnect", client.id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let config = ServerConfig::new("127.0.0.1:0")
            .with_max_connections(100)
            .with_buffer_size(500);

        let server = WebSocketServer::new(config.clone());

        assert_eq!(server.connection_count(), 0);
        assert!(!server.is_running()); // Not started yet
    }

    #[test]
    fn test_server_bind() {
        let server = WebSocketServer::bind("127.0.0.1:0");

        assert_eq!(server.connection_count(), 0);
        assert!(!server.is_running());
    }

    #[test]
    fn test_server_config() {
        let config = ServerConfig::new("0.0.0.0:8080")
            .with_max_connections(5000)
            .with_max_message_size(2048)
            .with_buffer_size(2000);

        assert_eq!(config.bind_address, "0.0.0.0:8080");
        assert_eq!(config.max_connections, 5000);
        assert_eq!(config.max_message_size, 2048);
        assert_eq!(config.buffer_size, 2000);
    }

    #[test]
    fn test_connected_client_id() {
        let (tx, _rx) = mpsc::channel(100);
        let client = ConnectedClient {
            id: Uuid::new_v4(),
            tx,
            remote_addr: "127.0.0.1:12345".to_string(),
        };

        assert_eq!(client.remote_addr(), "127.0.0.1:12345");
    }

    #[tokio::test]
    async fn test_broadcast_empty() {
        let server = WebSocketServer::bind("127.0.0.1:0");

        let result = server.broadcast_text("test").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_server_metrics() {
        let server = WebSocketServer::bind("127.0.0.1:0");

        let metrics = server.metrics();
        assert_eq!(metrics.active_connections(), 0);

        let heartbeat = server.heartbeat();
        assert!(heartbeat.is_enabled());

        let backpressure = server.backpressure();
        assert!(backpressure.can_send());
    }
}
