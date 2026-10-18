// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Notification;
import android.app.PendingIntent;
import android.app.RemoteInput;
import android.content.Context;
import android.content.Intent;
import android.os.Build;
import android.os.Bundle;
import android.os.Parcelable;

import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.List;

/**
 * A reply from your computers sent through a notification's own Reply, as
 * the phone's shade sends one typed under it (docs/android.md, "Replies from
 * your computers"; Rust's side: crates/sioul-app/src/replies.rs,
 * crates/sioul-core/src/replies.rs). Here: which of a notification's actions
 * is its Reply and which its Mark as read, found by what they mean, never by
 * their titles nor their places ({@link #choose}); which may not be sent
 * ({@link #refusal}); and the sending, the shade's own way ({@link #send}).
 *
 * Nothing else is ever sent: no Delete, Archive, Block, Call, a bot's
 * button nor Copy; no notification is tapped, dismissed or cancelled; no
 * action that would open an app on the phone. Before Android 12 nothing is
 * offered at all: its actions do not say what {@link #refusal} needs.
 */
final class Replies
{
    /** Where an action was found: the notification's own actions, the car extender's invisible ones, the watch's. */
    static final int VISIBLE = 0;
    static final int CAR = 1;
    static final int WATCH = 2;
    /** The places, in the order they are looked in. */
    private static final int[] PLACES = { VISIBLE, CAR, WATCH };
    /** Where AndroidX keeps a car extender's invisible actions in the extras (NotificationCompat.CarExtender, addInvisibleAction). */
    private static final String CAR_EXTENSIONS = "android.car.EXTENSIONS";
    private static final String CAR_INVISIBLE = "invisible_actions";

    private Replies()
    {
    }

    /**
     * One action of a notification as the choice reads it: plain data, made
     * on the phone from a Notification.Action or a car extender's bundle
     * ({@link #found}), by hand in android/jvm-checks/RepliesCheck.java.
     */
    static final class Act
    {
        final int place;
        /** What Android says it means (Notification.Action.getSemanticAction): 1 Reply, 2 Mark as read, 0 nothing said. */
        final int semantic;
        /** Added by the system or its assistant from the notification's words (an address to open): never the app's own. */
        final boolean contextual;
        /** How many of its inputs take free text (each with its key), and whether it has inputs at all. */
        final int freeText;
        final boolean inputs;
        /** Its PendingIntent: immutable, which would drop the words without a word from Android. */
        final boolean immutable;
        /** Its PendingIntent starts an activity: it would open the app on the phone, maybe under the lock screen. */
        final boolean activity;
        /** Its app asks that it be sent only from an unlocked phone (Notification.Action.isAuthenticationRequired). */
        final boolean authentication;
        /** Its title, as the app wrote it: never read by the choice. */
        final String title;

        Act(int place, int semantic, boolean contextual, int freeText, boolean inputs, boolean immutable, boolean activity, boolean authentication, String title)
        {
            this.place = place;
            this.semantic = semantic;
            this.contextual = contextual;
            this.freeText = freeText;
            this.inputs = inputs;
            this.immutable = immutable;
            this.activity = activity;
            this.authentication = authentication;
            this.title = title == null ? "" : title;
        }
    }

    /** What a notification offers ({@link #choose}): its Reply and its Mark as read, each null when none. */
    static final class Choice
    {
        final Act reply;
        final Act read;

        Choice(Act reply, Act read)
        {
            this.reply = reply;
            this.read = read;
        }
    }

