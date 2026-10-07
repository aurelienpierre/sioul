// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.annotation.TargetApi;
import android.app.AutomaticZenRule;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.content.ActivityNotFoundException;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.net.Uri;
import android.os.Build;
import android.provider.Settings;
import android.service.notification.Condition;
import android.service.notification.ZenPolicy;
import android.util.Log;

import org.json.JSONException;
import org.json.JSONObject;

import java.util.Map;

/**
 * The phone's do-not-disturb during Sioul's pauses and for its own
 * do-not-disturb (crates/sioul-app/src/dnd.rs, docs/android.md "Pauses"): a
 * mode of Sioul's own for each, an AutomaticZenRule named "Pause" / "En
 * pause", "Free time" / "Temps libre" or "Do not disturb (Sioul)",
 * letting through what the matrix of what reaches you lets through then, as
 * far as Android can say it (sioul_core::attention::Silence, docs/attention.md
 * §3.3): calls and messages from nobody, starred contacts, contacts or
 * anyone; a second call within 15 minutes; the conversations marked
 * important in Android (Android 11 and later); alarms; the sound of what the
 * person plays. Sioul's dose reminders and an event's alarms, through
 * channels of their own that pass it. Turned on and off with
 * setAutomaticZenRuleState: the person's own do-not-disturb, and every other
 * mode, is never touched; the system combines them, the strictest winning.
 * Needs "Do Not Disturb access" (Modes access from Android 15), which the
 * person gives in Android's settings; without it, nothing is done and Rust
 * says so. Also the tile and the shortcut's press (PauseOpener, PauseTile).
 *
 * Rust asks through call(verb, json) (android/main.cpp), and is answered in JSON.
 */
final class PauseMode
{
    private static final String TAG = DoseAlarms.TAG;
    /** The dose reminders' channel while a pause lets them through: it passes do-not-disturb. */
    static final String DOSES_CHANNEL = "doses-pause";
    private static final String DOSES_USUAL = "doses";
    /** An event's alarms while one of Sioul's modes lets them through: it passes do-not-disturb (EventAlarms). */
    static final String ALARMS_CHANNEL = "event-alarms-pause";
    private static final String KEPT = "sioul-pause";
    private static final String PRESSED = "pressed";
    /** A press older than this is not taken: Sioul did not come up meanwhile. */
    private static final long PRESS_KEPT_MS = 5 * 60 * 1000L;
    /** Sioul's modes: the two pauses, and its do-not-disturb on every device. */
    private static final String[] KINDS = { "pause", "free-time", "dnd" };

    private PauseMode() {}

    /** Rust's question (android/main.cpp): "can", "enter", "leave", "pressed", "open". Any thread. */
    static String call(Context context, String verb, String json)
    {
        try {
            switch (verb) {
            case "can":
                return can(context).toString();
            case "enter":
                return enter(context, new JSONObject(json)).toString();
            case "leave":
                return leave(context, json).toString();
            case "pressed":
                return takePressed(context) ? "true" : "false";
            case "open":
                open(context, json);
                return "{}";
            default:
                return null;
            }
        } catch (JSONException | RuntimeException e) {
            Log.e(TAG, "Pause: " + verb + " failed: " + e);
            JSONObject answer = new JSONObject();
            put(answer, "access", accessGranted(context));
            put(answer, "error", e.getMessage() == null ? e.getClass().getSimpleName() : e.getMessage());
            return answer.toString();
        }
    }

    // ---------------------------------------------------------------- what Android allows

    /** Modes with their own policy and state, set by the app: Android 10 (API 29). */
    private static boolean tooOld()
    {
        return Build.VERSION.SDK_INT < Build.VERSION_CODES.Q;
    }

    private static NotificationManager notifications(Context context)
    {
        return context.getSystemService(NotificationManager.class);
    }

    private static boolean accessGranted(Context context)
    {
        return notifications(context).isNotificationPolicyAccessGranted();
    }

    private static JSONObject can(Context context)
    {
        JSONObject answer = new JSONObject();
        put(answer, "api", Build.VERSION.SDK_INT);
        if (tooOld()) {
            put(answer, "too_old", true);
            return answer;
        }
        put(answer, "access", accessGranted(context));
        return answer;
    }

