// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.Context;
import android.content.Intent;
import android.graphics.Typeface;
import android.net.Uri;
import android.text.SpannableString;
import android.text.Spanned;
import android.text.style.StyleSpan;
import android.view.View;
import android.widget.RemoteViews;
import android.widget.RemoteViewsService;

import org.json.JSONArray;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.List;

/**
 * The rows of a home-screen card's list (HomeCard, through HomeCardService),
 * as a mail card and an agenda card show them, from what Rust wrote (crates/
 * sioul-app/src/homecard.rs). First "Mail": the messages the Porch shows in the
 * frame for now, newest first, each with a thin bar in its account's colour,
 * its sender (bold while not read), when it came, its subject and a line of
 * its text; or the Porch's sentence instead ("The Porch opens tomorrow at
 * 09:00.", "Nothing waits on the Porch."). Then "Agenda": the events not over
 * yet, each under its day ("Today, Wed 7 Oct", "Tomorrow, Thu 8 Oct", "Mon 12
 * Oct": each name as it holds now, Rust having said until when), as a block
 * in its calendar's colour with its title and its time. Never a count.
 *
 * A row tapped fills its address into the list's template (HomeCard), for
 * HomeCardOpener: the Porch, a message ("mail/" and its file), the Agenda, an
 * event ("event/" and its file).
 */
final class HomeCardRows implements RemoteViewsService.RemoteViewsFactory
{
    /** The rows' kinds, each with its layout. */
    static final int SECTION = 0, LINE = 1, MAIL = 2, DAY = 3, EVENT = 4;
    private static final int[] LAYOUTS = {
        R.layout.sioul_home_card_section, R.layout.sioul_home_card_line, R.layout.sioul_home_card_mail,
        R.layout.sioul_home_card_day, R.layout.sioul_home_card_event,
    };
    /**
     * The accounts' colours, by their place among your accounts: Theme.qml's
     * chart colours, taken between their light and dark tones so that each
     * reads on either ground (the host draws the card in the phone's theme,
     * these are fixed here).
     */
    private static final int[] ACCOUNTS = { 0xff5f8572, 0xffb07d3f, 0xff5b7ba3, 0xffa86464, 0xff8571ad, 0xff7d8c3a, 0xff4b8f8f };
    /** An event whose calendar gives no colour: Sioul's green. */
    private static final int OWN = 0xff5f8572;
    /** The text on a light block; on a dark one, white. */
    private static final int DARK = 0xff1f1f1c;

    /** One row: its kind, the message or event it shows, its words, where a tap leads. */
    static final class Row
    {
        final int type;
        final JSONObject item;
        final String text;
        final String open;

        Row(int type, JSONObject item, String text, String open)
        {
            this.type = type;
            this.item = item;
            this.text = text;
            this.open = open;
        }
    }

    private final Context context;
    private final String kind;
    private List<Row> rows = new ArrayList<>();
    /** When the rows were made: the dates and days said as at then. */
    private long now = System.currentTimeMillis();

    HomeCardRows(Context context, String kind)
    {
        this.context = context;
        this.kind = kind;
    }

    @Override
    public void onCreate()
    {
    }

    /** The file read again (HomeCard asks it at each drawing): the rows for now. */
    @Override
    public void onDataSetChanged()
    {
        now = System.currentTimeMillis();
        rows = rows(HomeCard.read(context), now, kind);
    }

    @Override
    public void onDestroy()
    {
        rows = new ArrayList<>();
    }

    @Override
    public int getCount()
    {
        return rows.size();
    }

    @Override
    public RemoteViews getViewAt(int position)
    {
        if (position < 0 || position >= rows.size())
            return getLoadingView();
        Row row = rows.get(position);
        RemoteViews views = new RemoteViews(context.getPackageName(), LAYOUTS[row.type]);
        if (row.type == MAIL)
            message(views, row.item);
        else if (row.type == EVENT)
            event(views, row.item);
        else
            views.setTextViewText(R.id.sioul_home_card_row, row.text);
        views.setOnClickFillInIntent(R.id.sioul_home_card_row, new Intent().setData(Uri.fromParts(HomeCard.SCHEME, row.open, null)));
        return views;
    }

    /** A blank line while a row comes, rather than Android's "Loading…". */
    @Override
    public RemoteViews getLoadingView()
    {
        RemoteViews views = new RemoteViews(context.getPackageName(), R.layout.sioul_home_card_line);
        views.setTextViewText(R.id.sioul_home_card_row, "");
        return views;
    }

    @Override
    public int getViewTypeCount()
    {
        return LAYOUTS.length;
    }

    @Override
    public long getItemId(int position)
    {
        return position;
    }

    @Override
    public boolean hasStableIds()
    {
        return false;
    }

    // ---------------------------------------------------------------- what the list says

