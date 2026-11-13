// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.companion.AssociationRequest;
import android.companion.BluetoothDeviceFilter;
import android.companion.CompanionDeviceManager;
import android.content.Intent;
import android.content.IntentSender;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;

/**
 * Android's question for the link to your computer (docs/android.md,
 * "Notifications from other apps"; option 2 of
 * docs/research/android-holding.md), asked from Settings ▸ This phone ("Let
 * Sioul make them silent"): Android's own list of devices, the paired
 * Bluetooth ones first, where you choose the computer Sioul relays this
 * phone's notifications to. The association asks for no device profile and
 * no permission; it lets Sioul's listener make the apps it gathers silent
 * itself (AppNotes, ChannelSilence). Android answers only an activity that
 * asks for a result, so this one, never seen, asks and goes; on a yes, the
 * listener is told at once ({@link AppNotes#LINKED}).
 */
public final class Companion extends Activity
{
    private static final int ASK = 0x5143;

    @Override
    protected void onCreate(Bundle saved)
    {
        super.onCreate(saved);
        if (saved != null)
            return;
        CompanionDeviceManager devices = getSystemService(CompanionDeviceManager.class);
        if (devices == null) {
            Log.i(AppNotes.TAG, "Notes: this phone offers no link to a device.");
            finish();
            return;
        }
        AssociationRequest request = new AssociationRequest.Builder()
            .addDeviceFilter(new BluetoothDeviceFilter.Builder().build())
            .build();
        CompanionDeviceManager.Callback answer = new CompanionDeviceManager.Callback() {
            /** Before Android 13: Android's list, to show. */
            @Override
            @SuppressWarnings("deprecation")
            public void onDeviceFound(IntentSender chooser)
            {
                ask(chooser);
            }

            /** Android 13 and later: the same list. */
            @Override
            public void onAssociationPending(IntentSender chooser)
            {
                ask(chooser);
            }

            @Override
            public void onFailure(CharSequence error)
            {
                Log.w(AppNotes.TAG, "Notes: the link to a device was not asked: " + error);
                finish();
            }
        };
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU)
                devices.associate(request, getMainExecutor(), answer);
            else
                associateBefore13(devices, request, answer);
        } catch (RuntimeException e) {
            Log.w(AppNotes.TAG, "Notes: the link to a device was not asked: " + e.getClass().getSimpleName());
            finish();
        }
    }

    /** Before Android 13, the callback on this activity's own thread. */
    @SuppressWarnings("deprecation")
    private static void associateBefore13(CompanionDeviceManager devices, AssociationRequest request, CompanionDeviceManager.Callback answer)
    {
        devices.associate(request, answer, null);
    }

    /** Android's list shown; its answer comes to {@link #onActivityResult}. */
    private void ask(IntentSender chooser)
    {
        try {
            startIntentSenderForResult(chooser, ASK, null, 0, 0, 0);
        } catch (IntentSender.SendIntentException | RuntimeException e) {
            Log.w(AppNotes.TAG, "Notes: Android's list of devices could not be shown: " + e.getClass().getSimpleName());
            finish();
        }
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data)
    {
        super.onActivityResult(request, result, data);
        if (request == ASK) {
            Log.i(AppNotes.TAG, "Notes: the link to a device " + (result == RESULT_OK ? "made." : "not made."));
            if (result == RESULT_OK)
                sendBroadcast(new Intent(this, AppNotes.Asked.class).setAction(AppNotes.LINKED));
        }
        finish();
    }
}
