// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.net.ConnectivityManager;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.os.BatteryManager;
import android.os.PowerManager;
import android.util.Log;

import org.json.JSONException;
import org.json.JSONObject;

/**
 * The spam filter's and the topics' training by itself on this phone
 * (docs/spam-filter.md, "Every device trains its own"; Rust's
 * {@code spam::background_step}, in the background service's process):
 * <ul>
 * <li>{@code learn-power}: what the phone says of itself, for Rust's rule
 * ({@code sioul_learn::auto::decide}): on its charger, saving power, its
 * screen on (in use), its network metered (Android's network capabilities;
 * none said without a network), its battery's temperature in tenths of a
 * degree Celsius. Read from Android's last word on the battery (a sticky
 * broadcast: no receiver kept), nothing asked of the network.</li>
 * <li>{@code learn-awake}: the phone kept awake while a training runs (a
 * partial wake lock of its own, {@code sioul:learn}, released when it ends,
 * and by Android after {@link #LONGEST} at most, beyond the guard's 30
 * minutes and the download).</li>
 * </ul>
 */
final class Learn
{
    private static final String TAG = "sioul";
    /** The wake lock's own limit: the guard stops a training at 30 minutes; the download comes before it. */
    static final long LONGEST = 90 * 60 * 1000L;

    private static PowerManager.WakeLock awake;

    private Learn()
    {
    }

    static String call(Context context, String verb, JSONObject asked) throws JSONException
    {
        switch (verb) {
        case "learn-power":
            return power(context).toString();
        case "learn-awake":
            awake(context, asked.optBoolean("on", false));
            return "true";
        default:
            return null;
        }
    }

    /** {plugged, saver, interactive, metered (absent: unknown), temperature (absent: unsaid)}. */
    static JSONObject power(Context context) throws JSONException
    {
        JSONObject out = new JSONObject();
        try {
            Intent battery = context.registerReceiver(null, new IntentFilter(Intent.ACTION_BATTERY_CHANGED));
            if (battery != null) {
                out.put("plugged", battery.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) != 0);
                int tenths = battery.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, Integer.MIN_VALUE);
                if (tenths != Integer.MIN_VALUE)
                    out.put("temperature", tenths);
            }
        } catch (RuntimeException e) {
            Log.w(TAG, "Learn: battery: " + e);
        }
        PowerManager power = context.getSystemService(PowerManager.class);
        if (power != null) {
            out.put("saver", power.isPowerSaveMode());
            out.put("interactive", power.isInteractive());
        }
        Boolean metered = metered(context);
        if (metered != null)
            out.put("metered", metered.booleanValue());
        return out;
    }

    /** The active network metered, as its capabilities say (not NOT_METERED); none without a network. */
    static Boolean metered(Context context)
    {
        try {
            ConnectivityManager connectivity = context.getSystemService(ConnectivityManager.class);
            if (connectivity == null)
                return null;
            Network network = connectivity.getActiveNetwork();
            NetworkCapabilities capabilities = network == null ? null : connectivity.getNetworkCapabilities(network);
            if (capabilities == null)
                return null;
            return !capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED);
        } catch (RuntimeException e) {
            Log.w(TAG, "Learn: network: " + e);
            return null;
        }
    }

    /** The phone kept awake while a training runs, or let sleep. */
    static synchronized void awake(Context context, boolean on)
    {
        if (on) {
            if (awake == null) {
                PowerManager power = context.getSystemService(PowerManager.class);
                if (power == null)
                    return;
                awake = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "sioul:learn");
                awake.setReferenceCounted(false);
            }
            awake.acquire(LONGEST);
        } else if (awake != null && awake.isHeld()) {
            awake.release();
        }
    }
}