    /**
     * The rows for `now`, for a card of `kind` (HomeCard.ALL, MAIL, AGENDA):
     * the Porch's part of the frame for now, then the agenda's. None before
     * Sioul wrote a card with lists (an older Sioul's card, or none: the lines
     * on top say what there is).
     */
    static List<Row> rows(JSONObject card, long now, String kind)
    {
        List<Row> rows = new ArrayList<>();
        if (card == null || card.optInt("v") < 2)
            return rows;
        JSONObject words = card.optJSONObject("words");
        JSONObject frame = HomeCard.frame(card.optJSONArray("frames"), now);
        if (!HomeCard.AGENDA.equals(kind))
            mail(rows, card, frame, words == null ? "" : words.optString("mail"), kind);
        if (!HomeCard.MAIL.equals(kind))
            agenda(rows, card, frame, words == null ? "" : words.optString("agenda"), now, kind);
        return rows;
    }

    /**
     * "Mail", then the messages the frame lists, or its sentence. Asleep, in a
     * pause, in free time, the Porch says nothing: on the full card the status
     * line says why; the mail card says it under its heading.
     */
    private static void mail(List<Row> rows, JSONObject card, JSONObject frame, String heading, String kind)
    {
        JSONObject porch = frame == null ? null : frame.optJSONObject("porch");
        if (porch == null) {
            if (HomeCard.MAIL.equals(kind)) {
                rows.add(new Row(SECTION, null, heading, "porch"));
                rows.add(new Row(LINE, null, frame == null ? card.optString("beyond") : frame.optString("status"), "porch"));
            }
            return;
        }
        rows.add(new Row(SECTION, null, heading, "porch"));
        String line = porch.optString("line");
        if (!line.isEmpty())
            rows.add(new Row(LINE, null, line, "porch"));
        JSONArray all = card.optJSONArray("mail");
        JSONArray listed = porch.optJSONArray("mail");
        for (int i = 0; all != null && listed != null && i < listed.length(); i++) {
            JSONObject message = all.optJSONObject(listed.optInt(i, -1));
            if (message == null)
                continue;
            String key = message.optString("key");
            rows.add(new Row(MAIL, message, "", key.isEmpty() ? "porch" : "mail/" + key));
        }
    }

    /**
     * "Agenda", then each event not over yet under its day: the day it starts,
     * or today while it runs. Rust gave them nearest first, so the days only
     * go forward. In a pause the full card shows the pause alone; the agenda
     * card says it under its heading. Without a calendar, nothing.
     */
    private static void agenda(List<Row> rows, JSONObject card, JSONObject frame, String heading, long now, String kind)
    {
        JSONObject agenda = card.optJSONObject("agenda");
        if (agenda == null)
            return;
        if (frame != null && !frame.optBoolean("events", true)) {
            if (HomeCard.AGENDA.equals(kind)) {
                rows.add(new Row(SECTION, null, heading, "agenda"));
                rows.add(new Row(LINE, null, frame.optString("status"), "agenda"));
            }
            return;
        }
        rows.add(new Row(SECTION, null, heading, "agenda"));
        JSONArray days = agenda.optJSONArray("days");
        JSONArray events = agenda.optJSONArray("events");
        int shownDay = -1;
        boolean any = false;
        for (int i = 0; events != null && i < events.length(); i++) {
            JSONObject event = events.optJSONObject(i);
            if (event == null || event.optLong("end") <= now)
                continue;
            int day = dayOf(days, Math.max(event.optLong("start"), now));
            if (day < 0)
                continue;
            if (day != shownDay) {
                JSONObject named = days.optJSONObject(day);
                rows.add(new Row(DAY, null, label(named == null ? null : named.optJSONArray("label"), now), "agenda"));
                shownDay = day;
            }
            String key = event.optString("key");
            rows.add(new Row(EVENT, event, "", key.isEmpty() ? "agenda" : "event/" + key));
            any = true;
        }
        if (!any)
            rows.add(new Row(LINE, null, agenda.optString("empty"), "agenda"));
    }

    /** The day `at` falls in, by its place in `days`; -1 outside them. */
    private static int dayOf(JSONArray days, long at)
    {
        for (int i = 0; days != null && i < days.length(); i++) {
            JSONObject day = days.optJSONObject(i);
            if (day != null && day.optLong("from") <= at && at < day.optLong("until"))
                return i;
        }
        return -1;
    }

    /** The wording that holds at `now`: the first whose `until` is ahead, or 0 (for good). */
    static String label(JSONArray labels, long now)
    {
        for (int i = 0; labels != null && i < labels.length(); i++) {
            JSONObject label = labels.optJSONObject(i);
            if (label == null)
                continue;
            long until = label.optLong("until");
            if (until == 0 || until > now)
                return label.optString("text");
        }
        return "";
    }

    /** The first `until` ahead of `now` among some wordings; none, Long.MAX_VALUE. */
    private static long ahead(JSONArray labels, long now)
    {
        for (int i = 0; labels != null && i < labels.length(); i++) {
            JSONObject label = labels.optJSONObject(i);
            long until = label == null ? 0 : label.optLong("until");
            if (until > now)
                return until;
        }
        return Long.MAX_VALUE;
    }

