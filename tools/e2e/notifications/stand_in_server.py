#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""A notification server standing in for the desktop's, on a private bus:
logs what Sioul asks (JSON lines) and presses its buttons, as Plasma does
(signals addressed to the connection that showed the notification).

    stand_in_server.py LOG PRESSES

LOG receives a JSON line per call; PRESSES, comma-separated, is the button
pressed three seconds after each Notify, in order ("pause,resume,stop"; an
empty one presses nothing). It quits two seconds after a notification is
closed, or after 150 seconds. run.sh starts it on the window's own bus."""
import json
import sys

import gi
gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

LOG = sys.argv[1]
PRESSES = sys.argv[2].split(",")  # pressed one after each Notify, in order
PATH = "/org/freedesktop/Notifications"
NAME = "org.freedesktop.Notifications"
XML = """<node><interface name="org.freedesktop.Notifications">
<method name="GetCapabilities"><arg type="as" direction="out"/></method>
<method name="Notify"><arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="as" direction="in"/><arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/></method>
<method name="CloseNotification"><arg type="u" direction="in"/></method>
<method name="GetServerInformation"><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/></method>
<signal name="NotificationClosed"><arg type="u"/><arg type="u"/></signal>
<signal name="ActionInvoked"><arg type="u"/><arg type="s"/></signal>
</interface></node>"""

state = {"next": 1, "notifies": 0}
loop = GLib.MainLoop()


def log(entry):
    with open(LOG, "a", encoding="utf-8") as f:
        f.write(json.dumps(entry, ensure_ascii=False) + "\n")


def press(conn, sender, nid, key):
    conn.emit_signal(sender, PATH, NAME, "ActionInvoked", GLib.Variant("(us)", (nid, key)))
    log({"pressed": key, "id": nid})
    return False


def on_call(conn, sender, path, iface, method, params, invocation):
    if method == "GetCapabilities":
        invocation.return_value(GLib.Variant("(as)", (["actions", "body", "body-markup", "persistence"],)))
    elif method == "GetServerInformation":
        invocation.return_value(GLib.Variant("(ssss)", ("stand-in", "sioul", "1", "1.2")))
    elif method == "Notify":
        app, replaces, icon, summary, body, actions, hints, timeout = params.unpack()
        nid = replaces or state["next"]
        if not replaces:
            state["next"] += 1
        state["notifies"] += 1
        log({"notify": state["notifies"], "replaces": replaces, "id": nid, "app": app, "icon": icon, "summary": summary,
             "body": body, "actions": actions, "hints": dict(hints), "timeout": timeout})
        invocation.return_value(GLib.Variant("(u)", (nid,)))
        if state["notifies"] <= len(PRESSES) and PRESSES[state["notifies"] - 1]:
            GLib.timeout_add(3000, press, conn, sender, nid, PRESSES[state["notifies"] - 1])
    elif method == "CloseNotification":
        (nid,) = params.unpack()
        log({"close": nid})
        invocation.return_value(None)
        conn.emit_signal(sender, PATH, NAME, "NotificationClosed", GLib.Variant("(uu)", (nid, 3)))
        GLib.timeout_add(2000, loop.quit)
    else:
        invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method)


def acquired(conn, name):
    node = Gio.DBusNodeInfo.new_for_xml(XML)
    conn.register_object(PATH, node.interfaces[0], on_call, None, None)


def owned(conn, name):
    log({"owned": name})


def lost(conn, name):
    log({"lost": name})
    loop.quit()


Gio.bus_own_name(Gio.BusType.SESSION, NAME, Gio.BusNameOwnerFlags.DO_NOT_QUEUE, acquired, owned, lost)
GLib.timeout_add(150000, loop.quit)
loop.run()
