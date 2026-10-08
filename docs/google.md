# Google: calendars, contacts, tasks and mail

Google keeps calendars and contacts behind CalDAV and CardDAV, like an open server, but signs you in with OAuth only, keeps less of what you write, and keeps tasks elsewhere (Google Tasks). Sioul reads and writes all three. What Google does not keep shows greyed, never hidden, with why. Gmail and Google Workspace mail come over IMAP and SMTP, with an app password or a sign-in of their own ([Mail](#mail)).

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
4. *Verification Center*: calendar, contacts (CardDAV) and tasks are **sensitive** scopes, not restricted ones (Gmail's are): no paid security assessment, but a written reason for each scope, a demonstration video of the sign-in and of what each scope is used for, and the privacy policy saying how Google data is used (it stays on the person's device; Sioul has no server). Google answers in about three to five working days.
5. Build Sioul with the two values. Everyone else then signs in with one click.

What the privacy policy must say, at least: which Google data Sioul reads and writes (calendars, contacts, tasks of the account signed in), that it stays on the person's device and in their own Google account, that nothing goes to the maintainer or anyone else, how to revoke the access (Accounts ▸ Remove, or myaccount.google.com/permissions), and that the use complies with Google's API Services User Data Policy, Limited Use requirements included.

## Your own Google key
Without Sioul's key in the build, or if you prefer yours, you make one, free, in about fifteen minutes (the same steps are in the Accounts page):

1. On [console.cloud.google.com](https://console.cloud.google.com), make a project named Sioul.
2. *APIs & Services → Library*: turn on **CalDAV API**, **CardDAV API** and **Google Tasks API** (not "Google Calendar API" or "People API": those are other doors).
3. *Google Auth platform*: *Branding*, a name and your address; *Audience*, External, yourself as a test user; *Data access*, the scopes `https://www.googleapis.com/auth/calendar`, `https://www.googleapis.com/auth/carddav` (typed by hand) and `https://www.googleapis.com/auth/tasks`.
4. *Clients → Create client → Desktop app*. Copy its ID and its secret at once: Google shows the secret only at creation.
5. *Audience → Publish app*. Left in testing, Google ends every sign-in after seven days. No verification is needed for your own use (under 100 users). A sign-in made while the project was in testing still ends on its seventh day: sign in again once after publishing.
6. In Sioul, paste both and sign in. Google says the app is not verified: it is yours; *Advanced → Go to Sioul*.

Google deletes a client unused for six months; Sioul uses it at every sync.

## Mail
Gmail and Google Workspace mail come over IMAP and SMTP like any other, but Google refuses the account's own password from another program. It takes an **app password** (made at Google, with 2-Step Verification on) or **OAuth 2.0** given to IMAP and SMTP with SASL XOAUTH2. Sioul offers both and never asks for the account's password at Google.

### Recognising Google's mail
When an address is added, `discover.rs` says it is Google's:
- **gmail.com, googlemail.com**: known here, nothing asked of anyone;
- **another domain whose mail Google receives**: its MX is a host under google.com or googlemail.com (`smtp.google.com` for Workspace domains set up since 2023, `aspmx.l.google.com`, `altN.aspmx.l.google.com` and `aspmxN.googlemail.com` before), asked of the system's DNS as the sender checks do (mail-auth's resolver; on Android, the network's DNS servers, which hickory reads through ConnectivityManager);
- **settings found elsewhere naming imap.gmail.com** (the domain's own autoconfig).

Its servers are then Google's: `imap.gmail.com:993` and `smtp.gmail.com:465`, encrypted from the first byte, the address as login. A Workspace domain whose MX is a filtering service's, and whose own settings do not name imap.gmail.com, is not recognised, and its server is not found in the window; the command line adds it (`sioul accounts add <address> --host imap.gmail.com`, with an app password). A password refused by imap.gmail.com is said as wanting an app password.

### What works, with which key
| Way | Works | What Google asks |
|---|---|---|
| **App password** (the default, without a key of your own) | Gmail, and Workspace where the organisation allows app passwords | 2-Step Verification on. Not offered with security keys as the only second step, with Advanced Protection, or where a work or school organisation turned them off. Google revokes them when the account's password changes. Made at [myaccount.google.com/apppasswords](https://myaccount.google.com/apppasswords): sixteen letters, shown in four groups (the spaces are dropped). |
| **Sign in with Google, Sioul's own key** | no, not now | Gmail's scope, `https://mail.google.com/`, is **restricted**. A public app gets it only after Google's restricted-scope review; the yearly security assessment (CASA, by a paid assessor) is required of apps that can reach the data "from or through a third-party server", which Sioul does not (its mail stays on the device). Sioul's key is not registered yet anyway (`built_in()` is empty in every build so far). The form says so plainly and offers the two other ways. |
| **Sign in with Google, a key of your own** | yes | Your own Google Cloud project, the Gmail API turned on, the scope `https://mail.google.com/` in *Data access*, a *Desktop app* client, the project published (*In production*) and never verified: Google shows "Google hasn't verified this app" (*Advanced → Go to Sioul*), and lets at most 100 people in. Google needs no verification for an app used by its owner alone, or by a few people they know. Left **in testing**, each sign-in ends after seven days (Google issues refresh tokens "expiring in 7 days" to projects in testing, unless they ask only for name, address and profile): Sioul says so when it happens. |

The steps for a key of your own are in the form (*How to make your Google key*): the same project as the calendars can hold both scopes. A Workspace organisation can also make its project *Internal*: no unverified screen, no seven-day end, for its own people only. An organisation may also block apps it has not allowed, in its admin console's API controls, and turn app passwords off.

### Signing in with Google, for mail
1. In Accounts ▸ Add an account, the address, then **Find the server**. For Google's mail, the form shows **Use an app password** and **Sign in with Google**, never a plain password field.
2. The key: the one typed (client ID and secret), else one of yours kept on this device for this address (the mail's own, else the calendars' when it is not Sioul's); Sioul's own key is refused for mail, said so.
3. Google's page opens in the system's browser, as for the calendars (loopback, PKCE, state, the ID token's address), with the scopes `openid email https://mail.google.com/`. Google lets each access be unticked on its page: without the mail's, Sioul says "Google gave no access to your mail", gives the token back and keeps nothing.
4. The server is tried (IMAP `AUTHENTICATE XOAUTH2`, the inbox opened read-only) before anything is kept. Then the account is added with `auth = "google"`, sending through `smtp.gmail.com:465`.
5. What stays: a keyring entry of its own, `google-mail:<address>` (client ID, secret, refresh token, scopes, the day of the sign-in); the access token in memory, an hour at most. The calendars' grant (`google:<address>`) is apart: either can be removed alone. Removing the mail gives its access back to Google (`/revoke`), unless the calendars use the same key: Google ends a key's access to an account as a whole, so it is then only forgotten here.
6. At each connection: the access token kept, while it lasts; refused (IMAP `NO`, SMTP `535`), a fresh one from the refresh token, once; refused again, or no refresh possible, "Google asks you to sign in again" (or the seven-day end of a project in testing, said as such). The address's card then offers **Sign in again** (Google's page, the key kept) and **App password…** (the account switches to an app password: tried, kept in the keyring, `auth` removed, the mail's grant given back).
7. An account come from another device arrives without its grant (grants stay in each device's keyring): **Sign in again** with the same key (its ID and secret; Google shows a secret only once, but a client can be given a new one), or **App password…**.

### On Android
- **The same flow.** Google's page opens in the system's browser and its answer comes to `http://127.0.0.1:<port>`, Sioul's listener. Google stopped the loopback redirect for its Android, iOS and Chrome **client types** in 2022, and keeps it for **Desktop app** clients, which Sioul's key and yours are. Expected to work on a phone; not tried against Google.
- **While the browser is in front**, Android may freeze Sioul (its cached-apps freezer). The listening socket stays with the system: the browser's connection is taken and its request waits in the system's queue, and Sioul reads it when it comes back (checked on Linux by stopping a listener: the request was read after it resumed). Sioul reads a waiting answer even past its five minutes, since a frozen app's clock jumps. On a phone, the waiting line and Google's success page say to come back to Sioul.
- **If Android kills Sioul meanwhile** (short of memory), the browser's request is refused and nothing changes: sign in again. Not built, proposed: a short foreground service while the sign-in waits (`shortService`, Android 14 and later, up to three minutes; a notification "Waiting for Google's page"), which would keep Sioul from being frozen or killed. It needs Java and the manifest, where other work is under way.
- **Not used: Android's AccountManager, GoogleAuthUtil, Credential Manager.** They need Google Play services or microG, and an *Android* OAuth client registered with Sioul's package name and the APK's signing certificate; Sioul's client would still need Google's review for the mail's scope. On /e/OS without Play services, the browser is the system's way. App passwords work on a phone as on a computer; the phone never asks for the Google account's password.

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
- **Not kept, greyed in the task form** (with "This list is on Google Tasks: what Google does not keep is greyed"): a start day, a length, a project, a kind, office hours, categories, repeating, waiting for another task, steps of a step. Moving a task to a Google list says first what it would lose.
- What Google does not know stays in the file here (an alarm, a link written by another program) but is not sent.
- Lists can be made, renamed and deleted from Sioul (Google Tasks allows it); a list deleted here that holds tasks there (added from a phone meanwhile) comes back.
- Sync: what changed here first (inserted, patched, moved under its parent, deleted), then what changed there since the last sync (`updatedMin`, deletions included), everything the first time. Changed on both sides between two syncs: yours is sent, and Google's answer is what stays.

## What was tested
- The sign-in's pieces: the page asked (PKCE challenge, state, scopes, login hint), the answer read on the loopback port (a favicon request first, Cancel), the ID token's address.
- Google Tasks against a stand-in of its API (`tools/google-tasks-stand-in.py`, pages of two): first sync with a step under its task, edits both ways, a new step under its parent, deletions, a list made, renamed and deleted here, a list deleted there.
- Mail: the XOAUTH2 string against Google's own example (`sasl.rs`), IMAP `AUTHENTICATE XOAUTH2` and SMTP `AUTH XOAUTH2` against stand-ins that refuse a token as Google does (its JSON reason, the empty line awaited) then take a fresh one; the refresh token traded for an access token against a stand-in of Google's token endpoint, a refusal said as "sign in again", or as the seven-day end of a project in testing; the mail's scope asked apart from the calendars'; Google's mail recognised by its domains and by MX stand-ins (lookalike names refused); an answer waiting in the system's queue read after the time is up.
- **Not tested against Google itself** (no Google account was used): the token exchange, Google's CalDAV and CardDAV answers (discovery, whether a PUT of a new contact is refused or relocated, its `Location`), Google Tasks' own answers, Gmail's IMAP and SMTP with a real token or a real app password, the sign-in on a phone. The first real sign-in is the test; what fails says so in the account's line.

## Files
- `crates/sioul-sync/src/google.rs`: the sign-in, the tokens, the revocation; Sioul's own key, read at build time (`built_in`), and the seven-day end of a project in testing, told as such (`IN_TESTING`); calendars and mail as two purposes, each its grant (`Purpose`).
- `crates/sioul-sync/src/sasl.rs`: SASL XOAUTH2 for IMAP and SMTP, and who gives the tokens (`OAuth`).
- `crates/sioul-sync/src/discover.rs`: Google's mail recognised (its domains, its MX), its servers.
- `crates/sioul-app/src/gmail.rs`, `qml/GoogleMail.qml`: the mail's sign-in in the window, and its app password.
- `crates/sioul-sync/src/dav.rs`: the bearer token, Google's discovery and its quirks.
- `crates/sioul-sync/src/google_tasks.rs`: Google Tasks.
- `crates/sioul-core/src/capabilities.rs`: what each kind of server keeps, for the greyed fields and the warnings before a move.
