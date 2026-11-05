# Accounting: bank accounts, budgets and reserves

Money arrives and leaves by mail: invoices, receipts, payment notifications, payouts, tax notices. It is held in **bank accounts** (a current account, PayPal, Stripe), set aside in **reserves** (a Livret A, an assurance vie), and spent from **budgets** (what it is for). Sioul keeps the three apart, so you see at a glance:
- until each account's next income, whether it holds every payment, and what is left for everyday spending;
- each budget in words: what is paid, what is still to come, what is left, how its period should end;
- what is coming over the next weeks, and how long each saving lasts at your own pace.

## Budgets
- **As many as you separate**: personal and professional, or one per project. For example: your duties (rent, energy, phone, food: what keeps you housed and fed), your leisure, and your work.
- **What each is for** (docs/areas.md): work, your admin, leisure, any of them together; it says the hours a budget is in view. Unsaid: your admin. The older word `area = "personal"` still reads as leisure.
- **A period and a target.** The period is a month or a year. The target is the balance to reach by the end of each period: 0 is breaking even, a positive target is saving.
- **From one income to the next.** A monthly budget may start its period on its income's day (`starts_with`: the id of a monthly money-in preset, chosen in its form as "Its period starts: With Salary (day 28)"), since people budget by their pay period, and a calendar month hides the stretch that matters to anyone paid on the 28th (Zhang et al. 2022; Stephens 2003, [research/budgets.md](research/budgets.md) §2.1, §2.3). Its period then runs from that day to the day before the income's next one (`Ledger::bounds`); its estimates are spent over it, its fixed payments counted in it, its verdict judged at its end. Without it, or once that preset is gone, the calendar month, which older versions read. Yearly budgets keep the calendar year.
- **A share set aside for taxes**, for a work budget: "Work money", below.
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
For a budget's current period (its month or year, or from one income to the next):
- **so far**: the lines up to today, presets dated up to today included;
- **to come**: the presets and planned lines until the end of the period;
- **carried in**: the balance at the start of the period;
- **expected**: carried in, plus so far, plus to come, compared with the target;
- **set aside**: the part of a surplus that planned lines after the period will take (a bill, a tax due later). It is kept, not counted as better than planned.
  - **Better than planned**: above the target by more than the tolerance.
  - **As planned**: within it.
  - **Short of plan**: below it.
- **The tolerance** is 5 % of what the period moves (each movement in or out, done or planned, and what it still needs beyond what is scheduled), and never less than 10: the same everywhere (`budget::tolerance`). Wide enough that a budget whose money comes later in the month is not short in its first days.
- **Said as the period's end**, in one sentence with the reserve that covers it (`Ledger::period_end`, "Each budget in words" below): once the budget has three past periods with movements that were not planned, at their pace (the middle of what they did beyond the plan); before that, what has come in or left beyond the plan is weighed against the part of the period gone, and what is ahead or behind today is carried to the end, so that a budget whose money comes later in its period is not said short in its first days. No share of the period gone is said: money does not flow evenly through a month, spending rises when income arrives and bills cluster after it (Stephens 2003; Gelman et al. 2014, [research/budgets.md](research/budgets.md) §2.3), and a fixed bill reads as all spent on its first day (§4.2).

## Reserves
- **A reserve is a savings account** (an assurance vie, a Livret A), with its balance on a date, a floor it is never planned below, and its delay (`delay_days`): how long money asked from it takes to reach your account, 0 for a Livret A, about two weeks for an assurance vie.
- **Cover rules** say which reserve covers which budget. At the end of each period, a budget's shortfall is drawn from its reserve; with `sweep`, a surplus goes back into it. The next period opens with the balance after that transfer.
- **Transfers**: a line with `reserve = "<id>"` moves money between a reserve and a budget, a withdrawal you ordered for example. It stays planned until it happens.
- **What each reserve shows**:
  - its balance today: the balance on its date, minus the transfers since;
  - what was used this month, and what is still planned;
  - its balance at the end of the year;
  - how many months it lasts above its floor at the coming year's pace;
  - how many weeks of your own spending it covers above its floor (your payments out of the past thirteen weeks, moves between your accounts and savings aside: `moneyview::own_pace`), since a buffer is counted in weeks of one's own take-home spending (Farrell et al. 2019, [research/budgets.md](research/budgets.md) §2.7, recommendation 10).

## At a glance
The budgets page, from its top: each account shown, in a few sentences, from one income to the next; its one chart; what the hour leaves out; each budget in words; What is coming; the reserves; the bank accounts, the contracts, the mail about money (the sections below). No percentage, no colour, no bar: the share of the month gone beside the share spent made the person compare two percentages and assumed money flows evenly (research/budgets.md §4.2), and the same data in red cut risk-taking and made losses seem likely to go on (Bazley et al. 2021, §2.5). Every term explains itself in its sentence, since teaching does not stick and help given at the decision does (Fernandes et al. 2014, §2.5). On a phone the same sentences, What is coming as a list, within 412 points.

`sioul budgets` says the same in lines, every area at once; on [../examples/sioul-budgets.toml](../examples/sioul-budgets.toml), with no export read, on 2 October 2026:
```
Budgets, October 2026
  Home (monthly)
    Paid: Rent.
    Still to come in: Withdrawal from the savings account, €1,000, Tuesday 20 October.
    Daily spending (estimate): €500 left until 31 October.
    By 31 October, it should end as planned.
  Projects (monthly)
    By 31 October, it should end as planned.

What is coming
  Week of Monday 19 October
    Tue 20 Oct · Withdrawal from the savings account · +€1,000 · Home · expected on Tuesday 20 October
  Week of Monday 26 October
    Sun 1 Nov · Rent · €600 · Home · expected on Sunday 1 November

Reserves
  Savings account
    Today: €20,000
    This month: €0 used, €1,100 planned
    By 31 December: €16,700
    At this pace: about 16 months
```

