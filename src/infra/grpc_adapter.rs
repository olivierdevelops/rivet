//! Native gRPC client over HTTP/2 (PROP-2026-0001 Increment 13; RES-2026-0002
//! finding 5): tonic channel + a small `Codec` over `prost_reflect::DynamicMessage`
//! built from the pinned FileDescriptorSet. No reflection, no protoc, no
//! generated code; compression stays off.
//!
//! Two pieces live here:
//! * `GrpcTransport` satisfies `GrpcDriver`: descriptor bootstrap, ProtoJSON
//!   mapping, dialing the broker-checked address (TLS done in our connector).
//! * `GrpcEffects` is the interpreter's `grpc` EffectAdapter: it decodes the
//!   evaluated form into a `GrpcPlan`, runs the injected `grpc.invoke_rpc` use
//!   case (policy + cardinality) and exposes the call as a scope-owned handle.

use crate::domain::errors::ErrorKind;
use crate::domain::grpc::{
    GrpcCatalog, GrpcConnectorInfo, GrpcDial, GrpcEvent, GrpcMetadataValue, GrpcMethodInfo,
    GrpcPlan, GrpcStatus, GrpcTerminal, GrpcTls, GrpcUsage, RpcMode, method_ref,
};
use crate::domain::ir::{CompiledProgram, Declaration, EffectForm, parse_duration_ms};
use crate::domain::ports::{GrpcCall, GrpcDriver, GrpcReceiver, GrpcSender, PolicyEvaluator};
use crate::domain::{RivetError, RivetResult, Value};
use crate::infra::execution_driver::{
    EffectAdapter, EffectCtx, EvalArg, EvaluatedForm, ResourceHandle, SharedResource,
};
use async_trait::async_trait;
use base64::Engine;
use futures_util::future::BoxFuture;
use hyper_util::rt::TokioIo;
use prost::Message;
use prost_reflect::{
    DescriptorPool, DynamicMessage, MessageDescriptor, ReflectMessage, SerializeOptions,
};
use sha2::Digest;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tonic::codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder};
use tonic::metadata::{KeyAndValueRef, MetadataMap};

/// Largest encoded or decoded message (tonic default, made explicit).
const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
/// Bounded queues in each direction (backpressure, never whole-stream buffering).
const QUEUE: usize = 16;
/// Metadata bounds for what scripts and traces can see.
const MAX_METADATA_ENTRIES: usize = 64;
const MAX_METADATA_VALUE: usize = 1024;

// vhco:infra grpc_adapter satisfies GrpcDriver
// vhco:file read descriptor -- connector `descriptor` FileDescriptorSet, read once at bundle load (bootstrap)
// vhco:file read tls -- connector `tls ca_file|cert_file|key_file`, read before connecting (after allow_read)
// vhco:net connect grpc -- HTTP/2 (TLS or development h2c) to the broker-checked address of the connector endpoint
pub struct GrpcTransport {
    catalog: GrpcCatalog,
    pools: HashMap<String, DescriptorPool>,
    root: String,
}

fn text_option(decl: &Declaration, key: &str) -> Option<String> {
    decl.option(key)
        .and_then(|o| o.args.first())
        .and_then(|a| a.to_expr().const_text())
}

