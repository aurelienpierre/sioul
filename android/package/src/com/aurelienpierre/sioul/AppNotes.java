// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.Manifest;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Person;
import android.content.ActivityNotFoundException;
import android.content.ComponentName;
import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.content.pm.ShortcutInfo;
import android.content.res.Resources;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.FileObserver;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.Parcelable;
import android.os.Process;
import android.provider.ContactsContract;
import android.provider.Settings;
import android.provider.Telephony;
import android.service.notification.NotificationListenerService;
import android.service.notification.StatusBarNotification;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * Other apps' notifications held until their time (docs/android.md,
 * "Notifications from other apps"; crates/sioul-app/src/appnotes.rs, which
 * decides; crates/sioul-core/src/appnotes.rs, how).
 *
 * Android's notification listener, bound by the system once the person gives
 * Sioul "Notification access", in a process of its own (":listener"): Qt's
 * window ends its whole process as it closes (System.exit in
 * QtActivityBase.onDestroy), which would unbind the listener at each close,
 * and the background service's process (":steps") runs only when its own
 * setting is on and does long work a notification must not wait for. Here,
 * Sioul's library without Qt's window (DoseAlarms.load), as for the doses.
 *
 * For each notification of another app: what Android says of it, its words
 * included, handed to Rust in memory on a thread of its own; Rust answers how
 * long it waits, and Android holds it that long (snoozeNotification): it
 * comes back whole, with its own tap and actions, alerting as its channel
 * says. Nothing is ever cancelled, answered, marked as read, kept or logged;
 * of a conversation, only where its page is in Android (its app, shortcut
 * and channel, a week), for the Other apps tab to open it.
 * Never asked about: Sioul's own, ongoing ones (a call, music, a download),
 * those that cannot be cleared.
 *
 * What is held is worked out again (`review`) when something changes that
 * moves its time: a pause ends, the switch of do-not-disturb, the hours, the
 * choices. Sioul's files are watched for that.
 */
public final class AppNotes extends NotificationListenerService
{
    static final String TAG = DoseAlarms.TAG;
    /** Android's words in a notification it redacted for an untrusted listener (Android 15). */
    private static final String REDACTED = "redacted_notification_message";
    /** A sound or vibration this recent was the notification's own, as it came. */
    private static final long ALERTED_MS = 15_000;
    /** Changes to Sioul's files settle this long before what is held is worked out again. */
    private static final long SETTLE_MS = 2_000;
    /** The files whose change may move what is held: the pause and free time, the
     *  switch of do-not-disturb and its people, a focus session, the hours and
     *  settings (who may reach you when among them), the lists of senders, the
     *  choices, Health's meals and nights. */
    private static final String[] WATCHED = {
        "quiet.toml", "do-not-disturb.toml", "dnd-people.toml", "running.toml", "config.toml", "app-notes.toml", "health.toml", "health-days.toml",
        "safe-senders.txt", "neutral-senders.txt", "restricted-senders.txt", "blocked-senders.txt",
    };

    private static volatile boolean connected;

    private HandlerThread thread;
    private Handler worker;
    private final List<FileObserver> watches = new ArrayList<>();
    private final Map<String, String> labels = new HashMap<>();
    private String redactedWords;
    private String smsApp = "";
    private long smsAppAt;
    private final Runnable review = () -> review("files");

    /** Rust's answer for one notification: {"hold": milliseconds, 0 to let it come, "why"}. */
    static native String nativeDecide(String json);

    /** What is held, worked out again: {"snooze": [{"key", "ms"}]}, for the keys Android holds now. */
    static native String nativeReview(String json);

    // ---------------------------------------------------------------- Rust's questions

