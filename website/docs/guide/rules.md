---
description: Rules in Sioul - "when this happens, do that", one list in Settings ▸ Rules; a rule made from a message in two presses, what it would have done in the last 30 days shown before it is saved; each act said once with Undo; what a rule may never do.
---

# Rules

## In short {#in-short}

A rule says what Sioul does when something happens, in one sentence: "When a message arrives from impots.example with “avis” in its subject: tie it to the project Taxes 2025." **Settings ▸ Rules** holds them all in one list, grouped under what happens ("When a message arrives"); the rules that share the same "when" are shown together under it, each by what it does. Each of your rules has its switch, **Edit**, **✕**, and what it did lately. **Add a rule…** starts one from nothing or from a message.

A rule acts on what comes after it is saved, never on what was there before. Each thing it does is said once in the status line, with **Undo**.

## Protected by default {#what-is-protected}

- **A rule never sends** a message, a text or a reply, **never moves money** (a payment it counts is a record of yours, not a payment), **never deletes** anything, never changes a meeting that has other guests, nor a list or a calendar you may only read, never acts on health, and never changes a setting or another rule. The editor does not offer these; a rule written by hand that asks for one is refused, and says so.
- **A rule about mail sees only what the Porch lets in**: never what it set aside as dangerous, nor what your spam filter holds. A condition on a message's sender holds only for a sender something proves (its server's signature, your own address book): anyone can write "From: your bank" in a message.
- **Once, on every device**: what a rule makes carries a mark worked out from the rule and the message, so that your phone and your computer, both seeing the same message, make it once.
- **Nothing to review**: a rule acts, or it is off. What it did is undone in one press, and **Not for this one** keeps it away from that thing for good.
- **No loop**: a rule never acts on what it made itself, and a chain of rules (one acting on what another made) stops at three steps.

## The list {#the-list}

Each line is a sentence. Your own rules have, from left to right, their switch (off, a rule stays in the list and does nothing), the sentence, **Edit** and **✕**; on a phone, **Edit** and **Remove** are under the sentence. **✕** removes the rule at once, with **Undo** in the status line for a moment. Under a rule that has acted, **What it did lately** unfolds its last five acts, each with when it was, **Undo** (put back as it was, as long as nothing changed it since), and **Not for this one**.

The rules that already exist elsewhere in Sioul are in the same list, said the same way, so that everything that happens by itself is in one place:

- **a project's routes**, and those that make a task of each message, kept on the project's page;
- **a budget's mail rules**, the mail that becomes a line of a budget;
- **Mail's filters**, kept in Mail ⚙ ▸ Filters, where their order is kept too;
- **a topic's senders**, "Always this topic for this sender", kept in Mail ⚙ ▸ Topics.

Each says where it is kept, and its link opens it there; it is changed there, as before, and runs as before. Nothing of it is copied into the list.

## Making a rule {#making-a-rule}

**Add a rule…** offers its starts: **Start from nothing**, or **From a message**, one of the last 30 days found by a few words of its subject or sender (mail the Porch set aside is not among them). Starting from a preset, one of the rules that come with Sioul, comes in a later version, and the card says so. The editor then opens, one card with two parts:

- **When**: what is watched and what happens to it (in this version, a message that arrives), then its conditions. Each condition is what it reads (its sender's domain, its subject, what Sioul recognises in it, its project…), how it compares (is, contains, has the words…) and a value, whose field follows what it is: words separated by commas, a domain, a project, a topic, a kind of attachment, days, a time. With two conditions or more, **Every condition holds** or **One of them is enough**.
- **Then**: what it does. In this version: tie the message to a project, make a task from a model, give it a topic, tell you, and keep its attachments in Papers. Each detail is either chosen here or taken from the message ("its subject", "its project"). What comes in a later version is in the lists, greyed, with "(later)". Keeping attachments in Papers asks which (its PDFs come first), the paper's type, its name, where `{sender}`, `{month}`, `{year}`, `{subject}` or `{file}` stand for the message's own, and whom it is for; each file is kept once, only once the antivirus has found it clean, and tied to its message.

Above the two parts, the rule reads as one sentence as you change it, the same sentence the list and the status line will say. Under them, **What it would have done in the last 30 days**: how many messages, the first few by their day, sender and subject, and what it would have done with each. **Also for these messages, once each** is unticked: tick it, and saving does it for them too. **Save** keeps the rule; it acts from then on.

## From a message {#from-a-message}

On any message, in the reader's ⋮ menu or the Porch's row menu: **Make a rule from this…**. The editor opens with its "when" filled in from that message: its sender's domain (only when something proves the sender), what Sioul recognises in it (a code, a bill, an order…), its project if it has one. Choose what it does, look at what it would have done, and save. Mail the Porch set aside has no such entry. **Make a task of mail like this…**, **Make it a filter…** and **Always this topic for this sender** stay where they are.

## Presets {#presets}

The second tab, **Presets**, holds the rules that come with Sioul: what it does by itself, said the same way, grouped by what happens. Each is a line of words, with its switch where a setting turns it on or off today; **Details** shows what else can be changed (the settings it has today, changed there and nowhere else). Each is still done the way Sioul always did it, and its card says so: its details become yours to change, and a preset can be copied to make your own, in a later version.

## Why it works this way {#why-it-works-this-way}

- **One list instead of six places.** Before rules, "when such mail comes, do that" was said in six places, each with its own way of saying "which mail" (a project's routes, a budget's mail rules, Mail's filters, a topic's senders, the routes that make tasks, the money read from mail). One list, one sentence for each, shows the whole truth of what happens by itself; the places that kept them still keep them until every one of your devices knows rules.
- **One sentence per rule.** A rule you can read aloud is a rule you can check. The same words in the list, the editor and the status line mean nothing is said two ways.
- **One action per rule.** One thing happens, one thing is done, one Undo. Rules that share a "when" are shown together, so that "When a message from your phone company brings a bill" reads once above everything it does.
- **From now on, with the past shown first.** A rule that acted on years of mail at once would surprise you; showing what it would have done in the last 30 days, and leaving the choice unticked, keeps the surprise on your side.
- **Made to converge, not to fire.** A rule is checked as a state to reach (for each message that matches, its result exists), so that a device that was asleep catches up, and two devices never do it twice.
- **What Sioul decides counts until you correct it.** No queue of suggestions to approve: a rule acts, and its Undo and **Not for this one** are always one press away.
