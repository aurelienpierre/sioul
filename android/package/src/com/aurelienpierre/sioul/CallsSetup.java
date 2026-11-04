// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.Manifest;
import android.app.Activity;
import android.app.role.RoleManager;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;

/**
 * Android's questions about calls, asked from Settings ▸ Calls
 * (docs/android.md, "Calls"): "Set Sioul as your default caller ID &amp;
 * spam app?", or, asked with {@link #ASKED} {@link #HISTORY}, whether Sioul
 * may read the phone's call history (READ_CALL_LOG). Android answers only an
 * activity that asks for a result, so this one, never seen, asks and goes.
 * Its answer is read again by Sioul when its window comes back.
 */
public final class CallsSetup extends Activity
{
    /** The extra naming what is asked; without it, the role. */
    static final String ASKED = "asked";
    /** Asked: reading the call history. */
    static final String HISTORY = "history";

    private static final int ASK = 0x5139;
    private static final int ASK_HISTORY = 0x513a;

    @Override
    protected void onCreate(Bundle saved)
    {
        super.onCreate(saved);
        if (saved != null)
            return;
        if (HISTORY.equals(getIntent().getStringExtra(ASKED))) {
            if (checkSelfPermission(Manifest.permission.READ_CALL_LOG) != PackageManager.PERMISSION_GRANTED) {
                requestPermissions(new String[] { Manifest.permission.READ_CALL_LOG }, ASK_HISTORY);
                return;
            }
            finish();
            return;
        }
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

    @Override
    public void onRequestPermissionsResult(int request, String[] permissions, int[] results)
    {
        super.onRequestPermissionsResult(request, permissions, results);
        if (request == ASK_HISTORY)
            Log.i(Calls.TAG, "Calls: the call history " + (results.length > 0 && results[0] == PackageManager.PERMISSION_GRANTED ? "may be read." : "is not read."));
        finish();
    }
}
