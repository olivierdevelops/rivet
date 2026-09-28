//! T-17 — native gRPC client (PROP-2026-0001 Increment 13; REF S110–S113, S117).
//!
//! An in-process tonic fixture server implements `example.Users` (all four
//! modes, from docs/demos/10-grpc/schemas/users.proto) and the test-only
//! `fixture.Types` echo service (ProtoJSON mapping). Descriptors are the
//! pinned `tests/fixtures/grpc/*.pb` sets generated with protoc ahead of time;
//! nothing is compiled or reflected at runtime.

use futures_util::future::BoxFuture;
use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor, ReflectMessage};
use rivet::domain::contracts::DataEvent;
use rivet::domain::errors::ErrorKind;
use rivet::domain::ports::DataSink;
use rivet::domain::{RivetError, RivetResult, Value};
use rivet::orchestrator::runtime::Runtime;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;
use tonic::codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder};
use tonic::codegen::http;
use tonic::server::NamedService;
use tonic::{Request, Response, Status, Streaming};

const USERS_PB: &str = "tests/fixtures/grpc/users.pb";
const TYPES_PB: &str = "tests/fixtures/grpc/types.pb";

// ---------------------------------------------------------------- fixture

#[derive(Clone)]
struct ServerCodec {
    input: MessageDescriptor,
}
struct Enc;
struct Dec(MessageDescriptor);

impl Codec for ServerCodec {
    type Encode = DynamicMessage;
    type Decode = DynamicMessage;
    type Encoder = Enc;
    type Decoder = Dec;
    fn encoder(&mut self) -> Enc {
        Enc
    }
    fn decoder(&mut self) -> Dec {
        Dec(self.input.clone())
    }
}

impl Encoder for Enc {
    type Item = DynamicMessage;
    type Error = Status;
    fn encode(&mut self, item: DynamicMessage, dst: &mut EncodeBuf<'_>) -> Result<(), Status> {
        item.encode(dst)
            .map_err(|e| Status::internal(e.to_string()))
    }
}

impl Decoder for Dec {
    type Item = DynamicMessage;
    type Error = Status;
    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<DynamicMessage>, Status> {
        DynamicMessage::decode(self.0.clone(), src)
            .map(Some)
            .map_err(|e| Status::internal(e.to_string()))
    }
}

type Out = Pin<Box<dyn futures_util::Stream<Item = Result<DynamicMessage, Status>> + Send>>;

fn msg(desc: &MessageDescriptor, json: serde_json::Value) -> DynamicMessage {
    DynamicMessage::deserialize(desc.clone(), json).unwrap()
}

