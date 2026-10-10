// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.appwidget.AppWidgetManager;
import android.content.Context;
import android.content.res.Configuration;
import android.os.Bundle;
import android.text.TextPaint;
import android.util.DisplayMetrics;
import android.util.TypedValue;
import android.view.View;
import android.widget.RemoteViews;

import org.json.JSONArray;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/**
 * Sioul's full card on the home screen (HomeCard draws it; docs/android.md,
 * "The card on the home screen"), from what Rust wrote (crates/sioul-app/src/
 * homecard.rs). Today first: the date, the weather now and the next four hours
 * on its row (the farthest left out first when they do not fit, never
 * wrapped), the next morning, afternoon, evening and night, the next seven days, and
 * Open-Meteo's credit. Then what the Porch has for you: a dose due, a code you
 * asked for and its warning, reminders, the calls declined, two events at
 * once. Then Now: what now is for, do-not-disturb, the next step, where you
 * stopped. The mail and the agenda are on their own cards.
 *
 * As the card shrinks, its parts are left out, the least needed first, today
 * kept longest: where you stopped, the step, two events at once, the calls,
 * the reminders, do-not-disturb, the code, the dose, then the seven days and
 * the parts of the day; the date, the weather now and what now is for stay.
 * Lines.of is the pure part (org.json alone), checked on a computer's JVM
 * (android/jvm-checks/CardCheck.java); views draws it.
 */
final class HomeCardToday
{
    /** The text lines, each a view of the layout. */
    static final int DATE = 0, WEATHER_LINE = 1, CREDIT = 2, DOSE = 3, CODE = 4, WARNING = 5,
                     REMINDER = 6, CALL = 9, OVERLAP = 11, NOTE = 12, STATUS = 13, DND = 14, STEP = 15, STOPPED = 16;
    /** Three reminders and two calls at most: the card is no list. */
    static final int REMINDERS = 3, CALLS = 2;
    private static final int LINES = 17;
    private static final int[] VIEWS = {
        R.id.sioul_home_card_date, R.id.sioul_home_card_weather_line, R.id.sioul_home_card_credit,
        R.id.sioul_home_card_dose, R.id.sioul_home_card_code, R.id.sioul_home_card_warning,
        R.id.sioul_home_card_reminder0, R.id.sioul_home_card_reminder1, R.id.sioul_home_card_reminder2,
        R.id.sioul_home_card_call0, R.id.sioul_home_card_call1, R.id.sioul_home_card_overlap,
        R.id.sioul_home_card_note, R.id.sioul_home_card_status, R.id.sioul_home_card_dnd,
        R.id.sioul_home_card_step, R.id.sioul_home_card_stopped,
    };
    /** Each line's text size (sp) and its most lines, as the layout has them. */
    private static final float[] SIZES = { 16, 13, 10, 13, 13, 12, 13, 13, 13, 13, 13, 13, 13, 14, 13, 14, 13 };
    private static final int[] MOST = { 1, 2, 1, 2, 1, 1, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 2 };
    /** The space above each line (dp), as the layout has it. */
    private static final float[] ABOVE = { 0, 4, 2, 0, 2, 0, 2, 2, 2, 2, 2, 2, 2, 0, 2, 2, 2 };
    /**
     * Kept while the card shrinks, the lowest first: today (the date, the
     * weather now or the line said instead, what now is for) always; then
     * the dose, a "not set up yet", the code, do-not-disturb, the reminders,
     * the calls, two events at once, the step, where you stopped. The parts
     * of the day and the seven days come before the dose (ranks 1 and 2).
     */
    private static final int[] RANKS = { 0, 0, 0, 3, 4, 4, 6, 6, 6, 7, 7, 8, 3, 0, 5, 9, 10 };
    private static final int PARTS_RANK = 1, DAYS_RANK = 2, LAST_RANK = 10;

