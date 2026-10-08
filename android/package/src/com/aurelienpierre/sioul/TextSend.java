// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.Manifest;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Build;
import android.telephony.SmsManager;
import android.telephony.SmsMessage;
import android.util.Log;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * A text written on a computer, sent by this phone (docs/texts.md, "Never
 * twice"; Rust's side: crates/sioul-app/src/texts.rs). Rust has decided it
 * and written its key in the phone's private ledger, flushed to the disk,
 * before it asks here: a key is handed to Android once. Android splits it
 * (`divideMessage`), sends it from the SIM named, files it in the phone's
 * sent box with Sioul as its creator, so that the SMS app shows it in the
 * conversation, and retries by itself three times ("reject duplicates");
 * Sioul never retries. Each part's sent and delivered results come to
 * `Result`, which writes them in Java's own file (`results.jsonl`), flushed,
 * that Rust copies into the phone's outcomes. Java never writes a shared file.
 */
final class TextSend
{
    static final String SENT = "com.aurelienpierre.sioul.action.TEXT_SENT";
    static final String DELIVERED = "com.aurelienpierre.sioul.action.TEXT_DELIVERED";
    /** Java's results, in Sioul's texts folder. */
    static final String RESULTS = "results.jsonl";

    private TextSend()
    {
    }

    /**
     * The text handed to Android: {handed, parts} or {handed: false, reason}.
     * The parts' intents name the key, so that each result finds its text.
     */
    static JSONObject send(Context context, String key, String to, String body, int sub) throws JSONException
    {
        if (key.isEmpty() || to.isEmpty() || body.isEmpty())
            return new JSONObject().put("handed", false).put("reason", "empty");
        if (!Texts.allowed(context, Manifest.permission.SEND_SMS))
            return new JSONObject().put("handed", false).put("reason", "permission");
        try {
            SmsManager manager = manager(context, sub);
            ArrayList<String> parts = manager.divideMessage(body);
            int count = parts.size();
            ArrayList<PendingIntent> sent = new ArrayList<>();
            ArrayList<PendingIntent> delivered = new ArrayList<>();
            for (int i = 0; i < count; i++) {
                sent.add(pending(context, SENT, key, i, count));
                delivered.add(pending(context, DELIVERED, key, i, count));
            }
            manager.sendMultipartTextMessage(to, null, parts, sent, delivered);
            return new JSONObject().put("handed", true).put("parts", count);
        } catch (RuntimeException e) {
            Log.w(Texts.TAG, "Texts: Android did not take a text: " + e.getClass().getSimpleName());
            return new JSONObject().put("handed", false).put("reason", "refused");
        }
    }

    /** The SIM's own manager (`sub` ≥ 0), else the phone's default one. */
    @SuppressWarnings("deprecation")
    private static SmsManager manager(Context context, int sub)
    {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            SmsManager manager = context.getSystemService(SmsManager.class);
            return sub >= 0 ? manager.createForSubscriptionId(sub) : manager;
        }
        return sub >= 0 ? SmsManager.getSmsManagerForSubscriptionId(sub) : SmsManager.getDefault();
    }

    /**
     * A part's result, to `Result`: one of its own (its address names the
     * key, the part, sent or delivered), mutable so that Android may add the
     * row it filed ("uri") and the carrier's report ("pdu").
     */
    private static PendingIntent pending(Context context, String action, String key, int part, int parts)
    {
        Intent intent = new Intent(context, Result.class).setAction(action)
            .setData(Uri.parse("sioul-text://" + key + "/" + (SENT.equals(action) ? "sent" : "delivered") + "/" + part))
            .putExtra("key", key).putExtra("part", part).putExtra("parts", parts);
        int flags = PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ? PendingIntent.FLAG_MUTABLE : 0);
        return PendingIntent.getBroadcast(context, 0, intent, flags);
    }

    /**
     * A part's result as Rust reads it (crates/sioul-app/src/texts.rs,
     * `Result`): its key, "sent" or "delivered", which part of how many,
     * Android's code (-1: done), the row it filed, the carrier's status for a
     * delivery (TP-Status: under 0x20, received), when (ms).
     */
    static JSONObject line(String key, String kind, int part, int parts, int code, String uri, int status, long at) throws JSONException
    {
        return new JSONObject().put("key", key).put("kind", kind).put("part", part).put("parts", parts).put("code", code)
            .put("uri", uri == null ? "" : uri).put("status", status).put("at", at);
    }

    /** A line added to Java's results file, flushed to the disk. */
    static synchronized void append(File folder, String line)
    {
        folder.mkdirs();
        try (FileOutputStream out = new FileOutputStream(new File(folder, RESULTS), true)) {
            out.write((line + "\n").getBytes(StandardCharsets.UTF_8));
            out.getFD().sync();
        } catch (IOException e) {
            Log.w(Texts.TAG, "Texts: a result not written: " + e);
        }
    }

    /** Android's word on a part, sent or delivered (in the background service's process): written, then a step soon. */
    public static final class Result extends BroadcastReceiver
    {
        @Override
        public void onReceive(Context context, Intent intent)
        {
            if (intent == null || intent.getAction() == null)
                return;
            boolean sent = SENT.equals(intent.getAction());
            int status = -1;
            if (!sent) {
                try {
                    byte[] pdu = intent.getByteArrayExtra("pdu");
                    String format = intent.getStringExtra("format");
                    SmsMessage report = pdu == null ? null : SmsMessage.createFromPdu(pdu, format);
                    status = report == null ? -1 : report.getStatus();
                } catch (RuntimeException e) {
                    // A report that does not read: no word on delivery.
                }
            }
            try {
                String line = line(intent.getStringExtra("key"), sent ? "sent" : "delivered", intent.getIntExtra("part", 0), intent.getIntExtra("parts", 1),
                                   getResultCode(), intent.getStringExtra("uri"), status, System.currentTimeMillis()).toString();
                append(new File(context.getFilesDir(), Texts.FOLDER), line);
            } catch (JSONException e) {
                Log.w(Texts.TAG, "Texts: a result not read: " + e);
            }
            StepService.soon(context.getApplicationContext());
        }
    }
}
