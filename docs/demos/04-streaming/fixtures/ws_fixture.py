#!/usr/bin/env python3
"""Local WebSocket fixture for docs/demos/04-streaming (needs the `websockets` package).

    python ws_fixture.py PORT [--silent]

Listens on ws://127.0.0.1:PORT/realtime. For every text message that parses as
{"type": "ping"} it answers {"type": "pong", "fixture": "04-streaming"} (the extra
field shows that socket.ping's `open true` output accepts unknown fields).
Any other path is rejected with HTTP 404. Each connection and close is logged to
stdout so you can see Rivet dispose the socket when the `with websocket` scope ends.

  --silent   accept the ping but never answer (exercises --timeout / Ctrl-C cleanup)

Install once:
    python3 -m venv "$TMPDIR/rivet-demo-venv"
    "$TMPDIR/rivet-demo-venv/bin/pip" install websockets
"""
import asyncio
import json
import sys

from websockets.asyncio.server import serve
from websockets.http11 import Response
from websockets.datastructures import Headers


def main():
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        sys.exit(2)
    port = int(sys.argv[1])
    silent = "--silent" in sys.argv[2:]

    def only_realtime(connection, request):
        if request.path != "/realtime":
            return Response(404, "Not Found", Headers(), b"not found\n")
        return None

    async def handler(ws):
        peer = ws.remote_address
        print(f"open  {peer} {ws.request.path}", flush=True)
        try:
            async for raw in ws:
                try:
                    msg = json.loads(raw)
                except (TypeError, ValueError):
                    continue
                print(f"recv  {msg}", flush=True)
                if msg == {"type": "ping"} and not silent:
                    await ws.send(json.dumps({"type": "pong", "fixture": "04-streaming"}))
        finally:
            print(f"close {peer} code={ws.close_code}", flush=True)

    async def run():
        async with serve(handler, "127.0.0.1", port, process_request=only_realtime):
            mode = " (silent)" if silent else ""
            print(f"ws fixture on ws://127.0.0.1:{port}/realtime{mode}", flush=True)
            await asyncio.Future()

    try:
        asyncio.run(run())
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
