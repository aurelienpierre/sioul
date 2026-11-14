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
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * A message opened from the phone by its address, never its file (the
 * unified mailbox's part 2), on the JVM (android/jvm-checks/run.sh):
 * ReminderOpener takes "mail" with an address (mid:, sioul:mail/) and
 * refuses a path; the home card's message rows and codes open by their
 * address, an older card's row by its file, else the Porch; and, read from
 * the sources (android.jar's Intent is a stub that cannot be made here), no
 * intent built in Sioul's Java names a file: no file: address, no
 * Uri.fromFile, a notice's intents made only from an address that passes
 * the check (Notices.opening), the card's rows likewise, and no Message-ID
 * logged. New mail's notice (MailNotes.said): one button per message Rust
 * names (`opens`, phase 2 of the notifications), three at most, each its
 * own address, a tap the first; without them, `open`'s message, else the
 * mailbox; a notice to know, its minutes Rust's, its lock screen the count.
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
        expect("new mail makes no intent of its own (Notices.opening)", !notes.contains("new Intent(") && !notes.contains("PendingIntent"));
        expect("new mail's buttons open a message only when it is an address", notes.contains("if (ReminderOpener.isMailAddress(address) && !label.isEmpty())") && notes.contains("ReminderOpener.isMailAddress(one) ? one : \"sioul:mailbox\""));
        expect("MailNotes logs no notification's text", !notes.matches("(?s).*Log\\.[a-z]\\([^;]*\\+ e\\).*") && !notes.matches("(?s).*Log\\.[a-z]\\([^;]*(json|said|address|open)[^;]*\\).*"));
        String poster = code(read(sources.resolve("Notices.java")));
        expect("a notice's taps made in Notices.opening alone", poster.split("new Intent\\(", -1).length == 2 && poster.contains("static PendingIntent opening(Context context, String tag, String address)"));
        expect("a notice's tap names a thing only when it is an address", poster.contains("ReminderOpener.isNoticeAddress(address) ? ReminderOpener.NOTICE : \"app\"") && poster.contains(".putExtra(ReminderOpener.KEY, ReminderOpener.NOTICE.equals(kind) ? address : \"\")"));
        expect("Notices logs no notification's text", !poster.matches("(?s).*Log\\.[a-z]\\([^;]*\\+ e\\).*") && !poster.matches("(?s).*Log\\.[a-z]\\([^;]*(json|said|address|about|title|body)[^;]*\\).*"));
        String receiver = code(read(sources.resolve("NoticeReceiver.java")));
        expect("a code copied is never logged", !receiver.matches("(?s).*Log\\.[a-z]\\([^;]*(text|tag|clip)[^;]*\\).*"));

        // New mail's notice: one button per message named, a tap the first.
        JSONObject words = new JSONObject().put("open", "Open");
        JSONObject three = new JSONObject().put("title", "Three e-mails").put("body", "Ana, Lunch\nNoa, Rent\nIo, Keys").put("words", words)
            .put("opens", new JSONArray()
                .put(new JSONObject().put("label", "Ana, Lunch").put("address", "mid:lunch-1@example.org"))
                .put(new JSONObject().put("label", "Noa, Rent").put("address", "sioul:mail/home/0123456789abcdef"))
                .put(new JSONObject().put("label", "A file").put("address", "/data/user/0/com.aurelienpierre.sioul/files/mail/home/cur/1"))
                .put(new JSONObject().put("label", "Io, Keys").put("address", "mid:keys@example.org"))
                .put(new JSONObject().put("label", "Al, More").put("address", "mid:more@example.org")))
            .put("open", "");
        Notices.Said mail = MailNotes.said(three, 300_000);
        expect("a button per message named, three at most, never a file", mail.buttons.size() == 3 && mail.buttons.get(0).open.equals("mid:lunch-1@example.org") && mail.buttons.get(1).open.equals("sioul:mail/home/0123456789abcdef") && mail.buttons.get(2).open.equals("mid:keys@example.org"));
        expect("each button its words", mail.buttons.get(0).label.equals("Ana, Lunch") && mail.buttons.get(2).label.equals("Io, Keys"));
        expect("a tap opens the first", mail.about.equals("mid:lunch-1@example.org"));
        expect("each press its own request", Notices.requestCode(mail.tag, mail.buttons.get(0).open) != Notices.requestCode(mail.tag, mail.buttons.get(1).open));
        expect("a notice to know, in its place, the minutes Rust gave", !mail.actionable && mail.group.equals(Notices.INFORMATIVE) && mail.tag.equals(MailNotes.TAG) && mail.lastsMs == 300_000 && mail.endsMs == 0);
        expect("on the lock screen, the count alone", "Three e-mails".equals(mail.publicTitle) && "".equals(mail.publicBody));
        Notices.Said said = MailNotes.said(new JSONObject(three.toString()).put("lasts_ms", 600_000).put("ends", 1_794_000_000_000L).put("about", "mid:keys@example.org"), 300_000);
        expect("its life and thing as Rust says them", said.lastsMs == 600_000 && said.endsMs == 1_794_000_000_000L && said.about.equals("mid:keys@example.org"));
        // Before phase 2 (no `opens`): one message, it; several, the mailbox at its top.
        Notices.Said one = MailNotes.said(new JSONObject().put("title", "One e-mail").put("body", "Ana, Lunch").put("open", "mid:lunch-1@example.org").put("words", words), 300_000);
        expect("one message: Open and a tap open it", one.buttons.size() == 1 && one.buttons.get(0).open.equals("mid:lunch-1@example.org") && one.about.equals("mid:lunch-1@example.org"));
        Notices.Said several = MailNotes.said(new JSONObject().put("title", "Two e-mails").put("open", "").put("words", words), 300_000);
        expect("several: the mailbox at its top", several.about.equals("sioul:mailbox") && several.buttons.get(0).open.equals("sioul:mailbox"));
        Notices.Said path = MailNotes.said(new JSONObject().put("title", "One e-mail").put("open", "/data/user/0/com.aurelienpierre.sioul/files/mail/home/cur/1").put("words", words), 300_000);
        expect("a file given: the mailbox, never the file", path.about.equals("sioul:mailbox"));
        expect("Open in Rust's words", one.buttons.get(0).label.equals("Open"));
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
