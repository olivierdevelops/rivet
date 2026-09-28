"""A small ctypes wrapper over librivet (an example, not a package).

PROP-2026-0002 UC-06, UC-11 (R13-R15, R23):

    rt = Rivet(file="app.rivet")                  # rivet_runtime_new
    rt.request("demo.add", a=2, b=3)              # rivet_request        -> envelope dict
    for record in rt.stream("events.count"):      # rivet_call_start/next -> records
        ...
    users = rt.load("./users.rivet")              # rivet_load           -> Module
    users.get(id=42)                              # rivet_module_call    -> envelope dict
    users.operations()                            # ['get', 'list']

Every librivet string is copied into Python and freed with rivet_string_free.
The library is found through RIVET_LIB, else target/release (then target/debug)
of this checkout.
"""
import ctypes
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
LIB_NAME = {"darwin": "librivet.dylib", "win32": "rivet.dll"}.get(sys.platform, "librivet.so")


def find_library():
    if os.environ.get("RIVET_LIB"):
        return os.environ["RIVET_LIB"]
    for profile in ("release", "debug"):
        path = os.path.join(ROOT, "target", profile, LIB_NAME)
        if os.path.exists(path):
            return path
    raise OSError(f"{LIB_NAME} not found: cargo build --release -p rivet-ffi, or set RIVET_LIB")


class RivetError(Exception):
    """An error envelope ({"status": "error", "error": {...}}) as an exception."""

    def __init__(self, envelope):
        self.envelope = envelope
        err = envelope.get("error") or {}
        super().__init__(f"{err.get('code')}: {err.get('message')}")


_lib = None


def library():
    """Load librivet once and declare every prototype of rivet.h."""
    global _lib
    if _lib is not None:
        return _lib
    lib = ctypes.CDLL(find_library())
    p, s, i64 = ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int64
    protos = {
        "rivet_abi_version": (ctypes.c_uint32, []),
        "rivet_version": (ctypes.c_char_p, []),
        "rivet_runtime_new": (ctypes.c_int, [s, ctypes.POINTER(p), ctypes.POINTER(p)]),
        "rivet_runtime_free": (ctypes.c_int, [p]),
        "rivet_request": (p, [p, s]),
        "rivet_call_start": (p, [p, s]),
        "rivet_call_next": (p, [p, i64]),
        "rivet_call_send": (p, [p, s]),
        "rivet_call_finish_input": (p, [p]),
        "rivet_call_cancel": (ctypes.c_int, [p]),
        "rivet_call_free": (ctypes.c_int, [p]),
        "rivet_highlight": (p, [s, s]),
        "rivet_load": (ctypes.c_int, [p, s, s, ctypes.POINTER(p), ctypes.POINTER(p)]),
        "rivet_module_operations": (p, [p]),
        "rivet_module_call": (p, [p, s, s]),
        "rivet_module_call_start": (p, [p, s, s]),
        "rivet_module_free": (ctypes.c_int, [p]),
        "rivet_string_free": (ctypes.c_int, [p]),
    }
    for name, (restype, argtypes) in protos.items():
        fn = getattr(lib, name)
        fn.restype, fn.argtypes = restype, argtypes
    _lib = lib
    return lib


def _take(ptr):
    """Copy a returned char * into a str and free it (None for NULL)."""
    if not ptr:
        return None
    try:
        return ctypes.string_at(ptr).decode("utf-8")
    finally:
        library().rivet_string_free(ptr)


def _json(ptr):
    text = _take(ptr)
    return None if text is None else json.loads(text)


def _enc(value):
    return None if value is None else json.dumps(value).encode("utf-8")


def abi_version():
    return library().rivet_abi_version()


def version():
    return library().rivet_version().decode("utf-8")


def highlight(source, fmt="json"):
    """Tokens of a .rivet source (json lines, html or ansi)."""
    return _take(library().rivet_highlight(source.encode("utf-8"), fmt.encode("utf-8")))


class Call:
    """A running call: iterate its records; send live input; cancel."""

    def __init__(self, handle):
        self._h = handle

    def next(self, timeout_ms=5000):
        """The next record dict, {'type': 'timeout'}, or None when done."""
        return _json(library().rivet_call_next(self._h, timeout_ms))

    def __iter__(self):
        while True:
            record = self.next()
            if record is None:
                return
            if record.get("type") != "timeout":
                yield record

    def send(self, item):
        return _json(library().rivet_call_send(self._h, _enc(item)))

    def finish_input(self):
        return _json(library().rivet_call_finish_input(self._h))

    def cancel(self):
        return library().rivet_call_cancel(self._h) == 0

    def close(self):
        if self._h:
            library().rivet_call_free(self._h)
            self._h = None

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()

    def __del__(self):
        self.close()


class Module:
    """A loaded .rivet file; its operations are attributes: users.get(id=42)."""

    def __init__(self, handle, alias):
        self._h = handle
        self.alias = alias

    def operations(self):
        """The module's own operation IDs (['get', 'list'])."""
        return [op["id"] for op in self.describe()]

    def describe(self):
        return _json(library().rivet_module_operations(self._h))

    def call(self, op_id, **data):
        """One envelope dict; `operation` is the namespaced ID (users.get)."""
        return _json(library().rivet_module_call(self._h, op_id.encode("utf-8"), _enc(data)))

    def stream(self, op_id, **data):
        return Call(library().rivet_module_call_start(self._h, op_id.encode("utf-8"), _enc(data)))

    def __getattr__(self, name):
        if name.startswith("_"):
            raise AttributeError(name)
        if name not in self.operations():
            raise AttributeError(f"module {self.alias!r} has no operation {name!r}")
        return lambda **data: self.call(name, **data)

    def close(self):
        if self._h:
            library().rivet_module_free(self._h)
            self._h = None

    def __del__(self):
        self.close()


class Rivet:
    """A runtime: Rivet(file=…) | Rivet(source=…, path=…, root=…) | Rivet(root=…),
    plus policy_file= / policy_json= / ceiling_json= / pretty=."""

    def __init__(self, **options):
        lib = library()
        handle, err = ctypes.c_void_p(), ctypes.c_void_p()
        if lib.rivet_runtime_new(_enc(options), ctypes.byref(handle), ctypes.byref(err)) != 0:
            raise RivetError(_json(err.value))
        self._h = handle.value

    def request(self, operation, **data):
        """Blocking call; always returns the envelope dict (errors included)."""
        return _json(library().rivet_request(self._h, _enc({"operation": operation, "data": data})))

    def stream(self, operation, **data):
        return Call(library().rivet_call_start(self._h, _enc({"operation": operation, "data": data})))

    def load(self, path, alias=None):
        module, err = ctypes.c_void_p(), ctypes.c_void_p()
        a = alias.encode("utf-8") if alias else None
        if library().rivet_load(self._h, path.encode("utf-8"), a, ctypes.byref(module), ctypes.byref(err)) != 0:
            raise RivetError(_json(err.value))
        stem = os.path.splitext(os.path.basename(path))[0]
        return Module(module.value, alias or stem)

    def close(self):
        if self._h:
            library().rivet_runtime_free(self._h)
            self._h = None

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
