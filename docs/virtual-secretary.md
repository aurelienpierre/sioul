# Virtual Secretary, with Sioul

[Virtual Secretary](https://github.com/aurelienpierreeng/VirtualSecretary) sorts mail with Python filters (`00-imap-spam.py`, `01-imap-taxes.py`…) that cross-check mail, contacts and calendars. Sioul keeps those filters working, in two modes.

## Mode A: Virtual Secretary keeps sorting on the server
Nothing changes on its side: it moves messages into IMAP folders and tags them. Sioul reads where a message was put, and turns the folder into a proposal (`rules::FolderMapping` in the core):

```toml
[[virtual_secretary.map]]
folder = "INBOX.Money.Taxes"
case = "taxes-2025"

[[virtual_secretary.map]]
folder = "INBOX.Services.Notifications"
shelf = "notifications"

[[virtual_secretary.map]]
folder = "INBOX.spam"
junk = true
```
Sioul does not move mail on the server while Virtual Secretary does, so the two never fight.

## Mode B: Virtual Secretary's filters run inside Sioul
- **How it runs**: a separate Python process loads the filter files unchanged, with globals `imap`, `smtp` and `carddav` that behave like Virtual Secretary's. They talk to Sioul's core over JSON-RPC on standard input and output.
- **Why a separate process**: Python stays out of the Rust core, and the projects keep separate licences.
- **What it turns into**: the filters' actions become Sioul proposals, listed below.

| Virtual Secretary | In Sioul |
|---|---|
| `imap.get_objects(mailbox, n)` | the account's messages (the inbox is the Porch's arrivals) |
| `imap.run_filters(filter, action, runs)` | each card is filtered; how many times each filter ran on a message is kept in Sioul's database instead of hidden log files |
| `email.is_in(query, field)`, `get_sender()`, `headers`, `email["X-…"]`, `attachments`, `get_body()`, `get_date()`, `age()` | the same, from the card |
| `email.ip`, `email.domains` | the server route from `Received` |
| `spf_pass()`, `dkim_pass()`, `arc_pass()`, `authenticity_score()`, `is_authentic()` | Sioul's trust results |
| `is_newsletter()`, `is_mailing_list()` | the card's list detection |
| `move(folder)` | a proposal to file or assign a case, through the folder mapping |
| `tag(k)`, `untag(k)` | a label |
| `spam(folder)` | junk: set aside, and the classifier learns |
| `delete()` | the trash, recoverable for 30 days |
| `mark_as_read/answered/important` | flags |
| `query_referenced_emails()`, `query_replied_email()` | thread lookups in Sioul's store (later) |
| `smtp` (autoresponders) | drafts in the Outbox: Sioul never sends without you |

- **"LEARN" filters** (read-only, heavy) run on demand.
- **Permissions**: every filter starts as a proposer, and you allow a filter to act alone, filter by filter.
