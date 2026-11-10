#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""What Sioul's window takes while it is left alone, on a demo profile.

    tools/measure-idle.py [--release] OUT [--sites N] [--gpu] [--bulk N] [--items N] [--shared] [--perf] [--app PATH]

The window starts as tools/measure-memory.py starts it (bubblewrap, a demo
profile made anew in OUT/profile, offscreen, software rendering, SIOUL_DEMO=1:
nothing fetched nor sent by itself) with the step list "rest" (main.qml): the
Porch shown, the window hidden as to the tray, then the profile's sites opened
and kept behind the Porch, then the window hidden again with them, then
hidden with a site in view, three minutes each, nothing done meanwhile. The log
says "sioul-rest: <moment>" as each begins; from 15 seconds after it to 175,
/proc is read: the CPU of the window's process (all its threads, as `ps`
counts it) and of the processes under it (Qt WebEngine's), its wakes (context
switches, a thread woken each), and its busiest threads by name.

--sites N: N local pages standing in for chats (measure-memory.py's: a long
list, a canvas drawn each second), kept open; 0: none. Without it, the demo's
sites, which the sandbox's own network cannot reach.
--bulk N: a mailbox of N more messages (make-demo.py --bulk).
--items N: N more past events in the demo's personal calendar and N more
contacts, invented (Event n, Person n at example.org), for what grows with them.
--shared: sharing started first, in a folder of the run's own (the step list
"rest-shared"), its key kept by a GNOME keyring started in the sandbox
(gnome-keyring-daemon): its watch of the shared files and its minute's
exchange run too.
--perf: `perf record` on the window's process through each moment (its
user-space side, sampled at 999 Hz, with call graphs from the frame tables),
into OUT/perf-<moment>.data, and its busiest symbols printed.
--app PATH: another build of the window (an earlier one kept aside, to
compare), in place of target/release/sioul-app.

