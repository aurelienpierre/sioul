// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.time.ZoneId;
import java.time.ZonedDateTime;

import org.json.JSONObject;

/**
 * Calls.Table.of and Calls.decideFor on the calls' table as the core writes
 * it with the matrix of what reaches you (crates/sioul-core/src/calls.rs,
 * 7 October 2026): still v1; the frames' columns "free", "slot", "dnd";
 * nine rows, "always-<state>" among them; Always through's numbers as
 * numbers["<key>"] = "always-<their own state>", never over a blocked one;
 * floors.people empty. Read through org.json's own jar (android.jar's is a
 * stub), from the JSON text as serde writes it, then the cases of calls.rs's
 * `who_rings_when_and_what_always_rings` and of its frames' tests.
 */
public final class TableCheck
{
    static long ms(String day, String time)
    {
        return ZonedDateTime.of(java.time.LocalDate.parse(day), java.time.LocalTime.parse(time), ZoneId.of("Europe/Paris")).toInstant().toEpochMilli();
    }

    static final String[] ROWS = { "safe", "neutral", "restricted", "stranger", "hidden", "always-safe", "always-neutral", "always-restricted", "always-stranger" };

    static String frame(long from, long until, String column, String... ringing)
    {
        StringBuilder ring = new StringBuilder();
        for (String row : ROWS) {
            boolean on = java.util.Arrays.asList(ringing).contains(row);
            ring.append(ring.length() == 0 ? "" : ",").append('"').append(row).append("\":").append(on);
        }
        return "{\"from\":" + from + ",\"until\":" + until + ",\"column\":\"" + column + "\",\"ring\":{" + ring + "}}";
    }

    static int failed = 0;
    static int checked = 0;

    static void expect(String what, Calls.Decision d, boolean refuse, String why, String who, String column)
    {
        checked++;
        if (d.refuse != refuse || !d.why.equals(why) || !d.who.equals(who) || !d.column.equals(column)) {
            failed++;
            System.out.println("FAIL " + what + ": " + d.refuse + " " + d.why + " " + d.who + " " + d.column + " (expected " + refuse + " " + why + " " + who + " " + column + ")");
        }
    }

