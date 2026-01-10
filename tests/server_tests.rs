//! Integration tests for WebSocket server

use websocket_fabric::{Message, ServerConfig, WebSocketServer};

#[tokio::test]
async fn test_server_creation_and_config() {
    let config = ServerConfig::new("127.0.0.1:0")
        .with_max_connections(100)
        .with_max_message_size(2048)
        .with_buffer_size(500);

    let server = WebSocketServer::new(config);

    assert!(!server.is_running());
    assert_eq!(server.connection_count(), 0);
}

#[tokio::test]
async fn test_server_bind_helper() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    assert_eq!(server.connection_count(), 0);
    assert!(!server.is_running());
}

#[tokio::test]
async fn test_server_broadcast_empty() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    let result = server.broadcast_text("test").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
}

#[tokio::test]
async fn test_server_broadcast_binary_empty() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    let result = server.broadcast_binary(&[1, 2, 3, 4]).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
}

#[tokio::test]
async fn test_server_get_client() {
    use uuid::Uuid;

    let server = WebSocketServer::bind("127.0.0.1:0");

    let client_id = Uuid::new_v4();
    let client = server.get_client(client_id);

    assert!(client.is_none());
}

#[tokio::test]
async fn test_server_metrics_accessors() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    let metrics = server.metrics();

    assert_eq!(metrics.active_connections(), 0);
    assert_eq!(metrics.messages_sent(), 0);
    assert_eq!(metrics.messages_received(), 0);
    assert_eq!(metrics.bytes_sent(), 0);
    assert_eq!(metrics.bytes_received(), 0);
}

#[tokio::test]
async fn test_server_heartbeat() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    let heartbeat = server.heartbeat();

    assert!(heartbeat.is_enabled());
    assert_eq!(heartbeat.ping_count(), 0);
    assert_eq!(heartbeat.pong_count(), 0);
}

#[tokio::test]
async fn test_server_backpressure() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    let backpressure = server.backpressure();

    assert!(backpressure.can_send());
    assert!(!backpressure.is_backpressure_active());
    assert_eq!(backpressure.current_size(), 0);
    assert_eq!(backpressure.utilization(), 0.0);
}

#[tokio::test]
async fn test_server_shutdown() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    // Should not panic even with no connections
    server.shutdown().await;

    assert!(!server.is_running());
    assert_eq!(server.connection_count(), 0);
}

#[tokio::test]
async fn test_server_clone() {
    let server = WebSocketServer::bind("127.0.0.1:0");
    let server_clone = server.clone();

    assert_eq!(server.connection_count(), server_clone.connection_count());
    assert_eq!(server.is_running(), server_clone.is_running());
}

#[tokio::test]
async fn test_message_creation_for_broadcast() {
    let text_msg = Message::text("Hello, world!");
    assert!(text_msg.is_text());
    assert_eq!(text_msg.len(), 13);

    let binary_msg = Message::binary_from_slice(&[1, 2, 3, 4]);
    assert!(binary_msg.is_binary());
    assert_eq!(binary_msg.len(), 4);

    let ping_msg = Message::ping(&[1u8, 2, 3][..]);
    assert!(ping_msg.is_ping());

    let pong_msg = Message::pong(&[4u8, 5, 6][..]);
    assert!(pong_msg.is_pong());

    let close_msg = Message::close(Some(1000), Some("Normal closure".to_string()));
    assert!(close_msg.is_close());
}

#[tokio::test]
async fn test_server_config_validation() {
    let config = ServerConfig::new("127.0.0.1:8080");

    assert_eq!(config.bind_address, "127.0.0.1:8080");
    assert_eq!(config.max_connections, 10_000);
    assert_eq!(config.buffer_size, 1000);
}

#[tokio::test]
async fn test_server_config_builder() {
    let config = ServerConfig::new("0.0.0.0:9000")
        .with_max_connections(5000)
        .with_max_message_size(2048)
        .with_buffer_size(2000);

    assert_eq!(config.bind_address, "0.0.0.0:9000");
    assert_eq!(config.max_connections, 5000);
    assert_eq!(config.max_message_size, 2048);
    assert_eq!(config.buffer_size, 2000);
}

#[tokio::test]
async fn test_server_metrics_tracking() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    let metrics = server.metrics();

    // Record some activity
    metrics.record_connection();
    assert_eq!(metrics.active_connections(), 1);

    metrics.record_message_sent(100);
    assert_eq!(metrics.messages_sent(), 1);
    assert_eq!(metrics.bytes_sent(), 100);

    metrics.record_message_received(50);
    assert_eq!(metrics.messages_received(), 1);
    assert_eq!(metrics.bytes_received(), 50);

    metrics.record_disconnection();
    assert_eq!(metrics.active_connections(), 0);
}

#[tokio::test]
async fn test_server_backpressure_tracking() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    let backpressure = server.backpressure();

    // Record some sends
    backpressure.record_send(100);
    assert_eq!(backpressure.current_size(), 100);
    assert_eq!(backpressure.utilization(), 0.1); // 100/1000

    // Record some receives
    backpressure.record_recv(50);
    assert_eq!(backpressure.current_size(), 50);
    assert_eq!(backpressure.utilization(), 0.05); // 50/1000
}

#[tokio::test]
async fn test_server_heartbeat_ping_pong() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    let heartbeat = server.heartbeat();

    // Generate ping
    let ping = heartbeat.ping();
    assert!(ping.is_ping());
    assert_eq!(heartbeat.ping_count(), 1);

    // Handle pong
    let result = heartbeat.handle_pong(ping.payload());
    assert!(result.is_ok());
    assert_eq!(heartbeat.pong_count(), 1);
    assert!(heartbeat.is_alive());
}

#[tokio::test]
async fn test_server_clients_list() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    let clients = server.clients();
    assert!(clients.is_empty());
    assert_eq!(clients.len(), 0);
}

#[tokio::test]
async fn test_server_multiple_broadcasts() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    // Multiple broadcasts should all succeed (with 0 clients)
    for i in 0..5 {
        let result = server.broadcast_text(&format!("Message {}", i)).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}

#[tokio::test]
async fn test_server_concurrent_broadcasts() {
    let server = WebSocketServer::bind("127.0.0.1:0");

    let server1 = server.clone();
    let server2 = server.clone();
    let server3 = server.clone();

    // Spawn concurrent broadcasts
    let task1 = tokio::spawn(async move {
        server1.broadcast_text("Broadcast 1").await
    });

    let task2 = tokio::spawn(async move {
        server2.broadcast_text("Broadcast 2").await
    });

    let task3 = tokio::spawn(async move {
        server3.broadcast_text("Broadcast 3").await
    });

    // All should complete successfully
    let results = tokio::join!(task1, task2, task3);
    assert!(results.0.is_ok());
    assert!(results.0.unwrap().is_ok());
    assert!(results.1.is_ok());
    assert!(results.1.unwrap().is_ok());
    assert!(results.2.is_ok());
    assert!(results.2.unwrap().is_ok());
}
