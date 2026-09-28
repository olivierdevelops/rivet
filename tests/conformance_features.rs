//! T-10 — Cargo features (PROP-2026-0002 R11, R12; ADR-0005 decision 4).
//!
//! ```text
//!  build ──cfg(feature = …)──▶ rivet::build_features() == rivet.capabilities data.build_features
//!                                   │
//!  bundle ─compile─▶ effect sites ──┴─▶ require_build_features(compiled) ─▶ ok | unsupported.feature
//!                                        grpc connector / grpc effect ─▶ grpc      (exit 5, HTTP 501,
//!                                        quic effect / HTTP/3 ─────────▶ quic       details.feature)
//!                                        auth NAME oauth2 ─────────────▶ oauth
//!  cargo package --list ─▶ src/**, src/infra/rivet.capy, editors/keywords.json; no tests/ or docs/
//! ```
//!
//! This suite compiles under every feature combination: CI's feature matrix
//! runs it with `--no-default-features` and each single feature, where the
//! `cfg(not(feature = …))` tests build a real runtime without the adapter.
#![allow(clippy::result_large_err)]

use rivet::internal::domain::capabilities::require_build_features;
use rivet::internal::domain::ir::CompiledProgram;
use rivet::internal::domain::source::SourceBundle;
use rivet::internal::features::audit::effect_sites::analyze_program;
use rivet::internal::features::language::compile_program::compile_program;
use rivet::internal::infra::capy_parser::CapyParser;
use rivet::{Error, InputEnvelope, Runtime};

/// The features this test binary was compiled with (integration tests see the
/// package's feature cfgs).
fn expected() -> Vec<&'static str> {
    let mut on = Vec::new();
    for (f, enabled) in [
        ("serve", cfg!(feature = "serve")),
        ("grpc", cfg!(feature = "grpc")),
        ("quic", cfg!(feature = "quic")),
        ("oauth", cfg!(feature = "oauth")),
        ("cli", cfg!(feature = "cli")),
    ] {
        if enabled {
            on.push(f);
        }
    }
    on
}

const GRPC_CONNECTOR: &str = "connector users grpc\n    endpoint \"http://127.0.0.1:50051\"\n    descriptor \"./schemas/users.pb\"\n    service \"example.Users\"\nend\n\n";
const GRPC_OP: &str = "operation users.get\n    param id text required\n    output json\n    response = grpc users.GetUser\n        message {id: id}\n    end\n    return response\nend\n\n";
const QUIC_OP: &str = "operation engine.status\n    output json\n    with quic \"quic://engine.example.com:4433\" as connection\n        alpn \"rivet-rpc/1\"\n    end\n    return 1\nend\n\n";
const H3_OP: &str = "operation web.get\n    output json\n    r = http get \"https://api.example.com/items\"\n        version 3\n        decode json\n    end\n    return r.body\nend\n\n";
const OAUTH_PROFILE: &str = "auth crm oauth2\n    flow client_credentials\n    issuer \"https://auth.example.com\"\n    token_url \"https://auth.example.com/token\"\n    client_id \"rivet-service\"\n    client_secret env \"CRM_SECRET\"\n    client_auth post\n    scopes [\"contacts.read\"]\n    resource_origins [\"https://api.example.com\"]\n    store memory\nend\n\n";
const PURE_OP: &str = "operation demo.add\n    param a integer required\n    param b integer required\n    output integer\n    return a + b\nend\n";

fn compile(text: &str) -> CompiledProgram {
    compile_program(
        &SourceBundle::single("app.rivet", text),
        &CapyParser::new().unwrap(),
    )
    .unwrap()
}

/// The first error and its suppressed siblings.
fn all(e: &Error) -> Vec<&Error> {
    std::iter::once(e).chain(e.suppressed.iter()).collect()
}

fn feature_of(e: &Error) -> String {
    e.details.to_json()["feature"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

// vhco:test registry.describe_capabilities -- rivet.capabilities reports the compiled Cargo features as build_features (== rivet::build_features()) and abi_version 1, after the 0.1.0 keys
#[tokio::test]
async fn capabilities_report_the_compiled_features() {
    assert_eq!(rivet::build_features(), expected());
    assert_eq!(rivet::ABI_VERSION, 1);
    let rt = Runtime::builder()
        .source("app.rivet", PURE_OP, ".")
        .build()
        .unwrap();
    let out = rt.call(InputEnvelope::new("rivet.capabilities")).await;
    let j = out.to_json();
    assert_eq!(j["status"], "ok", "{j}");
    let data = &j["data"];
    assert_eq!(data["build_features"], serde_json::json!(expected()));
    assert_eq!(data["abi_version"], 1);
    assert_eq!(data["version"], rivet::VERSION);
    let keys: Vec<&str> = data
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "version",
            "platform",
            "stages",
            "features",
            "sandbox",
            "serve",
            "build_features",
            "abi_version"
        ]
    );
    // The protocol rows follow the build: a compiled-out adapter is unsupported with a reason.
    let row = |n: &str| {
        data["features"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == n)
            .unwrap()
            .clone()
    };
    for (name, feature) in [
        ("grpc", "grpc"),
        ("quic_v1", "quic"),
        ("http3", "quic"),
        ("oauth2_pkce", "oauth"),
    ] {
        let on = expected().contains(&feature);
        assert_eq!(
            row(name)["support"],
            if on { "supported" } else { "unsupported" },
            "{name}"
        );
    }
}