    /** The next hours beside the weather now, four at most (weather.rs, CARD_HOURS). */
    static final int HOURS_SHOWN = 4;
    private static final int[] HOUR_CELLS = { R.id.sioul_home_card_hour0, R.id.sioul_home_card_hour1, R.id.sioul_home_card_hour2, R.id.sioul_home_card_hour3 };
    private static final int[][] HOURS = {
        { R.id.sioul_home_card_hour0_label, R.id.sioul_home_card_hour0_icon, R.id.sioul_home_card_hour0_temp, R.id.sioul_home_card_hour0_rain },
        { R.id.sioul_home_card_hour1_label, R.id.sioul_home_card_hour1_icon, R.id.sioul_home_card_hour1_temp, R.id.sioul_home_card_hour1_rain },
        { R.id.sioul_home_card_hour2_label, R.id.sioul_home_card_hour2_icon, R.id.sioul_home_card_hour2_temp, R.id.sioul_home_card_hour2_rain },
        { R.id.sioul_home_card_hour3_label, R.id.sioul_home_card_hour3_icon, R.id.sioul_home_card_hour3_temp, R.id.sioul_home_card_hour3_rain },
    };
    /** An hour's cell, at its narrowest (the layout's minWidth), in dp. */
    private static final int HOUR_DP = 48;
    private static final int[][] PARTS = {
        { R.id.sioul_home_card_part0, R.id.sioul_home_card_part0_label, R.id.sioul_home_card_part0_icon, R.id.sioul_home_card_part0_temp, R.id.sioul_home_card_part0_rain },
        { R.id.sioul_home_card_part1, R.id.sioul_home_card_part1_label, R.id.sioul_home_card_part1_icon, R.id.sioul_home_card_part1_temp, R.id.sioul_home_card_part1_rain },
        { R.id.sioul_home_card_part2, R.id.sioul_home_card_part2_label, R.id.sioul_home_card_part2_icon, R.id.sioul_home_card_part2_temp, R.id.sioul_home_card_part2_rain },
        { R.id.sioul_home_card_part3, R.id.sioul_home_card_part3_label, R.id.sioul_home_card_part3_icon, R.id.sioul_home_card_part3_temp, R.id.sioul_home_card_part3_rain },
    };
    private static final int[][] DAYS = {
        { R.id.sioul_home_card_day0, R.id.sioul_home_card_day0_label, R.id.sioul_home_card_day0_icon, R.id.sioul_home_card_day0_high, R.id.sioul_home_card_day0_low, R.id.sioul_home_card_day0_rain },
        { R.id.sioul_home_card_day1, R.id.sioul_home_card_day1_label, R.id.sioul_home_card_day1_icon, R.id.sioul_home_card_day1_high, R.id.sioul_home_card_day1_low, R.id.sioul_home_card_day1_rain },
        { R.id.sioul_home_card_day2, R.id.sioul_home_card_day2_label, R.id.sioul_home_card_day2_icon, R.id.sioul_home_card_day2_high, R.id.sioul_home_card_day2_low, R.id.sioul_home_card_day2_rain },
        { R.id.sioul_home_card_day3, R.id.sioul_home_card_day3_label, R.id.sioul_home_card_day3_icon, R.id.sioul_home_card_day3_high, R.id.sioul_home_card_day3_low, R.id.sioul_home_card_day3_rain },
        { R.id.sioul_home_card_day4, R.id.sioul_home_card_day4_label, R.id.sioul_home_card_day4_icon, R.id.sioul_home_card_day4_high, R.id.sioul_home_card_day4_low, R.id.sioul_home_card_day4_rain },
        { R.id.sioul_home_card_day5, R.id.sioul_home_card_day5_label, R.id.sioul_home_card_day5_icon, R.id.sioul_home_card_day5_high, R.id.sioul_home_card_day5_low, R.id.sioul_home_card_day5_rain },
        { R.id.sioul_home_card_day6, R.id.sioul_home_card_day6_label, R.id.sioul_home_card_day6_icon, R.id.sioul_home_card_day6_high, R.id.sioul_home_card_day6_low, R.id.sioul_home_card_day6_rain },
    };
    /** The weather's icons, as weather.rs names them (Breeze's), and their drawings (tools/make-weather-drawables.py). */
    private static final String[] ICON_NAMES = {
        "weather-clear-symbolic", "weather-clear-night-symbolic", "weather-few-clouds-symbolic", "weather-few-clouds-night-symbolic",
        "weather-clouds-symbolic", "weather-clouds-night-symbolic", "weather-overcast-symbolic", "weather-fog-symbolic",
        "weather-showers-scattered-day-symbolic", "weather-showers-scattered-night-symbolic", "weather-freezing-scattered-rain-symbolic",
        "weather-showers-day-symbolic", "weather-showers-night-symbolic", "weather-showers-symbolic", "weather-freezing-rain-symbolic",
        "weather-snow-day-symbolic", "weather-snow-night-symbolic", "weather-showers-scattered-symbolic", "weather-snow-scattered-symbolic",
        "weather-storm-day-symbolic", "weather-storm-night-symbolic", "weather-hail-symbolic", "weather-none-available-symbolic",
    };
    private static final int[] ICONS = {
        R.drawable.sioul_weather_clear, R.drawable.sioul_weather_clear_night, R.drawable.sioul_weather_few_clouds, R.drawable.sioul_weather_few_clouds_night,
        R.drawable.sioul_weather_clouds, R.drawable.sioul_weather_clouds_night, R.drawable.sioul_weather_overcast, R.drawable.sioul_weather_fog,
        R.drawable.sioul_weather_showers_scattered_day, R.drawable.sioul_weather_showers_scattered_night, R.drawable.sioul_weather_freezing_scattered_rain,
        R.drawable.sioul_weather_showers_day, R.drawable.sioul_weather_showers_night, R.drawable.sioul_weather_showers, R.drawable.sioul_weather_freezing_rain,
        R.drawable.sioul_weather_snow_day, R.drawable.sioul_weather_snow_night, R.drawable.sioul_weather_showers_scattered, R.drawable.sioul_weather_snow_scattered,
        R.drawable.sioul_weather_storm_day, R.drawable.sioul_weather_storm_night, R.drawable.sioul_weather_hail, R.drawable.sioul_weather_none_available,
    };

