use std::time::Instant;

use crate::event::{Event, Failure};
use crate::proxy::mount;
use crate::state::Record;

pub struct PortState {
    pub port: u16,
    pub route: String,
    pub checked: Option<bool>,
    pub up: bool,
    pub hits: u32,
}

pub struct Qr {
    pub size: usize,
    pub dark: Vec<bool>,
}

/// The launch: the ports sniffed, the tunnel dug, the link that works.
pub struct Boot {
    pub ports: Vec<PortState>,
    pub url: Option<String>,
    pub record: Option<Record>,
    pub qr_code: Option<Qr>,
    pub resolved: bool,
    pub copied: bool,
    pub progress: f32,
    target: f32,
    pub stage: String,
    pub ready_at: Option<Instant>,
    pub failure: Option<Failure>,
}

impl Boot {
    pub fn new(ports: &[u16]) -> Self {
        let ports = ports
            .iter()
            .enumerate()
            .map(|(i, &port)| PortState {
                port,
                route: if i == 0 { "/".into() } else { mount(port) },
                checked: None,
                up: true,
                hits: 0,
            })
            .collect();
        Self {
            ports,
            url: None,
            record: None,
            qr_code: None,
            resolved: false,
            copied: false,
            progress: 0.0,
            target: 0.05,
            stage: "Sniffing around".into(),
            ready_at: None,
            failure: None,
        }
    }

    /// A step of the launch: a port answered, the tunnel moved, the link works.
    pub fn on(&mut self, event: Event, now: Instant) {
        match event {
            Event::PortChecked { port, ok } => {
                if let Some(state) = self.ports.iter_mut().find(|p| p.port == port) {
                    state.checked = Some(ok);
                    state.up = ok;
                }
            }
            Event::FetchingCloudflared => {
                self.target = self.target.max(0.08);
                self.stage = "Fetching cloudflared, first run only".into();
            }
            Event::TunnelStarting => {
                self.target = self.target.max(0.12);
                self.stage = "Waking up cloudflared".into();
            }
            Event::TunnelUrl(url) => {
                self.url = Some(url);
                self.target = self.target.max(0.35);
                self.stage = "Plugging into Cloudflare's edge".into();
            }
            Event::TunnelRegistered => {
                self.target = self.target.max(0.5);
                self.stage = "Waiting for DNS to notice".into();
            }
            Event::DnsAttempt(attempt) => {
                let dns = 0.5 + 0.45 * (1.0 - 0.85f32.powi(attempt as i32));
                self.target = self.target.max(dns);
                self.stage = format!("Waiting for DNS to notice, attempt {attempt}");
            }
            Event::Ready {
                record,
                resolved,
                copied,
            } => {
                self.url = Some(record.url.clone());
                self.qr_code = qr(&record.url);
                self.record = Some(record);
                self.resolved = resolved;
                self.copied = copied;
                self.ready_at = Some(now);
                self.target = 1.0;
            }
            _ => {}
        }
    }

    pub fn health(&mut self, port: u16, ok: bool) {
        if let Some(state) = self.ports.iter_mut().find(|p| p.port == port) {
            state.up = ok;
        }
    }

    pub fn hit(&mut self, port: u16) {
        if let Some(state) = self.ports.iter_mut().find(|p| p.port == port) {
            state.hits += 1;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if self.ready_at.is_none() {
            // Creep between milestones so the bar never looks stuck.
            self.target = (self.target + dt * 0.01).min(0.97);
        }
        self.progress += (self.target - self.progress) * (dt * 3.0).min(1.0);
    }
}

fn qr(url: &str) -> Option<Qr> {
    const QUIET: usize = 2;
    let code = qrcode::QrCode::with_error_correction_level(url, qrcode::EcLevel::L).ok()?;
    let width = code.width();
    let colors = code.to_colors();
    let size = width + QUIET * 2;
    let mut dark = vec![false; size * size];
    for y in 0..width {
        for x in 0..width {
            dark[(y + QUIET) * size + x + QUIET] = colors[y * width + x] == qrcode::Color::Dark;
        }
    }
    Some(Qr { size, dark })
}