// vhco:test registry.describe_capabilities -- a bundle using grpc, quic, HTTP/3 and oauth2 is refused with one unsupported.feature per use (exit 5, HTTP 501, details.feature, source span) when those features are compiled out, and accepted when they are in
#[test]
fn compiled_out_adapters_are_refused_with_unsupported_feature() {
    let text = format!("{OAUTH_PROFILE}{GRPC_CONNECTOR}{GRPC_OP}{QUIC_OP}{H3_OP}{PURE_OP}");
    let program = compile(&text);
    let effects = analyze_program(&program);

    let e = require_build_features(&program, &effects, &[]).unwrap_err();
    let found: Vec<(String, u32)> = all(&e)
        .iter()
        .map(|d| {
            assert_eq!(d.code, "unsupported.feature");
            assert_eq!(d.exit_code(), 5);
            assert_eq!(d.http_status(), 501);
            assert!(d.message.contains("--features"), "{}", d.message);
            let span = d.source.clone().expect("span");
            assert_eq!(span.file, "app.rivet");
            (feature_of(d), span.start_line)
        })
        .collect();
    // In source order: the auth profile (line 1), the connector (12), the gRPC
    // effect (22), the QUIC scope (30) and the HTTP/3 request (38).
    let features: Vec<&str> = found.iter().map(|(f, _)| f.as_str()).collect();
    assert_eq!(
        features,
        ["oauth", "grpc", "grpc", "quic", "quic"],
        "{found:?}"
    );
    let lines: Vec<u32> = found.iter().map(|(_, l)| *l).collect();
    assert!(lines.windows(2).all(|w| w[0] <= w[1]), "{lines:?}");

    // Each feature on its own clears only its own uses.
    for (on, left) in [
        (&["grpc"][..], vec!["oauth", "quic", "quic"]),
        (&["quic"][..], vec!["oauth", "grpc", "grpc"]),
        (&["oauth"][..], vec!["grpc", "grpc", "quic", "quic"]),
    ] {
        let e = require_build_features(&program, &effects, on).unwrap_err();
        let got: Vec<String> = all(&e).iter().map(|d| feature_of(d)).collect();
        assert_eq!(got, left, "with {on:?}");
    }
    require_build_features(&program, &effects, &["grpc", "quic", "oauth"]).unwrap();

    // A pure bundle needs nothing.
    let pure = compile(PURE_OP);
    require_build_features(&pure, &analyze_program(&pure), &[]).unwrap();
}

/// Build a runtime and return its load error (`unsupported.feature`).
#[allow(dead_code)]
fn load_error(text: &str) -> Error {
    match Runtime::builder().source("app.rivet", text, ".").build() {
        Ok(_) => panic!("the bundle loaded in a build without its feature"),
        Err(e) => e,
    }
}

// vhco:test registry.describe_capabilities -- without the quic feature a bundle using HTTP/3 or a QUIC scope fails at load with unsupported.feature {feature: quic}
#[cfg(not(feature = "quic"))]
#[test]
fn without_quic_http3_and_quic_fail_at_load() {
    for text in [H3_OP, QUIC_OP] {
        let e = load_error(text);
        assert_eq!(e.code, "unsupported.feature");
        assert_eq!(feature_of(&e), "quic");
        assert_eq!(e.exit_code(), 5);
    }
}

// vhco:test registry.describe_capabilities -- without the grpc feature a bundle with a grpc connector fails at load with unsupported.feature {feature: grpc} before any descriptor is read
#[cfg(not(feature = "grpc"))]
#[test]
fn without_grpc_a_grpc_bundle_fails_at_load() {
    let e = load_error(&format!("{GRPC_CONNECTOR}{GRPC_OP}"));
    assert_eq!(e.code, "unsupported.feature");
    assert_eq!(feature_of(&e), "grpc");
    assert_eq!(e.http_status(), 501);
}

// vhco:test registry.describe_capabilities -- without the oauth feature an `auth NAME oauth2` profile fails at load with unsupported.feature {feature: oauth}
#[cfg(not(feature = "oauth"))]
#[test]
fn without_oauth_an_auth_profile_fails_at_load() {
    let e = load_error(&format!("{OAUTH_PROFILE}{PURE_OP}"));
    assert_eq!(e.code, "unsupported.feature");
    assert_eq!(feature_of(&e), "oauth");
}

// vhco:test serve.start_serve -- a CLI built without the serve feature keeps `rivet serve` and refuses it with unsupported.feature (exit 5)
#[cfg(all(feature = "cli", not(feature = "serve")))]
#[test]
fn without_serve_the_cli_refuses_serve() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("app.rivet"), PURE_OP).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_rivet"))
        .args(["--file", "app.rivet", "serve", "--stdio"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(5));
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("unsupported.feature"), "{text}");
}

// vhco:test registry.describe_capabilities -- R12 packaging: `cargo package --list` ships the library, the grammar and editors/keywords.json (embedded by highlight_source) and no tests or docs
#[test]
fn package_list_ships_the_embedded_files() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = std::process::Command::new(cargo)
        .args(["package", "--list", "--allow-dirty", "-p", "rivet-runtime"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let list = String::from_utf8_lossy(&out.stdout);
    let files: Vec<&str> = list.lines().collect();
    for want in [
        "Cargo.toml",
        "README.md",
        "src/lib.rs",
        "src/internal.rs",
        "src/main.rs",
        "src/infra/rivet.capy",
        "editors/keywords.json",
        "examples/embed.rs",
    ] {
        assert!(files.contains(&want), "missing {want}: {files:?}");
    }
    for f in &files {
        assert!(
            !f.starts_with("tests/") && !f.starts_with("docs/") && !f.starts_with("ffi/"),
            "{f} must not be packaged"
        );
    }
}