    private HomeCardToday()
    {
    }

    /** A cell of the weather: an hour, a part of the day, a day. */
    static final class Cell
    {
        String label = "", icon = "", temperature = "", low = "", rain = "", words = "";

        static Cell of(JSONObject slot)
        {
            Cell cell = new Cell();
            if (slot == null)
                return cell;
            cell.label = slot.optString("label");
            cell.icon = slot.optString("icon");
            // An hour or a part says "temperature"; a day, "high" and "low".
            cell.temperature = slot.has("high") ? slot.optString("high") : slot.optString("temperature");
            cell.low = slot.optString("low");
            cell.rain = slot.optString("rain");
            cell.words = slot.optString("words");
            return cell;
        }
    }

    /** What the full card says now, and when that changes next: the pure part. */
    static final class Lines
    {
        final String[] texts = new String[LINES];
        /** Where a tap on each line leads (HomeCardOpener's address); "" opens Sioul as its icon does. */
        final String[] opens = new String[LINES];
        String age = "";
        /** The weather now; none: no weather part. */
        Cell now;
        final List<Cell> hours = new ArrayList<>();
        final List<Cell> parts = new ArrayList<>();
        /** The days after today, seven at most. */
        final List<Cell> days = new ArrayList<>();
        /** When to draw again (Unix ms); never: Long.MAX_VALUE. */
        long next = Long.MAX_VALUE;

        Lines()
        {
            for (int i = 0; i < LINES; i++) {
                texts[i] = "";
                opens[i] = "";
            }
        }

        private void soon(long at, long now)
        {
            if (at > now && at < next)
                next = at;
        }

