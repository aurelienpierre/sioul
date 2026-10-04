# Design

Sioul is designed first for people for whom admin hurts. Its rule: **nothing enters without your consent; letters wait outside; deadlines live in the plan, not in your face; a written channel is always open.** Every choice below comes from that rule, and from the research in [research.md](research.md). The detailed notes behind it, each study with its evidence and the rule it gives, are in [research/README.md](research/README.md).

## Six places
- **The Porch**: everything new, from every account and portal, waits here until an admin window. It is checked (genuine or forged), sorted into cases and summarised. New senders wait in the screener until you let them in.
- **Cases**: one page per case (a tax return, a health-cover request, a bill), with its mail, portal letters, tasks, events, contacts, notes and documents, a timeline, and what comes next. The detailed record lives in your own Markdown files ([case-store.md](case-store.md)).
- **Next**: the one next step, this week's admin windows, and the Gantt chart on demand.
- **Outbox**: drafts, and what each one waits for. Sending is a deliberate key, never automatic.
- **Portals**: the web-only mailboxes of banks, tax offices, health insurers, and Proton without Bridge, each with its own saved login in a sealed browser profile.
- **Calendar and people**: calendars, contacts, the map.

## Admin windows
- **Defaults**: two windows a week, 45 minutes each, at times you choose. Outside them the Porch is closed, and the screen says when the next window opens. Mail keeps syncing in the background.
- **The exception: short-lived secrets.** One-time codes, passwords, password resets, sign-in links and addresses to confirm come from a request you just made, and expire within minutes or hours.
  - Sioul shows them at once, from automatic addresses (no-reply) too: one quiet notification, no sound, the code with a key to copy it, and how long it stays valid. The message does not open the rest of the Porch. After expiry the code is hidden.
  - Fake "your code" messages are a phishing trick: a forged one is set aside, and one from a sender that is not verified comes with a warning to use it only if you just asked that site for it.
- **What you send yourself**, from one of your addresses to another (a file from your phone), comes in any hour, never screened, when it is verified: a forged own address is a classic trick.
- **Optional exception**: a short list of people who may reach you anytime. Empty by default.
- **Opening**: a few sentences, then one card at a time. Small numbers are written as words; there are no badges.
- **Closing**: "Done for today". It shows what got done, and says "nothing else needs you before Friday" when that is true.

## Safe opening
Before the message itself, each card shows:
- **who sent it**: verified, not verified, or forged, with the reason;
- **the case** it belongs to;
- **a plain-language line** about what it is;
- **what it asks**, by when (with any legal deadline extracted), and what happens if nothing is done;
- **a suggested next step**.

From the card, one key does each of:
- **the full text**;
- **"open with the companion"**: an AI explains the message ([ai.md](ai.md));
- **"hand to my helper"**: forward it with the summary to someone who helps you with admin;
- **"set aside"**: back to the Porch, unread.

## Trust
- **Authentication**:
  - DKIM verified at arrival, and the result stored (keys rotate);
  - SPF and DMARC evaluated on the address your own provider received the message from (the trust boundary in the `Received` chain);
  - ARC for forwarded mail;
  - the provider's `Authentication-Results`, trusted only under its own server id (RFC 8601).
  - Version 0 reads the provider's results; independent checks with Stalwart's `mail-auth` come next.
