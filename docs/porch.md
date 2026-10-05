# The Porch: the first slice

The Porch is where new mail waits, checked and sorted, until you look. It is the first slice because it removes the main source of distress, the inbox that screams, before anything else is built.

## Lanes, in the order they are decided
Order matters: what protects you most comes first.

| Lane | When | Shown |
|---|---|---|
| **Set aside** | forged (DMARC fail) or flagged as spam by your provider | in the window, at the bottom, with the reason; never deleted |
| **Right now** | a one-time code, a password, a password reset, a sign-in link or an address to confirm: what you just asked a site for, from its automatic address (no-reply…) too | at once, whatever the time: the one exception to the windows |
| **A case** | a route of the case store matches ([case-store.md](case-store.md)) | in the window, under the case's title |
| **From yourself** | mail from one of your own addresses to another (a file sent from your phone), verified | with people you know, in any hour; never screened, never taken for spam |
| **Filed** | a newsletter or mailing list (`List-Id`, `List-Unsubscribe`, `Precedence: bulk`), or an automatic address (no-reply…) | in the window, folded |
| **Screener** | a sender you do not know yet | in the window; you let them in, or not |
| **People** | someone you know, outside any case | in the window |
| **Less important accounts** | any mail of an account you ranked below the others (social networks, notifications read sometimes, never answered), unless a case takes it | in the window, folded at the bottom; one sentence at the end of the summary, outside its count |

These messages come from an explicit request of yours, a moment ago: they are expected, and needed at once. A forged one is set aside before (DMARC fail, a borrowed brand's name, spam); one from a sender that is only **not verified** comes right now too, with a warning to use it only if you just asked that site for it, because fake codes are a phishing trick. A message "from yourself" that is not verified gets no pass: a forged own address is a classic trick.

Each lane says what it holds in one line under its title; its "?" shows how mail lands there, sentence by sentence, with the Porch's own settings that change it (the senders you let in, the words of automatic senders) and where the others are: an address's rank and shield on its card in Accounts, a project's routes on its page in Projects, who is safe, neutral or blocked in Accounts ▸ Senders. The Porch's ⚙ holds what is the Porch's alone: the projects that have a lane here (the others' mail stays on their page in Projects), where paper letters arrive, and every lane but the projects', empty or not, in the order mail is sorted.

## A public address, shielded
A contact address brings work, and also insults. An account with `shield = true` has its mail read before you see it (`shield.rs`):
- **its tone**: calm, rude (swearing, contempt), or hostile (insults aimed at you, harassment, threats), from word lists in French and English matched on whole words, case and accents aside; an insult near "you" weighs more, and shouting adds to it;
- **its topic**: work (a job, a mission, a quote), a question, the press, thanks, a donation, other.

Hostile mail goes to a folded "Hostile" lane that shows neither the sender's name nor the subject; opening one asks first (forward it to someone you trust, block, delete, or read anyway). The rest of the address's mail has its own lane, work first, with its topic and, when rude, a mark; the address can have its own fetching pace (`fetch_minutes`).

With `shield_ai = true` and a key for Anthropic's API (typed in the settings, kept in the system keyring), the subject and text of each new message are sent once to Claude Haiku, which says the tone, the topic and one neutral line on what it asks; the answer is kept by Message-ID (`$XDG_STATE_HOME/sioul/shield/<account>.toml`) and takes precedence over the word lists. Off by default: the text leaves the computer. `sioul shield [--account <id>] [--read]` shows what the shield makes of each message.

Sites (secure mailboxes, chats) add their notifications above the lanes, each site with its lines ([sites.md](sites.md)). Before them, when there is something, a few cards: where you stopped, until "Done" (`StoppedCard.qml`, [tasks.md](tasks.md)); today's two events at once, margins counted, with "Open “…”" for each event and "Don't mention it again" ([client.md](client.md), Calendars); the doses due while Sioul was closed, with "Taken…" and "Not taken" and the doubt under each when another device may know more ([health.md](health.md)); the week's payments and paper letters, but not at rest ([areas.md](areas.md)).

**Account priority**: above, average (the default) or below, in the Accounts page or with `sioul account priority <id> <above|average|below>`. Mail of the accounts ranked above comes first in every lane, and a folded lane opens by itself when it holds some. Verified codes come at once whatever the account's rank.

