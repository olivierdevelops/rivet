//! librivet — the C ABI of Rivet (ABI version 1; PROP-2026-0002 R13–R15, R23).
//!
//! Only `#[no_mangle] extern "C"` shims live here: each converts C arguments
//! (NUL-terminated UTF-8 strings, opaque handles) and forwards to the `ffi`
//! surface in `rivet-runtime` (`src/orchestrator/setup_ffi.rs`), which holds the
//! logic, the guards and the VHCO annotations (ADR-0005 decision 3).
//!
//! ```text
//!  C / Python / Go ─▶ rivet_*(const char *json, …) ─▶ this shim ─▶ setup_ffi::* ─▶ Runtime
//!                     ◀─ char * (malloc'd UTF-8 JSON; free with rivet_string_free)
//! ```
//!
//! Ownership: every `char *` returned (except `rivet_version()`) is owned by
//! the caller and freed with `rivet_string_free`. Handles are opaque tokens:
//! a freed or foreign handle is refused (`validation.ffi_argument` or
//! `RIVET_ERROR`), never dereferenced. `RivetRuntime` and `RivetModule` are
//! thread-safe; a `RivetCall` belongs to one thread at a time.

#![allow(clippy::missing_safety_doc)] // every function documents its contract in rivet.h

use rivet_runtime::internal::orchestrator::setup_ffi as ffi;
use std::ffi::{CStr, CString, c_char};

/// Result of the functions that also hand back a handle or an error envelope.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RivetStatus {
    RIVET_OK = 0,
    RIVET_ERROR = 1,
}

/// A loaded bundle and the async runtime that drives it (thread-safe).
pub struct RivetRuntime {
    _private: [u8; 0],
}

/// One running call: pull its records with `rivet_call_next` (one thread at a time).
pub struct RivetCall {
    _private: [u8; 0],
}

/// A `.rivet` file loaded into a runtime as an object of operations (thread-safe).
pub struct RivetModule {
    _private: [u8; 0],
}

fn status(code: i32) -> RivetStatus {
    if code == ffi::RIVET_OK {
        RivetStatus::RIVET_OK
    } else {
        RivetStatus::RIVET_ERROR
    }
}

/// The bytes of a C string; NULL is `None`.
unsafe fn bytes<'a>(p: *const c_char) -> Option<&'a [u8]> {
    if p.is_null() {
        None
    } else {
        // SAFETY: the caller passes a NUL-terminated string (rivet.h contract).
        Some(unsafe { CStr::from_ptr(p) }.to_bytes())
    }
}

/// Hand a string to C (tracked, so a double `rivet_string_free` is refused).
fn out(s: String) -> *mut c_char {
    let c = CString::new(s).unwrap_or_else(|e| {
        // JSON never carries a raw NUL; keep the text before it rather than fail.
        let n = e.nul_position();
        let mut v = e.into_vec();
        v.truncate(n);
        CString::new(v).unwrap_or_default()
    });
    let p = c.into_raw();
    ffi::track_string(p as usize);
    p
}

fn out_opt(s: Option<String>) -> *mut c_char {
    s.map(out).unwrap_or(std::ptr::null_mut())
}

fn token<T>(p: *mut T) -> ffi::Handle {
    p as usize as ffi::Handle
}

fn handle<T>(h: ffi::Handle) -> *mut T {
    h as usize as *mut T
}

/// Store an error envelope in `*error_json` when the caller asked for it.
unsafe fn set_error(error_json: *mut *mut c_char, e: String) {
    if !error_json.is_null() {
        // SAFETY: a non-NULL out-parameter points at writable storage (rivet.h).
        unsafe { *error_json = out(e) };
    }
}

/// The ABI major version (1). Bumped only on a breaking change to rivet.h.
#[unsafe(no_mangle)]
pub extern "C" fn rivet_abi_version() -> u32 {
    rivet_runtime::ABI_VERSION
}

/// The library version ("0.2.0"); a static string, do not free.
#[unsafe(no_mangle)]
pub extern "C" fn rivet_version() -> *const c_char {
    ffi::VERSION_NUL.as_ptr() as *const c_char
}

/// Build a runtime from `options_json` (`{file | source+path+root | root,
/// policy_file | policy_json, ceiling_json?, pretty?}`). On success `*out` is the
/// handle and RIVET_OK is returned; otherwise `*error_json` (if not NULL) is an
/// error envelope to free with `rivet_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_runtime_new(
    options_json: *const c_char,
    out_rt: *mut *mut RivetRuntime,
    error_json: *mut *mut c_char,
) -> RivetStatus {
    let r = ffi::guard_status("rivet_runtime_new", || {
        if out_rt.is_null() {
            return Err(ffi::out_param_error("out"));
        }
        ffi::runtime_new(unsafe { bytes(options_json) })
    });
    match r {
        Ok(h) => {
            // SAFETY: checked non-NULL above.
            unsafe { *out_rt = handle(h) };
            RivetStatus::RIVET_OK
        }
        Err(e) => {
            unsafe { set_error(error_json, e) };
            RivetStatus::RIVET_ERROR
        }
    }
}

/// Cancel and drain in-flight calls (5 s grace), then free the runtime.
/// RIVET_ERROR for NULL, a foreign or an already freed handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_runtime_free(rt: *mut RivetRuntime) -> RivetStatus {
    ffi::guard_status("rivet_runtime_free", || Ok(ffi::runtime_free(token(rt))))
        .map(status)
        .unwrap_or(RivetStatus::RIVET_ERROR)
}

/// Blocking call from an input envelope (`{operation, data, deadline_ms?,
/// restrict?, pretty?}`); always returns a ResponseEnvelope.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_request(
    rt: *mut RivetRuntime,
    input_json: *const c_char,
) -> *mut c_char {
    out(ffi::guard_text("rivet_request", || {
        ffi::request(token(rt), unsafe { bytes(input_json) })
    }))
}

