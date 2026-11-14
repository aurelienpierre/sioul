// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.KeyguardManager;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.graphics.drawable.Icon;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.PowerManager;
import android.service.notification.StatusBarNotification;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.List;

/**
 * Sioul's own notices on a phone, every one posted here (docs/android.md,
 * "Notifications"; docs/notifications.md): the time to leave for an event
 * (EventAlarms), new mail (MailNotes), the end of work, the night and the
 * meals (DayNotes), a one-time code (Rust's own, through StepService.call).
 * Rust says what each one is ({@link Said}, crates/sioul-app/src/
 * eventalarms.rs, {@code notice_json}): its thing's address, its class, how
 * long it lasts once it could be seen, when it goes at the latest. Here it
 * is only posted, in one of two kinds:
 *
 * - Something to know ("informative"): the "Notices" channel, at low
 *   importance (no sound, no banner), alerting once, taken away by a tap,
 *   never ongoing. It lasts its minutes from when you could see it: posted
 *   while the phone is in use (its screen on and unlocked), from then;
 *   otherwise it waits in the shade until the next unlock (the background
 *   service hears it: {@link #seen}), then lasts its minutes. Never past its
 *   relevance (Rust's "ends"), seen or not.
 * - Something to do ("actionable"): the "To act on" channel, at Android's
 *   default importance (its sound, no banner), or its twin that passes
 *   do-not-disturb while one of Sioul's modes is on and Rust says it
 *   pierces (PauseMode); no timeout but its thing's end (a code's expiry);
 *   taken away by a tap, or when Rust says its thing is settled
 *   ({@link #withdraw}).
 *
 * Every notice is in a group of its own class, so that Android groups none
 * of Sioul's by itself (it would make one card of them, ongoing beside the
 * background service's); on Android 16, which groups a group without a
 * summary all the same, each group gets its summary, taken away with its
 * last notice ({@link #tidy}). A tap, or a button, opens its thing by its
 * address (ReminderOpener), never a file; Copy puts a code on the clipboard
 * (NoticeReceiver). On the lock screen, private: Sioul's words for a
 * reminder, or what Rust gives (a count, a code's title), never more.
 *
 * The channels of an older Sioul ("events", "event-alarms", "mail", "day"…)
 * are deleted once these exist: Android never lets an app change a channel's
 * importance. Never the doses' (DoseAlarms keeps its own).
 */
final class Notices
{
    /** Something to know: "Notices", low importance. */
    static final String NOTICES = "notices";
    /** Something to do: "To act on", Android's default importance. */
    static final String ACT = "act";
    /** Its twin that passes do-not-disturb, made by PauseMode while one of Sioul's modes lets codes through. */
    static final String ACT_PAUSE = "act-pause";
    /**
     * The channels an older Sioul made, deleted once ours exist. New mail's
     * own that passes ("mail-through") stays while the owner's word on
     * notices to know that pierce is awaited (docs/android.md). Never
     * "doses" nor "doses-pause".
     */
    static final String[] RETIRED = { "events", "event-alarms", "event-alarms-pause", "mail", "day" };
    /** Every notice is known by its tag (Rust's) and this number. */
    static final int NOTE = 9;
    /** A group's summary on Android 16, by this tag and its group. */
    static final String SUMMARY = "sioul:summary:";
    /** The groups' keys: "sioul." and the class. */
    static final String GROUP = "sioul.";
    static final String INFORMATIVE = "informative";
    static final String ACTIONABLE = "actionable";
    /** Android 16 (API 36): a group child without a summary is grouped by force. */
    static final int SUMMARIES_FROM = 36;
    /** Android shows three buttons at most. */
    static final int AT_MOST = 3;

    /** What a notice keeps in its extras: its thing, how long it lasts once seen, when it goes at the latest, and whether it was seen yet. */
    static final String ABOUT = "sioul.about";
    static final String LASTS = "sioul.lasts";
    static final String ENDS = "sioul.ends";
    static final String UNSEEN = "sioul.unseen";

    private Notices() {}

    // ---------------------------------------------------------------- what Rust says

    /** A button: its words, and the address it opens or the text it copies (one of them). */
    static final class Button
    {
        final String label;
        final String open;
        final String copy;

        Button(String label, String open, String copy)
        {
            this.label = label;
            this.open = open;
            this.copy = copy;
        }
    }

