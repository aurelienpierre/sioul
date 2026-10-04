# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# Sourced by the AppImage's AppRun before Sioul starts (linuxdeploy's
# apprun-hooks). Chromium's sandbox, which Qt WebEngine runs the Sites page
# in, needs unprivileged user namespaces; Ubuntu 24.04 and later refuse them
# to programs without an AppArmor profile, AppImages among them, and the
# sites would not open at all. There, and only there, the sites run without
# that sandbox, and Sioul says so on the terminal: the Flatpak keeps it.
if [ "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns 2>/dev/null)" = "1" ]; then
    export QTWEBENGINE_DISABLE_SANDBOX=1
    echo "Sioul: this system refuses Chromium's sandbox to AppImages; the sites run without it. The Flatpak keeps it." >&2
fi
