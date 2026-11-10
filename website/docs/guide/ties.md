---
description: The chain of ties in Sioul - from any thing, what it is tied to and what those are tied to, both ways, the strongest chains first, drawn as a tree read from the left or listed as an outline.
---

# Ties

## In short {#in-short}

Everything in Sioul is tied to other things: a task made from a message, a payment told by a mail and counted in a budget, a letter for your child, a note in a project. From any thing, **Ties** shows the chain: what it is tied to, what those are tied to, up to four steps, both ways, whoever wrote each tie. The strongest chains come first; a project, a person or a budget is shown with how many things it holds, not opened unless you ask. Each thing opens its card in one click.

## Protected by default {#what-is-protected}

- The ties are read where they are already written (your tasks and events, your notes, Sioul's own small files): Sioul keeps no second copy of them, and the chain is made in memory each time, never written anywhere.
- Mail the Porch set aside never joins a chain by what it claims (a reply, its sender, a project's words); a tie you made by hand still shows it, in the Porch's own words, and goes no further.
- An event someone shares with you for their availability alone shows as "Busy", and its ties, which are theirs, are not followed.
- When a health thing is on the page (an errand to the pharmacy, its appointment), your phone keeps Sioul's picture out of its overview of apps, as it does for Health.
- An AI agent, only if you connect one, follows ties only through what you opened to it: a thing kept from it is neither shown nor gone through, and how many such things there are is counted, never named ([Using an AI agent](ai-agent.md)).

## Opening the chain {#opening-the-chain}

- **Ties** at the foot of any card (a task, an event, a note, a contact, a project), beside **Add** and **Link to…**;
- **See them all** under what a thing is tied to;
- **Its ties** in the menu of a thing (a right click, or a long press on a touch screen), in the chain itself too;
- **Ties** on a paper's, a contract's, a letter's, an invoice's, a budget line's or a movement's card (Budgets ▸ **Movements**), and on a person's page;
- **Ctrl+G** on the thing open.

Each opening starts afresh from that thing. **Escape**, or Back on Android, returns where you were.

## What it shows {#what-it-shows}

Each thing comes by its strongest chain of ties from where you started, said in words: "Told by: Message, Rent for November · Made into: Task, Pay the rent". A tie you made by hand, a task made from a message, a reply, a step, an appointment tie strongly; being in the same project, a sender, a mention less; a project's words alone, or a client by name, least. Each step after the first counts a little less.

- **At most five things under each thing**, 24 in all on a computer and 12 on Android.
- **A collection stops the chain**: a project, a contact, a person, a budget or a site is shown with how many things it holds. Opened, it shows one of each kind first (a letter, a task, a note, a message, a payment), then the rest by date, and no more than two messages from one sender before the others, so that a weekly newsletter never hides an office's letters.
- **Time orders, never cuts**: among equals, what is nearest in time to the thing you started from comes first.
- **Time noted** on a task is a count on it, not a thing of its own.

## More, and less {#more-and-less}

**+N** on a thing says how many of its ties are not shown, and what they are ("12 more: 9 messages, 3 tasks"). **+** shows eight more, then eight more; nothing already shown moves or goes. **−** folds them back, and the picture is as before.

## The drawing and the outline {#the-drawing-and-the-outline}

- **The drawing** reads from the left: where you started, then one column per step, the tie's words on each line. A tie between two things already shown is a thin curve. Zoom with Ctrl and the wheel, or a pinch; **Fit** shows it whole. On Android, two columns fit across the width, and a swipe moves one column.
- **The outline** lists the same things in the same order, each indented by its step, how it is tied on its second line.

A computer opens on the drawing, Android on the outline; **Chain** and **Outline** switch, staying on the same thing.

Each thing shows its family by its shape and its colour, and its kind by its icon and in words:

| Family | Shape |
|---|---|
| Time and doing (tasks, events) | a rounded square |
| Messages (mail, drafts, letters) | a wide pill |
| People (contacts, your people) | a circle |
| Money (payments, budgets, invoices, contracts) | a diamond |
| Records (notes, papers, sites, files, web pages) | a page with a folded corner |
| Projects | a hexagon |

## Keys {#keys}

In both views, as in a tree: **↓ ↑** move; **→** shows more, or steps into what is under; **←** folds, or steps out; **Enter** opens; **Home** goes back to where you started; the **menu key** (or Shift+F10) opens the thing's menu (Open, Its ties, show more or fold, undo the tie); typing finds a title. A screen reader reads each row as its kind, its title, how it is tied and what more there is, and says whether it is folded.

## Why it works this way {#why-it-works-this-way}

People find their things again by small steps from what they know, more than by searching (Teevan et al. 2004; Bergman et al. 2008), and the reason and the time of a message are what they remember of it (Elsweiler et al. 2008). A drawing helps to follow a chain from one thing to another, and only while it stays small (Ghoniem et al. 2004; Yoghourdjian et al. 2021); a list serves to look something up. So one walk gives both, small, with "+" for more.

What is near a thing is not everything tied to it: interest falls with each step, and a project, a correspondent of years or a budget is a place to stop, not to go through (Furnas 1986; van Ham & Perer 2009; Crestani 1997). Each thing is explained by its one chain, so the picture can be told in words, and the same files give the same picture.

Colour alone never tells a kind apart (WCAG 2.2): each family has its shape, each kind its icon and its word, and the colours are a colour-blind-safe set (Tol 2021).

More in [what the research says](../dev/research.md), findings 67 to 69.

## Compared with other apps {#compared-with-other-apps}

As of November 2026, from each app's own documentation.

✓ documented; **partly**, with a note; ✗ not found in the app's own documentation (for Sioul: not built); ? not confirmed; — not applicable. Sioul's column was checked against its code.

| | Sioul | Obsidian (local graph) | TheBrain | Tinderbox |
|---|---|---|---|---|
| The ties of one thing, drawn around it | ✓ | ✓ | ✓ | partly¹ |
| Ties of every kind of thing (mail, tasks, events, money, people) | ✓ | ✗² | ? | ✗² |
| Ties both ways, whoever wrote them | ✓ | ✓ | ✓ | ? |
| Things weighed by the kind of tie, the strongest chains first | ✓ | ✗³ | ✗ | ✗ |
| Big collections shown counted, not spread out | ✓ | ✗ | ✗ | ✗ |
| More shown on demand, nothing else moving | ✓ | ✗⁴ | partly⁵ | — |
| The same picture each time, laid out by role or by hand rather than by forces | ✓ | ✗⁴ | ✓ | ✓⁶ |
| The same chain as a list, for keyboards and screen readers | ✓ | ✗ | ? | ? |
| Kinds told apart without colour alone | ✓ | ✗⁷ | ? | ? |

1. Its map is the arrangement you make by hand, links drawn between notes; its Gaudí view lets related notes draw together.
2. Notes only.
3. A depth slider, and filters by search, tags and attachments.
4. Forces lay the graph out anew; its users say it moves on every load.
5. Clicking a thought makes it the centre, and the plex rearranges around it.
6. Placed by hand.
7. Colour groups by search.

Sioul has no graph of everything, on purpose: it answers no question of admin.

## For technical readers {#for-technical-readers}

**One index, in memory.** Every tie is read where it is written: a task's `LINK`, `RELATED-TO`, `CONTACT` and `REFID`; an event's guests, stand-in and `X-SIOUL-TAKEN-BY`; a note's links and front matter, and the projects whose files hold it; a draft's links and the message it answers; a budget line's links; `links.toml`; a payment as the budgets page derives it, with its line, mail, letter, invoice, contract and budget; papers, letters, contracts, invoices and persons; a message's `In-Reply-To` and `References`, its sender's card and a project's routes. Sioul gathers them in one pass into `ties::TieIndex` (each address once, each tie two numbers into the table with its kind), read from both ends. It is never written anywhere, and is made again when what it reads changes. A tie computed from a message's headers passes only through mail the Porch admits (`porch::Gate`).

**The walk.** `chain::walk` takes the strongest chains first, grown best-first from where you started: a thing's score is the product of its ties' strengths (1, 0.8, 0.6 or 0.4 by kind), times 0.75 per step after the first, each tie leaving a thing with *d* ties weighed min(1, √(4/d)). Four steps, 0.1 at least, five under each thing, 24 in all (12 on Android), eight more per "+" (`chain::Weights`). Rows come by a post-order count, so that a thing sits beside the first thing under it; the outline is the same tree in pre-order.

**In the window.** Rust computes the chain on a worker thread (`askChain`, answered by `chainReady`); QML draws it: `ChainPage.qml`, one `Shape` for every edge, each thing a `ChainNode.qml` with its `ChainBadge.qml`, the outline a `ListView` of tree items (`ChainOutline.qml`). The Related panel under each card reads the same index once it is made.

**On the command line.** `sioul links <address> --chain [--depth N] [--open <address>…] [--json]` prints the same chain as an outline, or as the window's JSON.

**What is not built.** Texts and calls in the chain (they have no address of their own).
