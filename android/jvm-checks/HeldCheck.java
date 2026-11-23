// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

import org.json.JSONObject;

/**
 * What Sioul says in the phone's shade of the other apps' notifications it
 * holds (HeldNotices, AppNotes; held apps phase 1, part B), on the JVM:
 *
 * - B1, the standing line: Rust's words read (HeldNotices.Line.of), posted
 *   again only when they change; its one door (ReminderOpener.opens, this
 *   phone's card, no other setting); its group and its summary's channel.
 *   Read from the sources: its channel at Android's minimum importance,
 *   never ongoing, Show now a broadcast to the listener's own receiver, a
 *   tap the card, the line written at each review and after a hold or a
 *   return, taken away when the listener is unbound or ends; Show now a
 *   release then a review; Rust's answer as Java reads it.
 */
public final class HeldCheck
{
    static int failed = 0;
    static int checked = 0;

    static void expect(String what, Object got, Object wanted)
    {
        checked++;
        if (!String.valueOf(got).equals(String.valueOf(wanted))) {
            failed++;
            System.out.println("FAIL " + what + ": " + got + " (expected " + wanted + ")");
        }
    }

    static String read(Path file) throws IOException
    {
        return new String(Files.readAllBytes(file), StandardCharsets.UTF_8);
    }

    /** A method's body in a source, from its signature to its closing brace at four spaces. */
    static String body(String source, String signature)
    {
        int at = source.indexOf(signature);
        if (at < 0)
            return "";
        int end = source.indexOf("\n    }\n", at);
        return end < 0 ? source.substring(at) : source.substring(at, end);
    }

    /** The log lines of a piece of code that could carry Rust's words or a notification's: none expected. */
    static java.util.List<String> logs(String code)
    {
        java.util.List<String> out = new java.util.ArrayList<>();
        for (String one : code.split("\n"))
            if (one.contains("Log.") && (one.contains("answer") || one.contains(".text") || one.contains("said")))
                out.add(one.trim());
        return out;
    }

