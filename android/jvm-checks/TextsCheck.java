// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Texts.sms, Texts.mms, Texts.media, Texts.rows, TextSend.line, a multimedia
 * message's word (TextSend.mmsLine), its file's address (Attachments.contentUri),
 * its SIM's carrier values (Texts.limitsLine) and a long text's parts still on
 * their way (Texts.segment) on the JVM, on a
 * stand-in provider (rows of `sms`, `mms`, its `addr` and `part` tables, as
 * Texts.copy makes them), as Rust reads them (crates/sioul-app/src/texts.rs,
 * `Raw`, `Result`): every column kept for a later restore, a part's file's
 * place on the phone never; which parts travel as files; which rows a read
 * takes; a part's result.
 * Fiction numbers only (ARCEP's 01 99 00, 04 65 71 and 05 36 49).
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

        // A multimedia message's word, as Rust reads it (`mms_outcomes`): Android's code, the row, the HTTP status, the carrier's answer in hex.
        JSONObject mmsLine = TextSend.mmsLine("0123456789abcdef0123456789abcdef", -1, "content://mms/sent/31", 200, new byte[] { (byte) 0x8C, (byte) 0x81, 0x00, 0x7F }, at);
        expect("mms result kind", "mms".equals(mmsLine.getString("kind")) && mmsLine.getInt("code") == -1 && mmsLine.getInt("http") == 200);
        expect("mms result row", "content://mms/sent/31".equals(mmsLine.getString("uri")));
        expect("mms result answer in hex", "8c81007f".equals(mmsLine.getString("conf")));
        expect("mms result, no answer", "".equals(TextSend.mmsLine("k", 5, null, 0, null, at).getString("conf")) && "".equals(TextSend.mmsLine(null, 5, null, 0, null, at).getString("key")));
        // A multimedia message's file lent to Android by Sioul's provider (root "files_path", the private files).
        String files = "/data/user/0/com.aurelienpierre.sioul/files";
        expect("mms file's address", "content://com.aurelienpierre.sioul.qtprovider/files_path/state/sioul/texts/outgoing/0123456789abcdef0123456789abcdef.pdu"
                                         .equals(Attachments.contentUri("com.aurelienpierre.sioul", files, files + "/state/sioul/texts/outgoing/0123456789abcdef0123456789abcdef.pdu", TextSend.FILES_ROOT)));
        expect("mms file outside the private files: none", Attachments.contentUri("com.aurelienpierre.sioul", files, "/data/user/0/other/files/x.pdu", TextSend.FILES_ROOT) == null);
        expect("mms file through ..: none", Attachments.contentUri("com.aurelienpierre.sioul", files, files + "/state/../../x.pdu", TextSend.FILES_ROOT) == null);
        expect("the attachments' addresses as before", Attachments.contentUri("com.aurelienpierre.sioul", "/c", "/c/a.pdf").endsWith("/attachments/a.pdf"));
        // What a SIM's carrier lets Sioul send, as Rust reads it (`texts::Limits`): never below zero.
        JSONObject limits = Texts.limitsLine(3, 614400, 2592, 1944, -1, true, true);
        expect("limits", limits.getInt("sub") == 3 && limits.getLong("max") == 614400 && limits.getInt("width") == 2592 && limits.getInt("height") == 1944 && limits.getBoolean("group") && limits.getBoolean("mms"));
        expect("limits: a carrier's no limit as the largest count", limits.getLong("recipients") == Integer.MAX_VALUE);
        expect("limits: none below zero", Texts.limitsLine(-1, -5, -1, -1, 20, false, false).getLong("max") == 0 && Texts.limitsLine(-1, -5, -1, -1, 20, false, false).getInt("recipients") == 20);

        // Android's answer to the texts' three, asked alone: at once and none given, no question was shown
        // (restricted settings after the first time, or "Don't allow" chosen twice before).
        expect("answered at once, nothing given", Texts.atOnce(0, 120));
        expect("answered after a question", !Texts.atOnce(0, 4000));
        expect("something given", !Texts.atOnce(2, 50));
        expect("no time measured", !Texts.atOnce(0, -1));

        // A long text's part still on its way (Texts.segment, Texts.SEGMENTS_WAITING), as Rust reads it
        // (`texts::arriving_from`): its sender, its text, its place and its time; never its words nor its PDU.
        Texts.Row waiting = new Texts.Row() {
            @Override
            public String text(String column)
            {
                return "address".equals(column) ? "+33536490042" : "";
            }

            @Override
            public long number(String column)
            {
                switch (column) {
                case "reference_number":
                    return 17;
                case "count":
                    return 9;
                case "sequence":
                    return 4;
                case "date":
                    return at;
                case "sub_id":
                    return 1;
                default:
                    return 0;
                }
            }
        };
        JSONObject segment = Texts.segment(waiting);
        expect("a part's sender, text, place and time", "+33536490042".equals(segment.getString("address")) && segment.getLong("reference") == 17 && segment.getLong("count") == 9
                                                         && segment.getLong("sequence") == 4 && segment.getLong("date") == at && segment.getLong("sub") == 1);
        List<String> columns = java.util.Arrays.asList(Texts.SEGMENT_COLUMNS);
        expect("never a part's words nor its PDU", !columns.contains("message_body") && !columns.contains("pdu") && !segment.has("message_body"));
        expect("the parts waiting: not handed over, of a plain text of two parts or more",
               Texts.SEGMENTS_WAITING.contains("deleted = 0") && Texts.SEGMENTS_WAITING.contains("count > 1") && Texts.SEGMENTS_WAITING.contains("(destination_port & 65536) != 0"));

        System.out.println("TextsCheck: " + checked + " checked, " + failed + " failed");
        if (failed > 0)
            System.exit(1);
    }
}
