// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * Recents' picture of Sioul while a health card is open (the unified agenda's
 * part 8), read from the sources (the repository's root comes as the property
 * sioul.root): android.jar's Activity is a stub that cannot be made here.
 * The bridge (android/main.cpp) turns Recents' picture off and on with
 * Activity.setRecentsScreenshotEnabled, on Android 13 and later only (its API
 * level read first), on Android's main thread, and never with FLAG_SECURE,
 * which would stop the person's own screenshots; FLAG_SECURE stays the
 * security key's alone (SecurityKeyLink.java). The doses' alarms
 * (DoseAlarms.java) do nothing of it. The window says when (main.qml's
 * RecentsGuard): the Health page, a dose's question, a prescription's date,
 * an errand's task card.
 */
public final class RecentsCheck
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

    /** The body of the function that starts at `head`, braces counted. */
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
        String bridge = body(read(root.resolve("android/main.cpp")), "extern \"C\" void sioul_android_recents_picture(bool shown)");
        expect("the bridge is there", !bridge.isEmpty());
        int guard = bridge.indexOf("android_get_device_api_level() < 33");
        int call = bridge.indexOf("setRecentsScreenshotEnabled");
        expect("Android 13 and later only: the API level read before anything", guard >= 0 && call > guard && bridge.indexOf("return;") > guard && bridge.indexOf("return;") < call);
        expect("on Android's main thread", bridge.indexOf("runOnAndroidMainThread") > guard && bridge.indexOf("runOnAndroidMainThread") < call);
        expect("on an activity alone", bridge.contains("IsInstanceOf(activity.object(), kind)"));
        expect("never FLAG_SECURE nor a window's flags", !bridge.contains("FLAG_SECURE") && !bridge.contains("addFlags") && !bridge.contains("setFlags"));

        // FLAG_SECURE: the security key's alone, in all of Sioul's Java.
        try (DirectoryStream<Path> sources = Files.newDirectoryStream(root.resolve("android/package/src/com/aurelienpierre/sioul"), "*.java")) {
            for (Path source : sources) {
                String text = read(source);
                String name = source.getFileName().toString();
                if (!name.equals("SecurityKeyLink.java"))
                    expect(name + ": no FLAG_SECURE", !text.contains("FLAG_SECURE"));
                expect(name + ": Recents' picture is the bridge's alone", !text.contains("setRecentsScreenshotEnabled"));
            }
        }
        String alarms = read(root.resolve("android/package/src/com/aurelienpierre/sioul/DoseAlarms.java"));
        expect("the doses' alarms do nothing of it", !alarms.contains("Recents"));

        // The window says when: the Health page, a dose's question, a prescription's date in the card host.
        String window = read(root.resolve("crates/sioul-app/qml/main.qml"));
        String guarded = body(window, "    RecentsGuard {");
        expect("main.qml's guard", guarded.contains("window.page === 10") && guarded.contains("doseTaken.item.opened") && guarded.contains("\"prescription-ends\", \"runs-out\""));
        expect("an errand's task card", guarded.contains("cardHost.ref.card === \"task\" && sioul.isErrand(cardHost.ref.owner"));
        String said = read(root.resolve("crates/sioul-app/qml/RecentsGuard.qml"));
        expect("the guard says it", said.contains("guard.shown = !guard.health") && said.contains("guard.sioul.recentsPicture(guard.shown)"));

        System.out.println(checked + " checked, " + failed + " failed");
        if (failed > 0)
            System.exit(1);
    }
}