impl GrpcTransport {
    /// Bootstrap: read and decode every grpc connector's descriptor set and
    /// resolve its service. A missing file is `not_found.descriptor`; a bad
    /// set or unknown service fails the bundle load before anything dials.
    pub fn load(program: &CompiledProgram, root: &str) -> RivetResult<GrpcTransport> {
        let mut catalog = GrpcCatalog::default();
        let mut pools = HashMap::new();
        for decl in program.connectors.iter().filter(|c| c.kind == "grpc") {
            let bad = |code: &str, m: String| {
                RivetError::validation(code, m).with_span(Some(decl.span.clone()))
            };
            let endpoint = text_option(decl, "endpoint").ok_or_else(|| {
                bad(
                    "grpc.connector",
                    format!(
                        "connector `{}` needs `endpoint \"https://host:port\"`",
                        decl.name
                    ),
                )
            })?;
            let descriptor = text_option(decl, "descriptor").ok_or_else(|| {
                bad(
                    "grpc.connector",
                    format!(
                        "connector `{}` needs `descriptor PATH` (a FileDescriptorSet made with protoc --include_imports)",
                        decl.name
                    ),
                )
            })?;
            let path = std::path::Path::new(root).join(&descriptor);
            let bytes = std::fs::read(&path).map_err(|e| {
                let kind = if e.kind() == std::io::ErrorKind::NotFound {
                    ErrorKind::NotFound
                } else {
                    ErrorKind::Validation
                };
                RivetError::new(
                    kind,
                    if kind == ErrorKind::NotFound {
                        "not_found.descriptor"
                    } else {
                        "grpc.descriptor"
                    },
                    format!("cannot read descriptor {descriptor} of connector `{}`: {e}", decl.name),
                )
                .with_span(Some(decl.span.clone()))
                .with_hint("generate it with protoc --include_imports --descriptor_set_out=… before loading the bundle")
            })?;
            let pool = DescriptorPool::decode(bytes.as_slice()).map_err(|e| {
                bad(
                    "grpc.descriptor",
                    format!("{descriptor} is not a valid FileDescriptorSet: {e}"),
                )
            })?;
            let service_name = match text_option(decl, "service") {
                Some(s) => s,
                None => {
                    let all: Vec<_> = pool.services().collect();
                    match all.as_slice() {
                        [one] => one.full_name().to_string(),
                        _ => {
                            return Err(bad(
                                "grpc.connector",
                                format!(
                                    "connector `{}` needs `service \"pkg.Service\"`",
                                    decl.name
                                ),
                            ));
                        }
                    }
                }
            };
            let service = pool.get_service_by_name(&service_name).ok_or_else(|| {
                bad(
                    "grpc.unknown_service",
                    format!("{descriptor} does not describe service {service_name}"),
                )
            })?;
            let mut tls = GrpcTls::default();
            for o in decl.options.iter().filter(|o| o.key == "tls") {
                let value = o.args.get(1).and_then(|a| a.to_expr().const_text());
                match (o.first_word(), value) {
                    (Some("server_name"), Some(v)) => tls.server_name = Some(v),
                    (Some("ca_file"), Some(v)) => tls.ca_file = Some(v),
                    (Some("cert_file"), Some(v)) => tls.cert_file = Some(v),
                    (Some("key_file"), Some(v)) => tls.key_file = Some(v),
                    _ => {
                        return Err(RivetError::validation(
                            "grpc.connector",
                            "expected `tls server_name|ca_file|cert_file|key_file \"VALUE\"`",
                        )
                        .with_span(Some(o.span.clone())));
                    }
                }
            }
            if tls.cert_file.is_some() != tls.key_file.is_some() {
                return Err(bad(
                    "grpc.connector",
                    "`tls cert_file` and `tls key_file` must be given together".into(),
                ));
            }
            let methods = service
                .methods()
                .map(|m| GrpcMethodInfo {
                    connector: decl.name.clone(),
                    service: service.full_name().to_string(),
                    method: m.name().to_string(),
                    mode: RpcMode::from_streaming(m.is_client_streaming(), m.is_server_streaming()),
                    input_type: m.input().full_name().to_string(),
                    output_type: m.output().full_name().to_string(),
                })
                .collect();
            let digest = sha2::Sha256::digest(&bytes);
            catalog.connectors.push(GrpcConnectorInfo {
                name: decl.name.clone(),
                endpoint,
                service: service_name,
                descriptor,
                descriptor_hash: format!(
                    "sha256:{}",
                    digest
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                ),
                tls,
                methods,
            });
            pools.insert(decl.name.clone(), pool);
        }
        Ok(GrpcTransport {
            catalog,
            pools,
            root: root.to_string(),
        })
    }

    fn descriptors(
        &self,
        method: &GrpcMethodInfo,
    ) -> RivetResult<(MessageDescriptor, MessageDescriptor)> {
        let m = self
            .pools
            .get(&method.connector)
            .and_then(|p| p.get_service_by_name(&method.service))
            .and_then(|s| s.methods().find(|m| m.name() == method.method))
            .ok_or_else(|| {
                RivetError::validation(
                    "grpc.unknown_method",
                    format!("no method {}", method.path()),
                )
            })?;
        Ok((m.input(), m.output()))
    }

