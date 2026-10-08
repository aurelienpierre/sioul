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

devices: whether calls take the camera, microphone and speaker chosen, and
change them during a call (docs/sites.md, "The devices of calls"). Chromium's
own fake devices stand in for real ones (--use-fake-device-for-media-stream:
three cameras, two microphones and two speakers besides the defaults), and
the sandbox has a /dev of its own, with no camera and no sound device, and no
sound server: no real camera or microphone can be opened. Four sites: a call
in the page ("direct", sent through a canvas effect, with a voice meter on
its stream), a call in a frame of another site ("frame"), a call in a pop-up
("popup"), and a mailbox allowed no device ("bank"). Each call is a loopback
between two peers in its page, keeping its own references to its tracks.
A camera and a microphone are chosen before Sioul starts; the grab steps
("site-devices") then change the camera, the speaker and the microphone
during the calls; the direct call turns its camera off and on with its own button,
then a camera that is not there is chosen, then every call hangs up. It
passes when each call starts on the devices chosen, follows each change
(fake camera 1 is grey: the preview and the effect's remote picture turn
grey), every element plays on the speaker chosen, the voice meter goes on
reading, the call's own button blacks the picture and brings it back, the
missing camera is said in the line above the site while the call keeps its
own, the bank hears nothing, after hanging up every track has ended and
Sioul sees no call, and the page reloaded after one more change starts its
new call on the camera chosen last.

    tools/check-sites.py quit --ways window,term,during,termall,kill --after 1,35
    tools/check-sites.py share --app target/release/sioul-app --keep DIR
    tools/check-sites.py devices

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