    /**
     * A notice as Rust says it (eventalarms.rs, {@code notice_json}), read
     * whole and checked: an address that is no thing of Sioul's is none, a
     * button that opens no such thing is left out. Pure, checked on a JVM
     * (android/jvm-checks/NoticesCheck.java).
     */
    static final class Said
    {
        /** Android's tag: the next notice with it takes its place. */
        String tag = "";
        /** Its thing's address, which a tap opens; "" none (Sioul as by its icon). */
        String about = "";
        /** The matrix's row ("before", "needs", "codes"), "mail", "minor". */
        String kind = "";
        boolean actionable;
        /** Its group: its class. */
        String group = INFORMATIVE;
        /** How long it stays once it could be seen (ms); 0, until settled. */
        long lastsMs;
        /** When it goes at the latest (Unix ms); 0, none. */
        long endsMs;
        /** Something to do that passes one of Sioul's modes. */
        boolean pierce;
        String title = "";
        String body = "";
        /** On the lock screen; null: Sioul's words for a reminder. */
        String publicTitle;
        String publicBody;
        final List<Button> buttons = new ArrayList<>();
        /** The words Rust gave with it (the channels' names), or null. */
        JSONObject words;
        /** A channel of its own (new mail's that passes); null: its class's. */
        String channel;

        /**
         * Rust's JSON: {tag, about, kind, class, group, lasts_ms, ends,
         * pierce, title, body, actions: [{label, open} | {label, copy}],
         * public: {title, body}, words}.
         *
         * @param said the notice, or null
         * @return the notice read; its tag "" when there is nothing to post
         */
        static Said of(JSONObject said)
        {
            Said out = new Said();
            if (said == null)
                return out;
            out.tag = said.optString("tag");
            String about = said.optString("about");
            out.about = ReminderOpener.isNoticeAddress(about) ? about : "";
            out.kind = said.optString("kind");
            out.actionable = ACTIONABLE.equals(said.optString("class"));
            String group = said.optString("group");
            out.group = INFORMATIVE.equals(group) || ACTIONABLE.equals(group) ? group : out.actionable ? ACTIONABLE : INFORMATIVE;
            out.lastsMs = Math.max(0, said.optLong("lasts_ms"));
            out.endsMs = Math.max(0, said.optLong("ends"));
            out.pierce = out.actionable && said.optBoolean("pierce");
            out.title = said.optString("title");
            out.body = said.optString("body");
            JSONObject shown = said.optJSONObject("public");
            if (shown != null) {
                out.publicTitle = shown.optString("title");
                out.publicBody = shown.optString("body");
            }
            JSONArray actions = said.optJSONArray("actions");
            for (int i = 0; actions != null && i < actions.length() && out.buttons.size() < AT_MOST; i++) {
                JSONObject action = actions.optJSONObject(i);
                if (action == null || action.optString("label").isEmpty())
                    continue;
                String open = action.optString("open");
                String copy = action.optString("copy");
                if (!open.isEmpty() && ReminderOpener.isNoticeAddress(open))
                    out.buttons.add(new Button(action.optString("label"), open, ""));
                else if (open.isEmpty() && !copy.isEmpty())
                    out.buttons.add(new Button(action.optString("label"), "", copy));
            }
            out.words = said.optJSONObject("words");
            return out;
        }
    }

    // ---------------------------------------------------------------- how long

    /**
     * How long Android keeps a notice from `now` (ms, as setTimeoutAfter
     * takes it); 0, no limit. Something to do: until its thing ends
     * (`ends`), else no limit. Something to know: seen (the phone in use),
     * its minutes (`lasts`), never past `ends`; not seen yet, until `ends`
     * (none: until it is seen), its minutes counted from the next unlock
     * ({@link #seen}). Pure, checked on a JVM (NoticesCheck).
     *
     * @param actionable something to do
     * @param lastsMs how long it stays once seen (ms); 0, until settled
     * @param endsMs when it goes at the latest (Unix ms); 0, none
     * @param now the time now (Unix ms)
     * @param seen whether the phone is in use now
     * @return the timeout (ms), at least 1; 0 for none
     */
    static long timeout(boolean actionable, long lastsMs, long endsMs, long now, boolean seen)
    {
        long cap = endsMs > 0 ? Math.max(1, endsMs - now) : 0;
        if (actionable || !seen)
            return cap;
        if (lastsMs <= 0)
            return cap;
        return cap > 0 ? Math.min(lastsMs, cap) : lastsMs;
    }

