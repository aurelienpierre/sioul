#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""Writes the reference of the window's QML files, as one Markdown page.

No standard tool fits: qdoc wants its own markup in the comments, and Sioul's
QML files are commented in plain words. So this reads what the files say:

- each file's header, the first block of `//` lines after the licence lines;
- what its root object declares itself, with the `//` lines just above each:
  its `required property` lines (what whoever makes it must give), its other
  properties, its signals, its functions, its inline components.

Nothing deeper is listed (a delegate, a nested object): the file is linked
under each name. The files are grouped by what they are (GROUPS and NAMED
below), and sorted by name in each group; members keep the files' order.
`Foo.qml` and `docs/….md` in the comments become links.

    tools/qml-docs.py [--out website/docs/dev/qml.md]

website/build.sh runs it at each build; the page it writes is git-ignored.
"""

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "crates" / "sioul-app"
FOLDERS = [APP / "qml", APP / "qml" / "android", APP / "qml-desktop"]
NOTES = ROOT / "docs"
OUT = ROOT / "website" / "docs" / "dev" / "qml.md"
GITHUB = "https://github.com/aurelienpierre/sioul/blob/main"
RUST_APP = "https://aurelienpierre.github.io/sioul/api/rust/sioul_app/backend/index.html"

# The page's sections, in its order: a key, a title, what they hold.
GROUPS = [
    ("window", "The window",
     "Sioul's main window and what frames every page: the places on the left, the title bar and "
     "edges Sioul draws on a computer, the system tray, the theme, the covers of a pause or of quiet time."),
    ("status", "The status line and its applets",
     "The line along the window's edge (at its top on a computer, at its bottom on a phone): what "
     "just happened, what now is, and the small switches beside it."),
    ("pages", "Pages", "One for each place of the window."),
    ("parts", "Parts of pages", "The sections, rows, cards, timelines and forms that pages are made of."),
    ("settings", "Settings", "The settings panels, their rows, and the set-up shown under some settings."),
    ("dialogs", "Dialogs and pop-ups", "Forms and questions shown over a page."),
    ("menus", "Menus", "Sioul's menus, all made from SioulMenu."),
    ("windows", "Other windows", "Windows of their own, beside the main one."),
    ("controls", "Controls", "Small pieces used across the pages: fields, buttons, a card, an icon, a player."),
    ("android", "On Android",
     "The phone's versions of two files, in `qml/android/`. Android has no Qt WebEngine and no Qt PDF: "
     "the phone's build takes these in place of the files of the same names (crates/sioul-app/build.rs)."),
]

# Files that their name or their root type does not place.
NAMED = {
    "main": "window", "SioulWindow": "window", "TitleBar": "window", "WindowEdges": "window",
    "Places": "window", "RailButton": "window", "Theme": "window", "Clock": "window",
    "PauseCover": "window", "RestCover": "window", "Tray": "window",
    "StatusLine": "status", "LineButton": "status", "NoisePlayer": "status", "HowWasIt": "status",
    "SettingsPanel": "settings", "SettingRow": "settings", "SettingsButton": "settings",
    "HealthSettings": "settings", "SharePanel": "settings", "NeedsSection": "settings",
    "ActionButton": "controls", "AddressField": "controls", "AudioPlayer": "controls",
    "Avatar": "controls", "DateField": "controls", "Icon": "controls", "LabeledRows": "controls",
    "Later": "controls", "MemoRecorder": "controls", "Panel": "controls", "PasswordField": "controls",
    "CostTiles": "controls", "ThingActions": "controls", "TimeDrag": "controls",
    "WrapCheckBox": "controls", "PlainComboBox": "controls",
}

# Qt's page for each root type the files are based on (Qt 6).
QT = "https://doc.qt.io/qt-6/"
QT_TYPES = {
    "ApplicationWindow": "qml-qtquick-controls-applicationwindow.html",
    "Button": "qml-qtquick-controls-button.html",
    "Canvas": "qml-qtquick-canvas.html",
    "CheckBox": "qml-qtquick-controls-checkbox.html",
    "ColumnLayout": "qml-qtquick-layouts-columnlayout.html",
    "ComboBox": "qml-qtquick-controls-combobox.html",
    "Dialog": "qml-qtquick-controls-dialog.html",
    "Item": "qml-qtquick-item.html",
    "ItemDelegate": "qml-qtquick-controls-itemdelegate.html",
    "Loader": "qml-qtquick-loader.html",
    "MapView": "qml-qtlocation-mapview.html",
    "Menu": "qml-qtquick-controls-menu.html",
    "Pane": "qml-qtquick-controls-pane.html",
    "PdfMultiPageView": "qml-qtquick-pdf-pdfmultipageview.html",
    "Platform.SystemTrayIcon": "qml-qt-labs-platform-systemtrayicon.html",
    "Popup": "qml-qtquick-controls-popup.html",
    "QtObject": "qml-qtqml-qtobject.html",
    "Rectangle": "qml-qtquick-rectangle.html",
    "RowLayout": "qml-qtquick-layouts-rowlayout.html",
    "TextField": "qml-qtquick-controls-textfield.html",
    "ToolButton": "qml-qtquick-controls-toolbutton.html",
}


@dataclass
class Line:
    """A line of a QML file, as the reading below sees it."""

    number: int
    depth: int  # of braces, at the line's start
    code: str  # strings emptied, comments taken out: for the structure
    raw: str  # the line as written, without its `//` comment: for showing
    comment: str | None  # what follows `//`, if anything


@dataclass
class Member:
    """Something the root object declares: a property, a signal, a function…"""

    kind: str  # "required", "property", "signal", "function", "component", "enum"
    declaration: str
    comment: list[str] = field(default_factory=list)


@dataclass
class QmlFile:
    """One QML file: its header, its root type, its members, and what went wrong reading it."""

    path: Path
    name: str
    root: str | None
    header: list[str]
    members: list[Member]
    problems: list[str]

    @property
    def group(self) -> str:
        """Its section of the page (GROUPS): by NAMED, else by its name or root type."""
        if self.path.parent.name == "android":
            return "android"
        if self.name in NAMED:
            return NAMED[self.name]
        if self.name.endswith("Applet") or self.root == "LineButton":
            return "status"
        if self.name.endswith("Page"):
            return "pages"
        if self.name.endswith("Setup"):
            return "settings"
        if self.root in ("Menu", "SioulMenu") or self.name.endswith("Menu"):
            return "menus"
        if self.root in ("Dialog", "Popup") or self.name.endswith("Dialog"):
            return "dialogs"
        if self.root == "SioulWindow":
            return "windows"
        return "parts"

    @property
    def anchor(self) -> str:
        """The id of its heading on the page."""
        prefix = "qml-android-" if self.group == "android" else "qml-"
        return prefix + self.name.lower()

    @property
    def relative(self) -> str:
        """Its path in the repository."""
        return self.path.relative_to(ROOT).as_posix()


def regex_may_start(code: list[str]) -> bool:
    """Whether a `/` here opens a regular expression rather than divides: after
    an operator, an opening bracket, a comma, or `return`, as JavaScript reads it."""
    before = "".join(code).rstrip()
    return not before or before[-1] in "(,=:[!&|?{};+-*%<>~^" or re.search(r"\b(return|typeof|case)$", before) is not None


def scan(text: str) -> list[Line]:
    """Each line with its depth of braces: strings, template strings, regular
    expressions and comments are followed, so that their braces do not count."""
    lines, depth, quote, in_block = [], 0, None, False
    for number, raw in enumerate(text.splitlines(), 1):
        start, code, comment, cut = depth, [], None, len(raw)
        i, n = 0, len(raw)
        while i < n:
            c = raw[i]
            if in_block:
                if raw.startswith("*/", i):
                    in_block = False
                    i += 1
                i += 1
                continue
            if quote:
                if c == "\\":
                    i += 2
                    continue
                if c == quote:
                    quote = None
                    code.append(c)
                i += 1
                continue
            if raw.startswith("//", i):
                comment, cut = raw[i + 2:], i
                break
            if raw.startswith("/*", i):
                in_block = True
                i += 2
                continue
            if c in "\"'`":
                quote = c
                code.append(c)
                i += 1
                continue
            if c == "/" and regex_may_start(code):
                j, in_class = i + 1, False
                while j < n:
                    if raw[j] == "\\":
                        j += 2
                        continue
                    if raw[j] == "[":
                        in_class = True
                    elif raw[j] == "]":
                        in_class = False
                    elif raw[j] == "/" and not in_class:
                        break
                    j += 1
                code.append("/re/")
                i = j + 1
                continue
            if c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
            code.append(c)
            i += 1
        # Only a template string goes on past the end of its line.
        if quote != "`":
            quote = None
        lines.append(Line(number, start, "".join(code), raw[:cut].rstrip(), comment))
    return lines


PROPERTY = re.compile(r"^\s*((?:(?:required|readonly|default|final|virtual|override)\s+)*)property\s+([\w.<>]+)\s+(\w+)\s*(?::\s*(.*))?$")
SIGNAL = re.compile(r"^\s*signal\s+(\w+)\s*(\([^)]*\))?")
FUNCTION = re.compile(r"^\s*function\s+(\w+)\s*\(")
COMPONENT = re.compile(r"^\s*component\s+(\w+)\s*:\s*([\w.]+)")
ENUM = re.compile(r"^\s*enum\s+(\w+)")
ROOT_OBJECT = re.compile(r"^\s*([A-Z][\w.]*)\s*\{")


def shown_value(value: str) -> str | None:
    """A property's value, when short and whole on its line; else None."""
    value = value.strip().rstrip(";")
    if not value or len(value) > 40:
        return None
    if any(value.count(a) != value.count(b) for a, b in ("()", "[]", "{}")):
        return None
    if value.endswith(("?", ":", "&&", "||", "+", "-", "*", "/", "=", ",")):
        return None
    return value