        /**
         * The lines for `now`: today's, then the frame's (the Porch's, Now's).
         * A card an older Sioul wrote (version 2) has no today and no
         * reminders: its other lines show; none at all, Java's own words.
         */
        static Lines of(JSONObject card, long now)
        {
            Lines lines = new Lines();
            if (card == null) {
                // Sioul not opened since it was installed: its own words, Java's.
                lines.texts[STATUS] = HomeCard.empty();
                return lines;
            }
            lines.today(card.optJSONObject("today"), now);
            JSONObject frame = HomeCard.frame(HomeCard.frames(card), now);
            if (frame == null) {
                // Past the last frame: Sioul was not opened for a day and more.
                lines.texts[STATUS] = card.optString("beyond");
            } else {
                lines.soon(frame.optLong("until"), now);
                lines.texts[STATUS] = frame.optString("status");
                lines.texts[DND] = frame.optString("dnd");
                lines.texts[NOTE] = frame.optString("note");
                // The step's title alone: why it comes now is on Now, where a tap leads.
                JSONObject step = frame.optJSONObject("step");
                if (step != null) {
                    lines.texts[STEP] = step.optString("title");
                    lines.opens[STEP] = "now/" + step.optString("uid");
                }
                if (frame.optBoolean("stopped")) {
                    lines.texts[STOPPED] = card.optString("stopped");
                    lines.opens[STOPPED] = "now";
                }
                if (frame.optBoolean("codes"))
                    lines.code(card.optJSONArray("codes"), now);
                if (frame.optBoolean("doses"))
                    lines.doses(card.optJSONArray("doses"), now);
                lines.reminders(card.optJSONArray("reminders"), frame.optJSONArray("reminders"), now);
                lines.calls(card.optJSONArray("calls"), frame.optJSONArray("calls"));
                if (frame.optBoolean("overlaps"))
                    lines.overlap(card.optJSONArray("overlaps"), now);
            }
            lines.age(card, now);
            return lines;
        }

        /** The date, then the weather: the hour under way's, and the days after today. */
        private void today(JSONObject today, long now)
        {
            if (today == null)
                return;
            JSONArray date = today.optJSONArray("date");
            texts[DATE] = HomeCardRows.label(date, now);
            soon(HomeCardRows.ahead(date, now), now);
            JSONObject weather = today.optJSONObject("weather");
            if (weather == null) {
                texts[WEATHER_LINE] = today.optString("line");
                return;
            }
            texts[CREDIT] = weather.optString("credit");
            JSONObject hour = within(weather.optJSONArray("hours"), now);
            if (hour != null) {
                soon(hour.optLong("until"), now);
                this.now = Cell.of(hour.optJSONObject("now"));
                add(hours, hour.optJSONArray("next"), HOURS_SHOWN);
                add(parts, hour.optJSONArray("parts"), 4);
            }
            // The days after today: from the end of the day now falls in.
            JSONArray all = weather.optJSONArray("days");
            JSONObject current = within(all, now);
            long after = current == null ? now : current.optLong("until");
            soon(after, now);
            for (int i = 0; all != null && i < all.length() && days.size() < 7; i++) {
                JSONObject day = all.optJSONObject(i);
                if (day != null && day.optLong("from") >= after)
                    days.add(Cell.of(day));
            }
            // No weather left to say (the forecast's hours and days are over): its credit goes with it.
            if (this.now == null && days.isEmpty())
                texts[CREDIT] = "";
        }

        private static void add(List<Cell> cells, JSONArray slots, int most)
        {
            for (int i = 0; slots != null && i < slots.length() && cells.size() < most; i++)
                cells.add(Cell.of(slots.optJSONObject(i)));
        }

        /** The newest code still valid; the next to expire, the next drawing. */
        private void code(JSONArray codes, long now)
        {
            for (int i = 0; codes != null && i < codes.length(); i++) {
                JSONObject code = codes.optJSONObject(i);
                long until = code == null ? 0 : code.optLong("until");
                if (until <= now)
                    continue;
                soon(until, now);
                if (texts[CODE].isEmpty()) {
                    texts[CODE] = code.optString("line");
                    texts[WARNING] = code.optString("warning");
                    opens[CODE] = "porch";
                    opens[WARNING] = "porch";
                }
            }
        }

        /**
         * The doses due now, in one line: each as Rust said it while it is known
         * not marked elsewhere, then with "check before taking it". Never more.
         */
        private void doses(JSONArray doses, long now)
        {
            StringBuilder line = new StringBuilder();
            for (int i = 0; doses != null && i < doses.length(); i++) {
                JSONObject dose = doses.optJSONObject(i);
                if (dose == null)
                    continue;
                long from = dose.optLong("from"), until = dose.optLong("until"), known = dose.optLong("known_until");
                soon(from, now);
                if (from > now || until <= now)
                    continue;
                soon(until, now);
                soon(known, now);
                if (line.length() > 0)
                    line.append("; ");
                line.append(known > now ? dose.optString("line") : dose.optString("check"));
            }
            texts[DOSE] = line.toString();
            opens[DOSE] = "porch";
        }

