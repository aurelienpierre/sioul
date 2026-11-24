---
description: Projects in Sioul - any matter you follow, for a client or for yourself, gathering its tasks, events, mail, notes, time and invoices on one line of time; its mail filed by itself as it arrives, once Sioul has checked who sent it.
---

# Projects

## In short {#in-short}

A project is any matter you follow: work for a client, whose time is billed, or one of your own, such as housing, health, a tax return or a trip. Its page gathers what belongs to it: its tasks, its events, its mail, its notes, the time you spent on it and its invoices, on one line of time, with what is coming first. As mail arrives, Sioul files it there by rules you give the project, after checking who sent it. The list of projects is one plain file on your device, among Sioul's own lists.

<figure markdown="span">
  [![The Projects page: three projects listed on the left, with New project; one open on the right: "For a client", its rate, Board, List, Gantt, Note time and Make the invoice; its tasks open and done, the time noted and left to bill; "Mail that comes here by itself", folded; its invoices, one marked Paid; then "On one line of time", what is coming and what happened before.](../assets/screens/projects.png){ loading=lazy }](../assets/screens/projects.png "Open the picture at full size")
  <figcaption>A project: what is coming, then what happened.</figcaption>
</figure>

## Protected by default {#what-is-protected}

- **Mail is checked before it is filed.** As it arrives, a forged message (its domain says it did not send it), or one that borrows a known name, is set aside first and never filed into a project, even when its address looks like your client's. The project's page checks its mail the same way each time it shows it, and so does what an AI agent reads of the project: such a message never appears on its line of time, and neither does a conversation that only such a message ties to it.
- **A message that fails the sender checks** (both of the signatures that servers use to vouch for a sender failed) is never filed by a route on your client's address or domain. A route on words of the subject, the text or an attachment's name can still file it, since those do not rest on who sent it; on the project's page, it then says "whose address could not be verified".
- **Every message filed says why it came**: "sender's domain: example.org".
- **Your projects are one plain file** on your device, among Sioul's own lists (on a device that kept them in your notes folder before, they stay there until you move them: [Where Sioul keeps its lists](sharing.md#where-sioul-keeps-its-lists)), and mail is filed on your device. Between your devices that file travels sealed through Sioul's sharing, in **Projects and money**: the folder and its server see that something changed, never what ([Sharing](sharing.md)). Before the move, your notes folder's own sync may carry it instead.
- There is no server of Sioul's, and nothing about your projects goes to one.

## Making a project {#making-a-project}

**New project** (or **New ▾ ▸ A project**):

