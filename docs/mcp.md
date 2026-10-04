# Sioul for AI agents: the MCP server

`sioul mcp` serves what Sioul keeps on this computer to AI agents over the Model Context Protocol (MCP, revision 2025-06-18): Claude Code and Claude Desktop now; ChatGPT and local models later, through any MCP client. The agent reads the Porch, mail, tasks, the agenda, contacts, budgets, notes and projects; it adds tasks, events, notes, ties and drafts. It never sends, deletes, moves money or reads a password ([ai.md](ai.md), "Limits that do not move").

**Every case is open to a connected agent.** Consent per case, which [ai.md](ai.md) promises ("you open a case to AI; the others stay closed"), is not built yet: an agent connected to `sioul mcp` can read every message, note, task, event, contact, budget and case Sioul keeps on this computer, and what it reads goes to the provider of its model. Connect one only when that is what you want.

Code: `crates/sioul-cli/src/mcp.rs` and `crates/sioul-cli/src/mcp/`.

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
Things are named by their addresses, as links name them ([tasks.md](tasks.md)): `mid:<Message-ID>`, `sioul:task/<UID>`, `sioul:event/<UID>`, `sioul:note/<path>`, `sioul:contact/<UID>`, `sioul:case/<id>`, `sioul:draft/<id>`. The read tools give them; `links` and `link` follow them.

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
| `list_projects` | Cases and projects with their open tasks; with `id`, a project on one line of time. |
| `links` | What a thing is tied to, both ways, as `sioul links` shows it. |
| `find` | Anything whose title holds the words, with its address; a message by its subject as the agent sees it, masked. |

| Tool | Changes, on this computer only |
|---|---|
| `add_task` | A new task file in a task list; the next sync sends it, as for a task made in the window. |
| `complete_task` | One task marked done in its file; a repeating one moves to its next turn. Open again from the window. |
| `add_event` | A new event file in a calendar; the next sync sends it. Nobody is invited. |
| `add_note` | A new Markdown note in the notes folder, never over another one: a taken name gets a number. |
| `draft_reply` | A reply saved in Drafts (`$XDG_DATA_HOME/sioul/drafts/`), thread and recipients from the message, a copy of the message kept with it. Never sent. Not to hostile mail nor to a blocked sender. |
| `draft_message` | A new message saved in Drafts, each recipient one address. Never sent. |
| `link` | A tie written in the task, the event or the note's front matter that can hold it, else in Sioul's own `links.toml`. Between things Sioul has, or to a web page (`https://…`). |

