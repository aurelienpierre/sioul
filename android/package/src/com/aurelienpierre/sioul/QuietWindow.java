// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * The quiet window (docs/android.md, "Notifications from other apps"; option
 * 1 of docs/research/android-holding.md): a few seconds under Android's
 * listener hint HINT_HOST_DISABLE_NOTIFICATION_EFFECTS, while what Sioul
 * gathered comes back, so that it comes back without a sound. From Android
 * 13 QPR1, and on Android 10, a held notification's return rings as a new
 * one: three times a day, every app gathered would ring at once.
 *
 * The listener ({@link AppNotes}) holds a gathered notification a few
 * minutes past its time ({@link #SAFETY_MS}: Android's own timer, the safety
 * net), and at its time, from Sioul's own alarm, opens the window: the hint
 * set, each released key held again for a second (Android brings it back
 * then), the hint taken off once each came back, or after
 * {@link #AT_MOST_MS} at most. A hint belongs to the listener that asked it:
 * Android drops it when the listener is unbound or its process dies, so a
 * crash inside the window never leaves the phone silent. Anything else posted
 * meanwhile (a person's message, one of Sioul's own) comes silent too: once
 * the window closes, each such one still shown is held again for a second,
 * so that it comes back ringing. On Android 9 and 10, where a held
 * notification keeps its first time, the window opens around that time
 * instead ({@link #LEAD_MS}).
 *
 * Pure: Android's side through {@link Host}; checked on a JVM
 * (android/jvm-checks/QuietCheck.java). Every method may be called from any
 * thread: the listener's worker opens it, Android's main thread hears each
 * notification posted and runs its end.
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

        /** `task` run in `ms`, on a thread never held up by Rust (Android's main thread). */
        void after(Runnable task, long ms);

        /** `task` dropped, when it has not run yet. */
        void cancel(Runnable task);

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
    /** Android's IMPORTANCE_DEFAULT: a channel that makes a sound. */
    static final int SOUND = 3;

    private final Host host;
    /** The released keys not come back yet. */
    private final Set<String> waiting = new HashSet<>();
    /** Posted inside the window, not released, ringing but for it: held again for a second at its end. */
    private final Set<String> strangers = new LinkedHashSet<>();
    /** The keys Android showed as the window opened: an update of one, alerting once, would not have rung. */
    private final Set<String> shownAtOpen = new HashSet<>();
    private boolean open;
    private final Runnable end = this::close;

    QuietWindow(Host host)
    {
        this.host = host;
    }

    /** Whether the hint is set now. */
    synchronized boolean isOpen()
    {
        return open;
    }

    /**
     * Opened over `keys` (none: nothing, and false): the hint set, then each
     * held again for a second when `move` (Android 11 and later), else left
     * to come at its own time (Android 9 and 10); closed after `atMost` at
     * the latest. `shown`, the keys Android shows now. Open already, the keys
     * join it and its end moves.
     */
    synchronized boolean open(Collection<String> keys, boolean move, long atMost, Collection<String> shown)
    {
        if (keys.isEmpty())
            return false;
        if (!open) {
            open = true;
            host.hint(true);
        }
        shownAtOpen.addAll(shown);
        waiting.addAll(keys);
        if (move)
            for (String key : keys)
                host.snooze(key, AGAIN_MS);
        host.cancel(end);
        host.after(end, atMost);
        return true;
    }

    /**
     * A notification posted, any app's, Sioul's own included: one released
     * is crossed off, and the window closes once each came back; another,
     * while open, is a stranger when it would have rung (`rings`), unless it
     * updates one shown as the window opened and alerts once.
     */
    synchronized void posted(String key, boolean rings, boolean alertOnce)
    {
        if (!open)
            return;
        if (waiting.remove(key)) {
            if (waiting.isEmpty())
                close();
            return;
        }
        if (rings && !(alertOnce && shownAtOpen.contains(key)))
            strangers.add(key);
    }

    /**
     * Closed: the hint taken off, then each stranger Android still shows held
     * again for a second, so that it comes back ringing (one Sioul held
     * meanwhile keeps its time); the alarm's broadcast let end.
     */
    synchronized void close()
    {
        if (!open)
            return;
        open = false;
        host.cancel(end);
        int left = waiting.size();
        try {
            host.hint(false);
        } finally {
            Set<String> still = strangers.isEmpty() ? Collections.<String>emptySet() : host.shown(new ArrayList<>(strangers));
            for (String key : strangers)
                if (still.contains(key))
                    host.snooze(key, AGAIN_MS);
            strangers.clear();
            waiting.clear();
            shownAtOpen.clear();
            host.closed(left);
        }
    }

    /**
     * Forgotten, the listener unbound: Android dropped its hint with it, so
     * nothing is asked of it; the broadcast let end.
     */
    synchronized void dropped()
    {
        if (!open)
            return;
        open = false;
        host.cancel(end);
        int left = waiting.size();
        strangers.clear();
        waiting.clear();
        shownAtOpen.clear();
        host.closed(left);
    }

    /**
     * Whether a notification posted inside the window would have rung
     * without it: its channel makes a sound (`importance`, Android's), the
     * phone's do-not-disturb lets it through (`matches`), and it is neither
     * what runs nor a call or an alarm (their sound is no notification's, and
     * holding them again would start them again).
     */
    static boolean rings(int importance, boolean matches, boolean fullScreen, String category, boolean ongoing)
    {
        if (ongoing || fullScreen || "call".equals(category) || "alarm".equals(category))
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
         * still to come; a release now cannot move it there.
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
