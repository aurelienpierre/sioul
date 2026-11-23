# Supplies

What you use up or sell: stamps, envelopes, cards printed by hand, cat food. Each supply is one line of a list of Sioul's own; how many are left is worked out from movements, never stored; a task can say what it will use, and finishing it takes that out of the stock; when what is free runs low, a task to resupply is made, once. The page is **Supplies** in English, **Stocks** in French (« Fournitures » would leave sold goods out). Part 1 of the design of 23 November 2026: the supplies, their page, and tasks that use them; part 2: routes that make a task of each message, each planning the supplies of its model ([notes-folder.md](notes-folder.md#routes-that-make-tasks)).

Code: `sioul_core::supplies` (the list, the movements, the stock, `settle`, the task to resupply), `sioul_core::tasks` (a task's lines), `crates/sioul-app/src/supplies.rs` and `qml/SuppliesPage.qml`, `SupplyCard.qml`, `SupplyLines.qml`, `SuppliesUsed.qml` (the window), `sioul_sync::share` (`SUPPLIES`, `SUPPLY_MOVES`). Tests: `crates/sioul-core/src/supplies_tests.rs`, `crates/sioul-app/tests/qml/tst_supplies.qml`.

## Calm first

- **No colour of alarm, no count of what is low, nothing "overdue"**: "low" and "none left" are quiet words beside a supply, in the same muted colour as the rest of its line.
- **The calm view first** (Supplies ▸ For now): the supplies of what now is for, by category, one line each: its name, what is free ("9 left"), "4 planned" when open tasks plan some, and "low" or "none left". **All supplies**, one tab away, shows every supply whatever the hour, with its stock, its threshold and its prices, the archived folded at the end ([areas.md](areas.md#calm-views-and-full-views)).
- **Finishing a task applies what it planned, with no question.** What it really used is said in one optional press: **Change…** beside "How was it?" in the status line, or the section **Supplies used** on its card, kept there to put right days later. Nothing asks to check a count: counts are taken as planned until corrected, and **Count again…** is the correction, at any time.

## The list

`sioul-supplies.toml`, where `sioul-projects.toml` is: the notes folder, then `records/` of the data folder once this device moved its lists ([notes-folder.md](notes-folder.md#what-lives-where)).

```toml
[[supply]]
id = "stamps"          # made once from the name (accents off, lowercase, dashes), unique; never changed
name = "Stamps"
category = "Post"      # a word of yours; one kind of category
area = "work"          # work, admin, leisure, joined by "+"; unsaid: every time
unit = ""              # unsaid: a piece
buy = "1.39"           # a unit's buying price, as typed; may be unsaid
sell = ""              # a unit's reselling price, optional
low_at = 10            # of what is free; unsaid: never low
bought_by = 50         # the usual quantity of an order, optional
project = ""           # where its tasks to resupply go, optional
archived = false       # kept with its history, out of the page's lists
```

- **Read leniently** (`List::load`): a `[[supply]]` without an id that can be one, or with an id said twice, is passed over and counted (`skipped`); a field of another type reads as unsaid. A file that is no TOML is said on the page, and never written over.
- **Written key by key** (`supplies::change`), under the file's lock, which the sharing takes too: a key changed keeps its place and its spaces, an empty text takes it out, every other key, supply and comment stays; what is written must read back as the list, else nothing is.
- **Never deleted while it has a history**: **Archive** on its card; **Bring it back** in the archived fold.

## The movements

Each device writes its own file alone, `sioul-supplies-moves/<device>.toml` beside the list (its id in the sharing, made safe as a file name; a device that never shared has an id too, from the same place, `share::Here`), and reads every device's file. So two devices never write one file.

```toml
[[move]]
id = "…"          # by chance for a count and "Received…" by hand; else made from what it is about
at = 1795435200   # when it was said, Unix seconds
on = 1795431600   # when it happened, when not when it was said (a task's completion)
supply = "stamps"
kind = "used"     # "count": there are `quantity` now; "received": so many more; "used": so many fewer
quantity = 1      # zero or more; decimals allowed, kept to the millionth, never rounded more
task = "…"        # the task's UID, for a use and for what a task to resupply brought

[[made]]          # a task to resupply made, by its UID: never made again
task = "…"
supply = "stamps"
at = 1795435200
```

- **Ids**: a task's use is `used_id`, a name-based UUID (RFC 9562 §5.5) of "used", the task's UID, the supply's id and the task's completion stamp; what a task to resupply brought, the same with "received". Several lines of one id (a quantity put right later, a task read finished by two devices): **the latest `at` wins**, the file read last at the same `at`; a quantity of 0 says nothing was used after all. A device's own file keeps each id once, its latest line (`record`).
- **The stock** (`stock_of`) = the latest count's quantity (none: 0) + what was received after it − what was used after it, each id once, "after" by when it happened (`on`, else `at`). `on` keeps a use put right after a count where it happened: the count already holds it. A stock under zero is shown as it is on the card, with "Count again…".
- **Free** (`standing`) = the stock − what the open tasks plan to use (`planned`). **Low**: free at or under `low_at`, above zero; **none left**: free at or under zero. Ten sales arriving with eight cards in stock make the stock "none left" when the mails arrive, not when the eighth card is used.
- A file that does not read is passed over and said; this device's own is never written into, and nothing is applied meanwhile.

## A task's supplies

| What | Written as |
|---|---|
| What it plans to use, a line a supply | `X-SIOUL-SUPPLY;X-SIOUL-ID=<supply id>:<quantity>` |
| What it used, written when Sioul finishes it (what it planned, unless changed there) and whenever it is put right; a supply it used none of at 0 | `X-SIOUL-SUPPLY-USED;X-SIOUL-ID=<supply id>:<quantity>` |
| A task to resupply | `X-SIOUL-RESUPPLY:<supply id>`, tagged `supplies` |

- Lines of Sioul's own, an X- property with an X- parameter, as [compatibility.md](compatibility.md) says of the others: a phone's account manager keeps them byte for byte; Google Tasks keeps none (said first when a task moves there, "the supplies it uses"). A line of one supply said twice: the last counts.
- **Planned**: the task's form, a section **Supplies** shown once you keep supplies or the task has some: a supply and how many a line, "Add a supply" (`TaskEdit::supplies`, written as a field of the form).
- **Used**: `tasks::set_status` writes them when the task's state changes to done (its plan copied) and takes them off when it changes back (open again or dropped), in the same single write; a tick on a task done already keeps what was put right. A finished task without them (finished in another program) used what it planned (`used_by`).
- **A repeating task**: a turn done comes back open, with no completion in its file, so no other device can see it. Its use is written at once by whatever finished it (the window, `sioul tasks done`, the agents' tools: `after_status` → `turn_done`, its id from the moment), never given back by `settle`; **Change…** in the status line puts that turn right while the window runs. No `X-SIOUL-SUPPLY-USED` on a repeating task.

## Tasks a route makes
A project's route that makes a task of each message ([notes-folder.md](notes-folder.md#routes-that-make-tasks)) writes the supplies of its model on each task (`X-SIOUL-SUPPLY`, as the form does): open, they count in what is free at once, so that ten sales arriving with eight cards in stock make the cards "none left" as the mails arrive, and a task to resupply is made then, not when the eighth card is used. The model's supplies are chosen in the route's form ("Make a task of each message"), shown once the person keeps supplies; finished, such a task is applied as any other.

## Applying, on every device

**Applying is idempotent** (`settle`): after every re-plan in the window (`supplies::upkeep`, which runs after the children's upkeep), and right after the command line or an agent's tool changes a task's state (`after_status`), each device makes its own file say what the tasks say:

- a task done (not repeating): a `used` line per supply it used, the id above;
- a line of a task no longer done, or of a supply taken off it: a line at 0;
- a task to resupply done: what it brought (`bought_by`) as a `received` line, **only when no device said it yet**, so that the person's answer to "How many came in?" is never undone; finished in another program with no `bought_by`, nothing, and the supply's card says once that a resupply was done and asks how many came in.

Only what is missing or different is written, and **never over a word said after the task last changed** (a line whose `at` is past the task's `LAST-MODIFIED`): a device that reads a correction's movement before the corrected task reaches it by CalDAV does not put the old quantity back. Every path that finishes a task therefore applies its supplies: a list's tick, Now's **Done**, the card's **Done**, a step's tick, the focus window's **Stop here, it counts**, several tasks changed together (Search), a bigger task closed with its last step, what closes by itself after an appointment, a call that closes a call-back, `sioul tasks done`, `sioul focus stop --done`, `complete_task`, `update_task`, and another program or device (read after a sync). The window and the command line also apply at once where they finish a task, to say what it used: "Done. Used 1 Stamps, 1 Envelopes."

## The task to resupply

After each pass, for each supply kept with a `low_at` whose free stock is at or under it (`to_resupply`), and none open for it:

- **made once**: its UID a name-based UUID of "resupply", the supply's id and the id of that supply's latest count or received line (`resupply_uid`), so that two devices make the same task; its file named by its UID (`<uid>.ics`, as the children's follow-ups are), so that two devices write one file; kept as made (`[[made]]`), so that one deleted is not made again until the next count or arrival;
- titled "Resupply: Stamps" (« Réapprovisionner : timbres »), the supply's name as you wrote it; tagged `supplies`; for what the supply is for; in the supply's project when it has one;
- in the list of the page's setting **Tasks to resupply go to** (`[supplies] list`, Supplies ⚙), else as Health's errands choose theirs: their list, your usual list, the first list on a server (so a phone has it), else one on this device (`resupply_list`); with no list at all, a list kept on this device, "Supplies";
- said once in the status line: "“Resupply: Stamps” is in Work.";
- finished in Sioul, "How many came in?" asks, filled with `bought_by`; the answer writes the `received` line.

## How they travel

In the part **Projects and money** of the sharing ([database.md](database.md)), both layouts (the notes folder, or `records/` once moved): the list as one of Sioul's own lists, each `[[supply]]` an entry by its id, guarded as the others ([database.md](database.md#sioul-s-own-lists)); the movements' folder whole file by whole file, each written by its own device alone. A store older Sioul versions do not know: both travel only once every other device's sealed entry says its Sioul has them (`devices::Entry::supplies`, `share::supplies_shared`), then stay in this device's exchanges for good (`Memory::supplies_open`), as the client profiles do. A sync app carrying the notes folder carries them itself before the move. Both are left out of the notes, and the move of Sioul's lists carries them to `records/` (`storage::list_names`, the movements' files one by one).

## For agents and the command line

`get_task` gives `supplies` (what it plans: `supply`, `name`, `quantity`, `unit`) and, done, `supplies_used`; `complete_task` and `update_task` apply them as the window does and say what was used ([mcp.md](mcp.md)); `sioul tasks show` says "Uses: …" and, done, "Used: …"; `sioul tasks done` says what it used. No tool of its own: supplies are read where tasks are.

## Not done

- A project's page listing the supplies its routes and tasks use.
- Named sets ("gift card kit"), the buying price as a sale's cost and a resupply as an expense in the books, one task per supplier for several low supplies, fields read from a mail, a card printed from a template, shops' orders: each on its own word.
- A repeating task's turn done among several changed together (Search's batch) is not counted: only the window's tick, `sioul tasks done` and the agents' tools write a turn's use, since the task comes back open and keeps nothing of it.
- No product was researched for a comparison: the guide's page has no table of other programs.