    /**
     * A notification's Reply and Mark as read, by what Android says they
     * mean, looked for in its own actions, then the car extender's, then the
     * watch's: Reply, an action that says it is one (SEMANTIC_ACTION_REPLY)
     * with an input that takes free text; Mark as read, one that says it is
     * one (SEMANTIC_ACTION_MARK_AS_READ). The first place that has one
     * decides; two in that place ("Reply" and "Reply privately" in a group,
     * two Mark as read) would be a guess: none, never the first by order.
     * When no action says it is a Reply, the one action whose input takes
     * free text, said nothing of, in a notification Android takes as a
     * conversation or the phone's SMS app's; two such would be a guess: none.
     * Never one the system added (contextual), never one found by its title
     * or its place. Pure: android/jvm-checks/RepliesCheck.java.
     */
    static Choice choose(List<Act> acts, boolean conversation, boolean smsApp)
    {
        Act reply = null;
        Act read = null;
        boolean saysReply = false;
        boolean replyFound = false;
        boolean readFound = false;
        for (int place : PLACES) {
            Act placeReply = null;
            Act placeRead = null;
            int replies = 0;
            int reads = 0;
            for (Act act : acts) {
                if (act.place != place || act.contextual)
                    continue;
                saysReply |= act.semantic == Notification.Action.SEMANTIC_ACTION_REPLY;
                if (act.semantic == Notification.Action.SEMANTIC_ACTION_REPLY && act.freeText > 0) {
                    placeReply = act;
                    replies++;
                }
                if (act.semantic == Notification.Action.SEMANTIC_ACTION_MARK_AS_READ) {
                    placeRead = act;
                    reads++;
                }
            }
            if (!replyFound && replies > 0) {
                reply = replies == 1 ? placeReply : null;
                replyFound = true;
            }
            if (!readFound && reads > 0) {
                read = reads == 1 ? placeRead : null;
                readFound = true;
            }
        }
        if (reply == null && !saysReply && (conversation || smsApp)) {
            for (int place : PLACES) {
                Act only = null;
                int count = 0;
                for (Act act : acts) {
                    if (act.place == place && !act.contextual && act.semantic == Notification.Action.SEMANTIC_ACTION_NONE && act.freeText > 0) {
                        only = act;
                        count++;
                    }
                }
                if (count == 1)
                    reply = only;
                if (count > 0)
                    break;
            }
        }
        return new Choice(reply, read);
    }

    /**
     * Why an action may not be sent; "" when it may. "none": there is none;
     * "activity": it would open an app on the phone; "closed": a Reply
     * (`fill`) whose PendingIntent is immutable; "locked": its app asks for
     * an unlocked phone and the phone is locked. Pure:
     * android/jvm-checks/RepliesCheck.java.
     */
    static String refusal(Act act, boolean fill, boolean locked)
    {
        if (act == null)
            return "none";
        if (act.activity)
            return "activity";
        if (fill && act.immutable)
            return "closed";
        if (act.authentication && locked)
            return "locked";
        return "";
    }

    /**
     * Which of the notifications listed a reply names: the one whose name is
     * its (AppNotes.talkKey of its app and Android's key, as Rust's
     * `replies::notice`), of this phone's own profile, never a work
     * profile's; -1 for none. Pure: android/jvm-checks/RepliesCheck.java.
     */
    static int live(String[] packages, String[] keys, boolean[] own, String notice)
    {
        for (int i = 0; i < packages.length && i < keys.length && i < own.length; i++)
            if (own[i] && packages[i] != null && keys[i] != null && AppNotes.talkKey(packages[i], keys[i]).equals(notice))
                return i;
        return -1;
    }

    // ---------------------------------------------------------------- on the phone

    /** An action found on the phone: what the choice reads, and what sends it. */
    static final class Found
    {
        final Act act;
        final PendingIntent intent;
        /** Its first input that takes free text, the one the shade fills; null for none. */
        final RemoteInput input;

        Found(Act act, PendingIntent intent, RemoteInput input)
        {
            this.act = act;
            this.intent = intent;
            this.input = input;
        }
    }

    /** What the choice reads of the actions found. */
    static List<Act> acts(List<Found> found)
    {
        List<Act> out = new ArrayList<>();
        for (Found one : found)
            out.add(one.act);
        return out;
    }

    /** The action found for an act chosen; null for none. */
    static Found of(List<Found> found, Act act)
    {
        for (Found one : found)
            if (one.act == act)
                return one;
        return null;
    }

    /**
     * A notification's actions in the three places: its own, the car
     * extender's invisible ones, the watch's. None before Android 12.
     */
    @SuppressWarnings("deprecation")
    static List<Found> found(Notification n)
    {
        List<Found> out = new ArrayList<>();
        if (n == null || Build.VERSION.SDK_INT < Build.VERSION_CODES.S)
            return out;
        if (n.actions != null)
            for (Notification.Action action : n.actions)
                add(out, VISIBLE, action);
        try {
            car(n.extras, out);
        } catch (RuntimeException e) {
            // A car extender's extras that do not read: none of its actions.
        }
        try {
            for (Notification.Action action : new Notification.WearableExtender(n).getActions())
                add(out, WATCH, action);
        } catch (RuntimeException e) {
            // A watch's extras that do not read: none of its actions.
        }
        return out;
    }

