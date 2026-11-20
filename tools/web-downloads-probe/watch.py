#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""Logs every file made, closed, renamed or removed under the folders given
(new folders included), with the time, for tools/web-downloads-probe/run.sh:
inotify queues each event, so a file that lives a microsecond is still
seen. Linux only. Usage: watch.py LOG FOLDER..."""
import ctypes, ctypes.util, os, struct, sys, time
libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
IN_ACCESS=0x1; IN_MODIFY=0x2; IN_CLOSE_WRITE=0x8; IN_OPEN=0x20; IN_MOVED_FROM=0x40; IN_MOVED_TO=0x80; IN_CREATE=0x100; IN_DELETE=0x200; IN_ISDIR=0x40000000
MASK = IN_CREATE | IN_MOVED_FROM | IN_MOVED_TO | IN_DELETE | IN_CLOSE_WRITE
names = {IN_CREATE: "create", IN_MOVED_FROM: "moved-from", IN_MOVED_TO: "moved-to", IN_DELETE: "delete", IN_CLOSE_WRITE: "close-write"}
out = open(sys.argv[1], "a", buffering=1)
fd = libc.inotify_init()
wd = {}
def add(path):
    w = libc.inotify_add_watch(fd, path.encode(), MASK)
    if w >= 0:
        wd[w] = path
        for entry in os.scandir(path):
            if entry.is_dir(follow_symlinks=False):
                add(entry.path)
for root in sys.argv[2:]:
    add(root)
out.write("watch: ready, %d folders\n" % len(wd))
t0 = time.time()
while True:
    data = os.read(fd, 65536)
    i = 0
    while i < len(data):
        w, mask, cookie, length = struct.unpack_from("iIII", data, i)
        name = data[i + 16:i + 16 + length].rstrip(b"\0").decode(errors="replace")
        i += 16 + length
        path = os.path.join(wd.get(w, "?"), name)
        kind = "+".join(v for k, v in names.items() if mask & k) or hex(mask)
        if mask & IN_ISDIR:
            kind += " (folder)"
            if mask & IN_CREATE:
                try: add(path)
                except OSError: pass
        size = ""
        try: size = " %d bytes" % os.stat(path).st_size
        except OSError: pass
        out.write("watch %.6f %s %s%s\n" % (time.time(), kind, path, size))