# devices: a call between two peers in one page, as a site makes it.
CALL_PAGE = """<!doctype html>
<html><head><meta charset="utf-8"><title>Test call</title></head>
<body>
<video id="me" autoplay muted playsinline width="160" height="120"></video>
<video id="them" autoplay playsinline width="160" height="120"></video>
<script>
"use strict";
const query = new URLSearchParams(location.search);
const who = query.get("who") || "direct";
const say = (event, data) => {
    try { navigator.sendBeacon("/report", JSON.stringify(Object.assign({ event: event, who: who, at: Date.now() }, data || {}))); } catch (e) {}
};
let phase = "start", hungUp = false;
window.setPhase = name => { phase = name; };
const canvas = document.createElement("canvas");
canvas.width = 32;
canvas.height = 24;
const drawing = canvas.getContext("2d", { willReadFrequently: true });
// A picture's mean colour, red/green/blue: fake camera 1 is grey, the others green.
const look = element => {
    if (!element.videoWidth)
        return null;
    drawing.drawImage(element, 0, 0, 32, 24);
    const d = drawing.getImageData(0, 0, 32, 24).data;
    let r = 0, g = 0, b = 0;
    for (let i = 0; i < d.length; i += 4) { r += d[i]; g += d[i + 1]; b += d[i + 2]; }
    const n = d.length / 4;
    return [r / n, g / n, b / n].map(Math.round);
};
(async () => {
    try {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true, video: true });
        // The site's own references to its tracks, as call sites keep them.
        const mine = stream.getTracks();
        document.getElementById("me").srcObject = stream;
        let send = stream;
        if (query.get("effect") === "canvas") {
            // A background effect as sites make one: a hidden video, a canvas, its stream.
            const hidden = document.createElement("video");
            hidden.muted = true;
            hidden.srcObject = stream;
            hidden.play();
            const effect = document.createElement("canvas");
            effect.width = 160;
            effect.height = 120;
            const paint = effect.getContext("2d");
            setInterval(() => { if (hidden.videoWidth) paint.drawImage(hidden, 0, 0, 160, 120); }, 33);
            send = new MediaStream([effect.captureStream(30).getVideoTracks()[0]].concat(stream.getAudioTracks()));
        }
        // A voice meter on the site's own stream, as call sites show one.
        const context = new AudioContext();
        const analyser = context.createAnalyser();
        context.createMediaStreamSource(stream).connect(analyser);
        context.resume();
        const samples = new Float32Array(analyser.fftSize);
        let peak = 0;
        setInterval(() => {
            analyser.getFloatTimeDomainData(samples);
            peak = Math.max(peak, Math.round(1000 * Math.sqrt(samples.reduce((a, x) => a + x * x, 0) / samples.length)));
        }, 20);
        const pc1 = new RTCPeerConnection(), pc2 = new RTCPeerConnection();
        pc1.onicecandidate = e => e.candidate && pc2.addIceCandidate(e.candidate);
        pc2.onicecandidate = e => e.candidate && pc1.addIceCandidate(e.candidate);
        const remote = new MediaStream();
        pc2.ontrack = e => { remote.addTrack(e.track); document.getElementById("them").srcObject = remote; };
        for (const track of send.getTracks())
            pc1.addTrack(track, send);
        const offer = await pc1.createOffer();
        await pc1.setLocalDescription(offer);
        await pc2.setRemoteDescription(offer);
        const answer = await pc2.createAnswer();
        await pc2.setLocalDescription(answer);
        await pc1.setRemoteDescription(answer);
        // The site's own camera button, and hanging up with its own references.
        window.mute = off => mine.forEach(t => { if (t.kind === "video") t.enabled = !off; });
        window.hangup = () => { mine.forEach(t => t.stop()); pc1.close(); pc2.close(); hungUp = true; };
        say("call", {});
        setInterval(async () => {
            const labels = {};
            mine.forEach(t => { labels[t.kind] = t.label; });
            let frames = 0, received = 0;
            if (!hungUp) {
                (await pc2.getStats()).forEach(r => {
                    if (r.type === "inbound-rtp" && r.kind === "video") frames = r.framesDecoded;
                    if (r.type === "inbound-rtp" && r.kind === "audio") received = r.totalSamplesReceived;
                });
            }
            const outputs = (await navigator.mediaDevices.enumerateDevices()).filter(d => d.kind === "audiooutput");
            const wanted = outputs.find(d => d.label === "Fake Audio Output 2");
            const sinks = Array.from(document.querySelectorAll("video")).map(v => v.sinkId);
            const level = peak;
            peak = 0;
            say("state", { phase: phase, video: labels.video, audio: labels.audio, states: mine.map(t => t.readyState),
                           preview: look(document.getElementById("me")), remote: look(document.getElementById("them")),
                           frames: frames, received: received, level: level, sinks: sinks, wanted: wanted ? wanted.deviceId : null });
        }, 1000);
    } catch (e) {
        say("error", { error: e.name + ": " + e.message });
    }
})();
</script>
</body></html>
"""
# devices: a site whose call is in a frame of another site (another host, the same port).
FRAME_PAGE = """<!doctype html>
<html><head><meta charset="utf-8"><title>Test room</title></head>
<body><script>
const frame = document.createElement("iframe");
frame.src = "http://localhost:" + location.port + "/call?who=frame";
frame.allow = "camera; microphone";
frame.width = 400;
frame.height = 200;
document.body.appendChild(frame);
</script></body></html>
"""
# devices: a site whose call opens in a pop-up.
OPENER_PAGE = """<!doctype html>
<html><head><meta charset="utf-8"><title>Test opener</title></head>
<body><script>window.open("/call?who=popup", "_blank", "width=400,height=300");</script></body></html>
"""
# devices: a site allowed no device, listening for what Sioul sends to calls.
LISTEN_PAGE = """<!doctype html>
<html><head><meta charset="utf-8"><title>Test bank</title></head>
<body><script>
const say = (event, data) => navigator.sendBeacon("/report", JSON.stringify(Object.assign({ event: event, who: "bank" }, data || {})));
addEventListener("sioul-devices", e => say("heard", { detail: e.detail }));
say("load", {});
</script></body></html>
"""
DEVICE_PAGES = {"/call": CALL_PAGE, "/frame": FRAME_PAGE, "/opener": OPENER_PAGE, "/listen": LISTEN_PAGE}


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
        page = DEVICE_PAGES.get(self.path.split("?")[0])
        if page is not None:
            body = page.encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
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


def pin(config, port, popup=False, sites=None):
    """The local page as the profile's only site, kept open as a chat; or
    `sites`, (id, name, path, type) each."""
    kept, skipping = [], False
    for line in config.read_text().splitlines():
        if line.strip().startswith("["):
            skipping = line.strip() == "[[site]]"
        if not skipping:
            kept.append(line)
    if sites is None:
        sites = [(SITE_ID, "Test site", "/site" + ("?popup=1" if popup else ""), "chat")]
    for site_id, name, path, kind in sites:
        kept += ["", "[[site]]", f'id = "{site_id}"', f'name = "{name}"', f'url = "http://127.0.0.1:{port}{path}"',
                 f'site = "{kind}"', 'area = "work+admin+leisure"', "background = true", "announced_by = []", ""]
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


def sandbox(profile, own_network=False, no_devices=False):
    """Bubblewrap: the profile as the home of a user "demo", the real home out of reach;
    with `own_network`, a network and a /tmp of its own (Weston's and Xwayland's sockets);
    with `no_devices`, a /dev of its own (no camera, no sound card) and no user's
    runtime folder (no sound server): no real camera or microphone can be opened."""
    command = ["bwrap", "--dev-bind", "/", "/", "--tmpfs", "/home"]
    if own_network:
        command += ["--unshare-net", "--tmpfs", "/tmp"]
    if no_devices:
        command += ["--dev", "/dev", "--tmpfs", "/run/user"]
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