fn text_of(m: &DynamicMessage, field: &str) -> String {
    m.get_field_by_name(field)
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Fixture contracts (docs/demos/10-grpc README plus test-only behaviours).
async fn handle(
    method: String,
    output: MessageDescriptor,
    req: Request<Streaming<DynamicMessage>>,
) -> Result<Response<Out>, Status> {
    let request_id = req
        .metadata()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let mut input = req.into_inner();
    let one = |m: DynamicMessage| -> Out { Box::pin(futures_util::stream::iter([Ok(m)])) };
    let mut response = match method.as_str() {
        "GetUser" => {
            let first = input
                .message()
                .await?
                .ok_or_else(|| Status::invalid_argument("no request"))?;
            match text_of(&first, "id").as_str() {
                "42" => Response::new(one(msg(
                    &output,
                    serde_json::json!({"id": "42", "name": "Ada"}),
                ))),
                "slow" => {
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    Response::new(one(msg(&output, serde_json::json!({"id": "slow"}))))
                }
                "twice" => Response::new(Box::pin(futures_util::stream::iter([
                    Ok(msg(&output, serde_json::json!({"id": "1"}))),
                    Ok(msg(&output, serde_json::json!({"id": "2"}))),
                ])) as Out),
                "denied" => return Err(Status::permission_denied("not yours")),
                "unauth" => return Err(Status::unauthenticated("log in")),
                other => return Err(Status::not_found(format!("no user {other}"))),
            }
        }
        "Watch" => {
            let first = input
                .message()
                .await?
                .ok_or_else(|| Status::invalid_argument("no request"))?;
            let m = |t: &str| Ok(msg(&output, serde_json::json!({"text": t})));
            match text_of(&first, "topic").as_str() {
                "changes" => Response::new(Box::pin(futures_util::stream::iter([
                    m("change 1"),
                    m("change 2"),
                ])) as Out),
                "fail_after_one" => Response::new(Box::pin(futures_util::stream::iter([
                    m("change 1"),
                    Err(Status::unavailable("backend went away")),
                ])) as Out),
                "forever" => {
                    let out = output.clone();
                    Response::new(Box::pin(futures_util::stream::unfold(0u64, move |n| {
                        let out = out.clone();
                        async move {
                            tokio::time::sleep(Duration::from_millis(20)).await;
                            Some((
                                Ok(msg(&out, serde_json::json!({"text": format!("tick {n}")}))),
                                n + 1,
                            ))
                        }
                    })) as Out)
                }
                _ => Response::new(Box::pin(futures_util::stream::empty()) as Out),
            }
        }
        "Upload" => {
            let mut count = 0;
            while let Some(m) = input.message().await? {
                count += 1;
                if text_of(&m, "text") == "early" {
                    break;
                }
            }
            Response::new(one(msg(&output, serde_json::json!({"count": count}))))
        }
        "Chat" => {
            let out = output.clone();
            let stream = futures_util::stream::unfold((input, false), move |(mut input, done)| {
                let out = out.clone();
                async move {
                    if done {
                        return None;
                    }
                    match input.message().await {
                        Ok(Some(m)) => {
                            let t = text_of(&m, "text");
                            let echo = Ok(msg(&out, serde_json::json!({"text": t.clone()})));
                            Some((echo, (input, t == "bye")))
                        }
                        Ok(None) => None,
                        Err(e) => Some((Err(e), (input, true))),
                    }
                }
            });
            Response::new(Box::pin(stream) as Out)
        }
        "Echo" => {
            let first = input
                .message()
                .await?
                .ok_or_else(|| Status::invalid_argument("no request"))?;
            Response::new(one(first))
        }
        _ => return Err(Status::unimplemented(method)),
    };
    if let Some(id) = request_id {
        response
            .metadata_mut()
            .insert("x-request-id", id.parse().unwrap());
    }
    response
        .metadata_mut()
        .insert("x-auth-token", "super-secret".parse().unwrap());
    Ok(response)
}

#[derive(Clone)]
struct Fixture<const TYPES: bool> {
    pool: DescriptorPool,
    calls: Arc<AtomicUsize>,
}

impl NamedService for Fixture<false> {
    const NAME: &'static str = "example.Users";
}
impl NamedService for Fixture<true> {
    const NAME: &'static str = "fixture.Types";
}

impl<const T: bool> tower::Service<http::Request<tonic::body::Body>> for Fixture<T> {
    type Response = http::Response<tonic::body::Body>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Infallible>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<tonic::body::Body>) -> Self::Future {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let pool = self.pool.clone();
        Box::pin(async move {
            let path = req.uri().path().to_string();
            let (service, method) = path.trim_start_matches('/').split_once('/').unwrap();
            let m = pool
                .get_service_by_name(service)
                .and_then(|s| s.methods().find(|m| m.name() == method))
                .unwrap();
            let mut grpc = tonic::server::Grpc::new(ServerCodec { input: m.input() });
            let (name, output) = (method.to_string(), m.output());
            let handler = tower::service_fn(move |r: Request<Streaming<DynamicMessage>>| {
                handle(name.clone(), output.clone(), r)
            });
            Ok(grpc.streaming(handler, req).await)
        })
    }
}

struct Server {
    addr: SocketAddr,
    calls: Arc<AtomicUsize>,
}

fn pool() -> DescriptorPool {
    let mut pool = DescriptorPool::decode(std::fs::read(USERS_PB).unwrap().as_slice()).unwrap();
    pool.decode_file_descriptor_set(std::fs::read(TYPES_PB).unwrap().as_slice())
        .unwrap();
    pool
}

async fn start_plain() -> Server {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let pool = pool();
    let users = Fixture::<false> {
        pool: pool.clone(),
        calls: Arc::clone(&calls),
    };
    let types = Fixture::<true> {
        pool,
        calls: Arc::clone(&calls),
    };
    let incoming = futures_util::stream::unfold(listener, |l| async move {
        let r = l.accept().await.map(|(s, _)| s);
        Some((r, l))
    });
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(users)
            .add_service(types)
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    Server { addr, calls }
}

struct TlsIo(tokio_rustls::server::TlsStream<tokio::net::TcpStream>);

impl tonic::transport::server::Connected for TlsIo {
    type ConnectInfo = ();
    fn connect_info(&self) {}
}
impl tokio::io::AsyncRead for TlsIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}
impl tokio::io::AsyncWrite for TlsIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

