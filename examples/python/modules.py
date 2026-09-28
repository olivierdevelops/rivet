"""Load .rivet files as module objects from Python (PROP-2026-0002 UC-11, T-19).

    python3 examples/python/modules.py [DIR]      (default examples/modules)
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rivet  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
root = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "..", "modules")

with rivet.Rivet(root=root) as rt:
    users = rt.load("./users.rivet")                   # alias "users" (the file stem)
    print("operations", users.operations())            # ['get', 'list']
    out = users.get(id=42)                             # an attribute per operation
    print("get", out["operation"], json.dumps(out["data"]))
    print("list", [u["id"] for u in users.list()["data"]])

    billing = rt.load("./lib/billing.rivet", alias="billing")
    print("invoice", json.dumps(billing.invoice(user=7)["data"]))

    try:
        rt.load("./users.rivet")
    except rivet.RivetError as e:
        print("duplicate", e.envelope["error"]["code"])

    try:
        users.missing()
    except AttributeError as e:
        print("no-such-operation", e)

    print("request", rt.request("users.get", id=7)["data"]["name"])
