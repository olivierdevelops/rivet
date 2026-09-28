//! Embed Rivet in a Rust program through the facade only (PROP-2026-0002 UC-05,
//! R10; the compiled form of `docs/demos/12-library/embedding.rs.txt`).
//!
//! ```text
//!   policy.json ──Policy::from_file─────────┐
//!   host limits ──Policy::from_json─.ceiling─┤   (a deny in either wins; limits narrow)
//!   app.rivet  ──.file(path)─────────────────┼──> Runtime::builder().build()
//!                                            │        │
//!   rt.call(InputEnvelope) ──────────────────┘        ├──> ResponseEnvelope  (every surface's shape)
//!   rt.request(id, Value, None) ──────────────────────├──> Completion        (Rust-idiomatic `?`)
//!   rt.scope(|scope| scope.stream(…)) ────────────────├──> Envelope records  (emits)
//!   rt.outputs / rt.io / rt.generate_policy ──────────└──> reports (nothing runs)
//! ```
//!
//! `cargo run --example embed [DIR]` (DIR defaults to `docs/demos/12-library`,
//! which holds `app.rivet` and `policy.json`). Another crate depends on it as
//! `rivet = { package = "rivet-runtime", git = "…", tag = "v0.2.0", default-features = false }`.

#![allow(clippy::result_large_err)] // rivet::Error carries the full error contract

use rivet::types::IoQuery;
use rivet::{Completion, Envelope, InputEnvelope, Policy, Runtime, Value};
use serde_json::json;

#[tokio::main]
async fn main() -> rivet::Result<()> {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/demos/12-library".to_string());

    // policy.json is loaded strictly (schema v1); the host ceiling can only narrow it.
    let ceiling = Policy::from_json(br#"{"version":1,"limits":{"max_concurrent_requests":8}}"#)?;
    let rt = Runtime::builder()
        .file(&format!("{dir}/app.rivet"))
        .policy(Policy::from_file(&format!("{dir}/policy.json"))?)
        .ceiling(ceiling)
        .build()?;

    // One envelope for every outcome: the same JSON as the CLI, HTTP, MCP and FFI.
    let out = rt
        .call(InputEnvelope::new("demo.add").data(json!({"a": 2, "b": 3})))
        .await;
    println!("{}", out.to_json_pretty());
    assert!(out.status().is_ok());
    assert_eq!(out.to_json()["data"], json!(5));

    // A failure is an envelope too (status "error", data null, the error object).
    let bad = rt
        .call(InputEnvelope::new("demo.add").data(json!({"a": "two"})))
        .await;
    println!("{}", bad.to_json_string());
    assert_eq!(bad.to_json()["status"], "error");

    // Rust-idiomatic: `request` returns Result<Completion, rivet::Error>.
    let c: Completion = rt
        .request(
            "demo.greet",
            Value::from_json(&json!({"person": "Ada"})),
            None,
        )
        .await?;
    assert_eq!(c.result, Value::text("Hello, Ada!"));

    // A server stream owned by a scope: unfinished requests are cancelled and
    // joined when the body returns. `next()` is Result<Option<Envelope>>.
    let items = rt
        .scope(|scope| async move {
            let mut s = scope
                .stream("events.count", Value::from_json(&json!({})))
                .await?;
            let mut items = Vec::new();
            while let Some(env) = s.next().await? {
                println!("{}", env.record().to_json_string());
                match env {
                    Envelope::Data(d) => items.push(d.data),
                    Envelope::Result(done) => assert_eq!(done.data_count, 3),
                    Envelope::Error { error, .. } => return Err(*error),
                }
            }
            Ok(items)
        })
        .await?;
    assert_eq!(items, vec![Value::Int(1), Value::Int(2), Value::Int(3)]);

    // Declared outputs, the I/O manifest and a least-privilege draft: nothing runs.
    let spec = &rt.outputs(Some("demo.add"), false)?[0];
    println!("{}", spec.to_json());
    let io = rt.io(&IoQuery {
        ids: vec!["demo.add".into()],
        format: "json".into(),
        ..IoQuery::default()
    })?;
    println!("{}", io.manifest.to_json());
    let draft = rt.generate_policy(&["events.count"])?;
    println!("{}", draft.policy_json());

    // What this build can do (`rivet.capabilities`, R11).
    println!("build features: {:?}", rivet::build_features());
    Ok(())
}
