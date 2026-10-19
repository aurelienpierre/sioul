// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.time.ZoneId;
import java.time.ZonedDateTime;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

/**
 * Calls.decideFor on the cases of calls.rs's `who_rings_when_and_what_always_rings`
 * (a table of version 1, an older Sioul's: still read in its own order), then
 * of `the_screening_decides_numbers_nobody_knows_apart` (version 2: the call
 * screening apart). Fiction numbers only: 02 61 91 stands for an automated
 * caller's root, 03 53 01 for a block attributed to nobody.
 */
public final class DecideCheck
{
    static long ms(String day, String time)
    {
        return ZonedDateTime.of(java.time.LocalDate.parse(day), java.time.LocalTime.parse(time), ZoneId.of("Europe/Paris")).toInstant().toEpochMilli();
    }

    static Calls.Frame frame(long from, long until, String column, String... ringing)
    {
        Calls.Frame f = new Calls.Frame();
        f.from = from;
        f.until = until;
        f.column = column;
        Set<String> on = new HashSet<>(Arrays.asList(ringing));
        for (String row : new String[] { "safe", "neutral", "restricted", "stranger", "hidden", "always-stranger" })
            f.ring.put(row, on.contains(row));
        f.hours = column.equals("work") || column.equals("admin");
        return f;
    }

    static void expectWho(String what, Calls.Decision d, boolean refuse, String why, String who)
    {
        expect(what, d, refuse, why);
        if (!d.who.equals(who)) {
            failed++;
            System.out.println("FAIL " + what + ": who " + d.who + " (expected " + who + ")");
        }
    }

    /** The screening as usual, with fiction roots for its kinds. */
    static Calls.Screening screening()
    {
        Calls.Screening s = new Calls.Screening();
        s.calledMs = 14L * 24 * 3600 * 1000;
        s.automatedPrefixes.add("026191");
        s.impossiblePrefixes.addAll(Arrays.asList("035301", "089"));
        s.longest = 6;
        return s;
    }

    static int failed = 0;

    static void expect(String what, Calls.Decision d, boolean refuse, String why)
    {
        if (d.refuse != refuse || !d.why.equals(why)) {
            failed++;
            System.out.println("FAIL " + what + ": " + d.refuse + " " + d.why + " (expected " + refuse + " " + why + ")");
        }
    }