- **Lookalike senders**:
  - a registry of genuine domains, learned from authenticated mail you confirmed, plus a shared starter list for public services and banks;
  - Unicode confusables (UTS #39) and near-spellings.
- **Reputation**: Spamhaus for the sending address, through its free Data Query Service key (its public mirrors refuse queries made through public DNS resolvers), and domain blocklists for the links.
- **The provider's spam verdicts**: SpamAssassin's `X-Spam-*`, rspamd's `X-Spamd-Result`.
- **Your own classifier**, trained by your "junk / not junk" keys: Bayesian at first, a linear SVM once there are a few hundred examples. It always says why.
- **Newsletters**: one-click unsubscribe (RFC 8058).

## Portals
- **An embedded Chromium** (QtWebEngine), one sealed profile per portal, with the session kept.
- **The notification emails** ("you have a new message in your space") become "a letter waits at <portal>" in the right case.
- **"File this page"** saves a portal letter as a dated PDF into its case.
- **No passwords stored by Sioul**: a password manager or the system keyring. No scraping.

## Tasks, the next step, the Gantt chart
As built, with the research it follows: [tasks.md](tasks.md).

- **Tasks are CalDAV tasks (VTODO)** on your server, so they are portable and visible on other devices.
- **The links use RFC 9253** (iCalendar relationships):
  - `RELATED-TO;RELTYPE=FINISHTOSTART` for "this waits for that", with `GAP` for "two weeks after";
  - `DEPENDS-ON` for looser dependencies;
  - `REFID` groups a case's tasks and events;
  - `LINK` points to emails (`mid:` URIs, RFC 2392), contacts and notes.
- **The solver**:
  - it orders tasks topologically (Kahn's algorithm), and shows any loop calmly instead of hiding it;
  - among the tasks free to start, it ranks by latest start (the deadline minus the chain behind it), then by how much each one unblocks, then by effort;
  - it schedules only into your admin windows. If a deadline cannot fit, it says so early and proposes to drop, shrink or hand over.
- **Waiting on others**: a task can wait for an incoming letter. When the letter arrives in its case, the task unblocks.
- **The Gantt chart** is computed from durations and links: soft bars, a today line, deadlines as marks, slack as faint extensions, all driven by keyboard.

## Calendar
- **CalDAV**, and invitations by mail (iTIP/iMIP): accept or decline from the card.
- **`.ics` attachments** open as a preview with any conflicts.
- **Dates found in letters** become proposed deadlines.
- **An event can become an email.**
- **Reminders appear in the window**, not as pop-ups.

## Contacts and the map
- **CardDAV, vCard 4.**
- **Deduplication**: normalise (case, accents, phone numbers in international format, addresses), pair candidates (a shared email or phone is certain; close names are a maybe), merge field by field, with undo and history.
- **The map**: OpenStreetMap, geocoded with Nominatim at one request a second, with a cache. Tiles offline or from OpenFreeMap: the OpenStreetMap tile servers refuse heavy use.

## Notes and drafts
- **Each case has Markdown files** with front matter (`refid`, links), readable in any editor, Obsidian included, and versioned with git.
- **Notion's API** reads and writes Markdown pages.
- **Drafts are Markdown files** whose front matter holds `to`, `cc`, `subject`, `account`, `attach`, `waits-for` and `case`.

## The status line
One sentence about what happened last, with "Undo" for ten seconds; in quiet time, when work comes back, and the way back behind a click ([porch.md](porch.md)); the sound button ([sounds.md](sounds.md)); the weather at a place you chose, in one colour: now, the next four hours, then the parts of the days to come (this evening, tonight, tomorrow morning…), from Open-Meteo, credited there, the place's coordinates (two decimals) the only thing sent, every half hour at most. Health has its own place ([health.md](health.md)).

## Keyboard and senses
- **Everything by keyboard**: a command palette, one key per action, Escape always goes back, key hints on screen, focus always visible, an editable keymap, screen readers through Qt's accessibility layer.
- **Passwords seen when needed**: every password, passphrase and key field has an eye at its end (reached with Tab, named for screen readers) that shows or hides what is typed; Settings ▸ "Show passwords as you type" shows them from the start, on this computer (`qml/PasswordField.qml`).
- **Senses**:
  - a muted palette with even lightness steps (OKLab), no pure white or black;
  - no red for lateness; forged mail is marked by a shape and a word, not an alarm colour;
  - no sounds, badges or counts; reduced motion respected;
  - the layout never reflows when mail arrives;
  - light and dark themes, adjustable text size and spacing, a dyslexia-friendly font as an option.
- **Language**: plain and literal, no idioms. "Waiting", "time left: four weeks", never a red "overdue". Every sentence is translated ([i18n.md](i18n.md)).
- **Low-energy days**: a filter that shows only the small tasks, or "nothing is needed today" when that is true.