def declaration(lines: list[Line], index: int) -> Member | None:
    """The member declared on this line of the root object, if any."""
    line = lines[index]
    if found := PROPERTY.match(line.raw):
        modifiers = found.group(1).split()
        kind = "required" if "required" in modifiers else "property"
        text = " ".join(modifiers + ["property", found.group(2), found.group(3)])
        if found.group(4) is not None:
            value = shown_value(found.group(4))
            text += f": {value}" if value is not None else ": …"
        return Member(kind, text)
    if found := SIGNAL.match(line.code):
        signature = SIGNAL.match(line.raw)
        return Member("signal", f"signal {signature.group(1)}{signature.group(2) or ''}")
    if FUNCTION.match(line.code):
        # Its parameters may go on over a few lines.
        text = line.raw.strip()
        following = index + 1
        while ")" not in text and following < len(lines) and following <= index + 5:
            text += " " + lines[following].raw.strip()
            following += 1
        text = re.sub(r"\s*\{.*$", "", text)
        return Member("function", re.sub(r"\s+", " ", text))
    if found := COMPONENT.match(line.code):
        return Member("component", f"component {found.group(1)}: {found.group(2)}")
    if found := ENUM.match(line.code):
        return Member("enum", f"enum {found.group(1)}")
    return None


def comment_above(lines: list[Line], index: int, depth: int) -> list[str]:
    """The `//` lines just above a line, with no blank line between."""
    block = []
    above = index - 1
    while above >= 0 and lines[above].comment is not None and not lines[above].code.strip() and lines[above].depth == depth:
        block.append(lines[above].comment.strip())
        above -= 1
    return list(reversed(block))