## Right now: the exception
- **When it shows**: as soon as the message arrives. Each account's inbox stays open with IMAP IDLE (RFC 2177), renewed every five minutes, so a code reaches you within seconds of reaching your provider.
- **How it shows**: one desktop notification, no sound, with the kind of secret, the sender and its trust, the code itself with a button to copy it where the desktop's notifications take buttons (Linux; on Windows and macOS the code is in the text), and how long it stays valid when the message says. In the window, the same card sits on top of the Porch, with a copy button. On a phone, the card alone: Android shows Sioul's notifications for the doses and the time running only ([android.md](android.md)).
- **What it does not do**: open the rest of the Porch.
- **Afterwards**: the code is hidden once expired, and the message goes to its lane. Expired means the validity the message states ("valid 10 minutes", "for 2 days"), else what its kind usually lasts: a code 30 minutes, a sign-in link an hour, a password reset two hours, an address to confirm a day, a temporary password a week (it lasts until you have used it).
- **Detection**: in French and English, of codes (4–8 digits, two groups of three, or a prefixed code like `G-482913`; two-factor and validation codes too), passwords (temporary, initial, "your login details"), password resets ("choose a new password"), sign-in links and addresses or accounts to confirm ("confirm your email", "activez votre compte"). Years, prices, phone numbers and postcodes are excluded, and so is a shop's code ("promo", "discount", "coupon"…) (`crates/sioul-core/src/codes.rs`).

## Working hours, and quiet time
- **Working hours**: the seven days of the week, each on or off, from a start to an end (Settings, at the bottom of the left column; `[[window]]` with `day`, `start`, `end` in the configuration). Older windows written as a start and a length still open the Porch, but only hours with an end bring quiet time.
- **Outside them**, the Porch says when it opens next, and nothing else: no counts, no names. "Open it anyway" stays possible, quietly.
- **Inside them**: a summary in a few sentences, then the lanes.
- **With no hours configured**, the Porch is always open and nothing is ever quiet.
- **Quiet time**: in the evening, on days without hours, during time off (Settings: from a day to another, with a word), and once the day is closed ("Done for today", [tasks.md](tasks.md)). Work rests until it comes back: the Porch shows the codes and links you ask sites for, what you send yourself, the mail of the senders you marked safe (below), and the mail of the addresses for what these hours are for ([areas.md](areas.md)); work addresses fold on the Mail page without their dots; work sites keep their notifications for later, chats and dating sites stay unless marked as work; budgets show only leisure money; Projects shows only yours, Time waits behind "Show anyway"; Tasks shows only yours. One sentence says when work comes back (`$XDG_STATE_HOME/sioul/quiet.toml` keeps the overrides).
- **The way back is never suggested**: the sentence in the status line opens a menu: work half an hour to four hours more, back to the usual hours, or, the day it was closed, back to today's plan. Detachment from work in the evening is what recovery needs most, and the mere expectation of work mail undoes it (Sonnentag & Fritz 2015; Becker et al. 2021).

## Who may write to you, and when
Three lists, each an address or a pattern with `*` per line (`*@example.org` for everyone there, `*@*.example.org` for its subdomains, `news*@shop.example`; `@example.org` too), in Accounts ▸ Senders ("Who may write to you"):
- **Safe** (`safe-senders.txt`): friends, chosen colleagues, chosen family. Their mail reaches you at any hour, quiet time included, and skips the screener. Only you put someone there.
- **Neutral** (`neutral-senders.txt`): everyone the lists do not name is neutral, strangers included; their mail waits for working hours. Naming someone here keeps them neutral inside a domain marked safe.
- **Blocked** (`blocked-senders.txt`): spam and harassment, set aside for good.
- **The most precise entry decides**: an address marked safe stays safe in a domain blocked; at equal precision, blocked comes before neutral before safe.
- **From a message** (⋮, "Their mail") or **a contact's card** ("Their mail", for every address on it), one choice among the three; the lists in Accounts ▸ Senders take patterns.
- **Forged mail is judged before and apart**: a forged message, or one borrowing a name, is set aside whatever the lists say, a safe sender's address included, and the reader offers no list choice on it ("not verified"): it could name someone else. Nobody is judged by the server or the domain they share with another sender (`crates/sioul-core/src/porch.rs`, `Senders`).

## The Porch's memory
- **An account the Porch was never closed on** shows its first two weeks only: the older mail kept since is the archive's, not the Porch's.
- **"Done for now"** closes the Porch on what it showed: for each account, the newest message shown (its UIDVALIDITY and UID). Anything newer waits for the next window, even mail that reached the server earlier and was fetched later.
- **"Done for now" is not told to the server.** Fetching uses EXAMINE and BODY.PEEK: closing the Porch changes no read flag. Opening a message marks it read, as in any client, and the reader's actions (reply, archive, delete, junk) are the mail client's ([client.md](client.md)).
- `sioul porch --all` shows everything again; the state lives in `~/.local/state/sioul/porch.toml`.