    /**
     * When the list says something else (Unix ms), for HomeCard's next
     * drawing: an event over, a message's or a day's name changing at
     * midnight. Long.MAX_VALUE: never.
     */
    static long nextChange(JSONObject card, long now)
    {
        long next = Long.MAX_VALUE;
        if (card == null)
            return next;
        JSONArray mail = card.optJSONArray("mail");
        for (int i = 0; mail != null && i < mail.length(); i++) {
            JSONObject message = mail.optJSONObject(i);
            if (message != null)
                next = Math.min(next, ahead(message.optJSONArray("when"), now));
        }
        JSONObject agenda = card.optJSONObject("agenda");
        JSONArray days = agenda == null ? null : agenda.optJSONArray("days");
        for (int i = 0; days != null && i < days.length(); i++) {
            JSONObject day = days.optJSONObject(i);
            if (day != null)
                next = Math.min(next, ahead(day.optJSONArray("label"), now));
        }
        JSONArray events = agenda == null ? null : agenda.optJSONArray("events");
        for (int i = 0; events != null && i < events.length(); i++) {
            JSONObject event = events.optJSONObject(i);
            long end = event == null ? 0 : event.optLong("end");
            if (end > now)
                next = Math.min(next, end);
        }
        return next;
    }

    // ---------------------------------------------------------------- drawn

    /** A message, as a mail card shows it; sender and subject in bold while it is not read. */
    private void message(RemoteViews views, JSONObject message)
    {
        boolean unread = message.optBoolean("unread");
        views.setTextViewText(R.id.sioul_home_card_sender, bold(message.optString("sender"), unread));
        views.setTextViewText(R.id.sioul_home_card_date, label(message.optJSONArray("when"), now));
        views.setTextViewText(R.id.sioul_home_card_subject, bold(message.optString("subject"), unread));
        String text = message.optString("text");
        views.setTextViewText(R.id.sioul_home_card_text, text);
        views.setViewVisibility(R.id.sioul_home_card_text, text.isEmpty() ? View.GONE : View.VISIBLE);
        int account = message.optInt("account", -1);
        if (account < 0) {
            views.setViewVisibility(R.id.sioul_home_card_bar, View.INVISIBLE);
            return;
        }
        views.setViewVisibility(R.id.sioul_home_card_bar, View.VISIBLE);
        views.setInt(R.id.sioul_home_card_bar, "setColorFilter", ACCOUNTS[account % ACCOUNTS.length]);
    }

    /** An event, as an agenda card shows it: a block in its calendar's colour, the text as that colour asks. */
    private void event(RemoteViews views, JSONObject event)
    {
        int color = colour(event.optString("color"));
        int text = textOn(color);
        views.setInt(R.id.sioul_home_card_block, "setColorFilter", color);
        views.setTextViewText(R.id.sioul_home_card_title, event.optString("title"));
        views.setTextColor(R.id.sioul_home_card_title, text);
        String time = event.optString("time");
        views.setTextViewText(R.id.sioul_home_card_time, time);
        // The time a little softer than the title, as the window's agenda has it.
        views.setTextColor(R.id.sioul_home_card_time, (text & 0x00ffffff) | 0xd9000000);
        views.setViewVisibility(R.id.sioul_home_card_time, time.isEmpty() ? View.GONE : View.VISIBLE);
    }

    private static CharSequence bold(String text, boolean bold)
    {
        if (!bold || text.isEmpty())
            return text;
        SpannableString styled = new SpannableString(text);
        styled.setSpan(new StyleSpan(Typeface.BOLD), 0, styled.length(), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        return styled;
    }

    /** "#4c6b5c" as a colour; anything else, Sioul's own. */
    private static int colour(String hex)
    {
        if (hex != null && hex.length() == 7 && hex.charAt(0) == '#') {
            try {
                return 0xff000000 | Integer.parseInt(hex.substring(1), 16);
            } catch (NumberFormatException e) {
                // Not a colour: Sioul's own.
            }
        }
        return OWN;
    }

    /** Dark text on a light block, white on a dark one: whichever contrasts more (WCAG 2's ratio). */
    static int textOn(int color)
    {
        double ground = luminance(color);
        double white = 1.05 / (ground + 0.05);
        double dark = (ground + 0.05) / (luminance(DARK) + 0.05);
        return white >= dark ? 0xffffffff : DARK;
    }

    /** WCAG 2's relative luminance of an sRGB colour. */
    private static double luminance(int color)
    {
        return 0.2126 * linear((color >> 16) & 0xff) + 0.7152 * linear((color >> 8) & 0xff) + 0.0722 * linear(color & 0xff);
    }

    private static double linear(int value)
    {
        double c = value / 255.0;
        return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
    }
}