Writes OUT/idle.json and prints a table. The figures in docs/building.md
("Measuring the load", at rest) were made with it. Linux only; build first.
"""

import argparse
import importlib.util
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
spec = importlib.util.spec_from_file_location("measure_memory", REPO / "tools/measure-memory.py")
memory = importlib.util.module_from_spec(spec)
spec.loader.exec_module(memory)
HOME = memory.HOME
HZ = os.sysconf("SC_CLK_TCK")
# Each moment measured from 15 seconds after its start to 175: the move into it settled, its end not reached.
FROM, TO = 15, 175


def threads(pid):
    """Each thread of `pid`: {tid: (name, CPU ticks, context switches)}."""
    out = {}
    for tid in os.listdir(f"/proc/{pid}/task"):
        try:
            with open(f"/proc/{pid}/task/{tid}/stat") as f:
                s = f.read()
            name = s[s.index("(") + 1:s.rindex(")")]
            fields = s[s.rindex(")") + 2:].split()
            ctx = 0
            with open(f"/proc/{pid}/task/{tid}/status") as f:
                for line in f:
                    if line.startswith(("voluntary_ctxt_switches", "nonvoluntary_ctxt_switches")):
                        ctx += int(line.split(":")[1])
            out[int(tid)] = (name, int(fields[11]) + int(fields[12]), ctx)
        except (OSError, ValueError):
            pass
    return out


def cpu_of(pid):
    try:
        with open(f"/proc/{pid}/stat") as f:
            fields = f.read().rsplit(")", 1)[1].split()
        return int(fields[11]) + int(fields[12])
    except (OSError, IndexError, ValueError):
        return None


def reading(pid):
    children = {child: cpu_of(child) for child in memory.descendants(pid)}
    return dict(at=time.time(), threads=threads(pid), window=cpu_of(pid), children={c: t for c, t in children.items() if t is not None})


def between(a, b):
    """What the window and the processes under it took between two readings."""
    seconds = b["at"] - a["at"]
    pct = lambda ticks: ticks / HZ / seconds * 100
    by_name = {}
    for tid, (name, cpu, ctx) in b["threads"].items():
        before = a["threads"].get(tid, (name, 0, 0))
        mine = by_name.setdefault(name, [0, 0, 0])
        mine[0] += cpu - before[1]
        mine[1] += ctx - before[2]
        mine[2] += 1
    busiest = sorted(by_name.items(), key=lambda kv: (-kv[1][0], -kv[1][1]))[:8]
    children = sum(t - a["children"].get(c, t) for c, t in b["children"].items())
    return dict(seconds=round(seconds, 1), window_pct=round(pct(b["window"] - a["window"]), 3), children_pct=round(pct(children), 3),
                children=len(b["children"]), wakes_per_s=round(sum(v[1] for v in by_name.values()) / seconds, 1),
                threads=len(b["threads"]),
                busiest=[dict(name=n, pct=round(pct(v[0]), 3), wakes_per_s=round(v[1] / seconds, 2), count=v[2]) for n, v in busiest])


def add_items(profile, count):
    """`count` past events (one a day back from 2020) and `count` contacts, invented."""
    calendar = profile / "data/sioul/calendars/cloud/personal"
    contacts = profile / "data/sioul/contacts/cloud/contacts"
    for n in range(count):
        day = time.gmtime(1_577_836_800 - n * 86_400)
        date = time.strftime("%Y%m%d", day)
        (calendar / f"bulk-event-{n}.ics").write_text(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Sioul//Demo profile//EN\r\nBEGIN:VEVENT\r\n"
            f"UID:bulk-event-{n}\r\nDTSTAMP:20191201T080000Z\r\nDTSTART:{date}T090000Z\r\nDTEND:{date}T100000Z\r\n"
            f"SUMMARY:Event {n}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")
        (contacts / f"bulk-contact-{n}.vcf").write_text(
            f"BEGIN:VCARD\r\nVERSION:3.0\r\nUID:bulk-contact-{n}\r\nFN:Person {n}\r\nN:{n};Person;;;\r\n"
            f"EMAIL;TYPE=home:person{n}@example.org\r\nEND:VCARD\r\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--release", action="store_true")
    parser.add_argument("out")
    parser.add_argument("--sites", type=int, default=-1, help="N local test sites in place of the demo's (0: no site)")
    parser.add_argument("--gpu", action="store_true", help="the real /dev, the graphics card reachable")
    parser.add_argument("--bulk", type=int, default=0, help="a mailbox of N more messages")
    parser.add_argument("--items", type=int, default=0, help="N more past events and N more contacts")
    parser.add_argument("--shared", action="store_true", help="sharing started first (step list rest-shared)")
    parser.add_argument("--perf", action="store_true", help="perf record through each moment")
    parser.add_argument("--app", help="another build of the window, to compare")
    args = parser.parse_args()
    app = pathlib.Path(args.app).resolve() if args.app else REPO / "target" / ("release" if args.release else "debug") / "sioul-app"
    if not app.exists():
        sys.exit(f"measure-idle.py: no {app}: build it first.")
    out = pathlib.Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    profile, shots = out / "profile", out / "shots"
    subprocess.run(["rm", "-rf", str(profile), str(shots)], check=True)
    shots.mkdir()
    now, zone = subprocess.run([sys.executable, str(REPO / "tools/demo/moment.py")], capture_output=True, text=True, check=True).stdout.split()
    if now == "none":
        sys.exit("measure-idle.py: no weekday is close enough now (moment.py): try again later.")
    subprocess.run([sys.executable, str(REPO / "tools/demo/make-demo.py"), "--into", str(profile), "--now", now, "--language", "en",
                    "--notes-at", f"{HOME}/Notes"] + (["--bulk", str(args.bulk)] if args.bulk else []), check=True, stdout=subprocess.DEVNULL)
    if args.items:
        add_items(profile, args.items)
    port = None
    if args.sites > 0:
        server = memory.http.server.ThreadingHTTPServer(("127.0.0.1", 0), memory.Site)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        port = server.server_address[1]
    if args.sites >= 0:
        memory.keep_sites(profile / "config/sioul/config.toml", port, args.sites)
    runtime = tempfile.mkdtemp(prefix="sioul-run.", dir="/tmp")
    env = {"HOME": HOME, "XDG_CONFIG_HOME": f"{HOME}/.config", "XDG_DATA_HOME": f"{HOME}/.local/share",
           "XDG_STATE_HOME": f"{HOME}/.local/state", "XDG_CACHE_HOME": f"{HOME}/.cache", "XDG_RUNTIME_DIR": runtime,
           "SIOUL_DEMO": "1", "SIOUL_GRAB": str(shots), "SIOUL_GRAB_STEPS": "rest-shared" if args.shared else "rest", "SIOUL_TEST_PASSWORD": "x", "SIOUL_THEME": "light",
           "FONTCONFIG_FILE": str(REPO / "tools/demo/fonts.conf"), "QT_FORCE_STDERR_LOGGING": "1", "QT_QPA_PLATFORM": "offscreen",
           "QT_QUICK_BACKEND": "software", "LANG": "en_US.UTF-8", "LC_ALL": "en_US.UTF-8", "TZ": zone}
    hide = ["--tmpfs", os.environ["XDG_RUNTIME_DIR"]] if os.environ.get("XDG_RUNTIME_DIR") else []
    built = []
    if (REPO / "target").is_symlink():
        target = str((REPO / "target").resolve())
        built = ["--ro-bind", target, target]
    if args.app:
        built += ["--ro-bind", str(app.parent), str(app.parent)]
    command = (["bwrap", "--die-with-parent"] + ([] if port else ["--unshare-net"]) + ["--dev-bind", "/", "/"]
               + ([] if args.gpu else ["--dev", "/dev"]) + ["--tmpfs", "/home"] + hide
               + ["--ro-bind", str(REPO), str(REPO)] + built
               + [x for inside, outside in (("config", ".config"), ("data", ".local/share"), ("state", ".local/state"), ("cache", ".cache"), ("notes", "Notes"))
                  for x in ("--bind", str(profile / inside), f"{HOME}/{outside}")]
               + ["--bind", str(out), str(out), "--bind", runtime, runtime, "--chdir", HOME, "env"]
               + [f"{k}={v}" for k, v in env.items()] + ["timeout", "--foreground", "1500", "dbus-run-session", "--"]
               # Sharing keeps its key in the keyring: GNOME's, started on the sandbox's own bus, unlocked with a test password.
               + (["sh", "-c", 'printf x | gnome-keyring-daemon --unlock --components=secrets > /dev/null && exec "$0"', str(app)] if args.shared else [str(app)]))
    base = {k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "SESSION_MANAGER", "DBUS_SESSION_BUS_ADDRESS")}
    log_path = out / "log"
    log = open(log_path, "w")
    started = time.time()
    launcher = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, env=base, start_new_session=True)
    results, marks, pending, recorders = {}, {}, {}, []
    try:
        pid = None
        while pid is None and time.time() - started < 30:
            pid = next((p for p in memory.descendants(launcher.pid) if memory.program(p) == str(app.resolve())), None)
            time.sleep(0.1)
        if pid is None:
            sys.exit(f"measure-idle.py: the window did not start (see {log_path}).")
        while launcher.poll() is None:
            for moment in re.findall(r"sioul-rest: (\S+)", log_path.read_text(errors="replace")):
                if moment not in marks and moment != "end":
                    marks[moment] = time.time()
                    pending[moment] = None
            for moment in list(pending):
                since = time.time() - marks[moment]
                if pending[moment] is None and since >= FROM:
                    pending[moment] = reading(pid)
                    if args.perf:
                        data = out / f"perf-{moment}.data"
                        recorders.append(subprocess.Popen(["perf", "record", "-q", "-F", "999", "-e", "cpu-clock:u", "--call-graph", "dwarf,16384", "--user-callchains", "-p", str(pid),
                                                           "-o", str(data), "--", "sleep", str(TO - FROM)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
                elif pending[moment] is not None and since >= TO:
                    results[moment] = between(pending.pop(moment), reading(pid))
            time.sleep(0.5)
    finally:
        try:
            os.killpg(launcher.pid, signal.SIGKILL)
        except OSError:
            pass
        log.close()
        subprocess.run(["rm", "-rf", runtime])
        for recorder in recorders:
            recorder.wait()
    (out / "idle.json").write_text(json.dumps(results, indent=1))
    print(f"{'moment':8} {'window':>9} {'under it':>9} {'wakes/s':>8}  (CPU, % of one core; {TO - FROM} s each)")
    for moment, r in results.items():
        print(f"{moment:8} {r['window_pct']:8.2f}% {r['children_pct']:8.2f}% {r['wakes_per_s']:8.1f}   {r['children']} processes under it, {r['threads']} threads")
        print("         busiest threads: " + ", ".join(f"{t['name']} {t['pct']:.2f}% {t['wakes_per_s']:.1f}/s" for t in r["busiest"][:6]))
    if args.perf:
        for moment in results:
            data = out / f"perf-{moment}.data"
            if data.exists():
                report = subprocess.run(["perf", "report", "-i", str(data), "--stdio", "--no-children", "--sort", "comm,dso,sym", "--percent-limit", "2", "-g", "none"],
                                        capture_output=True, text=True).stdout
                print(f"{moment}: " + "\n".join(line for line in report.splitlines() if line.strip() and not line.startswith("#"))[:3000])
    if "site-hidden" not in results:
        sys.exit("measure-idle.py: the steps did not end (see the log).")


if __name__ == "__main__":
    main()
