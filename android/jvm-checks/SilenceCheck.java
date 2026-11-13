// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import org.json.JSONObject;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/**
 * The channels Sioul makes silent itself, on the JVM (ChannelSilence; option
 * 2 of docs/research/android-holding.md), against a stand-in phone and a
 * stand-in of Rust's record: each channel's importance written down before
 * it changes, never after; one silent already or gone left, nothing written;
 * Rust refusing to write it down (a file of a later Sioul) changes nothing;
 * Android refusing a change, what was written forgotten; given back only
 * while still as Sioul left it, else left as you set it, and forgotten
 * either way; Android refusing to read (the link gone) stops everything,
 * nothing forgotten. Then, read from the sources: the association asks for
 * no profile and no permission, the feature is not required, the activity
 * and the listener's receiver are never exported, Android's chooser shows
 * the paired devices (a Bluetooth filter), channels changed in one place
 * for this phone's own profile, the link let go only when Rust says nothing
 * is left.
 */
public final class SilenceCheck
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

    /** Other apps' channels, and what was asked of them, in order. */
    static final class Phone implements ChannelSilence.Channels
    {
        final Map<String, Integer> importance = new TreeMap<>();
        final List<String> said = new ArrayList<>();
        boolean denied;
        String refuses = "";

        @Override
        public int importance(String pkg, String channel)
        {
            if (denied)
                return ChannelSilence.DENIED;
            Integer found = importance.get(pkg + " " + channel);
            return found == null ? ChannelSilence.GONE : found;
        }

        @Override
        public String name(String pkg, String channel)
        {
            return channel.substring(0, 1).toUpperCase() + channel.substring(1);
        }

        @Override
        public boolean set(String pkg, String channel, int value)
        {
            said.add("set " + pkg + " " + channel + " " + value);
            if (refuses.equals(pkg + " " + channel))
                return false;
            importance.put(pkg + " " + channel, value);
            return true;
        }
    }

    /** Rust's record (appnotes::Silenced), and what was asked of it, in the same order as the phone's. */
    static final class Record implements ChannelSilence.Keeper
    {
        final Map<String, Integer> kept = new TreeMap<>();
        final List<String> said;
        boolean later;

        Record(List<String> said)
        {
            this.said = said;
        }

        @Override
        public boolean keep(String pkg, String channel, String name, int was)
        {
            said.add("keep " + pkg + " " + channel + " " + name + " " + was);
            if (later)
                return false;
            kept.put(pkg + " " + channel, was);
            return true;
        }

        @Override
        public void forget(String pkg, String channel)
        {
            said.add("forget " + pkg + " " + channel);
            kept.remove(pkg + " " + channel);
        }
    }

    static String sources(String name) throws IOException
    {
        return new String(Files.readAllBytes(Paths.get(System.getProperty("sioul.root"), "android", "package", "src", "com", "aurelienpierre", "sioul", name)), StandardCharsets.UTF_8);
    }

    public static void main(String[] args) throws Exception
    {
        // Made silent: the shop's offers (a sound and a banner), its news (a sound); its orders silent
        // already, as you set them; a channel its app deleted.
        Phone phone = new Phone();
        Record record = new Record(phone.said);
        phone.importance.put("com.shop.example offers", 4);
        phone.importance.put("com.shop.example news", 3);
        phone.importance.put("com.shop.example orders", 2);
        JSONObject plan = new JSONObject("{\"silence\":[{\"package\":\"com.shop.example\",\"channel\":\"offers\"},{\"package\":\"com.shop.example\",\"channel\":\"news\"},"
                                         + "{\"package\":\"com.shop.example\",\"channel\":\"orders\"},{\"package\":\"com.shop.example\",\"channel\":\"deleted\"},{\"package\":\"\",\"channel\":\"x\"}]}");
        int[] done = ChannelSilence.apply(plan, phone, record);
        expect("two made silent", done[0] + " " + done[1], "2 0");
        expect("written down, then changed, each",
               phone.said, "[keep com.shop.example offers Offers 4, set com.shop.example offers 2, keep com.shop.example news News 3, set com.shop.example news 2]");
        expect("the record", record.kept, "{com.shop.example news=3, com.shop.example offers=4}");

        // Rust cannot write it down (a file of a later Sioul, one that does not read): nothing changes.
        phone = new Phone();
        record = new Record(phone.said);
        record.later = true;
        phone.importance.put("com.shop.example offers", 4);
        ChannelSilence.apply(new JSONObject("{\"silence\":[{\"package\":\"com.shop.example\",\"channel\":\"offers\"}]}"), phone, record);
        expect("not written down, not changed", phone.said, "[keep com.shop.example offers Offers 4]");
        expect("still ringing", phone.importance.get("com.shop.example offers"), 4);

        // Android refuses the change: what was written down is forgotten, nothing to give back.
        phone = new Phone();
        record = new Record(phone.said);
        phone.importance.put("com.shop.example offers", 4);
        phone.refuses = "com.shop.example offers";
        done = ChannelSilence.apply(new JSONObject("{\"silence\":[{\"package\":\"com.shop.example\",\"channel\":\"offers\"}]}"), phone, record);
        expect("refused", done[0], 0);
        expect("forgotten", record.kept.isEmpty(), true);
        expect("in that order", phone.said, "[keep com.shop.example offers Offers 4, set com.shop.example offers 2, forget com.shop.example offers]");

        // Given back: the offers as Sioul left them, to 4; the news changed since by hand (left at 3);
        // the app of the orders uninstalled; one Android refuses to give back stays written down, to try again.
        phone = new Phone();
        record = new Record(phone.said);
        record.kept.put("com.shop.example offers", 4);
        record.kept.put("com.shop.example news", 3);
        record.kept.put("org.example.gone orders", 3);
        record.kept.put("com.shop.example stuck", 4);
        phone.importance.put("com.shop.example offers", 2);
        phone.importance.put("com.shop.example news", 3);
        phone.importance.put("com.shop.example stuck", 2);
        phone.refuses = "com.shop.example stuck";
        done = ChannelSilence.apply(new JSONObject("{\"restore\":[{\"package\":\"com.shop.example\",\"channel\":\"offers\",\"importance\":4},"
                                                   + "{\"package\":\"com.shop.example\",\"channel\":\"news\",\"importance\":4},"
                                                   + "{\"package\":\"org.example.gone\",\"channel\":\"orders\",\"importance\":3},"
                                                   + "{\"package\":\"com.shop.example\",\"channel\":\"stuck\",\"importance\":4}]}"), phone, record);
        expect("one given back", done[1], 1);
        expect("as it was", phone.importance.get("com.shop.example offers"), 4);
        expect("changed by hand, left", phone.importance.get("com.shop.example news"), 3);
        expect("never set twice", phone.said.stream().filter(s -> s.startsWith("set com.shop.example news")).count(), 0);
        expect("only the refused one still written down", record.kept, "{com.shop.example stuck=4}");

        // The link gone: Android refuses to read; everything stops, nothing forgotten, nothing changed.
        phone = new Phone();
        record = new Record(phone.said);
        record.kept.put("com.shop.example offers", 4);
        phone.denied = true;
        done = ChannelSilence.apply(new JSONObject("{\"silence\":[{\"package\":\"com.shop.example\",\"channel\":\"news\"}],"
                                                   + "\"restore\":[{\"package\":\"com.shop.example\",\"channel\":\"offers\",\"importance\":4}]}"), phone, record);
        expect("nothing asked", phone.said, "[]");
        expect("kept to give back later", record.kept, "{com.shop.example offers=4}");
        expect("nothing done", done[0] + done[1], 0);
        expect("no plan, nothing", ChannelSilence.apply(null, new Phone(), new Record(new ArrayList<>()))[0], 0);

        // Read from the sources.
        String companion = sources("Companion.java");
        String notes = sources("AppNotes.java");
        String silence = sources("ChannelSilence.java");
        String manifest = new String(Files.readAllBytes(Paths.get(System.getProperty("sioul.root"), "android", "package", "AndroidManifest.xml")), StandardCharsets.UTF_8);
        expect("no device profile asked", companion.contains("setDeviceProfile") || companion.contains("DEVICE_PROFILE"), false);
        expect("never self-managed", companion.contains("setSelfManaged"), false);
        expect("Android's list shows the paired devices: a Bluetooth filter", companion.contains("new BluetoothDeviceFilter.Builder().build()"), true);
        expect("the feature not required", manifest.contains("<uses-feature android:name=\"android.software.companion_device_setup\" android:required=\"false\" />"), true);
        expect("no permission for it", manifest.contains("REQUEST_COMPANION"), false);
        int activity = manifest.indexOf("com.aurelienpierre.sioul.Companion\"");
        expect("the activity never exported", activity > 0 && manifest.substring(activity, manifest.indexOf("/>", activity)).contains("android:exported=\"false\""), true);
        expect("the listener told once linked", companion.contains("new Intent(this, AppNotes.Asked.class).setAction(AppNotes.LINKED)"), true);
        expect("only on a yes", companion.indexOf("if (result == RESULT_OK)\n                sendBroadcast(") > 0, true);
        expect("written down before the change", silence.indexOf("keeper.keep(") < silence.indexOf("channels.set(pkg, channel, LOW)"), true);
        expect("channels changed in one place", notes.split("updateNotificationChannel\\(", -1).length - 1, 1);
        expect("this phone's own profile", notes.contains("updateNotificationChannel(pkg, Process.myUserHandle(), found)"), true);
        expect("the link let go only as Rust says", notes.indexOf("unlink(this);") > notes.indexOf("plan.optBoolean(\"unlink\", false)") && notes.split("unlink\\(this\\)", -1).length - 1 == 1, true);
        expect("counts only in the log", notes.contains("\"Notes: channels: \" + done[0] + \" made silent, \" + done[1] + \" given back.\""), true);

        System.out.println("SilenceCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
