#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""How much memory Sioul's window holds, and in what, on a demo profile.

    tools/measure-memory.py [--release] OUT [--sites N] [--gpu]
    tools/measure-memory.py --pid PID

The window starts as tools/demo/run.sh starts it (bubblewrap, a demo profile
made anew in OUT/profile, offscreen, software rendering, SIOUL_DEMO=1) with
the step list "memory" (main.qml): a minute at rest on the Porch, every page
and form as "pages" opens them (each page made when first shown, as in use,
no picture taken), the same once the scripts' garbage is collected, then the
profile's sites shown one by one. The log says "sioul-memory: <moment>" at
each; 25 seconds later /proc is read: the window's process (PSS, USS, RSS,
and by kind of mapping: its heap, the other anonymous memory, the QML
engine's JavaScript heap, Sioul's own program, each library) and every
process under it (Qt WebEngine's zygotes and renderers).

--sites N: the demo's sites replaced by N local pages standing in for chats
(a long list, a canvas drawn each second), kept open, served here on
127.0.0.1; the sandbox then shares the network, and Sioul still fetches
nothing by itself. Without it, the demo's sites, which the sandbox's own
network cannot reach.
--gpu: the real /dev. By default the sandbox has a /dev of its own, without
the graphics card, and Chromium draws in software: no measure loads the GPU.
--pid PID: the same reading of a Sioul already running, yours, from /proc
alone (nothing attached to it, nothing sent to it): its process by kind of
mapping, and the processes under it.

Sioul's own program counts too, as the pages of it read from the disk: from
10 to 40 MB, as the system's file cache holds them (it is read whole first,
as after a day's use); the kernel may drop them at any time. Writes OUT/memory.json and OUT/smaps-<moment>.txt (the whole
mapping list) and prints a table. Linux only; build first.
"""

import argparse
import http.server
import json
import os
import pathlib
import re
import signal
import subprocess
import sys
import tempfile
import threading
import time

REPO = pathlib.Path(__file__).resolve().parent.parent
HOME = "/home/demo"
FIELDS = ("Rss", "Pss", "Private_Clean", "Private_Dirty", "Anonymous")

PAGE = """<!doctype html><html><head><meta charset="utf-8"><title>Site %(n)s</title></head><body>
<h1>Test site %(n)s</h1><canvas id="c" width="512" height="512"></canvas><ul id="l"></ul>
<script>
"use strict";
const kept = [];
for (let i = 0; i < 40000; i++) kept.push({ id: i, text: "message " + i + " " + "x".repeat(40), at: Date.now() });
const list = document.getElementById("l");
for (let i = 0; i < 2000; i++) { const li = document.createElement("li"); li.textContent = kept[i].text; list.appendChild(li); }
const g = document.getElementById("c").getContext("2d");
let t = 0;
setInterval(() => { t++; g.fillStyle = "hsl(" + (t * 7 %% 360) + ",50%%,50%%)"; g.fillRect((t * 13) %% 480, (t * 29) %% 480, 32, 32);
  localStorage.setItem("tick", String(t)); }, 1000);
</script></body></html>"""


class Site(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        n = re.search(r"n=(\d+)", self.path)
        body = (PAGE % {"n": n.group(1) if n else "0"}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def kind_of(path):
    """What a mapping is, by its name in /proc/<pid>/smaps."""
    if path == "[heap]":
        return "heap"
    if path == "":
        return "anonymous"
    if path.startswith("[stack"):
        return "stacks"
    if "JSGCHeap" in path:
        return "JavaScript heap"
    if path.startswith("[") or path.startswith("/dev/dri") or "memfd:" in path or path.startswith("/dev/shm"):
        return "other " + re.sub(r"[0-9]+|\(deleted\)", "", path).strip()[:40]
    name = os.path.basename(path.replace(" (deleted)", ""))
    if name.startswith("sioul-app"):
        return "Sioul's program"
    if re.search(r"\.(ttf|otf|ttc)$", name, re.I):
        return "fonts"
    return name.split(".so")[0] if ".so" in name else "file " + name[:40]


def mappings(pid):
    """Each kind's PSS, USS and RSS (kB)."""
    kinds, kind = {}, None
    with open(f"/proc/{pid}/smaps") as f:
        for line in f:
            if re.match(r"^[0-9a-f]+-[0-9a-f]+ ", line):
                parts = line.split(None, 5)
                kind = kinds.setdefault(kind_of(parts[5].strip() if len(parts) > 5 else ""), dict.fromkeys(("pss", "uss", "rss"), 0))
                continue
            key, _, value = line.partition(":")
            if kind is not None and key in ("Pss", "Rss", "Private_Clean", "Private_Dirty"):
                kind[{"Pss": "pss", "Rss": "rss"}.get(key, "uss")] += int(value.split()[0])
    return kinds


