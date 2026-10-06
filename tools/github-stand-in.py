#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""A stand-in for GitHub's REST API, for Sioul's tests only: searches of
"assignee:@me" and "user-review-requested:@me", and issues by number, with
ETags (an unchanged answer is a 304). Like GitHub with a fine-grained token,
a search that does not say "is:issue" or "is:pr" is refused (422). Usage:
github-stand-in.py PORT. The token asked is "test-token"."""

import hashlib
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlparse

REPO = "https://api.github.com/repos/example/tool"


def issue(number, title, state="open", reason=None, pull=False, updated="2026-10-01T08:00:00Z", closed=None):
    item = {"repository_url": REPO, "number": number, "title": title, "html_url": f"https://github.com/example/tool/{'pull' if pull else 'issues'}/{number}",
            "state": state, "state_reason": reason, "closed_at": closed, "updated_at": updated}
    if pull:
        item["pull_request"] = {"url": "x"}
    return item


issues = {
    12: issue(12, "Crash when exporting"),
    7: issue(7, "Faster thumbnails", pull=True),
    3: issue(3, "Old bug", state="closed", reason="completed", closed="2026-10-02T09:00:00Z"),
    4: issue(4, "Not mine anymore"),
}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def answer(self, status, body):
        data = json.dumps(body).encode()
        etag = '"' + hashlib.sha1(data).hexdigest() + '"'
        if status == 200 and self.headers.get("If-None-Match") == etag:
            self.send_response(304)
            self.send_header("ETag", etag)
            self.end_headers()
            return
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("ETag", etag)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.headers.get("Authorization") != "Bearer test-token":
            return self.answer(401, {"message": "Bad credentials"})
        url = urlparse(self.path)
        query = parse_qs(url.query)
        if url.path == "/search/issues":
            q = query.get("q", [""])[0]
            words = q.split()
            wants_issues = "is:issue" in words
            wants_pulls = "is:pr" in words or "is:pull-request" in words
            if wants_issues == wants_pulls:
                return self.answer(422, {"message": "Validation Failed", "errors": [{"message": "Query must include 'is:issue' or 'is:pull-request'", "resource": "Search", "field": "q", "code": "invalid"}]})
            found = []
            if "assignee:@me" in q:
                found = [issues[12]]
            elif "user-review-requested:@me" in q:
                found = [issues[7]]
            found = [item for item in found if ("pull_request" in item) == wants_pulls]
            return self.answer(200, {"total_count": len(found), "incomplete_results": False, "items": found})
        parts = url.path.strip("/").split("/")
        if len(parts) == 5 and parts[:3] == ["repos", "example", "tool"] and parts[3] == "issues":
            number = int(parts[4])
            if number in issues:
                return self.answer(200, issues[number])
        return self.answer(404, {"message": "Not Found"})


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
