#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""The weather's icons for the phone's card on the home screen, as Android's
own vector drawings, from the Breeze icons the window bundles
(crates/sioul-app/icons/sioul/48/weather-*-symbolic.svg, LGPL-3.0-or-later,
crates/sioul-app/icons/COPYING-ICONS).

Writes android/package/res/drawable/sioul_weather_*.xml, one per icon that
crates/sioul-core/src/weather.rs (`describe`) names, drawn in white: the
card's layout tints them in its text colour, light or dark as the phone is
(HomeCardToday.java). Run again after changing the icons; the results are
committed.

Only what those icons use is read: paths, circles and rectangles, their
groups' fill and stroke, and `<use>` of an element moved or turned
(translate, rotate).
"""

import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ICONS = ROOT / "crates" / "sioul-app" / "icons" / "sioul" / "48"
OUT = ROOT / "android" / "package" / "res" / "drawable"
WEATHER = ROOT / "crates" / "sioul-core" / "src" / "weather.rs"
SVG = "{http://www.w3.org/2000/svg}"
XLINK = "{http://www.w3.org/1999/xlink}href"
INHERITED = ("fill", "stroke", "stroke-width", "stroke-linejoin", "stroke-linecap", "fill-rule")
WHITE = "#ffffff"


def names():
    """The icons weather.rs names, in its order."""
    found = re.findall(r'"(weather-[a-z-]+-symbolic)"', WEATHER.read_text(encoding="utf-8"))
    return list(dict.fromkeys(found))


def number(text):
    return float(text) if text not in (None, "") else 0.0


def fmt(value):
    return f"{value:g}"


def shape(el):
    """An element's outline as path data; None for what draws nothing."""
    tag = el.tag.replace(SVG, "")
    if tag == "path":
        return el.get("d", "").strip()
    if tag == "circle":
        cx, cy, r = number(el.get("cx")), number(el.get("cy")), number(el.get("r"))
        return f"M{fmt(cx - r)},{fmt(cy)}a{fmt(r)},{fmt(r)} 0 1,0 {fmt(2 * r)},0a{fmt(r)},{fmt(r)} 0 1,0 {fmt(-2 * r)},0z"
    if tag == "rect":
        x, y, w, h = (number(el.get(k)) for k in ("x", "y", "width", "height"))
        rx = number(el.get("rx") or el.get("ry"))
        ry = number(el.get("ry") or el.get("rx"))
        if rx == 0 and ry == 0:
            return f"M{fmt(x)},{fmt(y)}h{fmt(w)}v{fmt(h)}h{fmt(-w)}z"
        rx, ry = min(rx, w / 2), min(ry, h / 2)
        return (
            f"M{fmt(x + rx)},{fmt(y)}h{fmt(w - 2 * rx)}a{fmt(rx)},{fmt(ry)} 0 0,1 {fmt(rx)},{fmt(ry)}"
            f"v{fmt(h - 2 * ry)}a{fmt(rx)},{fmt(ry)} 0 0,1 {fmt(-rx)},{fmt(ry)}"
            f"h{fmt(-(w - 2 * rx))}a{fmt(rx)},{fmt(ry)} 0 0,1 {fmt(-rx)},{fmt(-ry)}"
            f"v{fmt(-(h - 2 * ry))}a{fmt(rx)},{fmt(ry)} 0 0,1 {fmt(rx)},{fmt(-ry)}z"
        )
    return None


def transform(text):
    """A `<use>`'s move as a group's attributes: translate(x[, y]) or rotate(a[, cx, cy])."""
    if not text:
        return {}
    match = re.fullmatch(r"\s*(translate|rotate)\(([^)]*)\)\s*", text)
    if not match:
        sys.exit(f"make-weather-drawables: a transform not read: {text}")
    values = [float(v) for v in re.split(r"[\s,]+", match.group(2).strip()) if v]
    if match.group(1) == "translate":
        return {"translateX": fmt(values[0]), "translateY": fmt(values[1] if len(values) > 1 else 0)}
    out = {"rotation": fmt(values[0])}
    if len(values) == 3:
        out.update(pivotX=fmt(values[1]), pivotY=fmt(values[2]))
    return out


def style(el, inherited):
    """An element's fill and stroke, its group's where it says none."""
    out = dict(inherited)
    for key in INHERITED:
        if el.get(key) is not None:
            out[key] = el.get(key)
    return out


def drawn(el, inherited, ids, depth=0):
    """The drawable's lines for an element and what it holds."""
    tag = el.tag.replace(SVG, "")
    pad = "    " * (depth + 1)
    if tag in ("style", "title", "desc", "defs", "metadata"):
        return []
    here = style(el, inherited)
    if tag in ("g", "svg"):
        return [line for child in el for line in drawn(child, here, ids, depth)]
    if tag == "use":
        target = ids.get((el.get("href") or el.get(XLINK) or "").lstrip("#"))
        if target is None:
            sys.exit(f"make-weather-drawables: a <use> of nothing: {el.attrib}")
        moved = transform(el.get("transform"))
        inner = drawn(target, here, ids, depth + 1)
        attributes = "".join(f' android:{k}="{v}"' for k, v in moved.items())
        return [f"{pad}<group{attributes}>", *inner, f"{pad}</group>"]
    data = shape(el)
    if not data:
        return []
    fill = here.get("fill", "#000")
    stroke = here.get("stroke", "none")
    attributes = [f'android:pathData="{data}"']
    if fill != "none":
        attributes.append(f'android:fillColor="{WHITE}"')
        if here.get("fill-rule") == "evenodd":
            attributes.append('android:fillType="evenOdd"')
    if stroke != "none":
        attributes.append(f'android:strokeColor="{WHITE}"')
        attributes.append(f'android:strokeWidth="{fmt(number(here.get("stroke-width", "1")))}"')
        if here.get("stroke-linejoin") in ("round", "bevel", "miter"):
            attributes.append(f'android:strokeLineJoin="{here["stroke-linejoin"]}"')
        if here.get("stroke-linecap") in ("round", "square", "butt"):
            attributes.append(f'android:strokeLineCap="{here["stroke-linecap"]}"')
    if fill == "none" and stroke == "none":
        return []
    return [f"{pad}<path " + f"\n{pad}    ".join(attributes) + " />"]


def convert(name):
    path = ICONS / f"{name}.svg"
    root = ET.parse(path).getroot()
    ids = {el.get("id"): el for el in root.iter() if el.get("id")}
    view = (root.get("viewBox") or f"0 0 {root.get('width', '48')} {root.get('height', '48')}").split()
    width, height = view[2], view[3]
    lines = drawn(root, {}, ids)
    android_name = "sioul_" + name.removesuffix("-symbolic").replace("-", "_")
    text = "\n".join(
        [
            '<?xml version="1.0" encoding="utf-8"?>',
            "<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->",
            f"<!-- Breeze's {name} (crates/sioul-app/icons/COPYING-ICONS), made by",
            "     tools/make-weather-drawables.py for the card on the home screen: white,",
            "     tinted by the card's layout in its text colour. -->",
            '<vector xmlns:android="http://schemas.android.com/apk/res/android"',
            '    android:width="24dp"',
            '    android:height="24dp"',
            f'    android:viewportWidth="{width}"',
            f'    android:viewportHeight="{height}">',
            *lines,
            "</vector>",
            "",
        ]
    )
    (OUT / f"{android_name}.xml").write_text(text, encoding="utf-8")
    return android_name


def main():
    made = [convert(name) for name in names()]
    print(f"make-weather-drawables: {len(made)} drawings in {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
