---
title: Privacy policy
description: Sioul's privacy policy - Sioul runs on your device and has no server; what it reads and writes in your Google account, where it is kept, what is shared and how to take the access back.
---

# Privacy policy

*Last updated on 7 October 2026 (2026-10-07).*

Sioul is a free and open-source desktop application, published by Aurélien Pierre under the GPL-3.0-or-later licence. This policy says what Sioul does with your data, and in particular with the data it receives from Google when you sign in with a Google account.

## In short

- **Sioul runs on your device. There is no Sioul server.**
- Your data stays on your device and in your own accounts: your mail provider, your calendar server, your Google account.
- Nothing is sent to the developer of Sioul. Nothing is sold, nothing is used for advertising, nothing is used to build profiles.
- Data leaves your device only to the services you add, and to the few others described below, each for a feature you choose.

Anyone can read Sioul's source code, to check what this policy says: [github.com/aurelienpierre/sioul](https://github.com/aurelienpierre/sioul).

## Who is responsible

Sioul is made and published by Aurélien Pierre. Since Sioul has no server, its developer never receives, stores or sees your data. Questions about this policy: see [Contact](#contact).

## How Sioul works

Sioul connects, from your device, to the services you add to it: your mail provider (IMAP and SMTP), your calendar and contacts server (CalDAV and CardDAV), Google. It keeps a copy of what it fetches in files in your own user folders, so that it can work without waiting for the network. Passwords and access tokens go to your system's keyring (the Secret Service on Linux, the Credential Manager on Windows, the Keychain on macOS), never into a file.

## Google calendars, contacts and tasks

When you add a Google account in Sioul (Accounts ▸ Add an account ▸ Google calendars, contacts and tasks), you sign in on Google's own page, in your browser. Google then asks your permission for these scopes:

| Scope | What Sioul does with it |
|---|---|
| `openid`, `email` | Reads the address of the Google account you signed in with, to check that it is the one you gave. Nothing else. |
| `https://www.googleapis.com/auth/calendar` | Reads your calendars and their events, through Google's CalDAV interface, to show them in Sioul's agenda. Writes the events you create, change or delete in Sioul. |
| `https://www.googleapis.com/auth/carddav` | Reads your contacts, through Google's CardDAV interface, to show them in Sioul's contacts. Writes the contacts you create, change, move or delete in Sioul. |
| `https://www.googleapis.com/auth/tasks` | Reads the task lists and tasks of the account you signed in with, through the Google Tasks API, to show them in Sioul's tasks and plan. Writes the tasks and task lists you create, change, complete, move or delete in Sioul. |

If you untick tasks on Google's page, Sioul leaves tasks out, and the rest works.

Signing in with Google gives Sioul no access to your Gmail. If you read Gmail in Sioul, it is added as an ordinary mail account, with an app password you make yourself, as with any mail program.

### How Google data is used

Only to show your calendars, contacts and tasks in Sioul, on your device, and to write back to your Google account the changes you make there, when you make them. Within Sioul, this data serves the features you see: the agenda, the contacts, the tasks and the plan of your days, reminders before events and dates, and the ties you make between your things.

Google data is not used for advertising, not sold, not used to build profiles, and not used to develop, improve or train artificial intelligence or machine-learning models. Nobody working on Sioul can read it: it never reaches them.

### Where Google data is kept

- **On your device**: one file per event, task and contact, in Sioul's data folder (on Linux, `~/.local/share/sioul/calendars/` and `~/.local/share/sioul/contacts/`, in a folder named after the account).
- **In your own Google account**, where it already is.
- **The sign-in**: the refresh token Google gives, with the client's identifier, as one entry in your system's keyring. The access token stays in memory only, for an hour at most.

Nothing is kept on any server of the developer: there is none.

### Sharing of Google data

Sioul does not send data received from Google APIs to its developer, or to any third party. That data leaves your device only in these cases, each chosen by you:

1. **Back to Google**, when you change something in Sioul.
2. **When you send something yourself**: an e-mail is sent only when you press Send. When you write to the guests of an event, their addresses come from the event.
3. **To OpenStreetMap's geocoder**, only once you allow it (**Place them**, on the Contacts page's map, or **Place contacts on the map** in its settings): your contacts' postal addresses, Google contacts' included, are sent once each to OpenStreetMap's Nominatim service, to find where they are on the map. Nothing else is sent with them.
4. **To your other devices**, only if you turn on [sharing between your devices](guide/sharing.md): what Sioul keeps on this device about your things (for example, which note is tied to which task, or the time noted on a task) travels through a folder of your own sync service, encrypted on your device with your passphrase; that service cannot read it. Your Google events, contacts and tasks themselves travel only by Google.
5. **To an AI agent you connect yourself**, only if you do (see [Using an AI agent](guide/ai-agent.md)): an agent such as Claude Code or Claude Desktop, connected to Sioul on your device, can read what Sioul keeps, Google calendars, contacts and tasks included, and the company behind its model receives what it reads, under your own agreement with that company. Sioul never connects an agent by itself.

