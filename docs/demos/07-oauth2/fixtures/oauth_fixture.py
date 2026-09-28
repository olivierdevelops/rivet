#!/usr/bin/env python3
"""Local OAuth 2.0 fixture for docs/demos/07-oauth2 (Python 3 stdlib only).

Two loopback HTTP servers stand in for the demo's example hosts:

    auth origin  http://127.0.0.1:<auth_port>   (replaces https://auth.example.com)
        POST /token   grant_type=client_credentials, client_auth basic
                      -> 200 {"access_token": "DEMO-AT-<n>", "token_type": "Bearer", ...}
                      -> 401 {"error": "invalid_client"}         wrong client id/secret
                      -> 400 {"error": "unsupported_grant_type"} any other grant

    api origin   http://127.0.0.1:<api_port>    (replaces https://api.example.com)
        GET  /contacts  Authorization: Bearer <a token issued above>
                      -> 200 {"contacts": [...]}
                      -> 401 {"error": "invalid_token"}          missing/unknown token

Plain http is accepted by Rivet only because both origins are loopback.
Every issued token starts with DEMO-AT- so you can grep Rivet's output,
traces and logs and prove the token never appears there.

The fixture accepts exactly one client:
    client_id      rivet-service          (matches app.rivet)
    client_secret  $DEMO_FIXTURE_SECRET   (default demo-secret-not-real)
Rivet reads its copy of the secret from $CRM_CLIENT_SECRET; export the same value.

Set DEMO_FIXTURE_TOKEN_DELAY=<seconds> to make /token answer late (timeout demo).

Usage:
    python3 fixtures/oauth_fixture.py [auth_port] [api_port]   # default 18870 18871

Each request is logged to stderr as one line (never the secret or the token):
    FIXTURE token grant=client_credentials client_ok=True -> 200 (tokens issued: 1)
    FIXTURE contacts bearer_ok=True -> 200
Stop with Ctrl-C (or kill the process).
"""

import base64
import json
import os
import sys
import threading
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CLIENT_ID = "rivet-service"
CLIENT_SECRET = os.environ.get("DEMO_FIXTURE_SECRET", "demo-secret-not-real")
TOKEN_DELAY = float(os.environ.get("DEMO_FIXTURE_TOKEN_DELAY", "0"))
CONTACTS = {"contacts": [{"name": "Ada Lovelace", "email": "ada@example.com"},
                         {"name": "Grace Hopper", "email": "grace@example.com"}]}

_lock = threading.Lock()
_issued = set()  # access tokens handed out; the api origin accepts only these


def log(msg):
    sys.stderr.write("FIXTURE %s\n" % msg)
    sys.stderr.flush()


class Base(BaseHTTPRequestHandler):
    def log_message(self, *args):  # silence the default access log
        pass

    def send_json(self, status, obj):
        body = json.dumps(obj).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("cache-control", "no-store")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


class AuthServer(Base):
    """Token endpoint: client_credentials with HTTP Basic client authentication."""

    def client_ok(self):
        header = self.headers.get("authorization", "")
        if not header.startswith("Basic "):
            return False
        try:
            raw = base64.b64decode(header[6:]).decode()
        except Exception:
            return False
        user, _, password = raw.partition(":")
        # RFC 6749 section 2.3.1: id and secret are form-urlencoded before Basic encoding.
        return (urllib.parse.unquote_plus(user) == CLIENT_ID
                and urllib.parse.unquote_plus(password) == CLIENT_SECRET)

    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        form = dict(urllib.parse.parse_qsl(self.rfile.read(length).decode()))
        if self.path != "/token":
            return self.send_json(404, {"error": "not_found"})
        if TOKEN_DELAY:
            time.sleep(TOKEN_DELAY)  # simulate a slow authorization server
        grant = form.get("grant_type")
        ok = self.client_ok()
        if grant != "client_credentials":
            status, body = 400, {"error": "unsupported_grant_type"}
        elif not ok:
            status, body = 401, {"error": "invalid_client"}
        else:
            with _lock:
                token = "DEMO-AT-%d" % (len(_issued) + 1)
                _issued.add(token)
            status = 200
            body = {"access_token": token, "token_type": "Bearer",
                    "expires_in": 3600, "scope": form.get("scope", "")}
        log("token grant=%s client_ok=%s -> %d (tokens issued: %d)"
            % (grant, ok, status, len(_issued)))
        self.send_json(status, body)


class ApiServer(Base):
    """Protected resource: GET /contacts needs a Bearer token issued by AuthServer."""

    def do_GET(self):
        if self.path != "/contacts":
            return self.send_json(404, {"error": "not_found"})
        header = self.headers.get("authorization", "")
        ok = header.startswith("Bearer ") and header[7:] in _issued
        log("contacts bearer_ok=%s -> %d" % (ok, 200 if ok else 401))
        if ok:
            self.send_json(200, CONTACTS)
        else:
            self.send_json(401, {"error": "invalid_token"})


def main():
    auth_port = int(sys.argv[1]) if len(sys.argv) > 1 else 18870
    api_port = int(sys.argv[2]) if len(sys.argv) > 2 else 18871
    auth = ThreadingHTTPServer(("127.0.0.1", auth_port), AuthServer)
    api = ThreadingHTTPServer(("127.0.0.1", api_port), ApiServer)
    threading.Thread(target=api.serve_forever, daemon=True).start()
    log("auth http://127.0.0.1:%d/token  api http://127.0.0.1:%d/contacts"
        % (auth_port, api_port))
    try:
        auth.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
