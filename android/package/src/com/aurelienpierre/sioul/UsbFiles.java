// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.ComponentName;
import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Context;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.Binder;
import android.os.ParcelFileDescriptor;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.nio.file.Files;
import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import org.json.JSONObject;

/**
 * Sioul's files read from a computer, for a diagnosis (docs/android.md,
 * "Sioul's files over USB"): what Sioul keeps in its private storage, which
 * nothing else on the phone reads, listed and read from {@code adb shell}
 * while the person allows it in Settings ▸ Devices ▸ this phone's card ▸
 * Diagnosis. tools/android-files.sh wraps the two commands:
 * {@code content query --uri content://com.aurelienpierre.sioul.debug/state/sioul}
 * lists a folder, {@code content read --uri …/state/sioul/extensions/x.log}
 * writes a file out.
 * <ul>
 * <li>The switch is Android's own state of this provider (the manifest has it
 * off, {@code android:enabled="false"}): while off, Android knows no such
 * provider and {@code content} finds nothing to ask. It is this phone's
 * alone, never in Sioul's files nor in the sharing, kept through an update
 * and gone with Sioul.</li>
 * <li>Only the shell's uid is answered ({@link #shell}), and only a caller
 * holding {@code android.permission.DUMP} reaches the provider at all (the
 * manifest's {@code android:permission}: the shell holds it, no app a person
 * installs can): an app of the phone, the system included, is refused,
 * whatever the address. The shell's uid is adb's: a computer the person
 * allowed to debug the phone (USB or wireless debugging), and also, on the
 * phone itself, an adb client through wireless debugging or an app given
 * adb's access, such as Shizuku. While the switch is on, each of them reads
 * what is below.</li>
 * <li>Read only: a folder listed ({@link #query}), a file opened to read
 * ({@link #openFile}, mode "r" alone); nothing written, nothing removed.</li>
 * <li>Four roots ({@link #ROOTS}): config, data and state (Sioul's XDG folders,
 * android/main.cpp, where the Java side keeps its own files too), and cache,
 * Sioul's own folder of the cache. Nothing outside them: not the rest of
 * the files folder (the notes, papers and letters there, the doses'
 * record), not the web views' caches, cookies and storage, not
 * shared_prefs, where the keyring keeps its sealed entries; no path
 * climbing out, no link leading out ({@link #resolve}).</li>
 * <li>Never, neither listed nor read, however named ({@link #never}): the
 * secret parts of the person's OpenPGP keys ({@link #NEVER}); the home
 * card's file, which holds valid one-time codes, temporary passwords and
 * sign-in links, and the doses other devices said and what the sharing
 * holds of other devices' records ({@link #NEVER_NAMED}); every health
 * record ({@link #HEALTH}). Passwords and tokens are in the keyring, sealed
 * by a key that never leaves Android's KeyStore.</li>
 * </ul>
 * In a process of its own ({@code :files}), which loads neither Qt nor Sioul's
 * library: a listing never starts the window. Sioul logs nothing it is asked;
 * Android logs a refusal's message (its DatabaseUtils, at the binder's edge),
 * which says at most the path asked.
 */
public final class UsbFiles extends ContentProvider
{
    /** Its authority, as android/package/AndroidManifest.xml names it ({@code ${applicationId}.debug}). */
    static final String AUTHORITY = ".debug";
    /** The shell's uid in each user (Android's {@code Process.SHELL_UID}, hidden from apps): adb's shell runs as it. */
    static final int SHELL_APP_ID = 2000;
    /** How many uids each user of the phone has (Android's {@code UserHandle.PER_USER_RANGE}). */
    static final int PER_USER_RANGE = 100000;
    /** The roots an address starts with. */
    static final List<String> ROOTS = List.of("config", "data", "state", "cache");
    /**
     * Never listed nor read, from the files folder, with everything in it: the
     * secret parts of the person's OpenPGP keys, encrypted with a passphrase
     * the keyring keeps (crates/sioul-core/src/pgp.rs, {@code <data>/pgp/own}).
     */
    static final List<String> NEVER = List.of("data/sioul/pgp/own");
    /**
     * Never listed nor read: a name in a folder of the files folder, as it
     * starts once its leading dots are taken off, so that what is written
     * beside it ({@code .new}) and its witness ({@code .<name>.written}) go
     * with it. The home card's file, which holds valid one-time codes,
     * temporary passwords, reset and sign-in links, and today's doses
     * (crates/sioul-app/src/homecard.rs); the doses other devices said
     * (crates/sioul-app/src/health.rs, {@code peers.json}); what the sharing
     * holds of other devices' records not written yet, health's among them
     * ({@code memory.json}).
     */
    static final List<List<String>> NEVER_NAMED = List.of(
            List.of("state/sioul", "home-card.json"),
            List.of("state/sioul/share", "peers.json"),
            List.of("state/sioul/share", "memory.json"));
    /**
     * Health records, never listed nor read: in {@link #HEALTH_FOLDERS}, at
     * any depth, a name that starts with this once its leading dots are taken
     * off (health.toml, health-doses.toml and the others, their {@code .new},
     * their witnesses, the sharing's earlier versions in share/history/health).
     */
    static final String HEALTH = "health";
    /** Where health records are kept, from the files folder. */
    static final List<String> HEALTH_FOLDERS = List.of("data/sioul", "state/sioul");
    /** A listing's columns: what each entry is, its bytes, when it last changed, its name ("." for the one asked). */
    static final String[] COLUMNS = {"kind", "size", "modified", "name"};