## The budgets page
Code: `crates/sioul-core/src/ahead.rs` (each account's stretch), `crates/sioul-core/src/moneyview.rs` (the words), `view::budgets_page`, `qml/BudgetsPage.qml`, `qml/AheadChart.qml`, `qml/ComingSection.qml`, `qml/MovementActions.qml`. Everything is read from the movements ("Movements", below), the books made once and made again only when a file changed (`movements::books`).

## From one income to the next
The worry a budgets page answers is shaped by time: will I be all right until the next money comes in (research/budgets.md §4.1)? People budget by their pay period (Zhang et al. 2022, §2.1), spending follows income, and a calendar month hides the stretch for anyone paid on the 28th (§2.3), so each account's sentence ends at its next income (`ahead::of`, `ahead::sentences`; recommendation 1):
- **The balance now**: the bank's last balance, with the account's lines read after it.
- **What is expected**: every movement of the account still to come, planned, announced or not final yet; one whose day passed while the exports could not show it yet (a week before their last day at most) counts today. A movement in no budget yet (a letter's bill) is the first account's. Everyday estimates are no payments: what is left is for them.
- **The next income**: the first money in expected that is neither moved between your own accounts nor a refund, worth at least a fifth of the largest coming and €50: the pay, the benefits, a client's payment. A recurring one, or one a letter or a mail announced, is sure; a planned line or an invoice of yours waits on someone else, and is said with its doubt ("If Client A comes in by Friday 20 November, …"), since people give income almost three times the weight of expenses in their forecasts (Berman et al. 2016, §2.6; recommendation 8).
- **Until then**, the balance is carried with each payment (`ahead::carry`):
  - *it holds every payment* while it stays above what the account keeps (its floor): "Until Salary comes in on Saturday 28 November, the account holds every payment.";
  - *what is left for everyday spending* is the lowest point above the floor, less the past year's unusual spending for those days (below), and per day: "€460 is left for everyday spending, about €25 a day." The lowest point, not the end, since money that arrives later cannot be spent before it; daily spending is roughly even and bills are not (§4.1), so it is said per day ("about", rounded as people say it: to the euro under €20, to five under €100, to ten above);
  - *the lowest point*, its day and the payment that brings it there (GnuCash's Future Minimum, §3): "The lowest point comes on Thursday 26 November, after Rent.", its amount said only when it is not what is left;
  - *short*: the first payment it would not hold, how much is missing, and what tops it up: the reserves in the account's order (`topped_up_by`), each as far as it holds above its own floor, the missing sum rounded up to ten, asked its delay ahead ("Livret A can cover it: move €40 by Thursday 26 November."; "…: ask for €400 by Monday 9 November, as it takes 14 days to arrive."), or said when asking today is already too late;
  - *without a doubtful income*: where it would go short until the next sure one, said only when that changes the answer ("Without it, after Rent on Wednesday 25 November, €220 would be missing."): a second line appears only when it matters (§6 recommendation 8).
- **With no income expected** within two months: how long the account and its reserves last at your usual pace (your payments out of the past thirteen weeks; without any, those planned for the next four), in weeks: "At your usual pace, the account and Livret A last about 24 weeks." With irregular income the question is how long the money lasts, counted in weeks of one's own spending (Farrell et al. 2019; Hannagan & Morduch 2015, §2.7).
- **The one chart**, for the first account shown: its balance ahead as a step line from today to three days past its next income (three weeks without one); zero dashed, the floor plain, the incomes and the large payments (a tenth of today's balance, €100 at least) marked with their names, a doubtful income's absence as a lighter second line; never red (`ahead::chart`, `qml/AheadChart.qml`; §4.1). The sentences never need it.

## Each budget in words
Each budget says what remains, named, instead of percentages (`moneyview::budget_words`; recommendation 2; Heath & Soll 1996: budgets are tracked against their limits, §2.1):
- **Its fixed payments**: "Paid: Rent, Energy." and "Still to come: Phone, €19.99, Thursday 12 November.", from its movements in its current period: done ones by name; to come with their amount ("about" when it is one) and day; "Not on the account yet: …" for one whose day passed inside what its account's exports cover. With no export of its account, a payment whose day passed counts as paid, as the budgets have always counted it.
- **Its income**: "Came in: Salary." and "Still to come in: Client A, €720, Friday 20 November."
- **Each everyday estimate**: what is left of it until its period's end ("Food: €239.50 left until 27 November."), or how much more than planned ("Food: €30 more than planned.", research §4.3's word for overspent) (`Ledger::estimate_left`).
- **What is set aside** for the next declaration (below).
- **How its period should end**, one sentence with the reserve that covers a gap or takes a surplus: "By 30 November, it should end €40 better than planned, which goes to Long-term savings." ("On track", above). Its icon says the same, never a colour.

## What is coming
One list of every movement expected from today over the next six weeks, or until the income after next when that is further (75 days at most), in date order and grouped by week ("This week", "Week of Monday 23 November"); first what the past week did not show yet (`moneyview::coming`; recommendation 3; Berman et al. 2016: expenses are forgotten in forecasts, §2.6). Each row says, as Mail's rows do: its day ("Thu 12 Nov"), who, the amount or "about", its budget, its state in words ("expected on Thursday 12 November", "announced for…", "not final yet: the shop may take another amount", "expected on…, not on the account yet", "if it comes by…" for a doubtful income), and what tells of it together ("From a mail and the contract.": the mail's invoice, the letter's scan, the contract, the bank's line), each opened from its menu (recommendation 12). Money in is as plain as money out, its "+" alone telling it. After the rows, the past twelve months' unusual spending, which the forecast counts (below).
- **A row's menu** (a long press on a phone, a right click or its "⋯" on a computer; `qml/MovementActions.qml`): where it goes; split between two budgets; "Not the same payment" beside a source joined as probably the same; "The same as…", with the nearest of the same way to choose from; what tells of it, opened. Each says its effect, and asks once about the payee's next movements ("Moving a movement", below). The bank cards' rows offer the same.
- **A month grid on demand**: "As a month" lays the same movements on a month's calendar beside the list, never instead of it; on a phone, the list alone. No study compared a grid with a list; a list reads aloud, fits a phone and groups by week (§4.1).
- **Unusual spending**: the payments out of the past twelve months that stand for no recurring payment and are at least three times the account's usual payment (and €50), per month over the months its lines cover (three at least): "Unusual spending, about €60 a month over the past year, is counted in the forecast." What is left for everyday spending keeps that share for its days. People spend nearly twice what they expect on exceptional expenses, each of which looks unique (Sussman & Alter 2012; Howard et al. 2022, §2.6).

## A shortfall concerns every area
Budgets show by area with Mail's filter (`AreaFilter.qml` over `areafilter::hours_say`, research §6 recommendation 7; the person's own words of 4 November 2026: "the same rules as emails, … defaults to the current time of the day, … fast overriding"): work's in your work hours, admin's in your admin hours, leisure's in your free time, at meals and at night, so the page is never empty; tick others, and they hold until the hours change, a day at most; "As the hours say" brings the hours' back ([areas.md](areas.md), "A page's filter by area"). Each device keeps its own (`$XDG_STATE_HOME/sioul/budgets-view.toml`), never shared. An account's area is the areas of the budgets it fills; What is coming keeps the rows of the areas shown.
- **What the hour leaves out is said in words, never how many**: "Work budgets are kept for your work hours.", or "Work budgets are not ticked." when you unticked them. Any budget stays a choice for a movement, whatever the hour.
- **The facts about an account are never filtered**: an account that would not hold a payment before its next income shows whatever the filter, with its sentence and what covers it, since people look least when it matters most (Karlsson et al. 2009; Olafsson & Pagel 2017, §2.2) and alerts that come by default cut overdraft charges by a quarter (Caflisch et al. 2018, §2.4).
- **For the Porch**: `sioul_core::ahead::shortfall_notices(root: &Path, words: &Words, tr: &Translator, today: Date) -> Vec<notices::Notice>` gives one notice per account and day of its shortfall, in every area (its area unsaid), in the money section, opening the budgets page, the same on every device while the shortfall is the same (`notices::money_id("short/<account>/<day>")`): "After Rent on Thursday 26 November, the account would be €40 short. Livret A can cover it: move €40 by Thursday 26 November." The Porch says it at the next admin window, whatever the filter. `ahead::shortfalls` gives the same as data.

## Work money
A work budget may set aside a share of each income for taxes and contributions (`set_aside`, in percent, and `declare`, "month" or "quarter", the default; in its form: "Set aside for taxes, in %", "Declared each month / each quarter"). Off until set (recommendation 10).
- **Each income makes it planned**: the budget's money in of each declaration period (done or still expected), counted at what the client paid (its gross, finding 62) on the day it was paid at its processor, its share set aside, less what a platform already withheld as contributions for it, as one planned movement on the declaration's day: the last day of the next month, or of the month after its quarter, as URSSAF's calendar for the self-employed has it (`movements::declaration_day`). Money earmarked in its own place is kept: savings sealed in two envelopes beat one (Soman & Cheema 2011, §2.1), and freelancers find taxes "a nightmare" (Hsieh et al. 2025, §2.7). One movement per declaration, since the declaration is one payment: "Set aside for taxes and contributions: about €330 for the declaration of Sunday 31 January 2027."
- **Counted in the budget** as planned until its day, said in What is coming and in the account's stretch; then the payment itself counts, as the bank's line or your line tells it. It is said "about": the declaration's own sum may differ.
- **Paid before its day**: "The same as…" on its row, the bank's line that paid it chosen (from its quarter's days), makes them one movement, done on the bank's day with its amount, counted once as that line and said paid ("Paid: Taxes and contributions, 22 %."). The share is never matched alone: a declaration's sum is not its share's.
- **Reserves are said in weeks** of your own spending ("Reserves", above).

