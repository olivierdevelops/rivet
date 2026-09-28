//! The single serve listener (axum on hyper, ADR-0002) plus the outbound side
//! of a WebSocket connection.
//!
//! ```text
//!  bind(127.0.0.1:8080) ─▶ TcpListener ─┬─ mount http  ─▶ Router.merge
//!                                       ├─ mount sse / poll / ws / mcp
//!                                       └─ run(): axum::serve(listener, app) until shutdown
//! ```

use crate::domain::errors::ErrorKind;
use crate::domain::ports::{ServeListener, WsConnection};
use crate::domain::serve::{ListenerHandle, MountReceipt, ServeConfig, SurfaceMount, WsFrame};
use crate::domain::{RivetError, RivetResult};
use async_trait::async_trait;
use axum::Router;
use futures_util::StreamExt;
use futures_util::stream::{BoxStream, SelectAll};
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Mutex;
use tokio::sync::mpsc;

// vhco:infra serve_listener satisfies ServeListener, WsConnection
// vhco:net listen tcp --listen HOST:PORT -- the one socket every serve surface shares (default 127.0.0.1:8080)
pub struct AxumListener {
    listener: Option<tokio::net::TcpListener>,
    surfaces: HashMap<String, Router>,
    app: Router,
    fallback: Option<Router>,
}

impl AxumListener {
    /// `surfaces` maps a surface name to its prebuilt routes; `fallback`
    /// answers every unmounted route (404 error ResponseEnvelope).
    pub fn new(surfaces: Vec<(String, Router)>, fallback: Router) -> AxumListener {
        AxumListener {
            listener: None,
            surfaces: surfaces.into_iter().collect(),
            app: Router::new(),
            fallback: Some(fallback),
        }
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.listener.as_ref().and_then(|l| l.local_addr().ok())
    }

    /// Serve until `shutdown` resolves.
    pub async fn run(
        mut self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> RivetResult<()> {
        let listener = self
            .listener
            .take()
            .ok_or_else(|| RivetError::internal("serve listener was never bound"))?;
        let mut app = self.app;
        if let Some(f) = self.fallback.take() {
            app = app.fallback_service(f);
        }
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(|e| RivetError::new(ErrorKind::Connection, "connection.serve", e.to_string()))
    }
}

#[async_trait]
impl ServeListener for AxumListener {
    async fn bind(&mut self, config: ServeConfig) -> RivetResult<ListenerHandle> {
        let l = tokio::net::TcpListener::bind(&config.listen)
            .await
            .map_err(|e| {
                RivetError::new(
                    ErrorKind::Connection,
                    "connection.bind",
                    format!("cannot listen on {}: {e}", config.listen),
                )
            })?;
        let addr = l
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or(config.listen.clone());
        self.listener = Some(l);
        Ok(ListenerHandle {
            listen_addr: addr,
            loopback: config.loopback,
        })
    }

    async fn mount(&mut self, mount: SurfaceMount) -> RivetResult<MountReceipt> {
        if self.listener.is_none() {
            return Err(RivetError::internal("mount before bind"));
        }
        let Some(router) = self.surfaces.remove(&mount.surface) else {
            return Err(RivetError::internal(format!(
                "no routes registered for surface `{}`",
                mount.surface
            )));
        };
        let app = std::mem::take(&mut self.app);
        self.app = app.merge(router);
        Ok(MountReceipt {
            surface: mount.surface,
            mounted: true,
        })
    }
}

/// Frames buffered per ref (and for immediate replies) before the sender waits.
pub const WS_LANE_FRAMES: usize = 16;

enum Lane {
    Open(mpsc::Sender<String>),
    /// The ref's terminal frame was sent; later frames are discarded.
    Ended,
}

/// Outbound frames of one WebSocket connection. Each in-flight ref writes to
/// its own bounded lane of [`WS_LANE_FRAMES`] frames and replies use a control
/// lane, so a slow ref back-pressures only its own producer:
///
/// ```text
///  ref c1 pump ─▶ lane c1 (16) ─┐
///  ref c2 pump ─▶ lane c2 (16) ─┼─▶ WsFrames (fair merge) ─▶ socket writer
///  replies     ─▶ control (16) ─┘
/// ```
pub struct WsOutbox {
    control: mpsc::Sender<String>,
    lanes: Mutex<HashMap<String, Lane>>,
    new_lanes: mpsc::UnboundedSender<mpsc::Receiver<String>>,
}

/// The merged frame stream the socket writer drains.
pub struct WsFrames {
    lanes: SelectAll<BoxStream<'static, String>>,
    new_lanes: mpsc::UnboundedReceiver<mpsc::Receiver<String>>,
    accepting: bool,
}

fn lane_stream(rx: mpsc::Receiver<String>) -> BoxStream<'static, String> {
    futures_util::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|m| (m, rx)) })
        .boxed()
}