    fn read_file(&self, path: &str) -> RivetResult<Vec<u8>> {
        std::fs::read(std::path::Path::new(&self.root).join(path)).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                RivetError::not_found("not_found.file", format!("{path} does not exist"))
            } else {
                RivetError::new(
                    ErrorKind::Tls,
                    "tls.config",
                    format!("cannot read {path}: {e}"),
                )
            }
        })
    }

    fn tls_config(&self, tls: &GrpcTls) -> RivetResult<rustls::ClientConfig> {
        let tls_err = |m: String| RivetError::new(ErrorKind::Tls, "tls.config", m);
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let builder = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
            .with_safe_default_protocol_versions()
            .map_err(|e| tls_err(e.to_string()))?;
        let builder = match &tls.ca_file {
            Some(ca) => {
                use rustls_pki_types::pem::PemObject;
                let pem = self.read_file(ca)?;
                let mut roots = rustls::RootCertStore::empty();
                for cert in rustls_pki_types::CertificateDer::pem_slice_iter(&pem) {
                    let cert = cert.map_err(|e| tls_err(format!("{ca}: {e}")))?;
                    roots.add(cert).map_err(|e| tls_err(format!("{ca}: {e}")))?;
                }
                if roots.is_empty() {
                    return Err(tls_err(format!("{ca} holds no certificate")));
                }
                builder.with_root_certificates(roots)
            }
            None => {
                use rustls_platform_verifier::BuilderVerifierExt;
                builder
                    .with_platform_verifier()
                    .map_err(|e| tls_err(e.to_string()))?
            }
        };
        let mut config = match (&tls.cert_file, &tls.key_file) {
            (Some(cert), Some(key)) => {
                use rustls_pki_types::pem::PemObject;
                let cert_pem = self.read_file(cert)?;
                let key_pem = self.read_file(key)?;
                let chain = rustls_pki_types::CertificateDer::pem_slice_iter(&cert_pem)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| tls_err(format!("{cert}: {e}")))?;
                let key = rustls_pki_types::PrivateKeyDer::from_pem_slice(&key_pem)
                    .map_err(|_| tls_err(format!("{key}: no usable private key")))?;
                builder
                    .with_client_auth_cert(chain, key)
                    .map_err(|e| tls_err(e.to_string()))?
            }
            _ => builder.with_no_client_auth(),
        };
        config.alpn_protocols = vec![b"h2".to_vec()];
        Ok(config)
    }
}

/// ProtoJSON view of a Rivet value: bytes become base64, non-finite floats
/// their ProtoJSON strings; everything else maps one-to-one.
pub fn to_proto_json(v: &Value) -> serde_json::Value {
    use serde_json::Value as J;
    match v {
        Value::Null => J::Null,
        Value::Bool(b) => J::Bool(*b),
        Value::Int(i) => J::Number((*i).into()),
        Value::Float(f) if f.is_nan() => J::String("NaN".into()),
        Value::Float(f) if f.is_infinite() => {
            J::String(if *f > 0.0 { "Infinity" } else { "-Infinity" }.into())
        }
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(J::Number)
            .unwrap_or(J::Null),
        Value::Text(s) => J::String(s.clone()),
        Value::Bytes(b) => J::String(base64::engine::general_purpose::STANDARD.encode(b)),
        Value::List(items) => J::Array(items.iter().map(to_proto_json).collect()),
        Value::Object(pairs) => J::Object(
            pairs
                .iter()
                .map(|(k, v)| (k.clone(), to_proto_json(v)))
                .collect(),
        ),
    }
}

/// Strict ProtoJSON → message (unknown fields, bad enums, conflicting oneofs,
/// out-of-range numbers and invalid base64 are `grpc.invalid_message`).
pub fn encode_message(desc: &MessageDescriptor, v: &Value) -> RivetResult<DynamicMessage> {
    DynamicMessage::deserialize(desc.clone(), to_proto_json(v)).map_err(|e| {
        RivetError::validation(
            "grpc.invalid_message",
            format!("value does not match {}: {e}", desc.full_name()),
        )
    })
}

/// Message → canonical ProtoJSON (lowerCamelCase names, 64-bit integers as
/// decimal strings, bytes as base64) including default-valued scalar fields.
pub fn decode_message(m: &DynamicMessage) -> RivetResult<Value> {
    let json = m
        .serialize_with_options(
            serde_json::value::Serializer,
            &SerializeOptions::new().skip_default_fields(false),
        )
        .map_err(|e| {
            RivetError::new(
                ErrorKind::Protocol,
                "grpc.decode",
                format!(
                    "cannot render {} as ProtoJSON: {e}",
                    m.descriptor().full_name()
                ),
            )
        })?;
    Ok(Value::from_json(&json))
}

fn redact(key: &str) -> bool {
    matches!(
        key,
        "authorization" | "proxy-authorization" | "cookie" | "set-cookie"
    ) || key.contains("token")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("api-key")
}

