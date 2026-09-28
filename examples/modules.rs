//! Load `.rivet` files into a running runtime as module objects
//! (PROP-2026-0002 UC-11, R22).
//!
//! ```text
//!  Runtime::builder().root("examples/modules")   ── no entry file
//!     ├─ load("./users.rivet")               ─▶ Module "users"   (users.get, users.list)
//!     └─ load_as("./lib/billing.rivet", "billing") ─▶ Module "billing" (billing.invoice)
//!  users.call("get", {"id": 42})  ─▶ ResponseEnvelope{operation: "users.get", …}
//! ```
//!
//! `cargo run --example modules`

use rivet::{InputEnvelope, Runtime};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // No policy.json: deny-by-default, which these pure modules never need.
    let rt = Runtime::builder().root("examples/modules").build()?;

    let users = rt.load("./users.rivet")?; // alias "users" (the file stem)
    for op in users.operations() {
        println!("{} — {}", op.id, op.description.unwrap_or_default());
    }
    let out = users.call("get", json!({"id": 42})).await;
    println!("{}", out.to_json_pretty());

    let billing = rt.load_as("./lib/billing.rivet", "billing")?;
    let invoice = billing.call("invoice", json!({"user": 7})).await;
    println!("{}", invoice.to_json_pretty());

    // The runtime's own dispatcher sees the namespaced IDs too.
    let same = rt
        .call(InputEnvelope::new("users.list").data(json!({})))
        .await;
    println!("{}", same.to_json_string());

    // Loading the same alias twice is refused; the catalog stays as it was.
    if let Err(e) = rt.load("./users.rivet") {
        println!("{e}");
    }
    Ok(())
}
