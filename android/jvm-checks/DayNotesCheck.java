// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.json.JSONArray;
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
 * - EventAlarms.isDay: no words kept for them (EventAlarms.fallback);
 * - EventAlarms.merged, asked, answered: fifteen minutes second by second,
 *   the end of work's entry listed past its time and held by a pause, Rust
 *   giving the list again every second (an event held): its alarm set a few
 *   times, never again and again (`ringLoop`).
 */
public final class DayNotesCheck
{
    static int failed = 0;
    static int checked = 0;

    /** "Work hours are over", at the end of the day's last work or admin hours. */
    static final String WORK = "{\"key\":\"day:work:2026-10-14\",\"title\":\"Work hours are over\",\"body\":\"The work day can be closed, with a word on how it went if you like.\",\"buttons\":[{\"label\":\"Close the work day\",\"kind\":\"review\",\"key\":\"work:2026-10-14\"}],\"until\":1792000000000,\"words\":{\"day\":\"End of the day\"}}";
    /** The night's own notice, as its wind-down begins, its review still to give. */
    static final String NIGHT = "{\"key\":\"day:night:2026-10-14\",\"title\":\"Wind down\",\"body\":\"22:30\",\"buttons\":[{\"label\":\"Close the whole day\",\"kind\":\"review\",\"key\":\"night:2026-10-14\"},{\"label\":\"Options…\",\"kind\":\"need\",\"key\":\"sleep\"}],\"until\":1792000000000,\"words\":{\"day\":\"End of the day\"}}";
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

