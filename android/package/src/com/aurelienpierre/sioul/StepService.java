// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.ActivityManager;
import android.app.AlarmManager;
import android.app.Notification;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.ActivityNotFoundException;
import android.content.BroadcastReceiver;
import android.content.ComponentCallbacks;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.content.pm.ServiceInfo;
import android.content.res.Configuration;
import android.net.Uri;
import android.os.Build;
import android.os.FileObserver;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.IBinder;
import android.os.PowerManager;
import android.os.Process;
import android.os.SystemClock;
import android.provider.Settings;
import android.util.Log;

import org.json.JSONException;
import org.json.JSONObject;

import java.util.List;
import java.util.Locale;

/**
 * Sioul in the background: it keeps your devices in step while Sioul is
 * closed (crates/sioul-app/src/steps.rs; docs/android.md, "In the
 * background"). A foreground service in a process of its own (":steps"):
 * Qt's activity ends its whole process when it closes (System.exit in
 * QtActivityBase.onDestroy), which would end a service living there too. It
 * holds Sioul's library without Qt's window, and one quiet notification.
 *
 * - An alarm through Doze (exact while allowed, else inexact) asks Rust for
 *   a step: the other devices' news read (Sioul's own pull from the server
 *   while it works, else the sync app asked to look, then a quick exchange),
 *   do-not-disturb asked of Sioul's own process when it changed
 *   (DndReceiver), mail at its rhythm; Rust says when to look next.
 * - A watch on the sharing folder: another device's file written by the
 *   sync app brings a step three seconds later, without waiting for the alarm
 *   (Rust leaves it out when its pull brought that file already).
 * - The phone is kept awake only while a step runs: a step asked for later
 *   (a folder's, a message's) comes by an exact alarm of its own (`later`).
 * - While Sioul's window is on the screen, it exchanges by itself: the steps
 *   wait.
 * - While this phone screens calls (Calls.java), its notification offers "Let
 *   every call through", for an hour or until turned off, and "Screen calls
 *   again"; each step keeps the calls' table in step too.
 * - Its notification says fixed words ("Sioul keeps your devices in step"),
 *   never the time of day, in a group of its own, so that Android groups
 *   none of Sioul's notices under it (docs/notifications.md, "What stays a
 *   status").
 * - It hears the phone's unlock while it runs: the notices to know posted
 *   while nobody could see them count their minutes from then (Notices.seen).
 *
 * Rust's questions come through call(verb, json) (android/main.cpp), also
 * from Sioul's own process: starting and stopping the service, the stars of
 * the list's people (DndContacts), the battery's exemption, the alarm of
 * do-not-disturb's next end (DndReceiver).
 */
public final class StepService extends Service
{
    static final String TAG = DoseAlarms.TAG;
    static final String STEP = "com.aurelienpierre.sioul.action.STEP";
    /** "Let every call through" pressed on the notification (StepReceiver): extras "on", "minutes". */
    static final String CALLS = "com.aurelienpierre.sioul.action.CALLS";
    /** A press of the switch heard in Sioul's own process (DndReceiver, the tile): sent at once (StepReceiver). */
    static final String HEARD = "com.aurelienpierre.sioul.action.HEARD";
    /** The notification's words, as Rust says them after each apply in Sioul's own process (StepReceiver): extra "words". */
    static final String NOTE_WORDS = "com.aurelienpierre.sioul.action.NOTE_WORDS";
    /** A message's line written for your computers by the listener (AppNotes, `soon`): a step soon shares it. */
    static final String SOON = "com.aurelienpierre.sioul.action.SOON";
    /** The alarm of a step asked for later (`later`): a folder's, a message's. */
    static final String LATER = "com.aurelienpierre.sioul.action.LATER";
    /** How long a step asked by `SOON` waits: the messages that follow go with it. */
    private static final long SOON_MS = 20_000;
    private static final String CHANNEL = "steps";
    private static final int NOTE = 0x5137;
    /** Its notification's group: its own, so that it absorbs nothing. */
    static final String GROUP = "sioul.steps";
    private static final String KEPT = "sioul-steps";
    /** Seconds between two steps when Rust gave no answer. */
    private static final long USUAL_S = 300;
    /** A step holds the phone awake this long at most. */
    private static final long AWAKE_MS = 90_000;
    /** The sync app's writing settles this long before a step reads it. */
    private static final long SETTLE_MS = 3_000;

