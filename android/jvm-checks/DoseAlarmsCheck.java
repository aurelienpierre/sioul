// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import org.json.JSONArray;
import org.json.JSONObject;

import java.util.Map;
import java.util.TreeMap;

/**
 * What Android holds of the doses' alarms, decided on the JVM
 * (DoseAlarms.Plan; the check of 1b6795e, H1 and M6). Two processes of Sioul
 * (the window's and :steps') hand Android the doses in turn, each from the
 * one record they share: whichever hands, in whichever order, Android holds
 * what the last hand listed; none cancelled for another's old list, none
 * doubled, none missed. A dose its receiver asked again later (sleep) is kept
 * at that time by a hand that still lists its dose; a dose past, never
 * armed, is left alone; a process's first hand arms again what is to come,
 * and what is due within the half hour (the chain check of e3505f7, low 24),
 * never one that rang, whose decision may be running (the A4 check, #0 and
 * #22); a dose that replaced one whose reminder was taken away as gone, past
 * the half hour, is armed once, at once (the A4 check, #1).
 */
public final class DoseAlarmsCheck
{
    static int failed = 0;
    static int checked = 0;
    static final long MIN = 60_000L;

    static void expect(String what, Object got, Object wanted)
    {
        checked++;
        if (!String.valueOf(got).equals(String.valueOf(wanted))) {
            failed++;
            System.out.println("FAIL " + what + ": " + got + " (expected " + wanted + ")");
        }
    }

    /** Android's alarms (key → time), the record both processes share, and each process's own flag. */
    static final class Phone
    {
        final Map<String, Long> android = new TreeMap<>();
        String record = "[]";
    }

    /** The processes of Sioul's that run (their pids), as Android would list them. */
    static final java.util.Set<Integer> RUNNING = new java.util.HashSet<>();
    static int pids;

    static final class Process
    {
        boolean armedHere;
        final int pid = ++pids;

        Process()
        {
            RUNNING.add(pid);
        }

        /** Killed, or crashed: its decisions with it. */
        void dies()
        {
            RUNNING.remove(pid);
        }
    }

    static JSONArray doses(Object... keysAndTimes) throws Exception
    {
        JSONArray out = new JSONArray();
        for (int i = 0; i < keysAndTimes.length; i += 2) {
            JSONObject dose = new JSONObject();
            dose.put("key", keysAndTimes[i]);
            dose.put("at", keysAndTimes[i + 1]);
            dose.put("title", "Lumiel");
            out.put(dose);
        }
        return out;
    }

    /** A hand, as DoseAlarms.set makes it, the record's lock held. */
    static void hand(Phone phone, Process process, JSONArray doses, long now) throws Exception
    {
        DoseAlarms.Plan plan = DoseAlarms.Plan.of(new JSONArray(phone.record), doses, now, process.armedHere, (pid, boot) -> boot == 1 && RUNNING.contains(pid));
        for (String key : plan.cancel)
            phone.android.remove(key);
        for (JSONObject dose : plan.arm)
            phone.android.put(dose.optString("key"), dose.optLong("at"));
        phone.record = plan.record.toString();
        process.armedHere = true;
    }

    /** An alarm that rang at `at`, decided by `by`: out of Android's hands, noted in the record (DoseReceiver, DoseAlarms.rang). */
    static void rings(Phone phone, String key, long at, Process by) throws Exception
    {
        phone.android.remove(key);
        phone.record = DoseAlarms.Plan.rangIn(new JSONArray(phone.record), key, at, by.pid, 1).toString();
    }

    /** Rust's tag on a dose that replaced a gone one (`replaces`). */
    static JSONArray tagged(JSONArray doses, String key, String gone) throws Exception
    {
        for (int i = 0; i < doses.length(); i++)
            if (key.equals(doses.getJSONObject(i).optString("key")))
                doses.getJSONObject(i).put("replaces", gone);
        return doses;
    }

    /** A receiver's asking again later, as DoseAlarms.again makes it. */
    static void again(Phone phone, String key, long at) throws Exception
    {
        phone.android.put(key, at);
        phone.record = DoseAlarms.Plan.againIn(new JSONArray(phone.record), key, at, "Lumiel").toString();
    }