Sioul's use and transfer of information received from Google APIs adheres to the [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy), including the Limited Use requirements.

### Taking the access back

- **In Sioul**: Accounts ▸ Your accounts, on the Google account's card, **Remove**. Sioul gives the access back to Google (it revokes the token) and empties its keyring entry.
- **At Google**: [myaccount.google.com/permissions](https://myaccount.google.com/permissions), where you can remove Sioul's access at any time.

Removing the account leaves your Google account itself as it is. The copies already on your device stay in Sioul's data folder until you delete them: on Linux, the folders named after the account in `~/.local/share/sioul/calendars/` and `~/.local/share/sioul/contacts/`.

If you use a Google key of your own (your own Google Cloud project, as Sioul allows), all of the above holds the same: the data travels only between your device and Google.

## Other data

Sioul reads and keeps your mail, calendars, contacts, tasks, notes, budgets, papers and the other things you give it, on your device, as described above. Besides your own providers, it reaches other services only for a feature you choose:

- **Weather**, if you choose a place: Open-Meteo receives that place's coordinates, rounded to two decimals, at most every half hour, or the name of a town you search.
- **The map of your contacts**, if you allow it: OpenStreetMap's geocoder receives their postal addresses, once each; map images come from OpenStreetMap, or from the source you chose.
- **Finding someone's encryption key**, when you ask: their domain's Web Key Directory, then keys.openpgp.org, receive the address you write to.
- **The AI shield**, only for an address you protect against harassment and only if you turn it on: each new message to that address is sent once to Anthropic, with your own key, to tell its tone and topic. The answer stays on your device. It reads mail only, never Google data.
- **GitHub**, if you turn it on: your issues and pull requests are read with your own token. Nothing is written to GitHub.
- **Bitwarden**, when you fill a login from your vault: your Bitwarden server, with your account.
- **The sites you pin**, as any browser: each is also asked for its own icon.
- **Antivirus signatures**, once a day, only when your system keeps none of its own.

On a phone, if you give Sioul Android's notification access, it reads the notifications of the apps you let it see, on the phone, to hold them until their time; it keeps none of their words and sends nothing of them anywhere, your other devices included ([Settings](guide/settings.md#other-apps)).

On a phone, if you make Sioul Android's caller ID & spam app, Android shows it each call's number before the phone rings; Sioul decides there, on the phone, keeps the list of the calls it declined on the phone, and sends no number anywhere: no server, no lookup, your other devices included. It never answers, records or listens to a call ([Calls](guide/calls.md)).

**Your own spam filter**, on a computer, when you ask it to learn (**Train now**): Sioul reads from your mail provider, without changing anything there, the headers and the start of the text of your messages, in every folder but the trash, drafts, sent mail and Gmail's All Mail, and keeps them on that computer to learn from, with the language model it learns. Neither ever leaves that computer. Only the result, a table of numbers with no word of your mail in plain text, goes to your other devices, encrypted with your passphrase, if you share between your devices, with what you said is spam or not on each device (which message, where and when; never a word of it). If you import outside training material on that computer (`sioul spam import`: mail labelled elsewhere, its subjects and texts), it stays there, apart from your mail, never shared, until you remove it. Nothing of it goes to the developer, to an AI or to any other service. It learns from your mail only: never from data received from Google ([Privacy and security](guide/privacy-security.md#your-own-spam-filter)).

The complete list, with when each happens: [Privacy and security](guide/privacy-security.md#what-leaves-your-computer-and-when).

Sioul has no analytics, no advertising, no tracking, and no crash reports.

## Security

Connections to Google use HTTPS. The sign-in follows Google's guidance for installed applications: Google's page opens in your browser, and Sioul receives the answer on your own device only (a loopback address), checked with PKCE and a state value. Passwords and tokens are kept in your system's keyring. What you share between your devices is encrypted on your device before it leaves it.

## This website

This website has no analytics, no advertising and no cookies, and loads nothing from other sites. It is hosted by GitHub Pages, which keeps its own server logs under [GitHub's privacy statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

## Changes to this policy

Any change is published on this page, with a new date. The history of this page is public, in Sioul's repository.

## Contact

Questions about this policy, or about your data: [GitHub issues](https://github.com/aurelienpierre/sioul/issues).

The developer holds none of your data, so there is nothing of yours to ask the developer for, correct or delete: everything Sioul keeps is on your device and in your own accounts, where you can read it, change it or delete it.
