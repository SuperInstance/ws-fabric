//! Message fragmentation and reassembly example
//!
//! This example demonstrates how to use the Fragmenter and Reassembler
//! to handle large WebSocket messages that need to be split across multiple frames.

use websocket_fabric::{
    fragmentation::{Fragmenter, Reassembler, DEFAULT_MAX_FRAME_SIZE},
    Message,
};
use std::time::Duration;

fn main() {
    println!("WebSocket Message Fragmentation Example");
    println!("======================================\n");

    // Example 1: Fragment a large text message
    println!("Example 1: Fragmenting a large text message");
    let fragmenter = Fragmenter::new(DEFAULT_MAX_FRAME_SIZE); // 16KB frames

    // Create a large message (50KB)
    let large_text = "A".repeat(50 * 1024);
    let message = Message::text(large_text.clone());

    println!("Original message size: {} bytes", message.len());

    // Split into frames
    let frames = fragmenter.split_message(&message);
    println!("Number of frames: {}", frames.len());
    println!("First frame: FIN={}, opcode=0x{:02X}, payload={} bytes",
        frames[0].fin, frames[0].opcode, frames[0].payload.len());
    println!("Last frame: FIN={}, opcode=0x{:02X}, payload={} bytes",
        frames.last().unwrap().fin,
        frames.last().unwrap().opcode,
        frames.last().unwrap().payload.len());

    // Example 2: Reassemble fragmented message
    println!("\nExample 2: Reassembling fragmented message");
    let reassembler = Reassembler::new(Duration::from_secs(5));

    let mut reassembled_msg = None;
    for (i, frame) in frames.iter().enumerate() {
        match reassembler.ingest_frame(frame, 1) {
            Ok(Some(msg)) => {
                println!("Frame {}: Complete message received!", i);
                reassembled_msg = Some(msg);
                break;
            }
            Ok(None) => {
                println!("Frame {}: Partial message, waiting for more frames...", i);
            }
            Err(e) => {
                println!("Frame {}: Error: {}", i, e);
                break;
            }
        }
    }

    if let Some(msg) = reassembled_msg {
        println!("Reassembled message size: {} bytes", msg.len());
        println!("Reassembly successful: {}", msg.len() == large_text.len());
    }

    // Example 3: Multiple connections
    println!("\nExample 3: Handling multiple concurrent connections");
    let reassembler = Reassembler::default();

    // Simulate messages from different connections
    let msg1 = Message::text("Connection 1: ".repeat(1000));
    let msg2 = Message::binary(vec![1u8; 20 * 1024]);
    let msg3 = Message::text("Connection 3: ".repeat(500));

    let frames1 = fragmenter.split_message(&msg1);
    let frames2 = fragmenter.split_message(&msg2);
    let frames3 = fragmenter.split_message(&msg3);

    println!("Connection 1: {} frames", frames1.len());
    println!("Connection 2: {} frames", frames2.len());
    println!("Connection 3: {} frames", frames3.len());
    println!("In-progress reassemblies: {}", reassembler.in_progress_count());

    // Example 4: Calculate frame count
    println!("\nExample 4: Calculating frame requirements");
    let sizes = vec![1024, 16 * 1024, 100 * 1024, 1024 * 1024];
    for size in sizes {
        let count = fragmenter.calculate_frame_count(size);
        println!("Message size: {:>10} bytes -> {:>3} frames", size, count);
    }

    // Example 5: Check if fragmentation is needed
    println!("\nExample 5: Checking fragmentation requirements");
    let small_msg = Message::text("Hello");
    let large_msg = Message::text("A".repeat(100 * 1024));

    println!("Small message needs fragmentation: {}",
        fragmenter.needs_fragmentation(&small_msg));
    println!("Large message needs fragmentation: {}",
        fragmenter.needs_fragmentation(&large_msg));

    // Example 6: Timeout handling
    println!("\nExample 6: Fragment timeout handling");
    let short_timeout = Duration::from_millis(100);
    let reassembler = Reassembler::new(short_timeout);

    // Start a fragmented message
    let frames = fragmenter.split_message(&Message::text("A".repeat(100 * 1024)));
    let first_frame = &frames[0];

    match reassembler.ingest_frame(first_frame, 1) {
        Ok(None) => println!("First frame received, waiting for more..."),
        _ => println!("Unexpected result"),
    }

    println!("Waiting for timeout...");
    std::thread::sleep(short_timeout + Duration::from_millis(50));

    println!("In-progress reassemblies after timeout: {}",
        reassembler.in_progress_count());

    println!("\n=== Examples Complete ===");
}