        /** The reminders the frame shows, each from its time while it makes sense; the same words once. */
        private void reminders(JSONArray all, JSONArray shown, long now)
        {
            int at = 0;
            for (int i = 0; all != null && shown != null && i < shown.length(); i++) {
                JSONObject reminder = all.optJSONObject(shown.optInt(i, -1));
                if (reminder == null)
                    continue;
                long from = reminder.optLong("from"), until = reminder.optLong("until");
                soon(from, now);
                if (from > now || until <= now)
                    continue;
                soon(until, now);
                String line = reminder.optString("line");
                if (at >= REMINDERS || line.isEmpty() || said(REMINDER, at, line))
                    continue;
                String open = reminder.optString("open");
                texts[REMINDER + at] = line;
                opens[REMINDER + at] = open.isEmpty() ? "porch" : "reminder/" + open;
                at++;
            }
        }

        /** Whether one of the first `count` lines from `first` says `line` already. */
        private boolean said(int first, int count, String line)
        {
            for (int i = 0; i < count; i++)
                if (texts[first + i].equals(line))
                    return true;
            return false;
        }

        /** The calls declined the frame lists, in the Porch's words. */
        private void calls(JSONArray all, JSONArray listed)
        {
            int at = 0;
            for (int i = 0; all != null && listed != null && i < listed.length() && at < CALLS; i++) {
                String line = all.optString(listed.optInt(i, -1), "");
                if (line.isEmpty() || said(CALL, at, line))
                    continue;
                texts[CALL + at] = line;
                opens[CALL + at] = "porch";
                at++;
            }
        }

        /** Two events at once, on their day until both are over. */
        private void overlap(JSONArray overlaps, long now)
        {
            for (int i = 0; overlaps != null && i < overlaps.length(); i++) {
                JSONObject overlap = overlaps.optJSONObject(i);
                if (overlap == null)
                    continue;
                long from = overlap.optLong("from"), until = overlap.optLong("until");
                soon(from, now);
                if (from > now || until <= now)
                    continue;
                soon(until, now);
                if (texts[OVERLAP].isEmpty()) {
                    texts[OVERLAP] = overlap.optString("line");
                    opens[OVERLAP] = "porch";
                }
            }
        }

        /** "at 14:05" once the card is a quarter of an hour old. */
        private void age(JSONObject card, long now)
        {
            long after = card.optLong("stale_after");
            if (now < after) {
                soon(after, now);
                return;
            }
            JSONArray labels = card.optJSONArray("stale");
            age = HomeCardRows.label(labels, now);
            soon(HomeCardRows.ahead(labels, now), now);
        }
    }

    /** The item `now` falls in, among items each {from, until}; none. */
    static JSONObject within(JSONArray items, long now)
    {
        for (int i = 0; items != null && i < items.length(); i++) {
            JSONObject item = items.optJSONObject(i);
            if (item != null && item.optLong("from") <= now && now < item.optLong("until"))
                return item;
        }
        return null;
    }

    /** A weather icon's drawing, by weather.rs's name; one it does not know, "none available". */
    static int icon(String name)
    {
        for (int i = 0; i < ICON_NAMES.length; i++)
            if (ICON_NAMES[i].equals(name))
                return ICONS[i];
        return R.drawable.sioul_weather_none_available;
    }

    // ---------------------------------------------------------------- drawn

