# AI providers and agents

Sioul works with AI in two ways today: the agents you connect yourself, through `sioul mcp` and the command line (below), and one reading of its own, the shield's, which asks Anthropic's API, with your key, for the tone of the mail that reaches a shielded address ([porch.md](porch.md)). The companion this page describes next, with remote models (Claude, from Anthropic, and ChatGPT, from OpenAI) through your own keys kept in the system keyring, is not built yet; local models may come later, heavier and less accurate today. Whatever the AI does, **it never sends and never deletes.**

## What the companion is to do (not built)
- **"Open with the companion"**: explains a letter in plain language (what it is, what it asks, by when, what happens if nothing is done), line by line if needed.
- **Proposes**: a triage, a project, a deadline found in the text, a task and its first small steps.
- **Drafts replies and letters as Markdown files in the Outbox**, with recipients and attachments named, waiting for your review.
- **Keeps you company** during an admin session (body doubling), in a side panel.

## For agents
- **An MCP server and the `sioul` command line** expose the core to agents (Claude Code, Codex and others). They can:
  - list projects;
  - read a message;
  - search;
  - propose a triage;
  - create tasks and links;
  - write drafts.
- **The drafts an agent writes are ordinary `.md` files** ([design.md](design.md), "Notes and drafts"). Strategy notes written with an agent become emails without copying and pasting.

## Limits that do not move
- **No sending, no deleting**, by any agent or model. Sending is your keypress.
- **Per-project consent**: you open a project to AI; the others stay closed. Built for the agents connected with `sioul mcp` (below); the companion will keep to it too.
- **Least data**: a request carries the message and the project's summary, not the notes folder.
- **Masking**: IBANs, card numbers, one-time codes and identifiers are masked unless you allow them for that request.
- **A log** shows each request: for `sioul mcp`, what was asked and the addresses of what was handed over, never their words (`$XDG_STATE_HOME/sioul/mcp/`, yours alone); what the agent's provider then does with it is between you and that provider.
- **Everything an AI proposed is marked as such**, until you accept it: planned with the companion. What an agent adds through `sioul mcp` (tasks, events, notes, ties, drafts) carries no such mark yet; its drafts wait in Drafts, and nothing is sent.

## Configuration, planned
Not read yet: the companion's settings would look like this. Today the only key Sioul uses is the shield's, for Anthropic's API, typed in the settings and kept in the keyring.
```toml
[ai]
default = "claude"        # or "chatgpt"

[ai.claude]
model = "claude-sonnet-5-5"

[ai.chatgpt]
model = "…"               # the model you choose
```
The keys are not written here: Sioul asks for them once and keeps them in the keyring.

## The MCP server
`sioul mcp` serves Sioul to agents over the Model Context Protocol, on standard input and output: Claude Code (`claude mcp add sioul -- sioul mcp`), Claude Desktop, any MCP client. Its tools read what came, mail, tasks, the agenda, contacts, budgets, notes and projects, the phone's messages and calls, papers, contracts and paper letters, time and invoices, reminders and the moment now, and add tasks, events, notes, ties and drafts, on this computer only. Your texts come only when you allow them (Settings ▸ AI agents, "Texts", off at first): an agent then reads and searches them, codes masked, and writes drafts that wait in the Texts page until you send them yourself ([mcp.md](mcp.md), "Texts").

### Consent per project
**Every project is closed to agents until you open it.** The switch "Open to AI agents", on a project's page, in its form, or in Settings ▸ AI agents beside every other project's, writes `ai = true` on the project's entry in `sioul-projects.toml` (or `sioul-cases.toml`, the file's first name, until it is renamed); the sharing carries it to your other devices. A connected agent then reads that project's mail, tasks, notes, events, time, invoices, letters and budget, and adds tasks, notes, ties and drafts to it; what it reads goes to its model's provider. Of a closed project it sees nothing: every list leaves its things out and says how many it left out, never which; asked for by name, each is refused; nothing is written into it. A thing in two projects reaches an agent only when both are open. What belongs to a project, and how each tool keeps to it: [mcp.md](mcp.md), "Per-project access".

**Things in no project** (the Porch's other mail, tasks and notes of no project, the agenda, contacts, the phone's messages and calls, papers, contracts, the budgets no project uses) follow one setting, **Things in no project** in Settings ▸ AI agents (`[mcp] outside_projects`). It is open by default, so that an agent you connect is of use at once. **If you want agents to see only the projects you open, close it**: an agent then sees those projects and the moment now, nothing else.

**The AI shield is another thing.** It belongs to one public address ([porch.md](porch.md), "A public address protected"), with its own switch: when you allow it, Anthropic's model reads the tone of each message reaching that address, unmasked, with your key, so that the Porch can set hostile mail aside. It serves no agent and reads no project: opening or closing projects does not change it, and closing every project does not stop it.

What holds already: nothing is sent, deleted or paid; drafts wait for you in Drafts; codes, passwords, sign-in and confirmation links, IBANs, card and social security numbers are masked, and no mask can be lifted by an agent; mail and notes come framed as data; each call is logged by what was asked and the addresses given, never by their words. The tools, the rules, and the way to ChatGPT: [mcp.md](mcp.md).