    public static void main(String[] args) throws Exception
    {
        long h19 = 19 * 60 * MIN, h20 = 20 * 60 * MIN;
        JSONArray x = doses("lumiel@20", h20, "corvan@22", h20 + 120 * MIN);
        JSONArray y = doses("corvan@22", h20 + 120 * MIN);
        // Case A of the check: :steps hands X; the window hands Y (a mistaken Taken); the take
        // back comes, and :steps lists X again: Android holds X again, whatever it handed before.
        for (boolean stepsFirst : new boolean[] { true, false }) {
            Phone phone = new Phone();
            Process window = new Process(), steps = new Process();
            Process first = stepsFirst ? steps : window, second = stepsFirst ? window : steps;
            hand(phone, first, x, h19);
            hand(phone, second, y, h19 + MIN);
            expect("taken by mistake: its alarm cancelled, " + (stepsFirst ? "steps first" : "window first"), phone.android.keySet(), "[corvan@22]");
            hand(phone, first, x, h19 + 12 * MIN);
            expect("taken back: armed again by the process that handed it first", phone.android, "{corvan@22=" + (h20 + 120 * MIN) + ", lumiel@20=" + h20 + "}");
            hand(phone, second, x, h19 + 13 * MIN);
            expect("the other hand changes nothing", phone.android.size(), 2);
        }
        // M6: a dose held by sleep, asked again at waking, kept by a hand made from its own time.
        Phone phone = new Phone();
        Process steps = new Process();
        long h2330 = 23 * 60 * MIN + 30 * MIN, h0700 = 31 * 60 * MIN, h0040 = 24 * 60 * MIN + 40 * MIN;
        hand(phone, steps, doses("corvan@2330", h2330), 23 * 60 * MIN);
        again(phone, "corvan@2330", h0700);
        hand(phone, steps, doses("corvan@2330", h2330), h0040);
        expect("asked again at waking: kept", phone.android, "{corvan@2330=" + h0700 + "}");
        hand(phone, steps, doses(), h0040 + MIN);
        expect("marked meanwhile (no longer listed): cancelled", phone.android, "{}");
        // A dose past by more than half an hour, never armed: left alone (no alarm ringing at once, again and again).
        Phone late = new Phone();
        hand(late, new Process(), doses("lumiel@20", h20), h20 + 45 * MIN);
        expect("past, never armed: none", late.android, "{}");
        hand(late, new Process(), doses("lumiel@20", h20), h20 + 10 * MIN);
        expect("ten minutes past: armed, it rings now", late.android, "{lumiel@20=" + h20 + "}");
        // A process's first hand arms again what is to come (a stop by force forgets Android's alarms).
        Phone stopped = new Phone();
        hand(stopped, new Process(), x, h19);
        stopped.android.clear();
        hand(stopped, new Process(), x, h19 + MIN);
        expect("its first hand: armed again", stopped.android.size(), 2);
        // A force stop at 07:55, a dose at 08:00, Sioul opened at 08:10: armed again, it rings at once;
        // opened past the half hour, left to the question on doses due while Sioul was closed.
        long h0755 = 7 * 60 * MIN + 55 * MIN, h0800 = 8 * 60 * MIN;
        Phone forced = new Phone();
        hand(forced, new Process(), doses("corvan@0800", h0800), h0755);
        forced.android.clear();
        hand(forced, new Process(), doses("corvan@0800", h0800), h0800 + 10 * MIN);
        expect("forced to stop, opened within the half hour: armed, it rings at once", forced.android, "{corvan@0800=" + h0800 + "}");
        Phone longer = new Phone();
        hand(longer, new Process(), doses("corvan@0800", h0800), h0755);
        longer.android.clear();
        hand(longer, new Process(), doses("corvan@0800", h0800), h0800 + 45 * MIN);
        expect("opened past the half hour: none rings at once", longer.android, "{}");
        // Corvan and Lumiel both at 08:00, Sioul closed: both alarms ring in a new process; Corvan's
        // decision shows first and hands (the process's first hand) while Lumiel's is still decided:
        // Lumiel is not armed again (it would be decided twice, its reminder replaced by the fallback).
        Phone both = new Phone();
        hand(both, new Process(), doses("corvan@0800", h0800, "lumiel@0800", h0800), h0755);
        Process fresh = new Process();
        rings(both, "corvan@0800", h0800, fresh);
        rings(both, "lumiel@0800", h0800, fresh);
        hand(both, fresh, doses("lumiel@0800", h0800), h0800 + 5_000);
        expect("rung, decided still: not armed again by a first hand", both.android, "{}");
        expect("rung, kept in the record until reminded", new JSONArray(both.record).length(), 1);
        // Lumiel's decision runs longer than a minute (a slow pull, the exchange's lock): its
        // process is there, and no hand arms it again (the A6 check, L2: it was decided twice).
        hand(both, new Process(), doses("lumiel@0800", h0800), h0800 + 2 * MIN);
        expect("a decision longer than a minute: not decided twice", both.android, "{}");
        // Then its process dies (a crash, a kill): the next hand, a first one or not, arms it
        // again, its mark dropped, and it rings (the A5 check, L1).
        fresh.dies();
        hand(both, new Process(), doses("lumiel@0800", h0800), h0800 + 3 * MIN);
        expect("rung, its decider gone: armed again", both.android.get("lumiel@0800"), h0800);
        expect("its mark dropped", new JSONArray(both.record).getJSONObject(0).has(DoseAlarms.RANG), false);
        // The force stop's dose, never rung, still rings at once at the next opening (above).
        // A correction typed at 14:50 moves Corvan's 14:00 to 14:10, its reminder taken away as gone:
        // corvan@14:10, past the half hour, armed once at once; the next hand arms it no more.
        long h1410 = 14 * 60 * MIN + 10 * MIN, h1452 = 14 * 60 * MIN + 52 * MIN;
        Phone moved = new Phone();
        Process stepsHand = new Process();
        hand(moved, stepsHand, tagged(doses("corvan@1410", h1410, "corvan@2010", h1410 + 6 * 60 * MIN), "corvan@1410", "corvan@1400"), h1452);
        expect("the dose replacing a gone one, past the half hour: armed at once", moved.android.get("corvan@1410"), h1452);
        moved.android.remove("corvan@1410");
        hand(moved, stepsHand, tagged(doses("corvan@1410", h1410, "corvan@2010", h1410 + 6 * 60 * MIN), "corvan@1410", "corvan@1400"), h1452 + MIN);
        expect("armed once: not again at the next hand", moved.android.containsKey("corvan@1410"), false);
        hand(moved, new Process(), doses("corvan@2010", h1410 + 6 * 60 * MIN), h1452 + 2 * MIN);
        expect("reminded since (no longer listed): out of the record", new JSONArray(moved.record).length(), 1);
        System.out.println(failed == 0 ? "DoseAlarmsCheck: " + checked + " checks passed." : "DoseAlarmsCheck: " + failed + " of " + checked + " failed.");
        System.exit(failed == 0 ? 0 : 1);
    }
}