def rollup(pid):
    out = {}
    try:
        with open(f"/proc/{pid}/smaps_rollup") as f:
            for line in f:
                key, _, value = line.partition(":")
                if key in FIELDS:
                    out[key] = int(value.split()[0])
    except OSError:
        return None
    out["Uss"] = out.get("Private_Clean", 0) + out.get("Private_Dirty", 0)
    return out


def descendants(root):
    parents = {}
    for entry in os.listdir("/proc"):
        if entry.isdigit():
            try:
                with open(f"/proc/{entry}/stat") as f:
                    parents[int(entry)] = int(f.read().rsplit(")", 1)[1].split()[1])
            except (OSError, IndexError, ValueError):
                pass
    found, frontier = [], [root]
    while frontier:
        pid = frontier.pop()
        children = [p for p, parent in parents.items() if parent == pid]
        found += children
        frontier += children
    return found


def program(pid):
    try:
        return os.path.realpath(f"/proc/{pid}/exe", strict=True)
    except OSError:
        return ""


def process_kind(pid):
    try:
        with open(f"/proc/{pid}/cmdline", "rb") as f:
            line = f.read().replace(b"\0", b" ").decode("utf-8", "replace")
        found = [part.split("=", 1)[1] for part in line.split() if part.startswith(("--type=", "--utility-sub-type="))]
        return " ".join([os.path.basename(line.split(" ", 1)[0])] + [f.split(".")[-1] for f in found])
    except OSError:
        return "?"


def sample(pid):
    children = []
    for child in descendants(pid):
        figures = rollup(child)
        if figures:
            children.append(dict(pid=child, kind=process_kind(child), **figures))
    return dict(window=rollup(pid), kinds=mappings(pid), children=children, threads=len(os.listdir(f"/proc/{pid}/task")))


def keep_sites(config, port, count):
    """The demo's sites taken out; `count` local pages put in, kept open as chats (none for `port` None)."""
    kept, skipping = [], False
    for line in config.read_text().splitlines():
        if line.strip().startswith("["):
            skipping = line.strip() == "[[site]]"
        if not skipping:
            kept.append(line)
    for n in range(count if port else 0):
        kept += ["", "[[site]]", f'id = "test-{n}"', f'name = "Test {n}"', f'url = "http://127.0.0.1:{port}/site?n={n}"',
                 'site = "chat"', 'area = "work+admin+leisure"', "background = true", "announced_by = []", ""]
    config.write_text("\n".join(kept) + "\n")


