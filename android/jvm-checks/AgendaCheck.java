// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import org.json.JSONArray;
import org.json.JSONObject;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;

/**
 * The agenda's list on the home screen, on the JVM (org.json's own jar
 * before android.jar's stubs): HomeCardRows.rows from a card as homecard.rs
 * writes it. From version 4, each day's dates asked at its head, before its
 * events, each with its words as they hold then (the time left, given ahead)
 * and where its tap leads; a date gone once its day ends; a day with a date
 * and no event listed all the same; a card with dates and no event says no
 * "nothing in your calendars". A card of version 3 (the older Sioul's, on
 * screen just after an update) lists its events as before and no date, even
 * with dates in it. When the list draws again: a date's next change counts
 * from version 4 only. With SIOUL_CARD_SAMPLE naming a card Rust's test
 * wrote (homecard::tests::the_file_java_reads), its dates are listed too.
 */
public final class AgendaCheck
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

    /** Thursday 8 October 2026, 00:00 in Paris, Unix ms; an hour, a day. */
    static final long DAY = 1_791_410_400_000L, HOUR = 3_600_000L, DAYS = 24 * HOUR;

    static JSONArray labels(Object... pairs) throws Exception
    {
        JSONArray out = new JSONArray();
        for (int i = 0; i < pairs.length; i += 2)
            out.put(new JSONObject().put("until", pairs[i]).put("text", pairs[i + 1]));
        return out;
    }

    static JSONObject event(String title, long start, long end) throws Exception
    {
        return new JSONObject().put("key", "/calendars/me/personal/" + title.toLowerCase() + ".ics").put("title", title).put("time", "").put("all_day", false)
            .put("start", start).put("end", end).put("color", "#4c6b5c");
    }

    /** A card on Thursday: the dentist today, the choir tomorrow; the rent tomorrow, the CAF's form on Saturday. */
    static JSONObject card(int version) throws Exception
    {
        JSONArray days = new JSONArray()
            .put(new JSONObject().put("from", DAY).put("until", DAY + DAYS).put("label", labels(DAY + DAYS, "Today, Thu 8 Oct")))
            .put(new JSONObject().put("from", DAY + DAYS).put("until", DAY + 2 * DAYS).put("label", labels(DAY + DAYS, "Tomorrow, Fri 9 Oct", DAY + 2 * DAYS, "Today, Fri 9 Oct")))
            .put(new JSONObject().put("from", DAY + 2 * DAYS).put("until", DAY + 3 * DAYS)
                .put("label", labels(DAY + DAYS, "Sat 10 Oct", DAY + 2 * DAYS, "Tomorrow, Sat 10 Oct", DAY + 3 * DAYS, "Today, Sat 10 Oct")));
        JSONArray events = new JSONArray().put(event("Dentist", DAY + 11 * HOUR, DAY + 12 * HOUR)).put(event("Choir", DAY + DAYS + 19 * HOUR, DAY + DAYS + 20 * HOUR));
        JSONArray dates = new JSONArray()
            .put(new JSONObject().put("title", "Rent · €650").put("line", labels(DAY + DAYS, "Payment · tomorrow", DAY + 2 * DAYS, "Payment · today"))
                .put("mark", labels(0, "●")).put("from", DAY + DAYS).put("until", DAY + 2 * DAYS).put("open", "reminder/sioul:budget/home/3"))
            .put(new JSONObject().put("title", "CAF form").put("line", labels(DAY + DAYS, "Date asked · two days left", DAY + 2 * DAYS, "Date asked · tomorrow", DAY + 3 * DAYS, "Date asked · today"))
                .put("mark", labels(0, "●")).put("from", DAY + 2 * DAYS).put("until", DAY + 3 * DAYS).put("open", "reminder/sioul:task/caf"));
        JSONObject agenda = new JSONObject().put("empty", "Nothing in your calendars for the coming month.").put("days", days).put("events", events).put("dates", dates);
        JSONObject frame = new JSONObject().put("from", DAY).put("until", DAY + 3 * DAYS).put("kind", "work").put("status", "Work until 17:00.").put("events", true);
        return new JSONObject().put("v", version).put("times", new JSONArray().put(frame)).put("agenda", agenda)
            .put("words", new JSONObject().put("mail", "Mail").put("agenda", "Agenda"));
    }

    /** The rows as the list shows them: "day:", "date:", "event:" and their words, a date's tap. */
    static String said(JSONObject card, long now)
    {
        List<String> out = new ArrayList<>();
        for (HomeCardRows.Row row : HomeCardRows.rows(card, now, HomeCard.AGENDA)) {
            if (row.type == HomeCardRows.DAY)
                out.add("day:" + row.text);
            else if (row.type == HomeCardRows.DATE)
                out.add("date:" + row.item.optString("title") + " (" + row.text + ") → " + row.open);
            else if (row.type == HomeCardRows.EVENT)
                out.add("event:" + row.item.optString("title"));
            else
                out.add(row.text);
        }
        return String.join(" | ", out);
    }

    public static void main(String[] args) throws Exception
    {
        long morning = DAY + 9 * HOUR;
        expect("version 4", said(card(4), morning),
               "Agenda | day:Today, Thu 8 Oct | event:Dentist | day:Tomorrow, Fri 9 Oct | date:Rent · €650 (Payment · tomorrow) → reminder/sioul:budget/home/3 | event:Choir"
               + " | day:Sat 10 Oct | date:CAF form (Date asked · two days left) → reminder/sioul:task/caf");
        // The older Sioul's card, just after an update: its events as before, no date.
        expect("version 3", said(card(3), morning), "Agenda | day:Today, Thu 8 Oct | event:Dentist | day:Tomorrow, Fri 9 Oct | event:Choir");
        // Friday evening: the choir over, the words as they hold then.
        expect("Friday evening", said(card(4), DAY + DAYS + 21 * HOUR),
               "Agenda | day:Today, Fri 9 Oct | date:Rent · €650 (Payment · today) → reminder/sioul:budget/home/3 | day:Tomorrow, Sat 10 Oct | date:CAF form (Date asked · tomorrow) → reminder/sioul:task/caf");
        // Saturday: the rent's day over, it is gone.
        expect("Saturday", said(card(4), DAY + 2 * DAYS + HOUR), "Agenda | day:Today, Sat 10 Oct | date:CAF form (Date asked · today) → reminder/sioul:task/caf");
        // No event, dates all the same: listed, and no "nothing in your calendars".
        JSONObject noEvents = card(4);
        noEvents.getJSONObject("agenda").put("events", new JSONArray());
        expect("dates alone", said(noEvents, morning), "Agenda | day:Tomorrow, Fri 9 Oct | date:Rent · €650 (Payment · tomorrow) → reminder/sioul:budget/home/3 | day:Sat 10 Oct | date:CAF form (Date asked · two days left) → reminder/sioul:task/caf");
        JSONObject nothing = card(3);
        nothing.getJSONObject("agenda").put("events", new JSONArray());
        expect("version 3, nothing", said(nothing, morning), "Agenda | Nothing in your calendars for the coming month.");
        // Drawn again at a date's next change, from version 4 only.
        JSONObject marked = card(4);
        marked.getJSONObject("agenda").getJSONArray("dates").getJSONObject(1).put("mark", labels(DAY + 15 * HOUR, "○", 0, "●"));
        long afternoon = DAY + 13 * HOUR;
        expect("next change, version 4", HomeCardRows.nextChange(marked, afternoon), DAY + 15 * HOUR);
        expect("next change, version 3", HomeCardRows.nextChange(marked.put("v", 3), afternoon), DAY + DAYS);

        String sample = System.getenv("SIOUL_CARD_SAMPLE");
        if (sample != null && !sample.isEmpty()) {
            JSONObject written = new JSONObject(new String(Files.readAllBytes(Paths.get(sample)), StandardCharsets.UTF_8));
            String rows = said(written, written.getLong("made") + 60_000);
            expect("Rust's card: its dates", rows.contains("date:Rent · €650 (Payment · tomorrow) → reminder/sioul:budget/home/3") && rows.contains("date:Call the CAF about the housing aid"), true);
            System.out.println("Rust's card read: " + sample);
        }

        System.out.println(checked + " checked, " + failed + " failed");
        if (failed > 0)
            System.exit(1);
    }
}
