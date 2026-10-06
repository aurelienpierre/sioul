// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.AlarmManager;
import android.app.PendingIntent;
import android.appwidget.AppWidgetManager;
import android.appwidget.AppWidgetProvider;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.res.Configuration;
import android.graphics.Typeface;
import android.net.Uri;
import android.os.Bundle;
import android.text.SpannableStringBuilder;
import android.text.Spanned;
import android.text.StaticLayout;
import android.text.TextPaint;
import android.text.style.StyleSpan;
import android.util.DisplayMetrics;
import android.util.Log;
import android.util.TypedValue;
import android.view.View;
import android.widget.RemoteViews;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Locale;

/**
 * Sioul's card on the home screen (docs/android.md, "The card on the home
 * screen"): what now is for, the Porch as it shows now, the next step when it
 * is the time for one, a dose due. Rust writes what it says (crates/sioul-app/
 * src/homecard.rs) in Sioul's state folder, ahead, in frames: one per change
 * of time until the end of tomorrow, since Android freezes Sioul in the
 * background. Here: the frame for now, drawn again at its end by an alarm that
 * wakes nothing, and whenever Rust says so (HOME_CARD, sent through android/
 * main.cpp's sioul_android_broadcast). No Rust, no network: a card drawn from
 * a file. A tap opens Sioul; on the Porch's lines, the Porch; on the step, Now
 * (HomeCardOpener); no button does anything else.
 */
public final class HomeCard extends AppWidgetProvider
{
    static final String DRAW = "com.aurelienpierre.sioul.action.HOME_CARD";
    /** Rust's state folder (android/main.cpp: XDG_STATE_HOME is files/state). */
    private static final String FOLDER = "state/sioul";
    private static final String FILE = "home-card.json";
    private static final String OPENED = "home-card-opened";
    private static final String SCHEME = "sioul-card";
    /** The card's lines (where each shows is the layout's). */
    private static final int STATUS = 0, CODE = 1, WARNING = 2, SUMMARY = 3, ITEM_1 = 4, ITEM_2 = 5, ITEM_3 = 6, STEP = 7, WHY = 8, DOSE = 9, DND = 10;
    private static final int[] VIEWS = {
        R.id.sioul_home_card_status, R.id.sioul_home_card_code, R.id.sioul_home_card_warning, R.id.sioul_home_card_summary,
        R.id.sioul_home_card_item_1, R.id.sioul_home_card_item_2, R.id.sioul_home_card_item_3,
        R.id.sioul_home_card_step_title, R.id.sioul_home_card_step_why, R.id.sioul_home_card_dose, R.id.sioul_home_card_dnd,
    };
    /**
     * Kept when the card is small, the first first: the status line, the
     * do-not-disturb and a code, a dose, the step, the Porch.
     */
    private static final int[] RANKS = { 0, 1, 1, 4, 6, 7, 8, 3, 5, 2, 1 };
    private static final float[] SIZES = { 14, 13, 12, 14, 13, 13, 13, 14, 13, 13, 13 };
    private static final int[] MAX_LINES = { 2, 2, 2, 2, 1, 1, 1, 2, 2, 2, 2 };
    /** Each line's group: a space before the first line shown of each; the do-not-disturb goes with the status line. */
    private static final int[] GROUPS = { 0, 1, 1, 1, 1, 1, 1, 2, 2, 3, 0 };
    private static final float PADDING_DP = 14, GAP_DP = 6;

    @Override
    public void onReceive(Context context, Intent intent)
    {
        String action = intent.getAction();
        if (DRAW.equals(action) || Intent.ACTION_BOOT_COMPLETED.equals(action) || Intent.ACTION_MY_PACKAGE_REPLACED.equals(action)) {
            drawAll(context.getApplicationContext());
            return;
        }
        super.onReceive(context, intent);
    }

    @Override
    public void onUpdate(Context context, AppWidgetManager manager, int[] ids)
    {
        draw(context.getApplicationContext(), manager, ids);
    }

    @Override
    public void onAppWidgetOptionsChanged(Context context, AppWidgetManager manager, int id, Bundle options)
    {
        draw(context.getApplicationContext(), manager, new int[] { id });
    }

    @Override
    public void onDisabled(Context context)
    {
        schedule(context.getApplicationContext(), Long.MAX_VALUE);
    }

