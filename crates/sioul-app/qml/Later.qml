// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A form made the first time it is asked for, not with its page: a page
// holding a dozen forms would build them all as it opens (on a phone, most
// of a second). `now()` gives it, made; `close()` closes it if it was.

import QtQuick

Loader {
    // Out of the layouts it sits in: a form opens over the window.
    visible: false
    active: false

    // The form, made the first time it is asked for.
    function now() {
        active = true
        return item
    }

    // The form closed, if it was ever made.
    function close() {
        if (item)
            item.close()
    }
}
