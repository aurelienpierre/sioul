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
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;
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
 * Sioul's cards on the home screen (docs/android.md, "The card on the home
 * screen"). The full card (HomeCardToday): today first, the date and the
 * weather; then what the Porch has for you, a dose due, a code you asked for,
 * reminders, the calls declined, two events at once; then Now, what now is
 * for and the next step. The mail card (HomeCardMail) and the agenda card
 * (HomeCardAgenda): a list that scrolls (HomeCardRows, through
 * HomeCardService), the Porch's latest messages, or the coming events, day by
 * day. Rust writes what they say (crates/sioul-app/src/homecard.rs) in
 * Sioul's state folder, ahead, in frames: one per change of time until the
 * end of tomorrow, since Android freezes Sioul in the background; the weather
 * hour by hour. Here: the frame and the hour for now, drawn again at the next
 * change (a frame's end, an hour, a code expiring, a dose, a reminder, an
 * event over, midnight) by an alarm that wakes nothing, and whenever Rust
 * says so (HOME_CARD, sent through android/main.cpp's
 * sioul_android_broadcast). No Rust, no network: cards drawn from a file. A
 * tap opens Sioul: as by its icon on the card itself and on the weather; on
 * a code, a dose, a call, the Porch; on a reminder, what it is about; on the
 * step, Now; in a list, the Porch, a message on it, the Agenda or an event in
 * it (HomeCardOpener). No button does anything else. This class draws all three.
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
    /** A tap's address: {@code "sioul-card:porch"}, {@code "sioul-card:mail/<its file>"}… */
    static final String SCHEME = "sioul-card";
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
     * (as DoseAlarms.words); every other word is Rust's.
     */
    static String empty()
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
     * Cards of one kind drawn, the lists read again (HomeCardRows), and the
     * next drawing set: the same for every kind, made from the same file.
     */
    @SuppressWarnings("deprecation") // notifyAppWidgetViewDataChanged: a list's factory, the only way before Android 12.
    private static void draw(Context context, AppWidgetManager manager, int[] ids, String kind)
    {
        long now = System.currentTimeMillis();
        JSONObject card = read(context);
        HomeCardToday.Lines lines = HomeCardToday.Lines.of(card, now);
        for (int id : ids) {
            try {
                Bundle options = manager.getAppWidgetOptions(id);
                manager.updateAppWidget(id, ALL.equals(kind) ? HomeCardToday.views(context, lines, options) : views(context, kind, id));
            } catch (RuntimeException e) {
                Log.e(DoseAlarms.TAG, "Home card: not drawn", e);
            }
        }
        if (!ALL.equals(kind)) {
            try {
                manager.notifyAppWidgetViewDataChanged(ids, R.id.sioul_home_card_list);
            } catch (RuntimeException e) {
                Log.e(DoseAlarms.TAG, "Home card: its list not read again", e);
            }
        }
        schedule(context, Math.min(lines.next, HomeCardRows.nextChange(card, now)));
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

    /**
     * A card's frames: "times" from version 3, "frames" before (an older
     * Sioul's Java reads "frames" alone, finds none in a card of version 3,
     * and says its "beyond": "Open Sioul to bring this card up to date.").
     */
    static JSONArray frames(JSONObject card)
    {
        return card == null ? null : card.optJSONArray(card.optInt("v") >= 3 ? "times" : "frames");
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

    // ---------------------------------------------------------------- drawn

    /**
     * A mail card or an agenda card: its list (HomeCardRows, one factory per
     * card, its address naming the card and its kind).
     */
    @SuppressWarnings("deprecation") // setRemoteAdapter(int, Intent): the only way before Android 12.
    private static RemoteViews views(Context context, String kind, int id)
    {
        RemoteViews views = new RemoteViews(context.getPackageName(), R.layout.sioul_home_card);
        views.setOnClickPendingIntent(android.R.id.background, launch(context));
        Intent rows = new Intent(context, HomeCardService.class).putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id).putExtra(HomeCardService.KIND, kind);
        // Its own address, so that Android keeps one factory per card.
        rows.setData(Uri.parse(rows.toUri(Intent.URI_INTENT_SCHEME)));
        views.setRemoteAdapter(R.id.sioul_home_card_list, rows);
        // A row tapped: its address filled into this one (HomeCardRows), for HomeCardOpener.
        views.setPendingIntentTemplate(R.id.sioul_home_card_list, template(context));
        return views;
    }

    // ---------------------------------------------------------------- taps

    /** Sioul, as by its icon. */
    static PendingIntent launch(Context context)
    {
        Intent sioul = context.getPackageManager().getLaunchIntentForPackage(context.getPackageName());
        if (sioul == null)
            return null;
        return PendingIntent.getActivity(context, 0, sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED),
                                         PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /**
     * Sioul on the Porch, on Now, on what a reminder is about: through
     * HomeCardOpener, which keeps the tap for Rust. Each address its own
     * pending intent (Android tells them apart by their data).
     */
    static PendingIntent opener(Context context, String where)
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
     * window comes up: {"open": "porch" | "now" | "mail" | "agenda" | "event"
     * | "reminder", "uid" (a step's), "key" (a message's or an event's file, a
     * reminder's address: "sioul:task/…"), "at"}.
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
        } else if (where.startsWith("reminder/")) {
            open = "reminder";
            key = where.substring(9);
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