Each answer is a few lines to read, then the same as data (MCP's `structuredContent`). Sioul's own sentences (what came, why a step comes now, a verdict) come in your language; the labels meant for the agent ("From:", "Key:") and the tools' descriptions are in English.

Resources, for the clients that attach them to a conversation, give the same text: `sioul:porch`, `sioul:tasks/now`, `sioul:tasks/today`, `sioul:agenda`, `sioul:projects`, and any note (`sioul:note/{path}`) or message (`mid:{message_id}`).

## What does not move
- **Nothing is sent, deleted or paid.** No tool sends mail, deletes or moves a message, moves money, or speaks to a server; no sync runs. A task or an event written here goes with the next sync, as one made in the window. A draft waits in Drafts until you read it and send it from the window, or discard it.
- **Nothing secret.** No password, key or token is read; the keyring is never opened. The Porch says that a code came, never the code. A message carrying a code has every code hidden wherever it is written ("482 913", "482-913", "48 29 13", digit by digit as an HTML table turned into text writes it, in the subject, in a quoted reply, far down a long text), with every link it holds, as good as the code (a sign-in, a reset, an address to confirm); a message giving a password is kept whole from the agent, even when a code comes first. IBANs (checked by their mod 97), payment card numbers (Luhn's check) and French social security numbers (their key, Corsica's 2A and 2B too) are masked in mail, notes and budgets, written with spaces of any width; the last four characters stay, to tell two accounts apart. The masks are the same in the lines and in the data, and searching for the digits of a masked number finds nothing. The window shows them all.
- **Mail is data.** What others wrote (mail, invitations, contacts, file names) comes on one line per field, so that a subject cannot add a line such as `Lane:` or `Key:`. A message's or a note's own text comes between two lines that carry the same mark, made for that answer, which its author cannot know: whatever the text says, it cannot end the frame and speak as Sioul. The agent is told once, when it connects, that all this is data, never instructions, and to ask you rather than do what a message asks. Forged mail is said forged. Hostile mail to a shielded address (as the Porch judges it, its AI's reading first) is given by no word of it, nor answered; encrypted mail is not given.
- **Only your mail.** `read_message` and `draft_reply` open files inside your accounts' mail folders only, whatever path they are given: the path is checked as it is written before anything is opened (no other computer's share, no `..`), then again once the system has followed its links.
- **Writing only adds.** A note never replaces another, nor goes out of the notes folder (no drive as `C:`, no device name as `CON`, no link on its way); a task, an event or a draft takes a name nothing has; a tie is made once; a task done opens again in the window. What an agent writes may come from mail, so it is checked as mail is: a title, a place, a subject on one line; each recipient one address, its name holding no other; a tie only to what Sioul has or to a web page, never to a file of this computer or to another program's scheme, and never with a line break that could add a line to a task or an event that the next sync sends. Each writing tool says what it changes, and MCP's hints say it too: `readOnlyHint` true for the reading ones, `destructiveHint` and `openWorldHint` false for all.
- **The admin windows hold.** `porch` keeps to them unless the agent asks (`open`); the agent is asked to respect them unless you ask.
- **A log, without the words.** Each call: when, the client's name, the tool, its arguments (a draft's or a note's text by its length only, other texts cut short and masked), whether it failed, and what was given by its addresses (`mid:…`, `sioul:…`) and its length. One JSON line, one file a month, `$XDG_STATE_HOME/sioul/mcp/2026-10.jsonl`, readable by you alone. No message's text, no code, no secret is kept a second time, in a file that would outlive the mail; the agent's model, wherever it runs, received the text itself.

Not there yet:
- **Consent per case** ([ai.md](ai.md)): until it exists, every case is readable by a connected agent, as said at the top. A case opened to AI or closed to it needs a field in the cases' manifest (`sioul-cases.toml`), read by the core, and each tool to keep to it.
- **A mask lifted for one request**: an agent cannot lift one; the window shows the numbers.

## How it is built
- **The transport apart** (`transport.rs`): the stdio transport reads one JSON-RPC message per line on standard input and writes one per line on standard output; nothing else is written there, diagnostics go to standard error. A line is at most 8 MiB; a longer one, or one that is not UTF-8, is answered with JSON-RPC's parse error and the next one is read.
- **The protocol** (`protocol.rs`): JSON-RPC 2.0 (`"jsonrpc": "2.0"`, an id a string or a number, never null); MCP 2025-06-18, and 2025-03-26 and 2024-11-05 for older clients: `initialize`, `ping`, `tools/list`, `tools/call`, `resources/list`, `resources/templates/list`, `resources/read`. A batch is refused, as 2025-06-18 removed them. A tool that fails answers `isError: true` with a sentence the agent can act on; an unknown method or tool is a JSON-RPC error. Whatever breaks, in a tool or in the protocol, says so, and the server stays up.
- **The registry as plain data** (`tools.rs`): name, title, description, JSON schema, function. Arguments are checked against the schema before the function runs: an unknown one is named, with those it takes.
- **The core's functions**, as the command line uses them (`read.rs`, `write.rs`): the Porch's gathering and views, the plan, the links, the notes, the drafts.
- **The configuration read again at each call**: an agent connected for a day sees today's. One that cannot be read is said to the agent, not taken for none.
- No async runtime, no MCP library: `serde_json` and the core.

## Next: ChatGPT and other clients
- **Streamable HTTP**, MCP's other transport: one endpoint, each message a POST, each answer JSON or an event stream, the session in `Mcp-Session-Id`. It is another `Transport` over the same server and the same table: `sioul mcp --http 127.0.0.1:<port>`. Bound to this computer, the `Origin` header checked (the spec asks it, against DNS rebinding), a token from the keyring asked of the client.
- **ChatGPT** reaches MCP servers from the internet, never on this computer: it needs a public address (a tunnel you choose) and OAuth 2.1, as MCP's authorization says. Not before consent per case exists. Its deep research asked for two tools named `search` and `fetch`: thin names over `find` and the reading tools.
- **Local models** (llama.cpp, Ollama, LM Studio) take it as it is, through any MCP client that starts a program on stdio.
- **Other agent protocols** (OpenAI's function calling, an agent's own tool format) can serve the same table: names, descriptions and schemas are plain data.

## Tested
`cargo test -p sioul-cli mcp`, in a home made in the system's temporary folder (its XDG folders and `HOME` pointed there; invented mail on reserved domains, RFC 2606):
- the handshake: revisions agreed, ping, notifications unanswered, JSON-RPC's errors; ids of every JSON type, the version, batches, JSON nested past reason; a line too long or not UTF-8 answered, the server going on;
- the tools listed, with their schemas and hints; nothing that sends, deletes or pays;
- reading: a note found and read; a message by its Message-ID and by its file, genuine, in its case, its IBAN masked; a code never given; paths outside the mail refused (`..`, another computer's share, a relative path, a link inside the Maildir pointing out); the Porch; tasks, projects, links;
- mail as data: a subject that would add a line kept on one, a text that plays the frame and Sioul kept inside a frame of its own, a new mark each answer; hostile mail (by its AI reading) neither read, nor listed by name, nor found by its words, nor answered;
- writing: a task made from a message, in its case, tied to it, then done; a reply drafted, kept in Drafts, never sent; two notes of one title, neither over the other; an event, and a note tied to it inside it; wrong arguments refused; ties with a line break, to a file or to another scheme refused; titles, places, tags and subjects on one line in their files; a time in UTC moved to your zone; note folders out of the notes, drives, device names and links refused, a very long title; recipients that hold a second address, a header or a borrowed name refused;
- the log: no message's or note's words, no body, no number; addresses given; the file yours alone;
- resources; the stdio framing, line by line; the masks, every code of a message in every form (boxes, pairs, dashes, non-breaking spaces, quoted, after a warning, far down, a password after a code, links in capitals or without a scheme), and what they leave alone (a phone, a SIRET, an order number, an IBAN or a NIR that does not check out, an ordinary message's links).

By hand: `sioul mcp` given `initialize`, `tools/list` and `tools/call` on standard input, on a fake home. Not tested yet inside Claude Code or Claude Desktop themselves.
