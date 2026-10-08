# Sioul for AI agents: the MCP server

`sioul mcp` serves what Sioul keeps on this computer to AI agents over the Model Context Protocol (MCP, revision 2025-06-18): Claude Code and Claude Desktop now; ChatGPT and local models later, through any MCP client. The agent reads the Porch, mail, tasks, the agenda, contacts, budgets, notes and projects, the phone's messages and calls, papers, contracts and paper letters, time and invoices, reminders, and the moment now, and your texts when you allow them; it adds tasks, events, notes, ties and drafts; it runs, watches and judges your own spam filter, when you let it ("The spam filter's tools"). It never sends, deletes, moves mail or money, or reads a password ([ai.md](ai.md), "Limits that do not move").

**Every project is closed to agents until you open it** ("Per-project access", below): the switch "Open to AI agents" on its page, in its form, or in Settings ▸ AI agents writes `ai = true` on its entry in `sioul-projects.toml` (`sioul-cases.toml` under the file's first name). A connected agent reads and writes the projects you open, and nothing of the others. Things in no project (the Porch's other mail, tasks and notes of no project, the agenda, contacts, the phone's messages and calls, papers, contracts) are open unless you close them (`[mcp] outside_projects = false`). What an agent reads goes to the provider of its model.

Code: `crates/sioul-cli/src/mcp.rs` and `crates/sioul-cli/src/mcp/`; the rule of who may see what, `crates/sioul-core/src/consent.rs`.

## Connecting
### Claude Code
```
claude mcp add sioul -- sioul mcp
```
- `claude mcp add --scope user sioul -- sioul mcp` makes it there in every project, not only the current one.
- Another configuration, another language: `claude mcp add sioul -- sioul --config ~/admin.toml --language fr mcp`.
- When `sioul` is not on the `PATH`, its full path: `claude mcp add sioul -- /home/you/.cargo/bin/sioul mcp`.

### Claude Desktop
In `claude_desktop_config.json` (macOS: `~/Library/Application Support/Claude/`; Windows: `%APPDATA%\Claude\`), then restart Claude Desktop:
```json
{
  "mcpServers": {
    "sioul": {
      "command": "/home/you/.cargo/bin/sioul",
      "args": ["mcp"]
    }
  }
}
```
The full path: Claude Desktop does not read your shell's `PATH` (on Windows, `"C:\\Users\\you\\.cargo\\bin\\sioul.exe"`). `"args": ["--language", "fr", "mcp"]` for French.

## The tools
Things are named by their addresses, as links name them ([tasks.md](tasks.md)): `mid:<Message-ID>`, `sioul:task/<UID>`, `sioul:event/<UID>`, `sioul:note/<path>`, `sioul:contact/<UID>`, `sioul:project/<id>` (`sioul:case/<id>`, as written before, still opens it), `sioul:draft/<id>`. The read tools give them; `links` and `link` follow them.

| Tool | Reads |
|---|---|
| `porch` | What came, lane by lane, as `sioul porch` shows it; outside your admin windows, only when the Porch opens, unless `open`. A code that came is said, never its code. |
| `search_mail` | Messages whose subject or sender holds the words, newest first; their text too with `in_text`. The words are looked for in what the agent would be shown, masked: a code or an account number is not found by its digits. Blocked senders left out; hostile mail to a shielded address is found by none of its words, and listed as "someone at" its domain. |
| `read_message` | One message, by its file or its Message-ID: who wrote to whom, genuine or forged and why, its lane, its attachments' names, its text as plain text. |
| `list_tasks` | `now`: the next step, why, and the one after it. `today`: the day laid out, events and the plan's steps. `list`: every open task in the plan's order. |
| `agenda` | Events, day by day, from a day for so many days (today, two weeks). |
| `search_contacts` | Contacts by name, organisation, address or number. |
| `budgets` | Each budget's verdict at its pace and its figures; reserves; the bank accounts' last balance; what the money watch noticed. |
| `search_notes` | Notes by title, path or tag; their text too with `in_text`. |
| `read_note` | One note in full, its open checkboxes, what it is tied to both ways. |
| `list_projects` | The projects open to agents, with their open tasks; with `id`, an open project on one line of time. |
| `links` | What a thing is tied to, both ways, as `sioul links` shows it. |
| `find` | Anything whose title holds the words, with its address; a message by its subject as the agent sees it, masked. |
| `phone_messages` | The messages your phone shares with your computers ([android.md](android.md), "Messages on your computers"), newest first: when, which app, who wrote, the group, and the words where the phone sent them, framed as data. A code or an approval never left the phone: its line says that one came; what the words still hold of one is masked. A message the phone held comes once the phone lets it through. |
| `calls` | The calls your phone screened: when, who (their name in your address books, else their number; a hidden number said so), rang or declined and sent to voicemail. |
| `papers` | The papers wallet ([papers.md](papers.md)): each paper's kind, who it is for, issued, until when it holds, when renewing starts, recent enough to send or not. Never its file. |
| `contracts` | Contracts and subscriptions: with whom, when each renews, the notice it needs and the last day to send it, what it covers, when it ended. Never its reference. |
| `letters` | Paper letters as Sioul read their scans: sender, kind, received, amount, the date asked and why, an appointment, its project, its task. Never its text. |
| `time` | Time noted from a day to another (30 days by default), one project or all: by project, the time, its billable part, what is not billed yet; each stretch with its task and the invoice that billed it. |
| `invoices` | The invoices issued: number, date, client, amount, due date, paid or not. |
| `reminders` | What Sioul will remind you of in the coming days (14 by default): events, dates asked, waits, payments, papers, contracts, the money watch; each with when, and whether it was told. |
| `now` | What now is for and until when (work, admin, leisure, a meal, sleep, the pause, Free time), do-not-disturb (why, until when), and what reaches you now, row by row of the matrix (docs/attention.md). Cheap; an agent uses it to respect your hours. |

### Texts
Your texts (SMS and MMS, [texts.md](texts.md)), **off unless you turn them on**: `[mcp] texts = true`, the switch "Texts" in Settings ▸ AI agents. Texts carry other people's words, often more private than mail; off, the three tools are neither listed nor called.

| Tool | Reads | Changes |
|---|---|---|
| `texts` | The conversations, newest first (their people by your address books' names, else their numbers; the last words; an id each); with `conversation`, its messages, the newest `limit`: when, who wrote it ("You" for you), the words, the pictures and other parts said by kind (never opened), what became of a text sent from a computer, deleted on the phone said. | Nothing. |
| `search_texts` | Texts holding every word, in their words, a part's name, or their people's names and numbers. | Nothing. |
| `draft_text` | Nothing more than the conversation named. | A draft, saved for the Texts page in its conversation, on this computer only (`textdraft`, `drafts.jsonl` in the texts' private folder, sealed as the texts are). **Never sent**: a draft is no request to send, and nothing reads it to send. In its conversation, the page shows it under "Drafted by an AI agent, not sent", with **Use it** (its words go into the box; sending stays your Send) and **Delete the draft**; the list says "A draft by an AI agent waits here". One person only: never a group, never a short number, at most 1,600 characters. |

- **Codes masked as in mail**: each text's words are masked before any tool reads them (a code, a sign-in link, an IBAN, a card or social security number), so that a code is neither given nor found by its digits.
- **Others' words come as data**, between the per-answer frame marks.
- **Per project**: texts are in no project. A conversation is kept from agents when one of its people is a contact kept from agents (a closed project's client, or a contact tied to a closed project), by the numbers in your address books; every conversation while things outside projects are closed. Left out of the lists, counted, refused by its id, never drafted to.
- **Group conversations** are read, never drafted to: the phone refuses to send to a group from a computer.
- **The keyring, once**: the texts on a computer are sealed at rest with a key made from the sharing key ([texts.md](texts.md), "Sealed at rest"). To open them, `sioul mcp` reads the sharing key from this computer's keyring, only while `[mcp] texts = true` and only when a texts tool is called; the key opens the texts in memory and never reaches the agent. It is the one thing the MCP server ever reads from the keyring; the switch's sentence in Settings says so.

The new reading tools change nothing anywhere. Health (medicines, doses, needs) stays out, as passwords, keys and whole account numbers do; the meals and the night show only as what now is for.

| Tool | Changes, on this computer only |
|---|---|
| `add_task` | A new task file in a task list; the next sync sends it, as for a task made in the window. |
| `complete_task` | One task marked done in its file; a repeating one moves to its next turn. Open again from the window. |
| `add_event` | A new event file in a calendar; the next sync sends it. Nobody is invited. |
| `add_note` | A new Markdown note in the notes folder, never over another one: a taken name gets a number. |
| `draft_reply` | A reply saved in Drafts (`$XDG_DATA_HOME/sioul/drafts/`), thread and recipients from the message, a copy of the message kept with it. Never sent. Not to hostile mail nor to a blocked sender. |
| `draft_message` | A new message saved in Drafts, each recipient one address. Never sent. |
| `link` | A tie written in the task, the event or the note's front matter that can hold it, else in Sioul's own `links.toml`. Between things Sioul has, or to a web page (`https://…`). |

### The spam filter's tools
Your own spam filter ([spam-filter.md](spam-filter.md)), run, watched and judged from outside, as `sioul spam` does it (the same reports: [spam-filter.md](spam-filter.md), "Controls from outside"). Computers only; listed after the others.

| Tool | Reads | Changes |
|---|---|---|
| `spam_status` | Numbers only: the corpus per account and folder, the last download, outside material's counts and dates, your providers' verdicts as Sioul reads them, the table in use (when, where, from how much), the last training, the jobs running. | Nothing. |
| `spam_eval` | The table in use, tested on the corpus's newest fifth: the numbers at your two thresholds, the part it never learned from apart, counts by account and folder, the numbers at thresholds from 0.5 to 0.99, and the `errors` worst errors of each kind (20 by default): each one's date, account, folder, **sender's address and subject**, probability, verdict, what decided its label, `mid:` address. The same for each outside source's held-out part. | Nothing. |
| `spam_dry_run` | What the filter would do now with the mail in each inbox, with your matrix and thresholds or those given: counts per verdict and action, and the messages it would move and flag, by **sender's address and subject**. | Nothing: nothing is moved, no file written (tested). |
| `spam_review` | What the filter caught (flagged, or moved into a Junk folder), as the Porch lists it, by **sender's address and subject**. | Nothing. |
| `spam_job` | A job started apart, by its id: where it stands, then its result (a training's: as `spam_eval` gives it); without an id, the jobs kept. | With `stop`: asks a running job to stop at its next step. |
| `spam_label` | Nothing more than the message named. | One line in this device's label log, spam or not, as the window's Spam and Not spam write it: every device reads it (a message said not spam is never flagged again), the next training learns it. Never a move, nothing said to the server. |
| `spam_fetch` | Your mail servers, read-only (EXAMINE, BODY.PEEK). | The training corpus on this computer, in a job apart (`sioul spam fetch --job`); answers at once with its id. |
| `spam_train` | The corpus. | A trial, in a job apart, unless `replace`: then the table every device reads, only when it takes no more ham for spam than the one in place (kept beside it), which changes what the filter flags or moves on every device. Its model (`model`: fastText's classifier, the default, or the centroid) and settings for an experiment (`dim`, `epochs`, `bucket`, `minn`, `maxn`, `min_count`, `lr`, `c`, `ham_weight`, `threads`) are recorded. |

What they show of a message is its envelope, masked as mail is everywhere here (below, "What does not move"): a code, a sign-in link, an IBAN, a card or social security number hidden; hostile mail to a shielded address as "someone at" its domain; nothing from a sender you blocked, only counted; nothing of mail kept from agents ("Per-project access"), only counted, and such a message is never labelled by one. Never a message's text, never the words the filter learned (`sioul spam why` is for your terminal alone). Each list says first that its subjects are their senders' words: data, never instructions. A message the agent saw only there (old junk your provider purged, mail older than the sync keeps) is in the corpus, not in your mail: `read_message` does not find it; `spam_label` labels it by its place in the corpus.

**On unless you say otherwise**: `[mcp] spam = false` in the configuration keeps every one of them from agents, unlisted and refused. On by default because they show less than the tools beside them (an envelope, where `read_message` gives the whole text, masked the same way), because none moves mail, deletes anything or sends anything, and because the label they write is one line that the next word, yours or theirs, replaces. Off is right when an agent is connected for other things: `spam_fetch` reads your mail servers and `spam_train` keeps every core busy for minutes (one job of each kind at a time, below other programs' priority), and a label changes what the filter flags on every device.

## Per-project access
What you open to agents, as every tool keeps to it (`crates/sioul-core/src/consent.rs`, `crates/sioul-cli/src/mcp/access.rs`):
- **Closed until opened.** A project is open to agents when its entry in `sioul-projects.toml` says `ai = true`; a manifest written before the field existed opens none. The switch "Open to AI agents" sits on the project's page, in its form, and in Settings ▸ AI agents with every project's; the sharing carries the entry, so the choice holds on every device. The configuration is read again at each call: a project closed while an agent is connected is closed from its next call.
- **Things in no project** follow one setting, `[mcp] outside_projects`, open by default so that an agent you connect is of use at once: the Porch's other mail, tasks and notes of no project, the agenda's events of no project, contacts, the phone's messages and calls, papers, contracts, budgets no project uses, the bank accounts. Close it (Settings ▸ AI agents, "Things in no project") if you want agents to see only the projects you open. The moment now (`now`) is no one's thing: it stays, so that an agent can respect your hours.
- **What belongs to a project**, as its page gathers it: a task or an event by its project (REFID) or a tie, a step by its bigger task's too; mail by the project's routes (on the sender and the subject; on the text and the attachments once Sioul has read the message, which ties it then), by a tie, and the rest of their conversations; a note named in the project's files, inside a folder they name, or tied; a draft by its ties and the message it answers; a contact tied to it, or its client; a budget its invoices go to, a budget's line tied to it; time, invoices and paper letters by their project. A project taken out of the manifest leaves its things in no project, as its page does.
- **A thing in two projects reaches an agent only when both are open.** Never half of a closed project's things: a task of an open project that waits for a closed one's keeps its place in your plan, and the closed one is in no answer.
- **Lists leave out, and count.** Every list (the Porch, mail, tasks, the day, the agenda, contacts, budgets, notes, projects, links, a project's line of time, the phone's messages and calls, texts, papers, contracts, letters, time, invoices, reminders, the spam filter's lists) leaves out what is kept, and says how many things it left out, never which, so that the agent does not invent them. A search counts what it kept out of the whole scope (the days, the account), whatever its words: its count cannot tell whether a closed project's message holds a word. `find` gives no count. The next step (`list_tasks`, now) of a closed project is said to be kept, nothing of it given.
- **Asked for by its address, a kept thing is refused**, saying so: a message, a note, a project's page, what a thing is tied to.
- **Writing keeps to it too.** Nothing new goes into a closed project (`add_task` with its id, a note in its folder, a tie to it, a draft tied to it), nor outside projects while those are closed (`add_event`, which takes no project, among them); a kept thing is never tied to, answered, marked done, nor labelled spam or not. Each refusal says what to do: the person opens the project.
- **The AI shield is another thing.** It is a switch of one public address ([porch.md](porch.md), "A public address protected"): when you allow it, Anthropic reads the tone of the mail that reaches that address, unmasked, with your key, so that the Porch can set hostile mail aside. It reads no project and serves no agent; per-project access does not change it, and closing every project does not stop it.

Each answer is a few lines to read, then the same as data (MCP's `structuredContent`). Sioul's own sentences (what came, why a step comes now, a verdict) come in your language; the labels meant for the agent ("From:", "Key:") and the tools' descriptions are in English.

Resources, for the clients that attach them to a conversation, give the same text: `sioul:porch`, `sioul:tasks/now`, `sioul:tasks/today`, `sioul:agenda`, `sioul:projects`, and any note (`sioul:note/{path}`) or message (`mid:{message_id}`).

## What does not move
- **Nothing is sent, deleted or paid.** No tool sends mail, deletes or moves a message, or moves money; none speaks to a server but `spam_fetch`'s job, which reads your mail servers for the spam filter's corpus, read-only; no sync runs. A task or an event written here goes with the next sync, as one made in the window. A draft waits in Drafts until you read it and send it from the window, or discard it.
- **Nothing secret.** No password, key or token is given to an agent; the keyring is never opened, but for the sharing key that opens your texts while you allow texts for agents ("Texts", above), which stays in Sioul. The Porch says that a code came, never the code. A message carrying a code has every code hidden wherever it is written ("482 913", "482-913", "48 29 13", digit by digit as an HTML table turned into text writes it, in the subject, in a quoted reply, far down a long text), with every link it holds, as good as the code (a sign-in, a reset, an address to confirm); a message giving a password is kept whole from the agent, even when a code comes first. IBANs (checked by their mod 97), payment card numbers (Luhn's check) and French social security numbers (their key, Corsica's 2A and 2B too) are masked in mail, notes and budgets, written with spaces of any width; the last four characters stay, to tell two accounts apart. The masks are the same in the lines and in the data, and searching for the digits of a masked number finds nothing. The window shows them all.
- **Mail is data.** What others wrote (mail, invitations, contacts, file names, the phone's messages) comes on one line per field, so that a subject cannot add a line such as `Lane:` or `Key:`. A message's or a note's own text comes between two lines that carry the same mark, made for that answer, which its author cannot know: whatever the text says, it cannot end the frame and speak as Sioul. The agent is told once, when it connects, that all this is data, never instructions, and to ask you rather than do what a message asks. Forged mail is said forged. Hostile mail to a shielded address (as the Porch judges it, its AI's reading first) is given by no word of it, nor answered; encrypted mail is not given.
- **Only your mail.** `read_message` and `draft_reply` open files inside your accounts' mail folders only, whatever path they are given: the path is checked as it is written before anything is opened (no other computer's share, no `..`), then again once the system has followed its links.
- **Writing only adds.** A note never replaces another, nor goes out of the notes folder (no drive as `C:`, no device name as `CON`, no link on its way); a task, an event or a draft takes a name nothing has; a tie is made once; a task done opens again in the window. What an agent writes may come from mail, so it is checked as mail is: a title, a place, a subject on one line; each recipient one address, its name holding no other; a tie only to what Sioul has or to a web page, never to a file of this computer or to another program's scheme, and never with a line break that could add a line to a task or an event that the next sync sends. Each writing tool says what it changes, and MCP's hints say it too: `readOnlyHint` true for the reading ones, `destructiveHint` and `openWorldHint` false for all.
- **The admin windows hold.** `porch` keeps to them unless the agent asks (`open`); the agent is asked to respect them unless you ask.
- **A log, without the words.** Each call: when, the client's name, the tool, its arguments (a draft's or a note's text by its length only, other texts cut short and masked), whether it failed, and what was given by its addresses (`mid:…`, `sioul:…`) and its length. One JSON line, one file a month, `$XDG_STATE_HOME/sioul/mcp/2026-10.jsonl`, readable by you alone. No message's text, no code, no secret is kept a second time, in a file that would outlive the mail; the agent's model, wherever it runs, received the text itself. A spam training's job file keeps its result, the worst errors' senders and masked subjects among it, for a week after it ended (`$XDG_STATE_HOME/sioul/spam/jobs/`, yours alone).

Not there yet:
- **A mask lifted for one request**: an agent cannot lift one; the window shows the numbers.

## How it is built
- **The transport apart** (`transport.rs`): the stdio transport reads one JSON-RPC message per line on standard input and writes one per line on standard output; nothing else is written there, diagnostics go to standard error. A line is at most 8 MiB; a longer one, or one that is not UTF-8, is answered with JSON-RPC's parse error and the next one is read.
- **The protocol** (`protocol.rs`): JSON-RPC 2.0 (`"jsonrpc": "2.0"`, an id a string or a number, never null); MCP 2025-06-18, and 2025-03-26 and 2024-11-05 for older clients: `initialize`, `ping`, `tools/list`, `tools/call`, `resources/list`, `resources/templates/list`, `resources/read`. A batch is refused, as 2025-06-18 removed them. A tool that fails answers `isError: true` with a sentence the agent can act on; an unknown method or tool is a JSON-RPC error. Whatever breaks, in a tool or in the protocol, says so, and the server stays up.
- **The registry as plain data** (`tools.rs`): name, title, description, JSON schema, function. Arguments are checked against the schema before the function runs: an unknown one is named, with those it takes.
- **The core's functions**, as the command line uses them (`read.rs`, `write.rs`; `phone.rs` for the phone's messages and calls, `records.rs` for papers, contracts, letters, time, invoices, reminders and the moment now): the Porch's gathering and views, the plan, the links, the notes, the drafts.
- **Per-project access** (`access.rs`, on the core's `consent`): what everything belongs to, read once per call with the files the tool reads; each tool leaves out, counts and refuses through it.
- **The configuration read again at each call**: an agent connected for a day sees today's. One that cannot be read is said to the agent, not taken for none.
- No async runtime, no MCP library: `serde_json` and the core.

## Next: ChatGPT and other clients
- **Streamable HTTP**, MCP's other transport: one endpoint, each message a POST, each answer JSON or an event stream, the session in `Mcp-Session-Id`. It is another `Transport` over the same server and the same table: `sioul mcp --http 127.0.0.1:<port>`. Bound to this computer, the `Origin` header checked (the spec asks it, against DNS rebinding), a token from the keyring asked of the client.
- **ChatGPT** reaches MCP servers from the internet, never on this computer: it needs a public address (a tunnel you choose) and OAuth 2.1, as MCP's authorization says. Its deep research asked for two tools named `search` and `fetch`: thin names over `find` and the reading tools.
- **Local models** (llama.cpp, Ollama, LM Studio) take it as it is, through any MCP client that starts a program on stdio.
- **Other agent protocols** (OpenAI's function calling, an agent's own tool format) can serve the same table: names, descriptions and schemas are plain data.

## Tested
`cargo test -p sioul-cli mcp`, in a home made in the system's temporary folder (its XDG folders and `HOME` pointed there; invented mail on reserved domains, RFC 2606):
- the handshake: revisions agreed, ping, notifications unanswered, JSON-RPC's errors; ids of every JSON type, the version, batches, JSON nested past reason; a line too long or not UTF-8 answered, the server going on;
- the tools listed, with their schemas and hints; nothing that sends, deletes or pays;
- reading: a note found and read; a message by its Message-ID and by its file, genuine, in its project, its IBAN masked; a code never given; paths outside the mail refused (`..`, another computer's share, a relative path, a link inside the Maildir pointing out); the Porch; tasks, projects, links;
- mail as data: a subject that would add a line kept on one, a text that plays the frame and Sioul kept inside a frame of its own, a new mark each answer; hostile mail (by its AI reading) neither read, nor listed by name, nor found by its words, nor answered;
- writing: a task made from a message, in its project, tied to it, then done; a reply drafted, kept in Drafts, never sent; two notes of one title, neither over the other; an event, and a note tied to it inside it; wrong arguments refused; ties with a line break, to a file or to another scheme refused; titles, places, tags and subjects on one line in their files; a time in UTC moved to your zone; note folders out of the notes, drives, device names and links refused, a very long title; recipients that hold a second address, a header or a borrowed name refused;
- the log: no message's or note's words, no body, no number; addresses given; the file yours alone;
- per-project access (`a_closed_project_is_never_shown`): a project written before the field (closed) holds a message, a note in its folder, a task and a step of it, an event, time, an invoice, a paper letter and a budget, each with one word; no answer of any tool holds that word (the Porch, mail searched by it and by others, tasks in each view, the agenda, contacts, budgets, notes searched in their text, projects, links, `find`, time, invoices, letters, reminders, papers, contracts, the moment now, a resource); a search's count is the same whatever its words; read by its address, each is refused; nothing is written into it, tied to it, answered or labelled; `[mcp] outside_projects = false` (`things_outside_projects_can_be_closed`): only the opened project's mail, notes and tasks come, papers, the phone's messages and calls none, and nothing is written outside a project; an older manifest without the field opens nothing (`projects`'s and `consent`'s own tests, in the core);
- texts (`texts_only_when_allowed_and_never_sent`): off by default, unlisted and refused; allowed, the conversations listed with their words framed, a code masked and never found by its digits, a closed project's client's conversation kept (in the count, refused by its id, found by no word); a draft saved, sealed in its file, no request to send written; a group, a short number, a kept conversation refused; the drafts' file in the core (`textdraft`: sealed, once each, taken out when used or deleted); the tests' texts sealed with a key of their own, never a keyring's;
- the phone (`the_phone_messages_and_calls`): a code its phone did not catch masked, one it caught said without it, never found by its digits; a message held by the phone not given before its time; the words framed; a call declined; the moment now;
- the spam filter's tools, on a hand-made table and a corpus of invented mail: each answering (status, eval and its errors, a dry run, the review queue, the jobs) and refusing what is wrong before any job starts; a dry run that writes nothing, file by file; a label that only labels, never through a move; `[mcp] spam = false`; a job's file from start to end; the same reports from `sioul spam` ([spam-filter.md](spam-filter.md), "Tested"); and in `tests/spam_jobs.rs`, the real program served on standard input: `spam_train` started apart and followed with `spam_job` to its end, a trial that wrote no table, `spam_fetch` ending on what it could not do;
- resources; the stdio framing, line by line; the masks, every code of a message in every form (boxes, pairs, dashes, non-breaking spaces, quoted, after a warning, far down, a password after a code, links in capitals or without a scheme), and what they leave alone (a phone, a SIRET, an order number, an IBAN or a NIR that does not check out, an ordinary message's links).

By hand: `sioul mcp` given `initialize`, `tools/list` and `tools/call` on standard input, on a fake home. Not tested yet inside Claude Code or Claude Desktop themselves.
