//! Lightweight TCP server for injecting synthetic input events into the UI.
//!
//! Used for automated testing without moving the real cursor.
//! Listens on 127.0.0.1:19876 and accepts newline-delimited JSON commands:
//!   {"action":"click","x":100,"y":200}
//!   {"action":"double_click","x":100,"y":200}
//!   {"action":"right_click","x":100,"y":200}

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// A synthetic input event to inject into the egui event loop.
#[derive(Debug, Clone)]
pub enum TestInput {
    Click { x: f32, y: f32 },
    DoubleClick { x: f32, y: f32 },
    RightClick { x: f32, y: f32 },
}

/// Thread-safe queue of pending test inputs.
pub type TestInputQueue = Arc<Mutex<Vec<TestInput>>>;

/// Create an empty input queue.
pub fn create_queue() -> TestInputQueue {
    Arc::new(Mutex::new(Vec::new()))
}

/// Start the test server on a background thread.
///
/// Listens on `127.0.0.1:19876` and pushes parsed commands into `queue`.
pub fn start_server(queue: TestInputQueue) {
    std::thread::spawn(move || {
        let listener = match TcpListener::bind("127.0.0.1:19876") {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("Test server failed to bind: {}", e);
                return;
            }
        };
        tracing::info!("Test input server listening on 127.0.0.1:19876");

        for stream in listener.incoming() {
            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Test server accept error: {}", e);
                    continue;
                }
            };

            let queue = queue.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(stream);
                for line in reader.lines() {
                    let line = match line {
                        Ok(l) => l,
                        Err(_) => break,
                    };
                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    if let Some(input) = parse_command(&line) {
                        if let Ok(mut q) = queue.lock() {
                            q.push(input);
                        }
                    } else {
                        tracing::warn!("Unknown test command: {}", line);
                    }
                }
            });
        }
    });
}

/// Parse a JSON command line into a `TestInput`.
fn parse_command(line: &str) -> Option<TestInput> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let action = v.get("action")?.as_str()?;
    let x = v.get("x")?.as_f64()? as f32;
    let y = v.get("y")?.as_f64()? as f32;

    match action {
        "click" => Some(TestInput::Click { x, y }),
        "double_click" => Some(TestInput::DoubleClick { x, y }),
        "right_click" => Some(TestInput::RightClick { x, y }),
        _ => None,
    }
}
