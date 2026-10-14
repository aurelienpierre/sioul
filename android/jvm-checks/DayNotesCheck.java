// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.util.ArrayList;
import java.util.List;

import org.json.JSONObject;

/**
 * The end of work's notice and the night's, on the JVM (org.json's own jar
 * before android.jar's stubs on the class path), as Rust gives them
 * (eventalarms.rs, `day_note`: WORK, NIGHT and GIVEN below are its words,
 * which eventalarms::tests::the_days_notices_as_java_reads_them makes again
 * and compares with this file's):
 * - DayNotes.buttons: each notice's buttons, in their order, with what each
 *   opens; three at most; none that Sioul's notices do not open;
 * - DayNotes.requestCode: the tap and each button kept apart;
 * - ReminderOpener.opens: what a press may open, as Rust's `opened_of`;
 * - EventAlarms.isDay: no words kept for them (EventAlarms.fallback).
 */
public final class DayNotesCheck
{
    static int failed = 0;
    static int checked = 0;

    /** "Work hours are over", at the end of the day's last work or admin hours. */
    static final String WORK = "{\"key\":\"day:work:2026-10-14\",\"title\":\"Work hours are over\",\"body\":\"The work day can be closed, with a word on how it went if you like.\",\"buttons\":[{\"label\":\"Close the work day\",\"kind\":\"review\",\"key\":\"work\"}],\"until\":1792000000000,\"words\":{\"day\":\"End of the day\"}}";
    /** The night's own notice, as its wind-down begins, its review still to give. */
    static final String NIGHT = "{\"key\":\"day:night:2026-10-14\",\"title\":\"Wind down\",\"body\":\"22:30\",\"buttons\":[{\"label\":\"Close the whole day\",\"kind\":\"review\",\"key\":\"night\"},{\"label\":\"Options…\",\"kind\":\"need\",\"key\":\"sleep\"}],\"until\":1792000000000,\"words\":{\"day\":\"End of the day\"}}";
    /** The same, that night's review given already: its options alone. */
    static final String GIVEN = "{\"key\":\"day:night:2026-10-14\",\"title\":\"Wind down\",\"body\":\"22:30\",\"buttons\":[{\"label\":\"Options…\",\"kind\":\"need\",\"key\":\"sleep\"}],\"until\":1792000000000,\"words\":{\"day\":\"End of the day\"}}";

    static void expect(String what, Object got, Object wanted)
    {
        checked++;
        if (!String.valueOf(got).equals(String.valueOf(wanted))) {
            failed++;
            System.out.println("FAIL " + what + ": " + got + " (expected " + wanted + ")");
        }
    }

    /** "label: kind key | …", the buttons as Android shows them. */
    static String buttons(String json) throws Exception
    {
        List<String> out = new ArrayList<>();
        for (DayNotes.Button button : DayNotes.buttons(new JSONObject(json)))
            out.add(button.label + ": " + button.kind + " " + button.key);
        return String.join(" | ", out);
    }

    public static void main(String[] args) throws Exception
    {
        expect("the end of work", buttons(WORK), "Close the work day: review work");
        expect("the night, its review to give", buttons(NIGHT), "Close the whole day: review night | Options…: need sleep");
        expect("the night, its review given", buttons(GIVEN), "Options…: need sleep");
        // What a notice of Sioul's does not open, a button without words: left out.
        expect("strangers left out", buttons("{\"buttons\":[{\"label\":\"Settings\",\"kind\":\"settings:words\",\"key\":\"\"},{\"label\":\"Close\",\"kind\":\"review\",\"key\":\"settings\"},{\"label\":\"\",\"kind\":\"review\",\"key\":\"work\"},{\"label\":\"Close the work day\",\"kind\":\"review\",\"key\":\"work\"}]}"), "Close the work day: review work");
        // Android shows three buttons at most.
        expect("three at most", DayNotes.buttons(new JSONObject("{\"buttons\":[{\"label\":\"a\",\"kind\":\"porch\",\"key\":\"\"},{\"label\":\"b\",\"kind\":\"porch\",\"key\":\"\"},{\"label\":\"c\",\"kind\":\"porch\",\"key\":\"\"},{\"label\":\"d\",\"kind\":\"porch\",\"key\":\"\"}]}")).size(), 3);
        expect("nothing given", DayNotes.buttons(new JSONObject("{}")).size(), 0);
        expect("nothing at all", DayNotes.buttons(null).size(), 0);
        // The tap on the notice and each button, kept apart; and from another notice's.
        int tap = DayNotes.requestCode("day:night:2026-10-14", -1);
        int close = DayNotes.requestCode("day:night:2026-10-14", 0);
        int options = DayNotes.requestCode("day:night:2026-10-14", 1);
        expect("presses apart", tap != close && close != options && tap != options, true);
        expect("notices apart", DayNotes.requestCode("day:work:2026-10-14", 0) != close, true);
        // What a press may open: as Rust's eventalarms::opened_of.
        expect("the work day's sheet", ReminderOpener.opens("review", "work"), true);
        expect("the whole day's sheet", ReminderOpener.opens("review", "night"), true);
        expect("another sheet", ReminderOpener.opens("review", "settings"), false);
        expect("a block's question", ReminderOpener.opens("need", "sleep"), true);
        expect("an event", ReminderOpener.opens("event", "work/1.ics"), true);
        expect("the Porch", ReminderOpener.opens("porch", null), true);
        expect("the app alone", ReminderOpener.opens("app", null), false);
        expect("nothing", ReminderOpener.opens(null, null), false);
        // No words kept for the day's: Rust says them, or nothing.
        expect("the day's", EventAlarms.isDay("day"), true);
        expect("an event's", EventAlarms.isDay("event"), false);
        expect("an older list", EventAlarms.isDay(""), false);
        System.out.println(failed == 0 ? "all " + checked + " checks agree" : failed + " of " + checked + " failed");
        System.exit(failed == 0 ? 0 : 1);
    }
}
