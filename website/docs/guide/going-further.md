---
description: Going further with Sioul - your own spam filter and your mail filters, topics and the other labels, tasks that wait for each other with their steps, rides and views, and the people you do admin for.
---

# Going further {#going-further}

Four parts of Sioul for once its base is in place ([Getting started](getting-started.md)): mail that can wait, the labels Sioul sorts things with, tasks that wait for each other, and the admin you do for others. Each part stands alone: read the one you need.

None of them has to be set up, and none asks for upkeep. What Sioul guesses counts until you correct it, and nothing asks you to review its work.

## Mail that can wait {#mail-that-can-wait}

Three tools decide what your mail does before you see it. They answer different questions.

| Tool | The question it answers | Where it is |
|---|---|---|
| Your lists | Who is this, and may they reach me now? | **Contacts**, and **Settings ▸ What reaches you** |
| Your own spam filter | Can this stranger's message wait? | the Mail page's ⚙, **Your own spam filter** |
| Filters | What should always happen to mail like this? | the Mail page's ⚙, **Filters** |

### Your own spam filter {#your-own-spam-filter}

Most spam filters keep an inbox tidy. Sioul's decides what can wait. It learns from your own mail, on each of your devices: from your junk folders, and from every message you mark as spam or not spam. Sioul sends your mail to no filtering service.

**It judges strangers' mail only.** It never touches mail from someone you know, a code or a link you asked for, a project's mail, or what you send yourself.

**Train it once.** In the Mail page's ⚙, under **Your own spam filter**, **Train now** names the addresses it learns from, downloads what training needs, then learns. The first time takes long, and Sioul stays usable meanwhile. After that it trains by itself, once a week, while the device charges and nobody is at it (**Train here, once a week, while this device charges and is idle**): a computer while Sioul runs on it, a phone or a tablet while it shares with your other devices and keeps in step in the background. Each device judges the mail it fetches itself, and gives its table only to a device of yours that does not learn.

**Choose what it does.** It finds a stranger's message **probably spam** (from 95%), **maybe spam** (from 50%) or **probably not spam**. For each verdict, you choose one of three:

- **Move to spam**: the message goes into its address's Junk folder, on the server, where your other mail apps see it too;
- **Flag only**: it stays where it is, marked;
- **Do nothing**.

Until you choose, it flags what is probably or maybe spam, and moves nothing. Two sliders, **Spam from** and **Maybe spam from**, set how sure it must be.

**Look when you have time.** What it flagged or moved waits on the Porch, in **Caught by your own spam filter**: folded, without a count, never notified. Nothing there asks for your time. You step in only when it is wrong: **Not spam** puts a message back for good and teaches the filter, and **Spam** confirms it. What you say on one device, the others know.

