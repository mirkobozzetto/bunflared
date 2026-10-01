//! `bunflared mcp`: a Model Context Protocol server on stdio, one JSON-RPC
//! message per line. It runs shares of its own and gives the agent what the
//! dashboard gives a person.

mod host;

use std::io::{BufRead, Write};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::{os, widget};
use host::Host;

/// Revisions with an `initialize` handshake, the newest first.
const PROTOCOLS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const MAX_WAIT: u64 = 300;
const REQUESTS_SHOWN: usize = 20;

const INSTRUCTIONS: &str = "bunflared shares local ports with a widget in the pages: \
a feedback button and a live chat with whoever is on them. `open` starts a share, \
local by default (http://127.0.0.1:<port>, this computer only, no account); \
`public: true` gives a https link anyone can open instead. Hand its url to the user. \
`events` returns what visitors did since its `cursor`: pass the cursor back each time, \
and `wait` to hold the call until something happens instead of polling. Answer with \
`say`, send them to a page with `go`, `reload` after a fix. `requests`, `request` and \
`replay` inspect the traffic. With channels on, visitors' messages and notes also \
arrive by themselves as <channel source=\"bunflared\" kind=\"message|note\" share=... \
visitor=...>: answer with `say`, passing that share and visitor. Notes are Markdown \
files with a screenshot, in bunflared-feedback/. Every share closes when this session ends.";

pub fn run() -> i32 {
    let folder = std::env::current_dir()
        .ok()
        .map(|dir| dir.join(widget::FOLDER));
    let host = Host::new(folder);
    let signalled = host.clone();
    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        runtime.block_on(os::quit());
        signalled.close_all();
        std::process::exit(0);
    });
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        // A call can wait for visitors: the next ones must not queue behind it.
        let host = host.clone();
        std::thread::spawn(move || {
            if let Some(reply) = answer(&host, &line) {
                send(&reply);
            }
        });
    }
    host.close_all();
    0
}

/// Stdout carries protocol messages only, each on one line: JSON escapes
/// the newlines inside strings.
fn send(message: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{message}");
    let _ = out.flush();
}

fn notify(method: &str, params: Value) {
    send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
}

/// The reply to a request; notifications and responses get none.
fn answer(host: &Arc<Host>, line: &str) -> Option<Value> {
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        let error = json!({ "code": -32700, "message": "Parse error" });
        return Some(json!({ "jsonrpc": "2.0", "id": null, "error": error }));
    };
    let method = message.get("method")?.as_str()?;
    let id = message.get("id")?.clone();
    let params = message.get("params").cloned().unwrap_or(json!({}));
    let result = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => Ok(call(host, &params)),
        _ => Err(json!({ "code": -32601, "message": format!("Method not found: {method}") })),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    })
}