/// TLS fixture: a throwaway CA signs a `localhost` leaf; returns the CA PEM.
async fn start_tls() -> (Server, String) {
    use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair};
    let ca_key = KeyPair::generate().unwrap();
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();
    let issuer = Issuer::new(ca_params, ca_key);
    let leaf_key = KeyPair::generate().unwrap();
    let leaf = CertificateParams::new(vec!["localhost".to_string()])
        .unwrap()
        .signed_by(&leaf_key, &issuer)
        .unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![leaf.der().clone()],
            rustls_pki_types::PrivateKeyDer::try_from(leaf_key.serialize_der()).unwrap(),
        )
        .unwrap();
    config.alpn_protocols = vec![b"h2".to_vec()];
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let users = Fixture::<false> {
        pool: pool(),
        calls: Arc::clone(&calls),
    };
    let incoming = futures_util::stream::unfold((listener, acceptor), |(l, a)| async move {
        loop {
            let Ok((s, _)) = l.accept().await else {
                continue;
            };
            if let Ok(tls) = a.accept(s).await {
                return Some((Ok::<_, std::io::Error>(TlsIo(tls)), (l, a)));
            }
        }
    });
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(users)
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    (Server { addr, calls }, ca_cert.pem())
}

// ---------------------------------------------------------------- bundles

struct Bundle {
    _dir: tempfile::TempDir,
    rt: RivetResult<Runtime>,
}

fn connector(endpoint: &str) -> String {
    format!(
        "connector users grpc\n    endpoint \"{endpoint}\"\n    descriptor \"./schemas/users.pb\"\n    service \"example.Users\"\nend\n\nconnector types grpc\n    endpoint \"{endpoint}\"\n    descriptor \"./schemas/types.pb\"\n    service \"fixture.Types\"\nend\n\n"
    )
}

fn grants(origin: &str) -> String {
    format!(
        r#"{{"capability": "allow_network", "targets": ["{origin}"]}},
           {{"capability": "allow_grpc", "targets": ["users/example.Users/GetUser", "users/example.Users/Watch", "users/example.Users/Upload", "users/example.Users/Chat", "types/fixture.Types/Echo"]}}"#
    )
}

fn bundle(source: &str, policy: &str) -> Bundle {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("schemas")).unwrap();
    std::fs::copy(USERS_PB, dir.path().join("schemas/users.pb")).unwrap();
    std::fs::copy(TYPES_PB, dir.path().join("schemas/types.pb")).unwrap();
    std::fs::write(dir.path().join("app.rivet"), source).unwrap();
    std::fs::write(dir.path().join("policy.json"), policy).unwrap();
    let rt = Runtime::builder()
        .file(dir.path().join("app.rivet").to_str().unwrap())
        .build();
    Bundle { _dir: dir, rt }
}

