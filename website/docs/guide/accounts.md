---
description: The Accounts page in Sioul - your addresses and their services, adding an account, and your encryption keys; who may reach you and when is in Settings, What reaches you.
---

# Accounts

**Accounts** is the person icon at the bottom of the left column. It has three tabs: **Your accounts**, **Add an account**, **Encryption**. Who may reach you, and when, is in Settings ▸ [What reaches you](notifications.md).

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

## Who may reach you {#senders}

Who may reach you, and when, by mail, by phone and through other apps' messages, and who is on which list (safe, neutral, restricted, blocked, with addresses, numbers, patterns such as `*@example.org`, the people placed on a list and your contacts' categories): Settings ▸ **What reaches you** ▸ **By person** ([What reaches you, and when](notifications.md#by-person)). For one person, their sheet, from their card or a message: **How … reaches you** ([A person's sheet](notifications.md#a-persons-sheet)). What each address is for stays on its card, above: mail to an address for another time waits for that time.

## Encryption

Your OpenPGP keys, to sign and encrypt your messages ([Mail](mail.md#signing-and-encrypting)):

- **Make a key for** an address: a new key, valid three years; its passphrase is made at random and kept in your keyring, so nothing is asked at each message.
- **Import a key…**: a key exported from GnuPG, with its passphrase, asked once.
- **Save the public key**: into your downloads, to give to others.
- **Keys of others**: those that came with their messages, or from a file, or found by **Look for their keys** in the writing window.

Your own keys stay on this device. They are not shared with your other devices: copy them by hand if you need them there.

### Security key

Your OpenPGP keys can stay on a security key, such as a YubiKey or a Nitrokey: it signs and opens your mail itself, after its PIN, and its private keys never leave it. Sioul keeps only their public part. On a computer, Sioul reaches the key through the system's smart card service (on Linux, `pcscd`, which most systems start when a program asks); on a phone, not yet.

1. Plug the key in, then choose **Use a security key**. Sioul reads it without its PIN: its name and serial number, what it signs and decrypts with.
2. Sioul needs the public part of these keys, which the key does not hold. **Look for it** asks the address written on the key, your domain's key directory, then keys.openpgp.org; this tells those servers that someone looks for your key. Or **Import a file…**: with GnuPG, the command Sioul shows (`gpg --export --armor` followed by your key's fingerprint) writes it into a file.
3. Sioul keeps it only when it is the key's own: the same keys, the same public parts. Your security key then signs for the addresses it names among yours, alone (a key made or imported here for the same address only opens older mail), and opens what is encrypted to it.

Each key then shows what it signs for, whether it asks for a touch, where its certificate came from, and when it expires. Once it has expired, Sioul says so, with the two GnuPG commands that renew it, and says it a month ahead too: mail cannot be signed with it until it is renewed, while older mail still opens. **Look for a newer version** brings the renewed certificate, or **Import a file…** a new export. **Forget the PIN now** forgets the PIN Sioul holds. **Stop using this security key** puts its certificate aside.

GnuPG keeps the key for itself once it has used it. Sioul then says that another program holds it, and offers **Let GnuPG release it**, which stops GnuPG's smart card daemon, on that click only; gpg takes the key back the next time it needs it, and asks for its PIN again then. For both to share the key, add the line `pcsc-shared` to `~/.gnupg/scdaemon.conf`.

Sioul only uses the key: it never asks for its Admin PIN, never changes or unblocks a PIN, never loads or makes keys on it.
