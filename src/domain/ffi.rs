//! Options of `rivet_runtime_new` (PROP-2026-0002 "C ABI surface", R13, R14).
//!
//! ```text
//!  options_json ──parse──▶ FfiOptions ──▶ RuntimeBuilder
//!   {"file": "app.rivet"}                              .file(PATH)
//!   {"source": "…", "path": "app.rivet", "root": "."}  .source(PATH, TEXT, ROOT)
//!   {"root": "."}                                      .root(DIR)      (modules loaded later)
//!   + "policy_file": PATH | "policy_json": {…}         .policy_file / .policy(Policy::from_json)
//!   + "ceiling_json": {…}                              .ceiling(Policy::from_json)
//!   + "pretty": true                                   every returned envelope is indented
//! ```

use super::errors::codes::FFI_ARGUMENT;
use super::{RivetError, RivetResult, Value};
use serde_json::Value as Json;

// vhco:domain FfiOptions { file?: string; source?: string; path?: string; root?: string; policy_file?: string; policy_json?: Json; ceiling_json?: Json; pretty?: bool }
/// How a C host builds its runtime. Exactly one bundle source: `file`,
/// `source` (with optional `path`, default `app.rivet`, and `root`, default
/// `.`), or `root` alone. At most one of `policy_file` / `policy_json`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FfiOptions {
    pub file: Option<String>,
    pub source: Option<String>,
    pub path: Option<String>,
    pub root: Option<String>,
    pub policy_file: Option<String>,
    pub policy_json: Option<Json>,
    pub ceiling_json: Option<Json>,
    pub pretty: bool,
}

const KEYS: [&str; 8] = [
    "file",
    "source",
    "path",
    "root",
    "policy_file",
    "policy_json",
    "ceiling_json",
    "pretty",
];

/// `validation.ffi_argument` (exit 2, HTTP 422) naming the argument or key.
pub fn ffi_argument(what: &str, message: impl Into<String>) -> RivetError {
    RivetError::validation(FFI_ARGUMENT, message)
        .with_details(Value::object([("argument", Value::text(what))]))
}

impl FfiOptions {
    /// Parse and check `options_json` (strict: unknown keys and wrong types
    /// are `validation.ffi_argument`).
    pub fn parse(text: &str) -> RivetResult<FfiOptions> {
        let j: Json = serde_json::from_str(text).map_err(|e| {
            ffi_argument(
                "options_json",
                format!("options_json is not valid JSON: {e}"),
            )
        })?;
        let Some(map) = j.as_object() else {
            return Err(ffi_argument(
                "options_json",
                "options_json must be a JSON object",
            ));
        };
        if let Some(k) = map.keys().find(|k| !KEYS.contains(&k.as_str())) {
            return Err(ffi_argument(
                k,
                format!("unknown option `{k}` (expected one of {})", KEYS.join(", ")),
            ));
        }
        let text_of = |k: &str| -> RivetResult<Option<String>> {
            match map.get(k) {
                None | Some(Json::Null) => Ok(None),
                Some(Json::String(s)) => Ok(Some(s.clone())),
                Some(_) => Err(ffi_argument(k, format!("option `{k}` must be a string"))),
            }
        };
        let object_of = |k: &str| -> RivetResult<Option<Json>> {
            match map.get(k) {
                None | Some(Json::Null) => Ok(None),
                Some(v @ Json::Object(_)) => Ok(Some(v.clone())),
                Some(_) => Err(ffi_argument(
                    k,
                    format!("option `{k}` must be a JSON object"),
                )),
            }
        };
        let pretty = match map.get("pretty") {
            None | Some(Json::Null) => false,
            Some(Json::Bool(b)) => *b,
            Some(_) => return Err(ffi_argument("pretty", "option `pretty` must be a boolean")),
        };
        let o = FfiOptions {
            file: text_of("file")?,
            source: text_of("source")?,
            path: text_of("path")?,
            root: text_of("root")?,
            policy_file: text_of("policy_file")?,
            policy_json: object_of("policy_json")?,
            ceiling_json: object_of("ceiling_json")?,
            pretty,
        };
        if o.file.is_some() && o.source.is_some() {
            return Err(ffi_argument(
                "options_json",
                "use `file` or `source`, not both",
            ));
        }
        if o.file.is_none() && o.source.is_none() && o.root.is_none() {
            return Err(ffi_argument(
                "options_json",
                "no bundle: set `file`, `source` (+ `path`, `root`) or `root`",
            ));
        }
        if o.path.is_some() && o.source.is_none() {
            return Err(ffi_argument(
                "path",
                "`path` names a `source`; set `source` too",
            ));
        }
        if o.policy_file.is_some() && o.policy_json.is_some() {
            return Err(ffi_argument(
                "options_json",
                "use `policy_file` or `policy_json`, not both",
            ));
        }
        Ok(o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_bundle_source_and_refuses_bad_shapes() {
        let o = FfiOptions::parse(r#"{"file":"app.rivet","pretty":true}"#).unwrap();
        assert_eq!(o.file.as_deref(), Some("app.rivet"));
        assert!(o.pretty);
        let o = FfiOptions::parse(r#"{"source":"x","root":"/tmp","policy_json":{"version":1}}"#)
            .unwrap();
        assert_eq!(o.source.as_deref(), Some("x"));
        assert!(o.policy_json.is_some());
        assert!(FfiOptions::parse(r#"{"root":"."}"#).is_ok());
        for bad in [
            "not json",
            "[]",
            "{}",
            r#"{"file":"a","source":"b"}"#,
            r#"{"file":1}"#,
            r#"{"file":"a","colour":true}"#,
            r#"{"file":"a","policy_file":"p","policy_json":{}}"#,
            r#"{"file":"a","pretty":"yes"}"#,
            r#"{"file":"a","path":"b"}"#,
        ] {
            let e = FfiOptions::parse(bad).unwrap_err();
            assert_eq!(e.code, "validation.ffi_argument", "{bad}");
            assert_eq!(e.exit_code(), 2);
        }
    }
}
