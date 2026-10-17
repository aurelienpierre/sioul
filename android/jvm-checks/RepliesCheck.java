// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Notification;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;

/**
 * Replies on the JVM (Replies.choose, Replies.refusal, Replies.live), on
 * stand-in notifications shaped as the apps read in their sources make
 * theirs (docs/android.md, "Replies from your computers"): Signal, WhatsApp,
 * Telegram with a bot's buttons, Element X, /e/OS's Message with Delete,
 * Block filed as Mute and Call, K-9 Mail; a Reply that cannot be filled, one
 * that would open the app, one that asks for an unlocked phone; the buttons
 * in another order and under swapped titles; one the system added; the car
 * extender's and the watch's; the notification gone, or another profile's.
 * The notification's name as Rust's `replies::notice` makes it.
 */
public final class RepliesCheck
{
    static int checked = 0;
    static int failed = 0;

    static final int REPLY = Notification.Action.SEMANTIC_ACTION_REPLY;
    static final int READ = Notification.Action.SEMANTIC_ACTION_MARK_AS_READ;
    static final int NONE = Notification.Action.SEMANTIC_ACTION_NONE;

    static void expect(String what, boolean ok)
    {
        checked++;
        if (!ok) {
            failed++;
            System.out.println("FAIL " + what);
        }
    }

    /** An app's own action, a broadcast that may be filled: its place, meaning, free-text inputs and title. */
    static Replies.Act act(int place, int semantic, int free, String title)
    {
        return new Replies.Act(place, semantic, false, free, free > 0, false, false, false, title);
    }

    /** The same, an immutable PendingIntent (a Mark as read needs no fill-in). */
    static Replies.Act fixed(int place, int semantic, String title)
    {
        return new Replies.Act(place, semantic, false, 0, false, true, false, false, title);
    }

    static List<Replies.Act> list(Replies.Act... acts)
    {
        return new ArrayList<>(Arrays.asList(acts));
    }