    // ---------------------------------------------------------------- on, off

    /**
     * What one of Sioul's modes lets through, as Rust asks it (dnd.rs's Ask,
     * from the matrix: sioul_core::attention::Silence): who may call and who
     * may write ("none", "starred", "contacts", "anyone"), a second call
     * within 15 minutes, the conversations marked important in Android,
     * alarms, the doses' and an event's alarms' channels. An older Sioul says
     * only "people": starred contacts and repeat callers, or nobody; no
     * conversation. Pure, checked on a JVM.
     */
    static final class Through
    {
        /** ZenPolicy's PEOPLE_TYPE_*. */
        int calls;
        int messages;
        boolean repeat;
        boolean conversations;
        boolean alarms;
        boolean doses;
        boolean events;

        static Through of(JSONObject asked)
        {
            // The matrix's words beside the older ones (dnd.rs), or apart, as "silence".
            JSONObject said = asked.optJSONObject("silence");
            if (said == null)
                said = asked;
            Through through = new Through();
            int usual = asked.optBoolean("people", true) ? ZenPolicy.PEOPLE_TYPE_STARRED : ZenPolicy.PEOPLE_TYPE_NONE;
            through.calls = senders(said.optString("calls", ""), usual);
            through.messages = senders(said.optString("messages", ""), usual);
            through.repeat = said.optBoolean("repeat", through.calls != ZenPolicy.PEOPLE_TYPE_NONE);
            through.conversations = said.optBoolean("conversations", false);
            through.alarms = said.optBoolean("alarms", true);
            through.doses = said.optBoolean("doses", asked.optBoolean("doses", true));
            // An event's alarm is shown during a mode only when the matrix lets it come then (Rust decides).
            through.events = said.optBoolean("events", true);
            return through;
        }

        /** Android's senders for Rust's word; `usual` for none given, or a word not known. */
        static int senders(String word, int usual)
        {
            switch (word) {
            case "none":
                return ZenPolicy.PEOPLE_TYPE_NONE;
            case "starred":
                return ZenPolicy.PEOPLE_TYPE_STARRED;
            case "contacts":
                return ZenPolicy.PEOPLE_TYPE_CONTACTS;
            case "anyone":
                return ZenPolicy.PEOPLE_TYPE_ANYONE;
            default:
                return usual;
            }
        }

        /** The conversations let through, as Android names them (Android 11 and later). */
        int conversationSenders()
        {
            return conversations ? ZenPolicy.CONVERSATION_SENDERS_IMPORTANT : ZenPolicy.CONVERSATION_SENDERS_NONE;
        }
    }

