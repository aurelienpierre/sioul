// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Your security key held against the phone, on the JVM
 * (android/jvm-checks/run.sh), read from the sources (the repository's root
 * comes as the property sioul.root): android.jar's NfcAdapter and IsoDep are
 * stubs that cannot be made here. Other apps keep the key: no tag filter in
 * the manifest (holding the key never opens Sioul, so OpenKeychain, Yubico
 * Authenticator and Yubico OTP's tap work as before), reader mode only in
 * SecurityKeyLink, on while a band asks and ended after, the key let go to
 * no other app; the screen kept from captures meanwhile; nothing of an
 * APDU logged, in Java or in the bridge, each command wiped. Plugged in: no
 * USB filter in the manifest, USB host not required; the smart card
 * interface claimed alone (never the keyboard Yubico OTP types with);
 * Android's permission asked from code, its answer for this package alone,
 * asked again only once the key was unplugged; the key powered off and its
 * interface given back when Sioul no longer asks; unplugged mid-way, a key
 * gone.
 */
public final class SecurityKeyCheck
{
    static int failed = 0;
    static int checked = 0;

    static void expect(String what, boolean holds)
    {
        checked++;
        if (!holds) {
            failed++;
            System.out.println("FAIL " + what);
        }
    }

    static String read(Path path) throws IOException
    {
        return new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
    }

    /** The body of the method or function that starts at `head`, braces counted. */
    static String body(String text, String head)
    {
        int at = text.indexOf(head);
        if (at < 0)
            return "";
        int open = text.indexOf('{', at);
        int depth = 0;
        for (int i = open; i < text.length(); i++) {
            if (text.charAt(i) == '{')
                depth++;
            else if (text.charAt(i) == '}' && --depth == 0)
                return text.substring(open, i + 1);
        }
        return "";
    }

    public static void main(String[] args) throws IOException
    {
        Path root = Paths.get(System.getProperty("sioul.root"));
        Path sources = root.resolve("android/package/src/com/aurelienpierre/sioul");
        String link = read(sources.resolve("SecurityKeyLink.java"));
        String manifest = read(root.resolve("android/package/AndroidManifest.xml"));
        String bridge = read(root.resolve("android/main.cpp"));

        // The manifest: NFC asked (a normal permission), not required, no tag filter.
        expect("the NFC permission", manifest.contains("<uses-permission android:name=\"android.permission.NFC\" />"));
        expect("NFC not required to install", manifest.contains("<uses-feature android:name=\"android.hardware.nfc\" android:required=\"false\" />"));
        for (String action : new String[] { "NDEF_DISCOVERED", "TECH_DISCOVERED", "TAG_DISCOVERED" })
            expect("no tag filter " + action + " in the manifest", !manifest.contains(action));

        // Plugged in: asked from code alone, the smart card interface alone.
        expect("USB host not required to install", manifest.contains("<uses-feature android:name=\"android.hardware.usb.host\" android:required=\"false\" />"));
        for (String action : new String[] { "USB_DEVICE_ATTACHED", "usb_device_filter", "device_filter" })
            expect("no USB filter " + action + " in the manifest", !manifest.contains(action));
        String consider = body(link, "private static void consider(");
        String open = body(link, "private static void open(");
        String card = body(link, "static UsbInterface cardInterface(");
        String listen = body(link, "private static void listen(");
        String heard = body(link, "private static void heard(");
        String unplug = body(link, "private static void unplug(");
        String closeUsb = body(link, "private static void close() {");
        expect("the smart card interface alone (class 0x0B, bulk in and out)", card.contains("getInterfaceClass() == UsbConstants.USB_CLASS_CSCID") && card.contains("USB_DIR_IN") && card.contains("USB_DIR_OUT"));
        expect("one claim, of that interface", countOf(link, "claimInterface(") == 1 && open.contains("UsbInterface face = cardInterface(device)") && open.contains("claimInterface(face, true)"));
        expect("another app holding it, said", open.contains("busy = device.getDeviceName()"));
        expect("Android asked once for each plugging", consider.contains("if (!asked.add(device.getDeviceName()))") && consider.indexOf("refused.contains") >= 0 && countOf(link, "requestPermission(") == 1);
        expect("its answer for this package alone, mutable from Android 12", consider.contains("setPackage(activity.getPackageName())") && consider.contains("PendingIntent.FLAG_MUTABLE"));
        expect("the receiver not exported from Android 13", listen.contains("Context.RECEIVER_NOT_EXPORTED"));
        expect("plugged, unplugged and the answer heard", listen.contains("ACTION_USB_DEVICE_ATTACHED") && listen.contains("ACTION_USB_DEVICE_DETACHED") && listen.contains("addAction(PERMISSION)"));
        expect("unplugged: forgotten, asked again next time, the key gone", heard.contains("asked.remove") && heard.contains("refused.remove") && heard.contains("unplugged = true"));
        expect("refused: kept until unplugged, said", heard.contains("refused.add(device.getDeviceName())") && heard.contains("told()"));
        expect("no longer asked: powered off, interface given back, connection closed, receiver let go", unplug.contains("powerOff()") && unplug.contains("close()") && unplug.contains("unregisterReceiver") && closeUsb.contains("releaseInterface(face)") && closeUsb.contains("opened.close()"));
        expect("stop() lets the key plugged in go", body(link, "private static void stop(").contains("unplug(activity)"));
        expect("unplugged mid-way: a key gone", body(link, "private static byte[] exchange(").contains("throw new TagLostException()"));
        expect("a key plugged in comes before one held", body(link, "static int[] held(").indexOf("ccid != null") < body(link, "static int[] held(").indexOf("held != null"));
        // Review of 23 October 2026: F6, F8.
        expect("before Android 13, the receiver reached only with Sioul's signature permission", listen.contains("registerReceiver(usbReceiver, filter, BROADCASTS, null)") && manifest.contains("android:name=\"com.aurelienpierre.sioul.permission.SECURITY_KEY_BROADCAST\" android:protectionLevel=\"signature\"") && link.contains("BROADCASTS = \"com.aurelienpierre.sioul.permission.SECURITY_KEY_BROADCAST\""));
        expect("a key unplugged believed only when Android no longer lists it", heard.indexOf("getDeviceList().containsKey(device.getDeviceName())") >= 0 && heard.indexOf("getDeviceList().containsKey") < heard.indexOf("unplugged = true"));
        expect("Android's answer heard only for a key Sioul asked for, and checked", heard.contains("if (!asked.contains(device.getDeviceName()))") && heard.contains("manager.hasPermission(device)"));
        String follow = body(link, "private static void follow(");
        expect("in the background, the key let go and the receiver unregistered, unless Android's dialog is in front", follow.contains("onActivityPaused") && follow.contains("if (paused == reading && !asking)") && follow.contains("unplug(paused)"));
        expect("back in front, looked for again", follow.contains("if (resumed == reading)") && follow.contains("listen(resumed)"));
        expect("a question left unanswered at the end is asked again next time", unplug.contains("asked.remove(pending)"));
        String ccid = read(sources.resolve("Ccid.java"));
        for (String word : new String[] { "Log.", "System.out", "System.err", "printStackTrace", "Arrays.toString" })
            expect("Ccid never " + word, !ccid.contains(word));

        // Reader mode, and NFC itself, only in SecurityKeyLink.
        List<String> elsewhere = new ArrayList<>();
        try (DirectoryStream<Path> files = Files.newDirectoryStream(sources, "*.java")) {
            for (Path file : files) {
                String name = file.getFileName().toString();
                String text = read(file);
                if (!name.equals("SecurityKeyLink.java") && (text.contains("enableReaderMode") || text.contains("enableForegroundDispatch") || text.contains("android.nfc.") || text.contains("android.hardware.usb.")))
                    elsewhere.add(name);
            }
        }
        expect("NFC or USB used outside SecurityKeyLink: " + elsewhere, elsewhere.isEmpty());
        expect("no foreground dispatch, which would take every tag", !link.contains("enableForegroundDispatch"));
        String start = body(link, "private static void start(");
        String stop = body(link, "private static void stop(");
        String want = body(link, "static void want(");
        expect("reader mode turned on in start() alone", start.contains("adapter.enableReaderMode(") && countOf(link, "enableReaderMode(") == 1);
        expect("reader mode ended in stop()", stop.contains("disableReaderMode(activity)"));
        expect("the key let go before reader mode ends: ignored until it leaves the field", stop.indexOf("letGo(") >= 0 && stop.indexOf("letGo(") < stop.indexOf("disableReaderMode") && body(link, "private static void letGo(").contains("adapter.ignore(tag, AWAY_MS"));
        expect("NFC-A and B, no NDEF read", start.contains("FLAG_READER_NFC_A") && start.contains("FLAG_READER_NFC_B") && start.contains("FLAG_READER_SKIP_NDEF_CHECK"));
        expect("want() on Android's main thread", want.contains("runOnUiThread"));
        expect("the screen kept from captures while a key is wanted, and no longer after", start.contains("addFlags(WindowManager.LayoutParams.FLAG_SECURE)") && stop.contains("clearFlags(WindowManager.LayoutParams.FLAG_SECURE)"));

        // Nothing of an APDU logged; each command wiped once sent, even when it fails.
        for (String word : new String[] { "Log.", "System.out", "System.err", "printStackTrace", "Arrays.toString", "getMessage" })
            expect("SecurityKeyLink never " + word, !link.contains(word));
        String transceive = body(link, "static byte[] transceive(");
        expect("a command wiped in a finally", transceive.contains("finally") && transceive.indexOf("Arrays.fill(command, (byte) 0)") > transceive.indexOf("finally"));
        expect("no key held: lost, as a key that left the field", transceive.contains("throw new TagLostException()"));
        expect("five seconds an exchange", link.contains("TIMEOUT_MS = 5000") && body(link, "private static void found(").contains("setTimeout(TIMEOUT_MS)"));

        // The bridge: an exchange's exception cleared, never described; copies wiped.
        String bridged = body(bridge, "extern \"C\" int sioul_android_key_transceive(");
        expect("the bridge's exchange found", !bridged.isEmpty());
        for (String word : new String[] { "ExceptionDescribe", "threw(", "__android_log", "printf", "qDebug", "qWarning" })
            expect("the bridge's exchange never " + word, !bridged.contains(word));
        expect("the command's array wiped, both ways out", countOf(bridged, "SetByteArrayRegion(sent, 0, size, blank.data())") == 2);
        expect("the answer's array wiped once copied", bridged.contains("SetByteArrayRegion(got, 0, got_size, wiped.data())"));
        expect("the key left the field, said apart", bridged.contains("android/nfc/TagLostException") && bridged.contains("return gone ? 1 : 2"));
        String free = body(bridge, "extern \"C\" void sioul_android_key_free(");
        expect("an answer wiped before it is freed", free.contains("volatile uint8_t") && free.indexOf("bytes[i] = 0") < free.indexOf("free(answer)"));
        expect("SecurityKeyLink loaded as Sioul starts", bridge.contains("appClass(env, \"com/aurelienpierre/sioul/SecurityKeyLink\")"));

        System.out.println("SecurityKeyCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }

    static int countOf(String text, String part)
    {
        Matcher found = Pattern.compile(Pattern.quote(part)).matcher(text);
        int count = 0;
        while (found.find())
            count++;
        return count;
    }
}