    /**
     * How many times the end of work's alarm is set in fifteen simulated
     * minutes: its entry listed at its time, a minute past, while its ten
     * minutes last (reviews.rs, coming_work); an event's reminder held, asked
     * again a quarter of an hour on, so that Rust gives the list again every
     * second (the worst case: before 15 October 2026 its time moved each
     * second); each ring of the day's alarm giving the list again first
     * (eventalarms.rs, decide), then answering as decide_work does while a
     * pause holds the notice: again a minute on while its ten minutes last,
     * then nothing. Android's alarms stand in as one per key.
     *
     * @param before true: every listed alarm set again at each list, as
     *     before 15 October 2026; false: EventAlarms.merged
     */
    static int ringLoop(boolean before) throws Exception
    {
        final long start = 1_792_000_000_000L;
        final long end = start - 60_000;
        final String day = "day:work:2026-10-14";
        final Map<String, Long> alarms = new HashMap<>();
        JSONArray kept = new JSONArray();
        int arms = 0;
        for (long now = start; now < start + 15 * 60_000; now += 1000) {
            for (int given = 0; given < 2; given++) {
                // given 0: the list, given again this second; given 1: the day's alarm rings, if due.
                if (given == 1) {
                    Long ring = alarms.get(day);
                    if (ring == null || ring > now)
                        break;
                    alarms.remove(day);
                }
                JSONArray coming = new JSONArray();
                if (now - end < 10 * 60_000)
                    coming.put(new JSONObject().put("key", day).put("kind", "day").put("at", end));
                coming.put(new JSONObject().put("key", "before:dentist").put("kind", "before").put("at", now + 15 * 60_000));
                // set(): what is no longer listed cancelled, the rest armed as said.
                if (coming.length() < 2)
                    alarms.remove(day);
                List<EventAlarms.Arm> armed = new ArrayList<>();
                if (before) {
                    kept = coming;
                    for (int i = 0; i < coming.length(); i++)
                        armed.add(new EventAlarms.Arm(coming.getJSONObject(i).getString("key"), coming.getJSONObject(i).getLong("at")));
                } else {
                    kept = EventAlarms.merged(kept, coming, now, armed);
                }
                for (EventAlarms.Arm arm : armed) {
                    alarms.put(arm.key, arm.at);
                    if (arm.key.equals(day))
                        arms++;
                }
                if (given == 1) {
                    // Rust's answer: held by the pause, again a minute on while its ten minutes last.
                    if (now + 60_000 - end < 10 * 60_000) {
                        kept = EventAlarms.asked(kept, day, now + 60_000);
                        alarms.put(day, now + 60_000);
                        arms++;
                    } else {
                        kept = EventAlarms.answered(kept, day);
                    }
                }
            }
        }
        return arms;
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
        expect("the end of work", buttons(WORK), "Close the work day: review work:2026-10-14");
        expect("the night, its review to give", buttons(NIGHT), "Close the whole day: review night:2026-10-14 | Options…: need sleep");
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
        expect("the work day's sheet, its day said", ReminderOpener.opens("review", "work:2026-10-14"), true);
        expect("the whole day's sheet, its day said", ReminderOpener.opens("review", "night:2026-10-14"), true);
        expect("a day that is none", ReminderOpener.opens("review", "work:yesterday"), false);
        expect("a sheet without words", ReminderOpener.opens("review", null), false);
        expect("another sheet", ReminderOpener.opens("review", "settings"), false);
        expect("a block's question", ReminderOpener.opens("need", "sleep"), true);
        expect("an event", ReminderOpener.opens("event", "work/1.ics"), true);
        expect("the Porch", ReminderOpener.opens("porch", null), true);
        expect("the app alone", ReminderOpener.opens("app", null), false);
        expect("nothing", ReminderOpener.opens(null, null), false);
        // A past entry, a list given every second: its alarm set a few times, never again and again.
        int before = ringLoop(true);
        int now = ringLoop(false);
        expect("before 15 October 2026, set again and again (" + before + " times)", before >= 600, true);
        expect("set at most once a minute while its ten minutes last (" + now + " times)", now <= 12, true);
        expect("set at least once", now >= 1, true);
        System.out.println("the end of work's alarm, 15 minutes, a list every second: set " + now + " times (" + before + " as before 15 October 2026)");
        // Done with, then listed again: never set again; moved into the future by Rust: set.
        List<EventAlarms.Arm> armed = new ArrayList<>();
        JSONArray kept = EventAlarms.answered(new JSONArray("[{\"key\":\"day:night:2026-10-14\",\"at\":1000}]"), "day:night:2026-10-14");
        kept = EventAlarms.merged(kept, new JSONArray("[{\"key\":\"day:night:2026-10-14\",\"at\":1000}]"), 5000, armed);
        expect("done, listed again in the past: not set", armed.size(), 0);
        expect("and still done", kept.getJSONObject(0).optBoolean(EventAlarms.DONE), true);
        kept = EventAlarms.merged(kept, new JSONArray("[{\"key\":\"day:night:2026-10-14\",\"at\":9000}]"), 5000, armed);
        expect("moved into the future: set", armed.size(), 1);
        expect("no longer done", kept.getJSONObject(0).optBoolean(EventAlarms.DONE), false);
        // New to the list, though past: set, to ring at once.
        armed.clear();
        EventAlarms.merged(new JSONArray(), new JSONArray("[{\"key\":\"day:work:2026-10-14\",\"at\":1000}]"), 5000, armed);
        expect("new and past: set", armed.size(), 1);
        // Asked again: its own time kept though Rust lists the first one again.
        kept = EventAlarms.asked(new JSONArray("[{\"key\":\"day:work:2026-10-14\",\"at\":1000}]"), "day:work:2026-10-14", 65000);
        armed.clear();
        kept = EventAlarms.merged(kept, new JSONArray("[{\"key\":\"day:work:2026-10-14\",\"at\":1000}]"), 5000, armed);
        expect("asked again: not set over", armed.size(), 0);
        expect("its time kept", kept.getJSONObject(0).getLong("at"), 65000);
        // Moved earlier by Rust, into the past, not done: set again, to ring at once. The end of
        // work kept at 17:30, not rung; at 16:57 the hours are edited to end at 16:50.
        long half = 1_792_000_000_000L;
        armed.clear();
        kept = EventAlarms.merged(new JSONArray(), new JSONArray().put(new JSONObject().put("key", "day:work:2026-10-14").put("at", half)), half - 3_600_000, armed);
        armed.clear();
        kept = EventAlarms.merged(kept, new JSONArray().put(new JSONObject().put("key", "day:work:2026-10-14").put("at", half - 2_400_000)), half - 1_980_000, armed);
        expect("the end moved a few minutes into the past: set, at once", armed.size() == 1 && armed.get(0).at == half - 2_400_000, true);
        armed.clear();
        kept = EventAlarms.merged(kept, new JSONArray().put(new JSONObject().put("key", "day:work:2026-10-14").put("at", half - 2_400_000)), half - 1_970_000, armed);
        expect("listed so again: not set again", armed.size(), 0);
        // A reminder held (its alarm at the next quarter hour), its hold ended early: listed at
        // Rust's second plus one, which Java may only see a second later: set, at once.
        JSONArray held = EventAlarms.asked(EventAlarms.merged(new JSONArray(), new JSONArray().put(new JSONObject().put("key", "before:dentist").put("at", half + 900_000)), half, new ArrayList<>()), "before:dentist", half + 900_000);
        armed.clear();
        held = EventAlarms.merged(held, new JSONArray().put(new JSONObject().put("key", "before:dentist").put("at", half + 181_000)), half + 182_200, armed);
        expect("a hold ended early, seen late: set, at once", armed.size() == 1 && armed.get(0).at == half + 181_000, true);
        // Then listed a second from now at each second: a time that grows, set no more.
        int again = 0;
        for (long second = 183_000; second < 243_000; second += 1000) {
            armed.clear();
            held = EventAlarms.merged(held, new JSONArray().put(new JSONObject().put("key", "before:dentist").put("at", half + second)), half + second + 1200, armed);
            again += armed.size();
        }
        expect("a second from now, each second for a minute: set no more", again, 0);
        // Done with, then moved earlier into the past: left done, not set.
        armed.clear();
        JSONArray done = EventAlarms.answered(new JSONArray().put(new JSONObject().put("key", "day:night:2026-10-14").put("at", half).put(EventAlarms.GIVEN, half)), "day:night:2026-10-14");
        EventAlarms.merged(done, new JSONArray().put(new JSONObject().put("key", "day:night:2026-10-14").put("at", half - 60_000)), half + 60_000, armed);
        expect("done, moved earlier: not set", armed.size(), 0);
        // No words kept for the day's: Rust says them, or nothing.
        expect("the day's", EventAlarms.isDay("day"), true);
        expect("an event's", EventAlarms.isDay("event"), false);
        expect("an older list", EventAlarms.isDay(""), false);
        System.out.println(failed == 0 ? "all " + checked + " checks agree" : failed + " of " + checked + " failed");
        System.exit(failed == 0 ? 0 : 1);
    }
}