    /**
     * The pause's mode on, made or brought up to date first: {kind, name,
     * trigger, calls, messages, repeat, conversations, alarms, doses,
     * events, channel, events_channel} (an older Sioul: {kind, name,
     * trigger, people, doses, channel}). Answers what the mode lets through,
     * read back from Android (the person may have changed it in Android's
     * settings): {api, access, rule: {name, calls, messages, repeat, alarms,
     * conversations (Android 11 and later), channels (15 and later)}, doses,
     * events, already, disabled, turned_off}.
     */
    @TargetApi(Build.VERSION_CODES.Q)
    private static JSONObject enter(Context context, JSONObject asked)
    {
        JSONObject answer = new JSONObject();
        if (tooOld()) {
            put(answer, "too_old", true);
            return answer;
        }
        put(answer, "api", Build.VERSION.SDK_INT);
        boolean access = accessGranted(context);
        put(answer, "access", access);
        if (!access)
            return answer;
        NotificationManager notifications = notifications(context);
        String kind = asked.optString("kind", "pause");
        Through through = Through.of(asked);
        String name = asked.optString("name", "pause".equals(kind) ? "Pause" : "Free time");
        Uri condition = conditionId(context, kind);
        ZenPolicy policy = policy(through);
        SharedPreferences kept = kept(context);
        boolean wasOn = kept.getBoolean("on." + kind, false);

        // The channels that pass first: a reminder due meanwhile finds them.
        boolean dosesThrough = false;
        if (through.doses)
            dosesThrough = passingChannel(notifications, DOSES_CHANNEL, asked.optString("channel", "Doses during a pause"));
        boolean eventsThrough = false;
        if (through.events)
            eventsThrough = passingChannel(notifications, ALARMS_CHANNEL, asked.optString("events_channel", EventAlarms.word(context, "alarms-pause")));

        String id = null;
        AutomaticZenRule existing = null;
        Map<String, AutomaticZenRule> rules = notifications.getAutomaticZenRules();
        for (Map.Entry<String, AutomaticZenRule> rule : rules.entrySet()) {
            if (condition.equals(rule.getValue().getConditionId())) {
                id = rule.getKey();
                existing = rule.getValue();
                break;
            }
        }
        // A mode the person turned off in Android's settings stays off.
        boolean enabled = existing == null || existing.isEnabled();
        AutomaticZenRule wanted = rule(context, name, asked.optString("trigger", ""), condition, policy, enabled);
        boolean fresh = !wasOn;
        if (existing == null) {
            id = notifications.addAutomaticZenRule(wanted);
            fresh = true;
            Log.i(TAG, "Pause: the mode \"" + kind + "\" made.");
        } else if (!same(existing, wanted)) {
            // An update that changes the mode turns it off (up to Android 14,
            // any update): on again below, without overriding the person.
            notifications.updateAutomaticZenRule(id, wanted);
            fresh = true;
            Log.i(TAG, "Pause: the mode \"" + kind + "\" brought up to date.");
        }
        boolean others = otherOn(kept, kind);
        if (!enabled) {
            put(answer, "disabled", true);
        } else if (fresh) {
            int filter = notifications.getCurrentInterruptionFilter();
            boolean already = !wasOn && !others && filter != NotificationManager.INTERRUPTION_FILTER_ALL && filter != NotificationManager.INTERRUPTION_FILTER_UNKNOWN;
            if (!wasOn)
                kept.edit().putBoolean("already." + kind, already).commit();
            put(answer, "already", already);
            notifications.setAutomaticZenRuleState(id, state(condition, name, true, !wasOn));
        } else if (turnedOff(notifications, id)) {
            // Turned off by the person during the pause (the quick settings): left off.
            put(answer, "turned_off", true);
        }
        kept.edit().putBoolean("on." + kind, enabled).putBoolean("doses." + kind, through.doses).putBoolean("events." + kind, through.events).commit();

        AutomaticZenRule now = notifications.getAutomaticZenRule(id);
        ZenPolicy set = now == null ? null : now.getZenPolicy();
        JSONObject rule = new JSONObject();
        put(rule, "name", now == null ? name : now.getName());
        if (set != null) {
            put(rule, "calls", set.getPriorityCallSenders());
            put(rule, "messages", set.getPriorityMessageSenders());
            put(rule, "repeat", set.getPriorityCategoryRepeatCallers());
            put(rule, "alarms", set.getPriorityCategoryAlarms());
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R)
                put(rule, "conversations", set.getPriorityConversationSenders());
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.VANILLA_ICE_CREAM) {
                int channels = set.getPriorityChannelsAllowed();
                put(rule, "channels", channels);
                if (channels == ZenPolicy.STATE_DISALLOW) {
                    dosesThrough = false;
                    eventsThrough = false;
                }
            }
        }
        put(answer, "rule", rule);
        put(answer, "doses", dosesThrough);
        put(answer, "events", eventsThrough);
        return answer;
    }

    /** The pause's mode off, Sioul's own and nothing else: {was, still}. */
    @TargetApi(Build.VERSION_CODES.Q)
    private static JSONObject leave(Context context, String kind)
    {
        JSONObject answer = new JSONObject();
        if (tooOld()) {
            put(answer, "too_old", true);
            return answer;
        }
        SharedPreferences kept = kept(context);
        boolean was = kept.getBoolean("on." + kind, false);
        boolean already = kept.getBoolean("already." + kind, false);
        kept.edit().putBoolean("on." + kind, false).remove("already." + kind).commit();
        put(answer, "was", was);
        NotificationManager notifications = notifications(context);
        if (!notifications.isNotificationPolicyAccessGranted()) {
            // Taking the access back removed Sioul's modes: nothing is left on.
            put(answer, "access", false);
            return answer;
        }
        Uri condition = conditionId(context, kind);
        for (Map.Entry<String, AutomaticZenRule> rule : notifications.getAutomaticZenRules().entrySet()) {
            if (condition.equals(rule.getValue().getConditionId()))
                notifications.setAutomaticZenRuleState(rule.getKey(), state(condition, rule.getValue().getName(), false, true));
        }
        int filter = notifications.getCurrentInterruptionFilter();
        // Said only of a do-not-disturb on before the pause, and still on.
        put(answer, "still", already && !otherOn(kept, kind) && filter != NotificationManager.INTERRUPTION_FILTER_ALL && filter != NotificationManager.INTERRUPTION_FILTER_UNKNOWN);
        Log.i(TAG, "Pause: the mode \"" + kind + "\" off" + (was ? "." : " (it was not on)."));
        return answer;
    }

    /**
     * Who gets through, every field set: one left unset would take the
     * person's own do-not-disturb settings. Calls and messages as Rust asks
     * (nobody, starred contacts, contacts, anyone), a second call within 15
     * minutes, the conversations marked important (Android 11 and later;
     * Android 10 has no conversations: they are messages there); alarms;
     * media (the music the person plays); priority channels (Sioul's doses
     * and an event's alarms, from Android 15; before, Android lets a channel
     * that passes through whatever the mode says). Other apps' reminders and
     * events stay held. Notifications held stay in the shade and the status
     * bar, without sound, banner, light, full screen or ambient display, as
     * Android's own do-not-disturb does by default; they come back after.
     */
    @TargetApi(Build.VERSION_CODES.Q)
    private static ZenPolicy policy(Through through)
    {
        ZenPolicy.Builder policy = new ZenPolicy.Builder()
            .allowCalls(through.calls)
            .allowMessages(through.messages)
            .allowRepeatCallers(through.repeat)
            .allowAlarms(through.alarms)
            .allowMedia(true)
            .allowSystem(false)
            .allowReminders(false)
            .allowEvents(false)
            .showPeeking(false)
            .showFullScreenIntent(false)
            .showLights(false)
            .showInAmbientDisplay(false)
            .showStatusBarIcons(true)
            .showBadges(true)
            .showInNotificationList(true);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R)
            policy.allowConversations(through.conversationSenders());
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.VANILLA_ICE_CREAM)
            policy.allowPriorityChannels(true);
        return policy.build();
    }

    /**
     * The mode as Sioul makes it. Its settings in Android open Sioul (the
     * configuration activity a mode needs without a condition provider).
     * From Android 15, a mode of its own kind with its words and icon, not
     * turned on from Android's settings (only a pause turns it on).
     */
    @TargetApi(Build.VERSION_CODES.Q)
    private static AutomaticZenRule rule(Context context, String name, String trigger, Uri condition, ZenPolicy policy, boolean enabled)
    {
        ComponentName sioul = window(context);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.VANILLA_ICE_CREAM) {
            AutomaticZenRule.Builder rule = new AutomaticZenRule.Builder(name, condition)
                .setType(AutomaticZenRule.TYPE_OTHER)
                .setConfigurationActivity(sioul)
                .setZenPolicy(policy)
                .setInterruptionFilter(NotificationManager.INTERRUPTION_FILTER_PRIORITY)
                .setIconResId(R.drawable.sioul_pause)
                .setManualInvocationAllowed(false)
                .setEnabled(enabled);
            if (!trigger.isEmpty())
                rule.setTriggerDescription(trigger);
            return rule.build();
        }
        return new AutomaticZenRule(name, null, sioul, condition, policy, NotificationManager.INTERRUPTION_FILTER_PRIORITY, enabled);
    }

    /** Whether a mode is already as Sioul would make it. */
    @TargetApi(Build.VERSION_CODES.Q)
    private static boolean same(AutomaticZenRule existing, AutomaticZenRule wanted)
    {
        boolean same = wanted.getName().equals(existing.getName())
            && existing.getInterruptionFilter() == wanted.getInterruptionFilter()
            && existing.isEnabled() == wanted.isEnabled()
            && wanted.getZenPolicy().equals(existing.getZenPolicy());
        if (same && Build.VERSION.SDK_INT >= Build.VERSION_CODES.VANILLA_ICE_CREAM) {
            String trigger = wanted.getTriggerDescription();
            same = trigger == null || trigger.equals(existing.getTriggerDescription());
        }
        return same;
    }

    /** The mode's state as Sioul asks it: a press (or its end) is the person's own act. */
    private static Condition state(Uri condition, String name, boolean on, boolean pressed)
    {
        int state = on ? Condition.STATE_TRUE : Condition.STATE_FALSE;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.VANILLA_ICE_CREAM)
            return new Condition(condition, name, state, pressed ? Condition.SOURCE_USER_ACTION : Condition.SOURCE_UNKNOWN);
        return new Condition(condition, name, state);
    }

    /** Whether the person turned the mode off during the pause. */
    @TargetApi(Build.VERSION_CODES.Q)
    private static boolean turnedOff(NotificationManager notifications, String id)
    {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.VANILLA_ICE_CREAM)
            return notifications.getAutomaticZenRuleState(id) != Condition.STATE_TRUE;
        return notifications.getCurrentInterruptionFilter() == NotificationManager.INTERRUPTION_FILTER_ALL;
    }

    /** Sioul's own address for a pause's mode: condition://com.aurelienpierre.sioul/pause. */
    private static Uri conditionId(Context context, String kind)
    {
        return new Uri.Builder().scheme(Condition.SCHEME).authority(context.getPackageName()).appendPath(kind).build();
    }

    /** Sioul's window, as its icon opens it. */
    private static ComponentName window(Context context)
    {
        Intent launcher = context.getPackageManager().getLaunchIntentForPackage(context.getPackageName());
        ComponentName window = launcher == null ? null : launcher.getComponent();
        return window != null ? window : new ComponentName(context.getPackageName(), "org.qtproject.qt.android.bindings.QtActivity");
    }

    private static boolean otherOn(SharedPreferences kept, String kind)
    {
        for (String other : KINDS)
            if (!other.equals(kind) && kept.getBoolean("on." + other, false))
                return true;
        return false;
    }

    // ---------------------------------------------------------------- the channels that pass

    /**
     * A channel that passes do-not-disturb, for what one of Sioul's modes
     * lets through: the doses ("Doses during a pause"), an event's alarms,
     * the mail of someone Always through. At Android's default importance
     * (its sound, no banner), allowed to pass with Do Not Disturb access, and
     * only while the person has not changed the channel: then their choice
     * stays. The usual channels are never changed: outside Sioul's modes, the
     * person's own do-not-disturb holds them as before. Whether it gets
     * through, as Android has the channel now.
     */
    private static boolean passingChannel(NotificationManager notifications, String id, String name)
    {
        NotificationChannel channel = new NotificationChannel(id, name, NotificationManager.IMPORTANCE_DEFAULT);
        channel.setBypassDnd(true);
        notifications.createNotificationChannel(channel);
        NotificationChannel kept = notifications.getNotificationChannel(id);
        return kept != null && kept.canBypassDnd() && kept.getImportance() != NotificationManager.IMPORTANCE_NONE && notifications.areNotificationsEnabled();
    }

    /**
     * Whether a channel that passes is the one now: one of Sioul's modes is
     * on, and every mode on lets `flag` through ("doses.", "events."; null:
     * any mode on will do), as `kept` has them (SharedPreferences.getAll).
     * Pure, checked on a JVM.
     */
    static boolean passesNow(Map<String, ?> kept, String flag)
    {
        boolean on = false;
        for (String kind : KINDS) {
            if (!Boolean.TRUE.equals(kept.get("on." + kind)))
                continue;
            // Not said (a mode entered by an older Sioul): let through, as before.
            if (flag != null && Boolean.FALSE.equals(kept.get(flag + kind)))
                return false;
            on = true;
        }
        return on;
    }

    private static String channelNow(Context context, String flag, String passing, String usual)
    {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q || !passesNow(kept(context).getAll(), flag))
            return usual;
        return notifications(context).getNotificationChannel(passing) != null ? passing : usual;
    }

    /**
     * The channel a dose reminder goes to now (DoseAlarms): the pauses' own
     * while every mode on lets the doses through, the usual one otherwise
     * (a mode that holds them, or none).
     */
    static String dosesChannelNow(Context context)
    {
        return channelNow(context, "doses.", DOSES_CHANNEL, DOSES_USUAL);
    }

    /**
     * The channel an event's alarm goes to now (EventAlarms): the one that
     * passes while every mode on lets an event's alarms through, the usual
     * "An event's alarms" otherwise. Sioul's reminders before an event never
     * come here: they stay in the quiet "Events".
     */
    static String alarmsChannelNow(Context context)
    {
        return channelNow(context, "events.", ALARMS_CHANNEL, EventAlarms.ALARMS);
    }

    /** New mail's channel that passes, for someone Always through while one of Sioul's modes is on (MailNotes). */
    static final String MAIL_CHANNEL = "mail-through";

    /**
     * The channel new mail goes to now (MailNotes, from the window's process
     * or the background service's): the one that passes while one of Sioul's
     * modes is on and someone Always through wrote (`through`, Rust's word:
     * told through the system's do-not-disturb), made then with `name`; the
     * quiet "New mail" otherwise.
     */
    static String mailChannelNow(Context context, boolean through, String name)
    {
        if (!through || Build.VERSION.SDK_INT < Build.VERSION_CODES.Q || !passesNow(kept(context).getAll(), null))
            return MailNotes.CHANNEL;
        NotificationManager notifications = notifications(context);
        if (accessGranted(context))
            passingChannel(notifications, MAIL_CHANNEL, name);
        return notifications.getNotificationChannel(MAIL_CHANNEL) != null ? MAIL_CHANNEL : MailNotes.CHANNEL;
    }

    // ---------------------------------------------------------------- the tile and the shortcut

    /** "Pause" pressed on the tile or the home screen's shortcut (PauseOpener). */
    static void pressed(Context context)
    {
        kept(context).edit().putLong(PRESSED, System.currentTimeMillis()).commit();
        Log.i(TAG, "Pause: pressed, kept for Sioul.");
    }

    /** A press since last asked, and recent; once. */
    private static boolean takePressed(Context context)
    {
        SharedPreferences kept = kept(context);
        long at = kept.getLong(PRESSED, 0);
        if (at == 0)
            return false;
        kept.edit().remove(PRESSED).commit();
        long age = System.currentTimeMillis() - at;
        return age >= 0 && age < PRESS_KEPT_MS;
    }

    /** Whether the pause for overwhelming moments is on here: the tile shows it. */
    static boolean paused(Context context)
    {
        return kept(context).getBoolean("on.pause", false);
    }

    // ---------------------------------------------------------------- Android's pages

    /**
     * "access": Android's list where Sioul is given Do Not Disturb access (no
     * page there on some phones: Sioul's own page then). "starred": the
     * Contacts app's starred list, where the person stars who may reach them
     * (Sioul never writes stars), else the Contacts app.
     */
    private static void open(Context context, String which)
    {
        String app = context.getPackageName();
        Intent[] pages;
        if ("starred".equals(which)) {
            pages = new Intent[] {
                new Intent("com.android.contacts.action.LIST_STARRED"),
                Intent.makeMainSelectorActivity(Intent.ACTION_MAIN, Intent.CATEGORY_APP_CONTACTS),
            };
        } else {
            pages = new Intent[] {
                new Intent(Settings.ACTION_NOTIFICATION_POLICY_ACCESS_SETTINGS),
                new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:" + app)),
            };
        }
        for (Intent page : pages) {
            try {
                context.startActivity(page.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
                return;
            } catch (ActivityNotFoundException | SecurityException e) {
                // The next one.
            }
        }
        Log.w(TAG, "Pause: no page to open for \"" + which + "\".");
    }

    // ---------------------------------------------------------------- kept

    /**
     * What Sioul's own process keeps of its modes (the only one that turns
     * them on and off), read again when it changed: new mail is told from
     * the background service's process too (MailNotes).
     */
    @SuppressWarnings("deprecation")
    private static SharedPreferences kept(Context context)
    {
        return context.getSharedPreferences(KEPT, Context.MODE_PRIVATE | Context.MODE_MULTI_PROCESS);
    }

    private static void put(JSONObject object, String name, Object value)
    {
        try {
            object.put(name, value);
        } catch (JSONException e) {
            // Only for a name that is null or a number that is not one.
        }
    }
}
