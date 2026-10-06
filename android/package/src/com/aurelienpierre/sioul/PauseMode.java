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
 * letting through starred contacts and repeat callers (or nobody), alarms,
 * and the sound of what the person plays; Sioul's dose reminders through a
 * channel of their own that may pass it. Turned on and off with
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
     * The pause's mode on, made or brought up to date first: {kind, name,
     * trigger, people, doses, channel}. Answers what the mode lets through,
     * read back from Android (the person may have changed it in Android's
     * settings): {access, rule: {name, calls, messages, repeat, alarms},
     * doses, already, disabled, turned_off}.
     */
    @TargetApi(Build.VERSION_CODES.Q)
    private static JSONObject enter(Context context, JSONObject asked)
    {
        JSONObject answer = new JSONObject();
        if (tooOld()) {
            put(answer, "too_old", true);
            return answer;
        }
        boolean access = accessGranted(context);
        put(answer, "access", access);
        if (!access)
            return answer;
        NotificationManager notifications = notifications(context);
        String kind = asked.optString("kind", "pause");
        boolean people = asked.optBoolean("people", true);
        boolean doses = asked.optBoolean("doses", true);
        String name = asked.optString("name", "pause".equals(kind) ? "Pause" : "Free time");
        Uri condition = conditionId(context, kind);
        ZenPolicy policy = policy(people);
        SharedPreferences kept = kept(context);
        boolean wasOn = kept.getBoolean("on." + kind, false);

        // The doses' channel first: a reminder due meanwhile finds it.
        boolean dosesThrough = false;
        if (doses)
            dosesThrough = dosesChannel(context, notifications, asked.optString("channel", "Doses during a pause"));

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
        kept.edit().putBoolean("on." + kind, enabled).putBoolean("doses." + kind, doses).commit();

        AutomaticZenRule now = notifications.getAutomaticZenRule(id);
        ZenPolicy set = now == null ? null : now.getZenPolicy();
        JSONObject rule = new JSONObject();
        put(rule, "name", now == null ? name : now.getName());
        if (set != null) {
            put(rule, "calls", set.getPriorityCallSenders());
            put(rule, "messages", set.getPriorityMessageSenders());
            put(rule, "repeat", set.getPriorityCategoryRepeatCallers());
            put(rule, "alarms", set.getPriorityCategoryAlarms());
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.VANILLA_ICE_CREAM && set.getPriorityChannelsAllowed() == ZenPolicy.STATE_DISALLOW)
                dosesThrough = false;
        }
        put(answer, "rule", rule);
        put(answer, "doses", dosesThrough);
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
     * person's own do-not-disturb settings. Calls and messages from starred
     * contacts and a second call within 15 minutes, or nobody; alarms; media
     * (the music the person plays); priority channels (Sioul's doses, from
     * Android 15). Notifications held stay in the shade and the status bar,
     * without sound, banner, light, full screen or ambient display, as
     * Android's own do-not-disturb does by default; they come back after.
     */
    @TargetApi(Build.VERSION_CODES.Q)
    private static ZenPolicy policy(boolean people)
    {
        int who = people ? ZenPolicy.PEOPLE_TYPE_STARRED : ZenPolicy.PEOPLE_TYPE_NONE;
        ZenPolicy.Builder policy = new ZenPolicy.Builder()
            .allowCalls(who)
            .allowMessages(who)
            .allowRepeatCallers(people)
            .allowAlarms(true)
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
            policy.allowConversations(ZenPolicy.CONVERSATION_SENDERS_NONE);
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

    // ---------------------------------------------------------------- the doses

    /**
     * The channel of the dose reminders during a pause that lets them
     * through: it passes do-not-disturb (allowed with Do Not Disturb access,
     * and only while the person has not changed the channel: then their
     * choice stays). The usual "Doses" channel is never changed: outside the
     * pauses, the person's own do-not-disturb holds the reminders as before.
     * Whether they get through, as Android has the channel now.
     */
    private static boolean dosesChannel(Context context, NotificationManager notifications, String name)
    {
        NotificationChannel channel = new NotificationChannel(DOSES_CHANNEL, name, NotificationManager.IMPORTANCE_DEFAULT);
        channel.setBypassDnd(true);
        notifications.createNotificationChannel(channel);
        NotificationChannel kept = notifications.getNotificationChannel(DOSES_CHANNEL);
        return kept != null && kept.canBypassDnd() && kept.getImportance() != NotificationManager.IMPORTANCE_NONE && notifications.areNotificationsEnabled();
    }

    /**
     * The channel a dose reminder goes to now (DoseAlarms): the pauses' own
     * while every pause on lets the doses through, the usual one otherwise
     * (a pause that holds them, or none).
     */
    static String dosesChannelNow(Context context)
    {
        SharedPreferences kept = kept(context);
        boolean on = false;
        for (String kind : KINDS) {
            if (!kept.getBoolean("on." + kind, false))
                continue;
            if (!kept.getBoolean("doses." + kind, true))
                return DOSES_USUAL;
            on = true;
        }
        if (!on || Build.VERSION.SDK_INT < Build.VERSION_CODES.Q)
            return DOSES_USUAL;
        return context.getSystemService(NotificationManager.class).getNotificationChannel(DOSES_CHANNEL) != null ? DOSES_CHANNEL : DOSES_USUAL;
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

    private static SharedPreferences kept(Context context)
    {
        return context.getSharedPreferences(KEPT, Context.MODE_PRIVATE);
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