fn policy(grants: &str) -> String {
    format!(r#"{{"version": 1, "grants": [{grants}]}}"#)
}

fn std_bundle(server: &Server, ops: &str) -> Bundle {
    let origin = format!("http://{}", server.addr);
    bundle(
        &format!("{}{ops}", connector(&origin)),
        &policy(&grants(&origin)),
    )
}

#[derive(Default)]
struct Collect(Mutex<Vec<Value>>);

#[async_trait::async_trait]
impl DataSink for Collect {
    async fn send(&self, event: DataEvent) -> RivetResult<()> {
        self.0.lock().unwrap().push(event.data);
        Ok(())
    }
}

fn obj(json: serde_json::Value) -> Value {
    Value::from_json(&json)
}

async fn call(
    rt: &Runtime,
    id: &str,
    params: serde_json::Value,
) -> (RivetResult<Value>, Vec<Value>) {
    let sink = Arc::new(Collect::default());
    let r = rt
        .request(id, obj(params), Some(sink.clone() as Arc<dyn DataSink>))
        .await
        .map(|c| c.result);
    let items = sink.0.lock().unwrap().clone();
    (r, items)
}

fn err(r: RivetResult<Value>) -> RivetError {
    r.expect_err("expected an error")
}

const UNARY: &str = "operation users.grpc_get\n    param id text required\n    output json\n    response = grpc users.GetUser\n        timeout \"5s\"\n        message {id: id}\n        metadata \"x-request-id\" \"fixture-request\"\n    end\n    return response\nend\n\n";

const WATCH: &str = "operation users.watch\n    param topic text required\n    output json\n    emits json\n    with grpc users.Watch as rpc\n        timeout \"20s\"\n        message {topic: topic}\n        for message in rpc\n            emit message\n        end\n        return rpc.completion\n    end\nend\n\n";

const UPLOAD: &str = "operation users.upload\n    param items json required\n    output json\n    with grpc users.Upload as rpc\n        timeout \"10s\"\n        for item in items\n            rpc.send item\n        end\n        rpc.finish_send\n        response = rpc.result\n        return response.message\n    end\nend\n\n";

const CHAT: &str = "operation chat.batch\n    param items json required\n    output json\n    emits json\n    with grpc users.Chat as rpc\n        timeout \"30s\"\n        concurrent limit 2 fail fast\n            task send\n                for message in items\n                    rpc.send message\n                end\n                rpc.finish_send\n            end\n            task receive\n                for message in rpc\n                    emit message\n                end\n            end\n        end\n        return rpc.completion\n    end\nend\n\n";

// ---------------------------------------------------------------- tests

// vhco:test grpc.invoke_rpc -- unary GetUser returns GrpcResponse {message, initial_metadata, trailers, status} only after OK; metadata is sent and secrets in response metadata are redacted
#[tokio::test]
async fn unary_get_user() {
    let s = start_plain().await;
    let b = std_bundle(&s, UNARY);
    let rt = b.rt.as_ref().unwrap();
    let (r, _) = call(rt, "users.grpc_get", serde_json::json!({"id": "42"})).await;
    let v = r.unwrap();
    assert_eq!(
        v.get("message"),
        Some(&obj(serde_json::json!({"id": "42", "name": "Ada"})))
    );
    assert_eq!(v.get("status"), Some(&Value::text("OK")));
    let md = v.get("initial_metadata").unwrap();
    assert_eq!(
        md.get("x-request-id"),
        Some(&Value::text("fixture-request"))
    );
    assert_eq!(md.get("x-auth-token"), Some(&Value::text("[redacted]")));
    assert!(v.get("trailers").is_some());
}

// vhco:test grpc.invoke_rpc -- non-OK statuses map to the registry: NOT_FOUND, PERMISSION_DENIED, UNAUTHENTICATED, and a second unary response is a protocol error
#[tokio::test]
async fn unary_status_mapping() {
    let s = start_plain().await;
    let b = std_bundle(&s, UNARY);
    let rt = b.rt.as_ref().unwrap();
    let e = err(
        call(rt, "users.grpc_get", serde_json::json!({"id": "nobody"}))
            .await
            .0,
    );
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::NotFound, "grpc.not_found")
    );
    assert_eq!(e.details.get("grpc_status"), Some(&Value::Int(5)));
    let e = err(
        call(rt, "users.grpc_get", serde_json::json!({"id": "denied"}))
            .await
            .0,
    );
    assert_eq!(e.kind, ErrorKind::Permission);
    let e = err(
        call(rt, "users.grpc_get", serde_json::json!({"id": "unauth"}))
            .await
            .0,
    );
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Conflict, "auth.login_required")
    );
    let e = err(
        call(rt, "users.grpc_get", serde_json::json!({"id": "twice"}))
            .await
            .0,
    );
    assert_eq!(e.code, "grpc.cardinality");
}

// vhco:test grpc.invoke_rpc -- the `timeout` option bounds the call; DEADLINE_EXCEEDED is a timeout, distinct from UNAVAILABLE
#[tokio::test]
async fn deadline_is_timeout() {
    let s = start_plain().await;
    let b = std_bundle(
        &s,
        "operation users.slow\n    output json\n    response = grpc users.GetUser\n        timeout \"300ms\"\n        message {id: \"slow\"}\n    end\n    return response\nend\n",
    );
    let started = std::time::Instant::now();
    let e = err(
        call(b.rt.as_ref().unwrap(), "users.slow", serde_json::json!({}))
            .await
            .0,
    );
    assert_eq!(e.kind, ErrorKind::Timeout, "{e:?}");
    assert_eq!(e.code, "grpc.deadline_exceeded");
    assert!(started.elapsed() < Duration::from_secs(2));
}

// vhco:test grpc.invoke_rpc -- server streaming emits every message in order, then completion reports the final OK
#[tokio::test]
async fn server_stream_then_completion() {
    let s = start_plain().await;
    let b = std_bundle(&s, WATCH);
    let (r, items) = call(
        b.rt.as_ref().unwrap(),
        "users.watch",
        serde_json::json!({"topic": "changes"}),
    )
    .await;
    let v = r.unwrap();
    assert_eq!(
        items,
        vec![
            obj(serde_json::json!({"text": "change 1"})),
            obj(serde_json::json!({"text": "change 2"}))
        ]
    );
    assert_eq!(v.get("status"), Some(&Value::text("OK")));
    assert_eq!(v.get("data_count"), Some(&Value::Int(2)));
}

