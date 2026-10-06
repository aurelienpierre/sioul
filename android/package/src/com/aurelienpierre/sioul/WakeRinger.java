// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Notification;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.graphics.drawable.Icon;
import android.media.AudioAttributes;
import android.media.AudioFocusRequest;
import android.media.AudioManager;
import android.media.MediaPlayer;
import android.media.Ringtone;
import android.media.RingtoneManager;
import android.media.ToneGenerator;
import android.net.Uri;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.PowerManager;
import android.os.SystemClock;
import android.provider.Settings;
import android.text.format.DateFormat;
import android.util.Log;

import java.io.IOException;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;

/**
 * The alarm at waking, ringing (WakeAlarms): a service in the foreground for
 * as long as it rings, its notification on the "Waking" channel with the
 * screen over the lock screen (WakeScreen), Stop and 10 min later. Sioul plays
 * the sound itself, the phone's alarm sound as an alarm (it comes through
 * Android's do-not-disturb, which lets alarms through unless set to total
 * silence), rising from very low to full over half a minute: a channel's
 * sound would be at one volume, and an insistent one stops as the shade
 * opens. It rings until Stop or 10 min later; never silent: another sound
 * when the alarm's does not play, beeps when none does, and the ring without
 * this service when Android refuses it. From Android 14 the service is of
 * the type alarms keep ringing with ("systemExempted", allowed to apps that
 * set exact alarms). A try ("Try the alarm") rings here the same way; its
 * Stop and 10 min later touch only the try: 10 min later rings it once more,
 * then it offers Stop alone. A waking that comes while a try rings takes it
 * over; a try that comes while a waking rings changes nothing.
 */
public final class WakeRinger extends Service
{
    /** The ring's notification, by its number alone: the doses' are their keys and 1, the time running's 2. */
    static final int NOTE = 3;
    /** After "10 min later": when it rings again, with Stop; a try's, its own. */
    static final int AGAIN_NOTE = 4;
    static final int TRY_AGAIN_NOTE = 5;
    /** The sound rises over half a minute, from 40 dB under full, evenly to the ear. */
    private static final long RISE_MS = 30_000;
    private static final double QUIET_DB = -40;

    /** The ring going on in this process: Stop and 10 min later reach it here. */
    private static volatile WakeRinger running;
    /** Rung without this service, Android having refused it: its sound. */
    private static Ringtone bare;
    /** Held from the alarm to the sound playing: the phone does not fall asleep between. */
    private static PowerManager.WakeLock held;
    /** What rings now: a try; a try's last ring (no "10 min later"). */
    private static volatile boolean trying;
    private static volatile boolean last;

    private final Handler handler = new Handler(Looper.getMainLooper());
    private MediaPlayer player;
    private ToneGenerator beeps;
    private AudioFocusRequest focus;
    private long started;

    // ---------------------------------------------------------------- from the receiver and the screen

    /**
     * The alarm rings (WakeReceiver): this service, in the foreground; else
     * the ring from here. `trial`: a try's; `lastTry`: a try's last ring.
     */
    static void ring(Context context, long at, boolean trial, boolean lastTry)
    {
        hold(context);
        try {
            context.startForegroundService(new Intent(context, WakeRinger.class).setAction(trial ? WakeAlarms.TRY : WakeAlarms.RING)
                                               .putExtra(WakeAlarms.AT, at).putExtra(WakeAlarms.TRIAL, trial).putExtra(WakeAlarms.LAST, lastTry));
        } catch (RuntimeException e) {
            Log.e(DoseAlarms.TAG, "Waking: the ringer could not start; it rings from here.", e);
            ringBare(context, at, trial, lastTry);
        }
    }

    /**
     * Stop, or 10 min later (its buttons, its notification swiped away, the
     * screen's buttons). What rings says whether it is a try; when nothing
     * rings here (a "rings again" line's Stop, Sioul started again since),
     * the button's own word, `trial`.
     */
    static void stop(Context context, boolean later, boolean trial)
    {
        WakeRinger ringer = running;
        if (ringer != null)
            ringer.end(later);
        else
            ended(context, later, bare != null ? trying : trial, bare != null && last);
    }

