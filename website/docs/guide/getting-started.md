---
description: Getting started with Sioul - the four ideas it rests on, then installing it, adding your mail, calendars, tasks and contacts, setting your hours, a first look at each page, and bringing a second device in step.
---

# Getting started: the essentials {#getting-started}

This page is the short way in. It gives the four ideas Sioul rests on, sets up its base (mail, calendars, tasks and contacts), looks at each of them once, then brings a second device in step. It keeps to the common case, and each section ends with the page that says the rest.

Nothing has to be done at once. Each step is useful on its own, Sioul works with whatever you give it, and whatever you move, delete or send can be undone for ten seconds.

## Four ideas to know first {#four-ideas}

Sioul does familiar things, mail, an agenda, tasks and contacts, in an unfamiliar order: it starts from what you can give, and fits the rest around it. Four ideas explain almost everything you will see.

1. **You decide what gets through, and when.** You say once which hours are for work and which are for your own admin; every other time is leisure. You also say what each thing is for: this address is for work, that agenda is personal. From then on, a client's mail waits for your working hours, the tax office's for your admin hours, and nobody's work reaches your evening.
2. **Being shown is not being interrupted.** New things do not walk straight in. They wait on the **Porch**, checked and sorted, until you look. Only what you allowed may interrupt you, such as the code you just asked a website for. Sioul shows no unread counts, no badges and no red.
3. **One next step, and nothing is overdue.** Sioul lays your tasks out by itself, around your events, your meals and your rest, and shows the one step to take now, with the reason. A date that has passed is not a debt: the plan starts again from today.
4. **Sioul keeps the links.** A message, the task it asks for, the appointment and the person who wrote belong to one matter. Each of them opens the others, so you do not have to remember where things are.

Your data stays yours throughout. Your mail, calendars and contacts stay on your own providers' servers, everything else stays on your devices, and nothing reaches the developer.

[What Sioul is for, and for whom](../index.md)

