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
- **A line of the file**: right click, "Change this line…" or "Delete this line", asked once. Lines are named by their place in the file (`sioul:budget/<budget>/<place>`), which another device or an edit by hand can change meanwhile: the line changed or taken out is the one the page showed, found again by its budget, date, amount and words (`budget::SeenLine`, at its place, else the one line just like it); when it is not found as it was, or two lines just like it are, nothing is changed, and the page says so.

## Where lines come from
- **Mail.**
  - **What a message says about money is read first** (`crates/sioul-core/src/payments.rs`), in French and English: a payment received, a payment made, a merchant's order, a bill, a refund. Newsletters, activity reports, mandates and shipping notices are not payments; mail set aside (forged, spam, a borrowed brand name, a blocked sender) is never read.
  - **The amount** is the one the message states: the only amount of the subject ("SHOP: 23,98 € EUR"), else the one after the sentence that states the payment ("Vous avez payé 387,00 € à…"), else the total's line, else the last amount above zero, the way receipts end with what was paid. A discount is negative; a share capital in the legal footer is never read.
  - **Who was paid, or who paid**, comes from the same sentence, else from a receipt's label ("Paiement à", then the name), else it is the sender.
  - **Rules** on the sender's address or domain, and on words, as for a project's routes ([notes-folder.md](notes-folder.md#routes)): each condition listed must hold. A rule gives the budget, the direction, and the preset the message stands for (`preset`).
  - **One payment told twice counts once**: a merchant's order confirmation and the processor's payment for it (same amount, three days apart at most), a bill and the message saying it was settled (same sender), one message received at two addresses.
  - **The same payment as a movement already counted** (a bill and its debit, a receipt and its card payment): the message joins it as a source without a question, said "the same payment as …", or "probably" so when the matching is not sure ("Matching", below).
  - **Lines are proposed** otherwise: in the window's budgets page, "Add" writes the line into the budget file, with the message in its `links`, and a comment saying where it came from; "Not a payment" leaves the message out for good. From someone new and not verified, a line says to check it first, and is never joined. Any message, whatever Sioul read in it, can become a line with **A payment…** ("A payment from any mail", below).
  - **Its references** (an invoice's number, an order's, a mandate's: `[payments] references`) and **its attached invoice's own data**, when a PDF carries Factur-X or an XML is UBL or CII (`payments::invoice_of`, read once per message), are kept for the matching's keys; the invoice gives its amount when the message gives none, its seller when only the sender names who is paid, and its day due.
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
- **The tolerance** is 5 % of what the period moves (each movement in or out, done or planned, and what it still needs beyond what is scheduled), and never less than 10: the same for `sioul budgets` and the window's verdict at its pace (`budget::tolerance`). Wide enough that a budget whose money comes later in the month is not short in its first days.

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
- **Its exports**: "Take in an export…" on its card: OFX, CAMT.053, your bank's CSV, PayPal's activity download (net of fees, the name and the type of each row, its balance; pending, refused and memo rows left out, and rows in another currency than most of the file's) and Stripe's balance history (net, description, type). The number an export gives is kept with the account (`exports`), so that what was read under it before is the account's too. Every name its exports gave it (its number, its IBAN, none: an export without one is kept under `account`) is the same account: its balance is the newest of theirs, never their sum, and a movement read under two of them counts once, in the budgets, the top-ups and the watch's total (`accounts::balance_of`, `movements_of`, `Bank::balance_in`; until 10 October 2026 an export first read without its number, later with it, counted the same money twice: `accounts::tests::one_account_under_two_names_counts_its_money_once`). Two real accounts are two cards: two of a card's names whose balances on the same day differ cannot be one account, and the card says so calmly ("they seem to come from two accounts, and only the newest balance is shown"), for you to make a card for the second (`accounts::two_accounts_in_one`, `one_card_for_two_real_accounts_is_noticed`).
- **Where each movement goes**, in this order:
  1. *your hand*: one movement placed in a budget, or in none (`[[assign]]`, from its row on the card; `parts` share one between budgets in the file);
  2. *the same payment as a line the budgets hold already* (from mail, by hand: the same amount, the line up to a week before the movement and three days after): counted there, once. The budgets, What is coming and the Porch tell the same payment by the matching instead ("Matching", below), which places such a movement where its line or its occurrence says; the bank card's reasons and the top-ups keep these steps as written;
  3. *money moved between your own accounts*: the bank's "PAYPAL" or "STRIPE", PayPal's withdrawals and deposits, Stripe's payouts, when the other account's exports cover the day: in no budget, since the other side is read;
  4. *the first rule that holds* (`[[split]]`: words of the label, case and accents aside; money in, out or either; to a budget, to the recurring payment it stands for, to a reserve, or to another account). A rule without words takes every movement its way: "everything paid out of PayPal goes to duties";
  5. *the recurring payment it stands for* (a preset of a budget the account fills, by a word of its name or its amount, in the days around its date): the real amount replaces the planned one;
  6. *the account's first budget*.
- **Daily spending is spent from its estimate**: a card payment that stands for nothing is spent from its budget's estimate (food, clothes), the one with the most left in its period first, never added on top of it; past the estimate, it counts more.
- **Counted, not copied**: the budgets count each movement once, as lines on their day, in memory (`movements::counted`, "Movements" below); the budget file is not written. A budget's page lists them without "Delete": they are changed where they were placed.
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

## Movements
A **movement** is one sum of money in or out, once, with everything that tells of it. The rent is planned by its preset and its contract, announced by the landlord's notice, then done when the bank's line comes: one movement with four sources, never four lines. None of thirteen budgeting apps ties the mail or letter that announced a payment to its bank line, which leaves the person to hold the two together ([research/budgets.md](research/budgets.md) §3, §6 recommendation 4). Movements are made again each time from your files and Sioul's own store beside them, never kept whole: a file changed by hand or on another device is read as it is, and what you wrote stays the record. Code: `crates/sioul-core/src/movements.rs`.
- **What one holds**:
  - an id, the same on every device;
  - its bank account when known: the bank's line's, else the account that fills its budget first;
  - its way: in, out, or between your own accounts;
  - its amount, signed: exact, "about" (within a tenth), or between two amounts, the middle said. A forecast shown with its doubt is used better and trusted more than a single figure (Joslyn & LeClerc 2012, [research/budgets.md](research/budgets.md) §2.6);
  - its currency, and its day: the bank's, else the day announced or planned, which stays beside it as the day expected;
  - its budget, its parts across budgets, or none;
  - what it is for: its budgets' areas; in no budget, its account's, the areas of the budgets the account fills;
  - its sources and documents, its other links (tasks, events, projects, notes), what it is probably together with, and its state.
- **Its sources**, each with a key that corrections name it by:
  - a line of the budget file, `line:<id>`; one written by hand without an id, `line:~<a digest of what it holds>`;
  - a recurring payment's occurrence, `preset:<preset>/<day>`, and the contract its preset pays, `sioul:contract/<id>`;
  - the bank's line, `bank:<account>/<the bank's id>`, and a card hold, `hold:<account>/<id>`;
  - an invoice you issued, `sioul:invoice/<number>`; a paper letter, `sioul:letter/<id>`; a message, `mid:<Message-ID>`.

  A movement's id is the key of its first source in that order, so the same files give the same id on every device, for one reading. What is kept names it by its steadiest key instead (`Movement::steady_key`): its bank line's; else a line's with an id; else its occurrence; else its letter, its invoice, its mail; else a card hold's; a digest of a line written by hand only when nothing else names it, since that digest changes as soon as the line is edited. A contract names every occurrence of its preset, so never one movement. Its address, `sioul:movement/<steadiest key>`, finds it, and so does an address naming any of its other sources: a link made before the bank's line came still leads to it.
- **Five states**, in words, never "late", "overdue" or a count, since people look away from their accounts when they go badly (Karlsson et al. 2009; Olafsson & Pagel 2017, [research/budgets.md](research/budgets.md) §2.2) and red alone makes losses seem likely to go on (Bazley et al. 2021, §2.5):
  - *planned*: from a contract or a preset, or a line written as planned;
  - *announced*: a mail, a letter, an invoice or a debit notice gave its amount and day; a receipt that arrives before its bank line waits as announced, with nothing to file (§6 recommendation 12);
  - *not final yet*: a card hold, which the shop may still change;
  - *done*: the bank's line, or a line you wrote as done with nothing else to tell of it (cash);
  - *not seen*: its day passed inside what its account's exports cover, from three days before to seven after, as the watch has always looked, with no line for it. Said once: repeated contact about money harms (Murray & Bond 2023, §2.4).

  A planned movement whose day passed while no export of its account can tell stays planned: the budgets count it as done, as they always did, and nothing claims it happened.
- **How they are made**, in this order:
  1. each fixed preset's occurrences, with the contract that names the preset; an estimate (food, cash) is an envelope, not a movement;
  2. the lines of the budget file; a line standing for a preset joins its occurrence, with the real amount;
  3. the bank's lines and holds of the declared accounts, each placed as "Bank accounts" says (your hand, a transfer, a rule, the account's first budget) and a movement of its own;
  4. money moved between two of your accounts, read on both accounts' exports, as one movement with two sides, in no budget ("moved between your own accounts", §4.3); a line with `reserve` stays in its budget, since reserves cover budgets;
  5. letters with an amount and a date (a bill, a tax notice, a reminder, a formal notice), joined to the planned movement they announce (the same amount within a hundredth, ten days apart at most), else proposed in no budget;
  6. the store's links: what the matching or your hand joined before holds, whatever came since;
  7. the matching (below), which joins each line of the bank to what was planned or announced; lines of accounts not declared keep no books and only confirm, as the watch does;
  8. what is probably together, your placements, then what was not seen.
- **A card hold** stays not final until the bank's line for it comes (its id, or on its account within two weeks the same payee, by the first word of its name as the matching reads it, or nearly its amount: a tip changes it, a petrol station holds more than it takes), or for two weeks at most: posting takes one to five business days, rarely fourteen, and some holds simply disappear (Plaid, [research/budgets.md](research/budgets.md) §5.3).
- **The store**, `sioul-movements.toml`, beside the budget file in the notes folder, says what your files cannot:
  - `[[link]]`: a source joined to a movement, by the matching or by your hand, one per source, with what a message said (its amount, its day, who), what it is probably together with, and a purchase's instalments;
  - `[[apart]]`: "Not the same", two sources that are never one movement;
  - `[[move]]`: a movement placed by your hand where the files cannot say it, an occurrence moved or a line in parts;
  - `[[hold]]`: the card holds an account's last export listed;
  - `[[name]]`: a payee's name as the bank writes it, taught by a correction;
  - `[[decided]]`: your answer, once, on whether a payee's next movements go to a budget too.

  Its first line says its format (`format = 1`); a store written by a later Sioul is read for what this one knows of it, and never written over. It is written under the file's lock, which the sharing takes too, and keeps what this Sioul does not know. A bank line's own placement stays in the budget file's `[[assign]]`, and a payee's next movements in a `[[split]]` rule, which every version reads. Between your devices it travels with projects and money ([database.md](database.md), "Parts").
- **Counted once**: the budgets count the movements, not the files' lines and the bank's lines apart (`movements::counted`, through `Ledger::load_with_bank`, which the budgets page, `sioul budgets`, the agent's `budgets`, reminders and dates read). The file's lines and presets count as they always did; a movement a declared account's line tells counts by the line of the file it joins (done now, what the bank took beyond it on a line beside it, a tip), else by its own line, in its budget or its parts, standing for the preset whose occurrence it is or spent from an estimate. Your placements count where you placed them. Lines added in memory carry their bank line's key, so a budget's page lists them without "Delete", and the file's own lines keep their places, so their addresses hold.
- **Older files read as before**: the budget file keeps its format. Its new fields are optional, and older versions pass over them: a line's `id` (written since the sharing's format 2), a preset's `about` and its range, `low` and `high`, in euros, their sign taken from `amount`. Nothing reads the budget files with `deny_unknown_fields`, so a field a later Sioul adds is passed over too.
- **What is due soon**, for the Porch's Money section: `sioul_core::due::coming(today, days)` lists each movement planned or announced from today to `days` ahead, each card hold, and what was not seen in the past month, in day order, one plain row each (`due::Coming`: its day, who, the signed amount in cents, its currency, whether it is "about", its budget's title, what it is for, its state, its address). What it is for lets the Porch follow the hours: work money is not said outside work's hours, which a budget's title cannot tell. Done movements and transfers are not listed, since nothing is due of them. The facts reach the person unasked, once, in words, since attention to money fades when it matters most (§2.2, §6 recommendation 7). `due::coming_in` does the same for a notes folder given.

## Matching
Two things that tell of one payment are found by keys first, then by a score, then paired one to one, as record linkage does (Fellegi & Sunter 1969; Christen 2012, [research/budgets.md](research/budgets.md) §5.1). Nothing asks you to validate a pair: an automatic link counts until you correct it, and a correction teaches the payee's name (§6 recommendation 5). Code: `crates/sioul-core/src/matching.rs`, `crates/sioul-core/src/einvoice.rs`.
- **Keys**, each of which makes a pair sure:
  - the bank's own id within its exports: an export read twice adds nothing, and a hold posted under its id is gone;
  - the SEPA mandate's reference (RUM) and the creditor's identifier (ICS), which every debit notice and the bank's line carry (Banque de France, §5.2). Learnt from one month's debit joined to its preset, they hold the next month's line even when its amount changed (the winter's energy bill), within half of it;
  - an invoice's, an order's or a notice's number found in the bank's label, as Odoo's reconciliation finds an invoice's reference (§5.1): the number after its word in a message or a letter ("facture", "commande", "avis", "invoice", "order": `[payments] references`), each number of five characters or more in the bank's label, never a day's digits;
  - what a Factur-X PDF or a UBL invoice says of itself (its number, its payment reference, its mandate, its creditor, its amount and its day due), since every French business receives electronic invoices from September 2026 and small ones issue them from September 2027 (impots.gouv.fr, §5.2).
- **Candidates** only, by blocking (Christen 2012, §5.1): the same way; an account that may pay it (one that fills its budget); an amount within its kind's tolerance; a day within its kind's window:
  - a debit: its amount exactly, from two days before its day to the third business day after the first one on or after it, since debits move off weekends and TARGET2's holidays (the ECB's calendar: New Year's Day, Good Friday, Easter Monday, Labour Day, Christmas and the day after);
  - a bill a letter or a message gives a day for: its amount exactly, from a month before that day to a week after; a bill or a debit notice by mail alone: from its day to six weeks after;
  - a card payment told by a receipt or an order: 5 % apart, a euro at least, for a tip, a hold or another currency, from the day before to two weeks after (§5.3);
  - a transfer, planned or written by hand: its amount exactly, from three days before to a week after;
  - an invoice you issued: its amount exactly, from a month before its day due to two months after, the weeks a client takes;
  - a refund: up to its amount, partial refunds included, never more, up to eight weeks after, the time a debit can be disputed (§5.2, §5.3);
  - another currency: a quarter either way, and only with the payee's name or a key, never a near amount alone, since the network's rate and a separate fee line change it (§5.3).
- **The score**: a key (8 for a mandate's or a number, 4 for a creditor's alone, who may hold two mandates); the amount (3 exact, up to 2 within its tolerance); the days apart, against the window (up to 2); the payee's names compared token by token, TF-IDF with Jaro–Winkler between tokens ("SoftTFIDF", Cohen et al. 2003, §5.1), up to 4, or half a point less when they share nothing, since a bank's label is cut and coded where the plan is in your words. A name's words are folded, three letters at least, without the bank's words that name nothing ("PRLV SEPA", "CB", "VIR", "RUM": `[bank] filler`), without digits (card numbers, dates), and without a processor's prefix ("PAYPAL *", "SUMUP *": `[bank] processors`), which card lines put first, the descriptor being cut at 22 characters (Stripe, §5.2). The shorter name is also held against the longer one's leading words, since a bank writes the payee first, then a card line's city or a debit's creditor's suffix; and each distinct name counts once in the TF-IDF, so a preset said by each of its occurrences is no common word. The names compared are a preset's label, its contract's party and title, a line's label, a letter's sender, a message's payee, and the names corrections taught.
- **Assignment**: one to one, the best score first, the nearest day on a tie. Sure from 7, probable from 4 ("probably the same as …", with **Not the same** beside it), else kept apart, Fellegi and Sunter's three outcomes. A pair barely better than the next one for either of its sides, within a point, is only probable: two subscriptions of €9.99 on one day, two coffees, which HomeBank would take for duplicates (§5.3). Two lines with distinct bank ids are never duplicates of each other.
- **Probably together**, for the known patterns alone (§5.3): two or three invoices one client's transfer pays, to the cent (one movement); an order a receipt told, charged in two or three parts within a hundredth (one movement, its lines together); a deferred-debit card's sum, which settles the card payments of the month before (`[bank] deferred`, `[bank] card`: in no budget, beside them, so nothing counts twice); a purchase's instalments a message tells ("en 3 fois": `[payments] instalments`), each in its own month, about its share.
- **Consent stays where it is**: a message or a letter that tells of a movement already counted joins it as a source, without a question (`movements::acts::join_mails`: its link keeps what it said); one that matches nothing is proposed as before ("Add", "Not a payment"); one from a sender not verified is never joined, and says to check it first.

## Taking in an export
**Take in an export…** reads the file (`bank::read`), then `movements::acts::take_in`: its booked lines go into `sioul-bank.toml` by their ids, so the same file twice adds nothing; the lines it says are pending (CAMT's `PDNG`, a CSV's status in `[bank] pending`) become the account's card holds in the store, in place of what its last export listed, never movements of `sioul-bank.toml`, which keeps them for good. Each new line is then resolved against what was planned and announced, and its pair kept as a link (`resolve`), so that it stays joined whatever the matching says later, until you correct it. CAMT's mandate and creditor are written into a line's label only beside the bank's own id, since a label makes the id of a line without one. Any budget may be chosen for a line, whatever the hour: what the page shows follows the hour, never what a movement may be given to (`view::budgets_at`; the person's report of 4 November 2026: a line taken in could not go to a budget hidden at that hour).

## Moving a movement
A movement moves to another budget, or splits between budgets, from its row (`movements::acts::move_to`), since budgets are the person's own mental accounts (Thaler 1985; Zhang et al. 2022, [research/budgets.md](research/budgets.md) §2.1). It is written where every version reads it when it can be, a bank line's `[[assign]]` (its budget, its parts, or none), else in the store's `[[move]]`, by its steadiest key. The page says the effect, the budget that holds more now ("Home holds €12 more for October 2026.", the year alone in a yearly budget), and asks once whether the payee's next movements go there too: your answer is the consent (§6 recommendation 6). Yes is a rule of the budget file (`[[split]]`: the payee's words, its way, that budget), which the bank's rules list shows; either answer is kept (`[[decided]]`), so it is never asked again. Purchases atypical of a category escape it (Heath & Soll 1996, §2.1), which is why the matching, not the person, files a movement where its plan or its rule says.

## A payment from any mail
**A payment…**, in the reader's menu, in Mail's row menu and on a letter's card, opens a short form filled from what the message or the letter says: who, the amount (signed), the day (an attached invoice's day due when it has one), any budget; the message or the letter becomes its source (`movements::acts::Payment`, `crates/sioul-app/src/payments.rs`, `qml/PaymentForm.qml`). From a sender not verified it says to check it first. Saved, it is a line of the budget file, planned when its day is to come, the message or letter in its links, with a comment saying where it came from, and the movement it makes is announced until its bank line comes (§6 recommendation 12).

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
- **The bank's movements**, `sioul-bank.toml`, beside it: every account's booked lines, by the bank's own id, and the newest balance of each.
- **Sioul's own record of movements**, `sioul-movements.toml`, beside them: what the files cannot say, the card holds and the links the matching keeps among them (above, "Movements").
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