    private static volatile StepService running;

    private HandlerThread thread;
    private Handler worker;
    private PowerManager.WakeLock awake;
    private FileObserver watch;
    private String watched = "";
    private String own = "";
    /** Its text: while this phone screens calls, the calls screened or ringing until when (Rust's `steps::note`). */
    private String line = "";
    /** Its title: fixed words, Rust's in your language ("Sioul keeps your devices in step"); Sioul's own before Rust gave any. */
    private String title = "";
    /** The phone's unlock heard while the service runs (Notices.seen). */
    private BroadcastReceiver unlocks;
    private String channelName = "";
    /**
     * The calls screened, as Rust words them at each step: {screening, through,
     * line, hour, off, again} (crates/sioul-app/src/calls.rs, `step`);
     * empty while this phone does not screen calls.
     */
    private String calls = "";
    /** A step asked while one runs: the phone stays awake for it. */
    private volatile boolean posted;
    /** The step asked for later, waiting for its alarm (`later`); none: null. */
    private Later waiting;
    /** The names of the other devices' files the sync app wrote since the last step, for Rust ("folder:"). */
    private final java.util.Set<String> written = new java.util.TreeSet<>();
    /** When the words shown were made (Unix milliseconds, Rust's clock): older ones are left aside. */
    private long saidAt;

    /** One step, on the worker thread: {next (seconds), folder, own, line, title, channel, stop}. */
    static native String nativeStep(String reason);

    /** Tells Rust it runs in the service's process: it never touches Android's modes from here. */
    static native void nativeService();

    // ---------------------------------------------------------------- Rust's questions

    /** Rust's question (android/main.cpp), from either process. Any thread. */
    static String call(Context context, String verb, String json)
    {
        try {
            JSONObject asked = json == null || json.isEmpty() ? new JSONObject() : new JSONObject(json);
            switch (verb) {
            case "start":
                return start(context, asked) ? "true" : "false";
            case "stop":
                stop(context);
                return "true";
            case "running":
                return runningHere(context) ? "true" : "false";
            case "battery":
                return exempt(context) ? "true" : "false";
            case "ask-battery":
                askBattery(context);
                return "true";
            case "poke":
                DndReceiver.poke(context);
                return "true";
            case "heard":
                // A press made in Sioul's own process: sent at once by the service when it runs; else Rust sends it.
                if (!runningHere(context))
                    return "{\"stepped\":false}";
                context.sendBroadcast(new Intent(context, StepReceiver.class).setAction(HEARD));
                return "{\"stepped\":true}";
            case "next":
                DndReceiver.schedule(context, asked.optLong("at", 0));
                return "true";
            case "note":
                // The notification's words now, from Sioul's own process: to the service, in its own.
                return note(context, json) ? "true" : "false";
            case "stars":
                return DndContacts.stars(context, asked).toString();
            case "open-contact":
                return DndContacts.open(context, asked.optString("uri", "")) ? "true" : "false";
            case "add-contact":
                return DndContacts.add(context, asked) ? "true" : "false";
            case "contacts-allowed":
                return DndContacts.allowed(context) ? "true" : "false";
            case "device":
                // What this device is (devicekind.rs): its smallest width and whether it has telephony.
                return device(context);
            case "keyboard":
                // A hardware keyboard attached now; Rust told of each change from here on (devicekind.rs).
                watchKeyboard(context);
                return keyboard(context.getResources().getConfiguration()) ? "true" : "false";
            case "sms-app":
                // The phone's SMS app, for Settings ▸ This phone ▸ On your computers.
                return JSONObject.quote(PhoneMessages.smsApp(context));
            case "mail-withdraw":
                // New mail's notification, each of its messages dealt with on some device (Rust's notices).
                MailNotes.withdraw(context);
                return "true";
            case "notice":
                // A notice of Rust's own (a code: eventalarms.rs, `code_note`), posted in whichever process asks.
                Notices.show(context, json);
                return "true";
            case "notice-withdraw":
                // Every notice about a thing settled on some device (a code's message read).
                Notices.withdraw(context, asked.optString("about", ""));
                return "true";
            case "calendar-app":
                // Whether a calendar app rings an event's own alarms here in Sioul's place (CalendarApps).
                return CalendarApps.find(context);
            case "notices-seen":
                // Sioul on the screen: the notices to know posted meanwhile count their minutes from now.
                Notices.seen(context);
                return "true";
            default:
                // texts: read and sent for your computers (Texts.java, TextSend.java).
                if (verb.startsWith("texts-"))
                    return Texts.call(context, verb, asked);
                // A mail attachment opened in the app you choose, or saved where you choose (Attachments.java).
                if (verb.startsWith("attachment-"))
                    return Attachments.call(context, verb, asked);
                // The training by itself: the phone's power and network, its wake lock (Learn.java).
                if (verb.startsWith("learn-"))
                    return Learn.call(context, verb, asked);
                // The calls screened (Calls.java): the role, its pages, the phone app's dial pad.
                return verb.startsWith("calls-") ? Calls.call(context, verb, asked) : null;
            }
        } catch (JSONException | RuntimeException e) {
            Log.e(TAG, "Steps: " + verb + " failed: " + e);
            return null;
        }
    }

