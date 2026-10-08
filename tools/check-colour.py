#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""colour: checks the sites' colours end to end (docs/colour.md), with no
graphics card and no real screen.

    tools/check-colour.py [--app target/debug/sioul-app] [--keep FOLDER]

Inside bubblewrap, with a network of its own and a /dev without the graphics
cards (no /dev/dri, no /dev/nvidia*), Weston runs headless (pixman) with
Xwayland (without glamor): an X screen nobody sees. The check writes a test
colour profile on that screen's root window (`_ICC_PROFILE`): Adobe RGB
(1998)'s published primaries and gamma, never a real screen's. Sioul runs on a
demo profile on that X screen, drawn by Mesa's software OpenGL (llvmpipe), a
local page of colour patches pinned as its only site, and takes its grab steps
"site-colour": a picture in the screen's colours, one with calmer colours ("A
little") too, one with neither. Each patch is read back from each picture and
compared with what it should be, computed here from the profile in floating
point. It passes when the plain picture holds the page's own numbers, and the
two others are within one 8-bit code of what they should be.
"""

import argparse
import ctypes
import http.server
import importlib.util
import json
import math
import os
import pathlib
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import threading

REPO = pathlib.Path(__file__).resolve().parent.parent
HOME = "/home/demo"
SITE_ID = "test-chat"
# Loud colours of the web, a few of Sioul's own neighbours, greys: none of them Sioul's theme.
PATCHES = ["#25d366", "#ff0000", "#4285f4", "#ff8c00", "#5865f2", "#7f3fbf", "#00a0a0", "#e0c040", "#4c6b5d", "#808081", "#202122", "#f4f1ed"]

# The test profile: Adobe RGB (1998), as Adobe publishes it: its colorants
# adapted to D50 (Bradford), and a gamma of 563/256.
ADOBE_D50 = [[0.6097559, 0.2052401, 0.1492240], [0.3111242, 0.6256560, 0.0632197], [0.0194811, 0.0608902, 0.7448387]]
GAMMA = 563 / 256
# sRGB's colorants adapted to D50, as Little CMS's own sRGB profile has them.
SRGB_D50 = [[0.4360747, 0.3850649, 0.1430804], [0.2225045, 0.7168786, 0.0606169], [0.0139322, 0.0971045, 0.7141733]]


def s15(v):
    return round(v * 65536)


def test_profile():
    """A small ICC v2 display profile: a matrix and one gamma, its colorants as the file stores them."""
    def xyz(x, y, z):
        return b"XYZ " + bytes(4) + struct.pack(">iii", s15(x), s15(y), s15(z))
    def text(t):
        raw = t.encode() + b"\0"
        return b"desc" + bytes(4) + struct.pack(">I", len(raw)) + raw + bytes(4 + 4 + 2 + 1 + 67)
    curve = b"curv" + bytes(4) + struct.pack(">IH", 1, round(GAMMA * 256)) + bytes(2)
    tags = [(b"desc", text("Sioul test, Adobe RGB (1998) compatible")), (b"cprt", b"text" + bytes(4) + b"No copyright, test data\0"),
            (b"wtpt", xyz(0.9642, 1.0, 0.8249)),
            (b"rXYZ", xyz(*[ADOBE_D50[i][0] for i in range(3)])), (b"gXYZ", xyz(*[ADOBE_D50[i][1] for i in range(3)])),
            (b"bXYZ", xyz(*[ADOBE_D50[i][2] for i in range(3)])), (b"rTRC", curve), (b"gTRC", curve), (b"bTRC", curve)]
    offset = 128 + 4 + 12 * len(tags)
    table, data = b"", b""
    for sig, body in tags:
        while (offset + len(data)) % 4:
            data += b"\0"
        table += sig + struct.pack(">II", offset + len(data), len(body))
        data += body
    body = struct.pack(">I", len(tags)) + table + data
    size = 128 + len(body)
    header = struct.pack(">I4sI4s4s4s", size, b"none", 0x02100000, b"mntr", b"RGB ", b"XYZ ") + bytes(12) + b"acsp" + bytes(4) + struct.pack(">I", 0)
    header += bytes(8) + bytes(8) + struct.pack(">I", 0) + struct.pack(">iii", s15(0.9642), s15(1.0), s15(0.8249)) + bytes(4)
    header += bytes(128 - len(header))
    return header + body


def stored(v):
    """A colorant as the profile stores it (s15Fixed16)."""
    return s15(v) / 65536


def mul(m, v):
    return [sum(m[i][k] * v[k] for k in range(3)) for i in range(3)]


def invert(m):
    a, b, c = m[0]
    d, e, f = m[1]
    g, h, i = m[2]
    det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
    return [[(e * i - f * h) / det, (c * h - b * i) / det, (b * f - c * e) / det],
            [(f * g - d * i) / det, (a * i - c * g) / det, (c * d - a * f) / det],
            [(d * h - e * g) / det, (b * g - a * h) / det, (a * e - b * d) / det]]


def decode(v):
    return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4


def soften(linear, limit):
    """colour.rs's calmer colours: OKLCh chroma limited softly at `limit`."""
    lms = [x ** (1 / 3) if x > 0 else -((-x) ** (1 / 3)) for x in mul([[0.4122214708, 0.5363325363, 0.0514459929], [0.2119034982, 0.6806995451, 0.1073969566], [0.0883024619, 0.2817188376, 0.6299787005]], linear)]
    lab = mul([[0.2104542553, 0.7936177850, -0.0040720468], [1.9779984951, -2.4285922050, 0.4505937099], [0.0259040371, 0.7827717662, -0.8086757660]], lms)
    c = math.hypot(lab[1], lab[2])
    if c > 1e-9:
        k = limit * math.tanh(c / limit) / c
        lab = [lab[0], lab[1] * k, lab[2] * k]
    lms = [x ** 3 for x in mul([[1.0, 0.3963377774, 0.2158037573], [1.0, -0.1055613458, -0.0638541728], [1.0, -0.0894841775, -1.2914855480]], lab)]
    out = mul([[4.0767416621, -3.3077115913, 0.2309699292], [-1.2684380046, 2.6097574011, -0.3413193965], [-0.0041960863, -0.7034186147, 1.7076147010]], lms)
    return [min(max(v, 0.0), 1.0) for v in out]