    public static void main(String[] args) throws Exception
    {
        String day = "2026-10-05";
        long midnight = ms(day, "00:00"), work0 = ms(day, "09:00"), work1 = ms(day, "17:00"), dnd0 = ms(day, "18:00"), dnd1 = ms(day, "19:00");
        long slot0 = ms(day, "19:10"), slot1 = ms(day, "19:30"), leisure1 = ms(day, "22:00"), sleep1 = ms("2026-10-06", "07:00");
        long free0 = ms("2026-10-06", "07:00"), free1 = ms("2026-10-06", "12:00");
        // As serde writes calls::Table: the carer (01 99 00 00 09) Always through, in no address book:
        // "always-stranger"; a blocked number on the list too stays "blocked" (blocked beats Always through).
        String json = "{\"v\":1,\"made\":" + midnight
            + ",\"region\":{\"code\":\"FR\",\"calling\":\"33\",\"trunk\":\"0\",\"international\":\"00\",\"digits\":[10,10],\"french\":true,\"overseas\":{\"692\":\"262\"}}"
            + ",\"trunk_zero\":[\"32\",\"33\",\"44\",\"49\"]"
            + ",\"numbers\":{\"+33199000001\":\"safe\",\"+33199000002\":\"neutral\",\"+33199000003\":\"restricted\",\"+33199000004\":\"blocked\",\"+33199000009\":\"always-stranger\",\"+33199000010\":\"always-safe\",\"+33199000011\":\"always-neutral\"}"
            + ",\"prefixes\":[{\"prefix\":\"+3346571\",\"who\":\"blocked\"}]"
            + ",\"phone_contacts\":\"neutral\""
            + ",\"floors\":{\"emergency\":[\"112\",\"15\",\"114\",\"3114\",\"+33800112112\"],\"people\":[]}"
            + ",\"frames\":["
            + frame(midnight, work0, "sleep", "always-safe", "always-neutral", "always-restricted", "always-stranger") + ","
            + frame(work0, work1, "work", "safe", "neutral", "restricted", "hidden", "always-safe", "always-neutral", "always-restricted", "always-stranger") + ","
            + frame(work1, dnd0, "leisure", "safe", "always-safe", "always-neutral", "always-restricted", "always-stranger") + ","
            // Do-not-disturb's switch (a layer): the safe to voicemail, Always through rings.
            + frame(dnd0, dnd1, "dnd", "always-safe", "always-neutral", "always-restricted", "always-stranger") + ","
            + frame(dnd1, slot0, "leisure", "safe", "always-safe", "always-neutral", "always-restricted", "always-stranger") + ","
            // A slot of time for you holds no call.
            + frame(slot0, slot1, "slot", "safe", "always-safe", "always-neutral", "always-restricted", "always-stranger") + ","
            + frame(slot1, leisure1, "leisure", "safe", "always-safe", "always-neutral", "always-restricted", "always-stranger") + ","
            + frame(leisure1, sleep1, "sleep", "always-safe", "always-neutral", "always-restricted", "always-stranger") + ","
            // Free time with "Nothing at all": Always through alone.
            + frame(free0, free1, "free", "always-safe", "always-neutral", "always-restricted", "always-stranger")
            + "],\"repeat_minutes\":15,\"emergency_hours\":24}";
        Calls.Table t = Calls.Table.of(new JSONObject(json));
        Calls.Phone none = new Calls.Phone() {
            public boolean emergency(String raw) { return false; }
            public String contact(String raw) { return null; }
        };
        Calls.Phone contact = new Calls.Phone() {
            public boolean emergency(String raw) { return false; }
            public String contact(String raw) { return "Someone"; }
        };
        checked++;
        if (t.frames.size() != 9 || !t.people.isEmpty() || !"always-stranger".equals(t.numbers.get("+33199000009"))) {
            failed++;
            System.out.println("FAIL the table as read: " + t.frames.size() + " frames, people " + t.people + ", carer " + t.numbers.get("+33199000009"));
        }
        // The usual rows, as before.
        expect("safe at work", Calls.decideFor(t, "01 99 00 00 01", false, ms(day, "10:00"), 0, null, 0, none), false, "matrix", "safe", "work");
        expect("safe in the evening", Calls.decideFor(t, "01 99 00 00 01", false, ms(day, "17:30"), 0, null, 0, none), false, "matrix", "safe", "leisure");
        expect("safe at night", Calls.decideFor(t, "01 99 00 00 01", false, ms(day, "23:00"), 0, null, 0, none), true, "matrix", "safe", "sleep");
        expect("neutral in the evening", Calls.decideFor(t, "01 99 00 00 02", false, ms(day, "17:30"), 0, null, 0, none), true, "matrix", "neutral", "leisure");
        expect("hidden at work", Calls.decideFor(t, "", true, ms(day, "10:00"), 0, null, 0, none), false, "matrix", "hidden", "work");
        expect("phone contact in the evening", Calls.decideFor(t, "01 99 00 55 55", false, ms(day, "17:30"), 0, null, 0, contact), true, "matrix", "neutral", "leisure");
        // Always through: their own row, read in each frame; at night too (before: the list's floor, "people").
        expect("carer at night", Calls.decideFor(t, "01 99 00 00 09", false, ms(day, "23:00"), 0, null, 0, none), false, "matrix", "always-stranger", "sleep");
        expect("carer as +33", Calls.decideFor(t, "+33 1 99 00 00 09", false, ms(day, "03:00"), 0, null, 0, none), false, "matrix", "always-stranger", "sleep");
        expect("blocked, on the list too", Calls.decideFor(t, "01 99 00 00 04", false, ms(day, "23:00"), 0, null, 0, none), true, "blocked", "blocked", "");
        expect("blocked by prefix", Calls.decideFor(t, "04 65 71 12 34", false, ms(day, "10:00"), 0, null, 0, none), true, "blocked", "blocked", "");
        // The layers and Free time's columns.
        expect("safe during do-not-disturb", Calls.decideFor(t, "01 99 00 00 01", false, ms(day, "18:30"), 0, null, 0, none), true, "matrix", "safe", "dnd");
        expect("always-safe during do-not-disturb", Calls.decideFor(t, "01 99 00 00 10", false, ms(day, "18:30"), 0, null, 0, none), false, "matrix", "always-safe", "dnd");
        expect("safe in a slot", Calls.decideFor(t, "01 99 00 00 01", false, ms(day, "19:15"), 0, null, 0, none), false, "matrix", "safe", "slot");
        expect("safe in Free time, nothing at all", Calls.decideFor(t, "01 99 00 00 01", false, ms("2026-10-06", "08:00"), 0, null, 0, none), true, "matrix", "safe", "free");
        expect("always-neutral in Free time", Calls.decideFor(t, "01 99 00 00 11", false, ms("2026-10-06", "08:00"), 0, null, 0, none), false, "matrix", "always-neutral", "free");
        // The floors and the order, unchanged.
        expect("emergency callback at night", Calls.decideFor(t, "08 00 11 21 12", false, ms(day, "23:00"), 0, null, 0, none), false, "emergency", "", "");
        long called = ms(day, "03:00");
        expect("after an emergency call, blocked", Calls.decideFor(t, "01 99 00 00 04", false, ms(day, "23:00"), called, null, 0, none), false, "after-emergency", "", "");
        expect("a second call during do-not-disturb", Calls.decideFor(t, "01 99 00 00 01", false, ms(day, "18:40"), 0, null, ms(day, "18:30"), none), false, "repeat", "safe", "");
        Calls.Press on = new Calls.Press();
        on.pressed = 10;
        on.on = true;
        expect("every call let through, during do-not-disturb", Calls.decideFor(t, "01 99 00 00 02", false, ms(day, "18:30"), 0, on, 0, none), false, "through", "neutral", "");
        expect("every call let through, blocked", Calls.decideFor(t, "01 99 00 00 04", false, ms(day, "18:30"), 0, on, 0, none), true, "blocked", "blocked", "");
        // A gap between the frames (07:00 to 07:00 is joined here; 12:00 on is past them): it rings.
        expect("past the frames", Calls.decideFor(t, "01 99 00 00 01", false, ms("2026-10-06", "13:00"), 0, null, 0, none), false, "no-frame", "safe", "");
        // A row a frame does not name (a table of an older Sioul read by this Java, or the reverse): it rings.
        String older = json.replace("\"always-stranger\":true", "\"x\":true").replace("\"always-stranger\":false", "\"x\":false");
        Calls.Table o = Calls.Table.of(new JSONObject(older));
        expect("a row the frame does not name", Calls.decideFor(o, "01 99 00 00 09", false, ms(day, "23:00"), 0, null, 0, none), false, "no-row", "always-stranger", "sleep");
        // Another version is not read: calls ring.
        checked++;
        try {
            Calls.Table.of(new JSONObject(json.replace("\"v\":1", "\"v\":2")));
            failed++;
            System.out.println("FAIL a table of version 2 was read");
        } catch (org.json.JSONException e) {
            // As wanted.
        }
        System.out.println(failed == 0 ? "all " + checked + " table decisions agree with calls.rs" : failed + " of " + checked + " failed");
        System.exit(failed == 0 ? 0 : 1);
    }
}
