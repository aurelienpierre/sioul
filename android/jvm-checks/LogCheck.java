// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;

import org.json.JSONObject;

/**
 * Calls.line and Calls.append on the JVM, as Rust reads them
 * (crates/sioul-core/src/calls.rs, `Held`, `read_held`): every incoming call
 * screened, declined or let ring, with the time of day from the table's frame;
 * never an outgoing call; the file trimmed to a month past its size (in a
 * temporary folder, removed after). With SIOUL_CALLS_SAMPLE set, the lines
 * are written there for Rust's test (calls::tests::java_s_lines_read_here).
 * Fiction numbers only (ARCEP's 01 99 00 and 04 65 71).
 */
public final class LogCheck
{
    static int failed = 0;

    static void expect(String what, boolean ok)
    {
        if (!ok) {
            failed++;
            System.out.println("FAIL " + what);
        }
    }

    public static void main(String[] args) throws Exception
    {
        // A day of sleep for everyone but Always through, as Rust writes a table.
        Calls.Table table = Calls.Table.of(new JSONObject("{\"v\":1,\"made\":0,\"region\":{\"code\":\"FR\",\"calling\":\"33\",\"trunk\":\"0\",\"international\":\"00\",\"digits\":[10,10],\"french\":true},\"trunk_zero\":[\"33\"],\"numbers\":{},\"prefixes\":[],\"phone_contacts\":\"neutral\","
            + "\"floors\":{\"emergency\":[\"112\"],\"people\":[]},\"frames\":[{\"from\":0,\"until\":4102444800000,\"column\":\"sleep\","
            + "\"ring\":{\"safe\":false,\"neutral\":false,\"restricted\":false,\"stranger\":false,\"hidden\":false}}],\"repeat_minutes\":15,\"emergency_hours\":24}"));
        Calls.Phone phone = new Calls.Phone() {
            @Override
            public boolean emergency(String raw)
            {
                return false;
            }

            @Override
            public String contact(String raw)
            {
                return null;
            }
        };
        long at = 1791346200000L;
        List<String> lines = new ArrayList<>();
        // Declined: a stranger at night.
        Calls.Decision declined = Calls.decideFor(table, "01 99 00 12 34", false, at, 0, null, 0, phone);
        JSONObject d = new JSONObject(Calls.line(declined, table));
        expect("declined, keyed as the table's country writes numbers", d.getString("key").equals("+33199001234") && !d.getBoolean("rang") && d.getString("why").equals("matrix") && d.getString("column").equals("sleep") && d.getString("who").equals("stranger") && d.getLong("at") == at);
        lines.add(d.toString());
        // A second call within 15 minutes rings: decided before the frame, its time of day still said.
        Calls.Decision again = Calls.decideFor(table, "01 99 00 12 34", false, at + 480_000, 0, null, at, phone);
        expect("the decision itself says no column", again.column.isEmpty());
        JSONObject a = new JSONObject(Calls.line(again, table));
        expect("a second call rang, at night", a.getBoolean("rang") && a.getString("why").equals("repeat") && a.getString("column").equals("sleep"));
        lines.add(a.toString());
        // A hidden number: no key.
        JSONObject h = new JSONObject(Calls.line(Calls.decideFor(table, "", true, at + 3_000_000, 0, null, 0, phone), table));
        expect("hidden", h.getBoolean("hidden") && h.getString("key").isEmpty() && h.getString("who").equals("hidden") && !h.getBoolean("rang"));
        lines.add(h.toString());
        // No table: it rings, the number kept for Rust to key.
        JSONObject n = new JSONObject(Calls.line(Calls.decideFor(null, "04 65 71 00 42", false, at + 3_600_000, 0, null, 0, phone), null));
        expect("no table", n.getBoolean("rang") && n.getString("why").equals("no-table") && n.getString("key").isEmpty() && n.getString("number").equals("04 65 71 00 42") && n.getString("column").isEmpty());
        lines.add(n.toString());
        // An outgoing call, or one never decided: nothing written, nothing asked of the phone.
        Calls.Decision out = Calls.Decision.ring("outgoing");
        out.at = at;
        Calls.held(null, out);
        Calls.held(null, Calls.Decision.ring("error"));
        // Appended; past its size, its lines older than a month dropped first.
        File folder = Files.createTempDirectory("sioul-calls-log").toFile();
        StringBuilder old = new StringBuilder();
        long longAgo = at - 40L * 24 * 3600 * 1000;
        while (old.length() < 140 * 1024)
            old.append(new JSONObject(d.toString()).put("at", longAgo).toString()).append('\n');
        Files.write(new File(folder, Calls.HELD).toPath(), old.toString().getBytes(StandardCharsets.UTF_8));
        Calls.append(folder, d.toString(), at);
        List<String> kept = Files.readAllLines(new File(folder, Calls.HELD).toPath());
        expect("trimmed to a month, then appended: " + kept.size(), kept.size() == 1 && kept.get(0).equals(d.toString()));
        File[] left = folder.listFiles();
        for (File file : left == null ? new File[0] : left)
            file.delete();
        folder.delete();
        String sample = System.getenv("SIOUL_CALLS_SAMPLE");
        if (sample != null && !sample.isEmpty())
            Files.write(new File(sample).toPath(), (String.join("\n", lines) + "\n").getBytes(StandardCharsets.UTF_8));
        System.out.println(failed == 0 ? "LogCheck: all lines as Rust reads them" : "LogCheck: " + failed + " failed");
        if (failed > 0)
            System.exit(1);
    }
}
