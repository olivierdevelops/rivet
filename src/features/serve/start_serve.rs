use super::ports::ServeListener;
use crate::domain::policy::{ALL_SURFACES, ServeAuth};
use crate::domain::serve::{
    DEFAULT_LISTEN, ServeConfig, ServeReceipt, ServeStartInput, SurfaceMount, auth_type_name,
    surface_routes,
};
use crate::domain::{RivetError, RivetResult};
use std::net::{IpAddr, SocketAddr};

// vhco:usecase serve.start_serve(input: ServeStartInput) -> ServeReceipt needs ServeListener
// vhco:label Start serve
// vhco:about Binds one listener and mounts every enabled surface (REST, SSE, polling, WebSocket, MCP) on it, refusing an unauthenticated non-loopback bind; `--stdio` serves MCP only and binds nothing.
// vhco:example input={listen:"127.0.0.1:8080"} => { "listen_addr": "127.0.0.1:8080", "surfaces": ["http", "sse", "poll", "ws", "mcp"], "auth_type": "none" }
pub async fn start_serve(
    input: ServeStartInput,
    listener: &mut dyn ServeListener,
) -> RivetResult<ServeReceipt> {
    let auth_type = auth_type_name(&input.serve.auth).to_string();
    // vhco:todo resolve_mode -- `--stdio` serves MCP over stdio only and binds nothing (receipt stdio:true, surfaces [mcp]); otherwise listen on --listen (default 127.0.0.1:8080); there are no --transport or --mcp flags
    // vhco:step stdio return -- stdio mode: no bind, MCP only
    if input.stdio {
        return Ok(ServeReceipt {
            listen_addr: None,
            stdio: true,
            surfaces: vec!["mcp".into()],
            auth_type,
            catalog_version: input.catalog_version,
            policy_hash: input.policy_hash,
        });
    }
    let listen = input
        .listen
        .clone()
        .unwrap_or_else(|| DEFAULT_LISTEN.to_string());
    // vhco:step parse parse_listen -- HOST:PORT with an IP literal or `localhost`; anything else is validation.usage (exit 2)
    // vhco:error bad_listen -- --listen is not HOST:PORT => validation.usage returns
    let addr = parse_listen(&listen)?;
    let loopback = addr.ip().is_loopback();
    // vhco:todo refuse_unsafe_bind -- read serve.auth (missing = none); auth none on a non-loopback address refuses to start with serve.auth_required (exit 2) before anything is bound; mtls needs a TLS listener this build does not have, so it is unsupported.serve_mtls (exit 5) instead of silently serving plaintext
    // vhco:error auth_required -- auth none and a non-loopback --listen => serve.auth_required (exit 2) returns before bind
    if matches!(input.serve.auth, ServeAuth::None) && !loopback {
        return Err(RivetError::validation(
            "serve.auth_required",
            "non-loopback listener requires serve.auth in policy.json",
        ));
    }
    // vhco:error mtls_unsupported -- serve.auth type mtls => unsupported.serve_mtls (exit 5) returns before bind
    if matches!(input.serve.auth, ServeAuth::Mtls { .. }) {
        return Err(RivetError::unsupported(
            "unsupported.serve_mtls",
            "serve.auth type mtls needs a TLS listener, which this build does not provide yet; use bearer behind a TLS-terminating proxy",
        ));
    }
    // vhco:todo select_surfaces -- enabled = serve.surfaces (policy default: all five http, sse, poll, ws, mcp), kept in canonical order; a surface that is not enabled is never mounted and answers 404
    // vhco:step select filter -- canonical order ∩ serve.surfaces
    let enabled: Vec<String> = ALL_SURFACES
        .iter()
        .filter(|s| input.serve.surfaces.iter().any(|x| x == *s))
        .map(|s| s.to_string())
        .collect();
    // vhco:todo bind_and_mount -- bind once through ServeListener.bind, then mount each enabled surface's routes on that one listener (http: /v1/request + /v1/operations*, sse: /v1/request with Accept text/event-stream, poll: /v1/requests*, ws: /v1/ws, mcp: /mcp); every mount shares the catalog, dispatcher, session driver and authenticator; if a mount fails the error is returned (the listener is dropped, so nothing is served partially)
    // vhco:step bind listener.bind -- one socket for every surface
    let handle = listener
        .bind(ServeConfig {
            listen: addr.to_string(),
            loopback,
        })
        .await?;
    let mut mounted = Vec::new();
    for surface in &enabled {
        // vhco:step mount listener.mount -- one surface onto the shared listener; errors abort start
        // vhco:error mount_failed -- a surface fails to mount => the adapter's error returns and nothing is served
        let receipt = listener
            .mount(SurfaceMount {
                surface: surface.clone(),
                routes: surface_routes(surface),
                listener: handle.clone(),
            })
            .await?;
        if receipt.mounted {
            mounted.push(receipt.surface);
        }
    }
    // vhco:todo report -- return ServeReceipt{listen_addr (the actually bound address, so :0 reports its port), stdio:false, surfaces, auth_type, catalog_version, policy_hash}; with no policy.json serve still starts on loopback with auth none and every effect denied
    Ok(ServeReceipt {
        listen_addr: Some(handle.listen_addr),
        stdio: false,
        surfaces: mounted,
        auth_type,
        catalog_version: input.catalog_version,
        policy_hash: input.policy_hash,
    })
}

