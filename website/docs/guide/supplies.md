---
description: Supplies in Sioul - what you use up or sell (stamps, envelopes, cards, cat food) and how many are left, counted from what came in and what your tasks used; a task to resupply made once when one runs low, never a red number.
---

# Supplies

## In short {#in-short}

**Supplies** keeps what you use up or sell: stamps, envelopes, cards printed by hand, cat food. Each one says how many are left, counted from what came in and what your tasks used. A task can say what it will use ("1 stamp, 1 envelope, 1 card"); ticking it done takes that out of the stock, with no question, and one press changes it when a second envelope went into it. When what is left runs low, a task to resupply is made, once, in the list you choose. Nothing turns red, nothing counts what is low.

## Protected by default {#what-is-protected}

- Your supplies are one plain file on your device, among Sioul's own lists, and what came in and went out is a file per device beside it (until you move Sioul's lists, both are in your notes folder: [Where Sioul keeps its lists](sharing.md#where-sioul-keeps-its-lists)). There is no server of Sioul's.
- Between your devices they travel sealed through Sioul's sharing, in **Projects and money**: the folder and its server see that something changed, never what ([Sharing](sharing.md)). Each device writes only its own file of movements, so two devices never write over each other.
- What a task plans to use and what it used are written in the task itself, so they travel with it to your calendar server: other programs keep those lines, Google Tasks does not.
- The tasks to resupply go to the list you choose: if that list is on your calendar server, their titles ("Resupply: Stamps") are there too.

## The page {#the-page}

**Supplies** is among the places, after Projects.

- **For now**, first: the supplies of what now is for (work in working hours, your admin in its hours, leisure the rest of the time: [Hours](hours.md)), by category, one line each: its name, how many are left once the open tasks have what they plan ("9 left"), "4 planned" when some is, and a quiet "low" or "none left". How many are kept for other times is one line under them.
- **All supplies**, one tab away: every supply whatever the hour, with what is in stock, when it is low and its prices; the archived folded at the end.
- **⚙**: **Tasks to resupply go to**: a task list, else where Health's errands go.

## A supply {#a-supply}

**New supply**, then:

- **Name**: "Stamps", "Gift cards".
- **Category**: a word of yours ("Post", "Shop"); the ones in use are offered under the field.
- **What it is for**: work, your admin, leisure, as a task's; nothing ticked, it shows at every time.
- **Unit**: nothing for a piece, "g", "m", "sheets".
- **Buying price** and **Reselling price**, a unit, as you write them.
- **Low at**: under how many it is low, of what is left once planned tasks have theirs; empty, never low.
- **Usually bought by**: how many come in an order: what a task to resupply brings when it is done.
- **Its tasks to resupply go to the project**: optional.
- **How many there are now**: the first count.

A press on a supply opens its card:

- **What it is**: its fields, each saved as you change it.
- **In stock**: the number, with **Received…** (so many came in) and **Count again…** (there are so many now). Counting again is how a number is put right, at any time; nothing ever asks you to check one. A stock under zero is shown as it is, with that word.
- **Planned**: the open tasks that will use it, each opening its task.
- **History**: what came in and went out, newest first, each saying why and what was left after it ("Used: 1 → 11", with its task); **The whole history** shows past the last ten.
- **Archive**: a supply with a history is never deleted; archived, it leaves the lists, and **Bring it back** returns it.

## Tasks that use supplies {#tasks-that-use-supplies}

- **In the task's form**, the section **Supplies** (once you keep some): a supply and how many, a line each, **Add a supply**. That is what it plans to use: the page counts it as planned at once, so ten sales coming in with eight cards in stock make the cards "none left" when they come in, not when the eighth card goes.
- **Ticked done**, wherever you tick it (a list, Now, its card, the focus window, an agent, the command line, your phone, another program), what it planned is taken out of the stock, with no question. The status line says it: "Done. Used 1 Stamps, 1 Envelopes, 1 Gift cards.", with **Change…** beside **How was it?**, for a second envelope or a supply nobody planned; for a task that planned nothing, **Supplies used…**.
- **On a task done**, its card keeps the section **Supplies used**, each line changeable, **Add a supply**: put right days later if you like.
- **Open again**, its supplies come back.
- **A task that comes back** (every week) takes what it plans at each turn done; **Change…** puts that turn right.

## When one runs low {#when-one-runs-low}

When what is left of a supply is at or under its **Low at**, and no task to resupply it is open, one is made, once: "Resupply: Stamps", tagged `supplies`, for what the supply is for, in its project when it has one, in the list of **Tasks to resupply go to**. The status line says where it went. The plan lays it in the hours of its kind, as any task. Deleted, it is not made again until the next count or arrival.

Ticked done here, it asks **How many came in?**, filled with **Usually bought by**. Done in another program, what it is usually bought by comes in by itself; when it says none, the supply's card asks once.

## Going further {#going-further}

- **By hand, if you like**: the list is a readable file, `sioul-supplies.toml`; its form is [below](#for-technical-readers).
- **An AI agent you connect** ([Using an AI agent](ai-agent.md)) reads what a task plans to use and what it used, where it reads the task; marking a task done there takes its supplies out as here. It has no tool of its own for supplies.
- **On a device not updated yet**: the supplies travel only once every one of your devices has a Sioul that knows them; until then each device keeps its own.

## Why it works this way {#why-it-works-this-way}

A number kept by hand goes wrong in silence, and checking it becomes one more chore. So Sioul keeps what happened (what came in, what each task used, each count) and works the number out of it: every number is explained by its history, one task set back to open gives its supplies back, and two devices that each use some never write over each other. Low is said of what is left once planned tasks have theirs, so that the warning comes when the orders come in, not when the last card is gone. And it is said as a word, never a red number to fight: the task to resupply goes into the plan, where it is done in its time.

No other application was compared for this page.

## For technical readers {#for-technical-readers}

`sioul-supplies.toml`, beside `sioul-projects.toml`:

```toml
[[supply]]
id = "stamps"          # made once from its name; never changed
name = "Stamps"
category = "Post"
area = "work"          # work, admin, leisure, joined by "+"; unsaid: every time
unit = ""              # unsaid: a piece
buy = "1.39"
sell = ""
low_at = 10            # unsaid: never low
bought_by = 50
project = ""
archived = false
```

What came in and went out is in `sioul-supplies-moves/`, a file per device, each written by its own device alone; a task's supplies are lines of the task (`X-SIOUL-SUPPLY`, `X-SIOUL-SUPPLY-USED`). The rules, the files' forms and how they travel: [Supplies, for developers](../dev/supplies.md).