    /**
     * What this device is, for Rust's devicekind.rs, as
     * {"smallest_dp": n, "telephony": bool}: its smallest screen width in
     * density-independent pixels, as Android's resources choose a tablet's
     * layout from (sw600dp and up), and whether it has telephony at all (a
     * tablet without a SIM has none: no calls to screen, no texts to read).
     */
    static String device(Context context) throws JSONException
    {
        JSONObject said = new JSONObject();
        said.put("smallest_dp", context.getResources().getConfiguration().smallestScreenWidthDp);
        said.put("telephony", context.getPackageManager().hasSystemFeature(PackageManager.FEATURE_TELEPHONY));
        return said.toString();
    }

    /** Whether the configuration's change of keyboards is followed already, in this process. */
    private static boolean keyboardWatched;

    /** Rust's side: a hardware keyboard attached or taken away (android/main.cpp, devicekind.rs). */
    static native void nativeKeyboard(boolean attached);

    /**
     * Whether a hardware keyboard is attached and usable, as the
     * configuration says it: a keyboard with keys, not hidden (a keyboard
     * cover folded away, a Bluetooth keyboard gone).
     */
    static boolean keyboard(Configuration configuration)
    {
        return configuration.keyboard != Configuration.KEYBOARD_NOKEYS && configuration.hardKeyboardHidden == Configuration.HARDKEYBOARDHIDDEN_NO;
    }

    /**
     * Each configuration change (the window keeps running through them: the
     * manifest's configChanges) told to Rust with whether a hardware keyboard
     * is there then, so that the status line shows the keys while one is
     * (KeysHint.qml). Once per process, from the window's start.
     */
    static synchronized void watchKeyboard(Context context)
    {
        if (keyboardWatched)
            return;
        keyboardWatched = true;
        context.getApplicationContext().registerComponentCallbacks(new ComponentCallbacks() {
            @Override
            public void onConfigurationChanged(Configuration changed)
            {
                try {
                    nativeKeyboard(keyboard(changed));
                } catch (UnsatisfiedLinkError e) {
                    // Sioul's library not in this process: nobody to tell.
                }
            }

            @Override
            public void onLowMemory()
            {
            }
        });
    }

    /**
     * Whether it runs and its words, written by Sioul's own process and read
     * by the service's: read again from the file when it changed (both
     * processes use it, rarely).
     */
    @SuppressWarnings("deprecation")
    private static SharedPreferences kept(Context context)
    {
        return context.getSharedPreferences(KEPT, Context.MODE_PRIVATE | Context.MODE_MULTI_PROCESS);
    }

