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
 * key, and Rust's {@code opened_of} reads the file as before).
 * ReminderOpener.isNoticeAddress takes every thing a notice of Sioul's is
 * about (an event, a message, the mailbox, the Porch, a need, the end of the
 * work day or of the whole day, a tablet's task or paper) and refuses a path,
 * a file: address and what no notice opens; Rust's {@code is_notice_address}
 * reads `noticeTaken` and `noticeRefused` below alike (eventalarms.rs's test).
 * The words said when Rust is silent (EventAlarms.fallen) carry the event's
 * address too. Read from the sources (android.jar's Intent is a stub that
 * cannot be made here): EventAlarms makes no intent to ReminderOpener of its
 * own (Notices.opening makes them all); the doses' alarms (DoseAlarms.java)
 * are not EventAlarms' and are not read.
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

    public static void main(String[] args) throws Exception
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

        // Every thing a notice of Sioul's is about, by its address; Rust's is_notice_address reads these two lists.
        String[] noticeTaken = { "sioul:event/yoga-1@example.org#1793520000", "mid:code-1@example.org", "sioul:mail/home/0123456789abcdef", "sioul:mailbox", "sioul:porch", "sioul:need/meal:0", "sioul:need/sleep", "sioul:day/work:2026-11-14", "sioul:day/night:2026-11-14", "sioul:task/a%2Fb", "sioul:budget/#rent", "sioul:paper/passport" };
        String[] noticeRefused = { "/data/user/0/com.aurelienpierre.sioul/files/mail/home/cur/1", "file:///sdcard/x.eml", "sioul:need/", "sioul:need/../x", "sioul:need/a/b", "sioul:day/work:yesterday", "sioul:day/work:2026-02-30", "sioul:day/lunch:2026-11-14", "sioul:day/work", "sioul:task/..", "sioul:task/a/../b", "sioul:settings/words", "sioul:move", "sioul:site/chat#1", "sioul:dose/a", "mid:../../etc/passwd", "sioul:task/a\\b", "" };
        for (String key : noticeTaken)
            expect("a notice's thing: " + key, ReminderOpener.isNoticeAddress(key) && ReminderOpener.opens(ReminderOpener.NOTICE, key));
        for (String key : noticeRefused)
            expect("refused as a notice's thing: " + key, !ReminderOpener.isNoticeAddress(key) && !ReminderOpener.opens(ReminderOpener.NOTICE, key));
        expect("nothing", !ReminderOpener.isNoticeAddress(null));

        // Rust silent: the words kept say it, with the event's address, until the event starts.
        long now = 1_793_519_100_000L;
        org.json.JSONObject entry = new org.json.JSONObject().put("key", "before:yoga").put("kind", "before").put("title", "Yoga in 15 min").put("body", "")
            .put("about", "sioul:event/yoga-1@example.org#1793520000").put("ends", 1_793_520_000_000L);
        Notices.Said fallen = EventAlarms.fallen(entry, "before:yoga", 300_000, now, "Open");
        expect("the words kept carry the event's address", fallen.about.equals("sioul:event/yoga-1@example.org#1793520000") && fallen.buttons.size() == 1 && fallen.buttons.get(0).open.equals(fallen.about));
        expect("until the event starts", fallen.lastsMs == 900_000 && fallen.endsMs == 1_793_520_000_000L && !fallen.actionable && fallen.tag.equals("before:yoga"));
        Notices.Said path = EventAlarms.fallen(new org.json.JSONObject(entry.toString()).put("about", "/c/yoga.ics#1793520000"), "before:yoga", 300_000, now, "Open");
        expect("a file kept: none, never the file", path.about.isEmpty() && path.buttons.isEmpty());
        Notices.Said older = EventAlarms.fallen(new org.json.JSONObject().put("key", "before:yoga").put("title", "Yoga"), "before:yoga", 300_000, now, "Open");
        expect("an older list without them: Sioul as by its icon, the minutes kept", older.about.isEmpty() && older.lastsMs == 300_000 && older.endsMs == 0);

        Path sources = Paths.get(System.getProperty("sioul.root"), "android", "package", "src", "com", "aurelienpierre", "sioul");
        String alarms = code(new String(Files.readAllBytes(sources.resolve("EventAlarms.java")), StandardCharsets.UTF_8));
        expect("EventAlarms makes no intent to ReminderOpener of its own", !alarms.contains("new Intent(context, ReminderOpener.class)") && !alarms.contains("ReminderOpener.KEY"));
        expect("its thing checked as an address in the words kept", alarms.contains("said.about = ReminderOpener.isNoticeAddress(about) ? about : \"\";"));
        expect("no file in EventAlarms' intents", !alarms.contains("Uri.fromFile(") && !alarms.contains("\"file:"));

        System.out.println("EventOpenCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
