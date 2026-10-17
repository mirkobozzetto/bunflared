//! Turns the shared app's local URLs into the share's: in a header, a page,
//! or a body as it streams, so what the app writes little by little (a chat
//! answering word by word, NDJSON, a long export) reaches the visitor as it
//! comes.

use std::borrow::Cow;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};
use std::task::{Context, Poll, ready};

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::Body as HttpBody;
use hyper::body::{Frame, Incoming};
use regex::bytes::{Captures, Regex};

use super::{Body, Ctx, mount};

static LOCAL_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://(?:localhost|127\.0\.0\.1):(\d+)").unwrap());
/// How a local URL starts. A chunk ending on the beginning of one keeps it
/// back until the next chunk tells.
const STARTS: [&[u8]; 4] = [
    b"http://localhost:",
    b"https://localhost:",
    b"http://127.0.0.1:",
    b"https://127.0.0.1:",
];

impl Ctx {
    pub(super) fn rewrite<'a>(&self, text: &'a [u8]) -> Cow<'a, [u8]> {
        LOCAL_URL.replace_all(text, |caps: &Captures| {
            match String::from_utf8_lossy(&caps[1]).parse::<u16>() {
                Ok(port) if port == self.main => Vec::new(),
                Ok(port) if self.others.contains(&port) => mount(port).into_bytes(),
                _ => caps[0].to_vec(),
            }
        })
    }
}

/// The body, rewritten chunk by chunk.
pub(super) fn stream(body: Incoming, ctx: Arc<Ctx>) -> Body {
    Stream {
        body,
        ctx,
        held: Vec::new(),
        done: false,
    }
    .boxed()
}

struct Stream {
    body: Incoming,
    ctx: Arc<Ctx>,
    /// The end of what came, while it may be a local URL cut in two.
    held: Vec<u8>,
    done: bool,
}

impl HttpBody for Stream {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, hyper::Error>>> {
        while !self.done {
            match ready!(Pin::new(&mut self.body).poll_frame(cx)) {
                Some(Ok(frame)) => {
                    // Trailers are dropped, as when the whole body was read.
                    let Ok(data) = frame.into_data() else {
                        continue;
                    };
                    self.held.extend_from_slice(&data);
                    let at = cut(&self.held);
                    if at > 0 {
                        let rest = self.held.split_off(at);
                        let ready = std::mem::replace(&mut self.held, rest);
                        return Poll::Ready(Some(Ok(self.chunk(&ready))));
                    }
                }
                Some(Err(err)) => return Poll::Ready(Some(Err(err))),
                None => self.done = true,
            }
        }
        if self.held.is_empty() {
            return Poll::Ready(None);
        }
        let last = std::mem::take(&mut self.held);
        Poll::Ready(Some(Ok(self.chunk(&last))))
    }
}

impl Stream {
    fn chunk(&self, text: &[u8]) -> Frame<Bytes> {
        Frame::data(Bytes::from(self.ctx.rewrite(text).into_owned()))
    }
}

/// How much of `text` can go now: past it may sit a local URL whose end is
/// still to come.
fn cut(text: &[u8]) -> usize {
    match LOCAL_URL.find_iter(text).last() {
        // Its port may go on in the next chunk.
        Some(url) if url.end() == text.len() => url.start(),
        _ => (0..text.len())
            .find(|&at| STARTS.iter().any(|start| start.starts_with(&text[at..])))
            .unwrap_or(text.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::cut;

    #[test]
    fn holds_back_only_a_local_url_cut_in_two() {
        assert_eq!(cut(b"trois, lien http://local"), b"trois, lien ".len());
        assert_eq!(cut(b"lien http://localhost:80"), b"lien ".len());
        assert_eq!(cut(b"https://127.0.0.1:"), 0);
        for whole in [&b"un\n"[..], b"lien http://localhost:8765/doc\n", b"h1 {}"] {
            assert_eq!(cut(whole), whole.len());
        }
    }
}
