//! WebSocket subprotocol negotiation example
//!
//! This example demonstrates how to use subprotocol negotiation during the WebSocket handshake.

use websocket_fabric::{
    ClientConfig, ServerConfig,
    subprotocol::{SubprotocolList, SubprotocolNegotiator},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== WebSocket Subprotocol Negotiation Example ===\n");

    // ===== Client-Side Configuration =====
    println!("1. Client Configuration:");
    println!("   Client wants these subprotocols (in preference order):");

    let client_protocols = SubprotocolList::new()
        .with_protocol("chat-v2")?
        .with_protocol("chat-v1")?
        .with_protocol("mqtt")?;

    for (i, protocol) in client_protocols.protocols().iter().enumerate() {
        println!("     {}. {}", i + 1, protocol);
    }

    // Generate Sec-WebSocket-Protocol header
    let header_value = client_protocols.to_header_value();
    println!("\n   Sec-WebSocket-Protocol header:");
    println!("   \"{}\"", header_value);

    // Client can use this with ClientConfig
    let client_config = ClientConfig::new("ws://localhost:8080")
        .with_subprotocols(&["chat-v2", "chat-v1", "mqtt"]);
    println!("\n   Client config created with {} subprotocols", client_config.subprotocols.len());

    // ===== Server-Side Configuration =====
    println!("\n2. Server Configuration:");
    println!("   Server supports these subprotocols:");

    let server_negotiator = SubprotocolNegotiator::new()
        .with_protocol("mqtt")?
        .with_protocol("chat-v2")?
        .with_protocol("chat-v1")?;

    for (i, protocol) in server_negotiator.protocols().iter().enumerate() {
        println!("     {}. {}", i + 1, protocol);
    }

    // Server can use this with ServerConfig
    let server_config = ServerConfig::new("0.0.0.0:8080")
        .with_subprotocols(&["mqtt", "chat-v2", "chat-v1"]);
    println!("\n   Server config created with {} subprotocols", server_config.subprotocols.len());

    // ===== Negotiation =====
    println!("\n3. Negotiation:");
    let selected = server_negotiator.negotiate(&client_protocols);

    match selected {
        Some(ref protocol) => {
            println!("   ✓ Match found!");
            println!("   Selected subprotocol: \"{}\"", protocol);
            println!("   (Server selected client's first preference that it supports)");
        }
        None => {
            println!("   ✗ No match found");
            println!("   Connection will proceed without subprotocol");
        }
    }

    // ===== Client Validates Server Response =====
    println!("\n4. Client validates server response:");
    let validated = client_protocols.parse_response(selected.as_deref())?;

    match validated {
        Some(ref protocol) => {
            println!("   ✓ Server response validated");
            println!("   Negotiated subprotocol: \"{}\"", protocol);
        }
        None => {
            println!("   ✓ No subprotocol negotiated");
        }
    }

    // ===== Alternative Scenario: No Match =====
    println!("\n5. Alternative Scenario: Client offers unsupported protocols");

    let unsupported_client = SubprotocolList::new()
        .with_protocol("graphql")?
        .with_protocol("grpc")?;

    println!("   Client offers: {}", unsupported_client.to_header_value());

    let no_match = server_negotiator.negotiate(&unsupported_client);
    match no_match {
        Some(_) => println!("   Unexpected match!"),
        None => {
            println!("   ✓ No match found (as expected)");
            println!("   Server should either:");
            println!("     - Accept connection without subprotocol");
            println!("     - Reject connection with HTTP 426");
        }
    }

    // Check if server should reject
    if server_negotiator.should_reject(&unsupported_client) {
        println!("   Server configured to reject on no match");
    }

    // ===== Alternative Scenario: Wildcard =====
    println!("\n6. Alternative Scenario: Wildcard (proxy server)");

    let wildcard_server = SubprotocolNegotiator::new()
        .with_wildcard(true);

    println!("   Server has wildcard enabled (accepts any protocol)");
    let wildcard_selection = wildcard_server.negotiate(&unsupported_client);
    println!("   Selected: {:?}", wildcard_selection);
    println!("   (Useful for proxy servers that forward any subprotocol)");

    // ===== Common Real-World Subprotocols =====
    println!("\n7. Common Real-World Subprotocols:");
    println!("   - mqtt          Message Queuing Telemetry Transport");
    println!("   - graphql       GraphQL over WebSocket");
    println!("   - graphql-transport-ws  GraphQL WebSocket transport");
    println!("   - chat-v1, chat-v2  Custom chat protocols");
    println!("   - sip           Session Initiation Protocol");
    println!("   - wamp          Web Application Messaging Protocol");
    println!("   - stomp         Simple/Streaming Text Oriented Messaging Protocol");

    println!("\n=== Example Complete ===");
    Ok(())
}
