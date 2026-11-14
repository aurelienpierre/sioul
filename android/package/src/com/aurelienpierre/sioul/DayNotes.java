// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.Context;
import android.util.Log;

import org.json.JSONException;
import org.json.JSONObject;

/**
 * The end of work's notice, the night's and the meals' (docs/reviews.md,
 * docs/health.md), as on a computer: "Work hours are over" with Close the
 * work day; as the night's wind-down begins, its own notice with Close the
 * whole day while that night's review is to give, and Options…; a meal's or
 * a nap's heads-up and its own at its time, with Options…. Rust decides what
 * and when, at their alarms, given with the events' (crates/sioul-app/src/
 * eventalarms.rs, reviews.rs, health.rs), and only while nobody is at a
 * computer, which tells them then; it shows them here, notices to know
 * (Notices). Each button opens Sioul on its sheet or its question by its
 * address (ReminderOpener), the notice then taken away; a tap on the notice
 * opens its thing. Never said from words kept with the list: Rust alone
 * says them, or nothing (EventAlarms.fallback).
 */
final class DayNotes
{
    private DayNotes() {}

    /**
     * Rust's notice (eventalarms.rs, `day_note`: `notice_json`), shown in its
     * place by its tag ("day:work:2026-10-14", "day:night:2026-10-14",
     * "day:need:2026-10-14:meal:0"), its alarm then done with. Any thread.
     *
     * @param context the application's
     * @param json Rust's words
     */
    static void show(Context context, String json)
    {
        final JSONObject said;
        try {
            said = new JSONObject(json);
        } catch (JSONException e) {
            Log.e(DoseAlarms.TAG, "End of the day: a notice that does not read (" + e.getClass().getSimpleName() + ").");
            return;
        }
        Notices.Said notice = Notices.Said.of(said);
        if (notice.tag.isEmpty())
            return;
        Notices.post(context, notice);
        // Said: its alarm has nothing left to do, after a restart neither.
        EventAlarms.done(context, notice.tag);
    }
}
