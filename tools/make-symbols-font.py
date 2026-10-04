#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""Makes Sioul Symbols: the few symbols Sioul writes (→ ▸ ▾ ✓ ⚙…), cut from
DejaVu Sans, for systems whose fonts lack them.

Without it, Qt looks for each such symbol through the system's fonts, one
after the other: on a phone, a tenth of a second at the start. The window puts
this font first in line after the text's own (cpp/warmup.cpp). The symbols are
those the QML, Rust and translation sources write from the blocks of arrows,
mathematical operators, technical signs, shapes, symbols and dingbats.
DejaVu's licence (Bitstream Vera's and Arev's) allows changes under another
name; it goes along, in crates/sioul-app/fonts/.

Needs DejaVu Sans (dnf install dejavu-sans-fonts) and fontTools (pip install
fonttools). Run again after writing a new symbol: tools/make-symbols-font.py
"""

import shutil
import sys
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "crates" / "sioul-app"
OUT = APP / "fonts"
SOURCE = Path("/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf")
LICENCE = Path("/usr/share/licenses/dejavu-sans-fonts/LICENSE")
SOURCES = list((APP / "qml").rglob("*.qml")) + list(ROOT.glob("crates/*/src/**/*.rs")) + list(ROOT.glob("crates/*/locales/*/*.ftl"))
# Arrows, mathematical operators, technical signs, geometric shapes, symbols, dingbats.
BLOCKS = [(0x2190, 0x21FF), (0x2200, 0x22FF), (0x2300, 0x23FF), (0x25A0, 0x25FF), (0x2600, 0x26FF), (0x2700, 0x27BF)]
FAMILY = "Sioul Symbols"

if not SOURCE.is_file():
    sys.exit(f"{SOURCE}: DejaVu Sans is needed (dnf install dejavu-sans-fonts)")
written = sorted({c for f in SOURCES for c in f.read_text(encoding="utf-8") if any(a <= ord(c) <= b for a, b in BLOCKS)})
font = TTFont(SOURCE)
drawn = font.getBestCmap()
kept = [c for c in written if ord(c) in drawn]
if missing := [c for c in written if ord(c) not in drawn]:
    print(f"DejaVu Sans does not draw {' '.join(missing)}: left to the system's fonts")

options = subset.Options()
options.name_IDs = ["*"]
options.notdef_outline = True
subsetter = subset.Subsetter(options)
subsetter.populate(unicodes=[ord(c) for c in kept])
subsetter.subset(font)
# Another name, as DejaVu's licence asks of a changed font: family, full name, PostScript name.
for record in font["name"].names:
    if record.nameID in (1, 4, 16):
        record.string = FAMILY
    elif record.nameID in (3, 6):
        record.string = FAMILY.replace(" ", "")

OUT.mkdir(exist_ok=True)
font.save(OUT / "SioulSymbols.ttf")
shutil.copyfile(LICENCE, OUT / "LICENSE-DejaVu")
qrc = [
    '<!DOCTYPE RCC><RCC version="1.0">',
    "<!-- Sioul Symbols, for the symbols the system's fonts may lack (cpp/warmup.cpp): made by tools/make-symbols-font.py. -->",
    '<qresource prefix="/sioul/fonts">',
    "    <file>SioulSymbols.ttf</file>",
    "</qresource>",
    "</RCC>",
    "",
]
(OUT / "fonts.qrc").write_text("\n".join(qrc), encoding="utf-8")
print(f"{len(kept)} symbols in {(OUT / 'SioulSymbols.ttf').relative_to(ROOT)}: {''.join(kept)}")
