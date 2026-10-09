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
 * - StepService.othersFile: which files written in the sharing folder bring
 *   a step (another device's; never a hidden one, never this phone's own);
 * - Texts.changedSince: whether the import asks Android (the texts watched in
 *   the service's process, as marked; not watched: always).
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

        System.out.println("StepsCheck: " + (checked - failed) + " of " + checked + " checks passed");
        if (failed > 0)
            System.exit(1);
    }
}
