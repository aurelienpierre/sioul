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
 * - One sound at each return (B2): people's messages come back without a
 *   sound of their own (QuietWindow), and Sioul says it once, on a channel
 *   of its own at Android's default importance (its sound, no banner):
 *   "Four messages waited: Signal and WhatsApp.", posted once the window
 *   closed, gone after ten minutes or at a tap, which opens their app when
 *   they all came from one.
 * - After a restart (B3): what Android lost of what Sioul held, said once,
 *   without a sound, among Sioul's notices: "Before the restart, Sioul was
 *   holding two notifications from Signal: open it to read them."
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

    // ---------------------------------------------------------------- B2: one sound at each return

    /** The notice that people's messages waited: Android's default importance, its sound, no banner. */
    static final String WAITED = "waited";
    /** Its tag (with {@link Notices#NOTE}): the next takes its place, posted anew so that it rings. */
    static final String WAITED_TAG = "sioul:waited";
    /** Its group, alone in it. */
    static final String GROUP_WAITED = Notices.GROUP + "waited";
    /** It goes after ten minutes, or at a tap. */
    static final long WAITED_MS = 10 * 60_000L;

    /**
     * People's messages come back since Sioul last told it (B2): how many,
     * and their apps in the order they came, by package. Kept by the
     * listener on its worker; taken whole when told.
     */
    static final class Waited
    {
        int count;
        final java.util.LinkedHashMap<String, String> apps = new java.util.LinkedHashMap<>();

        /** One more, from this app (its package, its name as the phone shows it). */
        void add(String pkg, String label)
        {
            count++;
            if (pkg != null && !apps.containsKey(pkg))
                apps.put(pkg, label == null || label.isEmpty() ? pkg : label);
        }

        /** What was kept, handed whole; this one empty again. */
        Waited take()
        {
            Waited out = new Waited();
            out.count = count;
            out.apps.putAll(apps);
            count = 0;
            apps.clear();
            return out;
        }

        /** The one app they came from, or null for several (a tap then opens none). */
        String onlyApp()
        {
            return apps.size() == 1 ? apps.keySet().iterator().next() : null;
        }

        /** As Rust reads it ({@code shade}, "waited"): {count, apps}, the apps' names alone. */
        org.json.JSONObject json() throws org.json.JSONException
        {
            org.json.JSONArray names = new org.json.JSONArray();
            for (String label : apps.values())
                names.put(label);
            return new org.json.JSONObject().put("count", count).put("apps", names);
        }
    }

    /** A notice's words as Rust says them ({@code shade}, "waited", "lost"): {text, public, channel}. */
    static final class Told
    {
        /** What it says; "" when there is nothing to say. */
        String text = "";
        /** What the lock screen shows where the phone hides what notifications say. */
        String publicText = "";
        /** Its channel's name (the one that waited's). */
        String channel = "";

        /**
         * Rust's answer read whole; null or broken, nothing to say.
         *
         * @param said Rust's JSON, or null
         * @return the words
         */
        static Told of(JSONObject said)
        {
            Told out = new Told();
            if (said == null)
                return out;
            out.text = said.optString("text").trim();
            out.publicText = said.optString("public");
            out.channel = said.optString("channel");
            return out;
        }
    }

    /**
     * People's messages that waited, told once (B2): on a channel of its own
     * at Android's default importance (its sound, no banner), posted anew so
     * that it rings even when the last one still shows; gone after ten
     * minutes, or at a tap, which opens their app when they all came from one
     * (`onlyApp`), and only takes it away otherwise. Any thread.
     *
     * @param context the listener
     * @param told Rust's words
     * @param onlyApp the one app's package, or null for several
     */
    static void waited(Context context, Told told, String onlyApp)
    {
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        NotificationChannel channel = Channels.quiet(WAITED, told.channel.isEmpty() ? "Sioul" : told.channel, NotificationManager.IMPORTANCE_DEFAULT);
        channel.setLockscreenVisibility(Notification.VISIBILITY_PRIVATE);
        notifications.createNotificationChannel(channel);
        Notification.Builder note = new Notification.Builder(context, WAITED)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentText(told.text)
            .setStyle(new Notification.BigTextStyle().bigText(told.text))
            .setCategory(Notification.CATEGORY_MESSAGE)
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .setPublicVersion(publicOf(context, WAITED, told))
            .setContentIntent(tapped(context, WAITED_TAG, onlyApp))
            .setDeleteIntent(NoticeReceiver.deleted(context, WAITED_TAG))
            .setAutoCancel(true)
            .setOnlyAlertOnce(false)
            .setOngoing(false)
            .setTimeoutAfter(WAITED_MS)
            .setGroup(GROUP_WAITED)
            .setGroupAlertBehavior(Notification.GROUP_ALERT_CHILDREN);
        // Anew, never as an update of the one shown, which Android would keep silent.
        notifications.cancel(WAITED_TAG, Notices.NOTE);
        notifications.notify(WAITED_TAG, Notices.NOTE, note.build());
        Notices.tidy(context, Collections.singletonList(new Notices.Child(Notices.NOTE, WAITED_TAG, GROUP_WAITED, System.currentTimeMillis() + WAITED_MS)), Collections.emptySet());
    }

    /** As the lock screen shows a notice where the phone hides what notifications say: Rust's words without an app. */
    private static Notification publicOf(Context context, String channel, Told told)
    {
        return new Notification.Builder(context, channel)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentText(told.publicText)
            .build();
    }

    /**
     * A tap on a notice of what waited or was lost: their one app opened by
     * its launcher (Android resolves it; no list of apps is asked), else the
     * notice only taken away, its group's summary with it (NoticeReceiver).
     */
    private static PendingIntent tapped(Context context, String tag, String onlyApp)
    {
        if (onlyApp == null || onlyApp.isEmpty())
            return NoticeReceiver.deleted(context, tag);
        Intent app = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER).setPackage(onlyApp)
                         .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED);
        return PendingIntent.getActivity(context, Notices.requestCode(tag, onlyApp), app, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    // ---------------------------------------------------------------- B3: after a restart

    /** The notice after a restart: its tag (with {@link Notices#NOTE}), in the notices' group of things to know. */
    static final String LOST = "sioul:lost";
    /** It stays a day at most: past that, the restart is old news. */
    static final long LOST_MS = 24 * 3600_000L;
    /** {@link #afterStart}: nothing to do (the same start, or Android cannot say). */
    static final int SAME_START = 0;
    /** {@link #afterStart}: the first binding of this Sioul: the count noted, nothing to compare with. */
    static final int NOTE_START = 1;
    /** {@link #afterStart}: the phone started since the last binding: what Android lost is looked for. */
    static final int LOOK = 2;

    /**
     * What a binding does with the count of the phone's starts (B3): `kept`,
     * the count last noted (-1: none yet); `now`, Android's
     * (Settings.Global.BOOT_COUNT; -1: it cannot say). Pure (HeldCheck).
     *
     * @param kept the count last noted, or -1
     * @param now Android's count now, or -1
     * @return {@link #SAME_START}, {@link #NOTE_START} or {@link #LOOK}
     */
    static int afterStart(int kept, int now)
    {
        if (now < 0)
            return SAME_START;
        if (kept < 0)
            return NOTE_START;
        return kept == now ? SAME_START : LOOK;
    }

    /**
     * The one app of a list of packages (Rust's "packages"), or null for
     * none or several: a tap then opens no app. Pure (HeldCheck).
     *
     * @param packages the packages, or null
     * @return the one package, or null
     */
    static String onlyApp(org.json.JSONArray packages)
    {
        if (packages == null || packages.length() != 1)
            return null;
        String one = packages.optString(0, "").trim();
        return one.isEmpty() ? null : one;
    }

    /**
     * What Android lost of what Sioul held, after a restart, said once (B3):
     * something to know, on the "Notices" channel (no sound), in its group,
     * a day at most; a tap opens their app when they all came from one,
     * else only takes it away. Any thread.
     *
     * @param context the listener
     * @param told Rust's words
     * @param onlyApp the one app's package, or null for several
     */
    static void lost(Context context, Told told, String onlyApp)
    {
        Notices.channels(context, null);
        String group = Notices.GROUP + Notices.INFORMATIVE;
        Notification note = new Notification.Builder(context, Notices.NOTICES)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentText(told.text)
            .setStyle(new Notification.BigTextStyle().bigText(told.text))
            .setCategory(Notification.CATEGORY_STATUS)
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .setPublicVersion(publicOf(context, Notices.NOTICES, told))
            .setContentIntent(tapped(context, LOST, onlyApp))
            .setDeleteIntent(NoticeReceiver.deleted(context, LOST))
            .setAutoCancel(true)
            .setOnlyAlertOnce(true)
            .setOngoing(false)
            .setTimeoutAfter(LOST_MS)
            .setGroup(group)
            .setGroupAlertBehavior(Notification.GROUP_ALERT_CHILDREN)
            .build();
        context.getSystemService(NotificationManager.class).notify(LOST, Notices.NOTE, note);
        Notices.tidy(context, Collections.singletonList(new Notices.Child(Notices.NOTE, LOST, group, System.currentTimeMillis() + LOST_MS)), Collections.emptySet());
    }
}
