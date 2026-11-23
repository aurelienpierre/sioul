// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.graphics.drawable.Icon;
import android.net.Uri;

import org.json.JSONObject;

import java.util.Collections;

/**
 * What Sioul says in the phone's shade of the other apps' notifications it
 * holds (docs/android.md, "Notifications from other apps"; held apps, phase
 * 1, part B), posted by the listener ({@link AppNotes}) in its own process,
 * from the words Rust gives (crates/sioul-app/src/appnotes.rs, `shade`):
 *
 * - The standing line (B1): while Sioul holds anything that will come back,
 *   one notification of Sioul's own, on a channel of its own at Android's
 *   minimum importance (no sound, no banner, no icon in the status bar: seen
 *   when the shade is pulled down), never ongoing, the Porch's sentence on
 *   what waits ("Notifications held until 13:00: Signal and Shop."), apps'
 *   names only; Show now, a release of this phone (never offered during a
 *   pause); a tap opens Sioul at this phone's card, at its apps. Updated at
 *   each hold, return and review, taken away when nothing waits.
 *
 * Each in a group of its own, its summary kept on Android 16 as the notices'
 * are ({@link Notices#tidy}). Sioul's own notifications are never held by
 * the listener, nor counted in these lines.
 */
final class HeldNotices
{
    /** The standing line's channel: minimum importance, no sound, no banner. */
    static final String HELD = "held";
    /** The standing line's tag (with {@link Notices#NOTE}): one at a time, updated in its place. */
    static final String LINE = "sioul:held";
    /** The standing line's group, alone in it. */
    static final String GROUP_HELD = Notices.GROUP + "held";
    /** This phone's card in Settings ▸ Devices, at its apps: what a tap on the line opens (ReminderOpener). */
    static final String CARD = "settings:devices.apps";

    private HeldNotices() {}

    /** The standing line as Rust says it ({@code shade}, "line"): {text, public, show, channel}. */
    static final class Line
    {
        /** What waits, in one sentence; "" when nothing does: the line goes. */
        String text = "";
        /** What the lock screen shows where the phone hides what notifications say. */
        String publicText = "";
        /** Show now's words; "" when none is offered (a pause). */
        String show = "";
        /** The channel's name. */
        String channel = "";

        /**
         * Rust's answer read whole; null or broken, nothing to show.
         *
         * @param said Rust's JSON, or null
         * @return the line
         */
        static Line of(JSONObject said)
        {
            Line out = new Line();
            if (said == null)
                return out;
            out.text = said.optString("text").trim();
            out.publicText = said.optString("public");
            out.show = said.optString("show");
            out.channel = said.optString("channel");
            return out;
        }

        /** Whether it says the same as `other`: posted again only when it changed. */
        boolean same(Line other)
        {
            return other != null && text.equals(other.text) && show.equals(other.show);
        }
    }

    /**
     * The standing line posted in its place, or taken away when it says
     * nothing; its group's summary follows (Android 16). Any thread.
     *
     * @param context the listener
     * @param line what it says
     */
    static void line(Context context, Line line)
    {
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        if (line.text.isEmpty()) {
            cancelLine(context);
            return;
        }
        NotificationChannel channel = Channels.quiet(HELD, line.channel.isEmpty() ? "Sioul" : line.channel, NotificationManager.IMPORTANCE_MIN);
        channel.setSound(null, null);
        channel.enableVibration(false);
        notifications.createNotificationChannel(channel);
        Notification.Builder note = new Notification.Builder(context, HELD)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentText(line.text)
            .setStyle(new Notification.BigTextStyle().bigText(line.text))
            .setCategory(Notification.CATEGORY_STATUS)
            // Apps' names only, and on the lock screen not even those where the phone hides what notifications say.
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .setPublicVersion(new Notification.Builder(context, HELD)
                                  .setSmallIcon(R.drawable.sioul_notification)
                                  .setColor(context.getColor(R.color.sioul_green))
                                  .setContentText(line.publicText)
                                  .build())
            .setContentIntent(card(context))
            .setDeleteIntent(NoticeReceiver.deleted(context, LINE))
            .setAutoCancel(false)
            .setOnlyAlertOnce(true)
            .setOngoing(false)
            .setShowWhen(false)
            .setLocalOnly(true)
            .setGroup(GROUP_HELD)
            .setGroupAlertBehavior(Notification.GROUP_ALERT_CHILDREN);
        if (!line.show.isEmpty())
            note.addAction(new Notification.Action.Builder(Icon.createWithResource(context, R.drawable.sioul_notification), line.show, showNow(context)).build());
        notifications.notify(LINE, Notices.NOTE, note.build());
        Notices.tidy(context, Collections.singletonList(new Notices.Child(Notices.NOTE, LINE, GROUP_HELD, 0)), Collections.emptySet());
    }

    /** The standing line taken away (nothing waits, the listener unbound), its summary with it. Any thread. */
    static void cancelLine(Context context)
    {
        context.getSystemService(NotificationManager.class).cancel(LINE, Notices.NOTE);
        Notices.tidy(context, Collections.emptyList(), Collections.singleton(Notices.key(Notices.NOTE, LINE)));
    }

    /** A tap on the line: Sioul at this phone's card, at its apps (ReminderOpener, the one door a notification may open). */
    private static PendingIntent card(Context context)
    {
        Intent sioul = new Intent(context, ReminderOpener.class).setData(Uri.fromParts(ReminderOpener.SCHEME, CARD, null)).putExtra(ReminderOpener.KEY, "").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        return PendingIntent.getActivity(context, Notices.requestCode(LINE, CARD), sioul, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /** Show now: heard by the listener in its own process ({@link AppNotes.Asked}), never exported. */
    private static PendingIntent showNow(Context context)
    {
        Intent show = new Intent(context, AppNotes.Asked.class).setAction(AppNotes.SHOW);
        return PendingIntent.getBroadcast(context, Notices.requestCode(LINE, "show"), show, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }
}
