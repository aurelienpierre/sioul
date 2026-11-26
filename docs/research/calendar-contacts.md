# An event's contact and its categories in other calendar programs

**The question** (26 November 2026): an extension's appointment (a salon's booking) should say, in the calendar itself, whom it is with and which card, and what kind of appointment it is (the services booked), in a form other programs read: what does the standard offer, and what do Thunderbird, Nextcloud and DAVx⁵ (Android) do with it?

**How it was read**: the standard's text; each program's source code at its main branch on 26 November 2026, not tried with Sioul. No program was run.

## The standard

- **`CONTACT`** (RFC 5545 §3.8.4.2): "contact information or alternately a reference to contact information associated with the calendar component", a text, any number of times on an event, with an `ALTREP` parameter (§3.2.1), "an alternate text representation for the property value" given as a URI. The RFC's own examples point at an LDAP entry, a MIME part and a vCard file over HTTP. It schedules nothing: no one is invited, no server sends anything (iTIP, RFC 5546, knows `ATTENDEE` and `ORGANIZER`, never `CONTACT`).
- **The card's address**: no standard names a vCard by its UID inside a calendar. vCard 4.0's `UID` (RFC 6350 §6.7.6) is a URI, its example `urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6`; RFC 9562 defines that URN for UUIDs. Sioul writes `urn:uuid:` and the card's UID (the UID as it is when it is a URN already). A card an extension's door makes has a UID of Sioul's own form, and Sioul's own cards `sioul-…`: neither is a UUID, so their address is no valid UUID URN, only a valid URI; other programs keep it as text. Making the door's cards UUIDs would change every card it made, and older devices find them by that UID: left for a decision (handoff).
- **`ATTENDEE`** is what not to write: an attendee makes the event a meeting, and a CalDAV server with scheduling (RFC 6638) sends invitations.
- **`PARTICIPANT`** (RFC 9073 §7, with `PARTICIPANT-TYPE`) describes someone involved without inviting them ([agendas.md](agendas.md)); none of the three programs reads it.
- **`CATEGORIES`** (RFC 5545 §3.8.1.2): a list of texts separated by commas, a comma inside one escaped `\,` (§3.3.11); the property may appear several times.

## The programs

| Program | `CONTACT` | `CATEGORIES` | Read in |
|---|---|---|---|
| **DAVx⁵** (events into Android's calendar) | Not mapped to Android's calendar: kept as an "unknown property", JSON with its parameters (`ALTREP` included), 25,000 characters at most, in `ExtendedProperties`, and sent back on upload. No Android calendar app shows it. | Only the **first** `CATEGORIES` property is read (`getProperty<Categories>`), all its names joined into Android's extended property `categories`; a second line is dropped and never sent back. | bitfireAT/synctools: `mapping/calendar/builder/UnknownPropertiesBuilder.kt`, `ical4android/UnknownProperty.kt`, `mapping/calendar/builder/CategoriesBuilder.kt` |
| **Thunderbird** | Not one of the properties it promotes: kept in the event's property bag with its parameters (`importUnpromotedProperties`) and written back on save, **one value per property name** (a second `CONTACT` is lost when the event is saved). Not shown in the event's dialog or summary. | Each name of a line is a category of its own (its property iterator splits `CATEGORIES:a,b,c` in three: `test_ics_service.js`); shown in the event's dialog and in the list ("a, b, c": `test_unifinder_utils.js`); written back one line per name. | mozilla comm-central: `calendar/base/src/CalItemBase.sys.mjs`, `CalICSService.sys.mjs`, tests above |
| **Nextcloud** (its calendar app) | Not shown: the app's event model reads no contact (`src/models/event.js`); its library has one (`getContactList`, a text property with its parameters) that the app does not use. The editor changes the parsed event in place, so the line survives a save. The server stores the object as it is. | Shown as the event's tags, every name of every line (`getCategoryIterator`). | nextcloud/calendar `src/models/event.js`; nextcloud/calendar-js `src/components/root/abstractRecurringComponent.js` |

## What follows for Sioul

- **`CONTACT` with `ALTREP="urn:uuid:<UID>"`, one per event**: every program checked keeps it, none shows it, none sends an invitation for it. The person's name as the service writes it goes in its text, so that a program that some day shows `CONTACT` says whom; never the card's own name, which may be the person's private word for them in a calendar others read. One line only, since Thunderbird keeps one.
- **The tie is Sioul's to read**: any event or task carrying such an address that a card here has is tied to that card, whoever wrote it; an address no card has is no tie.
- **All categories on one line**, the extension's name first: DAVx⁵ reads the first line alone, Thunderbird and Nextcloud read every name of a line.
- **Not checked**: Apple's Calendar, Evolution, KOrganizer, Google's CalDAV (which may drop unknown properties; the door's hash, kept on the device too, keeps it from writing the event again at each refresh).
