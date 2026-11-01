// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.File;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;

import javax.xml.parsers.DocumentBuilderFactory;

import org.w3c.dom.Element;
import org.w3c.dom.Node;
import org.w3c.dom.NodeList;

/**
 * A calendar file opened on a phone (ShareActivity, calendarfile.rs), on the
 * JVM (android/jvm-checks/run.sh):
 * - ShareActivity.insideOwn: a file: path into Sioul's own folders is
 *   refused, as the system resolves it: written inside, climbing in with
 *   "..", through a link to a folder or a file of Sioul's, the folder
 *   itself; a file elsewhere, and a link out of them, are not;
 * - the manifest: ShareActivity takes VIEW of text/calendar,
 *   application/ics and text/x-vcalendar on content: and file: addresses.
 * The repository's root comes as the property sioul.root.
 */
public final class ShareCheck
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

    static List<Element> elements(Node parent, String name)
    {
        List<Element> found = new ArrayList<>();
        NodeList children = parent.getChildNodes();
        for (int i = 0; i < children.getLength(); i++) {
            if (children.item(i) instanceof Element && ((Element) children.item(i)).getTagName().equals(name))
                found.add((Element) children.item(i));
        }
        return found;
    }

    public static void main(String[] args) throws Exception
    {
        File base = Files.createTempDirectory("sioul-share-check").toFile();
        try {
            File own = new File(base, "data/com.aurelienpierre.sioul");
            File handed = new File(own, "files/data/sioul/handed");
            handed.mkdirs();
            File secret = new File(own, "files/data/sioul/config.toml");
            Files.writeString(secret.toPath(), "[[account]]\n");
            File downloads = new File(base, "storage/Download");
            downloads.mkdirs();
            File ticket = new File(downloads, "ticket.ics");
            Files.writeString(ticket.toPath(), "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n");
            List<String> roots = List.of(own.getPath());

            expect("a file elsewhere", ShareActivity.insideOwn(ticket.getPath(), roots), false);
            expect("one of Sioul's files", ShareActivity.insideOwn(secret.getPath(), roots), true);
            expect("Sioul's folder itself", ShareActivity.insideOwn(own.getPath(), roots), true);
            expect("climbing in", ShareActivity.insideOwn(downloads.getPath() + "/../../data/com.aurelienpierre.sioul/files/data/sioul/config.toml", roots), true);
            expect("a name that only begins the same", ShareActivity.insideOwn(own.getPath() + "-other/ticket.ics", roots), false);
            File toFile = new File(downloads, "trap.ics");
            Files.createSymbolicLink(toFile.toPath(), secret.toPath());
            expect("a link to one of Sioul's files", ShareActivity.insideOwn(toFile.getPath(), roots), true);
            File toFolder = new File(downloads, "folder");
            Files.createSymbolicLink(toFolder.toPath(), handed.toPath());
            expect("through a link to a folder of Sioul's", ShareActivity.insideOwn(toFolder.getPath() + "/x.ics", roots), true);
            File out = new File(handed, "out.ics");
            Files.createSymbolicLink(out.toPath(), ticket.toPath());
            expect("a link out of Sioul's folders", ShareActivity.insideOwn(out.getPath(), roots), false);
            // A root named by a link (/data/data → /data/user/0): the same folder.
            File alias = new File(base, "alias");
            Files.createSymbolicLink(alias.toPath(), own.toPath());
            expect("a root named by a link", ShareActivity.insideOwn(secret.getPath(), List.of(alias.getPath())), true);
        } finally {
            try (var walk = Files.walk(base.toPath())) {
                walk.sorted(java.util.Comparator.reverseOrder()).forEach(p -> p.toFile().delete());
            }
        }

        File root = new File(System.getProperty("sioul.root", "."));
        File manifest = new File(root, "android/package/AndroidManifest.xml");
        expect("manifest: found", manifest.isFile(), true);
        if (manifest.isFile()) {
            DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
            Element application = elements(factory.newDocumentBuilder().parse(manifest).getDocumentElement(), "application").get(0);
            boolean viewed = false;
            for (Element activity : elements(application, "activity")) {
                if (!activity.getAttribute("android:name").equals("com.aurelienpierre.sioul.ShareActivity"))
                    continue;
                for (Element filter : elements(activity, "intent-filter")) {
                    List<String> actions = new ArrayList<>();
                    for (Element action : elements(filter, "action"))
                        actions.add(action.getAttribute("android:name"));
                    List<String> types = new ArrayList<>();
                    List<String> schemes = new ArrayList<>();
                    for (Element data : elements(filter, "data")) {
                        if (!data.getAttribute("android:mimeType").isEmpty())
                            types.add(data.getAttribute("android:mimeType"));
                        if (!data.getAttribute("android:scheme").isEmpty())
                            schemes.add(data.getAttribute("android:scheme"));
                    }
                    if (actions.equals(List.of("android.intent.action.VIEW")) && types.contains("text/calendar")) {
                        viewed = true;
                        expect("calendar: its types", types, List.of("text/calendar", "application/ics", "text/x-vcalendar"));
                        expect("calendar: its schemes", schemes, List.of("content", "file"));
                    }
                }
            }
            expect("calendar: ShareActivity takes VIEW of calendar files", viewed, true);
        }

        System.out.println("ShareCheck: " + (checked - failed) + "/" + checked + " checks passed");
        if (failed > 0)
            System.exit(1);
    }
}
