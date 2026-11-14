// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * The lock screen's version of Sioul's notices, on the JVM
 * (android/jvm-checks/run.sh): every notice Notices posts is private, with a
 * public version that says only what Rust gives for it (new mail's count, a
 * code's title, never the code) or that Sioul has a reminder (composed tasks
 * §7.3: the leaving reminder names neither the appointment nor what to bring
 * there), in both languages when Rust has not given its words yet. Read from
 * the sources (the repository's root comes as the property sioul.root):
 * android.jar's Notification is a stub that cannot be made here. A code's
 * notice gives its title alone (eventalarms.rs, `code_note`). The doses'
 * alarms (DoseAlarms.java) are not Notices'.
 */
public final class EventNotesCheck
{
    static int failed = 0;
    static int checked = 0;

    static void expect(String what, boolean holds)
    {
        checked++;
        if (!holds) {
            failed++;
            System.out.println("FAIL " + what);
        }
    }

    /** The body of the method that starts with `head`, up to its closing brace at four spaces. */
    static String body(String text, String head)
    {
        int at = text.indexOf(head);
        int end = at < 0 ? -1 : text.indexOf("\n    }\n", at);
        return at < 0 || end < 0 ? "" : text.substring(at, end);
    }

    public static void main(String[] args) throws IOException
    {
        Path sources = Paths.get(System.getProperty("sioul.root"), "android", "package", "src", "com", "aurelienpierre", "sioul");
        String text = new String(Files.readAllBytes(sources.resolve("Notices.java")), StandardCharsets.UTF_8);
        String posted = body(text, "    static void post(Context context, Said said)");
        expect("Notices.post is there", !posted.isEmpty());
        expect("a notice is private", posted.contains(".setVisibility(Notification.VISIBILITY_PRIVATE)"));
        expect("with its public version", posted.contains(".setPublicVersion(publicVersion(context, channel, said))"));
        String version = body(text, "    private static Notification publicVersion(");
        expect("the public version says Rust's words for it, else Sioul's alone", version.contains("given ? said.publicTitle : \"Sioul\"") && version.contains("given ? said.publicBody : EventAlarms.word(context, \"public\")") && !version.contains("said.title") && !version.contains("said.body"));
        String alarms = new String(Files.readAllBytes(sources.resolve("EventAlarms.java")), StandardCharsets.UTF_8);
        expect("its words in both languages before Rust's", alarms.contains("{ \"public\", \"Sioul has a reminder for you.\", \"Sioul a un rappel pour vous.\" }"));
        String summaries = body(text, "    static void tidy(Context context)");
        expect("a group's summary is private, its title the channel's name alone", summaries.contains(".setVisibility(Notification.VISIBILITY_PRIVATE)") && !summaries.contains("said."));
        // A code's notice: on the lock screen, its title alone (Rust's `code_note`).
        String rust = new String(Files.readAllBytes(Paths.get(System.getProperty("sioul.root"), "crates", "sioul-app", "src", "eventalarms.rs")), StandardCharsets.UTF_8);
        int code = rust.indexOf("pub(crate) fn code_note(");
        expect("a code's public version: its title, never its body", code > 0 && rust.indexOf("Some((&notice.title, \"\"))", code) > code);
        System.out.println("EventNotesCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
