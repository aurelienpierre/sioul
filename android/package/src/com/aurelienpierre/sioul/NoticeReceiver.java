// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.ClipData;
import android.content.ClipDescription;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.PersistableBundle;
import android.util.Log;

/**
 * The presses on Sioul's notices that open nothing (Notices): Copy, a code
 * put on the clipboard, marked sensitive (Android shows no preview of it),
 * its notice then taken away, its work done; and a notice swiped away, its
 * group's summary following it (Android 16). Sent by the notices alone, by
 * no other app. Nothing it carries is ever logged.
 */
public final class NoticeReceiver extends BroadcastReceiver
{
    static final String COPY = "com.aurelienpierre.sioul.action.NOTICE_COPY";
    static final String DELETED = "com.aurelienpierre.sioul.action.NOTICE_DELETED";
    private static final String SCHEME = "sioul-notice";
    private static final String TEXT = "text";

    @Override
    public void onReceive(Context context, Intent intent)
    {
        String action = intent == null ? null : intent.getAction();
        Uri data = intent == null ? null : intent.getData();
        String tag = data != null && SCHEME.equals(data.getScheme()) ? data.getSchemeSpecificPart() : "";
        if (COPY.equals(action)) {
            String text = intent.getStringExtra(TEXT);
            if (text != null && !text.isEmpty()) {
                ClipData clip = ClipData.newPlainText("Sioul", text);
                PersistableBundle extras = new PersistableBundle();
                extras.putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true);
                clip.getDescription().setExtras(extras);
                context.getSystemService(ClipboardManager.class).setPrimaryClip(clip);
                Log.i(DoseAlarms.TAG, "Notices: a code copied.");
            }
            Notices.dismiss(context, tag);
            return;
        }
        if (DELETED.equals(action))
            Notices.tidy(context);
    }

    /** Copy on the notice `tag`: `text` on the clipboard, then the notice taken away. */
    static PendingIntent copying(Context context, String tag, String text)
    {
        Intent press = new Intent(context, NoticeReceiver.class).setAction(COPY).setData(Uri.fromParts(SCHEME, tag, null)).putExtra(TEXT, text);
        return PendingIntent.getBroadcast(context, Notices.requestCode(tag, "copy"), press, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /** The notice `tag` swiped away, or cleared: its group's summary looked at again. */
    static PendingIntent deleted(Context context, String tag)
    {
        Intent gone = new Intent(context, NoticeReceiver.class).setAction(DELETED).setData(Uri.fromParts(SCHEME, tag, null));
        return PendingIntent.getBroadcast(context, Notices.requestCode(tag, "deleted"), gone, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }
}
