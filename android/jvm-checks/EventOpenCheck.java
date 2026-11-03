// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * An event opened from its reminder by its address, never its file (the
 * leftovers round), on the JVM (android/jvm-checks/run.sh):
 * ReminderOpener.isEventAddress takes {@code sioul:event/<UID>}, with the
 * time it reminds of or without, and refuses a path, a file: address and
 * another thing's address; a reminder posted by an older Sioul, its file in
 * its intent, still opens (ReminderOpener.opens takes "event" with any
 * key, and Rust's {@code opened_of} reads the file as before). Read from the
 * sources (android.jar's Intent is a stub that cannot be made here):
 * EventAlarms.post names the event in its intent only by an address that
 * passes the check, and no other intent of EventAlarms names one; the
 * doses' alarms (DoseAlarms.java) are not EventAlarms' and are not read.
 */
public final class EventOpenCheck
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

    /** The text with its comments taken out, so that only code is searched. */
    static String code(String text)
    {
        return text.replaceAll("(?s)/\\*.*?\\*/", "").replaceAll("(?m)//.*$", "");
    }

    /** The body of `signature` in `text`, to its closing brace at four spaces; "" when it is not there. */
    static String body(String text, String signature)
    {
        int start = text.indexOf(signature);
        int end = start < 0 ? -1 : text.indexOf("\n    }\n", start);
        return start < 0 || end < 0 ? "" : text.substring(start, end);
    }

    public static void main(String[] args) throws IOException
    {
        // The addresses Rust gives (links::event_uri, remind::event_time's "#" and time).
        String[] taken = { "sioul:event/yoga-1@example.org#1793520000", "sioul:event/yoga-1@example.org", "sioul:event/a%2Fb%23c#1", "sioul:event/1234-ABCD" };
        for (String key : taken)
            expect("an event's address: " + key, ReminderOpener.isEventAddress(key));
        String[] refused = { "/data/user/0/com.aurelienpierre.sioul/files/calendars/local/club/yoga.ics#1793520000", "/c/yoga.ics", "file:///sdcard/yoga.ics",
            "sioul:event/", "sioul:event/../yoga.ics", "sioul:event/..", "sioul:event/.#1", "sioul:event/a/b", "sioul:event/a\\b", "sioul:event/a#b", "sioul:event/a#1#2",
            "sioul:task/yoga", "mid:yoga@example.org", "yoga.ics", "", null };
        for (String key : refused)
            expect("refused as an event's address: " + key, !ReminderOpener.isEventAddress(key));
        // A notice an older Sioul posted, its file in its intent: still opened, as before.
        expect("an older notice's file still opens", ReminderOpener.opens("event", "/data/user/0/com.aurelienpierre.sioul/files/calendars/local/club/yoga.ics#1793520000"));
        expect("an address opens", ReminderOpener.opens("event", "sioul:event/yoga-1@example.org#1793520000"));

        Path sources = Paths.get(System.getProperty("sioul.root"), "android", "package", "src", "com", "aurelienpierre", "sioul");
        String alarms = code(new String(Files.readAllBytes(sources.resolve("EventAlarms.java")), StandardCharsets.UTF_8));
        String post = body(alarms, "private static void post(");
        expect("EventAlarms.post is there", !post.isEmpty());
        expect("the event checked as an address before its intent", post.contains("String address = ReminderOpener.isEventAddress(event) ? event : \"\";"));
        expect("its intent names the checked address alone", post.contains(".putExtra(ReminderOpener.KEY, address)") && !post.contains(".putExtra(ReminderOpener.KEY, event)"));
        expect("without an address, Sioul as by its icon", post.contains("address.isEmpty() ? \"app\" : \"event\""));
        expect("one intent to ReminderOpener in EventAlarms, post's", alarms.split("new Intent\\(context, ReminderOpener\\.class\\)", -1).length == 2);
        expect("ReminderOpener.KEY put in post alone", alarms.split("ReminderOpener\\.KEY", -1).length == 2);
        expect("no file in EventAlarms' intents", !alarms.contains("Uri.fromFile(") && !alarms.contains("\"file:"));

        System.out.println("EventOpenCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
