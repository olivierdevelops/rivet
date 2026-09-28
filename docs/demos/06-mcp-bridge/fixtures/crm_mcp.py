#!/usr/bin/env python3
"""Controlled CRM MCP server for docs/demos/06-mcp-bridge (stdlib only).

Speaks MCP Streamable HTTP on POST /mcp (plus DELETE /mcp to end a session):

  initialize                 -> JSON result + `mcp-session-id: crm-fixture-session`
  notifications/*            -> 202 Accepted (no body)
  tools/list                 -> ../schemas/tools-list.fixture.json (the `search` tool)
  tools/call search {"Ada"}  -> text/event-stream: one progress notification, then
                                ../schemas/search-result.fixture.json
  tools/call search {other}  -> {"content":[...], "isError": true}  (-> mcp.tool_failed)

Every request after `initialize` must carry the session id, like a real server.

  python3 fixtures/crm_mcp.py [PORT] [--drift]

PORT defaults to 18860. `--drift` serves a tools/list whose `search` inputSchema
differs from the fixture, so a bundle approved against the fixture fails with
mcp.schema_drift before tools/call is ever sent. Each JSON-RPC method received is
logged to stderr as `MCP <method>`.
"""
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
SCHEMAS = os.path.join(HERE, "..", "schemas")
SESSION = "crm-fixture-session"

args = [a for a in sys.argv[1:] if not a.startswith("--")]
PORT = int(args[0]) if args else 18860
DRIFT = "--drift" in sys.argv


def fixture(name):
    with open(os.path.join(SCHEMAS, name)) as f:
        return json.load(f)["result"]


TOOLS = fixture("tools-list.fixture.json")["tools"]
if DRIFT:
    # A server-side change nobody reviewed: `search` gains a required `limit` argument.
    TOOLS = json.loads(json.dumps(TOOLS))
    TOOLS[0]["inputSchema"]["properties"]["limit"] = {"type": "integer"}
    TOOLS[0]["inputSchema"]["required"].append("limit")
RESULT = fixture("search-result.fixture.json")


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def send(self, code, body=b"", ctype=None, headers=()):
        self.send_response(code)
        if ctype:
            self.send_header("content-type", ctype)
        for k, v in headers:
            self.send_header(k, v)
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_DELETE(self):
        sys.stderr.write("MCP DELETE (session end)\n")
        self.send(204)

    def do_POST(self):
        if self.path != "/mcp":
            return self.send(404)
        msg = json.loads(self.rfile.read(int(self.headers.get("content-length", "0"))))
        method = msg.get("method", "")
        sys.stderr.write("MCP %s\n" % method)
        sys.stderr.flush()
        if method != "initialize" and self.headers.get("mcp-session-id") != SESSION:
            return self.send(400)
        if "id" not in msg:  # notification
            return self.send(202)
        rid = msg["id"]

        def reply(result, headers=()):
            body = json.dumps({"jsonrpc": "2.0", "id": rid, "result": result}).encode()
            self.send(200, body, "application/json", headers)

        if method == "initialize":
            return reply(
                {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {"tools": {"listChanged": False}},
                    "serverInfo": {"name": "crm-fixture", "version": "1.0"},
                },
                [("mcp-session-id", SESSION)],
            )
        if method == "tools/list":
            return reply({"tools": TOOLS})
        if method == "tools/call":
            query = msg.get("params", {}).get("arguments", {}).get("query")
            if query != "Ada":
                return reply({"content": [{"type": "text", "text": "no match"}], "isError": True})
            # Streamable HTTP may answer with an SSE stream; progress is not the result.
            events = [
                {"jsonrpc": "2.0", "method": "notifications/progress",
                 "params": {"progressToken": 1, "progress": 0.5}},
                {"jsonrpc": "2.0", "id": rid, "result": RESULT},
            ]
            body = "".join("event: message\ndata: %s\n\n" % json.dumps(e) for e in events)
            return self.send(200, body.encode(), "text/event-stream")
        if method == "ping":
            return reply({})
        body = json.dumps({"jsonrpc": "2.0", "id": rid,
                           "error": {"code": -32601, "message": "method not found"}}).encode()
        self.send(200, body, "application/json")


if __name__ == "__main__":
    sys.stderr.write("crm-fixture MCP on http://127.0.0.1:%d/mcp%s\n" % (PORT, " (drift)" if DRIFT else ""))
    sys.stderr.flush()
    ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