def read(path: Path) -> QmlFile:
    """A QML file, read."""
    lines = scan(path.read_text(encoding="utf-8"))
    problems = []
    root, opened = None, None
    for index, line in enumerate(lines):
        if line.depth == 0 and (found := ROOT_OBJECT.match(line.code)):
            root, opened = found.group(1), index
            break
    if root is None:
        problems.append("no root object found")
    # The header: the first block of `//` lines before the root object, but the licence's.
    header = []
    for line in lines[:opened]:
        alone = line.comment is not None and not line.code.strip()
        licence = alone and line.comment.strip().startswith(("SPDX-License-Identifier", "Copyright"))
        if alone and not licence:
            header.append(line.comment.strip())
        elif header:
            break
    members = []
    if opened is not None:
        for index in range(opened + 1, len(lines)):
            line = lines[index]
            if line.depth < 1:
                break
            if line.depth != 1:
                continue
            member = declaration(lines, index)
            if member is None:
                continue
            member.comment = comment_above(lines, index, 1)
            if line.comment and line.code.strip():
                member.comment.append(line.comment.strip())
            members.append(member)
    if lines and lines[-1].depth + lines[-1].code.count("{") - lines[-1].code.count("}") != 0:
        problems.append("its braces do not balance as read here: some members may be missing")
    if not header:
        problems.append("no header comment")
    name = path.stem
    return QmlFile(path, name, root, header, members, problems)