    // ---------------------------------------------------------------- the switch

    /** Its component, as Android knows it. */
    private static ComponentName component(Context context)
    {
        return new ComponentName(context, UsbFiles.class);
    }

    /** Whether the person turned it on: Android's state of the component, off unless turned on. */
    static boolean on(Context context)
    {
        try {
            return context.getPackageManager().getComponentEnabledSetting(component(context)) == PackageManager.COMPONENT_ENABLED_STATE_ENABLED;
        } catch (RuntimeException e) {
            return false;
        }
    }

    /** Turned on or off from Settings ▸ Devices; Sioul's processes left running. Its state then. */
    static boolean set(Context context, boolean on)
    {
        context.getPackageManager().setComponentEnabledSetting(component(context),
                on ? PackageManager.COMPONENT_ENABLED_STATE_ENABLED : PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                PackageManager.DONT_KILL_APP);
        return on(context);
    }

    /**
     * Rust's questions (StepService.call, crates/sioul-app/src/phonecard.rs):
     * "files-usb", whether it is on; "files-usb-set" with {@code {"on": bool}},
     * its state after. Null for another verb.
     */
    static String call(Context context, String verb, JSONObject asked)
    {
        switch (verb) {
        case "files-usb":
            return on(context) ? "true" : "false";
        case "files-usb-set":
            return set(context, asked.optBoolean("on", false)) ? "true" : "false";
        default:
            return null;
        }
    }

    // ---------------------------------------------------------------- what is answered, pure

    /** Whether a uid is adb's shell's, in whichever user of the phone. */
    static boolean shell(int uid)
    {
        return uid >= 0 && uid % PER_USER_RANGE == SHELL_APP_ID;
    }

    /**
     * The folder a root names: config, data and state in the app's files
     * folder ({@code files}), cache Sioul's own folder in the app's cache
     * ({@code cache}); null for another name.
     */
    static File rootOf(String name, File files, File cache)
    {
        switch (name) {
        case "config":
        case "data":
        case "state":
            return new File(files, name);
        case "cache":
            return new File(cache, "sioul");
        default:
            return null;
        }
    }

    /** A path as the system resolves it (links followed), or null when it cannot say. */
    private static String real(File file)
    {
        try {
            return file.getCanonicalPath();
        } catch (IOException | RuntimeException e) {
            return null;
        }
    }

    /** Whether {@code path} is {@code folder} or inside it, both resolved. */
    private static boolean within(String path, String folder)
    {
        return path.equals(folder) || path.startsWith(folder.endsWith("/") ? folder : folder + "/");
    }

    /** A name without its leading dots (a witness, {@code .health.toml.written}). */
    static String bare(String name)
    {
        int at = 0;
        while (at < name.length() && name.charAt(at) == '.')
            at++;
        return name.substring(at);
    }

    /**
     * Whether a resolved path is one never given: in a folder of {@link #NEVER},
     * a name of {@link #NEVER_NAMED} or below it, a health record
     * ({@link #HEALTH}). Each folder is resolved too, and one that cannot be
     * resolved refuses everything.
     */
    static boolean never(String path, File files)
    {
        for (String secret : NEVER) {
            String folder = real(new File(files, secret));
            if (folder == null || within(path, folder))
                return true;
        }
        for (List<String> named : NEVER_NAMED) {
            String folder = real(new File(files, named.get(0)));
            if (folder == null)
                return true;
            if (path.startsWith(folder + "/") && bare(path.substring(folder.length() + 1).split("/", 2)[0]).startsWith(named.get(1)))
                return true;
        }
        for (String kept : HEALTH_FOLDERS) {
            String folder = real(new File(files, kept));
            if (folder == null)
                return true;
            if (!path.startsWith(folder + "/"))
                continue;
            for (String segment : path.substring(folder.length() + 1).split("/")) {
                if (bare(segment).startsWith(HEALTH))
                    return true;
            }
        }
        return false;
    }

