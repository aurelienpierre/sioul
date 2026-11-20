// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

import org.json.JSONObject;

/**
 * A file chosen in one of Sioul's dialogs on a phone (Chosen, chosen.rs), on
 * the JVM (android/jvm-checks/run.sh):
 * - Chosen.room: a file larger than Rust's bound is not copied, nor one that
 *   would leave less room than Rust's margin; a size the provider does not
 *   say is bounded by the copy itself;
 * - Chosen.content, Chosen.copy and Chosen.call refuse any address but a
 *   content:// one, and a bound Java cannot hold, before reading anything;
 * - ShareActivity.cleaned, the share sheet's own, on the names a provider
 *   may give a document: no separator, no hidden dot, never ".part", not
 *   too long, its extension kept;
 * - read from the sources: Chosen opens nothing itself (its reading is
 *   ShareActivity's, its writing Attachments'), StepService hands it the
 *   verbs "chosen-…", and the manifest asks no permission to read the
 *   phone's media or storage beyond what it held.
 * The repository's root comes as the property sioul.root.
 */
public final class ChosenCheck
{
    static int failed = 0;
    static int checked = 0;

    static void expect(String what, Object got, Object wanted)
    {
        checked++;
        if (!String.valueOf(got).equals(String.valueOf(wanted))) {
            failed++;
            System.out.println("FAIL " + what + ": " + got + " (expected " + wanted + ")");
        }
    }

    public static void main(String[] args) throws Exception
    {
        final long mb = 1024L * 1024;
        final long most = 100 * mb;
        final long margin = 200 * mb;

        // The bound and the room, before a byte is read.
        expect("a small file, room to spare", Chosen.room(2 * mb, 10_000 * mb, most, margin), null);
        expect("at the bound", Chosen.room(most, 10_000 * mb, most, margin), null);
        expect("past the bound", Chosen.room(most + 1, 10_000 * mb, most, margin), "large");
        expect("past the bound, whatever the room", Chosen.room(most + 1, 10, most, margin), "large");
        expect("the margin kept", Chosen.room(50 * mb, 249 * mb, most, margin), "room");
        expect("the margin just kept", Chosen.room(50 * mb, 250 * mb, most, margin), null);
        expect("a size not said: the margin alone", Chosen.room(-1, 199 * mb, most, margin), "room");
        expect("a size not said, room enough", Chosen.room(-1, 300 * mb, most, margin), null);
        expect("the room not said", Chosen.room(3 * mb, -1, most, margin), null);

        // Only content:// addresses, and a bound Java holds, before anything is read.
        expect("content", Chosen.content("content://com.android.providers.downloads.documents/document/msf%3A12"), true);
        expect("content, in capitals", Chosen.content("CONTENT://media/external/file/12"), true);
        expect("a file's address", Chosen.content("file:///storage/emulated/0/Download/a.pdf"), false);
        expect("a path", Chosen.content("/storage/emulated/0/Download/a.pdf"), false);
        expect("nothing", Chosen.content(""), false);
        expect("null", Chosen.content(null), false);
        File folder = Files.createTempDirectory("sioul-chosen-check").toFile();
        try {
            expect("a file's address not copied", Chosen.copy(null, "file:///etc/hosts", folder, most, margin).optString("problem"), "refused");
            expect("no folder", Chosen.copy(null, "content://x/document/1", new File(""), most, margin).optString("problem"), "refused");
            expect("no bound", Chosen.copy(null, "content://x/document/1", folder, 0, margin).optString("problem"), "refused");
            expect("a bound past an int", Chosen.copy(null, "content://x/document/1", folder, Integer.MAX_VALUE, margin).optString("problem"), "refused");
            expect("nothing made", folder.list().length, 0);
            JSONObject save = new JSONObject().put("path", new File(folder, "x.csv").getPath()).put("uri", "file:///storage/emulated/0/Download/x.csv").put("name", "x.csv");
            expect("never saved to a file's address", new JSONObject(Chosen.call(null, "chosen-save", save)).optString("problem"), "refused");
            expect("no name asked of a file's address", new JSONObject(Chosen.call(null, "chosen-name", new JSONObject().put("uri", "/etc/hosts"))).optString("name"), "");
            expect("another verb", Chosen.call(null, "chosen-other", new JSONObject()), null);
        } finally {
            for (File f : folder.listFiles())
                f.delete();
            folder.delete();
        }

        // The names a provider gives, cleaned as a shared file's are (types null: Android's table is a stub here).
        expect("separators", ShareActivity.cleaned("Scans/2026\\avis.pdf", null), "Scans_2026_avis.pdf");
        expect("climbing", ShareActivity.cleaned("../../config.toml", null), "_.._config.toml");
        expect("hidden", ShareActivity.cleaned(".bashrc.pdf", null), "bashrc.pdf");
        expect("a part", ShareActivity.cleaned("relevé.pdf.part", null), "relevé.pdf.part_");
        expect("no name", ShareActivity.cleaned("", null), "attachment");
        expect("accents kept", ShareActivity.cleaned(" Relevé de compte – octobre.pdf ", null), "Relevé de compte – octobre.pdf");
        String longest = ShareActivity.cleaned("é".repeat(150) + ".pdf", null);
        expect("not too long", longest.getBytes(StandardCharsets.UTF_8).length <= 200, true);
        expect("its extension kept", longest.endsWith(".pdf"), true);

        // From the sources.
        String root = System.getProperty("sioul.root");
        String chosen = Files.readString(new File(root, "android/package/src/com/aurelienpierre/sioul/Chosen.java").toPath());
        for (String opener : new String[] { "openInputStream", "openOutputStream", "openAssetFileDescriptor", "openFileDescriptor", "FileInputStream", "FileOutputStream" })
            expect("Chosen opens nothing itself (" + opener + ")", chosen.contains(opener), false);
        expect("the share sheet's reading", chosen.contains("ShareActivity.copy(") && chosen.contains("ShareActivity.name(") && chosen.contains("ShareActivity.cleaned("), true);
        expect("the attachments' writer", chosen.contains("Attachments.write("), true);
        String steps = Files.readString(new File(root, "android/package/src/com/aurelienpierre/sioul/StepService.java").toPath());
        expect("StepService hands it its verbs", steps.contains("verb.startsWith(\"chosen-\")") && steps.contains("Chosen.call(context, verb, asked)"), true);
        String manifest = Files.readString(new File(root, "android/package/AndroidManifest.xml").toPath());
        for (String permission : new String[] { "READ_MEDIA_IMAGES", "READ_MEDIA_VIDEO", "READ_MEDIA_AUDIO", "READ_MEDIA_VISUAL_USER_SELECTED", "ACCESS_MEDIA_LOCATION" })
            expect("no permission " + permission, manifest.contains(permission), false);

        System.out.println("ChosenCheck: " + (checked - failed) + " of " + checked + " checks hold.");
        if (failed > 0)
            System.exit(1);
    }
}
