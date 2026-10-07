# DAVx⁵ and Android's calendar, contacts and tasks providers: what another app may write

**The question**: how does the /e/OS Account Manager, a fork of DAVx⁵, map iCalendar and vCard onto Android's calendar, contacts, OpenTasks and jtx Board providers, and what must another app write there so as not to break its sync?

Researched on 4 October 2026, read-only, from public source code: nothing was run on any device. The sources are the /e/ Foundation's Account Manager, its forks of ical4android and vcard4android, and its Tasks app; upstream DAVx⁵, ical4android and vcard4android; OpenTasks; jtx Board; and Android's calendar and contacts providers in the AOSP mirror: each at the commit given in §0, each fact with its file and line. Every claim carries its mark: **[code]**, read in the source at the line cited; **[doc]**, Android's API documentation in the AOSP source; **[inferred]**, reasoned from the code, not checked by running anything. One finding was checked again on 8 October 2026 and corrected: what DAVx⁵ uploads of the relation types that jtx Board stores (§4.3), which [compatibility.md](../compatibility.md) ("Other programs editing the same tasks and events") also reads in DAVx⁵ 4.5. Two citations were made exact on that day: the Android framework's commit (§0) and the date of jtx Board's (§4.1).

**Sioul writes to none of these providers.** Its author chose to sync mail, calendars and contacts with Sioul's own code, on a phone as on a computer: "Mail, calendars and contacts then sync through Sioul's own code, not the phone's" ([android.md](../android.md), "What differs on Android"). When the phone also runs DAVx⁵, the two meet at the server, where they sync the same collections, never in the phone's databases. On 8 October 2026 (version 0.0.3), the only provider Sioul touches is the contacts provider, and only to read it (`READ_CONTACTS`): for do-not-disturb (who on the person's list is not starred on the phone) and to screen calls; "Add to contacts" opens the Contacts app's own form, filled in, which the person saves ([android.md](../android.md), "Calls"). This note records what a bridge to the phone's providers would have to respect, should one ever be built, and why another app writing rows breaks DAVx⁵'s sync (§6). What Sioul writes into tasks, events and cards, and what DAVx⁵ keeps of it when the phone edits them, is in [compatibility.md](../compatibility.md) ("What Sioul writes in tasks, events and cards"; "Other programs editing the same tasks and events") and [tasks.md](../tasks.md) ("In the standards").

## 0. Versions and sources (all verified)

The version studied is `foundation.e.accountmanager` 4.3.9-16-ose. In the /e/ repository `e/os/AccountManager` (gitlab.e.foundation, project 83), commit `f1f24355503f` (2026-04-07, "fix(auth): use two-step Murena OIDC flow") sets `versionName '4.3.9-16'` (app/build.gradle:46) and the suffix `-ose+<date>` (app/build.gradle:89). The previous bump is `62f11626c508` = 4.3.9-15 and the next is `77e76ed7032f` = 4.3.9-17. All three pin the same libraries (build.gradle:22-24): `dav4jvm 1ed89c165d`, `foundation.e.lib:ical4android:1.1.2`, `foundation.e.lib:vcard4android:1.1.2`.

The `foundation.e.lib` libraries are /e/ forks published from:

- `e/os/ical4android` (project 97), branch `main` = `942bfdc0881c` (30 October 2023), version 1.1.2 (lib/build.gradle.kts). Compared with upstream `bitfireAT/ical4android@916f2228e8fb` (the exact commit that upstream DAVx⁵ v4.3.9-ose pins in build.gradle:23), the only code changes are in `TaskProvider.kt`: the OpenTasks authority and package become `foundation.e.tasks`, and the permissions become `foundation.e.permission.READ_TASKS` / `WRITE_TASKS`. The manifest permissions change the same way. The event and task mapping code is identical.
- `e/os/vcard4android` (project 96), branch `main` = `66b98e74cb27` (30 October 2023), version 1.1.2. Compared with upstream `bitfireAT/vcard4android@b376d2ec5af7` (pinned by upstream DAVx⁵ v4.3.9-ose, build.gradle:24), the only change is the "starred contacts" patch (see §2). Everything else is the same.

In `e/os/AccountManager@f1f24355503f`, compared with upstream `bitfireAT/davx5-ose@v4.3.9-ose`, these files are identical: `LocalEvent.kt`, `LocalContact.kt`, `LocalGroup.kt`, `LocalTask.kt`, `LocalResource.kt`, `LocalCollection.kt`, `TaskUtils.kt`, `CalendarSyncManager.kt`, `TasksSyncManager.kt`, `LocalJtxICalObject.kt`, `JtxSyncManager.kt`, `JtxSyncer.kt`, the `contactrow/UnknownProperties*` files and the `groups/*Strategy.kt` files. The rest differs as follows:

- `LocalCalendar`, `LocalTaskList` and `LocalJtxCollection` differ only in their notification tag.
- `LocalAddressBook` also differs in how the address-book account type is chosen.
- `ContactsSyncManager` carries the starred patch, plus an OAuth bearer header for photo downloads; `GroupMembershipBuilder` carries the starred patch.
- `SyncManager` adds token refresh, retries and notification changes. Its `processLocallyDeleted`, `uploadDirty` (with the 412/409 handling) and `syncRemote` blocks are byte-identical to upstream.

Citation shorthands used below:

| Short | Repository @ commit | Path prefix |
|---|---|---|
| I4A | github.com/bitfireAT/ical4android @ 916f2228e8fb (25 September 2023; = /e/ ical4android 1.1.2 except TaskProvider.kt) | lib/src/main/kotlin/at/bitfire/ical4android/ |
| TC | the same repo and commit, OpenTasks contract copy | opentasks-contract/src/main/java/org/dmfs/tasks/contract/TaskContract.java |
| V4A | github.com/bitfireAT/vcard4android @ b376d2ec5af7 (4 September 2023; = /e/ vcard4android 1.1.2 except the starred patch) | lib/src/main/java/at/bitfire/vcard4android/ |
| DX | github.com/bitfireAT/davx5-ose @ v4.3.9-ose (26 October 2023; identical in /e/ unless noted) | app/src/main/kotlin/at/bitfire/davdroid/ |
| EAM | gitlab.e.foundation e/os/AccountManager @ f1f24355503f (7 April 2026; 4.3.9-16) | app/src/main/… |
| CP2 | github.com/aosp-mirror/platform_packages_providers_calendarprovider @ 38e4d45 (AOSP main, 10 March 2025) | src/com/android/providers/calendar/CalendarProvider2.java |
| CC | github.com/aosp-mirror/platform_frameworks_base @ 1cdfff555f4a (AOSP main, 27 March 2025) | core/java/android/provider/CalendarContract.java |
| CP | github.com/aosp-mirror/platform_packages_providers_contactsprovider @ cf86ae5 (AOSP main, 10 March 2025) | src/com/android/providers/contacts/ContactsProvider2.java |

The repositories: the /e/ Foundation's Account Manager (https://gitlab.e.foundation/e/os/AccountManager), its ical4android (https://gitlab.e.foundation/e/os/ical4android) and vcard4android (https://gitlab.e.foundation/e/os/vcard4android) forks, and /e/ Tasks (https://gitlab.e.foundation/e/os/tasks); bitfire's DAVx⁵ OSE (https://github.com/bitfireAT/davx5-ose), ical4android (https://github.com/bitfireAT/ical4android) and vcard4android (https://github.com/bitfireAT/vcard4android); the Android Open Source Project's calendar provider (https://github.com/aosp-mirror/platform_packages_providers_calendarprovider), contacts provider (https://github.com/aosp-mirror/platform_packages_providers_contactsprovider) and framework (https://github.com/aosp-mirror/platform_frameworks_base), in GitHub's AOSP mirror; dmfs' OpenTasks (https://github.com/dmfs/opentasks); Techbee's jtx Board (https://github.com/TechbeeAT/jtxBoard); Tasks.org (https://github.com/tasks/tasks). The formats: iCalendar, RFC 5545 (2009, https://www.rfc-editor.org/rfc/rfc5545); its relations and links, RFC 9253 (2022, https://www.rfc-editor.org/rfc/rfc9253); vCard 3.0, RFC 2426 (1998, https://www.rfc-editor.org/rfc/rfc2426), and 4.0, RFC 6350 (2011, https://www.rfc-editor.org/rfc/rfc6350); WebDAV collection synchronisation, RFC 6578 (2012, https://www.rfc-editor.org/rfc/rfc6578); `ESTIMATED-DURATION`, the IETF draft "Task Extensions to iCalendar" (draft-ietf-calext-ical-tasks, https://datatracker.ietf.org/doc/draft-ietf-calext-ical-tasks/). Android's documentation of the two contracts: https://developer.android.com/reference/android/provider/CalendarContract and https://developer.android.com/reference/android/provider/ContactsContract.

The AOSP provider code is from AOSP main (2025). The LineageOS-based /e/OS is assumed to ship the same CalendarProvider logic **[inferred]**; this part of AOSP has been stable for a decade.

/e/ account types (EAM app/src/main/res/values/strings.xml:9-23) **[code]**:

- Main accounts, which own the calendars and task lists: `e.foundation.webdav` (generic CalDAV/CardDAV), `e.foundation.webdav.eelo` (Murena cloud), `e.foundation.webdav.google` and `e.foundation.webdav.yahoo`.
- Address-book accounts, one per CardDAV collection: `foundation.e.accountmanager.address_book`, `….eelo.address_book`, `….google.address_book` and `….yahoo.address_book`.
- Authorities: tasks `foundation.e.tasks` (strings.xml:31); the address-book helper provider is `foundation.e.accountmanager.addressbooks` (strings.xml:25).

---

## 1. Events (CalendarContract, authority `com.android.calendar`)

### 1.1 Calendars owned by DAVx⁵

| Calendars column | Value written by DAVx⁵ | Source |
|---|---|---|
| `account_name` / `account_type` | main account name (e-mail) / main account type (see §0) | DX resource/LocalCalendar.kt:41-42 [code] |
| `name` | **CalDAV collection URL** | LocalCalendar.kt:56 [code] |
| `calendar_displayName`, `calendar_color` | collection display name (or last URL segment) and colour | LocalCalendar.kt:57-60 [code] |
| `ownerAccount` | account name; also the fallback ORGANIZER | LocalCalendar.kt:46 [code] |
| `calendar_access_level` | 700 (OWNER) if the collection is writable, else 200 (READ) | LocalCalendar.kt:62-67 [code] |
| `cal_sync1` | DAVx⁵ sync state (CTag or sync-token) | LocalCalendar.kt:35, 94-105 [code] |
| `allowedAvailability` / `allowedAttendeeTypes` / `allowedReminders` | `0,1` / `0,2,1,3` / `0,1,2` | I4A AndroidCalendar.kt:38-42 [code] |

At every sync, any calendar of the account whose `name` is not the URL of a collection selected on the server is **deleted** (DX syncadapter/CalendarSyncer.kt:69-76) [code]. A calendar with `name` = NULL is not deleted, but `CalendarSyncManager.prepare()` returns false for it, so it is never synced (CalendarSyncManager.kt:58) [code]. Another app must therefore not create calendars inside a DAVx⁵ account; it can only use the existing ones. Only calendars with `sync_events != 0` are synced (CalendarSyncer.kt:46-47) [code].

### 1.2 DAVx⁵ bookkeeping columns on `Events`

| Column (constant) | Meaning in DAVx⁵ 4.3.9 | Source |
|---|---|---|
| `_sync_id` (`_SYNC_ID`) | file name of the resource in the collection, e.g. `<UID>.ics`. NULL until the first upload. | DX resource/LocalEvent.kt:155, 188, 243 [code] |
| `sync_data1` | ETag (with quotes, as the server sent it) | LocalEvent.kt:27 [code] |
| `sync_data2` | flags: 0, or 1 = `FLAG_REMOTELY_PRESENT` (seen in the current server listing) | LocalEvent.kt:28; LocalResource.kt:17 [code] |
| `sync_data3` | iCalendar SEQUENCE | LocalEvent.kt:29, 165, 180 [code] |
| `sync_data4` | CalDAV Schedule-Tag | LocalEvent.kt:30 [code] |
| `uid2445` (`UID_2445`) | iCalendar **UID**. Exception rows carry the master's UID. | LocalEvent.kt:164, 179 [code] |
| `original_id` | on exception rows: `_id` of the master row | I4A AndroidEvent.kt:594 [code] |
| `original_sync_id` | on exception rows: the master's `_sync_id` (file name) | LocalEvent.kt:185-186 [code] |
| `dirty`, `deleted` | change detection (§1.5) | LocalCalendar.kt:112-144 [code] |
| `mutators` | set by the provider to the comma-separated packages that changed the row; DAVx⁵ appends them to PRODID: `… (pkg1,pkg2)` | I4A AndroidEvent.kt:173-176, ICalendar.kt:72-76 [code]; CP2 2867-2893 [code] |

SEQUENCE: when a dirty main event is uploaded, `sync_data3` NULL becomes 0. Otherwise it is incremented if the event has no attendees or `isOrganizer` is true (LocalCalendar.kt:123-141) [code]. SEQUENCE is written into the iCalendar only when it is ≠ 0 (I4A Event.kt:283-286) [code]. After the upload, the value is stored back in `sync_data3` (LocalEvent.kt:246) [code].

### 1.3 Property ↔ column mapping (main event row)

Source file for all lines below: I4A AndroidEvent.kt, unless another file is named. All **[code]**.

| iCalendar | Events column(s) and format | Lines |
|---|---|---|
| UID | `uid2445` (written by DAVx⁵'s LocalEvent) | DX LocalEvent.kt:164, 179 |
| DTSTART | `dtstart` = ms since epoch. `eventTimezone` = the TZID mapped to an Android zone ID (exact match, then substring match, then the device default); `UTC` for UTC times; the device default zone for floating times. | 737-740; AndroidTimeUtils.kt:55-60, 105-121; util/DateUtils.kt:56-76 |
| DTSTART;VALUE=DATE (all-day) | `allDay` = 1, `dtstart` = **UTC midnight**, `eventTimezone` = `UTC` | 739; AndroidTimeUtils.kt:33, 119-121; CC 1684-1685 [doc] |
| DTEND / DURATION, non-recurring | `dtend` (ms) + `eventEndTimezone`; `duration` = NULL. A DURATION is converted to DTEND. With neither: all-day → +1 day; timed → DTEND = DTSTART. | 828-870 |
| DTEND / DURATION, recurring (RRULE or RDATE present) | `duration` = RFC 5545 string (`PT1H`, `P1D`, `P2W`, `P0S`…), `dtend` = NULL. A DTEND is converted to a duration. | 753-784; util/TimeApiExtensions.kt:130-198 |
| (reading back) | DAVx⁵ always generates DTEND, never DURATION: from `dtend`, or from `dtstart` + `duration`; defaults are 1 day (all-day) or 1 h. All-day `dtend` == `dtstart` → no DTEND. | 178-266 |
| RRULE (several allowed) | `rrule` = rule values joined by `\n` | 787-791; AndroidTimeUtils.kt:41 |
| EXRULE | `exrule`, same format | 816-819 |
| RDATE | `rdate` = `[TZID;]v1,v2,…` with v = `yyyyMMdd'T'HHmmss` (local time in TZID) or `…Z`. All-day: `yyyyMMdd'T'000000'Z'`. **DTSTART is prepended** (Android otherwise drops that instance). RDATE is dropped when there is an infinite RRULE (no COUNT, no UNTIL). PERIOD values are dropped. | 793-814; AndroidTimeUtils.kt:139-184 |
| EXDATE | `exdate`, same format as `rdate` | 821-826 |
| SUMMARY / LOCATION / DESCRIPTION | `title` / `eventLocation` / `description` | 872-874 |
| COLOR (a CSS3 name) | `eventColor_index` = the CSS3 colour **name** (e.g. `darkred`), set only if the account has a `Colors` row with that key and type 1. DAVx⁵ inserts all CSS3 colours into `Colors` when the account setting "event colours" is on, and removes them otherwise. A bare `eventColor` int without a key is **not read**. | 875-886, 295-301; AndroidCalendar.kt:66-110; DX CalendarSyncer.kt:39-42 |
| STATUS | CONFIRMED → 1, CANCELLED → 2, TENTATIVE (or any other value) → 0, absent → NULL. Reading: 0 → TENTATIVE, 1 → CONFIRMED, 2 → CANCELLED, NULL → none. | 910-916, 304-308 |
| TRANSP | OPAQUE or absent → `availability` 0 (BUSY); TRANSPARENT → 1 (FREE). Reading: anything ≠ 1, including 2 = TENTATIVE → OPAQUE. TRANSP is written only when TRANSPARENT. | 918, 311; Event.kt:305-306 |
| CLASS | absent or PUBLIC → `accessLevel` 3; CONFIDENTIAL → 1; PRIVATE or any other value → 2. Reading: 3/2/1 → PUBLIC/PRIVATE/CONFIDENTIAL; 0 (DEFAULT) → none. CONFIDENTIAL and x-values are **also** kept as an unknown-property row `["CLASS","CONFIDENTIAL"]`, which is used when `accessLevel` = 0. | 919-923, 325-329, 496-504, 1016-1032 |
| ORGANIZER | only when there are attendees: `hasAttendeeData` = 1 and `organizer` = the e-mail (from mailto:, or the EMAIL parameter), falling back to `ownerAccount`. Without attendees: `hasAttendeeData` = 0, `organizer` = `ownerAccount`. Reading: an ORGANIZER `mailto:<organizer>` is generated only if Attendees rows exist (CN is lost). | 889-906, 314-322 |
| ATTENDEE | rows in `Attendees` (§1.4) | 948-983 |
| VALARM | rows in `Reminders` (§1.4) | 926-946 |
| CATEGORIES | an `ExtendedProperties` row with `name` = **`categories`** and `value` = names joined by a **backslash**, e.g. `Work\Family`. Backslashes inside names are removed. | 64-73, 994-1001 |
| URL | an `ExtendedProperties` row with `name` = **`vnd.android.cursor.item/vnd.ical4android.url`** and `value` = the URL | 75-79, 561-563, 437-442 |
| any other property | one `ExtendedProperties` row per property (§1.4) | 565-567, 1003-1014 |
| RECURRENCE-ID components | separate Events rows (§1.6) | 570-629 |
| PRODID, DTSTAMP | dropped (DTSTAMP is regenerated at upload) | Event.kt:189, 278 |
| LAST-MODIFIED | read into `Event.lastModified` but never stored in the provider → **lost** | Event.kt:57, 188 (no use in AndroidEvent) [inferred from the absence] |

### 1.4 Attendees, reminders and extended properties

**Attendees** (AndroidEvent.kt:948-983; AttendeeMappings.kt:109-169) **[code]**

| iCalendar | Attendees column |
|---|---|
| `mailto:x@y` | `attendeeEmail` = `x@y` |
| other URI (e.g. `urn:uuid:…`) | `attendeeIdNamespace` = scheme, `attendeeIdentity` = the rest; the EMAIL parameter goes to `attendeeEmail` |
| CN | `attendeeName` |
| PARTSTAT | ACCEPTED → 1, DECLINED → 2, TENTATIVE → 4, DELEGATED → 0 (NONE), NEEDS-ACTION or anything else → 3 (INVITED) |
| CUTYPE × ROLE | `attendeeType` × `attendeeRelationship`. Defaults: INDIVIDUAL + REQ-PARTICIPANT → TYPE_REQUIRED (1) + RELATIONSHIP_ATTENDEE (1); OPT-PARTICIPANT → 2; NON-PARTICIPANT → 0 (TYPE_NONE); CHAIR → RELATIONSHIP_SPEAKER (4); GROUP → PERFORMER (3); UNKNOWN → NONE (0); RESOURCE → TYPE_RESOURCE (3); ROOM → TYPE_RESOURCE + PERFORMER. The full matrix is at AttendeeMappings.kt:87-101. |
| the attendee whose e-mail = the organizer | relationship 2 (ORGANIZER) instead of 1 |

When reading, DAVx⁵ adds `RSVP=TRUE` to every attendee and maps status 0 to no PARTSTAT (351-390). RSVP, DELEGATED-TO/FROM, SENT-BY, DIR, LANGUAGE, MEMBER and X- parameters are not stored, so they are lost **[inferred from the absence in insertAttendee]**.

**Reminders** (AndroidEvent.kt:926-946, 392-424; ICalendar.kt:298-376) **[code]**

- `method`: ACTION DISPLAY or AUDIO → 1 (ALERT); EMAIL → 2 (EMAIL); anything else → 0 (DEFAULT).
- `minutes` = minutes **before DTSTART**:
  - an END-related trigger is moved to START using the event duration;
  - an absolute DATE-TIME trigger becomes DTSTART − trigger;
  - seconds are truncated;
  - −1 if the value cannot be computed.
- Reading back: each row becomes `TRIGGER:-PT<minutes>M` with `ACTION:DISPLAY` and `DESCRIPTION:<SUMMARY>`. A method 2 row becomes `ACTION:EMAIL` with SUMMARY, DESCRIPTION and `ATTENDEE:mailto:<account name>` if the account name is an e-mail address.
- Everything else in a VALARM is lost: UID, RELATED=END, absolute times, REPEAT/DURATION, X- props and ACKNOWLEDGED.
- When an event is downloaded without alarms, DAVx⁵ may add the account's default alarm (setting) (DX CalendarSyncManager.kt:160-166).

**ExtendedProperties: the unknown-property format** (I4A UnknownProperty.kt) **[code]**

- `name` = `vnd.android.cursor.item/vnd.ical4android.unknown-property` (UnknownProperty.kt:31; the prefix is `ContentResolver.CURSOR_ITEM_BASE_TYPE`). In this version there is **no** `.v2` suffix.
- `value` = a JSON **array**: `["NAME","value"]` or `["NAME","value",{"PARAM1":"v1","PARAM2":"v2"}]` (UnknownProperty.kt:22-23, 78-91). The value is the property's unescaped text value. Parameters form a JSON object, so two parameters with the same name collapse into one (JSONObject.put), and each parameter value is ical4j's string form (multi-valued parameters stay comma-joined).
- Reading parses the array back with ical4j's PropertyBuilder and ParameterBuilder (50-70).
- Rows are skipped if the value is null or longer than 25 000 characters (`MAX_UNKNOWN_PROPERTY_SIZE`, UnknownProperty.kt:37; AndroidEvent.kt:1003-1014).
- **Which properties end there:** every VEVENT property not explicitly mapped in `Event.fromVEvent()` (Event.kt:163-191). That means all `X-` properties and `LINK` (RFC 9253), `RELATED-TO` (with its RELTYPE parameter), `CREATED`, `GEO`, `PRIORITY`, `RESOURCES`, `COMMENT`, `CONTACT`, `ATTACH` (≤ 25 000 chars), `REQUEST-STATUS`, `CONFERENCE`, `IMAGE`, and so on. Each is one row. They are re-emitted verbatim at upload (Event.kt:313).
- Exceptions do not keep unknown properties (§1.6).

When an event is re-downloaded, DAVx⁵ deletes only `ExtendedProperties` rows whose name is one of its three names: `categories`, `…ical4android.url` and `…ical4android.unknown-property`. Rows with any other name survive a server update (AndroidEvent.kt:668-673) [code]. They are lost if the event is rebuilt (§1.5 step 5) and they are never uploaded.

### 1.5 How DAVx⁵ finds local changes and uploads them

What the Calendar provider does for a normal app, i.e. a URI without `caller_is_syncadapter=true` **[code]**, CP2:

- Insert or update of `Events`: the provider forces `dirty` = 1 and adds the caller's package to `mutators` (2366-2369, 4121-4125).
- Inserting `Attendees` or `Reminders` marks the event dirty (`setEventDirty`, 2550-2554, 2572-2575). Updates and deletes of those rows also set `dirty` = 1 on the event (`deleteFromEventRelatedTable` 3595ff, `deleteReminders` 3645ff, `updateEventRelatedTable` 3752ff).
- A normal app may not write `_sync_id`, `dirty`, `mutators` or `sync_data1…10`: the provider throws `IllegalArgumentException("Only sync adapters may write to …")` (verifyNoSyncColumns 4718-4746; CC 1867-1881).
- **A normal app may not write `ExtendedProperties` or `Colors` at all** ("Only sync adapters may write using …", 4621-4632).
- Required on insert (validateEventData 2833-2865; CC 1646-1655): `calendar_id`, `eventTimezone`, `dtstart`, and `dtend` or `duration` (not both). With `rrule`/`rdate` → `duration`. The insert without `dtstart` fails (2370-2382).
- Delete by an app (deleteEventInternal 3495-3572):
  - if the row has a `_sync_id`: **soft delete**, i.e. `deleted` = 1 and `dirty` = 1. Reminders, ExtendedProperties, instances and alerts are removed at once. Attendees are kept. Exceptions without a `_sync_id` are removed.
  - if the row has no `_sync_id` (never uploaded): hard delete, plus its exceptions if it recurs.
- Every non-sync-adapter change also triggers `notifyChange(syncToNetwork=true)` (SQLiteContentProvider.java:110, 166, 198). The sync framework then calls DAVx⁵'s `onPerformSync` with `SYNC_EXTRAS_UPLOAD`, which enqueues its SyncWorker (EAM syncadapter/SyncAdapterServices.kt:45-64, subject to the Wi-Fi-only setting) [code].
- The sync-adapter flag is only the URI query parameter. There is no check of the caller's identity: `getIsCallerSyncAdapter()` = `readBooleanQueryParameter(uri, CALLER_IS_SYNCADAPTER)` (SQLiteContentProvider.java:208-213) [code]. Sync-adapter writes must also name `account_name` and `account_type` (verifyHasAccount, CP2 4672-4686).

What DAVx⁵ then does, in each sync, for each calendar **[code]**:

1. `processDirtyExceptions()` (DX LocalCalendar.kt:185-249):
   - a deleted exception row is removed, and its master gets `dirty` = 1 and SEQUENCE + 1;
   - a dirty exception gets SEQUENCE + 1 and `dirty` = 0, and its master gets `dirty` = 1.
2. `deleteDirtyEventsWithoutInstances()`: a dirty main event with 0 computed instances is marked `deleted` = 1 (LocalCalendar.kt:256-276; CalendarSyncManager.kt:61-65).
3. Deletes: rows with `deleted` AND `original_id IS NULL` (LocalCalendar.kt:112-113).
   - With a `_sync_id`: HTTP DELETE with `If-Match: <sync_data1>`, or with If-Schedule-Tag-Match if `sync_data4` is set. Errors are ignored, so the event may come back.
   - Then a hard delete as the sync adapter (DX syncadapter/SyncManager.kt:348-378).
4. Uploads: rows with `dirty` AND `original_id IS NULL` in this calendar (LocalCalendar.kt:115-144; AndroidCalendar.queryEvents adds `calendar_id=?`).
   - **New row** (`_sync_id` NULL), `prepareForUpload()` (LocalEvent.kt:202-237):
     - the UID is `uid2445`; if empty, a random UUID is generated and written to `uid2445`;
     - the file name is `<UID>.ics` if the UID contains only letters, digits and ``;:@&=+$,-_.!~*'()``, otherwise `<random UUID>.ics`;
     - `PUT` with `If-None-Match: *` (SyncManager.kt:418-425).
   - **Existing row**: `PUT` to `_sync_id` with `If-Match: <ETag>` or If-Schedule-Tag-Match (SyncManager.kt:427-436).
   - After the upload, `clearDirty` sets `_sync_id` (new rows only), `sync_data1` = the new ETag (or NULL if the server sent none), `sync_data4`, `sync_data3` = SEQUENCE and `dirty` = 0 (LocalEvent.kt:240-254).
   - Errors:
     - **412 or 409** (changed on the server): ignored, and `clearDirty` runs with ETag = NULL, so the next listing re-downloads the server copy and **the local edit is overwritten**: the server wins (SyncManager.kt:460-479).
     - 404 or 410 on an update: retried as a new upload (448-459).
     - 403 with need-privileges: treated like 412 (440-447).
5. Download. When an event's ETag differs, the server copy is applied with `LocalEvent.update()` (CalendarSyncManager.kt:169-176; I4A AndroidEvent.kt:640-680):
   - every mapped Events column is overwritten;
   - all exception rows, Reminders, Attendees and the three DAVx⁵ ExtendedProperties names are deleted and re-inserted;
   - unmapped Events columns and foreign ExtendedProperties rows are kept.
   - Exception: if the new STATUS is absent but the row has a non-NULL `eventStatus`, the whole event is **deleted and re-inserted** with a new `_id` (644-657).
6. After a full listing (PROPFIND/REPORT mode, or the first sync-collection pass), every row with `dirty` = 0, `original_id IS NULL` and `sync_data2` (flags) = 0 is **deleted** (SyncManager.kt:545-548, 706-710; LocalCalendar.kt:150-175). DAVx⁵'s own rows get flag 1 when the server lists them (SyncManager.kt:616).

### 1.6 Recurrence exceptions (RECURRENCE-ID)

DAVx⁵ writes each exception as its own Events row (AndroidEvent.kt:570-629; LocalEvent.kt:176-191) **[code]**. The row's columns:

- `original_id` = master `_id`;
- `original_sync_id` = master file name;
- `originalInstanceTime` = RECURRENCE-ID in ms. The value type is changed to the master's DTSTART type (DATE ↔ DATE-TIME; the time and zone are guessed from DTSTART).
- `originalAllDay` = master all-day flag;
- `uid2445` = master UID, `sync_data3` = the exception's SEQUENCE, `dirty` = 0;
- its own `dtstart`/`dtend`/`title`… as above.

Other rules:

- Only `Reminders` and `Attendees` are inserted for exceptions. **CATEGORIES, URL and unknown properties of an exception are dropped** (624-628).
- ical4android's comment (571-584) explains that the provider only links an exception to its master through `_sync_id` / `original_sync_id`, not through `_id` / `original_id`. A master without a `_sync_id` shows its exception as an extra instance [code comment].
- At upload, an exception with STATUS = CANCELLED (`eventStatus` 2) becomes an **EXDATE** of the master, not a VEVENT (466-481). Other exceptions get the master's ORGANIZER. Exceptions whose RECURRENCE-ID value type differs from DTSTART are skipped (Event.kt:236-243).
- Because the exceptions are rebuilt on every download (`deleteExceptions`, 702-707), their `_id`s change each time the server copy changes **[code]**.

### 1.7 Writing events without breaking DAVx⁵

Recommendations for another app, inferred from the facts above.

**Option A, as a normal app (recommended).** Use URIs without `caller_is_syncadapter`.

- **Create** in a DAVx⁵ calendar whose `calendar_access_level` ≥ 500 (DAVx⁵ uses 700). Set:
  - `calendar_id`, `dtstart`, `eventTimezone` (IANA ID, or `UTC` with `allDay` = 1 at UTC midnight);
  - `dtend` (non-recurring) or `duration` + `rrule` (recurring);
  - `title`, `description`, `eventLocation`;
  - `uid2445` = the app's own UID, using only URL-safe characters such as a UUID, so that the file becomes `<UID>.ics`. If left empty, DAVx⁵ generates one.
  - `eventStatus`: leave it NULL, or set 1 / 2. Do not write 0, which means TENTATIVE.
  - `availability` (0 busy / 1 free) and `accessLevel` (0 default, 2 private, 3 public).
  - For invitations: `hasAttendeeData` = 1, `organizer`, and Attendees rows. Reminders go in their own rows.
  - Leave `_sync_id`, `sync_data*`, `dirty` and `mutators` alone; the provider forbids them anyway.
- **Update**: change only the columns the app owns. The provider sets `dirty`, DAVx⁵ uploads, and the extended-property rows (CATEGORIES, URL, X- props, LINK, RELATED-TO…) stay attached and are re-uploaded unchanged.
- **Delete**: use a normal delete. The provider soft-deletes and DAVx⁵ sends DELETE.
- **Limit**: a normal app cannot add or modify CATEGORIES, URL or any unknown/X- property, because ExtendedProperties are reserved to sync adapters.

**Option B, writing "as sync adapter"** (any app with WRITE_CALENDAR can do this). Pass `caller_is_syncadapter=true&account_name=…&account_type=…`. This allows ExtendedProperties in the formats of §1.3/§1.4, and these rules then apply:

1. Set `dirty` = 1 yourself on every insert or update. Otherwise the change is never uploaded and, at the next full listing, a new row (`sync_data2` = 0) is **deleted** (§1.5 step 6).
2. Never hard-delete a row that has a `_sync_id`. DAVx⁵ would never send DELETE and the event would come back. Update it with `deleted` = 1, `dirty` = 1 instead.
3. Never write `_sync_id` or `sync_data1…4`. `sync_data2` must stay 0 or 1.
4. Write the UID into `uid2445`.
5. Sync-adapter writes do not trigger the upload sync request. Request a sync explicitly (`ContentResolver.requestSync` with `SYNC_EXTRAS_UPLOAD`), or wait for the periodic sync [inferred from SQLiteContentProvider.java:110].

Expect lossy round trips: reminders become minutes before start, attendee parameters are reduced, and LAST-MODIFIED and exception X- props are lost. Edits that conflict with a server change are silently overwritten by the server copy (412 → server wins).

---

## 2. Contacts (ContactsContract, authority `com.android.contacts`)

### 2.1 One Android account per CardDAV address book

DAVx⁵ creates one Android account for each CardDAV collection; the main account only holds settings (DX resource/LocalAddressBook.kt:37-44 comment, 66-84) **[code]**.

- **Account type**:
  - /e/ fork: `foundation.e.accountmanager.address_book`, or `….eelo.address_book` for Murena, `….google.address_book`, `….yahoo.address_book`. The fork picks it per service (EAM LocalAddressBook.kt:71-82; strings.xml:17-23).
  - Upstream DAVx⁵: `at.bitfire.davdroid.address_book`; its main account type is `bitfire.at.davdroid` (DX app/src/main/res/values/strings.xml:8-9).
- **Account name**: `<collection display name or last URL segment> (<main account name> <hash>)` (LocalAddressBook.kt:104-117).
- **Account user data** (LocalAddressBook.kt:52-55, 119-125): `real_account_name`, `real_account_type`, `url` (the collection URL) and `read_only`. Only the owning app can read AccountManager user data, so another app cannot **[inferred]**; it can identify DAVx⁵ address books by their account type.
- `ContactsContract.Settings`: `should_sync` = 1 and `ungrouped_visible` = 1 (LocalAddressBook.kt:76-80).
- **Read-only collections**:
  - DAVx⁵ sets `raw_contact_is_read_only` = 1, `Data.is_read_only` = 1 and `group_is_read_only` = 1 (V4A AndroidContact.kt:190-191; DataRowBuilder.kt:40-41; LocalAddressBook.kt:249-268).
  - The provider then silently skips updates by normal apps to those raw contacts, through an extra `raw_contact_is_read_only=0` selection (ContactsProvider2.java:4979-4982 **[code]**, AOSP mirror `platform_packages_providers_contactsprovider@cf86ae5`).
  - Any local change that still gets through is reverted by DAVx⁵ at the next sync: dirty → `clearDirty(ETag=null)` and re-download; deleted → undeleted (DX ContactsSyncManager.kt:167-215).

### 2.2 Bookkeeping columns

**RawContacts** **[code]**

| Column | Meaning | Source |
|---|---|---|
| `sourceid` (`SOURCE_ID`) | file name `<UID>.vcf`. NULL until the first upload. There is **no** character check, unlike events. | V4A AndroidContact.kt:28; DX LocalContact.kt:72-91 |
| `sync1` | vCard **UID**, with a `urn:uuid:` prefix stripped | AndroidContact.kt:29, 188; ContactReader.kt:89-109, 125-126; ContactProcessor.kt:77-79 |
| `sync2` | ETag | AndroidContact.kt:30 |
| `sync3` | data hash; used only on Android 7 to ignore metadata-only dirtiness | LocalContact.kt:42, 103-108 |
| `sync4` | flags: 0, or 1 = remotely present | LocalContact.kt:41 |
| `dirty` / `deleted` | change detection (§2.6) | LocalAddressBook.kt:319-338 |
| `starred` | **/e/ only**: set to 1 on download when the vCard CATEGORIES contain `starred`, else 0 | /e/ vcard4android AndroidContact.kt buildContact (diff in §0) |

**Groups.** These columns are used in GROUP_VCARDS mode (V4A AndroidGroup.kt:25-27, 108-133; DX LocalGroup.kt:33-38, 250-258) **[code]**:

- `sourceid` = file name, `sync1` = UID, `sync2` = ETag, `sync4` = flags;
- `sync3` = pending member UIDs separated by `\n`;
- `title` = FN, `notes` = NOTE.

In CATEGORIES mode a group has only a `title`, which equals the category name (LocalAddressBook.kt:415-426). Empty groups are deleted after each sync (CategoriesStrategy postProcess; LocalAddressBook.kt:428-434).

### 2.3 Data rows: mimetype ↔ vCard

The handlers and builders are in V4A `contactrow/` **[code]**. Columns are given as `Constant (dataN)`. In vCard 3, labels travel as `itemN.X-ABLabel`; a label becomes Android `TYPE_CUSTOM` + `LABEL` (ContactReader.kt:320-332; ContactWriter.kt:263-291).

| Mimetype | vCard | Columns and notes |
|---|---|---|
| `vnd.android.cursor.item/name` | FN, N, X-PHONETIC-FIRST/MIDDLE/LAST-NAME | `DISPLAY_NAME` (data1) = FN; `GIVEN_NAME` (data2), `FAMILY_NAME` (data3), `PREFIX` (data4), `MIDDLE_NAME` (data5), `SUFFIX` (data6); phonetic names in data7, data8, data9. Several prefixes or middle names are joined with spaces. (StructuredNameBuilder.kt:15-34) |
| `vnd.android.cursor.item/nickname` | NICKNAME (only one property kept, one row per value) | `NAME` (data1), `TYPE` (data2): `x-initials`, `x-maiden-name`, `x-short-name` → matching types, no type → DEFAULT, else OTHER_NAME; `LABEL` (data3) (NicknameBuilder.kt:17-50; property/CustomType.kt:23-27) |
| `vnd.android.cursor.item/email_v2` | EMAIL | `ADDRESS` (data1), `TYPE` (data2): HOME, WORK, `x-mobile` → MOBILE, else OTHER; `LABEL`. PREF (any value) → `is_primary` = `is_super_primary` = 1. (EmailBuilder.kt:21-66; EmailHandler.kt) |
| `vnd.android.cursor.item/phone_v2` | TEL | `NUMBER` (data1), `TYPE` (data2): CELL(+WORK), FAX+HOME/WORK, PAGER(+WORK), HOME, WORK, `x-callback`, CAR, `x-company_main`, ISDN, `x-radio`, `x-assistant`, `x-mms`. PREF → primary. (PhoneBuilder.kt:20-98; PhoneHandler.kt:26-80) |
| `vnd.android.cursor.item/postal-address_v2` | ADR (vCard 3 LABEL is **dropped**) | STREET (data4), POBOX (data5), NEIGHBORHOOD (data6) = extended address, CITY (data7), REGION (data8), POSTCODE (data9), COUNTRY (data10). Multiple values are joined with `\n`. FORMATTED_ADDRESS (data1) = the ADR LABEL parameter, or a generated European-style string. TYPE: HOME, WORK or OTHER. (StructuredPostalBuilder.kt:21-90; ContactReader.kt:175-177) |
| `vnd.android.cursor.item/organization` | ORG, TITLE, ROLE | `COMPANY` (data1) = first ORG value; `DEPARTMENT` (data5) = the other values joined by ` / `; `TITLE` (data4) = TITLE; `JOB_DESCRIPTION` (data6) = ROLE. (OrganizationBuilder.kt:17-40) |
| `vnd.android.cursor.item/website` | URL | `URL` (data1); TYPE from `x-homepage`, `x-blog`, `x-profile`, `x-ftp`, `home`, `work`, else OTHER (WebsiteBuilder.kt:17-46) |
| `vnd.android.cursor.item/contact_event` | BDAY, ANNIVERSARY, X-ABDATE | `START_DATE` (data1) is `yyyy-MM-dd`, `--MM-dd` (no year), or `yyyy-MM-dd'T'HH:mm:ss.SSS'Z'` (converted to UTC). `TYPE` (data2): BIRTHDAY (3), ANNIVERSARY (1), CUSTOM (0) + label, or OTHER (2). In vCard 3, ANNIVERSARY is written as X-ABDATE with the Apple anniversary label, and dates without a year as year 1604 + `X-APPLE-OMIT-YEAR=1604`. (EventBuilder.kt:24-140; ContactWriter.kt:98-124, 305-320; ContactReader.kt:179-200) |
| `vnd.android.cursor.item/note` | NOTE | `NOTE` (data1). Several NOTEs are concatenated with `\n\n\n`. (NoteBuilder.kt; ContactReader.kt:254-261) |
| `vnd.android.cursor.item/im` | IMPP (except `sip:`) | `DATA` (data1) = handle, `PROTOCOL` (data5) = −1 (CUSTOM), `CUSTOM_PROTOCOL` (data6) = URI scheme; TYPE HOME, WORK or OTHER. When reading, Android's legacy protocol codes become `aim:`, `msn…`, `ymsgr…`, `skype:`, `qq:`, `google-talk:`, `icq:`, `xmpp:`, `netmeeting:`. (ImBuilder.kt:18-56; ImHandler.kt:20-80) |
| `vnd.android.cursor.item/sip_address` | IMPP:sip:…, X-SIP | `SIP_ADDRESS` (data1), TYPE, LABEL (SipAddressBuilder.kt:17-49; ContactReader.kt:169-171) |
| `vnd.android.cursor.item/relation` | RELATED (vCard 4) / X-ABRELATEDNAMES (vCard 3) | `NAME` (data1) = text, or URI if there is no text; a URI comes back as **text**. TYPE: Android-specific types (assistant, brother, domestic-partner, father, manager, mother, partner, referred-by, sister) or RFC types (child, friend, kin → RELATIVE, parent, spouse); other types → CUSTOM with the types joined by `, ` in LABEL. (RelationBuilder.kt:20-66; RelationHandler.kt) |
| `vnd.android.cursor.item/photo` | PHOTO | Written through `RawContacts/<id>/display_photo` (openAssetFile "w"); DAVx⁵ then resets `dirty` = 0. Read back from `PHOTO_FILE_ID` (data14, the full-size display photo, re-encoded as JPEG q75) or from the thumbnail `PHOTO` (data15, JPEG q95). Uploaded as an embedded JPEG. A PHOTO given as a URL is downloaded at sync. (PhotoBuilder.kt:45-104; PhotoHandler.kt:34-96; ContactWriter.kt:93; ContactReader.kt:263-264, 334-345) |
| `vnd.android.cursor.item/group_membership` | CATEGORIES (CATEGORIES mode only) | `GROUP_ROW_ID` (data1) → a group whose `title` is the category name (DX contactrow/GroupMembershipBuilder.kt; GroupMembershipHandler.kt) |
| `x.davdroid/cached-group-membership` | none (internal, GROUP_VCARDS mode) | data1 = group `_id`. A copy of the memberships used to detect which groups changed (V4A CachedGroupMembership.kt:9-31) |
| **`x.davdroid/unknown-properties`** | every unmapped property | **data1 = a complete vCard text** (§2.4) |

UID: in the vCard, `UID` (vCard 4 often `urn:uuid:…`, prefix stripped). If a vCard has no UID, a random one is created (ContactReader.kt:290-293). New local contacts get `UUID.randomUUID()` in `sync1` at upload (LocalContact.kt:72-91).

### 2.4 The contact "unknown properties" row

- Mimetype **`x.davdroid/unknown-properties`**, column **`data1`** (DX resource/contactrow/UnknownProperties.kt:6-10) **[code]**. One row per raw contact, written only if something is left over (UnknownPropertiesBuilder.kt:7-13).
- Format: a **full vCard text** written by ez-vcard: `BEGIN:VCARD`, `VERSION:3.0` or `4.0` (the version of the vCard received from the server), the leftover properties (group prefixes such as `item3.` are kept), `END:VCARD`. No PRODID (V4A ContactReader.kt:302-312).
- What ends up there: every property that ContactReader does not consume (ContactReader.kt:121-288). That means:
  - all `X-` properties except X-PHONETIC-*, X-SIP, X-ABDATE, X-ABRELATEDNAMES, X-ADDRESSBOOKSERVER-KIND/MEMBER, and the X-ABLabel attached to a mapped property;
  - GENDER, LANG, TZ, GEO, KEY, FBURL, CALURI, CALADRURI, CLIENTPIDMAP, XML;
  - LOGO and SOUND up to 25 KiB.
- **Dropped**: PRODID, REV (regenerated), SORT-STRING, SOURCE, vCard 3 LABEL, LOGO/SOUND > 25 KiB, and unparsable duplicates of ANNIVERSARY, BDAY, KIND, FN, N, PRODID, REV and UID (ContactReader.kt:266-300).
- At upload, the text is parsed back (`Ezvcard.parse(...).first()`) and its properties are appended to the generated vCard. Item IDs are renumbered to avoid clashes (ContactWriter.kt:90-91, 282-303). If the text does not parse, it is dropped with a warning.
- **Unlike calendar ExtendedProperties, a normal app may write this row**. ContactsProvider does not restrict custom mimetypes, and the insert marks the raw contact dirty **[inferred from ContactsProvider2 insertData → markRawContactDirtyAndChanged, 3407-3436]**. Another app can therefore add or edit X- properties of a contact, provided the result is a valid vCard of the same version.
- Data rows with mimetypes DAVx⁵ does not know (Signal, WhatsApp, an app's private mimetype…) are left untouched when a server update arrives: DAVx⁵ deletes only the mimetypes it builds (AndroidContact.kt:146-158). They are never uploaded (ContactProcessor.kt:81-90).

### 2.5 Groups: the two group methods

The method is a per-account setting `contact_group_method` (DX settings/AccountSettings.kt:105, 444-457) **[code]**. The default is GROUP_VCARDS. The login flow suggests GROUP_VCARDS (ui/setup/LoginModel.kt:24), except the Nextcloud login flow, which suggests **CATEGORIES** (EAM NextcloudLoginFlowFragment.kt:163). The /e/ account-details screen preselects CATEGORIES before applying the suggestion (EAM AccountDetailsFragment.kt:96, 141-160). Another app cannot read the setting (account user data); it can guess **[inferred]**:

- groups with a `sourceid` or `sync1` → GROUP_VCARDS;
- groups with only a `title` and member rows → CATEGORIES.

- **CATEGORIES**:
  - Each category is a `group_membership` row pointing to a group titled with the category, created on demand.
  - Locally added or removed memberships mark the contact dirty (provider) and it is uploaded with the new CATEGORIES.
  - A dirty group marks all its members dirty; a deleted group marks its members dirty and is removed (DX syncadapter/groups/CategoriesStrategy.kt).
- **GROUP_VCARDS** (vCard 4 `KIND:group` + `MEMBER:urn:uuid:<UID>`, or vCard 3 `X-ADDRESSBOOKSERVER-KIND/MEMBER`):
  - Groups are separate resources (ContactWriter.kt:145-157).
  - When a dirty contact's memberships differ from its cached memberships, the affected groups are marked dirty and re-uploaded (VCard4Strategy.kt).
  - Memberships coming from the server are applied after the sync from the groups' pending members (`sync3`) (LocalGroup.applyPendingMemberships).
  - **In this mode a contact's own CATEGORIES are not stored anywhere** (GroupMembershipBuilder builds nothing — "TODO: CATEGORIES <-> unknown properties", DX GroupMembershipBuilder.kt:13-16 — and ContactReader removes CATEGORIES from the leftovers). They are therefore **lost** at the next upload of that contact **[code]**.
- **/e/ starred patch** (EAM ContactsSyncManager.kt:94, 234-239; GroupMembershipBuilder.kt:23-31; /e/ vcard4android):
  - CATEGORIES value `starred` ⇄ RawContacts `starred`;
  - no `starred` group is created;
  - at upload, `starred` is added to CATEGORIES if the contact is starred. Quirk: if the contact is starred *and* already carries the category, the code's `else` branch removes it.
  - Starring through `Contacts.STARRED` marks the raw contacts dirty (ContactsProvider2.java:5296-5302), so a star toggled by any app is uploaded.

### 2.6 How DAVx⁵ detects contact changes, and what another app must write

Provider side **[code]** (ContactsProvider2.java @ cf86ae5; ContactsContract.java:874-880, 2491-2502):

- Insert, update or delete of a **Data** row by a normal app marks its raw contact `dirty` = 1 at the end of the transaction (insertData 3407-3436; DataRowHandler.java:175; TransactionContext.java:71-80; SQL at 578-581, 3041-3048).
- Changing `starred` or the raw contact's account also marks it dirty (5133-5153).
- Inserting a bare raw contact with no data row does not mark it dirty by itself **[inferred: insertRawContact 3295-3338 sets no dirty flag]**.
- Delete by a normal app of a raw contact that has an account: `deleted` = 1 and `dirty` = 1, and it leaves its aggregate (markRawContactAsDeleted 4451-4463). This happens even if it was never uploaded. A sync-adapter delete is immediate (4329-4334).
- ContactsProvider does **not** forbid normal apps from writing `sourceid` or `sync1…4`. The restriction in CalendarProvider has no equivalent here **[inferred: no check found in ContactsProvider2]**.

DAVx⁵ side (DX SyncManager.kt as in §1.5; LocalAddressBook.kt:206-232, 316-338) **[code]**:

- Deleted rows: HTTP DELETE if `sourceid` is set, then a hard delete.
- Dirty rows: PUT. A new row gets UID = `sync1` or a fresh UUID, and file name `<UID>.vcf`, with `If-None-Match: *`. An existing row is uploaded with `If-Match`.
- 412 → the server copy wins.
- The vCard version is **4.0 if the server advertises it**, else 3.0. jCard is disabled (ContactsSyncManager.kt:131-154, 224-252).
- After a full listing, non-dirty raw contacts with `sync4` = 0 are deleted (LocalAddressBook.kt:221-232).
- A server update rewrites all DAVx⁵-known data rows (delete + insert), so their `_id`s change, and re-inserts the photo (AndroidContact.kt:136-166).

**Creating a contact to be uploaded** (as a normal app, recommended):

- One `applyBatch` that inserts:
  - the RawContacts row: `account_name` and `account_type` of the DAVx⁵ **address-book** account (not the main account), and optionally `sync1` = the app's own UID (a URL-safe UUID);
  - its Data rows, using the mimetypes and columns above.
- Leave `sourceid`, `sync2`, `sync3` and `sync4` empty.
- The provider sets `dirty` through the Data inserts, and the change notification triggers DAVx⁵'s upload sync.
- Do not target read-only address books.
- Group membership by category: add `group_membership` rows pointing to a group (same account) titled with the category. Create the group as a normal app if needed.
- To keep extra vCard fields, write or extend the `x.davdroid/unknown-properties` row as a valid vCard in the address book's version.

---

## 3. Tasks: OpenTasks contract (/e/ Tasks `foundation.e.tasks`; upstream OpenTasks `org.dmfs.tasks`)

### 3.0 Provider identity: /e/ Tasks vs upstream OpenTasks (verified)

| | Upstream OpenTasks | /e/ Tasks 1.4.5 |
|---|---|---|
| Repository @ commit | github.com/dmfs/opentasks @ `3ccb6f9d0195` (master, 2021-03-30; latest tag 1.4.2) | gitlab.e.foundation `e/os/tasks` (project 552) @ `52699e59c1c0` "Bump version to 1.4.5" (2026-01-26). Its `upstream/master` branch is the same `3ccb6f9d0195`. |
| Provider authority | `org.dmfs.tasks` (opentasks-provider/src/main/res/values/opentasks_defaults.xml:5-6) | `foundation.e.tasks` (same file) |
| Read / write permissions | `org.dmfs.permission.READ_TASKS` / `org.dmfs.permission.WRITE_TASKS`, protectionLevel `dangerous`, group `org.dmfs.tasks.permissiongroup.Tasks` (opentasks-provider AndroidManifest.xml) | `foundation.e.permission.READ_TASKS` / `foundation.e.permission.WRITE_TASKS`, `dangerous`, group `foundation.e.tasks.permissiongroup.Tasks` |

**Method.** The study compared all 90 main source files of `opentasks-contract` and `opentasks-provider` between the two commits, after removing comments and whitespace (the /e/ tree was reformatted with Spotless). Besides the authority and permission names, the code differs only in:

1. `TaskProvider.java` no longer sends the `org.dmfs.tasks.action.STALE_LIST_BROADCAST` intent (a UI prompt when a list belongs to an unknown account);
2. `PendingIntent.FLAG_IMMUTABLE` was added in `TaskProviderBroadcastReceiver`;
3. `getColumnIndexOrThrow` replaces `getColumnIndex` in `RelationHandler`;
4. import order and modifier order.

**The contract (tables, columns, mimetypes, values) and the provider rules are identical** **[code]**.

The `TaskContract.java` bundled in ical4android (and therefore in DAVx⁵) matches upstream's, apart from three `ACCESS_LEVEL_*` constants and one `IS_RECURRING` constant **[code]**.

DAVx⁵ chooses the tasks app in `TaskUtils.currentProvider()`: the preferred authority first, otherwise the first one installed in enum order **jtx Board → tasks.org → OpenTasks** (I4A TaskProvider.kt:30-32; DX resource/TaskUtils.kt) **[code]**. /e/ sets no default preference (EAM settings: no `preferred_tasks_provider` default found). On an /e/ phone where jtx Board is also installed and the user never picked a tasks app, DAVx⁵ would therefore use jtx Board **[inferred]**. See §4.0 for why that matters on /e/.

### 3.1 Task lists (`content://<authority>/tasklists`)

Written by DAVx⁵ as sync adapter (DX resource/LocalTaskList.kt; I4A AndroidTaskList.kt:9-15) **[code]**:

| Column | Value |
|---|---|
| `account_name` / `account_type` | main account (as CalendarContract) |
| `_sync_id` | **CalDAV collection URL** |
| `list_name` / `list_color` | display name (or last URL segment) / colour |
| `list_access_level` | 700 (owner) if writable, else 200 (read) |
| `list_owner` | account name |
| `sync_enabled` / `visible` | 1 / 1 at creation |
| `sync_version` | DAVx⁵ sync state (CTag or sync-token) |

As for calendars, lists of the account whose `_sync_id` is not a selected remote collection are **deleted** at each sync (DX syncadapter/TaskSyncer.kt:74-100) **[code]**.

### 3.2 Task rows (`content://<authority>/tasks`)

Mapping (I4A AndroidTask.kt buildTask 447-529, populateTask 134-236; Task.kt; DX resource/LocalTask.kt) **[code]**:

| iCalendar | Tasks column and format |
|---|---|
| UID | `_uid` |
| (file name) | `_sync_id` = `<UID>.ics`. For tasks **no character check** is made. A missing UID is generated at upload (LocalTask.kt:56-75). |
| (ETag) / (flags) | `sync1` / `sync2` (0 or 1) |
| SEQUENCE | `sync_version` (incremented on every upload of a dirty task, LocalTaskList.findDirty) |
| SUMMARY / LOCATION / DESCRIPTION / URL | `title` / `location` / `description` / `url` |
| GEO | `geo` = `"<longitude>,<latitude>"`, **longitude first** |
| COLOR | `task_color` = ARGB int. At upload, the nearest CSS3 colour **name** is written (Task.kt:105, 185). |
| ORGANIZER | `organizer` = e-mail only |
| PRIORITY | `priority` 0-9. 0 (undefined) is not written to iCalendar; the provider turns 0 into NULL. |
| CLASS | `class`: PUBLIC → 0; PRIVATE and any unknown value → 1; CONFIDENTIAL → 2; absent → NULL |
| STATUS | `status`: NEEDS-ACTION or absent → 0, IN-PROCESS → 1, COMPLETED → 2, CANCELLED → 3. When read back, a task without STATUS becomes `STATUS:NEEDS-ACTION`. |
| COMPLETED | `completed` (ms, UTC), `completed_is_allday` = 0 |
| PERCENT-COMPLETE | `percent_complete` (the provider rejects values outside 0-100) |
| DTSTART / DUE | `dtstart` / `due` in ms. `is_allday` = 1 if DTSTART (or else DUE) is a DATE, **or if both are absent**. `tz` = NULL for all-day, otherwise the TZID of DTSTART (or DUE; `UTC`; or the device zone) (Task.kt:244-248; AndroidTask.kt:498-507, 532-547) |
| DURATION | `duration` = the RFC 5545 string |
| CREATED / LAST-MODIFIED | `created` / `last_modified` (ms) |
| RRULE | `rrule`. **Only one RRULE is kept** (`Task.rRule` is a single field). |
| RDATE / EXDATE | `rdate` / `exdate` = comma-separated RFC 5545 values: `yyyyMMdd` for all-day, otherwise `yyyyMMdd'T'HHmmss` in `tz`, or with `Z` for UTC (AndroidTimeUtils.kt:256-281) |
| RELATED-TO, CATEGORIES, VALARM, other properties | rows in the Properties table (§3.3) |
| RECURRENCE-ID | **not mapped**. It falls into the unknown properties, and DAVx⁵ ignores any resource holding more than one VTODO, so recurrence exceptions of tasks are **not synced** (TasksSyncManager.kt:133-152) |
| PRODID, DTSTAMP | dropped or regenerated |

`parent_id` is set to NULL on write; the provider recomputes it from the parent relation row (AndroidTask.kt:462-463).

### 3.3 Properties table (`content://<authority>/properties`)

The common columns are `property_id`, `task_id`, `mimetype`, `prop_version`, `data0`…`data15` and `prop_sync1…8` (TC:1323-1384). The rows DAVx⁵ writes **[code]**:

| Mimetype | Columns | Mapping |
|---|---|---|
| `vnd.android.cursor.item/alarm` | `data0` minutes_before, `data1` reference, `data2` message, `data3` alarm_type | VALARM → minutes before the reference: `RELATED=END` → reference 1 (due), else 2 (start). Message = DESCRIPTION or the summary. Type: DISPLAY → 1, EMAIL → 2, AUDIO → 4, else 0. Read back as `TRIGGER;RELATED=START/END:-PT<n>M` with the matching ACTION and DESCRIPTION (AndroidTask.kt:256-282, 355-389; TC:1650-1703) |
| `vnd.android.cursor.item/category` | `data1` category_name (the provider fills `data0` category_id and `data2` colour) | one row per CATEGORIES value (AndroidTask.kt:391-400; TC:1507-1539) |
| `vnd.android.cursor.item/relation` | `data3` **related_uid**, `data2` **related_type** (0 PARENT, 1 CHILD, 2 SIBLING); `data1` related_id is filled by the provider; `data5` related_content_uri | RELATED-TO value → `related_uid`. RELTYPE CHILD → 1, SIBLING → 2, **anything else (PARENT, absent, DEPENDS-ON, FINISHTOSTART, X-…) → 0 = PARENT**. Other parameters are dropped. When reading, rows without `related_uid` are skipped, and a `parent_id` without a parent relation produces a RELATED-TO to the parent's `_uid` (AndroidTask.kt:100-117, 284-304, 402-420; TC:1589-1645) |
| `vnd.android.cursor.item/vnd.ical4android.unknown-property` | `data0` = the same JSON array as for events: `["NAME","value",{"PARAM":"v"}]` | every VTODO property not listed in Task.fromVToDo (Task.kt:96-125) (AndroidTask.kt:422-436) |

Defined by the OpenTasks contract but **not used by DAVx⁵ 4.3.9**: `…/comment` (data0 comment, data1 language), `…/attendee`, `…/attachment`, `…/contact`. COMMENT, ATTENDEE, ATTACH and CONTACT therefore travel as unknown-property rows.

**4.3.9 bug [code]:** in `insertUnknownProperties`, a property longer than 25 000 characters triggers `return` instead of `continue`, which **drops every following unknown property** of that task (AndroidTask.kt:424-427). The bug is fixed in DAVx⁵ 4.5.20 (§5).

### 3.4 Round trip of the properties Sioul cares about (DAVx⁵ 4.3.9 + OpenTasks)

| Property | Where it lives | Survives? |
|---|---|---|
| `LINK` (RFC 9253) | unknown-property row | **yes**, verbatim, if ≤ 25 000 characters; repeated parameters collapse |
| `RELATED-TO;RELTYPE=PARENT` (or no RELTYPE) | relation row, type 0 | **yes**. A missing RELTYPE comes back as an explicit `RELTYPE=PARENT`. |
| `RELATED-TO;RELTYPE=CHILD` / `SIBLING` | relation row, type 1 / 2 | **yes** |
| `RELATED-TO;RELTYPE=DEPENDS-ON` (and other RFC 9253 types: FINISHTOSTART, …, REFID…) | relation row, **type 0** | **no**: rewritten as `RELTYPE=PARENT` at the next upload of that task. Extra parameters (`GAP`, X-params) are lost. |
| `X-…` properties (with parameters) | unknown-property rows | **yes**, with the same limits |
| `CATEGORIES` | category rows | **yes** (as a set of names; all values go back into one CATEGORIES line) |
| `PERCENT-COMPLETE` | `percent_complete` | **yes** |
| `ESTIMATED-DURATION` | not mapped. ical4j parsing is relaxed (`ical4j.parsing.relaxed=true`, lib/src/main/resources/ical4j.properties), so it is parsed as a generic property → unknown-property row | **yes** **[inferred: relaxed parsing → generic property → JSON row; not run]** |
| `COMMENT`, `ATTENDEE`, `ATTACH`, `CONTACT`, `RESOURCES`, `REQUEST-STATUS`, `EXRULE` | unknown-property rows | yes (≤ 25 000 characters each) |
| VALARM details beyond minutes, reference and action | none | no |
| a second RRULE | none | no |
| RECURRENCE-ID overrides | none | no (the whole resource is ignored) |

### 3.5 OpenTasks provider rules (identical in /e/ Tasks) and what they mean for an app writing tasks

All **[code]**, in dmfs/opentasks @ 3ccb6f9, `opentasks-provider/src/main/java/org/dmfs/provider/tasks/`.

- **Sync-adapter flag**: the URI query parameter `caller_is_syncadapter=true`, plus `account_name` and `account_type` (TaskProvider.java:241-255; TC:57-72).
- A **normal app's insert or update** of a task sets `_dirty` = 1 and `last_modified` = now, plus `created` = now on insert. A `completed` without `status` sets status 2, and priority 0 → NULL (processors/tasks/AutoCompleting.java:54-125).
- **No-op updates are ignored**: if every supplied value equals the stored one, nothing runs and the task is not dirtied (model/CursorContentValuesTaskAdapter.java:120-123; TaskProvider.java:1110-1117).
- **Validation** (processors/tasks/Validating.java):
  - insert requires `list_id`;
  - a normal app may not write `_dirty`, `created` or `last_modified`, nor change `_uid` or `original_instance_*` on update;
  - *nobody* may write `_deleted`;
  - `tz` is required when `dtstart` or `due` is set and the task is not all-day;
  - `due` must be ≥ `dtstart`, and `due` and `duration` are exclusive;
  - `class` must be 0-2, `priority` 0-9, `percent_complete` 0-100 and `status` 0-3.
- **Delete** by a normal app → `_deleted` = 1, unless the list belongs to the local account. A sync-adapter delete removes the row immediately (processors/tasks/TaskCommitProcessor.java).
- **Changes to Properties rows** (categories, alarms, relations, unknown properties) by anyone do **not** dirty the task (handler/PropertyHandler.java:74-123; TaskProvider.java:978-1005, 1180-1210). Relation rows only update `parent_id`.
- Non-sync-adapter changes call `notifyChange(…, syncToNetwork=true)`, which triggers DAVx⁵'s upload sync (SQLiteContentProvider.java:332-333; TaskProvider.java:1386-1389).
- Completing one instance of a recurring task in a tasks app "detaches" it into a standalone, unsynced task: `_sync_id` and `_uid` are wiped and the task is dirty (processors/instances/Detaching.java:290-314). DAVx⁵ then uploads it as a new task with a new UID.

**Recipe for an app writing tasks** **[inferred from the above]**:

1. Read with the provider of the phone. On /e/: `content://foundation.e.tasks/tasks` and `/properties`, permissions `foundation.e.permission.READ_TASKS/WRITE_TASKS`. Upstream: `org.dmfs.tasks`, `org.dmfs.permission.*`. (tasks.org exposes the same contract under `org.tasks.opentasks` with `org.tasks.permission.READ_TASKS/WRITE_TASKS`, I4A TaskProvider.kt:31, 59-60.)
2. **Create** as a normal app:
   - insert the task with `list_id` (a DAVx⁵ list), `_uid` (a URL-safe UUID, allowed on insert), `title`, `dtstart`/`due` + `tz` or `is_allday`, and so on;
   - then insert the Properties rows (category, relation by `related_uid`, unknown-property JSON);
   - the insert alone makes the task dirty.
3. **Update** as a normal app: change at least one real column of the task row. If only Properties rows changed, also change something real (e.g. a trimmed or normalised field), or switch to sync-adapter mode for that call and write `_dirty` = 1 **plus** `last_modified` = now. A no-op update will **not** trigger an upload.
4. **Delete** as a normal app (the provider soft-deletes). Never delete as a sync adapter: DAVx⁵ would never send DELETE.
5. Do not write `_sync_id`, `sync1`, `sync2` or `sync_version`. Do not create task lists in DAVx⁵ accounts.
6. Encode DEPENDS-ON and other RFC 9253 RELTYPEs another way if they must survive DAVx⁵ (for example also as an `X-` property or a `LINK`). OpenTasks + DAVx⁵ will rewrite them to PARENT.

---

## 4. jtx Board provider (`at.techbee.jtx.provider`) for VTODO and VJOURNAL

### 4.0 Is it usable through DAVx⁵ on each phone?

- **Upstream DAVx⁵ OSE 4.3.9** declares a sync adapter for jtx: `JtxSyncAdapterService` → `@xml/sync_notes` with `contentAuthority="at.techbee.jtx.provider"` (DX app/src/main/res/xml/sync_notes.xml; AndroidManifest.xml:199-208) **[code]**.
- **/e/ Account Manager 4.3.9-16** still ships JtxSyncer, JtxSyncManager, LocalJtxCollection and LocalJtxICalObject, identical to upstream except one notification-tag line. However, /e/ commit `7cf92c37e13f` (2022-10-25, "6143-Fix_sync_authority_missing_issue") **repointed `sync_notes.xml` to the /e/ Notes authority** `foundation.e.notes.android.providers.AppContentProvider`. No sync-adapter XML of the fork declares `at.techbee.jtx.provider` (EAM res/xml/*; manifest:244-253) **[code]**.
- Consequence **[inferred]**: on /e/, local jtx changes never trigger DAVx⁵ through the Android sync framework. JtxSyncer can still run when DAVx⁵ itself schedules work for that authority, i.e. when jtx Board is the selected tasks app (SyncWorker routes the authority to JtxSyncer, EAM SyncWorker.kt:317-318). Whether this works in practice was not verified. **On /e/, treat jtx Board as unsupported**; on upstream DAVx⁵ it is supported.

### 4.1 Identity and access rules

- Authority `at.techbee.jtx.provider`; package `at.techbee.jtx`. DAVx⁵ requires version ≥ 2.04.03 (I4A TaskProvider.kt:30).
- Permissions `at.techbee.jtx.permission.READ` / `at.techbee.jtx.permission.WRITE` (I4A TaskProvider.kt:63-64; jtx manifest provider: `readPermission="${applicationId}.permission.READ"`, `writePermission="${applicationId}.permission.WRITE"`, applicationId `at.techbee.jtx`). Where jtx declares these permissions, and their protection level, was not found.
- **The jtx provider only accepts sync-adapter calls**, i.e. every query, insert, update and delete:
  - without `caller_is_syncadapter=true` it throws "Currently only Syncadapters are supported";
  - `account_name` and `account_type` are mandatory, and queries are limited to collections of that account;
  - the local jtx account type is refused to other apps. Sources: github.com/TechbeeAT/jtxBoard @ `702e6c1e` (develop, 2026-09-11), app/src/main/java/at/techbee/jtx/SyncContentProvider.kt:777-806 **[code]**.
- So another app would act as a "sync adapter" for DAVx⁵'s account. **Nothing marks rows dirty for it** (no dirty handling in SyncContentProvider) **[inferred from absence]**. It must therefore:
  - set `dirty` = 1 itself;
  - mark deletions with `deleted` = 1, or DAVx⁵ will never send DELETE;
  - never touch `filename`, `etag`, `scheduletag` or `flags`.

### 4.2 Tables and columns (contract in I4A at/techbee/jtx/JtxContract.kt, VERSION 6) **[code]**

URIs are `content://at.techbee.jtx.provider/<path>`:

- **`collection`**:
  - `_id`, `url` (DAVx⁵: the CalDAV collection URL), `displayname`, `description`, `owner`, `ownerdisplayname`, `color`;
  - `supportsVEVENT`, `supportsVTODO`, `supportsVJOURNAL`;
  - `accountname`, `accounttype` (the DAVx⁵ main account), `syncversion` (DAVx⁵ sync state), `readonly`.
  - DAVx⁵ creates one per CalDAV collection (DX resource/LocalJtxCollection.kt:20-40).
- **`icalobject`**, the main row:
  - identity and grouping: `_id`, `module` (JOURNAL/NOTE/TODO), `component` (`VTODO` or `VJOURNAL`, required), `collectionId`;
  - text fields: `summary`, `description`, `status` (free text, e.g. `NEEDS-ACTION`), `xstatus` (= X-STATUS), `classification` (text), `url`, `contact`;
  - places: `geolat`, `geolong`, `location`, `locationaltrep`, `geofenceRadius` (= X-GEOFENCE-RADIUS);
  - progress: `percent`, `priority`;
  - times:
    - `dtstart`, `due`, `completed`, `dtend` (all ms);
    - the matching time zone columns `dtstarttimezone`, `duetimezone`, `completedtimezone`, `dtendtimezone`. Each holds a Java zone ID, NULL = floating, or **`ALLDAY`**.
    - `duration`.
  - recurrence: `rrule`; `rdate` and `exdate` as **comma-separated ms timestamps**; `recurid` + `recuridtimezone`; `original_id`.
  - other iCalendar fields: `uid`, `created`, `dtstamp`, `lastmodified`, `sequence`, `color` (ARGB int);
  - sync fields: `dirty`, `deleted`, `filename`, `etag`, `scheduletag`, `flags`;
  - `other`: JSON object `{name: value}` of extra properties. (JtxContract.kt:175-576)
- **Sub-tables** (each has `icalObjectId`):
  - `attendee` (caladdress, cutype, member, role, partstat, rsvp, delegatedto, delegatedfrom, sentby, cn, dir, language, other);
  - `category` (text, language, other);
  - `comment` (text, altrep, language, other);
  - `organizer` (caladdress, cnparam, dirparam, sentbyparam, language, other);
  - **`relatedto`** (`text` = related UID, **`reltype` = text**, `linkedICalObjectId`, `other` = JSON of other parameters);
  - `resource` (text, language, other);
  - `attachment` (uri, binary, fmttype, filename, other);
  - `alarm` (action, description, summary, attendee, duration, repeat, attach, triggerTime, triggerTimezone, triggerRelativeTo, triggerRelativeDuration, other);
  - **`unknown`** (`value`). (JtxContract.kt:622-1447)

### 4.3 How DAVx⁵ maps VTODO/VJOURNAL into jtx (I4A JtxICalObject.kt) **[code]**

- Every VTODO and VJOURNAL in a resource becomes a row. RECURRENCE-ID components become rows with `recurid` and are matched to existing instances (JtxICalObject.kt:221-252; DX JtxSyncManager processICalObject).
- DAVx⁵ lists VTODO and VJOURNAL separately, by full listing every time (JtxSyncManager.kt:76-88).
- **RELATED-TO, on the way in**: `text` = value; **`reltype` = the RELTYPE text as received, e.g. `DEPENDS-ON` stays `DEPENDS-ON`** (default `PARENT`); other parameters go to `other` as JSON (JtxICalObject.kt:455-468; JtxContract.kt:1025). Unlike OpenTasks, every RFC 9253 relation type is stored. jtx Board itself only acts on PARENT, CHILD and SIBLING.
- **RELATED-TO, on the way out** (checked on 8 October 2026; the study had every relation type surviving the round trip): to upload an object the phone changed, DAVx⁵ reads back only its relation rows whose `reltype` is `PARENT` (`populateFromContentValues`, JtxICalObject.kt:1529-1543, reached from LocalJtxCollection's `findDirty`), and writes a RELATED-TO only for PARENT, CHILD or SIBLING, with that RELTYPE alone and the `other` parameters left out (`getICalendarFormat`, JtxICalObject.kt:881-892) **[code]**. So the upload drops the object's CHILD, SIBLING, DEPENDS-ON and other relations, and the parameters (`GAP`…) of its PARENT ones. In DAVx⁵ 4.5.20, the code that writes RELATED-TO from a jtx row still skips every type other than PARENT, CHILD and SIBLING, and writes no other parameter (synctools, `mapping/jtx/handler/RelatedToHandler.kt:30-40`) **[code]**; [compatibility.md](../compatibility.md) reads it so too.
- **Unknown properties**: every property not mapped (`LINK`, `X-…` other than the three jtx ones, `ESTIMATED-DURATION`, `REQUEST-STATUS`…) becomes one row in `unknown` with `value` = **the same JSON array as the ical4android unknown property** `["NAME","value",{params}]` (JtxICalObject.kt:527-532, 875-877, 1348-1356).
- CATEGORIES → `category` rows; COMMENT → `comment`; RESOURCES → `resource`; ATTACH → `attachment` (binary as Base64); ATTENDEE and ORGANIZER with all their parameters; VALARM → `alarm` with the leftover alarm properties in `other` (JtxICalObject.kt:260-296, 405-522).
- PERCENT-COMPLETE → `percent` (VTODO only). DUE and COMPLETED are VTODO-only. DTEND is rejected for VTODO/VJOURNAL (JtxICalObject.kt:332-370).
- File name at upload = `<uid>.ics`; no UID is generated (JtxICalObject.kt:1082-1084). Change detection: `dirty`=1 or `deleted`=1 with `recurid IS NULL` in the collection (JtxCollection.kt:103-135). After a full listing, rows with `dirty`=0 and `flags`=0 are deleted (JtxCollection.kt:216-217).

**Round trip through jtx**: LINK, X- properties, CATEGORIES, PERCENT-COMPLETE and ESTIMATED-DURATION (as unknown) survive, with the same ≤ 25 000-character and parameter-collapse limits for unknown rows (unknown rows are read back whole at upload, JtxICalObject.kt:1631-1640, and written at 875-878 **[code]**). Relations do not: in 4.3.9 only PARENT ones survive, without their parameters (above). This is the richer store for everything but relations, but on /e/ it is not synced (§4.0).

---

## 5. Upstream DAVx⁵ OSE (`at.bitfire.davdroid`) vs the /e/ Account Manager

### 5.1 Same base version (4.3.9): identical mapping (verified, §0)

For calendars and contacts the provider mapping of /e/ 4.3.9-16 is byte-identical to upstream DAVx⁵ OSE v4.3.9-ose:

- the same ical4android and vcard4android commits;
- the same `LocalEvent`, `LocalContact`, `LocalGroup` and contactrow files;
- in particular the same unknown-property formats: event ExtendedProperty `vnd.android.cursor.item/vnd.ical4android.unknown-property` + JSON array, and contact data row `x.davdroid/unknown-properties` + vCard text.

Only these points differ:

| Point | Upstream DAVx⁵ OSE | /e/ Account Manager |
|---|---|---|
| Main account types | `bitfire.at.davdroid` | `e.foundation.webdav`, `.eelo` (Murena), `.google`, `.yahoo` |
| Address-book account type | `at.bitfire.davdroid.address_book` | `foundation.e.accountmanager.address_book` and `.eelo` / `.google` / `.yahoo` variants |
| OpenTasks authority / permissions | `org.dmfs.tasks` / `org.dmfs.permission.*` | `foundation.e.tasks` / `foundation.e.permission.*` |
| Starred contacts | not mapped | CATEGORIES `starred` ⇄ RawContacts `starred` |
| jtx Board sync adapter | declared | removed (§4.0) |
| Group method preselected at account creation | GROUP_VCARDS (CATEGORIES for the Nextcloud login flow) | the account screen preselects CATEGORIES, then applies the login suggestion |

### 5.2 Current upstream release: DAVx⁵ OSE v4.5.20-ose (2026-09-27)

What an upstream phone gets today. The mapping now lives in the in-repo module `synctools` (davx5-ose @ v4.5.20-ose, `synctools/src/main/kotlin/at/bitfire/synctools/…`); ical4j is 4.3.0.

**Storage formats unchanged** **[code]**:

- events: `sync_data1` = ETag, `sync_data2` = flags, `sync_data3` = SEQUENCE, `sync_data4` = Schedule-Tag (storage/calendar/EventsContract.kt:32-53); `_sync_id` = file name and `original_sync_id` on exceptions (mapping/calendar/builder/SyncIdBuilder.kt); `uid2445` = UID (UidBuilder.kt);
- extended-property names `categories` (backslash-separated), `vnd.android.cursor.item/vnd.ical4android.url` and **`vnd.android.cursor.item/vnd.ical4android.unknown-property`**, with the same JSON array (mapping/UnknownProperty.kt:31, 78-90). There is **no `.v2`** name in DAVx⁵ up to 4.5.20.
- contacts: `sourceid` / `sync1` UID / `sync2` ETag / `sync3` hash / `sync4` flags, `x.davdroid/unknown-properties` in data1 (vCard text) and `x.davdroid/cached-group-membership` (storage/contacts/AddressContract.kt:42-115);
- tasks: `sync1` = ETag, `sync2` = flags, unknown property JSON in `data0` (storage/tasks/DmfsTasksContract.kt:28-40);
- account types `bitfire.at.davdroid` / `at.bitfire.davdroid.address_book` (core/src/main/res/values/strings.xml:11-12).

**Behaviour changes worth knowing** **[code]**:

- Events:
  - exceptions now get *all* builders, so they keep CATEGORIES, URL and unknown properties (mapping/calendar/AndroidEventBuilder.kt).
  - An absent CLASS is stored as `accessLevel` 0 (DEFAULT), no longer 3 (PUBLIC). CONFIDENTIAL and x-values are still retained as an unknown-property row (builder/AccessLevelBuilder.kt).
  - The ExtendedProperty `iCalUid` (Google Calendar's UID store) is now known (EventsContract.kt:83).
  - LAST-MODIFIED, DTSTAMP and PRODID are still dropped (UnknownPropertiesBuilder KNOWN_PROPERTY_NAMES).
- Tasks:
  - COMMENT is now mapped to OpenTasks `…/comment` rows (`data0`) (mapping/tasks/builder/CommentsBuilder.kt);
  - the unknown-property bug is fixed (`continue`);
  - EXRULE is kept as an unknown property;
  - **RELTYPE mapping unchanged: anything but CHILD/SIBLING → PARENT** (RelationsBuilder.kt, RelationsHandler.kt).
- Contacts: in GROUP_VCARDS mode CATEGORIES are still not stored (same TODO in mapping/contacts/builder/GroupMembershipBuilder.kt).
- jtx: the same contract use. On the way in, RELATED-TO keeps its RELTYPE text and unknown rows keep the same JSON (mapping/jtx/builder/RelatedToBuilder.kt, UnknownPropertiesBuilder.kt); on the way out, only PARENT, CHILD and SIBLING are written, without other parameters (mapping/jtx/handler/RelatedToHandler.kt:30-40, checked on 8 October 2026).

**For a bridge**: one implementation of the formats in §§1-4 works for /e/ 4.3.9-16, upstream 4.3.9 and upstream 4.5.20. Only the account types and the tasks authority and permissions need to be parameterised.

---

## 6. Things that make DAVx⁵ misbehave when another app writes the rows

Most important first. "[code]" points to the sections above; "[inferred]" is a reading of the code, not run.

1. **Writing as a sync adapter without setting the dirty flag.**
   - The change is never uploaded.
   - A *new* row, whose flags column is 0 (`sync_data2` / `sync4` / tasks `sync2` / jtx `flags`), is **deleted** at the next full listing: always for tasks and jtx (PROPFIND/REPORT every time); for calendars when the server has no sync-collection or a time range is set; for any collection at initial sync or full resync.
   - [code] §1.5 step 6, §2.6, §3.5, §4.3. Normal-app writes avoid this, because the provider sets dirty (calendar, contacts, OpenTasks). jtx accepts only sync-adapter writes, so there the app must set `dirty` = 1 itself.
2. **Hard deletes as a sync adapter.** DAVx⁵ never sends DELETE, and the item comes back from the server at the next sync. Use a normal-app delete (calendar, contacts, OpenTasks soft-delete), or set `deleted` = 1 + `dirty` = 1 (CalendarContract, jtx). OpenTasks lets nobody write `_deleted` directly, so use a normal delete there. [code]
3. **Touching DAVx⁵'s bookkeeping columns**:
   - `_sync_id`/`sourceid`/`filename` (the file name, used by `findByName`),
   - `sync_data1`/`sync2`/`sync1`/`etag` (the ETag),
   - `sync_data3`/`sync_version` (SEQUENCE),
   - `sync_data4` (Schedule-Tag),
   - the flags columns. Wrong values give duplicates (a second resource on the server), overwritten items, or a 412 on upload. A normal app is blocked from these columns by CalendarProvider only. ContactsProvider and jtx do not block them. [code]
4. **Conflicts: the server wins silently.** A PUT that gets 412 or 409 clears the dirty flag and nulls the ETag; the server copy is then re-downloaded over the local edit (SyncManager.kt:460-479). An app should request a sync promptly after local edits, and not rely on local-only edits surviving concurrent server changes. [code]
5. **Row `_id`s are not stable.** Each server update deletes and re-inserts reminders, attendees, DAVx⁵'s extended properties and **all exception events** (AndroidEvent.kt:640-707). Contact data rows are re-inserted too (AndroidContact.kt:136-166), and task Properties rows as well (AndroidTask.kt:324-346). A STATUS change to "none" re-creates the whole event with a new `_id`. **Key links on UIDs** (`uid2445`, contacts `sync1`, tasks `_uid`, jtx `uid`), never on `_id`. [code]
6. **Collections created inside DAVx⁵ accounts are deleted** (calendars, task lists, jtx collections) or, without a URL, never synced. Address books are AccountManager accounts owned by DAVx⁵. [code]
7. **Calendar ExtendedProperties are sync-adapter-only.** A normal app cannot set or edit event CATEGORIES, URL or X-/LINK/RELATED-TO properties (CP2 4621-4632). Doing it requires sync-adapter mode, with rule 1 applied by hand. [code]
8. **Value traps** [code]:
   - `eventStatus` 0 means TENTATIVE (use NULL for none);
   - `availability` 2 is uploaded as OPAQUE;
   - in 4.3.9 a missing CLASS comes back as `CLASS:PUBLIC` after a local edit;
   - an event colour must be a CSS3 key present in `Colors` (only when the account's "event colours" setting is on); a raw `eventColor` is ignored;
   - all-day events need UTC-midnight `dtstart` and `eventTimezone` = `UTC`;
   - RDATE is dropped when there is an infinite RRULE;
   - an RDATE/EXDATE list uses the time zone of its first entry for every entry;
   - tasks without DTSTART/DUE are written `is_allday` = 1;
   - only one task RRULE is kept;
   - task GEO is stored longitude first.
9. **Recurrence exceptions** [code]:
   - Events: create them through `Events.CONTENT_EXCEPTION_URI/<master id>` so the provider fills `original_id`/`original_sync_id`. The link only works once the master has a `_sync_id`, i.e. has been uploaded (AndroidEvent.kt:571-584 comment).
   - Cancelled exceptions are uploaded as EXDATE.
   - In 4.3.9 exceptions lose CATEGORIES, URL and X-props (fixed in 4.5.20).
   - Task exceptions are not synced by DAVx⁵ (OpenTasks); OpenTasks "detaching" turns completed instances into new standalone tasks.
10. **OpenTasks does not dirty a task when only its Properties change**, and it ignores no-op updates. Category, relation or unknown-property edits must come with a real change to the task row, or with `_dirty` = 1 written in sync-adapter mode. [code] §3.5
11. **Relationship types**: OpenTasks + DAVx⁵ (4.3.9 and 4.5.20) turn every RELTYPE other than CHILD or SIBLING (e.g. DEPENDS-ON) into PARENT. jtx Board stores them, but DAVx⁵ leaves them out when the phone's change is uploaded: all but PARENT in 4.3.9, all but PARENT, CHILD and SIBLING in 4.5.20 (§4.3, corrected on 8 October 2026). [code]
12. **Contact categories**: in GROUP_VCARDS mode (DAVx⁵'s default) a contact's CATEGORIES are dropped at the first local round trip. In CATEGORIES mode they are groups. On /e/ the category `starred` is the favourite flag. [code]
13. **Unknown-property limits**: values over 25 000 characters are dropped (4.3.9 tasks: all following ones too). Repeated parameters collapse to one (JSON object). A contact's unknown-properties row must stay a parseable vCard of the same version, or all its extra properties are dropped at upload. [code]
14. **Read-only collections**: local edits to contacts are silently reverted. For events and tasks the PUT fails; 403 with need-privileges is treated like 412, so the server copy wins. Check `calendar_access_level` / `list_access_level` / `raw_contact_is_read_only` before offering edits. [code, §1.1, §2.1, §3.1]
15. **The time range setting** ("past days" in the DAVx⁵ account) makes DAVx⁵ list only recent events and delete older non-dirty ones locally (CalendarSyncManager.kt:90-115 + SyncManager full listing). An app reading the provider sees only what DAVx⁵ keeps. [code]
16. **Privacy**: the provider records every package that edits an event in `mutators`, and DAVx⁵ appends them to the uploaded PRODID, e.g. `…ical4j/3.2.13 (org.example.app)` (AndroidEvent.kt:173-176; ICalendar.kt:72-76; Event.kt:209). The writing app's package name is visible on the server and to invitees. [code]
17. **Permissions**: `READ_/WRITE_CALENDAR`, `READ_/WRITE_CONTACTS`, and the tasks provider's own pair (`foundation.e.permission.*_TASKS` on /e/, `org.dmfs.permission.*_TASKS` upstream, `org.tasks.permission.*_TASKS` for tasks.org, `at.techbee.jtx.permission.READ/WRITE` for jtx). All are runtime ("dangerous") permissions for the OpenTasks variants. [code]
18. **Which tasks app DAVx⁵ syncs**: one at a time, the preferred one, else the first installed of jtx Board → tasks.org → OpenTasks. On /e/, installing jtx Board without choosing in DAVx⁵ may move task sync to jtx, which /e/ does not register as a sync adapter. [code + inferred]
