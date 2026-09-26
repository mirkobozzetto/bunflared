use std::borrow::Cow;
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::Instant;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full};
use hyper::body::Incoming;
use hyper::header::{self, HeaderMap, HeaderValue};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use regex::{Captures, Regex};
use tokio::net::{TcpListener, TcpStream};

use crate::live::Hub;
use crate::share::{Event, Hit, Tx};
use crate::widget;

pub type Body = BoxBody<Bytes, hyper::Error>;
type Error = Box<dyn std::error::Error + Send + Sync>;

static LOCAL_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://(?:localhost|127\.0\.0\.1):(\d+)").unwrap());
static TEXT_TYPES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"javascript|json|html|css|text/plain").unwrap());

const HOP_BY_HOP: [&str; 6] = [
    "connection",
    "keep-alive",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];
const BUNNY_PAGE: &str = include_str!("bunny-down.html");
// Not 502: Cloudflare swaps an origin's 502 page for its own.
const DOWN: StatusCode = StatusCode::SERVICE_UNAVAILABLE;

struct Ctx {
    main: u16,
    others: Vec<u16>,
    tx: Tx,
    /// Where feedback lands; `None` keeps the widget out of the pages.
    feedback: Option<PathBuf>,
    hub: Arc<Hub>,
}

pub fn mount(port: u16) -> String {
    format!("/_port/{port}")
}

pub fn routes(ports: &[u16]) -> BTreeMap<String, u16> {
    let mut routes = BTreeMap::from([("/".to_string(), ports[0])]);
    routes.extend(ports[1..].iter().map(|&port| (mount(port), port)));
    routes
}

pub async fn start(
    ports: &[u16],
    tx: Tx,
    feedback: Option<PathBuf>,
    hub: Arc<Hub>,
) -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let ctx = Arc::new(Ctx {
        main: ports[0],
        others: ports[1..].to_vec(),
        tx,
        feedback,
        hub,
    });
    tokio::spawn(async move {
        loop {
            let Ok((stream, peer)) = listener.accept().await else {
                continue;
            };
            let ctx = ctx.clone();
            tokio::spawn(async move {
                let service = service_fn(move |req| handle(req, ctx.clone(), peer));
                let _ = http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .with_upgrades()
                    .await;
            });
        }
    });
    Ok(port)
}

impl Ctx {
    fn target(&self, uri: &hyper::Uri) -> (u16, String) {
        let path = uri.path();
        let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
        for &port in &self.others {
            let prefix = mount(port);
            if let Some(rest) = path.strip_prefix(&prefix)
                && (rest.is_empty() || rest.starts_with('/'))
            {
                let rest = if rest.is_empty() { "/" } else { rest };
                return (port, format!("{rest}{query}"));
            }
        }
        (self.main, format!("{path}{query}"))
    }

    fn rewrite<'a>(&self, text: &'a str) -> Cow<'a, str> {
        LOCAL_URL.replace_all(text, |caps: &Captures| match caps[1].parse::<u16>() {
            Ok(port) if port == self.main => String::new(),
            Ok(port) if self.others.contains(&port) => mount(port),
            _ => caps[0].to_string(),
        })
    }

    async fn adapt(&self, response: Response<Incoming>, method: &Method) -> Response<Body> {
        let (mut parts, body) = response.into_parts();
        strip_hop_by_hop(&mut parts.headers);
        if let Some(location) = parts
            .headers
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            && let Ok(value) = HeaderValue::from_str(&self.rewrite(location))
        {
            parts.headers.insert(header::LOCATION, value);
        }
        let content_type = parts
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok());
        let page = self.feedback.is_some() && content_type.is_some_and(|t| t.contains("text/html"));
        let textual = content_type.is_some_and(|t| TEXT_TYPES.is_match(t))
            && !parts.headers.contains_key(header::CONTENT_ENCODING)
            && method != Method::HEAD;
        if !textual {
            return Response::from_parts(parts, body.boxed());
        }
        let Ok(collected) = body.collect().await else {
            return plain(DOWN, "The shared app stopped mid-answer.");
        };
        let bytes = collected.to_bytes();
        let bytes = match std::str::from_utf8(&bytes).map(|text| self.rewrite(text)) {
            Ok(text) if page => Bytes::from(widget::inject(&text)),
            Ok(Cow::Owned(text)) => Bytes::from(text),
            _ => bytes,
        };
        parts.headers.remove(header::CONTENT_LENGTH);
        Response::from_parts(parts, full(bytes))
    }
}