    /**
     * The full card at its size: each part kept while it fits, the most
     * needed first (RANKS), and its taps ({@link HomeCard#opener}, through
     * HomeCardOpener); the rest opens Sioul as its icon does.
     */
    static RemoteViews views(Context context, Lines lines, Bundle options)
    {
        RemoteViews views = new RemoteViews(context.getPackageName(), R.layout.sioul_home_card_today);
        DisplayMetrics metrics = context.getResources().getDisplayMetrics();
        boolean upright = context.getResources().getConfiguration().orientation != Configuration.ORIENTATION_LANDSCAPE;
        int widthDp = options == null ? 0 : options.getInt(upright ? AppWidgetManager.OPTION_APPWIDGET_MIN_WIDTH : AppWidgetManager.OPTION_APPWIDGET_MAX_WIDTH);
        int heightDp = options == null ? 0 : options.getInt(upright ? AppWidgetManager.OPTION_APPWIDGET_MAX_HEIGHT : AppWidgetManager.OPTION_APPWIDGET_MIN_HEIGHT);
        // Not said yet (a card being placed): about four cells by four.
        if (widthDp <= 0)
            widthDp = 300;
        if (heightDp <= 0)
            heightDp = 300;
        Fit fit = new Fit(lines, metrics, widthDp, heightDp);

        for (int i = 0; i < LINES; i++) {
            boolean shown = fit.kept[i];
            views.setTextViewText(VIEWS[i], shown ? lines.texts[i] : "");
            views.setViewVisibility(VIEWS[i], shown ? View.VISIBLE : View.GONE);
            if (shown && !lines.opens[i].isEmpty())
                views.setOnClickPendingIntent(VIEWS[i], HomeCard.opener(context, lines.opens[i]));
        }
        views.setTextViewText(R.id.sioul_home_card_age, lines.age);
        views.setViewVisibility(R.id.sioul_home_card_age, lines.age.isEmpty() ? View.GONE : View.VISIBLE);

        // The weather now, and the next hours beside it on the same row: as many as fit, the nearest first.
        views.setViewVisibility(R.id.sioul_home_card_now, lines.now == null ? View.GONE : View.VISIBLE);
        if (lines.now != null) {
            views.setImageViewResource(R.id.sioul_home_card_now_icon, icon(lines.now.icon));
            views.setContentDescription(R.id.sioul_home_card_now, join(lines.now.words, lines.now.temperature, lines.now.rain));
            views.setTextViewText(R.id.sioul_home_card_now_temp, lines.now.temperature);
            // The chance of rain alone under it, as the hours have it: its words would push them off the row.
            views.setTextViewText(R.id.sioul_home_card_now_rain, lines.now.rain);
            views.setViewVisibility(R.id.sioul_home_card_hours, fit.hours > 0 ? View.VISIBLE : View.GONE);
            for (int i = 0; i < HOURS.length; i++) {
                boolean shown = i < fit.hours;
                views.setViewVisibility(HOUR_CELLS[i], shown ? View.VISIBLE : View.GONE);
                if (shown)
                    cell(views, HOURS[i], lines.hours.get(i), false);
            }
        }
        views.setViewVisibility(R.id.sioul_home_card_parts, fit.parts ? View.VISIBLE : View.GONE);
        for (int i = 0; i < PARTS.length; i++) {
            boolean shown = fit.parts && i < lines.parts.size();
            views.setViewVisibility(PARTS[i][0], shown ? View.VISIBLE : View.GONE);
            if (shown)
                cell(views, Arrays.copyOfRange(PARTS[i], 1, 5), lines.parts.get(i), false);
        }
        views.setViewVisibility(R.id.sioul_home_card_days, fit.days > 0 ? View.VISIBLE : View.GONE);
        for (int i = 0; i < DAYS.length; i++) {
            boolean shown = i < fit.days;
            views.setViewVisibility(DAYS[i][0], shown ? View.VISIBLE : View.GONE);
            if (shown)
                cell(views, Arrays.copyOfRange(DAYS[i], 1, 6), lines.days.get(i), true);
        }
        // The Porch's part, its space above it only when it says something.
        boolean porch = false;
        for (int i = DOSE; i <= NOTE; i++)
            porch |= fit.kept[i];
        views.setViewVisibility(R.id.sioul_home_card_porch, porch ? View.VISIBLE : View.GONE);
        views.setOnClickPendingIntent(android.R.id.background, HomeCard.launch(context));
        return views;
    }

    /** A cell's views (label, icon, temperature, [low,] rain) said, and its words for screen readers. */
    private static void cell(RemoteViews views, int[] ids, Cell cell, boolean day)
    {
        views.setTextViewText(ids[0], cell.label);
        views.setImageViewResource(ids[1], icon(cell.icon));
        views.setContentDescription(ids[1], cell.words);
        views.setTextViewText(ids[2], cell.temperature);
        if (day) {
            views.setTextViewText(ids[3], cell.low);
            views.setTextViewText(ids[4], cell.rain);
        } else {
            views.setTextViewText(ids[3], cell.rain);
        }
    }