// vhco:test grpc.invoke_rpc -- S117: one message then UNAVAILABLE trailers is one terminal grpc.unavailable error (exit 5) that keeps the emitted item
#[tokio::test]
async fn partial_stream_then_unavailable() {
    let s = start_plain().await;
    let b = std_bundle(&s, WATCH);
    let (r, items) = call(
        b.rt.as_ref().unwrap(),
        "users.watch",
        serde_json::json!({"topic": "fail_after_one"}),
    )
    .await;
    assert_eq!(items, vec![obj(serde_json::json!({"text": "change 1"}))]);
    let e = err(r);
    assert_eq!(e.code, "grpc.unavailable");
    assert_eq!(e.details.get("grpc_status"), Some(&Value::Int(14)));
    assert_eq!(e.details.get("data_count"), Some(&Value::Int(1)));
    assert_eq!(e.exit_code(), 5);
}

// vhco:test grpc.invoke_rpc -- breaking out of an endless server stream cancels the owned call at scope exit
#[tokio::test]
async fn early_break_cancels_the_stream() {
    let s = start_plain().await;
    let b = std_bundle(
        &s,
        "operation users.first\n    output json\n    first = null\n    with grpc users.Watch as rpc\n        message {topic: \"forever\"}\n        for message in rpc\n            first = message\n            break\n        end\n    end\n    return first\nend\n",
    );
    let r = tokio::time::timeout(
        Duration::from_secs(5),
        call(b.rt.as_ref().unwrap(), "users.first", serde_json::json!({})),
    )
    .await
    .expect("scope exit must not wait for the endless stream");
    assert_eq!(r.0.unwrap(), obj(serde_json::json!({"text": "tick 0"})));
}

// vhco:test grpc.invoke_rpc -- client streaming sends a finite list, half-closes and reads one result after OK
#[tokio::test]
async fn client_stream_upload() {
    let s = start_plain().await;
    let b = std_bundle(&s, UPLOAD);
    let (r, _) = call(
        b.rt.as_ref().unwrap(),
        "users.upload",
        serde_json::json!({"items": [{"text": "a"}, {"text": "b"}]}),
    )
    .await;
    assert_eq!(r.unwrap(), obj(serde_json::json!({"count": 2})));
    // An item that violates the descriptor fails before it is sent.
    let (r, _) = call(
        b.rt.as_ref().unwrap(),
        "users.upload",
        serde_json::json!({"items": [{"text": "a"}, {"txt": "b"}]}),
    )
    .await;
    assert_eq!(err(r).code, "grpc.invalid_message");
}

// vhco:test grpc.invoke_rpc -- bidirectional: a send task and a receive task progress independently under `concurrent`; completion follows OK
#[tokio::test]
async fn bidi_concurrent_send_and_receive() {
    let s = start_plain().await;
    let b = std_bundle(&s, CHAT);
    let (r, items) = tokio::time::timeout(
        Duration::from_secs(10),
        call(
            b.rt.as_ref().unwrap(),
            "chat.batch",
            serde_json::json!({"items": [{"text": "hello"}, {"text": "goodbye"}]}),
        ),
    )
    .await
    .expect("bidi must not deadlock");
    assert_eq!(
        items,
        vec![
            obj(serde_json::json!({"text": "hello"})),
            obj(serde_json::json!({"text": "goodbye"}))
        ]
    );
    assert_eq!(r.unwrap().get("status"), Some(&Value::text("OK")));
}

// vhco:test grpc.invoke_rpc -- early server finish: the receive side still reports the real OK status and later sends fail grpc.input_closed
#[tokio::test]
async fn early_server_finish_closes_input() {
    let s = start_plain().await;
    let b = std_bundle(
        &s,
        "operation chat.early\n    output json\n    n = 0\n    late = \"sent\"\n    with grpc users.Chat as rpc\n        rpc.send {text: \"hi\"}\n        rpc.send {text: \"bye\"}\n        for message in rpc\n            n += 1\n        end\n        try\n            rpc.send {text: \"late\"}\n        catch error code \"grpc.input_closed\"\n            late = \"closed\"\n        end\n        done = rpc.completion\n    end\n    return {n: n, late: late, status: done.status}\nend\n",
    );
    let (r, _) = call(b.rt.as_ref().unwrap(), "chat.early", serde_json::json!({})).await;
    assert_eq!(
        r.unwrap(),
        obj(serde_json::json!({"n": 2, "late": "closed", "status": "OK"}))
    );
}

