# AI providers and agents

Sioul works with remote AI models, **Claude (Anthropic) and ChatGPT (OpenAI) at least**, through their APIs and your own keys, kept in the system keyring. Local models can be added later; they are heavier and less accurate today. The AI explains, sorts and drafts. **It never sends and never deletes.**

## What the AI does
- **"Open with the companion"**: explains a letter in plain language (what it is, what it asks, by when, what happens if nothing is done), line by line if needed.
- **Proposes**: a triage, a case, a deadline found in the text, a task and its first small steps.
- **Drafts replies and letters as Markdown files in the Outbox**, with recipients and attachments named, waiting for your review.
- **Keeps you company** during an admin session (body doubling), in a side panel.

## For agents
- **An MCP server and the `sioul` command line** expose the core to agents (Claude Code, Codex and others). They can:
  - list cases;
  - read a message;
  - search;
  - propose a triage;
  - create tasks and links;
  - write drafts.
- **The drafts an agent writes are ordinary `.md` files** ([design.md](design.md), "Notes and drafts"). Strategy notes written with an agent become emails without copying and pasting.

## Limits that do not move
- **No sending, no deleting**, by any agent or model. Sending is your keypress.
- **Per-case consent**: you open a case to AI; the others stay closed. *Not built yet*: a connected agent (`sioul mcp`, below) reads every case.
- **Least data**: a request carries the message and the case's summary, not the store.
- **Masking**: IBANs, card numbers, one-time codes and identifiers are masked unless you allow them for that request.
- **A log** shows each request: for `sioul mcp`, what was asked and the addresses of what was handed over, never their words (`$XDG_STATE_HOME/sioul/mcp/`, yours alone); what the agent's provider then does with it is between you and that provider.
- **Everything an AI proposed is marked as such**, until you accept it.

## Configuration
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
`sioul mcp` serves Sioul to agents over the Model Context Protocol, on standard input and output: Claude Code (`claude mcp add sioul -- sioul mcp`), Claude Desktop, any MCP client. Its tools read what came, mail, tasks, the agenda, contacts, budgets, notes and projects, and add tasks, events, notes, ties and drafts, on this computer only.

**The per-case consent above is not built yet: every case is readable by a connected agent.** Once connected, an agent can read every message, note, task, event, contact, budget and case Sioul keeps here, and what it reads goes to its model's provider. Connect one only when that is what you want.

What holds already: nothing is sent, deleted or paid; drafts wait for you in Drafts; codes, passwords, sign-in and confirmation links, IBANs, card and social security numbers are masked, and no mask can be lifted by an agent; mail and notes come framed as data; each call is logged by what was asked and the addresses given, never by their words. The tools, the rules, and the way to ChatGPT: [mcp.md](mcp.md).