/// Bounded, redacted script view of gRPC metadata (grpc-* transport keys omitted).
fn metadata_value(map: &MetadataMap) -> Value {
    let mut out = Value::Object(Vec::new());
    let mut n = 0;
    for kv in map.iter() {
        let (key, value) = match kv {
            KeyAndValueRef::Ascii(k, v) => {
                (k.as_str().to_string(), v.to_str().unwrap_or("").to_string())
            }
            KeyAndValueRef::Binary(k, v) => (
                k.as_str().to_string(),
                v.to_bytes()
                    .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
                    .unwrap_or_default(),
            ),
        };
        if key.starts_with("grpc-") || key == "content-type" || key == "te" {
            continue;
        }
        if n >= MAX_METADATA_ENTRIES {
            break;
        }
        n += 1;
        let value = if redact(&key) {
            "[redacted]".to_string()
        } else {
            value.chars().take(MAX_METADATA_VALUE).collect()
        };
        match out.get_mut(&key) {
            Some(Value::List(items)) => items.push(Value::Text(value)),
            Some(existing) => {
                let first = std::mem::take(existing);
                *existing = Value::List(vec![first, Value::Text(value)]);
            }
            None => out.set(&key, Value::Text(value)),
        }
    }
    out
}

/// Codec over `DynamicMessage`; the decoder knows the method's output type.
#[derive(Clone)]
struct DynCodec {
    output: MessageDescriptor,
}

struct DynEncoder;
struct DynDecoder {
    output: MessageDescriptor,
}

impl Codec for DynCodec {
    type Encode = DynamicMessage;
    type Decode = DynamicMessage;
    type Encoder = DynEncoder;
    type Decoder = DynDecoder;

    fn encoder(&mut self) -> DynEncoder {
        DynEncoder
    }

    fn decoder(&mut self) -> DynDecoder {
        DynDecoder {
            output: self.output.clone(),
        }
    }
}

impl Encoder for DynEncoder {
    type Item = DynamicMessage;
    type Error = tonic::Status;

    fn encode(
        &mut self,
        item: DynamicMessage,
        dst: &mut EncodeBuf<'_>,
    ) -> Result<(), tonic::Status> {
        item.encode(dst)
            .map_err(|e| tonic::Status::internal(format!("encode: {e}")))
    }
}

impl Decoder for DynDecoder {
    type Item = DynamicMessage;
    type Error = tonic::Status;

    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<DynamicMessage>, tonic::Status> {
        DynamicMessage::decode(self.output.clone(), src)
            .map(Some)
            .map_err(|e| {
                tonic::Status::internal(format!("decode {}: {e}", self.output.full_name()))
            })
    }
}

trait IoStream: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> IoStream for T {}

fn connect_error(e: &(dyn std::error::Error + 'static), endpoint: &str) -> RivetError {
    let mut chain = Vec::new();
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(e);
    let mut refused = false;
    while let Some(err) = cur {
        if let Some(io) = err.downcast_ref::<std::io::Error>() {
            refused |= io.kind() == std::io::ErrorKind::ConnectionRefused;
        }
        chain.push(err.to_string());
        cur = err.source();
    }
    let text = chain.join(": ");
    let (kind, code) = if text.contains("tls handshake") {
        (ErrorKind::Tls, "tls.handshake")
    } else if refused {
        (ErrorKind::Connection, "connection.refused")
    } else {
        (ErrorKind::Connection, "connection.failed")
    };
    RivetError::new(
        kind,
        code,
        format!("gRPC connect to {endpoint} failed: {text}"),
    )
}

type TlsTarget = (
    tokio_rustls::TlsConnector,
    rustls_pki_types::ServerName<'static>,
);

/// tonic connector that dials only the checked addresses, in order (plus TLS).
#[derive(Clone)]
struct CheckedConnector {
    addresses: Vec<SocketAddr>,
    tls: Option<TlsTarget>,
}

impl tower::Service<tonic::codegen::http::Uri> for CheckedConnector {
    type Response = TokioIo<Box<dyn IoStream>>;
    type Error = std::io::Error;
    type Future = BoxFuture<'static, std::io::Result<TokioIo<Box<dyn IoStream>>>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _uri: tonic::codegen::http::Uri) -> Self::Future {
        let addresses = self.addresses.clone();
        let tls = self.tls.clone();
        Box::pin(async move {
            let mut last = std::io::Error::other("no checked address to dial");
            let mut connected = None;
            for a in addresses {
                match tokio::net::TcpStream::connect(a).await {
                    Ok(t) => {
                        connected = Some(t);
                        break;
                    }
                    Err(e) => last = e,
                }
            }
            let tcp = connected.ok_or(last)?;
            let _ = tcp.set_nodelay(true);
            let io: Box<dyn IoStream> = match tls {
                Some((connector, name)) => Box::new(
                    connector
                        .connect(name, tcp)
                        .await
                        .map_err(|e| std::io::Error::other(format!("tls handshake: {e}")))?,
                ),
                None => Box::new(tcp),
            };
            Ok(TokioIo::new(io))
        })
    }
}