    /** Whether what rings is a try. */
    static boolean trying()
    {
        return trying;
    }

    /** Whether "10 min later" is offered: always but on a try's last ring. */
    static boolean laterOffered()
    {
        return !(trying && last);
    }

    /** What rings from now: a waking takes a try over; a try never takes a waking over. */
    private static synchronized void begin(boolean trial, boolean lastTry, boolean ringingAlready)
    {
        if (!trial) {
            trying = false;
            last = false;
        } else if (!ringingAlready || trying) {
            trying = true;
            last = lastTry;
        }
    }

    /** Whether it rings now (the screen opened late shows nothing). */
    static boolean ringing()
    {
        return running != null || bare != null;
    }

    // ---------------------------------------------------------------- the service

    @Override
    public int onStartCommand(Intent intent, int flags, int id)
    {
        long at = intent == null ? System.currentTimeMillis() : intent.getLongExtra(WakeAlarms.AT, System.currentTimeMillis());
        begin(intent != null && intent.getBooleanExtra(WakeAlarms.TRIAL, false), intent != null && intent.getBooleanExtra(WakeAlarms.LAST, false), running == this);
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
                startForeground(NOTE, note(this, at), ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED);
            else
                startForeground(NOTE, note(this, at));
        } catch (RuntimeException e) {
            Log.e(DoseAlarms.TAG, "Waking: Android refused the ringer in the foreground; it rings without it.", e);
            ringBare(this, at, trying, last);
            stopSelf();
            return START_NOT_STICKY;
        }
        // Its own "rings again" line, said: it rings now.
        getSystemService(NotificationManager.class).cancel(trying ? TRY_AGAIN_NOTE : AGAIN_NOTE);
        // The screen already shown (a ring over another): its buttons as they are now.
        WakeScreen.refresh();
        // Rung again while ringing ("10 min later" and the next waking at once): it goes on.
        if (running != this) {
            running = this;
            play();
        }
        // The player keeps the phone awake while it plays; beeps need the hold kept until the end.
        if (player != null)
            release();
        return START_NOT_STICKY;
    }

    @Override
    public void onDestroy()
    {
        silence();
        if (running == this)
            running = null;
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent)
    {
        return null;
    }

    /** Stop, or 10 min later: silent, the notification and the screen gone. */
    private void end(boolean later)
    {
        silence();
        running = null;
        stopForeground(STOP_FOREGROUND_REMOVE);
        stopSelf();
        ended(this, later, trying, last);
    }

    /**
     * After the ring, whichever way it rang: 10 min later set, or nothing
     * more until the next waking. A try's touch only the try: 10 min later
     * rings it once more, as a try, its last time; after that, or at Stop,
     * it is over. The next waking and its own "10 min later" stay as they are.
     */
    private static synchronized void ended(Context context, boolean later, boolean trial, boolean lastTry)
    {
        if (bare != null) {
            bare.stop();
            bare = null;
        }
        release();
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        notifications.cancel(NOTE);
        if (trial) {
            if (later && !lastTry) {
                again(context, WakeAlarms.tryLater(context), true);
            } else {
                WakeAlarms.tryStopped(context);
                notifications.cancel(TRY_AGAIN_NOTE);
            }
        } else if (later) {
            again(context, WakeAlarms.later(context), false);
        } else {
            WakeAlarms.stopped(context);
            notifications.cancel(AGAIN_NOTE);
        }
        trying = false;
        last = false;
        WakeScreen.close();
    }

    // ---------------------------------------------------------------- the sound

    private static AudioAttributes alarm()
    {
        return new AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM).setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build();
    }

    /**
     * The sounds tried in turn: the phone's alarm sound (the settings' own
     * address, which Android keeps readable before the first unlock), then
     * the same by its file, the notifications' sound, the ringtone.
     */
    private static List<Uri> sounds(Context context)
    {
        List<Uri> sounds = new ArrayList<>();
        sounds.add(Settings.System.DEFAULT_ALARM_ALERT_URI);
        try {
            Uri chosen = RingtoneManager.getActualDefaultRingtoneUri(context, RingtoneManager.TYPE_ALARM);
            if (chosen != null)
                sounds.add(chosen);
        } catch (RuntimeException e) {
            // Not readable now (before the first unlock): the others.
        }
        sounds.add(Settings.System.DEFAULT_NOTIFICATION_URI);
        sounds.add(Settings.System.DEFAULT_RINGTONE_URI);
        return sounds;
    }

    private void play()
    {
        AudioAttributes alarm = alarm();
        focus = new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT).setAudioAttributes(alarm).build();
        getSystemService(AudioManager.class).requestAudioFocus(focus);
        for (Uri sound : sounds(this)) {
            MediaPlayer tried = new MediaPlayer();
            try {
                tried.setAudioAttributes(alarm);
                tried.setWakeMode(this, PowerManager.PARTIAL_WAKE_LOCK);
                tried.setDataSource(this, sound);
                tried.setLooping(true);
                tried.prepare();
                tried.setVolume(gain(0), gain(0));
                tried.start();
                player = tried;
                break;
            } catch (IOException | RuntimeException e) {
                tried.release();
                Log.w(DoseAlarms.TAG, "Waking: " + sound + " does not play: " + e);
            }
        }
        started = SystemClock.elapsedRealtime();
        if (player != null)
            handler.post(rise);
        else
            beep();
    }

    /** The gain at `t` of the rise (0 to 1): even steps in decibels, as the ear hears them. */
    private static float gain(double t)
    {
        return (float) Math.pow(10, QUIET_DB * (1 - Math.min(1, Math.max(0, t))) / 20);
    }

    private final Runnable rise = new Runnable() {
        @Override
        public void run()
        {
            if (player == null)
                return;
            double t = (SystemClock.elapsedRealtime() - started) / (double) RISE_MS;
            try {
                player.setVolume(gain(t), gain(t));
            } catch (IllegalStateException e) {
                return;
            }
            if (t < 1)
                handler.postDelayed(this, 250);
        }
    };

    /** No sound plays: beeps, as an alarm, at full; the phone kept awake for them until the end. */
    private void beep()
    {
        try {
            beeps = new ToneGenerator(AudioManager.STREAM_ALARM, ToneGenerator.MAX_VOLUME);
        } catch (RuntimeException e) {
            Log.e(DoseAlarms.TAG, "Waking: no sound at all could play.", e);
            return;
        }
        synchronized (WakeRinger.class) {
            if (held != null)
                held.acquire();
        }
        handler.post(beeping);
    }

    private final Runnable beeping = new Runnable() {
        @Override
        public void run()
        {
            if (beeps == null)
                return;
            beeps.startTone(ToneGenerator.TONE_PROP_BEEP2, 600);
            handler.postDelayed(this, 1500);
        }
    };

    private void silence()
    {
        handler.removeCallbacks(rise);
        handler.removeCallbacks(beeping);
        if (player != null) {
            try {
                player.stop();
            } catch (IllegalStateException e) {
                // Never started.
            }
            player.release();
            player = null;
        }
        if (beeps != null) {
            beeps.release();
            beeps = null;
        }
        if (focus != null) {
            getSystemService(AudioManager.class).abandonAudioFocusRequest(focus);
            focus = null;
        }
    }

    /** Android refused this service: the notification and the alarm sound from here, as long as Android lets the process be. */
    private static synchronized void ringBare(Context context, long at, boolean trial, boolean lastTry)
    {
        begin(trial, lastTry, bare != null);
        context.getSystemService(NotificationManager.class).notify(NOTE, note(context, at));
        if (bare != null)
            return;
        bare = RingtoneManager.getRingtone(context, Settings.System.DEFAULT_ALARM_ALERT_URI);
        if (bare == null)
            bare = RingtoneManager.getRingtone(context, Settings.System.DEFAULT_NOTIFICATION_URI);
        if (bare != null) {
            bare.setAudioAttributes(alarm());
            bare.setLooping(true);
            bare.play();
        }
    }

    private static synchronized void hold(Context context)
    {
        if (held == null) {
            held = context.getSystemService(PowerManager.class).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "sioul:waking");
            held.setReferenceCounted(false);
        }
        held.acquire(60_000);
    }

    private static synchronized void release()
    {
        if (held != null && held.isHeld())
            held.release();
    }

    // ---------------------------------------------------------------- what it shows

    /** "07:00", as the phone writes times (12 or 24 hours). */
    static String clock(Context context, long at)
    {
        return DateFormat.getTimeFormat(context).format(new Date(at));
    }

    /**
     * The ring's notification: the screen over the lock screen (full-screen,
     * WakeScreen), Stop and 10 min later (not on a try's last ring); swiped
     * away, it stops. Shown on the lock screen whole: a time and two buttons.
     * A try's buttons by codes of their own, saying a try's.
     */
    private static Notification note(Context context, long at)
    {
        boolean trial = trying;
        int stop = trial ? WakeAlarms.TRY_STOP_CODE : WakeAlarms.STOP_CODE;
        WakeAlarms.channel(context);
        PendingIntent screen = PendingIntent.getActivity(context, WakeAlarms.SCREEN_CODE,
                                                         new Intent(context, WakeScreen.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_NO_USER_ACTION),
                                                         PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification.Builder built = new Notification.Builder(context, WakeAlarms.CHANNEL)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentTitle(WakeAlarms.word(context, "title"))
            .setContentText(clock(context, at))
            .setCategory(Notification.CATEGORY_ALARM)
            .setVisibility(Notification.VISIBILITY_PUBLIC)
            .setOngoing(true)
            .setShowWhen(false)
            .setFullScreenIntent(screen, true)
            .setContentIntent(screen)
            .setDeleteIntent(button(context, WakeAlarms.STOP, stop, trial))
            .addAction(action(context, WakeAlarms.STOP, stop, "stop", trial));
        if (laterOffered())
            built.addAction(action(context, WakeAlarms.LATER, trial ? WakeAlarms.TRY_LATER_CODE : WakeAlarms.LATER_CODE, "later", trial));
        return built.build();
    }

    /**
     * After "10 min later": when it rings again, with Stop (a try's, its own
     * line and Stop). Silent: the "Waking" channel has no sound of its own
     * (WakeAlarms.channel).
     */
    private static void again(Context context, long at, boolean trial)
    {
        WakeAlarms.channel(context);
        Notification note = new Notification.Builder(context, WakeAlarms.CHANNEL)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentTitle(WakeAlarms.word(context, "title"))
            .setContentText(WakeAlarms.word(context, "again").replace("{time}", clock(context, at)))
            .setCategory(Notification.CATEGORY_STATUS)
            .setVisibility(Notification.VISIBILITY_PUBLIC)
            .setShowWhen(false)
            .setOnlyAlertOnce(true)
            .setTimeoutAfter(at - System.currentTimeMillis() + 60_000L)
            .addAction(action(context, WakeAlarms.STOP, trial ? WakeAlarms.TRY_STOP_CODE : WakeAlarms.STOP_CODE, "stop", trial))
            .build();
        context.getSystemService(NotificationManager.class).notify(trial ? TRY_AGAIN_NOTE : AGAIN_NOTE, note);
    }

    private static PendingIntent button(Context context, String what, int code, boolean trial)
    {
        return PendingIntent.getBroadcast(context, code, new Intent(context, WakeReceiver.class).setAction(what).putExtra(WakeAlarms.TRIAL, trial),
                                          PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    private static Notification.Action action(Context context, String what, int code, String word, boolean trial)
    {
        return new Notification.Action.Builder(Icon.createWithResource(context, R.drawable.sioul_notification), WakeAlarms.word(context, word), button(context, what, code, trial)).build();
    }
}