def expected(colour, calmer):
    """What the screen must get for a patch, in 8-bit codes (floats)."""
    rgb = [int(colour[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    linear = [decode(v) for v in rgb]
    if calmer:
        linear = soften(linear, 0.12)
    screen = mul(invert([[stored(v) for v in row] for row in ADOBE_D50]), mul(SRGB_D50, linear))
    return [min(max(v, 0.0), 1.0) ** (1 / (round(GAMMA * 256) / 256)) * 255 for v in screen]


def page():
    cells = "".join(f'<div style="background:{c}"></div>' for c in PATCHES)
    return ("<!doctype html><meta charset=utf-8><title>Colours</title><style>body{margin:0;background:#fff;display:flex;flex-wrap:wrap}"
            "div{width:110px;height:110px}</style>" + cells).encode()


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        body = page()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def pin(config, port):
    """The patches' page as the profile's only site, kept open as a chat."""
    kept, skipping = [], False
    for line in config.read_text().splitlines():
        if line.strip().startswith("["):
            skipping = line.strip() == "[[site]]"
        if not skipping:
            kept.append(line)
    kept += ["", "[[site]]", f'id = "{SITE_ID}"', 'name = "Test site"', f'url = "http://127.0.0.1:{port}/site"',
             'site = "chat"', 'area = "work+admin+leisure"', "background = true", "announced_by = []", ""]
    config.write_text("\n".join(kept))


def set_profile(display, data):
    """The test profile as `_ICC_PROFILE` on the X screen's root window (that X server's, in the sandbox)."""
    x11 = ctypes.CDLL("libX11.so.6")
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XDefaultRootWindow.restype = ctypes.c_ulong
    x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
    x11.XInternAtom.restype = ctypes.c_ulong
    x11.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
    x11.XChangeProperty.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_char_p, ctypes.c_int]
    x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
    dpy = x11.XOpenDisplay(display.encode())
    if not dpy:
        return False
    atom = x11.XInternAtom(dpy, b"_ICC_PROFILE", 0)
    # XA_CARDINAL (6), 8-bit values, PropModeReplace (0).
    x11.XChangeProperty(dpy, x11.XDefaultRootWindow(dpy), atom, 6, 8, 0, data, len(data))
    x11.XSync(dpy, 0)
    x11.XCloseDisplay(dpy)
    return True


def inside(profile, app):
    """In the sandbox: the page, Weston with Xwayland, the profile on its root, Sioul's steps."""
    checks = importlib.util.spec_from_file_location("check_sites", REPO / "tools/check-sites.py")
    sites = importlib.util.module_from_spec(checks)
    checks.loader.exec_module(sites)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    pin(pathlib.Path(f"{HOME}/.config/sioul/config.toml"), server.server_address[1])
    runtime = pathlib.Path("/tmp/sioul-runtime")
    runtime.mkdir(mode=0o700, exist_ok=True)
    pathlib.Path("/tmp/.X11-unix").mkdir(exist_ok=True)
    os.chmod("/tmp/.X11-unix", 0o1777)
    base = {k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "XDG_SESSION_TYPE", "SESSION_MANAGER")}
    # Mesa's software renderers only: OpenGL and EGL from Mesa, never another
    # vendor's library (NVIDIA's crashed Xwayland, with no device to open).
    mesa = {"LIBGL_ALWAYS_SOFTWARE": "1", "GALLIUM_DRIVER": "llvmpipe", "__GLX_VENDOR_LIBRARY_NAME": "mesa",
            "__EGL_VENDOR_LIBRARY_FILENAMES": "/usr/share/glvnd/egl_vendor.d/50_mesa.json"}
    # Xwayland without glamor: no graphics card is used (and none is in this /dev).
    weston_env = dict(base, XDG_RUNTIME_DIR=str(runtime), XWAYLAND_NO_GLAMOR="1", **mesa)
    said = {}
    weston_log = open(profile / "weston.log", "w")
    weston = subprocess.Popen(["weston", "--backend=headless", "--renderer=pixman", "--xwayland", "--socket=wayland-sioul",
                               "--width=1400", "--height=900", "--idle-time=0", "--no-config"],
                              stdout=weston_log, stderr=subprocess.STDOUT, env=weston_env)
    try:
        def display():
            found = re.search(r"xserver listening on display (:\d+)", (profile / "weston.log").read_text())
            return found.group(1) if found else None
        if not sites.wait_for(display, 30):
            said["problem"] = "Weston gave no X display (see weston.log)"
            return
        if not set_profile(display(), test_profile()):
            said["problem"] = "the X display could not be opened to set the profile"
            return
        env = sites.sioul_env("site-colour", profile)
        # On that X screen, drawn by Mesa's software OpenGL; no Wayland session in sight.
        env.pop("QT_QUICK_BACKEND", None)
        env.update(mesa)
        env.update({"QT_QPA_PLATFORM": "xcb", "DISPLAY": display(),
                    "HOME": HOME, "XDG_CONFIG_HOME": f"{HOME}/.config", "XDG_DATA_HOME": f"{HOME}/.local/share",
                    "XDG_STATE_HOME": f"{HOME}/.local/state", "XDG_CACHE_HOME": f"{HOME}/.cache", "XDG_RUNTIME_DIR": str(runtime)})
        with open(profile / "run.log", "w") as log:
            sioul = subprocess.Popen(["dbus-run-session", "--", str(app)], stdout=log, stderr=subprocess.STDOUT, env=dict(base, **env))
            try:
                sioul.wait(200)
            except subprocess.TimeoutExpired:
                said["problem"] = "Sioul did not end its steps within 200 s"
                sioul.kill()
    finally:
        weston.terminate()
        try:
            weston.wait(10)
        except subprocess.TimeoutExpired:
            weston.kill()
        weston_log.close()
        server.shutdown()
        (profile / "colour.json").write_text(json.dumps(said))


def patches(picture, plain):
    """Each patch's mean colour in `picture`, found where `plain` shows its exact numbers."""
    from PIL import Image
    shown = Image.open(picture).convert("RGB")
    reference = Image.open(plain).convert("RGB")
    width, height = reference.size
    pixels, found = reference.load(), {}
    wanted = {tuple(int(c[i:i + 2], 16) for i in (1, 3, 5)): c for c in PATCHES}
    places = {c: [] for c in PATCHES}
    for y in range(0, height, 2):
        for x in range(0, width, 2):
            colour = wanted.get(pixels[x, y])
            if colour:
                places[colour].append((x, y))
    for colour, points in places.items():
        if len(points) < 200:
            found[colour] = None
            continue
        xs, ys = sorted(p[0] for p in points), sorted(p[1] for p in points)
        cx, cy = xs[len(xs) // 2], ys[len(ys) // 2]
        block = [shown.getpixel((x, y)) for x in range(cx - 5, cx + 5) for y in range(cy - 5, cy + 5)]
        found[colour] = [sum(p[k] for p in block) / len(block) for k in range(3)]
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--app", type=pathlib.Path, default=REPO / "target/debug/sioul-app")
    parser.add_argument("--keep", type=pathlib.Path, help="keep the profile, the pictures and the logs in this folder")
    parser.add_argument("--inside", type=pathlib.Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    app = args.app.resolve()
    if args.inside:
        inside(args.inside, app)
        return
    if not app.exists():
        sys.exit(f"No {app}: build it first (CARGO_INCREMENTAL=0 cargo build -p sioul-app).")
    if shutil.which("bwrap") is None or shutil.which("weston") is None or shutil.which("Xwayland") is None:
        sys.exit("check-colour.py needs bubblewrap, Weston and Xwayland.")
    checks = importlib.util.spec_from_file_location("check_sites", REPO / "tools/check-sites.py")
    sites = importlib.util.module_from_spec(checks)
    checks.loader.exec_module(sites)
    folder = args.keep or pathlib.Path(tempfile.mkdtemp(prefix="sioul-colour."))
    folder.mkdir(parents=True, exist_ok=True)
    profile = folder / "colour"
    sites.make_profile(profile, True)
    command, _ = sites.sandbox(profile, own_network=True)
    # A /dev of its own, without the graphics cards: nothing here may reach one.
    command[4:4] = ["--dev", "/dev", "--tmpfs", "/dev/shm"]
    subprocess.run(command + [sys.executable, str(REPO / "tools/check-colour.py"), "--inside", str(profile), "--app", str(app)],
                   env={k: v for k, v in os.environ.items() if k not in ("DISPLAY", "WAYLAND_DISPLAY", "SESSION_MANAGER")}, timeout=300)
    said = json.loads((profile / "colour.json").read_text())
    if said.get("problem"):
        print("colour: " + said["problem"])
        sys.exit(1)
    shots = profile / "shots"
    names = ["colour-screen", "colour-calmer", "colour-plain"]
    missing = [n for n in names if not (shots / f"{n}.png").exists()]
    if missing:
        print("colour: no picture " + ", ".join(missing) + f" (see {profile / 'run.log'})")
        sys.exit(1)
    plain = shots / "colour-plain.png"
    read = {n: patches(shots / f"{n}.png", plain) for n in names}
    failed = False
    worst = {"colour-screen": 0.0, "colour-calmer": 0.0, "colour-plain": 0.0}
    print(f"{'patch':9} {'plain':>15} {'screen (want)':>32} {'calmer (want)':>32}")
    for colour in PATCHES:
        if read["colour-plain"][colour] is None:
            print(f"{colour}: not found in the plain picture")
            failed = True
            continue
        own = [int(colour[i:i + 2], 16) for i in (1, 3, 5)]
        cells = []
        for name, want in (("colour-plain", own), ("colour-screen", expected(colour, False)), ("colour-calmer", expected(colour, True))):
            got = read[name][colour]
            worst[name] = max(worst[name], max(abs(got[k] - want[k]) for k in range(3)))
            cells.append("/".join(f"{v:.0f}" for v in got) + ("" if name == "colour-plain" else " (" + "/".join(f"{v:.1f}" for v in want) + ")"))
        print(f"{colour:9} {cells[0]:>15} {cells[1]:>32} {cells[2]:>32}")
    limits = {"colour-plain": 0.5, "colour-screen": 1.0, "colour-calmer": 1.0}
    for name in names:
        good = worst[name] <= limits[name]
        print(f"{name:14} largest difference {worst[name]:.2f} codes, at most {limits[name]}: {'as expected' if good else 'WRONG'}")
        failed = failed or not good
    errors = [line for line in (profile / "run.log").read_text().splitlines() if re.search(r"ShaderEffect|qsb|colour|TypeError|ReferenceError", line)]
    for line in errors[:10]:
        print("log: " + line)
    if not args.keep:
        shutil.rmtree(folder, ignore_errors=True)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