// vhco:test grpc.invoke_rpc -- ProtoJSON mapping: int64/uint64 as decimal strings, bytes base64, map, oneof, enum names, presence, Timestamp and non-finite doubles round-trip losslessly
#[tokio::test]
async fn protojson_mapping_round_trips() {
    let s = start_plain().await;
    let b = std_bundle(
        &s,
        "operation types.echo\n    param sample json required\n    output json\n    response = grpc types.Echo\n        message sample\n    end\n    return response.message\nend\n",
    );
    let rt = b.rt.as_ref().unwrap();
    let sample = serde_json::json!({
        "big": "9007199254740993",
        "ubig": "18446744073709551615",
        "blob": "aGVsbG8=",
        "counts": {"a": 1, "b": 2},
        "name": "x",
        "color": "RED",
        "tags": ["t1", "t2"],
        "maybe": 0,
        "at": "2026-09-28T10:00:00Z",
        "ratio": "NaN"
    });
    let (r, _) = call(rt, "types.echo", serde_json::json!({"sample": sample})).await;
    let v = r.unwrap();
    assert_eq!(v.get("big"), Some(&Value::text("9007199254740993")));
    assert_eq!(v.get("ubig"), Some(&Value::text("18446744073709551615")));
    assert_eq!(v.get("blob"), Some(&Value::text("aGVsbG8=")));
    assert_eq!(
        v.get("counts").and_then(|c| c.get("b")),
        Some(&Value::Int(2))
    );
    assert_eq!(v.get("name"), Some(&Value::text("x")));
    assert_eq!(v.get("number"), None, "unset oneof member is absent");
    assert_eq!(v.get("color"), Some(&Value::text("RED")));
    assert_eq!(
        v.get("maybe"),
        Some(&Value::Int(0)),
        "explicit presence keeps 0"
    );
    assert_eq!(v.get("at"), Some(&Value::text("2026-09-28T10:00:00Z")));
    assert_eq!(v.get("ratio"), Some(&Value::text("NaN")));
    let before = s.calls.load(Ordering::SeqCst);
    for bad in [
        serde_json::json!({"unknown": 1}),
        serde_json::json!({"name": "x", "number": 2}),
        serde_json::json!({"color": "PURPLE"}),
        serde_json::json!({"blob": "***"}),
        serde_json::json!({"big": "99999999999999999999"}),
    ] {
        let e = err(call(rt, "types.echo", serde_json::json!({"sample": bad}))
            .await
            .0);
        assert_eq!(e.code, "grpc.invalid_message", "{e:?}");
    }
    assert_eq!(
        s.calls.load(Ordering::SeqCst),
        before,
        "invalid input never dials"
    );
}