    /**
     * The file or folder an address's segments name in Sioul's folders, as
     * the system resolves it (links followed), or null when it is not one to
     * give: no segment, a first one that is not a root ({@link #ROOTS}), an
     * empty, "." or ".." segment, one holding "/" or a NUL (an address's
     * %2F or %00, decoded), a path that resolves out of its root (a link
     * leading out), one never given ({@link #never}). It may not exist.
     */
    static File resolve(List<String> segments, File files, File cache)
    {
        if (segments == null || segments.isEmpty())
            return null;
        File root = rootOf(segments.get(0), files, cache);
        if (root == null)
            return null;
        File file = root;
        for (String segment : segments.subList(1, segments.size())) {
            if (segment == null || segment.isEmpty() || segment.equals(".") || segment.equals("..") || segment.indexOf('/') >= 0 || segment.indexOf('\0') >= 0)
                return null;
            file = new File(file, segment);
        }
        String path = real(file);
        String top = real(root);
        if (path == null || top == null || !within(path, top) || never(path, files))
            return null;
        return new File(path);
    }

    /** What an entry is: "link" (never followed in a listing), "folder", "file", or "other". */
    static String kind(File file)
    {
        if (Files.isSymbolicLink(file.toPath()))
            return "link";
        return file.isDirectory() ? "folder" : file.isFile() ? "file" : "other";
    }

    /** A listing's row for an entry, named {@code name}: its bytes for a file alone, its last change in UTC to the second. */
    static Object[] row(File file, String name)
    {
        String kind = kind(file);
        long changed = file.lastModified();
        return new Object[] {kind, kind.equals("file") ? Long.valueOf(file.length()) : null, changed > 0 ? Instant.ofEpochMilli(changed).truncatedTo(ChronoUnit.SECONDS).toString() : null, name};
    }

    /**
     * A listing ({@link #COLUMNS}): the one asked first, named "."; then, for a
     * folder, each entry in it by name, but those never given. No segment:
     * the roots. Refused with why when the address names nothing to give.
     */
    static List<Object[]> listing(List<String> segments, File files, File cache)
    {
        List<Object[]> rows = new ArrayList<>();
        if (segments == null || segments.isEmpty()) {
            rows.add(new Object[] {"folder", null, null, "."});
            for (String root : ROOTS)
                rows.add(row(rootOf(root, files, cache), root));
            return rows;
        }
        File asked = resolve(segments, files, cache);
        if (asked == null || !asked.exists())
            throw new IllegalArgumentException("Not one of Sioul's files that can be read: " + String.join("/", segments));
        rows.add(row(asked, "."));
        String[] names = asked.isDirectory() ? asked.list() : null;
        if (names == null)
            return rows;
        Arrays.sort(names);
        String path = asked.getPath();
        for (String name : names) {
            File entry = new File(asked, name);
            // An entry never given is left out (a secret, a health record); a link is listed, and followed only when read.
            if (never(path + "/" + name, files))
                continue;
            rows.add(row(entry, name));
        }
        return rows;
    }

    // ---------------------------------------------------------------- the provider

    /** Refused with why, unless adb's shell asks while the person allows it. */
    private void allowed()
    {
        if (!shell(Binder.getCallingUid()))
            throw new SecurityException("Only adb's shell reads Sioul's files.");
        Context context = getContext();
        if (context == null || !on(context))
            throw new SecurityException("Reading Sioul's files over USB is off on this phone (Settings, Devices, this phone's card, Diagnosis).");
    }

    @Override
    public boolean onCreate()
    {
        return true;
    }

    @Override
    public Cursor query(Uri uri, String[] projection, String selection, String[] args, String order)
    {
        allowed();
        Context context = getContext();
        MatrixCursor cursor = new MatrixCursor(COLUMNS);
        for (Object[] row : listing(uri.getPathSegments(), context.getFilesDir(), context.getCacheDir()))
            cursor.addRow(row);
        return cursor;
    }

    @Override
    public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException
    {
        allowed();
        if (!"r".equals(mode))
            throw new SecurityException("Sioul's files are read only over USB.");
        Context context = getContext();
        File file = resolve(uri.getPathSegments(), context.getFilesDir(), context.getCacheDir());
        if (file == null || !file.isFile())
            throw new FileNotFoundException("Not one of Sioul's files that can be read: " + uri.getPath());
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY);
    }

    /** Said to nobody: any app may ask a provider's types, unchecked. */
    @Override
    public String getType(Uri uri)
    {
        return null;
    }

    @Override
    public Uri insert(Uri uri, ContentValues values)
    {
        throw new UnsupportedOperationException("Sioul's files are read only over USB.");
    }

    @Override
    public int delete(Uri uri, String selection, String[] args)
    {
        throw new UnsupportedOperationException("Sioul's files are read only over USB.");
    }

    @Override
    public int update(Uri uri, ContentValues values, String selection, String[] args)
    {
        throw new UnsupportedOperationException("Sioul's files are read only over USB.");
    }
}
