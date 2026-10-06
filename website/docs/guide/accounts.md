---
description: The Accounts page in Sioul - your addresses and their services, adding an account, who may write to you and when, and your encryption keys.
---

# Accounts

**Accounts** is the person icon at the bottom of the left column. It has four tabs: **Your accounts**, **Add an account**, **Senders**, **Encryption**.

Sites (secure mailboxes, chats) are not accounts: they are made and changed on the [Sites](sites.md) page.

## Your accounts

<figure markdown="span">
  [![The "Your accounts" tab: at the top, "How far back mail and the agenda reach", set to 2 weeks; below, an address's card: its mail service with its server, a switch and Remove, then what this address is for (Work ticked, Your admin, Leisure), how far back, how often it is fetched, the protection against harassment, its priority, "Name and signature…", and "Server and folders", folded.](../assets/screens/accounts.png){ loading=lazy }](../assets/screens/accounts.png "Open the picture at full size")
  <figcaption>One card per address, each of its services with a switch.</figcaption>
</figure>

At the top, **How far back mail and the agenda reach**: from one week to one year, or everything. Older mail is fetched at the next round; "Everything" can take a while, and room on disk.

Then one card per address, with each of its services: **Mail**; **Calendars, tasks and contacts**; **Google: calendars, contacts and tasks**. Each has:

- **a switch**: off, the service keeps its settings and is neither synced nor shown;
- its server, and what its last fetch said, in a sentence;
- **Remove**.

**What this server offers**, at the top of the card, asks the server what else it has: calendars and contacts beside the mail, and for a Nextcloud, its version and its apps. What Sioul can use is added in one click.

### A mail address

On its card, **Settings for this address**, folded until you open it:

- **What this address is for**: work, your admin, leisure, any of them together. Its mail comes in the hours for what it is for. Nothing ticked counts as work, so that it never reaches your evenings. See [Hours](hours.md).
- **How far back**: how many weeks of this address's mail its folders show, or like the other addresses.
- **Fetch every**: how often its folders other than the inbox are fetched; 0 follows the other addresses. A public address can be fetched twice a day (720 minutes).
- **Protected against harassment**, and once it is on, **Let the AI read it first**. See [the Porch](porch.md#a-public-address-protected).

Always in view below it:

- **Priority**: *More important*, *Normal* or *Less important*. See [the Porch](porch.md#some-addresses-first-others-last).
- **Name and signature…**: your name, as recipients see it, and your signature, in Markdown.

Then **Server and folders**, folded: the server, and where its mail is kept on this device.

Below the cards, **The AI shield** holds the key for Anthropic's service, used only by addresses that let the AI read them first. It is kept in your system's keyring, never in a file; **Forget the key** removes it.

### Removing one

**Remove** asks first. A mail account's password leaves the keyring; its mail stays on your disk and on the server. For Google, the access is given back to Google: the mail's and the calendars' apart, so that removing one leaves the other its access.

### An account from your other device

[Sharing between your devices](sharing.md) brings your accounts, never their passwords. Such an account says it has no password here yet, with **Password…** on its card: type it, or choose **From Bitwarden…**. Your vault opens (its master password, then its second step: an app's code, an e-mail's, a YubiKey's), and the logins whose user name is this address are listed, the server's own first; choose one, or type a site to narrow them ([Sites](sites.md#logins-from-bitwarden)). The password is tried with the server, then kept in this device's keyring. **Password…** comes back if the server ever refuses the one kept.

On a phone, a security key cannot open the vault yet: use another second step of your Bitwarden account.

### Google's mail

Gmail or Google Workspace mail signed in with Google says when Google ends the access (your Google password changed, your Google key deleted, a Google project left in testing). On its card, **Sign in again** opens the mail form on Google's page, with your key; **App password…** gives it an app password instead. Sioul never asks for your Google account's password. See [First steps](first-steps.md#gmail-and-google-workspace).

## Add an account

Three forms, one after the other:

- **Add a mail account**: your address, **Find the server**, your password, **Connect and add**. For Gmail and Google Workspace, **Use an app password** or **Sign in with Google** instead of a password.
- **Add contacts and calendars**: from a CalDAV and CardDAV server, such as Nextcloud, Fastmail, iCloud or your host.
- **Google calendars, contacts and tasks**: **Sign in with Google**, on Google's own page.

On a phone, **From this phone's accounts…**, above them, opens Android's own list of the accounts the phone knows. A Google address fills the Google form and the mail form, with Google's two ways; any other fills the mail form, whose server Sioul then looks for, and the one for contacts and calendars. Android lends no password: Sioul asks for it once.

Step by step: [First steps](first-steps.md#add-your-mail). Passwords go to your system's keyring, nowhere else.

## Senders

<figure markdown="span">
  [![The Senders tab: a grid of boxes, the lists Safe, Neutral and Restricted down and the times Work, Admin, Leisure, Meals and Sleep across; then four lists, "Safe: any time", "Neutral: work, admin", "Restricted: work" and "Blocked: never", each with addresses and patterns such as *@example.org, and a field to add one; then your contacts' categories, each with a list or none.](../assets/screens/accounts-senders.png){ loading=lazy }](../assets/screens/accounts-senders.png "Open the picture at full size")
  <figcaption>Who may write to you, and when.</figcaption>
</figure>

Who may write to you, and when, whatever the address they write to.

**When each list's mail comes**: a grid of boxes, the lists down (Safe, Neutral, Restricted), the five times across (Work, Admin, Leisure, Meals, Sleep: see [Hours](hours.md)). Tick as many as you like on each row. Ticked, their mail comes then; unticked, it waits, never lost, for the next time ticked. On a narrow screen the grid scrolls sideways. As it comes:

| | Work | Admin | Leisure | Meals | Sleep |
|---|---|---|---|---|---|
| Safe | ✓ | ✓ | ✓ | ✓ | ✓ |
| Neutral | ✓ | ✓ | | | |
| Restricted | ✓ | | | | |

The codes and links you just asked a site for, and what you send yourself, come at once whatever the grid says. While you sleep nothing notifies: mail ticked for sleep shows on the Porch if you open Sioul, without a sound or a notification.

**Four lists**, each saying its times:

- **Safe**: friends, chosen colleagues, chosen family. Their mail skips the screener, and comes to any of your addresses. Only you put someone there.
- **Neutral**: everyone no list names, strangers included. Name someone here to keep them neutral inside a domain or a category on another list.
- **Restricted**: those you would rather hear from only at chosen times: a demanding client, someone whose mail weighs.
- **Blocked**: spam and harassment, set aside for good, never shown, never counted, never notified. Nothing is deleted.

Each line is an address, or a pattern with `*`: `*@example.org` for everyone there, `*@*.example.org` for its subdomains. Nobody is blocked for sharing a server or a domain with someone else.

The mail of a neutral or restricted sender comes only to an address for what now is for (work's address in working hours, a personal address in admin hours and leisure); when the two never meet, their list's times alone decide, so that nothing waits for good.

**Your contacts' categories**: one row for each category your contacts use (Friends, Family, Clients…), with a choice: **No list**, **Safe**, **Neutral**, **Restricted** or **Blocked**. Everyone whose contact card is in the category takes it, at each of their addresses. Nothing goes on a list by itself, family and friends included.

**From the person to the group**: a person's own choice comes first, then the categories on their contact card, then their address's domain; everyone else is neutral. So one friend can be neutral while the category Friends is safe, and a colleague in Friends stays safe in a domain you marked restricted. When the same level gives two answers (a person in two categories on two lists), blocked wins, then restricted, then neutral, then safe.

Forged mail is judged apart, before the lists: a forged message is set aside even when it claims a safe sender's address, and weighed as a stranger's.

From a message or a contact, **Their mail** puts someone in one of the four lists, or back to **As their categories say**. The lists themselves, with their patterns, and the categories are edited here only.

## Encryption

Your OpenPGP keys, to sign and encrypt your messages ([Mail](mail.md#signing-and-encrypting)):

- **Make a key for** an address: a new key, valid three years; its passphrase is made at random and kept in your keyring, so nothing is asked at each message.
- **Import a key…**: a key exported from GnuPG, with its passphrase, asked once.
- **Save the public key**: into your downloads, to give to others.
- **Keys of others**: those that came with their messages, or from a file, or found by **Look for their keys** in the writing window.

Your own keys stay on this device. They are not shared with your other devices: copy them by hand if you need them there.
