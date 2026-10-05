// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.system.ErrnoException;
import android.system.Os;
import android.util.Log;
import android.view.accessibility.AccessibilityManager;

/**
 * What is chosen as Sioul's process starts, before anything of Qt's (Android
 * makes an app's providers first): Qt's accessibility is kept for screen
 * readers. Android counts as accessibility any service that reads windows,
 * such as a password manager filling forms (Bitwarden's), and Qt then walks
 * every item of a page at each change of the screen to describe it: seconds
 * of work while a long page scrolls, for nobody. With TalkBack or another
 * screen reader on (touch exploration), Qt's accessibility is as Qt makes
 * it; a screen reader turned on while Sioul runs reads it from its next start.
 */
public final class StartChoices extends ContentProvider
{
    /** Read by Qt's Java side (QtAccessibilityDelegate) at each change of accessibility. */
    private static final String NO_ACCESSIBILITY = "QT_ANDROID_DISABLE_ACCESSIBILITY";

    @Override
    public boolean onCreate()
    {
        final Context context = getContext();
        final AccessibilityManager manager = context == null ? null : (AccessibilityManager) context.getSystemService(Context.ACCESSIBILITY_SERVICE);
        final boolean reader = manager != null && manager.isTouchExplorationEnabled();
        try {
            if (reader)
                Os.unsetenv(NO_ACCESSIBILITY);
            else
                Os.setenv(NO_ACCESSIBILITY, "1", true);
        } catch (ErrnoException e) {
            Log.w(DoseAlarms.TAG, "Accessibility: the choice could not be made: " + e);
        }
        return true;
    }

    @Override
    public Cursor query(Uri uri, String[] projection, String selection, String[] args, String order)
    {
        return null;
    }

    @Override
    public String getType(Uri uri)
    {
        return null;
    }

    @Override
    public Uri insert(Uri uri, ContentValues values)
    {
        return null;
    }

    @Override
    public int delete(Uri uri, String selection, String[] args)
    {
        return 0;
    }

    @Override
    public int update(Uri uri, ContentValues values, String selection, String[] args)
    {
        return 0;
    }
}
