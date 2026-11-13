// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * The quiet window, on the JVM (QuietWindow; option 1 of
 * docs/research/android-holding.md; the review of phase 4), driven as the
 * listener drives it, against a stand-in phone with its own clock: used only
 * where returns ring (Android 9 and 10, 13 from its December 2022 patch, 14
 * and later), nothing changed on 11 to 13.0; a review never opens a window,
 * only Sioul's alarm does, after reading Sioul's own reminders and doses
 * again, moving past one due within it; Android unable to say what it holds,
 * the alarm comes again, the plan kept; a key gone meanwhile left out; the
 * phone kept awake from before the hint until the end, the end counted on
 * both clocks; the hint taken off when each came back or at the end, on
 * every way out; strangers held again for a second on the worker after the
 * hint, never one held meanwhile, a summary, an update alerting once, a call;
 * one posted just before the hint went caught too; Android 9 and 10 around
 * the first time, never moved; no window before the safety net, the batch
 * left to ring. Then, read from the sources, the listener's side.
 */
public final class QuietCheck
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

    static final long T = 1_000_000_000_000L;

    /** Android and the listener as the window sees them: what it asked, in order; its clock, alarms and worker. */
    static final class Phone implements QuietWindow.Host
    {
        final List<String> said = new ArrayList<>();
        final Set<String> shade = new HashSet<>();
        final Set<String> snoozed = new HashSet<>();
        final List<Runnable> worker = new ArrayList<>();
        long[] own = {};
        boolean blind;
        long clock = T;
        long alarm = -1;
        long ending = -1;
        long awake;
        int closed = -1;

        @Override
        public void hint(boolean quiet)
        {
            said.add(quiet ? "hint" : "no hint");
        }

        @Override
        public void snooze(String key, long ms)
        {
            said.add("hold " + key + " " + ms);
        }

        @Override
        public Set<String> shown(Collection<String> keys)
        {
            Set<String> found = new HashSet<>(keys);
            found.retainAll(shade);
            return found;
        }

        @Override
        public Set<String> active()
        {
            return new HashSet<>(shade);
        }

        @Override
        public Set<String> held()
        {
            return blind ? null : new HashSet<>(snoozed);
        }

        @Override
        public long[] own()
        {
            said.add("read own");
            return own;
        }

        @Override
        public long now()
        {
            return clock;
        }

        @Override
        public void alarm(long at)
        {
            alarm = at;
        }

        @Override
        public void ending(long ms)
        {
            ending = ms;
            said.add(ms < 0 ? "end dropped" : "end in " + ms);
        }

        @Override
        public void awake(long ms)
        {
            awake = ms;
            said.add(ms > 0 ? "awake " + ms : "asleep");
        }

        @Override
        public void later(Runnable task)
        {
            worker.add(task);
        }

        @Override
        public void closed(int left)
        {
            closed = left;
            said.add("closed " + left);
        }

        /** The worker runs what it was handed. */
        void work()
        {
            List<Runnable> tasks = new ArrayList<>(worker);
            worker.clear();
            for (Runnable task : tasks)
                task.run();
        }

        /** What was asked, without the reads of Sioul's own times. */
        String asked()
        {
            List<String> out = new ArrayList<>(said);
            out.removeIf(s -> s.equals("read own"));
            return out.toString();
        }
    }

    static String sources(String name) throws IOException
    {
        return new String(Files.readAllBytes(Paths.get(System.getProperty("sioul.root"), "android", "package", "src", "com", "aurelienpierre", "sioul", name)), StandardCharsets.UTF_8);
    }

    public static void main(String[] args) throws IOException
    {
        // Where a held notification's return rings, by version and security patch.
        expect("Android 9", QuietWindow.returnsRing(28, "2020-01-05"), true);
        expect("Android 10", QuietWindow.returnsRing(29, "2021-01-05"), true);
        expect("Android 11", QuietWindow.returnsRing(30, "2023-01-05"), false);
        expect("Android 12", QuietWindow.returnsRing(31, "2024-03-01"), false);
        expect("Android 12L", QuietWindow.returnsRing(32, "2024-03-01"), false);
        expect("Android 13.0", QuietWindow.returnsRing(33, "2022-11-05"), false);
        expect("Android 13 QPR1", QuietWindow.returnsRing(33, "2022-12-05"), true);
        expect("Android 13 QPR3", QuietWindow.returnsRing(33, "2023-06-01"), true);
        expect("Android 13, patch unknown", QuietWindow.returnsRing(33, ""), false);
        expect("Android 13, patch null", QuietWindow.returnsRing(33, null), false);
        expect("Android 14", QuietWindow.returnsRing(34, ""), true);
        expect("Android 16", QuietWindow.returnsRing(36, "2026-10-01"), true);

        // Android 12: nothing changes from before.
        Phone phone = new Phone();
        QuietWindow window = new QuietWindow(phone, false, false);
        expect("held as asked", window.hold("shop", 120_000), 120_000);
        expect("a review's new time as asked", window.moved(1_000, true), 1_000);
        window.follow(Map.of("shop", T));
        expect("no plan", window.planned(), 0);
        expect("no alarm", phone.alarm, -1);
        expect("no window", window.rang(), false);
        window.posted("noa", true, false, T);
        expect("nothing asked", phone.asked(), "[]");

        // Android 14: held five minutes past its time, planned, the alarm at its time.
        phone = new Phone();
        window = new QuietWindow(phone, true, false);
        expect("held past its time, the safety net", window.hold("shop", 120_000), 120_000 + QuietWindow.SAFETY_MS);
        expect("the alarm at its time", phone.alarm, T + 120_000);
        // A review never opens a window: a release asked now sets the alarm now.
        expect("due now: nothing asked of Android here", window.moved(1_000, true), -1);
        expect("moved later: past it again", window.moved(60_000, true), 60_000 + QuietWindow.SAFETY_MS);
        expect("a person's, as asked", window.moved(60_000, false), 60_000);
        Map<String, Long> rust = new TreeMap<>();
        rust.put("shop", T + 120_000);
        rust.put("noa", T);
        window.follow(rust);
        expect("the release's alarm at once", phone.alarm, T);
        expect("no hint from a review", phone.asked(), "[]");
        expect("both planned", window.planned(), 2);
        // The alarm rings at T, but a dose was armed since at T + 3 s: Sioul's times read again, the window moves.
        phone.own = new long[] { T + 3_000 };
        phone.snoozed.addAll(List.of("noa", "shop"));
        expect("no window on a dose", window.rang(), false);
        expect("Sioul's times read right then", phone.said.contains("read own"), true);
        expect("the alarm past the dose's minute", phone.alarm, T + 3_000 + QuietWindow.BUSY_MS);
        expect("no hint", phone.asked(), "[]");
        expect("the plan kept", window.planned(), 2);
        // Then: Android cannot say what it holds: again in five seconds, the plan kept.
        phone.clock = T + 63_000;
        phone.blind = true;
        expect("blind: no window", window.rang(), false);
        expect("again soon", phone.alarm, T + 63_000 + QuietWindow.RETRY_MS);
        expect("still planned", window.planned(), 2);
        // Then it opens over what is due: awake before the hint, each held again, the end on both clocks.
        phone.blind = false;
        phone.clock = T + 68_000;
        expect("opened", window.rang(), true);
        expect("awake, the hint, each held again, the end",
               phone.asked(), "[awake " + (QuietWindow.AT_MOST_MS + QuietWindow.MARGIN_MS) + ", hint, hold noa 1000, end in " + QuietWindow.AT_MOST_MS + "]");
        expect("the shop's still to come", window.next(), T + 120_000);
        expect("its alarm", phone.alarm, T + 120_000);
        phone.said.clear();
        window.posted("noa", true, false, T + 69_000);
        expect("each came back: closed", window.isOpen(), false);
        expect("the end dropped, the hint off, asleep", phone.asked(), "[end dropped, no hint, asleep, closed 0]");

        // The end comes (one never came back, the phone asleep but for the wake lock): the hint off all the same.
        phone = new Phone();
        window = new QuietWindow(phone, true, false);
        phone.snoozed.addAll(List.of("shop", "news"));
        window.hold("shop", 0);
        window.hold("news", 0);
        window.rang();
        window.posted("shop", true, false, T);
        phone.said.clear();
        window.close();
        expect("closed at its end", phone.asked(), "[end dropped, no hint, asleep, closed 1]");
        window.close();
        expect("closed once", phone.closed, 1);

        // A key gone meanwhile (its app took it away): left out; none left, no window.
        phone = new Phone();
        window = new QuietWindow(phone, true, false);
        window.hold("gone", 0);
        expect("nothing Android holds: no window", window.rang(), false);
        expect("no hint", phone.asked(), "[]");
        expect("taken out of the plan", window.planned(), 0);

        // Strangers: a person's message rings after the hint, on the worker; one Sioul held meanwhile keeps
        // its time; a summary, an update alerting once, a silent one and a call never; Sioul's own notice is
        // one; an alarm's notice with a sound is one (the hint mutes it too).
        phone = new Phone();
        window = new QuietWindow(phone, true, false);
        phone.snoozed.add("shop");
        phone.shade.addAll(List.of("noa", "update", "sioul-mail", "pills", "held-by-rust"));
        window.hold("shop", 0);
        window.rang();
        window.posted("noa", QuietWindow.rings(4, true, false, "msg", false, false), false, T);
        window.posted("held-by-rust", QuietWindow.rings(3, true, false, "msg", false, false), false, T);
        window.posted("summary", QuietWindow.rings(4, true, false, "msg", false, true), false, T);
        window.posted("silent", QuietWindow.rings(2, true, false, "promo", false, false), false, T);
        window.posted("update", QuietWindow.rings(4, true, false, "msg", false, false), true, T);
        window.posted("ringing", QuietWindow.rings(4, true, true, "call", false, false), false, T);
        window.posted("sioul-mail", QuietWindow.rings(3, true, false, "email", false, false), false, T);
        window.posted("pills", QuietWindow.rings(4, true, false, "alarm", false, false), false, T);
        phone.said.clear();
        window.posted("shop", true, false, T + 1_000);
        expect("the hint off before anything else", phone.asked(), "[end dropped, no hint, asleep, closed 0]");
        phone.shade.remove("held-by-rust");
        phone.work();
        expect("then, on the worker, each still shown held again",
               phone.asked(), "[end dropped, no hint, asleep, closed 0, hold noa 1000, hold sioul-mail 1000, hold pills 1000]");
        // One posted just before the hint went, heard just after: a stranger too; later or posted after, not.
        phone.said.clear();
        phone.shade.add("late");
        window.posted("late", true, false, T - 10);
        window.posted("after", true, false, T + 500);
        phone.clock = T + QuietWindow.LATE_STRANGER_MS + 1;
        window.posted("too-late", true, false, T - 10);
        phone.work();
        expect("the one posted under the hint", phone.asked(), "[hold late 1000]");

        // What would have rung.
        expect("a sound", QuietWindow.rings(3, true, false, null, false, false), true);
        expect("a silent channel", QuietWindow.rings(2, true, false, "msg", false, false), false);
        expect("held by do-not-disturb", QuietWindow.rings(4, false, false, "msg", false, false), false);
        expect("a call", QuietWindow.rings(4, true, false, "call", false, false), false);
        expect("full screen", QuietWindow.rings(4, true, true, "alarm", false, false), false);
        expect("an alarm's notice with a sound", QuietWindow.rings(4, true, false, "alarm", false, false), true);
        expect("what runs", QuietWindow.rings(4, true, false, "progress", true, false), false);
        expect("a summary", QuietWindow.rings(4, true, false, "msg", false, true), false);

        // Android 10: held exactly, the window two seconds before its own time, nothing held again; one of
        // Sioul's reminders then: no window, the batch rings.
        phone = new Phone();
        window = new QuietWindow(phone, true, true);
        expect("held exactly", window.hold("shop", 120_000), 120_000);
        expect("the alarm before its time", phone.alarm, T + 120_000 - QuietWindow.LEAD_MS);
        phone.snoozed.add("shop");
        phone.clock = T + 120_000 - QuietWindow.LEAD_MS;
        expect("opened", window.rang(), true);
        expect("the hint alone, the end after the lead",
               phone.asked(), "[awake " + (QuietWindow.LEAD_MS + QuietWindow.AT_MOST_MS + QuietWindow.MARGIN_MS) + ", hint, end in " + (QuietWindow.LEAD_MS + QuietWindow.AT_MOST_MS) + "]");
        window.posted("shop", true, false, T + 120_000);
        phone = new Phone();
        window = new QuietWindow(phone, true, true);
        window.hold("shop", 120_000);
        phone.snoozed.add("shop");
        phone.own = new long[] { T + 120_000 };
        phone.clock = T + 120_000 - QuietWindow.LEAD_MS;
        expect("Android 10, a reminder then: no window", window.rang(), false);
        expect("left to ring", window.planned(), 0);
        expect("no hint", phone.asked(), "[]");

        // No window fits before the safety net: the batch left to ring, no alarm.
        phone = new Phone();
        long[] wall = new long[6];
        for (int i = 0; i < wall.length; i++)
            wall[i] = T + i * 60_000L;
        phone.own = wall;
        window = new QuietWindow(phone, true, false);
        window.hold("shop", 0);
        expect("nothing planned", window.planned(), 0);
        expect("no alarm", phone.alarm, -1);

        // Unbound inside a window: Android dropped the hint; nothing asked of it, the end and wake lock let go.
        phone = new Phone();
        window = new QuietWindow(phone, true, false);
        phone.snoozed.add("shop");
        window.hold("shop", 0);
        window.rang();
        phone.said.clear();
        window.dropped();
        expect("forgotten, nothing asked of Android", phone.asked(), "[end dropped, asleep, closed 1]");

        // Where a window opens.
        long length = QuietWindow.AGAIN_MS + QuietWindow.AT_MOST_MS;
        long latest = T + QuietWindow.SAFETY_MS - QuietWindow.BEFORE_SAFETY_MS;
        expect("nothing of Sioul's near", QuietWindow.start(T, new long[] { T - 120_000, T + 600_000 }, length, latest), T);
        expect("a dose at its time", QuietWindow.start(T, new long[] { T }, length, latest), T + 60_000);
        expect("an event 3 s after", QuietWindow.start(T, new long[] { T + 3_000 }, length, latest), T + 63_000);
        expect("one rang 30 s before", QuietWindow.start(T, new long[] { T - 30_000 }, length, latest), T + 30_000);
        expect("two in a row", QuietWindow.start(T, new long[] { T + 62_000, T }, length, latest), T + 122_000);

        // The listener's side, read from the sources.
        String notes = sources("AppNotes.java");
        String manifest = new String(Files.readAllBytes(Paths.get(System.getProperty("sioul.root"), "android", "package", "AndroidManifest.xml")), StandardCharsets.UTF_8);
        expect("the hint asked in one place", notes.split("requestListenerHints\\(", -1).length - 1, 1);
        expect("only the alarm opens a window", notes.split("window\\.rang\\(\\)", -1).length - 1, 1);
        expect("and it is the alarm's", notes.contains("review(\"gathered\");\n        if (!window.rang())"), true);
        expect("the version rule from the security patch", notes.contains("QuietWindow.returnsRing(Build.VERSION.SDK_INT, Build.VERSION.SECURITY_PATCH)"), true);
        expect("a partial wake lock, with a timeout", notes.contains("PowerManager.PARTIAL_WAKE_LOCK") && notes.contains("awake.acquire(ms)"), true);
        expect("the end on the clock that counts sleep", notes.contains("AlarmManager.ELAPSED_REALTIME_WAKEUP, SystemClock.elapsedRealtime() + ms"), true);
        expect("the end alarm heard", notes.contains("} else if (CLOSE.equals(action)) {\n                listener.window.close();"), true);
        expect("forgotten when unbound", notes.contains("window.dropped();\n        worker.post(this::unwatch);"), true);
        expect("and when destroyed", notes.contains("window.dropped();\n        letGo();\n        unwatch();"), true);
        expect("Sioul's doses never touched", notes.contains("boolean dose = own && channel.startsWith(\"doses\")"), true);
        expect("the doses' record only read", notes.contains("DOSE_RECORD") && !notes.contains("dose-alarms.lock"), true);
        int receiver = manifest.indexOf("com.aurelienpierre.sioul.AppNotes$Asked");
        String declared = receiver < 0 ? "" : manifest.substring(receiver, manifest.indexOf("/>", receiver));
        expect("the alarm's receiver in the listener's process", declared.contains("android:process=\":listener\""), true);
        expect("never exported", declared.contains("android:exported=\"false\""), true);
        expect("the wake lock declared", manifest.contains("android.permission.WAKE_LOCK"), true);
        // A batch Android's safety net brings back still comes with its gathering (Rust's appnotes::LATE).
        String core = new String(Files.readAllBytes(Paths.get(System.getProperty("sioul.root"), "crates", "sioul-core", "src", "appnotes.rs")), StandardCharsets.UTF_8);
        Matcher late = Pattern.compile("pub const LATE: i64 = (\\d+) \\* 60;").matcher(core);
        boolean found = late.find();
        expect("Rust's LATE read", found, true);
        long lateMs = found ? Long.parseLong(late.group(1)) * 60_000L : 0;
        expect("the safety net within Rust's LATE", lateMs > QuietWindow.SAFETY_MS + QuietWindow.BEFORE_SAFETY_MS, true);

        System.out.println("QuietCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