// vhco:test grpc.invoke_rpc -- policy: a missing allow_grpc method grant or network grant is permission.denied before any connection
#[tokio::test]
async fn policy_denial_before_dialing() {
    let s = start_plain().await;
    let origin = format!("http://{}", s.addr);
    let only_network = bundle(
        &format!("{}{UNARY}", connector(&origin)),
        &policy(&format!(
            r#"{{"capability": "allow_network", "targets": ["{origin}"]}}"#
        )),
    );
    let e = err(call(
        only_network.rt.as_ref().unwrap(),
        "users.grpc_get",
        serde_json::json!({"id": "42"}),
    )
    .await
    .0);
    assert_eq!(e.code, "permission.denied");
    assert!(
        e.message.contains("users/example.Users/GetUser"),
        "{}",
        e.message
    );
    let only_method = bundle(
        &format!("{}{UNARY}", connector(&origin)),
        &policy(r#"{"capability": "allow_grpc", "targets": ["users/example.Users/GetUser"]}"#),
    );
    let e = err(call(
        only_method.rt.as_ref().unwrap(),
        "users.grpc_get",
        serde_json::json!({"id": "42"}),
    )
    .await
    .0);
    assert_eq!(e.code, "permission.denied");
    assert!(e.message.contains("allow_network"), "{}", e.message);
    assert_eq!(
        s.calls.load(Ordering::SeqCst),
        0,
        "no call reached the server"
    );
}

// vhco:test grpc.invoke_rpc -- a DNS name resolving to a private address is refused unless that address is granted literally or private ranges are allowed
#[tokio::test]
async fn private_resolved_address_is_refused() {
    let s = start_plain().await;
    let origin = format!("http://localhost:{}", s.addr.port());
    let src = format!("{}{UNARY}", connector(&origin));
    let denied = bundle(&src, &policy(&grants(&origin)));
    let e = err(call(
        denied.rt.as_ref().unwrap(),
        "users.grpc_get",
        serde_json::json!({"id": "42"}),
    )
    .await
    .0);
    assert_eq!(e.code, "permission.denied");
    assert_eq!(s.calls.load(Ordering::SeqCst), 0);
    let allowed = bundle(
        &src,
        &format!(
            r#"{{"version": 1, "grants": [{}], "network": {{"deny_private_ranges": false}}}}"#,
            grants(&origin)
        ),
    );
    let (r, _) = call(
        allowed.rt.as_ref().unwrap(),
        "users.grpc_get",
        serde_json::json!({"id": "42"}),
    )
    .await;
    assert!(r.is_ok(), "{r:?}");
}

// vhco:test grpc.invoke_rpc -- https endpoints verify the server with `tls ca_file` (an allow_read site) and `tls server_name`
#[tokio::test]
async fn tls_with_ca_file() {
    let (s, ca) = start_tls().await;
    let origin = format!("https://127.0.0.1:{}", s.addr.port());
    let src = format!(
        "connector users grpc\n    endpoint \"{origin}\"\n    descriptor \"./schemas/users.pb\"\n    service \"example.Users\"\n    tls server_name \"localhost\"\n    tls ca_file \"./ca.pem\"\nend\n\n{UNARY}"
    );
    let ok = bundle(
        &src,
        &policy(&format!(
            r#"{},{{"capability": "allow_read", "targets": ["./ca.pem"]}}"#,
            grants(&origin)
        )),
    );
    std::fs::write(ok._dir.path().join("ca.pem"), &ca).unwrap();
    let (r, _) = call(
        ok.rt.as_ref().unwrap(),
        "users.grpc_get",
        serde_json::json!({"id": "42"}),
    )
    .await;
    assert_eq!(
        r.unwrap().get("message"),
        Some(&obj(serde_json::json!({"id": "42", "name": "Ada"})))
    );
    let no_read = bundle(&src, &policy(&grants(&origin)));
    std::fs::write(no_read._dir.path().join("ca.pem"), &ca).unwrap();
    let e = err(call(
        no_read.rt.as_ref().unwrap(),
        "users.grpc_get",
        serde_json::json!({"id": "42"}),
    )
    .await
    .0);
    assert_eq!(e.code, "permission.denied");
    assert!(e.message.contains("allow_read"));
    // Without the fixture CA the platform roots reject the certificate.
    let untrusted = bundle(
        &src.replace("    tls ca_file \"./ca.pem\"\n", ""),
        &policy(&grants(&origin)),
    );
    let e = err(call(
        untrusted.rt.as_ref().unwrap(),
        "users.grpc_get",
        serde_json::json!({"id": "42"}),
    )
    .await
    .0);
    assert_eq!(e.kind, ErrorKind::Tls, "{e:?}");
}

// vhco:test grpc.invoke_rpc -- bundle load fails on a missing descriptor, unknown method or a call form that contradicts the descriptor cardinality
#[test]
fn load_time_descriptor_checks() {
    let origin = "http://127.0.0.1:1";
    let load = |ops: &str| {
        let b = bundle(
            &format!("{}{ops}", connector(origin)),
            &policy(&grants(origin)),
        );
        b.rt.err().expect("load must fail")
    };
    let e = load(
        "operation a.b\n    output json\n    r = grpc users.Nope\n        message {id: \"1\"}\n    end\n    return r\nend\n",
    );
    assert_eq!(e.code, "grpc.unknown_method");
    let e = load(
        "operation a.b\n    output json\n    r = grpc users.Watch\n        message {topic: \"x\"}\n    end\n    return r\nend\n",
    );
    assert_eq!(e.code, "grpc.mode");
    let e = load(
        "operation a.b\n    output json\n    with grpc users.GetUser as rpc\n        return rpc.result\n    end\nend\n",
    );
    assert_eq!(e.code, "grpc.mode");
    let e = load(
        "operation a.b\n    output json\n    with grpc users.Upload as rpc\n        message {text: \"x\"}\n        return rpc.result\n    end\nend\n",
    );
    assert_eq!(e.code, "grpc.mode");
    let e = load(
        "operation a.b\n    output json\n    with grpc users.Upload as rpc\n        for m in rpc\n            x = m\n        end\n        return rpc.result\n    end\nend\n",
    );
    assert_eq!(e.code, "grpc.mode");
    let e = load(
        "operation a.b\n    output json\n    with grpc users.Watch as rpc\n        message {topic: \"x\"}\n        rpc.send {text: \"no\"}\n        return rpc.completion\n    end\nend\n",
    );
    assert_eq!(e.code, "grpc.mode");
    // Missing descriptor file: not_found (exit 4) at load.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("app.rivet"),
        format!("{}{UNARY}", connector(origin)),
    )
    .unwrap();
    let e = Runtime::builder()
        .file(dir.path().join("app.rivet").to_str().unwrap())
        .build()
        .err()
        .unwrap();
    assert_eq!(
        (e.code.as_str(), e.exit_code()),
        ("not_found.descriptor", 4)
    );
}

