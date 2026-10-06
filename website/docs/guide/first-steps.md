---
description: The window, its keys, and the first things to set up in Sioul - mail, calendars and contacts, Google, your notes folder, your hours, your sites.
---

# First steps

Nothing has to be set up at once. Each step below is useful on its own, and Sioul works with whatever you give it. Open it from your application menu, or with `sioul-app` in a terminal.

Sioul speaks English and French, as your system does, unless you choose otherwise in **Settings ▸ Display ▸ Language**.

## The window

<figure markdown="span">
  [![Sioul's window: on the left, New, then the places from Porch to Health, and three icons at the bottom (Accounts, Settings, Refresh everything); on the right, the Porch; along the bottom, the status line with the keys, the sound button and the weather.](../assets/screens/porch.png){ loading=lazy }](../assets/screens/porch.png "Open the picture at full size")
</figure>

**On the left**, from top to bottom:

- **New ▾** makes something of any kind: a message, a task, an event, a contact, a note, a project, time spent, a budget movement, a paper; and **Where I stopped…** leaves one line on where you were ([Tasks](tasks.md#starting-and-stopping)).
- **The places**: Porch, Tasks, Mail, Sites, Agenda, Contacts, Notes, Projects, Time, Budgets, Papers, Health.
- **Three icons**, apart at the bottom: Accounts (a person), Settings (sliders), and Refresh everything, which fetches mail, the agenda, tasks and contacts again at once.

**At the bottom**, the status line says one sentence about what happened last. After anything is moved, deleted or sent, "Undo" waits there for ten seconds. In quiet time, it says when work comes back; once the day's hours are over, it offers to close the work day, and in the evening the day. At its right end are the keys, the sound button ([sounds to focus or rest by](tasks.md#sounds)) and the weather at a place you choose.

**On each page**, the ⚙ at the end of the first row holds that page's own settings, each with a sentence on what it changes; they are saved at once. Where long text is read (a message, a note), "Aa" sets the font, its size and the space between lines. A right click, the Menu key, or a long press on a touch screen, on anything gives what is not in view.

### Keys

Everything works from the keyboard: Tab to move, Enter to choose, Escape to go back.

| Keys | What they do |
|---|---|
| ++ctrl+1++ to ++ctrl+9++, ++ctrl+0++ | the first ten places, in the order of the list: Porch, Tasks, Mail, Sites, Agenda, Contacts, Notes, Projects, Time, Budgets |
| ++ctrl+n++ | New ▾ |
| ++ctrl+z++ | Undo, while it is offered |
| ++f5++ | Refresh everything |
| ++ctrl+enter++ | Send, in the writing window |

### On a phone

An Android version is being tried ([Install](install.md#on-android)). On a phone, or in a window under 720 pixels wide:

- **The places** slide in from the left, behind ☰; a bar on top names the page.
- **One pane at a time**: a page shows its list, then what you open across the whole screen; **Back**, on the bar or Android's, comes back.
- **Menus** open at a long press on a touch screen, where a mouse would right click.

What differs on a phone:

- **Notifications** come only for doses ([Health](health.md#reminders)) and for the time running ([Time](time.md#where-time-comes-from)). A code you asked for shows on the Porch; other reminders do not come there yet.
- **Mail** is fetched while Sioul is open: Android stops it in the background.
- **Folders**: a setting's **Choose…** opens Sioul's own list of the phone's folders, with **Allow access to files** when Android has not given Sioul that access yet.
- **Sites** open in your browser ([Sites](sites.md)); PDFs open in another app, with **Open with…**.
- **Paper letters** are not read, and attachments are not checked by an antivirus: the programs Sioul uses for that on a computer do not exist on a phone.
- **Your other devices** share with it through a folder your phone's sync app carries ([Sharing](sharing.md)). Passwords never travel: each account asks for its own, once ([Accounts](accounts.md#an-account-from-your-other-device)).

## Add your mail

1. Open **Accounts** (the person icon), then the **Add an account** tab.
2. Under "Add a mail account", type your **email address** and choose **Find the server**.
3. Sioul says what it found and where from: your provider's own settings, Thunderbird's list of providers, or a guess, said as such, to check before going on.
4. Type your **password**. It goes to your system's keyring, nowhere else.
5. Choose **Connect and add**. Sioul tests the password before keeping anything, then fetches your recent mail.

Gmail, and other providers when two-step verification is on, want an **app password** instead of your usual one: you make it in your account's settings at the provider (for Gmail, at myaccount.google.com/apppasswords). When the provider is known, the form offers **Make an app password**, which opens the right page.

Outlook.com, Hotmail and Microsoft 365 addresses cannot be added yet: Microsoft takes only its own sign-in page from other mail programs ([Works with](compatibility.md#mail)).

Fetching changes nothing on your mail server. Sioul writes there only when you act: opening a message marks it read, as any mail program does; archiving, deleting and moving happen ten seconds after you asked, so that "Undo" can stop them.

Then, on the address's card in **Your accounts**, unfold **Settings for this address** and tick **What this address is for**: work, your admin, leisure, or several. Until you say, an address counts as work, so that it never reaches your evenings. See [Hours](hours.md).

## Add your calendars, tasks and contacts

From a CalDAV and CardDAV server: Nextcloud, Fastmail, iCloud, your host.

1. In **Accounts ▸ Add an account**, under "Add contacts and calendars", type your address and password.
2. If Sioul cannot find the server from the address, unfold **Server address, when it cannot be found** and give it.
3. Choose **Connect and add**. Your address books and calendars come; tasks come with the calendars that hold them.

Already added your mail? On its card in **Your accounts**, **What this server offers** asks the server: its calendars and contacts, and for a Nextcloud, its version and apps. One click adds them.

## Add Google

Google takes no password from other programs: you sign in on Google's own page.

1. In **Accounts ▸ Add an account**, under "Google calendars, contacts and tasks", type your Google address.
2. Choose **Sign in with Google**. Your browser opens on Google's page; Sioul waits for its answer for five minutes at most.
3. Sign in, and allow what Sioul asks: your calendars, your contacts and your tasks.

Sioul keeps the access in your system's keyring. Google keeps less than an open server: what it does not keep shows greyed in Sioul, never hidden, with why. What Sioul reads and writes in your Google account, and how to take the access back: [Privacy policy](../privacy.md#google-calendars-contacts-and-tasks).

!!! note "If Sioul asks for a Google key"
    A copy of Sioul built without its own Google key asks for yours. **How to make your Google key** unfolds the steps in Accounts: a free project in Google Cloud, about fifteen minutes, once. You can also choose **Use a Google key of my own** at any time.

Google's mail is added as any mail account, above, with an app password.

## Choose your notes folder

Your notes are a folder of Markdown files: an Obsidian vault works as it is. Sioul also keeps your projects, budgets, papers and scanned letters in that folder, so that they travel with it to your other devices.

In **Settings ▸ Your folder and sharing**, choose **The notes folder**. Sioul reads it and links to it; it never owns it. See [Notes](notes.md).

## Set your hours

In **Settings ▸ Hours**: your working hours and hours for your own admin; every other time is leisure, and meals and sleep come from the Health page. Without hours, everything comes at any hour, as in other mail programs. Until they are set, the Porch asks once, with **Set my hours** and **Leave as is**. See [Hours](hours.md).

## Pin the websites you check

The secure mailboxes of your bank, your health insurer, the tax office; a chat; a video call.

On the **Sites** page, **Usual sites ▾** lists about 400 of them by country, or **Pin a site** finds one by a word ("bank", "ameli"), or takes any address by hand. You log in once; the site keeps you logged in. See [Sites](sites.md).

## Keep Sioul open

While its window is open, Sioul keeps each inbox open on the server: a code or a sign-in link you asked a site for reaches you within seconds, as one quiet notification, whatever the hour.

Reminders can also come with the window closed: in **Settings ▸ Reminders and notifications**, tick **With Sioul's window closed**. A small watcher then starts with your session; it fetches no mail.

## Next

- [The Porch](porch.md), where new mail waits.
- [Tasks](tasks.md), and the one next step.
- [Hours](hours.md), and quiet time.
- [Works with](compatibility.md): the servers, apps and systems Sioul works with, and how far each was tried.
