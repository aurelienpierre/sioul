// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.NotificationManager;

import java.util.Map;

/**
 * What a change of the phone's own do-not-disturb means for Sioul's switch
 * (docs/do-not-disturb.md, "Both ways"; DndReceiver), decided in Java so that
 * most changes end without loading Sioul's library. Pure, checked on a JVM
 * (android/jvm-checks/HeardCheck.java).
 *
 * - The phone's do-not-disturb is one fact: on (the interruption filter is
 *   not ALL) or off. A change of it from the state last seen, read once
 *   Sioul's own change settled, that disagrees with Sioul's do-not-disturb
 *   here is a press; one that agrees is Sioul's own coming back, or nothing.
 *   Rust has the last word (`everywhere::heard_from_system`).
 * - Sioul's own modes say their own state too
 *   (ACTION_AUTOMATIC_ZEN_RULE_STATUS_CHANGED): one turned off while Sioul
 *   wants it on is an "off", even while another mode keeps the phone
 *   silenced; one turned on while Sioul wants none, an "on". Each is
 *   confirmed against the mode as Android has it now (`live`): a status that
 *   came late, after Sioul's own change, is no press.
 */
final class DndHeard
{
    /** Look again once Sioul's own change settled: this, then the milliseconds to wait. */
    static final String LATER = "later:";

    /** A mode of Sioul's as Android has it now (PauseMode.live). */
    static final int LIVE_UNKNOWN = 0;
    static final int LIVE_ON = 1;
    static final int LIVE_OFF = 2;
    static final int LIVE_DISABLED = 3;
    static final int LIVE_GONE = 4;

    private DndHeard() {}

    /**
     * `kept`: PauseMode's record ({@code on.<kind>}, `sioul_on`, `changing_until`,
     * `seen_on`); `action`: the broadcast's; `status`, `kind` and `live`: the
     * mode's, for a status; `filter`: the interruption filter now; `now`:
     * SystemClock.elapsedRealtime(). Answers "on", "off", LATER and a delay,
     * or "" (nothing for Sioul). The caller notes the filter seen (`seen`).
     */
    static String decide(boolean access, Map<String, ?> kept, String action, int status, String kind, int live, int filter, long now)
    {
        if (!access)
            return "";
        long until = number(kept.get("changing_until"));
        if (now < until)
            return LATER + (until - now + 300);
        boolean sioul = Boolean.TRUE.equals(kept.get("sioul_on"));
        if (NotificationManager.ACTION_AUTOMATIC_ZEN_RULE_STATUS_CHANGED.equals(action)) {
            if (kind == null)
                return "";
            boolean wanted = Boolean.TRUE.equals(kept.get("on." + kind));
            switch (status) {
            case NotificationManager.AUTOMATIC_RULE_STATUS_DEACTIVATED:
                return wanted && (live == LIVE_OFF || live == LIVE_DISABLED || live == LIVE_GONE) ? "off" : "";
            case NotificationManager.AUTOMATIC_RULE_STATUS_DISABLED:
                return wanted && (live == LIVE_DISABLED || live == LIVE_GONE) ? "off" : "";
            case NotificationManager.AUTOMATIC_RULE_STATUS_REMOVED:
                return wanted && live == LIVE_GONE ? "off" : "";
            case NotificationManager.AUTOMATIC_RULE_STATUS_ACTIVATED:
                return !wanted && !sioul && live == LIVE_ON ? "on" : "";
            default:
                return "";
            }
        }
        // The filter: any change of the phone's do-not-disturb, whatever caused it.
        if (filter == NotificationManager.INTERRUPTION_FILTER_UNKNOWN)
            return "";
        boolean on = seen(filter);
        Object before = kept.get("seen_on");
        // First sight (found, not heard), or no change of on and off.
        if (!(before instanceof Boolean) || (Boolean) before == on)
            return "";
        return on == sioul ? "" : on ? "on" : "off";
    }

    /** The phone's do-not-disturb on for this filter. */
    static boolean seen(int filter)
    {
        return filter != NotificationManager.INTERRUPTION_FILTER_ALL && filter != NotificationManager.INTERRUPTION_FILTER_UNKNOWN;
    }

    private static long number(Object value)
    {
        return value instanceof Number ? ((Number) value).longValue() : 0;
    }
}