- **Name**.
- **For a client: its time is billed at an hourly rate**, ticked when the form opens on a new project. Then: **For** (who it is for), **An hour costs** (else the rate in [Settings ▸ Invoices](settings.md#invoices)), and **Invoices expected in**: the budget where the money is expected until paid. When **For** names one of your [contacts](contacts.md) (their name, or their organisation), the invoices take its client profile when it has one ([Your clients](time.md#your-clients)), else its postal address.
- Without it, the project is yours.
- **What it is for**: work, your admin, leisure, any of them together, the same three switches as everywhere ([Hours](hours.md)). The Projects and Time pages show it in those hours, and its tasks come in them unless they say their own. Nothing ticked, a client's project counts as work; another follows its open tasks, and shows at any hour when it has none.
- **Where it stands**: open, waiting, or closed.
- **Open to AI agents**, in the form of a new project: off until you switch it on ([Choosing what an agent sees](ai-agent.md#choosing-what-an-agent-sees)). Afterwards it is changed on the project's page alone.

**Edit**, at the top of a project's page, opens this form again. **Take out of the list**, at its foot, removes the project once you confirm. Its tasks, notes, mail and time stay where they are; they are no longer gathered under it.

## A project's page {#a-projects-page}

On the left, every project, with its open tasks and the time left to bill; closed ones come last, greyed. On the right, the project open:

- **On one line of time**: what is **coming** first (dates asked of its open tasks, events), then what happened **before**, newest first: tasks done, mail, notes changed, time noted, invoices.
- **Its tasks**, as a **Board**, a **List** or a **Gantt**: each opens the [Tasks](tasks.md) page, kept to this project. The **Gantt** is the Tasks page's own ([List, Board, Gantt](tasks.md#list-board-timeline)): each task on its days, the date asked as a small diamond.
- **Note time**: a meeting, a call, work done away from the timer.
- **Make the invoice**, when billable time waits to be billed: a draft of that time opens in the editor, in Budgets ▸ Invoices, where you check it and issue it. A project with a draft already gets that draft back, with the time noted since added ([The editor](time.md#the-editor)).
- **Its client's profile**, on a project for a client: one line, such as "Client profile: A business · France · 904 123 452", or "No client profile yet: invoices bill it as a business, from its card.", with **Client profile…** (**Make its client profile…** while it has none) ([Your clients](time.md#your-clients)).
- **Its invoices**: each opens its pane in Budgets ▸ Invoices, with **Paid…**, **Write the mail**, **Print again** and **Credit note…** ([An issued invoice's pane](time.md#an-issued-invoice)). The **Paid** box beside each says it is paid, by transfer, today; on a credit note with money to give back, the box says **Paid back**. **Print again**, beside the box, copies the file issued where your invoices go, and opens it.
- **Open to AI agents**: a switch, off until you turn it on. An AI agent you connect then sees the project and what is tied to it, and may add tasks, notes, ties and drafts to it ([Choosing what an agent sees](ai-agent.md#choosing-what-an-agent-sees)).
- **Add ▾** and **Link to…**, above **Mail that comes here by itself**: something new tied to the project, or a tie to something that exists, such as the client's contact; **Ties**, everything the project holds and what that is tied to ([Ties](ties.md)).

On a phone, the list of projects fills the screen, then the project you open; **Back** returns to the list ([On a phone or a tablet](first-steps.md#on-a-phone)).

## Mail that comes here by itself {#mail-that-comes-here-by-itself}

**Mail that comes here by itself**, on the project's page, unfolds the project's routes; **Add a route** starts one, and × takes one out. A message comes to the project when one route matches; every line filled in a route must match:

- **From the domains**: the sender's domain, or any of its subdomains;
- **From the addresses**;
- **Words in the subject**;
- **Words in the text**;
- **Words in an attachment's name** ("quote" takes `Quote_2026.pdf`).

Several words or domains on one line, separated by commas: any one of them. Words are matched whole, without case or accents; in an attachment's name, they are found anywhere in the name.

A reply in a conversation of the project goes with it, and so does a message you tie to it by hand (**Link to…**): what you tie stays as you tied it, even a message the checks would set aside. Every routing keeps its reason, shown on request under **Why it is here** when you open the message from the Porch: "sender's domain: example.org".

As it arrives, and each time the project's page shows it, mail is checked before a route files it ([above](#what-is-protected)): forged mail and mail that borrows a known name are set aside first, and a route on your client's address or domain does not file a message that failed the sender checks. The other way round, a project's route is stronger than a spam flag, so your client's real mail is not lost among spam, and Sioul's own spam filter leaves a project's conversations alone.

### A task of each message {#a-task-of-each-message}

A route can make a task of each message it takes: a shop's notification of a sale, "[New sale] Gift card 50 €", becomes the task "Gift card 50 €" in the project and in your plan, with the message one press away in its ties, everything to print in it. Nothing of the mail is copied into the task.

- **Make a task of each message**, under a route, opens its form; from a message, **⋯ ▸ Make a task of mail like this…** opens the same form, filled with its sender and the words between brackets in its subject ("New sale"), in its project, or a new project you name there.
- **The task**: its name ("Send a gift card"), words before its title ("Send:"), the list it goes into (your usual one unless you choose), **the date asked**, so many business days after the mail came (3 at first: a sale on Friday is asked for Wednesday), its first estimate, and the supplies each task will use ([Supplies](supplies.md)), which count as planned at once.
- **Business days** are the days with working hours in your week (Monday to Friday when you set none), less your days off: the periods you typed, and public holidays once their calendar is subscribed ([Vacations and holidays](hours.md)). Before that calendar is subscribed, a public holiday counts as a business day.
- **Once per message, on every device**: two of your devices that receive the same mail make one task. A task you deleted is never made again.
- **Only mail the Porch lets in**: a forged "[New sale]" makes no task and plans no supplies; by its sender, only mail whose sender is proven ([above](#what-is-protected)). Mail you send yourself makes none.
- **Mail already here makes nothing by itself**: once the route is saved, **Also for the 12 messages already here** makes theirs, once, when you ask.
- Each task made is said once in the status line: "“Gift card 50 €” is in Work, made from a message, asked for Wednesday 25 November." Under the route, its line says what each task is, with **Change…**; untick the box and the route only files mail again.

## Where projects live {#where-projects-live}

In one file, `sioul-projects.toml`, among Sioul's own lists, in `records/` of Sioul's data folder (`~/.local/share/sioul/records/` on Linux): readable, and written by hand if you like. On a device that kept Sioul's lists in your notes folder before, it stays at that folder's root until you move them ([Where Sioul keeps its lists](sharing.md#where-sioul-keeps-its-lists)). It travels sealed through Sioul's sharing, in **Projects and money**; before the move, your notes folder's own sync may carry it instead ([Sharing](sharing.md)). A project's notes stay in your notes folder, and the project names them by their path there. A project's tasks and events carry its identifier in a standard field, so other calendar programs keep the grouping.

If Sioul's lists still hold `sioul-cases.toml`, the file's name before projects had one name, nothing changes: Sioul reads it and writes it as it is, so that a device not yet updated reads it too. When each of your devices has this version, **Rename to sioul-projects.toml**, in the Notes page's settings (⚙), under the notes folder, renames it, with a copy of the old file kept beside it. Links to a project written before still open it.

## By your hours {#in-quiet-time}

The button above the list says what it shows ("Leisure ▾") and opens three boxes, **Work**, **Admin** and **Leisure**, ticked as your hours say on this device: work's projects in your working hours, your admin's in your admin hours, leisure's in the evening, at meals and at night. Tick another to see its projects too: it holds until your hours change, a day at most; **As the hours say** brings theirs back at once. On a device you said is for some areas only, the boxes follow that too, and the **?** beside them says what waits on your other devices ([What each device is for](hours.md#what-each-device-is-for)). A client's project with nothing ticked counts as work; another one with nothing ticked shows for what its open tasks are for, and at any hour when it has none. When the ticks show no project, one sentence says so: "No project for what is ticked." Nothing covers the page, at any hour, and a project opened from elsewhere opens whatever the ticks. The [Time](time.md#in-quiet-time) page shares these ticks. See [Hours](hours.md).

## Going further {#going-further}

- **A lane on the Porch**: **Projects shown here**, in the Porch's ⚙, gives a project its own lane there ([The Porch](porch.md)).
- **On a device not updated yet**: **Admin** and **Leisure** together, and all three on a client's project, are written as the older **Yours, outside work**, which an older Sioul reads, and its box reads here as admin and leisure (beside a client, all three, as the older Sioul read it). Other choices are new to an older Sioul: it reads them as nothing said, and saving the project's form there takes them out. Update every device before using them.
- **By hand, if you like**: the list of projects is a readable file, `sioul-projects.toml`, which you can edit; its form is [below](#for-technical-readers).
- **An AI agent you connect** ([Using an AI agent](ai-agent.md)) sees a project only once you open it: **Open to AI agents**, on its page (or in the form that makes it). It then reads what the project's page gathers, with one-time codes, sign-in links, account and card numbers masked in its mail, and may add tasks, notes, ties and drafts to it. Of a project left closed it sees nothing ([Choosing what an agent sees](ai-agent.md#choosing-what-an-agent-sees)).

## Why it works this way {#why-it-works-this-way}

Filing mail by hand costs and fails: in 345 people's mail, finding a message by its folder took 59 seconds, by a search 17, and those who filed most found no more (Whittaker et al. 2011). So a project's mail comes to it by the routes you give it once, after Sioul checked who sent it, and you file nothing. Visual supports help many autistic people (Steinbrenner et al. 2020): a project's tasks can be drawn on a Gantt when that helps, and never must be.

The list follows your hours as Mail does, since work cues in the evening undo recovery, and everything is one tick away: a list is for managing too. More in [what the research says](../dev/research.md), findings 13, 21 and 61.

## Compared with other apps {#compared-with-other-apps}

As of October 2026, from each app's own documentation.

✓ documented; **partly**, with a note; ✗ not found in the app's own documentation (for Sioul: not built); ? not confirmed; — not applicable. Sioul's column was checked against its code.

| | Sioul | Trello | Asana | Notion | Nextcloud Deck | Vikunja |
|---|---|---|---|---|---|---|
| Mail filed into the project by itself, by your rules | ✓ | partly¹ | partly² | ?³ | ✗⁴ | ?⁵ |
| Board | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| List | ✓ | partly⁶ | ✓ | ✓ | ✗⁷ | ✓ |
| Calendar or timeline (Gantt) | ✓⁸ | partly⁶ | ✓ | ✓ | partly⁹ | ✓ |
| Tasks that wait for others | ✓ | ✗¹⁰ | ✓¹¹ | ✓ | ✗⁷ | ✓ |
| Time spent, noted on the project | ✓ | ✗¹⁰ | ✓¹² | ✗¹⁰ | ✗⁷ | partly¹³ |
| Invoices made from the project's time | ✓ | ✗¹⁰ | ✗¹⁰ | ✗¹⁰ | ✗⁷ | ✗¹⁰ |
| Links both ways to notes, people and events | ✓ | ? | ? | partly¹⁴ | ? | partly¹⁵ |
| Shared with other people | ✗¹⁶ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Tasks other apps can read (CalDAV) | ✓ | ✗¹⁰ | ✗¹⁰ | ✗¹⁰ | partly⁹ | ✓¹⁷ |
| Free software, on your own device or server | ✓ | ✗¹⁸ | ✗¹⁸ | ✗¹⁸ | ✓ | ✓ |

1. You send or forward a message to the board's own address; it becomes a card.
2. From Gmail or Outlook, by hand, with Asana's add-on. A project's own e-mail address could not be confirmed: Asana's help centre could not be read.
3. Notion Mail's pages could not be read.
4. Not in Deck's documentation. Nextcloud Mail can make a calendar task from a message, by hand, and its filters move mail into folders, not boards.
5. Not on Vikunja's features page.
6. Table, Calendar and Timeline views are in the Premium and Enterprise plans; the Planner, on a calendar, in the paid plans.
7. Not in Deck's user documentation, which describes boards of stacks of cards.
8. The project's **Gantt** button opens the Tasks page's Gantt, kept to the project.
9. Each board is offered over CalDAV as a list of tasks, read only, so cards with a date show in Nextcloud's calendar (from Deck's code).
10. Not in its documentation or its plan list.
11. From the Starter plan.
12. From the Advanced plan, estimated time against actual time.
13. A Pro feature for self-hosted instances, "in private beta", not in Vikunja's cloud.
14. Two-way relations between databases.
15. Relations between tasks, also across projects ("a subtask or blocking task").
16. Your projects are yours: they travel only between your own devices. Sioul has no team features.
17. Vikunja describes its CalDAV as "in an early alpha stage"; over CalDAV it writes only parent and child relations, not its blocking ones.
18. A hosted service under its own terms; no free-software licence is published for it.

Others do more for working together: every other app here shares a project with other people, and Notion's and Asana's many views and automations, and Trello's and Asana's integrations, go well beyond Sioul's. What Sioul keeps in one place is the client's mail filed by itself, the time noted and the invoices made from it, with its tasks kept as standard CalDAV tasks.

??? info "Sources"
    All read on 8 October 2026.

    - **Trello**, Atlassian's page on creating cards by email, and its prices and plans: <https://support.atlassian.com/trello/docs/creating-cards-by-email/>, <https://trello.com/pricing>
    - **Asana**, its prices and plans, and its page on the e-mail integrations: <https://asana.com/pricing>, <https://asana.com/apps/email>
    - **Notion**, its help pages on views, tasks and dependencies, relations and rollups, and its prices: <https://www.notion.com/help/views-filters-and-sorts>, <https://www.notion.com/help/tasks-and-dependencies>, <https://www.notion.com/help/relations-and-rollups>, <https://www.notion.com/pricing>
    - **Nextcloud Deck**, its README, its user documentation and its CalDAV code (`lib/DAV/Calendar.php`, `CalendarObject.php`), and Nextcloud Mail's user manual: <https://github.com/nextcloud/deck>, <https://deck.readthedocs.io/en/latest/User_documentation_en/>, <https://github.com/nextcloud/deck/blob/main/lib/DAV/Calendar.php>, <https://docs.nextcloud.com/server/latest/user_manual/en/groupware/mail.html>
    - **Vikunja**, its features page and its pages on CalDAV and task relations: <https://vikunja.io/features/>, <https://vikunja.io/help/caldav/>, <https://vikunja.io/help/task-relations/>

## For technical readers {#for-technical-readers}

### The file {#the-file}

`sioul-projects.toml`, in `records/` of Sioul's data folder (at the root of your notes folder until Sioul's lists move): TOML, one `[[project]]` per project (`id`, `title`, `kind = "project"` for a client's, `client`, `rate`, `budget`, `status`, `area` for what it is for, `files` for its notes, `ai`), each with its `[[project.route]]` lines, and its models of task, `[[project.model]]` (`id`, `name`, `list`, `prefix`, `business_days`, `estimate`, `supplies`), which a route names in `makes` ([A task of each message](#a-task-of-each-message)). A task a route made carries `X-SIOUL-ORIGIN:mid:<Message-ID>`, its UID made from the project and the message, and is tied to its message in `links.toml` (`how = "made"`). The file's first name, `sioul-cases.toml`, with `[[case]]` and `[[case.route]]`, reads the same and keeps its names when Sioul writes it; renaming it checks that nothing changes before it writes, and keeps `sioul-cases.toml.before-rename`. A project's tasks and events carry its `id` in `X-SIOUL-REFID` (RFC 9253 §8.3's `REFID` under Sioul's own name, which Android's account managers can send back; `REFID` is read too), so other CalDAV programs keep the grouping.

### How routes match {#how-routes-match}

- `from_domains`: the sender's domain or any subdomain ("example.org" takes "mail.example.org", never "notexample.org"); `from_addresses`: exact, case ignored; `subject_contains`, `text_contains`: whole words, case and accents ignored; `attachment_contains`: anywhere in the file's name, case and accents ignored. Every list filled must match; any item of a list will do.
- The text is read in its first 6,000 characters (the HTML part, read as text, when the text part is only a stub).
- Conversations follow `In-Reply-To` and `References`, through the messages the checks let in only: a forged reply neither shows nor joins two conversations. A message filed as it arrives is tied to its project (`links.toml`: `mid:` ↔ `sioul:project/<id>`; `sioul:case/<id>`, as written before, reads the same), so it stays when routes change. Changing a project's routes ties the mail already here that its text or attachment routes take, after the same checks.

### Security, precisely {#security-precisely}

- When mail arrives, on the Porch, on a project's page and in what an agent reads of a project, one function decides (`porch::admission`), in the order of protection: a blocked sender stays blocked; forged mail (DMARC failing under a `quarantine` or `reject` policy) and mail whose display name borrows a known one are set aside; one-time codes keep their lane; then project routes. Authentication results come from your own provider's `Authentication-Results` (RFC 8601, only from an `authserv-id` you trust) and from Sioul's own check of each message as it is fetched ([Every message checked](privacy-security.md#every-message-checked)).
- Mail that is not authenticated (SPF and DKIM both failed, no DMARC pass, no ARC seal from a forwarder you trust) is routed with its sender removed, so `from_domains` and `from_addresses` cannot take it, while subject, text and attachment routes still can. A message with no results, or with only one of the two failing, keeps its sender: a sender route then trusts the address as written.
- A matching route wins over a provider's spam flag, and Sioul's own spam filter leaves a project's conversations alone.
- The sharing part **Projects and money** seals the projects' file (`sioul-projects.toml`, or `sioul-cases.toml`) with the client profiles, budgets, bank, movements and contracts files, and, once Sioul's lists have moved, the papers' and letters' lists: each record with XChaCha20-Poly1305, under a key made from your passphrase with Argon2id (64 MiB, 3 passes) and kept in each device's keyring ([Sharing](sharing.md#what-it-protects-and-what-it-cannot-hide)).
