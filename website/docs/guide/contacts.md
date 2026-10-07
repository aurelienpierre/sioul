---
description: Contacts in Sioul - names first, details folded, Write and Call beside each address, categories as Nextcloud's groups, duplicates found and merged on your click, and the map of everyone, placed only when you allow it.
---

# Contacts and the map

Contacts are names first. Details stay folded until you open them.

<figure markdown="span">
  [![The Contacts page: a search field, a button for a new contact and one for the map above a list of names, each with an address or a number under it; one contact open on the right, with their role and organisation, an e-mail address with Write, a phone number with Call, "Their mail" set to Neutral, a small map with a pin at their address, and what is tied to them: two tasks and two events.](../assets/screens/contacts.png){ loading=lazy }](../assets/screens/contacts.png "Open the picture at full size")
  <figcaption>A contact: how to reach them, where they are, and what involves them. <small>Map © <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors.</small></figcaption>
</figure>

## The list

Names, with one line under each, and a search field on top: a few letters of a name, an organisation, an address or a number find them.

When your cards have categories, a choice under the search field shows the contacts of one category only, or **All categories**.

## A contact

A contact opens on the right:

- its **categories**, small labels under its name: one shows the list of that category;
- its **e-mail addresses**, each with **Write**;
- its **phone numbers**, each with **Call**;
- folded under **More**: postal addresses, organisation and role, birthday, notes, web sites;
- **How they reach you**: who they are to you in a sentence ("Neutral: in your address book, on no list."), and **How … reaches you**, their sheet: their list, for all their addresses and numbers, on any card, one with a number only too; **Always through**; and what reaches you from them at each time ([A person's sheet](notifications.md#a-persons-sheet)). Anyone in your address books is neutral until you choose; their categories can decide, when a list names one of them; their own choice comes first: safe, neutral, restricted or blocked, or back to **As their categories say**;
- a small map with a pin at their address, once it is placed (below);
- what is **tied to it**: the mail exchanged, tasks, events, notes, projects.

**To change it**, **Edit**, then **Save**: it is changed in place. On a phone, **Back** leaves the form without saving. **To add one**, the **+** above the list, or **New ▾ ▸ A contact**. From a message, **Add to contacts** makes the sender a contact in one click.

**To delete one**, **Delete**: it waits ten seconds, with **Undo**.

**To move one** to another address book, across accounts too, choose it under **Address book** on its card. When the other place would not keep something (Google keeps less than an open server), Sioul says what, and asks before moving.

When you write a message, addresses are completed from your contacts.

## Categories

Categories are the groups Nextcloud Contacts shows: "Family", "Friends", "Neighbours"; a card can have several. In the form, under **Categories**, × takes one off, and the field after them adds one, chosen among those your cards already have or typed. "amis" and "Amis" are one category, written as your cards first wrote it.

They are saved in the card itself (vCard's `CATEGORIES`), so Nextcloud, your phone and other programs see them, and a card saved in Sioul keeps those it had. A list can name a category (Settings ▸ [What reaches you](notifications.md#who-is-on-which-list), "Your contacts' categories"): they then reach you as that list says, by mail, by phone and through other apps' messages, unless you chose otherwise for them on their card. Nothing goes on a list by itself, family and friends included.

## Duplicates

**Duplicates**, above the list, looks for two things, and changes nothing until you click:

- **A number or an address written twice on one card.** "04 65 71 12 34" and "+33 4 65 71 12 34" are one number: spaces, dots and the country's prefix aside. Each card concerned is listed with what would go, ticked; **Take the duplicates off** keeps one of each, the one written with its country, with what the others said of it (mobile, work).
- **Two cards that may be one person**: the same name (in any order, case and accents aside), the same number, or the same address. They come one pair at a time, side by side, with what they share. Choose the name kept, then **Merge**: one card keeps everything of both (numbers, addresses, web sites, categories, notes, and the photo, organisation and birthday of the name kept when it has them), and the other is deleted, here and on the server. **Not the same** keeps them apart and never asks again; **Later** shows the next pair.

**Done lately** lists what was cleaned and merged, each with **Undo**, for thirty days: the cards come back as they were, here and on the server. Contacts kept on this device only are handled the same way.

!!! note "Numbers written without their country"
    "04 65 71 12 34" has no country: Sioul reads it as a number of the country set in the Contacts settings, by default your system's (France for French). This only serves to compare numbers: your cards keep them as they are written.

## The map

The map button above the list shows everyone with a postal address on one map, a pin each; a pin opens the card.

To be placed, an address has to be turned into a point on the map. Sioul does not do it by itself: the first time, it asks.

!!! note "Placing your contacts"
    **Place them** sends your contacts' postal addresses to OpenStreetMap's geocoder (Nominatim), once each, one a second. Nothing else goes with them. The places found are kept on this device, and each address is asked only once.

The map images come from OpenStreetMap, fetched sparingly and kept. You can give another source in the settings.

## The Contacts settings

The ⚙ at the top of the page:

- **Address books**: renamed here and on the server at the next sync. An empty address book can be deleted; one holding contacts stays.
- **Place contacts on the map**: on or off.
- **Map tiles**: where map images come from, as `https://…/{z}/{x}/{y}.png`. Empty: OpenStreetMap's.
- **Country for phone numbers written without one**: the country "04 65 71 12 34" belongs to, to find the same number written "+33 4 65 71 12 34". By default, your system's.

## Where contacts live

On your contacts server, as standard CardDAV cards: your phone and other programs see them. Editing in Sioul changes only what you changed: a photo or another program's fields come back as they were. When a card was changed both here and on the server between two syncs, the server's version is kept, yours is set aside in a file, and the status line says where.

Google contacts work the same way, with less kept: Sioul writes them as Google reads them, and says what would be lost before moving a contact there.

## Not there yet

Changing a contact's photo in Sioul is planned.
