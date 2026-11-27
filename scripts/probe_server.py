#!/usr/bin/env python3
"""Serve dist-web and receive the live game's frame probe JSON.

The standalone web build is the live page used for the screenshot assertions.
It POSTs /probe?name=<state> once it freezes; the same-origin handler writes
docs/probe-<state>.json next to the screenshots.
"""

import http.server
import pathlib
import re
import sys
import urllib.parse

ROOT = pathlib.Path(__file__).resolve().parent.parent
DIST = ROOT / "dist-web"
DOCS = ROOT / "docs"


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(DIST), **kwargs)

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        self.send_header("Access-Control-Allow-Origin", "*")
        super().end_headers()

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path != "/probe":
            self.send_error(404)
            return
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length)
        query = urllib.parse.parse_qs(parsed.query)
        name = re.sub(r"[^A-Za-z0-9_-]", "", query.get("name", ["probe"])[0]) or "probe"
        DOCS.mkdir(exist_ok=True)
        (DOCS / f"probe-{name}.json").write_bytes(body)
        print(f"probe {name}: {len(body)} bytes", flush=True)
        self.send_response(204)
        self.end_headers()

    def log_message(self, fmt, *args):
        # Keep the server log useful for the harness (only request lines).
        print(f"{self.address_string()} {fmt % args}", flush=True)


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8090
    print(f"serving {DIST} on http://127.0.0.1:{port}", flush=True)
    http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()