    public static void main(String[] args)
    {
        Calls.Table t = new Calls.Table();
        t.region = new Calls.Region();
        t.region.calling = "33"; t.region.trunk = "0"; t.region.international = "00"; t.region.fewest = 10; t.region.most = 10; t.region.french = true;
        t.trunkZero.addAll(Arrays.asList("33", "32", "44", "49"));
        t.numbers.put("+33199000001", "safe");
        t.numbers.put("+33199000002", "neutral");
        t.numbers.put("+33199000003", "restricted");
        t.numbers.put("+33199000004", "blocked");
        t.prefixes.add(new String[] { "+3346571", "blocked" });
        t.emergency.addAll(Arrays.asList("112", "15", "114", "3114", "+33800112112"));
        t.people.add("+33199000009");
        String day = "2026-10-05";
        long work0 = ms(day, "09:00"), work1 = ms(day, "17:00"), leisure1 = ms(day, "22:00"), sleep1 = ms("2026-10-06", "07:00");
        t.frames.add(frame(ms(day, "00:00"), work0, "sleep"));
        t.frames.add(frame(work0, work1, "work", "safe", "neutral", "restricted", "hidden"));
        t.frames.add(frame(work1, leisure1, "leisure", "safe"));
        t.frames.add(frame(leisure1, sleep1, "sleep"));
        Calls.Phone none = new Calls.Phone() {
            public boolean emergency(String raw) { return false; }
            public String contact(String raw) { return null; }
        };
        Calls.Phone contact = new Calls.Phone() {
            public boolean emergency(String raw) { return false; }
            public String contact(String raw) { return "Someone"; }
        };
        Calls.Phone android17 = new Calls.Phone() {
            public boolean emergency(String raw) { return raw.equals("17"); }
            public String contact(String raw) { return null; }
        };
        Object[][] matrix = {
            { "01 99 00 00 01", "10:00", false }, { "01 99 00 00 01", "18:00", false }, { "01 99 00 00 01", "23:00", true },
            { "01 99 00 00 02", "10:00", false }, { "01 99 00 00 02", "18:00", true },
            { "+33 1 99 00 00 03", "10:00", false }, { "0199000003", "18:00", true },
            { "01 99 00 55 55", "10:00", true }, { "", "10:00", false }, { "", "18:00", true },
        };
        for (Object[] c : matrix) {
            String number = (String) c[0];
            expect(number + " at " + c[1], Calls.decideFor(t, number, number.isEmpty(), ms(day, (String) c[1]), 0, null, 0, none), (Boolean) c[2], "matrix");
        }
        expect("blocked", Calls.decideFor(t, "01 99 00 00 04", false, ms(day, "10:00"), 0, null, 0, none), true, "blocked");
        expect("blocked prefix", Calls.decideFor(t, "04 65 71 12 34", false, ms(day, "10:00"), 0, null, 0, none), true, "blocked");
        expect("blocked again", Calls.decideFor(t, "01 99 00 00 04", false, ms(day, "10:00"), 0, null, ms(day, "09:55"), none), true, "blocked");
        expect("phone contact at work", Calls.decideFor(t, "01 99 00 55 55", false, ms(day, "10:00"), 0, null, 0, contact), false, "matrix");
        expect("phone contact at leisure", Calls.decideFor(t, "01 99 00 55 55", false, ms(day, "18:00"), 0, null, 0, contact), true, "matrix");
        long first = ms(day, "18:00");
        expect("repeat", Calls.decideFor(t, "01 99 00 55 55", false, ms(day, "18:10"), 0, null, first, none), false, "repeat");
        expect("after 15", Calls.decideFor(t, "01 99 00 55 55", false, ms(day, "18:16"), 0, null, first, none), true, "matrix");
        expect("hidden repeat", Calls.decideFor(t, "", true, ms(day, "18:10"), 0, null, first, none), true, "matrix");
        for (String n : new String[] { "112", "15", "0 800 112 112", "+33800112112" })
            expect("emergency " + n, Calls.decideFor(t, n, false, ms(day, "23:00"), 0, null, 0, none), false, "emergency");
        expect("17 by Android", Calls.decideFor(t, "17", false, ms(day, "23:00"), 0, null, 0, android17), false, "emergency");
        expect("17 unknown", Calls.decideFor(t, "17", false, ms(day, "23:00"), 0, null, 0, none), true, "matrix");
        expect("people", Calls.decideFor(t, "01 99 00 00 09", false, ms(day, "23:00"), 0, null, 0, none), false, "people");
        long called = ms(day, "03:00");
        expect("after emergency, blocked", Calls.decideFor(t, "01 99 00 00 04", false, ms(day, "23:00"), called, null, 0, none), false, "after-emergency");
        expect("a day later", Calls.decideFor(t, "01 99 00 55 55", false, ms(day, "23:00"), called - 86_400_000L, null, 0, none), true, "matrix");
        Calls.Press on = new Calls.Press(); on.pressed = 10; on.on = true; on.until = 0;
        t.through = on;
        expect("through", Calls.decideFor(t, "01 99 00 55 55", false, ms(day, "23:00"), 0, null, 0, none), false, "through");
        expect("through, blocked", Calls.decideFor(t, "01 99 00 00 04", false, ms(day, "23:00"), 0, null, 0, none), true, "blocked");
        Calls.Press offHere = new Calls.Press(); offHere.pressed = 11; offHere.on = false;
        expect("off on the phone after", Calls.decideFor(t, "01 99 00 55 55", false, ms(day, "23:00"), 0, offHere, 0, none), true, "matrix");
        t.through = null;
        Calls.Press ended = new Calls.Press(); ended.pressed = 12; ended.on = true; ended.until = ms(day, "22:30");
        expect("an hour that ended", Calls.decideFor(t, "01 99 00 55 55", false, ms(day, "23:00"), 0, ended, 0, none), true, "matrix");
        expect("no table", Calls.decideFor(null, "01 99 00 55 55", false, ms(day, "23:00"), 0, null, 0, none), false, "no-table");
        expect("past the frames", Calls.decideFor(t, "01 99 00 55 55", false, ms("2026-10-06", "08:00"), 0, null, 0, none), false, "no-frame");

        // ---------------------------------------------------------------- version 2: the call screening apart
        t.v = 2;
        t.people.clear();
        t.numbers.put("+33199000009", "always-stranger");
        t.frames.clear();
        t.frames.add(frame(ms(day, "00:00"), work0, "sleep", "always-stranger"));
        t.frames.add(frame(work0, work1, "work", "safe", "neutral", "restricted", "hidden", "always-stranger"));
        t.frames.add(frame(work1, leisure1, "leisure", "safe", "always-stranger"));
        t.frames.add(frame(leisure1, sleep1, "sleep", "always-stranger"));
        t.screening = screening();
        String stranger = "01 99 00 55 55", automated = "02 61 91 12 34", impossible = "03 53 01 12 34", other = "05 36 49 12 34";
        for (String n : new String[] { "112", "15", "0 800 112 112" })
            expect("v2 emergency " + n, Calls.decideFor(t, n, false, ms(day, "23:00"), 0, null, 0, none), false, "emergency");
        for (String n : new String[] { "01 99 00 00 04", stranger, automated, impossible, "" })
            expect("v2 after an emergency call: " + n, Calls.decideFor(t, n, n.isEmpty(), ms(day, "23:00"), called, null, 0, none), false, "after-emergency");
        expect("v2 blocked", Calls.decideFor(t, "01 99 00 00 04", false, ms(day, "10:00"), 0, null, 0, none), true, "blocked");
        expect("v2 blocked again", Calls.decideFor(t, "01 99 00 00 04", false, ms(day, "10:00"), 0, null, ms(day, "09:55"), none), true, "blocked");
        expectWho("v2 a machine's second call", Calls.decideFor(t, automated, false, ms(day, "18:10"), 0, null, first, none), false, "repeat", "stranger");
        expectWho("v2 a hidden number's second call", Calls.decideFor(t, "", true, ms(day, "18:10"), 0, null, first, none), true, "screening", "hidden");
        expectWho("v2 Always through at night", Calls.decideFor(t, "01 99 00 00 09", false, ms(day, "23:00"), 0, null, 0, none), false, "matrix", "always-stranger");
        expectWho("v2 safe at work", Calls.decideFor(t, "01 99 00 00 01", false, ms(day, "10:00"), 0, null, 0, none), false, "matrix", "safe");
        expectWho("v2 safe at night", Calls.decideFor(t, "01 99 00 00 01", false, ms(day, "23:00"), 0, null, 0, none), true, "matrix", "safe");
        expectWho("v2 neutral in the evening", Calls.decideFor(t, "01 99 00 00 02", false, ms(day, "18:00"), 0, null, 0, none), true, "matrix", "neutral");
        expectWho("v2 phone contact at work", Calls.decideFor(t, stranger, false, ms(day, "10:00"), 0, null, 0, contact), false, "matrix", "neutral");
        expectWho("v2 phone contact in the evening", Calls.decideFor(t, stranger, false, ms(day, "18:00"), 0, null, 0, contact), true, "matrix", "neutral");
        long lately = ms("2026-10-02", "12:00");
        Calls.Decision calledLately = Calls.decideFor(t, stranger, false, ms(day, "10:00"), 0, null, 0, none, "", lately);
        expectWho("v2 called lately, at work", calledLately, false, "matrix", "neutral");
        if (!calledLately.called) {
            failed++;
            System.out.println("FAIL v2 called lately: not said");
        }
        expectWho("v2 called lately, at night", Calls.decideFor(t, stranger, false, ms(day, "23:00"), 0, null, 0, none, "", lately), true, "matrix", "neutral");
        expectWho("v2 called a while ago", Calls.decideFor(t, stranger, false, ms(day, "10:00"), 0, null, 0, none, "", ms("2026-09-20", "12:00")), true, "screening", "stranger");
        // Declined by your contacts' row, a number called lately never rings less than a number nobody knows; spam kinds aside.
        Calls.Press waiting = new Calls.Press(); waiting.pressed = 10; waiting.on = true;
        t.expecting = waiting;
        expectWho("v2 called lately, at night, expecting a call", Calls.decideFor(t, stranger, false, ms(day, "23:00"), 0, null, 0, none, "", lately), false, "expecting", "neutral");
        t.expecting = null;
        t.screening.on = false;
        expectWho("v2 called lately, at night, the screening off", Calls.decideFor(t, stranger, false, ms(day, "23:00"), 0, null, 0, none, "", lately), false, "screening-off", "neutral");
        t.screening = screening();
        t.screening.unknown = "ring";
        expectWho("v2 called lately, at night, numbers nobody knows ringing", Calls.decideFor(t, stranger, false, ms(day, "23:00"), 0, null, 0, none, "", lately), false, "screening", "neutral");
        t.screening = screening();
        t.screening.hours = "outside-work";
        Calls.Frame workFrame = t.frames.get(1);
        workFrame.column = "dnd";
        workFrame.ring.put("neutral", false);
        expectWho("v2 called lately, do-not-disturb at work, the screening resting", Calls.decideFor(t, stranger, false, ms(day, "10:00"), 0, null, 0, none, "", lately), false, "work-hours", "neutral");
        expectWho("v2 a phone contact, do-not-disturb at work: their row alone", Calls.decideFor(t, stranger, false, ms(day, "10:00"), 0, null, 0, contact), true, "matrix", "neutral");
        workFrame.column = "work";
        workFrame.ring.put("neutral", true);
        t.screening = screening();
        expectWho("v2 a machine's number called lately, at work", Calls.decideFor(t, automated, false, ms(day, "10:00"), 0, null, 0, none, "", lately), false, "matrix", "neutral");
        expectWho("v2 a machine's number called lately, at night", Calls.decideFor(t, automated, false, ms(day, "23:00"), 0, null, 0, none, "", lately), true, "matrix", "neutral");
        expect("v2 called lately, verification failed", Calls.decideFor(t, stranger, false, ms(day, "10:00"), 0, null, 0, none, "failed", lately), false, "matrix");
        for (String at : new String[] { "03:00", "10:00", "18:00", "23:00" })
            expectWho("v2 a number nobody knows at " + at, Calls.decideFor(t, stranger, false, ms(day, at), 0, null, 0, none), true, "screening", "stranger");
        expect("v2 automated", Calls.decideFor(t, automated, false, ms(day, "10:00"), 0, null, 0, none), true, "automated");
        expect("v2 impossible", Calls.decideFor(t, impossible, false, ms(day, "10:00"), 0, null, 0, none), true, "impossible");
        expect("v2 089", Calls.decideFor(t, "08 99 12 34 56", false, ms(day, "10:00"), 0, null, 0, none), true, "impossible");
        expectWho("v2 failed verification", Calls.decideFor(t, other, false, ms(day, "10:00"), 0, null, 0, none, "failed", 0), true, "unverified", "stranger");
        expect("v2 verified, in a block attributed to nobody", Calls.decideFor(t, impossible, false, ms(day, "10:00"), 0, null, 0, none, "passed", 0), true, "screening");
        expect("v2 someone known, verification failed", Calls.decideFor(t, "01 99 00 00 01", false, ms(day, "10:00"), 0, null, 0, none, "failed", 0), false, "matrix");
        // Each kind as any number nobody knows, and those let ring.
        Calls.Screening lenient = screening();
        lenient.automated = "unknown"; lenient.impossible = "unknown"; lenient.unverified = "unknown"; lenient.unknown = "ring";
        t.screening = lenient;
        expect("v2 lenient automated", Calls.decideFor(t, automated, false, ms(day, "23:00"), 0, null, 0, none), false, "screening");
        expect("v2 lenient impossible", Calls.decideFor(t, impossible, false, ms(day, "23:00"), 0, null, 0, none), false, "screening");
        expect("v2 lenient failed", Calls.decideFor(t, other, false, ms(day, "10:00"), 0, null, 0, none, "failed", 0), false, "screening");
        t.screening = screening();
        String[][] hidden = { { "03:00", "true" }, { "10:00", "false" }, { "18:00", "true" }, { "23:00", "true" } };
        for (String[] h : hidden)
            expectWho("v2 hidden at " + h[0], Calls.decideFor(t, "", true, ms(day, h[0]), 0, null, 0, none), Boolean.parseBoolean(h[1]), "screening", "hidden");
        t.screening.hidden = "voicemail";
        expect("v2 hidden, voicemail at all hours", Calls.decideFor(t, "", true, ms(day, "10:00"), 0, null, 0, none), true, "screening");
        t.screening.hidden = "ring";
        expect("v2 hidden, ringing", Calls.decideFor(t, "", true, ms(day, "23:00"), 0, null, 0, none), false, "screening");
        t.screening = screening();
        // Expecting a call: numbers nobody knows and hidden ones ring; never the blocked, machines, impossible numbers, failed verifications; nor the people you know beyond their row.
        Calls.Press expecting = new Calls.Press(); expecting.pressed = 10; expecting.on = true;
        t.expecting = expecting;
        expect("v2 expecting, nobody knows", Calls.decideFor(t, stranger, false, ms(day, "23:00"), 0, null, 0, none), false, "expecting");
        expect("v2 expecting, hidden", Calls.decideFor(t, "", true, ms(day, "23:00"), 0, null, 0, none), false, "expecting");
        expect("v2 expecting, blocked", Calls.decideFor(t, "01 99 00 00 04", false, ms(day, "23:00"), 0, null, 0, none), true, "blocked");
        expect("v2 expecting, automated", Calls.decideFor(t, automated, false, ms(day, "23:00"), 0, null, 0, none), true, "automated");
        expect("v2 expecting, impossible", Calls.decideFor(t, impossible, false, ms(day, "23:00"), 0, null, 0, none), true, "impossible");
        expect("v2 expecting, failed", Calls.decideFor(t, other, false, ms(day, "23:00"), 0, null, 0, none, "failed", 0), true, "unverified");
        expect("v2 expecting, neutral at night", Calls.decideFor(t, "01 99 00 00 02", false, ms(day, "23:00"), 0, null, 0, none), true, "matrix");
        expect("v2 expecting, off on the phone after", Calls.decideFor(t, stranger, false, ms(day, "23:00"), 0, offHere, 0, none), true, "screening");
        t.expecting = null;
        expect("v2 expecting that ended", Calls.decideFor(t, stranger, false, ms(day, "23:00"), 0, ended, 0, none), true, "screening");
        // The screening off; only outside work and admin hours.
        t.screening.on = false;
        for (String n : new String[] { stranger, automated, "" })
            expect("v2 off: " + n, Calls.decideFor(t, n, n.isEmpty(), ms(day, "23:00"), 0, null, 0, none), false, "screening-off");
        expect("v2 off, blocked", Calls.decideFor(t, "01 99 00 00 04", false, ms(day, "23:00"), 0, null, 0, none), true, "blocked");
        t.screening = screening();
        t.screening.hours = "outside-work";
        expect("v2 outside work, a machine at work", Calls.decideFor(t, automated, false, ms(day, "10:00"), 0, null, 0, none), false, "work-hours");
        expect("v2 outside work, in the evening", Calls.decideFor(t, stranger, false, ms(day, "18:00"), 0, null, 0, none), true, "screening");
        t.screening = screening();
        // Neither do-not-disturb nor any frame of the people you know opens or closes the screening.
        String[][] unknownCalls = { { stranger, "10:00" }, { stranger, "23:00" }, { "", "10:00" }, { "", "18:00" }, { automated, "10:00" }, { impossible, "23:00" } };
        boolean[] before = new boolean[unknownCalls.length];
        for (int i = 0; i < unknownCalls.length; i++)
            before[i] = Calls.decideFor(t, unknownCalls[i][0], unknownCalls[i][0].isEmpty(), ms(day, unknownCalls[i][1]), 0, null, 0, none).refuse;
        for (boolean everyone : new boolean[] { false, true }) {
            for (Calls.Frame f : t.frames) {
                f.column = "dnd";
                for (String row : new String[] { "safe", "neutral", "restricted", "always-stranger" })
                    f.ring.put(row, everyone);
            }
            for (int i = 0; i < unknownCalls.length; i++) {
                Calls.Decision d = Calls.decideFor(t, unknownCalls[i][0], unknownCalls[i][0].isEmpty(), ms(day, unknownCalls[i][1]), 0, null, 0, none);
                if (d.refuse != before[i]) {
                    failed++;
                    System.out.println("FAIL v2 a frame of the people you know moved the screening: " + unknownCalls[i][0] + " at " + unknownCalls[i][1]);
                }
            }
        }
        // Nothing to decide from: it rings, past the frames everyone but the blocked.
        expect("v2 no table", Calls.decideFor(null, automated, false, ms(day, "23:00"), 0, null, 0, none), false, "no-table");
        for (String n : new String[] { stranger, automated, "01 99 00 00 01", "" })
            expect("v2 past the frames: " + n, Calls.decideFor(t, n, n.isEmpty(), ms("2026-10-06", "08:00"), 0, null, 0, none), false, "no-frame");
        expect("v2 past the frames, blocked", Calls.decideFor(t, "01 99 00 00 04", false, ms("2026-10-06", "08:00"), 0, null, 0, none), true, "blocked");
        // National form, as screening.rs's `national`.
        String[][] national = { { "+33199001234", "0199001234" }, { "+262262123456", "0262123456" }, { "+590590123456", "0590123456" }, { "+3319900123", null }, { "+441632960000", null } };
        for (String[] n : national) {
            String got = Calls.national(n[0]);
            if (n[1] == null ? got != null : !n[1].equals(got)) {
                failed++;
                System.out.println("FAIL national " + n[0] + ": " + got);
            }
        }
        System.out.println(failed == 0 ? "all decisions agree with calls.rs" : failed + " failed");
        System.exit(failed == 0 ? 0 : 1);
    }
}
