// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.NotificationManager;

import java.util.HashMap;
import java.util.Map;

/**
 * Both ways' Java, on the JVM (org.json's own jar before android.jar's stubs):
 * - DndHeard.decide: the phone's own do-not-disturb heard as it changes, a
 *   press only when it disagrees with Sioul, never Sioul's own change coming
 *   back, never a late word against what Android has now;
 * - PauseMode.kindOf: Sioul's mode from its condition id;
 * - PauseMode.passesNow: a mode held by the phone's own do-not-disturb keeps
 *   Sioul's channels that pass.
 */
public final class HeardCheck
{
    static int failed = 0;
    static int checked = 0;

    static void expect(String what, Object got, Object wanted)
    {
        checked++;
        if (!String.valueOf(got).equals(String.valueOf(wanted))) {
            failed++;
            System.out.println("FAIL " + what + ": " + got + " (expected " + wanted + ")");
        }
    }

    static Map<String, Object> kept(Object... pairs)
    {
        Map<String, Object> kept = new HashMap<>();
        for (int i = 0; i + 1 < pairs.length; i += 2)
            kept.put((String) pairs[i], pairs[i + 1]);
        return kept;
    }

    public static void main(String[] args)
    {
        final String filter = NotificationManager.ACTION_INTERRUPTION_FILTER_CHANGED;
        final String status = NotificationManager.ACTION_AUTOMATIC_ZEN_RULE_STATUS_CHANGED;
        final int all = NotificationManager.INTERRUPTION_FILTER_ALL;
        final int priority = NotificationManager.INTERRUPTION_FILTER_PRIORITY;
        final int alarms = NotificationManager.INTERRUPTION_FILTER_ALARMS;
        final int unknown = NotificationManager.INTERRUPTION_FILTER_UNKNOWN;
        final long now = 100_000L;

        // Android 12: the tile's "off" while Sioul's mode is on (snoozed with every other): off.
        expect("tile off", DndHeard.decide(true, kept("on.dnd", true, "sioul_on", true, "seen_on", true), filter, 0, null, DndHeard.LIVE_UNKNOWN, all, now), "off");
        // Sioul off; the person's tile on, a schedule, Bedtime or another app: on.
        expect("tile on", DndHeard.decide(true, kept("sioul_on", false, "seen_on", false), filter, 0, null, DndHeard.LIVE_UNKNOWN, priority, now), "on");
        // A nightly schedule's end, which turned the switch on (held, no mode of Sioul's): off.
        expect("schedule ends", DndHeard.decide(true, kept("held.dnd", true, "sioul_on", true, "seen_on", true), filter, 0, null, DndHeard.LIVE_UNKNOWN, all, now), "off");
        // Sioul's own changes coming back agree with Sioul: nothing.
        expect("own on agrees", DndHeard.decide(true, kept("on.dnd", true, "sioul_on", true, "seen_on", false), filter, 0, null, DndHeard.LIVE_UNKNOWN, priority, now), "");
        expect("own off agrees", DndHeard.decide(true, kept("sioul_on", false, "seen_on", true), filter, 0, null, DndHeard.LIVE_UNKNOWN, all, now), "");
        // While Sioul's own change settles: looked at again then.
        String settling = DndHeard.decide(true, kept("changing_until", now + 2_000L, "sioul_on", true, "seen_on", true), filter, 0, null, DndHeard.LIVE_UNKNOWN, all, now);
        expect("settling", settling, DndHeard.LATER + 2_300);
        // On again within the settle after Sioul turned it off: on before, on after; no press, no loop.
        expect("on again within the settle", DndHeard.decide(true, kept("sioul_on", false, "seen_on", true), filter, 0, null, DndHeard.LIVE_UNKNOWN, priority, now), "");
        // First sight, nothing ever noted: found, not heard.
        expect("first sight", DndHeard.decide(true, kept("sioul_on", false), filter, 0, null, DndHeard.LIVE_UNKNOWN, priority, now), "");
        // After a restart or an update the state found is noted (PauseMode.noteSeen): the first change after it is heard
        // (0.0.3 forgot it instead, and lost that change: 8 October 2026, on a phone).
        Map<String, Object> restarted = kept("sioul_on", false);
        restarted.put("seen_on", DndHeard.noted(all));
        expect("first change after an update", DndHeard.decide(true, restarted, filter, 0, null, DndHeard.LIVE_UNKNOWN, priority, now), "on");
        Map<String, Object> restartedOn = kept("on.dnd", true, "sioul_on", true);
        restartedOn.put("seen_on", DndHeard.noted(priority));
        expect("first off after an update", DndHeard.decide(true, restartedOn, filter, 0, null, DndHeard.LIVE_UNKNOWN, all, now), "off");
        expect("nothing noted when Android does not say", DndHeard.noted(unknown), null);
        // Stricter (alarms only): still on, no change of on and off.
        expect("stricter", DndHeard.decide(true, kept("sioul_on", false, "seen_on", true), filter, 0, null, DndHeard.LIVE_UNKNOWN, alarms, now), "");
        // Without the access, or a filter Android does not say: nothing.
        expect("no access", DndHeard.decide(false, kept("on.dnd", true, "sioul_on", true, "seen_on", true), filter, 0, null, DndHeard.LIVE_UNKNOWN, all, now), "");
        expect("unknown filter", DndHeard.decide(true, kept("sioul_on", false, "seen_on", false), filter, 0, null, DndHeard.LIVE_UNKNOWN, unknown, now), "");

        // Android 15 and later: Sioul's mode turned off while another keeps the phone silenced: off.
        expect("deactivated", DndHeard.decide(true, kept("on.dnd", true, "sioul_on", true), status, NotificationManager.AUTOMATIC_RULE_STATUS_DEACTIVATED, "dnd", DndHeard.LIVE_OFF, priority, now), "off");
        // A late word of Sioul's own update, the mode on again by then: nothing.
        expect("late deactivated", DndHeard.decide(true, kept("on.dnd", true, "sioul_on", true), status, NotificationManager.AUTOMATIC_RULE_STATUS_DEACTIVATED, "dnd", DndHeard.LIVE_ON, priority, now), "");
        // Sioul's own leave: it wants it off already.
        expect("own leave", DndHeard.decide(true, kept("on.dnd", false, "sioul_on", false), status, NotificationManager.AUTOMATIC_RULE_STATUS_DEACTIVATED, "dnd", DndHeard.LIVE_OFF, all, now), "");
        // Turned off or deleted in Android's settings while on: off.
        expect("disabled", DndHeard.decide(true, kept("on.pause", true, "sioul_on", true), status, NotificationManager.AUTOMATIC_RULE_STATUS_DISABLED, "pause", DndHeard.LIVE_DISABLED, all, now), "off");
        expect("removed", DndHeard.decide(true, kept("on.pause", true, "sioul_on", true), status, NotificationManager.AUTOMATIC_RULE_STATUS_REMOVED, "pause", DndHeard.LIVE_GONE, all, now), "off");
        expect("removed, there again", DndHeard.decide(true, kept("on.pause", true, "sioul_on", true), status, NotificationManager.AUTOMATIC_RULE_STATUS_REMOVED, "pause", DndHeard.LIVE_ON, priority, now), "");
        // Turned on in Android's settings (a cross-device sync) while Sioul holds nothing: on.
        expect("activated", DndHeard.decide(true, kept("sioul_on", false), status, NotificationManager.AUTOMATIC_RULE_STATUS_ACTIVATED, "dnd", DndHeard.LIVE_ON, priority, now), "on");
        expect("activated by Sioul", DndHeard.decide(true, kept("on.dnd", true, "sioul_on", true), status, NotificationManager.AUTOMATIC_RULE_STATUS_ACTIVATED, "dnd", DndHeard.LIVE_ON, priority, now), "");
        expect("enabled", DndHeard.decide(true, kept("on.dnd", true, "sioul_on", true), status, NotificationManager.AUTOMATIC_RULE_STATUS_ENABLED, "dnd", DndHeard.LIVE_OFF, all, now), "");
        expect("another app's mode", DndHeard.decide(true, kept("on.dnd", true, "sioul_on", true), status, NotificationManager.AUTOMATIC_RULE_STATUS_DEACTIVATED, null, DndHeard.LIVE_UNKNOWN, all, now), "");
        expect("status settling", DndHeard.decide(true, kept("on.dnd", true, "changing_until", now + 100L), status, NotificationManager.AUTOMATIC_RULE_STATUS_DEACTIVATED, "dnd", DndHeard.LIVE_OFF, all, now), DndHeard.LATER + 400);
        expect("seen", DndHeard.seen(priority) + " " + DndHeard.seen(all) + " " + DndHeard.seen(unknown), "true false false");

        // Sioul's mode from its condition id.
        String app = "com.aurelienpierre.sioul";
        expect("kind dnd", PauseMode.kindOf("condition://com.aurelienpierre.sioul/dnd", app), "dnd");
        expect("kind free time", PauseMode.kindOf("condition://com.aurelienpierre.sioul/free-time", app), "free-time");
        expect("another app", PauseMode.kindOf("condition://org.example/dnd", app), null);
        expect("unknown kind", PauseMode.kindOf("condition://com.aurelienpierre.sioul/other", app), null);
        expect("none", PauseMode.kindOf(null, app), null);

        // A mode held by the phone's own do-not-disturb keeps Sioul's channels that pass.
        expect("held passes", PauseMode.passesNow(kept("held.dnd", true, "doses.dnd", true), "doses."), true);
        expect("held holds its doses", PauseMode.passesNow(kept("held.dnd", true, "doses.dnd", false), "doses."), false);
        expect("any held", PauseMode.passesNow(kept("held.dnd", true), null), true);
        expect("nothing on", PauseMode.passesNow(kept(), null), false);

        System.out.println("HeardCheck: " + checked + " checked, " + failed + " failed");
        if (failed > 0)
            System.exit(1);
    }
}
