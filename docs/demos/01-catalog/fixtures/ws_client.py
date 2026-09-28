"""Minimal rivet.v1 WebSocket client for the demos (needs: pip install websockets).

Usage: python3 fixtures/ws_client.py ws://127.0.0.1:8080/v1/ws < requests/ws-frames.jsonl

Sends every JSONL line from stdin as one text frame, then prints each server
frame on its own line until every ref it sent has received a terminal frame
(type "result" or "error"). Optional: --token-file PATH adds a bearer token.
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