def prose(text: str, index: dict[str, str]) -> str:
    """A comment's words as Markdown: code spans kept as written; outside them,
    what Markdown or HTML would read as markup escaped, and the QML files and
    design notes named turned into links."""
    parts = re.split(r"(`[^`]*`)", text)
    out = []
    for number, part in enumerate(parts):
        if number % 2:
            out.append(part)
            continue
        part = part.replace("\\", "\\\\").replace("*", "\\*").replace("[", "\\[").replace("<", "&lt;")
        # Braces would read as attributes (attr_list) right after a code span.
        part = part.replace("{", "\\{").replace("}", "\\}")

        def to_type(found: re.Match) -> str:
            folder, name = found.group(1) or "", found.group(2)
            key = ("android/" if folder.endswith("android/") else "") + name
            return f"[{found.group(0)}](#{index[key]})" if key in index else found.group(0)

        part = re.sub(r"(?<![\w/.-])(qml/android/|qml-desktop/|qml/)?([A-Za-z]\w*)\.qml\b", to_type, part)

        def to_note(found: re.Match) -> str:
            note = found.group(1)
            return f"[docs/{note}]({note})" if (NOTES / note).is_file() else found.group(0)

        part = re.sub(r"(?<![\w/.-])docs/([\w/.-]+?\.md)\b", to_note, part)
        out.append(part)
    text = "".join(out)
    # What would open a heading, a quote or a list at the start of a paragraph.
    return re.sub(r"^(#|>|[-+]\s|\d+[.)]\s)", r"\\\1", text)


def paragraphs(block: list[str]) -> list[str]:
    """Comment lines joined into paragraphs, an empty `//` line between two."""
    found, current = [], []
    for line in block:
        if line:
            current.append(line)
        elif current:
            found.append(" ".join(current))
            current = []
    if current:
        found.append(" ".join(current))
    return found


def as_code(text: str) -> str:
    """Text as a Markdown code span, whatever backticks it holds."""
    return f"`` {text} ``" if "`" in text else f"`{text}`"


SECTIONS = [
    ("required", "Required properties (whoever makes it must give them)"),
    ("property", "Properties"),
    ("signal", "Signals"),
    ("function", "Functions"),
    ("component", "Inline components (used inside this file)"),
    ("enum", "Enumerations"),
]


