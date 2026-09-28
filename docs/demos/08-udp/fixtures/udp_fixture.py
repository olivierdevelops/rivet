#!/usr/bin/env python3
"""Local UDP fixtures for docs/demos/08-udp (Python 3 stdlib only).

Two modes:

  status  PORT             Bind 127.0.0.1:PORT. Answer every {"command":"status"}
                           datagram with {"state":"ready"}; ignore anything else.
                           Runs until killed.

  ping    FROM_PORT TO_PORT
                           Bind 127.0.0.1:FROM_PORT and send {"reading":42} to
                           127.0.0.1:TO_PORT every 100 ms until a reply arrives
                           (or 30 s pass). Print the reply and exit 0; exit 1 on
                           timeout. The repeat covers the moment before Rivet binds.

Options for status mode (negative checks):
  --big N    reply with an N-byte datagram instead (exercises udp.truncated)
  --silent   never reply (exercises the receive timeout)
"""
import json
import socket
import sys


def status(port, big=0, silent=False):
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.bind(("127.0.0.1", port))
    print(f"status fixture on udp://127.0.0.1:{port}", flush=True)
    while True:
        data, peer = s.recvfrom(65535)
        try:
            msg = json.loads(data)
        except ValueError:
            continue
        if msg != {"command": "status"} or silent:
            continue
        reply = b"x" * big if big else json.dumps({"state": "ready"}, separators=(",", ":")).encode()
        s.sendto(reply, peer)


def ping(from_port, to_port):
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.bind(("127.0.0.1", from_port))
    s.settimeout(0.1)
    payload = json.dumps({"reading": 42}, separators=(",", ":")).encode()
    for _ in range(300):
        s.sendto(payload, ("127.0.0.1", to_port))
        try:
            data, peer = s.recvfrom(65535)
        except socket.timeout:
            continue
        print(f"reply from {peer[0]}:{peer[1]}: {data.decode()}", flush=True)
        return 0
    print("no reply", flush=True)
    return 1


def main(argv):
    if len(argv) >= 2 and argv[0] == "status":
        big = int(argv[argv.index("--big") + 1]) if "--big" in argv else 0
        status(int(argv[1]), big=big, silent="--silent" in argv)
        return 0
    if len(argv) == 3 and argv[0] == "ping":
        return ping(int(argv[1]), int(argv[2]))
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
