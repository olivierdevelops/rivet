//! T-12 — FFI misuse never crashes the host (PROP-2026-0002 R14, UC-06).
//!
//! ```text
//!  NULL / non-UTF-8 argument ─▶ validation.ffi_argument envelope
//!  bad JSON (options, input, data) ─▶ validation.* envelope
//!  panic inside an entry point ─▶ internal.panic envelope (no unwind into C)
//!  double free (runtime, call, module, string) ─▶ RIVET_ERROR, no UB
//!  a handle of the wrong kind ─▶ refused, never dereferenced
//!  free while running ─▶ cancel + bounded cleanup (5 s grace)
//! ```
//!
//! These tests call the exported `extern "C"` functions directly (the rlib of
//! rivet-ffi), exactly as a C host would.

use rivet::*;
use serde_json::Value as Json;
use std::ffi::{CStr, CString, c_char};
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

const APP: &str = r#"{"source":"operation demo.add\n    param a integer required\n    param b integer default 0\n    output integer\n    return a + b\nend\n\noperation chat.echo\n    output integer\n    emits text\n    receives text\n    count = 0\n    for item in incoming\n        emit item\n        count = count + 1\n    end\n    return count\nend\n"}"#;

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

/// Take a returned string: parse it as JSON and free it (exactly once).
fn take(p: *mut c_char) -> Json {
    assert!(!p.is_null(), "a NULL string where an envelope was expected");
    let text = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string();
    assert_eq!(unsafe { rivet_string_free(p) }, RivetStatus::RIVET_OK);
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

fn code(j: &Json) -> &str {
    assert_eq!(j["status"], "error", "{j}");
    j["error"]["code"].as_str().unwrap()
}

fn runtime() -> *mut RivetRuntime {
    let mut rt = null_mut();
    let mut err = null_mut();
    let opts = c(APP);
    let s = unsafe { rivet_runtime_new(opts.as_ptr(), &mut rt, &mut err) };
    assert_eq!(s, RivetStatus::RIVET_OK);
    assert!(err.is_null());
    rt
}

// vhco:test execution.request_operation -- FFI: NULL and non-UTF-8 arguments on every entry point are validation.ffi_argument envelopes or RIVET_ERROR, never a crash
#[test]
fn null_and_bad_utf8_arguments_are_refused() {
    // runtime_new: NULL options, NULL out-parameter, non-UTF-8 options.
    let mut rt = null_mut();
    let mut err = null_mut();
    let s = unsafe { rivet_runtime_new(null(), &mut rt, &mut err) };
    assert_eq!(s, RivetStatus::RIVET_ERROR);
    assert_eq!(code(&take(err)), "validation.ffi_argument");
    let opts = c(APP);
    let mut err = null_mut();
    let s = unsafe { rivet_runtime_new(opts.as_ptr(), null_mut(), &mut err) };
    assert_eq!(s, RivetStatus::RIVET_ERROR);
    assert_eq!(code(&take(err)), "validation.ffi_argument");
    let bad = [0xffu8, 0xfe, 0x00];
    let mut err = null_mut();
    let s = unsafe { rivet_runtime_new(bad.as_ptr() as *const c_char, &mut rt, &mut err) };
    assert_eq!(s, RivetStatus::RIVET_ERROR);
    let e = take(err);
    assert_eq!(code(&e), "validation.ffi_argument");
    assert!(e["error"]["message"].as_str().unwrap().contains("UTF-8"));
    // A NULL error out-parameter is allowed: the status alone reports it.
    assert_eq!(
        unsafe { rivet_runtime_new(null(), &mut rt, null_mut()) },
        RivetStatus::RIVET_ERROR
    );

    let rt = runtime();
    unsafe {
        assert_eq!(
            code(&take(rivet_request(rt, null()))),
            "validation.ffi_argument"
        );
        assert_eq!(
            code(&take(rivet_request(rt, bad.as_ptr() as *const c_char))),
            "validation.ffi_argument"
        );
        assert_eq!(
            code(&take(rivet_request(null_mut(), c("{}").as_ptr()))),
            "validation.ffi_argument"
        );
        // A NULL call handle: every call function answers without dereferencing it.
        assert_eq!(
            code(&take(rivet_call_next(null_mut(), 0))),
            "validation.ffi_argument"
        );
        assert_eq!(
            code(&take(rivet_call_send(null_mut(), c("1").as_ptr()))),
            "validation.ffi_argument"
        );
        assert_eq!(
            code(&take(rivet_call_finish_input(null_mut()))),
            "validation.ffi_argument"
        );
        assert_eq!(rivet_call_cancel(null_mut()), RivetStatus::RIVET_ERROR);
        assert_eq!(rivet_call_free(null_mut()), RivetStatus::RIVET_ERROR);
        assert_eq!(
            code(&take(rivet_highlight(null(), null()))),
            "validation.ffi_argument"
        );
        assert_eq!(
            code(&take(rivet_highlight(
                c("operation a.b\nend\n").as_ptr(),
                c("pdf").as_ptr()
            ))),
            "validation.ffi_argument"
        );
        let mut m = null_mut();
        let mut err = null_mut();
        assert_eq!(
            rivet_load(rt, null(), null(), &mut m, &mut err),
            RivetStatus::RIVET_ERROR
        );
        assert_eq!(code(&take(err)), "validation.ffi_argument");
        assert_eq!(
            code(&take(rivet_module_operations(null_mut()))),
            "validation.ffi_argument"
        );
        assert_eq!(
            code(&take(rivet_module_call(
                null_mut(),
                c("get").as_ptr(),
                null()
            ))),
            "validation.ffi_argument"
        );
        assert_eq!(rivet_module_free(null_mut()), RivetStatus::RIVET_ERROR);
        assert_eq!(rivet_string_free(null_mut()), RivetStatus::RIVET_OK);
        assert_eq!(rivet_runtime_free(rt), RivetStatus::RIVET_OK);
    }
}

// vhco:test serve.parse_input -- FFI: malformed JSON in options, input envelopes and data is an error envelope with the input-envelope or ffi_argument code
#[test]
fn bad_json_is_an_error_envelope() {
    let mut rt = null_mut();
    let mut err = null_mut();
    for opts in ["{not json", "[1]", r#"{"file":"a.rivet","colour":1}"#] {
        let o = c(opts);
        assert_eq!(
            unsafe { rivet_runtime_new(o.as_ptr(), &mut rt, &mut err) },
            RivetStatus::RIVET_ERROR
        );
        assert_eq!(code(&take(err)), "validation.ffi_argument", "{opts}");
    }
    let rt = runtime();
    unsafe {
        let e = take(rivet_request(rt, c("{\"operation\": ").as_ptr()));
        assert_eq!(code(&e), "validation.input_envelope");
        let e = take(rivet_request(
            rt,
            c(r#"{"operation":"demo.add","error":{}}"#).as_ptr(),
        ));
        assert_eq!(code(&e), "validation.input_envelope");
        // A bad input envelope on call_start: the error is the first record, then NULL.
        let call = rivet_call_start(rt, c("[]").as_ptr());
        assert!(!call.is_null());
        assert_eq!(
            code(&take(rivet_call_next(call, 0))),
            "validation.input_envelope"
        );
        assert!(rivet_call_next(call, 0).is_null());
        assert_eq!(rivet_call_free(call), RivetStatus::RIVET_OK);
        // Bad data JSON on a running call.
        let chat = rivet_call_start(rt, c(r#"{"operation":"chat.echo"}"#).as_ptr());
        let e = take(rivet_call_send(chat, c("{oops").as_ptr()));
        assert_eq!(code(&e), "validation.ffi_argument");
        assert_eq!(e["operation"], "chat.echo");
        // A valid item of the wrong type is the input schema's validation error.
        let e = take(rivet_call_send(chat, c("42").as_ptr()));
        assert_eq!(e["status"], "error");
        assert_eq!(rivet_call_free(chat), RivetStatus::RIVET_OK);
        assert_eq!(rivet_runtime_free(rt), RivetStatus::RIVET_OK);
    }
}

// vhco:test execution.request_operation -- FFI: a panic inside an entry point (block_on inside a tokio worker) is caught and returned as an internal.panic envelope
#[test]
fn a_panic_becomes_an_internal_panic_envelope() {
    let rt = runtime();
    let rt_token = rt as usize;
    // Calling a blocking entry point from inside an async runtime makes tokio
    // panic ("Cannot start a runtime from within a runtime"): the probe.
    let host = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let out = host.block_on(async move {
        let input = c(r#"{"operation":"demo.add","data":{"a":1}}"#);
        take(unsafe { rivet_request(rt_token as *mut RivetRuntime, input.as_ptr()) })
    });
    assert_eq!(code(&out), "internal.panic");
    assert_eq!(out["error"]["kind"], "internal");
    // The runtime survives the probe.
    let ok = take(unsafe {
        rivet_request(rt, c(r#"{"operation":"demo.add","data":{"a":1}}"#).as_ptr())
    });
    assert_eq!(ok["status"], "ok");
    // The guard itself, for every other entry point.
    let j: Json = serde_json::from_str(
        &rivet_runtime::internal::orchestrator::setup_ffi::guard_text("probe", || panic!("probe")),
    )
    .unwrap();
    assert_eq!(code(&j), "internal.panic");
    assert_eq!(unsafe { rivet_runtime_free(rt) }, RivetStatus::RIVET_OK);
}

// vhco:test execution.request_operation -- FFI: double free of a runtime, call, module or string, and a handle of the wrong kind, are refused (RIVET_ERROR / ffi_argument) instead of undefined behaviour
#[test]
fn double_free_and_wrong_handles_are_refused() {
    let rt = runtime();
    unsafe {
        let s = rivet_request(rt, c(r#"{"operation":"demo.add","data":{"a":1}}"#).as_ptr());
        assert_eq!(rivet_string_free(s), RivetStatus::RIVET_OK);
        assert_eq!(rivet_string_free(s), RivetStatus::RIVET_ERROR);
        // A pointer librivet never returned is not freed.
        let foreign = CString::new("mine").unwrap().into_raw();
        assert_eq!(rivet_string_free(foreign), RivetStatus::RIVET_ERROR);
        drop(CString::from_raw(foreign));
        // rivet_version() is static.
        assert_eq!(
            rivet_string_free(rivet_version() as *mut c_char),
            RivetStatus::RIVET_ERROR
        );

        let call = rivet_call_start(rt, c(r#"{"operation":"demo.add","data":{"a":2}}"#).as_ptr());
        // A call handle is not a runtime (and the reverse): refused, not dereferenced.
        assert_eq!(
            code(&take(rivet_request(
                call as *mut RivetRuntime,
                c("{}").as_ptr()
            ))),
            "validation.ffi_argument"
        );
        assert_eq!(
            code(&take(rivet_call_next(rt as *mut RivetCall, 0))),
            "validation.ffi_argument"
        );
        let first = take(rivet_call_next(call, 5000));
        assert_eq!(first["status"], "ok");
        assert_eq!(first["data"], 2);
        assert!(rivet_call_next(call, 0).is_null());
        assert_eq!(rivet_call_free(call), RivetStatus::RIVET_OK);
        assert_eq!(rivet_call_free(call), RivetStatus::RIVET_ERROR);
        assert_eq!(
            code(&take(rivet_call_next(call, 0))),
            "validation.ffi_argument"
        );

        assert_eq!(rivet_runtime_free(rt), RivetStatus::RIVET_OK);
        assert_eq!(rivet_runtime_free(rt), RivetStatus::RIVET_ERROR);
        let e = take(rivet_request(rt, c(r#"{"operation":"demo.add"}"#).as_ptr()));
        assert_eq!(code(&e), "validation.ffi_argument");
        // A call started on a freed runtime still answers with its error.
        let late = rivet_call_start(rt, c(r#"{"operation":"demo.add"}"#).as_ptr());
        assert_eq!(
            code(&take(rivet_call_next(late, 0))),
            "validation.ffi_argument"
        );
        assert_eq!(rivet_call_free(late), RivetStatus::RIVET_OK);
    }
}

// vhco:test sessions.cancel_session -- FFI: freeing a call that is still running cancels it and returns within the 5 s grace; the runtime keeps working
#[test]
fn free_while_running_cancels_within_the_grace() {
    let rt = runtime();
    unsafe {
        let chat = rivet_call_start(rt, c(r#"{"operation":"chat.echo"}"#).as_ptr());
        let ack = take(rivet_call_send(chat, c("\"hi\"").as_ptr()));
        assert_eq!(ack["accepted_seq"], 1);
        assert_eq!(take(rivet_call_next(chat, 5000))["data"], "hi");
        // Nothing more arrives: a timeout marker, not an envelope.
        assert_eq!(
            take(rivet_call_next(chat, 20)),
            serde_json::json!({"type": "timeout"})
        );
        let started = Instant::now();
        assert_eq!(rivet_call_free(chat), RivetStatus::RIVET_OK);
        assert!(
            started.elapsed() < Duration::from_secs(6),
            "{:?}",
            started.elapsed()
        );

        // Explicit cancel: the terminal record says cancelled, then NULL.
        let chat = rivet_call_start(rt, c(r#"{"operation":"chat.echo"}"#).as_ptr());
        assert_eq!(rivet_call_cancel(chat), RivetStatus::RIVET_OK);
        let last = take(rivet_call_next(chat, 5000));
        assert_eq!(last["type"], "result");
        assert_eq!(last["status"], "cancelled");
        assert!(rivet_call_next(chat, 0).is_null());
        assert_eq!(rivet_call_free(chat), RivetStatus::RIVET_OK);

        // Freeing the runtime with a call still running drains it; the call
        // handle then reports the cancellation and is freed normally.
        let chat = rivet_call_start(rt, c(r#"{"operation":"chat.echo"}"#).as_ptr());
        let started = Instant::now();
        assert_eq!(rivet_runtime_free(rt), RivetStatus::RIVET_OK);
        assert!(started.elapsed() < Duration::from_secs(6));
        assert_eq!(rivet_call_free(chat), RivetStatus::RIVET_OK);
    }
}

// vhco:test execution.request_operation -- FFI: one runtime handle is used from several threads at once (RivetRuntime is thread-safe)
#[test]
fn a_runtime_is_shared_by_threads() {
    let rt = runtime() as usize;
    let threads: Vec<_> = (0..8)
        .map(|i| {
            std::thread::spawn(move || {
                let input = c(&format!(
                    r#"{{"operation":"demo.add","data":{{"a":{i},"b":1}}}}"#
                ));
                let out = take(unsafe { rivet_request(rt as *mut RivetRuntime, input.as_ptr()) });
                out["data"].as_i64().unwrap()
            })
        })
        .collect();
    let mut sums: Vec<i64> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    sums.sort();
    assert_eq!(sums, (1..=8).collect::<Vec<_>>());
    assert_eq!(
        unsafe { rivet_runtime_free(rt as *mut RivetRuntime) },
        RivetStatus::RIVET_OK
    );
}

// vhco:test registry.load_module -- FFI: rivet_load / rivet_module_* over a root-only runtime; load errors are envelopes; a freed module is refused
#[test]
fn module_handles() {
    let dir = std::env::temp_dir().join(format!("rivet-ffi-modules-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("users.rivet"),
        "operation get\n    param id integer required min 1\n    output json\n    return {id: id}\nend\n",
    )
    .unwrap();
    let opts = c(&format!(
        r#"{{"root":{}}}"#,
        serde_json::json!(dir.to_str().unwrap())
    ));
    let mut rt = null_mut();
    let mut err = null_mut();
    unsafe {
        assert_eq!(
            rivet_runtime_new(opts.as_ptr(), &mut rt, &mut err),
            RivetStatus::RIVET_OK
        );
        let mut m = null_mut();
        assert_eq!(
            rivet_load(rt, c("./users.rivet").as_ptr(), null(), &mut m, &mut err),
            RivetStatus::RIVET_OK
        );
        let ops = take(rivet_module_operations(m));
        assert_eq!(ops[0]["id"], "get");
        assert_eq!(ops[0]["operation"], "users.get");
        let out = take(rivet_module_call(
            m,
            c("get").as_ptr(),
            c(r#"{"id":42}"#).as_ptr(),
        ));
        assert_eq!(out["operation"], "users.get");
        assert_eq!(out["data"]["id"], 42);
        let out = take(rivet_module_call(m, c("get").as_ptr(), c("{bad").as_ptr()));
        assert_eq!(code(&out), "validation.ffi_argument");
        // Missing file and duplicate alias are error envelopes.
        let mut other = null_mut();
        let mut e1 = null_mut();
        assert_eq!(
            rivet_load(rt, c("./nope.rivet").as_ptr(), null(), &mut other, &mut e1),
            RivetStatus::RIVET_ERROR
        );
        assert_eq!(code(&take(e1)), "not_found.import");
        let mut e2 = null_mut();
        assert_eq!(
            rivet_load(rt, c("./users.rivet").as_ptr(), null(), &mut other, &mut e2),
            RivetStatus::RIVET_ERROR
        );
        assert_eq!(code(&take(e2)), "check.import_duplicate");
        let call = rivet_module_call_start(m, c("get").as_ptr(), c(r#"{"id":7}"#).as_ptr());
        let rec = take(rivet_call_next(call, 5000));
        assert_eq!(
            (rec["operation"].as_str(), rec["status"].as_str()),
            (Some("users.get"), Some("ok"))
        );
        assert_eq!(rivet_call_free(call), RivetStatus::RIVET_OK);
        assert_eq!(rivet_module_free(m), RivetStatus::RIVET_OK);
        assert_eq!(rivet_module_free(m), RivetStatus::RIVET_ERROR);
        assert_eq!(
            code(&take(rivet_module_operations(m))),
            "validation.ffi_argument"
        );
        // The module stays loaded in the runtime.
        let out = take(rivet_request(
            rt,
            c(r#"{"operation":"users.get","data":{"id":1}}"#).as_ptr(),
        ));
        assert_eq!(out["status"], "ok");
        assert_eq!(rivet_runtime_free(rt), RivetStatus::RIVET_OK);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// vhco:test registry.describe_capabilities -- FFI: rivet_abi_version is 1, rivet_version equals the crate version and rivet.capabilities reports abi_version
#[test]
fn versions_agree() {
    assert_eq!(rivet_abi_version(), 1);
    let v = unsafe { CStr::from_ptr(rivet_version()) }.to_str().unwrap();
    assert_eq!(v, env!("CARGO_PKG_VERSION"));
    assert_eq!(v, rivet_runtime::VERSION);
    let rt = runtime();
    let caps =
        take(unsafe { rivet_request(rt, c(r#"{"operation":"rivet.capabilities"}"#).as_ptr()) });
    assert_eq!(caps["data"]["abi_version"], 1);
    assert_eq!(caps["data"]["version"], v);
    assert_eq!(unsafe { rivet_runtime_free(rt) }, RivetStatus::RIVET_OK);
}
