"""tour.py — the same librivet from Python through ctypes (DEMO-2026-0021).

Run from this folder with the wrapper examples/python/rivet.py beside it (or on
PYTHONPATH) and RIVET_LIB pointing at the shipped librivet.dylib / librivet.so.
"""
import rivet

print("library ", rivet.version(), "abi", rivet.abi_version())
with rivet.Rivet(file="app.rivet", policy_file="policy.json") as rt:
    print("add     ", rt.request("demo.add", a=2, b=3)["data"])
    print("global  ", rt.request("demo.greet", who="Python")["data"])
    print("stream  ", [r["data"] for r in rt.stream("events.count")])
    denied = rt.request("data.private")
    print("denied  ", denied["status"], denied["error"]["code"])
    users = rt.load("lib/users.rivet", "users")
    print("module  ", users.operations(), users.get(id=7)["data"])
