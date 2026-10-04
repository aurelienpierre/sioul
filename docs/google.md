# Google: calendars, contacts and tasks

Google keeps calendars and contacts behind CalDAV and CardDAV, like an open server, but signs you in with OAuth only, keeps less of what you write, and keeps tasks elsewhere (Google Tasks). Sioul reads and writes all three. What Google does not keep shows greyed, never hidden, with why.

## Signing in
Google takes no password from another program, not even an app password for CalDAV or CardDAV since 14 March 2025: OAuth only. In Accounts ▸ Add an account, "Google calendars, contacts and tasks":

1. Your address. With Sioul's own key in the build (below), nothing else; otherwise the ID and secret of **your own** Google key ("Use a Google key of my own").
2. "Sign in with Google" opens Google's page in your browser, not in Sioul (Google refuses sign-ins inside an application's browser). Sioul waits on this computer for Google's answer (`http://127.0.0.1:<random port>`, RFC 8252), five minutes at most; Cancel stops it.
3. Google's answer is checked: the same request (`state`), the same program (PKCE, RFC 7636, S256), the same address (the ID token's `email`; another account signed in in the browser gets its access sent back to Google, and nothing changes).
4. What stays: the client ID, its secret, the refresh token and the day it was given, as one entry in the system keyring (`google:<address>`); the access token, in memory, an hour at most, asked again when it ends or is refused (once). Nothing in a file.

Sioul asks every scope at once (Google adds none later to a desktop app): `openid email`, `…/auth/calendar`, `…/auth/carddav`, `…/auth/tasks`. Unticking tasks on Google's page leaves tasks out; the rest works.

When Google ends the access (a password change, six months unused, the key deleted), the account says "Google asks you to sign in again" and stops trying; "Sign in again" opens Google's page with the key kept. When it ends about a week after signing in with a key of your own, it says why: Google ends every sign-in after seven days while the project is in testing, and tells how to publish it.

Removing the account gives the access back to Google (`/revoke`) and empties the keyring entry.

## Sioul's own key: how Thunderbird does it
Thunderbird ships its own Google key: an OAuth client Mozilla registered with Google, verified by Google, its ID and secret written in Thunderbird's sources (`mailnews/base/src/OAuth2Providers.sys.mjs`, with a note asking others to register their own). Nobody using Thunderbird makes a key; they sign in. Google's documentation for installed applications says as much: the client secret of a desktop application is embedded in it and is not treated as a secret; PKCE, not the secret, proves the program asking.

