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
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.text.TextPaint;
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
 * screen"). On top, one short line each: what now is for, do-not-disturb, a
 * code you asked for, the next step when it is the time for one, a dose due.
 * Under them, a list that scrolls (HomeCardRows, through HomeCardService): the
 * Porch's latest messages, then the coming events, day by day. Rust writes
 * what it says (crates/sioul-app/src/homecard.rs) in Sioul's state folder,
 * ahead, in frames: one per change of time until the end of tomorrow, since
 * Android freezes Sioul in the background. Here: the frame for now, drawn
 * again at the next change (a frame's end, a code expiring, a dose, an event
 * over, midnight) by an alarm that wakes nothing, and whenever Rust says so
 * (HOME_CARD, sent through android/main.cpp's sioul_android_broadcast). No
 * Rust, no network: a card drawn from a file. A tap opens Sioul: as by its
 * icon on the card itself; on the code and the dose, the Porch; on the step,
 * Now; in the list, the Porch, a message on it, the Agenda or an event in it
 * (HomeCardOpener). No button does anything else.
 *
 * The mail card (HomeCardMail) and the agenda card (HomeCardAgenda) show one
 * part of the list alone; this class draws all three.
 */
public class HomeCard extends AppWidgetProvider
{
    static final String DRAW = "com.aurelienpierre.sioul.action.HOME_CARD";
    /** What a card shows: everything, its messages alone, its events alone. */
    static final String ALL = "all", MAIL = "mail", AGENDA = "agenda";
    /** Rust's state folder (android/main.cpp: XDG_STATE_HOME is files/state). */
    private static final String FOLDER = "state/sioul";
    private static final String FILE = "home-card.json";
    private static final String OPENED = "home-card-opened";
    /** A tap's address: "sioul-card:porch", "sioul-card:mail/<its file>"… */
    static final String SCHEME = "sioul-card";
    /** The lines on top (where each shows is the layout's). */
    private static final int STATUS = 0, DND = 1, CODE = 2, WARNING = 3, DOSE = 4, STEP = 5;
    private static final int[] VIEWS = {
        R.id.sioul_home_card_status, R.id.sioul_home_card_dnd, R.id.sioul_home_card_code,
        R.id.sioul_home_card_warning, R.id.sioul_home_card_dose, R.id.sioul_home_card_step,
    };
    /**
     * Kept when the card is short, the first first: the status line;
     * do-not-disturb and a code; a dose; the step. The list has the rest.
     */
    private static final int[] RANKS = { 0, 1, 1, 1, 2, 3 };
    private static final float[] SIZES = { 15, 13, 13, 12, 13, 14 };
    /** The space above each line, and around them all (the layout's). */
    private static final float[] ABOVE_DP = { 0, 3, 3, 0, 3, 3 };
    private static final float TOP_DP = 14, BOTTOM_DP = 4;
    /** The three kinds of cards, by their providers. */
    private static final Class<?>[] PROVIDERS = { HomeCard.class, HomeCardMail.class, HomeCardAgenda.class };
    private static final String[] KINDS = { ALL, MAIL, AGENDA };

    /** What this card shows; the mail and agenda cards say theirs. */
    String kind()
    {
        return ALL;
    }

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
        draw(context.getApplicationContext(), manager, ids, kind());
    }

    @Override
    public void onAppWidgetOptionsChanged(Context context, AppWidgetManager manager, int id, Bundle options)
    {
        draw(context.getApplicationContext(), manager, new int[] { id }, kind());
    }

    @Override
    public void onDisabled(Context context)
    {
        // This kind's last card gone: the next drawing only while a card of another kind is left.
        drawAll(context.getApplicationContext());
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

    /** Every card on the home screens drawn again, of every kind; none, the next drawing let go. */
    static void drawAll(Context context)
    {
        AppWidgetManager manager = AppWidgetManager.getInstance(context);
        boolean any = false;
        for (int i = 0; i < PROVIDERS.length; i++) {
            int[] ids = manager.getAppWidgetIds(new ComponentName(context, PROVIDERS[i]));
            if (ids == null || ids.length == 0)
                continue;
            any = true;
            draw(context, manager, ids, KINDS[i]);
        }
        if (!any)
            schedule(context, Long.MAX_VALUE);
    }

    /**
     * Cards of one kind drawn, their lists read again (HomeCardRows), and the
     * next drawing set: the same for every kind, made from the same file.
     */
    @SuppressWarnings("deprecation") // notifyAppWidgetViewDataChanged: a list's factory, the only way before Android 12.
    private static void draw(Context context, AppWidgetManager manager, int[] ids, String kind)
    {
        long now = System.currentTimeMillis();
        JSONObject card = read(context);
        Shown shown = Shown.of(card, now);
        for (int id : ids) {
            try {
                manager.updateAppWidget(id, views(context, shown, manager.getAppWidgetOptions(id), kind, id));
            } catch (RuntimeException e) {
                Log.e(DoseAlarms.TAG, "Home card: not drawn", e);
            }
        }
        try {
            manager.notifyAppWidgetViewDataChanged(ids, R.id.sioul_home_card_list);
        } catch (RuntimeException e) {
            Log.e(DoseAlarms.TAG, "Home card: its list not read again", e);
        }
        schedule(context, Math.min(shown.next, HomeCardRows.nextChange(card, now)));
    }

    // ---------------------------------------------------------------- what Rust wrote

    /** The card Rust wrote last; none before Sioul's first start, or one that does not read. */
    static JSONObject read(Context context)
    {
        File file = new File(new File(context.getFilesDir(), FOLDER), FILE);
        try {
            return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
        } catch (IOException | JSONException e) {
            return null;
        }
    }

    /** The frame for now; before the first (the clock set back), the first; past the last, none. */
    static JSONObject frame(JSONArray frames, long now)
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

    /** What the lines on top say now, and when that changes next. */
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

        static Shown of(JSONObject card, long now)
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
                // The step's title alone: why it comes now is on Now, where a tap leads.
                JSONObject step = frame.optJSONObject("step");
                if (step != null) {
                    shown.texts[STEP] = step.optString("title");
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
    }

    // ---------------------------------------------------------------- drawn

    /**
     * A card at its size. The full card: its lines on top, kept the most
     * needed first as long as they fit, and the list in what is left. The mail
     * and agenda cards: the list alone. The list's rows are HomeCardRows's,
     * one factory per card, its address naming the card and its kind.
     */
    @SuppressWarnings("deprecation") // setRemoteAdapter(int, Intent): the only way before Android 12.
    private static RemoteViews views(Context context, Shown shown, Bundle options, String kind, int id)
    {
        RemoteViews views = new RemoteViews(context.getPackageName(), R.layout.sioul_home_card);
        boolean lines = ALL.equals(kind);
        views.setViewVisibility(R.id.sioul_home_card_top, lines ? View.VISIBLE : View.GONE);
        if (lines) {
            boolean[] kept = fit(context, shown, options);
            for (int i = 0; i < VIEWS.length; i++) {
                views.setTextViewText(VIEWS[i], kept[i] ? shown.texts[i] : "");
                views.setViewVisibility(VIEWS[i], kept[i] ? View.VISIBLE : View.GONE);
            }
            views.setTextViewText(R.id.sioul_home_card_age, shown.age);
            views.setViewVisibility(R.id.sioul_home_card_age, shown.age.isEmpty() ? View.GONE : View.VISIBLE);
            // A tap: the code and the dose, the Porch; the step, Now; the rest, Sioul as by its icon.
            views.setOnClickPendingIntent(R.id.sioul_home_card_code, opener(context, "porch"));
            views.setOnClickPendingIntent(R.id.sioul_home_card_warning, opener(context, "porch"));
            views.setOnClickPendingIntent(R.id.sioul_home_card_dose, opener(context, "porch"));
            views.setOnClickPendingIntent(R.id.sioul_home_card_step, opener(context, "now/" + shown.step));
        }
        views.setOnClickPendingIntent(android.R.id.background, launch(context));
        Intent rows = new Intent(context, HomeCardService.class).putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id).putExtra(HomeCardService.KIND, kind);
        // Its own address, so that Android keeps one factory per card.
        rows.setData(Uri.parse(rows.toUri(Intent.URI_INTENT_SCHEME)));
        views.setRemoteAdapter(R.id.sioul_home_card_list, rows);
        // A row tapped: its address filled into this one (HomeCardRows), for HomeCardOpener.
        views.setPendingIntentTemplate(R.id.sioul_home_card_list, template(context));
        return views;
    }

    /**
     * Which lines on top show, the most needed first while they fit in the
     * card's height; the status line always. One line each.
     */
    private static boolean[] fit(Context context, Shown shown, Bundle options)
    {
        DisplayMetrics metrics = context.getResources().getDisplayMetrics();
        boolean upright = context.getResources().getConfiguration().orientation != Configuration.ORIENTATION_LANDSCAPE;
        int heightDp = options == null ? 0 : options.getInt(upright ? AppWidgetManager.OPTION_APPWIDGET_MAX_HEIGHT : AppWidgetManager.OPTION_APPWIDGET_MIN_HEIGHT);
        // Not said yet (a card being placed): about four cells high.
        if (heightDp <= 0)
            heightDp = 250;
        float density = metrics.density;
        float room = (heightDp - TOP_DP - BOTTOM_DP) * density;
        boolean[] kept = new boolean[VIEWS.length];
        float used = 0;
        for (int rank = 0; rank <= 3; rank++) {
            for (int i = 0; i < VIEWS.length; i++) {
                if (RANKS[i] != rank || shown.texts[i].length() == 0)
                    continue;
                // The warning goes with its code.
                if (i == WARNING && !kept[CODE])
                    continue;
                float height = paint(SIZES[i], metrics).getFontSpacing() * 1.05f + ABOVE_DP[i] * density;
                if (i != STATUS && used + height > room)
                    continue;
                used += height;
                kept[i] = true;
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
     * The list's taps: HomeCardOpener, each row's address filled in
     * (HomeCardRows). Mutable for that alone, from Android 12 where it must be
     * said; its class is fixed, so a row can only say where Sioul opens.
     */
    private static PendingIntent template(Context context)
    {
        Intent open = new Intent(context, HomeCardOpener.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        int mutable = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ? PendingIntent.FLAG_MUTABLE : 0;
        return PendingIntent.getActivity(context, 3, open, mutable | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /**
     * A tap kept for Rust (homecard.rs, `opened`), which takes it when the
     * window comes up: {"open": "porch" | "now" | "mail" | "agenda" | "event",
     * "uid" (a step's), "key" (a message's or an event's file), "at"}.
     */
    static void opened(Context context, Intent intent)
    {
        Uri address = intent == null ? null : intent.getData();
        if (address == null || !SCHEME.equals(address.getScheme()))
            return;
        String where = address.getSchemeSpecificPart();
        if (where == null)
            where = "";
        String open = "porch", uid = "", key = "";
        if (where.startsWith("now")) {
            open = "now";
            uid = where.startsWith("now/") ? where.substring(4) : "";
        } else if (where.startsWith("mail/")) {
            open = "mail";
            key = where.substring(5);
        } else if (where.startsWith("event/")) {
            open = "event";
            key = where.substring(6);
        } else if (where.equals("agenda")) {
            open = "agenda";
        }
        JSONObject tap = new JSONObject();
        try {
            tap.put("open", open);
            tap.put("uid", uid);
            tap.put("key", key);
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
     * The cards drawn again at `at`, when a frame ends, a code expires, a dose
     * comes or goes, the age is to be said, an event is over, or a day's or a
     * message's name changes at midnight: one alarm for every card, that wakes
     * nothing (the cards are drawn when the phone is next awake), on time when
     * Android allows Sioul exact alarms (the doses' "Alarms &amp; reminders"),
     * else within a few minutes. Long.MAX_VALUE: none.
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
