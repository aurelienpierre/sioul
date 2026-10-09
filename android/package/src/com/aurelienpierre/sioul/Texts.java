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
import android.provider.Telephony;
import android.telephony.SubscriptionInfo;
import android.telephony.SubscriptionManager;
import android.util.Log;
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
 * Asked through StepService.call ("texts-…", from either process): the
 * permissions and the SIMs, Android's question for the permissions
 * (TextsAsk), the texts since the reader's marks (the whole history, every
 * column, multimedia messages and their parts), a part's content, the ids
 * there are now, the texts Sioul sent lately (to find one after a crash),
 * and a text to send (TextSend), whether the phone's texts changed since the
 * last look and whether it charges (`changes`). Heard: a new text, which
 * brings a step soon. The background service watches Android's provider of
 * texts (`watch`): the import asks Android only when they changed.
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
    /** Android's provider of texts watched in this process (the background service's); null: not watched here. */
    private static ContentObserver watching;

    private Texts()
    {
    }

    /**
     * Android's provider of texts watched in this process (the background
     * service's), once READ_SMS is given: a text received, sent, deleted or
     * read marks them changed. Asked again at each step until it holds.
     */
    static synchronized void watch(Context context)
    {
        if (watching != null || !allowed(context, Manifest.permission.READ_SMS))
            return;
        ContentObserver observer = new ContentObserver(null) {
            @Override
            public void onChange(boolean self)
            {
                changed.set(true);
            }
        };
        try {
            ContentResolver resolver = context.getContentResolver();
            for (Uri uri : new Uri[] { Telephony.Sms.CONTENT_URI, Telephony.Mms.CONTENT_URI, Telephony.MmsSms.CONTENT_URI })
                resolver.registerContentObserver(uri, true, observer);
            watching = observer;
            changed.set(true);
        } catch (RuntimeException e) {
            context.getContentResolver().unregisterContentObserver(observer);
            Log.w(TAG, "Texts: the texts cannot be watched: " + e.getClass().getSimpleName());
        }
    }

    /** No longer watched (the service ending). */
    static synchronized void unwatch(Context context)
    {
        if (watching != null)
            context.getContentResolver().unregisterContentObserver(watching);
        watching = null;
        changed.set(true);
    }

    /** A new text heard (Heard): changed, whatever the provider said yet. */
    static void touched()
    {
        changed.set(true);
    }

    /**
     * Whether the texts changed since the last look: always when not watched
     * here (the window's process; READ_SMS not given yet); else as marked,
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

    /** Rust's question (StepService.call): its answer in JSON, or null. */
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
        case "texts-changes":
            return changes(context, asked.optBoolean("clear", true)).toString();
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
     * Which rows a read takes: those after the mark, or, given `ids` (rows the
     * daily look found on the phone and not in the archive: a text that was
     * in the outbox, hidden, when the mark passed it), those rows only.
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
     * The texts after the reader's marks (`sms`, `mms`: the last provider ids
     * read), or the rows `smsIds` and `mmsIds` name; `limit` at most, the
     * plain texts first, oldest first, every column; each conversation's
     * people; and the new marks. The whole history: no date.
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
     * Every provider id there is now and, when `measure`, how much media the
     * multimedia messages hold: which texts were deleted on the phone (gone
     * from it), and the sizes the settings say before and during the import.
     */
    static JSONObject ids(ContentResolver resolver, boolean measure) throws JSONException
    {
        JSONArray sms = new JSONArray();
        try (Cursor cursor = resolver.query(Telephony.Sms.CONTENT_URI, new String[] { "_id" }, null, null, null)) {
            while (cursor != null && cursor.moveToNext())
                sms.put(cursor.getLong(0));
        } catch (RuntimeException e) {
            Log.w(TAG, "Texts: the texts could not be counted: " + e.getClass().getSimpleName());
        }
        JSONArray mms = new JSONArray();
        try (Cursor cursor = resolver.query(Telephony.Mms.CONTENT_URI, new String[] { "_id" }, null, null, null)) {
            while (cursor != null && cursor.moveToNext())
                mms.put(cursor.getLong(0));
        } catch (RuntimeException e) {
            Log.w(TAG, "Texts: the multimedia messages could not be counted: " + e.getClass().getSimpleName());
        }
        long media = 0;
        if (measure) {
            try (Cursor cursor = resolver.query(Uri.parse("content://mms/part"), new String[] { "_id", "ct" }, null, null, null)) {
                while (cursor != null && cursor.moveToNext())
                    if (media(cursor.getString(1)))
                        media += Math.max(0, size(resolver, cursor.getLong(0)));
            } catch (RuntimeException e) {
                Log.w(TAG, "Texts: the media could not be measured: " + e.getClass().getSimpleName());
            }
        }
        return new JSONObject().put("sms", sms).put("mms", mms).put("media", media);
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
