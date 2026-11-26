// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import javax.xml.parsers.DocumentBuilderFactory;

import org.w3c.dom.Element;
import org.w3c.dom.Node;
import org.w3c.dom.NodeList;

/**
 * Sioul's files read over USB, for a diagnosis (UsbFiles), on the JVM
 * (android/jvm-checks/run.sh):
 * - UsbFiles.shell: the shell's uid answered in every user of the phone,
 *   never an app, the system or root;
 * - UsbFiles.resolve: an address inside one of the four roots (config, data,
 *   state, Sioul's own cache), as the system resolves it; refused: no root
 *   or another (the whole files folder with the doses' record and the notes,
 *   the web views' cache), an empty, "." or ".." segment, a "/" or a NUL
 *   decoded in a segment, a link leading out of its root; and never, however
 *   named (a link, a witness, a file written beside): the secret parts of
 *   OpenPGP keys, the home card's codes, the doses other devices said, the
 *   sharing's memory, every health record, whose names are read from the
 *   sharing's list (crates/sioul-sync/src/share.rs) and the window's own
 *   (crates/sioul-app/src/health.rs), and health's earlier versions; a root
 *   named by a link (/data/user/0 for /data/data) changes nothing;
 * - UsbFiles.listing: the roots; a folder, the one asked first as ".", its
 *   entries by name, a link said as one, what is never given left out; a
 *   file as one row with its bytes; nothing to give, refused;
 * - the manifest: the one exported provider, off until turned on, in a
 *   process of its own, its authority, only a caller holding DUMP let in, no
 *   grants;
 * - read from the sources: the shell and the switch checked before a
 *   listing or a file, "r" alone opened, nothing logged by Sioul, no root
 *   for the whole files folder or cache, Rust's verbs routed by StepService.
 * The repository's root comes as the property sioul.root.
 */
