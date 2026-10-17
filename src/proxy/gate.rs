//! `--password`: a page that asks for it, then a cookie that lets the
//! browser through. Every request passes here first, widget and sockets too.

use http_body_util::{BodyExt, Limited};
use hyper::body::Incoming;
use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};

use crate::http::{Body, html, plain};

const COOKIE: &str = "bunflared_pass";
/// Where the password page posts; under the widget prefix, out of the app's way.
const FORM: &str = "/_bunflared/password";
const PAGE: &str = include_str!("password.html");
const FORM_LIMIT: usize = 4096;
/// No 0/o or 1/l/i to confuse on a phone keyboard; 32 letters, so a byte
/// masked to 5 bits picks one without bias.
const ALPHABET: &[u8; 32] = b"abcdefghjkmnpqrstuvwxyz23456789-";
const GROUPS: usize = 3;
const GROUP_LEN: usize = 4;
const TOKEN_BYTES: usize = 32;

pub struct Gate {
    password: String,
    /// What the cookie carries, never the password itself.
    token: String,
}

/// A password easy to type on a phone, like `k7fp-2mxa-q9rt`.
pub fn generate() -> String {
    let letters = &ALPHABET[..31];
    let mut groups = Vec::with_capacity(GROUPS);
    while groups.len() < GROUPS {
        let mut group = String::with_capacity(GROUP_LEN);
        while group.len() < GROUP_LEN {
            let byte = random::<1>()[0] & 31;
            // The 32nd slot is the dash: drawn again to stay unbiased.
            if let Some(&c) = letters.get(byte as usize) {
                group.push(c as char);
            }
        }
        groups.push(group);
    }
    groups.join("-")
}

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the system has no random source");
    bytes
}

impl Gate {
    pub fn new(password: String) -> Self {
        let token = random::<TOKEN_BYTES>()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Self { password, token }
    }

    /// Hands the request back when its cookie opens the gate, or the answer
    /// to send instead: the page, its form's verdict, or a 401.
    pub async fn check(
        &self,
        request: Request<Incoming>,
    ) -> Result<Request<Incoming>, Box<Response<Body>>> {
        if self.opened(request.headers()) {
            return Ok(request);
        }
        if request.method() == Method::POST && request.uri().path() == FORM {
            return Err(Box::new(self.submit(request).await));
        }
        if request.method() != Method::GET || !super::accepts_html(request.headers()) {
            return Err(Box::new(plain(
                StatusCode::UNAUTHORIZED,
                "This share needs a password.",
            )));
        }
        let next = request
            .uri()
            .path_and_query()
            .map_or("/", |p| p.as_str())
            .to_string();
        Err(Box::new(page(&next, "")))
    }

    fn opened(&self, headers: &HeaderMap) -> bool {
        headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .filter_map(|pair| pair.trim().split_once('='))
            .any(|(name, value)| name == COOKIE && same(value, &self.token))
    }

    async fn submit(&self, request: Request<Incoming>) -> Response<Body> {
        let Ok(body) = Limited::new(request.into_body(), FORM_LIMIT)
            .collect()
            .await
        else {
            return plain(StatusCode::PAYLOAD_TOO_LARGE, "The form is too long.");
        };
        let body = body.to_bytes();
        let form = String::from_utf8_lossy(&body);
        let field = |name: &str| {
            form.split('&')
                .filter_map(|pair| pair.split_once('='))
                .find(|(key, _)| *key == name)
                .map(|(_, value)| decode(value))
                .unwrap_or_default()
        };
        let next = field("next");
        // Only a path of this share, never another site.
        let next = if next.starts_with('/') && !next.starts_with("//") && !next.contains('\\') {
            next
        } else {
            "/".into()
        };
        if !same(&field("password"), &self.password) {
            return page(&next, "Wrong password, try again.");
        }
        let mut response = plain(StatusCode::SEE_OTHER, "");
        let headers = response.headers_mut();
        if let Ok(location) = HeaderValue::from_str(&next) {
            headers.insert(header::LOCATION, location);
        }
        let cookie = format!(
            "{COOKIE}={}; Path=/; HttpOnly; Secure; SameSite=Lax",
            self.token
        );
        if let Ok(cookie) = HeaderValue::from_str(&cookie) {
            headers.insert(header::SET_COOKIE, cookie);
        }
        response
    }
}

fn page(next: &str, error: &str) -> Response<Body> {
    let page = PAGE
        .replace("{next}", &escape(next))
        .replace("{error}", error);
    let mut response = html(StatusCode::UNAUTHORIZED, page);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// Compares in time that does not depend on where the strings differ.
fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len()
        && std::hint::black_box(a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y))) == 0
}

/// `application/x-www-form-urlencoded` back to text.
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 2;
            }
            (b'+', _) => out.push(b' '),
            (byte, _) => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
