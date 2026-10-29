// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * The lock screen's version of Sioul's reminders, on the JVM
 * (android/jvm-checks/run.sh): every reminder EventAlarms posts in its
 * channels is private, with a public version that says only that Sioul has
 * a reminder (composed tasks §7.3: the leaving reminder names neither the
 * appointment nor what to bring there), in both languages when Rust has not
 * given its words yet. Read from the sources (the repository's root comes as
 * the property sioul.root): android.jar's Notification is a stub that cannot
 * be made here. The doses' alarms (DoseAlarms.java) are not EventAlarms'.
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

    public static void main(String[] args) throws IOException
    {
        Path sources = Paths.get(System.getProperty("sioul.root"), "android", "package", "src", "com", "aurelienpierre", "sioul");
        String text = new String(Files.readAllBytes(sources.resolve("EventAlarms.java")), StandardCharsets.UTF_8);
        int post = text.indexOf("private static void post(");
        int end = post < 0 ? -1 : text.indexOf("\n    }\n", post);
        String posted = post < 0 || end < 0 ? "" : text.substring(post, end);
        expect("EventAlarms.post is there", !posted.isEmpty());
        expect("a reminder is private", posted.contains(".setVisibility(Notification.VISIBILITY_PRIVATE)"));
        expect("with its public version", posted.contains(".setPublicVersion(publicVersion(context, channel))"));
        int made = text.indexOf("private static Notification publicVersion(");
        int madeEnd = made < 0 ? -1 : text.indexOf("\n    }\n", made);
        String version = made < 0 || madeEnd < 0 ? "" : text.substring(made, madeEnd);
        expect("the public version says Sioul's words alone", version.contains("word(context, \"public\")") && !version.contains("title") && !version.contains("body"));
        expect("its words in both languages before Rust's", text.contains("{ \"public\", \"Sioul has a reminder for you.\", \"Sioul a un rappel pour vous.\" }"));
        System.out.println("EventNotesCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
