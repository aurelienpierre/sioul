// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;

/**
 * An event's reminder or new mail's notification tapped (EventAlarms,
 * MailNotes): what it opens, the event's file or the Porch, kept for Sioul
 * (EventAlarms.takeOpened, which Rust reads when the window comes up), then
 * Sioul brought up as by its icon. A window of its own, never shown, as
 * DoseOpener: no intent naming a file stays with Sioul's window.
 */
public final class ReminderOpener extends Activity
{
    /** The intent's address: "sioul-open:event", "sioul-open:porch". */
    static final String SCHEME = "sioul-open";
    /** The event's file, on an event's reminder. */
    static final String KEY = "key";

    @Override
    protected void onCreate(Bundle state)
    {
        super.onCreate(state);
        Uri address = getIntent().getData();
        String kind = address != null && SCHEME.equals(address.getScheme()) ? address.getSchemeSpecificPart() : null;
        if (state == null && ("event".equals(kind) || "porch".equals(kind))) {
            EventAlarms.opened(this, kind, getIntent().getStringExtra(KEY));
            Log.i(DoseAlarms.TAG, "Events: a notification tapped, kept for Sioul (" + kind + ").");
        }
        Intent sioul = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (sioul != null)
            startActivity(sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED));
        finish();
    }
}
