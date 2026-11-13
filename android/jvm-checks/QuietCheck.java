// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
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
 * docs/research/android-holding.md): what Sioul gathered comes back under
 * Android's hint that silences notifications, a few seconds at most. The
 * hint set before anything is held again, each released key held again for
 * a second (Android 11 and later) or left to come at its own time (9 and
 * 10); the hint taken off once each came back, or at the window's end, the
 * alarm's broadcast let end then; a stranger posted inside it that would
 * have rung held again for a second after the hint is off, never one Sioul
 * held meanwhile, one that would not ring, an update alerting once, a dose;
 * a second release joining an open window; the window forgotten when the
 * listener is unbound, nothing asked of Android. Then where a window opens,
 * off Sioul's own reminders, and the plan of what comes back quiet. Then,
 * read from the sources, the listener's side: the hint asked in one place,
 * taken off on every way out, the alarm's receiver in the listener's process
 * and never exported, the doses never touched.
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

    /** Android and the listener as the window sees them: what it asked, in order; its end, run by hand. */
    static final class Phone implements QuietWindow.Host
    {
        final List<String> said = new ArrayList<>();
        final Set<String> shade = new HashSet<>();
        Runnable end;
        long endIn = -1;
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
        public void after(Runnable task, long ms)
        {
            end = task;
            endIn = ms;
        }

        @Override
        public void cancel(Runnable task)
        {
            if (task == end) {
                end = null;
                endIn = -1;
            }
        }

        @Override
        public void closed(int left)
        {
            closed = left;
            said.add("closed " + left);
        }

        /** The window's end comes. */
        void time()
        {
            Runnable task = end;
            end = null;
            if (task != null)
                task.run();
        }
    }

    static String sources(String name) throws IOException
    {
        Path file = Paths.get(System.getProperty("sioul.root"), "android", "package", "src", "com", "aurelienpierre", "sioul", name);
        return new String(Files.readAllBytes(file), StandardCharsets.UTF_8);
    }

    public static void main(String[] args) throws IOException
    {
        // The gathering at 13:00 on Android 11 and later: the hint first, then each held again a second.
        Phone phone = new Phone();
        QuietWindow window = new QuietWindow(phone);
        expect("nothing to release, no window", window.open(List.of(), true, QuietWindow.AT_MOST_MS, List.of()), false);
        expect("no hint for nothing", phone.said, "[]");
        expect("opened", window.open(List.of("shop", "news"), true, QuietWindow.AT_MOST_MS, List.of()), true);
        expect("the hint, then each held again", phone.said, "[hint, hold shop 1000, hold news 1000]");
        expect("its end in five seconds", phone.endIn, 5000);
        window.posted("shop", true, false);
        expect("one came back: still open", window.isOpen(), true);
        window.posted("news", true, false);
        expect("each came back: closed", window.isOpen(), false);
        expect("the hint taken off, the broadcast let end", phone.said, "[hint, hold shop 1000, hold news 1000, no hint, closed 0]");
        expect("its end dropped", phone.end == null, true);
        phone.time();
        expect("closed once", phone.said.size(), 5);

        // One never comes back (its app took it away): the hint off at the window's end, said.
        phone = new Phone();
        window = new QuietWindow(phone);
        window.open(List.of("shop", "gone"), true, QuietWindow.AT_MOST_MS, List.of());
        window.posted("shop", true, false);
        phone.time();
        expect("closed at its end", window.isOpen(), false);
        expect("one not come back", phone.closed, 1);
        expect("the hint off", phone.said.get(phone.said.size() - 2), "no hint");

        // Strangers inside the window: a person's message rings after it, once the hint is off;
        // one Sioul held meanwhile keeps its time; one that would not ring, and an update alerting
        // once of one shown as the window opened, are left as they are.
        phone = new Phone();
        window = new QuietWindow(phone);
        phone.shade.addAll(List.of("noa", "update", "sioul-mail"));
        window.open(List.of("shop"), true, QuietWindow.AT_MOST_MS, List.of("update"));
        window.posted("noa", QuietWindow.rings(4, true, false, "msg", false), false);
        window.posted("held-by-rust", QuietWindow.rings(3, true, false, "msg", false), false);
        window.posted("silent", QuietWindow.rings(2, true, false, "promo", false), false);
        window.posted("update", QuietWindow.rings(4, true, false, "msg", false), true);
        window.posted("sioul-mail", QuietWindow.rings(3, true, false, "email", false), false);
        expect("still open for the shop's", window.isOpen(), true);
        window.posted("shop", true, false);
        expect("the hint off first, then the strangers rung again",
               phone.said, "[hint, hold shop 1000, no hint, hold noa 1000, hold sioul-mail 1000, closed 0]");
        window.posted("late", true, false);
        expect("closed: nothing more heard", phone.said.size(), 6);

        // What would have rung: a channel with a sound, through do-not-disturb; never what runs, a call, an alarm.
        expect("a sound", QuietWindow.rings(3, true, false, null, false), true);
        expect("a high one", QuietWindow.rings(4, true, false, "msg", false), true);
        expect("a silent channel", QuietWindow.rings(2, true, false, "msg", false), false);
        expect("held by do-not-disturb", QuietWindow.rings(4, false, false, "msg", false), false);
        expect("a call", QuietWindow.rings(4, true, true, "call", false), false);
        expect("an alarm", QuietWindow.rings(4, true, false, "alarm", false), false);
        expect("a full screen", QuietWindow.rings(4, true, true, null, false), false);
        expect("what runs", QuietWindow.rings(4, true, false, "progress", true), false);

        // Android 9 and 10: nothing held again (it would do nothing there); the window waits for their own time.
        phone = new Phone();
        window = new QuietWindow(phone);
        window.open(List.of("shop"), false, QuietWindow.LEAD_MS + QuietWindow.AT_MOST_MS, List.of());
        expect("the hint alone", phone.said, "[hint]");
        expect("its end after the lead and five seconds", phone.endIn, 7000);
        window.posted("shop", true, false);
        expect("closed as it came", phone.said, "[hint, no hint, closed 0]");

        // A second release while open: its keys join, its end moves, the hint asked once.
        phone = new Phone();
        window = new QuietWindow(phone);
        window.open(List.of("shop"), true, QuietWindow.AT_MOST_MS, List.of());
        window.open(List.of("noa"), true, QuietWindow.AT_MOST_MS, List.of());
        expect("one hint", phone.said, "[hint, hold shop 1000, hold noa 1000]");
        window.posted("shop", true, false);
        expect("waits for the second", window.isOpen(), true);
        window.posted("noa", true, false);
        expect("then closes", phone.said, "[hint, hold shop 1000, hold noa 1000, no hint, closed 0]");

        // The listener unbound inside a window: Android dropped the hint; nothing asked of it, the broadcast let end.
        phone = new Phone();
        window = new QuietWindow(phone);
        phone.shade.add("noa");
        window.open(List.of("shop"), true, QuietWindow.AT_MOST_MS, List.of());
        window.posted("noa", true, false);
        window.dropped();
        expect("forgotten, nothing asked", phone.said, "[hint, hold shop 1000, closed 1]");
        expect("its end dropped", phone.end == null, true);
        window.dropped();
        window.close();
        expect("forgotten once", phone.said.size(), 3);

        // Where a window opens: at its time, or past one of Sioul's reminders (each may sound a minute after its time).
        long due = 1_000_000_000L;
        long length = QuietWindow.AGAIN_MS + QuietWindow.AT_MOST_MS;
        long latest = due + QuietWindow.SAFETY_MS - 30_000;
        expect("nothing of Sioul's near", QuietWindow.start(due, new long[] { due - 120_000, due + 600_000 }, length, latest), due);
        expect("a dose at its time", QuietWindow.start(due, new long[] { due }, length, latest), due + 60_000);
        expect("an event's reminder 3 s after", QuietWindow.start(due, new long[] { due + 3_000 }, length, latest), due + 63_000);
        expect("one rang 30 s before, still deciding", QuietWindow.start(due, new long[] { due - 30_000 }, length, latest), due + 30_000);
        expect("one long past", QuietWindow.start(due, new long[] { due - 61_000 }, length, latest), due);
        expect("one after the window", QuietWindow.start(due, new long[] { due + length }, length, latest), due);
        expect("two in a row, in any order", QuietWindow.start(due, new long[] { due + 62_000, due }, length, latest), due + 122_000);
        long[] wall = new long[6];
        for (int i = 0; i < wall.length; i++)
            wall[i] = due + i * 60_000L;
        expect("none fits before the safety net", QuietWindow.start(due, wall, length, latest), -1);
        expect("Android 10: it cannot move", QuietWindow.start(due - QuietWindow.LEAD_MS, new long[] { due }, QuietWindow.LEAD_MS + QuietWindow.AT_MOST_MS, due - QuietWindow.LEAD_MS), -1);
        expect("Android 10: nothing near", QuietWindow.start(due - QuietWindow.LEAD_MS, new long[] {}, QuietWindow.LEAD_MS + QuietWindow.AT_MOST_MS, due - QuietWindow.LEAD_MS), due - QuietWindow.LEAD_MS);

        // The plan: each one's time; what is due, in order, taken out; the next.
        QuietWindow.Plan plan = new QuietWindow.Plan();
        expect("nothing planned", plan.next(), -1);
        plan.put("news", due + 1_000);
        plan.put("shop", due);
        plan.put("later", due + 3_600_000);
        expect("the soonest", plan.next(), due);
        expect("due, in order, within a second", plan.due(due, QuietWindow.DUE_MS), "[shop, news]");
        expect("taken out", plan.size(), 1);
        expect("the next", plan.next(), due + 3_600_000);
        plan.remove("later");
        expect("came back by itself", plan.next(), -1);
        // Rust's list at a review: Android 11 and later take its times, whatever was planned.
        plan.put("shop", due);
        plan.put("gone", due);
        Map<String, Long> rust = new TreeMap<>();
        rust.put("shop", due + 60_000);
        rust.put("released", due - 5_000);
        plan.follow(rust, false, due - 10_000);
        expect("Rust's times", plan.of("shop") + " " + plan.of("released") + " " + plan.of("gone"), (due + 60_000) + " " + (due - 5_000) + " -1");
        // Android 9 and 10: the time known stays (a held one keeps it there); one not known only when to come.
        plan = new QuietWindow.Plan();
        plan.put("shop", due);
        plan.put("gone", due);
        plan.follow(rust, true, due);
        expect("fixed times", plan.of("shop") + " " + plan.of("released") + " " + plan.of("gone"), due + " -1 -1");

        // The listener's side, read from the sources.
        String notes = sources("AppNotes.java");
        String quiet = sources("QuietWindow.java");
        String manifest = new String(Files.readAllBytes(Paths.get(System.getProperty("sioul.root"), "android", "package", "AndroidManifest.xml")), StandardCharsets.UTF_8);
        expect("the hint asked in one place", notes.split("requestListenerHints\\(", -1).length - 1, 1);
        expect("set or taken off there", notes.contains("requestListenerHints(quiet ? HINT_HOST_DISABLE_NOTIFICATION_EFFECTS : 0)"), true);
        expect("the hint taken off before the strangers ring again", quiet.indexOf("host.hint(false)") < quiet.indexOf("host.snooze(key, AGAIN_MS);\n            strangers.clear()"), true);
        expect("taken off in a finally", quiet.contains("host.hint(false);\n        } finally {"), true);
        expect("forgotten when unbound", notes.contains("window.dropped();\n        worker.post(this::unwatch);"), true);
        expect("and when destroyed", notes.contains("window.dropped();\n        letGo();\n        unwatch();"), true);
        expect("the window's end on Android's main thread", notes.contains("main.postDelayed(task, ms)"), true);
        expect("a gathered one held past its time, the safety net", notes.contains("snooze(key, fixed() ? ms : ms + QuietWindow.SAFETY_MS)"), true);
        expect("Sioul's doses never touched", notes.contains("boolean dose = own && channel.startsWith(\"doses\")"), true);
        expect("the doses' record only read", notes.contains("DOSE_RECORD") && !notes.contains("DoseAlarms.put(") && !notes.contains("dose-alarms.lock"), true);
        int receiver = manifest.indexOf("com.aurelienpierre.sioul.AppNotes$Asked");
        String declared = receiver < 0 ? "" : manifest.substring(receiver, manifest.indexOf("/>", receiver));
        expect("the alarm's receiver declared", receiver > 0, true);
        expect("in the listener's process", declared.contains("android:process=\":listener\""), true);
        expect("never exported", declared.contains("android:exported=\"false\""), true);
        expect("its actions checked", notes.contains("if (RELEASE.equals(action)) {") && notes.contains("} else if (LINKED.equals(action)) {"), true);
        expect("the safety net longer than the window can move", QuietWindow.SAFETY_MS > 2 * QuietWindow.BUSY_MS, true);
        // A batch Android's safety net brings back still comes with its gathering (Rust's appnotes::LATE).
        String core = new String(Files.readAllBytes(Paths.get(System.getProperty("sioul.root"), "crates", "sioul-core", "src", "appnotes.rs")), StandardCharsets.UTF_8);
        Matcher late = Pattern.compile("pub const LATE: i64 = (\\d+) \\* 60;").matcher(core);
        boolean found = late.find();
        expect("Rust's LATE read", found, true);
        long lateMs = found ? Long.parseLong(late.group(1)) * 60_000L : 0;
        expect("the safety net within Rust's LATE", lateMs > QuietWindow.SAFETY_MS + 30_000, true);

        System.out.println("QuietCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
