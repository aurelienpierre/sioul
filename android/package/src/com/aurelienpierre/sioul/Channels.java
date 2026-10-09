// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.NotificationChannel;

/**
 * Sioul's notification channels, every one made here: without a badge on
 * Sioul's icon, neither the launcher's dot nor a count (docs/design.md: no
 * badges, no counts). Android keeps a channel's badge as the channel was
 * first made, and takes no change of it afterwards: a channel an older Sioul
 * made with its badge keeps it until you switch it off in Android's settings
 * (Sioul, Notifications, its category).
 * Checked on a JVM (android/jvm-checks/ChannelsCheck.java): no other place
 * makes a channel.
 */
final class Channels
{
    private Channels() {}

    /**
     * A channel to hand to NotificationManager.createNotificationChannel,
     * its badge off; the rest (sound, vibration, do-not-disturb) set by the caller.
     *
     * @param id the channel's id, kept for good
     * @param name its name, as Android's settings show it
     * @param importance NotificationManager's importance
     * @return the channel, not yet made
     */
    static NotificationChannel quiet(String id, CharSequence name, int importance)
    {
        NotificationChannel channel = new NotificationChannel(id, name, importance);
        channel.setShowBadge(false);
        return channel;
    }
}