impl WsFrames {
    /// The next frame from any lane; `None` once the outbox is gone and every lane drained.
    pub async fn next(&mut self) -> Option<String> {
        loop {
            tokio::select! {
                lane = self.new_lanes.recv(), if self.accepting => match lane {
                    Some(rx) => self.lanes.push(lane_stream(rx)),
                    None => self.accepting = false,
                },
                Some(text) = self.lanes.next() => return Some(text),
                else => return None,
            }
        }
    }
}

fn closed() -> RivetError {
    RivetError::new(
        ErrorKind::Cancelled,
        "cancelled.socket",
        "the WebSocket closed",
    )
}

impl WsOutbox {
    pub fn new() -> (WsOutbox, WsFrames) {
        let (control, control_rx) = mpsc::channel(WS_LANE_FRAMES);
        let (new_lanes, new_lanes_rx) = mpsc::unbounded_channel();
        let mut lanes = SelectAll::new();
        lanes.push(lane_stream(control_rx));
        (
            WsOutbox {
                control,
                lanes: Mutex::new(HashMap::new()),
                new_lanes,
            },
            WsFrames {
                lanes,
                new_lanes: new_lanes_rx,
                accepting: true,
            },
        )
    }

    /// A new in-flight ref gets a fresh lane (a reused ref name starts over).
    pub fn open_ref(&self, r: &str) {
        let (tx, rx) = mpsc::channel(WS_LANE_FRAMES);
        if let Ok(mut m) = self.lanes.lock() {
            m.insert(r.to_string(), Lane::Open(tx));
        }
        let _ = self.new_lanes.send(rx);
    }

    /// Forget a finished ref's lane (its pump ended).
    pub fn release_ref(&self, r: &str) {
        if let Ok(mut m) = self.lanes.lock() {
            m.remove(r);
        }
    }
}

#[async_trait]
impl WsConnection for WsOutbox {
    async fn send(&self, frame: WsFrame) -> RivetResult<()> {
        let terminal = frame.is_terminal();
        let lane = {
            let mut m = self.lanes.lock().map_err(|_| closed())?;
            match m.get_mut(&frame.r#ref) {
                Some(Lane::Ended) => return Ok(()),
                Some(Lane::Open(tx)) if terminal => {
                    let tx = tx.clone();
                    m.insert(frame.r#ref.clone(), Lane::Ended);
                    Some(tx)
                }
                Some(Lane::Open(tx)) => Some(tx.clone()),
                None => None,
            }
        };
        let text = frame.to_json().to_string();
        match lane {
            Some(tx) => tx.send(text).await.map_err(|_| closed()),
            None => self.control.send(text).await.map_err(|_| closed()),
        }
    }

    async fn reply(&self, frame: WsFrame) -> RivetResult<()> {
        self.control
            .send(frame.to_json().to_string())
            .await
            .map_err(|_| closed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Value;
    use std::time::Duration;

    // vhco:test serve.multiplex_ws -- G6: each ref has its own 16-frame lane: a full lane blocks only its ref, other refs and replies still flow; the first terminal frame closes the lane so a later terminal frame for that ref is discarded
    #[tokio::test]
    async fn per_ref_lanes_backpressure_and_single_terminal() {
        let (out, mut frames) = WsOutbox::new();
        out.open_ref("a");
        out.open_ref("b");
        for i in 0..WS_LANE_FRAMES as u64 {
            out.send(WsFrame::data("a", i + 1, Value::Int(i as i64)))
                .await
                .unwrap();
        }
        let blocked = tokio::time::timeout(
            Duration::from_millis(100),
            out.send(WsFrame::data("a", 17, Value::Int(17))),
        )
        .await;
        assert!(blocked.is_err(), "the 17th frame of ref a must wait");
        tokio::time::timeout(
            Duration::from_millis(100),
            out.send(WsFrame::data("b", 1, Value::Int(1))),
        )
        .await
        .expect("ref b is not blocked by ref a")
        .unwrap();
        out.send(WsFrame::error(
            "b",
            RivetError::new(ErrorKind::Conflict, "conflict.input_sequence", "x"),
        ))
        .await
        .unwrap();
        out.send(WsFrame::error("b", RivetError::internal("second terminal")))
            .await
            .unwrap();
        out.reply(WsFrame::error("zz", RivetError::internal("reply")))
            .await
            .unwrap();
        drop(out);
        let mut b = Vec::new();
        let mut a = 0;
        let mut replies = 0;
        while let Some(text) = frames.next().await {
            let j: serde_json::Value = serde_json::from_str(&text).unwrap();
            match j["ref"].as_str().unwrap() {
                "a" => a += 1,
                "b" => b.push(format!(
                    "{}{}",
                    j["type"].as_str().unwrap(),
                    j["status"]
                        .as_str()
                        .map(|s| format!(":{s}"))
                        .unwrap_or_default()
                )),
                _ => replies += 1,
            }
        }
        assert_eq!(a, WS_LANE_FRAMES);
        assert_eq!(
            b,
            vec!["data", "result:error"],
            "exactly one terminal record for b"
        );
        assert_eq!(replies, 1);
    }
}