# devices

DEVICE_SITES = [("test-bank", "Test bank", "/listen", "mailbox"), ("test-popup", "Test opener", "/opener", "chat"),
                ("test-frame", "Test room", "/frame", "chat"), ("test-chat", "Test call", "/call?who=direct&effect=canvas", "chat")]
FAKE_DEVICES = "--use-fake-device-for-media-stream=device-count=3 --allow-loopback-in-peer-connection"


def devices_inside(profile, app):
    """In the sandbox: the four sites, Sioul taking the grab steps "site-devices"
    on Chromium's fake devices; what the pages and Sioul said, into PROFILE/devices.json."""
    server = Server()
    server.run = "devices"
    threading.Thread(target=server.serve_forever, daemon=True).start()
    config = pathlib.Path(f"{HOME}/.config/sioul/config.toml")
    pin(config, server.server_address[1], sites=DEVICE_SITES)
    # Chosen before Sioul starts, as you would have: the first calls, before
    # any site was allowed a device, must start on them.
    chosen = 'camera = "fake_device_2"\nmicrophone = "Fake Audio Input 1"\n'
    text = config.read_text()
    config.write_text(text.replace("\n[calls]\n", "\n[calls]\n" + chosen, 1) if "\n[calls]\n" in text else text + "\n[calls]\n" + chosen)
    runtime = pathlib.Path("/tmp/sioul-runtime")
    runtime.mkdir(mode=0o700, exist_ok=True)
    said = {"reports": []}
    try:
        env = sioul_env("site-devices", profile)
        env.update({"HOME": HOME, "XDG_CONFIG_HOME": f"{HOME}/.config", "XDG_DATA_HOME": f"{HOME}/.local/share",
                    "XDG_STATE_HOME": f"{HOME}/.local/state", "XDG_CACHE_HOME": f"{HOME}/.cache",
                    "XDG_RUNTIME_DIR": str(runtime), "QTWEBENGINE_CHROMIUM_FLAGS": FAKE_DEVICES})
        outside = {k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "PULSE_SERVER", "PIPEWIRE_REMOTE")}
        with open(profile / "run.log", "w") as log:
            sioul = subprocess.Popen(["dbus-run-session", "--", str(app)], stdout=log, stderr=subprocess.STDOUT, env={**outside, **env})
            try:
                sioul.wait(150)
            except subprocess.TimeoutExpired:
                said["problem"] = "Sioul did not end its steps within 150 s"
                sioul.kill()
        with server.lock:
            said["reports"] = list(server.reports)
        text = (profile / "run.log").read_text(errors="replace")
        line = re.search(r"sioul-devices: the line above the site says: (.*)", text)
        said["line"] = line.group(1).strip() if line else None
        for key, pattern in (("calling", r"sioul-devices: calling (\{.*\})"), ("after", r"sioul-devices: after hanging up, calling (\{.*\})")):
            found = re.search(pattern, text)
            said[key] = json.loads(found.group(1)) if found else None
    finally:
        server.shutdown()
        (profile / "devices.json").write_text(json.dumps(said, indent=1))


def grey(colour):
    """Fake camera 1's picture: grey, not black (the others are green)."""
    return colour is not None and max(colour) - min(colour) <= 6 and max(colour) >= 4