fn initialize(params: &Value) -> Value {
    let asked = params["protocolVersion"].as_str().unwrap_or("");
    let version = PROTOCOLS
        .into_iter()
        .find(|known| *known == asked)
        .unwrap_or(PROTOCOLS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": {
            "tools": {},
            "experimental": { "claude/channel": {} },
        },
        "serverInfo": { "name": "bunflared", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn call(host: &Arc<Host>, params: &Value) -> Value {
    let name = params["name"].as_str().unwrap_or("");
    let (text, failed) = match tool(host, name, &params["arguments"]) {
        Ok(value) => (
            serde_json::to_string_pretty(&value).unwrap_or_default(),
            false,
        ),
        Err(message) => (message, true),
    };
    json!({ "content": [{ "type": "text", "text": text }], "isError": failed })
}

fn tool(host: &Arc<Host>, name: &str, args: &Value) -> Result<Value, String> {
    let text = |key: &str| args[key].as_str();
    let share = || host.share(text("share"));
    let visitor = text("visitor");
    let n = || {
        args["n"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("Pass the request's number, n.")
    };
    match name {
        "open" => host.open(ports(args)?, args["public"].as_bool().unwrap_or(false)),
        "list" => Ok(host.list()),
        "close" => host.close(text("share").ok_or("Pass the id of the share to close.")?),
        "visitors" => Ok(share()?.visitors()),
        "say" => share()?.say(visitor, text("text").unwrap_or("")),
        "go" => share()?.go(visitor, text("path").unwrap_or("")),
        "reload" => share()?.reload(visitor),
        "events" => {
            let since = args["since"].as_u64().unwrap_or(0);
            let wait = args["wait"].as_u64().unwrap_or(0).min(MAX_WAIT);
            Ok(host.events(since, Duration::from_secs(wait), text("share")))
        }
        "requests" => {
            let limit = args["limit"]
                .as_u64()
                .map_or(REQUESTS_SHOWN, |n| n as usize);
            Ok(share()?.requests(limit))
        }
        "request" => share()?.request(n()?),
        "replay" => share()?.replay(n()?),
        _ => Err(format!("No tool named {name}.")),
    }
}

fn ports(args: &Value) -> Result<Vec<u16>, String> {
    let bad = || "Pass the ports to share, like [5173] or [5173, 3000].".to_string();
    let list = match &args["ports"] {
        Value::Array(list) => list.clone(),
        Value::Number(_) => vec![args["ports"].clone()],
        _ => return Err(bad()),
    };
    let ports: Option<Vec<u16>> = list
        .iter()
        .map(|port| {
            port.as_u64()
                .and_then(|p| u16::try_from(p).ok())
                .filter(|p| *p > 0)
        })
        .collect();
    ports.filter(|ports| !ports.is_empty()).ok_or_else(bad)
}

fn tools() -> Value {
    let share = json!({ "type": "string", "description": "The share's id; the last one opened when left out." });
    let visitor = json!({ "type": "string", "description": "One visitor's id, from visitors or an event; everyone when left out." });
    let n = json!({ "type": "integer", "description": "The request's number, from requests." });
    json!([
        {
            "name": "open",
            "description": "Share local ports, with the feedback widget and the live chat in their pages. Local unless public is true: http://127.0.0.1:<port>, for the user's own browser, ready in under a second. The first port is served at /, the others under /_port/<port>. Returns the share's id and url once it answers.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ports": { "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 65535 }, "minItems": 1 },
                    "public": { "type": "boolean", "description": "Only when someone on another device must open it, a client or a phone: a https link through a Cloudflare quick tunnel, which hands out a limited number of links. Default false." },
                },
                "required": ["ports"],
            },
        },
        {
            "name": "list",
            "description": "The shares this server runs: id, url, ports, visitors, requests.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "close",
            "description": "Stop one share.",
            "inputSchema": { "type": "object", "properties": { "share": share }, "required": ["share"] },
        },
        {
            "name": "visitors",
            "description": "Who is on a share now: each visitor's id, device, page and status.",
            "inputSchema": { "type": "object", "properties": { "share": share } },
        },
        {
            "name": "say",
            "description": "Show a chat message on the pages, as a bubble. It goes in the session log too.",
            "inputSchema": {
                "type": "object",
                "properties": { "text": { "type": "string" }, "visitor": visitor, "share": share },
                "required": ["text"],
            },
        },
        {
            "name": "go",
            "description": "Send the pages to a path of the shared app, like /pricing.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" }, "visitor": visitor, "share": share },
                "required": ["path"],
            },
        },
        {
            "name": "reload",
            "description": "Reload the pages, after a fix for instance.",
            "inputSchema": { "type": "object", "properties": { "visitor": visitor, "share": share } },
        },
        {
            "name": "events",
            "description": "What happened on the shares after a cursor: messages, notes (text, page, pointed element, Markdown file, screenshot), reactions, arrivals, departures, closed shares. Pass back the returned cursor next time.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "since": { "type": "integer", "minimum": 0, "description": "The cursor of the last call; 0 or left out for everything kept." },
                    "wait": { "type": "integer", "minimum": 0, "maximum": MAX_WAIT, "description": "Seconds to wait for the next event when there is none yet." },
                    "share": share,
                },
            },
        },
        {
            "name": "requests",
            "description": "The latest requests through a share, newest first: number, method, path, status, duration, visitor.",
            "inputSchema": {
                "type": "object",
                "properties": { "limit": { "type": "integer", "minimum": 1 }, "share": share },
            },
        },
        {
            "name": "request",
            "description": "One request in full: headers, and bodies kept up to 32 KiB.",
            "inputSchema": { "type": "object", "properties": { "n": n, "share": share }, "required": ["n"] },
        },
        {
            "name": "replay",
            "description": "Send a request again to the local server, as it was, and return the new status.",
            "inputSchema": { "type": "object", "properties": { "n": n, "share": share }, "required": ["n"] },
        },
    ])
}
