---
description: Contacts in Sioul - names first, details folded; duplicates found and merged when you say so, each merge undone for thirty days; the map of everyone, placed only when you allow it; who reaches you, and when; compared with other address books.
---

# Contacts and the map

## In short {#in-short}

Contacts holds the people you deal with: their names first, their details folded until you open them. Sioul finds the same person written twice and merges the cards when you say so, with thirty days to undo each merge. It shows everyone with a postal address on one map, once you allow it. Address books are made, renamed, merged and removed from the page's ⚙; many contacts at once are moved, given a group or imported, each change undone as a whole. Each person is in a circle (Safe, Your contacts, Restricted, Everyone else for strangers, or Blocked; a few are also Always through), and that circle decides when their mail, their calls and their messages reach you; a forged message cannot take their place. Your contacts stay on your own server, at Google or on this device, and Sioul reaches them only over an encrypted connection. At the head of the page, your people: those your admin is done for, each with their papers.

<figure markdown="span">
  [![The Contacts page: your people in a row with Add a person, then a search field, buttons for a new contact, the map, the duplicates, a selection and an import, and the filters above a list of names, each with an address or a number under it; one contact open on the right, with their role and organisation, an e-mail address with Write, a phone number with Call, "Your contacts: in your address book, on no list.", This is one of your people… and How Iris Calloway reaches you, the client profile with Bill this client, then what is tied to them: two tasks and two events.](../assets/screens/contacts.png){ loading=lazy }](../assets/screens/contacts.png "Open the picture at full size")
  <figcaption>A contact: how to reach them, and what involves them.</figcaption>
</figure>

## Protected by default {#what-is-protected}

