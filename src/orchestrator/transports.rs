//! Wires the transports feature into the interpreter: each effect kind gets
//! an infra adapter with its use case injected as a closure.
//!
//! ```text
//!  http ──────────────▶ HttpEffects    ── exchange_http  ──▶ HyperClient + StdCodec
//!  tcp | unix | websocket ▶ SocketEffects ── exchange_socket ─▶ Dialer
//!  command ───────────▶ ProcessEffects ── run_process    ──▶ TokioRunner (+ sandbox backend)
//! ```

use crate::domain::ports::{FileAccess, PolicyEvaluator};
use crate::domain::transports::{
    Codec, HttpClient, HttpExchange, ProcessPlan, ProcessRunner, SocketPlan, SocketStream,
};
use crate::features::transports::exchange_http::exchange_http;
use crate::features::transports::exchange_socket::exchange_socket;
use crate::features::transports::run_process::run_process;
use crate::infra::execution_driver::Interpreter;
use crate::infra::http_adapter::{ExchangeHttpFn, HttpEffects};
use crate::infra::process_adapter::{ProcessEffects, RunProcessFn};
use crate::infra::socket_adapter::{ExchangeSocketFn, SocketEffects};
use std::sync::Arc;

/// Register `http`, `tcp`, `unix`, `websocket` and `command`.
pub fn register(interp: &mut Interpreter, files: Arc<dyn FileAccess>, root: &str) {
    let http: Arc<ExchangeHttpFn> = Arc::new(
        |x: HttpExchange, ev: &dyn PolicyEvaluator, client: &dyn HttpClient, codec: &dyn Codec| {
            Box::pin(exchange_http(x, ev, client, codec))
        },
    );
    interp.register_adapter("http", Arc::new(HttpEffects::new(http, Arc::clone(&files))));

    let socket: Arc<ExchangeSocketFn> = Arc::new(
        |p: SocketPlan, ev: &dyn PolicyEvaluator, s: &dyn SocketStream| {
            Box::pin(exchange_socket(p, ev, s))
        },
    );
    let sockets = Arc::new(SocketEffects::new(socket, files));
    interp.register_adapter("tcp", sockets.clone());
    interp.register_adapter("unix", sockets.clone());
    interp.register_adapter("websocket", sockets);

    let process: Arc<RunProcessFn> = Arc::new(
        |p: ProcessPlan, ev: &dyn PolicyEvaluator, r: &dyn ProcessRunner, c: &dyn Codec| {
            Box::pin(run_process(p, ev, r, c))
        },
    );
    interp.register_adapter("command", Arc::new(ProcessEffects::new(process, root)));
}
