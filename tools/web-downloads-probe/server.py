#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""A stand-in site for tools/web-downloads-probe/run.sh, on 127.0.0.1 only:
pages that offer files (one of 30 MB, one sent at a network's pace, two at
once, thirty by script with no click, names that climb out of a folder),
and a file cut in its middle. Usage: server.py PORT LOG."""
import http.server, socketserver, sys, threading, time, socket

HITS = {}
LOG = open(sys.argv[2], "a", buffering=1)

class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def log_message(self, fmt, *args):
        LOG.write("server: %s %s\n" % (self.path, fmt % args))
    def page(self, body):
        data = ("<!doctype html><meta charset=utf-8><body>" + body + "</body>").encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def do_GET(self):
        path = self.path.split("?")[0]
        HITS[path] = HITS.get(path, 0) + 1
        LOG.write("server: GET %s (hit %d) range=%s\n" % (self.path, HITS[path], self.headers.get("Range")))
        if path == "/held.html":
            self.page("<a id=a href=/big>file</a><script>setTimeout(() => document.getElementById('a').click(), 300)</script>")
        elif path == "/two.html":
            self.page("<a id=a href=/big>file</a><a id=b href=/big?second>file</a><script>setTimeout(() => document.getElementById('a').click(), 300); setTimeout(() => document.getElementById('b').click(), 900)</script>")
        elif path == "/trickle.html":
            self.page("<a id=a href=/trickle>file</a><script>setTimeout(() => document.getElementById('a').click(), 300)</script>")
        elif path == "/trickle":
            size = 64 * 1024 * 50
            self.send_response(200)
            self.send_header("Content-Type", "application/pdf")
            self.send_header("Content-Length", str(size))
            self.send_header("Content-Disposition", 'attachment; filename="trickle.pdf"')
            self.end_headers()
            for i in range(50):
                self.wfile.write(b"t" * 65536); self.wfile.flush(); time.sleep(0.06)
            LOG.write("server: /trickle sent whole\n")
        elif path == "/cut.html":
            self.page("<a id=a href=/slow>file</a><script>setTimeout(() => document.getElementById('a').click(), 300)</script>")
        elif path == "/many.html":
            self.page("<script>let n = 0; const t = setInterval(() => { const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob(['x'.repeat(1000)])); a.download = 'auto-' + n + '.txt'; document.body.appendChild(a); a.click(); if (++n >= 30) clearInterval(t) }, 50)</script>")
        elif path == "/names.html":
            self.page("".join("<iframe src='/named/%d'></iframe>" % i for i in range(len(NAMES))))
        elif path.startswith("/named/"):
            i = int(path.rsplit("/", 1)[1])
            data = b"x" * 100
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Content-Disposition", NAMES[i])
            self.end_headers()
            self.wfile.write(data)
        elif path == "/other.html":
            self.page("another page")
        elif path == "/big":
            size = 30 * 1024 * 1024
            self.send_response(200)
            self.send_header("Content-Type", "application/pdf")
            self.send_header("Content-Length", str(size))
            self.send_header("Content-Disposition", 'attachment; filename="big.pdf"')
            self.end_headers()
            try:
                for _ in range(size // 65536):
                    self.wfile.write(b"\0" * 65536)
                LOG.write("server: /big sent whole\n")
            except Exception as e:
                LOG.write("server: /big stopped: %r\n" % (e,))
        elif path == "/slow":
            size = 3 * 1024 * 1024
            rng = self.headers.get("Range")
            start = int(rng.split("=")[1].split("-")[0]) if rng else 0
            self.send_response(206 if rng else 200)
            self.send_header("Content-Type", "application/pdf")
            self.send_header("Content-Length", str(size - start))
            if rng:
                self.send_header("Content-Range", "bytes %d-%d/%d" % (start, size - 1, size))
            self.send_header("Accept-Ranges", "bytes")
            self.send_header("ETag", '"abc123"')
            self.send_header("Last-Modified", "Wed, 21 Oct 2026 07:28:00 GMT")
            self.send_header("Content-Disposition", 'attachment; filename="slow.pdf"')
            self.end_headers()
            if HITS[path] == 1:
                # The first time: a third, a pause, then the connection reset.
                self.wfile.write(b"a" * (1024 * 1024))
                self.wfile.flush()
                time.sleep(1.5)
                try:
                    self.connection.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, b"\x01\x00\x00\x00\x00\x00\x00\x00")
                except Exception as e:
                    LOG.write("server: linger %r\n" % (e,))
                self.connection.close()
                self.close_connection = True
                LOG.write("server: /slow cut after 1 MB\n")
            else:
                self.wfile.write(b"b" * (size - start))
                LOG.write("server: /slow rest sent from %d\n" % start)
        else:
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()

NAMES = [
    'attachment; filename="../../escape.txt"',
    'attachment; filename="..\\\\..\\\\escape2.txt"',
    'attachment; filename=".hidden"',
    "attachment; filename*=UTF-8''invoice%E2%80%AEfdp.exe",
    'attachment; filename="CON.txt"',
    'attachment; filename="a/b/c.txt"',
    'attachment; filename="trailing.txt. . "',
    'attachment',
]

class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = True

server = Server(("127.0.0.1", int(sys.argv[1])), Handler)
LOG.write("server: listening on %d\n" % server.server_address[1])
server.serve_forever()