    /** Some words for screen readers, the empty ones left out: "Rain, 16°, 98 %". */
    private static String join(String... parts)
    {
        StringBuilder said = new StringBuilder();
        for (String part : parts) {
            if (part.isEmpty())
                continue;
            if (said.length() > 0)
                said.append(", ");
            said.append(part);
        }
        return said.toString();
    }

    /** What fits in the card's height, the most needed first; and what its width allows. */
    private static final class Fit
    {
        final boolean[] kept = new boolean[LINES];
        boolean parts;
        /** The next hours shown beside the weather now; the days shown. */
        int hours, days;

        Fit(Lines lines, DisplayMetrics metrics, int widthDp, int heightDp)
        {
            float density = metrics.density;
            float width = (widthDp - 32) * density;
            // Beside the weather now (its icon, its temperature or its chance of rain, the
            // widest), as many hours as fit, the farthest left out first: never a second row.
            if (lines.now != null) {
                float now = (36 + 8 + 8) * density + Math.max(paint(24, metrics).measureText(lines.now.temperature), paint(12, metrics).measureText(lines.now.rain));
                hours = Math.max(0, Math.min(Math.min(HOURS_SHOWN, lines.hours.size()), (int) Math.floor((width - now) / (HOUR_DP * density))));
            }
            // Seven days of 36 dp at least; fewer on a narrow card, four at least.
            int columns = Math.max(4, Math.min(7, (widthDp - 32) / 36));
            float room = (heightDp - 22) * density;
            // Now's part is always there (what now is for), its space above it too.
            float used = 10 * density;
            // The weather now: its row, as tall as its hours' cells when they show.
            if (lines.now != null) {
                float hour = line(11, metrics) + 22 * density + line(13, metrics) + line(11, metrics);
                float alone = Math.max(36 * density, line(24, metrics) + line(12, metrics));
                used += (hours > 0 ? Math.max(hour, alone) : alone) + 6 * density;
            }
            boolean porchGap = false;
            for (int rank = 0; rank <= LAST_RANK; rank++) {
                if (rank == PARTS_RANK && !lines.parts.isEmpty()) {
                    float height = line(11, metrics) + 22 * density + line(12, metrics) + line(11, metrics) + 8 * density;
                    if (used + height <= room) {
                        used += height;
                        parts = true;
                    }
                }
                if (rank == DAYS_RANK && !lines.days.isEmpty()) {
                    float height = line(11, metrics) + 20 * density + line(12, metrics) + line(11, metrics) + line(10, metrics) + 8 * density;
                    if (used + height <= room) {
                        used += height;
                        days = Math.min(columns, lines.days.size());
                    }
                }
                for (int i = 0; i < LINES; i++) {
                    if (RANKS[i] != rank || lines.texts[i].isEmpty())
                        continue;
                    // The warning goes with its code.
                    if (i == WARNING && !kept[CODE])
                        continue;
                    float height = lines(lines.texts[i], i, metrics, width) + ABOVE[i] * density;
                    boolean porch = i >= DOSE && i <= NOTE;
                    if (porch && !porchGap)
                        height += 10 * density;
                    // Today's lines and what now is for stay, whatever the height.
                    if (rank > 0 && used + height > room)
                        continue;
                    used += height;
                    kept[i] = true;
                    porchGap |= porch;
                }
            }
        }

        /** A line's height, its text wrapped on its most lines. */
        private static float lines(String text, int i, DisplayMetrics metrics, float width)
        {
            TextPaint paint = paint(SIZES[i], metrics);
            int count = width <= 0 ? 1 : (int) Math.ceil(paint.measureText(text) / width);
            return paint.getFontSpacing() * Math.max(1, Math.min(MOST[i], count));
        }

        private static float line(float sp, DisplayMetrics metrics)
        {
            return paint(sp, metrics).getFontSpacing();
        }

        private static TextPaint paint(float sp, DisplayMetrics metrics)
        {
            TextPaint paint = new TextPaint(TextPaint.ANTI_ALIAS_FLAG);
            paint.setTextSize(TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, sp, metrics));
            return paint;
        }
    }
}
