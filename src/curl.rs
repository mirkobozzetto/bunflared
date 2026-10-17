//! A request of the inspector as a curl command, sent to the local server.

use crate::event::Hit;

/// Left out: curl sets them itself, or they only meant something on the way in.
const SKIPPED: [&str; 8] = [
    "host",
    "content-length",
    "connection",
    "keep-alive",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// The command, or why there is none.
pub fn curl(hit: &Hit) -> Result<String, String> {
    if let Some(reason) = hit.unreplayable() {
        return Err(reason);
    }
    let exchange = &hit.exchange;
    let body = exchange.request_body.lock().unwrap();
    let text = std::str::from_utf8(&body.bytes).map_err(|_| "Its body is not text.".to_string())?;
    let url = format!("http://localhost:{}{}", hit.port, hit.path);
    let mut parts = vec![format!("curl -X {} {}", hit.method, quote(&url))];
    for (name, value) in &exchange.request_headers {
        if !SKIPPED.contains(&name.as_str()) {
            let value = String::from_utf8_lossy(value.as_bytes());
            parts.push(format!("-H {}", quote(&format!("{name}: {value}"))));
        }
    }
    if !text.is_empty() {
        parts.push(format!("--data-binary {}", quote(text)));
    }
    Ok(parts.join(" \\\n  "))
}

/// Between single quotes, which the shell takes as is.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}