// vhco:test grpc.invoke_rpc -- `auth PROFILE account A` is unsupported.auth until OAuth exists; reserved metadata is rejected; nothing dials
#[tokio::test]
async fn auth_and_reserved_metadata_rejected() {
    let s = start_plain().await;
    let b = std_bundle(
        &s,
        "auth svc oauth2\n    flow client_credentials\nend\n\noperation users.secure_get\n    output json\n    response = grpc users.GetUser\n        message {id: \"42\"}\n        auth svc account \"service\"\n    end\n    return response\nend\n\noperation users.reserved\n    output json\n    response = grpc users.GetUser\n        message {id: \"42\"}\n        metadata \"grpc-timeout\" \"1S\"\n    end\n    return response\nend\n",
    );
    let rt = b.rt.as_ref().unwrap();
    let e = err(call(rt, "users.secure_get", serde_json::json!({})).await.0);
    assert_eq!(e.code, "unsupported.auth");
    let e = err(call(rt, "users.reserved", serde_json::json!({})).await.0);
    assert_eq!(e.code, "grpc.metadata");
    assert_eq!(s.calls.load(Ordering::SeqCst), 0);
}

// vhco:test grpc.invoke_rpc -- docs/demos/10-grpc compiles and its README flows (unary, watch, late UNAVAILABLE, upload) run against the fixture
#[tokio::test]
async fn demo_10_grpc_flows() {
    let s = start_plain().await;
    let origin = format!("http://{}", s.addr);
    let src = std::fs::read_to_string("docs/demos/10-grpc/app.rivet")
        .unwrap()
        .replace("https://users.example.com:443", &origin);
    let demo_policy = std::fs::read_to_string("docs/demos/10-grpc/policy.json")
        .unwrap()
        .replace("https://users.example.com:443", &origin);
    let b = bundle(&src, &demo_policy);
    let rt = b.rt.as_ref().unwrap();
    let (r, _) = call(rt, "users.grpc_get", serde_json::json!({"id": "42"})).await;
    assert_eq!(
        r.unwrap(),
        obj(serde_json::json!({"id": "42", "name": "Ada"}))
    );
    let (r, items) = call(rt, "users.watch", serde_json::json!({"topic": "changes"})).await;
    assert_eq!(items.len(), 2);
    assert_eq!(r.unwrap().get("status"), Some(&Value::text("OK")));
    let (r, items) = call(
        rt,
        "users.watch",
        serde_json::json!({"topic": "fail_after_one"}),
    )
    .await;
    assert_eq!(items.len(), 1);
    assert_eq!(err(r).code, "grpc.unavailable");
    let (r, _) = call(
        rt,
        "users.upload",
        serde_json::json!({"items": [{"text": "a"}, {"text": "b"}]}),
    )
    .await;
    assert_eq!(r.unwrap(), obj(serde_json::json!({"count": 2})));
}

#[test]
fn fixture_descriptors_match_the_demo_schema() {
    let pool = pool();
    let users = pool.get_service_by_name("example.Users").unwrap();
    let modes: Vec<_> = users
        .methods()
        .map(|m| {
            (
                m.name().to_string(),
                m.is_client_streaming(),
                m.is_server_streaming(),
            )
        })
        .collect();
    assert_eq!(
        modes,
        vec![
            ("GetUser".into(), false, false),
            ("Watch".into(), false, true),
            ("Upload".into(), true, false),
            ("Chat".into(), true, true)
        ]
    );
    let _ = DynamicMessage::new(users.methods().next().unwrap().input()).descriptor();
}
