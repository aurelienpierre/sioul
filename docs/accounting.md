# Accounting: bank accounts, budgets and reserves

Money arrives and leaves by mail: invoices, receipts, payment notifications, payouts, tax notices. It is held in **bank accounts** (a current account, PayPal, Stripe), set aside in **reserves** (a Livret A, an assurance vie), and spent from **budgets** (what it is for). Sioul keeps the three apart, so you see at a glance:
- which budgets are positive or negative;
- which are on track, better or worse than planned;
- what each saving covers, and how long it lasts.

## Budgets
- **As many as you separate**: personal and professional, or one per project. For example: your duties (rent, energy, phone, food: what keeps you housed and fed), your leisure, and your work.
- **What each is for** (docs/areas.md): work, your admin, leisure, any of them together; it says the hours a budget is in view. Unsaid: your admin. The older word `area = "personal"` still reads as leisure.
- **A period and a target.** The period is a month or a year. The target is the balance to reach by the end of each period: 0 is breaking even, a positive target is saving.
- **The balance carries over**, as an envelope keeps what was not spent. Money set aside in October for a bill due in December is still there in December, and no reserve is drawn twice for it. `since` sets the day a budget's balance starts from; without it, the budget starts with the current period.
- **Lines**: credits in, debits out, each with a date and a label. Each can be linked to what it belongs with: the message it came from (a `mid:` URI), a task, an event, a Markdown file, a project. Budgets can be linked the same way.

## Made, changed, taken out, in the window
- **A budget**: "New budget" on the budgets page, "Edit" on its page: its name, month or year, the balance to reach, what it is for. Taking one out keeps its lines in the file, as written; they no longer count.
- **A bank account**: "New bank account" under Bank accounts, "Edit" on its card (below).
- **A reserve**: "New reserve" over the reserves, "Edit" on each: its name, its balance on a day, its floor, how many days money asked from it takes to arrive.
- **A line of the file**: right click, "Delete this line", asked once.

