// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Texts.sms, Texts.mms, Texts.media, Texts.rows and TextSend.line on the JVM, on a
 * stand-in provider (rows of `sms`, `mms`, its `addr` and `part` tables, as
 * Texts.copy makes them), as Rust reads them (crates/sioul-app/src/texts.rs,
 * `Raw`, `Result`): every column kept for a later restore, a part's file's
 * place on the phone never; which parts travel as files; which rows a read
 * takes; a part's result.
 * Fiction numbers only (ARCEP's 01 99 00 and 04 65 71).
 */
public final class TextsCheck
{
    static int checked = 0;
    static int failed = 0;

    static void expect(String what, boolean ok)
    {
        checked++;
        if (!ok) {
            failed++;
            System.out.println("FAIL " + what);
        }
    }

    public static void main(String[] args) throws Exception
    {
        long at = 1791446340000L;
        // The stand-in `sms` table's row: every column a non-default app sees.
        JSONObject row = new JSONObject().put("_id", 412).put("thread_id", 7).put("address", "+33199001234").put("date", at).put("date_sent", at - 2000)
            .put("protocol", 0).put("read", 1).put("seen", 1).put("status", -1).put("type", 1).put("reply_path_present", 0).put("body", "Votre rendez-vous est déplacé.")
            .put("service_center", "+33199001390").put("locked", 0).put("sub_id", 2).put("error_code", 0).put("creator", "foundation.e.message");
        JSONObject sms = Texts.sms(row);
        expect("sms kind", "sms".equals(sms.getString("kind")));
        for (String column : new String[] { "_id", "thread_id", "address", "date", "date_sent", "protocol", "read", "seen", "status", "type", "body", "service_center", "locked", "sub_id", "error_code", "creator" })
            expect("sms column " + column, sms.getJSONObject("fields").has(column));

        // A received multimedia message with a picture, its SMIL and its words, and one still to download.
        JSONObject mmsRow = new JSONObject().put("_id", 31).put("thread_id", 9).put("date", at / 1000).put("msg_box", 1).put("m_type", 132).put("ct_t", "application/vnd.wap.multipart.related").put("sub_id", 1);
        List<JSONObject> addrs = new ArrayList<>();
        addrs.add(new JSONObject().put("address", "+33465710042").put("type", 137).put("charset", 106));
        addrs.add(new JSONObject().put("address", "insert-address-token").put("type", 151).put("charset", 106));
        List<JSONObject> parts = new ArrayList<>();
        parts.add(new JSONObject().put("_id", 70).put("seq", -1).put("ct", "application/smil").put("text", "<smil/>").put("cid", "<smil>"));
        parts.add(new JSONObject().put("_id", 71).put("seq", 0).put("ct", "image/jpeg").put("name", "a.jpg").put("cl", "a.jpg").put("_data", "/data/user/0/…/PART_71").put("size", 220000));
        parts.add(new JSONObject().put("_id", 72).put("seq", 1).put("ct", "text/plain").put("text", "Le colis est arrivé.").put("chset", 106));
        JSONObject mms = Texts.mms(mmsRow, addrs, parts);
        expect("mms kind", "mms".equals(mms.getString("kind")));
        expect("mms fields", mms.getJSONObject("fields").getLong("m_type") == 132 && mms.getJSONObject("fields").getLong("date") == at / 1000);
        expect("mms addrs", mms.getJSONArray("addrs").length() == 2 && mms.getJSONArray("addrs").getJSONObject(0).getLong("type") == 137);
        expect("mms parts", mms.getJSONArray("parts").length() == 3);
        expect("a part's size", mms.getJSONArray("parts").getJSONObject(1).getLong("size") == 220000);
        expect("a part's place on the phone never kept", !mms.toString().contains("PART_71") && !mms.getJSONArray("parts").getJSONObject(1).has("_data"));
        expect("a picture travels as a file", Texts.media("image/jpeg") && Texts.media("audio/amr") && Texts.media("text/x-vcard"));
        expect("words and the SMIL in the line", !Texts.media("text/plain") && !Texts.media("application/smil") && !Texts.media(null));

        // Which rows a read takes: after the mark, or those the daily look found missing (numbers only).
        expect("rows after the mark", "_id > ?".equals(Texts.rows(null)));
        expect("rows named", "_id IN (414,417)".equals(Texts.rows(new JSONArray().put(414).put(417))));
        expect("no row named, none read", "_id IN (-1)".equals(Texts.rows(new JSONArray())));
        expect("words never reach the query", "_id IN (-1)".equals(Texts.rows(new JSONArray().put("1); DROP TABLE sms; --"))));

        JSONObject line = TextSend.line("0123456789abcdef0123456789abcdef", "sent", 1, 2, -1, "content://sms/414", -1, at);
        expect("result key", "0123456789abcdef0123456789abcdef".equals(line.getString("key")));
        expect("result part", line.getInt("part") == 1 && line.getInt("parts") == 2);
        expect("result code", line.getInt("code") == -1 && "content://sms/414".equals(line.getString("uri")));
        expect("result no uri", "".equals(TextSend.line("k", "delivered", 0, 1, -1, null, 0, at).getString("uri")));

        System.out.println("TextsCheck: " + checked + " checked, " + failed + " failed");
        if (failed > 0)
            System.exit(1);
    }
}
