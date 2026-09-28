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
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
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
    /// answers every unmounted route (404 ErrorEnvelope).
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

/// Outbound frames of one WebSocket connection, serialized to JSON text and
/// handed to the socket writer task through a bounded channel.
pub struct WsOutbox {
    tx: mpsc::Sender<String>,
}

impl WsOutbox {
    pub fn new(tx: mpsc::Sender<String>) -> WsOutbox {
        WsOutbox { tx }
    }
}

#[async_trait]
impl WsConnection for WsOutbox {
    async fn send(&self, frame: WsFrame) -> RivetResult<()> {
        self.tx
            .send(frame.to_json().to_string())
            .await
            .map_err(|_| {
                RivetError::new(
                    ErrorKind::Cancelled,
                    "cancelled.socket",
                    "the WebSocket closed",
                )
            })
    }
}
