// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * The channels of other apps Sioul makes silent itself, and gives back
 * (docs/android.md, "Notifications from other apps"; option 2 of
 * docs/research/android-holding.md). Android lets a notification listener
 * read and change another app's channels once a companion-device
 * association links its app to one of your devices
 * (NotificationManagerService, verifyPrivilegedListener); the change is
 * recorded as yours. Rust plans which (appnotes::channels_plan): the
 * channels that rang while Sioul held an automaton's notification, of the
 * apps it gathers, and those you named; never a chat's.
 *
 * Each channel's importance is written down by Rust before it changes, so
 * that it can always be given back; a channel silent already (yours), or
 * gone, is left. One given back is set to what it was only while it is still
 * as Sioul left it: changed since, it stays as you set it. Android refusing
 * (the link gone) stops everything, and nothing is forgotten.
 *
 * Pure: Android's side through {@link Channels}, Rust's through
 * {@link Keeper}; checked on a JVM (android/jvm-checks/SilenceCheck.java).
 */
final class ChannelSilence
{
    /** Another app's channels, as the listener reads and changes them. */
    interface Channels
    {
        /** A channel's importance now; {@link #GONE} when its app or it is not there, {@link #DENIED} when Android refuses. */
        int importance(String pkg, String channel);

        /** Its name, as Android gives it; "" when none. */
        String name(String pkg, String channel);

        /** Its importance set; false when Android refused. */
        boolean set(String pkg, String channel, int importance);
    }

    /** Rust's record of what Sioul made silent (appnotes::Silenced). */
    interface Keeper
    {
        /** Its importance written down before Sioul changes it; false: not written, nothing changes. */
        boolean keep(String pkg, String channel, String name, int was);

        /** Done with: given back, gone, never changed, or left as you set it. */
        void forget(String pkg, String channel);
    }

    /** The app or the channel is not there (uninstalled, deleted by its app). */
    static final int GONE = Integer.MIN_VALUE;
    /** Android refuses to read it: no link to a device (any more). */
    static final int DENIED = Integer.MIN_VALUE + 1;
    /** Android's IMPORTANCE_LOW: in the shade, without a sound or a banner. */
    static final int LOW = 2;

    private ChannelSilence() {}

    /**
     * Rust's plan applied: {silence: [{package, channel}], restore:
     * [{package, channel, importance}]}. How many were made silent, and how
     * many given back: {silenced, restored}.
     */
    static int[] apply(JSONObject plan, Channels channels, Keeper keeper)
    {
        int silenced = 0;
        int restored = 0;
        JSONArray silence = plan == null ? null : plan.optJSONArray("silence");
        for (int i = 0; silence != null && i < silence.length(); i++) {
            JSONObject one = silence.optJSONObject(i);
            String pkg = one == null ? "" : one.optString("package", "");
            String channel = one == null ? "" : one.optString("channel", "");
            if (pkg.isEmpty() || channel.isEmpty())
                continue;
            int was = channels.importance(pkg, channel);
            if (was == DENIED)
                return new int[] { silenced, restored };
            // Gone, or silent already (as you set it): nothing to do, nothing written down.
            if (was == GONE || was <= LOW)
                continue;
            if (!keeper.keep(pkg, channel, channels.name(pkg, channel), was))
                continue;
            if (channels.set(pkg, channel, LOW))
                silenced++;
            else
                keeper.forget(pkg, channel);
        }
        JSONArray restore = plan == null ? null : plan.optJSONArray("restore");
        for (int i = 0; restore != null && i < restore.length(); i++) {
            JSONObject one = restore.optJSONObject(i);
            String pkg = one == null ? "" : one.optString("package", "");
            String channel = one == null ? "" : one.optString("channel", "");
            int was = one == null ? 0 : one.optInt("importance", 0);
            if (pkg.isEmpty() || channel.isEmpty())
                continue;
            int now = channels.importance(pkg, channel);
            if (now == DENIED)
                return new int[] { silenced, restored };
            if (now == LOW && was > LOW) {
                // As Sioul left it: given back; Android refusing, kept to try again.
                if (!channels.set(pkg, channel, was))
                    continue;
                restored++;
            }
            // Given back, gone, or changed since (left as you set it): done with.
            keeper.forget(pkg, channel);
        }
        return new int[] { silenced, restored };
    }
}