## Where lines come from
- **Mail.**
  - **What a message says about money is read first** (`crates/sioul-core/src/payments.rs`), in French and English: a payment received, a payment made, a merchant's order, a bill, a refund. Newsletters, activity reports, mandates and shipping notices are not payments; mail set aside (forged, spam, a borrowed brand name, a blocked sender) is never read.
  - **The amount** is the one the message states: the only amount of the subject ("SHOP: 23,98 € EUR"), else the one after the sentence that states the payment ("Vous avez payé 387,00 € à…"), else the total's line, else the last amount above zero, the way receipts end with what was paid. A discount is negative; a share capital in the legal footer is never read.
  - **Who was paid, or who paid**, comes from the same sentence, else from a receipt's label ("Paiement à", then the name), else it is the sender.
  - **Rules** on the sender's address or domain, and on words, as for a project's routes ([notes-folder.md](notes-folder.md#routes)): each condition listed must hold. A rule gives the budget, the direction, and the preset the message stands for (`preset`).
  - **One payment told twice counts once**: a merchant's order confirmation and the processor's payment for it (same amount, three days apart at most), a bill and the message saying it was settled (same sender), one message received at two addresses.
  - **Lines are proposed**: in the window's budgets page, "Add" writes the line into the budget file, with the message in its `links`, and a comment saying where it came from; "Not a payment" leaves the message out for good. From someone new and not verified, a line says to check it first.
  - **Amounts in another currency** are flagged for conversion.
- **Presets**, for payments that come back: rent, insurance, wages; and estimates of what is spent without a trace, like food.
  - Each is an amount every month or every year, on a day. Day 31 means the last day of every month.
  - Optional start and end dates, and an `id` for lines and rules to name it.
  - **A line that stands for a preset** (`preset = "phone"`, from a rule or by hand) replaces a fixed preset's amount for its month: the real bill instead of the planned one. For an estimate, it is spent from the envelope: what remains is still to come, and only spending beyond it counts more. Never on top.
  - A preset with no line standing for it counts as done once its day has passed; the bank's movements say when one did not pass (below, "The bank").
- **By hand**: a line, done or planned.

## On track, without assuming money flows evenly
For a budget's current period:
- **so far**: the lines up to today, presets dated up to today included;
- **to come**: the presets and planned lines until the end of the period;
- **carried in**: the balance at the start of the period;
- **expected**: carried in, plus so far, plus to come, compared with the target;
- **set aside**: the part of a surplus that planned lines after the period will take (a bill, a tax due later). It is kept, not counted as better than planned.
  - **Better than planned**: above the target by more than the tolerance.
  - **As planned**: within it.
  - **Short of plan**: below it.
- **The tolerance** is 2 % of what goes out in the period, and never less than 10.

## Reserves
- **A reserve is a savings account** (an assurance vie, a Livret A), with its balance on a date, a floor it is never planned below, and its delay (`delay_days`): how long money asked from it takes to reach your account, 0 for a Livret A, about two weeks for an assurance vie.
- **Cover rules** say which reserve covers which budget. At the end of each period, a budget's shortfall is drawn from its reserve; with `sweep`, a surplus goes back into it. The next period opens with the balance after that transfer.
- **Transfers**: a line with `reserve = "<id>"` moves money between a reserve and a budget, a withdrawal you ordered for example. It stays planned until it happens.
- **What each reserve shows**:
  - its balance today: the balance on its date, minus the transfers since;
  - what was used this month, and what is still planned;
  - its balance at the end of the year;
  - how many months it lasts above its floor at the coming year's pace.

## At a glance
In the window, each budget is a card: its title and period, the verdict on its own line (better, as planned, short, with its icon), then one figure per line: what came so far, what is expected by the end of the period, the target, what is carried in and set aside; then what a reserve does at the end. Each reserve shows its balance today, this month's flows, its balance at the end of the year, and how long it lasts at this pace. Below them, the mail about money, newest first, each with its state: counted, the same payment as another message, or waiting for you.

`sioul budgets`, on [../examples/sioul-budgets.toml](../examples/sioul-budgets.toml), on 2 October 2026:
```
Budgets, October 2026
  Home (monthly): −€600 so far, −€100 expected by 31 October, target €0: short of plan by €100.
    Savings account covers €100 on 31 October.
  Projects (monthly): €0 so far, €0 expected by 31 October, target €0: as planned.

Reserves
  Savings account: €20,000 today. This month, €0 used and €1,100 still planned. By 31 December, €16,700.
    At this pace it lasts about 16 months.

From mail
  2 October · Received · Jean Exemple · +€12 · Projects
  1 October · Paid · SHOP · −€23.98 · Home
  1 October · Order · Shop · −€23.98 · ?
      Same payment as SHOP (1 October)
```

## Bank accounts
Where money is and what it is for are two things. A **bank account** (`[[bank_account]]`: a current account, PayPal, Stripe) holds the money; the **budgets** say what it is for. Each account takes its own exports, and its movements fill the budgets it is said to fill (`fills`, the first taking what nothing else places), so one account can fill several budgets and one budget be filled by several accounts. Code: `crates/sioul-core/src/accounts.rs`, `crates/sioul-app/src/bank.rs`, `qml/BankSection.qml`, `qml/BankAccountDialog.qml`, `qml/BankRulesDialog.qml`, `qml/ReserveDialog.qml`.
- **Its exports**: "Take in an export…" on its card: OFX, CAMT.053, your bank's CSV, PayPal's activity download (net of fees, the name and the type of each row, its balance; pending, refused and memo rows left out, and rows in another currency than most of the file's) and Stripe's balance history (net, description, type). The number an export gives is kept with the account (`exports`), so that what was read under it before is the account's too.
- **Where each movement goes**, in this order:
  1. *your hand*: one movement placed in a budget, or in none (`[[assign]]`, from its row on the card; `parts` share one between budgets in the file);
  2. *the same payment as a line the budgets hold already* (from mail, by hand: the same amount, the line up to a week before the movement and three days after): counted there, once;
  3. *money moved between your own accounts*: the bank's "PAYPAL" or "STRIPE", PayPal's withdrawals and deposits, Stripe's payouts, when the other account's exports cover the day: in no budget, since the other side is read;
  4. *the first rule that holds* (`[[split]]`: words of the label, case and accents aside; money in, out or either; to a budget, to the recurring payment it stands for, to a reserve, or to another account). A rule without words takes every movement its way: "everything paid out of PayPal goes to duties";
  5. *the recurring payment it stands for* (a preset of a budget the account fills, by a word of its name or its amount, in the days around its date): the real amount replaces the planned one;
  6. *the account's first budget*.
- **Daily spending is spent from its estimate**: a card payment that stands for nothing is spent from its budget's estimate (food, clothes), the one with the most left in its period first, never added on top of it; past the estimate, it counts more.
- **Counted, not copied**: the budgets count the movements as lines on their day (`Ledger::with_bank`), in memory; the budget file is not written. A budget's page lists them without "Delete": they are changed where they were placed.
- **Topped up in your order**: each account is carried forward a month (its movements, then its budgets' recurring payments and planned lines; a budget filled by two accounts counts on the first). On the first day it would go under its floor (`floor`), what is missing, rounded up to ten, comes from its reserves in `topped_up_by` order, each as far as it holds above its own floor, asked its delay ahead: "Livret A: €300 to move before Sunday 25 October, when the insurance leaves; it arrives at once." "Assurance vie: ask for €400 by Thursday 15 October; it takes about ten days…" When asking today is already too late, it says so.
- **Before any is declared**, movements only feed the watch below, as before.

`sioul-budgets.toml`, for example:
```toml
[[bank_account]]
id = "bank"
title = "My bank"
fills = ["duties", "leisure", "projects"]
floor = 100
topped_up_by = ["livret-a", "assurance-vie"]

[[bank_account]]
id = "paypal"
title = "PayPal"
kind = "paypal"
fills = ["projects", "duties"]

[[split]]
words = ["mobile operator"]
preset = "phone"

[[split]]
account = "paypal"
direction = "debit"
budget = "duties"

[[assign]]
account = "bank"
movement = "0a1b2c3d4e5f6071"
budget = "leisure"
```

## The bank
Arrears that build up in silence are how it goes wrong: a debit stops, nothing says so, and nobody looks when accounts go badly (Olafsson & Pagel 2017); a payment reminder that ignores the balance can push an account into overdraft (Medina 2021). So Sioul reads your bank's own movements and holds them against your budgets' recurring payments. Code: `crates/sioul-core/src/bank.rs`, `crates/sioul-app/src/bank.rs`, `qml/BankSection.qml`.
- **Taken in**: Budgets ▸ The bank ▸ "Take in an export…": the file your bank gives (OFX or QFX, ISO 20022 camt.053, or its CSV: the header found by its words, `;` or `,`, decimal commas, debit and credit apart or not, the balance written above the table as some banks do). Movements are kept by the bank's own id (or one made from the day, the amount and the label), so an export read twice adds nothing; the newest balance is kept. `sioul-bank.toml` at the root of the notes folder; nothing is sent anywhere, except sealed to your other devices when they share projects and budgets (docs/database.md).
- **What it says**, in sentences, never in red:
  - *missed*: a recurring payment (or wages) expected around a day, inside what the exports cover, with no movement within three days before and seven after that names it or has its amount: "Free Mobile (€19.99) has not left the account. Expected around 12 September…";
  - *changed*: one that passed with another amount (more than 5 % and €2 apart): "EDF took €58.30 on 5 September, where €52 was expected";
  - *short*: the balance carried forward a month (the movements after it, the recurring payments to come, the planned lines, the daily spending spread over its period) going below zero on a day, with the payment that takes it there, what will be missing, and the reserve that covers it when one does: "The account may not hold the rent on 1 November. €579.99 will be missing then; Livret A covers it."
- **The week**: the payments of the next seven days, on the Budgets page and, while the Porch is open, in one line at its top: "This week: EDF €52 (Mon). The account holds them.", and, when the watch found something, "Something about money to look at", never a count.
- **The month ahead**: the balance as a plain line, zero dashed.
- **Reminders** ([reminders.md](reminders.md)): a payment missed, once, a week after its day; the day the account would not hold a payment, five working days before; and a planned payment's reminder says whether the account will hold it.
- **Short, with bank accounts**: once accounts say which reserves top them up, the topping-up sentences on each card replace the watch's own "short" sentence.
- **Not built**: fetching from the bank itself (Woob has modules for many banks; Sioul installs nothing): exports are taken by hand for now. Without bank accounts declared, bank lines do not become budget lines: the watch confirms, it does not keep the books.

## Contracts and subscriptions
What you are bound to (rent, energy, phone and internet, insurances, health cover, subscriptions, hosting, the bank), each with when it renews, the notice it needs, how to stop it and what it covers. Forgotten subscriptions are part of what people with ADHD call the "ADHD tax"; knowing what an insurance covers matters before a legal step. Code: `crates/sioul-core/src/contracts.rs`, `crates/sioul-app/src/contracts.rs`, `qml/ContractsSection.qml`, `qml/ContractDialog.qml`.
- **Where**: `sioul-contracts.toml` at the root of the notes folder, beside the budgets; shown on the Budgets page under "Contracts and subscriptions".
- **Found, not typed**: the recurring payments out with no contract yet are listed below the contracts, with "Note it" (the kind guessed from the payment's name, with the contracts' word lists of [the words Sioul looks for](words.md): "Loyer" a rent, "EDF" energy, "Free Mobile" a phone line, "MAIF" an insurance, "Abonnement mensuel" a subscription); a mail becomes one with "Keep as a contract…" in the reader's menu (its subject, its sender, the kind they suggest). Bank lines will propose the debits that no preset names yet.
- **Where it stands**, in a sentence: "Renews on Tuesday 1 December; to stop it, the notice must leave by 2 October", "…the notice for this renewal can no longer leave in time", "No renewal date; it can be stopped at any time". A renewal moves on by its term (each month, each year) from the date written.
- **The notice proposed** is the one usually asked for its kind, said with its rule in the form (France): three months for a tenant (one in a furnished flat or a tight area), one month for insurances and health covers after their first year (loi Hamon; health covers since December 2020), at most ten days for phone and internet after twelve months, none for energy (art. L224-13 of the consumer code). Yours is what your contract says.
- **The reminder** ([reminders.md](reminders.md)): for contracts renewing each year, once, two weeks before the last day a notice can leave. Monthly ones are not reminded: they can be stopped any month.
- **Stopping one**: "Stop it…" opens its own cancel page (online, a three-click cancel button is required in France since 1 June 2023, loi 2022-1158), or writes the letter: a draft, formal, to its address when it is a mail address, with your number with them, asking for a written confirmation and its date; you read it, sign it and send it. "It has ended" keeps it, faded, as a record.
- **Never**: offers, comparisons, "better deals", switching suggestions. Facts about your contracts only.

## Invoices on one device
Invoice numbers must never repeat. With sharing on, one device numbers them (`sioul_sync::lease`, "staying put"): the first that made invoices keeps them; another says "Invoices are numbered on <computer>" and offers "Make invoices on this device", which takes them over once the others had time to know (a minute and a half). A device whose sharing folder cannot be written, or whose other devices went silent for a few minutes, waits rather than risk a number twice.

## Storage
- **The budget file**, `sioul-budgets.toml`, sits at the root of the notes folder: readable, written by hand or by Sioul, versioned with git. It holds the budgets, presets, lines, reserves, covers and mail rules, and the bank accounts, their rules (`[[split]]`) and your choices for single movements (`[[assign]]`).
- **The bank's movements**, `sioul-bank.toml`, beside it: every account's, by the bank's own id, and the newest balance of each.
- **Lines accepted from mail** go into it, at the end, each with a comment saying which message it came from and when it was added, and the message's `mid:` in its `links`, so it is never proposed twice.
- **Messages you said are not payments** are remembered in `~/.local/state/sioul/money.toml`.

## Later
- **Bank statements as PDF** (their text layout checked against the statements' own totals and the continuity of balances); the bank's own exports are read already (above). Their debits with no preset will propose contracts.
- **Currency conversion** at the day's rate (ECB).
- **Reports from the same lines, France first**:
  - income by tax line (non-professional BNC on 5KU, micro-BNC on 5HQ), with the allowances;
  - the micro-entrepreneur's receipts register, which the law requires, and the Urssaf declarations as tasks;
  - resources over rolling windows, for means-tested aid;
  - invoices with their legal mentions, and a fair price from target income, expenses and contributions.
- **Simulations**: how many days of work a budget takes.

## Privacy
Everything is local. An AI sees money only for a project you opened to it ([ai.md](ai.md)).
