// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.Manifest;
import android.app.role.RoleManager;
import android.content.ActivityNotFoundException;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.provider.ContactsContract;
import android.provider.Settings;
import android.telecom.Call;
import android.telecom.Connection;
import android.telecom.TelecomManager;
import android.telephony.TelephonyManager;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Calls screened (docs/android.md, "Calls"; docs/research/call-screening.md,
 * section 8). Android asks Sioul about each call (CallScreen, as the "Caller
 * ID &amp; spam app"); the answer comes from a table Sioul's Rust side wrote
 * ahead (crates/sioul-app/src/calls.rs, `calls/table.json` in its state
 * folder), read here in milliseconds, without Sioul's library or Qt. A call
 * is let ring, or refused plainly: the network sends it to the operator's
 * voicemail, as a call declined by hand; it stays in the phone's own call
 * history, and in Sioul's list of calls held (`held.jsonl`), which Rust reads.
 *
 * The order (research 8.3): the floors first, which always ring (an
 * emergency number or the emergency services' callback number; any call for
 * a day after you call an emergency number; the people on your
 * do-not-disturb list); then your blocked numbers, refused; then "Let every
 * call through"; then a second call from the same number within 15 minutes,
 * which rings; then who calls at this time, as the table's frame for now
 * says. Nothing to decide from (no table, a table past its frames, anything
 * that fails): it rings, as Android would.
 *
 * Here too: Rust's questions (StepService.call, verbs "calls-…"), and the
 * press of "Let every call through" on the background service's notification,
 * kept in a file of this phone's (`through-here.json`) that Rust reads and
 * shares with your other devices.
 */
final class Calls
{
    static final String TAG = DoseAlarms.TAG;
    /** Rust's state folder (android/main.cpp: XDG_STATE_HOME is files/state), and the calls' own in it. */
    private static final String FOLDER = "state/sioul/calls";
    static final String TABLE = "table.json";
    static final String HELD = "held.jsonl";
    static final String THROUGH = "through-here.json";
    private static final String RECENT = "recent.json";
    private static final String EMERGENCY = "emergency.json";
    /** The table's shape this code reads; another one is not read (the calls ring). */
    private static final int VERSION = 1;
    /** Calls held are kept this long in the list's file. */
    private static final long KEPT_MS = 30L * 24 * 3600 * 1000;
    /** Past this size the list's file is rewritten without its old lines. */
    private static final long TRIM_BYTES = 128 * 1024;

    private Calls() {}

    // ---------------------------------------------------------------- the table

    /** How the table's country writes its numbers (phones.rs's Region). */
    static final class Region
    {
        String calling = "";
        String trunk = "";
        String international = "00";
        int fewest;
        int most;
        boolean french;
        /** The French plan's overseas numbers: the three digits after the 0 → their country code. */
        final Map<String, String> overseas = new HashMap<>();
    }

    /** One stretch of time and who rings in it. */
    static final class Frame
    {
        long from;
        long until;
        /** "work", "admin", "leisure", "meals", "sleep", "pause". */
        String column = "";
        /** A row ("safe", "neutral", "restricted", "stranger", "hidden") → rings. */
        final Map<String, Boolean> ring = new HashMap<>();
    }

    /** A press of "Let every call through": when (ms), on or off, until (ms; 0: until turned off). */
    static final class Press
    {
        long pressed;
        boolean on;
        long until;

        static Press of(JSONObject json)
        {
            if (json == null)
                return null;
            Press press = new Press();
            press.pressed = json.optLong("pressed", 0);
            press.on = json.optBoolean("on", false);
            press.until = json.optLong("until", 0);
            return press;
        }

        boolean holds(long now)
        {
            return on && (until <= 0 || now < until);
        }
    }

    /** The table as Rust wrote it (crates/sioul-core/src/calls.rs, `Table`). */
    static final class Table
    {
        long made;
        Region region;
        /** Country codes whose numbers drop a written trunk "0" ("+33 06…"). */
        final Set<String> trunkZero = new HashSet<>();
        /** Number keys → who: "safe", "neutral", "restricted", "blocked". */
        final Map<String, String> numbers = new HashMap<>();
        /** The lists' prefixes ("+3346571") and who they make a number, the longest first. */
        final List<String[]> prefixes = new ArrayList<>();
        /** Who a number in this phone's contacts is when the table does not know it. */
        String phoneContacts = "neutral";
        /** Numbers that always ring: emergency numbers and callbacks; the do-not-disturb list's people. */
        final Set<String> emergency = new HashSet<>();
        final Set<String> people = new HashSet<>();
        Press through;
        final List<Frame> frames = new ArrayList<>();
        long repeatMs = 15 * 60 * 1000L;
        long emergencyMs = 24 * 3600 * 1000L;

        static Table of(JSONObject json) throws JSONException
        {
            if (json.optInt("v", 0) != VERSION)
                throw new JSONException("a table of another version: " + json.optInt("v", 0));
            Table table = new Table();
            table.made = json.optLong("made", 0);
            JSONObject region = json.optJSONObject("region");
            if (region != null) {
                Region r = new Region();
                r.calling = region.optString("calling", "");
                r.trunk = region.optString("trunk", "");
                r.international = region.optString("international", "00");
                JSONArray digits = region.optJSONArray("digits");
                r.fewest = digits == null ? 0 : digits.optInt(0, 0);
                r.most = digits == null ? 0 : digits.optInt(1, 0);
                r.french = region.optBoolean("french", false);
                JSONObject overseas = region.optJSONObject("overseas");
                if (overseas != null) {
                    for (Iterator<String> keys = overseas.keys(); keys.hasNext();) {
                        String prefix = keys.next();
                        r.overseas.put(prefix, overseas.optString(prefix, ""));
                    }
                }
                table.region = r;
            }
            strings(json.optJSONArray("trunk_zero"), table.trunkZero);
            JSONObject numbers = json.optJSONObject("numbers");
            if (numbers != null) {
                for (Iterator<String> keys = numbers.keys(); keys.hasNext();) {
                    String key = keys.next();
                    table.numbers.put(key, numbers.optString(key, ""));
                }
            }
            JSONArray prefixes = json.optJSONArray("prefixes");
            for (int i = 0; prefixes != null && i < prefixes.length(); i++) {
                JSONObject prefix = prefixes.getJSONObject(i);
                String start = prefix.optString("prefix", "");
                String who = prefix.optString("who", "");
                if (!start.isEmpty() && !who.isEmpty())
                    table.prefixes.add(new String[] { start, who });
            }
            table.phoneContacts = json.optString("phone_contacts", "neutral");
            JSONObject floors = json.optJSONObject("floors");
            if (floors != null) {
                strings(floors.optJSONArray("emergency"), table.emergency);
                strings(floors.optJSONArray("people"), table.people);
            }
            table.through = Press.of(json.optJSONObject("through"));
            JSONArray frames = json.optJSONArray("frames");
            for (int i = 0; frames != null && i < frames.length(); i++) {
                JSONObject f = frames.getJSONObject(i);
                Frame frame = new Frame();
                frame.from = f.optLong("from", 0);
                frame.until = f.optLong("until", 0);
                frame.column = f.optString("column", "");
                JSONObject ring = f.optJSONObject("ring");
                if (ring != null) {
                    for (Iterator<String> rows = ring.keys(); rows.hasNext();) {
                        String row = rows.next();
                        frame.ring.put(row, ring.optBoolean(row, true));
                    }
                }
                table.frames.add(frame);
            }
            table.repeatMs = json.optLong("repeat_minutes", 15) * 60 * 1000L;
            table.emergencyMs = json.optLong("emergency_hours", 24) * 3600 * 1000L;
            return table;
        }

        /** Who a prefix on your lists makes this number; null when none fits. */
        String byPrefix(String key)
        {
            for (String[] prefix : prefixes)
                if (key.startsWith(prefix[0]))
                    return prefix[1];
            return null;
        }

        /** The frame `now` falls in; none before the first or past the last. */
        Frame frameAt(long now)
        {
            for (Frame frame : frames)
                if (frame.from <= now && now < frame.until)
                    return frame;
            return null;
        }

        private static void strings(JSONArray array, Set<String> into)
        {
            for (int i = 0; array != null && i < array.length(); i++) {
                String value = array.optString(i, "");
                if (!value.isEmpty())
                    into.add(value);
            }
        }
    }

    private static Table cached;
    private static long cachedStamp = -1;
    private static long cachedSize = -1;

    static File folder(Context context)
    {
        return new File(context.getFilesDir(), FOLDER);
    }

    /** The table, read again only when its file changed; none when there is none or it does not read. */
    static synchronized Table table(Context context)
    {
        File file = new File(folder(context), TABLE);
        long stamp = file.lastModified();
        long size = file.length();
        if (stamp == 0) {
            cached = null;
            return null;
        }
        if (cached != null && stamp == cachedStamp && size == cachedSize)
            return cached;
        try {
            cached = Table.of(new JSONObject(read(file)));
            cachedStamp = stamp;
            cachedSize = size;
            return cached;
        } catch (IOException | JSONException | RuntimeException e) {
            Log.w(TAG, "Calls: the table does not read; calls ring. " + e);
            cached = null;
            return null;
        }
    }

    // ---------------------------------------------------------------- numbers

    /**
     * A number as the table keys it: phones.rs's `key` for what a caller ID
     * holds (crates/sioul-core/src/calls.rs, `incoming_key`, does the same, and
     * its tests check the two agree): "+33199001234"; a short code as written
     * ("112", "3114"); a value that is not a number, as written without spaces.
     */
    static String key(String raw, Region region, Set<String> trunkZero)
    {
        String value = trimmed(raw == null ? "" : raw);
        if (value.length() >= 4 && value.substring(0, 4).equalsIgnoreCase("tel:"))
            value = value.substring(4).trim();
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            if (c == '+' && i == 0)
                out.append('+');
            else if (c >= '0' && c <= '9')
                out.append(c);
            else if (c == '(' || c == ')' || spacing(c))
                continue;
            else
                return asWritten(value);
        }
        String digits = out.toString();
        if (digits.isEmpty() || digits.equals("+"))
            return asWritten(value);
        if (digits.startsWith("+"))
            return "+" + withoutWrittenTrunk(digits.substring(1), trunkZero);
        if (region == null)
            return digits;
        for (String prefix : new String[] { region.international, "00" }) {
            if (prefix.isEmpty() || !digits.startsWith(prefix))
                continue;
            String rest = digits.substring(prefix.length());
            if (rest.length() >= 6 && !rest.startsWith("0"))
                return "+" + withoutWrittenTrunk(rest, trunkZero);
        }
        int length = digits.length();
        if (length < region.fewest || length > region.most)
            return digits;
        if (region.trunk.isEmpty())
            return "+" + region.calling + digits;
        if (region.trunk.equals("1")) {
            if (length == 10 && digits.charAt(0) != '0' && digits.charAt(0) != '1')
                return "+1" + digits;
            if (length == 11 && digits.startsWith("1"))
                return "+" + digits;
            return digits;
        }
        if (digits.startsWith(region.trunk)) {
            String national = digits.substring(region.trunk.length());
            if (!national.isEmpty() && !national.startsWith("0")) {
                String calling = region.calling;
                if (region.french && national.length() >= 3) {
                    String overseas = region.overseas.get(national.substring(0, 3));
                    if (overseas != null && !overseas.isEmpty())
                        calling = overseas;
                }
                return "+" + calling + national;
            }
        }
        return digits;
    }

    /** "+330199…" → "+33199…": a trunk 0 written after a country code that has none. */
    private static String withoutWrittenTrunk(String digits, Set<String> trunkZero)
    {
        for (int length = 1; length <= 3 && length < digits.length(); length++) {
            String calling = digits.substring(0, length);
            String rest = digits.substring(length);
            if (rest.startsWith("0") && trunkZero.contains(calling))
                return calling + rest.substring(1);
        }
        return digits;
    }

    private static boolean spacing(char c)
    {
        switch (c) {
        case ' ': case '\t': case '.': case '-': case '/':
            return true;
        default:
            // No-break, narrow, thin and figure spaces, hyphens and dashes, the
            // marks of writing direction phones put around numbers, a BOM.
            return c == 0x00a0 || c == 0x202f || c == 0x2009 || c == 0x2007
                || (c >= 0x2010 && c <= 0x2014)
                || c == 0x200e || c == 0x200f || (c >= 0x202a && c <= 0x202e)
                || (c >= 0x2066 && c <= 0x2069) || c == 0xfeff;
        }
    }

    /** Without the spaces and marks around it (phones.rs: `trim_matches` of both). */
    private static String trimmed(String value)
    {
        int start = 0;
        int end = value.length();
        while (start < end && (Character.isWhitespace(value.charAt(start)) || spacing(value.charAt(start))))
            start++;
        while (end > start && (Character.isWhitespace(value.charAt(end - 1)) || spacing(value.charAt(end - 1))))
            end--;
        return value.substring(start, end);
    }

    private static String asWritten(String value)
    {
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < value.length(); i++)
            if (!Character.isWhitespace(value.charAt(i)))
                out.append(value.charAt(i));
        return out.toString().toLowerCase(java.util.Locale.ROOT);
    }

    /** An emergency number on Android's own list (the SIM's, the network's); false when it cannot say. */
    static boolean systemEmergency(Context context, String raw)
    {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q || raw == null || raw.isEmpty())
            return false;
        try {
            TelephonyManager phone = context.getSystemService(TelephonyManager.class);
            return phone != null && phone.isEmergencyNumber(raw);
        } catch (RuntimeException e) {
            // No telephony here, or not allowed: the table's numbers only.
            return false;
        }
    }

    /** The name this phone's contacts give a number; null when none has it (or contacts are not readable). */
    static String phoneContact(Context context, String raw)
    {
        if (raw == null || raw.isEmpty() || context.checkSelfPermission(Manifest.permission.READ_CONTACTS) != PackageManager.PERMISSION_GRANTED)
            return null;
        Uri lookup = Uri.withAppendedPath(ContactsContract.PhoneLookup.CONTENT_FILTER_URI, Uri.encode(raw));
        String[] columns = { ContactsContract.PhoneLookup.DISPLAY_NAME };
        try (Cursor rows = context.getContentResolver().query(lookup, columns, null, null, null)) {
            if (rows != null && rows.moveToFirst()) {
                String name = rows.getString(0);
                return name == null ? "" : name;
            }
        } catch (RuntimeException e) {
            Log.w(TAG, "Calls: a number not looked up in the contacts: " + e);
        }
        return null;
    }

    // ---------------------------------------------------------------- the decision

    /** What Sioul answers, and why: for the response, the list of calls held, and the log. */
    static final class Decision
    {
        boolean refuse;
        /** "emergency", "after-emergency", "through", "people", "blocked", "repeat", "matrix", "no-table", "no-frame", "error", "outgoing". */
        String why = "";
        long at;
        String key = "";
        String raw = "";
        boolean hidden;
        int presentation;
        /** The row: "safe", "neutral", "restricted", "stranger", "hidden", "blocked". */
        String who = "";
        /** The name this phone's contacts give the number, when the table did not know it. */
        String name = "";
        String column = "";
        /** "passed", "failed", "" (not verified, or not known). */
        String verified = "";

        static Decision ring(String why)
        {
            Decision decision = new Decision();
            decision.why = why;
            return decision;
        }
    }

    /** What the decision asks of the phone beyond the table: Android's emergency list, its contacts. */
    interface Phone
    {
        boolean emergency(String raw);

        /** The phone's contacts' name for the number; null when none has it. */
        String contact(String raw);
    }

    /** Sioul's answer about a call (CallScreen), in milliseconds. */
    static Decision decide(final Context context, Call.Details details, long now)
    {
        Uri handle = details.getHandle();
        String raw = handle == null ? "" : handle.getSchemeSpecificPart();
        if (raw == null)
            raw = "";
        int presentation = details.getHandlePresentation();
        boolean outgoing = Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q && details.getCallDirection() == Call.Details.DIRECTION_OUTGOING;
        boolean hidden = presentation != TelecomManager.PRESENTATION_ALLOWED || !hasDigit(raw);
        Table table = table(context);
        Phone phone = new Phone() {
            @Override
            public boolean emergency(String number)
            {
                return systemEmergency(context, number);
            }

            @Override
            public String contact(String number)
            {
                return phoneContact(context, number);
            }
        };
        // You call an emergency number: every call rings for a day (Apple's rule; research 7.1).
        if (outgoing) {
            String key = table == null ? "" : key(raw, table.region, table.trunkZero);
            if (!hidden && (phone.emergency(raw) || (table != null && table.emergency.contains(key))))
                rememberEmergency(context, now);
            return Decision.ring("outgoing");
        }
        String key = hidden || table == null ? "" : key(raw, table.region, table.trunkZero);
        // A second call within 15 minutes: the last one known before this one, this one remembered.
        long last = hidden ? 0 : rememberCall(context, key.isEmpty() ? raw : key, now);
        Press local = Press.of(readJson(new File(folder(context), THROUGH)));
        Decision decision = decideFor(table, raw, hidden, now, emergencyAt(context), local, last, phone);
        decision.presentation = presentation;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            int verified = details.getCallerNumberVerificationStatus();
            decision.verified = verified == Connection.VERIFICATION_STATUS_PASSED ? "passed" : verified == Connection.VERIFICATION_STATUS_FAILED ? "failed" : "";
        }
        return decision;
    }

    /**
     * The decision itself, from what the phone knows (crates/sioul-core/src/
     * calls.rs, `decide`, takes the same steps, and its tests the same
     * cases): the floors; blocked; a second call; the frame for now.
     */
    static Decision decideFor(Table table, String raw, boolean hidden, long now, long emergencyAt, Press local, long lastCall, Phone phone)
    {
        Decision decision = Decision.ring("");
        decision.at = now;
        decision.raw = hidden ? "" : raw;
        decision.hidden = hidden;
        if (table == null)
            return said(decision, false, "no-table");
        String key = hidden ? "" : key(raw, table.region, table.trunkZero);
        decision.key = key;

        // 1. What always rings.
        if (!hidden && (table.emergency.contains(key) || phone.emergency(raw)))
            return said(decision, false, "emergency");
        if (emergencyAt > 0 && now >= emergencyAt && now - emergencyAt < table.emergencyMs)
            return said(decision, false, "after-emergency");
        if (!hidden && table.people.contains(key))
            return said(decision, false, "people");

        // Who calls: the table's number, else a prefix on your lists, else this phone's
        // contacts' (Android's own, research CS6), else nobody known.
        String who = hidden ? "hidden" : table.numbers.get(key);
        if (!hidden && (who == null || who.isEmpty()))
            who = table.byPrefix(key);
        if (who == null || who.isEmpty()) {
            String name = phone.contact(raw);
            if (name != null) {
                who = table.phoneContacts;
                decision.name = name;
            } else {
                who = "stranger";
            }
        }
        decision.who = who;

        // 2. Blocked: refused, whenever and however often they call, every call let through or not.
        if ("blocked".equals(who))
            return said(decision, true, "blocked");
        // 3. Let every call through: the later press of this phone's and the table's (all devices').
        Press latest = local != null && (table.through == null || local.pressed > table.through.pressed) ? local : table.through;
        if (latest != null && latest.holds(now))
            return said(decision, false, "through");
        // 4. A second call within 15 minutes rings (hidden numbers cannot be told apart: not them).
        if (!hidden && lastCall > 0 && now - lastCall <= table.repeatMs)
            return said(decision, false, "repeat");
        // 5. Who rings now, as the frame for now says; past the frames, nothing to decide from.
        Frame frame = table.frameAt(now);
        if (frame == null)
            return said(decision, false, "no-frame");
        decision.column = frame.column;
        Boolean rings = frame.ring.get(who);
        if (rings == null)
            return said(decision, false, "no-row");
        return said(decision, !rings, "matrix");
    }

    private static Decision said(Decision decision, boolean refuse, String why)
    {
        decision.refuse = refuse;
        decision.why = why;
        return decision;
    }

    private static boolean hasDigit(String text)
    {
        for (int i = 0; i < text.length(); i++)
            if (Character.isDigit(text.charAt(i)))
                return true;
        return false;
    }

    // ---------------------------------------------------------------- "Let every call through"

    /**
     * "Let every call through" pressed on this phone (the notification): on
     * for `minutes` (0: until turned off), or off. Kept for the screening at
     * once; Rust shares it with your other devices at its next step.
     */
    static void press(Context context, boolean on, int minutes)
    {
        long now = System.currentTimeMillis();
        Table table = table(context);
        Press here = Press.of(readJson(new File(folder(context), THROUGH)));
        // Never before a press already known: the latest wins, everywhere.
        long stamp = now;
        if (here != null)
            stamp = Math.max(stamp, here.pressed + 1);
        if (table != null && table.through != null)
            stamp = Math.max(stamp, table.through.pressed + 1);
        JSONObject press = new JSONObject();
        try {
            press.put("pressed", stamp);
            press.put("on", on);
            press.put("until", on && minutes > 0 ? now + minutes * 60_000L : 0);
        } catch (JSONException e) {
            return;
        }
        writeWhole(new File(folder(context), THROUGH), press.toString());
        Log.i(TAG, "Calls: every call " + (on ? "rings" + (minutes > 0 ? " for " + minutes + " min." : " until turned off.") : "screened again."));
    }

    // ---------------------------------------------------------------- what Sioul remembers here

    /** The number's last call before this one (ms, 0 none); this one remembered. Calls older than an hour forgotten. */
    private static synchronized long rememberCall(Context context, String key, long now)
    {
        if (key.isEmpty())
            return 0;
        File file = new File(folder(context), RECENT);
        JSONObject recent = readJson(file);
        if (recent == null)
            recent = new JSONObject();
        long last = recent.optLong(key, 0);
        JSONObject kept = new JSONObject();
        try {
            for (Iterator<String> keys = recent.keys(); keys.hasNext();) {
                String other = keys.next();
                long at = recent.optLong(other, 0);
                if (now - at < 3600_000L && at <= now)
                    kept.put(other, at);
            }
            kept.put(key, now);
        } catch (JSONException e) {
            return last;
        }
        writeWhole(file, kept.toString());
        return last;
    }

    private static void rememberEmergency(Context context, long now)
    {
        JSONObject at = new JSONObject();
        try {
            at.put("at", now);
        } catch (JSONException e) {
            return;
        }
        writeWhole(new File(folder(context), EMERGENCY), at.toString());
        Log.i(TAG, "Calls: an emergency number called; every call rings for a day.");
    }

    /** When you last called an emergency number (ms), as this phone saw it; 0 never. */
    static long emergencyAt(Context context)
    {
        JSONObject at = readJson(new File(folder(context), EMERGENCY));
        return at == null ? 0 : at.optLong("at", 0);
    }

    /** A call refused, in the list Rust reads (one JSON line each); lines older than a month dropped now and then. */
    static synchronized void held(Context context, Decision decision)
    {
        JSONObject line = new JSONObject();
        try {
            line.put("at", decision.at);
            line.put("key", decision.key);
            line.put("number", decision.raw);
            line.put("hidden", decision.hidden);
            line.put("presentation", decision.presentation);
            line.put("who", decision.who);
            line.put("name", decision.name);
            line.put("column", decision.column);
            line.put("why", decision.why);
            line.put("verified", decision.verified);
        } catch (JSONException e) {
            return;
        }
        File file = new File(folder(context), HELD);
        folder(context).mkdirs();
        if (file.length() > TRIM_BYTES)
            trim(file, decision.at);
        try (OutputStream out = new FileOutputStream(file, true)) {
            out.write((line.toString() + "\n").getBytes(StandardCharsets.UTF_8));
        } catch (IOException e) {
            Log.w(TAG, "Calls: a call held not written to its list: " + e);
        }
    }

    /** The list's file without its lines older than a month. */
    private static void trim(File file, long now)
    {
        try {
            StringBuilder kept = new StringBuilder();
            for (String line : read(file).split("\n")) {
                if (line.trim().isEmpty())
                    continue;
                try {
                    if (now - new JSONObject(line).optLong("at", 0) < KEPT_MS)
                        kept.append(line).append('\n');
                } catch (JSONException e) {
                    // A broken line: dropped.
                }
            }
            writeWhole(file, kept.toString());
        } catch (IOException e) {
            Log.w(TAG, "Calls: the list's old lines not dropped: " + e);
        }
    }

    // ---------------------------------------------------------------- Rust's questions

    /** Rust's question (StepService.call, verbs "calls-…"), from Sioul's own process or the service's. */
    static String call(Context context, String verb, JSONObject asked) throws JSONException
    {
        switch (verb) {
        case "calls-state":
            return state(context).toString();
        case "calls-ask-role":
            return start(context, new Intent(context, CallsSetup.class)) ? "true" : "false";
        case "calls-open-roles":
            return openRoles(context) ? "true" : "false";
        case "calls-open-blocked":
            return openBlocked(context) ? "true" : "false";
        case "calls-dial":
            // The phone app with a code typed in (`*#61#`), for you to press call: never called by Sioul.
            return start(context, new Intent(Intent.ACTION_DIAL, Uri.parse("tel:" + Uri.encode(asked.optString("number", ""))))) ? "true" : "false";
        default:
            return null;
        }
    }

    /**
     * What this phone allows: {api, available (the role exists here),
     * held (Sioul holds it), contacts (READ_CONTACTS), table (one is
     * written), emergency_at (ms)}.
     */
    static JSONObject state(Context context) throws JSONException
    {
        JSONObject state = new JSONObject();
        state.put("api", Build.VERSION.SDK_INT);
        boolean available = false;
        boolean held = false;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            RoleManager roles = context.getSystemService(RoleManager.class);
            if (roles != null) {
                available = roles.isRoleAvailable(RoleManager.ROLE_CALL_SCREENING);
                held = available && roles.isRoleHeld(RoleManager.ROLE_CALL_SCREENING);
            }
        }
        state.put("available", available);
        state.put("held", held);
        state.put("contacts", context.checkSelfPermission(Manifest.permission.READ_CONTACTS) == PackageManager.PERMISSION_GRANTED);
        state.put("table", new File(folder(context), TABLE).isFile());
        state.put("emergency_at", emergencyAt(context));
        return state;
    }

    /** Android's page of default apps, where "Caller ID &amp; spam app" is chosen or set to none. */
    private static boolean openRoles(Context context)
    {
        Intent[] pages = {
            new Intent(Settings.ACTION_MANAGE_DEFAULT_APPS_SETTINGS),
            new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:" + context.getPackageName())),
        };
        for (Intent page : pages)
            if (start(context, page))
                return true;
        return false;
    }

    /** Android's own blocked numbers (calls and texts, for every app): its page, for you to add one. */
    private static boolean openBlocked(Context context)
    {
        TelecomManager telecom = context.getSystemService(TelecomManager.class);
        Intent page = telecom == null ? null : telecom.createManageBlockedNumbersIntent();
        return page != null && start(context, page);
    }

    private static boolean start(Context context, Intent intent)
    {
        try {
            context.startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            return true;
        } catch (ActivityNotFoundException | SecurityException e) {
            Log.w(TAG, "Calls: no page for " + intent.getAction() + ": " + e);
            return false;
        }
    }

    // ---------------------------------------------------------------- files

    static String read(File file) throws IOException
    {
        try (InputStream in = new FileInputStream(file)) {
            byte[] bytes = new byte[(int) Math.min(file.length(), 4 * 1024 * 1024)];
            int read = 0;
            while (read < bytes.length) {
                int n = in.read(bytes, read, bytes.length - read);
                if (n < 0)
                    break;
                read += n;
            }
            return new String(bytes, 0, read, StandardCharsets.UTF_8);
        }
    }

    private static JSONObject readJson(File file)
    {
        if (!file.isFile())
            return null;
        try {
            return new JSONObject(read(file));
        } catch (IOException | JSONException e) {
            return null;
        }
    }

    /** Written beside, then renamed: never half a file. */
    private static void writeWhole(File file, String text)
    {
        File parent = file.getParentFile();
        if (parent != null)
            parent.mkdirs();
        File fresh = new File(parent, "." + file.getName() + ".new");
        try (OutputStream out = new FileOutputStream(fresh)) {
            out.write(text.getBytes(StandardCharsets.UTF_8));
        } catch (IOException e) {
            Log.w(TAG, "Calls: " + file.getName() + " not written: " + e);
            return;
        }
        if (!fresh.renameTo(file))
            Log.w(TAG, "Calls: " + file.getName() + " not put in place.");
    }
}
