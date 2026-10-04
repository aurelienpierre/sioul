#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""Sioul's own icon, in every size and format a system asks for, from the
drawings in data/icons/:

- sioul.svg: the icon, a quill on Sioul's green;
- sioul-small.svg: the same drawn for 32 pixels and under;
- sioul-symbolic.svg: one colour, the desktop's text colour (panels, menus);
- sioul-mark.svg, sioul-mark-on-dark.svg: the quill alone, for pages.

Writes, from them:

- data/icons/hicolor/: the icon theme folders a Linux desktop reads
  (scalable, symbolic, and PNGs from 16 to 512 pixels);
- packaging/windows/sioul.ico and packaging/macos/sioul.icns;
- website/docs/assets/images/: the favicon (SVG, and PNG for older
  browsers), the logo, and the icon Apple's systems put on a home screen.

The window takes its icon from data/icons/hicolor through
crates/sioul-app/app.qrc. Needs rsvg-convert (librsvg). Run again after
changing a drawing; the results are committed.
"""

import shutil
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ICONS = ROOT / "data" / "icons"
HICOLOR = ICONS / "hicolor"
SITE = ROOT / "website" / "docs" / "assets" / "images"

# Under 48 pixels the drawing made for small sizes reads better.
SMALL = [16, 22, 24, 32]
LARGE = [48, 64, 128, 256, 512]


def render(svg: Path, size: int, background: str | None = None) -> bytes:
    """The drawing as a PNG of `size` pixels square."""
    command = ["rsvg-convert", "--width", str(size), "--height", str(size), "--keep-aspect-ratio"]
    if background:
        command += ["--background-color", background]
    return subprocess.run(command + [str(svg)], check=True, capture_output=True).stdout


def drawing_for(size: int) -> Path:
    return ICONS / ("sioul-small.svg" if size <= 32 else "sioul.svg")


def ico(sizes: list[int]) -> bytes:
    """A Windows icon holding one PNG per size (Windows Vista and later read PNG entries)."""
    images = [render(drawing_for(size), size) for size in sizes]
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = len(header) + 16 * len(images)
    entries, data = b"", b""
    for size, png in zip(sizes, images):
        # 0 stands for 256 pixels in an entry's one-byte width and height.
        side = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(png), offset + len(data))
        data += png
    return header + entries + data


def icns() -> bytes:
    """A macOS icon: PNG entries from 16 to 1024 pixels, the doubled ones for Retina screens."""
    kinds = [
        ("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256), ("ic09", 512), ("ic10", 1024),
        ("ic11", 32), ("ic12", 64), ("ic13", 256), ("ic14", 512),
    ]
    body = b""
    for kind, size in kinds:
        png = render(drawing_for(size), size)
        body += kind.encode("ascii") + struct.pack(">I", 8 + len(png)) + png
    return b"icns" + struct.pack(">I", 8 + len(body)) + body


def write(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    print(path.relative_to(ROOT))


def main() -> int:
    if shutil.which("rsvg-convert") is None:
        sys.exit("rsvg-convert is needed (librsvg: dnf install librsvg2-tools, apt install librsvg2-bin)")
    # The icon theme: the drawing itself, its one-colour form, and PNGs.
    if HICOLOR.exists():
        shutil.rmtree(HICOLOR)
    write(HICOLOR / "scalable" / "apps" / "sioul.svg", (ICONS / "sioul.svg").read_bytes())
    write(HICOLOR / "symbolic" / "apps" / "sioul-symbolic.svg", (ICONS / "sioul-symbolic.svg").read_bytes())
    for size in SMALL + LARGE:
        write(HICOLOR / f"{size}x{size}" / "apps" / "sioul.png", render(drawing_for(size), size))
    # Windows and macOS.
    write(ROOT / "packaging" / "windows" / "sioul.ico", ico([16, 20, 24, 32, 40, 48, 64, 256]))
    write(ROOT / "packaging" / "macos" / "sioul.icns", icns())
    # The website: the favicon drawn for small sizes, a PNG one for browsers
    # without SVG favicons, the logo in the header, and Apple's home-screen
    # icon, square and filled (its corners are rounded by the system).
    write(SITE / "favicon.svg", (ICONS / "sioul-small.svg").read_bytes())
    write(SITE / "favicon-32.png", render(ICONS / "sioul-small.svg", 32))
    write(SITE / "logo.svg", (ICONS / "sioul.svg").read_bytes())
    write(SITE / "apple-touch-icon.png", render(ICONS / "sioul.svg", 180, background="#4c6b5c"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
