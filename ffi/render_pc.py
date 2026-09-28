#!/usr/bin/env python3
"""Render ffi/rivet.pc.in into rivet.pc for this OS (PLAN-2026-0002 TASK-041).

    python3 ffi/render_pc.py --prefix /usr/local > rivet.pc

Libs.private is this OS's `native-static-libs` list (RES-2026-0004 E2; `-lSystem`
is dropped on macOS because the C driver links it already). Override it with
--libs-private "…" after checking `cargo rustc -p rivet-ffi --release
--crate-type staticlib -- --print native-static-libs` on the build host.
"""
import argparse
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)

LIBS_PRIVATE = {
    "darwin": "-framework Security -framework CoreFoundation -liconv -lc -lm",
    "linux": "-lgcc_s -lutil -lrt -lpthread -lm -ldl -lc",
    "win32": "-lkernel32 -ladvapi32 -lntdll -luserenv -lws2_32 -lbcrypt",
}


def workspace_version():
    text = open(os.path.join(ROOT, "Cargo.toml"), encoding="utf-8").read()
    table = text.split("[workspace.package]", 1)[1].split("\n[", 1)[0]
    return re.search(r'^version\s*=\s*"([^"]+)"', table, re.M).group(1)


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--prefix", default="/usr/local")
    p.add_argument("--libs-private", default=None)
    a = p.parse_args()
    platform = "linux" if sys.platform.startswith("linux") else sys.platform
    libs = a.libs_private if a.libs_private is not None else LIBS_PRIVATE.get(platform, "")
    text = open(os.path.join(HERE, "rivet.pc.in"), encoding="utf-8").read()
    body = "\n".join(l for l in text.splitlines() if not l.startswith("#")) + "\n"
    sys.stdout.write(
        body.replace("@PREFIX@", a.prefix)
        .replace("@VERSION@", workspace_version())
        .replace("@LIBS_PRIVATE@", libs)
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