    /**
     * The service started, or told new words: {title, channel}. Kept, so that
     * a restart of the phone starts it again with them. False when Android
     * refused (Sioul in the background on Android 12 and later).
     */
    static boolean start(Context context, JSONObject words)
    {
        kept(context).edit().putBoolean("on", true)
            .putString("title", words.optString("title", ""))
            .putString("channel", words.optString("channel", "")).commit();
        // Running already: nothing more (its next step brings the words).
        if (runningHere(context))
            return true;
        Intent intent = new Intent(context, StepService.class).setAction(STEP).putExtra("reason", "start")
            .putExtra("title", words.optString("title", "")).putExtra("channel", words.optString("channel", ""));
        try {
            context.startForegroundService(intent);
            return true;
        } catch (IllegalStateException | SecurityException e) {
            // Not allowed from the background now: the next start of Sioul's window, or the next restart.
            Log.w(TAG, "Steps: not started now: " + e);
            return false;
        }
    }

    /** Stopped, and not started again at the phone's restart. */
    static void stop(Context context)
    {
        kept(context).edit().putBoolean("on", false).commit();
        context.stopService(new Intent(context, StepService.class));
        cancel(context);
    }

    /** At the phone's restart or after an update (StepReceiver): started again if it was on. */
    static void restart(Context context, String reason)
    {
        if (!kept(context).getBoolean("on", false))
            return;
        try {
            context.startForegroundService(new Intent(context, StepService.class).setAction(STEP).putExtra("reason", reason));
        } catch (IllegalStateException | SecurityException e) {
            Log.w(TAG, "Steps: not started again: " + e);
        }
    }

    /** An alarm's step (StepReceiver): to the service running, else it starts (an exact alarm allows it). */
    static void alarm(Context context)
    {
        StepService service = running;
        if (service != null) {
            service.step("alarm", 0);
            return;
        }
        restart(context, "alarm");
    }

    /** Whether the service runs in this very process (its own, ":steps"). */
    static boolean inThisProcess()
    {
        return running != null;
    }

    /** Whether the service runs, asked from either process. */
    static boolean runningHere(Context context)
    {
        if (running != null)
            return true;
        ActivityManager manager = context.getSystemService(ActivityManager.class);
        try {
            @SuppressWarnings("deprecation")
            List<ActivityManager.RunningServiceInfo> services = manager.getRunningServices(64);
            for (ActivityManager.RunningServiceInfo info : services)
                if (StepService.class.getName().equals(info.service.getClassName()) && info.foreground)
                    return true;
        } catch (SecurityException e) {
            // Not ours to ask: not known.
        }
        return false;
    }

    /** Whether Android lets Sioul run in the background, battery optimisation aside. */
    static boolean exempt(Context context)
    {
        PowerManager power = context.getSystemService(PowerManager.class);
        return power != null && power.isIgnoringBatteryOptimizations(context.getPackageName());
    }

    /** Android's question to let Sioul run in the background; else its list. */
    static void askBattery(Context context)
    {
        Intent[] pages = {
            new Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:" + context.getPackageName())),
            new Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS),
            new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:" + context.getPackageName())),
        };
        for (Intent page : pages) {
            try {
                context.startActivity(page.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
                return;
            } catch (ActivityNotFoundException | SecurityException e) {
                // The next one.
            }
        }
    }

    // ---------------------------------------------------------------- the service

