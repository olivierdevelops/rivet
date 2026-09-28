use super::ports::{Codec, HttpClient, PolicyEvaluator};
use crate::domain::effect_checks::{authorize, checked_addr};
use crate::domain::policy::{AccessVerb, Capability, EffectTarget};
use crate::domain::transports::{
    CodecInput, CodecKind, HttpBody, HttpExchange, HttpOutcome, HttpReply, HttpResponse,
    HttpVersionPolicy, HttpWire, WireTarget, replay_safe, request_unsent,
};
use crate::domain::{ErrorKind, RivetError, RivetResult, Value};

/// Headers that carry credentials; dropped when a redirect changes origin.
const CREDENTIAL_HEADERS: [&str; 3] = ["authorization", "proxy-authorization", "cookie"];

// vhco:usecase transports.exchange_http(input: HttpExchange) -> HttpResponse needs HttpClient, Codec, PolicyEvaluator
// vhco:label Exchange http
// vhco:about One brokered HTTP exchange: encodes the typed body, authorizes the origin and every resolved address before dialing, sends through the connection-level client at the declared HTTP version (`version 3` strict HTTP/3, `version prefer [3, 2]` falling back only before any request byte), re-authorizes each redirect hop (only when `redirect follow` is set), retries only replay-safe methods on listed statuses, then maps a non-accepted status to http.status and decodes the body.
// vhco:example input={method:"GET", url:"https://api.example.com/users/42", decode:"json"} => { "status": 200, "headers": {"content-type": "application/json"}, "body": {"id": 42, "name": "Ada"} }
pub async fn exchange_http(
    input: HttpExchange,
    evaluator: &dyn PolicyEvaluator,
    client: &dyn HttpClient,
    codec: &dyn Codec,
) -> RivetResult<HttpOutcome> {
    // vhco:todo build_request -- upper-case the method; refuse `retry` on a method that is not replay-safe (GET/HEAD/OPTIONS) with validation.http_retry_unsafe; parse the already component-encoded URL (http/https only), append `query` pairs with form encoding, encode `body json|text|form|bytes` through Codec and add its Content-Type unless a header sets one; nothing is sent yet
    // vhco:step method to_ascii_uppercase -- methods are case-insensitive in source
    let mut method = input.method.to_ascii_uppercase();
    // vhco:error retry_unsafe -- `retry` on POST/PUT/PATCH/DELETE => validation.http_retry_unsafe (exit 2) before any effect
    if input.retry.is_some() && !replay_safe(&method) {
        return Err(RivetError::validation(
            "validation.http_retry_unsafe",
            format!(
                "`retry` is only allowed for replay-safe methods (GET, HEAD, OPTIONS), not {method}"
            ),
        )
        .with_span(input.origin.span.clone()));
    }
    // vhco:step url url::Url::parse -- absolute http(s) URL; query options appended as encoded pairs
    let mut url = url::Url::parse(&input.url).map_err(|e| {
        RivetError::validation(
            "validation.url",
            format!("invalid URL `{}`: {e}", input.url),
        )
        .with_span(input.origin.span.clone())
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(RivetError::validation(
            "validation.url",
            format!(
                "`http` needs an http:// or https:// URL, got `{}`",
                url.scheme()
            ),
        ));
    }
    if !input.query.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (k, v) in &input.query {
            pairs.append_pair(k, v);
        }
    }
    let mut headers = input.headers.clone();
    // vhco:step body codec.encode -- typed body → bytes; its Content-Type is added unless set explicitly
    let mut body = match &input.body {
        Some(b) => {
            let bytes = codec.encode(b)?;
            if !headers
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            {
                headers.push(("content-type".into(), b.kind.content_type().into()));
            }
            Some(bytes)
        }
        None => None,
    };
    let first_origin = url.origin();

    // vhco:todo send -- for each attempt: authorize allow_network connect scheme://host:port/path (or allow_unix connect PATH for `unix`), resolve the host once and dial only a checked address, send through HttpClient at the declared version (`version 3` = HTTP/3 over QUIC to the same checked host:port, no downgrade; `version prefer [3, 2|1.1]` falls back to the TCP version only when the client proves no request byte was written, so a POST whose bytes may have left is never sent twice); a 3xx with Location is followed only under `redirect follow limit N` (each hop re-authorized, credentials stripped on origin change, 303 → GET); a status listed in `retry … on status` is retried at most N more times after the backoff (Retry-After honoured up to `max`)
    let mut hops = 0u32;
    let mut attempt = 0u32;
    let reply: HttpReply = loop {
        // vhco:step authorize checked_target -- permit per attempt; private resolved IPs need a literal grant
        let target = checked_target(
            &url,
            &input,
            first_origin == url.origin(),
            evaluator,
            client,
        )
        .await?;
        let wire = HttpWire {
            method: method.clone(),
            url: url.to_string(),
            headers: headers.clone(),
            body: body.clone(),
            version: input.version,
            tls: input.tls.clone(),
            stream: input.stream.is_some(),
            target,
            max_body: input.max_body,
        };
        // vhco:step send client.send -- one connection-level exchange at the checked address; the client never follows redirects
        // vhco:step h3 send_versioned -- `version 3` sends over HTTP/3 only (a peer without h3 => http.version_unavailable, exit 5, never a TCP retry); `version prefer [3, …]` re-sends over the listed TCP version at the SAME checked address only when the H3 failure carries request_sent=false
        let reply = send_versioned(client, wire, input.version).await?;
        if (300..400).contains(&reply.status)
            && reply.status != 304
            && input.redirect_limit > 0
            && let Some(location) = reply.header("location").map(str::to_string)
        {
            // vhco:error redirect_limit -- more hops than `redirect follow limit N` => limit.http_redirects
            if hops >= input.redirect_limit {
                discard(reply).await;
                return Err(RivetError::new(
                    ErrorKind::Limit,
                    "limit.http_redirects",
                    format!("more than {} redirects", input.redirect_limit),
                ));
            }
            hops += 1;
            let next = url.join(&location).map_err(|e| {
                RivetError::new(
                    ErrorKind::Protocol,
                    "protocol.http_location",
                    format!("invalid redirect Location: {e}"),
                )
            })?;
            // vhco:step redirect strip -- a new origin loses Authorization/Cookie; 303 (and 301/302 after POST) become GET without a body
            if next.origin() != url.origin() {
                headers
                    .retain(|(k, _)| !CREDENTIAL_HEADERS.iter().any(|c| k.eq_ignore_ascii_case(c)));
            }
            if reply.status == 303 || (matches!(reply.status, 301 | 302) && method == "POST") {
                method = "GET".into();
                body = None;
                headers.retain(|(k, _)| !k.eq_ignore_ascii_case("content-type"));
            }
            discard(reply).await;
            url = next;
            continue;
        }
        if let Some(r) = &input.retry
            && r.on_status.contains(&reply.status)
            && attempt < r.attempts
        {
            attempt += 1;
            // vhco:step backoff client.wait -- Retry-After (seconds) bounded by `max`, else the policy's exponential/fixed delay
            let hinted = reply
                .header("retry-after")
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(|s| s.saturating_mul(1000).min(r.max_ms));
            discard(reply).await;
            client
                .wait(
                    hinted.unwrap_or_else(|| r.delay_ms(attempt)),
                    hinted.is_none() && r.jitter,
                )
                .await;
            continue;
        }
        break reply;
    };

    // vhco:todo decode -- a status outside `accept status [...]` (default any 2xx) closes the body and fails http.status (kind http, details.status, 502 / exit 5); otherwise decode the finite body through Codec per `decode` (default text, bytes when not UTF-8); an accepted non-2xx body that does not decode falls back to text; a stream body is handed back undecoded for the scope to frame
    let accepted = if input.accept.is_empty() {
        (200..300).contains(&reply.status)
    } else {
        input.accept.contains(&reply.status)
    };
    let status = reply.status;
    let header_value = headers_value(&reply.headers);
    let version = reply.version.clone();
    // vhco:error http_status -- non-accepted status => http.status with details.status (kind http, 502, exit 5)
    if !accepted {
        discard(reply).await;
        // Scheme, host, explicit port and path: never userinfo or query.
        let shown = format!(
            "{}://{}{}{}",
            url.scheme(),
            url.host_str().unwrap_or(""),
            url.port().map(|p| format!(":{p}")).unwrap_or_default(),
            url.path()
        );
        return Err(RivetError::new(
            ErrorKind::Http,
            "http.status",
            format!("{method} {shown} returned {status}"),
        )
        .with_span(input.origin.span.clone())
        .with_details(Value::object([
            ("status", Value::Int(status as i64)),
            ("method", Value::text(&method)),
        ])));
    }
    let (body_value, stream) = match reply.body {
        // vhco:step decode codec.decode -- finite body → Value
        HttpBody::Complete(bytes) => (decode_body(codec, input.decode, bytes, status)?, None),
        HttpBody::Stream(s) => (Value::Null, Some(s)),
    };
    Ok(HttpOutcome {
        response: HttpResponse {
            status,
            headers: header_value,
            body: body_value,
            version,
        },
        stream,
        decode: input.decode,
    })
}

