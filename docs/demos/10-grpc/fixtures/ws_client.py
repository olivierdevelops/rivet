"""Minimal rivet.v1 WebSocket client for the demos (needs: pip install websockets).

Usage: python fixtures/ws_client.py ws://127.0.0.1:18881/v1/ws < requests/ws-chat.jsonl

Sends every JSONL line from stdin as one text frame, then prints each server
record on its own line until every ref it sent has received its terminal record
(a 0.2 ResponseEnvelope with "type": "result" and "status" ok, error or
cancelled; a 0.1.0 server's "error" frame also ends a ref). Request frames use
the 0.2 input envelope {"type":"request","ref","operation","data"}.
Optional: --token-file PATH adds a bearer token.
"""
import json
import sys

from websockets.sync.client import connect


def main() -> int:
    args = sys.argv[1:]
    headers = {}
    if "--token-file" in args:
        i = args.index("--token-file")
        with open(args[i + 1]) as f:
            headers["Authorization"] = "Bearer " + f.read().strip()
        del args[i : i + 2]
    url = args[0]
    frames = [line.strip() for line in sys.stdin if line.strip()]
    pending = {json.loads(f)["ref"] for f in frames if json.loads(f).get("type") == "request"}
    with connect(url, subprotocols=["rivet.v1"], additional_headers=headers) as ws:
        for f in frames:
            ws.send(f)
        while pending:
            msg = json.loads(ws.recv(timeout=30))
            print(json.dumps(msg, separators=(",", ":")))
            if msg.get("type") in ("result", "error"):
                pending.discard(msg.get("ref"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
