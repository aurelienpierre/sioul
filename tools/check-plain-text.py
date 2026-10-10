#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""Words from outside drawn as plain text: a check of the window's QML.

Qt guesses rich text. A Text or a Label left at Text.AutoText draws as HTML
any string whose first tag looks like one, and the controls (Button, MenuItem,
CheckBox, ItemDelegate, ToolTip…) and the titles of dialogs and menus draw
their words through such an item, with no textFormat to say otherwise. A name
or a subject holding <img src="http://…"> then loads that image from the
network, which tells its sender the item was seen, and in a button crashes the
window (Qt 6.11). So every binding that draws words (`text:`, `title:`,
`ToolTip.text:`, `placeholderText:`, a ComboBox's `model:` by its textRole, a
TextEdit's `text:` when its textFormat is not plain, `ToolTip.show(…)`, and
words given in JavaScript, `label.text = …`, to an item named by its id or by
a constant cast to its type) must show only:

- fixed words: a string, a number, a translation (`sioul.text("…")`,
  `sioul.textCounted(…)`), or `sioul.textWith(…)`/`sioul.textArgs(…)` whose
  values are themselves fixed words;
- or words made plain: through `theme.plain(…)` (a word joiner after each
  "<"), or HTML-escaped (`escaped(…)`);
- or on a Text or a Label told `textFormat: Text.PlainText`.