    public static void main(String[] args) throws Exception
    {
        // B1: the standing line as Rust says it.
        HeldNotices.Line line = HeldNotices.Line.of(new JSONObject("{\"text\":\"Notifications held until 13:00: Shop and Signal.\",\"public\":\"Sioul holds notifications until their time.\",\"show\":\"Show now\",\"channel\":\"Held notifications\"}"));
        expect("its words", line.text + "|" + line.show + "|" + line.channel, "Notifications held until 13:00: Shop and Signal.|Show now|Held notifications");
        expect("the lock screen's: no app named", line.publicText, "Sioul holds notifications until their time.");
        expect("nothing said: nothing to show", HeldNotices.Line.of(null).text + HeldNotices.Line.of(new JSONObject("{\"text\":\"  \"}")).text, "");
        HeldNotices.Line paused = HeldNotices.Line.of(new JSONObject("{\"text\":\"Notifications held until 13:00: Shop and Signal.\",\"show\":\"\"}"));
        expect("in a pause, no Show now", paused.show.isEmpty(), true);
        expect("the same words: not posted again", line.same(HeldNotices.Line.of(new JSONObject("{\"text\":\"Notifications held until 13:00: Shop and Signal.\",\"show\":\"Show now\"}"))), true);
        expect("Show now gone: posted again", line.same(paused), false);
        expect("other words: posted again", line.same(HeldNotices.Line.of(new JSONObject("{\"text\":\"Notifications held until 18:00: Shop.\",\"show\":\"Show now\"}"))), false);
        expect("none posted yet: posted", line.same(null), false);
        // Its one door: this phone's card at its apps; no other setting opens from a notification.
        expect("the card opens", ReminderOpener.opens(HeldNotices.CARD, ""), true);
        expect("the card's door", HeldNotices.CARD, "settings:devices.apps");
        expect("no other setting", ReminderOpener.opens("settings:words", "") || ReminderOpener.opens("settings:devices.calls", ""), false);
        expect("its group, summed on Android 16", java.util.Arrays.asList(Notices.GROUPS).contains(HeldNotices.GROUP_HELD), true);
        expect("its summary on its own channel", Notices.summaryChannel(HeldNotices.GROUP_HELD), HeldNotices.HELD);

        // From the sources.
        Path sources = Paths.get(System.getProperty("sioul.root"), "android", "package", "src", "com", "aurelienpierre", "sioul");
        String held = read(sources.resolve("HeldNotices.java"));
        String notes = read(sources.resolve("AppNotes.java"));
        String lineCode = body(held, "static void line(Context context, Line line)");
        expect("its channel: minimum importance, no sound", lineCode.contains("Channels.quiet(HELD, ") && lineCode.contains("NotificationManager.IMPORTANCE_MIN") && lineCode.contains("channel.setSound(null, null)"), true);
        expect("never ongoing, alerting once, staying at a tap", lineCode.contains(".setOngoing(false)") && lineCode.contains(".setOnlyAlertOnce(true)") && lineCode.contains(".setAutoCancel(false)"), true);
        expect("nothing said goes", lineCode.indexOf("if (line.text.isEmpty()) {") >= 0 && lineCode.indexOf("cancelLine(context);") > lineCode.indexOf("if (line.text.isEmpty()) {"), true);
        expect("private on the lock screen, its public words", lineCode.contains(".setVisibility(Notification.VISIBILITY_PRIVATE)") && lineCode.contains(".setContentText(line.publicText)"), true);
        expect("Show now only when offered", lineCode.contains("if (!line.show.isEmpty())"), true);
        String show = body(held, "private static PendingIntent showNow(Context context)");
        expect("Show now: the listener's own receiver", show.contains("new Intent(context, AppNotes.Asked.class).setAction(AppNotes.SHOW)") && show.contains("PendingIntent.getBroadcast("), true);
        String card = body(held, "private static PendingIntent card(Context context)");
        expect("a tap: the card, through ReminderOpener, the line left", card.contains("ReminderOpener.class") && card.contains("Uri.fromParts(ReminderOpener.SCHEME, CARD, null)") && !card.contains("ReminderOpener.NOTE"), true);
        expect("Show now heard on the worker", notes.contains("} else if (SHOW.equals(action)) {\n                listener.worker.post(listener::show);"), true);
        String showNow = body(notes, "private void show()");
        expect("Show now: a release, then a review", showNow.indexOf("nativeShade(\"show\", \"{}\")") > 0 && showNow.indexOf("review(\"show\");") > showNow.indexOf("nativeShade(\"show\", \"{}\")"), true);
        String review = body(notes, "private void review(String reason)");
        expect("the line written at each review, of what Android holds", review.contains("line(keys);"), true);
        expect("and after a hold or a return, once a burst settled", notes.contains("if (decided.optBoolean(\"changed\", false)) {\n                worker.removeCallbacks(lineSoon);\n                worker.postDelayed(lineSoon, LINE_SETTLE_MS);"), true);
        expect("gone when the listener is unbound", body(notes, "public void onListenerDisconnected()").contains("HeldNotices.cancelLine(this);"), true);
        expect("and when it ends", body(notes, "public void onDestroy()").contains("HeldNotices.cancelLine(this);"), true);
        String lineNotes = body(notes, "private void line(JSONArray keys)");
        expect("posted again only when it changed", lineNotes.contains("if (line.same(shown))"), true);
        expect("nothing of the line logged", logs(lineNotes + body(notes, "private void show()")), "[]");
        // Rust's answer, as Java reads it.
        String rust = read(Paths.get(System.getProperty("sioul.root"), "crates", "sioul-app", "src", "appnotes.rs"));
        String rustLine = rust.substring(Math.max(0, rust.indexOf("        \"line\" => {")));
        expect("Rust says text, public, show and channel", rustLine.contains("\"text\": text,") && rustLine.contains("\"public\":") && rustLine.contains("\"show\":") && rustLine.contains("\"channel\":"), true);
        expect("and changed at a hold or a return", rust.contains("\"changed\": changed"), true);
        String main = read(Paths.get(System.getProperty("sioul.root"), "android", "main.cpp"));
        expect("the bridge to Rust", main.contains("Java_com_aurelienpierre_sioul_AppNotes_nativeShade") && main.contains("extern \"C\" char *sioul_appnotes_shade(const char *verb, const char *json);"), true);

        System.out.println("HeldCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
