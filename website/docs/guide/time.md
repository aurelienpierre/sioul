---
description: Time spent in Sioul - from the focus timer and noted by hand, kept to bill clients and to learn how long things really take; what is left to bill, a spreadsheet export, and invoices numbered without gaps.
---

# Time and invoices

Time spent is noted as you work. It is kept for two uses:

- **billing**: the hours you work for clients become invoices;
- **planning**: set beside your guesses, it tells how long things really take, and the plan uses it: see [How long things take](#how-long-things-take).

<figure markdown="span">
  [![The Time page on "Week": a bar per day, stacked in a few calm colours by project; under it, each project's hours and what is left to bill, then each stretch of time, newest first.](../assets/screens/time.png){ loading=lazy }](../assets/screens/time.png "Open the picture at full size")
  <figcaption>A week of time, by project, and what is left to bill.</figcaption>
</figure>

## The Time page

**Week**, **Month** or **Year**, for **All projects** or one; **Today** and the arrows ◂ ▸ move through time:

- a bar per day (per month, over a year), stacked by project;
- each project's hours, and what is left to bill, in hours and in money at its rate;
- then each stretch of time, newest first.

## Where time comes from

- **The focus timer**: the minutes of a [focus session](tasks.md#starting-and-stopping) count for its task, and for the task's project.
- **Note time**, on this page, on a project's page, or with **New ▾ ▸ Time spent**: a meeting, a call, work done away from the timer. **For** (a project, or a task), **How long** (`1h30`, `45m`, `90`), **When**, **What it was**, and **Not to bill** when it should not be.

While a focus session runs, on Linux and on a phone, a notification shows it: its task, since when, and **Pause** (**Go on** once paused) and **Stop**, which do what the focus window's buttons do. On a Linux desktop it stays among the desktop's notifications while Sioul is open (in KDE Plasma, under the bell once its popup goes), and goes when Sioul quits. On a phone, a chronometer counts in it, and its buttons work with Sioul in the background or closed. Paused or stopped elsewhere, in the focus window or on another device, it follows. On Windows and macOS, the focus window alone shows the time running, for now.

Click any stretch of time, timed or noted by hand, to change it: its task, its project, its day, from when to when, and what it was; the time the timer kept after a pause you missed comes back that way. A right click also offers **Take this time out**. Time already on an invoice stays as it was billed.

Each stretch says quietly how its minutes were known:
- *timed*;
- *noted by hand*;
- *timed, then corrected*, once you changed a timed stretch's length, or Sioul cut a timer left running all night;
- *not known how*, for time noted before Sioul kept this.

### Billed or not

Work for a client is billed. A task can say otherwise, under **Billed** in its panel: *As its project says*, *Its time is billed*, or *Not billed*.

## How long things take

A task's **Takes about** is a guess. The time noted for it is what it took. The plan sets the two side by side, and corrects your guesses in the plan only:

- **Your first guess is kept.** The first time a task gets a length, Sioul keeps it with the task and never changes it, even when you change **Takes about** later. A task given a length before this existed has none kept; its current length is used instead.
- **Only finished tasks teach it**, each one's time spent against its first guess, all its stretches added up.
  - Timed minutes count fully, corrected ones too.
  - Minutes noted by hand count a quarter, because remembered durations are less sure.
  - Dropped tasks never count.
- **Recent tasks count most**: a task's weight halves every four or five days.
- **Few tasks, little correction.** Until about nine tasks stand behind it, the correction leans toward "a little longer than guessed".
- **By what tasks are for**: work, your admin and leisure each have their own correction, pulled toward yours until they have enough tasks of their own.
- **The plan uses it; you see your own.** Each task takes its corrected length in the plan and in the day, while **Takes about** stays as you wrote it, and the focus timer starts from it.
- **The margin is the day's, not each task's.** Each day keeps some time free after its last step for steps running long: about enough to cover a day going worse than usual, never more than a third of the day. Once today's first tasks are done, a day going slower keeps more. The next step always keeps part of today.
- **On request**, in a task's panel, a line such as "Tasks like this usually take about 1.3× the first guess; the plan already allows for it", only once enough tasks stand behind it.

It never scores you, never shows anything as late, never fills in **Takes about** for you, and never compares you with anyone.

Timing leisure is up to you. Gaps are fine: time not noted is left out, not counted against anything. The detail, with what comes from research and what is a guess: [What a day holds](../dev/capacity.md).

## A spreadsheet of billable time

The export button, at the top right of the Time page: **Billable time, as a spreadsheet (CSV)…**, for one project, and a stretch of time (this week, last week, this month, last month, all time, or from one day to another). One line per task, with its hours, the hourly rate and the amount, then the total. In French, the file uses `;` and a decimal comma, as French spreadsheets read them.

## Invoices

On a project's page, or beside the project on the Time page, **Make the invoice** when billable time waits.

- **Numbers** follow each other within a year and are never reused: 2026-001, 2026-002, or your own prefix.
- **Lines**: one per task (or per note of time noted without a task), its hours at the project's rate.
- **Billed once**: the time an invoice bills carries its number; the next invoice leaves it out.
- **The mentions French law asks for**: your name, address and SIRET, the VAT line ("TVA non applicable, art. 293 B du CGI" for a micro-entrepreneur), payment details, late-payment penalties and the €40 recovery fee.
- **The PDF** (A4) goes to `Documents/Invoices` in your home folder, or the folder you chose. **Print again** writes it anew.
- **Your budget**: the total is expected in the project's budget, due thirty days later. **Paid** dates it today, and the money counts.

Your name, address, numbers, currency, payment details and hourly rate are set once, in [Settings ▸ Invoices](settings.md#invoices).

### On several devices {#on-several-computers}

An invoice number must never be given twice. When you [share between your devices](sharing.md), one device numbers the invoices: another says "Invoices are numbered on …", and **Make invoices on this device** takes them over, after a minute and a half, once your other devices know. If your sharing folder cannot be written, or another device has not been heard from for a few minutes, Sioul waits rather than risk a number twice.

## In quiet time

Outside working hours, the Time page waits behind **Show anyway**. See [Hours](hours.md).