/// Dial only the broker-checked address; the URI keeps the original authority
/// (`:authority`, TLS server name).
fn connect_channel(
    endpoint: tonic::transport::Endpoint,
    addresses: Vec<SocketAddr>,
    tls: Option<TlsTarget>,
) -> BoxFuture<'static, Result<tonic::transport::Channel, tonic::transport::Error>> {
    let connector = CheckedConnector { addresses, tls };
    Box::pin(async move { endpoint.connect_with_connector(connector).await })
}

enum RawEvent {
    Message(Value),
    End(GrpcTerminal),
    Fail(RivetError),
}

struct TransportSender {
    tx: Option<mpsc::Sender<DynamicMessage>>,
    input: MessageDescriptor,
    path: String,
}

#[async_trait]
impl GrpcSender for TransportSender {
    async fn send(&mut self, message: Value) -> RivetResult<()> {
        let msg = encode_message(&self.input, &message)?;
        let closed = || {
            RivetError::new(
                ErrorKind::Conflict,
                "grpc.input_closed",
                format!("{}: the call no longer accepts input", self.path),
            )
        };
        match &self.tx {
            Some(tx) => tx.send(msg).await.map_err(|_| closed()),
            None => Err(closed()),
        }
    }

    async fn finish(&mut self) {
        self.tx = None;
    }
}

struct TransportReceiver {
    rx: mpsc::Receiver<RawEvent>,
    task: tokio::task::JoinHandle<()>,
    path: String,
}

#[async_trait]
impl GrpcReceiver for TransportReceiver {
    async fn next_event(&mut self) -> RivetResult<GrpcEvent> {
        match self.rx.recv().await {
            Some(RawEvent::Message(v)) => Ok(GrpcEvent::Message(v)),
            Some(RawEvent::End(t)) => Ok(GrpcEvent::End(t)),
            Some(RawEvent::Fail(e)) => Err(e),
            None => Err(RivetError::new(
                ErrorKind::Cancelled,
                "cancelled.grpc",
                format!("{} was cancelled", self.path),
            )),
        }
    }

    fn cancel(&mut self) {
        self.task.abort();
    }
}

impl Drop for TransportReceiver {
    fn drop(&mut self) {
        // The call never outlives its owner.
        self.task.abort();
    }
}

/// Final status of a failed call. When our own deadline has already passed,
/// the peer's reaction to `grpc-timeout` (tonic servers answer CANCELLED) is
/// reported as DEADLINE_EXCEEDED so a deadline always surfaces as `timeout`.
fn terminal(status: &tonic::Status, initial: Value, expired: bool) -> GrpcTerminal {
    let code = if expired && status.code() != tonic::Code::Ok {
        4
    } else {
        status.code() as i32
    };
    GrpcTerminal {
        status: GrpcStatus::from_code(code, status.message()),
        initial_metadata: initial,
        trailers: metadata_value(status.metadata()),
    }
}

#[async_trait]
impl GrpcDriver for GrpcTransport {
    fn catalog(&self) -> &GrpcCatalog {
        &self.catalog
    }

    fn validate_input(&self, method: &GrpcMethodInfo, message: &Value) -> RivetResult<()> {
        let (input, _) = self.descriptors(method)?;
        encode_message(&input, message).map(|_| ())
    }

    async fn resolve(&self, host: &str, port: u16) -> RivetResult<Vec<IpAddr>> {
        let addrs = tokio::net::lookup_host((host, port)).await.map_err(|e| {
            RivetError::new(
                ErrorKind::Dns,
                "dns.resolve",
                format!("cannot resolve {host}: {e}"),
            )
        })?;
        Ok(addrs.map(|a| a.ip()).collect())
    }

