// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import org.json.JSONObject;

/**
 * PhoneMessages on the JVM, as Rust reads it (crates/sioul-core/src/phonemsgs.rs,
 * `Extra`, `ExtraMessage`): a notification's visibility, its times and its
 * picture under the names Rust reads; a message's time; a picture known by
 * its type, never an audio clip or a message without data.
 */
public final class MessagesCheck
{
    static int checked = 0;
    static int failed = 0;

    static void expect(String what, boolean ok)
    {
        checked++;
        if (!ok) {
            failed++;
            System.out.println("FAIL " + what);
        }
    }

    public static void main(String[] args) throws Exception
    {
        // Thursday 8 October 2026, 09:59 in Paris.
        long at = 1791446340000L;
        JSONObject secret = PhoneMessages.notice(new JSONObject().put("package", "foundation.e.message"), -1, at, at + 1000, false);
        expect("visibility", secret.getInt("visibility") == -1);
        expect("when", secret.getLong("when") == at);
        expect("posted", secret.getLong("posted") == at + 1000);
        expect("picture", !secret.getBoolean("picture"));
        expect("the rest kept", "foundation.e.message".equals(secret.getString("package")));
        JSONObject pictured = PhoneMessages.notice(new JSONObject(), 0, 0, at, true);
        expect("a picture of its own", pictured.getBoolean("picture") && pictured.getInt("visibility") == 0);

        JSONObject photo = PhoneMessages.message(new JSONObject().put("text", ""), at, "image/jpeg");
        expect("a message's time", photo.getLong("at") == at);
        expect("a picture by its type", photo.getBoolean("picture"));
        expect("its text kept", "".equals(photo.getString("text")));
        expect("a voice note is no picture", !PhoneMessages.message(new JSONObject(), at, "audio/ogg").getBoolean("picture"));
        expect("no data, no picture", !PhoneMessages.message(new JSONObject(), at, null).getBoolean("picture"));
        expect("no time said", PhoneMessages.message(new JSONObject(), 0, null).getLong("at") == 0);

        System.out.println("MessagesCheck: " + checked + " checked, " + failed + " failed");
        if (failed > 0)
            System.exit(1);
    }
}