- Your contacts stay where you keep them: on your own contacts server (a Nextcloud, say), at Google, or on this device. Sioul has no server of its own, and sends your contacts nowhere else.
- Sioul reaches your server only over an encrypted connection, and refuses one that is not. Your password stays in your system's keyring, never in a file.
- The map sends nothing until you allow it. Then it sends postal addresses alone, one at a time, to OpenStreetMap's geocoder, and an address it cannot read to one or two other address services ([The map](#the-map)): never a name, a number or an e-mail address.
- Nobody can borrow a place on your lists. Mail that shows a safe contact's address but fails that address's checks counts as a stranger's, and mail forged outright is set aside. Someone you blocked stays blocked, whatever their message proves.
- Who is on which list, and the calls your phones screened, travel between your devices sealed with your passphrase: the server of the shared folder never reads them.
- Your people stay on your own devices, sealed when they travel: never written into a contact card, never sent to a server, each closed to AI agents until you open them ([Your people](#household)).

## The list

Names come first, with one line under each, and a search field sits on top: a few letters of a name, an organisation, an e-mail address or a phone number find a contact. The search does not look into postal addresses.

Above the list, filters narrow it, each kept on this device until you change it: the address books shown (all, one or several, once you have more than one), a group or **In no group**, and whether a card has or lacks a phone number, an e-mail address, a postal address or a title (Mr, Mrs…). They combine with the search, and the row says how many show: "132 of 840". **Show all** takes the filters off; the search stays until you clear it, and is never kept on disk. On a phone, the filters fold behind one **Filters** button, which says how many are set ("Filters (2)"), with **Show all** and the count beside it: the first contact comes right after one row.

A title the card keeps in its own field shows greyed before the name.

## Your people {#household}

At the head of the page, **Your people** is a row of names: you first, then the people your admin is done for, a child, a parent you help, a partner, family, someone you help, and after a small gap a child's other parent. A name opens that person's page, in the place of a card; on a small screen it takes the page. Right click, or a long press on a touch screen, gives **Open**, **Rename** and **This is not a person**. **Add a person** at the end of the row adds one.

When Sioul makes people from what your agendas and papers already named (a child's agenda, a paper's holder), the Porch says once who it made, and from what, in your admin hours; there is nothing to answer.

A person is not a contact. A contact is a card for reaching someone; a person is who your papers are for. A baby is a person with no card; someone with a home card and a work card is one person with two.

**A person's page** shows their name and one line, such as "Child · 12 · lives with you", then:

- **Papers**: the papers that are theirs, each with where it stands, as on the [Papers](papers.md#papers) page, and **Add a paper for Léa**;
- **Agendas**: their agendas, a child's with **Who does what**; for a child's other parent, the children's agendas they share;
- **Letters**: the letters addressed to them;
- **Money**: the budgets for them and the sites you open for them;
- **Contacts**: the cards that are them, each opening its card, with **Unlink**, and **Link a card…**;
- at the foot, quiet: **What Sioul keeps about Léa…**, one PDF of their papers, agendas and letters and a folder with the papers' files, saved where you choose, for you to hand to them (their budgets and the sites you open for them are not in it); **Ends…**, for anyone but a child: the day it ended, and for a partner, their own papers saved in a folder to hand back, then taken out of your papers if you say so; **Same as…**, to make two persons one ("Léa" and "Léa M."); **This is not a person**, for a name read from a paper that is nobody ("Banque"), never made again;
- **Open to AI agents**, off until you turn it on. Your own page has no such switch: you are open as before.

On a child's page, once each and in your admin hours, a line says what passes to them: at 16, that they can have a carte Vitale and a reimbursement account in their own name; at 18, that their papers are their own, with **What Sioul keeps about Léa…** beside. A paper whose name for someone names nobody here offers **Make a person of "…"** on its card.

**Edit** changes their name, how papers write it ("Léa Martin"), who they are to you, their date of birth, whether they live with you, and for a child's other parent, whose. A new name is also written into the papers that named the old one, so that a device on an older Sioul shows it too.

**On a contact's card**, **This is one of your people…** links the card to one of them, or makes a new person of it. Once linked, the button opens their page instead.

**Where they come from.** The first time, Sioul reads the names your agendas and papers already gave: a child's agenda gives that child, and its other parent; a paper "for Odile" gives Odile. A paper with no name, or with yours, is yours. These persons count from then on; a wrong one is changed on its page. Nothing else is written.

## A contact

A contact opens on the right; on a phone, it takes the page:

- under its name, its role and its organisation, then its **categories**, small labels: a click on one shows that category's list;
- its **e-mail addresses**, each with **Write**, and its **phone numbers**, each with **Call**, which hands the number to your phone, or on a computer to the program that answers phone links;
- on an office's card, beside its numbers, its **Opening hours**: what is true now, its hours day by day, and **Look them up…**, **Change**, **Call back…** ([Opening hours](#opening-hours), below); on another card, under **More**;
- **How they reach you**: who they are to you, in a sentence ("Your contacts: in your address book, on no list."), and their sheet ([below](#how-they-reach-you));
- a small map with a pin at their address, once it is placed ([The map](#the-map));
- folded under **More**: their postal addresses, birthday, notes and web sites, then the address book the card is in. **More** shows only on a card that has one of the first four. A card of a book you can only read offers neither **Edit** nor **Delete**, and its address book cannot be changed there;
- beside each postal address, **Directions**, which opens the way there as an event's place does ([Agenda](agenda.md#status-map-directions)): in your maps app when this system has one for `geo:` links, with the point the map placed it at when it has one, else on OpenStreetMap's page, or Apple's on a Mac;
- what is **tied to it**: the tasks that name them, the events they are invited to, the notes that mention them, and what you tied to them by hand. A message shows there when you tied it to them, and so does each message they sent you that Sioul proved comes from them; the [mail search](mail.md#searching) finds the rest, what you sent them among it.
- **Bill this client**, which opens a draft invoice for them in Budgets ▸ Invoices, and **Client profile…**: what their invoices print (a business, a person or a public body, the legal name, the addresses, a SIREN or a VAT number, the language, currency and terms of their invoices), kept among Sioul's own lists beside your projects, never in the card on your contacts' server. Once made, the profile shows in one line above them, "Client profile: A business · Belgium · BE0123456789 · invoices in French"; with none yet, the button reads **Make its client profile…**, and starts from the card ([Your clients](time.md#your-clients)).

**To change it**, **Edit**, then **Save**: it is changed in place. Under the name, **Title, given names, family name** opens the name's parts: its title (Mr, Mrs…), given names, family name and suffix, which other programs sort and address by. Parts you type are kept as you typed them; a card that gives none gets them from its name, a title at its start going to the title. On Android, **Back** leaves the form without saving. **To add one**, **New contact** above the list (a **+** alone beside an open card, and on a phone), or **New ▾ ▸ A contact**. It goes into the first address book that can be written, in the order Contacts ⚙ lists them; to keep it elsewhere, move it once saved. From a message, **Add to contacts** makes the sender a contact in one click.

**To delete one**, **Delete** under its card, beside **Edit**, or right click on its name in the list (a long press on a touch screen), then **Delete**: the card closes, and the contact waits ten seconds, with **Undo**. A card of an address book you can only read is never changed nor deleted: its card offers neither, and its row's menu says so.

**To move one** to another address book, across accounts too, choose it under **More ▸ Address book** on its card, which shows the book it is in. When the other place would not keep something (Google keeps less than an open server), or when the move changes who may reach you, Sioul says what, and asks before moving. To move many, or a card that shows no **More**, select them ([Many contacts at once](#many-at-once)).

When you write a message, addresses are completed from your contacts.

## Opening hours {#opening-hours}

An office's hours are written once, on its card, beside the number you call, and every task that involves that office follows them.

- **Where**: on the card of an organisation (a town hall, a tax office, a practice, a shop), or of any card that has hours, the section **Opening hours** comes right under its numbers; on another card, it is under **More**, with **Add opening hours** and one link, **This is an office, a shop or a practice**, which marks the card as an organisation's, as other programs read it (its name written as its organisation, or vCard 4.0's `KIND:org`); everything else on the card stays as it was. Marked by mistake, the card is put back as it was with **Undo** in the status line, or later, for thirty days, under **Done lately** in Contacts ▸ **Duplicates**.
- **What it says**: in one line, what is true now ("Open until 12:00", "Closed: opens Monday at 9:00", "Closed for the public holiday: opens tomorrow at 9:00"); then its hours day by day ("Monday to Friday: 9:00–12:00, 14:00–17:00"), and "Closed on public holidays.", the holidays Sioul knows (Settings ▸ Hours), or, when Sioul knows none yet, where to add their calendar; under **By phone**, when it answers the phone at other hours, those; its free line, for what Sioul cannot count ("Closed in August, by appointment on Thursdays"), shown and never counted; and where the hours come from: "Typed by you on 20 November 2026", or "From the administration's directory (service-public.gouv.fr), checked on…", or "From OpenStreetMap (© OpenStreetMap contributors), checked on…". Hours checked more than a year ago say so, quietly, and offer **Look them up again…**.
- **Change** (**Add opening hours** on a card without them) opens the week to fill in, a line per day, empty when closed, or the same typed short above it: days, then hours, "mo-fr 9-12, 14-17; sa 9-12" (days in two letters, in English or in French, "lu-ve 9-12, 14-17", or their full names; hours as "9", "9:30" or "9h30", each range after the one before); under it, what Sioul reads. A day Sioul cannot count keeps what you typed in its field, says so under it, and goes into the free line when you save. "Di" is Sunday among French days and Tuesday among German ones: written beside English days, Sioul does not guess, and says so. **Open on public holidays**, unticked, keeps the office closed on the public holidays Sioul knows; "24/7" ticks it. **Answers the phone at other hours** adds those, typed the same way. What Sioul cannot count goes into the free line when you save; nothing typed is dropped. Saved with its hours unchanged, the card is not written.
- **Look them up…** is offered only on a card that says it is an organisation's, or that already holds hours or a free line (typing them is your word that it is an office's). It opens what would be sent, the office's name and where it is, which you can correct, and to whom: the administration's directory (service-public.gouv.fr) for an office whose address is in France (a postal code with "France" after it, or your region France), and the map's search service, OpenStreetMap's unless you chose another in the Contacts settings. With the words go Sioul's name and version, and to the map the window's language. Nothing leaves before you press **Look up**; then one question goes to each, both at once. The answer is a short list, **Is it this one?**, each office with its address and hours, its source named. **This one** writes its hours on the card, with their source and the day, and "Open on public holidays" when the source says so; what it says that Sioul cannot count is added to the free line. A number or an address the card lacks is offered after (**Add its number … to the card**), never written by itself. Nothing found changes nothing, and a lookup never takes hours off a card. A person's card offers no lookup: a person's name is never sent anywhere. Google is not a source: its terms do not let hours it gives be kept on a card; hours read on its page can be typed in the short form.
- **Call back…** makes the task "Call ‹name›" at once: a call, that office attached, proposed when it answers. One press, no form; a second press at once makes nothing more. On a card that is not an office's and has no hours, it is a plain call, proposed at any time.
- **The tasks that follow them**: a task that needs an open office follows the hours typed on it, else those on the card of the first office it involves that has some (the phone's for a call), else offices' usual hours (Tasks ⚙). Changing the card changes every task of that office. A call or an outing given an office whose card has hours is ticked **Needs an open office** by itself, and so are the calls and outings already tied to an office when you first give its card hours on this device (typed or looked up; never when the card arrives from another device, where the tick was already made or not); untick it and it stays so. An unticked task says that it does not follow its office's hours, with **Follow them** ([Tasks](tasks.md#offices-have-hours)).
- **An office abroad**: its hours are read on your device's clock, and the public holidays are those of your own calendars of public holidays; for an office in another country or time zone, type its hours as they fall on your clock, and its closing days in the free line.
- **Where they are kept**: on the card, in lines of Sioul's own that other programs and an older Sioul keep as they are. A card of an address book you can only read shows its hours, and says they cannot be written there. Google's address books may not keep them: moving such a card there says so first.

## How they reach you {#how-they-reach-you}

Everyone is on one list, and the same list holds for their mail, their calls and their messages in other apps:

- **Safe**: family, friends, chosen colleagues. Only you put someone there.
- **Your contacts** (once "Neutral"): anyone in your address books, until you choose otherwise.
- **Restricted**: people you would rather hear from only at times you choose.
- **Blocked**: never, on any channel. Their mail is set aside for good, and their calls are declined.
- **Strangers** ("Everyone else" in What reaches you): everyone in none of your address books and on no list. Their first message waits on [the Porch](porch.md#the-lanes), in the screener, until you let them in.

Your choice for the person comes first; then their categories, when a list names one of them ("Friends" are safe); then their address book. **How … reaches you** opens their sheet: their list and why, **Their list** to choose it (**As their groups say**, or one of the four lists), **Always through** for the few who should always get through, what reaches you from them at each time of your day, **Their own times** for their calls and their texts, their calls of the month, rung or declined, and their messages of the week when a phone of yours shares them ([A person's sheet](notifications.md#a-persons-sheet)). When each list's mail and messages reach you is yours to change, in Settings ▸ [What reaches you](notifications.md#by-person); calls are set for each phone, in Settings ▸ Devices ([Each phone its own calls](calls.md#calls-per-phone)). By default, for example, the mail of one of your contacts comes in your working and admin hours, and a stranger's call goes to your voicemail once a phone of yours screens calls.

## Categories

Categories are the groups Nextcloud Contacts shows: "Family", "Friends", "Neighbours"; a card can have several. In the form, under **Categories**, × takes one off, and the field after them adds one, chosen among those your cards already have or typed. "amis" and "Amis" are one category, written as your cards first wrote it. The filters, the changes to many contacts and What reaches you call them groups: they are the same thing.

They are saved in the card itself, so Nextcloud, your phone and other programs see them, and a card saved in Sioul keeps those it had. A list can name a category (Settings ▸ [What reaches you](notifications.md#who-is-on-which-list), under **Groups**): its people then reach you as that list says, by mail, by phone and through other apps' messages, unless you chose otherwise for them on their card. Nothing goes on a list by itself, family and friends included.

## Address books {#address-books}

An address book is where cards are kept, and with whom they are shared: one on your Nextcloud, Google's, one on this device alone. Contacts ⚙ ▸ **Address books** lists each with where it is kept, how many contacts it holds, whose it is ("Yours", "Shared with you by Sam") and whether it can be written. A line opens to what may be done with it; what you choose or type in a line stays while a sync redraws the list:

- **New address book…**: on an account whose server makes them, or on this device alone. It is made on the server at the next sync.
- **Rename**: here, and on the server at the next sync, with **Undo**.
- **Merge into…**: first it asks, saying how many contacts go into which book, what that book would not keep, and who sees it ("“Professional” is shared with office"). Then every contact moves into the other book, and the emptied book is removed, here and on its server once a whole answer of its server shows it empty. A book someone added contacts to on its server meanwhile is kept, and shows again here with them, saying why. **Undo** moves them all back, the book made again when it was on this device alone.
- **Export…**, on a computer: its contacts as one vCard file, where you choose: the copy to keep before a large change, or for another program. **With them… ▸ Export…** does the same for a selection. A phone's file picker gives no file to write, so a phone does not offer it.
- **Remove…**: an empty book goes at once. A book holding contacts asks where they go: moved to another book (asked as a merge is), or deleted with it, a copy of them kept on this device first. The count is this device's: on its server, the book goes for everyone it is shared with, and one that holds contacts added there since the last sync is kept and shows again here. **Undo** brings it back while its server has not deleted it; after that, the copy is there to import.
- **Share**, on a Nextcloud: a user or a group of that server, to read or to change contacts; the line says who has it, and **Stop** takes it back.
- **Stop showing**, for a book someone else shares with you: it leaves this device, and stays theirs.
- **Its groups decide who may reach me**: on for a book only you can write, and for one whose server says neither whose it is nor with whom it is shared (its line says so: "This server does not say who can write this book"); off, until you turn it on, for a book someone else can write (one you share with others to write, or someone else's), because someone who can write it could otherwise add a card in a group that a list lets through. Off, its contacts stay yours, their groups are shown and filtered as ever, and a card's own place on a list still counts; only its groups are held back from the lists. When a book's writers change (shared to write here or elsewhere, a writer added or taken off), a notice says once that its groups stopped deciding (one notice naming every book when several stop at once), and its line says it too, with how to turn them on. **Add your safe senders**, under the Always through list ([Always through](notifications.md#always-through)), reads groups the same way. A group typed on a card of such a book says, under the groups, that it does not decide who may reach you there, with the way to the switch.

Google keeps one address book: Sioul says so on its line and offers nothing it would refuse. Contacts move into it and out of it like any other.

## Many contacts at once {#many-at-once}

**Select**, above the list, gives each row a box; **Select the 132 shown** takes all that the filters show. Another filter or another search keeps ticked only the cards still shown, and **Done** puts the boxes away. On a phone, the bar keeps to one row: the count is itself a box, which takes all that the filters show, or none once they are all ticked. Then **With them…**:

- **Move to an address book…**: each card keeps its identifier, so that nothing that points at it changes (a task, a person, a list naming it), and two cards with one identifier never land in one book. The question says who sees that book. Where its server has its own move, the card is moved there, a change not yet sent going first; else it is copied, and the old copy deleted only once the new one is there. A card someone changed on the server before your move reached it is kept in both books, as you left it and as they changed it, and said.
- **Add to a group…** and **Remove from a group…**: a group in use, or a new one typed.
- **Move the title into its field…**: a title at the start of a name (M., Mr, Mme, Mlle, Dr, Me, Pr, Mrs, Ms, Miss, Monsieur, Madame…) goes into the card's title field, and the name shown loses it. Each card shows its name before and after, with its own box: ticked when the word cannot be anything but a title (a full stop after it, or Mme, Mlle, Mrs, Ms, Miss followed by a name), unticked when it may be part of a name or a company ("Me", "Pr", "M" without a full stop, "Dr" on a company's card); you tick what you want. A name of two people ("M. et Mme Dupont", "Mr and Mrs Smith") is never changed, and said. The rest of the name's line (its other names, its parameters) is kept as it is. **Also add them to a group** gives them a group in the same step.
- **Export…**, on a computer: the cards selected as one vCard file, where you choose ([Address books](#address-books)). It changes no card.
- **Delete…**

Each says how many it will change before it runs, and why some cannot ("its address book can only be read"). It runs with its progress and a **Stop**, and ends by naming what it could not do, card by card. A card changed since it was counted is left as it is, and said. **Undo** in the status line takes the whole change back for ten seconds; then Contacts ⚙ ▸ Changes to many contacts lists those of the last thirty days, each with **Undo…**, asked first, or why it can no longer be undone. Undo puts back each card the change touched while it is as the change left it; a card changed, moved or deleted since stays as it is, and is named.

## Importing contacts {#importing}

**Import…**, above the list (an icon: its name shows when the pointer rests on it), reads a vCard file (`.vcf`, as Google, a phone or a Nextcloud writes it) or a CSV file (`.csv`, as a spreadsheet or an address book exports it). On a computer, a `.vcf` file opened with Sioul goes to the same window (a `.csv` file through "Open with"). On Android, open the file with Sioul from where it is (Files, a message): the phone's file picker gives no file Sioul can read, and **Import…** says so there. The file is read on this device; nothing is sent anywhere. A file of more than 32 MB is not read. The window says how it read it: its encoding, and any limit reached (20,000 cards or rows, 200 columns, a card of more than 2 MB, a cell of more than 64 KB), a quote never closed, a row with more cells than columns, each in a sentence.

- **For a CSV file, how its columns are separated** (semicolons, commas or tabs: found by its first line, and yours to change), then **which column is what**: each column is offered a field (title, given names, middle names, family name, the whole name, suffix, organisation, role, mobile, phone, e-mail address, street, postcode, town, region, country, the whole postal address, birthday, note, groups, web site) or **Leave out**, guessed from its heading in French or English and shown with its first values. Google's and Outlook's exports are recognised. The columns can be kept under a name, kept when you press **Import**, and come back by themselves for a file with the same headings.
- **Where they go**: an address book that can be written, or a new one made in the same step, and groups added to every new contact (to those already here too, only when you tick it).
- **Those already here** are found by their identifier, or by the same name with the same e-mail address or number, two different titles being two people ("M. Martin" and "Mme Martin"): never written twice. The same number or address under another name is another person (a household, a switchboard), imported as their own card and counted. A card whose identifier is here under a clearly different name is listed as "same identifier, another name", left alone, with its own box to complete it all the same. A contact already here is left exactly as it is: the count lists each one with what the file would add to it, and **Complete them with what the file adds**, off at first, adds it. Nothing it holds is ever replaced. Two cards with the same name alone are both kept, for [Duplicates](#duplicates) to offer. The groups the file gives its new contacts are listed with how many; one of them that one of your lists reads is kept only when you tick "Also keep the groups that decide who may reach you"; the same box governs what "Complete them" gives a contact already here.
- **Count them** says what will happen before anything is written: how many rows were read as contacts, the new ones, those already here, those found twice in the file, those imported apart, numbers rewritten, birthdays that read two ways (kept as written in the card's note), and the lines that do not read. **Import** runs it with its progress, undone as a whole with **Undo**.

Numbers written without their country are written with it ("+33 6…"), by the country set in the Contacts settings, when you tick it (ticked at first for a CSV file); the count says how many. A title column goes to the title field; a title at the start of a name moves there too when you tick it, where it cannot be anything but a title. A spreadsheet's own `.xlsx` file is not read: save it as CSV first.

## Duplicates

**Duplicates**, above the list, looks for two things, and changes nothing until you click:

- **A number or an e-mail address written twice on one card.** "04 65 71 12 34" and "+33 4 65 71 12 34" are one number: spaces, dots and the country's prefix aside. Each card concerned is listed with what would go, ticked; **Take the duplicates off** keeps one of each, the one written with its country, with what the others said of it (mobile, work).
- **Two cards that may be one person**: the same name (in any order, case and accents aside), the same number, or the same e-mail address. They come one pair at a time, side by side, with what they share. Choose the name kept, then **Merge**: one card keeps everything of both (numbers, e-mail and postal addresses, web sites, categories, notes, and the photo, organisation and birthday of the name kept when it has them), and the other is deleted, here and on the server. **Not the same** keeps them apart and never asks again on this device; **Later** shows the next pair.

When the card kept sits in a book whose groups decide who may reach you and the other in one whose groups do not (someone else can write it), the groups only the other carries are left out: the merge names them, with one box, unticked, to take them too, and says what taking them would change.

**Done lately** lists what was cleaned and merged, each with **Undo**, for thirty days: the cards come back as they were, here and on the server. Contacts kept on this device only are handled the same way.

!!! note "Numbers written without their country"
    "04 65 71 12 34" has no country: Sioul reads it as a number of the country set in the Contacts settings, by default your system's (France for French). This only serves to compare numbers: your cards keep them as they are written.

## The map

**Map**, above the list, shows everyone with a postal address on one map, a pin each; a pin opens the card, and **List** brings the list back.

To be placed, an address has to be turned into a point on the map. Sioul does not do it by itself: the first time, it asks.

!!! note "Placing your contacts"
    **Place them** sends your contacts' postal addresses to OpenStreetMap's geocoder (Nominatim), once each, one a second. An address it cannot read (a venue's name before its street) is asked once of the Base Adresse Nationale when your region is France, then of Photon, which tolerate words they don't know. Nothing else goes with them. The places found are kept on this device, and each address is asked only once.

The map images come from OpenStreetMap, fetched sparingly and kept. You can give another source in the settings.

## On Android {#on-a-phone}

On Android, Sioul syncs your address books itself and keeps its own copy of them, apart from the device's own contacts. With your leave (Settings ▸ Devices, this phone's card, **What Android must allow** ▸ **Contacts**), it reads the device's own contacts, and only for three things: to name a caller your address books do not know, to recognise who wrote in another app's notification, and to see which of your Always through people are starred for Android's do-not-disturb. It never writes a contact, a name or a number: for someone who should be starred, it opens their contact in your Contacts app, or that app's own form, filled, for you to save. It sets a star itself, and nothing else, only once you turn on **Starred contacts are Always through, both ways**, under your Always through list (off until you do; Android then asks whether Sioul may modify your contacts): a contact you star on the phone joins the list on every device, and everyone on the list is starred where their contact is. A favourite on a server that keeps favourites (a Nextcloud) counts the same way ([Always through](notifications.md#always-through)). When the phone screens your calls ([Calls](calls.md)), a blocked person's calls are declined before the phone rings.

## The Contacts settings

The ⚙ above the list, after its buttons, has three parts, **Address books**, **Changes to many contacts, the last thirty days** and **Map and numbers**:

- **Address books**: each with where it is kept, how many contacts, whose it is and whether it can be written; made, renamed, merged, removed and shared there ([Address books](#address-books)).
- **Changes to many contacts**: those of the last thirty days, the newest first, each with what it did and when, and **Undo…** or why it can no longer be undone ([Many contacts at once](#many-at-once)).
- **Place contacts on the map**: on or off.
- **The map's search service**: where Sioul asks where an address is (the contacts' map, an event's map) and where an office is and its hours (**Look them up…**), only when you ask: by default OpenStreetMap's (Nominatim); another address (`https://…`), or **None**, which stops both. OpenStreetMap's rules ask that an application can be moved off its service, or stopped, without an update: Sioul is never switched from afar, by design, so this setting is that switch; it travels with your settings to your other devices. When the service asks Sioul to wait (it answers 403 or 429), Sioul says so once and asks it nothing more that day, for both uses.
- **Map tiles**: where map images come from, as `https://…/{z}/{x}/{y}.png`. Empty: OpenStreetMap's.
- **Country for phone numbers written without one**: the country "04 65 71 12 34" belongs to, to find the same number written "+33 4 65 71 12 34". By default, your system's, when it is one of the 34 countries and territories whose numbering Sioul knows; else such numbers are compared as they are written, until you choose a country.

## Where contacts live

On your contacts server, as standard cards: your phone and other programs see them. Editing in Sioul changes only what you changed: a photo or another program's fields stay as they were. When a card was changed both here and on the server between two syncs, the server's version is kept, and yours is set aside among the earlier versions (**Settings ▸ Sharing ▸ Show earlier versions**, under **Your calendars and contacts**), on Android too, to put back if you prefer it; the status line says so.

Google contacts work the same way, with less kept: Sioul writes them as Google reads them, and says what would be lost before moving a contact there.

## Not there yet

Changing a contact's photo in Sioul is planned. Sioul makes no calendar of your contacts' birthdays, and keeps no bin for deleted contacts beyond **Undo**: ten seconds in the status line, thirty days for a change to many at once (Contacts ⚙ ▸ Changes to many contacts).

## Going further {#going-further}

- **When two cards are taken for one person**: a number, an e-mail address or a name that more than four cards share (a switchboard, an office, a common name) says nothing of one person, so it makes no pair. The pairs that share most come first. Cards you can only read are never cleaned nor merged.
- **What a merge keeps**: the card of the name kept stays as it is, and takes from the other what it lacks: numbers, e-mail and postal addresses, web sites, chat addresses, categories, nicknames, public keys; the notes of both, joined; the other's photo, organisation and birthday only when the kept card has none; and the other program's fields it holds, unless the kept card has the same.
- **Undo after a sync**: when a card changed after its cleaning or merge (a sync, an edit), its new version is set aside in a file before the older one is written back, so nothing is lost either way.
- **Who decides a person's list**, the most precise first: their own address or number on a list; their card; their categories; a domain or the start of a number (`*@example.org`, a switchboard's first digits); then Your contacts, when they are in an address book or you let them in. At the same level, blocked comes before restricted, your contacts and safe.
- **A card's photo** is read from the card itself. A card whose photo is a web address (`https` only) never has it fetched by itself: the card offers **Load the photo**, naming the server. Pressed, Sioul asks that server once (twenty seconds and one megabyte at most, a picture only) and keeps the copy here, which the card shows from then on.

## Why it works this way {#why-it-works-this-way}

**Books for keeping and sharing, groups for deciding.** An address book says where a card is kept and with whom it is shared; a group says what the person is to you (a client, a supplier, family). Only groups, with your own choices, decide who reaches you, never a book (one exception, about trust: the groups of a book someone else can write count only once you say so for that book, since they could otherwise add a card in a group that a list lets through): sorting contacts between books whose groups count never changes who can reach you, and a card moved to another book keeps its identifier, its groups and its list. A move, a merge, a group given or an import that crosses into or out of a book whose groups are held back says, with counts, what it changes before it runs. Groups travel inside each card to every app, and to Google, which keeps a single address book; so the same rule holds wherever your contacts are.

A person is not a card. A card is for reaching someone, and every app of your phone reads it, as does the server that keeps it; a baby has no card, and one person may have two. So a person is Sioul's own record, kept on your devices, linked to the cards that are them.

Nothing is written into your cards. vCard has a line for relations (RFC 6350), but no address book keeps it as a link that every app follows: Google keeps a name and a type, Apple a labelled name, Nextcloud a typed text, and Google's contacts server has no place for it at all. Written into a card, "Léa: child" would be read by every app that reads your contacts, and the people closest to us are the ones who can misuse such details: abusive partners turn ordinary family features into watching (Freed et al. 2018; Levy & Schneier 2020).

Each person is closed to AI agents until you open them. Keeping your family's papers on your own devices is a household matter, which data protection law leaves to you (GDPR art. 2(2)(c)); what an agent reads goes to the company that runs its model, outside your household, and children's data deserve particular protection (GDPR recital 38). So an agent sees a person's papers only once you open that person, and nobody's health papers but yours.

Sioul watches nobody: no location, no "last seen", no reading of anyone's messages, no tally of what was done for whom. Children rated parental monitoring apps one star in three reviews out of four, and more control went with more risk, not less (Ghosh et al. 2018); adult children wanted sensors and tracking more than their mothers did (Berridge & Wetle 2020). Care ends in the answer of the one cared for (Tronto 1993): Sioul keeps nothing about someone that you could not show them.

More in [what the research says](../dev/research.md), findings 55 to 60.

## Compared with other apps {#compared-with-other-apps}

**Finding duplicates is not Sioul's alone.** Google Contacts suggests merges under "Merge & fix". Apple Contacts looks for duplicates on the Mac, and on iPhone and iPad finds cards with the same first and last name. KAddressBook finds cards with the same name or nickname, or a shared e-mail address, and Proton Contacts offers to merge cards with the same name. What Sioul does its own way:

- **It merges the cards themselves, on the server that keeps them**, your own or Google's, so that your phone and every other app then see one card. KAddressBook does too; GNOME Contacts and Fossify only show two cards as one, in their own window.
- **It compares numbers by each country's numbering plan**, so that "04 65 71 12 34" and "+33 4 65 71 12 34" are one number, where GNOME Contacts compares their last seven digits.
- **It leaves out what many cards share**: a number, an e-mail address or a name on more than four cards makes no pair.
- **It undoes each merge on its own, for thirty days**, here and on the server, where Google, and Apple on iCloud.com, can only put the whole address book back to an earlier moment, and KAddressBook, Proton and Nextcloud describe no undo.

As of October 2026, from each app's own documentation.

✓ documented; **partly**, with a note; ✗ not found in the app's own documentation (for Sioul: not built); ? not confirmed; — not applicable. Sioul's column was checked against its code.

=== "Everyday"

    | | Sioul | Google | Apple | Outlook | Thunderbird | DAVx⁵ + Fossify |
    |---|---|---|---|---|---|---|
    | Finds duplicates and merges them when you say so | ✓ | ✓ | ✓ | partly¹ | partly² | partly³ |
    | Takes "04 65 71 12 34" and "+33 4 65 71 12 34" as one number | ✓ | ?⁴ | ? | ? | partly⁵ | partly⁶ |
    | Undoes a merge later | ✓⁷ | partly⁸ | partly⁹ | ?¹⁰ | ✗ | —¹¹ |
    | Shows your contacts on a map | ✓ | partly¹² | partly¹² | ? | ✗¹³ | partly¹⁴ |
    | Groups that follow you to your other devices | ✓ | partly¹⁵ | ✓ | ✓¹⁶ | partly¹⁷ | ✓ |
    | Blocks someone on every channel at once | ✓¹⁸ | partly¹⁹ | ✓²⁰ | partly²¹ | partly²¹ | partly²² |
    | Lets chosen people through at chosen times | ✓²³ | partly²⁴ | ✓ | partly²⁵ | ✗ | partly²⁶ |
    | Lists the tasks, events and notes tied to the person | ✓ | partly²⁷ | ? | partly²⁸ | ✗²⁹ | ✗ |
    | Lists the mail you exchanged with them | partly³⁰ | partly²⁷ | ? | ✓ | partly²⁹ | ✗ |
    | Gets a deleted contact back | partly³¹ | ✓ | partly⁹ | ✓ | ✗ | ✗ |
    | Imports and exports vCard files | ✓³² | ✓ | ✓ | partly³³ | ✓ | ✓ |
    | Puts your contacts' birthdays in your calendar | ✗³⁴ | ✓ | ✓ | ✓ | ✗ | ✓³⁵ |
    | For other people: the people your admin is done for | ✓³⁶ | partly³⁷ | partly³⁸ | ? | ? | ? |

    1. New Outlook and Outlook on the web hide a card that is an exact copy or a subset of another, without asking and without merging; classic Outlook asks only when you save or import a duplicate.
    2. Only with the Duplicate Contacts Manager add-on, made by someone else: you edit or delete one card of each pair.
    3. Fossify shows cards with the same name as one entry, without asking; the stored cards are not merged.
    4. Google keeps each number's international form, but does not say whether it finds duplicates by it.
    5. With the add-on of note 2, once given a country code; numbers are compared, never rewritten.
    6. On a contact's screen, numbers that end with the same nine digits are shown once.
    7. Each cleaning and each merge on its own, for thirty days.
    8. "Undo changes" puts the whole address book back to a moment of the last 30 days, undoing every later change.
    9. No undo of a merge and no bin are described; iCloud.com can put back a whole earlier copy of your contacts.
    10. New Outlook merges nothing; classic Outlook describes no undo of its "Update information of selected contact".
    11. Nothing is merged in storage: turning the setting off shows the cards apart again.
    12. One contact's address opens in Google Maps, or in Apple's Maps; a map of all your contacts is not described.
    13. "Get Map" went away with the new address book of Thunderbird 102; the request to bring it back is open.
    14. A postal address opens in the map app you installed.
    15. Labels show in Gmail and in Google's Android app; Google's CardDAV does not carry them to other apps.
    16. Within Outlook's apps on the same Microsoft account.
    17. Mailing lists, in address books kept on the computer only; a server's groups are not shown yet.
    18. Their mail is set aside, their messages in other apps never come, and their calls are declined on an Android phone that screens calls.
    19. Android's Contacts app blocks a contact's calls and texts; Gmail blocks mail apart, and neither applies to the other.
    20. One block covers Phone, FaceTime, Messages and Mail, on every device of the same Apple Account.
    21. Mail only, through a block list or a filter; it handles no calls.
    22. Calls and texts, through Android's blocked numbers; no mail.
    23. Mail on every device; calls and other apps' messages on an Android phone that runs Sioul.
    24. Android's modes let chosen people's calls and messages through; mail is apart.
    25. Its phone app mutes its notifications on a schedule; letting chosen people through is not described.
    26. Android's do-not-disturb can let starred contacts through.
    27. For work and school accounts only: mail conversations and meetings with people of your organisation.
    28. The profile card in Outlook's phone app shows the events you share.
    29. A button searches the contact's mail, another makes an event with them; the card lists nothing.
    30. The messages they sent you that Sioul proved are theirs, and those you tied to them; what you sent them, through the mail search.
    31. **Undo** for ten seconds after **Delete**; contacts deleted through a selection come back for thirty days (Contacts ⚙ ▸ Changes to many contacts); no bin.
    32. vCard and CSV files are imported, those already here found by their identifier or their name and left as they are unless you ask to complete them; a book or a selection is exported as one vCard file, on a computer.
    33. Classic Outlook opens and saves single vCards; new Outlook and the web import and export CSV only.
    34. Sioul makes no birthday calendar; one your server keeps (Nextcloud keeps one) shows as any other calendar you can only read.
    35. In Fossify Calendar; Etar has none.
    36. Persons kept by you, on your devices, linked to their papers and their cards, each closed to AI agents until opened ([Your people](#household)).
    37. A contact's "Relation" keeps another person's name and a type, as text; Family Link makes a child's account part of a family group, whose parent sees its location.
    38. "Related names" are labelled names that link to no card; Family Sharing's organizer sees members' Screen Time, and members share their location unless they turn it off.

=== "Technical"

    | | Sioul | Google | Apple | Outlook | Thunderbird | DAVx⁵ + Fossify |
    |---|---|---|---|---|---|---|
    | Any CardDAV server, read and write | ✓ | partly¹ | ✓ | ✗² | ✓ | ✓ |
    | Google contacts, read and write | ✓ | — | partly³ | partly⁴ | ✓⁵ | ✓⁶ |
    | Keeps groups inside each card, where other apps find them | ✓ | ✗⁷ | ✗⁷ | partly⁸ | ✗⁹ | ✓¹⁰ |
    | Contacts encrypted end to end | ✗¹¹ | ✗ | ✗¹² | ✗¹³ | ✗ | ✗ |
    | Free software | ✓ GPL-3.0+ | ✗ | ✗ | ✗ | ✓ MPL-2.0 | ✓ GPL-3.0 |

    1. Other apps reach Google's contacts by CardDAV; Google Contacts cannot add another server's address book.
    2. Microsoft lists no CardDAV among the accounts Outlook adds, and says Outlook for Mac does not support it.
    3. Read and write on iPhone and iPad; read only on the Mac.
    4. New Outlook and Outlook's phone app copy a Gmail account's contacts into Microsoft's cloud; classic Outlook imports a CSV export.
    5. Through Google's CardDAV and Google's sign-in; labels do not come through.
    6. Through Android's own Google account, or through DAVx⁵, which calls it "not officially supported"; labels do not come through.
    7. Groups are objects of their own (Google's labels, Apple's groups), not words in each card.
    8. Categories are kept on each contact, in Microsoft's own format.
    9. A server's groups and categories are not shown yet.
    10. DAVx⁵ keeps groups as categories in each card, or as cards of their own, as you choose for each account.
    11. Contacts on a server can be read by that server, as with every CardDAV app. What Sioul shares between your devices (who is on which list, the calls screened, the address books kept on a device) is sealed.
    12. Not with Advanced Data Protection either: Apple says CardDAV has no end-to-end encryption built in.
    13. Encrypted on the way and on Microsoft's disks, with keys Microsoft holds.

**Other address books.** GNOME Contacts suggests linking two cards that share a number, an e-mail address or a similar name, remembers a "no", and lets you unlink them at any time; it leaves both cards as they are in their address books. Nextcloud Contacts merges two cards you pick, side by side, field by field, and keeps groups in each card as Sioul reads them; it does not look for duplicates by itself. Proton Contacts encrypts every field of a card end to end, except the name and the e-mail addresses, and has no CardDAV.

??? info "Sources"
    - Google Contacts: merging duplicates ("Merge & fix"), <https://support.google.com/contacts/answer/7078226>, read 8 October 2026.
    - Google Contacts: deleting, restoring and blocking contacts, and undoing changes, <https://support.google.com/contacts/answer/7280886>, read 8 October 2026.
    - Google Contacts: contacts on Android, grouped and labelled, <https://support.google.com/contacts/answer/30970>, read 8 October 2026.
    - Google Maps: a contact's address on the map, <https://support.google.com/maps/answer/3131570>, read 8 October 2026.
    - Google Account: blocked users, and what a block covers, <https://support.google.com/accounts/answer/6388749>, read 8 October 2026.
    - Gmail: blocking a sender, <https://support.google.com/mail/answer/8151>, read 8 October 2026.
    - Android: modes and do-not-disturb, <https://support.google.com/android/answer/9069335>, read 8 October 2026.
    - Phone by Google: Call Screen, <https://support.google.com/phoneapp/answer/9118387>, read 8 October 2026.
    - Google Workspace Updates: the contact page for work accounts (June 2021), <https://workspaceupdates.googleblog.com/2021/06/update-google-contacts-experience.html>, read 8 October 2026.
    - Google Calendar: birthdays, <https://support.google.com/calendar/answer/6084659>, read 8 October 2026.
    - Google People API: CardDAV, <https://developers.google.com/people/carddav>, read 8 October 2026.
    - Google People API: the person resource (numbers' international form), <https://developers.google.com/people/api/rest/v1/people>, read 8 October 2026.
    - Google People API: contact groups, <https://developers.google.com/people/api/rest/v1/contactGroups>, read 8 October 2026.
    - Apple Contacts for Mac: merging contact cards, <https://support.apple.com/guide/contacts/merge-contact-cards-adrbk1456/mac>, read 8 October 2026.
    - iPhone User Guide: merging or hiding duplicate contacts, <https://support.apple.com/guide/iphone/merge-or-hide-duplicate-contacts-iph2ab28320d/ios>, read 8 October 2026.
    - iCloud User Guide: restoring contacts, <https://support.apple.com/guide/icloud/restore-contacts-mm1d9cfdb498/icloud>, read 8 October 2026.
    - Apple Contacts for Mac: a contact's address on a map, <https://support.apple.com/guide/contacts/show-a-contacts-address-on-a-map-adrb183385bb/mac>, read 8 October 2026.
    - Apple Personal Safety User Guide: blocking calls and messages, <https://support.apple.com/guide/personal-safety/block-calls-and-messages-ipsac1e87c54/web>, read 8 October 2026.
    - iPhone User Guide: Focus, allowing people, <https://support.apple.com/guide/iphone/allow-or-silence-notifications-for-a-focus-iph21d43af5b/ios>, read 8 October 2026.
    - iPhone User Guide: screening and blocking calls, <https://support.apple.com/guide/iphone/screen-and-block-calls-iphe4b3f7823/ios>, read 8 October 2026.
    - Apple Calendar for Mac: holidays and birthdays, <https://support.apple.com/guide/calendar/show-holidays-and-birthdays-iclead4e0ec3/mac>, read 8 October 2026.
    - Apple Contacts for Mac: accounts and what they let you change, <https://support.apple.com/guide/contacts/update-contact-information-adrbk1515/mac>, read 8 October 2026.
    - Apple Developer: CNGroup, <https://developer.apple.com/documentation/contacts/cngroup>, read 8 October 2026.
    - Apple: iCloud data security overview, <https://support.apple.com/en-us/102651>, read 8 October 2026.
    - Outlook: managing duplicate contacts, <https://support.microsoft.com/en-us/Outlook/people/manage-duplicate-contacts-in-outlook>, read 8 October 2026.
    - Outlook on the web: contacts and lists, <https://support.microsoft.com/en-us/outlook/using-contacts-people-in-outlook-on-the-web>, read 8 October 2026.
    - Outlook: blocking senders, <https://support.microsoft.com/en-us/Outlook/mail/block-or-unblock-senders-in-outlook>, read 8 October 2026.
    - Outlook: notifications on a phone, <https://support.microsoft.com/en-us/outlook/how-can-i-turn-push-notifications-and-sounds-on-or-off>, read 8 October 2026.
    - Microsoft 365: profile cards, <https://support.microsoft.com/en-us/Outlook/profile-cards-in-microsoft-365>, read 8 October 2026.
    - Outlook.com: contacts and contact lists, <https://support.microsoft.com/en-us/outlook/create-view-and-edit-contacts-and-contact-lists-in-outlook-com>, read 8 October 2026.
    - Outlook: restoring a deleted contact, <https://support.microsoft.com/en-us/Outlook/people/restore-or-recover-a-deleted-contact-in-outlook>, read 8 October 2026.
    - Outlook: importing vCards, <https://support.microsoft.com/en-us/outlook/import-vcards-to-outlook-contacts>, read 8 October 2026.
    - Outlook for Mac: iCloud calendars, and no CalDAV or CardDAV, <https://support.microsoft.com/en-us/outlook/sync-your-icloud-calendar-with-outlook-for-mac>, read 8 October 2026.
    - Outlook: accounts synced through the Microsoft cloud, <https://support.microsoft.com/en-us/Outlook/getstarted/sync-your-account-in-outlook-to-the-microsoft-cloud>, read 8 October 2026.
    - Microsoft Purview: encryption in Microsoft 365, <https://learn.microsoft.com/en-us/purview/encryption>, read 8 October 2026.
    - Outlook: a birthday calendar, <https://support.microsoft.com/en-us/outlook/calendar/add-a-birthday-calendar-and-reminder-in-outlook>, read 8 October 2026.
    - Thunderbird: bug 259531, merging duplicate contacts (open since 2004), <https://bugzilla.mozilla.org/show_bug.cgi?id=259531>, read 8 October 2026.
    - Thunderbird add-ons: Duplicate Contacts Manager, <https://addons.thunderbird.net/en-US/thunderbird/addon/duplicate-contacts-manager/>, read 8 October 2026.
    - Thunderbird: bug 94407, undo for a deleted card, <https://bugzilla.mozilla.org/show_bug.cgi?id=94407>, read 8 October 2026.
    - Thunderbird: bug 1781076, "Get Map" in the new address book, <https://bugzilla.mozilla.org/show_bug.cgi?id=1781076>, read 8 October 2026.
    - Thunderbird: bug 1764184, a CardDAV server's groups, <https://bugzilla.mozilla.org/show_bug.cgi?id=1764184>, read 8 October 2026.
    - Thunderbird Support: blocking a sender, <https://support.mozilla.org/en-US/kb/blocking-sender>, read 8 October 2026.
    - Thunderbird Support: junk mail and your address books, <https://support.mozilla.org/en-US/kb/thunderbird-and-junk-spam-messages>, read 8 October 2026.
    - Thunderbird blog: the address book of Thunderbird 102, <https://blog.thunderbird.net/2022/05/7-great-new-features-coming-to-thunderbird-102/>, read 8 October 2026.
    - Thunderbird: bug 151994, a birthday calendar from the address book, <https://bugzilla.mozilla.org/show_bug.cgi?id=151994>, read 8 October 2026.
    - Fossify Commons: ContactsHelper.kt, same-name cards shown as one, <https://github.com/FossifyOrg/Commons/blob/main/commons/src/main/kotlin/org/fossify/commons/helpers/ContactsHelper.kt>, read 8 October 2026.
    - Fossify Contacts: issue 607, merging by name only, <https://github.com/FossifyOrg/Contacts/issues/607>, read 8 October 2026.
    - Fossify Contacts: ViewContactActivity.kt, numbers compared on nine digits, <https://github.com/FossifyOrg/Contacts/blob/main/app/src/main/kotlin/org/fossify/contacts/activities/ViewContactActivity.kt>, read 8 October 2026.
    - Android Developers: BlockedNumberContract, <https://developer.android.com/reference/android/provider/BlockedNumberContract>, read 8 October 2026.
    - Android Developers: NotificationManager.Policy, <https://developer.android.com/reference/android/app/NotificationManager.Policy>, read 8 October 2026.
    - Fossify Calendar: its strings ("Add contact birthdays"), <https://github.com/FossifyOrg/Calendar/blob/main/app/src/main/res/values/strings.xml>, read 8 October 2026.
    - Etar: issue 234, birthdays, <https://github.com/Etar-Group/Etar-Calendar/issues/234>, read 8 October 2026.
    - DAVx⁵: tested with Google, <https://www.davx5.com/tested-with/google>, read 8 October 2026.
    - DAVx⁵ manual: technical information (groups), <https://manual.davx5.com/technical_information.html>, read 8 October 2026.
    - KAddressBook: its page, <https://apps.kde.org/kaddressbook/>, read 8 October 2026.
    - KAddressBook: the duplicate search's code, <https://invent.kde.org/pim/kdepim-addons/-/blob/master/kaddressbook/plugins/mergelib/job/searchpotentialduplicatecontactjob.cpp>, read 8 October 2026.
    - KAddressBook Handbook: the command reference, <https://docs.kde.org/stable_kf6/en/kaddressbook/kaddressbook/commands.html>, read 8 October 2026.
    - Proton: merging contacts, <https://proton.me/support/merge-contacts>, read 8 October 2026.
    - Proton: Proton Contacts and its encryption, <https://proton.me/support/proton-contacts>, read 8 October 2026.
    - Proton: adding and importing contacts, <https://proton.me/support/adding-contacts>, read 8 October 2026.
    - GNOME Help: linking and unlinking contacts, <https://help.gnome.org/gnome-help/contacts-link-unlink.html>, read 8 October 2026.
    - GNOME Contacts: contacts-store.vala, <https://gitlab.gnome.org/GNOME/gnome-contacts/-/blob/main/src/contacts-store.vala>, read 8 October 2026.
    - GNOME folks: phone-details.vala, numbers compared on seven digits, <https://gitlab.gnome.org/GNOME/folks/-/blob/main/folks/phone-details.vala>, read 8 October 2026.
    - GNOME Contacts: issue 145, linking two cards of one Nextcloud address book, <https://gitlab.gnome.org/GNOME/gnome-contacts/-/issues/145>, read 8 October 2026.
    - Nextcloud user manual: Contacts, <https://docs.nextcloud.com/server/latest/user_manual/en/groupware/contacts.html>, read 8 October 2026.
    - Nextcloud Contacts: issue 5619, finding duplicates, <https://github.com/nextcloud/contacts/issues/5619>, read 8 October 2026.

## For technical readers {#for-technical-readers}

- **vCard**: Sioul reads vCard 4.0 (RFC 6350) and 3.0 (RFC 2426), through calcard, and writes new cards in 3.0. An edit rewrites only the lines that changed: an unchanged card comes back byte for byte, and Apple's grouped labels (`item1.X-ABLabel`), photos and other programs' fields are kept. Categories are the card's `CATEGORIES` lines, as Nextcloud Contacts writes its groups. Groups kept as cards of their own (vCard 4's `KIND:group`, Apple's `X-ADDRESSBOOKSERVER-KIND`) show as cards, not as categories ([Works with](compatibility.md#calendars-tasks-and-contacts)).
- **CardDAV** (RFC 6352, over WebDAV, RFC 4918): discovery by `/.well-known/carddav` (RFC 6764) and the current user's principal (RFC 5397); changes fetched with the sync token (RFC 6578), else by ETags, in `addressbook-multiget` batches of 50; writes with `If-Match`, so nothing is overwritten silently, and `If-None-Match: *` for a new card. When both sides changed a card, the server's version wins and yours is kept among this device's earlier versions, to put back from the window. An interrupted sync resumes.
- **Transport**: HTTPS only (an `http://` address is refused), TLS by rustls with the system's root certificates. During discovery, a redirect to another domain is not followed, so your password never goes to a host your domain merely points at. Basic authentication, over TLS only.
- **Google contacts** go through Google's own CardDAV, signed in with OAuth 2.0 for native apps (RFC 8252, a loopback redirect) and PKCE (RFC 7636): the refresh token in the keyring, the access token in memory only. Cards are written as vCard 3.0 for Google: labels, birthdays without a year and newer fields are lost there, which Sioul says before a move.
- **Secrets**: passwords in the Secret Service (KWallet, GNOME Keyring), the macOS Keychain, the Windows Credential Manager, or Android's KeyStore. Passwords never travel between your devices: an account shared from another device asks for its password, which the server checks before this device's keyring keeps it.
- **Phone numbers** are compared by an E.164 key, built from a table of the numbering plans of 34 countries and territories (the trunk prefix, the international prefix, the numbers' lengths, the French overseas departments), not by libphonenumber. Short codes ("112", "3631") and numbers with letters are compared as written. Values are never rewritten.
- **Duplicates**: names are compared with case, accents, ligatures (ß, æ, œ) and punctuation folded and their words sorted; a name, number or e-mail address shared by two to four cards makes pairs, ranked by how many kinds of evidence they share. "Not the same" is kept by UID in `$XDG_STATE_HOME/sioul/contacts-not-the-same.toml`. A merge keeps the kept card's single-valued properties (`ORG`, `TITLE`, `BDAY`, `PHOTO`, `TZ`, `GEO`…) and adds the multi-valued ones it lacks (`ADR`, `URL`, `IMPP`, `RELATED`, `NICKNAME`, `KEY`, `CALURI`…); a vCard 4.0 line going into a 3.0 card is converted. Every change is first recorded, with the cards before and after, in `$XDG_STATE_HOME/sioul/contacts-undo`, for thirty days; a failed write rolls back the ones before it.
- **Who someone is**: their own address or number on a list, then their card (`contact:<UID>`), then their categories, then the most precise pattern (`*@example.org`, `tel:+3319900*`), then Your contacts (`neutral` in the files) when they are in an address book or were let in, else a stranger; at the same precision, blocked before restricted, your contacts and safe. The lists are plain text files; numbers are matched by their E.164 key.
- **Proof before lists**: a message whose SPF and DKIM both fail, with no DMARC pass, is judged a stranger's whatever list its address is on, unless an ARC seal (RFC 8617) from your own provider or domain vouches for it once forwarded. A DMARC failure under a `quarantine` or `reject` policy is judged forged and set aside before that. Blocked is checked first and always holds. A name read in another app's notification can only lower someone's standing, never raise it, since anyone can pick any name in a chat.
- **Geocoding** follows Nominatim's usage policy: a User-Agent that names Sioul, one request every 1.1 seconds, over HTTPS only (redirects included), the address alone (its lines joined), at the address of the map's search service (`[map] search`, Contacts ⚙; `none` stops it; after a 403 or a 429, nothing more that local day, kept in `$XDG_STATE_HOME/sioul/map-search-wait`); an address it finds nothing for is asked once of the Base Adresse Nationale (region France), then of Photon, their first answer kept (`sioul_sync::geocode::locate`); the answers, found or not, are kept in `$XDG_CACHE_HOME/sioul/places.toml`. The map is Qt Location's OpenStreetMap plugin, with Qt's online lookup of providers turned off; the full map loads tiles only once at least one address is placed. The tile server, as with any web map, sees which areas you look at.
- **Showing a card**: a card comes from a server or from anyone's vCard, so it is shown without markup: notes as plain text, web sites as links only when they are `http` or `https`, their text escaped. An embedded photo is kept in the cache, named by its content.
- **Storage**: one `.vcf` file per card in vdir folders (`~/.local/share/sioul/contacts`), which khard and vdirsyncer read too; the sync's state is kept apart. Sioul does not encrypt them on the disk: your disk encryption does. Sioul's data, state and cache folders are made, or narrowed, to mode 0700 at each start on Linux and macOS, so that other accounts on the computer cannot read them; on Android, they are in the app's private storage.
- **Address books**: made by an extended MKCOL (RFC 5689) at the next sync, renamed by PROPPATCH, deleted by DELETE (a book emptied by a merge only once its server holds none of its cards); a card moved between two books of one server by WebDAV's MOVE (RFC 4918 §9.9) with `If-Match` and `Overwrite: F`, else sent anew with `If-None-Match: *` and its old copy deleted once the new one has its ETag. Shared through Nextcloud's sharing (`oc:share`, `oc:invite`), as agendas are. Every writer of a card goes through one door that refuses a book its server makes read only.
- **Many at once**: a batch keeps the whole plan, each card as it was, in `$XDG_STATE_HOME/sioul/contacts-batches/` before it runs (thirty days), then changes one card after the other; a note beside it says each step done. Its Undo takes back each step that was done while its card is as the step left it, and leaves the others. The filters are kept in `$XDG_STATE_HOME/sioul/contacts-view.toml` (never the search), the import's kept columns in `contacts-import.toml` beside it. Titles are data (`crates/sioul-core/data/titles.toml`), and go to `N`'s honorific prefix (RFC 6350 §6.2.2); `TITLE`, the job, is not touched.
- **Import**: vCard 2.1, 3.0 and 4.0 read by calcard (folded lines, quoted-printable, charsets); a 2.1 card is written as 3.0, the others as they come, each keeping its UID or given one. CSV: the byte-order mark dropped, UTF-16 when it says so, UTF-8 else Windows-1252, the separator found among semicolon, comma and tab (or chosen), quoted cells with line breaks; the headings' table is data (`contact-columns.toml`). At most 32 MB, 20,000 cards or rows, 200 columns, 2 MB a card, 64 KB a cell. A card is the one here with its UID, or with its name and an address or a number.
- **Your people**: `persons.toml` in Sioul's data folder (`~/.local/share/sioul/`), beside the ties, never in the notes folder, a card or a contacts server ([the design note](../dev/persons.md)).
- **Sealed sharing**: each record is encrypted with XChaCha20-Poly1305, under a key made from your passphrase by Argon2id and kept in each device's keyring; the shared folder's server sees which device wrote, when and how much, never what. The part **Senders** carries the four lists, the senders let in, Always through and others' public keys; **Calls** carries the screened calls, never a blocked number's; **Lists kept here** carries the address books kept on a device only; **Settings and accounts** carries your people, each field of each person apart, so that a name changed on one device and a date of birth added on another both stay. Contacts on a server are not in the sharing: they travel by their server ([Sharing](sharing.md#what-it-protects-and-what-it-cannot-hide)).
- **Android**: Sioul syncs address books into its private storage, not into Android's contacts provider. It asks for `READ_CONTACTS`, and for `WRITE_CONTACTS` only once you turn on starred contacts both ways (`[dnd] stars`, off unless you turn it on): it then writes one column, `Contacts.STARRED`, of a contact that exists, and never creates, edits nor deletes a contact. On a server that keeps a favourite per card (Nextcloud's `favorite` property), the same switch sets that property on the cards of your own books, and nothing else, once the server has answered one as its own. Calls are screened as Android's "Caller ID & spam app" (`CallScreeningService`), from a table Rust writes ahead and Java reads in milliseconds, without the network; a call that cannot be decided rings.