Sioul does the same once its maintainer has registered a key for it, and reads it at build time: `SIOUL_GOOGLE_CLIENT_ID` and `SIOUL_GOOGLE_CLIENT_SECRET` in the environment of `cargo build` (a release workflow's secrets, not the repository). A build without them asks for a key of your own, as before.

**Registering Sioul's key, once, by its maintainer:**
1. A Google Cloud project for Sioul; *APIs & Services → Library*: **CalDAV API**, **CardDAV API**, **Google Tasks API**.
2. *Google Auth Platform*: *Branding* (Sioul's name, logo, a homepage and a privacy policy on a domain you own, verified in Google Search Console; authorised domains); *Audience*, External; *Data access*: the three scopes above. *Clients → Create client → Desktop app*.
3. *Audience → Publish app*. From then on, sign-ins no longer end after seven days, even before verification: Google shows "Google hasn't verified this app" (*Advanced → Go to Sioul*) and lets at most 100 people sign in.
4. *Verification Center*: calendar, contacts (CardDAV) and tasks are **sensitive** scopes, not restricted ones (Gmail's are): no paid security assessment, but a written reason for each scope, a demonstration video of the sign-in and of what each scope is used for, and the privacy policy saying how Google data is used (it stays on the person's computer; Sioul has no server). Google answers in about three to five working days.
5. Build Sioul with the two values. Everyone else then signs in with one click.

What the privacy policy must say, at least: which Google data Sioul reads and writes (calendars, contacts, tasks of the account signed in), that it stays on the person's computer and in their own Google account, that nothing goes to the maintainer or anyone else, how to revoke the access (Accounts ▸ Remove, or myaccount.google.com/permissions), and that the use complies with Google's API Services User Data Policy, Limited Use requirements included.

## Your own Google key
Without Sioul's key in the build, or if you prefer yours, you make one, free, in about fifteen minutes (the same steps are in the Accounts page):

1. On [console.cloud.google.com](https://console.cloud.google.com), make a project named Sioul.
2. *APIs & Services → Library*: turn on **CalDAV API**, **CardDAV API** and **Google Tasks API** (not "Google Calendar API" or "People API": those are other doors).
3. *Google Auth platform*: *Branding*, a name and your address; *Audience*, External, yourself as a test user; *Data access*, the scopes `https://www.googleapis.com/auth/calendar`, `https://www.googleapis.com/auth/carddav` (typed by hand) and `https://www.googleapis.com/auth/tasks`.
4. *Clients → Create client → Desktop app*. Copy its ID and its secret at once: Google shows the secret only at creation.
5. *Audience → Publish app*. Left in testing, Google ends every sign-in after seven days. No verification is needed for your own use (under 100 users). A sign-in made while the project was in testing still ends on its seventh day: sign in again once after publishing.
6. In Sioul, paste both and sign in. Google says the app is not verified: it is yours; *Advanced → Go to Sioul*.

Google deletes a client unused for six months; Sioul uses it at every sync.

## Calendars (CalDAV)
- Found from the address: the principal `https://apidata.googleusercontent.com/caldav/v2/<address>/user`, then its calendar home.
- Which calendars come is chosen on Google's page [calendar.google.com/calendar/syncselect](https://calendar.google.com/calendar/syncselect).
- **Not from Sioul**: making, renaming or deleting a calendar (no MKCALENDAR, no PROPPATCH of names): greyed in the settings, with why. Google's calendars hold no task: greyed in a task's list.
- New events go without `If-None-Match` (Google takes `If-Match` only); where Google files them (its `Location`) is where they are known, and Google's version comes back at the next pull.
- Google adds your calendar's default reminders to every event it receives, and shifts floating times; both come back as Google keeps them.

## Contacts (CardDAV)
- Found from `https://www.googleapis.com/.well-known/carddav`.
- Written as vCard 3.0 (`contacts::as_vcard3`): what only 4.0 has goes (gender, anniversary, relations, kind), `PREF` becomes `TYPE=pref`, a `data:` photo is written inline. Moving a contact into a Google address book says first what would be lost (labels, a birthday without its year, vCard 4.0 fields).
- A new contact: PUT, else POST to the address book (RFC 5995) when the PUT is refused; Google gives it its own name and UID.
- No new address book from Sioul (greyed).

## Tasks (Google Tasks)
Google's CalDAV refuses tasks; its task lists come over the Google Tasks API (`google_tasks.rs`), each kept here as a folder of VTODO files like any list (`calendars/<account>/tasks-<id>`), so the rest of Sioul reads them as it reads a CalDAV list.

- **Kept by Google**: the title, the notes, done or not (and when), a day (Google's "due" is the day it should be done; no hour), one level of steps.
- **Not kept, greyed in the task form** (with "This list is on Google Tasks: what Google does not keep is greyed"): a start day, a length, a case, a kind, office hours, categories, repeating, waiting for another task, steps of a step. Moving a task to a Google list says first what it would lose.
- What Google does not know stays in the file here (an alarm, a link written by another program) but is not sent.
- Lists can be made, renamed and deleted from Sioul (Google Tasks allows it); a list deleted here that holds tasks there (added from a phone meanwhile) comes back.
- Sync: what changed here first (inserted, patched, moved under its parent, deleted), then what changed there since the last sync (`updatedMin`, deletions included), everything the first time. Changed on both sides between two syncs: yours is sent, and Google's answer is what stays.

## What was tested
- The sign-in's pieces: the page asked (PKCE challenge, state, scopes, login hint), the answer read on the loopback port (a favicon request first, Cancel), the ID token's address.
- Google Tasks against a stand-in of its API (`tools/google-tasks-stand-in.py`, pages of two): first sync with a step under its task, edits both ways, a new step under its parent, deletions, a list made, renamed and deleted here, a list deleted there.
- **Not tested against Google itself** (no Google account was used): the token exchange, Google's CalDAV and CardDAV answers (discovery, whether a PUT of a new contact is refused or relocated, its `Location`), Google Tasks' own answers. The first real sign-in is the test; what fails says so in the account's line.

## Files
- `crates/sioul-sync/src/google.rs`: the sign-in, the tokens, the revocation; Sioul's own key, read at build time (`built_in`), and the seven-day end of a project in testing, told as such (`IN_TESTING`).
- `crates/sioul-sync/src/dav.rs`: the bearer token, Google's discovery and its quirks.
- `crates/sioul-sync/src/google_tasks.rs`: Google Tasks.
- `crates/sioul-core/src/capabilities.rs`: what each kind of server keeps, for the greyed fields and the warnings before a move.