    /**
     * When a group's summary goes (ms from `now`, as setTimeoutAfter takes
     * it): with its last notice, each notice's end its post time and timeout;
     * 0, never, when one of them has none. Pure (NoticesCheck).
     *
     * @param posted each notice's post time (Unix ms)
     * @param timeouts each notice's timeout (ms; 0 none), in the same order
     * @param now the time now (Unix ms)
     * @return the summary's timeout (ms), at least 1; 0 for none
     */
    static long summaryTimeout(long[] posted, long[] timeouts, long now)
    {
        long last = 0;
        for (int i = 0; i < posted.length && i < timeouts.length; i++) {
            if (timeouts[i] <= 0)
                return 0;
            last = Math.max(last, posted[i] + timeouts[i]);
        }
        return last > 0 ? Math.max(1, last - now) : 0;
    }

    /**
     * The channel a notice goes to: its own when it says one (new mail's
     * that passes); something to do, "To act on", or its twin that passes
     * while one of Sioul's modes is on and Rust says it pierces (`passing`:
     * PauseMode's answer); something to know, "Notices". Pure (NoticesCheck).
     *
     * @param said the notice
     * @param passing the twin that passes, when it is the one now; else null
     * @return the channel's id
     */
    static String channelOf(Said said, String passing)
    {
        if (said.channel != null && !said.channel.isEmpty())
            return said.channel;
        if (!said.actionable)
            return NOTICES;
        return said.pierce && passing != null ? passing : ACT;
    }

    /**
     * Each press's own request code, so that Android keeps their intents
     * apart (the same address in two notices, two buttons of one): the
     * notice's tag and what the press opens or copies.
     *
     * @param tag the notice's tag
     * @param what the address it opens, or "copy"
     * @return the request code
     */
    static int requestCode(String tag, String what)
    {
        return (tag + "#" + what).hashCode();
    }

    // ---------------------------------------------------------------- posting

    /** Rust's notice (eventalarms.rs, `notice_json`), posted as `post` says. Any thread. */
    static void show(Context context, String json)
    {
        final JSONObject said;
        try {
            said = new JSONObject(json);
        } catch (JSONException e) {
            // Its kind of failure only: org.json's message quotes the text, which may name what it is about.
            Log.e(DoseAlarms.TAG, "Notices: one that does not read (" + e.getClass().getSimpleName() + ").");
            return;
        }
        post(context, Said.of(said));
    }

    /**
     * A notice posted in its place (its tag), in its class's channel and
     * group, lasting as {@link #timeout} says; nothing once it ended, or
     * without a tag. Its group's summary follows (Android 16). Any thread.
     */
    static void post(Context context, Said said)
    {
        long now = System.currentTimeMillis();
        if (said.tag.isEmpty() || (said.endsMs > 0 && said.endsMs <= now))
            return;
        if (said.words != null)
            EventAlarms.keepWords(context, said.words);
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        channels(context, said.words);
        String passing = said.actionable && said.pierce ? PauseMode.actChannelNow(context, word(context, said.words, "act-pause")) : null;
        String channel = channelOf(said, passing);
        boolean seen = inUse(context);
        long timeout = timeout(said.actionable, said.lastsMs, said.endsMs, now, seen);
        Bundle extras = new Bundle();
        extras.putString(ABOUT, said.about);
        extras.putLong(LASTS, said.lastsMs);
        extras.putLong(ENDS, said.endsMs);
        extras.putBoolean(UNSEEN, !said.actionable && !seen);
        Notification.Builder note = new Notification.Builder(context, channel)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentTitle(said.title.isEmpty() ? "Sioul" : said.title)
            .setContentText(said.body)
            .setStyle(new Notification.BigTextStyle().bigText(said.body))
            .setCategory("mail".equals(said.kind) ? Notification.CATEGORY_EMAIL : Notification.CATEGORY_REMINDER)
            // Where the phone hides what a notification says on its lock screen: Rust's words for it, or
            // that Sioul has a reminder, never what it is about (composed tasks §7.3), never a code.
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .setPublicVersion(publicVersion(context, channel, said))
            .setContentIntent(opening(context, said.tag, said.about))
            .setDeleteIntent(NoticeReceiver.deleted(context, said.tag))
            .setAutoCancel(true)
            .setOnlyAlertOnce(true)
            .setOngoing(false)
            .setGroup(GROUP + said.group)
            .setGroupAlertBehavior(Notification.GROUP_ALERT_CHILDREN)
            .addExtras(extras);
        if (timeout > 0)
            note.setTimeoutAfter(timeout);
        for (Button button : said.buttons) {
            PendingIntent press = button.open.isEmpty() ? NoticeReceiver.copying(context, said.tag, button.copy) : opening(context, said.tag, button.open);
            note.addAction(new Notification.Action.Builder(Icon.createWithResource(context, R.drawable.sioul_notification), button.label, press).build());
        }
        notifications.notify(said.tag, NOTE, note.build());
        tidy(context);
    }