    /**
     * The card before Sioul wrote one, in Sioul's two languages by the phone's
     * (as DoseAlarms.words): Qt's Gradle template keeps the English resources
     * alone (resConfig "en"), so no values-fr; every other word is Rust's.
     */
    private static String empty()
    {
        return "fr".equals(Locale.getDefault().getLanguage()) ? "Ouvrez Sioul pour remplir cette carte." : "Open Sioul to fill this card.";
    }

    /** Every card on the home screens drawn again; none, the next drawing let go. */
    static void drawAll(Context context)
    {
        AppWidgetManager manager = AppWidgetManager.getInstance(context);
        int[] ids = manager.getAppWidgetIds(new ComponentName(context, HomeCard.class));
        if (ids == null || ids.length == 0) {
            schedule(context, Long.MAX_VALUE);
            return;
        }
        draw(context, manager, ids);
    }

    private static void draw(Context context, AppWidgetManager manager, int[] ids)
    {
        Shown shown = Shown.of(context, read(context), System.currentTimeMillis());
        for (int id : ids) {
            try {
                manager.updateAppWidget(id, views(context, shown, manager.getAppWidgetOptions(id)));
            } catch (RuntimeException e) {
                Log.e(DoseAlarms.TAG, "Home card: not drawn", e);
            }
        }
        schedule(context, shown.next);
    }

    // ---------------------------------------------------------------- what Rust wrote

    private static JSONObject read(Context context)
    {
        File file = new File(new File(context.getFilesDir(), FOLDER), FILE);
        try {
            return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
        } catch (IOException | JSONException e) {
            return null;
        }
    }

    /** What the card says now, and when that changes next. */
    private static final class Shown
    {
        final CharSequence[] texts = new CharSequence[VIEWS.length];
        String age = "";
        String step = "";
        /** When to draw again (Unix ms); never: Long.MAX_VALUE. */
        long next = Long.MAX_VALUE;

        Shown()
        {
            for (int i = 0; i < texts.length; i++)
                texts[i] = "";
        }

        private void soon(long at, long now)
        {
            if (at > now && at < next)
                next = at;
        }