<figure markdown="span">
  [![The Porch. On the left, "Caught by your own spam filter" opened, without a count, with its sentence under the title: what your own filter caught waits there, never notified; Sioul learns from it as it is, and "Not spam" puts one back and teaches the filter. Then strangers' messages, each with its quiet word, "probably spam" or "maybe spam", and Not spam and Spam under it; above them, Not spam for all and Spam for all. The first, which the filter moved into the Junk folder, is open on the right: "Why it is here" says "your own filter: probably spam" and that it was moved into your Junk folder, "Not spam" bringing it back to the inbox; under the message, Spam, Not spam and Close.](../assets/screens/porch-spam.png){ loading=lazy }](../assets/screens/porch-spam.png "Open the picture at full size")
  <figcaption>What your own filter flagged or moved.</figcaption>
</figure>

More: [Spam, and your own filter](porch.md#spam-and-your-own-filter) · [Its settings](mail.md#your-own-spam-filter)

### Filters {#filters}

A filter is a rule you write for mail as it arrives: **If** this, **Then** that. In the Mail page's ⚙, under **Filters**, **Add a filter** opens one in place.

- **A condition** reads the sender, the recipients, the subject, the text, an attachment, who the sender is to you, whether it is a newsletter, the day or the time it arrived, or its size. With several conditions, you choose whether **every condition holds** or **one of them holds**.
- **An action** moves the message to a folder, archives it, marks it as spam, flags it, marks it as read, moves it to the trash, or adds a keyword. Filters never forward or answer mail.
- **A sentence** says the filter back as you build it, and **Try it on the inboxes** counts what it would take now.

Filters act on the server, once, on whichever of your devices fetches the message first, and they travel to your other devices with your settings. They never touch a code you asked for, mail the Porch set aside, or what your spam filter caught. **Run them on the inboxes…** applies them to what is there already, after saying what would change.

A search can become a filter: **Make it a filter…**, beside the search's Clear.

More: [Filters](mail.md#filters)

## Labels: how Sioul sorts things {#labels}

Sioul has no single "tag" for everything. It uses a few kinds of label, and each answers one question. Knowing which is which explains where a thing shows, and when.

| Label | What carries it | The question it answers | What it changes |
|---|---|---|---|
| **For**: work, your admin, leisure | a mail address, an agenda, a budget, a site, a task | Is now the time for this? | when it comes forward: work in working hours, admin in admin hours |
| **List**: safe, your contacts, restricted, blocked | a person | May they reach me now? | when their mail, calls and messages reach you |
| **Topic** | a message, or a sender | Which part of life is this about? | nothing moves: a word on the row, and a filter |
| **Category** | a contact | Which group is this person in? | a filter; a list can follow it |
| **Tag** and **Kind** | a task | What is it about, and what sort of act is it? | a filter ("the next call"); what the task is for can follow its tags |
| **Project** | mail, tasks, events, notes, time | Which matter, with an end, is this part of? | everything about it gathers on one page |

### Topics on your mail {#topics}

A topic is a quiet word on a message, such as Health, Tax or Choir. It is a way to look at your mail, never a folder: nothing moves, a conversation stays whole, and a wrong word costs nothing. A message has one topic at most.

- **To start**, Sioul knows some topics (Bank, Taxes, Health, Phone and internet…) by the senders it can prove. Those that find five messages or more start by themselves, said once on the Porch; the others wait in the Mail page's ⚙, under **Topics**.
- **To give one yourself**: **Topic…**, under the sender's line in the reader, in a message's menu, or above a selection. **New topic…** makes one and gives it in one act.
- **To teach a sender**: **Always <topic> for this sender**, or **Never…**, in the same menu.
- **To look by topic**: the Topic filter, beside the filter by area, shows one topic's mail alone.

Behind this, a small model learns your topics on each device, once a week, with the spam filter. It learns a topic once that topic has 20 messages from 3 senders, or 50 from a single one, such as an office; below that, your words and Sioul's rules sort it. It is made not to move things under your eyes: a new version replaces the one in use only when it does no worse on your newest mail and changes fewer than 2% of the topics already shown, and a doubt shows only under its topic's filter, as "Perhaps". On a computer, **Groups Sioul noticed** gathers mail that no topic names yet, and **Name it** gives the whole group a topic.

More: [Topics](mail.md#topics)

### Tags, kinds and projects on tasks {#tags-kinds-and-projects}

- **In one line**: `#taxes` gives a project or a tag; `@call`, `@write`, `@online`, `@out`, `@read`, `@think` or `@make` gives the kind. A first word that says it plainly gives the kind too: "Call…" is a call.
- **Three choices at the top of the Tasks page**, **Every project**, **All kinds** and **Any category**, keep every view to one of them. The plan stays whole: a call that waits for a message still waits.
- **What a task is for** follows its tags and its project. In the Tasks ⚙, **What is work** and **What is yours** name the categories that say so. A work task then waits for your working hours on **Now**.

A task's tags are its categories: they are written into the task itself, on your calendar server, where your other apps read them as categories.

### Categories on contacts {#categories-on-contacts}

Categories are the groups your address book has already: "Family", "Friends", "Neighbours". They are saved in the card itself, so your phone and your other programs see them. A list can name one (**Settings ▸ What reaches you**, under **Who is in which circle**, **Groups**): its people then reach you as that list says. Nothing goes on a list by itself, family and friends included. In an address book that someone else can write, groups decide nothing until you turn them on for that book, in the Contacts ⚙ under **Address books**.

### A project, or a topic? {#a-project-or-a-topic}

A project is something to complete: a tax return, a lease, a client's work. Its mail comes to it by itself, and it lays its tasks, events and notes on one line of time. A topic is a standing part of life. The two are independent.

More: [Projects](projects.md) · [Hours](hours.md#what-each-thing-is-for)

## Tasks that wait for each other {#tasks-that-wait-for-each-other}

A plan written as a list is a wall. Sioul asks for what the list leaves out: what comes before what. From that it finds the one next step, and it can show the same plan five ways.

### Steps {#steps}

A bigger task holds its steps, in the order you give them. Open the task, choose **Edit**, and type a step in "A step, in one line". Drag a step by its handle ⠿ to reorder, or use **Move up** and **Move down** in its menu.

The first step you can do now is the next step. The order never traps you: you can tick a later step first, and nothing says it was out of order.

A step's menu also offers **Break this step down**, to give it steps of its own, and **Make it a task of its own**. A task's menu offers **Make it a step of…**.

### Waits {#waits}

A wait says that one task cannot start before another is done. In a task's form, **Waits for…** finds the other task by its title. **Then wait** adds a gap after it, in days or weeks: "the answer comes within two weeks".

A task that waits is never the next step. When what it waited for is done, the status line says so: "Done. This frees: Send the registered letter." Tasks that wait for each other in a loop are said calmly, by name.

### An appointment and what leads to it {#an-appointment-and-its-steps}

A task can prepare an appointment. In the Agenda, an event's card offers **Add steps and a list**; in a task's form, **Its appointment…** finds the event by its title.

- **Its date follows the appointment**: the task's date asked becomes the moment the appointment starts, and moves with it.
- **On the day**, its steps are laid one after the other, so that the last ends when you leave. The appointment's card lists them under **Before**, and what to bring under **Bring**.
- **A step can come After the appointment**: it is laid once you are back.
- **A single task** can also come before one time of an event: the event's card lists it under **To do before**.

### Getting there and back {#getting-there-and-back}

**Before** and **After**, on an event or on a task, keep the time to get ready, get there and come back, from 5 minutes to 2 hours each. The plan keeps that time free, and never counts it as a pause.

When the way there has parts of its own, a bus, then the walk from the stop, add them as legs: **Add a ride or a walk…**, in the appointment's card, with **From a timetable** when its time comes from one. Its form says which way it goes: **To get there** puts it before the appointment, **To come back** after it.

- **A leg is an event of its own**, in the appointment's calendar, so your phone's calendar shows it too.
- **It moves with the appointment.** Change the appointment's time, and its legs move by as much, under the same **Undo**. A bus moved that way is said "timetable not checked" until you set its time yourself.
- **"Leave at"** is then the first leg's start. One reminder comes for the whole outing: "Leave in 15 minutes for Doctor (14:00)", then the bus, and what to bring.
- **The card** says **Getting there**, the legs before it, and **Coming back**, the legs after it, then the steps that follow.

An appointment that repeats, or that lasts the whole day, takes no leg.

### Pinned to a time {#pinned-to-a-time}

The plan places your tasks by itself, and writes no time into them. When a step must happen at a set time, **Do at…**, in its details, pins it: a block goes into your calendar, and the plan lays everything else around it. Dragging a step in **The day** does the same.

**Let the plan place it** takes the block away. When the block passes and the task is not done, the plan places it again: nothing is said, and nothing is counted.

### Five views of one plan {#five-views-of-one-plan}

| View | What it shows | What it is good for |
|---|---|---|
| **Now** | one step, why it comes now, and the one after it | doing |
| **The day** | today's hours, with your events, your meals, and the steps placed in them | seeing whether today holds |
| **List** | every open task in the plan's order, **By project** or **By list**; a task with steps is one row that unfolds | finding and changing |
| **Board** | four columns, as on a Kanban board: *Free to start*, *Started*, *Waiting*, *Done* | seeing what is blocked, and by what |
| **Gantt** | each open task on its days, the date asked as a small diamond, the days without room shaded | seeing whether a date will hold |

<figure markdown="span">
  [![The List: a search field, "By project" and "Done too" at the top; tasks grouped by project, a bigger task followed by its steps, each with how long it takes, the date asked as days left and what it waits for; tasks without a project below, and those "when you say so" last.](../assets/screens/tasks-list.png){ loading=lazy }](../assets/screens/tasks-list.png "Open the picture at full size")
  <figcaption>The list, by project.</figcaption>
</figure>

On the Board, cards move by dragging. "Waiting" is decided by what each task waits for, and each card says what. Past three started tasks, one line asks: "Finish or park one?"

A project's **Board**, **List** and **Gantt** buttons open the same views, kept to that project. Your hours shape **Now** alone: List, Board and Gantt show every task, whatever the hour.

### What your other apps see {#what-your-other-apps-see}

Steps, waits and kinds are written into your tasks themselves, on your calendar server. Steps are written as the standard says: task apps that know steps show yours in your order (Nextcloud Tasks, Tasks.org). Waits and kinds are written in lines of Sioul's own, with the standard's words for them: other apps do not show them, and keep them as long as they save a task without dropping what they do not know. [Works with](compatibility.md#other-apps-on-the-same-accounts) says which app keeps what. A list on Google Tasks keeps less, and Sioul greys what Google would not keep.

More: [Tasks](tasks.md) · [Time around an event](agenda.md#time-around-an-event)

## Your people: admin done for others {#your-people}

Admin is done for others too: a child, a parent you help, a partner. Sioul gives each of them a page, and gives each of a child's events one adult who takes it.

### A person is not a contact {#a-person-is-not-a-contact}

A contact is a card for reaching someone. A person is who your papers, your agendas and your budgets are for. A baby is a person with no card; someone with a home card and a work card is one person with two.

**Your people** is a row of names at the head of the **Contacts** page: you first, then the people your admin is done for, and, after a small gap, a child's other parent.

- **Add a person**, at the end of the row, adds one.
- **This is one of your people…**, on a contact's card, links that card to one of them.
- **Sioul makes some by itself**, from what your agendas and papers named already: a child's agenda, a paper's holder. The Porch says once who it made, and from what. A wrong one is changed on its page.

**A person's page** says who they are in one line, such as "Child · 12 · lives with you", then shows what is theirs.

| On the page | What it shows | How a thing becomes theirs |
|---|---|---|
| **Papers** | their papers, each with where it stands | **For**, in a paper's form |
| **Agendas** | their agendas; for a child, with **Who does what** | **Whose it is**, in the Agenda ⚙ |
| **Contacts** | the cards that are them | **Link a card…** |
| **Letters** | the paper letters addressed to them | **For…**, on a letter |
| **Money** | the budgets and the sites you keep for them | **For**, in a budget's form; **Opened for**, in a site's **Name and address…** |

### A child's agenda {#a-childs-agenda}

Give each child an agenda of their own. In the Agenda ⚙, under **Your agendas**, **New agenda…** makes one, or open one you have already, and say **Whose it is**: a child's. On the same line, you say who may take its events, you and the other parent, and how the other parent answers: by an invitation in their mail, or in their own Sioul.

From then on, each event in it has one adult.

- **Who takes it** is said on the event's card: "Sam takes it, since 3 November.", or "Nobody takes it yet." You answer **Me**, the other parent, **Not this time**, or **Ask Sam to take it**.
- **It is asked once.** An event that comes into a child's agenda with nobody to take it is asked about once, under the Agenda's head line. Sioul proposes who usually takes that kind of event, else who has the child that day. **Later** asks once more, at your next admin time, then never.
- **You take it**: it comes into your own agenda, with your time around it, your reminder and your rides. In a work agenda, a health or school event says only "Away".
- **The other parent takes it**: with Sioul, theirs asks them, and the event enters their agenda only if they take it. Without Sioul, an invitation waits in your Drafts until you send it.
- **Tasks** in a child's list, such as a winter coat, work the same way.

**Who does what**, beside each child's agenda in **Agendas**, opens its week: each event and each task, with who takes it. It never counts who did more. Sioul keeps acts ("Sam took it on 3 November"), never a score.

So far, the copy in your own agenda and the invitations to the other parent were tested against stand-ins of the servers only, not against Google, Outlook or Nextcloud themselves.

### Two places at once {#two-places-at-once}

With several agendas, what matters is you in two places. Two events that need you at the same time stand side by side in **Day** and **Week**, and their cards say it in a sentence.

When someone else's change made the conflict, the Porch says it once, at your next admin time, with **Keep both**, a button to move yours, or, for a child's event you take, **Ask Sam to take it**. A child's event that nobody takes yet is nobody's conflict.

### What stays private, and what is handed over {#private-and-handed-over}

- **Your people stay on your own devices.** They are never written into a contact card, so other apps and your contacts' server never see them.
- **Each person is closed to AI agents** until you turn on **Open to AI agents** on their page.
- **What another adult shares of their time stays with you.** It only says "Sam is busy then" when you choose who takes a child's event.
- **At 16 and at 18**, one line on a child's page says what passes to them. **What Sioul keeps about Léa…** makes one PDF of their papers, agendas and letters, with the papers' files, for you to hand to them.
- **Sioul watches nobody.**

More: [Your people](contacts.md#household) · [Children's events](agenda.md#childrens-events) · [Several agendas](agenda.md#several-agendas)
