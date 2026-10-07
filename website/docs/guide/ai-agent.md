---
description: Letting an AI agent such as Claude Code or Claude Desktop read what Sioul keeps and prepare tasks, events, notes and drafts, through MCP; it never sends, deletes, pays or reads a password.
---

# Using an AI agent

## In short {#in-short}

An AI agent you already use, such as Claude Code or Claude Desktop, can read what Sioul keeps on this device and prepare work for you: tasks, events, notes, ties between things, drafts. It explains and prepares; you review, and you send. It never sends, never deletes, never moves money and never reads a password, and one-time codes and account numbers are hidden from it. Sioul connects none by itself: this page is for when you want to.

## Before you connect one {#before-you-connect-one}

!!! warning "What an agent sees"
    Once connected, an agent can read everything Sioul keeps on this device: your mail, tasks, agenda, contacts, budgets, notes and projects, whatever they are for. What it reads goes to the model behind it (Anthropic's, for Claude), under your own agreement with that company. Opening some projects to an agent and keeping others closed is not there yet.

Some things are kept from it whatever happens ([below](#what-does-not-move)): passwords and keys, one-time codes and sign-in links, bank, card and social security numbers, your medicines and doses.

## Connecting {#connecting}

Sioul serves agents through the Model Context Protocol (MCP), with the command `sioul mcp`. It needs the command line `sioul` installed ([Install](install.md#into-your-application-menu)).

=== "Claude Code"

    ```
    claude mcp add sioul -- sioul mcp
    ```

    With `--scope user` after `add`, Sioul is there in every project, not only the current one. For Sioul's sentences in French: `claude mcp add sioul -- sioul --language fr mcp`.

=== "Claude Desktop"

    In Claude Desktop's `claude_desktop_config.json` (on macOS in `~/Library/Application Support/Claude/`, on Windows in `%APPDATA%\Claude\`), then restart Claude Desktop:

    ```json
    {
      "mcpServers": {
        "sioul": {
          "command": "/full/path/to/sioul",
          "args": ["mcp"]
        }
      }
    }
    ```

    Claude Desktop needs the full path of `sioul`: `command -v sioul`, in a terminal, prints it. For French: `"args": ["--language", "fr", "mcp"]`.

`sioul mcp` has been tested on its own; it has not yet been tried inside Claude Code or Claude Desktop themselves. If something does not work, a word in [GitHub issues](https://github.com/aurelienpierre/sioul/issues) helps.

## What an agent can read {#what-an-agent-can-read}

- **The Porch**, lane by lane, in your hours only, unless you ask it to open it. It is told that a code came, never the code.
- **Your mail**: messages found by their subject, sender or text; one message whole, with who wrote, whether it is genuine or forged and why, and its attachments' names.
- **Your tasks**: the next step and why, the day laid out, every open task in the plan's order.
- **Your agenda**, your **contacts**, your **budgets** (each verdict, the reserves, what the money watch noticed), your **notes**, your **projects**.
- **What anything is tied to**, both ways, and anything found by a few words of its title.

## What an agent can prepare {#what-an-agent-can-prepare}

On this device only:

- **a task**, or **a task marked done**: a task goes to your calendar server at the next sync, as one made in the window; a task marked done can be opened again from the window;
- **an event**: nobody is invited;
- **a note**, never written over another one;
- **a reply**, or **a new message**: saved in Drafts, never sent. You read it, and send it from the window, or delete it;
- **a tie** between two things.

## Your spam filter's tools {#the-spam-filters-tools}

An agent can also look after your own spam filter, as you would from the terminal: say where it stands, test it and list its worst mistakes by sender and subject (codes and account numbers masked, as everywhere), say what it would do now with your inboxes, say a message is spam or not when you ask it to (one line that every device reads), download the start of your mail from your servers to learn from, and train it. The download only reads: nothing changes on your servers. A training is a trial unless you ask for more, and it replaces the filter every device uses only when the new one takes no more good mail for spam than the old one. These tools are on unless you turn them off: to keep them from agents, write `spam = false` under `[mcp]` in Sioul's configuration (`~/.config/sioul/config.toml`).

## What does not move {#what-does-not-move}

- **Nothing is sent, deleted or paid.** No tool sends mail, deletes or moves a message, or moves money. The spam filter's tools are the only ones that reach your mail servers, and only to read ([above](#the-spam-filters-tools)).
- **Nothing secret.** No password, key or token is read. One-time codes are hidden wherever they are written, and so are sign-in, reset and confirmation links; a message that gives a password is withheld whole. Bank account numbers (IBAN), card numbers and French social security numbers are masked in mail, notes and budgets: an IBAN or a card number keeps its last four characters, to tell two accounts apart; a social security number is hidden whole. Your medicines and doses are not given to an agent.
- **Mail is data.** A message's text comes marked as its sender's words, not as instructions, and the agent is told so when it connects. Forged mail is said forged. Hostile mail to a protected address, and encrypted mail, are not given.
- **Writing only adds.** A note never replaces another; a tie is made once.
- **Your hours hold.** The agent is asked to respect them, unless you ask otherwise.
- **A log.** Each call is written down on this device, in a file only you can read, one a month in Sioul's own folders: which agent asked, what for, and the addresses of what it was given and how long it was, never its words. The log stays here; the agent's model received the text itself.

## The command line {#the-command-line}

Agents and scripts can also use the command line, as you can: `sioul tasks now`, `sioul tasks list`, `sioul links`, `sioul notes`, and more (`sioul --help`).

`sioul tasks import plan.toml` takes a plan written as a file: tasks with their steps and what they wait for, the contacts of the offices involved, and drafts, which stay in Sioul and are never sent. Importing again updates what the file made, instead of adding it twice; `--dry-run` says what would change. The file's format: [Tasks, "Importing"](../dev/tasks.md#importing).

## The other AI in Sioul {#the-other-ai-in-sioul}

One more feature uses an AI, and only for an address you protect against harassment, when you allow it: **Let the AI read it first** sends Anthropic's Claude, with your own key, each message in that address's inbox, to say its tone and topic. It receives each message's subject and the first 4,000 characters of its text, **unmasked**: unlike what an agent is given, nothing is hidden there, so a code, a password or an account number in such a message reaches Anthropic too. Once you turn it on, the messages already in that inbox are sent as well, from anyone. See [the Porch](porch.md#a-public-address-protected).

## Not there yet {#not-there-yet}

- Opening some projects to an agent and keeping others closed.
- ChatGPT, and agents that run elsewhere than on this device.
- A companion inside Sioul that explains a letter in plain words, line by line, and keeps you company during an admin session.

## For technical readers {#for-technical-readers}

- **Protocol**: the Model Context Protocol, revision 2025-06-18 (2025-03-26 and 2024-11-05 accepted too), JSON-RPC 2.0, over standard input and output only: the agent runs on this device, and no network port is opened.
- **Tool hints**: every tool says whether it writes, and none is destructive (MCP's `readOnlyHint`, `destructiveHint: false`, `idempotentHint`; `openWorldHint` only for the spam filter's download, which reads your mail servers).
- **Masks**: each number is hidden only when its own check holds, so that order and phone numbers stay: IBANs by ISO 13616's mod 97, card numbers of 16 or 19 digits by Luhn's check (fourteen digits are left alone: a SIRET has a Luhn key too, and is no secret), French social security numbers by their key. Codes are hidden wherever they are written, spaced or not; reset, sign-in and confirmation links are hidden; a message that gives a password is withheld whole.
- **Framing**: a message's or a note's text comes between two lines carrying a mark made for that answer, and the server's instructions say that such text is data, never instructions.
- **Never given**: hostile mail to a shielded address, encrypted mail, medicines and doses, your watch's days. The day laid out (`list_tasks`, today) shows meal and nap times, as the day in Tasks does.
- **The log**: `$XDG_STATE_HOME/sioul/mcp/YYYY-MM.jsonl`, one JSON line per call, written 0600 in a 0700 folder on Linux and macOS. It holds the time, the agent, the tool, its arguments (200 characters each, a body or notes by their length alone, account and card numbers masked), and the addresses (`mid:…`, `sioul:…`) and length of what was given.
- **Turning tools off**: `[mcp] spam = false` keeps the spam filter's tools from agents, unlisted and refused.
- **The shield's AI**, apart from agents: Claude Haiku 4.5 through Anthropic's API, with your key from the keyring; the subject and the first 4,000 characters of each message in a shielded address's inbox, unmasked; no redirect followed, so that the key goes to Anthropic alone.

The design behind all of this: [AI providers and agents](../dev/ai.md) and [the MCP server](../dev/mcp.md).