async fn handle(
    mut request: Request<Incoming>,
    ctx: Arc<Ctx>,
    peer: SocketAddr,
) -> Result<Response<Body>, Infallible> {
    if let Some(folder) = &ctx.feedback
        && request.uri().path().starts_with(widget::PREFIX)
    {
        return Ok(widget::handle(request, folder, &ctx.tx, &ctx.hub).await);
    }
    let started = Instant::now();
    let (port, path) = ctx.target(request.uri());
    let method = request.method().clone();
    let visitor = visitor(request.headers(), peer);
    let wants_html = accepts_html(request.headers());
    let upgrade = request.headers().contains_key(header::UPGRADE);
    let client_side = upgrade.then(|| hyper::upgrade::on(&mut request));

    let response = match forward(request, port, &path, upgrade).await {
        Ok(mut response) if upgrade && response.status() == StatusCode::SWITCHING_PROTOCOLS => {
            let local_side = hyper::upgrade::on(&mut response);
            if let Some(client_side) = client_side {
                tokio::spawn(async move {
                    if let (Ok(client), Ok(local)) = (client_side.await, local_side.await) {
                        let _ = tokio::io::copy_bidirectional(
                            &mut TokioIo::new(client),
                            &mut TokioIo::new(local),
                        )
                        .await;
                    }
                });
            }
            let (parts, _) = response.into_parts();
            Response::from_parts(parts, Empty::new().map_err(|never| match never {}).boxed())
        }
        Ok(response) => ctx.adapt(response, &method).await,
        Err(_) if wants_html => html(DOWN, BUNNY_PAGE.replace("{port}", &port.to_string())),
        Err(_) => plain(DOWN, "The shared app is not answering on this computer."),
    };

    let _ = ctx.tx.send(Event::Request(Hit {
        method: method.to_string(),
        path,
        status: response.status().as_u16(),
        ms: started.elapsed().as_millis().min(u32::MAX as u128) as u32,
        port,
        visitor,
        upgrade,
    }));
    Ok(response)
}

async fn forward(
    request: Request<Incoming>,
    port: u16,
    path: &str,
    upgrade: bool,
) -> Result<Response<Incoming>, Error> {
    let stream = TcpStream::connect(("localhost", port)).await?;
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;
    tokio::spawn(async move {
        let _ = connection.with_upgrades().await;
    });
    let (mut parts, body) = request.into_parts();
    parts.uri = path.parse()?;
    if !upgrade {
        strip_hop_by_hop(&mut parts.headers);
    }
    // Vite and friends reject unknown hosts; the local server must see itself.
    parts.headers.insert(
        header::HOST,
        HeaderValue::from_str(&format!("localhost:{port}"))?,
    );
    parts.headers.insert(
        header::ACCEPT_ENCODING,
        HeaderValue::from_static("identity"),
    );
    Ok(sender
        .send_request(Request::from_parts(parts, body))
        .await?)
}

fn strip_hop_by_hop(headers: &mut HeaderMap) {
    for name in HOP_BY_HOP {
        headers.remove(name);
    }
}

fn visitor(headers: &HeaderMap, peer: SocketAddr) -> String {
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    header("cf-connecting-ip")
        .or_else(|| header("x-forwarded-for").and_then(|v| v.split(',').next()))
        .map(|ip| ip.trim().to_string())
        .unwrap_or_else(|| peer.ip().to_string())
}

fn accepts_html(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/html"))
}

pub fn full(bytes: impl Into<Bytes>) -> Body {
    Full::new(bytes.into())
        .map_err(|never| match never {})
        .boxed()
}

fn plain(status: StatusCode, text: &'static str) -> Response<Body> {
    let mut response = Response::new(full(text));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

fn html(status: StatusCode, page: String) -> Response<Body> {
    let mut response = Response::new(full(page));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response
}
