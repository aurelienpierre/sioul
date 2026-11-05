---
description: Budgets, reserves and bank accounts in Sioul - each account until its next income and each budget in words, never in red; payments read from mail, your bank's export files read on your device, what is coming week by week, and your contracts with their notice deadlines.
---

# Budgets and bank accounts

## In short {#in-short}

Money comes and goes by mail: invoices, receipts, payment notices, payouts, tax notices. Budgets tells you where your money stands, in sentences, and keeps three things apart:

- **bank accounts**, where the money is: a current account, PayPal, Stripe;
- **reserves**, where it is set aside: savings accounts;
- **budgets**, what it is for: your duties (rent, energy, food), your leisure, your work, a project.

Sioul reads your bank's export files and the mail about payments, holds them against the payments you expect, and opens with each account until its next income: whether it holds every payment, what is left for everyday spending, its lowest point. It says calmly when a payment did not leave, or when an account may not hold the next one, early enough to move money. It never connects to your bank, never shows red, and never shows offers.

<figure markdown="span">
  [![The Budgets page, "Budgets, November 2026", with its area filter and New budget: the main account in sentences (until the next income, whether it holds every payment, what is left for everyday spending and a day, the lowest point), its balance ahead as a step line, then each budget in words: what is paid, what is still to come, what is left of the food, how its month should end.](../assets/screens/budgets.png){ loading=lazy }](../assets/screens/budgets.png "Open the picture at full size")
  <figcaption>Each account until its next income, each budget in words.</figcaption>
</figure>

## Protected by default {#what-is-protected}

- **Sioul never logs in to your bank** and holds no bank password: you download your bank's file, and Sioul reads it on your device.
- **Your budgets, your bank's movements and your contracts are plain files** in your own notes folder. There is no server of Sioul's.
- **Between your devices**, they travel by your folder's own sync, or sealed through Sioul's sharing once you switch **Projects and money** on: the folder and its server can read nothing of them ([Sharing](sharing.md)).
- **Mail is read as a payment only once it passed Sioul's checks**: forged mail, spam, a sender borrowing a bank's or a shop's name, and blocked senders are never read as payments. A payment from someone new and not verified says to check it first: a fake "your payment" is a common trick.
- **An AI agent**, only if you connect one, reads a summary of your budgets, never moves money, and sees account numbers masked.
- **No offers**, no comparisons, no "better deals", no advertising, no analytics.

## Budgets {#budgets}

**New budget**:

- **Name**.
- **Counted by**: the month or the year.
- **Its period starts**: on the 1st, or with an income, your pay or a monthly benefit: from one pay to the next, as you live it.
- **Balance to reach** by the end of each period: 0 to break even; more is saving.
- **For**: work, your admin, leisure. It decides the hours the budget is in view ([Hours](hours.md)). None ticked: your admin.
- **Set aside for taxes**, for work: a share of each income, in percent, set aside as it arrives for the declaration, **each month** or **each quarter**. Off until you set it. Paid before the declaration's day, **The same as…** on its row joins it to the bank's line that paid it: counted once, said paid.

What a budget did not spend carries over, as an envelope keeps what is left: money set aside in October for a bill due in December is still there in December.

### At a glance {#at-a-glance}

The page opens with each account, in a few sentences, until its next income: "Until Salary comes in on Saturday 28 November, the account holds every payment. €460 is left for everyday spending, about €25 a day. The lowest point comes on Thursday 26 November, after Rent." A client's payment that may come later is said with its doubt ("If Client A comes in by Friday 20 November, …"), and so is what the account would do without it, when that changes the answer. With no income expected: "At your usual pace, the account and Livret A last about 24 weeks." One chart shows the balance ahead to the next income, zero dashed; the sentences never need it.

Each budget then says what remains, in words: "Paid: Rent, Energy." "Still to come: Phone, €19.99, Thursday 12 November." "Food: €180 left until 27 November." Then one sentence about the end of its period, with the savings that cover a gap: "By 30 November, it should end as planned." No percentage, no colour, no bar.