/// One attempt under the version policy. Only an H3 failure that provably
/// wrote no request byte (`request_unsent`) may fall back, and only when the
/// policy lists a fallback; everything else propagates unchanged.
async fn send_versioned(
    client: &dyn HttpClient,
    wire: HttpWire,
    policy: HttpVersionPolicy,
) -> RivetResult<HttpReply> {
    if !policy.wants_h3() {
        return client.send(wire).await;
    }
    let mut tcp = wire.clone();
    let h3 = HttpWire {
        version: HttpVersionPolicy::Http3,
        ..wire
    };
    // vhco:error h3_unavailable -- strict `version 3` and the peer has no HTTP/3 (no QUIC answer, ALPN h3 refused, h3 setup failed) => http.version_unavailable (kind protocol, exit 5) returns
    match client.send(h3).await {
        Ok(reply) => Ok(reply),
        Err(e) => match policy.h3_fallback() {
            Some(v) if request_unsent(&e) => {
                tcp.version = v;
                client.send(tcp).await
            }
            // vhco:error ambiguous_send -- the H3 request may already have reached the server (bytes written, response lost) => its connection error returns and nothing is re-sent over TCP, whatever the method
            _ => Err(e),
        },
    }
}

async fn checked_target(
    url: &url::Url,
    input: &HttpExchange,
    same_origin: bool,
    evaluator: &dyn PolicyEvaluator,
    client: &dyn HttpClient,
) -> RivetResult<WireTarget> {
    if let (Some(path), true) = (&input.unix_socket, same_origin) {
        authorize(
            evaluator,
            &input.origin,
            Capability::Unix,
            AccessVerb::Connect,
            EffectTarget::Path(path.clone()),
        )?;
        return Ok(WireTarget::Unix(path.clone()));
    }
    let host = url.host_str().unwrap_or("").to_string();
    let port = url.port_or_known_default().unwrap_or(80);
    authorize(
        evaluator,
        &input.origin,
        Capability::Network,
        AccessVerb::Connect,
        EffectTarget::Url(format!(
            "{}://{}:{}{}",
            url.scheme(),
            host,
            port,
            url.path()
        )),
    )?;
    let addr = checked_addr(
        evaluator,
        &input.origin,
        url.scheme(),
        &host,
        port,
        |h, p| async move { client.resolve(&h, p).await },
    )
    .await?;
    Ok(WireTarget::Tcp(addr))
}

