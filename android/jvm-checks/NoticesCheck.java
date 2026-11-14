// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Sioul's own notices on a phone (Notices.java; docs/android.md,
 * "Notifications"), on the JVM (org.json's own jar before android.jar's
 * stubs on the class path):
 * - Notices.Said.of: the two classes as Rust says them (CODE below is
 *   Rust's code_note, which eventalarms::tests::a_code_s_notice_as_java_reads_it
 *   makes again and compares with this file's), each in its group, a code's
 *   Copy and Open the message, its lock screen its title alone;
 * - Notices.timeout: the lifetime rule, five minutes from when the phone
 *   could show it, never past its relevance; something to do until its end;
 * - Notices.summaryTimeout: a group's summary goes with its last notice;
 * - Notices.channelOf: the class's channel, the twin that passes only for
 *   something to do that pierces while one of Sioul's modes is on;
 * - EventAlarms.tapped, fresh: taps kept apart, each once, none older than
 *   five minutes, none from the future;
 * - read from the sources (android.jar's NotificationManager is a stub):
 *   the old channels deleted once the new ones exist, never the doses'; an
 *   explicit group on every notification but the doses' (left as they
 *   were); the background service's card in a group of its own, its words
 *   fixed, its calls' buttons only while calls are screened, the phone's
 *   unlock heard; a code withdrawn after Copy and once its message is read;
 *   a tap taken at the window's start too (cold start).
 */
public final class NoticesCheck
{
    static int failed = 0;
    static int checked = 0;

    /** A one-time code's notice, as Rust's code_note gives it (an expiry ten minutes on). */
    static final String CODE = "{\"tag\":\"codes|mid:code-1@example.org\",\"about\":\"mid:code-1@example.org\",\"kind\":\"codes\",\"class\":\"actionable\",\"group\":\"actionable\",\"lasts_ms\":0,\"ends\":1794000600000,\"pierce\":true,\"title\":\"A code from example.org\",\"body\":\"482913, valid about 10 minutes\",\"actions\":[{\"label\":\"Copy the code\",\"copy\":\"482913\"},{\"label\":\"Open the message\",\"open\":\"mid:code-1@example.org\"}],\"public\":{\"title\":\"A code from example.org\",\"body\":\"\"},\"words\":{}}";

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

    /** The text with its comments taken out, so that only code is searched. */
    static String code(String text)
    {
        return text.replaceAll("(?s)/\\*.*?\\*/", "").replaceAll("(?m)//.*$", "");
    }

    public static void main(String[] args) throws Exception
    {
        final long now = 1_794_000_000_000L;
        final long five = 5 * 60_000L;

        // The two classes, each in its group.
        Notices.Said code = Notices.Said.of(new JSONObject(CODE));
        expect("a code: something to do", code.actionable + " " + code.group + " " + code.lastsMs + " " + code.endsMs + " " + code.pierce, "true actionable 0 1794000600000 true");
        expect("Copy, then Open the message", code.buttons.size() + " " + code.buttons.get(0).copy + " " + code.buttons.get(0).open.isEmpty() + " " + code.buttons.get(1).open, "2 482913 true mid:code-1@example.org");
        expect("a tap opens its message", code.about, "mid:code-1@example.org");
        expect("on the lock screen its title, never the code", code.publicTitle + "|" + code.publicBody, "A code from example.org|");
        expect("never the code there", code.publicTitle.contains("482913") || code.publicBody.contains("482913"), false);
        Notices.Said know = Notices.Said.of(new JSONObject("{\"tag\":\"before:a\",\"class\":\"informative\",\"pierce\":true,\"lasts_ms\":300000,\"ends\":1794003600000}"));
        expect("something to know never pierces", know.actionable + " " + know.group + " " + know.pierce, "false informative false");
        Notices.Said unsaid = Notices.Said.of(new JSONObject("{\"tag\":\"x\",\"class\":\"actionable\"}"));
        expect("a group unsaid: its class's", unsaid.group, "actionable");
        Notices.Said forged = Notices.Said.of(new JSONObject("{\"tag\":\"x\",\"group\":\"steps\",\"about\":\"file:///sdcard/x\",\"actions\":[{\"label\":\"Copy\",\"copy\":\"1\",\"open\":\"/data/x\"}]}"));
        expect("no group but the two classes, no thing that is no address, no button half one", forged.group + " [" + forged.about + "] " + forged.buttons.size(), "informative [] 0");

        // The lifetime rule.
        expect("in use: five minutes from now", Notices.timeout(false, five, now + 3_600_000, now, true), five);
        expect("in use, its relevance sooner: then", Notices.timeout(false, five, now + 60_000, now, true), 60_000);
        expect("not seen: until its relevance, then its minutes from the unlock", Notices.timeout(false, five, now + 3_600_000, now, false), 3_600_000);
        expect("seen at the unlock, 50 minutes on: five minutes from then", Notices.timeout(false, five, now + 3_600_000, now + 3_000_000, true), five);
        expect("seen at the unlock, 58 minutes on: two left", Notices.timeout(false, five, now + 3_600_000, now + 3_480_000, true), 120_000);
        expect("not seen, no relevance said: until seen", Notices.timeout(false, five, 0, now, false), 0);
        expect("seen, no relevance said: its minutes", Notices.timeout(false, five, 0, now, true), five);
        expect("the time to leave: until its event starts", Notices.timeout(false, 1_200_000, now + 1_200_000, now, true), 1_200_000);
        expect("ended a moment ago: a millisecond, never none", Notices.timeout(false, five, now - 1, now, true), 1);
        expect("something to do: until its thing ends, seen or not", Notices.timeout(true, 0, now + 600_000, now, false) + " " + Notices.timeout(true, 0, now + 600_000, now, true), "600000 600000");
        expect("something to do without an end: stays", Notices.timeout(true, 0, 0, now, true), 0);

        // A group's summary goes with its last notice; never while one stays.
        expect("with the last", Notices.summaryTimeout(new long[] { now - 60_000, now }, new long[] { five, five }, now), five);
        expect("one stays: the summary stays", Notices.summaryTimeout(new long[] { now, now }, new long[] { five, 0 }, now), 0);
        expect("none", Notices.summaryTimeout(new long[0], new long[0], now), 0);

        // The channels.
        expect("to know: Notices", Notices.channelOf(know, Notices.ACT_PAUSE), Notices.NOTICES);
        expect("to do, piercing, a mode on: its twin", Notices.channelOf(code, Notices.ACT_PAUSE), Notices.ACT_PAUSE);
        expect("to do, piercing, no mode on: To act on", Notices.channelOf(code, null), Notices.ACT);
        Notices.Said quiet = Notices.Said.of(new JSONObject(CODE).put("pierce", false));
        expect("to do, not piercing: To act on", Notices.channelOf(quiet, Notices.ACT_PAUSE), Notices.ACT);
        Notices.Said through = MailNotes.said(new JSONObject().put("title", "One e-mail").put("open", "mid:a@example.org"), five);
        through.channel = PauseMode.MAIL_CHANNEL;
        expect("new mail's own that passes, as before", Notices.channelOf(through, null), PauseMode.MAIL_CHANNEL);

        // The taps: each kept apart, once, five minutes at most.
        JSONArray taps = new JSONArray();
        taps = EventAlarms.tapped(taps, "notice", "mid:a@example.org", now);
        taps = EventAlarms.tapped(taps, "notice", "sioul:event/yoga#1", now + 1_000);
        taps = EventAlarms.tapped(taps, "dose", "x", now + 2_000);
        expect("taps of different kinds kept apart", taps.length(), 3);
        taps = EventAlarms.tapped(taps, "notice", "mid:a@example.org", now + 3_000);
        expect("the same thing tapped again: once, last", taps.length() + " " + taps.getJSONObject(2).getString("key"), "3 mid:a@example.org");
        JSONArray fresh = EventAlarms.fresh(taps, now + 4_000);
        expect("taken in their order", fresh.getJSONObject(0).getString("key") + " " + fresh.getJSONObject(2).getString("key"), "sioul:event/yoga#1 mid:a@example.org");
        expect("no time kept with them", fresh.getJSONObject(0).has("at"), false);
        expect("a tap five minutes old: still", EventAlarms.fresh(taps, now + 1_000 + EventAlarms.TAP_KEPT_MS).length(), 3);
        expect("older: only the later ones", EventAlarms.fresh(taps, now + 2_001 + EventAlarms.TAP_KEPT_MS).length(), 1);
        expect("all gone after", EventAlarms.fresh(taps, now + 3_001 + EventAlarms.TAP_KEPT_MS).length(), 0);
        expect("none from the future (a clock set back)", EventAlarms.fresh(taps, now - 60_000).length(), 0);
        expect("an older Sioul's, without its time: none", EventAlarms.fresh(new JSONArray("[{\"kind\":\"mail\",\"key\":\"mid:a@example.org\"}]"), now).length(), 0);
        JSONArray many = new JSONArray();
        for (int i = 0; i < 12; i++)
            many = EventAlarms.tapped(many, "notice", "mid:" + i + "@example.org", now + i);
        expect("eight kept at most, the latest", many.length() + " " + many.getJSONObject(7).getString("key"), "8 mid:11@example.org");

        // From the sources.
        Path sources = Paths.get(System.getProperty("sioul.root"), "android", "package", "src", "com", "aurelienpierre", "sioul");
        String notices = code(read(sources.resolve("Notices.java")));
        expect("the retired channels", Arrays.toString(Notices.RETIRED), "[events, event-alarms, event-alarms-pause, mail, day]");
        expect("never the doses'", Arrays.asList(Notices.RETIRED).contains("doses") || Arrays.asList(Notices.RETIRED).contains("doses-pause"), false);
        int made = notices.indexOf("Channels.quiet(ACT, ");
        int deleted = notices.indexOf("deleteNotificationChannel(id)");
        expect("deleted once ours exist", made > 0 && deleted > made, true);
        expect("the classes' channels: low, default", notices.contains("Channels.quiet(NOTICES, word(context, words, \"notices\"), NotificationManager.IMPORTANCE_LOW)") && notices.contains("Channels.quiet(ACT, word(context, words, \"act\"), NotificationManager.IMPORTANCE_DEFAULT)"), true);
        List<String> stillMaking = new ArrayList<>();
        List<String> ungrouped = new ArrayList<>();
        try (DirectoryStream<Path> files = Files.newDirectoryStream(sources, "*.java")) {
            for (Path file : files) {
                String text = code(read(file));
                String name = file.getFileName().toString();
                for (String id : Notices.RETIRED)
                    if (text.contains("Channels.quiet(\"" + id + "\"") || text.contains("CHANNEL = \"" + id + "\"") || text.contains("_CHANNEL = \"" + id + "\""))
                        stillMaking.add(name + ": " + id);
                // Every notification posted (built, then handed to notify or startForeground) has its group; the doses' are left as they were.
                if (text.contains(".notify(") && text.contains("new Notification.Builder(") && !name.equals("DoseAlarms.java") && !text.contains(".setGroup("))
                    ungrouped.add(name);
            }
        }
        expect("no retired channel made again", stillMaking, "[]");
        expect("every notification in a group, the doses' left as they were", ungrouped, "[]");
        String service = code(read(sources.resolve("StepService.java")));
        expect("the background service's card: a group of its own", service.contains("static final String GROUP = \"sioul.steps\";") && service.contains(".setGroup(GROUP)"), true);
        expect("its words fixed: no time of day, the calls' line only while screened", service.contains("if (!line.isEmpty())") && !service.contains("line + \" \" + through"), true);
        expect("its calls' buttons only while calls are screened", service.contains("if (words != null && words.optBoolean(\"screening\"))"), true);
        expect("the phone's unlock heard while it runs", service.contains("Intent.ACTION_USER_PRESENT") && service.contains("Notices.seen(context.getApplicationContext())") && service.contains("unregisterReceiver(unlocks)"), true);
        String receiver = code(read(sources.resolve("NoticeReceiver.java")));
        expect("a code copied: sensitive, then its notice taken away", receiver.contains("ClipDescription.EXTRA_IS_SENSITIVE") && receiver.indexOf("setPrimaryClip(clip)") < receiver.indexOf("Notices.dismiss(context, tag)"), true);
        expect("a code withdrawn once its message is read (Rust's notice-withdraw)", service.contains("case \"notice-withdraw\":") && service.contains("Notices.withdraw(context, asked.optString(\"about\", \"\"))"), true);
        String manifest = read(Paths.get(System.getProperty("sioul.root"), "android", "package", "AndroidManifest.xml"));
        int at = manifest.indexOf("android:name=\"com.aurelienpierre.sioul.NoticeReceiver\"");
        expect("NoticeReceiver declared, never exported", at > 0 && manifest.indexOf("android:exported=\"false\"", at) - at < 80, true);
        String backend = read(Paths.get(System.getProperty("sioul.root"), "crates", "sioul-app", "src", "backend.rs"));
        int start = backend.indexOf("    fn start(mut self: Pin<&mut Self>) {");
        int startEnd = backend.indexOf("\n    }\n", start);
        expect("a tap taken at the window's start too (cold start)", start > 0 && backend.substring(start, startEnd).contains("qt.queue(|sioul| sioul.take_taps())"), true);
        int back = backend.indexOf("    fn back_here(mut self: Pin<&mut Self>) {");
        expect("and as it comes back", back > 0 && backend.substring(back, backend.indexOf("\n    }\n", back)).contains("self.as_mut().take_taps();"), true);

        System.out.println(failed == 0 ? "all " + checked + " checks agree" : failed + " of " + checked + " failed");
        System.exit(failed == 0 ? 0 : 1);
    }
}