def page(files: list[QmlFile]) -> str:
    """The Markdown page, for all the files."""
    index = {("android/" if f.group == "android" else "") + f.name: f.anchor for f in files}
    by_group = {key: [] for key, _, _ in GROUPS}
    for qml in files:
        by_group[qml.group].append(qml)
    out = [
        "---",
        # Quoted: a colon in a value would end it, as YAML reads front matter.
        'title: "QML files"',
        'description: "Each QML file of Sioul\'s window: what it is for, what it must be given, what it offers, read from its comments."',
        "---",
        "",
        "# The window's QML files",
        "",
        "This page is written by `tools/qml-docs.py` from the files' own comments, at each build of this "
        "website: a description is corrected in its `.qml` file, never here. How the window is made: "
        "[API reference](api.md).",
        "",
        "## Reading a QML file",
        "",
        "QML is Qt's language for interfaces. A `.qml` file describes one type of object: its *root object* "
        "(`Button { … }`) and the objects inside it. The file's name is the type's name: `RailButton.qml` "
        "makes a `RailButton`, which another file uses as `RailButton { … }`. A type has everything its root "
        "object's type has: a `RailButton` is a Qt `Button`, with a few properties of its own. Each file below "
        "says what it is based on, with a link to Qt's page for Qt's types.",
        "",
        "- **`required property`**: what whoever makes the object must give it. Most of Sioul's types ask for "
        "`theme` (colours, sizes and spacing, [Theme](#qml-theme)), and many for `sioul`, the Rust object "
        f"behind the window (`Sioul`, in `crates/sioul-app/src/backend.rs`; its functions are in the "
        f"[Rust reference]({RUST_APP})), or `window` (the main window, [main](#qml-main)).",
        "- **`property`**: a value that can be set from outside, or read. `readonly property`: one that is "
        "only read, worked out from others (a *binding*, which follows them when they change). `property "
        "alias`: another object's property, reached through this one. A value is shown when it is short; "
        "`…` stands for a longer one.",
        "- **`signal`**: what the object tells whoever made it; that one answers with a handler named after "
        "it (`signal chosen` is answered by `onChosen: …`).",
        "- **`function`**: what other objects may call.",
        "- **Inline component** (`component Name: Type { … }`): a small type used inside its file only.",
        "",
        "Only what each file's root object declares is listed. What is deeper (a delegate, a nested object) "
        "is in the file, linked under each name. The files belong to the QML module "
        "`com.aurelienpierre.sioul`, built by `crates/sioul-app/build.rs`; `qml-desktop/` holds what only "
        "computers have (the system tray), `qml/android/` the phone's versions of two pages.",
        "",
    ]
    for key, title, about in GROUPS:
        members = sorted(by_group[key], key=lambda f: (f.name != "main", f.name.lower()))
        if not members:
            continue
        out += [f"## {title}", "", prose(about, index), ""]
        for qml in members:
            heading = f"{qml.name} (Android)" if qml.group == "android" else qml.name
            out += [f"### {heading} {{#{qml.anchor}}}", ""]
            line = f"[`{qml.relative}`]({GITHUB}/{qml.relative})"
            if qml.root:
                base = qml.root
                if base in QT_TYPES:
                    base = f"[`{base}`]({QT}{QT_TYPES[base]})"
                elif base in index:
                    base = f"[`{base}`](#{index[base]})"
                else:
                    base = f"`{base}`"
                line += f" · based on {base}"
            out += [line, ""]
            texts = paragraphs(qml.header)
            if not texts:
                texts = ["No description in the file yet."]
            for text in texts:
                out += [prose(text, index), ""]
            for kind, label in SECTIONS:
                chosen = [m for m in qml.members if m.kind == kind]
                if not chosen:
                    continue
                out += [f"**{label}**", ""]
                for member in chosen:
                    words = " ".join(paragraphs(member.comment))
                    out.append(f"- {as_code(member.declaration)}" + (f": {prose(words, index)}" if words else ""))
                out.append("")
    return "\n".join(out).rstrip() + "\n"


def main() -> int:
    """Reads every QML file of the window, writes the page."""
    parser = argparse.ArgumentParser(description="Writes the reference of the window's QML files, as one Markdown page.")
    parser.add_argument("--out", type=Path, default=OUT, help=f"the page to write (default: {OUT.relative_to(ROOT)})")
    arguments = parser.parse_args()
    paths = sorted(p for folder in FOLDERS for p in folder.glob("*.qml"))
    files = [read(path) for path in paths]
    for qml in files:
        for problem in qml.problems:
            print(f"qml-docs.py: {qml.relative}: {problem}.", file=sys.stderr)
    arguments.out.parent.mkdir(parents=True, exist_ok=True)
    arguments.out.write_text(page(files), encoding="utf-8")
    print(f"qml-docs.py: {len(files)} QML files written into {arguments.out}.", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