public final class UsbFilesCheck
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

    static List<String> path(String... segments)
    {
        return List.of(segments);
    }

    static String resolved(List<String> segments, File files, File cache)
    {
        File file = UsbFiles.resolve(segments, files, cache);
        return file == null ? "refused" : file.getPath();
    }

    static String names(List<Object[]> rows)
    {
        List<String> names = new ArrayList<>();
        for (Object[] row : rows)
            names.add(row[0] + ":" + row[3]);
        return String.join(" ", names);
    }

    /** The text of a method's body in a source, from its signature to the closing brace at its indentation. */
    static String body(String source, String signature)
    {
        int at = source.indexOf(signature);
        if (at < 0)
            return "";
        int end = source.indexOf("\n    }\n", at);
        return end < 0 ? source.substring(at) : source.substring(at, end);
    }

    /** Writes a small file, its folders made. */
    static File write(File file) throws Exception
    {
        file.getParentFile().mkdirs();
        Files.writeString(file.toPath(), file.getName(), StandardCharsets.UTF_8);
        return file;
    }

    public static void main(String[] args) throws Exception
    {
        File root = new File(System.getProperty("sioul.root", "."));

        // The shell, in user 0 and in a second user; the system, root, an app, nobody.
        expect("shell: adb's shell", UsbFiles.shell(2000), true);
        expect("shell: adb's shell in user 10", UsbFiles.shell(1002000), true);
        expect("shell: the system", UsbFiles.shell(1000), false);
        expect("shell: root", UsbFiles.shell(0), false);
        expect("shell: an app", UsbFiles.shell(10234), false);
        expect("shell: an app in user 10", UsbFiles.shell(1010234), false);
        expect("shell: no uid", UsbFiles.shell(-1), false);

        // The health records' names, from the sharing's list and the window's own (a new one cannot slip by).
        List<String> health = new ArrayList<>();
        Matcher shared = Pattern.compile("file\\(\"health\",\\s*\"(data|state)/([^\"]+)\"").matcher(Files.readString(new File(root, "crates/sioul-sync/src/share.rs").toPath(), StandardCharsets.UTF_8));
        while (shared.find())
            health.add(shared.group(1) + "/sioul/" + shared.group(2));
        Matcher own = Pattern.compile("state_dir\\(\\)\\.join\\(\"(health[^\"]*)\"\\)").matcher(Files.readString(new File(root, "crates/sioul-app/src/health.rs").toPath(), StandardCharsets.UTF_8));
        while (own.find())
            health.add("state/sioul/" + own.group(1));
        expect("health: the records named in the sources", health.size() >= 7 && health.contains("data/sioul/health.toml") && health.contains("state/sioul/health-doubt.toml"), true);

        File base = Files.createTempDirectory("sioul-usb-files-check").toFile();
        try {
            File app = new File(base, "data/com.aurelienpierre.sioul");
            File files = new File(app, "files");
            File cache = new File(app, "cache");
            File log = write(new File(files, "state/sioul/extensions/planity.log"));
            File config = write(new File(files, "config/sioul/config.toml"));
            File secret = write(new File(files, "data/sioul/pgp/own/0123.asc"));
            write(new File(files, "data/sioul/pgp/own/.4567.asc.tmp"));
            File others = write(new File(files, "data/sioul/pgp/others/4567.asc"));
            File owned = write(new File(files, "data/sioul/pgp/owned/89ab.asc"));
            File cached = write(new File(cache, "sioul/mail/index.tsv"));
            File webview = write(new File(cache, "WebView/Default/HTTP Cache/index"));
            File prefs = write(new File(app, "shared_prefs/sioul.xml"));
            File doses = write(new File(files, "dose-alarms.json"));
            File notes = write(new File(files, "Notes/Doctor.md"));
            File card = write(new File(files, "state/sioul/home-card.json"));
            write(new File(files, "state/sioul/home-card.json.new"));
            File calls = write(new File(files, "state/sioul/calls/log.jsonl"));
            File peers = write(new File(files, "state/sioul/share/peers.json"));
            write(new File(files, "state/sioul/share/memory.json"));
            write(new File(files, "state/sioul/share/memory.json.new"));
            write(new File(files, "state/sioul/share/.memory.json.written"));
            File sharedConfig = write(new File(files, "state/sioul/share/history/settings/config.toml"));
            write(new File(files, "state/sioul/share/history/health/health.toml"));
            File wealth = write(new File(files, "data/sioul/wealth.toml"));
            for (String record : health) {
                File file = write(new File(files, record));
                write(new File(file.getParentFile(), file.getName() + ".new"));
                write(new File(file.getParentFile(), "." + file.getName() + ".written"));
            }

            // Inside a root.
            expect("an extension's log", resolved(path("state", "sioul", "extensions", "planity.log"), files, cache), log.getCanonicalPath());
            expect("the settings", resolved(path("config", "sioul", "config.toml"), files, cache), config.getCanonicalPath());
            expect("the Java side's own, in state", resolved(path("state", "sioul", "calls", "log.jsonl"), files, cache), calls.getCanonicalPath());
            expect("Sioul's cache", resolved(path("cache", "mail", "index.tsv"), files, cache), cached.getCanonicalPath());
            expect("a root itself", resolved(path("state"), files, cache), new File(files, "state").getCanonicalPath());
            expect("the cache's root is Sioul's own", resolved(path("cache"), files, cache), new File(cache, "sioul").getCanonicalPath());
            expect("a file not there yet, inside", resolved(path("state", "sioul", "none.toml"), files, cache), new File(files, "state/sioul/none.toml").getCanonicalPath());
            expect("others' keys", resolved(path("data", "sioul", "pgp", "others", "4567.asc"), files, cache), others.getCanonicalPath());
            expect("a name that only begins as the secret folder's", resolved(path("data", "sioul", "pgp", "owned", "89ab.asc"), files, cache), owned.getCanonicalPath());
            expect("an earlier version that is not health's", resolved(path("state", "sioul", "share", "history", "settings", "config.toml"), files, cache), sharedConfig.getCanonicalPath());
            expect("a name that only ends as health's", resolved(path("data", "sioul", "wealth.toml"), files, cache), wealth.getCanonicalPath());

            // Refused: the addresses.
            expect("no segment", resolved(path(), files, cache), "refused");
            expect("another root", resolved(path("shared_prefs", "sioul.xml"), files, cache), "refused");
            expect("the files folder: no root", resolved(path("files", "dose-alarms.json"), files, cache), "refused");
            expect("the notes in the files folder: no root", resolved(path("files", "Notes", "Doctor.md"), files, cache), "refused");
            expect("a root's name in another case", resolved(path("State", "sioul"), files, cache), "refused");
            expect("climbing out", resolved(path("state", "..", "dose-alarms.json"), files, cache), "refused");
            expect("climbing out of Sioul's cache", resolved(path("cache", "..", "WebView", "Default", "HTTP Cache", "index"), files, cache), "refused");
            expect("climbing within", resolved(path("state", "..", "config", "sioul", "config.toml"), files, cache), "refused");
            expect("a dot", resolved(path("state", ".", "sioul"), files, cache), "refused");
            expect("an empty segment", resolved(path("state", "", "sioul"), files, cache), "refused");
            expect("a slash decoded in a segment", resolved(path("state", "sioul/../../../shared_prefs"), files, cache), "refused");
            expect("a NUL decoded in a segment", resolved(path("state", "sioul\0x"), files, cache), "refused");

            // Refused: never given.
            expect("a secret key", resolved(path("data", "sioul", "pgp", "own", "0123.asc"), files, cache), "refused");
            expect("a secret key being written", resolved(path("data", "sioul", "pgp", "own", ".4567.asc.tmp"), files, cache), "refused");
            expect("the secret keys' folder", resolved(path("data", "sioul", "pgp", "own"), files, cache), "refused");
            expect("the home card", resolved(path("state", "sioul", "home-card.json"), files, cache), "refused");
            expect("the home card being written", resolved(path("state", "sioul", "home-card.json.new"), files, cache), "refused");
            expect("the doses other devices said", resolved(path("state", "sioul", "share", "peers.json"), files, cache), "refused");
            expect("the sharing's memory", resolved(path("state", "sioul", "share", "memory.json"), files, cache), "refused");
            expect("the sharing's memory being written", resolved(path("state", "sioul", "share", "memory.json.new"), files, cache), "refused");
            expect("the sharing's memory's witness", resolved(path("state", "sioul", "share", ".memory.json.written"), files, cache), "refused");
            expect("health's earlier versions", resolved(path("state", "sioul", "share", "history", "health", "health.toml"), files, cache), "refused");
            expect("health's earlier versions' folder", resolved(path("state", "sioul", "share", "history", "health"), files, cache), "refused");
            for (String record : health) {
                List<String> segments = new ArrayList<>(List.of(record.split("/")));
                expect("health: " + record, resolved(segments, files, cache), "refused");
                String name = segments.remove(segments.size() - 1);
                List<String> beside = new ArrayList<>(segments);
                beside.add(name + ".new");
                expect("health: " + record + ".new", resolved(beside, files, cache), "refused");
                for (String folder : List.of("data", "state")) {
                    List<String> witness = new ArrayList<>(List.of(folder, "sioul", "." + name + ".written"));
                    write(new File(files, String.join("/", witness)));
                    expect("health: " + String.join("/", witness), resolved(witness, files, cache), "refused");
                }
            }

            // Links: out of the root refused, within it followed, never to what is never given.
            File out = new File(files, "state/sioul/out");
            Files.createSymbolicLink(out.toPath(), prefs.getParentFile().toPath());
            expect("a link out of the app's roots", resolved(path("state", "sioul", "out", "sioul.xml"), files, cache), "refused");
            File outFile = new File(files, "state/sioul/prefs.xml");
            Files.createSymbolicLink(outFile.toPath(), prefs.toPath());
            expect("a link to a file outside", resolved(path("state", "sioul", "prefs.xml"), files, cache), "refused");
            File across = new File(files, "state/sioul/settings");
            Files.createSymbolicLink(across.toPath(), config.getParentFile().toPath());
            expect("a link to another root", resolved(path("state", "sioul", "settings", "config.toml"), files, cache), "refused");
            File toNotes = new File(files, "data/sioul/notes");
            Files.createSymbolicLink(toNotes.toPath(), notes.getParentFile().toPath());
            expect("a link to the notes in the files folder", resolved(path("data", "sioul", "notes", "Doctor.md"), files, cache), "refused");
            File toWebView = new File(cache, "sioul/web");
            Files.createSymbolicLink(toWebView.toPath(), webview.getParentFile().toPath());
            expect("a link to the web views' cache", resolved(path("cache", "web", "index"), files, cache), "refused");
            File inside = write(new File(files, "data/sioul/records/sioul-budgets.toml"));
            File within = new File(files, "data/sioul/budgets.toml");
            Files.createSymbolicLink(within.toPath(), inside.toPath());
            expect("a link within its root", resolved(path("data", "sioul", "budgets.toml"), files, cache), inside.getCanonicalPath());
            File keys = new File(files, "data/sioul/keys");
            Files.createSymbolicLink(keys.toPath(), secret.getParentFile().toPath());
            expect("a link to the secret keys", resolved(path("data", "sioul", "keys", "0123.asc"), files, cache), "refused");
            File toCard = new File(files, "state/sioul/today.json");
            Files.createSymbolicLink(toCard.toPath(), card.toPath());
            expect("a link to the home card", resolved(path("state", "sioul", "today.json"), files, cache), "refused");
            File toHealth = new File(files, "data/sioul/doses.toml");
            Files.createSymbolicLink(toHealth.toPath(), new File(files, "data/sioul/health.toml").toPath());
            expect("a link to a health record", resolved(path("data", "sioul", "doses.toml"), files, cache), "refused");
            File toPeers = new File(files, "state/sioul/devices.json");
            Files.createSymbolicLink(toPeers.toPath(), peers.toPath());
            expect("a link to the doses other devices said", resolved(path("state", "sioul", "devices.json"), files, cache), "refused");
            // The files folder named by a link (Android's /data/user/0 for /data/data): the same rules.
            File alias = new File(base, "user0");
            Files.createSymbolicLink(alias.toPath(), app.toPath());
            File aliasFiles = new File(alias, "files");
            File aliasCache = new File(alias, "cache");
            expect("a root named by a link: inside", resolved(path("state", "sioul", "extensions", "planity.log"), aliasFiles, aliasCache), log.getCanonicalPath());
            expect("a root named by a link: a secret key", resolved(path("data", "sioul", "pgp", "own", "0123.asc"), aliasFiles, aliasCache), "refused");
            expect("a root named by a link: the home card", resolved(path("state", "sioul", "home-card.json"), aliasFiles, aliasCache), "refused");
            expect("a root named by a link: a health record", resolved(path("data", "sioul", "health.toml"), aliasFiles, aliasCache), "refused");

            // Listings: what is never given is left out.
            expect("the roots", names(UsbFiles.listing(path(), files, cache)), "folder:. folder:config folder:data folder:state folder:cache");
            expect("a folder, the secret one left out", names(UsbFiles.listing(path("data", "sioul", "pgp"), files, cache)), "folder:. folder:others folder:owned");
            String state = names(UsbFiles.listing(path("state", "sioul"), files, cache));
            expect("state: no home card, no health record", state.contains("home-card") || state.contains("health"), false);
            expect("state: the rest listed, links said as links", state, "folder:. folder:calls link:devices.json folder:extensions link:out link:prefs.xml link:settings folder:share link:today.json");
            String data = names(UsbFiles.listing(path("data", "sioul"), files, cache));
            expect("data: no health record", data.contains("health"), false);
            expect("data: the rest listed", data, "folder:. link:budgets.toml link:doses.toml link:keys link:notes folder:pgp folder:records file:wealth.toml");
            expect("share: neither peers nor memory", names(UsbFiles.listing(path("state", "sioul", "share"), files, cache)), "folder:. folder:history");
            expect("history: not health's", names(UsbFiles.listing(path("state", "sioul", "share", "history"), files, cache)), "folder:. folder:settings");
            expect("Sioul's cache, not the web views'", names(UsbFiles.listing(path("cache"), files, cache)), "folder:. folder:mail link:web");
            List<Object[]> one = UsbFiles.listing(path("state", "sioul", "extensions", "planity.log"), files, cache);
            expect("a file: one row", names(one), "file:.");
            expect("a file: its bytes", one.get(0)[1], (long) "planity.log".length());
            expect("a file: when it changed, in UTC", String.valueOf(one.get(0)[2]).matches("\\d{4}-\\d\\d-\\d\\dT\\d\\d:\\d\\d:\\d\\dZ"), true);
            expect("a folder: no bytes", UsbFiles.listing(path("state"), files, cache).get(0)[1], null);
            for (List<String> asked : List.of(path("data", "sioul", "pgp", "own"), path("state", "sioul", "home-card.json"), path("data", "sioul", "health.toml"), path("files"), path("state", "sioul", "none.toml"))) {
                String refused;
                try {
                    UsbFiles.listing(asked, files, cache);
                    refused = "listed";
                } catch (IllegalArgumentException e) {
                    refused = e.getMessage();
                }
                expect("listing refused: " + String.join("/", asked), refused, "Not one of Sioul's files that can be read: " + String.join("/", asked));
            }
            expect("the doses' record and the notes stay where they were", doses.isFile() && notes.isFile(), true);
        } finally {
            try (var walk = Files.walk(base.toPath())) {
                walk.sorted(java.util.Comparator.reverseOrder()).forEach(p -> p.toFile().delete());
            }
        }

        File manifest = new File(root, "android/package/AndroidManifest.xml");
        expect("manifest: found", manifest.isFile(), true);
        if (manifest.isFile()) {
            DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
            Element application = elements(factory.newDocumentBuilder().parse(manifest).getDocumentElement(), "application").get(0);
            List<String> exported = new ArrayList<>();
            for (Element provider : elements(application, "provider")) {
                if (provider.getAttribute("android:exported").equals("true"))
                    exported.add(provider.getAttribute("android:name"));
                if (!provider.getAttribute("android:name").equals("com.aurelienpierre.sioul.UsbFiles"))
                    continue;
                expect("provider: its authority", provider.getAttribute("android:authorities"), "${applicationId}" + UsbFiles.AUTHORITY);
                expect("provider: off until turned on", provider.getAttribute("android:enabled"), "false");
                expect("provider: a process of its own", provider.getAttribute("android:process"), ":files");
                expect("provider: no grants", provider.getAttribute("android:grantUriPermissions"), "false");
                expect("provider: only a caller holding DUMP (the shell) let in", provider.getAttribute("android:permission"), "android.permission.DUMP");
                expect("provider: no weaker read or write permission", provider.getAttribute("android:readPermission") + provider.getAttribute("android:writePermission"), "");
                expect("provider: not before the first unlock", provider.getAttribute("android:directBootAware"), "");
                expect("provider: no paths of a FileProvider", elements(provider, "meta-data").size(), 0);
            }
            expect("provider: the one exported", exported, List.of("com.aurelienpierre.sioul.UsbFiles"));
        }

        String source = Files.readString(new File(root, "android/package/src/com/aurelienpierre/sioul/UsbFiles.java").toPath(), StandardCharsets.UTF_8);
        String query = body(source, "public Cursor query(");
        String open = body(source, "public ParcelFileDescriptor openFile(");
        expect("source: a listing checks the shell and the switch first", query.contains("{\n        allowed();"), true);
        expect("source: a file checks the shell and the switch first", open.contains("{\n        allowed();"), true);
        expect("source: a file read only", open.contains("if (!\"r\".equals(mode))") && open.contains("MODE_READ_ONLY") && !open.contains("MODE_READ_WRITE") && !open.contains("MODE_WRITE_ONLY"), true);
        String allowed = body(source, "private void allowed()");
        expect("source: the caller's uid, then the switch", allowed.contains("shell(Binder.getCallingUid())") && allowed.contains("on(context)"), true);
        expect("source: Sioul logs nothing", source.contains("Log.") || source.contains("import android.util.Log"), false);
        expect("source: no root for the whole files folder nor the whole cache", source.contains("case \"files\"") || source.contains("return cache;") || source.contains("return files;"), false);
        expect("source: turned off, the processes left running", source.contains("PackageManager.DONT_KILL_APP"), true);
        String steps = Files.readString(new File(root, "android/package/src/com/aurelienpierre/sioul/StepService.java").toPath(), StandardCharsets.UTF_8);
        expect("source: Rust's verbs routed", steps.contains("if (verb.startsWith(\"files-usb\"))\n                    return UsbFiles.call(context, verb, asked);"), true);

        System.out.println("UsbFilesCheck: " + (checked - failed) + "/" + checked + " checks passed");
        if (failed > 0)
            System.exit(1);
    }
}
