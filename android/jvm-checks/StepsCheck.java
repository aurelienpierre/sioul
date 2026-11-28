// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.util.concurrent.atomic.AtomicBoolean;

/**
 * The background service's decisions on the JVM (docs/android.md, "In the
 * background"):
 * - StepService.Later.of: a step asked for later, by its own alarm, joined
 *   to the one waiting: a folder's moved on by each file written, never in
 *   place of a message's; a message's joining the one waiting at its time;
 * - StepService.unlockedDelay: the step at an unlock while the faster fetch
 *   for codes is on: none while off or a step is queued, now two minutes or
 *   more after the last one, else two minutes after it (an unlock thirty
 *   seconds after a step still has its step, at ninety seconds);
 * - StepService.othersFile: which files written in the sharing folder bring
 *   a step (another device's; never a hidden one, never this phone's own);
 * - Texts.changedSince: whether the import asks Android (the texts watched in
 *   the service's process, as marked; not watched: always);
 * - Texts.grew: whether a message was received since the watch last looked
 *   (an inbox's newest id grown), which brings a step soon;
 * - Texts.newestBy: an inbox's newest id asked with the limit in the sort
 *   (Android's own providers pass no QUERY_ARG_LIMIT on), the plain sort
 *   from a provider that refuses it, -1 when neither is taken.
 */
public final class StepsCheck
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

    static String later(StepService.Later waiting, String asked, long delay, long now)
    {
        StepService.Later next = StepService.Later.of(waiting, asked, delay, now);
        return next.reason + "@" + next.at;
    }

    public static void main(String[] args)
    {
        // The unlock's step (the review of 28 November 2026, F1), the faster fetch on or off.
        long two = StepService.UNLOCKED_EVERY_MS;
        expect("off: no step at the unlock", StepService.unlockedDelay(false, false, 100_000, 100_000 + 3 * two), -1L);
        expect("a step queued: it reads the unlock's time", StepService.unlockedDelay(true, true, 100_000, 130_000), -1L);
        expect("thirty seconds after a step: ninety seconds on", StepService.unlockedDelay(true, false, 100_000, 130_000), two - 30_000);
        expect("two minutes after a step: now", StepService.unlockedDelay(true, false, 100_000, 100_000 + two), 0L);
        expect("no step yet: now", StepService.unlockedDelay(true, false, 0, 5_000), 0L);
        expect("a clock before the last step: now", StepService.unlockedDelay(true, false, 100_000, 50_000), 0L);
        // None waiting: the step comes `delay` on.
        expect("a folder's, alone", later(null, "folder", 3_000, 100_000), "folder@103000");
        expect("a message's, alone", later(null, "messages", 20_000, 100_000), "messages@120000");
        // A folder's moved on by each file written (the sync app's writing settles first).
        StepService.Later folder = new StepService.Later("folder", 103_000);
        expect("another file written", later(folder, "folder", 3_000, 102_000), "folder@105000");
        // A message's line never waits behind a folder's step, which Rust may leave out:
        // it joins the folder's, at its time, as a message's step.
        expect("a message while a folder's waits", later(folder, "messages", 20_000, 101_000), "messages@103000");
        // A folder's file while a message's waits: the message's step comes then, and shares the line.
        StepService.Later message = new StepService.Later("messages", 120_000);
        expect("a file while a message's waits", later(message, "folder", 3_000, 110_000), "messages@113000");
        // The messages that follow go with the first one's step.
        expect("another message", later(message, "messages", 20_000, 105_000), "messages@120000");

        String own = "7c41d09e";
        expect("a computer's round", StepService.othersFile("a1b2c3d4-12.jsonl", own), true);
        expect("a computer's entry", StepService.othersFile("a1b2c3d4.toml", own), true);
        expect("this phone's own", StepService.othersFile("7c41d09e-3.jsonl", own), false);
        expect("a file being written", StepService.othersFile(".a1b2c3d4-12.jsonl.part", own), false);
        expect("no name", StepService.othersFile(null, own), false);
        expect("an empty name", StepService.othersFile("", own), false);
        expect("own not known yet", StepService.othersFile("7c41d09e-3.jsonl", ""), true);

        AtomicBoolean mark = new AtomicBoolean(true);
        expect("not watched: always", Texts.changedSince(false, new AtomicBoolean(false), true), true);
        expect("watched, changed", Texts.changedSince(true, mark, true), true);
        expect("read: cleared", Texts.changedSince(true, mark, true), false);
        mark.set(true);
        expect("asked without clearing", Texts.changedSince(true, mark, false), true);
        expect("still marked", mark.get(), true);

        // A message received: an inbox's newest id grown, since a look that read it.
        expect("a new message", Texts.grew(41, 42), true);
        expect("the same newest", Texts.grew(42, 42), false);
        expect("one deleted since", Texts.grew(42, 40), false);
        expect("never read before: the first look brings no step", Texts.grew(-1, 42), false);
        expect("an inbox that cannot be read now", Texts.grew(42, -1), false);
        expect("the first message in an empty inbox", Texts.grew(0, 1), true);

        // The newest id: the limit in the sort first; refused, the plain sort; neither, unread.
        java.util.List<String> asked = new java.util.ArrayList<>();
        expect("the limit taken", Texts.newestBy(order -> { asked.add(order); return 42; }), 42);
        expect("asked once, the limit in the sort", asked, "[_id DESC LIMIT 1]");
        asked.clear();
        expect("the limit refused: the plain sort", Texts.newestBy(order -> {
            asked.add(order);
            if (order.contains("LIMIT"))
                throw new IllegalArgumentException("Invalid token LIMIT");
            return 41;
        }), 41);
        expect("then the plain sort", asked, "[_id DESC LIMIT 1, _id DESC]");
        expect("an empty inbox", Texts.newestBy(order -> 0), 0);
        expect("neither taken: unread", Texts.newestBy(order -> { throw new SecurityException("no permission"); }), -1);

        System.out.println("StepsCheck: " + (checked - failed) + " of " + checked + " checks passed");
        if (failed > 0)
            System.exit(1);
    }
}