    public static void main(String[] args)
    {
        int visible = Replies.VISIBLE;

        // Signal: Mark read and Reply, the Reply on the watch too; it asks no unlocked phone.
        Replies.Act signalRead = act(visible, READ, 0, "Mark read");
        Replies.Act signalReply = act(visible, REPLY, 1, "Reply");
        Replies.Choice signal = Replies.choose(list(signalRead, signalReply, act(Replies.WATCH, REPLY, 1, "Reply")), true, false);
        expect("Signal: its own Reply, not the watch's", signal.reply == signalReply);
        expect("Signal: its Mark read", signal.read == signalRead);
        expect("Signal: sent locked or not", Replies.refusal(signal.reply, true, true).isEmpty() && Replies.refusal(signal.read, false, true).isEmpty());

        // WhatsApp, as Android Auto's rules make it; an older build saying no meaning: the one free-text action, in a conversation only.
        Replies.Act whatsReply = act(visible, REPLY, 1, "Reply");
        expect("WhatsApp: its Reply", Replies.choose(list(act(visible, READ, 0, "Mark as read"), whatsReply), true, false).reply == whatsReply);
        Replies.Act unsaid = act(visible, NONE, 1, "Reply");
        expect("a Reply that says no meaning, in a conversation", Replies.choose(list(unsaid), true, false).reply == unsaid);
        expect("…or the SMS app's", Replies.choose(list(unsaid), false, true).reply == unsaid);
        expect("…never in another notification", Replies.choose(list(unsaid), false, false).reply == null);

        // Telegram: Reply, Mark as read, a code's Copy and a bot's buttons (one takes words): Reply alone.
        Replies.Act telegramReply = act(visible, REPLY, 1, "Reply");
        Replies.Act telegramRead = act(visible, READ, 0, "Mark as read");
        List<Replies.Act> telegram = list(act(visible, NONE, 0, "Copy"), act(visible, NONE, 0, "Yes"), act(visible, NONE, 1, "Answer the bot"), telegramReply, telegramRead);
        Replies.Choice bot = Replies.choose(telegram, true, false);
        expect("Telegram: Reply, never a bot's button", bot.reply == telegramReply && bot.read == telegramRead);
        expect("no Reply said, two that take words: a guess, none", Replies.choose(list(act(visible, NONE, 1, "Reply"), act(visible, NONE, 1, "Answer the bot")), true, false).reply == null);
        expect("a Reply said that takes no words: none, never another", Replies.choose(list(act(visible, REPLY, 0, "Reply"), act(visible, NONE, 1, "Answer the bot")), true, false).reply == null);

        // Element X: its Mark as read is immutable, which a Mark as read may be.
        Replies.Act elementRead = fixed(visible, READ, "Mark as read");
        Replies.Choice element = Replies.choose(list(act(visible, REPLY, 1, "Quick reply"), elementRead), true, false);
        expect("Element: Reply and Mark as read", element.reply != null && element.read == elementRead);
        expect("an immutable Mark as read goes", Replies.refusal(element.read, false, false).isEmpty());

        // /e/OS's Message: Read, Reply, Archive, Delete, Block filed under Mute, Call (it dials).
        Replies.Act messageRead = fixed(visible, READ, "Read");
        Replies.Act messageReply = act(visible, REPLY, 1, "Reply");
        Replies.Act call = new Replies.Act(visible, Notification.Action.SEMANTIC_ACTION_CALL, false, 0, false, true, true, false, "Call");
        List<Replies.Act> message = list(messageRead, messageReply, act(visible, Notification.Action.SEMANTIC_ACTION_ARCHIVE, 0, "Archive"),
                                         act(visible, Notification.Action.SEMANTIC_ACTION_DELETE, 0, "Delete"), act(visible, Notification.Action.SEMANTIC_ACTION_MUTE, 0, "Block"), call);
        Replies.Choice sms = Replies.choose(message, false, true);
        expect("Message: Reply and Read alone", sms.reply == messageReply && sms.read == messageRead);
        expect("never Call, Block, Delete nor Archive", sms.reply != call && sms.read != call);

        // K-9 Mail: a Reply that opens its writing screen, no input; a Mark as read that says no meaning.
        List<Replies.Act> mail = list(new Replies.Act(visible, NONE, false, 0, false, true, true, false, "Reply"), act(visible, NONE, 0, "Mark as read"));
        Replies.Choice k9 = Replies.choose(mail, false, false);
        expect("K-9: nothing to fill, nothing to mark", k9.reply == null && k9.read == null);
        expect("none said so", "none".equals(Replies.refusal(k9.reply, true, false)) && "none".equals(Replies.refusal(k9.read, false, false)));

        // What may not go, and why.
        expect("a Reply that cannot be filled", "closed".equals(Replies.refusal(fixed(visible, REPLY, "Reply"), true, false)));
        expect("one that opens the app", "activity".equals(Replies.refusal(new Replies.Act(visible, REPLY, false, 1, true, false, true, false, "Reply"), true, false)));
        expect("a Mark as read that opens the app", "activity".equals(Replies.refusal(new Replies.Act(visible, READ, false, 0, false, true, true, false, "Read"), false, false)));
        Replies.Act guarded = new Replies.Act(visible, REPLY, false, 1, true, false, false, true, "Reply");
        expect("an unlocked phone asked, and locked", "locked".equals(Replies.refusal(guarded, true, true)));
        expect("…and unlocked", Replies.refusal(guarded, true, false).isEmpty());

        // Found by meaning, never by title or place: swapped titles, another order.
        Replies.Act likeTitled = act(visible, REPLY, 1, "Like");
        Replies.Act replyTitled = act(visible, Notification.Action.SEMANTIC_ACTION_THUMBS_UP, 0, "Reply");
        List<Replies.Act> swapped = list(replyTitled, likeTitled);
        expect("swapped titles: the one that means Reply", Replies.choose(swapped, true, false).reply == likeTitled);
        Collections.reverse(swapped);
        expect("another order: the same", Replies.choose(swapped, true, false).reply == likeTitled);
        Replies.Act system = new Replies.Act(visible, REPLY, true, 1, true, false, false, false, "Reply");
        expect("one the system added: never", Replies.choose(list(system), true, false).reply == null);

        // The car extender's invisible Mark as read, a Reply on the watch only.
        Replies.Act carRead = act(Replies.CAR, READ, 0, "");
        Replies.Choice car = Replies.choose(list(act(visible, REPLY, 1, "Reply"), carRead), true, false);
        expect("the car extender's Mark as read", car.read == carRead);
        Replies.Act watchReply = act(Replies.WATCH, REPLY, 1, "Reply");
        expect("a Reply on the watch only", Replies.choose(list(act(visible, READ, 0, "Read"), watchReply), true, false).reply == watchReply);

        // The live notification by its name: Rust's `replies::notice` of the same app and key; this profile's only; gone.
        String notice = AppNotes.talkKey("org.thoughtcrime.securesms", "0|org.thoughtcrime.securesms|42|null|10123");
        expect("the name Rust makes", "a2b7df14a4081c04".equals(notice));
        String[] packages = { "org.telegram.messenger", "org.thoughtcrime.securesms", "org.thoughtcrime.securesms" };
        String[] keys = { "0|org.telegram.messenger|42|null|10123", "0|org.thoughtcrime.securesms|42|null|10123", "0|org.thoughtcrime.securesms|42|null|10123" };
        expect("found by its name", Replies.live(packages, keys, new boolean[] { true, true, true }, notice) == 1);
        expect("never a work profile's", Replies.live(packages, keys, new boolean[] { true, false, true }, notice) == 2);
        expect("gone", Replies.live(packages, keys, new boolean[] { true, false, false }, notice) == -1);
        expect("none listed", Replies.live(new String[0], new String[0], new boolean[0], notice) == -1);

        System.out.println("RepliesCheck: " + checked + " checked, " + failed + " failed");
        if (failed > 0)
            System.exit(1);
    }
}
