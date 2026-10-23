// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.app.PendingIntent;
import android.content.ActivityNotFoundException;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageManager;
import android.hardware.usb.UsbConstants;
import android.hardware.usb.UsbDevice;
import android.hardware.usb.UsbDeviceConnection;
import android.hardware.usb.UsbEndpoint;
import android.hardware.usb.UsbInterface;
import android.hardware.usb.UsbManager;
import android.nfc.NfcAdapter;
import android.nfc.Tag;
import android.nfc.TagLostException;
import android.nfc.tech.IsoDep;
import android.os.Build;
import android.os.Bundle;
import android.provider.Settings;
import android.view.WindowManager;

import java.io.IOException;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

/**
 * Your security key held against the phone or plugged into its USB-C port
 * (crates/sioul-app/src/nfc.rs, crates/sioul-sync/src/securitykey/phone.rs,
 * docs/android.md "Your security key"), for Rust through android/main.cpp.
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
 * <li><b>Plugged in</b>: while a band asks for the key, a key plugged in
 * (before or after) is looked for among the USB devices, by its smart card
 * interface (class 0x0B) alone: never its keyboard (Yubico OTP types there,
 * untouched) nor its FIDO interface. Android's permission is asked once for
 * each time it is plugged in: refused, Sioul says so and asks again only
 * once it has been unplugged. Its interface claimed, the APDUs go as CCID
 * messages ({@link Ccid}), on Rust's thread; a key unplugged meanwhile is a
 * key gone, as over NFC ({@link TagLostException}). Claimed by another app
 * (Yubico Authenticator, OpenKeychain), it is said so. When Sioul no longer
 * asks, the key is powered off, its interface released and its connection
 * closed: other apps have it again. No USB filter in the manifest: plugging
 * the key never opens Sioul.</li>
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

    /** The permission's answer, for this package's receiver alone. */
    static final String PERMISSION = "com.aurelienpierre.sioul.SECURITY_KEY_USB";
    /** A key plugged in, open: its connection, its interface, its messages. */
    private static UsbDevice plugged;
    private static UsbDeviceConnection connection;
    private static UsbInterface claimed;
    private static Ccid ccid;
    private static boolean powered;
    /** Unplugged while open: what is sent now fails as a key gone. */
    private static boolean unplugged;
    /** Keys plugged in that Android was asked for (by device name), until unplugged. */
    private static final Set<String> asked = new HashSet<>();
    /** Those Android refused, until unplugged. */
    private static final Set<String> refused = new HashSet<>();
    /** One that another app holds, until unplugged or asked again. */
    private static String busy;
    /** Android's USB service, once a key was wanted: what is plugged in now. */
    private static UsbManager usb;
    /** The receiver of plugging, unplugging and Android's answer, while a key is wanted. */
    private static BroadcastReceiver usbReceiver;

    private SecurityKeyLink() {}

    /** Whether this phone takes a key plugged into its USB-C port (USB host). */
    static boolean usbHost(Context context) {
        return context.getPackageManager().hasSystemFeature(PackageManager.FEATURE_USB_HOST);
    }

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
        listen(activity);
        NfcAdapter adapter = NfcAdapter.getDefaultAdapter(activity);
        if (adapter == null) {
            activity.getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
            reading = activity;
            return;
        }
        Bundle extras = new Bundle();
        extras.putInt(NfcAdapter.EXTRA_READER_PRESENCE_CHECK_DELAY, 50);
        int flags = NfcAdapter.FLAG_READER_NFC_A | NfcAdapter.FLAG_READER_NFC_B | NfcAdapter.FLAG_READER_SKIP_NDEF_CHECK;
        try {
            adapter.enableReaderMode(activity, SecurityKeyLink::found, flags, extras);
        } catch (IllegalStateException | SecurityException e) {
            // NFC refused: a key plugged in still comes.
        }
        reading = activity;
        activity.getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
    }

    private static void stop(Activity activity) {
        NfcAdapter adapter = NfcAdapter.getDefaultAdapter(activity);
        letGo(adapter);
        unplug(activity);
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
        told();
    }

    /** The longest APDU sent to a key plugged in: short APDUs, chained by Rust, and their header. */
    static final int PLUGGED_LONGEST = 271;

    /**
     * The key in reach now, as {what (0 none, 1 a key, 2 refused by Android, 3
     * held by another app), longest exchange, extended length (1) or not (0),
     * plugged in (1) or held (0)}: a key plugged in first, else one held.
     */
    static int[] held() {
        synchronized (lock) {
            if (ccid != null && !unplugged)
                return new int[] { 1, PLUGGED_LONGEST, 0, 1 };
            if (held != null && held.isConnected())
                return new int[] { 1, held.getMaxTransceiveLength(), held.isExtendedLengthApduSupported() ? 1 : 0, 0 };
            // A key refused, then unplugged while Sioul did not listen: no longer refused.
            if (!refused.isEmpty() && usb != null)
                refused.retainAll(usb.getDeviceList().keySet());
            if (!refused.isEmpty())
                return new int[] { 2, 0, 0, 1 };
            if (busy != null)
                return new int[] { 3, 0, 0, 1 };
            return new int[] { 0, 0, 0, 0 };
        }
    }

    /** One APDU to the key in reach, on Rust's thread; the command wiped once sent. */
    static byte[] transceive(byte[] command) throws IOException {
        IsoDep iso;
        Ccid messages;
        synchronized (lock) {
            iso = held;
            messages = unplugged ? null : ccid;
        }
        try {
            if (messages != null)
                return plugged(messages, command);
            if (iso == null)
                throw new TagLostException();
            return iso.transceive(command);
        } finally {
            Arrays.fill(command, (byte) 0);
        }
    }

    /** One APDU to the key plugged in, powered on first; unplugged meanwhile, a key gone. */
    private static byte[] plugged(Ccid messages, byte[] command) throws IOException {
        // One message at a time: Rust's exchanges, the power off at the end.
        synchronized (messages) {
            return exchange(messages, command);
        }
    }

    private static byte[] exchange(Ccid messages, byte[] command) throws IOException {
        try {
            boolean on;
            synchronized (lock) {
                on = powered;
            }
            if (!on) {
                Arrays.fill(messages.powerOn(), (byte) 0);
                synchronized (lock) {
                    powered = true;
                }
            }
            return messages.transmit(command);
        } catch (IOException e) {
            // Unplugged: heard (closed meanwhile), or not yet heard but no longer there.
            synchronized (lock) {
                if (unplugged || ccid != messages || plugged == null || usb == null || !usb.getDeviceList().containsKey(plugged.getDeviceName()))
                    throw new TagLostException();
            }
            throw e;
        }
    }

    /** Plugging, unplugging and Android's answer heard while a key is wanted; a key plugged in already looked for. */
    private static void listen(Activity activity) {
        UsbManager manager = (UsbManager) activity.getSystemService(Context.USB_SERVICE);
        if (manager == null || !usbHost(activity))
            return;
        usb = manager;
        if (usbReceiver == null) {
            usbReceiver = new BroadcastReceiver() {
                @Override
                public void onReceive(Context context, Intent intent) {
                    heard(activity, manager, intent);
                }
            };
            IntentFilter filter = new IntentFilter();
            filter.addAction(UsbManager.ACTION_USB_DEVICE_ATTACHED);
            filter.addAction(UsbManager.ACTION_USB_DEVICE_DETACHED);
            filter.addAction(PERMISSION);
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU)
                activity.registerReceiver(usbReceiver, filter, Context.RECEIVER_NOT_EXPORTED);
            else
                activity.registerReceiver(usbReceiver, filter);
        }
        // What was refused or asked for a key no longer plugged in is forgotten: plugged in again, it is asked again.
        Set<String> there = manager.getDeviceList().keySet();
        synchronized (lock) {
            asked.retainAll(there);
            refused.retainAll(there);
            if (busy != null && !there.contains(busy))
                busy = null;
        }
        for (UsbDevice device : manager.getDeviceList().values())
            consider(activity, manager, device);
    }

    private static void heard(Activity activity, UsbManager manager, Intent intent) {
        UsbDevice device = Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU ? intent.getParcelableExtra(UsbManager.EXTRA_DEVICE, UsbDevice.class) : deviceOf(intent);
        if (device == null)
            return;
        String action = intent.getAction();
        if (UsbManager.ACTION_USB_DEVICE_DETACHED.equals(action)) {
            boolean ours;
            synchronized (lock) {
                asked.remove(device.getDeviceName());
                refused.remove(device.getDeviceName());
                if (device.getDeviceName().equals(busy))
                    busy = null;
                ours = plugged != null && plugged.getDeviceName().equals(device.getDeviceName());
                if (ours)
                    unplugged = true;
            }
            if (ours)
                close();
            told();
        } else if (UsbManager.ACTION_USB_DEVICE_ATTACHED.equals(action)) {
            consider(activity, manager, device);
        } else if (PERMISSION.equals(action)) {
            if (intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)) {
                open(manager, device);
            } else {
                synchronized (lock) {
                    refused.add(device.getDeviceName());
                }
                told();
            }
        }
    }

    /** The device an intent names, as Android before 13 gives it. */
    @SuppressWarnings("deprecation")
    private static UsbDevice deviceOf(Intent intent) {
        return intent.getParcelableExtra(UsbManager.EXTRA_DEVICE);
    }

    /** A USB device: a security key when it has a smart card interface; opened, or Android asked once. */
    private static void consider(Activity activity, UsbManager manager, UsbDevice device) {
        if (cardInterface(device) == null)
            return;
        synchronized (lock) {
            if (ccid != null || refused.contains(device.getDeviceName()))
                return;
        }
        if (manager.hasPermission(device)) {
            open(manager, device);
            return;
        }
        synchronized (lock) {
            if (!asked.add(device.getDeviceName()))
                return;
        }
        Intent answer = new Intent(PERMISSION).setPackage(activity.getPackageName());
        int flags = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ? PendingIntent.FLAG_MUTABLE : 0;
        manager.requestPermission(device, PendingIntent.getBroadcast(activity, 0, answer, flags));
    }

    /** The device's smart card interface (CCID, class 0x0B), with its two bulk endpoints; null when it has none. */
    static UsbInterface cardInterface(UsbDevice device) {
        for (int i = 0; i < device.getInterfaceCount(); i++) {
            UsbInterface candidate = device.getInterface(i);
            if (candidate.getInterfaceClass() == UsbConstants.USB_CLASS_CSCID && endpoint(candidate, UsbConstants.USB_DIR_IN) != null && endpoint(candidate, UsbConstants.USB_DIR_OUT) != null)
                return candidate;
        }
        return null;
    }

    private static UsbEndpoint endpoint(UsbInterface face, int direction) {
        for (int i = 0; i < face.getEndpointCount(); i++) {
            UsbEndpoint point = face.getEndpoint(i);
            if (point.getType() == UsbConstants.USB_ENDPOINT_XFER_BULK && point.getDirection() == direction)
                return point;
        }
        return null;
    }

    /** The key plugged in, its smart card interface claimed alone; another app holding it, said so. */
    private static void open(UsbManager manager, UsbDevice device) {
        UsbInterface face = cardInterface(device);
        if (face == null)
            return;
        UsbDeviceConnection opened = manager.openDevice(device);
        if (opened == null || !opened.claimInterface(face, true)) {
            if (opened != null)
                opened.close();
            synchronized (lock) {
                busy = device.getDeviceName();
            }
            told();
            return;
        }
        UsbEndpoint in = endpoint(face, UsbConstants.USB_DIR_IN);
        UsbEndpoint out = endpoint(face, UsbConstants.USB_DIR_OUT);
        Ccid messages = new Ccid(new Ccid.Pipe() {
            @Override
            public int send(byte[] data, int offset, int length, int timeoutMs) {
                return opened.bulkTransfer(out, data, offset, length, timeoutMs);
            }

            @Override
            public int receive(byte[] into, int timeoutMs) {
                return opened.bulkTransfer(in, into, into.length, timeoutMs);
            }
        }, out.getMaxPacketSize());
        synchronized (lock) {
            plugged = device;
            connection = opened;
            claimed = face;
            ccid = messages;
            powered = false;
            unplugged = false;
            busy = null;
        }
        told();
    }

    /** No longer asked: the key plugged in powered off, its interface given back, and Android's receiver let go. */
    private static void unplug(Activity activity) {
        Ccid messages;
        synchronized (lock) {
            messages = powered && !unplugged ? ccid : null;
            // Held by another app: looked at again the next time a key is wanted.
            busy = null;
        }
        if (messages != null) {
            // On Android's main thread: the power off is one short message; a key gone fails at once.
            synchronized (messages) {
                try {
                    messages.powerOff();
                } catch (IOException e) {
                    // Gone already.
                }
            }
        }
        close();
        if (usbReceiver != null) {
            try {
                activity.unregisterReceiver(usbReceiver);
            } catch (IllegalArgumentException e) {
                // Not registered.
            }
            usbReceiver = null;
        }
    }

    private static void close() {
        UsbDeviceConnection opened;
        UsbInterface face;
        synchronized (lock) {
            opened = connection;
            face = claimed;
            connection = null;
            claimed = null;
            ccid = null;
            plugged = null;
            powered = false;
        }
        if (opened != null) {
            if (face != null)
                opened.releaseInterface(face);
            opened.close();
        }
    }

    /** Rust told that something changed: a key came, went, or was refused. */
    private static void told() {
        try {
            nativeHeld();
        } catch (UnsatisfiedLinkError e) {
            // Sioul's library not loaded: no band waits.
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
