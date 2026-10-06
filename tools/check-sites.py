#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""Checks of the sites kept in Sioul (docs/sites.md), on a local page standing
in for a site: served here on 127.0.0.1, no real site, no account. Sioul runs
on a demo profile of its own (tools/demo/make-demo.py, in a temporary folder),
off the screen (the offscreen platform), off the network for everything but
the page (SIOUL_DEMO=1), its home hidden by bubblewrap, the page as a chat
kept open and in view; grab steps in main.qml drive it.

    tools/check-sites.py quit [--ways window,term,during] [--after 3]
    tools/check-sites.py share

quit: whether a site finds what it wrote when Sioul starts again (docs/sites.md,
"Closing"). Some sites keep their login only in the open page and write it
back as the page goes: Discord takes its token out of its storage once the page
has loaded, and writes it back as the page unloads. A browser runs a page's
beforeunload, pagehide and unload when it closes the tab or quits; Sioul must
too, or the next start finds no login. What a page wrote must reach the disk
as well, which happens when Qt WebEngine shuts down. At each load the page says
what it finds, then:
  - takes three tokens out of localStorage and writes each back in one
    handler: "beforeunload", "pagehide", "unload" (Discord's way);
  - writes "once" the first time, never again;
  - writes "tick", in localStorage and in a cookie, four times a second: the
    last one found says how much of the end of a session was lost;
  - gets a cookie from the server the first time ("server_cookie": whether
    the next request brings it back).
Sioul runs twice; the first run ends one of these ways:
  window   its window closed (grab steps "site-quit", some 15 s after the
           start): the window's close button, Alt+F4, the taskbar's Close,
           Plasma's logout on Wayland all reach Qt so; an X11 logout (the
           session manager's "Die") is Qt's quit, which closes the windows
  term     SIGTERM to Sioul alone, AFTER seconds after the page loaded: `kill`,
           systemd stopping a unit whose KillMode is "mixed"; Sioul takes it
           as Qt's quit
  during   pages closed while Sioul runs (grab steps "site-during"): the page
           opens a pop-up with tokens of its own, closed as you close a
           window; then the site is taken out of Sioul; then the window is
           closed. The site is put back before the second run.
  termall  SIGTERM to Sioul and its web processes at once, as systemd stops a
           unit whose KillMode is "control-group" (Plasma's app-*.service):
           the pages may die with their processes, before they could go
  kill     SIGKILL: a crash, the power cut
The second run reads what the page finds. "window", "term" and "during" pass
when the tokens (and the pop-up's), "once" and both cookies are found, "tick"
within 1.5 s of the close, and Sioul quits within 4 s; "termall" and "kill"
only show what is kept.

share: whether a site's call can share the screen (docs/sites.md, "The
browser"). Inside bubblewrap with a network of its own, Weston runs headless
with Xwayland: a screen nobody sees, which Chromium's capturer can list. The
page asks for the screen (getDisplayMedia) three times; each time Sioul's
chooser opens and the grab steps ("site-share") choose its first screen, as
you would: with Qt WebEngine's screen capture off (as Sioul had it), then on
(as it has it), then with the site's switch for sharing the screen off. It
passes when the second gives the page a video track, and the first and the
third give none, the third with the line saying the switch is off.

    tools/check-sites.py quit --ways window,term,during,termall,kill --after 1,35
    tools/check-sites.py share --app target/release/sioul-app --keep DIR

Build first (CARGO_INCREMENTAL=0 cargo build -p sioul-app). Exits with 1 when
a check fails. --keep DIR keeps the profiles and logs there.
"""

import argparse
import http.server
import json
import os
import pathlib
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time

REPO = pathlib.Path(__file__).resolve().parent.parent
HOME = "/home/demo"
SITE_ID = "test-chat"
TOKENS = ("beforeunload", "pagehide", "unload")
# The ways whose losses are only shown.
SHOWN = ("termall", "kill")

PAGE = """<!doctype html>
<html><head><meta charset="utf-8"><title>Test site</title></head>
<body>
<h1>Test site</h1>
<pre id="found"></pre>
<script>
"use strict";
const say = (event, data) => {
    const body = JSON.stringify(Object.assign({ event: event, at: Date.now() }, data || {}));
    try { navigator.sendBeacon("/report", body); } catch (e) {}
};
const cookie = name => (document.cookie.split("; ").find(c => c.startsWith(name + "=")) || "").slice(name.length + 1);
// The pop-up ("/popup") keeps tokens of its own; the page opens it when asked (?popup=1).
const popup = location.pathname === "/popup";
const own = popup ? "popup-" : "";
const found = {};
for (const key of ["beforeunload", "pagehide", "unload", "once", "tick", "popup-beforeunload", "popup-pagehide", "popup-unload"])
    found[key] = localStorage.getItem(key);
found.cookie_tick = cookie("tick");
// Discord's way: the token out of storage while the page runs, back as it goes.
const tokens = {};
for (const event of ["beforeunload", "pagehide", "unload"]) {
    const key = own + event;
    tokens[key] = found[key] || key + "-" + Date.now();
    localStorage.removeItem(key);
    addEventListener(event, () => {
        localStorage.setItem(key, tokens[key]);
        say(key);
    });
}
let last = 0;
if (!popup) {
    if (!found.once)
        localStorage.setItem("once", "once-" + Date.now());
    const tick = () => {
        last = Date.now();
        localStorage.setItem("tick", String(last));
        document.cookie = "tick=" + last + "; max-age=31536000; path=/; samesite=lax";
    };
    tick();
    setInterval(tick, 250);
    // Alive, and the last tick written: a page stopped (hidden, throttled) is told from a write lost.
    setInterval(() => say("alive", { tick: last }), 1000);
    if (new URLSearchParams(location.search).get("popup") === "1")
        window.open("/popup", "_blank", "width=400,height=300");
}
// The screen asked for, as a call's "share the screen" does (the grab steps call it).
window.share = label => {
    say("asked", { label: label });
    navigator.mediaDevices.getDisplayMedia({ video: true }).then(stream => {
        const track = stream.getVideoTracks()[0];
        const settings = track ? track.getSettings() : {};
        say("share", { label: label, ok: !!track, width: settings.width || 0, height: settings.height || 0 });
        stream.getTracks().forEach(t => t.stop());
    }, error => say("share", { label: label, ok: false, error: error.name + ": " + error.message }));
};
say(own + "load", { found: found });
document.getElementById("found").textContent = JSON.stringify(found, null, 1);
</script>
</body></html>
"""


class Server(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self):
        super().__init__(("127.0.0.1", 0), Handler)
        self.run = None
        self.reports = []
        self.lock = threading.Lock()

    def said(self, run, event):
        with self.lock:
            return [r for r in self.reports if r.get("run") == run and r.get("event") == event]


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        if not self.path.startswith(("/site", "/popup")):
            self.send_error(404)
            return
        brought = "server=" in self.headers.get("Cookie", "")
        with self.server.lock:
            self.server.reports.append({"run": self.server.run, "event": "request", "server_cookie": brought, "heard": time.time()})
        body = PAGE.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        if not brought:
            self.send_header("Set-Cookie", f"server=s{int(time.time() * 1000)}; Max-Age=31536000; Path=/; SameSite=Lax")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0") or 0)
        try:
            report = json.loads(self.rfile.read(length) or b"{}")
        except ValueError:
            report = {}
        report["run"] = self.server.run
        report["heard"] = time.time()
        with self.server.lock:
            self.server.reports.append(report)
        self.send_response(204)
        self.end_headers()


def make_profile(folder, bwrap):
    """A demo profile, its sites taken out (`pin` puts the local page in)."""
    notes = f"{HOME}/Notes" if bwrap else str(folder / "notes")
    subprocess.run([sys.executable, str(REPO / "tools/demo/make-demo.py"), "--into", str(folder), "--notes-at", notes],
                   check=True, stdout=subprocess.DEVNULL)


def pin(config, port, popup=False):
    """The local page as the profile's only site, kept open as a chat."""
    kept, skipping = [], False
    for line in config.read_text().splitlines():
        if line.strip().startswith("["):
            skipping = line.strip() == "[[site]]"
        if not skipping:
            kept.append(line)
    address = f"http://127.0.0.1:{port}/site" + ("?popup=1" if popup else "")
    kept += ["", "[[site]]", f'id = "{SITE_ID}"', 'name = "Test site"', f'url = "{address}"',
             'site = "chat"', 'area = "work+admin+leisure"', "background = true", "announced_by = []", ""]
    config.write_text("\n".join(kept))


def descendants(root):
    """The processes under `root`, by /proc."""
    parents = {}
    for entry in pathlib.Path("/proc").iterdir():
        if entry.name.isdigit():
            try:
                stat = (entry / "stat").read_text()
                parents[int(entry.name)] = int(stat.rsplit(")", 1)[1].split()[1])
            except (OSError, IndexError, ValueError):
                pass
    found, frontier = [], [root]
    while frontier:
        pid = frontier.pop()
        children = [p for p, parent in parents.items() if parent == pid]
        found += children
        frontier += children
    return found


def app_pid(launcher, app):
    for pid in descendants(launcher.pid):
        try:
            if os.readlink(f"/proc/{pid}/exe") == str(app):
                return pid
        except OSError:
            pass
    return None


def sioul_env(grab, folder):
    """What Sioul runs with: off the screen and the network, timed; `grab`: its grab steps, or none."""
    env = {"SIOUL_DEMO": "1", "SIOUL_THEME": "light", "SIOUL_TIMING": "1", "QT_FORCE_STDERR_LOGGING": "1",
           "QT_QPA_PLATFORM": "offscreen", "QT_QUICK_BACKEND": "software", "LANG": "en_US.UTF-8", "LC_ALL": "en_US.UTF-8"}
    if grab:
        (folder / "shots").mkdir(exist_ok=True)
        env.update({"SIOUL_GRAB": str(folder / "shots"), "SIOUL_GRAB_STEPS": grab})
    return env


def sandbox(profile, own_network=False):
    """Bubblewrap: the profile as the home of a user "demo", the real home out of reach;
    with `own_network`, a network and a /tmp of its own (Weston's and Xwayland's sockets)."""
    command = ["bwrap", "--dev-bind", "/", "/", "--tmpfs", "/home"]
    if own_network:
        command += ["--unshare-net", "--tmpfs", "/tmp"]
    command += ["--ro-bind", str(REPO), str(REPO),
                "--bind", str(profile / "config"), f"{HOME}/.config", "--bind", str(profile / "data"), f"{HOME}/.local/share",
                "--bind", str(profile / "state"), f"{HOME}/.local/state", "--bind", str(profile / "cache"), f"{HOME}/.cache",
                "--bind", str(profile / "notes"), f"{HOME}/Notes", "--bind", str(profile), str(profile), "--chdir", HOME]
    homes = {"HOME": HOME, "XDG_CONFIG_HOME": f"{HOME}/.config", "XDG_DATA_HOME": f"{HOME}/.local/share",
             "XDG_STATE_HOME": f"{HOME}/.local/state", "XDG_CACHE_HOME": f"{HOME}/.cache"}
    return command, homes


def start(app, profile, log, bwrap, grab):
    """Sioul on `profile`, off the screen, on a bus of its own."""
    env = sioul_env(grab, profile)
    if bwrap:
        command, homes = sandbox(profile)
        env.update(homes)
        command += ["env"] + [f"{k}={v}" for k, v in env.items()] + ["dbus-run-session", "--", str(app)]
    else:
        env.update({"HOME": str(profile), "XDG_CONFIG_HOME": str(profile / "config"), "XDG_DATA_HOME": str(profile / "data"),
                    "XDG_STATE_HOME": str(profile / "state"), "XDG_CACHE_HOME": str(profile / "cache")})
        command = ["env"] + [f"{k}={v}" for k, v in env.items()] + ["dbus-run-session", "--", str(app)]
    base = {k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "SESSION_MANAGER")}
    return subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, env=base, start_new_session=True)


def wait_for(condition, seconds):
    end = time.time() + seconds
    while time.time() < end:
        if condition():
            return True
        time.sleep(0.05)
    return condition()


def stop(launcher):
    """Whatever is left of a run, gone."""
    try:
        os.killpg(launcher.pid, signal.SIGKILL)
    except OSError:
        pass
    try:
        launcher.wait(10)
    except subprocess.TimeoutExpired:
        pass


# quit

def first_run(way, after, app, server, profile, bwrap, result):
    """The page loaded, then Sioul closed `way`; what the run said, into `result`."""
    log = open(profile / "run1.log", "w")
    launcher = start(app, profile, log, bwrap, {"window": "site-quit", "during": "site-during"}.get(way, "site-open"))
    try:
        if not wait_for(lambda: server.said(server.run, "load"), 90):
            result["problem"] = f"run 1: the page never loaded (see {profile}/run1.log)"
            return
        closed_at = None
        if way not in ("window", "during"):
            wait_for(lambda: False, after)
            pid = app_pid(launcher, app)
            if pid is None:
                result["problem"] = "run 1: Sioul's process not found"
                return
            closed_at = time.time()
            if way == "termall":
                for each in [pid] + descendants(pid):
                    try:
                        os.kill(each, signal.SIGTERM)
                    except OSError:
                        pass
            else:
                os.kill(pid, signal.SIGKILL if way == "kill" else signal.SIGTERM)
        try:
            launcher.wait(120)
        except subprocess.TimeoutExpired:
            result["problem"] = "run 1: Sioul did not quit within two minutes"
            return
        ended = time.time()
        unloads = {e: server.said(server.run, e) for e in TOKENS}
        # The window's close, which the grab steps did: the first handler heard, else the end.
        heard = [r["at"] / 1000 for rows in unloads.values() for r in rows]
        result["closed"] = closed_at or (min(heard) if heard else ended)
        result["quit_s"] = ended - result["closed"]
        result["handlers"] = [e for e in TOKENS if unloads[e]]
        if way == "during":
            # Closed while it ran, seconds before the window: not by the quit.
            result["closed"] = ended
            result["quit_s"] = 0
            result["during"] = {key: min((r["at"] / 1000 for r in server.said(server.run, key)), default=None)
                                for key in ("popup-beforeunload", "beforeunload")}
            result["ended"] = ended
        alive = [r for r in server.said(server.run, "alive") if r["at"] / 1000 <= result["closed"]]
        result["last_alive"] = max((r["at"] / 1000 for r in alive), default=None)
    finally:
        stop(launcher)
        log.close()


def second_run(app, server, profile, bwrap, result):
    """What the page finds as Sioul starts again."""
    with open(profile / "run2.log", "w") as log:
        launcher = start(app, profile, log, bwrap, "")
        try:
            if not wait_for(lambda: server.said(server.run, "load"), 90):
                result["problem"] = f"run 2: the page never loaded (see {profile}/run2.log)"
                return
            result["found"] = server.said(server.run, "load")[0].get("found", {})
            result["found"]["server_cookie"] = any(r["server_cookie"] for r in server.said(server.run, "request"))
        finally:
            stop(launcher)


def check_quit(way, after, app, server, port, folder, bwrap):
    """One way of closing: a profile, two runs, what the second found."""
    profile = folder / f"{way}-{after:g}"
    make_profile(profile, bwrap)
    config = profile / "config/sioul/config.toml"
    pin(config, port, popup=(way == "during"))
    result = {"way": way, "after": after}
    server.run = (way, after, 1)
    first_run(way, after, app, server, profile, bwrap, result)
    if "problem" not in result:
        if way == "during":
            pin(config, port)
        server.run = (way, after, 2)
        second_run(app, server, profile, bwrap, result)
    return result


def quit_verdict(result):
    """What was lost, in words; [] when nothing."""
    if "problem" in result:
        return [result["problem"]]
    found, lost = result.get("found", {}), []
    popup = tuple("popup-" + key for key in TOKENS) if result["way"] == "during" else ()
    for key in TOKENS + popup + ("once", "server_cookie"):
        if not found.get(key):
            lost.append(f"{key} lost")
    # "during": each page closed as it went, the pop-up first, the site a few seconds before the window.
    for key, what in (("popup-beforeunload", "the pop-up's"), ("beforeunload", "the site's")):
        if result["way"] == "during":
            at = result["during"].get(key)
            if at is None or result["ended"] - at < 3:
                lost.append(f"{what} page not closed when it went")
    # Measured from the page's last tick before the close: a page throttled is not a loss.
    end = min(result["closed"], (result.get("last_alive") or result["closed"]) + 1.0)
    for key, name in (("tick", "localStorage"), ("cookie_tick", "cookie")):
        if found.get(key):
            gap = end - int(found[key]) / 1000
            if gap > 1.5:
                lost.append(f"the last {gap:.1f} s of the {name} ticks lost")
        else:
            lost.append(f"every {name} tick lost")
    if result.get("quit_s", 0) > 4:
        lost.append(f"quitting took {result['quit_s']:.1f} s")
    return lost


def run_quit(args, app, folder, bwrap):
    server = Server()
    port = server.server_address[1]
    threading.Thread(target=server.serve_forever, daemon=True).start()
    failed = False
    try:
        for way in args.ways.split(","):
            for after in ([0.0] if way in ("window", "during") else [float(a) for a in args.after.split(",")]):
                result = check_quit(way, after, app, server, port, folder, bwrap)
                lost = quit_verdict(result)
                when = "by the grab steps" if way in ("window", "during") else f"{after:g} s after the load"
                handlers = ", ".join(result.get("handlers", [])) or "none"
                quit = f"quit in {result['quit_s']:.2f} s" if "quit_s" in result else ""
                print(f"{way:7} {when:20} handlers heard: {handlers:28} {quit:16} {'kept' if not lost else 'LOST: ' + '; '.join(lost)}")
                print("        found: " + json.dumps(result.get("found", {})), flush=True)
                if lost and way not in SHOWN:
                    failed = True
    finally:
        server.shutdown()
    return failed


# share

def share_inside(profile, app):
    """In the sandbox: the page, a screen nobody sees (Weston, headless, with
    Xwayland), Sioul taking the grab steps "site-share"; what the page said, into
    PROFILE/share.json."""
    server = Server()
    server.run = "share"
    threading.Thread(target=server.serve_forever, daemon=True).start()
    pin(pathlib.Path(f"{HOME}/.config/sioul/config.toml"), server.server_address[1])
    # The sandbox's own /tmp: a socket's path must be short, and X's live there.
    runtime = pathlib.Path("/tmp/sioul-runtime")
    runtime.mkdir(mode=0o700, exist_ok=True)
    pathlib.Path("/tmp/.X11-unix").mkdir(exist_ok=True)
    os.chmod("/tmp/.X11-unix", 0o1777)
    env = dict(os.environ, XDG_RUNTIME_DIR=str(runtime))
    env.pop("DISPLAY", None)
    env.pop("WAYLAND_DISPLAY", None)
    weston_log = open(profile / "weston.log", "w")
    weston = subprocess.Popen(["weston", "--backend=headless", "--renderer=pixman", "--xwayland", "--socket=wayland-sioul",
                               "--width=1280", "--height=800", "--idle-time=0", "--no-config"],
                              stdout=weston_log, stderr=subprocess.STDOUT, env=env)
    said = {"reports": [], "display": None}
    try:
        def display():
            found = re.search(r"xserver listening on display (:\d+)", (profile / "weston.log").read_text())
            return found.group(1) if found else None
        if not wait_for(display, 30):
            said["problem"] = "Weston gave no X display (see weston.log)"
            return
        said["display"] = display()
        env = sioul_env("site-share", profile)
        env.update({"HOME": HOME, "XDG_CONFIG_HOME": f"{HOME}/.config", "XDG_DATA_HOME": f"{HOME}/.local/share",
                    "XDG_STATE_HOME": f"{HOME}/.local/state", "XDG_CACHE_HOME": f"{HOME}/.cache",
                    # Chromium's capturer lists Xwayland's screen; Qt itself stays off the screen.
                    "DISPLAY": said["display"], "XDG_RUNTIME_DIR": str(runtime)})
        with open(profile / "run.log", "w") as log:
            sioul = subprocess.Popen(["dbus-run-session", "--", str(app)], stdout=log, stderr=subprocess.STDOUT,
                                     env={**{k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY")}, **env})
            try:
                sioul.wait(150)
            except subprocess.TimeoutExpired:
                said["problem"] = "Sioul did not end its steps within 150 s"
                sioul.kill()
        said["reports"] = [r for r in server.reports if r.get("event") in ("asked", "share", "load")]
        line = re.search(r"sioul-share: the line above the site says: (.*)", (profile / "run.log").read_text())
        said["line"] = line.group(1).strip() if line else None
    finally:
        weston.terminate()
        try:
            weston.wait(10)
        except subprocess.TimeoutExpired:
            weston.kill()
        weston_log.close()
        server.shutdown()
        (profile / "share.json").write_text(json.dumps(said, indent=1))


def run_share(args, app, folder):
    """The share check, its sandbox made here."""
    if shutil.which("bwrap") is None or shutil.which("weston") is None:
        print("share: needs bubblewrap and Weston (with Xwayland).")
        return True
    profile = folder / "share"
    make_profile(profile, True)
    command, _ = sandbox(profile, own_network=True)
    subprocess.run(command + [sys.executable, str(REPO / "tools/check-sites.py"), "share", "--inside", str(profile), "--app", str(app)],
                   env={k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "SESSION_MANAGER")},
                   timeout=240)
    said = json.loads((profile / "share.json").read_text())
    if said.get("problem"):
        print("share: " + said["problem"])
        return True
    answers = {r["label"]: r for r in said["reports"] if r.get("event") == "share"}
    asked = [r["label"] for r in said["reports"] if r.get("event") == "asked"]
    failed = False
    expected = {"capture-off": False, "capture-on": True, "switch-off": False}
    for label, wanted in expected.items():
        answer = answers.get(label)
        if label not in asked:
            text, good = "never asked (the page did not run its step)", False
        elif answer is None:
            text, good = "no answer: the chooser never came, or the choice never went", False
        elif answer["ok"]:
            text, good = f"a video track, {answer['width']}×{answer['height']}", wanted
        else:
            text, good = f"refused ({answer.get('error', '')})", not wanted
        if label == "switch-off":
            text += f"; the line above the site: {said.get('line') or 'nothing'}"
            good = good and bool(said.get("line"))
        print(f"share {label:12} {text:70} {'as expected' if good else 'WRONG'}")
        failed = failed or not good
    return failed


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("check", choices=("quit", "share"))
    parser.add_argument("--app", type=pathlib.Path, default=REPO / "target/debug/sioul-app")
    parser.add_argument("--ways", default="window,term,during", help="quit: window, term, during, termall, kill, joined by commas")
    parser.add_argument("--after", default="3", help="quit: seconds between the page loading and the signal, several joined by commas")
    parser.add_argument("--keep", type=pathlib.Path, help="keep the profiles and logs in this folder")
    parser.add_argument("--no-bwrap", action="store_true", help="quit: run without bubblewrap (the profile's folder is HOME)")
    parser.add_argument("--inside", type=pathlib.Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    app = args.app.resolve()
    if args.inside:
        share_inside(args.inside, app)
        return
    if not app.exists():
        sys.exit(f"No {app}: build it first (CARGO_INCREMENTAL=0 cargo build -p sioul-app).")
    bwrap = not args.no_bwrap and shutil.which("bwrap") is not None
    folder = args.keep or pathlib.Path(tempfile.mkdtemp(prefix="sioul-sites."))
    folder.mkdir(parents=True, exist_ok=True)
    try:
        failed = run_quit(args, app, folder, bwrap) if args.check == "quit" else run_share(args, app, folder)
    finally:
        if args.keep is None:
            shutil.rmtree(folder, ignore_errors=True)
        else:
            print(f"Profiles and logs: {folder}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
