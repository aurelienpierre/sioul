// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The call that fills a login (crates/sioul-app/src/sites.rs, `fill_call`),
// as Rust builds it for every device, run in Qt's JavaScript engine with
// stand-ins for a page: its `location` and its `document` with a form's
// fields. The calls are Rust's own, written into fillcalls.js by a Rust test
// that fails while that file differs from what Rust builds now. A page at
// another origin than the one the login was asked for gets nothing: the call
// returns false, writes into no field and asks the document nothing (review
// of 22 November 2026, finding 1). The page asked is filled as typing
// would; a page asking for the one-time code alone gets the code, a page
// with a password field does not. Invented logins only. Run by
// tools/qml-test.sh.

import QtQuick
import QtTest
import "fillcalls.js" as Calls

TestCase {
    name: "FillCall"

    // One of Rust's calls, by its name in fillcalls.js.
    function made(name) {
        const found = Calls.cases.filter(c => c.name === name)
        verify(found.length === 1, name)
        return found[0]
    }

    // A page at `origin` whose form holds `kinds` ({type, name, id,
    // autocomplete}), in that order: its fields, and how many times the
    // document was asked for them.
    function page(origin, kinds) {
        class Field {
            constructor(at, kind) {
                this.at = at
                this.type = kind.type || ""
                this.name = kind.name || ""
                this.id = kind.id || ""
                this.autocomplete = kind.autocomplete || ""
                this.written = undefined
                this.events = []
            }
            // Set through the prototype's own setter, as the script sets a browser's input.
            get value() {
                return this.written === undefined ? "" : this.written
            }
            set value(text) {
                this.written = text
            }
            dispatchEvent(event) {
                this.events.push(event.type)
            }
            // DOCUMENT_POSITION_FOLLOWING (4) when `other` comes after this field.
            compareDocumentPosition(other) {
                return other.at > this.at ? 4 : 2
            }
            getAttribute(name) {
                return null
            }
        }
        const fields = kinds.map((kind, at) => new Field(at, kind))
        // The selectors the script asks, each as a browser reads it; any other is a script changed.
        const selectors = {
            "input[type=password]": f => f.type === "password",
            "input[type=email], input[type=text], input:not([type])": f => f.type === "email" || f.type === "text" || f.type === "",
            "input[autocomplete=\"one-time-code\"]": f => f.autocomplete === "one-time-code",
            "input:not([type=password]):not([type=hidden])": f => f.type !== "password" && f.type !== "hidden"
        }
        const made = { fields: fields, asked: 0, location: { origin: origin } }
        const matching = selector => {
            made.asked += 1
            if (!(selector in selectors))
                throw new Error("a selector this stand-in does not know: " + selector)
            return fields.filter(selectors[selector])
        }
        made.document = {
            querySelector: selector => matching(selector)[0] || null,
            querySelectorAll: selector => matching(selector)
        }
        return made
    }

    // The call run in the page, as a web view runs it: its result.
    function run(call, at) {
        class Event {
            constructor(type, init) {
                this.type = type
                this.bubbles = !!(init && init.bubbles)
            }
        }
        const node = { DOCUMENT_POSITION_FOLLOWING: 4 }
        return new Function("location", "document", "Node", "Event", "return " + call)(at.location, at.document, node, Event)
    }

    // A sign-in form: a name, then a password.
    function signIn(origin) {
        return page(origin, [{ type: "email", name: "login" }, { type: "password", name: "password" }])
    }

    function test_the_page_asked_is_filled() {
        const right = made("right")
        // The address asked was https://jane@Bank.Example.org:443/login?next=%2F: its origin, as a browser says it.
        const at = signIn("https://bank.example.org")
        compare(run(right.call, at), true)
        compare(at.fields[0].value, right.user)
        compare(at.fields[1].value, right.password, "the password whole, quotes, backslash and line breaks in it")
        verify(right.password.indexOf("</script>") > 0 && right.password.indexOf("\n") > 0, "a password made to break a script")
        compare(at.fields[0].events, ["input", "change"], "as typing would")
        compare(at.fields[1].events, ["input", "change"])
    }

    function test_another_origin_gets_nothing() {
        const right = made("right")
        for (const elsewhere of ["https://evil.example.net", "https://bank.example.org:8443", "http://bank.example.org", "https://bank.example.org.evil.example.net", "https://login.bank.example.org", "null", ""]) {
            const at = signIn(elsewhere)
            compare(run(right.call, at), false, elsewhere)
            verify(at.fields.every(f => f.written === undefined && f.events.length === 0), elsewhere + ": no field written")
            compare(at.asked, 0, elsewhere + ": the document asked nothing")
        }
    }

    function test_a_port_and_an_ipv6_host_are_their_own_origins() {
        const port = made("port")
        compare(run(port.call, signIn("https://bank.example.org")), false, "the same host on 443")
        const there = signIn("https://bank.example.org:8443")
        compare(run(port.call, there), true)
        compare(there.fields[1].value, "pass-port")
        const six = made("ipv6")
        compare(run(six.call, signIn("https://[::1]")), false)
        compare(run(six.call, signIn("https://[::2]:8443")), false)
        const home = signIn("https://[::1]:8443")
        compare(run(six.call, home), true)
        compare(home.fields[1].value, "pass-six")
    }

    function test_the_code_goes_to_a_page_that_asks_for_it_alone() {
        const code = made("code")
        const second = page("https://bank.example.org", [{ type: "text", name: "otp" }])
        run(code.call, second)
        compare(second.fields[0].value, "654321", "a page asking for the code alone")
        const first = page("https://bank.example.org", [{ type: "email", name: "login" }, { type: "password" }, { type: "text", name: "otp", autocomplete: "one-time-code" }])
        run(code.call, first)
        compare(first.fields[1].value, "pass-code")
        compare(first.fields[2].written, undefined, "a password's page: no code")
        // The right call's code neither, on its password page.
        const right = signIn("https://bank.example.org")
        run(made("right").call, right)
        verify(right.fields.every(f => f.value !== "123456"))
    }
}