<figure markdown="span">
  [![The Porch in working hours. At the top, "Open until 17:00." and its settings, Show all mail now under them; the card to join your other devices; then its sections, each a title with its "?" and its chevron: Right now, a code from a verified sender with Copy; a project's mail; the people you know; From your phone; Sites; Money, each payment with its amount on the right; each row with Mark read and Dismiss beside it.](../assets/screens/porch.png){ loading=lazy }](../assets/screens/porch.png "Open the picture at full size")
  <figcaption>Codes on top, then each section, a row per new thing.</figcaption>
</figure>

## Set it up {#set-it-up}

Four steps. The first two give you a working mailbox: you can stop there and come back another day.

### 1. Install Sioul {#install-sioul}

Download the package for your system from [the releases page](https://github.com/aurelienpierre/sioul/releases/latest).

| System | Package |
|---|---|
| Windows 10 and 11 | `sioul-<version>-setup.exe` |
| macOS 13 and later | `Sioul-<version>-macos-universal.dmg` |
| Linux | `Sioul-<version>-x86_64.AppImage`, or `Sioul-<version>.flatpak` |
| Android 9 and later, phones and tablets | `Sioul-<version>-android-arm64.apk` |

Sioul is not in a store, and its Windows and macOS packages are not signed yet, so each system asks you to confirm the first opening: [Install](install.md) says what to press on each. Sioul is young: it is used every day on Linux and on Android, and little tried elsewhere.

Then open Sioul from your application menu. It speaks English and French, as your system does.

### 2. Add your mail {#add-your-mail}

1. Open **Accounts** (the person icon, at the bottom of the left column), then the **Add an account** tab.
2. Type your **email address** and choose **Find the server**. Sioul says what it found, and where from.
3. Type your **password**. It goes to your system's keyring, and nowhere else.
4. Choose **Connect and add**. Sioul tests the password before it keeps anything, then fetches your recent mail.

Three providers take no ordinary password. For Gmail, Sioul offers an app password, or Google's own sign-in page; for Yahoo Mail, an app password, made on your Yahoo account's security page; for Outlook.com, Hotmail and Microsoft 365, Microsoft's own sign-in page. [First steps](first-steps.md#gmail-and-google-workspace) has all three.

Fetching changes nothing on your mail server. Sioul writes there only when you act: archiving, deleting and moving happen ten seconds after you asked, so that **Undo** can stop them.

Then say what the address is for. On its card in **Your accounts**, unfold **Settings for this address** and tick **What this address is for**: work, your admin, leisure, or several. Until you say, an address counts as work, so that it never reaches your evenings.

### 3. Add your calendars, tasks and contacts {#add-your-calendars-tasks-and-contacts}

They come from the server that already holds them.

- **A CalDAV and CardDAV server** (Nextcloud, Fastmail, iCloud, your host): in **Accounts ▸ Add an account**, under "Add contacts and calendars", type your address and password, then **Connect and add**. Your address books and calendars come; tasks come with the calendars that hold them. When your mail is at the same provider, **What this server offers**, on the mail address's card, adds them in one click.
- **Google**: under "Google calendars, contacts and tasks", type your Google address and choose **Sign in with Google**. You sign in on Google's own page, in your browser.
- **No server**: Sioul can keep calendars, task lists and contacts on this device only. The first time you make a task, **Make the list** makes a list in one click.
- **A calendar read by its web address**, such as a school's or a university's timetable: in the Agenda's ⚙, under **Your agendas**, **Subscribe to a calendar…**. Sioul reads it and never changes it; France's public and school holidays are ready-made there ([Agenda](agenda.md#calendars-by-their-address)).
- **Contacts kept in a file**: on the Contacts page, **Import…**, an icon above the list, reads a vCard or a CSV file into an address book ([Contacts](contacts.md#importing)).

Sioul does not reach Microsoft's calendars and contacts: Microsoft does not offer them through the open standards Sioul speaks.

### 4. Set your hours {#set-your-hours}

In **Settings ▸ Hours**, there are two weeks to fill: **Working hours** and **Hours for your admin**. Each day is on or off, with one range of hours or several. Every other time is leisure, with nothing to set; your usual meals and sleep are set in the same tab, under **Meals, rest and sleep**.

This is the step that makes Sioul calm. Without hours, everything comes at any hour, as in other mail programs. Until they are set, the Porch asks once, with **Set my hours** and **Leave as is**.

### Then leave it open {#then-leave-it-open}

Sioul is made to stay open all day. On a computer, closing its window hides it in the system tray, where it goes on fetching mail and keeping your reminders; **Quit Sioul** is in the tray icon's menu, or ++ctrl+q++. Where the desktop has no system tray (GNOME without its extension), closing the window quits Sioul. On a phone or a tablet, put Sioul away as any app: what it still does then is in [On a phone or a tablet](first-steps.md#on-a-phone). [Keep Sioul open](first-steps.md#keep-sioul-open)

## A first look {#a-first-look}

The left column holds the places, one icon each. On a phone, they slide in from the left, behind ☰. Five of them are the base.

| Place | Key | What it holds |
|---|---|---|
| **Porch** | ++ctrl+1++ | what came, waiting for its hours |
| **Tasks** | ++ctrl+2++ | the one next step, and the plan behind it |
| **Mail** | ++ctrl+3++ | all your mail, for when you choose to look |
| **Agenda** | ++ctrl+5++ | what comes, from today |
| **Contacts** | ++ctrl+6++ | the people you deal with, and who may reach you |

### The Porch: what came {#the-porch}

The Porch is where new mail from every address waits until you look. Sioul checks each message first, genuine or forged, and sorts it into a lane: people you know, a project, newsletters, what is set aside.

- **It opens in your hours**, and says until when: "Open until 17:00." Outside them, it says when it opens next, and nothing else of your mail.
- **The codes and sign-in links you just asked a site for come at once**, whatever the hour, in **Right now**, with **Copy**.
- **A stranger's first message waits in the screener.** **Let this address in** sends their next messages to the people you know.
- **Each row** has **Mark read** and **Dismiss**, which hold on every device. **Done for now** closes the Porch on what it showed. Nothing is deleted, and closing it marks nothing read.

More: [The Porch](porch.md)

### Mail: everything, when you choose to look {#mail}

The **Mail** page keeps all your mail. It opens on **All addresses**: one list for every address, newest first, each message once, with a small dot before what you have not read, and nothing counted.

- **A click opens a message** beside the list. Sioul says, before you read it, whether it really comes from the address it shows. Nothing in a message loads or runs.
- **Write** starts a message, in plain text or in Markdown. **Send** (++ctrl+enter++) waits ten seconds, with **Undo**, before the message leaves. Drafts are saved as you type.
- **Which mail shows** follows your hours: work's addresses in working hours, your admin's in admin hours. The button beside the chips (**All**, **Unread**, **Sent**, **Spam**) shows another area whenever you want it.

More: [Mail](mail.md)

### Agenda: what comes {#agenda}

The **Agenda** opens on **What comes**: today in full, then the days that hold something, as a calm list. **Day**, **Week** and **Month** are one switch away.

- **A new event**: **New ▾ ▸ An event**, or a double tap on a day or an hour. It asks for a title, when, and where.
- **Before** and **After**, in its form, keep the time to get ready, get there and come back. Your plan keeps that time free, and the event says when to leave.
- **One quiet reminder** comes a quarter of an hour before the event and its time to get there. Nothing is ever late: the past shows only when you go back to it.

Your events stay on your calendar server, as standard events: your phone and your other programs see them.

More: [Agenda](agenda.md)

### Tasks: one next step {#tasks}

The **Tasks** page opens on **Now**: one step, why it comes now, and the one after it. It has five buttons: **Start**, **Done**, **Not now**, **Details** and **What makes it hard?**

<figure markdown="span">
  [![The Tasks page on "Now". At the top, the views Now, The day, List, Board and Gantt, then Routines and three choices: every project, all kinds, any category. In the middle, one card, "The next step": its title, the project it is part of, why it comes now, about how long it takes and where you stopped, with Start, Done, Not now, Details and What makes it hard?. Below, "After that" and the following step, its date asked as days left; then "Other choices" and "Done this week", folded.](../assets/screens/tasks-now.png){ loading=lazy }](../assets/screens/tasks-now.png "Open the picture at full size")
  <figcaption>Now: one step, why it comes now, and the one after it.</figcaption>
</figure>

- **A new task**: **New task**, or one line at the top of the page. `Call the tax office tomorrow ~15m {30/10}` makes a task that can start tomorrow, takes a quarter of an hour, and is asked for 30 October.
- **Not now** is honoured without argument: the step comes back tomorrow, and the next step comes from something else.
- **How is today?** At the top of the page, **Clear**, **Haze** or **Fog**: a hard day holds less. Only you say it; nothing is guessed from what you do.
- **The day** lays today out: your events, your meals, and the steps the plan gives today, in the hours meant for them.

Your tasks are standard tasks, on your calendar server too.

More: [Tasks](tasks.md)

### Contacts: who may reach you {#contacts}

The **Contacts** page lists the people you deal with, names first; a few letters find one. From a message, **Add to contacts** makes its sender a contact in one click.

Contacts do more here than hold addresses. Each person is on a list, and the list decides when their mail, their calls and their messages reach you.

| List | Who is on it |
|---|---|
| **Safe** | family, friends, chosen colleagues; only you put someone there |
| **Your contacts** | anyone in your address books, until you choose otherwise |
| **Restricted** | people you would rather hear from only at times you choose |
| **Blocked** | never, on any channel |
| **Strangers** | everyone else; their first message waits in the Porch's screener |

On a contact's card, **How Sam reaches you**, with the person's own name, shows their list and changes it. What each list does, for mail and for messages, is in **Settings ▸ What reaches you**, under **People**; calls are set for each phone, in **Settings ▸ Devices**. One more list, **Always through**, is for the few people who must reach you even during do-not-disturb: it is the same on every device, and a blocked person is never on it.

More: [Contacts](contacts.md) · [What reaches you, and when](notifications.md)

## A second device {#a-second-device}

Each device keeps all of its data, and works without the others. Three things keep them in step, each in its own way.

| What | How it reaches your other devices |
|---|---|
| Mail, contacts, events and tasks | through their own servers, as with any program |
| What Sioul keeps on the device: your settings, your accounts (never their passwords), your hours, who may reach you, the ties between things, drafts, and its own lists (projects, budgets…) once they have left your notes folder | through Sioul's **sharing**: a folder that carries sealed changes |
| Your notes folder | by its own sync app, or through the sharing once you switch it on |

A sync app you already use carries the sharing folder (Nextcloud, Dropbox, Syncthing, Google Drive, OneDrive or another), or Sioul carries it itself when you have a Nextcloud account. Every change is encrypted on your device before it is written there, with a key made from a passphrase you choose: the folder and its server can read nothing. There is no server of ours.

### On the first device: start sharing {#start-sharing}

1. Open **Settings ▸ Sharing**, and find **Between your devices**.
2. **Folder**: choose a folder inside the one your sync app carries, such as a new folder `Sioul` in your Nextcloud folder. With a Nextcloud account in Sioul, you can choose **Sioul keeps it in step with your Nextcloud itself** instead, and need no sync app.
3. **Passphrase**, then **Once more**: a few words you will not forget, at least 12 characters.
4. **Share**.

!!! warning "A lost passphrase cannot be found again"
    It is never sent anywhere, so nobody can recover it for you. If it is lost, stop sharing on every device and start again with a new folder. Nothing on your devices is lost.

### On the new device: join {#join}

Install Sioul and open it. Its Porch offers **Join your other devices**, with three ways in:

- **Sign in to Murena**, or **Sign in to another Nextcloud**: your Nextcloud's own sign-in page opens in your browser, and Sioul then finds your sharing folder there.
- **I use a sync app's folder**: choose the same folder, as your sync app shows it on this device.

Type your passphrase, once, and choose **Join**. A first exchange brings your settings, your accounts and your hours. Passwords never travel: **Still to sign in on this device** lists the accounts that ask for theirs, mail mostly. Give each its password in **Accounts**.

On a computer that shares already, **Add a device**, in the same panel, says what the new device needs.

### What to expect {#what-to-expect}

- **Your rules follow you.** Hours, lists and do-not-disturb set on one device hold on the others, usually within minutes.
- **Each device chooses what travels from it**, part by part, under **What travels from this device**.
- **Update your devices together when you can.** Devices on different versions go on sharing, but a part whose form is newer than a device can read waits on that device until it is updated. Sioul's own lists (projects, budgets…) leave your notes folder only once every device runs a Sioul that knows their new place: an older one keeps the move waiting until it is updated, or forgotten in **Settings ▸ Devices**.
- **On Android**, a sync app often brings changes only every half hour. Where the folder is on your Nextcloud, Sioul reads your other devices' news there itself: each minute while it is on the screen or another of your devices is in use, and otherwise every few minutes in the background, while the phone keeps in step there.

More: [Sharing between your devices](sharing.md)

## Next {#next}

- [Going further](going-further.md): your own spam filter and your mail filters, topics and the other labels, tasks that wait for each other, and the people you do admin for.
- [First steps](first-steps.md): the window and its keys, each provider's sign-in, the notes folder, the sites you pin, and Sioul on a phone.
- [Hours](hours.md) and [What reaches you, and when](notifications.md): the two settings that shape everything else.
