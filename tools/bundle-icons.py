#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""Bundles the Breeze icons Sioul uses into its window, as Qt resources.

Linux desktops have an icon theme; Windows and macOS have none, and some Linux
desktops lack Breeze's names. Every quoted name in the QML and Rust sources
that is a Breeze icon is copied, light and dark, into
crates/sioul-app/icons/{sioul,sioul-dark}/<context>/<size>/, with an
index.theme each and icons.qrc listing them. Breeze is LGPL-3.0-or-later
(https://invent.kde.org/frameworks/breeze-icons): its licence goes along.

Run again after using a new icon: tools/bundle-icons.py
"""

import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "crates" / "sioul-app"
OUT = APP / "icons"
SOURCES = list((APP / "qml").glob("*.qml")) + list(ROOT.glob("crates/*/src/*.rs"))
THEMES = {"sioul": Path("/usr/share/icons/breeze"), "sioul-dark": Path("/usr/share/icons/breeze-dark")}
SIZES = ["16", "22", "32"]

names = set()
for source in SOURCES:
    names.update(re.findall(r'"([a-z][a-z0-9]*(?:-[a-z0-9]+)+)"', source.read_text(encoding="utf-8")))

if OUT.exists():
    shutil.rmtree(OUT)
files = []
contexts = set()
for theme, base in THEMES.items():
    if not base.is_dir():
        sys.exit(f"{base}: Breeze is needed to bundle its icons (dnf install breeze-icon-theme)")
    for name in sorted(names):
        found = [p for size in SIZES for p in sorted(base.glob(f"*/{size}/{name}.svg"))]
        # Breeze draws some icons at 48 pixels only: the weather's, some applications' (the browser).
        for path in found or sorted(base.glob(f"applets/48/{name}.svg")) or sorted(base.glob(f"apps/48/{name}.svg")):
            context, size = path.parent.parent.name, path.parent.name
            target = OUT / theme / context / size / path.name
            target.parent.mkdir(parents=True, exist_ok=True)
            # Breeze links some icons to others: the file itself is copied.
            shutil.copyfile(path.resolve(), target)
            files.append(target.relative_to(OUT))
            contexts.add((context, size))
    directories = sorted(f"{c}/{s}" for c, s in contexts)
    index = [f"[Icon Theme]\nName={theme}\nComment=The Breeze icons Sioul uses (LGPL-3.0-or-later)\nDirectories={','.join(directories)}\n"]
    for d in directories:
        size = d.split("/")[1]
        index.append(f"[{d}]\nSize={size}\nType=Fixed\n")
    (OUT / theme / "index.theme").write_text("\n".join(index), encoding="utf-8")
    files.append(Path(theme) / "index.theme")

# Breeze's licence goes along with its icons.
licences = sorted(Path("/usr/share/licenses").glob("*breeze-icon*/*"))
if licences:
    shutil.copyfile(licences[0], OUT / "COPYING-ICONS")
else:
    (OUT / "COPYING-ICONS").write_text("Breeze icons: LGPL-3.0-or-later, https://invent.kde.org/frameworks/breeze-icons\n")
qrc = ['<!DOCTYPE RCC><RCC version="1.0">', '<qresource prefix="/icons">']
qrc += [f"    <file>{f.as_posix()}</file>" for f in sorted(set(files))]
qrc += ["</qresource>", "</RCC>", ""]
(OUT / "icons.qrc").write_text("\n".join(qrc), encoding="utf-8")
print(f"{len(names)} names looked at, {len(set(files))} files bundled in {OUT.relative_to(ROOT)}")
