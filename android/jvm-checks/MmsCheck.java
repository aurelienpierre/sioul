// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.util.Log;

import com.google.android.mms.pdu.EncodedStringValue;
import com.google.android.mms.pdu.GenericPdu;
import com.google.android.mms.pdu.PduBody;
import com.google.android.mms.pdu.PduHeaders;
import com.google.android.mms.pdu.PduParser;
import com.google.android.mms.pdu.PduPart;
import com.google.android.mms.pdu.SendConf;
import com.google.android.mms.pdu.SendReq;

import org.json.JSONArray;
import org.json.JSONObject;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.List;

/**
 * Rust's multimedia messages read by Android's own parser: each sample of
 * android/jvm-checks/mms-samples.jsonl, which sioul-core's mms tests write
 * from what its encoder writes now (crates/sioul-core/src/mms.rs), parsed by
 * AOSP's PduParser (android/jvm-checks/aosp/, unchanged, Apache-2.0: its
 * NOTICE) as Android's MMS service parses the message an app hands it, with
 * the parts' Content-Disposition read or not (each carrier's choice).
 * A message Android cannot read is sent, but never filed in the person's
 * messages: each must read as a SendReq with its key, its numbers (their
 * /TYPE=PLMN left out, as Android files them), its version, date, class and
 * reports, and each part with its type, name, Content-ID, place, charset
 * and very bytes; the carrier's answers as a SendConf with its status and
 * Message-ID. The parser must say nothing at its error level.
 */
public final class MmsCheck
{
    static int checked = 0;
    static int failed = 0;

    static void check(boolean ok, String what)
    {
        checked++;
        if (!ok) {
            failed++;
            System.out.println("FAIL " + what);
        }
    }

    static byte[] bytes(String hex)
    {
        byte[] out = new byte[hex.length() / 2];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) Integer.parseInt(hex.substring(2 * i, 2 * i + 2), 16);
        }
        return out;
    }

    static String text(byte[] bytes)
    {
        return bytes == null ? null : new String(bytes, StandardCharsets.UTF_8);
    }

    static String sha256(byte[] bytes) throws Exception
    {
        StringBuilder out = new StringBuilder();
        for (byte b : MessageDigest.getInstance("SHA-256").digest(bytes == null ? new byte[0] : bytes)) {
            out.append(String.format("%02x", b));
        }
        return out.toString();
    }

    static void request(String name, SendReq req, JSONObject sample) throws Exception
    {
        check(req.getMessageType() == PduHeaders.MESSAGE_TYPE_SEND_REQ, name + ": a send request");
        check(sample.getString("key").equals(text(req.getTransactionId())), name + ": its key as transaction id, " + text(req.getTransactionId()));
        check(req.getMmsVersion() == PduHeaders.CURRENT_MMS_VERSION, name + ": MMS 1.2, " + req.getMmsVersion());
        check(req.getDate() == sample.getLong("date"), name + ": its date, " + req.getDate());
        EncodedStringValue from = req.getFrom();
        check(from != null && PduHeaders.FROM_INSERT_ADDRESS_TOKEN_STR.equals(from.getString()), name + ": From for the carrier to write");
        JSONArray numbers = sample.getJSONArray("to");
        EncodedStringValue[] to = req.getTo();
        check(to != null && to.length == numbers.length(), name + ": " + numbers.length() + " numbers");
        for (int i = 0; to != null && i < Math.min(to.length, numbers.length()); i++) {
            check(numbers.getString(i).equals(to[i].getString()), name + ": number " + i + " " + to[i].getString());
        }
        check(PduHeaders.MESSAGE_CLASS_PERSONAL_STR.equals(text(req.getMessageClass())), name + ": class personal");
        check(req.getDeliveryReport() == PduHeaders.VALUE_NO && req.getReadReport() == PduHeaders.VALUE_NO, name + ": no reports asked");
        check("application/vnd.wap.multipart.related".equals(text(req.getContentType())), name + ": multipart/related");
        check(req.getSubject() == null, name + ": no subject");
        PduBody body = req.getBody();
        JSONArray parts = sample.getJSONArray("parts");
        check(body != null && body.getPartsNum() == parts.length(), name + ": " + parts.length() + " parts");
        for (int i = 0; body != null && i < Math.min(body.getPartsNum(), parts.length()); i++) {
            PduPart part = body.getPart(i);
            JSONObject expected = parts.getJSONObject(i);
            String at = name + ", part " + i + ": ";
            check(expected.getString("ct").equals(text(part.getContentType())), at + "type " + text(part.getContentType()));
            check(expected.getString("name").equals(text(part.getName())), at + "name " + text(part.getName()));
            check(expected.getString("id").equals(text(part.getContentId())), at + "Content-ID " + text(part.getContentId()));
            check(expected.getString("location").equals(text(part.getContentLocation())), at + "Content-Location " + text(part.getContentLocation()));
            check(expected.getInt("charset") == part.getCharset(), at + "charset " + part.getCharset());
            check(part.getData() != null && expected.getInt("size") == part.getData().length, at + "its bytes' length");
            check(expected.getString("sha256").equals(sha256(part.getData())), at + "its very bytes");
        }
    }

    static void answer(String name, SendConf conf, JSONObject sample)
    {
        check(conf.getMessageType() == PduHeaders.MESSAGE_TYPE_SEND_CONF, name + ": an answer");
        check(sample.getString("key").equals(text(conf.getTransactionId())), name + ": its key");
        check(conf.getResponseStatus() == sample.getInt("status"), name + ": status " + conf.getResponseStatus());
        String id = sample.getString("id");
        check(id.isEmpty() ? conf.getMessageId() == null : id.equals(text(conf.getMessageId())), name + ": Message-ID " + text(conf.getMessageId()));
    }

    public static void main(String[] args) throws Exception
    {
        Path samples = Path.of(System.getProperty("sioul.root", "."), "android", "jvm-checks", "mms-samples.jsonl");
        List<String> lines = Files.readAllLines(samples, StandardCharsets.UTF_8);
        int read = 0;
        for (String line : lines) {
            if (line.isBlank()) {
                continue;
            }
            read++;
            JSONObject sample = new JSONObject(line);
            String name = sample.getString("name");
            byte[] pdu = bytes(sample.getString("pdu"));
            for (boolean disposition : new boolean[] { true, false }) {
                Log.ERRORS.clear();
                GenericPdu parsed = new PduParser(Arrays.copyOf(pdu, pdu.length), disposition).parse();
                String as = name + (disposition ? "" : " (no Content-Disposition)");
                if ("send-req".equals(sample.getString("kind"))) {
                    check(parsed instanceof SendReq, as + ": read as a SendReq");
                    if (parsed instanceof SendReq req) {
                        request(as, req, sample);
                    }
                } else {
                    check(parsed instanceof SendConf, as + ": read as a SendConf");
                    if (parsed instanceof SendConf conf) {
                        answer(as, conf, sample);
                    }
                }
                check(Log.ERRORS.isEmpty(), as + ": the parser said " + Log.ERRORS);
            }
        }
        check(read >= 7, "the samples read: " + read);
        System.out.println("MmsCheck: " + read + " samples, " + checked + " checked, " + failed + " failed");
        if (failed > 0) {
            System.exit(1);
        }
    }
}
