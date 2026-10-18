// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.Manifest;
import android.content.BroadcastReceiver;
import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.PackageManager;
import android.database.ContentObserver;
import android.database.Cursor;
import android.net.Uri;
import android.os.BatteryManager;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.provider.Settings;
import android.provider.Telephony;
import android.telephony.SubscriptionInfo;
import android.telephony.SubscriptionManager;
import android.util.Log;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Texts read on the phone for your computers (docs/texts.md; Rust's side:
 * crates/sioul-app/src/texts.rs), without Sioul being the phone's SMS app:
 * the SMS app stays the record. With READ_SMS, Android shows another app its
 * inbox and its sent box only (the provider's restricted views): never the
 * drafts, the failed or queued texts, nor a multimedia message still to
 * download. Java reads, and hands Rust plain JSON; Rust keys the numbers,
 * seals the lines, and decides what travels. Java never writes a shared file.
 *
 * Asked through {@link StepService#call} ("texts-…", from either process): the
 * permissions and the SIMs, Android's question for the permissions
 * (TextsAsk), the rows Rust's reader asks for by their ids (the whole
 * history, every column, multimedia messages and their parts), a part's
 * content, every row there is with its date, the texts Sioul sent lately
 * (to find one after a crash),
 * and a text to send (TextSend), whether the phone's texts changed since the
 * last look and whether it charges (`changes`). Heard: a new text, which
 * brings a step soon. The background service watches Android's provider of
 * texts (`watch`): the import asks Android only when they changed, and a
 * message received (a multimedia one too, once downloaded) brings a step
 * soon, as a new text does.
 */
public final class Texts
{
    static final String TAG = DoseAlarms.TAG;
    /** Sioul's private texts folder in its state (Rust's `texts::private_folder`): Java's own files there. */
    static final String FOLDER = "state/sioul/texts";
    /** Android's provider: a text sent; received. */
    static final int SENT_BOX = 2;

    /** The phone's texts changed since Rust last asked with "clear" (`changes`): set at first, so that the first step looks. */
    private static final AtomicBoolean changed = new AtomicBoolean(true);
    /** Android's provider of texts watched in this process (the background service's, or the window's); null: not watched here. */
    private static ContentObserver watching;
    /** The newest message received, by the provider's id, of each table (SMS, MMS), as the watch last saw it; -1: not read yet. */
    private static long newestSms = -1;
    private static long newestMms = -1;
    /** How long after the provider's last word the watch looks for a message received: a burst of changes, one look. */
    private static final long LOOK_MS = 1_000;
    /** Where that look runs (the process's main thread, which the service leaves free), and the look itself; null: not watched. */
    private static Handler looks;
    private static Runnable look;

    private Texts()
    {
    }

    /**
     * Android's provider of texts watched in this process (the background
     * service's), once READ_SMS is given: a text received, sent, deleted or
     * read marks them changed, read at the next step; a message received (its
     * newest id grown, a multimedia one once downloaded) brings a step soon,
     * as SMS_RECEIVED does (`Heard`), so that it reaches your computers
     * within the minute. The look for it comes a second after the provider's
     * last word: one message tells it several times (its text, its row, its
     * conversation), one look. Asked again at each step until it holds.
     */
    static synchronized void watch(Context context)
    {
        watch(context, true);
    }

    /**
     * The same, `steps` saying whether a message received brings a step (the
     * background service's process); in the window's, which exchanges each
     * minute by itself, the watch only marks the texts changed, so that its
     * exchanges ask Android for every row only after a change (the review of
     * 16 October 2026, LOW 3: unwatched, it listed every row once a minute
     * while on screen).
     */
    static synchronized void watch(Context context, boolean steps)
    {
        if (watching != null || !allowed(context, Manifest.permission.READ_SMS))
            return;
        final Context app = context.getApplicationContext();
        final Handler later = steps ? new Handler(Looper.getMainLooper()) : null;
        final Runnable once = steps ? () -> {
            if (arrived(app))
                StepService.soon(app);
        } : null;
        ContentObserver observer = new ContentObserver(null) {
            @Override
            public void onChange(boolean self)
            {
                changed.set(true);
                if (later != null) {
                    later.removeCallbacks(once);
                    later.postDelayed(once, LOOK_MS);
                }
            }
        };
        try {
            ContentResolver resolver = context.getContentResolver();
            for (Uri uri : new Uri[] { Telephony.Sms.CONTENT_URI, Telephony.Mms.CONTENT_URI, Telephony.MmsSms.CONTENT_URI })
                resolver.registerContentObserver(uri, true, observer);
            watching = observer;
            looks = later;
            look = once;
            changed.set(true);
            // The messages there now: none of them brings a step.
            if (steps)
                arrived(app);
        } catch (RuntimeException e) {
            context.getContentResolver().unregisterContentObserver(observer);
            Log.w(TAG, "Texts: the texts cannot be watched: " + e.getClass().getSimpleName());
        }
    }

    /**
     * Whether a message was received since the watch last looked: the newest
     * id of either inbox grew (`grew`); the newest ones kept. Nothing of them
     * is read but their ids.
     */
    static boolean arrived(Context context)
    {
        long sms = newest(context, Telephony.Sms.Inbox.CONTENT_URI);
        long mms = newest(context, Telephony.Mms.Inbox.CONTENT_URI);
        synchronized (Texts.class) {
            boolean grown = grew(newestSms, sms) || grew(newestMms, mms);
            newestSms = Math.max(newestSms, sms);
            newestMms = Math.max(newestMms, mms);
            return grown;
        }
    }

    /**
     * Whether an inbox's newest id grew: read before (not -1), larger now (a
     * box that could not be read now says -1: no). Pure:
     * android/jvm-checks/StepsCheck.java.
     */
    static boolean grew(long before, long now)
    {
        return before >= 0 && now > before;
    }

    /**
     * The newest id in an inbox, 0 when it is empty, -1 when it cannot be
     * read (`newestBy`).
     */
    private static long newest(Context context, Uri box)
    {
        return newestBy(order -> {
            try (Cursor cursor = context.getContentResolver().query(box, new String[] { "_id" }, null, null, order)) {
                return cursor != null && cursor.moveToFirst() ? cursor.getLong(0) : 0;
            }
        });
    }

    /** An inbox's newest id asked in an order (`NEWEST`); a refusal throws. */
    interface Look
    {
        long newest(String order);
    }

    /**
     * How the newest row is asked for: the limit in the sort, since Android's
     * own SMS and MMS providers pass no ContentResolver.QUERY_ARG_LIMIT on and
     * would give every id each time; then, from a provider that refuses a
     * limit there, the plain sort, its ids in that order and only the first
     * read.
     */
    static final String[] NEWEST = { "_id DESC LIMIT 1", "_id DESC" };

    /**
     * The newest id as `look` gives it, asked in each order of NEWEST until
     * one is taken; -1 when none is (the inbox cannot be read). Pure:
     * android/jvm-checks/StepsCheck.java.
     */
    static long newestBy(Look look)
    {
        for (String order : NEWEST) {
            try {
                return look.newest(order);
            } catch (RuntimeException e) {
                // Refused: the next order.
            }
        }
        return -1;
    }

    /** No longer watched (the service ending). */
    static synchronized void unwatch(Context context)
    {
        if (watching != null)
            context.getContentResolver().unregisterContentObserver(watching);
        if (looks != null && look != null)
            looks.removeCallbacks(look);
        watching = null;
        looks = null;
        look = null;
        changed.set(true);
    }

    /** A new text heard (Heard): changed, whatever the provider said yet. */
    static void touched()
    {
        changed.set(true);
    }

    /**
     * Whether the texts changed since the last look: always when not watched
     * here (READ_SMS not given yet); else as marked,
     * the mark cleared when `clear`. Pure: android/jvm-checks/StepsCheck.java.
     */
    static boolean changedSince(boolean watched, AtomicBoolean mark, boolean clear)
    {
        if (!watched)
            return true;
        return clear ? mark.getAndSet(false) : mark.get();
    }

    /** Rust's "texts-changes": {changed (`changedSince`), plugged (the phone on its charger)}. */
    static JSONObject changes(Context context, boolean clear) throws JSONException
    {
        boolean watched;
        synchronized (Texts.class) {
            watched = watching != null;
        }
        return new JSONObject().put("changed", changedSince(watched, changed, clear)).put("plugged", plugged(context));
    }

    /** The phone on its charger, by Android's last word on the battery (a sticky broadcast: no receiver kept). */
    static boolean plugged(Context context)
    {
        try {
            Intent battery = context.registerReceiver(null, new IntentFilter(Intent.ACTION_BATTERY_CHANGED));
            return battery != null && battery.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) != 0;
        } catch (RuntimeException e) {
            return false;
        }
    }

    /** A row of a cursor, or of a check's stand-in: its values by column. */
    interface Row
    {
        String text(String column);

        long number(String column);
    }

    /** A cursor's current row. */
    private static Row of(Cursor cursor)
    {
        return new Row() {
            @Override
            public String text(String column)
            {
                int index = cursor.getColumnIndex(column);
                if (index < 0 || cursor.isNull(index))
                    return "";
                String value = cursor.getString(index);
                return value == null ? "" : value;
            }

            @Override
            public long number(String column)
            {
                int index = cursor.getColumnIndex(column);
                return index < 0 || cursor.isNull(index) ? 0 : cursor.getLong(index);
            }
        };
    }

    // ---------------------------------------------------------------- Rust's questions

    /** Rust's question ({@link StepService#call}): its answer in JSON, or null. */
    static String call(Context context, String verb, JSONObject asked) throws JSONException
    {
        ContentResolver resolver = context.getContentResolver();
        switch (verb) {
        case "texts-state":
            return state(context).toString();
        case "texts-ask":
            ask(context);
            return "true";
        case "texts-read":
            if (!allowed(context, Manifest.permission.READ_SMS))
                return new JSONObject().put("allowed", false).toString();
            return read(resolver, asked.optLong("sms", 0), asked.optLong("mms", 0), asked.optInt("limit", 500), asked.optJSONArray("sms_ids"), asked.optJSONArray("mms_ids")).put("allowed", true).toString();
        case "texts-ids":
            if (!allowed(context, Manifest.permission.READ_SMS))
                return new JSONObject().put("allowed", false).toString();
            return ids(resolver, asked.optBoolean("measure", false)).put("allowed", true).toString();
        case "texts-part":
            if (!allowed(context, Manifest.permission.READ_SMS))
                return new JSONObject().put("allowed", false).toString();
            return part(resolver, asked.optLong("id", 0), asked.optString("target", "")).put("allowed", true).toString();
        case "texts-find":
            if (!allowed(context, Manifest.permission.READ_SMS))
                return new JSONObject().put("allowed", false).toString();
            return find(resolver, context.getPackageName(), asked.optLong("since", 0)).put("allowed", true).toString();
        case "texts-send":
            return TextSend.send(context, asked.optString("key", ""), asked.optString("to", ""), asked.optString("body", ""), asked.optInt("sub", -1)).toString();
        case "texts-send-mms":
            return TextSend.sendMms(context, asked.optString("key", ""), asked.optInt("sub", -1), asked.optString("path", "")).toString();
        case "texts-limits":
            return limits(context, asked.has("sub") ? asked.optInt("sub", -1) : null).toString();
        case "texts-find-mms":
            if (!allowed(context, Manifest.permission.READ_SMS))
                return new JSONObject().put("allowed", false).toString();
            return findMms(resolver, context.getPackageName(), asked.optString("key", "")).put("allowed", true).toString();
        case "texts-changes":
            // Watched in this process too (the window's, without steps of its own): a step asks Android only after a change.
            watch(context, StepService.inThisProcess());
            return changes(context, asked.optBoolean("clear", true)).toString();
        case "texts-info":
            return info(context) ? "true" : "false";
        default:
            return null;
        }
    }

    static boolean allowed(Context context, String permission)
    {
        return context.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED;
    }

    /** What Android allows Sioul: each permission, the phone's SMS app, its SIMs (with READ_PHONE_STATE: their names). */
    static JSONObject state(Context context) throws JSONException
    {
        JSONObject out = new JSONObject();
        out.put("api", Build.VERSION.SDK_INT);
        out.put("read", allowed(context, Manifest.permission.READ_SMS));
        out.put("receive", allowed(context, Manifest.permission.RECEIVE_SMS));
        out.put("send", allowed(context, Manifest.permission.SEND_SMS));
        boolean phoneState = allowed(context, Manifest.permission.READ_PHONE_STATE);
        out.put("phone_state", phoneState);
        String sms = null;
        try {
            sms = Telephony.Sms.getDefaultSmsPackage(context);
        } catch (RuntimeException e) {
            // No telephony.
        }
        out.put("sms_app", sms == null ? "" : sms);
        // The texts' question asked here once at least, and its last answer at once, nothing given: none was shown.
        out.put("asked", asked(context) != null);
        out.put("at_once", askedAtOnce(context));
        JSONArray sims = new JSONArray();
        if (phoneState) {
            try {
                SubscriptionManager subscriptions = context.getSystemService(SubscriptionManager.class);
                List<SubscriptionInfo> active = subscriptions == null ? null : subscriptions.getActiveSubscriptionInfoList();
                if (active != null)
                    for (SubscriptionInfo info : active)
                        sims.put(new JSONObject().put("id", info.getSubscriptionId()).put("slot", info.getSimSlotIndex())
                                     .put("name", info.getDisplayName() == null ? "" : info.getDisplayName().toString()));
            } catch (SecurityException e) {
                // Taken back meanwhile.
            }
        }
        out.put("sims", sims);
        return out;
    }

    // ---------------------------------------------------------------- what Android withholds

    /**
     * How quickly Android must answer the question, giving nothing, for no
     * question to have been shown: a person reads one longer (ms).
     */
    static final long AT_ONCE_MS = 700;
    /** Java's own file, in the private texts folder: how Android answered the last question (TextsAsk). */
    static final String ASKED = "asked.json";

    /*
     * Whether Android keeps "restricted settings" from Sioul (Android 13 and
     * later, an app installed from a file) cannot be read by an app: the app
     * op that says it, ACCESS_RESTRICTED_SETTINGS, is restrict-read on every
     * release that has it (MANAGE_APPOPS and the like, which no app holds),
     * and Android 15's enhanced confirmation asks a permission of its own.
     * Sioul says the steps that hold either way instead (docs/android.md,
     * "What Android withholds").
     */

    /**
     * Whether Android answered the texts' question at once, giving none of
     * the three: it showed none (Android 15's restricted settings after the
     * first time, or "Don't allow" chosen twice before). `took` is -1 when
     * not measured. Pure: android/jvm-checks/TextsCheck.java.
     */
    static boolean atOnce(int given, long took)
    {
        return given == 0 && took >= 0 && took < AT_ONCE_MS;
    }

    /** How Android answered the question (TextsAsk), kept in Java's own file for `state`: how many of how many, how fast. */
    static void remember(Context context, int given, int of, long took)
    {
        File folder = new File(context.getFilesDir(), FOLDER);
        folder.mkdirs();
        try (FileOutputStream out = new FileOutputStream(new File(folder, ASKED))) {
            out.write(new JSONObject().put("at", System.currentTimeMillis()).put("given", given).put("of", of).put("took", took).toString().getBytes(StandardCharsets.UTF_8));
        } catch (IOException | JSONException e) {
            Log.w(TAG, "Texts: the answer could not be kept: " + e.getClass().getSimpleName());
        }
    }

    /** How Android answered the texts' last question (`remember`); null when none was asked here. */
    static JSONObject asked(Context context)
    {
        try {
            return new JSONObject(new String(Files.readAllBytes(new File(new File(context.getFilesDir(), FOLDER), ASKED).toPath()), StandardCharsets.UTF_8));
        } catch (IOException | JSONException | RuntimeException e) {
            return null;
        }
    }

    /** Whether the last question was answered at once, nothing given (`atOnce`); false when none was asked. */
    static boolean askedAtOnce(Context context)
    {
        JSONObject asked = asked(context);
        return asked != null && atOnce(asked.optInt("given", 0), asked.optLong("took", -1));
    }

    /** Android's App info for Sioul, where ⋮ ▸ Allow restricted settings and the permissions are. */
    static boolean info(Context context)
    {
        try {
            context.startActivity(new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:" + context.getPackageName())).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            return true;
        } catch (RuntimeException e) {
            Log.w(TAG, "Texts: App info could not be opened: " + e.getClass().getSimpleName());
            return false;
        }
    }

    /** Android's question for the texts' permissions: an activity never seen (TextsAsk), from Sioul's window. */
    static void ask(Context context)
    {
        try {
            context.startActivity(new Intent(context, TextsAsk.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        } catch (RuntimeException e) {
            Log.w(TAG, "Texts: Android's question could not be asked: " + e);
        }
    }

    // ---------------------------------------------------------------- reading

    /** Columns never kept: where the provider keeps a part's file on this phone. */
    static final String LOCAL_PATH = "_data";

    /** A row's every column, as the provider gives it (a part's file's place on the phone aside). */
    static JSONObject copy(Cursor cursor) throws JSONException
    {
        JSONObject out = new JSONObject();
        for (String column : cursor.getColumnNames()) {
            int index = cursor.getColumnIndex(column);
            if (index < 0 || LOCAL_PATH.equals(column))
                continue;
            switch (cursor.getType(index)) {
            case Cursor.FIELD_TYPE_INTEGER:
                out.put(column, cursor.getLong(index));
                break;
            case Cursor.FIELD_TYPE_FLOAT:
                out.put(column, cursor.getDouble(index));
                break;
            case Cursor.FIELD_TYPE_STRING:
                out.put(column, cursor.getString(index));
                break;
            default:
                // Null and blobs: not kept (no provider column a restore needs holds a blob).
                break;
            }
        }
        return out;
    }

    /** A text of the SMS provider, as Rust reads it (crates/sioul-app/src/texts.rs, `Raw`): every column. */
    static JSONObject sms(JSONObject row) throws JSONException
    {
        return new JSONObject().put("kind", "sms").put("fields", row);
    }

    /**
     * A multimedia message, as Rust reads it: every column of its row, of its
     * addresses and of its parts (each part's size from Android, its file's
     * place on the phone left out), so that a later restore loses nothing.
     */
    static JSONObject mms(JSONObject row, List<JSONObject> addresses, List<JSONObject> parts) throws JSONException
    {
        JSONArray addrs = new JSONArray();
        for (JSONObject a : addresses)
            addrs.put(a);
        JSONArray all = new JSONArray();
        for (JSONObject p : parts) {
            p.remove(LOCAL_PATH);
            all.put(p);
        }
        return new JSONObject().put("kind", "mms").put("fields", row).put("addrs", addrs).put("parts", all);
    }

    /** Whether a part's content travels as a file: anything but words and the SMIL that lays them out. */
    static boolean media(String type)
    {
        return type != null && !type.startsWith("text/plain") && !"application/smil".equals(type);
    }

    /** A part's size in bytes, as Android opens it; -1 when it does not open (not downloaded). */
    static long size(ContentResolver resolver, long part)
    {
        try (android.content.res.AssetFileDescriptor file = resolver.openAssetFileDescriptor(Uri.parse("content://mms/part/" + part), "r")) {
            return file == null ? -1 : file.getLength();
        } catch (java.io.IOException | RuntimeException e) {
            return -1;
        }
    }

    /**
     * Each conversation's people, by its thread: Android's own list of each
     * thread's recipients (the person themselves never among them), so that a
     * group reads as a group.
     */
    static JSONObject threads(ContentResolver resolver) throws JSONException
    {
        JSONObject addresses = new JSONObject();
        try (Cursor cursor = resolver.query(Uri.parse("content://mms-sms/canonical-addresses"), null, null, null, null)) {
            while (cursor != null && cursor.moveToNext()) {
                Row row = of(cursor);
                addresses.put(String.valueOf(row.number("_id")), row.text("address"));
            }
        } catch (RuntimeException e) {
            Log.w(TAG, "Texts: the conversations' people could not be read: " + e.getClass().getSimpleName());
        }
        JSONObject threads = new JSONObject();
        try (Cursor cursor = resolver.query(Uri.parse("content://mms-sms/conversations?simple=true"), new String[] { "_id", "recipient_ids" }, null, null, null)) {
            while (cursor != null && cursor.moveToNext()) {
                Row row = of(cursor);
                JSONArray people = new JSONArray();
                for (String id : row.text("recipient_ids").trim().split("\\s+"))
                    if (!id.isEmpty() && addresses.has(id))
                        people.put(addresses.getString(id));
                threads.put(String.valueOf(row.number("_id")), people);
            }
        } catch (RuntimeException e) {
            Log.w(TAG, "Texts: the conversations could not be read: " + e.getClass().getSimpleName());
        }
        return threads;
    }

    /**
     * Which rows a read takes: given `ids` (the rows Rust's reader asks for:
     * those its log lacks, by their ids), those rows only; else those after
     * the mark (no reader asks so since its version 4, 16 October 2026).
     */
    static String rows(JSONArray ids)
    {
        if (ids == null)
            return "_id > ?";
        StringBuilder in = new StringBuilder("_id IN (");
        for (int i = 0; i < ids.length(); i++)
            in.append(i == 0 ? "" : ",").append(ids.optLong(i, -1));
        return in.append(ids.length() == 0 ? "-1)" : ")").toString();
    }

    /**
     * The rows `smsIds` and `mmsIds` name (else those after the marks `sms`,
     * `mms`: `rows`); `limit` at most, the plain texts first, by id, every
     * column, the date among them (Rust tells a row by its id and its date);
     * each conversation's people; and the new marks.
     */
    static JSONObject read(ContentResolver resolver, long smsMark, long mmsMark, int limit, JSONArray smsIds, JSONArray mmsIds) throws JSONException
    {
        JSONArray texts = new JSONArray();
        long lastSms = smsMark;
        int smsCount = 0;
        try (Cursor cursor = resolver.query(Telephony.Sms.CONTENT_URI, null, rows(smsIds), smsIds == null ? new String[] { String.valueOf(smsMark) } : null, "_id ASC")) {
            while (cursor != null && cursor.moveToNext() && smsCount < limit) {
                JSONObject row = copy(cursor);
                lastSms = Math.max(lastSms, row.optLong("_id"));
                texts.put(sms(row));
                smsCount++;
            }
        } catch (RuntimeException e) {
            Log.w(TAG, "Texts: the texts could not be read: " + e.getClass().getSimpleName());
        }
        long lastMms = mmsMark;
        int mmsCount = 0;
        try (Cursor cursor = resolver.query(Telephony.Mms.CONTENT_URI, null, rows(mmsIds), mmsIds == null ? new String[] { String.valueOf(mmsMark) } : null, "_id ASC")) {
            while (cursor != null && smsCount + mmsCount < limit && cursor.moveToNext()) {
                JSONObject row = copy(cursor);
                long id = row.optLong("_id");
                lastMms = Math.max(lastMms, id);
                List<JSONObject> parts = copies(resolver, Uri.parse("content://mms/part"), "mid = ?", new String[] { String.valueOf(id) });
                for (JSONObject part : parts)
                    if (media(part.optString("ct", "")))
                        part.put("size", size(resolver, part.optLong("_id")));
                texts.put(mms(row, copies(resolver, Uri.parse("content://mms/" + id + "/addr"), null, null), parts));
                mmsCount++;
            }
        } catch (RuntimeException e) {
            Log.w(TAG, "Texts: the multimedia messages could not be read: " + e.getClass().getSimpleName());
        }
        // The conversations' people only when there is something to read: most steps find nothing new.
        JSONObject threads = texts.length() > 0 ? threads(resolver) : new JSONObject();
        return new JSONObject().put("texts", texts).put("threads", threads).put("sms", lastSms).put("mms", lastMms);
    }

    /** Every row of a query, every column copied (the cursor closed after). */
    private static List<JSONObject> copies(ContentResolver resolver, Uri uri, String selection, String[] arguments)
    {
        List<JSONObject> out = new ArrayList<>();
        try (Cursor cursor = resolver.query(uri, null, selection, arguments, null)) {
            while (cursor != null && cursor.moveToNext())
                out.add(copy(cursor));
        } catch (JSONException | RuntimeException e) {
            Log.w(TAG, "Texts: a message's parts could not be read: " + e.getClass().getSimpleName());
        }
        return out;
    }

    /**
     * Every row there is now, each id with its own date ("sms" with "sms_at",
     * "mms" with "mms_at": the plain texts' table gives a deleted newest
     * text's id to the next one, and the date tells the two apart; Rust's
     * `texts::Row`) and, when `measure`, how much media the multimedia
     * messages hold: the rows to read, the texts deleted on the phone, and
     * the sizes the settings say. A table that could not be listed whole is
     * left out: Rust then reads nothing and marks nothing deleted, never
     * taking "not known" for "none there".
     */
    static JSONObject ids(ContentResolver resolver, boolean measure) throws JSONException
    {
        JSONObject out = new JSONObject();
        listed(resolver, Telephony.Sms.CONTENT_URI, "sms", out);
        listed(resolver, Telephony.Mms.CONTENT_URI, "mms", out);
        long media = 0;
        // Measured only on a whole listing: a look that cannot list waits, and measures nothing meanwhile.
        if (measure && out.has("sms") && out.has("mms")) {
            try (Cursor cursor = resolver.query(Uri.parse("content://mms/part"), new String[] { "_id", "ct" }, null, null, null)) {
                while (cursor != null && cursor.moveToNext())
                    if (media(cursor.getString(1)))
                        media += Math.max(0, size(resolver, cursor.getLong(0)));
            } catch (RuntimeException e) {
                Log.w(TAG, "Texts: the media could not be measured: " + e.getClass().getSimpleName());
            }
        }
        return out.put("media", media);
    }

    /** A table's ids and dates (`ids`), put in `out` under `name` and `name_at` only when listed whole. */
    private static void listed(ContentResolver resolver, Uri table, String name, JSONObject out) throws JSONException
    {
        JSONArray ids = new JSONArray();
        JSONArray dates = new JSONArray();
        try (Cursor cursor = resolver.query(table, new String[] { "_id", "date" }, null, null, null)) {
            if (cursor == null)
                return;
            while (cursor.moveToNext()) {
                ids.put(cursor.getLong(0));
                dates.put(cursor.getLong(1));
            }
        } catch (RuntimeException e) {
            Log.w(TAG, "Texts: the " + name + " rows could not be listed: " + e.getClass().getSimpleName());
            return;
        }
        out.put(name, ids).put(name + "_at", dates);
    }

    /** A part's content copied to `target` (Sioul's own folder), for Rust to seal and share: {copied: bytes}, or -1. */
    static JSONObject part(ContentResolver resolver, long part, String target) throws JSONException
    {
        long copied = -1;
        java.io.File file = new java.io.File(target);
        java.io.File parent = file.getParentFile();
        if (parent != null)
            parent.mkdirs();
        try (java.io.InputStream in = resolver.openInputStream(Uri.parse("content://mms/part/" + part));
             java.io.OutputStream out = new java.io.FileOutputStream(file)) {
            if (in != null) {
                byte[] buffer = new byte[64 * 1024];
                long total = 0;
                int read;
                while ((read = in.read(buffer)) > 0) {
                    out.write(buffer, 0, read);
                    total += read;
                }
                copied = total;
            }
        } catch (java.io.IOException | RuntimeException e) {
            copied = -1;
        }
        if (copied < 0)
            file.delete();
        return new JSONObject().put("copied", copied);
    }

    /**
     * The texts Sioul sent since `since` (ms), as Android filed them in the
     * sent box with Sioul as their creator: after a crash between a claim and
     * Android's word, Rust looks for the one it claimed.
     */
    static JSONObject find(ContentResolver resolver, String own, long since) throws JSONException
    {
        JSONArray texts = new JSONArray();
        try (Cursor cursor = resolver.query(Telephony.Sms.CONTENT_URI, null, "type = ? AND date >= ?", new String[] { String.valueOf(SENT_BOX), String.valueOf(since) }, "date ASC")) {
            while (cursor != null && cursor.moveToNext()) {
                JSONObject row = copy(cursor);
                if (own.equals(row.optString(Telephony.TextBasedSmsColumns.CREATOR, "")))
                    texts.put(sms(row));
            }
        } catch (RuntimeException e) {
            Log.w(TAG, "Texts: the texts sent could not be read: " + e);
        }
        return new JSONObject().put("texts", texts);
    }

    // ---------------------------------------------------------------- multimedia messages

    /**
     * What each SIM's carrier lets Sioul send (Android's carrier
     * configuration, {@code SmsManager.getCarrierConfigValues}, which asks no
     * permission): that SIM's alone ({@code only}; -1 the phone's default),
     * else each active SIM's (READ_PHONE_STATE lists them), else the
     * default's. Numbers only, never the carrier's name.
     */
    static JSONObject limits(Context context, Integer only) throws JSONException
    {
        List<Integer> subs = new ArrayList<>();
        if (only != null) {
            subs.add(only);
        } else {
            if (allowed(context, Manifest.permission.READ_PHONE_STATE)) {
                try {
                    SubscriptionManager subscriptions = context.getSystemService(SubscriptionManager.class);
                    List<SubscriptionInfo> active = subscriptions == null ? null : subscriptions.getActiveSubscriptionInfoList();
                    if (active != null)
                        for (SubscriptionInfo info : active)
                            subs.add(info.getSubscriptionId());
                } catch (SecurityException e) {
                    // Taken back meanwhile.
                }
            }
            if (subs.isEmpty())
                subs.add(-1);
        }
        JSONArray out = new JSONArray();
        for (int sub : subs) {
            try {
                android.os.Bundle values = TextSend.manager(context, sub).getCarrierConfigValues();
                if (values != null)
                    out.put(limitsLine(sub, values.getInt(android.telephony.SmsManager.MMS_CONFIG_MAX_MESSAGE_SIZE, 0), values.getInt(android.telephony.SmsManager.MMS_CONFIG_MAX_IMAGE_WIDTH, 0),
                                       values.getInt(android.telephony.SmsManager.MMS_CONFIG_MAX_IMAGE_HEIGHT, 0), values.getInt(android.telephony.SmsManager.MMS_CONFIG_RECIPIENT_LIMIT, Integer.MAX_VALUE),
                                       values.getBoolean(android.telephony.SmsManager.MMS_CONFIG_GROUP_MMS_ENABLED, true), values.getBoolean(android.telephony.SmsManager.MMS_CONFIG_MMS_ENABLED, true)));
            } catch (RuntimeException e) {
                Log.w(TAG, "Texts: a SIM's carrier values could not be read: " + e.getClass().getSimpleName());
            }
        }
        return new JSONObject().put("limits", out);
    }

    /**
     * One SIM's carrier values as Rust reads them ({@code texts::Limits}):
     * none below zero (a carrier's "no limit" said as the largest count).
     * Pure: android/jvm-checks/TextsCheck.java.
     */
    static JSONObject limitsLine(int sub, int max, int width, int height, int recipients, boolean group, boolean mms) throws JSONException
    {
        return new JSONObject().put("sub", sub).put("max", Math.max(max, 0)).put("width", Math.max(width, 0)).put("height", Math.max(height, 0))
            .put("recipients", recipients < 0 ? Integer.MAX_VALUE : recipients).put("group", group).put("mms", mms);
    }

    /**
     * Sioul's multimedia message by its key: the rows Android filed with that
     * transaction id (the message Rust built carries it) and Sioul as their
     * creator, each its id, its date (seconds) and its box (2: sent).
     */
    static JSONObject findMms(ContentResolver resolver, String own, String key) throws JSONException
    {
        JSONArray found = new JSONArray();
        if (!key.isEmpty()) {
            String[] columns = { Telephony.Mms._ID, Telephony.Mms.DATE, Telephony.Mms.MESSAGE_BOX, Telephony.Mms.CREATOR };
            try (Cursor cursor = resolver.query(Telephony.Mms.CONTENT_URI, columns, Telephony.Mms.TRANSACTION_ID + " = ?", new String[] { key }, null)) {
                while (cursor != null && cursor.moveToNext()) {
                    Row row = of(cursor);
                    if (own.equals(row.text(Telephony.Mms.CREATOR)))
                        found.put(new JSONObject().put("id", row.number(Telephony.Mms._ID)).put("date", row.number(Telephony.Mms.DATE)).put("box", row.number(Telephony.Mms.MESSAGE_BOX)));
                }
            } catch (RuntimeException e) {
                Log.w(TAG, "Texts: the multimedia messages sent could not be read: " + e);
            }
        }
        return new JSONObject().put("found", found);
    }

    // ---------------------------------------------------------------- heard

    /**
     * A new text, after the phone's SMS app had it (SMS_RECEIVED, sent by
     * the system alone: its permission): a step soon, in which Rust reads it
     * and shares it. Nothing is read here.
     */
    public static final class Heard extends BroadcastReceiver
    {
        @Override
        public void onReceive(Context context, Intent intent)
        {
            if (intent != null && Telephony.Sms.Intents.SMS_RECEIVED_ACTION.equals(intent.getAction())) {
                touched();
                StepService.soon(context.getApplicationContext());
            }
        }
    }
}
