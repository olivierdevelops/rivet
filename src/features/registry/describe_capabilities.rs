use crate::domain::Value;
use crate::domain::capabilities::BuildProbe;
use serde_json::{Value as Json, json};

/// One advertised feature: `support` is `supported` or `unsupported`, with a
/// reason whenever it is not supported (S102 "support/reason").
fn feature(name: &str, stage: &str, supported: bool, detail: Json) -> Json {
    let mut j = json!({
        "name": name,
        "stage": stage,
        "support": if supported { "supported" } else { "unsupported" },
    });
    if let Json::Object(extra) = detail {
        for (k, v) in extra {
            j[k] = v;
        }
    }
    j
}

// vhco:usecase registry.describe_capabilities(input: BuildProbe) -> CapabilityReport
// vhco:label Describe build capabilities
// vhco:about Reports what this build and platform support — protocols and their versions/modes, OAuth flows, the process sandbox backend (active, gated or unsupported), serve surfaces, Stage C features that are refused, and the version — without I/O or sensitive data, for any authenticated principal.
// vhco:example input={version:"0.1.0-dev", os:"macos", sandbox_backend:"macos-seatbelt", sandbox_status:"active"} => { "version": "0.1.0-dev", "sandbox": { "backend": "macos-seatbelt", "status": "active" } }
pub fn describe_capabilities(input: &BuildProbe) -> Value {
    // vhco:todo report_build -- the report states what this build and OS actually support (protocols, OAuth flows, sandbox backend and status, serve surfaces, refused Stage C forms, version) with no I/O and no sensitive data
    // vhco:step protocols -- fixed table of this build's transports: http 1.1/2/3 (strict or fallback), websocket, tcp, unix, udp (+ multicast), quic v1 (+ datagram), grpc (4 modes), mcp (stdio, http), oauth2 (client_credentials, pkce, device, refresh)
    let features = vec![
        feature(
            "http",
            "A",
            true,
            json!({"versions": ["1.1", "2", "3"], "streaming": ["sse", "jsonl", "lines", "bytes"]}),
        ),
        feature(
            "http3",
            "B",
            true,
            json!({"selection": ["strict", "prefer [3, 2]"]}),
        ),
        feature("files", "A", true, json!({"scoped": ["open"]})),
        feature("websocket", "B", true, json!({"schemes": ["ws", "wss"]})),
        feature("tcp", "B", true, json!({})),
        feature("unix", "B", cfg!(unix), json!({})),
        feature("udp", "B", true, json!({})),
        feature("udp_multicast", "B", true, json!({})),
        feature("quic_v1", "B", true, json!({"streams": ["bidi", "uni"]})),
        feature("quic_datagram", "B", true, json!({})),
        feature(
            "quic_migration",
            "B",
            false,
            json!({"reason": "no per-path policy hook; `migration true` is refused (unsupported.quic_migration)"}),
        ),
        feature(
            "quic_early_data",
            "B",
            false,
            json!({"reason": "0-RTT replay risk; `early_data true` is refused (unsupported.quic_early_data)"}),
        ),
        feature(
            "grpc",
            "B",
            true,
            json!({"modes": ["unary", "server_stream", "client_stream", "bidi"], "transport": "http2"}),
        ),
        feature(
            "mcp",
            "B",
            true,
            json!({"client_transports": ["stdio", "http"], "server_transports": ["stdio", "http"]}),
        ),
        feature("oauth2_client_credentials", "B", true, json!({})),
        feature(
            "oauth2_pkce",
            "B",
            true,
            json!({"grant": "authorization_code"}),
        ),
        feature("oauth2_device", "B", true, json!({})),
        feature("oauth2_refresh", "B", true, json!({})),
        feature(
            "oauth2_password",
            "B",
            false,
            json!({"reason": "resource-owner password grant is not supported (RFC 9700)"}),
        ),
        feature(
            "oauth2_implicit",
            "B",
            false,
            json!({"reason": "implicit grant is not supported (RFC 9700)"}),
        ),
        feature(
            "shell",
            "declined",
            false,
            json!({"reason": "unsupported.shell; use an explicit argv `command`"}),
        ),
        // vhco:step stage_c -- Stage C rows are listed as unsupported with the error each form raises
        feature(
            "named_pipes",
            "C",
            false,
            json!({"reason": "Stage C (`with pipe` fails unsupported.adapter)"}),
        ),
        feature(
            "file_watch",
            "C",
            false,
            json!({"reason": "Stage C (`with file watch` fails unsupported.stage_c)"}),
        ),
        feature(
            "tcp_tls",
            "C",
            false,
            json!({"reason": "Stage C (`tls` on tcp/unix fails unsupported.tcp_tls)"}),
        ),
        feature(
            "serve_mtls",
            "C",
            false,
            json!({"reason": "Stage C (serve mTLS fails unsupported.serve_mtls)"}),
        ),
        feature(
            "reconnect",
            "C",
            false,
            json!({"reason": "Stage C (`reconnect` fails unsupported.reconnect)"}),
        ),
        feature(
            "interactive_process",
            "C",
            false,
            json!({"reason": "Stage C (`interactive` fails unsupported.interactive)"}),
        ),
        feature(
            "custom_codecs",
            "C",
            false,
            json!({"reason": "Stage C (generic non-gRPC protobuf/custom codecs)"}),
        ),
    ];
    // vhco:step sandbox -- the backend compiled for this OS and whether it confines (active), is implemented but gated, or does not exist (unsupported)
    let sandbox = json!({
        "backend": input.sandbox_backend,
        "status": input.sandbox_status.as_str(),
        "reason": input.sandbox_reason,
    });
    Value::from_json(&json!({
        "version": input.version,
        "platform": {"os": input.os, "arch": input.arch},
        "stages": {"A": "supported", "B": "supported", "C": "unsupported"},
        "features": features,
        "sandbox": sandbox,
        "serve": {
            "surfaces": ["cli", "http", "sse", "poll", "websocket", "mcp", "library"],
            "auth": ["none", "bearer"],
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::capabilities::SandboxStatus;

    // vhco:test registry.describe_capabilities -- S102 names are listed with support/reason and Stage C is refused
    #[test]
    fn lists_s102_features_and_stage_c() {
        let v = describe_capabilities(&BuildProbe {
            version: "0.1.0-dev".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
            sandbox_backend: "linux-landlock-seccomp".into(),
            sandbox_status: SandboxStatus::Gated,
            sandbox_reason: "gated".into(),
        })
        .to_json();
        let names: Vec<&str> = v["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect();
        for n in [
            "udp",
            "udp_multicast",
            "oauth2_client_credentials",
            "oauth2_pkce",
            "oauth2_device",
            "quic_v1",
            "quic_datagram",
            "http3",
        ] {
            assert!(names.contains(&n), "{n}");
        }
        for f in v["features"].as_array().unwrap() {
            if f["support"] == "unsupported" {
                assert!(f["reason"].is_string(), "{f}");
            }
            if f["stage"] == "C" {
                assert_eq!(f["support"], "unsupported");
            }
        }
        assert_eq!(v["sandbox"]["status"], "gated");
        assert_eq!(v["version"], "0.1.0-dev");
    }
}
