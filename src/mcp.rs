//! Model Context Protocol server over standard input and output.
//!
//! Speaks JSON-RPC 2.0, one message per line, and exposes the board through the
//! tools of `automation::Session`. The generator tools are listed only when the
//! session allows them, and they can only set the tracking-source frequency: the
//! output itself is switched by the S1 button on the board.

use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

use crate::automation::Session;

const PROTOCOL_VERSION: &str = "2025-06-18";

const INSTRUCTIONS: &str = "Controls an LTDZ 35-4400M spectrum analyzer board (35 MHz to 4.4 GHz). \
Levels are raw detector counts; dbm is filled only where a calibration covers the reading, otherwise null. \
The board input takes at most +5 dBm: never ask the user to connect a strong signal without an attenuator. \
A sweep takes about 1.3 ms per point.";

fn tools(allow_generator: bool) -> Vec<Value> {
    let freq = json!({"type": "integer", "description": "Frequency in Hz, 35000000 to 4400000000"});
    let mut list = vec![
        json!({
            "name": "list_ports",
            "description": "List the serial ports and mark the one that looks like the board.",
            "inputSchema": {"type": "object", "properties": {}},
        }),
        json!({
            "name": "status",
            "description": "Show the session: source, calibration, limits and whether the generator tools are allowed. Does not touch the board.",
            "inputSchema": {"type": "object", "properties": {}},
        }),
        json!({
            "name": "sweep",
            "description": "Run one sweep and return frequencies, raw detector values, dBm (null when not calibrated) and the highest point. At most 9999 points; the step cannot exceed 9.99 MHz (the full band needs 438 points).",
            "inputSchema": {"type": "object", "properties": {
                "start": {"type": "integer", "description": "Start frequency in Hz (default 35000000)"},
                "stop": {"type": "integer", "description": "Stop frequency in Hz (default 1000000000)"},
                "points": {"type": "integer", "description": "Number of points (default 101)"},
            }},
        }),
        json!({
            "name": "measure",
            "description": "Read the level at one frequency once. The output is stopped afterwards.",
            "inputSchema": {"type": "object", "properties": {"freq_hz": freq}, "required": ["freq_hz"]},
        }),
    ];
    if allow_generator {
        list.push(json!({
            "name": "generator",
            "description": "Set the tracking-source frequency and turn the output on at that frequency, even if the S1 button is off. Ask the user what is connected to RF OUT before using it, and call generator_off when done.",
            "inputSchema": {"type": "object", "properties": {"freq_hz": freq}, "required": ["freq_hz"]},
        }));
        list.push(json!({
            "name": "generator_off",
            "description": "Stop the tracking-source frequency.",
            "inputSchema": {"type": "object", "properties": {}},
        }));
    }
    list
}

fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Answers one JSON-RPC message; `None` for notifications, which get no reply.
pub fn handle_message(session: &mut Session, message: &Value) -> Option<Value> {
    let id = message.get("id")?.clone();
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let result = match method {
        "initialize" => json!({
            "protocolVersion": message["params"]["protocolVersion"]
                .as_str()
                .unwrap_or(PROTOCOL_VERSION),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "pakespec-mcp", "version": env!("CARGO_PKG_VERSION")},
            "instructions": INSTRUCTIONS,
        }),
        "ping" => json!({}),
        "tools/list" => json!({"tools": tools(session.generator_allowed())}),
        "tools/call" => {
            let params = &message["params"];
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(error(&id, -32602, "missing tool name"));
            };
            if !tools(session.generator_allowed())
                .iter()
                .any(|t| t["name"] == name)
            {
                return Some(error(&id, -32602, &format!("unknown tool {name:?}")));
            }
            let mut request = match params.get("arguments") {
                Some(Value::Object(map)) => Value::Object(map.clone()),
                _ => json!({}),
            };
            request["cmd"] = json!(name);
            let reply = session.execute(&request);
            json!({
                "content": [{"type": "text", "text": reply.to_string()}],
                "isError": reply["ok"] != true,
            })
        }
        _ => return Some(error(&id, -32601, "method not found")),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// Serves messages, one per line, until the input ends.
pub fn serve(session: &mut Session, input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle_message(session, &message),
            Err(_) => Some(error(&Value::Null, -32700, "parse error")),
        };
        if let Some(reply) = reply {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::Source;

    fn run(allow_generator: bool, lines: &[Value]) -> Vec<Value> {
        let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let mut out = Vec::new();
        let mut s = Session::new(Source::Simulator, None, allow_generator);
        serve(&mut s, input.as_bytes(), &mut out).unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn handshake_listing_and_call() {
        let out = run(
            false,
            &[
                json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26"}}),
                json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
                json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
                json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "measure", "arguments": {"freq_hz": 940_000_000u64}}}),
            ],
        );
        assert_eq!(out.len(), 3, "the notification gets no reply");
        assert_eq!(out[0]["result"]["protocolVersion"], "2025-03-26");
        let names: Vec<&str> = out[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["list_ports", "status", "sweep", "measure"]);
        assert_eq!(out[2]["result"]["isError"], false);
        let text = out[2]["result"]["content"][0]["text"].as_str().unwrap();
        assert!(serde_json::from_str::<Value>(text).unwrap()["raw"].is_number());
    }

    #[test]
    fn generator_tools_exist_only_when_allowed() {
        let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
        let count = |allow| {
            run(allow, std::slice::from_ref(&list))[0]["result"]["tools"]
                .as_array()
                .unwrap()
                .len()
        };
        assert_eq!(count(true), count(false) + 2);
        let call = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "generator", "arguments": {"freq_hz": 100_000_000u64}}});
        assert_eq!(run(false, &[call])[0]["error"]["code"], -32602);
    }

    #[test]
    fn tool_errors_are_results_with_is_error() {
        let call = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "sweep", "arguments": {"start": 1}}});
        let out = run(false, &[call]);
        assert_eq!(out[0]["result"]["isError"], true);
    }

    #[test]
    fn unknown_method_and_bad_json_get_errors() {
        let out = run(
            false,
            &[json!({"jsonrpc": "2.0", "id": 9, "method": "nope"})],
        );
        assert_eq!(out[0]["error"]["code"], -32601);
    }
}