def show(samples):
    mb = lambda kb: f"{kb / 1024:7.1f}"
    print(f"{'moment':10} {'PSS':>7} {'USS':>7} {'RSS':>7}  (MB, the window's process)   under it: PSS, processes")
    for moment, s in samples.items():
        w = s["window"]
        print(f"{moment:10} {mb(w['Pss'])} {mb(w['Uss'])} {mb(w['Rss'])}   {mb(sum(c['Pss'] for c in s['children']))}, {len(s['children'])}")
    for moment, s in samples.items():
        large = sorted(s["kinds"].items(), key=lambda kv: -kv[1]["pss"])[:10]
        print(f"{moment}: " + ", ".join(f"{k} {v['pss'] / 1024:.1f}" for k, v in large))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--release", action="store_true")
    parser.add_argument("out", nargs="?")
    parser.add_argument("--sites", type=int, default=-1, help="N local test sites in place of the demo's (0: no site)")
    parser.add_argument("--gpu", action="store_true", help="the real /dev, the graphics card reachable")
    parser.add_argument("--pid", type=int, help="a Sioul running already, read from /proc alone")
    args = parser.parse_args()
    if args.pid:
        show({"now": sample(args.pid)})
        return
    if not args.out:
        parser.error("OUT is needed, or --pid")
    app = REPO / "target" / ("release" if args.release else "debug") / "sioul-app"
    if not app.exists():
        sys.exit(f"measure-memory.py: no {app}: build it first.")
    out = pathlib.Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    profile, shots = out / "profile", out / "shots"
    subprocess.run(["rm", "-rf", str(profile), str(shots)], check=True)
    shots.mkdir()
    now, zone = subprocess.run([sys.executable, str(REPO / "tools/demo/moment.py")], capture_output=True, text=True, check=True).stdout.split()
    if now == "none":
        sys.exit("measure-memory.py: no weekday is close enough now (moment.py): try again later.")
    subprocess.run([sys.executable, str(REPO / "tools/demo/make-demo.py"), "--into", str(profile), "--now", now, "--language", "en",
                    "--notes-at", f"{HOME}/Notes"], check=True, stdout=subprocess.DEVNULL)
    port = None
    if args.sites > 0:
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Site)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        port = server.server_address[1]
    if args.sites >= 0:
        keep_sites(profile / "config/sioul/config.toml", port, args.sites)
    runtime = tempfile.mkdtemp(prefix="sioul-run.", dir="/tmp")
    env = {"HOME": HOME, "XDG_CONFIG_HOME": f"{HOME}/.config", "XDG_DATA_HOME": f"{HOME}/.local/share",
           "XDG_STATE_HOME": f"{HOME}/.local/state", "XDG_CACHE_HOME": f"{HOME}/.cache", "XDG_RUNTIME_DIR": runtime,
           "SIOUL_DEMO": "1", "SIOUL_GRAB": str(shots), "SIOUL_GRAB_STEPS": "memory", "SIOUL_TEST_PASSWORD": "x", "SIOUL_THEME": "light",
           "FONTCONFIG_FILE": str(REPO / "tools/demo/fonts.conf"), "QT_FORCE_STDERR_LOGGING": "1", "QT_QPA_PLATFORM": "offscreen",
           "QT_QUICK_BACKEND": "software", "LANG": "en_US.UTF-8", "LC_ALL": "en_US.UTF-8", "TZ": zone}
    hide = ["--tmpfs", os.environ["XDG_RUNTIME_DIR"]] if os.environ.get("XDG_RUNTIME_DIR") else []
    command = (["bwrap", "--die-with-parent"] + ([] if port else ["--unshare-net"]) + ["--dev-bind", "/", "/"]
               + ([] if args.gpu else ["--dev", "/dev"]) + ["--tmpfs", "/home"] + hide
               + ["--ro-bind", str(REPO), str(REPO)]
               + [x for inside, outside in (("config", ".config"), ("data", ".local/share"), ("state", ".local/state"), ("cache", ".cache"), ("notes", "Notes"))
                  for x in ("--bind", str(profile / inside), f"{HOME}/{outside}")]
               + ["--bind", str(out), str(out), "--bind", runtime, runtime, "--chdir", HOME, "env"]
               + [f"{k}={v}" for k, v in env.items()] + ["timeout", "--foreground", "600", "dbus-run-session", "--", str(app)])
    # timeout --foreground keeps the window in this run's process group, which goes whole at the end.
    base = {k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "SESSION_MANAGER", "DBUS_SESSION_BUS_ADDRESS")}
    # The program read whole first, as a day's use leaves it in the file cache:
    # the pages of it the window maps depend on what the cache holds.
    with open(app, "rb") as whole:
        while whole.read(1 << 24):
            pass
    log_path = out / "log"
    log = open(log_path, "w")
    started = time.time()
    launcher = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, env=base, start_new_session=True)
    samples, marks = {}, {}
    try:
        pid = None
        while pid is None and time.time() - started < 30:
            pid = next((p for p in descendants(launcher.pid) if program(p) == str(app.resolve())), None)
            time.sleep(0.1)
        if pid is None:
            sys.exit(f"measure-memory.py: the window did not start (see {log_path}).")
        due = {}
        while launcher.poll() is None:
            for moment in re.findall(r"sioul-memory: (\S+)", log_path.read_text(errors="replace")):
                if moment not in marks:
                    marks[moment] = time.time() - started
                    due[moment] = time.time() + 25
            for moment, when in list(due.items()):
                if time.time() >= when:
                    with open(f"/proc/{pid}/smaps") as f:
                        (out / f"smaps-{moment}.txt").write_text(f.read())
                    samples[moment] = sample(pid)
                    del due[moment]
            time.sleep(1)
    finally:
        try:
            os.killpg(launcher.pid, signal.SIGKILL)
        except OSError:
            pass
        log.close()
        subprocess.run(["rm", "-rf", runtime])
    made = re.findall(r"sioul-perf: (\S+) made in (\d+) ms", log_path.read_text(errors="replace"))
    (out / "memory.json").write_text(json.dumps(dict(samples=samples, marks=marks, made=made), indent=1))
    show(samples)
    print("pages made (ms): " + ", ".join(f"{page} {ms}" for page, ms in made))
    if "sites" not in samples:
        sys.exit("measure-memory.py: the steps did not end (see the log).")


if __name__ == "__main__":
    main()
