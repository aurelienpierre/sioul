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
 * - B2, one sound at each return: what waited kept by app and told whole,
 *   once; Rust's words. Read from the sources: its channel at Android's
 *   default importance, posted anew so that it rings, gone after ten
 *   minutes or at a tap (the one app's launcher, else only taken away); a
 *   decision queued before the window hears its notification; told only
 *   for what would have rung and came back silent, once the window closed;
 *   people's messages planned on every version; counts only in the log.
 * - B3, after a restart: what a binding does with Android's count of the
 *   phone's starts; a tap's one app. Read from the sources: looked for a
 *   minute after the first binding of a start, before its first review, the
 *   reviews keeping the ledger's marks meanwhile, what Android holds and
 *   shows compared, said once among Sioul's notices (no sound, a day at
 *   most), the start noted after; counts only in the log.
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
        // The second look (the review of 28 November 2026, F3 and F4): the texting apps alone, no conversation's
        // record written from a held notification, nothing on Android 9 and 10.
        java.util.Set<String> texting = new java.util.HashSet<>(java.util.Arrays.asList("foundation.e.message", "org.fossify.messages"));
        expect("the phone's SMS app is looked at", AppNotes.looksAt("org.example.sms", "org.example.sms", texting), true);
        expect("a texting app Sioul knows", AppNotes.looksAt("foundation.e.message", "", texting), true);
        expect("never another app's chats", AppNotes.looksAt("com.whatsapp", "foundation.e.message", texting) || AppNotes.looksAt("org.thoughtcrime.securesms", "", texting), false);
        expect("no app, none", AppNotes.looksAt("", "", texting), false);
        String describing = body(notes, "private JSONObject describe(StatusBarNotification sbn, Ranking ranking)");
        expect("a conversation's record written only from a ranked notification", describing.contains("if (ranking != null && !shortcut.trim().isEmpty())") && describing.indexOf("remember(this,") > describing.indexOf("if (ranking != null && !shortcut.trim().isEmpty())"), true);
        String looking = body(notes, "private void look(String why, boolean now)");
        expect("the look: none on Android 9 and 10, texting apps alone, described without a ranking", looking.contains("if (!connected || fixed())") && looking.contains("looksAt(sbn.getPackageName(), smsApp(), apps)") && looking.contains("describe(sbn, null)") && !looking.contains("CATEGORY_MESSAGE"), true);
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

        // B2: people's messages that waited, kept and told once.
        HeldNotices.Waited waited = new HeldNotices.Waited();
        waited.add("org.thoughtcrime.securesms", "Signal");
        waited.add("com.whatsapp", "WhatsApp");
        waited.add("org.thoughtcrime.securesms", "Signal");
        waited.add("com.whatsapp", "WhatsApp");
        expect("counted, each app once, in the order they came", waited.json().toString(), "{\"count\":4,\"apps\":[\"Signal\",\"WhatsApp\"]}");
        expect("two apps: a tap opens none", waited.onlyApp(), null);
        HeldNotices.Waited batch = waited.take();
        expect("taken whole, the batch empty again", batch.count + " " + waited.count + " " + waited.apps.size(), "4 0 0");
        waited.add("org.thoughtcrime.securesms", "");
        expect("one app: a tap opens it", waited.onlyApp(), "org.thoughtcrime.securesms");
        expect("an app without a name: its package", waited.apps.get("org.thoughtcrime.securesms"), "org.thoughtcrime.securesms");
        HeldNotices.Told told = HeldNotices.Told.of(new JSONObject("{\"text\":\"Four messages waited: Signal and WhatsApp.\",\"public\":\"Messages waited.\",\"channel\":\"Messages that waited\"}"));
        expect("Rust's words", told.text + "|" + told.publicText + "|" + told.channel, "Four messages waited: Signal and WhatsApp.|Messages waited.|Messages that waited");
        expect("nothing said: nothing told", HeldNotices.Told.of(null).text, "");
        expect("its group, summed on Android 16, on its own channel", java.util.Arrays.asList(Notices.GROUPS).contains(HeldNotices.GROUP_WAITED) + " " + Notices.summaryChannel(HeldNotices.GROUP_WAITED), "true waited");
        expect("gone after ten minutes", HeldNotices.WAITED_MS, 600_000);
        String waitedCode = body(held, "static void waited(Context context, Told told, String onlyApp)");
        expect("its channel: Android's default importance, a sound, no banner", waitedCode.contains("Channels.quiet(WAITED, ") && waitedCode.contains("NotificationManager.IMPORTANCE_DEFAULT"), true);
        expect("posted anew so that it rings", waitedCode.indexOf("notifications.cancel(WAITED_TAG, Notices.NOTE);") > 0 && waitedCode.indexOf("notifications.cancel(WAITED_TAG, Notices.NOTE);") < waitedCode.indexOf("notifications.notify(WAITED_TAG, Notices.NOTE, note.build());") && waitedCode.contains(".setOnlyAlertOnce(false)"), true);
        expect("gone at a tap or after its time, never ongoing", waitedCode.contains(".setAutoCancel(true)") && waitedCode.contains(".setTimeoutAfter(WAITED_MS)") && waitedCode.contains(".setOngoing(false)"), true);
        String tapped = body(held, "private static PendingIntent tapped(Context context, String tag, String onlyApp)");
        expect("a tap: the one app by its launcher, else only taken away", tapped.contains("return NoticeReceiver.deleted(context, tag);") && tapped.contains("Intent.CATEGORY_LAUNCHER") && tapped.contains(".setPackage(onlyApp)") && tapped.contains("PendingIntent.FLAG_IMMUTABLE"), true);
        String posted = body(notes, "public void onNotificationPosted(StatusBarNotification sbn, RankingMap rankings)");
        expect("its decision queued before the window hears it, so that a close tells it", posted.indexOf("worker.post(() -> decide(sbn, given));") > 0 && posted.indexOf("worker.post(() -> decide(sbn, given));") < posted.indexOf("heard(sbn, n, given, runs);"), true);
        expect("heard on every version", body(notes, "private void heard(StatusBarNotification sbn, Notification n, Ranking ranking, boolean runs)").contains("window.used()"), false);
        String waitedNotes = body(notes, "private void waited(StatusBarNotification sbn, Ranking ranking)");
        expect("told only when it would have rung, and came back silent", waitedNotes.contains("QuietWindow.rings(ranking.getImportance(), ranking.matchesInterruptionFilter()") && waitedNotes.contains("!window.tells(sbn.getPostTime())"), true);
        expect("Rust says which are", notes.contains("if (decided.optBoolean(\"waited\", false))\n                waited(sbn, ranking);"), true);
        expect("told once the window closed", body(notes, "public void closed(int left)").contains("tellSoon();"), true);
        String tell = body(notes, "private void tell()");
        expect("never while a window is open; taken whole", tell.indexOf("if (window.isOpen())") >= 0 && tell.contains("waited.take()"), true);
        expect("people's messages planned on every version", notes.contains("window.hold(sbn.getKey(), hold, decided.optBoolean(\"people\", false))") && notes.contains("window.follow(times, people);"), true);
        expect("nothing of what waited logged but a count", logs(tell + waitedNotes), "[]");
        expect("Rust says people and waited", rust.contains("\"people\": people, \"waited\": waited") && rust.contains("\"people\": person"), true);

        // B3: after a restart, the count of the phone's starts.
        expect("the first binding of this Sioul: the count noted, nothing compared", HeldNotices.afterStart(-1, 41), HeldNotices.NOTE_START);
        expect("the same start: nothing", HeldNotices.afterStart(41, 41), HeldNotices.SAME_START);
        expect("the phone started since: looked for", HeldNotices.afterStart(41, 42), HeldNotices.LOOK);
        expect("Android cannot say: nothing", HeldNotices.afterStart(41, -1) + " " + HeldNotices.afterStart(-1, -1), HeldNotices.SAME_START + " " + HeldNotices.SAME_START);
        expect("one app: a tap opens it", HeldNotices.onlyApp(new org.json.JSONArray("[\"org.thoughtcrime.securesms\"]")), "org.thoughtcrime.securesms");
        expect("several, none or nothing said: a tap opens none", HeldNotices.onlyApp(new org.json.JSONArray("[\"a.b\",\"c.d\"]")) + " " + HeldNotices.onlyApp(new org.json.JSONArray("[]")) + " " + HeldNotices.onlyApp(null) + " " + HeldNotices.onlyApp(new org.json.JSONArray("[\" \"]")), "null null null null");
        expect("a day at most", HeldNotices.LOST_MS, 86_400_000L);
        String lostCode = body(held, "static void lost(Context context, Told told, String onlyApp)");
        expect("something to know: the notices' channel, no sound, their group", lostCode.contains("new Notification.Builder(context, Notices.NOTICES)") && lostCode.contains("Notices.channels(context, null);") && lostCode.contains("Notices.GROUP + Notices.INFORMATIVE") && lostCode.contains(".setOnlyAlertOnce(true)"), true);
        expect("gone at a tap or after a day", lostCode.contains(".setAutoCancel(true)") && lostCode.contains(".setTimeoutAfter(LOST_MS)") && lostCode.contains("tapped(context, LOST, onlyApp)"), true);
        String connected = body(notes, "public void onListenerConnected()");
        expect("looked for at the binding, before the first review", connected.indexOf("started();") > 0 && connected.indexOf("started();") < connected.indexOf("review(\"connected\");"), true);
        String started = body(notes, "private void started()");
        expect("Android's count of starts, a minute on", notes.contains("Settings.Global.BOOT_COUNT") && started.contains("worker.postDelayed(lost, LOST_AFTER_MS);") && started.contains("lostPending = true;"), true);
        expect("the marks kept meanwhile", body(notes, "private void review(String reason)").contains(".put(\"keep\", lostPending)"), true);
        String lostNotes = body(notes, "private void lost()");
        expect("what Android holds and shows compared", lostNotes.contains("getSnoozedNotifications()") && lostNotes.contains("getActiveNotifications()") && lostNotes.contains("nativeShade(\"lost\""), true);
        expect("the start noted once said, never looked at again", lostNotes.indexOf("listenerPrefs().edit().putInt(BOOT, bootCount()).apply();") > lostNotes.indexOf("nativeShade(\"lost\"") && lostNotes.contains("lostPending = false;"), true);
        expect("unbound meanwhile: the next binding looks", lostNotes.indexOf("if (!connected)\n            return;") >= 0, true);
        expect("nothing of what was lost logged but a count", logs(lostNotes.replace("said.optInt(\"count\", 0)", "count")), "[]");
        expect("Rust keeps the marks while asked", rust.contains("ledger.held.retain(|hash, held| keep || present.contains(hash)"), true);

        System.out.println("HeldCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
