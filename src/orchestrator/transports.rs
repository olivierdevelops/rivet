//! Wires the transports feature into the interpreter: each effect kind gets
//! an infra adapter with its use case injected as a closure.
//!
//! ```text
//!  http ──────────────▶ HttpEffects    ── exchange_http  ──▶ HyperClient + StdCodec
//!        └ auth P account A ─▶ AuthorizedCredentials (auth.acquire_credential ─▶ OAuthAdapter)
//!  tcp | unix | websocket ▶ SocketEffects ── exchange_socket ─▶ Dialer
//!  command ───────────▶ ProcessEffects ── run_process    ──▶ TokioRunner (+ sandbox backend)
//! ```

use crate::domain::ports::{CredentialProvider, FileAccess, PolicyEvaluator};
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

/// The `transports.exchange_http` use case as the closure adapters receive
/// (HTTP effects and the OAuth adapter's token/device requests).
pub fn exchange_http_fn() -> Arc<ExchangeHttpFn> {
    Arc::new(
        |x: HttpExchange, ev: &dyn PolicyEvaluator, client: &dyn HttpClient, codec: &dyn Codec| {
            Box::pin(exchange_http(x, ev, client, codec))
        },
    )
}

/// Register `http`, `tcp`, `unix`, `websocket` and `command`. `credentials`
/// (the authorized `auth.acquire_credential` provider) enables `auth P account A`.
pub fn register(
    interp: &mut Interpreter,
    files: Arc<dyn FileAccess>,
    root: &str,
    credentials: Option<Arc<dyn CredentialProvider>>,
) {
    interp.register_adapter(
        "http",
        Arc::new(
            HttpEffects::new(exchange_http_fn(), Arc::clone(&files)).with_credentials(credentials),
        ),
    );

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
