#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""A stand-in web server for the QML tests that need one.

    tools/qml-stand-in.py PORT

Listens on 127.0.0.1:PORT, answers every request with 404 and notes its path;
GET /seen lists the paths noted so far, one a line, in order. A test shows
words holding <img src="http://127.0.0.1:PORT/…"> and asks /seen whether Qt
fetched them (tst_plaintext.qml). tools/qml-test.sh starts it for a test file
that names it, and stops it after.
"""
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Lock

seen = []
noting = Lock()


class Handler(BaseHTTPRequestHandler):
    def answer(self):
        if self.path == "/seen":
            with noting:
                body = "\n".join(seen).encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        with noting:
            seen.append(self.path)
        self.send_response(404)
        self.send_header("Content-Length", "0")
        self.end_headers()

    do_GET = do_HEAD = do_POST = answer

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
