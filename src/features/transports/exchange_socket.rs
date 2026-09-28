use super::ports::{PolicyEvaluator, SocketStream};
use crate::domain::effect_checks::{authorize, checked_addr};
use crate::domain::policy::{AccessVerb, Capability, EffectTarget};
use crate::domain::transports::{SocketConnection, SocketPlan, SocketScheme};
use crate::domain::{RivetError, RivetResult};

// vhco:usecase transports.exchange_socket(input: SocketPlan) -> SocketConnection needs SocketStream, PolicyEvaluator
// vhco:label Exchange socket
// vhco:about Opens one scope-owned duplex connection (TCP, Unix socket or WebSocket) after refusing Stage C options, authorizing the endpoint (allow_network connect for tcp/ws/wss, allow_unix connect for a socket path) and checking every resolved address; framing and message exchange then run through the returned connection until the scope closes it.
// vhco:example input={scheme:"tcp", endpoint:"127.0.0.1:9000", framing:"newline"} => { "connection": "open" }
pub async fn exchange_socket(
    input: SocketPlan,
    evaluator: &dyn PolicyEvaluator,
    sockets: &dyn SocketStream,
) -> RivetResult<Box<dyn SocketConnection>> {
    // vhco:todo authorize_and_connect -- refuse Stage C first (TLS/mTLS on tcp or unix → unsupported.tcp_tls, `reconnect` → unsupported.reconnect) before any effect; then tcp → allow_network connect tcp://host:port, ws/wss → allow_network connect ws(s)://host:port/path, unix → allow_unix connect PATH; resolve names once and dial only a checked address (private results need a literal grant)
    // vhco:step stage_c guard -- Stage C options fail before authorization or I/O
    // vhco:error stage_c -- `tls` on tcp/unix or `reconnect` => unsupported.* (501, exit 5) with zero effects
    if input.tls_requested && matches!(input.scheme, SocketScheme::Tcp | SocketScheme::Unix) {
        return Err(RivetError::unsupported(
            "unsupported.tcp_tls",
            "TLS/mTLS over raw TCP or Unix sockets is Stage C and not available in this build",
        )
        .with_span(input.origin.span.clone()));
    }
    if input.reconnect {
        return Err(RivetError::unsupported(
            "unsupported.reconnect",
            "explicit socket reconnect is Stage C and not available in this build",
        )
        .with_span(input.origin.span.clone()));
    }
    // vhco:step authorize effect_checks::authorize -- one permit for the endpoint, then per resolved address
    // vhco:error permission_denied -- endpoint or resolved address not granted => permission.denied (403, exit 3) before dialing
    let addr = match input.scheme {
        SocketScheme::Unix => {
            authorize(
                evaluator,
                &input.origin,
                Capability::Unix,
                AccessVerb::Connect,
                EffectTarget::Path(input.endpoint.clone()),
            )?;
            None
        }
        SocketScheme::Tcp | SocketScheme::Ws | SocketScheme::Wss => {
            let (host, port) = match (&input.host, input.port) {
                (Some(h), Some(p)) => (h.clone(), p),
                _ => {
                    return Err(RivetError::validation(
                        "validation.endpoint",
                        format!("`{}` needs a host and port", input.endpoint),
                    ));
                }
            };
            let target = match input.scheme {
                SocketScheme::Tcp => format!("tcp://{host}:{port}"),
                s => {
                    let path = url::Url::parse(&input.endpoint)
                        .map(|u| u.path().to_string())
                        .unwrap_or_else(|_| "/".into());
                    format!("{}://{host}:{port}{path}", s.as_str())
                }
            };
            authorize(
                evaluator,
                &input.origin,
                Capability::Network,
                AccessVerb::Connect,
                EffectTarget::Url(target),
            )?;
            Some(
                checked_addr(
                    evaluator,
                    &input.origin,
                    input.scheme.as_str(),
                    &host,
                    port,
                    |h, p| async move { sockets.resolve(&h, p).await },
                )
                .await?,
            )
        }
    };
    // vhco:todo frame_and_exchange -- the connection applies the declared framing (newline, length32 big|little, delimiter, raw; WebSocket keeps message boundaries) with max_frame checked before allocation; send/receive/iteration happen through it under the scope deadline
    // vhco:step connect sockets.connect -- dial the checked address (or the authorized socket path) and perform the WebSocket upgrade when asked
    // vhco:todo close -- the returned connection is owned by the `with` scope: on exit, error, break or cancel it half-closes (TCP FIN / WebSocket close frame) and is dropped within the cleanup grace; it is never returned as a value
    sockets
        .connect(&input, addr)
        .await
        .map_err(|e| e.with_span(input.origin.span.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::policy::{Decision, EffectIntent, Grant, Permit, Policy};
    use crate::domain::transports::{EffectOrigin, Frame, Framing, TlsMaterial};
    use async_trait::async_trait;
    use std::net::{IpAddr, SocketAddr};
    use std::sync::Mutex;

    struct Only(Policy);
    impl PolicyEvaluator for Only {
        fn evaluate(&self, i: &EffectIntent) -> Permit {
            let ok = self.0.grants.iter().any(|g| {
                g.capability == i.capability && g.targets.iter().any(|t| t == i.target.as_str())
            });
            Permit {
                intent: i.clone(),
                decision: if ok {
                    Decision::Allowed
                } else {
                    Decision::Denied
                },
                rule: "test".into(),
            }
        }
        fn policy(&self) -> &Policy {
            &self.0
        }
    }

    struct Null;
    #[async_trait]
    impl SocketConnection for Null {
        async fn send(&mut self, _f: Frame) -> RivetResult<()> {
            Ok(())
        }
        async fn receive(&mut self) -> RivetResult<Option<Frame>> {
            Ok(None)
        }
        async fn finish_send(&mut self) -> RivetResult<()> {
            Ok(())
        }
        async fn close(self: Box<Self>) -> RivetResult<()> {
            Ok(())
        }
    }

    struct Dialer(Mutex<Vec<Option<SocketAddr>>>);
    #[async_trait]
    impl SocketStream for Dialer {
        async fn resolve(&self, _h: &str, _p: u16) -> RivetResult<Vec<IpAddr>> {
            Ok(vec!["127.0.0.1".parse().unwrap()])
        }
        async fn connect(
            &self,
            _plan: &SocketPlan,
            addr: Option<SocketAddr>,
        ) -> RivetResult<Box<dyn SocketConnection>> {
            self.0.lock().unwrap().push(addr);
            Ok(Box::new(Null))
        }
    }

    fn plan(scheme: SocketScheme, endpoint: &str) -> SocketPlan {
        SocketPlan {
            scheme,
            endpoint: endpoint.into(),
            host: Some("127.0.0.1".into()),
            port: Some(9000),
            framing: Framing::Newline,
            max_frame: 65536,
            tls_requested: false,
            tls: TlsMaterial::default(),
            timeout_ms: None,
            reconnect: false,
            origin: EffectOrigin::default(),
        }
    }

    fn policy(cap: Capability, target: &str) -> Only {
        Only(Policy {
            present: true,
            grants: vec![Grant {
                capability: cap,
                targets: vec![target.into()],
                access: None,
            }],
            ..Policy::default()
        })
    }

    // vhco:test transports.exchange_socket -- tcp is authorized as tcp://host:port and dials the checked address; Stage C TLS and reconnect are refused before any dial
    #[tokio::test]
    async fn tcp_authorized_and_stage_c_refused() {
        let d = Dialer(Mutex::new(vec![]));
        let ev = policy(Capability::Network, "tcp://127.0.0.1:9000");
        exchange_socket(plan(SocketScheme::Tcp, "127.0.0.1:9000"), &ev, &d)
            .await
            .unwrap();
        assert_eq!(
            d.0.lock().unwrap()[0],
            Some("127.0.0.1:9000".parse().unwrap())
        );
        let mut tls = plan(SocketScheme::Tcp, "127.0.0.1:9000");
        tls.tls_requested = true;
        let e = exchange_socket(tls, &ev, &d).await.err().unwrap();
        assert_eq!(e.code, "unsupported.tcp_tls");
        let mut rc = plan(SocketScheme::Ws, "ws://127.0.0.1:9000/x");
        rc.reconnect = true;
        assert_eq!(
            exchange_socket(rc, &ev, &d).await.err().unwrap().code,
            "unsupported.reconnect"
        );
        assert_eq!(d.0.lock().unwrap().len(), 1);
    }

    // vhco:test transports.exchange_socket -- a unix socket needs allow_unix, not network or read authority
    #[tokio::test]
    async fn unix_needs_allow_unix() {
        let d = Dialer(Mutex::new(vec![]));
        let net = policy(Capability::Network, "/tmp/r.sock");
        let e = exchange_socket(plan(SocketScheme::Unix, "/tmp/r.sock"), &net, &d)
            .await
            .err()
            .unwrap();
        assert_eq!(e.code, "permission.denied");
        let unix = policy(Capability::Unix, "/tmp/r.sock");
        exchange_socket(plan(SocketScheme::Unix, "/tmp/r.sock"), &unix, &d)
            .await
            .unwrap();
        assert_eq!(d.0.lock().unwrap()[0], None);
    }
}
