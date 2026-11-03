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
import org.json.JSONObject;

/**
 * A message opened from the phone by its address, never its file (the
 * unified mailbox's part 2), on the JVM (android/jvm-checks/run.sh):
 * ReminderOpener takes "mail" with an address (mid:, sioul:mail/) and
 * refuses a path; the home card's message rows and codes open by their
 * address, an older card's row by its file, else the Porch; and, read from
 * the sources (android.jar's Intent is a stub that cannot be made here), no
 * intent built in Sioul's Java names a file: no file: address, no
 * Uri.fromFile, new mail's intent made only from an address that passes
 * the check, the card's rows likewise, and no Message-ID logged.
 */
public final class MailOpenCheck
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

    static String read(Path file) throws IOException
    {
        return new String(Files.readAllBytes(file), StandardCharsets.UTF_8);
    }

    /** The text of `name` with its comments taken out, so that only code is searched. */
    static String code(String text)
    {
        return text.replaceAll("(?s)/\\*.*?\\*/", "").replaceAll("(?m)//.*$", "");
    }

    public static void main(String[] args) throws Exception
    {
        // ReminderOpener.opens: "mail" with an address alone.
        expect("mail by its Message-ID", ReminderOpener.opens("mail", "mid:lunch-1@example.org"));
        expect("mail by its digest", ReminderOpener.opens("mail", "sioul:mail/home/0123456789abcdef"));
        expect("an id with an encoded slash", ReminderOpener.opens("mail", "mid:a%2Fb@example.org"));
        String[] refused = { "/data/user/0/com.aurelienpierre.sioul/files/mail/home/cur/1.U1-1.sioul:2,S", "mid:../../etc/passwd", "mid:", "file:///sdcard/x.eml",
            "sioul:mail/../0123456789abcdef", "sioul:mail/./0123456789abcdef", "sioul:mail/home/xyz", "lunch-1@example.org", "", null };
        for (String key : refused)
            expect("mail refused: " + key, !ReminderOpener.opens("mail", key));
        expect("the other kinds as before", ReminderOpener.opens("porch", null) && ReminderOpener.opens("event", "/c/yoga.ics") && !ReminderOpener.opens("elsewhere", "x"));
        // New mail of several: the mailbox at its top (the unified mailbox's part 5).
        expect("the mailbox, without a key", ReminderOpener.opens("mailbox", null) && ReminderOpener.opens("mailbox", ""));

        // The home card's message rows: the address, an older card's file, else the Porch.
        expect("a row by its address", "mail/mid:lunch-1@example.org".equals(HomeCardRows.opens(new JSONObject().put("key", "/mail/home/cur/1.sioul").put("address", "mid:lunch-1@example.org"))));
        expect("an older card's row by its file", "mail//mail/home/cur/1.sioul".equals(HomeCardRows.opens(new JSONObject().put("key", "/mail/home/cur/1.sioul"))));
        expect("a row with a file as its address: its file", "mail//mail/home/cur/1.sioul".equals(HomeCardRows.opens(new JSONObject().put("key", "/mail/home/cur/1.sioul").put("address", "/mail/home/cur/1.sioul"))));
        expect("a row with neither: the Porch", "porch".equals(HomeCardRows.opens(new JSONObject())));

        // From the sources: no intent names a file.
        Path sources = Paths.get(System.getProperty("sioul.root"), "android", "package", "src", "com", "aurelienpierre", "sioul");
        List<String> fileIntents = new ArrayList<>();
        try (DirectoryStream<Path> files = Files.newDirectoryStream(sources, "*.java")) {
            for (Path file : files) {
                String text = code(read(file));
                if (text.contains("Uri.fromFile(") || text.contains("\"file:"))
                    fileIntents.add(file.getFileName().toString());
            }
        }
        expect("no intent names a file (file: or Uri.fromFile): " + fileIntents, fileIntents.isEmpty());
        String notes = code(read(sources.resolve("MailNotes.java")));
        expect("new mail's tap made in MailNotes.opening alone", notes.split("new Intent\\(", -1).length == 2 && notes.contains("static Intent opening(Context context, String address)"));
        expect("new mail's tap names a message only when it is an address", notes.contains("if (ReminderOpener.isMailAddress(address))") && notes.contains("putExtra(ReminderOpener.KEY, address)"));
        expect("new mail of several opens the mailbox, never a file", notes.contains("Uri.fromParts(ReminderOpener.SCHEME, \"mailbox\", null)") && !notes.contains("Uri.fromParts(ReminderOpener.SCHEME, \"porch\", null)"));
        expect("MailNotes logs no notification's text", !notes.matches("(?s).*Log\\.[a-z]\\([^;]*\\+ e\\).*") && !notes.matches("(?s).*Log\\.[a-z]\\([^;]*(json|said|address|open)[^;]*\\).*"));
        String rows = code(read(sources.resolve("HomeCardRows.java")));
        expect("the card's rows open by an address that passes the check", rows.contains("if (ReminderOpener.isMailAddress(address))") && rows.contains("new Row(MAIL, message, \"\", opens(message))"));
        String today = code(read(sources.resolve("HomeCardToday.java")));
        expect("the card's code opens by an address that passes the check", today.contains("ReminderOpener.isMailAddress(address) ? \"mail/\" + address : \"porch\""));
        String opener = code(read(sources.resolve("ReminderOpener.java")));
        expect("ReminderOpener logs the kind alone", opener.contains("(\" + kind + \").\"") && !opener.matches("(?s).*Log\\.[a-z]\\([^;]*key[^;]*\\).*"));

        System.out.println("MailOpenCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
