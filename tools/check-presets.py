#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""Checks every address of Sioul's site presets (presets/sites.json).

Each address is asked as a browser would ask it, following redirects. It says,
for each site:
  ok        the page answers;
  moved     it answers from another address: worth updating the file;
  blocked   it refuses robots (400, 401, 403, 429, 503, a challenge page): check by hand;
  silent    no answer in time, a reset, a certificate this script cannot follow:
            often a site that drops scripts, sometimes one gone: check by hand;
  broken    not found (404, 410), or no such host: fix it.

Exits with 1 when an address is broken, so it can run on a schedule.

    tools/check-presets.py                    # the file of this source tree
    tools/check-presets.py path/to/sites.json
    tools/check-presets.py --only FR          # a country (or "international")
    tools/check-presets.py --json > report.json
"""

import argparse
import concurrent.futures
import http.cookiejar
import json
import pathlib
import socket
import ssl
import sys
import urllib.error
import urllib.parse
import urllib.request

AGENT = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0"
BLOCKED = {400, 401, 403, 405, 406, 429, 451, 503}
# What a browser sends when you type an address: some sites refuse less.
HEADERS = {
    "User-Agent": AGENT,
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    "Accept-Language": "en,fr;q=0.8",
    "Upgrade-Insecure-Requests": "1",
    "Sec-Fetch-Dest": "document",
    "Sec-Fetch-Mode": "navigate",
    "Sec-Fetch-Site": "none",
    "Sec-Fetch-User": "?1",
}
BROKEN = {404, 410}


def sites(presets, only=None):
    """Every site of the file, with where it comes from: "international", "FR", "CA/QC"."""
    if only in (None, "international"):
        for site in presets.get("international", []):
            yield "international", site
    for code, country in sorted(presets.get("countries", {}).items()):
        if only not in (None, code):
            continue
        for site in country.get("sites", []):
            yield code, site
        for region, inside in sorted(country.get("regions", {}).items()):
            for site in inside.get("sites", []):
                yield f"{code}/{region}", site


class Keep(urllib.request.HTTPRedirectHandler):
    """Follows redirects, remembering the last address."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        new = super().redirect_request(req, fp, code, msg, headers, newurl)
        if new is not None:
            new.add_header("User-Agent", AGENT)
        return new


def ask(url, timeout):
    """(verdict, status, final address, detail)."""
    # Cookies kept along the way: many sign-in pages redirect to set one first.
    opener = urllib.request.build_opener(Keep(), urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()), urllib.request.HTTPSHandler(context=ssl.create_default_context()))
    request = urllib.request.Request(url, headers=HEADERS)
    try:
        with opener.open(request, timeout=timeout) as answer:
            final = answer.geturl()
            status = answer.status
    except urllib.error.HTTPError as error:
        final, status = error.geturl() or url, error.code
        if status in BROKEN:
            return "broken", status, final, "not found"
        if status in BLOCKED:
            return "blocked", status, final, "refuses robots"
        return "blocked" if status >= 500 else "broken", status, final, error.reason
    except (urllib.error.URLError, socket.timeout, ssl.SSLError, ConnectionError, TimeoutError) as error:
        reason = getattr(error, "reason", error)
        if isinstance(reason, socket.gaierror):
            return "broken", 0, url, "no such host"
        return "silent", 0, url, str(reason)
    if moved(url, final):
        return "moved", status, final, f"now {final}"
    return "ok", status, final, ""


# What a sign-in detour's address holds: a redirect there is the site asking who you are.
SIGN_IN = ("login", "signin", "sign-in", "connexion", "connect", "auth", "oauth", "openid", "sso", "cas/", "authorize", "session", "identif", "redirect_uri", "service=", "client_id", "state=")


def moved(asked, final):
    """Whether an answer came from elsewhere than a sign-in page's usual detours."""
    a, f = urllib.parse.urlsplit(asked), urllib.parse.urlsplit(final)
    if any(mark in (f.path + "?" + f.query).lower() for mark in SIGN_IN):
        return False
    if a.netloc.lower() != f.netloc.lower():
        # A sign-in service of the same site (login.example.org for www.example.org) is no move.
        def domain(host):
            parts = host.lower().split(":")[0].split(".")
            return ".".join(parts[-2:])
        return domain(a.netloc) != domain(f.netloc)
    return a.path.rstrip("/") != f.path.rstrip("/") and not f.path.startswith(a.path.rstrip("/") + "/")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    here = pathlib.Path(__file__).resolve().parent.parent / "presets" / "sites.json"
    parser.add_argument("file", nargs="?", default=str(here))
    parser.add_argument("--only", help='one country\'s code, or "international"')
    parser.add_argument("--timeout", type=float, default=25)
    parser.add_argument("--json", action="store_true", help="the report as JSON")
    args = parser.parse_args()

    presets = json.loads(pathlib.Path(args.file).read_text(encoding="utf-8"))
    listed = list(sites(presets, args.only))
    report = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        futures = {pool.submit(ask, site["url"], args.timeout): (where, site) for where, site in listed}
        for future in concurrent.futures.as_completed(futures):
            where, site = futures[future]
            verdict, status, final, detail = future.result()
            report.append({"where": where, "name": site["name"], "url": site["url"], "verdict": verdict, "status": status, "final": final, "detail": detail})
    report.sort(key=lambda r: (["broken", "moved", "silent", "blocked", "ok"].index(r["verdict"]), r["where"], r["name"].lower()))

    if args.json:
        json.dump(report, sys.stdout, ensure_ascii=False, indent=1)
        print()
    else:
        for r in report:
            status = r["status"] or "—"
            print(f"{r['verdict']:8} {status:>4}  {r['where']:14} {r['name']:32} {r['url']}" + (f"  ({r['detail']})" if r["detail"] else ""))
        counts = {v: sum(1 for r in report if r["verdict"] == v) for v in ("ok", "moved", "silent", "blocked", "broken")}
        print(f"\n{len(report)} sites: {counts['ok']} ok, {counts['moved']} moved, {counts['blocked']} blocked and {counts['silent']} silent (check by hand), {counts['broken']} broken.")
    return 1 if any(r["verdict"] == "broken" for r in report) else 0


if __name__ == "__main__":
    sys.exit(main())