    /** One of a notification's own or a watch's actions. */
    private static void add(List<Found> out, int place, Notification.Action action)
    {
        if (action == null || action.actionIntent == null)
            return;
        RemoteInput first = null;
        int free = 0;
        RemoteInput[] inputs = action.getRemoteInputs();
        if (inputs != null) {
            for (RemoteInput input : inputs) {
                if (input != null && input.getAllowFreeFormInput() && input.getResultKey() != null && !input.getResultKey().isEmpty()) {
                    free++;
                    if (first == null)
                        first = input;
                }
            }
        }
        PendingIntent intent = action.actionIntent;
        Act act = new Act(place, action.getSemanticAction(), action.isContextual(), free, inputs != null && inputs.length > 0,
                          intent.isImmutable(), intent.isActivity(), action.isAuthenticationRequired(), action.title == null ? "" : action.title.toString());
        out.add(new Found(act, intent, first));
    }

    /**
     * A car extender's invisible actions, as AndroidX keeps them: bundles
     * "0", "1"… each with its PendingIntent ("actionIntent"), its meaning
     * ("semanticAction"), its title and its inputs as bundles of their own
     * ("remoteInputs": "resultKey", "allowFreeFormInput"); an input made again
     * here from its key, which is all the shade's filling reads of it.
     */
    @SuppressWarnings("deprecation")
    private static void car(Bundle extras, List<Found> out)
    {
        Bundle car = extras == null ? null : extras.getBundle(CAR_EXTENSIONS);
        Bundle invisible = car == null ? null : car.getBundle(CAR_INVISIBLE);
        if (invisible == null)
            return;
        for (int i = 0; invisible.containsKey(Integer.toString(i)); i++) {
            Bundle one = invisible.getBundle(Integer.toString(i));
            Parcelable intent = one == null ? null : one.getParcelable("actionIntent");
            if (!(intent instanceof PendingIntent))
                continue;
            RemoteInput first = null;
            int free = 0;
            boolean any = false;
            Parcelable[] inputs = one.getParcelableArray("remoteInputs");
            if (inputs != null) {
                for (Parcelable input : inputs) {
                    if (!(input instanceof Bundle))
                        continue;
                    any = true;
                    Bundle described = (Bundle) input;
                    String key = described.getString("resultKey");
                    if (described.getBoolean("allowFreeFormInput", false) && key != null && !key.isEmpty()) {
                        free++;
                        if (first == null)
                            first = new RemoteInput.Builder(key).build();
                    }
                }
            }
            PendingIntent pending = (PendingIntent) intent;
            CharSequence title = one.getCharSequence("title");
            Act act = new Act(CAR, one.getInt("semanticAction", Notification.Action.SEMANTIC_ACTION_NONE), false, free, any,
                              pending.isImmutable(), pending.isActivity(), false, title == null ? "" : title.toString());
            out.add(new Found(act, pending, first));
        }
    }

    /**
     * What a notification offers a reply from your computers, for Rust
     * (phonemsgs.rs, `Offer`): a Reply and a Mark as read there and
     * sendable (the lock aside, which is looked at when one is sent).
     */
    static JSONObject offer(Notification n, boolean conversation, boolean smsApp) throws JSONException
    {
        Choice choice = choose(acts(found(n)), conversation, smsApp);
        return new JSONObject()
            .put("reply", refusal(choice.reply, true, false).isEmpty())
            .put("read", refusal(choice.read, false, false).isEmpty());
    }

    /**
     * What another profile's notification (a work profile's) offers your
     * computers: nothing, since the listener never answers one ({@link #live}).
     */
    static JSONObject nothing() throws JSONException
    {
        return new JSONObject().put("reply", false).put("read", false);
    }

    /**
     * Sent as the phone's shade sends a reply typed under a notification
     * (SystemUI's RemoteInputViewController): the words under its first
     * free-text input's key, in a fill-in intent sent to the foreground's
     * receivers, marked typed by a person (SOURCE_FREE_FORM_INPUT); a Mark
     * as read (`fill` false), with no fill-in. No start of an activity is
     * asked for. Throws when the intent is spent or cancelled: its
     * notification is gone.
     */
    static void send(Context context, Found found, String words, boolean fill) throws PendingIntent.CanceledException
    {
        if (!fill) {
            found.intent.send(context, 0, null, null, null);
            return;
        }
        Intent filled = new Intent().addFlags(Intent.FLAG_RECEIVER_FOREGROUND);
        Bundle results = new Bundle();
        results.putCharSequence(found.input.getResultKey(), words);
        RemoteInput.addResultsToIntent(new RemoteInput[] { found.input }, filled, results);
        RemoteInput.setResultsSource(filled, RemoteInput.SOURCE_FREE_FORM_INPUT);
        found.intent.send(context, 0, filled, null, null);
    }
}
