#!/usr/bin/env python3
"""Local HTTP fixture for docs/demos/03-http (Python 3 standard library only).

Stands in for https://api.example.com during the README's "Local fixture run".
It listens on plain HTTP on loopback only.

    python3 fixtures/users_fixture.py --port 18830

Contract (JSON bodies):

    GET  /users/42      200 {"id":42,"name":"Ada"}
    GET  /users/7       alternates: 503, then 200 {"id":7,"name":"Grace"}, 503, 200, ...
                        (each call's first attempt fails, so every call shows the
                        bounded `retry 3 on status [429, 503]` recovering)
    GET  /users/9       200 {"id":9,"name":"Linus","role":"admin"}  (extra field)
    GET  /users/500     500 {"error":"boom"}
    GET  /users/<other> 404 {"error":"not found"}
    POST /users         201 {"id":101,"name":<name from the JSON body>}
    GET  /search?q=..   200 list of users whose name contains q (case-insensitive)

Every request is logged to stderr as one line: method, raw path+query, and the
`traceparent` header if the caller sent one.
"""

import argparse
import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit

USERS = {42: "Ada", 7: "Grace"}
FLAKY = {"next_is_503": True}


class Handler(BaseHTTPRequestHandler):
    server_version = "users-fixture/1"

    def log_message(self, fmt, *args):  # quiet default access log
        pass

    def _log(self, status):
        tp = self.headers.get("traceparent") or "-"
        sys.stderr.write(f"fixture {self.command} {self.path} -> {status} traceparent={tp}\n")
        sys.stderr.flush()

    def _send(self, status, body):
        data = json.dumps(body, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)
        self._log(status)

    def do_GET(self):
        parts = urlsplit(self.path)
        if parts.path.startswith("/users/"):
            raw = parts.path[len("/users/"):]
            try:
                uid = int(raw)
            except ValueError:
                return self._send(404, {"error": "not found"})
            if uid == 7:
                FLAKY["next_is_503"] = not FLAKY["next_is_503"]
                if not FLAKY["next_is_503"]:
                    return self._send(503, {"error": "warming up"})
            if uid == 9:
                return self._send(200, {"id": 9, "name": "Linus", "role": "admin"})
            if uid == 500:
                return self._send(500, {"error": "boom"})
            if uid in USERS:
                return self._send(200, {"id": uid, "name": USERS[uid]})
            return self._send(404, {"error": "not found"})
        if parts.path == "/search":
            q = parse_qs(parts.query).get("q", [""])[0].lower()
            limit = int(parse_qs(parts.query).get("limit", ["10"])[0])
            hits = [{"id": i, "name": n} for i, n in sorted(USERS.items()) if q in n.lower()]
            return self._send(200, hits[:limit])
        return self._send(404, {"error": "not found"})

    def do_POST(self):
        if urlsplit(self.path).path != "/users":
            return self._send(404, {"error": "not found"})
        n = int(self.headers.get("Content-Length") or 0)
        try:
            body = json.loads(self.rfile.read(n) or b"{}")
        except ValueError:
            return self._send(400, {"error": "bad json"})
        return self._send(201, {"id": 101, "name": body.get("name", "")})


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=int, default=18830)
    args = ap.parse_args()
    srv = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    sys.stderr.write(f"fixture listening on http://127.0.0.1:{args.port}\n")
    sys.stderr.flush()
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