        static Shown of(Context context, JSONObject card, long now)
        {
            Shown shown = new Shown();
            if (card == null) {
                // Sioul not opened since it was installed: its own words, Java's.
                shown.texts[STATUS] = empty();
                return shown;
            }
            JSONObject frame = frame(card.optJSONArray("frames"), now);
            if (frame == null) {
                // Past the last frame: Sioul was not opened for a day and more.
                shown.texts[STATUS] = card.optString("beyond");
            } else {
                shown.soon(frame.optLong("until"), now);
                shown.texts[STATUS] = frame.optString("status");
                shown.texts[DND] = frame.optString("dnd");
                JSONObject porch = frame.optJSONObject("porch");
                if (porch != null) {
                    shown.texts[SUMMARY] = porch.optString("line");
                    JSONArray items = porch.optJSONArray("items");
                    for (int i = 0; items != null && i < items.length() && i < 3; i++)
                        shown.texts[ITEM_1 + i] = item(items.optJSONObject(i));
                }
                JSONObject step = frame.optJSONObject("step");
                if (step != null) {
                    shown.texts[STEP] = step.optString("title");
                    shown.texts[WHY] = step.optString("why");
                    shown.step = step.optString("uid");
                }
                if (frame.optBoolean("codes"))
                    shown.code(card.optJSONArray("codes"), now);
                if (frame.optBoolean("doses"))
                    shown.doses(card.optJSONArray("doses"), now);
            }
            shown.age(card, now);
            return shown;
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
                if (texts[CODE].length() == 0) {
                    texts[CODE] = code.optString("line");
                    texts[WARNING] = code.optString("warning");
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
            for (int i = 0; labels != null && i < labels.length(); i++) {
                JSONObject label = labels.optJSONObject(i);
                long until = label == null ? -1 : label.optLong("until");
                if (until == 0 || until > now) {
                    soon(until, now);
                    age = label.optString("text");
                    return;
                }
            }
        }

        /** The frame for now; before the first (the clock set back), the first. */
        private static JSONObject frame(JSONArray frames, long now)
        {
            for (int i = 0; frames != null && i < frames.length(); i++) {
                JSONObject frame = frames.optJSONObject(i);
                if (frame == null)
                    continue;
                if (i == 0 && now < frame.optLong("from"))
                    return frame;
                if (frame.optLong("from") <= now && now < frame.optLong("until"))
                    return frame;
            }
            return null;
        }

        /** "Marie Dupont · Dinner on Saturday", the sender a little stronger. */
        private static CharSequence item(JSONObject item)
        {
            if (item == null)
                return "";
            SpannableStringBuilder line = new SpannableStringBuilder(item.optString("sender"));
            line.setSpan(new StyleSpan(Typeface.BOLD), 0, line.length(), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
            String subject = item.optString("subject");
            if (!subject.isEmpty())
                line.append(" · ").append(subject);
            return line;
        }
    }

    // ---------------------------------------------------------------- drawn

    /** The card at its size: its lines kept, the most needed first, as long as they fit. */
    private static RemoteViews views(Context context, Shown shown, Bundle options)
    {
        RemoteViews views = new RemoteViews(context.getPackageName(), R.layout.sioul_home_card);
        DisplayMetrics metrics = context.getResources().getDisplayMetrics();
        boolean upright = context.getResources().getConfiguration().orientation != Configuration.ORIENTATION_LANDSCAPE;
        int widthDp = options == null ? 0 : options.getInt(upright ? AppWidgetManager.OPTION_APPWIDGET_MIN_WIDTH : AppWidgetManager.OPTION_APPWIDGET_MAX_WIDTH);
        int heightDp = options == null ? 0 : options.getInt(upright ? AppWidgetManager.OPTION_APPWIDGET_MAX_HEIGHT : AppWidgetManager.OPTION_APPWIDGET_MIN_HEIGHT);
        // Not said yet (a card being placed): about four cells by two.
        if (widthDp <= 0)
            widthDp = 250;
        if (heightDp <= 0)
            heightDp = 110;
        float density = metrics.density;
        int ageWidth = shown.age.isEmpty() ? 0 : (int) (paint(12, metrics).measureText(shown.age) + 8 * density);
        int width = (int) ((widthDp - 2 * PADDING_DP) * density);
        float room = (heightDp - 2 * PADDING_DP) * density;
        boolean[] shownLines = fit(shown, metrics, width, ageWidth, room);
        for (int i = 0; i < VIEWS.length; i++) {
            views.setTextViewText(VIEWS[i], shownLines[i] ? shown.texts[i] : "");
            views.setViewVisibility(VIEWS[i], shownLines[i] ? View.VISIBLE : View.GONE);
        }
        views.setTextViewText(R.id.sioul_home_card_age, shown.age);
        views.setViewVisibility(R.id.sioul_home_card_age, shown.age.isEmpty() ? View.GONE : View.VISIBLE);
        boolean porch = shownLines[CODE] || shownLines[SUMMARY];
        boolean step = shownLines[STEP];
        views.setViewVisibility(R.id.sioul_home_card_porch, porch ? View.VISIBLE : View.GONE);
        views.setViewVisibility(R.id.sioul_home_card_step, step ? View.VISIBLE : View.GONE);
        // A tap: the card opens Sioul; the Porch's lines and the dose, the Porch; the step, Now.
        views.setOnClickPendingIntent(android.R.id.background, launch(context));
        views.setOnClickPendingIntent(R.id.sioul_home_card_porch, opener(context, "porch"));
        views.setOnClickPendingIntent(R.id.sioul_home_card_dose, opener(context, "porch"));
        views.setOnClickPendingIntent(R.id.sioul_home_card_step, opener(context, "now/" + shown.step));
        return views;
    }

    /** Which lines show, the most needed first while they fit in `room` (pixels). */
    private static boolean[] fit(Shown shown, DisplayMetrics metrics, int width, int ageWidth, float room)
    {
        boolean[] kept = new boolean[VIEWS.length];
        boolean[] groupOpen = new boolean[4];
        float gap = GAP_DP * metrics.density;
        // The card's age alone on its row, when no status line says what now is for.
        float used = shown.texts[STATUS].length() == 0 && ageWidth > 0 ? paint(12, metrics).getFontSpacing() * 1.05f : 0;
        for (int rank = 0; rank <= 8; rank++) {
            for (int i = 0; i < VIEWS.length; i++) {
                if (RANKS[i] != rank || shown.texts[i].length() == 0)
                    continue;
                // A line that goes with another: the warning with its code, the items
                // with the Porch's sentence, the reason with its step.
                if ((i == WARNING && !kept[CODE]) || (i >= ITEM_1 && i <= ITEM_3 && !kept[SUMMARY]) || (i == WHY && !kept[STEP]))
                    continue;
                TextPaint paint = paint(SIZES[i], metrics);
                int lines = Math.min(MAX_LINES[i], lines(shown.texts[i], paint, i == STATUS ? width - ageWidth : width));
                float height = lines * paint.getFontSpacing() * 1.05f + (GROUPS[i] > 0 && !groupOpen[GROUPS[i]] ? gap : 0);
                if (used + height > room)
                    continue;
                used += height;
                kept[i] = true;
                groupOpen[GROUPS[i]] = true;
            }
        }
        return kept;
    }

    private static TextPaint paint(float sp, DisplayMetrics metrics)
    {
        TextPaint paint = new TextPaint(TextPaint.ANTI_ALIAS_FLAG);
        paint.setTextSize(TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, sp, metrics));
        return paint;
    }