async fn discard(reply: HttpReply) {
    if let HttpBody::Stream(s) = reply.body {
        let _ = s.close().await;
    }
}

fn headers_value(headers: &[(String, String)]) -> Value {
    let mut v = Value::Object(Vec::new());
    for (k, val) in headers {
        let key = k.to_ascii_lowercase();
        let joined = match v.get(&key).and_then(Value::as_str) {
            Some(prev) => format!("{prev}, {val}"),
            None => val.clone(),
        };
        v.set(&key, Value::Text(joined));
    }
    v
}

fn decode_body(
    codec: &dyn Codec,
    decode: Option<CodecKind>,
    bytes: Vec<u8>,
    status: u16,
) -> RivetResult<Value> {
    let kind = match decode {
        Some(k) => k,
        None => {
            return Ok(match String::from_utf8(bytes) {
                Ok(s) => Value::Text(s),
                Err(e) => Value::Bytes(e.into_bytes()),
            });
        }
    };
    let input = CodecInput {
        kind,
        bytes: Some(bytes),
        value: None,
    };
    match codec.decode(&input) {
        Ok(v) => Ok(v),
        Err(_) if !(200..300).contains(&status) => {
            let bytes = input.bytes.unwrap_or_default();
            Ok(if bytes.is_empty() {
                Value::Null
            } else {
                Value::Text(String::from_utf8_lossy(&bytes).into_owned())
            })
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::policy::{Decision, EffectIntent, Grant, NetworkPolicy, Permit, Policy};
    use crate::domain::transports::{EffectOrigin, RetryPolicy, TlsMaterial};
    use async_trait::async_trait;
    use std::net::IpAddr;
    use std::sync::Mutex;

    struct Grants(Policy);
    impl PolicyEvaluator for Grants {
        fn evaluate(&self, i: &EffectIntent) -> Permit {
            let ok = self.0.grants.iter().any(|g| {
                g.capability == i.capability
                    && g.targets.iter().any(|t| i.target.as_str().starts_with(t))
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

    fn grants(targets: &[&str]) -> Grants {
        Grants(Policy {
            present: true,
            grants: vec![Grant {
                capability: Capability::Network,
                targets: targets.iter().map(|s| s.to_string()).collect(),
                access: None,
            }],
            network: NetworkPolicy {
                deny_private_ranges: true,
            },
            ..Policy::default()
        })
    }

    type Scripted = (u16, Vec<(String, String)>, &'static str);

    /// Scripted client: replies in order, records every wire and wait.
    struct Script {
        replies: Mutex<Vec<Scripted>>,
        sent: Mutex<Vec<HttpWire>>,
        waits: Mutex<Vec<u64>>,
        resolves_to: IpAddr,
    }
    #[async_trait]
    impl HttpClient for Script {
        async fn resolve(&self, _h: &str, _p: u16) -> RivetResult<Vec<IpAddr>> {
            Ok(vec![self.resolves_to])
        }
        async fn send(&self, wire: HttpWire) -> RivetResult<HttpReply> {
            self.sent.lock().unwrap().push(wire);
            let (status, headers, body) = self.replies.lock().unwrap().remove(0);
            Ok(HttpReply {
                status,
                headers,
                version: "HTTP/1.1".into(),
                body: HttpBody::Complete(body.as_bytes().to_vec()),
            })
        }
        async fn wait(&self, ms: u64, _j: bool) {
            self.waits.lock().unwrap().push(ms);
        }
    }

    struct Json;
    impl Codec for Json {
        fn decode(&self, i: &CodecInput) -> RivetResult<Value> {
            let b = i.bytes.clone().unwrap_or_default();
            serde_json::from_slice::<serde_json::Value>(&b)
                .map(|j| Value::from_json(&j))
                .map_err(|e| RivetError::new(ErrorKind::Parse, "parse.json", e.to_string()))
        }
        fn encode(&self, i: &CodecInput) -> RivetResult<Vec<u8>> {
            Ok(i.value
                .clone()
                .unwrap_or_default()
                .to_json()
                .to_string()
                .into_bytes())
        }
    }

    fn script(replies: Vec<Scripted>, ip: &str) -> Script {
        Script {
            replies: Mutex::new(replies),
            sent: Mutex::new(vec![]),
            waits: Mutex::new(vec![]),
            resolves_to: ip.parse().unwrap(),
        }
    }

    fn get(url: &str) -> HttpExchange {
        HttpExchange {
            method: "get".into(),
            url: url.into(),
            headers: vec![("Authorization".into(), "Bearer t".into())],
            query: vec![],
            body: None,
            version: HttpVersionPolicy::Auto,
            decode: Some(CodecKind::Json),
            accept: vec![],
            retry: None,
            redirect_limit: 0,
            tls: TlsMaterial::default(),
            stream: None,
            unix_socket: None,
            max_body: 1 << 20,
            origin: EffectOrigin::default(),
        }
    }

    // vhco:test transports.exchange_http -- retries only listed statuses, then maps an unaccepted 404 to http.status with details.status
    #[tokio::test]
    async fn retries_then_maps_status() {
        let c = script(
            vec![
                (503, vec![], ""),
                (429, vec![("Retry-After".into(), "1".into())], ""),
                (404, vec![], "{\"missing\":true}"),
            ],
            "93.184.216.34",
        );
        let mut x = get("https://api.example.com/users/42");
        x.retry = Some(RetryPolicy {
            attempts: 3,
            on_status: vec![429, 503],
            base_ms: 100,
            max_ms: 2000,
            exponential: true,
            jitter: false,
        });
        let e = exchange_http(x, &grants(&["https://api.example.com:443"]), &c, &Json)
            .await
            .unwrap_err();
        assert_eq!(e.code, "http.status");
        assert_eq!(e.kind, ErrorKind::Http);
        assert_eq!(e.details.get("status"), Some(&Value::Int(404)));
        assert_eq!(*c.waits.lock().unwrap(), vec![100, 1000]);
        assert_eq!(c.sent.lock().unwrap().len(), 3);
    }

    // vhco:test transports.exchange_http -- accepted 404 decodes; retry on POST is refused before sending
    #[tokio::test]
    async fn accept_and_unsafe_retry() {
        let c = script(vec![(404, vec![], "{\"id\":1}")], "93.184.216.34");
        let mut x = get("https://api.example.com/u");
        x.accept = vec![200, 404];
        let out = exchange_http(x, &grants(&["https://api.example.com:443"]), &c, &Json)
            .await
            .unwrap();
        assert_eq!(out.response.status, 404);
        assert_eq!(out.response.body.get("id"), Some(&Value::Int(1)));
        let mut post = get("https://api.example.com/u");
        post.method = "post".into();
        post.retry = Some(RetryPolicy {
            attempts: 1,
            on_status: vec![503],
            base_ms: 1,
            max_ms: 1,
            exponential: false,
            jitter: false,
        });
        let e = exchange_http(post, &grants(&["https://api.example.com:443"]), &c, &Json)
            .await
            .unwrap_err();
        assert_eq!(e.code, "validation.http_retry_unsafe");
    }

    // vhco:test transports.exchange_http -- a hostname resolving to a private address is refused (DNS rebinding), and redirects need their own grant with credentials stripped
    #[tokio::test]
    async fn rebinding_and_redirects() {
        let c = script(vec![(200, vec![], "{}")], "10.0.0.5");
        let e = exchange_http(
            get("https://api.example.com/x"),
            &grants(&["https://api.example.com:443"]),
            &c,
            &Json,
        )
        .await
        .unwrap_err();
        assert_eq!(e.code, "permission.denied");
        assert!(c.sent.lock().unwrap().is_empty());

        let c = script(
            vec![
                (
                    302,
                    vec![("Location".into(), "https://cdn.example.com/f".into())],
                    "",
                ),
                (200, vec![], "{\"ok\":true}"),
            ],
            "93.184.216.34",
        );
        let mut x = get("https://api.example.com/download");
        x.redirect_limit = 2;
        let out = exchange_http(
            x.clone(),
            &grants(&["https://api.example.com:443", "https://cdn.example.com:443"]),
            &c,
            &Json,
        )
        .await
        .unwrap();
        assert_eq!(out.response.body.get("ok"), Some(&Value::Bool(true)));
        {
            let sent = c.sent.lock().unwrap();
            assert!(sent[0].headers.iter().any(|(k, _)| k == "Authorization"));
            assert!(!sent[1].headers.iter().any(|(k, _)| k == "Authorization"));
        }
        let c = script(
            vec![(
                302,
                vec![("Location".into(), "https://evil.example.net/".into())],
                "",
            )],
            "93.184.216.34",
        );
        let e = exchange_http(x, &grants(&["https://api.example.com:443"]), &c, &Json)
            .await
            .unwrap_err();
        assert_eq!(e.code, "permission.denied");
        assert_eq!(c.sent.lock().unwrap().len(), 1);
    }

    /// H3-aware scripted client: the H3 attempt fails (sent or unsent) or
    /// succeeds; TCP attempts always answer 200. Records each wire version.
    struct Versions {
        h3: Option<bool>,
        seen: Mutex<Vec<HttpVersionPolicy>>,
    }
    #[async_trait]
    impl HttpClient for Versions {
        async fn resolve(&self, _h: &str, _p: u16) -> RivetResult<Vec<IpAddr>> {
            Ok(vec!["93.184.216.34".parse().unwrap()])
        }
        async fn send(&self, wire: HttpWire) -> RivetResult<HttpReply> {
            self.seen.lock().unwrap().push(wire.version);
            let version = if wire.version == HttpVersionPolicy::Http3 {
                if let Some(sent) = self.h3 {
                    let code = if sent {
                        "connection.http3"
                    } else {
                        "http.version_unavailable"
                    };
                    return Err(RivetError::new(ErrorKind::Protocol, code, "h3 failed")
                        .with_details(Value::object([("request_sent", Value::Bool(sent))])));
                }
                "3"
            } else {
                "2"
            };
            Ok(HttpReply {
                status: 200,
                headers: vec![],
                version: version.into(),
                body: HttpBody::Complete(b"{}".to_vec()),
            })
        }
        async fn wait(&self, _ms: u64, _j: bool) {}
    }

    async fn versioned(
        policy: HttpVersionPolicy,
        h3: Option<bool>,
        method: &str,
    ) -> (RivetResult<HttpOutcome>, Vec<HttpVersionPolicy>) {
        let c = Versions {
            h3,
            seen: Mutex::new(vec![]),
        };
        let mut x = get("https://api.example.com/items");
        x.method = method.into();
        x.version = policy;
        let r = exchange_http(x, &grants(&["https://api.example.com:443"]), &c, &Json).await;
        (r, c.seen.into_inner().unwrap())
    }

    // vhco:test transports.exchange_http -- strict version 3 never downgrades; prefer [3, 2] falls back only when the H3 failure wrote no request byte, and a POST whose H3 bytes may have left is sent exactly once
    #[tokio::test]
    async fn version_selection_and_safe_fallback() {
        use HttpVersionPolicy::*;
        let (r, seen) = versioned(Http3, None, "get").await;
        assert_eq!(r.unwrap().response.version, "3");
        assert_eq!(seen, vec![Http3]);

        let (r, seen) = versioned(Http3, Some(false), "get").await;
        assert_eq!(r.unwrap_err().code, "http.version_unavailable");
        assert_eq!(seen, vec![Http3], "strict H3 must not downgrade");

        let (r, seen) = versioned(Http3OrHttp2, Some(false), "post").await;
        let out = r.unwrap();
        assert_eq!(out.response.version, "2");
        assert_eq!(out.response.to_value().get("version"), Some(&Value::Int(2)));
        assert_eq!(seen, vec![Http3, Http2]);

        let (r, seen) = versioned(Http3OrAuto, Some(true), "post").await;
        assert_eq!(r.unwrap_err().code, "connection.http3");
        assert_eq!(seen, vec![Http3], "an ambiguous send is never replayed");

        assert_eq!(
            HttpVersionPolicy::from_preference(&["3".into(), "2".into()]),
            Some(Http3OrHttp2)
        );
        assert_eq!(
            HttpVersionPolicy::from_preference(&["2".into(), "3".into()]),
            None,
            "HTTP/3 may only come first"
        );
    }
}
