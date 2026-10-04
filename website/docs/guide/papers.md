---
description: Papers and paper letters in Sioul - the papers asked again and again, with where each stands and a reminder before renewing; scanned post read on your computer and shown as a calm card.
---

# Papers and letters

## Papers

The same papers are asked again and again: an identity card, the last tax notice, bank details, rent receipts less than three months old, a health insurance attestation. An expired one blocks what depends on it: a lease, a cover, a trip. Remembering that a passport ends in March is the kind of remembering that fails most (Landsiedel, Williams & Abbot-Smith 2017). So Sioul keeps your papers, says where each one stands, and reminds you when renewing should start.

<figure markdown="span">
  [![The Papers page, with Add a paper: papers grouped by family (Identity, Health, Home, Money, Warranties), each with its name and where it stands in words, such as "valid until Friday 18 December: time to renew it"; Plan the renewal beside those to renew, and Open beside each.](../assets/screens/papers.png){ loading=lazy }](../assets/screens/papers.png "Open the picture at full size")
  <figcaption>The papers, by family, each with where it stands.</figcaption>
</figure>

### Where each stands

In words, never in red:

- "valid until 20 December";
- "valid until 20 December: time to renew it";
- "ended on 3 May";
- "from 30 June, older than what is usually asked", for a rent receipt or a payslip.

### Adding a paper

**Add a paper** (or **New ▾ ▸ A paper**), then:

- **What**: an identity card, a passport, a residence permit, a driving licence, a health insurance card or attestation, a health cover, an insurance certificate, a tax notice, a rent receipt, bank details, a payslip, an attestation or certificate, a warranty, or another paper;
- **Name**: "Passport", "Rent receipt September";
- **File**: a scan or a photo, copied into your papers;
- **Issued on** and **Valid until**. For a passport, an identity card or a warranty, an end is proposed from the issue when you give none;
- **Whose**, in a household;
- **Notes**.

From a message, **Keep in papers** beside an attachment does the same: the antivirus checks the file, it is kept, and the form opens with a name and a kind guessed from the file's name and the subject. You only say what cannot be guessed.

To send one, the writing window has **A paper**, beside **Attach**. One that has ended, or is older than usually asked, says so in the list.

### Renewing in time

| Kind | Renewing starts |
|---|---|
| Identity card, passport | 3 months before it ends: an appointment, then the making |
| Residence permit | 4 months before |
| Driving licence | 2 months before |
| Health cover | 3 months before: some covers do not renew by themselves |
| Insurance certificate | 1 month before |
| Warranty, proof of purchase | 1 month before |

Rent receipts, payslips and attestations have no end: they are said older than three months. A tax notice is usually asked as the last one.

**A reminder** comes once, when renewing starts, at the next working hours: "Passport: valid until Sunday 20 December", with what renewing takes. Not for a paper you added after renewing began (you have just seen it), nor once its renewal is planned.

**Plan the renewal** makes a task in your usual list, so your phone has it, from when renewing starts to the day the paper ends, tied to the paper, with the steps known for its kind.

### Where papers live

`sioul-papers.toml` at the root of your notes folder, and the files in its `papers` folder: they travel with your notes. **Take out** removes a paper from the wallet; its file stays where it is.

## Paper letters

The envelope stays outside. A scan, a phone photo or a PDF dropped in a folder, by you, a scanner, or someone who opens the post for you, is read on this computer and waits for your hours as a card, like mail. Opening post is part of admin anxiety (Money and Mental Health 2018), and a delay written "within two months" is remembered by no one: the card says it as a date.

### Setting it up

1. In the Porch's ⚙, **Paper letters ▸ Where scans arrive**: the folder your scans come to. Unset, it is `letters/inbox` in your notes folder. A phone's scanner app syncing into it works too.
2. To read scans, Sioul uses two programs of your system: Poppler, for the text a PDF already holds, and Tesseract, to read images. Without them, the scans wait, unread, and the Porch says how to install them ([Install](install.md#the-packages)).

PDF, PNG, JPEG, TIFF and WebP files are read. A file still being written is read the next minute. Nothing leaves your computer.

### The card

On the Porch, each letter is a card: who sent it and what it is, then a sentence each for what it found:

- "Asks for €86.40."
- "By Sunday 15 November", with the words that set it, as the letter writes them.
- "An appointment: Thursday 12 November at 14:15."
- "Sent registered, with acknowledgment of receipt."
- "Dated 28 September." "Your number: …"

Then:

- **See the scan**;
- **A task for that date**: a task in your usual list, due that day, tied to the scan, such as "Pay the water bill €86.40";
- **Into the agenda**, for an appointment;
- the project it belongs to;
- **Done, filed**: the scan moves to `letters/<year>/`, named by its day, its sender and its kind; its text is kept.

The rules read French first, and English too. They know the French bodies that write to everyone (the tax office, health insurance, family allowances, Urssaf, France Travail, a court, a hospital, a town hall, a bank, an energy supplier), and the kinds of letters, the gravest first: a formal notice, a tax notice, a decision with its ways of appeal, a reminder, an appointment, a bill. Other senders are named by the first line of their letterhead.

### Not there yet

Planned: an AI that reads a letter line by line; a check that a letter is genuine (the office's domain, its bank details); an address where a helper could send scans by mail.