    /**
     * What a tap or a button opens: Sioul on the thing by its address
     * (ReminderOpener, "notice"), this notice named to go; without one,
     * Sioul as by its icon.
     */
    static PendingIntent opening(Context context, String tag, String address)
    {
        String kind = ReminderOpener.isNoticeAddress(address) ? ReminderOpener.NOTICE : "app";
        Intent sioul = new Intent(context, ReminderOpener.class).setData(Uri.fromParts(ReminderOpener.SCHEME, kind, null))
                           .putExtra(ReminderOpener.KEY, ReminderOpener.NOTICE.equals(kind) ? address : "").putExtra(ReminderOpener.NOTE, tag).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        return PendingIntent.getActivity(context, requestCode(tag, address), sioul, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /** As the lock screen shows it where the phone hides private content: Rust's words for it, else that Sioul has a reminder. */
    private static Notification publicVersion(Context context, String channel, Said said)
    {
        boolean given = said.publicTitle != null && !said.publicTitle.isEmpty();
        return new Notification.Builder(context, channel)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentTitle(given ? said.publicTitle : "Sioul")
            .setContentText(given ? said.publicBody : EventAlarms.word(context, "public"))
            .build();
    }

    /**
     * Whether the phone is in use now: its screen on and unlocked. A notice
     * to know posted then counts its minutes at once; otherwise from the
     * next unlock.
     */
    static boolean inUse(Context context)
    {
        PowerManager power = context.getSystemService(PowerManager.class);
        KeyguardManager keyguard = context.getSystemService(KeyguardManager.class);
        return power != null && power.isInteractive() && (keyguard == null || !keyguard.isKeyguardLocked());
    }

    // ---------------------------------------------------------------- seen, settled

    /**
     * The phone unlocked (the background service hears it), or Sioul on
     * the screen: each notice to know posted while nobody could see it lasts
     * its minutes from now, never past its end; posted again in its place,
     * silently (it alerts once). Any thread.
     */
    static void seen(Context context)
    {
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        long now = System.currentTimeMillis();
        channels(context, null);
        for (StatusBarNotification shown : active(notifications)) {
            Notification note = shown.getNotification();
            if (shown.getId() != NOTE || note.extras == null || !note.extras.getBoolean(UNSEEN, false))
                continue;
            long timeout = timeout(false, note.extras.getLong(LASTS), note.extras.getLong(ENDS), now, true);
            try {
                Notification.Builder again = Notification.Builder.recoverBuilder(context, note);
                again.getExtras().putBoolean(UNSEEN, false);
                if (timeout > 0)
                    again.setTimeoutAfter(timeout);
                notifications.notify(shown.getTag(), NOTE, again.build());
            } catch (RuntimeException e) {
                // One that cannot be made again here stays until its end: the others are seen all the same.
                Log.w(DoseAlarms.TAG, "Notices: one could not be counted from now (" + e.getClass().getSimpleName() + ").");
            }
        }
        tidy(context);
    }

    /** Every notice about `about` taken away: its thing is settled (a code's message read on some device). Any thread. */
    static void withdraw(Context context, String about)
    {
        if (about == null || about.isEmpty())
            return;
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        for (StatusBarNotification shown : active(notifications)) {
            Bundle extras = shown.getNotification().extras;
            if (shown.getId() == NOTE && extras != null && about.equals(extras.getString(ABOUT)))
                notifications.cancel(shown.getTag(), NOTE);
        }
        tidy(context);
    }

    /** The notice `tag` taken away (a button pressed, a code copied, said for nothing); nothing when none shows. */
    static void dismiss(Context context, String tag)
    {
        if (tag == null || tag.isEmpty())
            return;
        context.getSystemService(NotificationManager.class).cancel(tag, NOTE);
        tidy(context);
    }

    private static List<StatusBarNotification> active(NotificationManager notifications)
    {
        List<StatusBarNotification> out = new ArrayList<>();
        try {
            for (StatusBarNotification shown : notifications.getActiveNotifications())
                out.add(shown);
        } catch (RuntimeException e) {
            // The system's service gone a moment (a restart): nothing to look at.
        }
        return out;
    }

    // ---------------------------------------------------------------- groups and channels

    /**
     * Each group's summary as Android 16 needs it: one while the group has
     * a notice, going with its last (its timeout the latest of theirs, none
     * when one of them stays), taken away once none is left; never before 16,
     * where a group without a summary is grouped by nobody. Any thread.
     */
    static void tidy(Context context)
    {
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        List<StatusBarNotification> shown = active(notifications);
        long now = System.currentTimeMillis();
        for (String group : new String[] { INFORMATIVE, ACTIONABLE }) {
            List<StatusBarNotification> children = new ArrayList<>();
            boolean summary = false;
            for (StatusBarNotification one : shown) {
                if (one.getId() != NOTE || !(GROUP + group).equals(one.getNotification().getGroup()))
                    continue;
                if ((SUMMARY + group).equals(one.getTag()))
                    summary = true;
                else
                    children.add(one);
            }
            if (children.isEmpty() || Build.VERSION.SDK_INT < SUMMARIES_FROM) {
                if (summary)
                    notifications.cancel(SUMMARY + group, NOTE);
                continue;
            }
            long[] posted = new long[children.size()];
            long[] timeouts = new long[children.size()];
            for (int i = 0; i < children.size(); i++) {
                posted[i] = children.get(i).getPostTime();
                timeouts[i] = children.get(i).getNotification().getTimeoutAfter();
            }
            String channel = ACTIONABLE.equals(group) ? ACT : NOTICES;
            Notification.Builder head = new Notification.Builder(context, channel)
                .setSmallIcon(R.drawable.sioul_notification)
                .setColor(context.getColor(R.color.sioul_green))
                .setContentTitle(word(context, null, ACTIONABLE.equals(group) ? "act" : "notices"))
                .setVisibility(Notification.VISIBILITY_PRIVATE)
                .setGroup(GROUP + group)
                .setGroupSummary(true)
                .setGroupAlertBehavior(Notification.GROUP_ALERT_CHILDREN)
                .setOnlyAlertOnce(true)
                .setAutoCancel(true);
            long timeout = summaryTimeout(posted, timeouts, now);
            if (timeout > 0)
                head.setTimeoutAfter(timeout);
            notifications.notify(SUMMARY + group, NOTE, head.build());
        }
    }

    /** Whether the channels of an older Sioul were looked at in this process. */
    private static boolean retired;

    /**
     * The two channels made ("Notices", low; "To act on", default), their
     * names as Rust gave them (`words`, else those kept), each without a
     * badge (Channels.quiet); then, once per process, those an older Sioul
     * made deleted ({@link #RETIRED}): each notification still on them goes
     * with it. The twin that passes is PauseMode's to make. Any thread.
     */
    static void channels(Context context, JSONObject words)
    {
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        notifications.createNotificationChannel(Channels.quiet(NOTICES, word(context, words, "notices"), NotificationManager.IMPORTANCE_LOW));
        notifications.createNotificationChannel(Channels.quiet(ACT, word(context, words, "act"), NotificationManager.IMPORTANCE_DEFAULT));
        synchronized (Notices.class) {
            if (retired)
                return;
            retired = true;
        }
        for (String id : RETIRED) {
            NotificationChannel old = notifications.getNotificationChannel(id);
            if (old != null) {
                notifications.deleteNotificationChannel(id);
                Log.i(DoseAlarms.TAG, "Notices: the channel \"" + id + "\" of an older Sioul deleted.");
            }
        }
    }

    /** A word Rust gave with a notice (`words`), else the one kept or the phone's own (EventAlarms.word). */
    static String word(Context context, JSONObject words, String which)
    {
        String said = words == null ? "" : words.optString(which);
        return said.isEmpty() ? EventAlarms.word(context, which) : said;
    }
}