/// Start a call (streams and live input included). Never NULL: a failed start
/// yields its error record on the first `rivet_call_next`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_call_start(
    rt: *mut RivetRuntime,
    input_json: *const c_char,
) -> *mut RivetCall {
    let h = ffi::guard_status("rivet_call_start", || {
        Ok::<_, String>(ffi::call_start(token(rt), unsafe { bytes(input_json) }))
    })
    .unwrap_or(0);
    handle(h)
}

/// The next record (`type: data` … then `type: result`), `{"type":"timeout"}`
/// when nothing arrived within `timeout_ms` (negative waits), NULL when done.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_call_next(call: *mut RivetCall, timeout_ms: i64) -> *mut c_char {
    out_opt(ffi::guard_opt_text("rivet_call_next", || {
        ffi::call_next(token(call), timeout_ms)
    }))
}

/// Send one live input item (JSON); returns the ack or an error envelope.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_call_send(
    call: *mut RivetCall,
    data_json: *const c_char,
) -> *mut c_char {
    out(ffi::guard_text("rivet_call_send", || {
        ffi::call_send(token(call), unsafe { bytes(data_json) })
    }))
}

/// Close the call's live input; returns the ack or an error envelope.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_call_finish_input(call: *mut RivetCall) -> *mut c_char {
    out(ffi::guard_text("rivet_call_finish_input", || {
        ffi::call_finish_input(token(call))
    }))
}

/// Cancel the call; its records end with `status: "cancelled"`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_call_cancel(call: *mut RivetCall) -> RivetStatus {
    ffi::guard_status("rivet_call_cancel", || Ok(ffi::call_cancel(token(call))))
        .map(status)
        .unwrap_or(RivetStatus::RIVET_ERROR)
}

/// Free the call (a running call is cancelled first, 5 s grace). A second
/// free returns RIVET_ERROR.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_call_free(call: *mut RivetCall) -> RivetStatus {
    ffi::guard_status("rivet_call_free", || Ok(ffi::call_free(token(call))))
        .map(status)
        .unwrap_or(RivetStatus::RIVET_ERROR)
}

/// Highlight a `.rivet` source: `format` is "json" (default when NULL),
/// "html" or "ansi".
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_highlight(
    source: *const c_char,
    format: *const c_char,
) -> *mut c_char {
    out(ffi::guard_text("rivet_highlight", || {
        ffi::highlight(unsafe { bytes(source) }, unsafe { bytes(format) })
    }))
}

/// Load a `.rivet` file below the runtime root as a module (alias = file stem
/// when `alias_or_null` is NULL). RIVET_OK and `*out`, or RIVET_ERROR and
/// `*error_json`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_load(
    rt: *mut RivetRuntime,
    path: *const c_char,
    alias_or_null: *const c_char,
    out_module: *mut *mut RivetModule,
    error_json: *mut *mut c_char,
) -> RivetStatus {
    let r = ffi::guard_status("rivet_load", || {
        if out_module.is_null() {
            return Err(ffi::out_param_error("out"));
        }
        ffi::load(token(rt), unsafe { bytes(path) }, unsafe {
            bytes(alias_or_null)
        })
    });
    match r {
        Ok(h) => {
            // SAFETY: checked non-NULL above.
            unsafe { *out_module = handle(h) };
            RivetStatus::RIVET_OK
        }
        Err(e) => {
            unsafe { set_error(error_json, e) };
            RivetStatus::RIVET_ERROR
        }
    }
}

/// The module's operations as a JSON array of `{id, operation, name,
/// description, emits, receives}` (or an error envelope).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_module_operations(module: *mut RivetModule) -> *mut c_char {
    out(ffi::guard_text("rivet_module_operations", || {
        ffi::module_operations(token(module))
    }))
}

/// Blocking call of one module operation by its short ID (`"get"`); the
/// envelope names the namespaced operation (`users.get`). NULL data = `{}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_module_call(
    module: *mut RivetModule,
    id: *const c_char,
    data_json: *const c_char,
) -> *mut c_char {
    out(ffi::guard_text("rivet_module_call", || {
        ffi::module_call(token(module), unsafe { bytes(id) }, unsafe {
            bytes(data_json)
        })
    }))
}

/// Start a call of one module operation (see `rivet_call_start`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_module_call_start(
    module: *mut RivetModule,
    id: *const c_char,
    data_json: *const c_char,
) -> *mut RivetCall {
    let h = ffi::guard_status("rivet_module_call_start", || {
        Ok::<_, String>(ffi::module_call_start(
            token(module),
            unsafe { bytes(id) },
            unsafe { bytes(data_json) },
        ))
    })
    .unwrap_or(0);
    handle(h)
}

/// Free the module handle; the module stays loaded in its runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_module_free(module: *mut RivetModule) -> RivetStatus {
    ffi::guard_status("rivet_module_free", || Ok(ffi::module_free(token(module))))
        .map(status)
        .unwrap_or(RivetStatus::RIVET_ERROR)
}

/// Free a string returned by librivet. NULL is a no-op (RIVET_OK); a pointer
/// librivet did not return, or already freed, is refused (RIVET_ERROR).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rivet_string_free(s: *mut c_char) -> RivetStatus {
    if s.is_null() {
        return RivetStatus::RIVET_OK;
    }
    if !ffi::untrack_string(s as usize) {
        return RivetStatus::RIVET_ERROR;
    }
    // SAFETY: the address was produced by CString::into_raw in `out` and is
    // freed exactly once (it was just removed from the live set).
    drop(unsafe { CString::from_raw(s) });
    RivetStatus::RIVET_OK
}
