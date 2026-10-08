// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.json.JSONObject;

/**
 * The decisions of phase 5's Java, on the JVM (org.json's own jar before
 * android.jar's stubs on the class path):
 * - PauseMode.Through.of: the policy's parts from Rust's ask (dnd.rs: the
 *   matrix's Silence flattened beside "people" and "doses"; nested; older);
 * - PauseMode.passesNow: which modes on let a channel that passes be used;
 * - EventAlarms.isAlarm: an event's own alarm, by Rust's kind or its key;
 * - AppNotes.talkKey against Rust's talk_key (values from Rust's own code,
 *   rustc on a verbatim copy of appnotes.rs's fnv and talk_key);
 * - AppNotes.forgotten, AppNotes.pages.
 */
public final class ModeCheck
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

    /** "calls messages repeat conversations alarms doses events", ZenPolicy's numbers for the senders. */
    static String through(String json) throws Exception
    {
        PauseMode.Through t = PauseMode.Through.of(new JSONObject(json));
        return t.calls + " " + t.messages + " " + t.repeat + " " + t.conversations + " " + t.alarms + " " + t.doses + " " + t.events + " " + t.conversationSenders();
    }

    public static void main(String[] args) throws Exception
    {
        // ZenPolicy: ANYONE 1, CONTACTS 2, STARRED 3, NONE 4; CONVERSATION_SENDERS IMPORTANT 2, NONE 3.
        // dnd.rs today: Silence's fields flattened beside the older ones.
        expect("pause, screening, listener", through("{\"kind\":\"pause\",\"people\":true,\"doses\":true,\"calls\":\"contacts\",\"messages\":\"starred\",\"repeat\":true,\"conversations\":true,\"alarms\":true,\"events\":true}"), "2 3 true true true true true 2");
        expect("free time, anyone may call", through("{\"people\":true,\"calls\":\"anyone\",\"messages\":\"anyone\",\"repeat\":true,\"conversations\":true,\"alarms\":true,\"doses\":true,\"events\":true}"), "1 1 true true true true true 2");
        expect("nothing at all", through("{\"people\":false,\"doses\":true,\"calls\":\"none\",\"messages\":\"none\",\"repeat\":false,\"conversations\":false,\"alarms\":true,\"events\":true}"), "4 4 false false true true true 3");
        expect("doses and alarms held", through("{\"people\":true,\"calls\":\"starred\",\"messages\":\"starred\",\"repeat\":true,\"conversations\":true,\"alarms\":true,\"doses\":false,\"events\":false}"), "3 3 true true true false false 2");
        // Nested, as "silence".
        expect("nested", through("{\"people\":true,\"doses\":true,\"silence\":{\"calls\":\"contacts\",\"messages\":\"none\",\"repeat\":true,\"conversations\":false,\"alarms\":true,\"doses\":true,\"events\":false}}"), "2 4 true false true true false 3");
        expect("silence null", through("{\"people\":false,\"doses\":false,\"silence\":null}"), "4 4 false false true false true 3");
        // An older Sioul: "people" only, as before (starred and repeat callers, or nobody; no conversation).
        expect("older, people", through("{\"kind\":\"pause\",\"people\":true,\"doses\":true}"), "3 3 true false true true true 3");
        expect("older, nobody", through("{\"kind\":\"pause\",\"people\":false,\"doses\":false}"), "4 4 false false true false true 3");
        expect("nothing said", through("{}"), "3 3 true false true true true 3");
        // A word not known: as "people" says.
        expect("unknown word", through("{\"people\":false,\"calls\":\"favourites\",\"messages\":\"\"}"), "4 4 false false true true true 3");

        // Which modes on let a channel that passes be used.
        Map<String, Object> kept = new HashMap<>();
        expect("no mode on", PauseMode.passesNow(kept, "doses."), false);
        expect("no mode on, mail", PauseMode.passesNow(kept, null), false);
        kept.put("on.pause", true);
        kept.put("doses.pause", true);
        kept.put("events.pause", true);
        expect("pause lets doses", PauseMode.passesNow(kept, "doses."), true);
        expect("pause lets events", PauseMode.passesNow(kept, "events."), true);
        expect("pause, mail", PauseMode.passesNow(kept, null), true);
        kept.put("on.dnd", true);
        kept.put("events.dnd", false);
        expect("one mode on holds events", PauseMode.passesNow(kept, "events."), false);
        expect("doses not said by dnd: let through, as before", PauseMode.passesNow(kept, "doses."), true);
        kept.put("on.pause", false);
        kept.put("on.dnd", false);
        expect("modes off", PauseMode.passesNow(kept, "doses."), false);
        kept.put("on.free-time", true);
        expect("free time entered by an older Sioul: nothing said", PauseMode.passesNow(kept, "events."), true);

        // An event's own alarm.
        expect("kind alarm", EventAlarms.isAlarm("alarm", "alarm:uid:1:0"), true);
        expect("kind before", EventAlarms.isAlarm("before", "before:uid:1"), false);
        expect("kind event", EventAlarms.isAlarm("event", "event:uid:1"), false);
        expect("older list, alarm by key", EventAlarms.isAlarm("", "alarm:uid:1:-900"), true);
        expect("older list, before by key", EventAlarms.isAlarm("", "before:uid:1"), false);
        expect("nothing", EventAlarms.isAlarm(null, null), false);

        // A conversation's key, as Rust's talk_key (values from Rust's own code).
        String[][] keys = {
            { "com.whatsapp", "120363000000000001@g.us", "58006e1412c3b446" },
            { "com.discord", "110022003344556677", "e0c6526c72b3d0c5" },
            { "com.google.android.apps.messaging", "conv-41", "ded279d56ac1079f" },
            { "org.example", "École Jules Ferry ✓", "284875702b05a347" },
            { "", "", "af63d24c8601db8e" },
        };
        for (String[] k : keys)
            expect("talk_key " + k[0] + " " + k[1], AppNotes.talkKey(k[0], k[1]), k[2]);

        // Forgotten: unseen for over eight days, broken, then the oldest past 300.
        long now = 1_800_000_000_000L;
        Map<String, Object> talks = new HashMap<>();
        talks.put("fresh", "{\"at\":" + (now - 3600_000L) + "}");
        talks.put("old", "{\"at\":" + (now - 9L * 24 * 3600_000L) + "}");
        talks.put("broken", "not json");
        talks.put("no-time", "{\"package\":\"x\"}");
        List<String> gone = AppNotes.forgotten(talks, now);
        java.util.Collections.sort(gone);
        expect("forgotten", gone, Arrays.asList("broken", "no-time", "old"));
        Map<String, Object> many = new HashMap<>();
        for (int i = 0; i < 305; i++)
            many.put("k" + i, "{\"at\":" + (now - (305 - i) * 1000L) + "}");
        List<String> past = AppNotes.forgotten(many, now);
        java.util.Collections.sort(past);
        expect("the five oldest past 300", past, Arrays.asList("k0", "k1", "k2", "k3", "k4"));

        // Android's pages for a conversation, in order.
        expect("Android 10", Arrays.toString(AppNotes.pages(29, true, true, true)), "[app]");
        expect("not a conversation in Android", Arrays.toString(AppNotes.pages(31, false, false, true)), "[app]");
        expect("its own page", Arrays.toString(AppNotes.pages(31, true, true, true)), "[conversation, conversations, app]");
        expect("not changed yet", Arrays.toString(AppNotes.pages(31, true, false, true)), "[conversations, conversation, app]");
        expect("no channel known", Arrays.toString(AppNotes.pages(34, true, true, false)), "[conversations, app]");

        System.out.println(failed == 0 ? "all " + checked + " checks agree" : failed + " of " + checked + " failed");
        System.exit(failed == 0 ? 0 : 1);
    }
}
