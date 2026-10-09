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

/**
 * No badge on Sioul's icon, on the JVM (android/jvm-checks/run.sh): every
 * notification channel is made by Channels.quiet, which turns its badge off,
 * and no other place of Sioul's Java makes one. Read from the sources (the
 * repository's root comes as the property sioul.root): android.jar's
 * NotificationChannel is a stub that cannot be made here.
 */
public final class ChannelsCheck
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

    public static void main(String[] args) throws IOException
    {
        Path sources = Paths.get(System.getProperty("sioul.root"), "android", "package", "src", "com", "aurelienpierre", "sioul");
        List<String> making = new ArrayList<>();
        String helper = "";
        try (DirectoryStream<Path> files = Files.newDirectoryStream(sources, "*.java")) {
            for (Path file : files) {
                String text = new String(Files.readAllBytes(file), StandardCharsets.UTF_8);
                String name = file.getFileName().toString();
                if (name.equals("Channels.java"))
                    helper = text;
                else if (text.contains("new NotificationChannel("))
                    making.add(name);
            }
        }
        expect("a channel made outside Channels.quiet, with its badge: " + making, making.isEmpty());
        expect("Channels.quiet makes the channel", helper.contains("new NotificationChannel(id, name, importance)"));
        expect("Channels.quiet turns its badge off", helper.contains("setShowBadge(false)"));
        expect("Channels.quiet is used", !helper.isEmpty());
        System.out.println("ChannelsCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
