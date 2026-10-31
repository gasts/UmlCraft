//! MCP over stdio: JSON-RPC 2.0 messages, one per line, on stdin and stdout. Only what a tool
//! server needs: `initialize`, `ping`, `tools/list` and `tools/call`.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use crate::tools::{self, Reply};

/// The protocol versions this server speaks, newest first.
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// What runs the tools: the app or a headless project ([`crate::backend::Auto`]).
pub trait Backend {
    fn call(&mut self, name: &str, args: &Value) -> Reply;
}

/// Answers requests from `input` on `output` until `input` ends.
pub fn serve(
    backend: &mut dyn Backend,
    input: impl BufRead,
    mut output: impl Write,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let answers: Vec<Value> =
                    batch.iter().filter_map(|message| answer(backend, message)).collect();
                (!answers.is_empty()).then(|| Value::Array(answers))
            }
            Ok(message) => answer(backend, &message),
            Err(err) => Some(error(Value::Null, -32700, &format!("parse error: {err}"))),
        };
        if let Some(response) = response {
            writeln!(output, "{response}")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// The response to one message; none for notifications and responses.
fn answer(backend: &mut dyn Backend, message: &Value) -> Option<Value> {
    let id = message.get("id")?.clone();
    let method = message.get("method").and_then(Value::as_str)?;
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => {
            let asked = params["protocolVersion"].as_str().unwrap_or(VERSIONS[0]);
            let version = VERSIONS.iter().find(|v| **v == asked).unwrap_or(&VERSIONS[0]);
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": {
                    "name": "umlcraft",
                    "title": "UmlCraft",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "instructions": tools::INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools::list() }),
        "tools/call" => {
            let Some(name) = params["name"].as_str() else {
                return Some(error(id, -32602, "tools/call needs a tool name"));
            };
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            backend.call(name, &args).to_json()
        }
        _ => return Some(error(id, -32601, &format!("unknown method `{method}`"))),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{Access, Headless};

    struct Local(Headless);

    impl Backend for Local {
        fn call(&mut self, name: &str, args: &Value) -> Reply {
            tools::call(&mut self.0, &Access::default(), name, args)
        }
    }

    fn exchange(lines: &[Value]) -> Vec<Value> {
        let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let mut output = Vec::new();
        serve(&mut Local(Headless::new()), input.as_bytes(), &mut output).unwrap();
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn a_session_initializes_lists_and_calls() {
        let answers = exchange(&[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                    "params": { "protocolVersion": "2025-03-26", "capabilities": {},
                                "clientInfo": { "name": "test", "version": "1" } } }),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                    "params": { "name": "get_document", "arguments": {} } }),
            json!({ "jsonrpc": "2.0", "id": 4, "method": "nope" }),
        ]);
        // The notification gets no answer.
        assert_eq!(answers.len(), 4);
        assert_eq!(answers[0]["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(answers[0]["result"]["serverInfo"]["name"], "umlcraft");
        assert!(answers[1]["result"]["tools"].as_array().unwrap().len() > 20);
        assert_eq!(answers[2]["result"]["isError"], false);
        assert_eq!(answers[3]["error"]["code"], -32601);
    }
}