    @Override
    public void onCreate()
    {
        super.onCreate();
        running = this;
        thread = new HandlerThread("sioul-steps", Process.THREAD_PRIORITY_BACKGROUND);
        thread.start();
        worker = new Handler(thread.getLooper());
        PowerManager power = getSystemService(PowerManager.class);
        awake = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "sioul:steps");
        awake.setReferenceCounted(false);
        SharedPreferences words = kept(this);
        title = words.getString("title", "");
        channelName = words.getString("channel", "");
        foreground();
        // The phone's texts watched in this process: the import asks Android only when they changed.
        Texts.watch(this);
        // The phone unlocked: the notices to know waiting to be seen count their minutes from now.
        unlocks = new BroadcastReceiver() {
            @Override
            public void onReceive(Context context, Intent intent)
            {
                if (Intent.ACTION_USER_PRESENT.equals(intent.getAction()))
                    Notices.seen(context.getApplicationContext());
            }
        };
        IntentFilter unlocked = new IntentFilter(Intent.ACTION_USER_PRESENT);
        // Exported: SystemUI sends it, not Android's own process; a protected broadcast, which no app can send.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU)
            registerReceiver(unlocks, unlocked, Context.RECEIVER_EXPORTED);
        else
            registerReceiver(unlocks, unlocked);
        // The notices' channels made, an older Sioul's deleted, from the phone's start too.
        Notices.channels(this, null);
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId)
    {
        if (intent != null) {
            String given = intent.getStringExtra("title");
            if (given != null && !given.isEmpty())
                title = given;
            String channel = intent.getStringExtra("channel");
            if (channel != null && !channel.isEmpty())
                channelName = channel;
        }
        // Within five seconds of a start, every time: Android asks it of a foreground service.
        foreground();
        step(intent == null ? "restart" : intent.getStringExtra("reason") == null ? "start" : intent.getStringExtra("reason"), 0);
        return START_STICKY;
    }

    @Override
    public void onDestroy()
    {
        running = null;
        // Its card goes with the service: its group's summary with it.
        Notices.tidy(this, java.util.Collections.emptyList(), java.util.Collections.singleton(Notices.key(NOTE, null)));
        if (unlocks != null)
            unregisterReceiver(unlocks);
        if (watch != null)
            watch.stopWatching();
        Texts.unwatch(this);
        cancelLater(this);
        if (awake.isHeld())
            awake.release();
        thread.quitSafely();
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent)
    {
        return null;
    }

    /** The service in the foreground with its notification: at each start, as Android asks. */
    private void foreground()
    {
        Notification built = note();
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
                startForeground(NOTE, built, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);
            else
                startForeground(NOTE, built);
            // Its group's own summary on Android 16: never bundled with another.
            Notices.tidy(this, java.util.Collections.singletonList(new Notices.Child(NOTE, null, GROUP, 0)), java.util.Collections.emptySet());
        } catch (RuntimeException e) {
            // Refused (Android 12 and later, from the background): the next start.
            Log.w(TAG, "Steps: not in the foreground: " + e);
            stopSelf();
        }
    }

    /** Its notification changed in place: new words, the do-not-disturb line. */
    private void refresh()
    {
        getSystemService(NotificationManager.class).notify(NOTE, note());
        Notices.tidy(this, java.util.Collections.singletonList(new Notices.Child(NOTE, null, GROUP, 0)), java.util.Collections.emptySet());
    }

    /**
     * One quiet notification: the lowest importance, no sound, no badge,
     * hidden on the lock screen; fixed words, in a group of its own; the
     * calls' line and buttons only while this phone screens calls.
     */
    private Notification note()
    {
        NotificationManager notifications = getSystemService(NotificationManager.class);
        notifications.createNotificationChannel(Channels.quiet(CHANNEL, channelName.isEmpty() ? word(0) : channelName, NotificationManager.IMPORTANCE_MIN));
        Notification.Builder note = new Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(getColor(R.color.sioul_green))
            .setContentTitle(title.isEmpty() ? word(1) : title)
            .setCategory(Notification.CATEGORY_SERVICE)
            .setVisibility(Notification.VISIBILITY_SECRET)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setShowWhen(false)
            .setGroup(GROUP)
            .setContentIntent(open(this));
        // While this phone screens calls (Rust says it then only): "Calls are screened.", or "Every call rings until 15:30.".
        JSONObject words = callsWords();
        if (!line.isEmpty()) {
            note.setContentText(line);
            note.setStyle(new Notification.BigTextStyle().bigText(line));
        }
        // "Let every call through" while this phone screens calls: for an hour or until
        // turned off; once on, "Screen calls again" (docs/android.md, "Calls").
        if (words != null && words.optBoolean("screening")) {
            if (words.optBoolean("through")) {
                note.addAction(action(this, 0x5142, words.optString("again", ""), false, 0));
            } else {
                note.addAction(action(this, 0x5140, words.optString("hour", ""), true, 60));
                note.addAction(action(this, 0x5141, words.optString("off", ""), true, 0));
            }
        }
        return note.build();
    }

    private JSONObject callsWords()
    {
        if (calls.isEmpty())
            return null;
        try {
            return new JSONObject(calls);
        } catch (JSONException e) {
            return null;
        }
    }

    /** One of the notification's buttons for the calls: on for `minutes` (0: until turned off), or off. */
    private static Notification.Action action(Context context, int code, String title, boolean on, int minutes)
    {
        Intent press = new Intent(context, StepReceiver.class).setAction(CALLS).putExtra("on", on).putExtra("minutes", minutes);
        PendingIntent pending = PendingIntent.getBroadcast(context, code, press, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        return new Notification.Action.Builder(null, title, pending).build();
    }

    /**
     * The notification's words, {title, text, calls}, told by Rust after each
     * apply in Sioul's own process: to the service running in this process,
     * else to its process (StepReceiver), only while it runs. True when told.
     */
    static boolean note(Context context, String json)
    {
        StepService service = running;
        if (service != null) {
            service.words(json);
            return true;
        }
        if (!runningHere(context))
            return false;
        // Android's queue of foreground broadcasts: never behind one held open meanwhile
        // (DndReceiver holds its own while Sioul's change settles, three seconds).
        context.sendBroadcast(new Intent(context, StepReceiver.class).setAction(NOTE_WORDS).putExtra("words", json)
                                  .addFlags(Intent.FLAG_RECEIVER_FOREGROUND));
        return true;
    }

    /** The words told (StepReceiver, in this process): the notification changed in place at once when they differ. */
    static void told(String json)
    {
        StepService service = running;
        if (service != null && json != null)
            service.words(json);
    }

    /**
     * New words, kept and shown at once when they changed: on the caller's
     * thread, never behind a step, which may wait for the sync app or for
     * mail on the worker's.
     */
    private void words(String json)
    {
        try {
            said(new JSONObject(json));
        } catch (JSONException e) {
            Log.w(TAG, "Steps: words not read: " + e);
        }
    }

    /**
     * The notification's words from Rust ({at, title, text, channel, calls}):
     * shown again when one changed; words made before those shown (`at`,
     * Unix milliseconds: a step's, finished after a press told newer ones)
     * are left aside.
     */
    private synchronized void said(JSONObject answer)
    {
        long at = answer.optLong("at", 0);
        if (at > 0 && at < saidAt)
            return;
        saidAt = Math.max(saidAt, at);
        String said = answer.optString("text", answer.optString("line", line));
        String words = answer.optString("title", title);
        String channel = answer.optString("channel", channelName);
        JSONObject callsSaid = answer.optJSONObject("calls");
        String callsNow = callsSaid == null ? calls : callsSaid.toString();
        if (!said.equals(line) || !words.equals(title) || !channel.equals(channelName) || !callsNow.equals(calls)) {
            line = said;
            title = words;
            channelName = channel;
            calls = callsNow;
            refresh();
            // When, for the time each change takes to show (adb logcat -s sioul); nothing of what it says.
            Log.i(TAG, "Steps: the notification says the state now.");
        }
    }

    /**
     * A line written for your computers, from the listener's process: a step
     * soon, while the service runs (else the window's own exchanges, or the
     * next start, send it).
     */
    static void soon(Context context)
    {
        if (runningHere(context))
            context.sendBroadcast(new Intent(context, StepReceiver.class).setAction(SOON));
    }

    /**
     * `SOON` heard (StepReceiver, in this process): a step `SOON_MS` on, or
     * the one waiting for its alarm then, which shares the line too
     * (`Later.of`); none when a step is about to run.
     */
    static void soonHere()
    {
        StepService service = running;
        if (service != null && !service.posted)
            service.step("messages", SOON_MS);
    }

    /** The alarm of the step asked for later (StepReceiver, in this process): it runs now; the service started again if Android had stopped it. */
    static void laterHere(Context context)
    {
        StepService service = running;
        if (service == null) {
            restart(context, "alarm");
            return;
        }
        String reason;
        synchronized (service) {
            reason = service.waiting == null ? "folder" : service.waiting.reason;
            service.waiting = null;
        }
        service.step(reason, 0);
    }

    /** A press of the switch made in Sioul's own process (StepReceiver): a step at once, which sends it. */
    static void heard(Context context)
    {
        StepService service = running;
        if (service != null)
            service.step("heard", 0);
    }

    /**
     * Expecting a call (once "Let every call through") pressed on the
     * notification (StepReceiver, in this process): kept for the screening at
     * once ({@link Calls#press}), then a
     * step, in which Rust shares it with your other devices and words the
     * notification again.
     */
    static void callsPressed(Context context, boolean on, int minutes)
    {
        Calls.press(context, on, minutes);
        StepService service = running;
        if (service != null)
            service.step("calls", 0);
    }

    /** A tap: Sioul, as by its icon. */
    private static PendingIntent open(Context context)
    {
        Intent sioul = context.getPackageManager().getLaunchIntentForPackage(context.getPackageName());
        if (sioul == null)
            return null;
        return PendingIntent.getActivity(context, NOTE, sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED),
                                         PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /**
     * A step: now, on the worker thread, the phone kept awake while it waits
     * there and runs (one asked meanwhile replaces one not started; the one
     * waiting for its alarm goes, this one does its work); or `delay`
     * milliseconds on, by an exact alarm of its own, the phone left to sleep
     * meanwhile (`later`).
     */
    void step(String reason, long delay)
    {
        if (delay > 0) {
            later(reason, delay);
            return;
        }
        synchronized (this) {
            waiting = null;
        }
        cancelLater(this);
        awake.acquire(AWAKE_MS);
        posted = true;
        worker.removeCallbacksAndMessages(null);
        worker.post(() -> run(reason));
    }

    /**
     * A step asked for later, as `Later.of` joins it to the one waiting:
     * its exact alarm set, or moved (one alarm at a time). Through Doze too
     * (allow-while-idle); Android brings an alarm five seconds on at the
     * soonest.
     */
    private void later(String reason, long delay)
    {
        long now = SystemClock.elapsedRealtime();
        Later next;
        synchronized (this) {
            next = Later.of(waiting, reason, delay, now);
            waiting = next;
        }
        AlarmManager alarms = getSystemService(AlarmManager.class);
        PendingIntent pending = laterPending(this);
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S || alarms.canScheduleExactAlarms())
            alarms.setExactAndAllowWhileIdle(AlarmManager.ELAPSED_REALTIME_WAKEUP, next.at, pending);
        else
            alarms.setAndAllowWhileIdle(AlarmManager.ELAPSED_REALTIME_WAKEUP, next.at, pending);
    }

    /**
     * A step asked for later: its reason, and when it comes (elapsed realtime,
     * milliseconds). Pure: android/jvm-checks/StepsCheck.java.
     */
    static final class Later
    {
        final String reason;
        final long at;

        Later(String reason, long at)
        {
            this.reason = reason;
            this.at = at;
        }

        /**
         * `asked`, `delay` ms on at `now`, joined to the one `waiting` (null:
         * none). A folder's step comes `delay` after the last file written
         * (each moves it on), never in place of a message's, which then comes
         * at that time instead: a folder's may find nothing new and be left
         * out, a message's line must go. A message's joins the one waiting,
         * at its time (the lines that follow go with it), shares the line
         * then; none waiting, it comes `delay` on.
         */
        static Later of(Later waiting, String asked, long delay, long now)
        {
            if (waiting == null)
                return new Later(asked, now + delay);
            if ("folder".equals(asked))
                return new Later(waiting.reason, now + delay);
            if ("messages".equals(asked))
                return new Later(asked, waiting.at);
            return new Later(asked, now + delay);
        }
    }

    private void run(String reason)
    {
        posted = false;
        String names = takeWritten();
        try {
            // A folder's step whose files a step since has read (their names taken): nothing left to do.
            if ("folder".equals(reason) && names.isEmpty())
                return;
            // Sioul's window on the screen exchanges by itself: the step waits for the next alarm.
            if (windowShown()) {
                schedule(this, USUAL_S);
                return;
            }
            DoseAlarms.load(getApplicationContext());
            nativeService();
            Texts.watch(getApplicationContext());
            // A folder's step names the files written, for Rust to leave out one its pull brought already.
            JSONObject answer = new JSONObject(nativeStep("folder".equals(reason) ? "folder:" + names : reason));
            // Nothing new: the alarm stays as it was.
            if (answer.optBoolean("skip"))
                return;
            if (answer.optBoolean("stop")) {
                // Sharing off, or the setting: no more steps; whether it starts again
                // at the next restart stays as Sioul's window last said (`stop`, `start`).
                cancel(this);
                stopSelf();
                return;
            }
            schedule(this, answer.optLong("next", USUAL_S));
            own = answer.optString("own", own);
            watchFolder(answer.optString("folder", ""));
            if (answer.optJSONObject("calls") == null)
                answer.put("calls", new JSONObject());
            said(answer);
        } catch (Throwable e) {
            // Sioul's library that does not load, a step that failed: tried again at the usual pace.
            Log.e(TAG, "Steps: a step failed (" + reason + ")", e);
            schedule(this, USUAL_S);
        } finally {
            if (!posted && awake.isHeld())
                awake.release();
        }
    }

    /** Whether Sioul's window is on the screen (its own process in the foreground). */
    private boolean windowShown()
    {
        ActivityManager manager = getSystemService(ActivityManager.class);
        List<ActivityManager.RunningAppProcessInfo> processes = manager.getRunningAppProcesses();
        if (processes == null)
            return false;
        for (ActivityManager.RunningAppProcessInfo process : processes)
            if (getPackageName().equals(process.processName) && process.importance <= ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND)
                return true;
        return false;
    }

    /** The sharing folder watched: another device's file written there brings a step. */
    private void watchFolder(String folder)
    {
        if (folder.equals(watched))
            return;
        if (watch != null)
            watch.stopWatching();
        watch = null;
        watched = folder;
        if (folder.isEmpty())
            return;
        @SuppressWarnings("deprecation")
        FileObserver observer = new FileObserver(folder, FileObserver.CLOSE_WRITE | FileObserver.MOVED_TO) {
            @Override
            public void onEvent(int event, String name)
            {
                if (!othersFile(name, own))
                    return;
                synchronized (written) {
                    written.add(name);
                }
                step("folder", SETTLE_MS);
            }
        };
        watch = observer;
        watch.startWatching();
    }

    /**
     * Another device's file, by its name in the sharing folder's top: hidden
     * names are files being written; this device's own are its own steps.
     * Pure: android/jvm-checks/StepsCheck.java.
     */
    static boolean othersFile(String name, String own)
    {
        return name != null && !name.isEmpty() && !name.startsWith(".") && name.indexOf('/') < 0 && (own.isEmpty() || !name.startsWith(own));
    }

    /** The names written since the last step, "/" between them, forgotten here (any step reads the folder). */
    private String takeWritten()
    {
        synchronized (written) {
            String names = String.join("/", written);
            written.clear();
            return names;
        }
    }

    /** The next step's alarm, `seconds` on: exact through Doze while allowed, else inexact. */
    static void schedule(Context context, long seconds)
    {
        AlarmManager alarms = context.getSystemService(AlarmManager.class);
        long at = SystemClock.elapsedRealtime() + Math.max(60, seconds) * 1000L;
        PendingIntent step = pending(context);
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S || alarms.canScheduleExactAlarms())
            alarms.setExactAndAllowWhileIdle(AlarmManager.ELAPSED_REALTIME_WAKEUP, at, step);
        else
            alarms.setAndAllowWhileIdle(AlarmManager.ELAPSED_REALTIME_WAKEUP, at, step);
    }

    private static void cancel(Context context)
    {
        context.getSystemService(AlarmManager.class).cancel(pending(context));
        cancelLater(context);
    }

    /** The alarm of a step asked for later, taken away: a step now does its work. */
    private static void cancelLater(Context context)
    {
        context.getSystemService(AlarmManager.class).cancel(laterPending(context));
    }

    private static PendingIntent laterPending(Context context)
    {
        return PendingIntent.getBroadcast(context, NOTE + 1, new Intent(context, StepReceiver.class).setAction(LATER),
                                          PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    private static PendingIntent pending(Context context)
    {
        return PendingIntent.getBroadcast(context, NOTE, new Intent(context, StepReceiver.class).setAction(STEP),
                                          PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    // ---------------------------------------------------------------- words before Rust gave them

    private static final String[][] WORDS = {
        { "Devices in step", "Appareils à jour" },
        { "Sioul keeps your devices in step", "Sioul tient vos appareils à jour" },
    };

    private static String word(int which)
    {
        boolean french = "fr".equals(Locale.getDefault().getLanguage());
        return WORDS[which][french ? 1 : 0];
    }
}
