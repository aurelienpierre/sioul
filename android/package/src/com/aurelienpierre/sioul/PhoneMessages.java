// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.Context;
import android.provider.Telephony;

import org.json.JSONException;
import org.json.JSONObject;

/**
 * What the listener says of a notification for your computers, beside what
 * decides its time (AppNotes, describe; crates/sioul-core/src/phonemsgs.rs,
 * `Extra`): whether its app keeps it secret, when it was sent and posted,
 * whether it shows a picture; and of each message, when it was sent and
 * whether it is a picture. Never the picture itself: its address stays on
 * the phone. Rust decides what of it, if anything, leaves the phone, and
 * writes it (docs/android.md, "Messages on your computers").
 */
final class PhoneMessages
{
    private PhoneMessages()
    {
    }

    /** A notification's own: Notification.visibility (-1 secret), Notification.when, its post time, a picture of its own. */
    static JSONObject notice(JSONObject out, int visibility, long when, long posted, boolean picture) throws JSONException
    {
        return out.put("visibility", visibility).put("when", when).put("posted", posted).put("picture", picture);
    }

    /** The phone's default SMS app, for the tab ({@link StepService#call}, "sms-app"); "" when none, or no telephony. */
    static String smsApp(Context context)
    {
        try {
            String found = Telephony.Sms.getDefaultSmsPackage(context);
            return found == null ? "" : found;
        } catch (RuntimeException e) {
            return "";
        }
    }

    /** A message's own: when it was sent, and whether its data is a picture, by its type. */
    static JSONObject message(JSONObject one, long at, String type) throws JSONException
    {
        return one.put("at", at).put("picture", type != null && type.startsWith("image/"));
    }
}