**By area**, as in Mail: the filter at the top shows what your hours say (work's budgets in your work hours, admin's in your admin hours, leisure's in your free time, at meals and at night), and you can tick others for a while; **As the hours say** brings them back. What it leaves out is said in a line: "Work budgets are kept for your work hours." An account that would not hold a payment always shows.

### What is coming {#what-is-coming}

Every payment and every income expected over the next six weeks, or until the income after next, week by week: its day, who, the amount or "about", its budget, where it stands ("expected on…", "announced for…", "not final yet", "if it comes by…"), and what tells of it, together: the mail's invoice, the letter, the contract, the bank's line. **As a month** lays the same on a calendar beside the list, on a computer. The past year's unusual spending (a repair, a trip) is counted in the forecast as its own line, about so much a month.

A long press on a phone, a right click or **⋯** on a computer: **Where it goes…**, **Split…** between two budgets, **Not the same payment** beside one Sioul thinks is probably the same, **The same as…**, and what tells of it, opened.

### A budget, open {#a-budget-open}

A budget opens on its words and the end of its period, then its movements, newest first, and its balance by **Days**, **Weeks**, **Months** or **Years**, the planned ones dotted.

**Add a movement**: **Once**, or **Recurring** (every month or every year, on a day). An amount is negative for money out (`-650`) and positive for money in (`1200`). For what is spent little by little, such as food, see [Daily spending](#daily-spending).

A right click on a line: **Change this line…**, **Delete this line** (asked once). If the line changed in the file meanwhile (on another device, or by hand), nothing is changed, and Sioul says so: look at it again.

A budget can be **For** someone of your household (a child's pocket money, an account you keep for a parent you help): chosen in its form, it shows on their page ([People in your household](contacts.md#household)).

**Take this budget out** keeps its lines in the file, as they were written; they no longer count.

## Reserves {#reserves}

**New reserve**: its **Name**, its **Balance** on a day, the amount it should **never** go **below**, and the **days** money asked from it takes **to arrive** (0 for a savings account you can move from at once, longer for others).

Each reserve shows its balance today, what moved this month, its balance at the end of the year, how long it lasts at this pace above its floor, and how many weeks of your own spending it covers.

Which reserve covers which budget is written in the budget file: at the end of each period, a budget's shortfall is taken from its reserve.

## Mail about money {#mail-about-money}

Below the budgets, the mail about money, newest first. Sioul reads, in English and French, what a message says: a payment received or made, an order, a bill, a refund; the amount; who paid, or was paid.

Each line says where it stands: *counted*; *the same payment as* another message (an order and its payment count once); *the same payment as* a payment already counted, a bill and its debit, a receipt and its card payment, which the message joins by itself, or *probably* so when Sioul is not sure; or waiting for you, with **Add** and **Not a payment**. A line from someone new and not verified says to check it is genuine first, and joins nothing by itself. Forged mail, spam and blocked senders are never read as payments. An amount in another currency is marked "to convert": Sioul does not convert it yet.

**A payment…**, in a message's menu, in Mail's list and on a paper letter's card, turns any message or letter into a payment, whatever Sioul read in it: a short form filled from what it says (who, the amount, the day), any budget. The message or the letter stays its source.

Rules in the budget file can send a sender's or a word's payments to a budget by themselves.

## Bank accounts {#bank-accounts}

<figure markdown="span">
  [![Bank accounts on the Budgets page: two cards, each with its balance from the bank's file, the budgets it fills, the reserves that top it up, its last movements, each saying where it went and why, with ⋯ beside it, and Take in an export…, Rules… and Edit; below, a payment that took another amount than expected.](../assets/screens/bank-accounts.png){ loading=lazy }](../assets/screens/bank-accounts.png "Open the picture at full size")
  <figcaption>Each account, the budgets it fills, and where each movement went.</figcaption>
</figure>

**New bank account**: its **Name**, its **Kind** (a bank, PayPal, Stripe, something else), the budgets it **Fills**, the amount it is **Kept above**, and the reserves it is **Topped up by**, in your order.

**Take in an export…**, on its card: the file your bank gives (OFX or QFX, CAMT.053, or its CSV), PayPal's activity download, or Stripe's balance history. Reading the same file twice adds nothing. Nothing is fetched from the bank itself: you download the file, Sioul reads it. A payment the file says is not final yet (a card hold) waits apart until the bank's line for it comes. Each movement then goes to a budget by your hand, your rules, or the payment it stands for, and says why ([Where each movement goes](#where-each-movement-goes)).

**Where it goes**, on a movement's row, offers every budget, whatever the hour. Moved, the movement says what it changes ("Home holds €12 more for October 2026.") and asks once whether that payee's next payments go there too; **Yes, from now on** writes a rule you find under **Rules…**. **⋯** beside it splits it between two budgets, says it is **Not the same payment** as the one Sioul joined it to, or **The same as…** another; each row says what else tells of it.

### Topped up in time {#topped-up-in-time}

Each account is carried to its next income. On the first payment it would not hold above its floor, Sioul says what to move, from which reserve, and by when, counting the days that reserve takes: "Savings: ask for €590 by Thursday 15 October; it takes about ten days, and the rent leaves on 1 November." When asking today is already too late, it says so.

## The bank watch {#the-bank-watch}

A debit that stops does not say so. Sioul holds your bank's movements against the payments you expect, and says, in sentences:

- **what has not left**: "Phone (€19.99) has not left the account. Expected around 12 September, not in your bank's movements.";
- **what changed**: "Electricity took €71.40 on 5 September, where €62 was expected.";
- **what may not be held**: an account that would not hold a payment before its next income, with what covers it ([Topped up in time](#topped-up-in-time)), said whatever the hour.

**This week**: the payments of the next seven days, in one line at the top of the Porch: "This week: Electricity €62 (Mon). The account holds them."

**Reminders**, each once, when work starts: two working days before a payment you make yourself (a planned bill, a tax, a letter's bill, a recurring payment ticked **You pay it yourself**) or an invoice you send (**You send its invoice**), saying whether the account will hold it; a week after a payment that did not leave; five working days before a day the account would not hold a payment, a direct debit too. A direct debit gets no reminder otherwise. They come from a computer only, while Sioul is open or, if you ask, with its window closed (Settings ▸ [Reminders](settings.md#reminders); not on Windows yet). A phone does not tell them.

## Contracts and subscriptions {#contracts-and-subscriptions}

What you are bound to: rent, energy, phone and internet, insurances, health cover, subscriptions, hosting, the bank. Forgotten subscriptions are part of what people with ADHD call the "ADHD tax"; knowing what an insurance covers matters before a legal step.

**Add a contract**: with whom, your number with them, the recurring payment that pays it, since when, when it renews (each month, each year, or no renewal), the notice it needs, how to stop it (its cancel page, or where to write), and what it covers.

- **Where it stands**, in a sentence: "Renews on Tuesday 1 December; to stop it, the notice must leave by 2 October."
- **The usual notice** for its kind is proposed, with the rule it comes from (in France). Yours is what your contract says.
- **A reminder**, once, two weeks before the last day a notice can leave, for contracts that renew each year. Like the money reminders, it comes from a computer only.
- **Stop it…**: **Open its cancel page**, or **Write the letter**: a formal draft to its address, with your number, asking for a written confirmation. You read it, sign it and send it. **It has ended** keeps it, faded, as a record.

Recurring payments with no contract yet are listed below, each with **Note it**. A message can become one with **Keep as a contract…**, in its menu.

Sioul never shows offers, comparisons or "better deals": only facts about your contracts.

## Where it is kept {#where-it-is-kept}

In plain files at the root of your notes folder: `sioul-budgets.toml` (budgets, reserves, lines, rules, bank accounts), `sioul-bank.toml` (the bank's movements), `sioul-movements.toml` (what Sioul learnt of your payments: which ones are the same, the holds not final yet, your answers), `sioul-contracts.toml`. Readable, editable by hand, versioned with git if you like. They travel with your notes folder, or sealed through Sioul's sharing once you switch **Projects and money** on in it ([Sharing](sharing.md)); nothing is sent anywhere else.

Budgets show by area as your hours say, as in Mail ([At a glance](#at-a-glance)); every budget can still be chosen for a payment, at any hour. See [Hours](hours.md).

## Going further {#going-further}

### Where each movement goes {#where-each-movement-goes}

In this order:

1. **Your hand**: **Where it goes**, on a movement's row, places it in a budget, or in none.
2. **The same payment as a line already counted**, from mail or by hand: counted once.
3. **Money moved between your own accounts**: in no budget, since the other side is read.
4. **The first of your rules that holds** (**Rules…**, on the account's card): words in the label, money in or out, and where it goes: a budget, the recurring payment it stands for, a reserve, another account. A rule without words takes every movement its way.
5. **The recurring payment it stands for**, by its name or its amount, near its day: the real amount replaces the planned one.
6. **The account's first budget**.

Each movement says why it went where it did.

The budgets count each payment once. Sioul tells which payment a bank's line is, a line already counted, a recurring payment, a letter's bill, an invoice you sent, first by what they share (a direct debit's mandate, an invoice's or an order's number), then by the amount, the day and the payee's name, each kind with its own room: a debit around its day, a card payment a few percent apart for a tip, a client's transfer weeks after the invoice, a refund up to eight weeks later. What it is only probably the same as, it says so. Two invoices paid by one transfer, an order charged in parts, a deferred-debit card's monthly sum and a purchase in instalments are told together, so nothing counts twice.

### Daily spending {#daily-spending}

**An estimate (food, cash), not a fixed amount**, on a recurring line, says it is spent little by little: a card payment that stands for nothing else is taken from it, the estimate with the most left first, and only spending beyond it counts more. Real spending is taken from the estimate, never added on top.

### Downloading your bank's file {#downloading-your-banks-file}

Your bank's site can be kept in [Sites](sites.md): Sioul fills your login from Bitwarden, and your security key answers where your bank accepts one. The file you download there is then one **Take in an export…** away.

### In the terminal {#in-the-terminal}

`sioul budgets` prints what the page says, every area at once: each account until its next income, each budget in words, what is coming week by week, the reserves, and the payments mail proposes.

### With an AI agent {#with-an-ai-agent}

If you connect an AI agent ([Using an AI agent](ai-agent.md)), it can read what this page says: each account's sentences, each budget in words, what is coming and the reserves. It cannot move money or change a line, and it sees account numbers masked.

## Why it works this way {#why-it-works-this-way}

Money does not flow evenly through a month: spending rises when pay arrives, and bills follow it (Stephens 2003; Gelman et al. 2014); people who keep a budget count by their pay rather than by the calendar (Zhang et al. 2022). So the page starts from today's balance and your next income: whether the account holds every payment until then, what is left for everyday spending, and the lowest point.

The same numbers shown in red make people expect worse (Bazley, Cronqvist & Mormann 2021), and lessons in managing money fade within two years (Fernandes, Lynch & Netemeyer 2014). So each budget says in plain words what is paid and what is still to come, with no colour, bar or percentage, and no accounting word to learn.

People forecasting their own money count what will come in and forget what will go out, unusual expenses most of all (Berman et al. 2016; Sussman & Alter 2012). So Sioul makes the forecast itself: What is coming lists every payment expected, in and out, and the past year's unusual spending has a line of its own. A forecast that states its doubt is used better, and trusted more after it errs (Joslyn & LeClerc 2012), so an income not yet certain is shown with its condition.

Arrears grow silently: a debit stops, nothing says so, and nobody looks when accounts go badly (Olafsson & Pagel 2017). Bank alerts cut overdraft charges by a quarter when they came by default (Caflisch et al. 2018), and few people sign up for them on their own. So Sioul reads the bank's own movements, and says when the account will not hold a payment, before it happens, at your next admin window, whichever budgets the hour shows.

A reminder helps most when it names what the money is for (Karlan et al. 2016), and one that ignores the balance can push an account into overdraft (Medina 2021). So a payment you make by hand has one reminder, a few days ahead, saying whether the account will hold it.

A payment is often told by a mail, a letter or an invoice, then by the bank. Sioul keeps them together as one payment and links them itself: keeping records by hand weighs on many people (Rudberg Selin, Unéus & Knudsen 2026), so nothing waits for you to check, and a link that is wrong is undone in one step.

Most people who keep a budget sort money into kinds of their own, from two to one per bill (Zhang et al. 2022), and an expense that fits none of them is the one that escapes (Heath & Soll 1996). So budgets are the kinds you choose, Sioul files every payment, and moving one says what it changes.

Irregular income is common: in a year of household diaries, monthly income varied by about 40 % (Hannagan & Morduch 2015), and getting through a lean month and a large expense together took about six weeks of take-home pay (Farrell, Greig & Yu 2019). So reserves are counted in weeks of your own spending, and a work budget can set aside a share of each payment for taxes as it arrives, as money set apart in an envelope is kept (Soman & Cheema 2011).

## Compared with other apps {#compared-with-other-apps}

As of November 2026, from each app's own documentation.

✓ documented; **partly**, with a note; ✗ not found in the app's own documentation (for Sioul: not built); ? not confirmed; — not applicable. Sioul's column was checked against its code.

**For everyone**

| | Sioul | YNAB | Actual | Firefly III | GnuCash | Bankin' |
|---|---|---|---|---|---|---|
| Budgets by what money is for; unspent money kept | ✓ | ✓ | ✓ | partly¹ | partly² | partly³ |
| Where each budget stands, in words: paid, still to come, left | ✓ | partly⁴ | ✗ | ✗ | partly⁵ | ✗ |
| Payments expected, matched with the bank's movements | ✓ | ✓ | ✓ | ✓ | ✓ | partly⁶ |
| Says when an expected payment did not leave, or took another amount | ✓ | ✗ | partly⁷ | partly⁸ | ✗ | ✗ |
| Says ahead that an account will not hold a payment, and what to move by when | ✓ | ✗ | ✗ | ✗ | ✗ | partly⁹ |
| Payments read from your mail, counted once | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Contracts: renewal, notice deadline, a cancellation letter | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Reads your bank's export files | ✓ | ✓¹⁰ | ✓ | ✓¹¹ | ✓ | ✗ |
| Connects to your bank by itself | ✗¹² | ✓ | ✓ | ✓¹¹ | partly¹³ | ✓ |
| Reports and charts | partly¹⁴ | ✓ | ✓ | ✓ | ✓ | partly¹⁵ |
| Investments, several currencies, double-entry books | ✗¹⁶ | partly¹⁷ | partly¹⁸ | ✓ | ✓ | partly¹⁹ |
| How the account stands until your next income: what is left, the lowest point | ✓ | ✗ | ✗ | ? | partly²⁰ | partly⁶ |
| What is coming: every payment expected, in and out, by date | ✓ | partly²¹ | partly²² | ? | partly²³ | partly²⁴ |
| One payment with all that tells of it: mail, letter, contract, bank line | ✓ | partly²⁵ | ✗ | partly²⁶ | partly²⁷ | ? |

1. By the kind of budget: a fixed amount each period is gone at the next one; "add money … every month" makes it grow.
2. A budget sets planned amounts per account and period against the actual ones, in reports.
3. Budgets by category of spending; carrying over is not described.
4. Progress bars in colours against a target; red when spending exceeds what is available.
5. A budget report sets the planned against the actual.
6. A forecast of the end-of-month balance.
7. A schedule whose date has passed without its payment shows "missed" (from its source code); amounts matched "approximately" within 7.5 %.
8. Subscriptions have a minimum and a maximum amount, and a box on the front page "will tell you how you're doing".
9. A notification before an overdraft, and the end-of-month forecast.
10. On the web and on an iPad, not on phones.
11. Through the Data Importer, installed beside it.
12. By choice: you download the file from your bank, and Sioul reads it on your device.
13. German banks through HBCI/FinTS, and banks that speak OFX.
14. Each budget's balance by days, weeks, months or years, and the balance ahead until the next income.
15. Budget history, and this month against last month, in Bankin' Plus.
16. An amount in another currency is marked, not converted.
17. Investment accounts kept as tracking accounts; another currency in a separate budget.
18. Reports include investment accounts.
19. Savings and stock accounts, in Bankin' Plus.
20. A "Projected Minimum" balance in each register's summary bar.
21. Scheduled transactions, at the top of each account's register.
22. Schedules in the register, up to a length you choose.
23. Scheduled transactions, created or reminded a chosen number of days ahead.
24. The upcoming operations that some banks send (ING France, Crédit Mutuel).
25. One photo per transaction; no PDF.
26. Attachments, on transactions among other things.
27. One linked file or web address per transaction.

**For technical readers**

| | Sioul | YNAB | Actual | Firefly III | GnuCash | Bankin' |
|---|---|---|---|---|---|---|
| Where your money's data lives | plain files in your notes folder | YNAB's servers | your device and your server | your server | a file on your computer | Bankin's servers, in Europe |
| Needs an account with the maker | no | yes | no | no | no | yes |
| Encrypted end to end between your devices | ✓¹ | ✗² | partly³ | — | — | ✗ |
| Export formats read | OFX, QFX, CAMT.053, CSV, PayPal, Stripe | CSV, OFX, QFX, QIF | CSV, QIF, OFX, QFX, CAMT | CSV, CAMT.053 | QIF, OFX, QFX, CSV, MT940, MT942, CAMT.052 and more | — |
| A file read twice adds nothing | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| A bank line matched to what was expected | a reference first (the bank's id, a SEPA mandate, an invoice number), then amount, days apart and name: sure, probable or apart | the same amount within 10 days | a schedule within 2 days; an import by the same amount within 7 days, the same payee first | a rule, the amount between a subscription's minimum and maximum | a score of amount, date, cheque number and words | ? |
| Bank connections through | none | Plaid, MX | Enable Banking, SimpleFIN, Pluggy.ai, Akahu; GoCardless⁴ | GoCardless, Enable Banking, SimpleFIN, FinTS and others | AqBanking (HBCI/FinTS), OFX | its own (Bridge, licensed by the ACPR) |
| Free software | ✓ GPL-3.0+ | ✗ | ✓ MIT | ✓ AGPL-3.0 | ✓ GPL | ✗ |

1. XChaCha20-Poly1305, with a key made from your passphrase by Argon2id; once you switch **Projects and money** on.
2. Encrypted at rest and in transit on YNAB's servers; end-to-end encryption is not mentioned.
3. Optional; bank-sync tokens are not covered, and the copy on your device is not encrypted.
4. Listed as "not accepting new accounts".

Others do more in places: YNAB, Actual, Firefly III and Bankin' connect to the bank by themselves, and GnuCash does for German banks; GnuCash and Firefly III keep double-entry books, investments and several currencies; the four budget apps make fuller reports, and their phone apps have more use behind them; PocketSmith, not in the table, forecasts each account's balance for every day, years ahead. What Sioul adds is the account said until your next income, each payment kept with the mail, letter and contract that tell of it, the watch in sentences, and your contracts with their notice deadlines, all without a bank connection.

??? info "Sources"
    Read on 8 October 2026; the rows on the next income, what is coming, a payment's sources and matching, on 4 November 2026.

    - **YNAB**, its help centre (glossary, progress bars, file import, direct import in Europe, scheduled transactions, overspending, and its index), its security page and its prices: <https://support.ynab.com/en_us/ynab-glossary-a-guide-BJd80SORq>, <https://support.ynab.com/en_us/progress-bars-a-guide-SkDEhot09>, <https://support.ynab.com/en_us/file-based-import-a-guide-Bkj4Sszyo>, <https://support.ynab.com/en_us/direct-import-in-europe-Syae1z_A9>, <https://support.ynab.com/en_us/scheduled-transactions-a-guide-BygrAIFA9>, <https://support.ynab.com/en_us/overspending-in-ynab-a-guide-ryWoxEyi>, <https://support.ynab.com/llms.txt>, <https://ynab.com/security>, <https://www.ynab.com/pricing>
    - **Actual Budget**, its documentation on budgeting, importing, bank sync, rules, schedules, sync, installing and reports, and its repository (`schedules.ts` for "missed"): <https://actualbudget.org/docs/budgeting/>, <https://actualbudget.org/docs/transactions/importing>, <https://actualbudget.org/docs/advanced/bank-sync>, <https://actualbudget.org/docs/budgeting/rules/>, <https://actualbudget.org/docs/schedules>, <https://actualbudget.org/docs/getting-started/sync>, <https://actualbudget.org/docs/install/>, <https://actualbudget.org/docs/reports/>, <https://github.com/actualbudget/actual>
    - **Firefly III**, its documentation on budgets, subscriptions, file imports and data providers, and its repository: <https://docs.firefly-iii.org/how-to/firefly-iii/finances/budgets/>, <https://docs.firefly-iii.org/explanation/financial-concepts/budgets/>, <https://docs.firefly-iii.org/explanation/financial-concepts/subscriptions/>, <https://docs.firefly-iii.org/how-to/data-importer/import/file/>, <https://docs.firefly-iii.org/tutorials/data-importer/data-providers/>, <https://github.com/firefly-iii/firefly-iii>
    - **GnuCash**, its features page, its manual on importing and on preferences, and its guide's chapter on budgets: <https://www.gnucash.org/features.phtml>, <https://www.gnucash.org/docs/v5/C/gnucash-manual/trans-import.html>, <https://www.gnucash.org/docs/v5/C/gnucash-manual/set-prefs.html>, <https://code.gnucash.org/docs/C/gnucash-guide/chapter_budgets.html>
    - **Bankin'**, its own description in Apple's App Store (its help centre could not be read), its Plus and Pro page and its home page: <https://apps.apple.com/fr/app/bankin-la-meilleure-app-pour-g%C3%A9rer-mon-argent/id447040033>, <https://bankin.com/fr/pluspro.html>, <https://bankin.com>
    - **Added on 4 November 2026**: YNAB on matching and photos, <https://support.ynab.com/en_us/approving-and-matching-transactions-a-guide-ByYNZaQ1i>, <https://support.ynab.com/en_us/add-context-to-transactions-HyCpTR4bg>; Actual's matching in its repository, `packages/loot-core/src/server/accounts/sync.ts`; Firefly III's attachments, <https://docs.firefly-iii.org/api/storeattachment/> (through a search summary); GnuCash's register, <https://www.gnucash.org/docs/v5/C/gnucash-manual/gui-acct-reg.html>, its scheduled transactions and linked documents, through a search summary, <https://code.gnucash.org/docs/C/gnucash-manual/trans-sched-slr.html>, <https://code.gnucash.org/docs/C/gnucash-manual/trans-assoc.html>, and its matching, `gnucash/import-export/import-backend.cpp` at <https://github.com/Gnucash/gnucash>; Bankin' on upcoming operations, through a search summary, <https://support.bankin.com/hc/fr/articles/360027356511-Op%C3%A9rations-%C3%A0-venir>; PocketSmith's forecast, <https://www.pocketsmith.com/features/budgets-and-planning>

## For technical readers {#for-technical-readers}

**Files.** `sioul-budgets.toml` (budgets, recurring payments, lines, reserves, covers, mail rules, bank accounts, their rules and your single choices), `sioul-bank.toml` (each account's movements by the bank's own id, its newest balance), `sioul-contracts.toml`: TOML at the root of your notes folder, written with their comments kept. Messages you said are not payments are listed in `~/.local/state/sioul/money.toml` on Linux. Amounts are kept in whole cents, so sums never drift, and amounts written in every national style are read ("1 234,56 €", "€1,234.56", "(62.00)").

**Exports.** OFX 1.x (SGML) and 2.x (XML), by each movement's `FITID`; ISO 20022 camt.053, versions 001.02 to 001.08 (booked entries, the closing booked balance, names under `Pty`); a bank's CSV, its header found by its words, with `;` or `,`, decimal commas, debit and credit apart or not, and a balance written above the table; PayPal's activity (net of fees; pending, refused and memo rows left out, and rows in another currency than most of the file's); Stripe's balance history. A movement without an id gets one made from its day, amount and label: a file read twice adds nothing.

**From one income to the next.** An account's balance is the bank's last one with the lines read after it, carried with every movement expected (planned, announced, a card hold) to its next income: the first money in that is neither a move between your accounts nor a refund, worth a fifth of the largest coming and €50 at least; a recurring or announced one is sure, a planned line or an invoice of yours is said with its doubt. Left for everyday spending is the lowest point above what the account keeps, less the past year's unusual spending for those days: the payments standing for no recurring one, three times the usual payment and €50 at least, per month. With no income within two months, the account and its savings are counted in weeks of your payments of the past thirteen weeks. A budget's end is judged at the middle pace of its past periods once it has three with unplanned movements; before that, what has come in or left beyond the plan is weighed against the part of the period gone, and what is ahead or behind today is carried to the end. Within 5 % of what the period moves (and never closer than 10), it ends as planned.

**The watch.** A payment is not seen when its days (three before to seven after) passed inside what its account's exports cover with no line for it. "Took another amount" means more than 5 % and €2 apart. A top-up is rounded up to ten and asked as many days ahead as its reserve takes. The budgets' filter is kept on each device, in `~/.local/state/sioul/budgets-view.toml`, and a copy of the money movements made from your files in `~/.cache/sioul/money-books.json`, made anew whenever a file changes: neither is shared, and deleting them loses nothing.

**Mail.** Only mail that passed the authentication checks is read for payments ([Every message checked](privacy-security.md#every-message-checked)): never mail set aside (forged, spam, a borrowed name, a blocked sender), nor the review queue. Payments are read by rules, in French and English, from the subject and the start of the text; a share capital in a legal footer is never taken for an amount.

**Sealed between devices.** With **Projects and money** on, each change to the three files is sealed with XChaCha20-Poly1305 (a random 192-bit nonce each), bound to the device that wrote it, its place and its time, under a key made from your passphrase by Argon2id (64 MiB, three passes) and kept in each device's keyring ([Sharing](sharing.md#what-it-protects-and-what-it-cannot-hide)).

**The AI agent.** Its `budgets` tool reads through the same view as the window, writes nothing, and masks IBANs ("[IBAN …0189]").

**Contracts.** The usual notices proposed are France's: three months for a tenant (one in a furnished flat or a tight area); one month for insurances and health covers after their first year (loi Hamon); at most ten days for phone and internet after twelve months; none for energy (art. L224-13 of the consumer code). Online, a cancel button has been required since 1 June 2023 (loi 2022-1158).
