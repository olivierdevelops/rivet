"""librivet from Python through ctypes (PROP-2026-0002 UC-06, T-11).

    python3 examples/python/demo.py [BUNDLE]      (default examples/ffi/app.rivet)
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rivet  # noqa: E402  (the example wrapper next to this file)

HERE = os.path.dirname(os.path.abspath(__file__))
bundle = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "..", "ffi", "app.rivet")

print("abi", rivet.abi_version(), "version", rivet.version())

try:
    rivet.Rivet(file=bundle, colour=True)
except rivet.RivetError as e:
    print("options-error", e.envelope["error"]["code"])

with rivet.Rivet(file=bundle) as rt:
    out = rt.request("demo.add", a=2, b=3)
    print("request", json.dumps(out))
    bad = rt.request("demo.add", a="two")
    print("invalid", bad["status"], bad["error"]["code"])

    with rt.stream("events.count") as call:
        records = list(call)
    print("stream", [r["data"] for r in records], records[-1]["type"], records[-1]["status"])

    with rt.stream("chat.echo") as chat:
        print("send", chat.send("hi")["accepted_seq"])
        print("send", chat.send("there")["accepted_seq"])
        chat.finish_input()
        records = list(chat)
    print("chat", [r["data"] for r in records])

    with rt.stream("chat.echo") as waiting:
        waiting.send("ping")
        print("first", waiting.next()["data"])
        print("poll", waiting.next(timeout_ms=50)["type"])
        waiting.cancel()
        last = list(waiting)[-1]
    print("cancel", last["status"], last["error"]["code"])

print("highlight", rivet.highlight('global api = "https://api.example.com"\n').splitlines()[0])