    async fn invoke(
        &self,
        dial: GrpcDial,
    ) -> RivetResult<(Box<dyn GrpcSender>, Box<dyn GrpcReceiver>)> {
        let (input, output) = self.descriptors(&dial.method)?;
        let deadline = Duration::from_millis(dial.timeout_ms.max(1));
        let started = tokio::time::Instant::now();
        let https = dial.endpoint.starts_with("https://");
        let url = url::Url::parse(&dial.endpoint).map_err(|e| {
            RivetError::validation("grpc.endpoint", format!("{}: {e}", dial.endpoint))
        })?;
        let host = match url.host() {
            Some(url::Host::Ipv6(a)) => a.to_string(),
            Some(h) => h.to_string(),
            None => String::new(),
        };
        let tls = if https {
            let config = self.tls_config(&dial.tls)?;
            let name = dial.tls.server_name.clone().unwrap_or(host);
            let server_name =
                rustls_pki_types::ServerName::try_from(name.clone()).map_err(|_| {
                    RivetError::new(
                        ErrorKind::Tls,
                        "tls.config",
                        format!("`{name}` is not a valid TLS server name"),
                    )
                })?;
            Some((
                tokio_rustls::TlsConnector::from(Arc::new(config)),
                server_name,
            ))
        } else {
            None
        };
        let endpoint = tonic::transport::Endpoint::from_shared(dial.endpoint.clone())
            .map_err(|e| RivetError::validation("grpc.endpoint", e.to_string()))?
            .connect_timeout(deadline);
        let channel = match tokio::time::timeout(
            deadline,
            connect_channel(endpoint, dial.addresses.clone(), tls),
        )
        .await
        {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => return Err(connect_error(&e, &dial.endpoint)),
            Err(_) => {
                return Err(RivetError::new(
                    ErrorKind::Timeout,
                    "timeout.connect",
                    format!(
                        "gRPC connect to {} exceeded {} ms",
                        dial.endpoint, dial.timeout_ms
                    ),
                ));
            }
        };
        let (req_tx, req_rx) = mpsc::channel::<DynamicMessage>(QUEUE);
        let body = futures_util::stream::unfold(req_rx, |mut rx| async move {
            rx.recv().await.map(|m| (m, rx))
        });
        let mut request = tonic::Request::new(body);
        let remaining = deadline
            .saturating_sub(started.elapsed())
            .max(Duration::from_millis(1));
        request.set_timeout(remaining);
        for (k, v) in &dial.metadata {
            let bad = |m: String| RivetError::validation("grpc.metadata", m);
            match v {
                GrpcMetadataValue::Ascii(s) => {
                    let key = tonic::metadata::AsciiMetadataKey::from_bytes(k.as_bytes())
                        .map_err(|e| bad(format!("{k}: {e}")))?;
                    let val = tonic::metadata::AsciiMetadataValue::try_from(s.as_str())
                        .map_err(|e| bad(format!("{k}: {e}")))?;
                    request.metadata_mut().append(key, val);
                }
                GrpcMetadataValue::Binary(b) => {
                    let key = tonic::metadata::BinaryMetadataKey::from_bytes(k.as_bytes())
                        .map_err(|e| bad(format!("{k}: {e}")))?;
                    request
                        .metadata_mut()
                        .append_bin(key, tonic::metadata::BinaryMetadataValue::from_bytes(b));
                }
            }
        }
        let path_text = dial.method.path();
        let path = tonic::codegen::http::uri::PathAndQuery::try_from(path_text.as_str())
            .map_err(|e| RivetError::validation("grpc.method", e.to_string()))?;
        let codec = DynCodec { output };
        let (ev_tx, ev_rx) = mpsc::channel::<RawEvent>(QUEUE);
        let timeout_tx = ev_tx.clone();
        let timeout_ms = dial.timeout_ms;
        let expired = move || started.elapsed() + Duration::from_millis(10) >= deadline;
        let call = async move {
            let mut grpc = tonic::client::Grpc::new(channel)
                .max_decoding_message_size(MAX_MESSAGE_BYTES)
                .max_encoding_message_size(MAX_MESSAGE_BYTES);
            if let Err(e) = grpc.ready().await {
                let _ = ev_tx
                    .send(RawEvent::Fail(connect_error(&e, "gRPC channel")))
                    .await;
                return;
            }
            let response = match grpc.streaming(request, path, codec).await {
                Ok(r) => r,
                Err(status) => {
                    let _ = ev_tx
                        .send(RawEvent::End(terminal(
                            &status,
                            Value::Object(Vec::new()),
                            expired(),
                        )))
                        .await;
                    return;
                }
            };
            let initial = metadata_value(response.metadata());
            let mut stream = response.into_inner();
            loop {
                match stream.message().await {
                    Ok(Some(m)) => {
                        let event = match decode_message(&m) {
                            Ok(v) => RawEvent::Message(v),
                            Err(e) => {
                                let _ = ev_tx.send(RawEvent::Fail(e)).await;
                                return;
                            }
                        };
                        if ev_tx.send(event).await.is_err() {
                            return;
                        }
                    }
                    Ok(None) => {
                        let trailers = match stream.trailers().await {
                            Ok(Some(t)) => metadata_value(&t),
                            Ok(None) => Value::Object(Vec::new()),
                            Err(status) => {
                                let _ = ev_tx
                                    .send(RawEvent::End(terminal(&status, initial, expired())))
                                    .await;
                                return;
                            }
                        };
                        let _ = ev_tx
                            .send(RawEvent::End(GrpcTerminal {
                                status: GrpcStatus::ok(),
                                initial_metadata: initial,
                                trailers,
                            }))
                            .await;
                        return;
                    }
                    Err(status) => {
                        let _ = ev_tx
                            .send(RawEvent::End(terminal(&status, initial, expired())))
                            .await;
                        return;
                    }
                }
            }
        };
        let task = tokio::spawn(async move {
            if tokio::time::timeout(remaining, call).await.is_err() {
                let _ = timeout_tx
                    .send(RawEvent::End(GrpcTerminal {
                        status: GrpcStatus::from_code(
                            4,
                            format!("deadline of {timeout_ms} ms exceeded"),
                        ),
                        initial_metadata: Value::Object(Vec::new()),
                        trailers: Value::Object(Vec::new()),
                    }))
                    .await;
            }
        });
        Ok((
            Box::new(TransportSender {
                tx: Some(req_tx),
                input,
                path: path_text.clone(),
            }),
            Box::new(TransportReceiver {
                rx: ev_rx,
                task,
                path: path_text,
            }),
        ))
    }
}