## Words
The page's words are research §4.3's (Alenazi & Sas 2023: the apps' concepts of accounts and transactions overlap and confuse), in English and French, never debit, credit, cleared or pending:

| Instead of | English | French |
|---|---|---|
| credit, debit | money in, money out | entrées, sorties |
| scheduled, forecast | expected on Friday 5 December | prévu le vendredi 5 décembre |
| pending, hold | not final yet: the shop may take another amount | pas encore définitif |
| cleared, posted | has left the account; arrived | passé sur le compte ; arrivé |
| available to spend | left for everyday spending | reste à vivre |
| projected minimum | the lowest point | le point le plus bas |
| transfer | between your accounts | entre vos comptes |
| matched, reconciled | the same payment | le même paiement |
| sinking fund, earmark | set aside for… | mis de côté pour… |
| overspent | €30 more than planned | 30 € de plus que prévu |

## Bank accounts
Where money is and what it is for are two things. A **bank account** (`[[bank_account]]`: a current account, PayPal, Stripe) holds the money; the **budgets** say what it is for. Each account takes its own exports, and its movements fill the budgets it is said to fill (`fills`, the first taking what nothing else places), so one account can fill several budgets and one budget be filled by several accounts. Code: `crates/sioul-core/src/accounts.rs`, `crates/sioul-app/src/bank.rs`, `qml/BankSection.qml`, `qml/BankAccountDialog.qml`, `qml/BankRulesDialog.qml`, `qml/ReserveDialog.qml`.
- **Its exports**: "Take in an export…" on its card: OFX, CAMT.053, your bank's CSV, PayPal's activity download (the net of each row, the name and the type of each row, its balance; pending, refused and memo rows left out, and rows in another currency than most of the file's), Stripe's balance history (net, description, type), Revolut's and Wise's statements. What the payer paid and the fee kept, when an export says them, are kept with each row ("Fees", below). An account's **Kind** is a bank, or a processor or a platform Sioul knows (`presets/processors.json`: PayPal, Stripe, SumUp, Malt, Wise, Revolut, Lydia, Vinted, Etsy), which says how its payouts read on the bank's line. The number an export gives is kept with the account (`exports`), so that what was read under it before is the account's too. Every name its exports gave it (its number, its IBAN, none: an export without one is kept under `account`) is the same account: its balance is the newest of theirs, never their sum, and a movement read under two of them counts once, in the budgets, the account's stretch and the exports' own total (`accounts::balance_of`, `movements_of`, `Bank::balance_in`; until 10 October 2026 an export first read without its number, later with it, counted the same money twice: `accounts::tests::one_account_under_two_names_counts_its_money_once`). Two real accounts are two cards: two of a card's names whose balances on the same day differ cannot be one account, and the card says so calmly ("they seem to come from two accounts, and only the newest balance is shown"), for you to make a card for the second (`accounts::two_accounts_in_one`, `one_card_for_two_real_accounts_is_noticed`).
- **Where each movement goes**, in this order:
  1. *your hand*: one movement placed in a budget, or in none (`[[assign]]`, from its row on the card; `parts` share one between budgets in the file);
  2. *the same payment as a line the budgets hold already* (from mail, by hand: the same amount, the line up to a week before the movement and three days after): counted there, once. The budgets, What is coming, the bank cards and the Porch tell the same payment by the matching instead ("Matching", below), which places such a movement where its line or its occurrence says; these steps stay for what the matching leaves;
  3. *money moved between your own accounts*: the bank's "PAYPAL" or "STRIPE", PayPal's withdrawals and deposits, Stripe's payouts, when the other account's exports cover the day: in no budget, since the other side is read;
  4. *the first rule that holds* (`[[split]]`: words of the label, case and accents aside; money in, out or either; to a budget, to the recurring payment it stands for, to a reserve, or to another account). A rule without words takes every movement its way: "everything paid out of PayPal goes to duties";
  5. *the recurring payment it stands for* (a preset of a budget the account fills, by a word of its name or its amount, in the days around its date): the real amount replaces the planned one;
  6. *the account's first budget*.
- **Daily spending is spent from its estimate**: a card payment that stands for nothing is spent from its budget's estimate (food, clothes), the one with the most left in its period first, never added on top of it; past the estimate, it counts more.
- **Counted, not copied**: the budgets count each movement once, as lines on their day, in memory (`movements::counted`, "Movements" below); the budget file is not written. A budget's page lists them without "Delete": they are changed where they were placed.
- **Its rows**: its own lines as the matching left them, newest first, each saying where it went and why ("the same payment as what was planned or told" when it joined a plan, a letter or a mail), and what else tells of it together (the mail's invoice, the letter, the contract), with the same menu as What is coming's rows: where it goes, split, "Not the same payment", "The same as…".
- **Topped up in your order**, before its next income ("From one income to the next", above): on the first payment it would not hold above its floor (`floor`), what is missing, rounded up to ten, comes from its reserves in `topped_up_by` order, each as far as it holds above its own floor, asked its delay ahead: "Livret A: €300 to move before Sunday 25 October, when the insurance leaves; it arrives at once." "Assurance vie: ask for €400 by Thursday 15 October; it takes about ten days…" When asking today is already too late, it says so.
- **Before any is declared**, the exports' own lines and balance stand for one account ("Your account"): they confirm what was planned, and keep no books.

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
Arrears that build up in silence are how it goes wrong: a debit stops, nothing says so, and nobody looks when accounts go badly (Olafsson & Pagel 2017); a payment reminder that ignores the balance can push an account into overdraft (Medina 2021). So Sioul reads your bank's own movements and holds them against your budgets' recurring payments, as money movements ("Movements", below). Code: `crates/sioul-core/src/bank.rs`, `crates/sioul-app/src/bank.rs`, `qml/BankSection.qml`.
- **Taken in**: Budgets ▸ The bank ▸ "Take in an export…": the file your bank gives (OFX or QFX, ISO 20022 camt.053, or its CSV: the header found by its words, `;` or `,`, decimal commas, debit and credit apart or not, the balance written above the table as some banks do). Movements are kept by the bank's own id (or one made from the day, the amount and the label), so an export read twice adds nothing; the newest balance is kept. `sioul-bank.toml` at the root of the notes folder; nothing is sent anywhere, except sealed to your other devices when they share projects and budgets (docs/database.md).
- **What it says**, in sentences, never in red, from the movements of the past month (`bank.rs`'s `findings` in the window):
  - *not seen*: a payment expected around a day, inside what its account's exports cover, with no line for it: "Free Mobile (€19.99) has not left the account. Expected around 12 September…";
  - *changed*: a recurring payment of an exact amount that passed with another one (more than 5 % and €2 apart): "EDF took €58.30 on 5 September, where €52 was expected";
  - *short*: an account that would not hold a payment before its next income, with what covers it ("From one income to the next", above).
- **The week**: the payments of the next seven days, for the Porch, in one line at its top while it is open: "This week: EDF €52 (Mon). The account holds them.", and, when the movements found something, "Something about money to look at", never a count.
- **The balance ahead** is the budgets page's one chart, and what is coming its list (above).
- **Reminders** ([reminders.md](reminders.md)), from the movements (research §6, recommendation 9): a payment you make yourself (a planned line, a bill a letter gave, a recurring payment marked "You pay it yourself") or an invoice you send ("You send its invoice"), a few working days before, naming it and saying whether the account holds it, since a reminder that names its bill works and one blind to the balance moves the cost to an overdraft (Karlan et al. 2016; Cadena & Schoar 2011; Medina 2021, §2.4); a payment not seen, once, a week after its day; and the day an account would not hold a payment, a direct debit too, five working days before. A direct debit is not reminded otherwise: repeated contact about money harms (Murray & Bond 2023, §2.4).
- **Not built**: fetching from the bank itself (Woob has modules for many banks; Sioul installs nothing): exports are taken by hand for now. Without bank accounts declared, bank lines do not become budget lines: they confirm what was planned, and keep no books.

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
  - what was paid (`gross`: money in, what the payer paid; money out, the price asked), its fees (each with its kind and where it was read) and what arrived (`sum`, the net, which the budgets count): `gross − fees = net`; a movement whose figures do not add up keeps the bank's net and says its fees are not known yet (`fees_unknown`), never one made up ("Fees", below);
  - the day it was paid at its processor (`paid_on`), which the declaration counts, when not its day;
  - your note, the day Sioul was first told of it (`made`: a mail's day, the first export that covered its bank line, its line's own) and the day it was last corrected (`changed`);
  - a payout's payments (`carried`), or the payout that carried a payment (`carried_by`);
  - its sources and documents, its other links (tasks, events, projects, notes, the payer's card), what it is probably together with, and its state.
- **Its sources**, each with a key that corrections name it by:
  - a line of the budget file, `line:<id>`; one written by hand without an id, `line:~<a digest of what it holds>`;
  - a recurring payment's occurrence, `preset:<preset>/<day>`, and the contract its preset pays, `sioul:contract/<id>`;
  - the bank's line, `bank:<account>/<the bank's id>`, and a card hold, `hold:<account>/<id>`;
  - an invoice you issued, `sioul:invoice/<number>`; a paper letter, `sioul:letter/<id>`; a message, `mid:<Message-ID>`; a fee invoice (Malt's commission), `feeinvoice:<its message's key>`, which opens its message.

  A movement's id is the key of its first source in that order, so the same files give the same id on every device, for one reading. What is kept names it by its steadiest key instead (`Movement::steady_key`): its bank line's; else a line's with an id; else its occurrence; else its letter, its invoice, its mail; else a card hold's; a digest of a line written by hand only when nothing else names it, since that digest changes as soon as the line is edited. A contract names every occurrence of its preset, so never one movement. Its address, `sioul:movement/<steadiest key>`, finds it, and so does an address naming any of its other sources: a link made before the bank's line came still leads to it.
- **Five states**, in words, never "late", "overdue" or a count, since people look away from their accounts when they go badly (Karlsson et al. 2009; Olafsson & Pagel 2017, [research/budgets.md](research/budgets.md) §2.2) and red alone makes losses seem likely to go on (Bazley et al. 2021, §2.5):
  - *planned*: from a contract or a preset, or a line written as planned;
  - *announced*: a mail, a letter, an invoice or a debit notice gave its amount and day; a receipt that arrives before its bank line waits as announced, with nothing to file (§6 recommendation 12);
  - *not final yet*: a card hold, which the shop may still change;
  - *done*: the bank's line, or a line you wrote as done with nothing else to tell of it (cash);
  - *not seen*: its day passed inside what its account's exports cover, from three days before to seven after, with no line for it. Said once: repeated contact about money harms (Murray & Bond 2023, §2.4).

  A planned movement whose day passed while no export of its account can tell stays planned: the budgets count it as done, as they always did, and nothing claims it happened.
- **How they are made**, in this order:
  1. each fixed preset's occurrences, with the contract that names the preset; an estimate (food, cash) is an envelope, not a movement;
  2. the lines of the budget file; a line standing for a preset joins its occurrence, with the real amount;
  3. the bank's lines and holds of the declared accounts, each placed as "Bank accounts" says (your hand, a transfer, a rule, the account's first budget) and a movement of its own;
  4. money moved between two of your accounts, read on both accounts' exports, as one movement with two sides, in no budget ("moved between your own accounts", §4.3); a line with `reserve` stays in its budget, since reserves cover budgets;
  5. letters with an amount and a date (a bill, a tax notice, a reminder, a formal notice), joined to the planned movement they announce (the same amount within a hundredth, ten days apart at most), else proposed in no budget;
  6. the store's links: what the matching or your hand joined before holds, whatever came since;
  7. the matching (below), which joins each line of the bank to what was planned or announced; lines of accounts not declared keep no books and only confirm what was planned;
  8. what is probably together, your placements, then what was not seen.
- **A card hold** stays not final until the bank's line for it comes (its id, or on its account within two weeks the same payee, by the first word of its name as the matching reads it, or nearly its amount: a tip changes it, a petrol station holds more than it takes), or for two weeks at most: posting takes one to five business days, rarely fourteen, and some holds simply disappear (Plaid, [research/budgets.md](research/budgets.md) §5.3).
- **The store**, `sioul-movements.toml`, beside the budget file in the notes folder, says what your files cannot:
  - `[[link]]`: a source joined to a movement, by the matching or by your hand, one per source, with what a message said (its amount, its day, who), what it is probably together with, and a purchase's instalments;
  - `[[apart]]`: "Not the same", two sources that are never one movement;
  - `[[move]]`: a movement placed by your hand where the files cannot say it, an occurrence moved or a line in parts;
  - `[[hold]]`: the card holds an account's last export listed;
  - `[[name]]`: a payee's name as the bank writes it, taught by a correction;
  - `[[decided]]`: your answer, once, on whether a payee's next movements go to a budget too;
  - `[[fee]]`: what a source tells of a payment, by the source (an export's row, a message, a fee invoice): what was paid, its currency when another, its fees and where each was read, the day paid, the processor; never in `sioul-bank.toml`, which every version writes whole;
  - `[[carried]]`: the payments a payout carried, by its bank line, with what each one's message said;
  - `[[edit]]`: your words on a movement the files cannot hold (a bank line's, an occurrence's): its note, the day paid, who;
  - `[[gone]]`: what you removed: an occurrence "not this time", a bank line "left out" of a wrong export, counted as nothing;
  - `[[import]]`: each export taken in, its account and the days it covers, from which a bank line's `made` is read.

  New entries carry `at`, the moment they were written, from which `changed` is read. The format stays 1: an older Sioul reads the store, ignores these tables and keeps them; both devices take a version that reads them together, since an older one shows nets without fees and counts what was removed. The sharing keys each table's entries by what they name, so two devices adding some keep both.

  Its first line says its format (`format = 1`); a store written by a later Sioul is read for what this one knows of it, and never written over. It is written under the file's lock, which the sharing takes too, and keeps what this Sioul does not know. A bank line's own placement stays in the budget file's `[[assign]]`, and a payee's next movements in a `[[split]]` rule, which every version reads. Between your devices it travels with projects and money ([database.md](database.md), "Parts").
- **Counted once**: the budgets count the movements, not the files' lines and the bank's lines apart (`movements::counted`, through `Ledger::load_with_bank`, which the budgets page, `sioul budgets`, the agent's `budgets`, reminders and dates read). The books (the files read, every movement derived over everything they tell, the budget file counted) are made once and made again only when one of the files changed, by its modification time and size, or the day, or the words (`movements::books`): five years of a busy account would otherwise be derived anew by each page, each reminder and each date. The movements are kept on the device too (`~/.cache/sioul/money-books.json`, by the same stamp, the words' digest and Sioul's version), so that a process of its own (`sioul budgets`, the window's first look) reads them rather than deriving them; a copy that does not match is made anew. On a stand-in of five years of a busy account (5,752 bank lines, 40 recurring payments, two accounts, letters, 200 links), deriving everything took about 240 ms, reading the kept copy about 50 ms, the budgets page about 40 ms, the reminders about 4 ms, and `sioul budgets` about 140 ms in all (a release build, 4 November 2026; `ahead::tests::timing`). The names' words and their similarities are made once per reading, since a recurring payment meets the same bank labels month after month. The file's lines and presets count as they always did; a movement a declared account's line tells counts by the line of the file it joins (done now, what the bank took beyond it on a line beside it, a tip), else by its own line, in its budget or its parts, standing for the preset whose occurrence it is or spent from an estimate. Your placements count where you placed them. Lines added in memory carry their bank line's key, so a budget's page lists them without "Delete", and the file's own lines keep their places, so their addresses hold.
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
**Take in an export…** reads the file (`bank::read`), then `movements::acts::take_in`: its booked lines go into `sioul-bank.toml` by their ids, so the same file twice adds nothing; what its new rows say of their fees goes into the store's `[[fee]]`, and the export itself into `[[import]]`; the lines it says are pending (CAMT's `PDNG`, a CSV's status in `[bank] pending`) become the account's card holds in the store, in place of what its last export listed, never movements of `sioul-bank.toml`, which keeps them for good. Each new line is then resolved against what was planned and announced, and its pair kept as a link (`resolve`), so that it stays joined whatever the matching says later, until you correct it. CAMT's mandate and creditor are written into a line's label only beside the bank's own id, since a label makes the id of a line without one. Any budget may be chosen for a line, whatever the hour: what the page shows follows the hour, never what a movement may be given to (`view::budgets_page`; the person's report of 4 November 2026: a line taken in could not go to a budget hidden at that hour).

## Moving a movement
A movement moves to another budget, or splits between two, from its row's menu, on What is coming or a bank card (`movements::acts::move_to`; "Not the same payment" and "The same as…" there too: `acts::not_same`, `acts::same`), since budgets are the person's own mental accounts (Thaler 1985; Zhang et al. 2022, [research/budgets.md](research/budgets.md) §2.1). It is written where every version reads it when it can be, a bank line's `[[assign]]` (its budget, its parts, or none), else in the store's `[[move]]`, by its steadiest key. The page says the effect, the budget that holds more now ("Home holds €12 more for October 2026.", the year alone in a yearly budget), and asks once whether the payee's next movements go there too: your answer is the consent (§6 recommendation 6). Yes is a rule of the budget file (`[[split]]`: the payee's words, its way, that budget), which the bank's rules list shows; either answer is kept (`[[decided]]`), so it is never asked again. Purchases atypical of a category escape it (Heath & Soll 1996, §2.1), which is why the matching, not the person, files a movement where its plan or its rule says.

## A payment from any mail
**A payment…**, in the reader's menu, in Mail's row menu and on a letter's card, opens a short form filled from what the message or the letter says: who, the amount (signed), the day (an attached invoice's day due when it has one), any budget; the message or the letter becomes its source (`movements::acts::Payment`, `crates/sioul-app/src/payments.rs`, `qml/PaymentForm.qml`). From a sender not verified it says to check it first. Saved, it is a line of the budget file, planned when its day is to come, the message or letter in its links, with a comment saying where it came from, and the movement it makes is announced until its bank line comes (§6 recommendation 12).

## Fees
A processor's mail states what the payer paid; the bank later receives less, often batched with other payments (research/money-ledger.md §3). A movement keeps the three amounts, and each fee comes from the most exact record at hand, by kind (`movements::acts::settle_fees`; finding 63):
1. **your hand**: a line's own `gross` and `fees`, or what you said of a bank line;
2. **the processor's export**: PayPal's Gross and Fee (French "Avant commission" and "Commission"), Stripe's Amount and Fee, Revolut's Fee (on top of its amount) and Wise's Total fees (inside it), as each file's running balance shows; CAMT.053's charges when the bank says they are inside the entry (`ChrgInclInd`), or its instructed amount when it differs;
3. **a fee invoice**: Malt's commission invoice, beside its service invoice (`Processor::fee_invoice`);
4. **the mail's own fee line**, for money in only ("Frais", "Fee", "Montant net", "You'll receive": `[payments] fee`, `net`), checked against what was paid;
5. **a payout's difference**, within the processor's bounds ("Payouts", below);
6. **for what is still expected only**, about: your own rate on that account (the middle of the past year's exact fees), else the processor's published rule (`presets/processors.json`, each with the day it was checked and its source: PayPal France's 2.90 % plus €0.35). A rule is never used for a declaration.

A refund keeps the original fee where it was (PayPal France, Stripe): its movement has no fee. Contributions a platform withholds for you (from 2027) are a deduction of their own kind, never a fee: a work budget that sets money aside counts them as paid. In words, a fee is said inside its payment's row, "paid €520 · fees €15.43", never as a row, a column or a total of its own (finding 65).

## Payouts
- **With the processor's export** (PayPal, Stripe, Wise, Revolut declared as bank accounts), a withdrawal or a payout is a move between two of your accounts, and it carried that account's money in since the previous one, as its running balance between them says; each payment says the payout that carried it (`carried_by`).
- **Without the export**, the bank's line named by a processor ("PAYPAL", "STRIPE", "SUMUP", "MALT") carries the payments its mail told of since its previous payout, oldest first, as far as its amount goes, its fees within the processor's bounds (`matching::carried_prefix`, `Bounds`: Sioul's own, wide on purpose); kept in the store's `[[carried]]`, said probable. The difference is the batch's fees: exact for one payment, each payment's share "about" for several, by the processor's rule scaled to the batch. A payment not carried yet waits; one recorded as a line says what may arrive, about. That processors pay out oldest first is an inference a trial on real payouts must check.
- **A platform that bills its commission apart** (Malt): its service invoice and its fee invoice, a few days apart, whose difference is to the cent a payout of its, join that payout: what the client paid, and the commission read from its invoice.
- **Not a payout** (removing a carried payout) keeps each payment apart from it for good (`[[apart]]`).

## Make, change, remove
The same acts in the window, for an agent (`add_movement`, `update_movement`, `delete_movement`, `same_movement`: [mcp.md](mcp.md)) and from the command line (`sioul movement add|change|remove|back`), each said in one line, its effect on a budget first ("Home holds €12 more for November 2026."), heard by every view (`changes`), with Undo in the window (`movements::acts::make`, `change`, `remove`, `put_back`):

| Made from | Make | Change | Remove |
|---|---|---|---|
| your hand (a line of the budget file) | **New movement**: who, what was paid or what arrived, fees, account, budget, day, day paid, planned or done, note, ties | every field, in the file | deleted, asked once |
| a recurring payment's occurrence | made by its preset | this time's amount and day (a line standing for it), budget, note, who | **Not this time** (`[[gone]]`) |
| a bank line | made by the export | budget, what was paid and its fees, the day paid, note, who; never its amount nor its day, the bank's facts | **Leave out** (`[[gone]]`), for a wrong export |
| a mail or a letter | **A payment…** | its budget and your words; its amount once recorded | **Not a payment** |
| an invoice you issued | made by the invoice | refused: an invoice is a numbered record | refused |
| a share set aside | made by the budget | refused: switched off in the budget's form | refused |
| a payout | made by the bank line | which payments it carried | **Not a payout** |

Your line joined to the bank's line is one payment: "Not the same" comes first, then the one that is wrong is removed.

## Movements tab
The full ledger (`moneylist::query`): every movement, newest first, grouped by day, 60 a page, the search on top (who, the note, the budget, the amount), the filters folded away and shown as words under it (account, budget, days, in or out, amount, area, what told of it: the bank, a mail, a plan, your hand). The area filter starts as the hours say; a typed search reaches every area, and each row names its area. A row gives its day, who, what moved with its sign, its budget and its state in words; a second line only when it has fees, carried payments, or a payout that carried it. No running total, no red, no count: a sum only when the filters ask a question that has one, one way over some days ("Money in: €820 paid, €804.57 arrived."). A movement opened (`moneylist::detail_words`) shows each fee and where it was read, the day paid, its sources, its ties, the payments a payout carried, and when Sioul was told of it and when it changed. People find things again by scrolling the recent past and by searching, not by filing (finding 64). `sioul movements` and the agents' `movements` read the same list.

## From mail
Every message about money kept on the device, Sent mail included, never what is set aside as spam or forged, nor what lies in the junk or the trash (`moneyindex::history`), in place of what the Porch still held:
- **Found once**: a reader of every message kept on the device (`mailpass`, which other readers share) opens a message only when what the mail cache says of it makes money possible (a processor's domain, a sender a mail rule names, a payment's word or an amount in its subject, an attachment named like an invoice: `[payments] attachments`), reads it as the budgets page does (`budget::mail_line_of`), and keeps what it found in `$XDG_CACHE_HOME/sioul/money-index/<account>/<folder>.tsv`, under the file's lock, once, ever. Its stamp covers the payment words and the mail rules: when they change, only this index is made again. It stays on each device; the ties it shows come from the shared store. On a stand-in of 20,034 messages over 24 folders, the mail cache and the money index made from nothing took 2.05 s on a computer (the index alone 1.40 s), and 0.22 s once kept (a release build, 5 November 2026).
- **Each row says what became of it**: the same payment as a movement, part of a payout, the fees of a movement, counted in a budget, not counted (you said it is no payment, or of mail of its shape), not tied to any movement. Tied or not is a filter, never the default, never counted.
- **"Not a payment" learns the shape** of that sender's mail, its subject with its digits and amounts taken out, kept beside the messages you said are not payments (`money.toml`, `shape:<sender>␟<shape>`, which an older Sioul never takes for a message): mail of that shape from that sender is no longer counted, said once on its row, until you count it again. Nothing is asked (finding 45).
- **The payer's card** is tied to the movement a mail joined, or to the payment of an invoice you issued, when the mail's address or the name it gives names exactly one card, with the person whose card it is (`movements::acts::tie_payers`: `links.toml`, how "payer"); a processor's mail names the payer it carries, never the processor. A tie you take apart is never made again (`[[apart]]` with the card).
- `sioul money-mail` and the agents' `money_mail` read the same history.

## The declaration
A work budget that declares (it sets money aside, or says how often it declares: `receipts::declares`) has a declaration view (`receipts::register`, `sentences`):
- **The turnover received** in its period (its quarter, or its month), at what the clients paid, by the day each was paid at its processor, a payout's payments each on its own day: "Turnover received from 1 October to 31 December: €6,940, paid by clients. €6,735 reached your accounts after €205 of fees. To declare by 31 January." A micro-entrepreneur declares the gross receipts (CGI art. 102 ter), received when the collector receives them (BOFiP, BOI-BNC-BASE-20-10-10 §80; finding 62). Overview says it beside what the budget sets aside ("Turnover received this quarter: €6,940, to declare by 31 January.", `receipts::period_line`).
- **The receipts register** (Service-public F23266): each receipt's day, client, invoice, means of payment and what was paid, oldest first, its total last; saved as CSV (`receipts::csv`: the language's decimal mark, `;` where it is a comma) and as a PDF, which the window prints from its page (`receipts::html`) as it prints invoices. `sioul receipts [--from D --to D] [--budget ID] [--csv | --html]`.
- **Nothing guessed**: a receipt whose gross is not known (a processor's payout that says nothing of what was paid, figures that do not add up) says "what the client paid is not known yet", and the total then reads "at least".

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
- **The budgets' filter** (your ticks while they hold) is this device's own, in `~/.local/state/sioul/budgets-view.toml`, never shared.
- **The money index**, what each message kept on the device says of money, in `~/.cache/sioul/money-index/` ("From mail", above): this device's alone, never shared, made again whenever the payment words or the mail rules change; deleting it loses nothing.
- **A copy of the money movements**, made from the files above, is kept in `~/.cache/sioul/money-books.json` (above, "Counted once"): this device's alone, never shared, made anew whenever a file changes; deleting it loses nothing.

## Later
- **Bank statements as PDF** (their text layout checked against the statements' own totals and the continuity of balances); the bank's own exports are read already (above). Their debits with no preset will propose contracts.
- **Currency conversion** at the day's rate (ECB).
- **Reports from the same lines, France first**:
  - income by tax line (non-professional BNC on 5KU, micro-BNC on 5HQ), with the allowances;
  - the Urssaf declarations as tasks (the receipts register is built: "The declaration");
  - resources over rolling windows, for means-tested aid;
  - invoices with their legal mentions, and a fair price from target income, expenses and contributions.
- **Simulations**: how many days of work a budget takes.

## Privacy
Everything is local. An AI sees money only for a project you opened to it ([ai.md](ai.md)).
