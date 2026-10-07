---
description: The Porch, where new mail waits in Sioul - checked, sorted into lanes, and shown in your hours; the codes and links you just asked for, at once.
---

# The Porch

The Porch is where new mail waits until you look: from every address, checked (genuine or forged) and sorted into lanes. It takes the place of the inbox that shouts.

<figure markdown="span">
  [![The Porch in working hours. At the top, "Open until 17:00." and the Real time box; this week's payments in one line; two cards for a confirmation link and a code from verified senders, the code with a Copy button and how long it stays valid; a chat's news; then two lanes, a project's and "From people you know", each with its messages.](../assets/screens/porch.png){ loading=lazy }](../assets/screens/porch.png "Open the picture at full size")
  <figcaption>Codes and links on top, the sites' news, then the lanes.</figcaption>
</figure>

## When it opens

- **In your hours**, the Porch says until when it is open ("Open until 17:00."), then shows what came, sorted into lanes.
- **Outside your hours**, the Porch says when it opens next, and nothing else of your mail: no counts, no names. Your doses of the day still show (below). **Open it anyway** stays possible, quietly. Meanwhile, what comes is checked and sorted.
- **Without any hours set**, the Porch is always open.

Which mail comes when follows who wrote, at the times you tick for each of the five states (safe, neutral, restricted, strangers; the blocked never), and what each address is for: work, your admin, leisure. See [Who may reach you](accounts.md#senders) and [Hours](hours.md).

Mail keeps arriving in the background all the while. The Porch only decides when it is shown, and when it is told ([below](#new-mail-told-at-its-times)).

## Codes and links come at once

One-time codes, temporary passwords, password resets, sign-in links and links to confirm an address come from something you just asked a site for, and they expire. So Sioul shows them at once, whatever the hour, even when they come from a site's automatic address (no-reply…):

- one desktop notification, without sound, with the code, a button to copy it (on Linux), and how long it stays valid; none while you sleep: the card waits here;
- the same card on top of the Porch, with **Copy**.

Nothing else opens with it. Once it has expired, it is hidden, and the message goes to its lane. It expires when the message says, else when its kind usually does: a code after 30 minutes, a sign-in link after an hour, a password reset after two hours, a link to confirm an address after a day, a temporary password after a week.

Many sites send their codes and links through the same services as their newsletters, with the same headers. They still come at once. Sioul looks for the code in their subject and at the top of their text, where a site puts what you asked for: a newsletter's article about passwords stays a newsletter.

Fake "your code" messages are a common phishing trick. A forged one is set aside, even when it carries a newsletter's headers. One from a sender that is only *not verified* still comes, with a warning: use it only if you just asked that site for it.

## New mail, told at its times

When mail your lists let through now arrives, one quiet notification says it, for the whole batch: "Two letters", and the first senders with their subjects: "Murena, Your invoice · Alice, Dinner on Friday". **Open** shows the Porch.

- **What waited** (mail that came outside its list's times, while you slept or paused) is told once, when its time comes: "The Porch opens: three letters wait for you."
- **Never** for codes (they have their own, above), mail set aside, blocked senders, your less important addresses, what you send yourself, newsletters unless you include them, or mail you already read elsewhere. Nothing while you sleep or pause.
- **No sound.** With a phone and a computer, only the one you used last tells.

Settings ▸ Reminders and notifications ▸ **New mail: notify at the times it may come**, and **Include newsletters** ([Settings](settings.md#reminders-and-notifications)).

## The lanes

Each message goes to the first lane that takes it, in this order:

| Lane | What it holds |
|---|---|
| **Set aside** | Forged mail, mail borrowing a name, spam (your provider's word, or your own filter's when you let it: [below](#spam-and-your-own-filter)), and senders you blocked. Shown at the bottom, each with the reason. Nothing is deleted. Spam has **Not spam**: back in its lane for good, on every device. |
| **Hostile, set aside** | Only for an address you protect against harassment (below): insults, harassment, threats. Their words stay hidden. |
| **Right now** | Codes and links you just asked a site for, above. |
| **One lane per project** | Mail that matches the project's routes, or that belongs to a conversation of the project. See [Projects](projects.md). |
| **Your public address** | Mail to an address you protect, from someone you have not let in, read first and sorted by topic: work first. |
| **Less important accounts** | Mail of the addresses you ranked below the others: social networks, notifications you read now and then. Folded at the bottom. |
| **Filed: newsletters and notifications** | Mailing lists, newsletters and automatic senders (no-reply…). Folded. |
| **Someone new, in the screener** | A stranger: in none of your address books and on no list. You let them in, or not. |
| **From people you know** | People in your address books, those you let in, and those on a list by their own address. |

Mail you send yourself, from one of your addresses to another (a file sent from your phone), comes with the people you know, at any hour, never screened, when it is verified: a forged own address is a classic trick.

Under each lane's title, one line says what it holds. Its **?** (How mail lands here) says, sentence by sentence, how mail lands there, and where to change it.

### Letting someone in

A message in the screener has **Let this address in**: their next messages go to "From people you know".

Every message also has **Their mail**, in its menu (⋮). First, one line says what decides for this sender now: "Safe, as the category Friends says." Then the choices: **As their categories say** (their own entry taken out of the lists: the categories on their contact card decide, else their address's domain), or one of the four lists, each with its mail's times: **Safe**, **Neutral**, **Restricted**, **Blocked** (set aside for good, never shown). A sender in none of your address books and on no list is a stranger, with times of their own. The same lists, with patterns such as `*@example.org` and numbers, and when each comes, are in [Accounts ▸ Who may reach you](accounts.md#senders).

Forged mail is judged apart: a forged message is set aside whatever the lists say, even if it claims to come from someone you marked safe, and it is weighed as a stranger's.

## Spam, and your own filter

Only a stranger's mail is ever called spam. Whoever says it, your provider or Sioul's own filter, it never touches mail from someone you know (in your address books, on a list, let in), the codes and links you asked for, a project's mail, what you send yourself, or a message you said is not spam. Forged mail, borrowed names and blocked senders are set aside before, as always. A message nothing authenticates (it failed both SPF and DKIM) counts as a stranger's, whatever address it shows.

- **Your provider's word**: a stranger's message your provider marks as spam goes to Set aside.
- **Your own filter**, once it has learned from your mail ([Settings](settings.md#your-own-spam-filter)), says how likely a stranger's message is spam:
    - **probably spam**, from 95% unless you change it: set aside, if you let it (**Set spam aside**); as it starts (**Show its verdict only**), the message stays in its lane, with "probably spam" beside its subject;
    - **maybe spam**, from 50%: the message stays in its lane, with "maybe spam" beside its subject;
    - below: nothing.

<figure markdown="span">
  [![The Porch with a message open. On the left, in the screener, two strangers' messages, one with "probably spam" beside its subject, the other with "maybe spam"; the first is open on the right, "Why it is here" unfolded: "your own filter: probably spam (97%) — a price, a link; replies go to another domain, its name shows another domain, links to other domains"; under the message, Let this address in, Not spam and Close.](../assets/screens/porch-spam.png){ loading=lazy }](../assets/screens/porch-spam.png "Open the picture at full size")
  <figcaption>The filter's quiet word beside two strangers' messages; why, once one is open.</figcaption>
</figure>

The words are quiet: no colour, no count. With the pointer on them, Sioul says how sure the filter is and why: the words, and the signs in the message's headers, that weighed most ("a price, a link; replies go to another domain"). **Why it is here**, in the message, says the same. The words are shown as the filter reads them, shortened: "loteri" for lottery.

**Not spam**, under the message, answers each of them: it goes back to its lane, or keeps it without the word, for good and on every device, and the filter learns from it at its next training. **Junk** moves a message to the junk folder, as always, and teaches the filter too. Nothing is deleted, and the filter moves nothing on the server.

Mail set aside is never notified. A message that keeps its lane, with a word beside it, is notified as its lane says.

## Reading a message

A message opens on the right. Before its text:

- **who sent it**, and how far that is verified: *verified*, *not verified*, or *forged*. Hovering over the shield shows each check (SPF, DKIM, DMARC, ARC, reverse DNS), its result, and who made it;
- **to whom**, copied to whom, when, the subject;
- **why it is here**, folded until you ask.

Then the text, made safe:

- HTML mail keeps its paragraphs, lists, bold text and links. Nothing else is shown: no images, no styles, no scripts. Nothing loads from the network, nothing runs.
- Earlier messages that a reply quotes are folded under **Show the earlier messages**. A signature is dimmed.
- Every link shows its full address under the message while the pointer is on it, before you click.
- **Attachments** are folded, one line each with their kind, name and size. Opening or saving one runs the antivirus first; if the device has none, Sioul says so and asks before opening. See [Privacy and security](privacy-security.md#attachments-and-the-antivirus).

From the message: reply, forward, archive, delete, and the rest, as on the [Mail](mail.md) page. A newsletter has **Unsubscribe** too ([Mail](mail.md#unsubscribing)).

## Done for now

**Done for now** closes the Porch on what it showed. Anything newer waits for the next time it opens, even mail that reached the server earlier and arrived here later.

Closing the Porch tells nothing to the server: no message is marked read by it. Only opening a message marks it read, as in any mail program.

An address the Porch was never closed on shows its last two weeks only. Older mail stays in [Mail](mail.md), where it belongs.

## Real time

The **Real time** box, at the top of the Porch, fetches every folder of every address each minute, and keeps the Porch open: for a code or a password you are waiting for. Untick it to go back to the usual pace.

In quiet time, **Work now** appears beside it, to show work whatever the hours. See [Hours](hours.md#work-now).

## Above the lanes

When there is something, a few lines come before the lanes:

- **When your hours are not set**, a card asks for them, with **Set my hours** (which opens Settings at them) and **Leave as is** (which stops asking).
- **When your night is not set**, a card says that nothing keeps notifications away while you sleep, with **Set my night** (which opens the Health page where meals and the night are set) and **Leave as is**. See [Hours](hours.md#sleep).
- **Where you stopped**: the line you left when something interrupted you, with its task, until you press **Done**. See [Tasks](tasks.md#starting-and-stopping).
- **Two events at once today**, the time to get there and back counted, with **Open "…"** for each and **Don't mention it again**. See [Agenda](agenda.md#two-events-at-once).
- **Today's doses not marked yet**, from their time on, whether a notification reminded you or not: each with its time, its name and **Taken** (more than half an hour late, **Taken…** asks when you took it). Whatever your hours: a dose is not mail. Each stays until you mark it, the day ends or twelve hours have passed; while you sleep with doses kept silent, they wait for your waking. When another device may know more, the doubt is said under the dose: check before taking it. See [Health](health.md#reminders).
- **Doses due while Sioul was closed**, neither marked nor reminded anywhere: **Taken…** (when you took it) or **Not taken**. When another device may know more, the doubt is said under the dose. See [Health](health.md#reminders).
- **Calls Sioul declined**, on a phone that screens calls, each at a time its caller may reach you: "While you slept: a number not in your contacts called at 09:30.", with **Text back**, **Call back**, **Listen** when Free mailed the voicemail. See [Calls](calls.md#afterwards-on-the-porch).
- **"*The site* has news"**: what the websites you keep in Sioul notified, waiting for you. Opening the site clears its news. See [Sites](sites.md).
- **Paper letters** you scanned, each as a card: who, what, how much, by when. See [Papers and letters](papers.md#paper-letters).
- **This week's payments**, in one line: "This week: Electricity €62 (Mon). The account holds them." When something about money needs a look, it says so, without a count. See [Budgets](budgets.md#the-bank-watch).

<figure markdown="span">
  [![A card at the top of the Porch: "Your hours are not set: work and your admin come at any time.", a sentence on what each kind of hours brings, and two buttons, "Set my hours" and "Leave as is".](../assets/screens/porch-hours.png){ loading=lazy }](../assets/screens/porch-hours.png "Open the picture at full size")
  <figcaption>Until your hours are set, the Porch asks once.</figcaption>
</figure>

## A public address, protected

An address you publish brings work, and sometimes insults. On its card in [Accounts](accounts.md#a-mail-address), **Protected against harassment** has its mail read before you see it:

- **its tone**: calm, rude, or hostile (insults aimed at you, harassment, threats), from word lists in French and English;
- **its topic**: work, a question, the press, thanks, a donation, something else.

Hostile mail goes to its own folded lane, which shows neither the sender's name nor the subject. Opening one asks first: it can wait, go to someone you trust (**Forward to someone you trust**), or go away (blocked, deleted). Reading it anyway is your choice, at a time that suits you. The rest of the address's mail has its own lane, work first, its topic shown, and a mark when it is rude.

**Let the AI read it first**, below it, is off unless you turn it on: then each new message to that address is sent once to Anthropic's Claude, with its subject, to say its tone and topic more finely than word lists can. The text leaves your device for that. The key for Anthropic's service is typed once in Accounts and kept in your keyring.

## The Porch's settings

The ⚙ at the top of the Porch holds what is the Porch's alone, and how a message opened here reads:

- **Projects shown here**: which projects have a lane on the Porch. The others' mail stays on their page in Projects.
- **Paper letters ▸ Where scans arrive**: the folder your scans come to.
- **How text reads**: the font, its size and the space between lines of a message opened here, the same as in the Mail page's ⚙.
- **How mail is sorted**: every lane, in the order mail is sorted, with its rules; the senders you know; the words that make a sender automatic (no-reply…).

What belongs to something else is set where that thing is: an address's rank and protection on its card in [Accounts](accounts.md), a project's routes on its page in [Projects](projects.md), who may write to you when in [Accounts ▸ Senders](accounts.md#senders), your hours in [Settings](settings.md#hours).

### Some addresses first, others last

On each address's card in Accounts, **Priority**: *More important*, *Normal* or *Less important*. Mail of the more important addresses comes first in every lane, with a small mark. Mail of the less important ones skips the screener and waits folded at the bottom. Their codes still come at once.

## Why it works this way

- Checking mail three times a day lowered daily stress in a randomised trial (Kushlev & Dunn 2015). Batching notifications helped attention and mood, while having none at all made people more anxious (Fitz et al. 2019): what helps is predictability, not silence. So the Porch opens in hours you choose, and says when.
- Interrupted people work faster, with more stress and frustration (Mark, Gudith & Klocke 2008), and a notification left unanswered still costs attention (Stothart, Mitchum & Yehnert 2015). So nothing pops up outside the times you choose, one notification says a whole batch, and nothing moves under your eyes when mail arrives.
- People avoid information they expect to hurt (Sweeny et al. 2010). So what a message is, who sent it and how far that is verified come before its text.

More in [what the research says](../dev/research.md).