    /**
     * Rust's question (android/main.cpp), from the window's process: the
     * access, Android's pages for it and for each app's, channel's or
     * conversation's notifications, the contacts' permission, what Android
     * says of the conversations seen lately. Any thread.
     */
    static String call(Context context, String verb, String json)
    {
        try {
            JSONObject asked = json == null || json.isEmpty() ? new JSONObject() : new JSONObject(json);
            switch (verb) {
            case "conversations":
                return conversations(context).toString();
            case "open-conversation":
                return openConversation(context, asked) ? "true" : "false";
            case "access":
                return granted(context) ? "true" : "false";
            case "restricted":
                // Android 13 and later grey the access out for an app installed from a file, until allowed.
                return Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU ? "true" : "false";
            case "open-access":
                return openAccess(context) ? "true" : "false";
            case "open-info":
                return open(context, new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:" + context.getPackageName()))) ? "true" : "false";
            case "open-app":
                return open(context, new Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, asked.optString("package", ""))) ? "true" : "false";
            case "open-channel":
                return open(context, new Intent(Settings.ACTION_CHANNEL_NOTIFICATION_SETTINGS)
                    .putExtra(Settings.EXTRA_APP_PACKAGE, asked.optString("package", ""))
                    .putExtra(Settings.EXTRA_CHANNEL_ID, asked.optString("channel", ""))) ? "true" : "false";
            case "contacts":
                return context.checkSelfPermission(Manifest.permission.READ_CONTACTS) == PackageManager.PERMISSION_GRANTED ? "true" : "false";
            default:
                return null;
            }
        } catch (JSONException | RuntimeException e) {
            Log.e(TAG, "Notes: " + verb + " failed: " + e);
            return null;
        }
    }

    private static ComponentName component(Context context)
    {
        return new ComponentName(context, AppNotes.class);
    }

    /** Whether the person gave Sioul "Notification access". */
    static boolean granted(Context context)
    {
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        return notifications != null && notifications.isNotificationListenerAccessGranted(component(context));
    }