## Blocked senders and borrowed names
- **Blocking** an address, or everyone at a domain, sets their mail aside for good: never shown, never counted, never read as a payment, never notified. Nothing is deleted. The list is `~/.config/sioul/blocked-senders.txt` (above); the window blocks from a message or a contact and unblocks from Accounts ▸ Senders, the command line has `sioul block` and `sioul unblock`.
- **A name borrowed from a brand or a public service** ("lnfos PrimeVlDEO" sending from an unrelated domain), or **from one of your own domains** (a name made only of "yourdomain" and words like "support" or "mail admin", from an address outside that domain: the fake "your account will be deleted"), is set aside with the reason. Shared providers such as gmail.com are nobody's own domain, and your own name in full is not your domain. A signature proves the domain, not the name shown before it: names are compared by their skeleton, as Unicode's confusable detection does (UTS #39 §4), so lowercase l for capital I, 0 for O, rn for m and Cyrillic letters do not hide the brand. A sender you let in keeps the name they use (`crates/sioul-core/src/lookalike.rs`).

## Reading a message
- **Its headers apart**: who wrote, the address and how far it is verified, to whom, copied to whom, when, the subject.
- **Why it is where it is**, folded, on request.
- **Its attachments apart**, folded until asked for: one line each with its kind, name and size. **Opening or saving runs the antivirus first** (ClamAV: its daemon when it runs, else its scanner); a threat found, and nothing opens and the copy is deleted; no antivirus on the computer, and Sioul says the file will not be checked and asks first, with the command that installs one ([client.md](client.md), "Antivirus"). Mail set aside lists them and does not open them.
- **HTML mail, formatted and safe** (`ammonia`): paragraphs, lists, bold, links; nothing else survives, no images, styles, scripts, forms or frames, so nothing remote loads and nothing runs. Layout tables become plain blocks, headings bold paragraphs. The messages a reply quotes or a forward carries (Gmail's, Apple Mail's, Thunderbird's, Outlook's) are folded under "Show the earlier messages".
- **Plain text in parts**: what this message says; "On …, X wrote:"; the messages it quotes, set off by a bar per level and folded when long; the headers of the messages it forwards or answers (Outlook's "De : / Envoyé : / À : / Objet :"), in a box; the signature, dimmed. Its web links are clickable and shown by their site (`crates/sioul-core/src/reading.rs`).
- **Every link says where it goes** before you click: its full address shows under the message while the pointer is on it.

## Trust: Sioul checks every message
- **When a message is fetched**, Sioul checks it itself (Stalwart's `mail-auth`, through the system's DNS):
  - **SPF**, on the server that handed the message to your provider: the first hop from a public address at the top of the `Received` chain. Every hop below it could be written by anyone.
  - **DKIM**, each signature against the key its domain publishes. Checking at arrival matters: keys rotate, and some signatures expire within days.
  - **DMARC**, with the domain's own policy. A failure is a forgery when the domain asks receivers to act on it (quarantine, reject). With `p=none`, on a mailing list (which rewrites what it relays), or for mail that never came from outside (your provider's own notices, a message to yourself), the sender is only "not verified".
  - **ARC**, for forwarded mail, and the **reverse DNS** of the sending server.
- **The results go on top of the stored message**, as an `Authentication-Results` header (RFC 8601) under a name unique to this installation and under `.invalid`, so no sender can write results that pass for Sioul's. The Porch trusts them first, then the provider's. Mail fetched before is checked once, in the background (`sioul verify`; `--again` checks everything again).
- **Verified**: DMARC pass for the domain the From line shows, else a valid DKIM signature of that same domain (a signature of another domain says nothing of the sender). A DMARC failure always wins over any pass, and a results line that cannot be read to its end (an unclosed quote or comment) proves nothing. **Forged**: DMARC fail under a policy that says so. **Not verified**: everything else, with the reason.
- **Hovering the shield** says each check, its result and the domain it was for, and who checked: Sioul at arrival, or the provider.
- **Spam verdicts**: SpamAssassin's `X-Spam-Flag` and `X-Spam-Status`; rspamd's `X-Spamd-Result`, `X-Spam` and `X-Rspamd-Score`.

## Paper letters
The envelope stays outside: a scan, a phone photo or a PDF dropped in a folder (by you, a scanner, or someone who opens the post for you) is read on this computer, understood by rules, and waits for the window as a card, like mail. Opening post is part of admin anxiety (Money and Mental Health 2018), and a date written "within two months" is remembered by no one: the card says it as a date. Code: `crates/sioul-core/src/letters.rs` (reading, keeping, filing), `crates/sioul-sync/src/ocr.rs` (the text), `crates/sioul-app/src/letters.rs`, `qml/LettersSection.qml`.
- **Where scans arrive**: Porch ⚙ ▸ Paper letters ▸ "Where scans arrive" (unset: `letters/inbox` in the case store). PDF, PNG, JPEG, TIFF, WebP. A file still being written (less than twenty seconds old) is read the next minute. Each is read once (by its name, size and time).
- **The text**: a PDF's own text when it has one (Poppler's `pdftotext`), else its pages at 300 dpi (`pdftoppm`) read by Tesseract, in French and English when those are installed; photos and images by Tesseract directly. Both are your system's programs, optional as the antivirus is: without them the scans wait, unread, and the Porch says the command that installs them (Fedora: `sudo dnf install tesseract tesseract-langpack-fra poppler-utils`). Nothing leaves this computer.
- **What the rules find**, French first, English too:
  - *who*: a body that writes to everyone found in the letterhead (CAF, Assurance Maladie, Finances publiques, Urssaf, France Travail, MDPH, préfecture, tribunal, commissaire de justice, hospital, mairie, banks, EDF…), else the letterhead's first line;
  - *what*, the gravest first: a formal notice ("mise en demeure"), a tax notice, a decision with its ways of appeal, a reminder, an appointment ("convocation"), a bill ("avis des sommes à payer", "titre exécutoire", "facture"), an acknowledgment, an attestation, a contract;
  - *how much*: the amount on the line that asks for it ("Montant à payer", "somme de");
  - *by when*: a date after "avant le", "au plus tard le", "date limite", "échéance", "jusqu'au", "no later than", "by"; else a delay ("dans un délai de deux mois à compter de la notification", "sous huitaine", "sous quinzaine", "within 30 days"), counted from the day the scan came (the notification) or from the letter's own date when it says so; the words that set it shown as the letter writes them;
  - its own date ("Lyon, le 28 septembre 2026"), an appointment's day and hour ("le jeudi 12 novembre 2026 à 14 h 15"), "recommandée avec accusé de réception", your number with them ("Numéro allocataire", "Référence"), and the project whose name it carries.
