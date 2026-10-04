#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""A stand-in for Google Tasks' REST API (v1), for Sioul's tests only: lists
and tasks in memory, pages of two, updatedMin, deletions kept as deleted.
Usage: google-tasks-stand-in.py PORT. The token asked is "test-token".
/_state and /_poke let a test read and change what "Google" holds."""

import json
import re
import sys
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlparse

PAGE = 2
counter = [0]


def now():
    counter[0] += 1
    # Each change a little later than the one before.
    return time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(time.time() + counter[0])) + ".000Z"


def task(id, title, **fields):
    return {"kind": "tasks#task", "id": id, "etag": f'"{id}-0"', "title": title, "status": "needsAction", "updated": "2026-10-01T08:00:00.000Z", **fields}


lists = {"L1": {"id": "L1", "title": "My Tasks", "updated": "2026-10-01T08:00:00.000Z"}}
tasks = {"L1": [task("t1", "Buy stamps", due="2026-10-05T00:00:00.000Z"),
                task("t2", "Find the envelope", parent="t1"),
                task("t3", "Old one", status="completed", completed="2026-09-30T10:00:00.000Z")]}


def touch(t):
    t["updated"] = now()
    n = int(t["etag"].strip('"').rsplit("-", 1)[1]) + 1
    t["etag"] = f'"{t["id"]}-{n}"'


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def answer(self, status, body=None):
        data = b"" if body is None else json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def body(self):
        size = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(size) or b"{}")

    def handle_any(self, method):
        url = urlparse(self.path)
        query = {k: v[0] for k, v in parse_qs(url.query).items()}
        path = url.path
        if path.startswith("/_state"):
            rest = path[len("/_state"):]
            if rest == "":
                return self.answer(200, list(lists.values()))
            m = re.fullmatch(r"/lists/([^/]+)", rest)
            return self.answer(200, {"tasks": tasks.get(m.group(1), [])})
        if path.startswith("/_poke"):
            rest = path[len("/_poke"):]
            change = self.body()
            m = re.fullmatch(r"/lists/([^/]+)/tasks/([^/]+)", rest)
            if m:
                t = next(t for t in tasks[m.group(1)] if t["id"] == m.group(2))
                t.update(change)
                touch(t)
            m = re.fullmatch(r"/delete-list/([^/]+)", rest)
            if m:
                lists.pop(m.group(1), None)
                tasks.pop(m.group(1), None)
            return self.answer(200, {})
        if self.headers.get("Authorization") != "Bearer test-token":
            return self.answer(401, {"error": {"code": 401, "message": "Invalid Credentials"}})
        if path == "/users/@me/lists" and method == "GET":
            return self.page(list(lists.values()), query)
        if path == "/users/@me/lists" and method == "POST":
            id = f"L{len(lists) + 10}"
            lists[id] = {"id": id, "title": self.body().get("title", ""), "updated": now()}
            tasks[id] = []
            return self.answer(200, lists[id])
        m = re.fullmatch(r"/users/@me/lists/([^/]+)", path)
        if m:
            id = m.group(1)
            if id not in lists:
                return self.answer(404, {"error": {"code": 404}})
            if method == "PATCH":
                lists[id].update(self.body())
                return self.answer(200, lists[id])
            if method == "DELETE":
                lists.pop(id)
                tasks.pop(id)
                return self.answer(204)
        m = re.fullmatch(r"/lists/([^/]+)/tasks", path)
        if m:
            held = tasks.get(m.group(1))
            if held is None:
                return self.answer(404, {"error": {"code": 404}})
            if method == "GET":
                shown = [t for t in held if (query.get("showDeleted") == "true" or not t.get("deleted"))
                         and (query.get("showCompleted") != "false" or t["status"] != "completed")
                         and ("updatedMin" not in query or t["updated"] >= query["updatedMin"])]
                if "maxResults" in query and int(query["maxResults"]) < PAGE:
                    shown = shown[: int(query["maxResults"])]
                return self.page(shown, query)
            if method == "POST":
                fields = self.body()
                counter[0] += 1
                t = task(f"n{counter[0]}", fields.get("title", ""))
                t.update({k: v for k, v in fields.items() if v is not None})
                if "parent" in query:
                    t["parent"] = query["parent"]
                touch(t)
                held.append(t)
                return self.answer(200, t)
        m = re.fullmatch(r"/lists/([^/]+)/tasks/([^/]+)(/move)?", path)
        if m:
            held = tasks.get(m.group(1), [])
            t = next((t for t in held if t["id"] == m.group(2) and not t.get("deleted")), None)
            if t is None:
                return self.answer(404, {"error": {"code": 404}})
            if m.group(3) and method == "POST":
                if "parent" in query:
                    t["parent"] = query["parent"]
                else:
                    t.pop("parent", None)
                touch(t)
                return self.answer(200, t)
            if method == "PATCH":
                for k, v in self.body().items():
                    if v is None:
                        t.pop(k, None)
                    else:
                        t[k] = v
                touch(t)
                return self.answer(200, t)
            if method == "DELETE":
                t["deleted"] = True
                touch(t)
                return self.answer(204)
        return self.answer(404, {"error": {"code": 404, "message": f"{method} {path}"}})

    def page(self, items, query):
        start = int(query.get("pageToken", "0"))
        body = {"items": items[start:start + PAGE]}
        if start + PAGE < len(items):
            body["nextPageToken"] = str(start + PAGE)
        return self.answer(200, body)

    def do_GET(self):
        self.handle_any("GET")

    def do_POST(self):
        self.handle_any("POST")

    def do_PATCH(self):
        self.handle_any("PATCH")

    def do_DELETE(self):
        self.handle_any("DELETE")


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
