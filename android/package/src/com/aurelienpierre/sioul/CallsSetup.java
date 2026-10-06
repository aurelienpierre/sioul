// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.app.role.RoleManager;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;

/**
 * Android's question "Set Sioul as your default caller ID & spam app?"
 * (docs/android.md, "Calls"), asked from Settings ▸ Calls: Android answers
 * only an activity that asks for a result, so this one, never seen, asks
 * and goes. Its answer is read again by Sioul when its window comes back.
 */
public final class CallsSetup extends Activity
{
    private static final int ASK = 0x5139;

    @Override
    protected void onCreate(Bundle saved)
    {
        super.onCreate(saved);
        if (saved != null)
            return;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            RoleManager roles = getSystemService(RoleManager.class);
            if (roles != null && roles.isRoleAvailable(RoleManager.ROLE_CALL_SCREENING) && !roles.isRoleHeld(RoleManager.ROLE_CALL_SCREENING)) {
                try {
                    startActivityForResult(roles.createRequestRoleIntent(RoleManager.ROLE_CALL_SCREENING), ASK);
                    return;
                } catch (ActivityNotFoundException | SecurityException e) {
                    Log.w(Calls.TAG, "Calls: Android's question could not be asked: " + e);
                }
            }
        }
        finish();
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data)
    {
        super.onActivityResult(request, result, data);
        if (request == ASK)
            Log.i(Calls.TAG, "Calls: the screening role " + (result == RESULT_OK ? "given." : "not given."));
        finish();
    }
}
