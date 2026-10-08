// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import org.json.JSONArray;
import org.json.JSONObject;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;

/**
 * The full card on the home screen, on the JVM (org.json's own jar before
 * android.jar's stubs): HomeCardToday.Lines.of, what the card says at a time,
 * from a card as homecard.rs writes it (version 3):
 * - today: the date's wording, the weather's hour for now (now, the next four
 *   hours, the parts of the day), the seven days after today, the credit;
 * - the Porch's lines: a dose, a code and its warning, the reminders the
 *   frame shows from their time (the same words once), the calls, two events
 *   at once; Now's: the status line, the step, where you stopped; where each
 *   tap leads;
 * - when it draws again: the hour's end, a reminder's time, the frame's end;
 * - past the last frame, "beyond"; a card of version 2 still read; an older
 *   Java, which reads "frames", finds none in a card of version 3.
 * With SIOUL_CARD_SAMPLE naming a card Rust's test wrote
 * (homecard::tests::the_file_java_reads), that card is read too.
 */
public final class CardCheck
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

    /** Thursday 8 October 2026, 00:00 in Paris, Unix ms; an hour. */
    static final long DAY = 1_791_410_400_000L, HOUR = 3_600_000L;

    static JSONObject slot(String label, String temperature, String rain) throws Exception
    {
        return new JSONObject().put("label", label).put("icon", "weather-clouds-symbolic").put("temperature", temperature).put("rain", rain).put("words", "Partly cloudy");
    }

    static JSONObject day(int offset, String label, String high, String low) throws Exception
    {
        return new JSONObject().put("from", DAY + offset * 24 * HOUR).put("until", DAY + (offset + 1) * 24 * HOUR).put("date", "2026-10-0" + (8 + offset))
            .put("label", label).put("icon", "weather-showers-day-symbolic").put("high", high).put("low", low).put("rain", "").put("words", "Rain");
    }

    static JSONObject hour(long from, String now) throws Exception
    {
        return new JSONObject().put("from", from).put("until", from + HOUR).put("now", slot("Now", now, "98 %"))
            .put("next", new JSONArray().put(slot("10:00", "16°", "98 %")).put(slot("11:00", "15°", "")).put(slot("12:00", "15°", "")).put(slot("13:00", "14°", "")))
            .put("parts", new JSONArray().put(slot("Afternoon", "14° – 15°", "98 %")).put(slot("Evening", "13° – 14°", "65 %")).put(slot("Night", "13°", "")).put(slot("Morning", "13° – 14°", "")));
    }

    static JSONObject timed(String line, long from, long until) throws Exception
    {
        return new JSONObject().put("line", line).put("from", from).put("until", until);
    }

    /** A card as homecard.rs writes it, on Thursday at 09:20: work until 12:10, then lunch. */
    static JSONObject card() throws Exception
    {
        JSONObject today = new JSONObject()
            .put("date", new JSONArray().put(new JSONObject().put("until", DAY + 24 * HOUR).put("text", "Thursday 8 October")).put(new JSONObject().put("until", DAY + 48 * HOUR).put("text", "Friday 9 October")))
            .put("line", "")
            .put("weather", new JSONObject()
                .put("hours", new JSONArray().put(hour(DAY + 9 * HOUR, "16°")).put(hour(DAY + 10 * HOUR, "15°")))
                .put("days", new JSONArray().put(day(0, "Thu", "16°", "13°")).put(day(1, "Fri", "17°", "13°")).put(day(2, "Sat", "16°", "12°")))
                .put("credit", "Weather: Open-Meteo.com"));
        JSONObject work = new JSONObject().put("from", DAY + 9 * HOUR + 20 * 60_000).put("until", DAY + 12 * HOUR + 10 * 60_000).put("kind", "work")
            .put("status", "Work until 12:10.").put("dnd", "").put("note", "")
            .put("step", new JSONObject().put("title", "Call the CAF about the housing aid").put("why", "").put("uid", "form"))
            .put("codes", true).put("doses", true).put("events", true).put("overlaps", true).put("stopped", true)
            .put("reminders", new JSONArray().put(0).put(1).put(2).put(3)).put("calls", new JSONArray().put(0));
        JSONObject lunch = new JSONObject().put("from", DAY + 12 * HOUR + 10 * 60_000).put("until", DAY + 13 * HOUR).put("kind", "meals")
            .put("status", "Meal until 13:00.").put("dnd", "").put("note", "").put("codes", true).put("doses", true).put("events", true)
            .put("overlaps", true).put("stopped", false).put("reminders", new JSONArray().put(1)).put("calls", new JSONArray());
        return new JSONObject().put("v", 3).put("made", DAY + 9 * HOUR + 20 * 60_000).put("details", true).put("stale_after", DAY + 9 * HOUR + 35 * 60_000)
            .put("stale", new JSONArray().put(new JSONObject().put("until", 0).put("text", "at 09:20")))
            .put("beyond", "Open Sioul to bring this card up to date.")
            .put("today", today)
            .put("times", new JSONArray().put(work).put(lunch))
            .put("codes", new JSONArray().put(new JSONObject().put("line", "Code from La Banque (verified): 482913").put("warning", "").put("until", DAY + 9 * HOUR + 28 * 60_000)))
            .put("doses", new JSONArray().put(new JSONObject().put("line", "Magnesium · 300 mg, 09:15").put("check", "Magnesium · 300 mg, 09:15: check before taking it.")
                .put("from", DAY + 9 * HOUR + 15 * 60_000).put("until", DAY + 21 * HOUR).put("known_until", DAY + 9 * HOUR + 40 * 60_000)))
            .put("reminders", new JSONArray()
                .put(new JSONObject().put("line", "Call the CAF · Asked for Friday 30 October").put("from", DAY + 9 * HOUR).put("until", DAY + 23 * 24 * HOUR).put("open", "sioul:task/form"))
                .put(new JSONObject().put("line", "Rent · €650").put("from", DAY + 9 * HOUR).put("until", DAY + 48 * HOUR).put("open", "sioul:budget/home"))
                .put(new JSONObject().put("line", "Rent · €650").put("from", DAY + 9 * HOUR).put("until", DAY + 48 * HOUR).put("open", "sioul:budget/home"))
                .put(new JSONObject().put("line", "Passport: valid until Sunday 1 November").put("from", DAY + 33 * HOUR).put("until", DAY + 30 * 24 * HOUR).put("open", "sioul:paper/passport")))
            .put("calls", new JSONArray().put("While you slept, Marie Dupont called at 07:40."))
            .put("overlaps", new JSONArray().put(timed("Today, two events at once: Dentist and School meeting.", DAY, DAY + 12 * HOUR + 30 * 60_000)))
            .put("stopped", "Where you stopped: halfway through the form")
            .put("mail", new JSONArray()).put("words", new JSONObject().put("mail", "Mail").put("agenda", "Agenda"));
    }

    public static void main(String[] args) throws Exception
    {
        long at = DAY + 9 * HOUR + 25 * 60_000;
        HomeCardToday.Lines lines = HomeCardToday.Lines.of(card(), at);
        expect("date", lines.texts[HomeCardToday.DATE], "Thursday 8 October");
        expect("weather now", lines.now.temperature + " " + lines.now.rain + " " + lines.now.icon, "16° 98 % weather-clouds-symbolic");
        expect("next hours", lines.hours.size() + " " + lines.hours.get(0).label + " " + lines.hours.get(3).label, "4 10:00 13:00");
        expect("parts", lines.parts.size() + " " + lines.parts.get(0).label + " " + lines.parts.get(3).label, "4 Afternoon Morning");
        // The days after today: Friday first, its high and low.
        expect("days", lines.days.size() + " " + lines.days.get(0).label + " " + lines.days.get(0).temperature + "/" + lines.days.get(0).low, "2 Fri 17°/13°");
        expect("credit", lines.texts[HomeCardToday.CREDIT], "Weather: Open-Meteo.com");
        expect("status", lines.texts[HomeCardToday.STATUS], "Work until 12:10.");
        expect("dose, known", lines.texts[HomeCardToday.DOSE], "Magnesium · 300 mg, 09:15");
        expect("code", lines.texts[HomeCardToday.CODE] + " → " + lines.opens[HomeCardToday.CODE], "Code from La Banque (verified): 482913 → porch");
        // The reminders from their time, the same words once; the passport's from tomorrow.
        expect("reminders", lines.texts[HomeCardToday.REMINDER] + " | " + lines.texts[HomeCardToday.REMINDER + 1] + " | " + lines.texts[HomeCardToday.REMINDER + 2],
               "Call the CAF · Asked for Friday 30 October | Rent · €650 | ");
        expect("a reminder's tap", lines.opens[HomeCardToday.REMINDER], "reminder/sioul:task/form");
        expect("call", lines.texts[HomeCardToday.CALL] + " → " + lines.opens[HomeCardToday.CALL], "While you slept, Marie Dupont called at 07:40. → porch");
        expect("overlap", lines.texts[HomeCardToday.OVERLAP], "Today, two events at once: Dentist and School meeting.");
        expect("step", lines.texts[HomeCardToday.STEP] + " → " + lines.opens[HomeCardToday.STEP], "Call the CAF about the housing aid → now/form");
        expect("stopped", lines.texts[HomeCardToday.STOPPED] + " → " + lines.opens[HomeCardToday.STOPPED], "Where you stopped: halfway through the form → now");
        // Drawn again when the code expires (09:28), the first change ahead.
        expect("next drawing", lines.next, DAY + 9 * HOUR + 28 * 60_000);
        // At 09:45: the code gone, the dose no longer known as not marked elsewhere; the age said.
        lines = HomeCardToday.Lines.of(card(), DAY + 9 * HOUR + 45 * 60_000);
        expect("code expired", lines.texts[HomeCardToday.CODE], "");
        expect("dose, check", lines.texts[HomeCardToday.DOSE], "Magnesium · 300 mg, 09:15: check before taking it.");
        expect("age", lines.age, "at 09:20");
        // At 10:30: the next hour's weather.
        lines = HomeCardToday.Lines.of(card(), DAY + 10 * HOUR + 30 * 60_000);
        expect("an hour later", lines.now.temperature, "15°");
        expect("redrawn at its end", lines.next, DAY + 11 * HOUR);
        // At lunch: no step, no line on where you stopped; the payment alone (work's date waits).
        lines = HomeCardToday.Lines.of(card(), DAY + 12 * HOUR + 30 * 60_000);
        expect("lunch", lines.texts[HomeCardToday.STATUS] + " | " + lines.texts[HomeCardToday.STEP] + " | " + lines.texts[HomeCardToday.STOPPED] + " | " + lines.texts[HomeCardToday.REMINDER],
               "Meal until 13:00. |  |  | Rent · €650");
        // Past the weather's hours: no weather now, the days still.
        expect("hours over", lines.now == null && lines.days.size() == 2, true);
        // Past the last frame: Open Sioul.
        lines = HomeCardToday.Lines.of(card(), DAY + 14 * HOUR);
        expect("beyond", lines.texts[HomeCardToday.STATUS], "Open Sioul to bring this card up to date.");
        // No card yet: Java's own words.
        expect("no card", HomeCardToday.Lines.of(null, at).texts[HomeCardToday.STATUS], HomeCard.empty());
        // A card an older Sioul wrote (version 2, "frames"): its lines, no today.
        JSONObject older = card().put("v", 2);
        older.put("frames", older.remove("times")).remove("today");
        lines = HomeCardToday.Lines.of(older, at);
        expect("version 2", lines.texts[HomeCardToday.STATUS] + " | " + lines.texts[HomeCardToday.DATE] + " | " + (lines.now == null), "Work until 12:10. |  | true");
        // An older Java reads "frames": none in a card of version 3, so it says "beyond".
        expect("an older Java", HomeCard.frame(card().optJSONArray("frames"), at) == null, true);
        // No place for the weather: the line instead, no credit.
        JSONObject noPlace = card();
        noPlace.getJSONObject("today").put("weather", JSONObject.NULL).put("line", "Choose a place for the weather in Sioul: tap the weather in its status line.");
        lines = HomeCardToday.Lines.of(noPlace, at);
        expect("no place", lines.texts[HomeCardToday.WEATHER_LINE] + " | " + lines.texts[HomeCardToday.CREDIT] + " | " + (lines.now == null),
               "Choose a place for the weather in Sioul: tap the weather in its status line. |  | true");
        // An icon weather.rs names, and one it does not.
        expect("icons", HomeCardToday.icon("weather-hail-symbolic") == R.drawable.sioul_weather_hail && HomeCardToday.icon("elsewhere") == R.drawable.sioul_weather_none_available, true);

        String sample = System.getenv("SIOUL_CARD_SAMPLE");
        if (sample != null && !sample.isEmpty()) {
            JSONObject written = new JSONObject(new String(Files.readAllBytes(Paths.get(sample)), StandardCharsets.UTF_8));
            lines = HomeCardToday.Lines.of(written, written.getLong("made") + 60_000);
            expect("Rust's card: date", lines.texts[HomeCardToday.DATE], "Thursday 8 October");
            expect("Rust's card: weather", lines.now != null && lines.hours.size() == 4 && lines.parts.size() == 4 && lines.days.size() == 7, true);
            expect("Rust's card: lines", !lines.texts[HomeCardToday.STATUS].isEmpty() && !lines.texts[HomeCardToday.REMINDER].isEmpty() && !lines.texts[HomeCardToday.CALL].isEmpty() && !lines.texts[HomeCardToday.STEP].isEmpty(), true);
            System.out.println("Rust's card read: " + sample);
        }

        System.out.println(checked + " checked, " + failed + " failed");
        if (failed > 0)
            System.exit(1);
    }
}