def run_devices(args, app, folder):
    """The devices check, its sandbox made here."""
    if shutil.which("bwrap") is None:
        print("devices: needs bubblewrap.")
        return True
    profile = folder / "devices"
    make_profile(profile, True)
    command, _ = sandbox(profile, own_network=True, no_devices=True)
    subprocess.run(command + [sys.executable, str(REPO / "tools/check-sites.py"), "devices", "--inside", str(profile), "--app", str(app)],
                   env={k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "SESSION_MANAGER")},
                   timeout=240)
    said = json.loads((profile / "devices.json").read_text())
    if said.get("problem"):
        print("devices: " + said["problem"])
        return True
    reports = said["reports"]
    errors = [r for r in reports if r.get("event") == "error"]
    states = [r for r in reports if r.get("event") == "state"]

    def last(who, phase):
        found = [r for r in states if r.get("who") == who and r.get("phase") == phase]
        return found[-1] if found else None

    def every(who, phase):
        return [r for r in states if r.get("who") == who and r.get("phase") == phase]

    results = []

    def check(name, good, text):
        results.append((name, good, text))

    for error in errors:
        check(f"{error.get('who')} call", False, error.get("error", ""))
    for who in ("direct", "frame", "popup"):
        # The first word of each call: it must never have been on another device.
        start, camera = (every(who, "start") or [None])[0], last(who, "camera")
        if start is None:
            check(f"{who} start", False, "the call never reported")
            continue
        check(f"{who} start", start["video"] == "fake_device_2" and start["audio"] == "Fake Audio Input 1",
              f"camera {start['video']}, microphone {start['audio']} (chose fake_device_2, Fake Audio Input 1)")
        check(f"{who} camera", camera is not None and camera["video"] == "fake_device_1" and grey(camera["preview"]),
              f"camera {camera and camera['video']}, preview {camera and camera['preview']} (chose fake_device_1, grey)")
        speaker = last(who, "speaker")
        check(f"{who} speaker", speaker is not None and speaker["wanted"] is not None and all(s == speaker["wanted"] for s in speaker["sinks"]),
              f"elements on {speaker and speaker['sinks']}, chosen {speaker and speaker['wanted']}")
        microphone = every(who, "microphone")
        flowing = len(microphone) >= 2 and microphone[-1]["received"] > microphone[0]["received"]
        check(f"{who} microphone", bool(microphone) and microphone[-1]["audio"] == "Fake Audio Input 2" and flowing,
              f"microphone {microphone[-1]['audio'] if microphone else None}, sound received {'growing' if flowing else 'not growing'}")
        missing = last(who, "missing")
        check(f"{who} missing camera", missing is not None and missing["video"] == "fake_device_1",
              f"camera {missing and missing['video']} (kept fake_device_1)")
        hung = last(who, "hangup")
        check(f"{who} hang-up", hung is not None and all(s == "ended" for s in hung["states"]),
              f"tracks {hung and hung['states']}")
    hung = last("direct", "hangup")
    again = [r for r in states if r.get("who") == "direct" and r.get("phase") == "start" and hung is not None and r["heard"] > hung["heard"]]
    check("direct reloaded", bool(again) and again[0]["video"] == "fake_device_0",
          f"camera {again[0]['video'] if again else None} (chose fake_device_0 before the reload)")
    camera = last("direct", "camera")
    check("direct effect", camera is not None and grey(camera["remote"]), f"remote picture {camera and camera['remote']} (grey)")
    meter = every("direct", "microphone")
    check("direct meter", any(r["level"] > 0 for r in meter), f"levels {[r['level'] for r in meter]}")
    muted, unmuted = last("direct", "mute"), last("direct", "unmute")
    check("direct own mute", muted is not None and muted["remote"] == [0, 0, 0] and unmuted is not None and grey(unmuted["remote"]),
          f"off {muted and muted['remote']}, on again {unmuted and unmuted['remote']}")
    heard = [r for r in reports if r.get("who") == "bank" and r.get("event") == "heard"]
    check("bank", not heard and any(r.get("who") == "bank" and r.get("event") == "load" for r in reports), f"{len(heard)} change(s) heard")
    line = said.get("line") or ""
    check("line", "Unplugged camera" in line, line or "nothing")
    calling, after = said.get("calling") or {}, said.get("after") or {}
    check("call button", calling.get("chat") and calling.get("frame") and calling.get("popup") and not calling.get("bank"),
          f"during {calling}")
    check("calls ended", after != {} and not any(after.values()), f"after {after}")
    for name, good, text in results:
        print(f"devices {name:20} {text[:90]:90} {'as expected' if good else 'WRONG'}")
    return not all(good for _, good, _ in results)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("check", choices=("quit", "share", "devices"))
    parser.add_argument("--app", type=pathlib.Path, default=REPO / "target/debug/sioul-app")
    parser.add_argument("--ways", default="window,term,during", help="quit: window, term, during, termall, kill, joined by commas")
    parser.add_argument("--after", default="3", help="quit: seconds between the page loading and the signal, several joined by commas")
    parser.add_argument("--keep", type=pathlib.Path, help="keep the profiles and logs in this folder")
    parser.add_argument("--no-bwrap", action="store_true", help="quit: run without bubblewrap (the profile's folder is HOME)")
    parser.add_argument("--inside", type=pathlib.Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    app = args.app.resolve()
    if args.inside:
        (devices_inside if args.check == "devices" else share_inside)(args.inside, app)
        return
    if not app.exists():
        sys.exit(f"No {app}: build it first (CARGO_INCREMENTAL=0 cargo build -p sioul-app).")
    bwrap = not args.no_bwrap and shutil.which("bwrap") is not None
    folder = args.keep or pathlib.Path(tempfile.mkdtemp(prefix="sioul-sites."))
    folder.mkdir(parents=True, exist_ok=True)
    try:
        failed = {"quit": lambda: run_quit(args, app, folder, bwrap), "share": lambda: run_share(args, app, folder),
                  "devices": lambda: run_devices(args, app, folder)}[args.check]()
    finally:
        if args.keep is None:
            shutil.rmtree(folder, ignore_errors=True)
        else:
            print(f"Profiles and logs: {folder}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