- **The card**, in the window only: who and what, then a sentence each (asks €86.40; by Sunday 15 November ("avant le 15 novembre 2026"); sent registered; dated; your number). "See the scan"; "A task for that date" (in your usual list, due that day, tied to the scan and the project: "Pay the water bill €86.40", "The tax office's decision: contest it or not"); "Into the agenda" for an appointment, an hour long; a project; "Done, filed": the scan moves to `letters/<year>/<day> <sender> <kind>.pdf`, its text kept beside the list (`letters/<id>.txt`). Making the task files the scan first, so the task's tie holds.
- **Not built**: the AI reading a letter line by line, the "is this genuine?" checks (the office's domain, its IBAN), a mail intake address for a helper.

## Done
In `crates/sioul-core`, `crates/sioul-sync`, `crates/sioul-cli` and `crates/sioul-app`, with tests:
- reading messages from a Maildir or a folder of `.eml` files, and the card of each message;
- trust v0, the provider's id learned from mail, spam verdicts, the route;
- detection of one-time codes, and their expiry;
- the case store and routing, the known senders and "let in", the lanes and their reasons;
- the summary, in English and French;
- admin windows, and the Porch's memory ("Done for now");
- **accounts**: found from the address (the provider's autoconfig, then Thunderbird's ISPDB, then a guess said as such), passwords in the system keyring, written into the configuration without losing its comments;
- **IMAP sync**, over TLS (rustls) or STARTTLS, into Maildir: the last 14 days at first, then only what arrives, for every folder but views of others (Gmail's "All Mail"); flags and deletions made elsewhere brought back; one fetch at a time per account, across processes; the server written to only when you act ([client.md](client.md));
- **the watcher**: IDLE per account, reconnection with growing pauses, no retry after a refused password, the notification for "right now";
- `sioul porch`, `done`, `card`, `cases`, `window`, `budgets`, `account add|portal|list|test|password|remove`, `sync` and `watch`;
- **the window** (Qt Quick through CXX-Qt): the Porch with codes on top, the lanes and a plain-text reading pane; budgets; accounts with the add form; the status line; keyboard throughout; the desktop's light or dark colours. Built with Qt's development files ([building.md](building.md)), linted, and checked page by page on invented mail.

## Next, in this slice
1. **Trust, the rest**: Spamhaus DQS for the sending address; lookalike domains (the names are done).
2. **The screener's memory, continued**: known senders learned from your Sent folders.
3. **The background service on its own**: a user service that watches with the window closed, and a tray icon.
4. **Mail deleted or moved on the server** leaves the Porch too (CONDSTORE/QRESYNC).
5. **One card at a time**, the safe-opening card of the design; paper letters already come as cards (above).
6. **Portals as accounts**: web-only mailboxes (Proton without Bridge among them) opening in sealed profiles inside Sioul; their notification emails become "a letter waits".
7. **Gmail without an app password**: OAuth, once its restricted scope can be dealt with ([architecture.md](architecture.md), "Known hurdles").

## Accepted when
- A morning's mail sorts as the tests' fixtures do, in both languages.
- A verified code reaches you within seconds, outside any window, without opening anything else.
- Outside a window, nothing but "the Porch opens…" and codes reaches the screen.
- Every lane shows its reason on request; nothing is deleted.