A binding that is rich on purpose (a message's HTML made safe in Rust) says
why on its line or the line above, in a comment holding "rich on purpose:".

The check reads the expressions (a small parser of the JavaScript the QML
uses): a ternary is safe when both its branches are, `a + b` when both are, a
comparison always, `x.map(v => …)` when what it maps to is, `x.join(…)` when
x is, and so on; any other name or call is someone's words until shown
otherwise. Custom components are followed to their root type: a Button made
with its own `contentItem` draws its words itself, and is checked inside.

    tools/check-plain-text.py [QML folder…]

Without a folder, the window's (crates/sioul-app/qml, its android/, and
qml-desktop). Prints
each binding that fails, "file:line: Type.property: expression", and exits
with 1 when there is one. tools/lint-qml.sh and tools/final-pass.sh run it.
"""
import re
import sys
from pathlib import Path

# Items whose `text` Qt guesses, with a textFormat to say plain text.
TEXTS = {"Text", "Label", "PlaceholderText"}
# Controls that draw `text` through an IconLabel: no textFormat.
CONTROLS = {"AbstractButton", "Button", "ToolButton", "RoundButton", "DelayButton", "MenuItem", "MenuBarItem", "Action",
            "CheckBox", "RadioButton", "Switch", "ItemDelegate", "CheckDelegate", "SwitchDelegate", "RadioDelegate",
            "TabButton", "ToolTip"}
# Popups and groups whose `title` a Label draws (a menu's, in the menu line that opens it).
TITLED = {"Dialog", "Menu", "GroupBox"}
# Fields: their own text is plain; their placeholder is guessed.
FIELDS = {"TextField", "TextArea"}
# Editors: plain unless told otherwise (a message's HTML, a note's).
EDITS = {"TextEdit"}
COMBOS = {"ComboBox"}
# Types known to draw nothing of `text`/`title` (windows: the system draws their titles plain).
OTHERS = {"Window", "ApplicationWindow", "Item", "Rectangle", "Popup", "Drawer", "Page", "Pane", "Frame", "Control",
          "TextInput", "SpinBox", "Slider", "Tumbler", "ScrollView", "Flickable", "ListView", "GridView",
          "Repeater", "Loader", "QtObject", "Instantiator", "Component", "Shortcut", "Timer", "Connections",
          "Binding", "MenuSeparator", "ToolBar", "TabBar", "RowLayout", "ColumnLayout", "GridLayout", "Flow", "Row",
          "Column", "Grid", "MouseArea", "TapHandler", "HoverHandler", "DragHandler", "WheelHandler", "Image",
          "IconImage", "Canvas", "Shape", "ShapePath", "SystemTrayIcon", "WebEngineView", "MediaPlayer",
          "AudioOutput", "VideoOutput", "FileDialog", "FolderDialog", "ColorDialog", "FontDialog", "MessageDialog"}

RICH_MARK = "rich on purpose:"


# ------------------------------------------------------------ the source

def code_mask(src):
    """The source with comments, strings and regular expressions blanked to
    spaces (newlines kept): what stays is the structure."""
    out = list(src)
    i, n = 0, len(src)
    last = ""  # the last character that is code, to tell a regular expression from a division

    def blank(a, b):
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            blank(i, j)
            i = j
            continue
        if src.startswith("/*", i):
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            blank(i, j)
            i = j
            continue
        if c in "\"'`":
            j = i + 1
            while j < n and src[j] != c:
                j += 2 if src[j] == "\\" else 1
            blank(i + 1, min(j, n))
            i = j + 1
            last = c
            continue
        if c == "/" and (last == "" or last in "(,=:[!&|?{};+-*%<>~^\n" or src[max(0, i - 7):i].endswith("return ")):
            j = i + 1
            klass = False
            while j < n and src[j] != "\n":
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == "[":
                    klass = True
                elif src[j] == "]":
                    klass = False
                elif src[j] == "/" and not klass:
                    break
                j += 1
            if j < n and src[j] == "/":
                blank(i + 1, j)
                i = j + 1
                last = "/"
                continue
        if not c.isspace():
            last = c
        i += 1
    return "".join(out)


def objects(src, mask):
    """Each QML object: (type, start of "{", end "}", parent index or None)."""
    found = []
    stack = []  # (index in found or None, position)
    for i, c in enumerate(mask):
        if c == "{":
            m = re.search(r"([A-Za-z_][\w.]*)\s*$", mask[max(0, i - 120):i])
            name = m.group(1) if m else ""
            before = mask[max(0, i - 120):i - (len(m.group(0)) if m else 0)].rstrip()
            is_object = bool(name) and name.split(".")[-1][:1].isupper() and not before.endswith(("function", "new", "."))
            if is_object:
                parent = next((k for k, _ in reversed(stack) if k is not None), None)
                found.append([name, i, None, parent])
                stack.append((len(found) - 1, i))
            else:
                stack.append((None, i))
        elif c == "}" and stack:
            k, _ = stack.pop()
            if k is not None:
                found[k][2] = i
    return [tuple(o) for o in found if o[2] is not None]


def own_bindings(src, mask, start, end):
    """The object's own bindings, its children's left out: name → (position
    of the value, the value's source)."""
    depth = 0
    out = {}
    i = start + 1
    line_start = True
    while i < end:
        c = mask[i]
        if c in "{([":
            depth += 1
        elif c in "})]":
            depth -= 1
        elif c == "\n":
            line_start = True
            i += 1
            continue
        if depth == 0 and line_start and not c.isspace():
            m = re.match(r"([A-Za-z_][\w.]*)\s*:(?!:)", mask[i:end])
            if m and not re.match(r"(case|default)\b", m.group(1)):
                value_at = i + m.end()
                value_end = expression_end(mask, value_at, end)
                out[m.group(1)] = (value_at, src[value_at:value_end].strip())
                # Its value is read; go on after it.
                i = value_end
                line_start = False
                continue
            line_start = False
        elif not c.isspace():
            line_start = False
        i += 1
    return out


CONTINUES_AFTER = ("+", "-", "*", "/", "%", "?", ":", "&&", "||", "??", "(", "[", "{", ",", "=", "!", "<", ">", ".")
CONTINUES_BEFORE = ("?", ":", "+", "-", "*", "/", "%", "&&", "||", "??", ".", ")", "]", "}")


def expression_end(mask, at, end):
    """Where a binding's value ends: at a line end outside brackets, unless
    the value goes on (an operator ends the line or opens the next)."""
    depth = 0
    i = at
    while i < end:
        c = mask[i]
        if c in "{([":
            depth += 1
        elif c in "})]":
            if depth == 0:
                return i
            depth -= 1
        elif c == ";" and depth == 0:
            return i
        elif c == "\n" and depth == 0:
            so_far = mask[at:i].rstrip()
            rest = mask[i + 1:end].lstrip()
            if so_far and not so_far.endswith(CONTINUES_AFTER) and not rest.startswith(CONTINUES_BEFORE):
                return i
        i += 1
    return end


# ------------------------------------------------------------ expressions

TOKEN = re.compile(r"""
    (?P<space>\s+)
  | (?P<comment>//[^\n]*|/\*.*?\*/)
  | (?P<num>(?:0[xX][0-9a-fA-F]+|\d+\.?\d*(?:[eE][+-]?\d+)?|\.\d+))
  | (?P<id>[A-Za-z_$][\w$]*)
  | (?P<str>"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')
  | (?P<tpl>`(?:\\.|[^`\\])*`)
  | (?P<punct>=>|\.\.\.|===|!==|\?\?|\?\.|==|!=|<=|>=|&&|\|\||\*\*|<<|>>|[()\[\]{}.,?:;+\-*/%!<>=&|^~])
""", re.VERBOSE | re.DOTALL)


class Unparsed(Exception):
    pass


def tokens(text):
    out = []
    i = 0
    while i < len(text):
        # A regular expression where an operand is awaited.
        if text[i] == "/" and (not out or out[-1][1] in ("(", ",", "=", ":", "[", "!", "&&", "||", "?", "{", "}", ";", "=>", "return", "+")):
            m = re.compile(r"/(?:\\.|\[(?:\\.|[^\]\\])*\]|[^/\\\n\[])+/[a-z]*").match(text, i)
            if m:
                out.append(("regex", m.group(0)))
                i = m.end()
                continue
        m = TOKEN.match(text, i)
        if not m:
            raise Unparsed(text[i:i + 20])
        kind = m.lastgroup
        if kind not in ("space", "comment"):
            out.append((kind, m.group(0)))
        i = m.end()
    return out


class Parser:
    """Expressions to small tuples: ("str", text), ("num",), ("lit", word),
    ("tpl", [parts]), ("regex", text), ("id", name), ("member", object,
    name), ("index", object, key), ("call", callee, [args]), ("cond", test,
    yes, no), ("bin", op, left, right), ("unary", op, operand), ("arrow",
    body or None), ("array", [items]), ("object", [(key, value)])."""

    BINARY = [("??",), ("||",), ("&&",), ("|",), ("^",), ("&",), ("==", "!=", "===", "!=="),
              ("<", ">", "<=", ">=", "instanceof", "in"), ("<<", ">>"), ("+", "-"), ("*", "/", "%"), ("**",)]

    def __init__(self, text):
        self.t = tokens(text)
        self.i = 0

    def peek(self, k=0):
        return self.t[self.i + k] if self.i + k < len(self.t) else (None, None)

    def take(self, value=None):
        tok = self.peek()
        if value is not None and tok[1] != value:
            raise Unparsed(f"{value} expected, {tok[1]} found")
        self.i += 1
        return tok

    def whole(self):
        node = self.expression()
        if self.peek()[0] is not None:
            raise Unparsed(f"more after: {self.peek()[1]}")
        return node

    def expression(self):
        node = self.assignment()
        while self.peek()[1] == ",":
            self.take()
            node = self.assignment()
        return node

    def assignment(self):
        # An arrow function: "x => …", "(a, b) => …".
        if self.peek()[0] == "id" and self.peek(1)[1] == "=>":
            self.take()
            self.take()
            return self.arrow_body()
        if self.peek()[1] == "(":
            depth, k = 0, 0
            while True:
                tok = self.peek(k)
                if tok[0] is None:
                    break
                if tok[1] == "(":
                    depth += 1
                elif tok[1] == ")":
                    depth -= 1
                    if depth == 0:
                        break
                k += 1
            if self.peek(k + 1)[1] == "=>":
                self.i += k + 2
                return self.arrow_body()
        return self.conditional()

    def arrow_body(self):
        if self.peek()[1] == "{":
            self.skip_block()
            return ("arrow", None)
        return ("arrow", self.assignment())

    def skip_block(self):
        depth = 0
        while True:
            tok = self.take()
            if tok[0] is None:
                raise Unparsed("block not closed")
            if tok[1] in ("{", "(", "["):
                depth += 1
            elif tok[1] in ("}", ")", "]"):
                depth -= 1
                if depth == 0:
                    return

    def conditional(self):
        test = self.binary(0)
        if self.peek()[1] == "?":
            self.take()
            yes = self.assignment()
            self.take(":")
            no = self.assignment()
            return ("cond", test, yes, no)
        return test

    def binary(self, level):
        if level == len(self.BINARY):
            return self.unary()
        node = self.binary(level + 1)
        while self.peek()[1] in self.BINARY[level]:
            op = self.take()[1]
            node = ("bin", op, node, self.binary(level + 1))
        return node

    def unary(self):
        if self.peek()[1] in ("!", "-", "+", "~", "typeof", "void", "delete"):
            op = self.take()[1]
            return ("unary", op, self.unary())
        return self.postfix()

    def postfix(self):
        node = self.primary()
        while True:
            tok = self.peek()
            if tok[1] in (".", "?."):
                self.take()
                name = self.take()
                if name[0] != "id":
                    raise Unparsed("a name after '.'")
                node = ("member", node, name[1])
            elif tok[1] == "[":
                self.take()
                key = self.expression()
                self.take("]")
                node = ("index", node, key)
            elif tok[1] == "(":
                self.take()
                args = []
                while self.peek()[1] != ")":
                    if self.peek()[1] == "...":
                        self.take()
                    args.append(self.assignment())
                    if self.peek()[1] == ",":
                        self.take()
                self.take(")")
                node = ("call", node, args)
            else:
                return node

    def primary(self):
        kind, value = self.take()
        if kind == "str":
            return ("str", value)
        if kind == "num":
            return ("num",)
        if kind == "regex":
            return ("regex", value)
        if kind == "tpl":
            return ("tpl", [Parser(part).whole() for part in re.findall(r"\$\{(.*?)\}", value, re.DOTALL)])
        if kind == "id":
            if value in ("true", "false", "null", "undefined", "NaN", "Infinity"):
                return ("lit", value)
            if value == "new":
                callee = self.postfix()
                return ("new", callee)
            if value == "function":
                while self.peek()[1] != "{":
                    self.take()
                self.skip_block()
                return ("arrow", None)
            return ("id", value)
        if value == "(":
            node = self.expression()
            self.take(")")
            return node
        if value == "[":
            items = []
            while self.peek()[1] != "]":
                if self.peek()[1] == "...":
                    self.take()
                items.append(self.assignment())
                if self.peek()[1] == ",":
                    self.take()
            self.take("]")
            return ("array", items)
        if value == "{":
            pairs = []
            while self.peek()[1] != "}":
                key = self.take()
                if self.peek()[1] == ":":
                    self.take()
                    pairs.append((key[1], self.assignment()))
                else:
                    pairs.append((key[1], ("id", key[1])))
                if self.peek()[1] == ",":
                    self.take()
            self.take("}")
            return ("object", pairs)
        raise Unparsed(f"unexpected {value}")


def path(node):
    """A callee's dotted name ("page.sioul.text"), or None."""
    if node[0] == "id":
        return [node[1]]
    if node[0] == "member":
        head = path(node[1])
        return None if head is None else head + [node[2]]
    return None


# Methods whose result is a number or a date's words, whatever they are called on.
NUMBERS = {"toFixed", "toPrecision", "toLocaleDateString", "toLocaleTimeString", "getFullYear",
           "getMonth", "getDate", "getDay", "getHours", "getMinutes", "getSeconds", "indexOf", "lastIndexOf",
           "findIndex", "some", "every", "includes", "test", "startsWith", "endsWith", "localeCompare"}
# Methods whose result is made of what they are called on.
SAME = {"trim", "trimStart", "trimEnd", "slice", "substring", "substr", "toUpperCase", "toLowerCase", "charAt",
        "at", "normalize", "split", "filter", "sort", "reverse", "find", "pop", "shift", "flat", "toString"}


def plain_replace(args):
    """`.replace(/</g, "<\\u2060")`: Theme.qml's `plain`, written out (the
    word joiner escaped, or as it is)."""
    return (len(args) == 2 and args[0][0] == "regex" and args[0][1].startswith("/<")
            and args[1][0] == "str" and args[1][1][1:].startswith(("<\\u2060", "<\u2060")))


def safe(node):
    """Whether everything `node` can show is fixed words, a number, or made plain."""
    kind = node[0]
    if kind in ("str", "num", "lit", "regex"):
        return True
    if kind == "tpl":
        return all(safe(part) for part in node[1])
    if kind == "id":
        return node[1] in ("index",)
    if kind == "member":
        return node[2] in ("length", "index")
    if kind == "index":
        return safe(node[1])
    if kind == "cond":
        return safe(node[2]) and safe(node[3])
    if kind == "bin":
        op = node[1]
        if op == "&&":
            return safe(node[3])
        if op in ("||", "??", "+"):
            return safe(node[2]) and safe(node[3])
        return True
    if kind == "unary":
        return True
    if kind == "array":
        return all(safe(item) for item in node[1])
    if kind == "arrow":
        return node[1] is not None and safe(node[1])
    if kind == "call":
        return safe_call(node[1], node[2])
    return False


def safe_call(callee, args):
    name = path(callee)
    if name:
        last = name[-1]
        if last in ("plain", "escaped"):
            return True
        if name[-2:] in (["sioul", "text"], ["sioul", "textCounted"]) or name in (["qsTr"], ["qsTrId"]):
            return True
        if name[-2:] == ["sioul", "textWith"]:
            return len(args) < 3 or safe(args[2])
        if name[-2:] == ["sioul", "textArgs"]:
            if len(args) < 2:
                return True
            given = args[1]
            if given[0] == "call" and path(given[1]) == ["JSON", "stringify"] and given[2] and given[2][0][0] == "object":
                return all(safe(value) for _, value in given[2][0][1])
            return False
        if name == ["String"]:
            return not args or safe(args[0])
        if name in (["Number"], ["parseInt"], ["parseFloat"], ["Boolean"]) or name[0] == "Math":
            return True
        if name[:1] == ["Qt"] and last in ("formatDate", "formatTime", "formatDateTime"):
            return True
    if callee[0] != "member":
        return False
    receiver, method = callee[1], callee[2]
    if method in NUMBERS:
        return True
    if method == "replace":
        if plain_replace(args):
            return True
        return safe(receiver) and len(args) == 2 and safe(args[1])
    if method in ("padStart", "padEnd", "join", "concat", "repeat"):
        return safe(receiver) and all(safe(a) for a in args)
    if method in SAME:
        return safe(receiver)
    if method == "map":
        return bool(args) and args[0][0] == "arrow" and safe(args[0])
    return False


def safe_model(node, role):
    """Whether a ComboBox's model shows only safe words: its items, or the
    `role` of each (its textRole) when they are objects."""
    kind = node[0]
    if kind == "cond":
        return safe_model(node[2], role) and safe_model(node[3], role)
    if kind == "bin" and node[1] in ("||", "??"):
        return safe_model(node[2], role) and safe_model(node[3], role)
    if kind == "bin" and node[1] == "&&":
        return safe_model(node[3], role)
    if kind == "array":
        return all(safe_item(item, role) for item in node[1])
    if kind == "call" and node[1][0] == "member":
        receiver, method, args = node[1][1], node[1][2], node[2]
        if method == "map":
            return bool(args) and args[0][0] == "arrow" and args[0][1] is not None and safe_item(args[0][1], role)
        if method == "concat":
            return safe_model(receiver, role) and all(safe_model(a, role) for a in args)
        if method in ("filter", "slice", "sort", "reverse"):
            return safe_model(receiver, role)
    return role is None and safe(node)


def safe_item(node, role):
    """One item of a model: safe words, or an object whose `role` is."""
    if role is None:
        return safe(node)
    if node[0] == "cond":
        return safe_item(node[2], role) and safe_item(node[3], role)
    if node[0] == "object":
        value = dict(node[1]).get(role)
        return value is None or safe(value)
    # Object.assign({}, item, { label: … }): the last object says the label.
    if node[0] == "call" and path(node[1]) == ["Object", "assign"] and node[2] and node[2][-1][0] == "object":
        value = dict(node[2][-1][1]).get(role)
        return value is not None and safe(value)
    return False


# ------------------------------------------------------------ components

def root_object(src, mask):
    """The root object of a component file: (type, start, end)."""
    for name, start, end, parent in objects(src, mask):
        if parent is None:
            return name, start, end
    return None


class Types:
    """What each type draws: built-in, or a component of these folders, followed to its root."""

    def __init__(self, folders):
        self.files = {}
        for folder in folders:
            for path_ in sorted(Path(folder).glob("*.qml")):
                self.files.setdefault(path_.stem, path_)
        self.cache = {}

    def kind(self, name):
        """("text"|"control"|"titled"|"field"|"combo"|"other"|"unknown", own contentItem, own header, plain root)."""
        short = name.split(".")[-1]
        if short in self.cache:
            return self.cache[short]
        self.cache[short] = ("unknown", False, False, False)
        if short in TEXTS:
            kind = ("text", False, False, False)
        elif short in CONTROLS:
            kind = ("control", False, False, False)
        elif short in TITLED:
            kind = ("titled", False, False, False)
        elif short in FIELDS:
            kind = ("field", False, False, False)
        elif short in EDITS:
            kind = ("edit", False, False, False)
        elif short in COMBOS:
            kind = ("combo", False, False, False)
        elif short in OTHERS:
            kind = ("other", False, False, False)
        elif short in self.files:
            src = self.files[short].read_text()
            mask = code_mask(src)
            root = root_object(src, mask)
            if root is None:
                kind = ("other", False, False, False)
            else:
                base = self.kind(root[0])
                bindings = own_bindings(src, mask, root[1], root[2])
                plain_root = is_plain(bindings) or base[3]
                kind = (base[0], base[1] or "contentItem" in bindings, base[2] or "header" in bindings, plain_root)
        else:
            kind = ("other", False, False, False)
        self.cache[short] = kind
        return kind


def is_plain(bindings):
    value = bindings.get("textFormat", (0, ""))[1]
    return re.fullmatch(r"(Text|Label|TextEdit)\.PlainText", value) is not None


# ------------------------------------------------------------ the check

def explained(src, at):
    """Whether the binding at `at` says why it is rich on purpose."""
    line_start = src.rfind("\n", 0, at) + 1
    line_end = src.find("\n", at)
    above_start = src.rfind("\n", 0, max(0, line_start - 1)) + 1
    return RICH_MARK in src[line_start:line_end if line_end >= 0 else len(src)] or RICH_MARK in src[above_start:line_start]


def verdict(value, role=None, model=False):
    """None when safe, else what is wrong."""
    try:
        node = Parser(value).whole()
    except (Unparsed, IndexError, TypeError) as e:
        return f"not read ({e})"
    ok = safe_model(node, role) if model else safe(node)
    return None if ok else "outside words"


def check_file(path_, types, root):
    src = path_.read_text()
    mask = code_mask(src)
    problems = []

    def report(at, what, value, why):
        line = src.count("\n", 0, at) + 1
        shown = " ".join(value.split())
        problems.append(f"{path_.relative_to(root)}:{line}: {what}: {shown[:150]} ({why})")

    # What each id names draws: for the words given to it in JavaScript.
    named = {}
    for name, start, end, _ in objects(src, mask):
        kind, own_content, own_header, plain_root = types.kind(name)
        bindings = own_bindings(src, mask, start, end)
        short = name.split(".")[-1]
        plain = is_plain(bindings) or plain_root
        sinks = []
        if kind == "text":
            if not plain:
                sinks.append("text")
        elif kind == "control" and not own_content and "contentItem" not in bindings:
            sinks.append("text")
        elif kind == "titled" and not own_header and "header" not in bindings:
            sinks.append("title")
        elif kind in ("field", "edit"):
            if kind == "field":
                sinks.append("placeholderText")
            # TextEdit and TextArea are plain text unless their textFormat says otherwise.
            if "textFormat" in bindings and not plain:
                sinks.append("text")
        elif kind == "combo" and "delegate" not in bindings:
            sinks.append("model")
        if "id" in bindings:
            named[bindings["id"][1]] = (short, [sink for sink in sinks if sink != "model"])
        sinks.append("ToolTip.text")
        for sink in sinks:
            if sink not in bindings:
                continue
            at, value = bindings[sink]
            # A rich text item: its words must be escaped, or said rich on purpose.
            if explained(src, at):
                continue
            role = None
            if sink == "model" and "textRole" in bindings:
                role = bindings["textRole"][1].strip("\"'") or None
            why = verdict(value, role, sink == "model")
            if why:
                report(at, f"{short}.{sink}", value, why)
    # Words given in JavaScript: "label.text = …", to an item named by its id
    # or by a constant cast to its type ("const tip = loader.item as ToolTip").
    for m in re.finditer(r"\bconst\s+(\w+)\s*=[^\n;]*\bas\s+([A-Z]\w*)", mask):
        kind = types.kind(m.group(2))[0]
        named.setdefault(m.group(1), (m.group(2), {"text": ["text"], "control": ["text"], "titled": ["title"]}.get(kind, [])))
    for m in re.finditer(r"\b(\w+)\.(text|title|placeholderText)\s*=(?![=>])", mask):
        short, sinks = named.get(m.group(1), (None, []))
        if m.group(2) not in sinks:
            continue
        value_end = expression_end(mask, m.end(), len(mask))
        value = src[m.end():value_end].strip()
        why = verdict(value)
        if why and not explained(src, m.start()):
            report(m.start(), f"{short}.{m.group(2)} =", value, why)
    # ToolTip.show(words, …) anywhere.
    for m in re.finditer(r"\bToolTip\.show\(", mask):
        at = m.end()
        depth, i = 1, at
        while i < len(mask) and depth:
            depth += {"(": 1, ")": -1}.get(mask[i], 0)
            i += 1
        call = src[m.start() + len("ToolTip.show"):i]
        try:
            node = Parser("f" + call).whole()
            words = node[2][0] if node[0] == "call" and node[2] else ("str", "")
            why = None if safe(words) else "outside words"
        except (Unparsed, IndexError) as e:
            why = f"not read ({e})"
        if why and not explained(src, m.start()):
            report(m.start(), "ToolTip.show", call, why)
    return problems


def main(argv):
    repo = Path(__file__).resolve().parent.parent
    folders = [Path(a) for a in argv] or [repo / "crates/sioul-app/qml", repo / "crates/sioul-app/qml-desktop", repo / "crates/sioul-app/qml/android"]
    types = Types(folders)
    problems = []
    for folder in folders:
        for path_ in sorted(folder.glob("*.qml")):
            problems += check_file(path_, types, repo if path_.is_relative_to(repo) else folder)
    for line in problems:
        print(line)
    print(f"check-plain-text: {len(problems)} binding{'s' if len(problems) != 1 else ''} of outside words drawn as rich text", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
