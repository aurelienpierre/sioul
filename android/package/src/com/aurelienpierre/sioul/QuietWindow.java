// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * The quiet window (docs/android.md, "The quiet window"; option 1 of
 * docs/research/android-holding.md): a few seconds under Android's listener
 * hint HINT_HOST_DISABLE_NOTIFICATION_EFFECTS, while what Sioul gathered
 * comes back, so that it comes back without a sound.
 *
 * Used only where a held notification's return rings ({@link #returnsRing}):
 * Android 9 and 10, and Android 13 from its first quarterly release on. On
 * 11, 12, 12L and 13.0 every return is silent already: there nothing changes
 * from before (no window, no time past, no plan).
 *
 * A gathered notification is held a few minutes past its time
 * ({@link #SAFETY_MS}: Android's own timer, the safety net) and planned at
 * its time ({@link #hold}). Only Sioul's own alarm opens a window
 * ({@link #rang}), never a review: right then Sioul's own reminders and doses
 * are read again, and the window moves past any whose sound could fall
 * inside it ({@link #start}); then the hint is set, each due notification
 * held again for a second (Android brings it back then), and the hint taken
 * off once each came back, or at the window's end ({@link #AT_MOST_MS}),
 * counted both on Android's main thread and by an exact alarm that counts
 * sleep, with a partial wake lock held at most that long and a margin. A
 * hint belongs to the listener that asked it: Android drops it when the
 * listener is unbound or its process dies.
 *
 * Anything else posted meanwhile that would have rung (a person's message,
 * one of Sioul's own notices) came silent: once the hint is off, each one
 * still shown is held again for a second, on the listener's worker after any
 * decision pending for it, so that Android posts it again as new and it
 * rings; so is one posted in the instant before the hint went
 * ({@link #LATE_STRANGER_MS}). On Android 9 and 10, where a held
 * notification keeps its first time, the window opens around that time
 * instead ({@link #LEAD_MS}), and cannot move.
 *
 * Pure: Android's side through {@link Host}; checked on a JVM
 * (android/jvm-checks/QuietCheck.java). Every method may be called from any
 * thread: the listener's worker, Android's main thread, the alarm's receiver.
 */
final class QuietWindow
{
    /** What the window asks of Android and of the listener. */
    interface Host
    {
        /** Android's hint that silences every notification set (true) or taken off (false). */
        void hint(boolean quiet);

        /** One notification held for `ms`: one held already comes back after it (Android 11 and later). */
        void snooze(String key, long ms);

        /** Of these keys, those Android shows now: neither gone nor held since. */
        Set<String> shown(Collection<String> keys);

        /** Every key Android shows now. */
        Set<String> active();

        /** The keys Android holds now; null when it cannot say (unbound, a failure). */
        Set<String> held();

        /** The times of Sioul's own reminders and doses to come (ms), read now. */
        long[] own();

        /** The wall clock (ms). */
        long now();

        /** Sioul's release alarm set at `at` (wall clock, ms), exact while the phone sleeps; -1: taken away. */
        void alarm(long at);

        /** The window's end in `ms`, counted on Android's main thread and by an exact alarm that counts sleep; -1: both taken away. */
        void ending(long ms);

        /** A partial wake lock held for `ms` at most; 0: let go. */
        void awake(long ms);

        /** `task` run on the listener's worker, after what it has to do already. */
        void later(Runnable task);

        /** The window closed, `left` of its keys not come back: the alarm's broadcast may end. */
        void closed(int left);
    }

    /** The window lasts this long at most once open (ms). */
    static final long AT_MOST_MS = 5_000;
    /** A released notification is held again this long (ms): Android brings it back then. */
    static final long AGAIN_MS = 1_000;
    /** Android's own timer brings a gathered notification back this long after its time (ms), when Sioul's alarm never came. */
    static final long SAFETY_MS = 5 * 60_000;
    /** On Android 9 and 10, the window opens this long before the notifications' own time (ms). */
    static final long LEAD_MS = 2_000;
    /** One of Sioul's own reminders may sound this long after its time (ms): Rust decides it first. */
    static final long BUSY_MS = 60_000;
    /** Due: its time is past, or within this (ms). */
    static final long DUE_MS = 1_000;
    /** The wake lock outlasts the window's end by this much at most (ms). */
    static final long MARGIN_MS = 2_000;
    /** After the hint went, a notification posted before it is still a stranger this long (ms). */
    static final long LATE_STRANGER_MS = 2_000;
    /** Android could not say what it holds: the alarm asked again this soon (ms). */
    static final long RETRY_MS = 5_000;
    /** A window ends this long before the safety net at the latest (ms). */
    static final long BEFORE_SAFETY_MS = 30_000;
    /** Android's IMPORTANCE_DEFAULT: a channel that makes a sound. */
    static final int SOUND = 3;
    /** The first security patch of Android 13 QPR1, where a held notification's return rings again. */
    static final String QPR1_PATCH = "2022-12";

    private final Host host;
    private final boolean used;
    private final boolean fixed;
    private final Plan plan = new Plan();
    /** The released keys not come back yet. */
    private final Set<String> waiting = new HashSet<>();
    /** Posted inside the window, not released, ringing but for it: held again for a second at its end. */
    private final Set<String> strangers = new LinkedHashSet<>();
    /** The keys Android showed as the window opened: an update of one, alerting once, would not have rung. */
    private final Set<String> shownAtOpen = new HashSet<>();
    private boolean open;
    /** When the hint was last taken off (wall clock, ms); -1: never. */
    private long clearedAt = -1;

    /**
     * `used`: a held notification's return rings here ({@link #returnsRing});
     * `fixed`: a held notification keeps its first time (Android 9 and 10).
     */
    QuietWindow(Host host, boolean used, boolean fixed)
    {
        this.host = host;
        this.used = used;
        this.fixed = fixed;
    }

    /**
     * Whether a held notification's return rings as a new one on this
     * Android (docs/research/android-holding.md, 1), from its version and
     * its security patch ("2022-12-05"): on Android 9 and 10, yes; on 11,
     * 12 and 12L, no (Android brings every one back silent); on 13, only
     * from its first quarterly release (December 2022), which no version
     * number tells apart, so by the security patch, December 2022 or later
     * (a 13.0 kept at a later patch reads as QPR1: the window then gains
     * nothing there, and a stranger inside it comes back silent); from 14
     * on, yes.
     */
    static boolean returnsRing(int sdk, String patch)
    {
        if (sdk <= 29)
            return true;
        if (sdk <= 32)
            return false;
        if (sdk == 33)
            return patch != null && patch.length() >= 7 && patch.compareTo(QPR1_PATCH) >= 0;
        return true;
    }

    /** Whether the window is used here. */
    boolean used()
    {
        return used;
    }

    /** Whether the hint is set now. */
    synchronized boolean isOpen()
    {
        return open;
    }

    /**
     * A notification whose return comes without a sound, which Rust holds
     * `ms`: planned at its time, and how long Android holds it: a few minutes
     * past its time, the safety net; on Android 9 and 10, exactly its time;
     * where the window is not used, as asked.
     */
    synchronized long hold(String key, long ms)
    {
        if (!used)
            return ms;
        plan.put(key, host.now() + ms);
        arm();
        return fixed ? ms : ms + SAFETY_MS;
    }

    /** A notification no longer planned (come back, let through, held as a person's). */
    synchronized void forget(String key)
    {
        if (plan.of(key) >= 0) {
            plan.remove(key);
            arm();
        }
    }

    /**
     * A review's new time for a held notification (`ms` from now): how long
     * Android holds it now; -1, nothing to ask Android: one due now comes in
     * the window Sioul's alarm opens (never from a review).
     */
    synchronized long moved(long ms, boolean quiet)
    {
        if (!quiet || !used)
            return ms;
        if (ms <= DUE_MS)
            return -1;
        return fixed ? ms : ms + SAFETY_MS;
    }

    /** Rust's list at a review (key → time, ms): the plan rebuilt from it, the alarm set; no window opens here. */
    synchronized void follow(Map<String, Long> rust)
    {
        if (!used)
            return;
        plan.follow(rust, fixed, host.now());
        arm();
    }

    /** How many are planned. */
    synchronized int planned()
    {
        return plan.size();
    }

    /** When the next one is planned (ms), or -1. */
    synchronized long next()
    {
        return plan.next();
    }

    /**
     * Sioul's alarm rang: the window opened over what is due now, Sioul's
     * own reminders and doses read again first; one of them due within it,
     * the alarm moves past it instead (on Android 9 and 10, where nothing
     * moves, and when no window fits before the safety net, what is due comes
     * back with its sound); Android unable to say what it holds, the alarm
     * comes again in a few seconds, the plan kept. Whether a window opened
     * (the alarm's broadcast is then held until it closes).
     */
    synchronized boolean rang()
    {
        if (!used)
            return false;
        long now = host.now();
        long ahead = fixed ? LEAD_MS + DUE_MS : DUE_MS;
        if (!plan.hasDue(now, ahead)) {
            arm();
            return false;
        }
        long length = fixed ? LEAD_MS + AT_MOST_MS : AGAIN_MS + AT_MOST_MS;
        long latest = fixed ? now : plan.next() + SAFETY_MS - BEFORE_SAFETY_MS;
        long start = start(now, host.own(), length, latest);
        if (start < 0) {
            plan.due(now, ahead);
            arm();
            return false;
        }
        if (start > now + DUE_MS) {
            host.alarm(start);
            return false;
        }
        Set<String> held = host.held();
        if (held == null) {
            host.alarm(now + RETRY_MS);
            return false;
        }
        List<String> due = plan.due(now, ahead);
        due.retainAll(held);
        boolean opened = open(due, !fixed, fixed ? LEAD_MS + AT_MOST_MS : AT_MOST_MS);
        arm();
        return opened;
    }

    /**
     * The alarm set at the next window's start, off the times of Sioul's own
     * reminders and doses, or taken away when nothing is planned; what no
     * window can take before the safety net (or, on Android 9 and 10, at its
     * own time) comes back with its sound rather than silence one of Sioul's.
     */
    private void arm()
    {
        long[] own = host.own();
        while (true) {
            long next = plan.next();
            if (next < 0) {
                host.alarm(-1);
                return;
            }
            long start = fixed
                ? start(next - LEAD_MS, own, LEAD_MS + AT_MOST_MS, next - LEAD_MS)
                : start(next, own, AGAIN_MS + AT_MOST_MS, next + SAFETY_MS - BEFORE_SAFETY_MS);
            if (start >= 0) {
                host.alarm(start);
                return;
            }
            plan.due(next, 0);
        }
    }

    /**
     * Opened over `keys` (none: nothing, and false): the phone kept awake
     * the window's length and a margin, the hint set, then each held again
     * for a second when `move` (Android 11 and later), else left to come at
     * its own time; closed after `atMost` at the latest.
     */
    private boolean open(Collection<String> keys, boolean move, long atMost)
    {
        if (keys.isEmpty())
            return false;
        if (!open) {
            open = true;
            host.awake(atMost + MARGIN_MS);
            host.hint(true);
        }
        shownAtOpen.addAll(host.active());
        waiting.addAll(keys);
        if (move)
            for (String key : keys)
                host.snooze(key, AGAIN_MS);
        host.ending(atMost);
        return true;
    }

    /**
     * A notification posted, any app's, Sioul's own included (`postTime`, as
     * Android says it): one released is crossed off, and the window closes
     * once each came back; another, while open, is a stranger when it would
     * have rung (`rings`), unless it updates one shown as the window opened
     * and alerts once; one posted before the hint went, heard just after, is
     * one too.
     */
    synchronized void posted(String key, boolean rings, boolean alertOnce, long postTime)
    {
        if (!open) {
            if (rings && clearedAt >= 0 && postTime <= clearedAt && host.now() - clearedAt < LATE_STRANGER_MS)
                ringAgain(List.of(key));
            return;
        }
        if (waiting.remove(key)) {
            if (waiting.isEmpty())
                close();
            return;
        }
        if (rings && !(alertOnce && shownAtOpen.contains(key)))
            strangers.add(key);
    }

    /**
     * Closed: the hint taken off, on every way out; then, on the listener's
     * worker after any decision pending, each stranger Android still shows
     * held again for a second, so that it comes back ringing (one Sioul held
     * meanwhile keeps its time); the wake lock let go, the alarm's broadcast
     * let end.
     */
    synchronized void close()
    {
        if (!open)
            return;
        open = false;
        int left = waiting.size();
        try {
            host.ending(-1);
            host.hint(false);
        } finally {
            clearedAt = host.now();
            ringAgain(new ArrayList<>(strangers));
            strangers.clear();
            waiting.clear();
            shownAtOpen.clear();
            host.awake(0);
            host.closed(left);
        }
    }

    /** Each of `keys` still shown held again for a second, on the worker, after what it has to do already. */
    private void ringAgain(List<String> keys)
    {
        if (keys.isEmpty())
            return;
        host.later(() -> {
            Set<String> still = host.shown(keys);
            for (String key : keys)
                if (still.contains(key))
                    host.snooze(key, AGAIN_MS);
        });
    }

    /**
     * Forgotten, the listener unbound: Android dropped its hint with it, so
     * nothing is asked of it; its end and wake lock let go, the broadcast let
     * end.
     */
    synchronized void dropped()
    {
        if (!open)
            return;
        open = false;
        int left = waiting.size();
        strangers.clear();
        waiting.clear();
        shownAtOpen.clear();
        host.ending(-1);
        host.awake(0);
        host.closed(left);
    }

    /**
     * Whether a notification posted inside the window would have rung
     * without it: its channel makes a sound (`importance`, Android's), the
     * phone's do-not-disturb lets it through (`matches`), and it is not a
     * call (its ringtone passes the hint), what runs or rings full screen
     * (holding them again would start them again), nor a group's summary
     * (holding it holds its whole group).
     */
    static boolean rings(int importance, boolean matches, boolean fullScreen, String category, boolean ongoing, boolean summary)
    {
        if (ongoing || fullScreen || summary || "call".equals(category))
            return false;
        return importance >= SOUND && matches;
    }

    /**
     * When a window for notifications due at `due` opens (ms): at `due`, or
     * later, past each of Sioul's own reminders whose sound could fall inside
     * it (`own`: their times; each may sound from its time to
     * {@link #BUSY_MS} after); -1 when that is past `latest`, and no window
     * opens (Android's own return comes then, with its sound).
     */
    static long start(long due, long[] own, long length, long latest)
    {
        long[] sorted = own.clone();
        Arrays.sort(sorted);
        long at = due;
        boolean moved = true;
        while (moved) {
            moved = false;
            for (long time : sorted) {
                if (at < time + BUSY_MS && time < at + length) {
                    at = time + BUSY_MS;
                    moved = true;
                }
            }
        }
        return at > latest ? -1 : at;
    }

    /**
     * What is to come back quiet, by key, with its time (ms): kept by the
     * listener in memory, set as Rust decides each one, and rebuilt from
     * Rust's list at each review, so that a listener started again finds it.
     */
    static final class Plan
    {
        private final Map<String, Long> at = new HashMap<>();

        synchronized void put(String key, long time)
        {
            at.put(key, time);
        }

        synchronized void remove(String key)
        {
            at.remove(key);
        }

        synchronized int size()
        {
            return at.size();
        }

        /** Its time, or -1 when not planned. */
        synchronized long of(String key)
        {
            Long time = at.get(key);
            return time == null ? -1 : time;
        }

        /**
         * Rust's list at a review (key → time): what Android holds and Rust
         * says comes back quiet. Android 11 and later: its times. Android 9
         * and 10 (`fixed`), where a held notification keeps its first time,
         * the times known here stay, and one not known is planned only when
         * still to come (after a restart of the listener there, its first
         * return may ring: docs/android.md).
         */
        synchronized void follow(Map<String, Long> rust, boolean fixed, long now)
        {
            if (!fixed) {
                at.clear();
                at.putAll(rust);
                return;
            }
            at.keySet().retainAll(rust.keySet());
            for (Map.Entry<String, Long> one : rust.entrySet())
                if (!at.containsKey(one.getKey()) && one.getValue() > now)
                    at.put(one.getKey(), one.getValue());
        }

        /** Whether one is due by `now` (its time within `ahead`). */
        synchronized boolean hasDue(long now, long ahead)
        {
            for (long time : at.values())
                if (time <= now + ahead)
                    return true;
            return false;
        }

        /** Those due by `now` (their time within `ahead`), taken out of the plan, in order of time. */
        synchronized List<String> due(long now, long ahead)
        {
            List<Map.Entry<String, Long>> due = new ArrayList<>();
            for (Map.Entry<String, Long> one : at.entrySet())
                if (one.getValue() <= now + ahead)
                    due.add(one);
            due.sort((a, b) -> a.getValue().equals(b.getValue()) ? a.getKey().compareTo(b.getKey()) : Long.compare(a.getValue(), b.getValue()));
            List<String> keys = new ArrayList<>();
            for (Map.Entry<String, Long> one : due) {
                keys.add(one.getKey());
                at.remove(one.getKey());
            }
            return keys;
        }

        /** The soonest time planned, or -1. */
        synchronized long next()
        {
            long next = -1;
            for (long time : at.values())
                if (next < 0 || time < next)
                    next = time;
            return next;
        }
    }
}