    private static int lines(CharSequence text, TextPaint paint, int width)
    {
        if (text.length() == 0)
            return 0;
        if (width <= 0)
            return 1;
        return StaticLayout.Builder.obtain(text, 0, text.length(), paint, width).build().getLineCount();
    }

    // ---------------------------------------------------------------- taps

    /** Sioul, as by its icon. */
    private static PendingIntent launch(Context context)
    {
        Intent sioul = context.getPackageManager().getLaunchIntentForPackage(context.getPackageName());
        if (sioul == null)
            return null;
        return PendingIntent.getActivity(context, 0, sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED),
                                         PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /** Sioul on the Porch, or on Now: through HomeCardOpener, which keeps the tap for Rust. */
    private static PendingIntent opener(Context context, String where)
    {
        Intent open = new Intent(context, HomeCardOpener.class).setData(Uri.fromParts(SCHEME, where, null)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        return PendingIntent.getActivity(context, where.startsWith("now") ? 2 : 1, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /**
     * A tap kept for Rust (homecard.rs, `opened`), which takes it when the
     * window comes up: {"open": "porch" | "now", "uid", "at"}.
     */
    static void opened(Context context, Intent intent)
    {
        Uri address = intent == null ? null : intent.getData();
        if (address == null || !SCHEME.equals(address.getScheme()))
            return;
        String where = address.getSchemeSpecificPart();
        JSONObject tap = new JSONObject();
        try {
            tap.put("open", where != null && where.startsWith("now") ? "now" : "porch");
            tap.put("uid", where != null && where.startsWith("now/") ? where.substring(4) : "");
            tap.put("at", System.currentTimeMillis());
        } catch (JSONException e) {
            return;
        }
        File folder = new File(context.getFilesDir(), FOLDER);
        if (!folder.isDirectory() && !folder.mkdirs())
            return;
        try (FileOutputStream out = new FileOutputStream(new File(folder, OPENED))) {
            out.write(tap.toString().getBytes(StandardCharsets.UTF_8));
        } catch (IOException e) {
            Log.w(DoseAlarms.TAG, "Home card: the tap could not be kept: " + e);
        }
    }

    // ---------------------------------------------------------------- drawn again

    /**
     * The card drawn again at `at`, when its frame ends, a code expires, a dose
     * comes or goes, or its age is to be said: an alarm that wakes nothing (the
     * card is drawn when the phone is next awake), on time when Android allows
     * Sioul exact alarms (the doses' "Alarms & reminders"), else within a few
     * minutes. Long.MAX_VALUE: none.
     */
    private static void schedule(Context context, long at)
    {
        AlarmManager alarms = context.getSystemService(AlarmManager.class);
        PendingIntent redraw = PendingIntent.getBroadcast(context, 0, new Intent(context, HomeCard.class).setAction(DRAW),
                                                          PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        if (at == Long.MAX_VALUE) {
            alarms.cancel(redraw);
            return;
        }
        try {
            if (DoseAlarms.exact(context)) {
                alarms.setExact(AlarmManager.RTC, at, redraw);
                return;
            }
        } catch (SecurityException e) {
            // Taken back between the question and the alarm.
        }
        alarms.setWindow(AlarmManager.RTC, at, 5 * 60 * 1000L, redraw);
    }
}
