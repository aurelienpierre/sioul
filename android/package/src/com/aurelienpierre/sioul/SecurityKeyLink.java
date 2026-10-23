// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Context;
import android.content.Intent;
import android.nfc.NfcAdapter;
import android.nfc.Tag;
import android.nfc.TagLostException;
import android.nfc.tech.IsoDep;
import android.os.Bundle;
import android.provider.Settings;
import android.view.WindowManager;

import java.io.IOException;
import java.util.Arrays;

/**
 * Your security key held against the phone (crates/sioul-app/src/nfc.rs,
 * crates/sioul-sync/src/securitykey/phone.rs, docs/android.md "Your
 * security key"), for Rust through android/main.cpp.
 *
 * <ul>
 * <li><b>Reader mode only while asked.</b> While a band of Sioul's waits for
 * the key and Sioul is in front, NFC is read in reader mode
 * ({@link NfcAdapter#enableReaderMode}: NFC-A and B, no NDEF check, a
 * presence check every 50 ms), ahead of other apps; never otherwise, so that
 * OpenKeychain, Yubico Authenticator and the key's Yubico OTP (its NDEF
 * address) work as before. Android keeps it on the activity across a pause
 * and a return, until Sioul lets it go. No tag filter in the manifest:
 * holding the key never opens Sioul.</li>
 * <li><b>The key held</b> is an {@link IsoDep}, connected at once, five
 * seconds an exchange; its APDUs go through {@link #transceive}, on Rust's
 * thread, never Android's main one. A key that leaves the field throws
 * {@link TagLostException}, which Rust says as a key moved away.</li>
 * <li><b>Let go</b> when Sioul no longer asks: {@link NfcAdapter#ignore}, so
 * that Android hands the key, still in the field, to no other app until it
 * has left it, then reader mode ends.</li>
 * <li><b>The screen kept from captures</b> (FLAG_SECURE) while Sioul asks
 * for the key: its PIN is typed then.</li>
 * <li><b>Nothing logged</b>: never a byte of an APDU, a PIN's among them;
 * each command's array is wiped once sent.</li>
 * </ul>
 */
public final class SecurityKeyLink {
    /** How long one exchange may take: Ed25519 and Curve25519 take far less; the margin is for RSA and slower keys. */
    static final int TIMEOUT_MS = 5000;
    /** How long a key let go must stay away before Android reads it again. */
    static final int AWAY_MS = 1000;

    private static final Object lock = new Object();
    /** The key held now. */
    private static IsoDep held;
    /** Its tag, to let it go. */
    private static Tag heldTag;
    /** The activity reader mode is on for. */
    private static Activity reading;

    private SecurityKeyLink() {}

    /** NFC in this phone: 0 none, 1 switched off, 2 ready. */
    static int state(Context context) {
        NfcAdapter adapter = NfcAdapter.getDefaultAdapter(context);
        if (adapter == null)
            return 0;
        return adapter.isEnabled() ? 2 : 1;
    }

    /** A key wanted (reader mode on, the screen kept from captures), or no longer (the key let go, reader mode off). Any thread. */
    static void want(Context context, boolean wanted) {
        if (!(context instanceof Activity))
            return;
        Activity activity = (Activity) context;
        activity.runOnUiThread(() -> {
            if (wanted)
                start(activity);
            else
                stop(activity);
        });
    }

    private static void start(Activity activity) {
        if (reading == activity)
            return;
        NfcAdapter adapter = NfcAdapter.getDefaultAdapter(activity);
        if (adapter == null)
            return;
        Bundle extras = new Bundle();
        extras.putInt(NfcAdapter.EXTRA_READER_PRESENCE_CHECK_DELAY, 50);
        int flags = NfcAdapter.FLAG_READER_NFC_A | NfcAdapter.FLAG_READER_NFC_B | NfcAdapter.FLAG_READER_SKIP_NDEF_CHECK;
        try {
            adapter.enableReaderMode(activity, SecurityKeyLink::found, flags, extras);
        } catch (IllegalStateException | SecurityException e) {
            return;
        }
        reading = activity;
        activity.getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
    }

    private static void stop(Activity activity) {
        NfcAdapter adapter = NfcAdapter.getDefaultAdapter(activity);
        letGo(adapter);
        if (reading != activity)
            return;
        if (adapter != null) {
            try {
                adapter.disableReaderMode(activity);
            } catch (IllegalStateException e) {
                // The activity went away meanwhile: Android ended reader mode with it.
            }
        }
        reading = null;
        activity.getWindow().clearFlags(WindowManager.LayoutParams.FLAG_SECURE);
    }

    /** The key held let go: ignored by Android until it has left the field, then closed. */
    private static void letGo(NfcAdapter adapter) {
        IsoDep iso;
        Tag tag;
        synchronized (lock) {
            iso = held;
            tag = heldTag;
            held = null;
            heldTag = null;
        }
        if (adapter != null && tag != null) {
            try {
                adapter.ignore(tag, AWAY_MS, null, null);
            } catch (RuntimeException e) {
                // Gone already.
            }
        }
        close(iso);
    }

    private static void close(IsoDep iso) {
        if (iso == null)
            return;
        try {
            iso.close();
        } catch (IOException e) {
            // Gone already.
        }
    }

    /** A tag in reach, on NFC's thread: kept when it speaks ISO-DEP (an OpenPGP card does), Rust told. */
    private static void found(Tag tag) {
        IsoDep iso = IsoDep.get(tag);
        if (iso == null)
            return;
        try {
            iso.connect();
            iso.setTimeout(TIMEOUT_MS);
        } catch (IOException | SecurityException e) {
            return;
        }
        IsoDep before;
        synchronized (lock) {
            before = held;
            held = iso;
            heldTag = tag;
        }
        close(before);
        try {
            nativeHeld();
        } catch (UnsatisfiedLinkError e) {
            // Sioul's library not loaded: no band waits.
        }
    }

    /** The key held now, as {longest exchange, extended length (1) or not (0)}; null when none is. */
    static int[] held() {
        synchronized (lock) {
            if (held == null || !held.isConnected())
                return null;
            return new int[] { held.getMaxTransceiveLength(), held.isExtendedLengthApduSupported() ? 1 : 0 };
        }
    }

    /** One APDU to the key held, on Rust's thread; the command wiped once sent. */
    static byte[] transceive(byte[] command) throws IOException {
        IsoDep iso;
        synchronized (lock) {
            iso = held;
        }
        try {
            if (iso == null)
                throw new TagLostException();
            return iso.transceive(command);
        } finally {
            Arrays.fill(command, (byte) 0);
        }
    }

    /** Android's NFC settings, where it is switched on. */
    static void settings(Context context) {
        Intent intent = new Intent(Settings.ACTION_NFC_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        try {
            context.startActivity(intent);
        } catch (ActivityNotFoundException e) {
            try {
                context.startActivity(new Intent(Settings.ACTION_WIRELESS_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            } catch (ActivityNotFoundException none) {
                // No settings to open: the band's words say what to do.
            }
        }
    }

    /** Rust told that a key came (android/main.cpp). */
    private static native void nativeHeld();
}
