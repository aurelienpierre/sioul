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
                out.add("event:" + HomeCardRows.eventTitle(row.item, now) + (HomeCardRows.eventTime(row.item, now).isEmpty() ? "" : " (" + HomeCardRows.eventTime(row.item, now) + ")"));
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
        // An event over several days (the ongoing round): once, under its first day with its
        // whole span; on the days after, under today, its one line "Ongoing: …, until …".
        JSONObject trip = event("Trip", DAY + 8 * HOUR, DAY + 2 * DAYS + 19 * HOUR).put("time", "08:00 – Sat 10 Oct 19:00")
            .put("since", DAY + DAYS).put("ongoing", "Ongoing: Trip, until Sat 10 Oct 19:00");
        JSONObject away = card(4);
        away.getJSONObject("agenda").put("events", new JSONArray().put(trip)).put("dates", new JSONArray());
        expect("an event over several days, its first day", said(away, morning), "Agenda | day:Today, Thu 8 Oct | event:Trip (08:00 – Sat 10 Oct 19:00)");
        expect("an event over several days, the day after", said(away, DAY + DAYS + 10 * HOUR), "Agenda | day:Today, Fri 9 Oct | event:Ongoing: Trip, until Sat 10 Oct 19:00");
        JSONObject alone = new JSONObject().put("v", 4).put("agenda", new JSONObject().put("events", new JSONArray().put(trip)));
        expect("next change, its ongoing line", HomeCardRows.nextChange(alone, morning), DAY + DAYS);
        // An older Sioul's card, without them: its title and time as before.
        JSONObject older = event("Trip", DAY + 8 * HOUR, DAY + 2 * DAYS + 19 * HOUR).put("time", "08:00 – Sat 10 Oct 19:00");
        expect("an older card's event", HomeCardRows.eventTitle(older, DAY + DAYS + 10 * HOUR) + " / " + HomeCardRows.eventTime(older, DAY + DAYS + 10 * HOUR), "Trip / 08:00 – Sat 10 Oct 19:00");
        // No event, dates all the same: listed, and no "nothing in your calendars".
        JSONObject noEvents = card(4);
        noEvents.getJSONObject("agenda").put("events", new JSONArray());
        expect("dates alone", said(noEvents, morning), "Agenda | day:Tomorrow, Fri 9 Oct | date:Rent · €650 (Payment · tomorrow) → reminder/sioul:budget/home/3 | day:Sat 10 Oct | date:CAF form (Date asked · two days left) → reminder/sioul:task/caf");
        JSONObject nothing = card(3);
        nothing.getJSONObject("agenda").put("events", new JSONArray());
        expect("version 3, nothing", said(nothing, morning), "Agenda | Nothing in your calendars for the coming month.");
        // The hours' default (review of the agendas' parts 1 and 2): a frame's "events_out" left
        // out, a work agenda's review tomorrow out of this evening, back at work in the morning;
        // nothing else to list, the hours' own line; a place out of reach changes nothing.
        JSONObject hours = card(4);
        hours.getJSONObject("agenda").put("dates", new JSONArray()).put("empty_hours", "The calendars these hours show have nothing for the coming month.")
            .put("events", new JSONArray().put(event("Review", DAY + DAYS + 10 * HOUR, DAY + DAYS + 11 * HOUR)).put(event("Choir", DAY + DAYS + 19 * HOUR, DAY + DAYS + 20 * HOUR)));
        JSONObject evening = new JSONObject().put("from", DAY + 17 * HOUR).put("until", DAY + DAYS + 9 * HOUR).put("kind", "leisure").put("status", "").put("events", true)
            .put("events_out", new JSONArray().put(0).put(7));
        hours.put("times", new JSONArray()
            .put(new JSONObject().put("from", DAY).put("until", DAY + 17 * HOUR).put("kind", "work").put("status", "Work until 17:00.").put("events", true))
            .put(evening)
            .put(new JSONObject().put("from", DAY + DAYS + 9 * HOUR).put("until", DAY + 3 * DAYS).put("kind", "work").put("status", "Work until 17:00.").put("events", true)));
        expect("hours: at work", said(hours, morning), "Agenda | day:Tomorrow, Fri 9 Oct | event:Review | event:Choir");
        expect("hours: this evening", said(hours, DAY + 18 * HOUR), "Agenda | day:Tomorrow, Fri 9 Oct | event:Choir");
        expect("hours: back at work", said(hours, DAY + DAYS + 9 * HOUR + 60_000), "Agenda | day:Today, Fri 9 Oct | event:Review | event:Choir");
        hours.getJSONObject("agenda").getJSONArray("events").remove(1);
        expect("hours: nothing else", said(hours, DAY + 18 * HOUR), "Agenda | The calendars these hours show have nothing for the coming month.");
        hours.getJSONObject("agenda").remove("empty_hours");
        expect("hours: an older card's line", said(hours, DAY + 18 * HOUR), "Agenda | Nothing in your calendars for the coming month.");
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