/// The `grpc.invoke_rpc` use case, injected by the orchestrator.
pub type InvokeFn = dyn Fn(GrpcPlan, Arc<dyn PolicyEvaluator>) -> BoxFuture<'static, RivetResult<Box<dyn GrpcCall>>>
    + Send
    + Sync;

/// Interpreter adapter for `grpc` forms (one-shot unary and scoped streams).
pub struct GrpcEffects {
    invoke: Arc<InvokeFn>,
}

impl GrpcEffects {
    pub fn new(invoke: Arc<InvokeFn>) -> GrpcEffects {
        GrpcEffects { invoke }
    }
}

/// Decode the evaluated option lines of one grpc form into a plan.
pub fn plan_from_form(
    ctx: &EffectCtx,
    form: &EffectForm,
    args: &EvaluatedForm,
    usage: GrpcUsage,
) -> RivetResult<GrpcPlan> {
    let bad = |code: &str, m: String| RivetError::validation(code, m);
    let (connector, method) = method_ref(&form.head)
        .ok_or_else(|| bad("grpc.form", "expected `grpc CONNECTOR.Method`".into()))?;
    let mut plan = GrpcPlan {
        connector,
        method,
        usage,
        message: None,
        metadata: Vec::new(),
        timeout_ms: None,
        deadline_ms: ctx.remaining().as_millis().max(1) as u64,
        auth: None,
        principal: ctx.request.principal.clone(),
        operation_id: ctx.operation_id.clone(),
        span: Some(ctx.span.clone()),
    };
    let text = |a: Option<&EvalArg>| match a {
        Some(EvalArg::Value(Value::Text(s))) => Some(s.clone()),
        Some(EvalArg::Word(w)) => Some(w.clone()),
        _ => None,
    };
    for (key, a) in &args.options {
        match key.as_str() {
            "message" => {
                if plan.message.is_some() {
                    return Err(bad(
                        "grpc.option",
                        "only one `message` option is allowed".into(),
                    ));
                }
                match a.last() {
                    Some(EvalArg::Value(v @ Value::Object(_))) => plan.message = Some(v.clone()),
                    _ => {
                        return Err(bad(
                            "grpc.option",
                            "`message` takes an object, e.g. message {id: id}".into(),
                        ));
                    }
                }
            }
            "metadata" => {
                let k = text(a.first()).ok_or_else(|| {
                    bad("grpc.metadata", "expected `metadata \"KEY\" VALUE`".into())
                })?;
                let value = match (a.get(1).and_then(EvalArg::word), a.len()) {
                    (Some("bytes"), 3) => match &a[2] {
                        EvalArg::Value(Value::Bytes(b)) => GrpcMetadataValue::Binary(b.clone()),
                        EvalArg::Value(Value::Text(s)) => {
                            GrpcMetadataValue::Binary(s.as_bytes().to_vec())
                        }
                        _ => {
                            return Err(bad(
                                "grpc.metadata",
                                format!("`metadata \"{k}\" bytes V` needs bytes or text"),
                            ));
                        }
                    },
                    (_, 2) => match &a[1] {
                        EvalArg::Value(Value::Text(s)) => GrpcMetadataValue::Ascii(s.clone()),
                        EvalArg::Value(v @ (Value::Int(_) | Value::Float(_) | Value::Bool(_))) => {
                            GrpcMetadataValue::Ascii(v.to_display())
                        }
                        EvalArg::Value(Value::Bytes(b)) => GrpcMetadataValue::Binary(b.clone()),
                        _ => {
                            return Err(bad(
                                "grpc.metadata",
                                format!("metadata `{k}` needs a text value"),
                            ));
                        }
                    },
                    _ => return Err(bad(
                        "grpc.metadata",
                        "expected `metadata \"KEY\" VALUE` or `metadata \"KEY-bin\" bytes VALUE`"
                            .into(),
                    )),
                };
                plan.metadata.push((k, value));
            }
            "timeout" => {
                let t = text(a.first())
                    .and_then(|t| parse_duration_ms(&t))
                    .ok_or_else(|| {
                        bad(
                            "grpc.option",
                            "`timeout` takes a duration such as \"5s\"".into(),
                        )
                    })?;
                plan.timeout_ms = Some(t);
            }
            "auth" => {
                let profile = text(a.first()).ok_or_else(|| {
                    bad(
                        "grpc.option",
                        "expected `auth PROFILE account ACCOUNT`".into(),
                    )
                })?;
                let account = a
                    .iter()
                    .position(|x| x.word() == Some("account"))
                    .and_then(|i| text(a.get(i + 1)))
                    .unwrap_or_default();
                plan.auth = Some((profile, account));
            }
            other => {
                return Err(bad(
                    "grpc.option",
                    format!("`{other}` is not a grpc option (message, metadata, timeout, auth)"),
                ));
            }
        }
    }
    Ok(plan)
}

