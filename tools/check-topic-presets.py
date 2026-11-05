#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""Checks Sioul's topic presets (presets/topics.json, docs/topics.md, "Topic presets").

Offline, it checks that:
  - each topic has an id, names in en, fr, de, es and it, and an area of
    work, admin or leisure;
  - each sender domain is well formed (lowercase letters, digits, dots and
    dashes, a dot at least) and has a source, an https address;
  - no domain sits in two topics, nor twice in one;
  - no domain is already a site preset's (presets/sites*.json: its address
    or the domains its mail comes from): such mail is matched through its
    site's category, never listed again;
  - each site category and type a topic takes exists in the site presets;
  - each List-Id pattern is "*" or a well-formed identifier, with a source.

Exits with 1 when something is wrong, so it can run in CI.

    tools/check-topic-presets.py
    tools/check-topic-presets.py path/to/topics.json path/to/sites.json
"""

import argparse
import json
import pathlib
import re
import sys
import urllib.parse

ROOT = pathlib.Path(__file__).resolve().parent.parent
LANGUAGES = ("en", "fr", "de", "es", "it")
AREAS = ("work", "admin", "leisure")
DOMAIN = re.compile(r"^(?=.{4,253}$)([a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$")
LIST_ID = re.compile(r"^[a-z0-9*][a-z0-9.*-]*$")


def site_domains(sites):
    """Every site preset's domains (its address's host, the domains its mail comes from), its categories and types."""
    every = list(sites.get("international", []))
    for country in sites.get("countries", {}).values():
        every += country.get("sites", [])
        for region in country.get("regions", {}).values():
            every += region.get("sites", [])
    domains = set()
    for site in every:
        host = (urllib.parse.urlparse(site.get("url", "")).hostname or "").lower()
        if host.startswith("www."):
            host = host[4:]
        if host:
            domains.add(host)
        domains.update(d.lower().lstrip("@") for d in site.get("announced_by", []))
    tags = set(sites.get("tags", {}))
    types = {site.get("type", "") for site in every}
    return domains, tags, types


def within(domain, domains):
    """Whether a domain is one of `domains`, or under one of them."""
    parts = domain.split(".")
    return any(".".join(parts[i:]) in domains for i in range(len(parts) - 1))


def check(topics, sites):
    problems = []
    site_doms, tags, types = site_domains(sites)
    seen = {}
    ids = set()
    if not topics.get("checked"):
        problems.append("the file has no 'checked' date")
    for topic in topics.get("topics", []):
        tid = topic.get("id", "")
        where = tid or "a topic without an id"
        if not re.match(r"^[a-z0-9]+(?:-[a-z0-9]+)*$", tid):
            problems.append(f"{where}: its id is not lowercase ASCII words joined by dashes")
        if tid in ids:
            problems.append(f"{where}: two topics have this id")
        ids.add(tid)
        names = topic.get("name", {})
        for language in LANGUAGES:
            if not names.get(language, "").strip():
                problems.append(f"{where}: no name in {language}")
        if topic.get("area") not in AREAS:
            problems.append(f"{where}: its area is not one of {', '.join(AREAS)}")
        rules = topic.get("rules", {})
        for tag in rules.get("site_tags", []):
            if tag not in tags:
                problems.append(f"{where}: the site category {tag!r} is not in the site presets")
        for kind in rules.get("site_types", []):
            if kind not in types:
                problems.append(f"{where}: the site type {kind!r} is not in the site presets")
        for entry in rules.get("domains", []):
            domain = entry.get("domain", "")
            source = entry.get("source", "")
            if not DOMAIN.match(domain):
                problems.append(f"{where}: {domain!r} is not a well-formed domain")
            if not source.startswith("https://") or not urllib.parse.urlparse(source).hostname:
                problems.append(f"{where}: {domain} has no https source")
            if domain in seen:
                problems.append(f"{where}: {domain} is also in {seen[domain]}")
            seen[domain] = tid
            if within(domain, site_doms):
                problems.append(f"{where}: {domain} is a site preset's already: match it through its site's category")
        for entry in rules.get("lists", []):
            pattern = entry.get("pattern", "")
            if pattern != "*" and not LIST_ID.match(pattern):
                problems.append(f"{where}: the List-Id pattern {pattern!r} is not well formed")
            if not entry.get("source", "").startswith("https://"):
                problems.append(f"{where}: the List-Id pattern {pattern!r} has no https source")
        if not any(rules.get(k) for k in ("site_tags", "site_types", "domains", "lists")):
            problems.append(f"{where}: it has no rule")
    # Domains under one another, in two topics: the longer would never be reached apart.
    for domain, tid in seen.items():
        parts = domain.split(".")
        for i in range(1, len(parts) - 1):
            parent = ".".join(parts[i:])
            if parent in seen and seen[parent] != tid:
                problems.append(f"{tid}: {domain} is under {parent}, which is {seen[parent]}'s")
    return problems


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("topics", nargs="?", default=ROOT / "presets" / "topics.json")
    parser.add_argument("sites", nargs="*", default=None)
    args = parser.parse_args()
    topics = json.loads(pathlib.Path(args.topics).read_text(encoding="utf-8"))
    files = [pathlib.Path(s) for s in args.sites] if args.sites else sorted((ROOT / "presets").glob("sites*.json"))
    sites = {"international": [], "countries": {}, "tags": {}}
    for path in files:
        part = json.loads(path.read_text(encoding="utf-8"))
        sites["international"] += part.get("international", [])
        sites["countries"].update(part.get("countries", {}))
        sites["tags"].update(part.get("tags", {}))
    problems = check(topics, sites)
    for problem in problems:
        print(problem)
    count = sum(len(t.get("rules", {}).get("domains", [])) for t in topics.get("topics", []))
    print(f"{len(topics.get('topics', []))} topics, {count} domains: {'fine' if not problems else str(len(problems)) + ' problems'}.")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
