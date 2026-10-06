---
description: Letting an AI agent such as Claude Code or Claude Desktop read what Sioul keeps and prepare tasks, events, notes and drafts, through MCP; it never sends, deletes, pays or reads a password.
---

# Using an AI agent

An AI agent you already use, such as Claude Code or Claude Desktop, can read what Sioul keeps on this device and prepare work for you: tasks, events, notes, ties between things, drafts. It explains and prepares; you review. It never sends, never deletes, never moves money, and never reads a password. Sending stays your keypress.

Sioul does not connect one by itself. This page is for when you want to.

## Before you connect one

!!! warning "What an agent sees"
    Once connected, an agent can read everything Sioul keeps on this device: your mail, tasks, agenda, contacts, budgets, notes and projects, whatever they are for. What it reads goes to the model behind it (Anthropic's, for Claude), under your own agreement with that company. Opening some projects to an agent and keeping others closed is not there yet.

Some things are kept from it whatever happens (below): passwords and keys, one-time codes, bank and card numbers.

## Connecting

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

## What an agent can read

- **The Porch**, lane by lane, in your hours only, unless you ask it to open it. It is told that a code came, never the code.
- **Your mail**: messages found by their subject, sender or text; one message whole, with who wrote, whether it is genuine or forged and why, and its attachments' names.
- **Your tasks**: the next step and why, the day laid out, every open task in the plan's order.
- **Your agenda**, your **contacts**, your **budgets** (each verdict, the reserves, what the money watch noticed), your **notes**, your **projects**.
- **What anything is tied to**, both ways, and anything found by a few words of its title.

## What an agent can prepare

On this device only:

- **a task**, or **a task marked done**: a task goes to your calendar server at the next sync, as one made in the window; a task marked done can be opened again from the window;
- **an event**: nobody is invited;
- **a note**, never written over another one;
- **a reply**, or **a new message**: saved in Drafts, never sent. You read it, and send it from the window, or delete it;
- **a tie** between two things.

## What does not move

- **Nothing is sent, deleted or paid.** No tool sends mail, deletes or moves a message, moves money, or speaks to a server.
- **Nothing secret.** No password, key or token is read. One-time codes are hidden wherever they are written. Bank account numbers (IBAN), card numbers and French social security numbers are masked in mail, notes and budgets; their last four characters stay, to tell two accounts apart.
- **Mail is data.** A message's text comes marked as its sender's words, not as instructions, and the agent is told so when it connects. Forged mail is said forged. Hostile mail to a protected address, and encrypted mail, are not given.
- **Writing only adds.** A note never replaces another; a tie is made once.
- **Your hours hold.** The agent is asked to respect them, unless you ask otherwise.
- **A log.** Each call is written down on this device: which agent asked, what for, and the text it was given, one file a month in Sioul's own folders. That log stays here; the agent's model received that text.

## The command line

Agents and scripts can also use the command line, as you can: `sioul tasks now`, `sioul tasks list`, `sioul links`, `sioul notes`, and more (`sioul --help`).

`sioul tasks import plan.toml` takes a plan written as a file: tasks with their steps and what they wait for, the contacts of the offices involved, and drafts, which stay in Sioul and are never sent. Importing again updates what the file made, instead of adding it twice; `--dry-run` says what would change. The file's format: [Tasks, "Importing"](../dev/tasks.md#importing).

## The other AI in Sioul

One more feature uses an AI, and only for an address you protect against harassment, when you allow it: **Let the AI read it first** sends each new message to that address to Anthropic's Claude, to say its tone and topic. See [the Porch](porch.md#a-public-address-protected).

## Not there yet

- Opening some projects to an agent and keeping others closed.
- ChatGPT, and agents that run elsewhere than on this device.
- A companion inside Sioul that explains a letter in plain words, line by line, and keeps you company during an admin session.

The design behind all of this: [AI providers and agents](../dev/ai.md) and [the MCP server](../dev/mcp.md).
