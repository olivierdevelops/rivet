//! capy-core — Rust port of the Go reference implementation.
//!
//! Module layout mirrors the Go package layout file-for-file:
//!
//! ```text
//! domain/        → src/domain/          (pure data model)
//! infra/         → src/infra/           (lexing, helpers, parsing of .capy libs)
//! orchestrator/  → src/orchestrator/    (loader, parser, evaluator)
//! capy.go        → src/lib.rs           (public embedding API)
//! ```

pub mod capy;
pub mod domain;
pub mod features;
pub mod gofmt;
pub mod gojson;
pub mod gopath;
pub mod infra;
pub mod io;
pub mod usecases;
pub mod orchestrator;