#[async_trait]
impl EffectAdapter for GrpcEffects {
    async fn run(
        &self,
        ctx: &EffectCtx,
        form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Value> {
        let plan = plan_from_form(ctx, form, &args, GrpcUsage::OneShot)?;
        let call = (self.invoke)(plan, Arc::clone(&ctx.policy)).await?;
        let result = call.result().await;
        call.close().await;
        Ok(result?.to_value())
    }

    async fn open(
        &self,
        ctx: &EffectCtx,
        form: &EffectForm,
        args: EvaluatedForm,
    ) -> RivetResult<Box<dyn ResourceHandle>> {
        let plan = plan_from_form(ctx, form, &args, GrpcUsage::Scoped)?;
        let call = (self.invoke)(plan, Arc::clone(&ctx.policy)).await?;
        Ok(Box::new(GrpcHandle {
            shared: Arc::new(GrpcShared { call }),
        }))
    }
}

struct GrpcShared {
    call: Box<dyn GrpcCall>,
}

#[async_trait]
impl SharedResource for GrpcShared {
    async fn call(&self, _ctx: &EffectCtx, method: &str, args: Vec<EvalArg>) -> RivetResult<Value> {
        match method {
            "send" => {
                let value = args
                    .iter()
                    .rev()
                    .find_map(|a| a.value().cloned())
                    .ok_or_else(|| {
                        RivetError::validation("grpc.send", "`rpc.send` needs a message value")
                    })?;
                self.call.send(value).await?;
                Ok(Value::Null)
            }
            "finish_send" => {
                self.call.finish_send().await?;
                Ok(Value::Null)
            }
            other => Err(RivetError::unsupported(
                "unsupported.method",
                format!("a gRPC call has no `{other}` method (send, finish_send)"),
            )),
        }
    }

    async fn next(&self, _ctx: &EffectCtx) -> RivetResult<Option<Value>> {
        self.call.next().await
    }

    async fn property(&self, _ctx: &EffectCtx, name: &str) -> RivetResult<Value> {
        match name {
            "result" => Ok(self.call.result().await?.to_value()),
            "completion" => Ok(self.call.completion().await?.to_value()),
            other => Err(RivetError::unsupported(
                "unsupported.property",
                format!("a gRPC call has no `{other}` property (result, completion)"),
            )),
        }
    }
}

struct GrpcHandle {
    shared: Arc<GrpcShared>,
}

#[async_trait]
impl ResourceHandle for GrpcHandle {
    async fn call(
        &mut self,
        ctx: &EffectCtx,
        method: &str,
        args: Vec<EvalArg>,
    ) -> RivetResult<Value> {
        self.shared.call(ctx, method, args).await
    }

    async fn next(&mut self, ctx: &EffectCtx) -> RivetResult<Option<Value>> {
        self.shared.next(ctx).await
    }

    async fn property(&mut self, ctx: &EffectCtx, name: &str) -> RivetResult<Value> {
        self.shared.property(ctx, name).await
    }

    async fn close(self: Box<Self>) -> RivetResult<()> {
        self.shared.call.close().await;
        Ok(())
    }

    fn shared(&self) -> Option<Arc<dyn SharedResource>> {
        Some(self.shared.clone())
    }
}