    /** Android's page for Sioul's notification access: its own switch from Android 11, else the list. */
    private static boolean openAccess(Context context)
    {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Intent detail = new Intent(Settings.ACTION_NOTIFICATION_LISTENER_DETAIL_SETTINGS)
                .putExtra(Settings.EXTRA_NOTIFICATION_LISTENER_COMPONENT_NAME, component(context).flattenToString());
            if (open(context, detail))
                return true;
        }
        return open(context, new Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS));
    }

    private static boolean open(Context context, Intent page)
    {
        try {
            context.startActivity(page.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            return true;
        } catch (ActivityNotFoundException | SecurityException e) {
            Log.w(TAG, "Notes: no page " + page.getAction() + ": " + e);
            return false;
        }
    }

    // ---------------------------------------------------------------- conversations marked important

    /**
     * What Android says of each conversation seen lately, by Rust's key for it
     * (crates/sioul-core/src/appnotes.rs, `talk_key` of its app and its
     * shortcut): its app, its shortcut and its channel, so that the Other
     * apps tab can open its page in Android, where a conversation set to
     * Always through is marked important (priority), which Sioul's modes let
     * through (PauseMode). This phone's alone, in Sioul's private storage,
     * never shared; forgotten after a week unseen, as the tab forgets its name.
     */
    private static final String TALKS = "sioul-conversations";
    private static final long TALK_KEPT_MS = 8L * 24 * 3600 * 1000;
    private static final int TALKS_AT_MOST = 300;
    /** Written again at most this often when nothing changed: not at each message. */
    private static final long TALK_AGAIN_MS = 24L * 3600 * 1000;
    /** Android's list of conversations (Settings ▸ Notifications ▸ Conversations), Android 11 and later. */
    private static final String CONVERSATIONS_PAGE = "android.settings.CONVERSATION_SETTINGS";

    /** Written by the listener's process, read by the window's. */
    @SuppressWarnings("deprecation")
    private static SharedPreferences talks(Context context)
    {
        return context.getSharedPreferences(TALKS, Context.MODE_PRIVATE | Context.MODE_MULTI_PROCESS);
    }

    /**
     * A conversation's key as Rust's `talk_key` makes it: FNV-1a (64 bits)
     * of its app, U+001F and its id, in hexadecimal. Pure, checked on a JVM
     * against Rust's values (android/jvm-checks/ModeCheck.java).
     */
    static String talkKey(String pkg, String id)
    {
        long hash = 0xcbf29ce484222325L;
        for (byte b : (pkg + "\u001f" + id).getBytes(StandardCharsets.UTF_8)) {
            hash ^= b & 0xff;
            hash *= 0x100000001b3L;
        }
        return String.format(Locale.ROOT, "%016x", hash);
    }

    /**
     * The keys to forget among those kept (each a JSON text with its time
     * "at"): those unseen for a week, then the oldest past the most kept.
     * Pure, checked on a JVM (android/jvm-checks/ModeCheck.java).
     */
    static List<String> forgotten(Map<String, ?> kept, long now)
    {
        List<String> gone = new ArrayList<>();
        List<Map.Entry<String, Long>> left = new ArrayList<>();
        for (Map.Entry<String, ?> entry : kept.entrySet()) {
            long at = 0;
            try {
                at = new JSONObject(String.valueOf(entry.getValue())).optLong("at", 0);
            } catch (JSONException e) {
                // Not one of ours: forgotten.
            }
            if (at <= 0 || now - at > TALK_KEPT_MS)
                gone.add(entry.getKey());
            else
                left.add(new java.util.AbstractMap.SimpleEntry<>(entry.getKey(), at));
        }
        if (left.size() > TALKS_AT_MOST) {
            left.sort((a, b) -> Long.compare(a.getValue(), b.getValue()));
            for (int i = 0; i < left.size() - TALKS_AT_MOST; i++)
                gone.add(left.get(i).getKey());
        }
        return gone;
    }

    /**
     * A conversation seen (the listener, on its worker thread): its app, its
     * shortcut, the channel it was posted on (its parent's when Android gave
     * it a channel of its own), whether Android takes it as a conversation,
     * whether it is marked important there, and whether it has a page of its
     * own there yet (customized once). Written when something changed, or
     * once a day.
     */
    private static void remember(Context context, String pkg, String shortcut, String channel, NotificationChannel given, boolean conversation, long now)
    {
        boolean important = false;
        boolean own = false;
        String parent = channel;
        if (given != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            important = given.isImportantConversation();
            own = given.getConversationId() != null;
            if (given.getParentChannelId() != null)
                parent = given.getParentChannelId();
        }
        String key = talkKey(pkg, shortcut);
        SharedPreferences talks = talks(context);
        try {
            String before = talks.getString(key, null);
            if (before != null) {
                JSONObject was = new JSONObject(before);
                boolean same = pkg.equals(was.optString("package")) && shortcut.equals(was.optString("shortcut")) && parent.equals(was.optString("channel"))
                    && conversation == was.optBoolean("conversation") && important == was.optBoolean("important") && own == was.optBoolean("own");
                if (same && now - was.optLong("at", 0) < TALK_AGAIN_MS)
                    return;
            }
            JSONObject seen = new JSONObject().put("package", pkg).put("shortcut", shortcut).put("channel", parent)
                .put("conversation", conversation).put("important", important).put("own", own).put("at", now);
            SharedPreferences.Editor edit = talks.edit().putString(key, seen.toString());
            if (before == null)
                for (String old : forgotten(talks.getAll(), now))
                    if (!old.equals(key))
                        edit.remove(old);
            edit.apply();
        } catch (JSONException | RuntimeException e) {
            Log.w(TAG, "Notes: a conversation not kept: " + e);
        }
    }

    /** One conversation as kept, by Rust's key; null when not seen lately. */
    private static JSONObject talk(Context context, String key)
    {
        String kept = key.isEmpty() ? null : talks(context).getString(key, null);
        try {
            return kept == null ? null : new JSONObject(kept);
        } catch (JSONException e) {
            return null;
        }
    }

    /**
     * What Android says of the conversations seen lately, for the Other apps
     * tab: {api, conversations: {key: {important, page}}}: marked important
     * there; `page`, Android takes it as a conversation, whose page Sioul
     * can open (Android 11 and later).
     */
    static JSONObject conversations(Context context) throws JSONException
    {
        JSONObject known = new JSONObject();
        long now = System.currentTimeMillis();
        for (Map.Entry<String, ?> entry : talks(context).getAll().entrySet()) {
            try {
                JSONObject seen = new JSONObject(String.valueOf(entry.getValue()));
                if (now - seen.optLong("at", 0) > TALK_KEPT_MS)
                    continue;
                boolean page = Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && seen.optBoolean("conversation");
                known.put(entry.getKey(), new JSONObject().put("important", seen.optBoolean("important")).put("page", page));
            } catch (JSONException e) {
                // Not one of ours.
            }
        }
        return new JSONObject().put("api", Build.VERSION.SDK_INT).put("conversations", known);
    }

    /**
     * Which of Android's pages a conversation is opened at, in order, the
     * next one when a page does not open: "conversation", its own page
     * (where Priority is), once Android gave it one; "conversations", the
     * list of conversations, where one not changed yet shows among the
     * recent ones; "app", the app's notifications. Pure, checked on a JVM
     * (android/jvm-checks/ModeCheck.java).
     */
    static String[] pages(int api, boolean conversation, boolean own, boolean channel)
    {
        if (api < 30 || !conversation)
            return new String[] { "app" };
        if (own && channel)
            return new String[] { "conversation", "conversations", "app" };
        return channel ? new String[] { "conversations", "conversation", "app" } : new String[] { "conversations", "app" };
    }

    /**
     * A conversation's page in Android: {key} (Rust's key for it), or
     * {package, shortcut, channel}. False when no page opened.
     */
    private static boolean openConversation(Context context, JSONObject asked)
    {
        JSONObject seen = talk(context, asked.optString("key", ""));
        String pkg = seen != null ? seen.optString("package") : asked.optString("package", "");
        String shortcut = seen != null ? seen.optString("shortcut") : asked.optString("shortcut", "");
        String channel = seen != null ? seen.optString("channel") : asked.optString("channel", "");
        boolean conversation = seen == null ? !shortcut.isEmpty() : seen.optBoolean("conversation");
        boolean own = seen != null && seen.optBoolean("own");
        if (pkg.isEmpty())
            return Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && open(context, new Intent(CONVERSATIONS_PAGE));
        for (String page : pages(Build.VERSION.SDK_INT, conversation && !shortcut.isEmpty(), own, !channel.isEmpty())) {
            Intent intent;
            switch (page) {
            case "conversation":
                intent = new Intent(Settings.ACTION_CHANNEL_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, pkg)
                    .putExtra(Settings.EXTRA_CHANNEL_ID, channel).putExtra(Settings.EXTRA_CONVERSATION_ID, shortcut);
                break;
            case "conversations":
                intent = new Intent(CONVERSATIONS_PAGE);
                break;
            default:
                intent = new Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, pkg);
            }
            if (open(context, intent))
                return true;
        }
        return false;
    }

    // ---------------------------------------------------------------- the listener

    @Override
    public void onCreate()
    {
        super.onCreate();
        thread = new HandlerThread("sioul-notes", Process.THREAD_PRIORITY_BACKGROUND);
        thread.start();
        worker = new Handler(thread.getLooper());
    }

    @Override
    public void onDestroy()
    {
        connected = false;
        unwatch();
        thread.quitSafely();
        super.onDestroy();
    }

    @Override
    public void onListenerConnected()
    {
        connected = true;
        worker.post(() -> {
            watch();
            review("connected");
        });
    }

    /**
     * The phone's do-not-disturb changed, whatever changed it: Android tells a
     * bound listener of every change (NotificationManagerService,
     * notifyInterruptionFilterChanged), a word surer than the broadcast to
     * the packages with Do Not Disturb access. Decided in Sioul's own process,
     * where its modes are kept (DndReceiver.HEARD), as the broadcast is.
     */
    @Override
    public void onInterruptionFilterChanged(int interruptionFilter)
    {
        DndReceiver.forward(getApplicationContext());
    }

    @Override
    public void onListenerDisconnected()
    {
        // The access taken back, or Android unbinding: nothing is held from here until it binds again.
        connected = false;
        worker.post(this::unwatch);
    }

    @Override
    public void onNotificationPosted(StatusBarNotification sbn, RankingMap rankings)
    {
        if (sbn == null || getPackageName().equals(sbn.getPackageName()))
            return;
        Notification n = sbn.getNotification();
        int kept = Notification.FLAG_ONGOING_EVENT | Notification.FLAG_FOREGROUND_SERVICE | Notification.FLAG_NO_CLEAR;
        if (n == null || (n.flags & kept) != 0 || !sbn.isClearable())
            return;
        // The ranking as Android gave it with this notification: its channel, whether it rang.
        Ranking ranking = new Ranking();
        boolean ranked = rankings != null && rankings.getRanking(sbn.getKey(), ranking);
        Ranking given = ranked ? ranking : null;
        worker.post(() -> decide(sbn, given));
    }

    /** Rust asked, the answer followed: held so long, or left as it came. On the worker thread. */
    private void decide(StatusBarNotification sbn, Ranking ranking)
    {
        if (!connected)
            return;
        try {
            String said = describe(sbn, ranking).toString();
            DoseAlarms.load(getApplicationContext());
            String answer = nativeDecide(said);
            if (answer == null || answer.isEmpty() || "null".equals(answer))
                return;
            long hold = new JSONObject(answer).optLong("hold", 0);
            if (hold > 0)
                snooze(sbn.getKey(), hold);
            // A line written for your computers (crates/sioul-app/src/phonemsgs.rs): shared at a step soon.
            if (new JSONObject(answer).optBoolean("shared", false))
                StepService.soon(getApplicationContext());
        } catch (Throwable e) {
            // Sioul's library that does not load, Rust that fails: the notification stays as it came.
            Log.e(TAG, "Notes: a notification was not decided; it comes as sent.", e);
        }
    }

    private void snooze(String key, long ms)
    {
        try {
            snoozeNotification(key, Math.max(1000, ms));
        } catch (RuntimeException e) {
            // The access taken back meanwhile: nothing held.
            Log.w(TAG, "Notes: not held: " + e);
        }
    }

    /** What is held, worked out again; Android told the new times. On the worker thread. */
    private void review(String reason)
    {
        if (!connected)
            return;
        try {
            StatusBarNotification[] held = getSnoozedNotifications();
            JSONArray keys = new JSONArray();
            if (held != null)
                for (StatusBarNotification sbn : held)
                    keys.put(sbn.getKey());
            DoseAlarms.load(getApplicationContext());
            String answer = nativeReview(new JSONObject().put("snoozed", keys).put("reason", reason).toString());
            if (answer == null || answer.isEmpty() || "null".equals(answer))
                return;
            JSONArray again = new JSONObject(answer).optJSONArray("snooze");
            for (int i = 0; again != null && i < again.length(); i++) {
                JSONObject one = again.optJSONObject(i);
                if (one != null && !one.optString("key", "").isEmpty())
                    snooze(one.optString("key"), one.optLong("ms", 1000));
            }
        } catch (Throwable e) {
            Log.e(TAG, "Notes: what is held was not worked out again (" + reason + ")", e);
        }
    }

    // ---------------------------------------------------------------- Sioul's files watched

    /** The folders of Sioul's state, configuration and data, watched for the files that move what is held. */
    private void watch()
    {
        unwatch();
        File files = getFilesDir();
        for (String folder : new String[] { "state/sioul", "config/sioul", "data/sioul", "data/sioul/time" }) {
            String path = new File(files, folder).getAbsolutePath();
            @SuppressWarnings("deprecation")
            FileObserver observer = new FileObserver(path, FileObserver.CLOSE_WRITE | FileObserver.MOVED_TO) {
                @Override
                public void onEvent(int event, String name)
                {
                    if (name == null)
                        return;
                    for (String watched : WATCHED) {
                        if (watched.equals(name)) {
                            worker.removeCallbacks(review);
                            worker.postDelayed(review, SETTLE_MS);
                            return;
                        }
                    }
                }
            };
            observer.startWatching();
            watches.add(observer);
        }
    }

    private void unwatch()
    {
        for (FileObserver observer : watches)
            observer.stopWatching();
        watches.clear();
    }

    // ---------------------------------------------------------------- what Android says of a notification

    /** The notification as Rust reads it (crates/sioul-core/src/appnotes.rs, `Posted`). */
    @SuppressWarnings("deprecation")
    private JSONObject describe(StatusBarNotification sbn, Ranking ranking) throws JSONException
    {
        Notification n = sbn.getNotification();
        Bundle extras = n.extras == null ? new Bundle() : n.extras;
        String app = label(sbn.getPackageName(), extras);
        JSONObject out = new JSONObject();
        out.put("key", sbn.getKey());
        out.put("package", sbn.getPackageName());
        out.put("app", app);
        out.put("category", n.category == null ? "" : n.category);
        out.put("tag", sbn.getTag() == null ? "" : sbn.getTag());
        out.put("group", n.getGroup() == null ? "" : n.getGroup());
        out.put("summary", (n.flags & Notification.FLAG_GROUP_SUMMARY) != 0);
        out.put("full_screen", n.fullScreenIntent != null);
        out.put("template", text(extras, Notification.EXTRA_TEMPLATE));
        String title = text(extras, Notification.EXTRA_TITLE);
        String body = text(extras, Notification.EXTRA_TEXT);
        out.put("title", title);
        out.put("text", body);
        out.put("sub", text(extras, Notification.EXTRA_SUB_TEXT));
        out.put("big", text(extras, Notification.EXTRA_BIG_TEXT));
        JSONArray lines = new JSONArray();
        CharSequence[] textLines = extras.getCharSequenceArray(Notification.EXTRA_TEXT_LINES);
        if (textLines != null)
            for (CharSequence line : textLines)
                lines.put(line == null ? "" : line.toString());
        out.put("lines", lines);
        out.put("conversation", text(extras, Notification.EXTRA_CONVERSATION_TITLE));
        out.put("group_conversation", extras.getBoolean(Notification.EXTRA_IS_GROUP_CONVERSATION, false));
        String shortcut = n.getShortcutId() == null ? "" : n.getShortcutId();
        boolean conversation = false;
        if (ranking != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            conversation = ranking.isConversation();
            ShortcutInfo info = ranking.getConversationShortcutInfo();
            if (shortcut.isEmpty() && info != null && info.getId() != null)
                shortcut = info.getId();
        }
        out.put("shortcut", shortcut);
        out.put("is_conversation", conversation);
        String channel = n.getChannelId() == null ? "" : n.getChannelId();
        out.put("channel", channel);
        NotificationChannel given = ranking == null ? null : ranking.getChannel();
        out.put("channel_name", given == null || given.getName() == null ? "" : given.getName().toString());
        // Its page in Android, for the tab: where a conversation Always through is marked important.
        if (!shortcut.trim().isEmpty())
            remember(this, sbn.getPackageName(), shortcut.trim(), channel, given, conversation, System.currentTimeMillis());
        boolean alerted = ranking != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q
            && ranking.getLastAudiblyAlertedMillis() > 0 && System.currentTimeMillis() - ranking.getLastAudiblyAlertedMillis() < ALERTED_MS;
        out.put("alerted", alerted);
        out.put("importance", ranking == null ? -1 : ranking.getImportance());
        out.put("intercepted", ranking != null && !ranking.matchesInterruptionFilter());
        out.put("redacted", redacted(title, body, app));
        out.put("sms_app", sbn.getPackageName().equals(smsApp()));
        boolean contacts = checkSelfPermission(Manifest.permission.READ_CONTACTS) == PackageManager.PERMISSION_GRANTED;
        out.put("messages", messages(extras, contacts));
        Object user = extras.getParcelable(Notification.EXTRA_MESSAGING_PERSON);
        if (user instanceof Person) {
            Person me = (Person) user;
            out.put("user", new JSONObject().put("name", me.getName() == null ? "" : me.getName().toString()).put("key", me.getKey() == null ? "" : me.getKey()));
        }
        out.put("people", people(extras, contacts));
        // For your computers, when its app sends to them (crates/sioul-core/src/phonemsgs.rs, `Extra`):
        // whether its app keeps it secret, when it was sent and posted, whether it shows a picture (never the picture).
        PhoneMessages.notice(out, n.visibility, n.when, sbn.getPostTime(), extras.containsKey(Notification.EXTRA_PICTURE) || extras.containsKey("android.pictureIcon"));
        return out;
    }

    /** A text of the extras, a CharSequence or a String; "" when none. */
    private static String text(Bundle extras, String key)
    {
        CharSequence value = extras.getCharSequence(key);
        return value == null ? "" : value.toString();
    }

    /**
     * A conversation's messages (MessagingStyle), each with its sender as the
     * app names them; the phone's address book asked about the last one's
     * sender only, the one Rust judges.
     */
    @SuppressWarnings("deprecation")
    private JSONArray messages(Bundle extras, boolean contacts) throws JSONException
    {
        JSONArray out = new JSONArray();
        Parcelable[] bundles = extras.getParcelableArray(Notification.EXTRA_MESSAGES);
        if (bundles == null)
            return out;
        List<Notification.MessagingStyle.Message> messages;
        try {
            messages = Notification.MessagingStyle.Message.getMessagesFromBundleArray(bundles);
        } catch (RuntimeException | LinkageError e) {
            return out;
        }
        int last = -1;
        for (int i = 0; i < messages.size(); i++)
            if (messages.get(i).getSenderPerson() != null)
                last = i;
        for (int i = 0; i < messages.size(); i++) {
            Notification.MessagingStyle.Message message = messages.get(i);
            JSONObject one = new JSONObject();
            one.put("text", message.getText() == null ? "" : message.getText().toString());
            // When it was sent, and whether it is a picture (its address stays here): for your computers.
            PhoneMessages.message(one, message.getTimestamp(), message.getDataMimeType());
            Person sender = message.getSenderPerson();
            if (sender != null)
                one.put("sender", person(sender.getName(), sender.getKey(), sender.getUri(), sender.isBot(), contacts && i == last));
            out.put(one);
        }
        return out;
    }

    /**
     * The people a notification names (EXTRA_PEOPLE_LIST), and the older
     * addresses (EXTRA_PEOPLE); the phone's address book asked about the
     * first only, the one Rust judges when no message names a sender.
     */
    @SuppressWarnings("deprecation")
    private JSONArray people(Bundle extras, boolean contacts) throws JSONException
    {
        JSONArray out = new JSONArray();
        ArrayList<Person> people = extras.getParcelableArrayList(Notification.EXTRA_PEOPLE_LIST);
        if (people != null)
            for (Person p : people)
                if (p != null)
                    out.put(person(p.getName(), p.getKey(), p.getUri(), p.isBot(), contacts && out.length() == 0));
        String[] older = extras.getStringArray(Notification.EXTRA_PEOPLE);
        if (older != null)
            for (String uri : older)
                if (uri != null && !uri.isEmpty())
                    out.put(person(null, null, uri, false, contacts && out.length() == 0));
        return out;
    }

    /**
     * A person, with what the phone's address book says of them when Sioul
     * may read it: a contact's lookup address resolved to its numbers and
     * addresses; a name alone, the numbers of the one contact of that name
     * (`by_name`: counted only for apps that name people as the address book
     * does, Rust decides).
     */
    private JSONObject person(CharSequence name, String key, String uri, boolean bot, boolean contacts) throws JSONException
    {
        JSONObject out = new JSONObject();
        String named = name == null ? "" : name.toString();
        out.put("name", named);
        out.put("key", key == null ? "" : key);
        out.put("uri", uri == null ? "" : uri);
        out.put("bot", bot);
        if (!contacts)
            return out;
        long id = -1;
        String by = "numbers";
        if (uri != null && uri.startsWith("content://com.android.contacts/")) {
            id = contactOfLookup(uri);
        } else if ((uri == null || uri.isEmpty()) && !named.trim().isEmpty()) {
            id = contactOfName(named.trim());
            by = "by_name";
        }
        if (id < 0)
            return out;
        out.put(by, data(id, ContactsContract.CommonDataKinds.Phone.CONTENT_URI, ContactsContract.CommonDataKinds.Phone.CONTACT_ID, ContactsContract.CommonDataKinds.Phone.NUMBER));
        out.put("by_name".equals(by) ? "emails_by_name" : "emails", data(id, ContactsContract.CommonDataKinds.Email.CONTENT_URI, ContactsContract.CommonDataKinds.Email.CONTACT_ID, ContactsContract.CommonDataKinds.Email.ADDRESS));
        return out;
    }

    /** The contact a lookup address names; -1 when none. */
    private long contactOfLookup(String uri)
    {
        try {
            Uri contact = ContactsContract.Contacts.lookupContact(getContentResolver(), Uri.parse(uri));
            if (contact == null)
                return -1;
            try (Cursor c = getContentResolver().query(contact, new String[] { ContactsContract.Contacts._ID }, null, null, null)) {
                return c != null && c.moveToFirst() ? c.getLong(0) : -1;
            }
        } catch (RuntimeException e) {
            return -1;
        }
    }

    /** The one contact with exactly this name; -1 when none, or when several share it. */
    private long contactOfName(String name)
    {
        try (Cursor c = getContentResolver().query(ContactsContract.Contacts.CONTENT_URI, new String[] { ContactsContract.Contacts._ID },
                                                    ContactsContract.Contacts.DISPLAY_NAME + " = ?", new String[] { name }, null)) {
            if (c == null || c.getCount() != 1 || !c.moveToFirst())
                return -1;
            return c.getLong(0);
        } catch (RuntimeException e) {
            return -1;
        }
    }

    /** A contact's numbers or addresses. */
    private JSONArray data(long id, Uri table, String contact, String column)
    {
        JSONArray out = new JSONArray();
        try (Cursor c = getContentResolver().query(table, new String[] { column }, contact + " = ?", new String[] { Long.toString(id) }, null)) {
            while (c != null && c.moveToNext()) {
                String value = c.getString(0);
                if (value != null && !value.trim().isEmpty())
                    out.put(value.trim());
            }
        } catch (RuntimeException e) {
            // Not readable: nothing known.
        }
        return out;
    }

    /** The app's name as the phone shows it, kept; else the one its notification carries; else its package. */
    @SuppressWarnings("deprecation")
    private String label(String pkg, Bundle extras)
    {
        String known = labels.get(pkg);
        if (known != null)
            return known;
        PackageManager packages = getPackageManager();
        String label = pkg;
        try {
            label = packages.getApplicationLabel(packages.getApplicationInfo(pkg, 0)).toString();
        } catch (PackageManager.NameNotFoundException e) {
            // Not visible to Sioul (Android 11's package visibility): the notification's own copy of it.
            Object info = extras.get("android.appInfo");
            if (info instanceof ApplicationInfo)
                label = ((ApplicationInfo) info).loadLabel(packages).toString();
        }
        labels.put(pkg, label);
        return label;
    }

    /**
     * Whether Android gave Sioul a redacted copy (Android 15, for a
     * notification its assistant judged sensitive: a code): its title is the
     * app's name and its text Android's own words for it.
     */
    private boolean redacted(String title, String text, String app)
    {
        if (redactedWords == null) {
            Resources system = Resources.getSystem();
            int id = system.getIdentifier(REDACTED, "string", "android");
            redactedWords = id == 0 ? "" : system.getString(id);
        }
        return !redactedWords.isEmpty() && redactedWords.contentEquals(text) && app.contentEquals(title);
    }

    /** The phone's default SMS app, asked again each minute at most. */
    private String smsApp()
    {
        long now = System.currentTimeMillis();
        if (now - smsAppAt > 60_000) {
            String found = null;
            try {
                found = Telephony.Sms.getDefaultSmsPackage(this);
            } catch (RuntimeException e) {
                // No telephony on this device.
            }
            smsApp = found == null ? "" : found;
            smsAppAt = now;
        }
        return smsApp;
    }
}