fn parse_listen(listen: &str) -> RivetResult<SocketAddr> {
    if let Ok(a) = listen.parse::<SocketAddr>() {
        return Ok(a);
    }
    if let Some((host, port)) = listen.rsplit_once(':') {
        if let Ok(port) = port.parse::<u16>() {
            let host = host.trim_start_matches('[').trim_end_matches(']');
            if host.eq_ignore_ascii_case("localhost") {
                return Ok(SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port));
            }
            if let Ok(ip) = host.parse::<IpAddr>() {
                return Ok(SocketAddr::new(ip, port));
            }
        }
    }
    Err(RivetError::validation(
        "validation.usage",
        format!("--listen {listen}: use HOST:PORT with an IP address or localhost"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::policy::ServePolicy;
    use crate::domain::serve::{ListenerHandle, MountReceipt};
    use async_trait::async_trait;

    #[derive(Default)]
    struct Recorder {
        bound: Option<String>,
        mounts: Vec<String>,
    }

    #[async_trait]
    impl ServeListener for Recorder {
        async fn bind(&mut self, c: ServeConfig) -> RivetResult<ListenerHandle> {
            self.bound = Some(c.listen.clone());
            Ok(ListenerHandle {
                listen_addr: c.listen,
                loopback: c.loopback,
            })
        }
        async fn mount(&mut self, m: SurfaceMount) -> RivetResult<MountReceipt> {
            self.mounts.push(m.surface.clone());
            Ok(MountReceipt {
                surface: m.surface,
                mounted: true,
            })
        }
    }

    fn start(listen: &str, serve: ServePolicy) -> ServeStartInput {
        ServeStartInput {
            listen: Some(listen.into()),
            stdio: false,
            serve,
            catalog_version: "sha256:x".into(),
            policy_hash: None,
        }
    }

    // vhco:test serve.start_serve -- non-loopback + auth none refuses before binding; loopback mounts the enabled surfaces in order; stdio binds nothing
    #[tokio::test]
    async fn refuses_and_mounts() {
        let mut l = Recorder::default();
        let e = start_serve(start("0.0.0.0:8080", ServePolicy::default()), &mut l)
            .await
            .unwrap_err();
        assert_eq!(e.code, "serve.auth_required");
        assert_eq!(e.exit_code(), 2);
        assert!(l.bound.is_none(), "nothing bound");

        let mut l = Recorder::default();
        let serve = ServePolicy {
            surfaces: vec!["mcp".into(), "http".into()],
            ..ServePolicy::default()
        };
        let r = start_serve(start("localhost:0", serve), &mut l)
            .await
            .unwrap();
        assert_eq!(r.surfaces, vec!["http", "mcp"]);
        assert_eq!(l.bound.as_deref(), Some("127.0.0.1:0"));

        let mut l = Recorder::default();
        let mut s = start("127.0.0.1:1", ServePolicy::default());
        s.stdio = true;
        let r = start_serve(s, &mut l).await.unwrap();
        assert!(r.stdio && l.bound.is_none());
        assert!(
            start_serve(
                start("nope", ServePolicy::default()),
                &mut Recorder::default()
            )
            .await
            .is_err()
        );
    }
}
