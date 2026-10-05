# Projects, time and invoices

A project is any matter you follow, one list for all of them (`sioul-cases.toml` in your notes folder, [case-store.md](case-store.md)): the umbrella over its tasks (REFID), events, mail (its routes), notes, time and budget lines. Projects were once "cases" beside "projects"; they are one thing now, and "for a client" is a choice in its form. Made, renamed, given its routes and taken out from the Projects page (its form, "Its mail"); taken out, its tasks, notes, mail and time stay where they are. The Porch's settings only choose which projects have a lane there (Porch ⚙, "Projects shown here"). A project **for a client** (`kind = "project"`) has a client, an hourly rate and the budget its invoices are expected in, and its time can be billed.

```toml
[[case]]
id = "lumen"
title = "Studio Lumen — website"
kind = "project"
client = "Studio Lumen"        # or "sioul:contact/<UID>": the invoice takes its postal address
rate = 60.0                    # else [invoice] rate
budget = "work"                # invoices expected there until paid
```

## A project's page (Projects, Ctrl+8)
- **One line of time**: what is coming first (dates asked of its open tasks, events), then what happened, newest first (tasks done, mail by its routes or tied by hand, notes of its record or tied to it, time noted, invoices). Mail comes by its routes on sender and subject (matched again each time), by being tied to the case (when it arrived, or by hand), and by conversation: the whole of a conversation one of its messages belongs to ([case-store.md](case-store.md)).
- **Its tasks** as a board, a list or a calendar: the Tasks page, filtered to the project.
- **"Note time"**: a meeting, a call, work away from the timer; for a project alone or a task.
- **"Make the invoice"**, when billable time waits.

## Time (Ctrl+9)
A week, a month or a year: a bar per day (per month over a year) stacked by project, each project's hours and what is left to bill, in hours and in money at its rate, then each stretch of time, newest first.
- **Billed or not, task by task**: a task's form says "Billed": as its project says (work for a client is billed), its time is billed, or not billed (`X-SIOUL-BILLABLE` in the task, kept by CalDAV servers; greyed on Google Tasks).
- **As a spreadsheet**: the export button on the Time page writes a project's billable time as CSV: what (a line per task, or per note), hours, hourly rate, amount, then the total; this week, last week, this month, last month, all time, or from one day to another. In French, `;` between fields and a decimal comma, as French spreadsheets read them. Nothing else goes in: the full invoice is the invoice's (below). Time comes from the focus timer (a task's time counts for its project: the first of its cases that is a project, else its first case) and from what is noted by hand (`$XDG_DATA_HOME/sioul/time/<month>.toml`, sessions with `project`, `unbilled`, `invoice`). Any stretch not billed, timed or noted by hand, changes with a click: its task, its project, its day, from when to when, what it was (`qml/TimeDialog.qml`; moved to another month, it is written there first); its right click takes it out. Billed time stays as it was billed.

## Invoices
- **Numbers** follow each other within a year and are never reused: `[invoice] prefix`, else the year: 2026-001, 2026-002.
- **Lines**: one per task (or per note, for time noted without a task), its hours at the project's rate, rounded to the cent.
- **Billed once**: the time an invoice bills carries its number; the next invoice leaves it out.
- **Legal mentions** (France): the issuer's name, address and SIRET, the VAT line (`TVA non applicable, art. 293 B du CGI` for a micro-entrepreneur), payment details, the late-payment penalties and the €40 recovery fee (Code de commerce, L441-10 and D441-5), no discount for early payment.
- **Records** in `$XDG_DATA_HOME/sioul/invoices/<number>.toml`; the PDF (A4, through Qt's text document and PDF writer) in `[invoice] folder`, else `~/Documents/Invoices` (Factures in French).
- **Budget**: the total is expected in the project's budget, a planned line due thirty days after, tied to `sioul:invoice/<number>`; ticking "Paid" dates it today and makes it real.

## Budgets at their pace
A budget's verdict no longer compares a projection made only of what is scheduled with its target, which said "short" on the 3rd of every month. What is known in advance (presets, planned lines) counts in full; the rest, what simply happens, is weighed against the part of the period gone: ten days into thirty, a third of what the period still needs should have come in (or no more than a third of what it may spend should be spent). Within 5 % of what the period moves (and never closer than 10 units), it is on track.

The end of the period is estimated:
- with fewer than three past periods: the difference of today carried to the end (no extrapolation of three days into a month);
- with three or more: what is known, plus the median past period's unscheduled movements for the part left; the worst and best past periods give a range (the 10th and 90th percentiles from ten periods on).

A budget opens on its ledger (lines and recurring movements of the period, newest first), its balance by day, week, month or year (planned movements dotted), and "Add a movement": once, or each month or year (`[[preset]]`, written in place, comments kept).
