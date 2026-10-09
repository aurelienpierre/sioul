// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The date picker (DateField.qml): its month arrows have a name a screen
// reader says and a tip shows, "Previous month: September 2026", in
// Sioul's words; the month alone where no words are given. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 400
    height: 400

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        readonly property var words: ({
            "date-month-previous": "Previous month: {month}",
            "date-month-next": "Next month: {month}"
        })

        function text(key) {
            return mock.words[key] || key
        }
        function textWith(key, name, value) {
            return mock.text(key).replace("{" + name + "}", value)
        }
    }

    DateField {
        id: named

        theme: testTheme
        sioul: mock
        locale: Qt.locale("en_GB")
        shownYear: 2026
        shownMonth: 9
    }

    DateField {
        id: bare

        theme: testTheme
        locale: Qt.locale("en_GB")
        shownYear: 2026
        shownMonth: 0
    }

    TestCase {
        name: "DateField"
        when: windowShown

        function test_the_arrows_are_named() {
            compare(named.arrowName(-1), "Previous month: September 2026")
            compare(named.arrowName(1), "Next month: November 2026")
        }

        function test_across_a_year() {
            compare(bare.arrowName(-1), "December 2025", "the month alone, without words")
            compare(bare.arrowName(1), "February 2026")
        }
    }
}
