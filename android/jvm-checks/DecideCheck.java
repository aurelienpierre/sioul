// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.time.ZoneId;
import java.time.ZonedDateTime;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

/** Calls.decideFor on the cases of calls.rs's `who_rings_when_and_what_always_rings`. */
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
        for (String row : new String[] { "safe", "neutral", "restricted", "stranger", "hidden" })
            f.ring.put(row, on.contains(row));
        return f;
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
        System.out.println(failed == 0 ? "all decisions agree with calls.rs" : failed + " failed");
        System.exit(failed == 0 ? 0 : 1);
    }
}
