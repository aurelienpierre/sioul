---
description: Budgets, reserves and bank accounts in Sioul - on track or not said in words, payments read from mail, bank exports placed by your rules, a quiet watch on what did not leave, and your contracts.
---

# Budgets and bank accounts

Money comes and goes by mail: invoices, receipts, payment notices, payouts, tax notices. Sioul keeps three things apart:

- **bank accounts**, where the money is: a current account, PayPal, Stripe;
- **reserves**, where it is set aside: savings accounts;
- **budgets**, what it is for: your duties (rent, energy, food), your leisure, your work, a project.

So you see at a glance which budgets are positive or negative, which are on track, and how long each saving lasts. Never in red.

<figure markdown="span">
  [![The Budgets page, "Budgets, October 2026", with New budget: three budgets (work, household duties, leisure), each a card with its name and period, "On track" on its own line, a sentence such as "16% of the month gone; 51% of what it can spend is spent.", then so far, expected by 31 October and the target, one per line.](../assets/screens/budgets.png){ loading=lazy }](../assets/screens/budgets.png "Open the picture at full size")
  <figcaption>Each budget, its verdict in words, one figure per line.</figcaption>
</figure>

## Budgets

**New budget**:

- **Name**.
- **Counted by**: the month or the year.
- **Balance to reach** by the end of each period: 0 to break even; more is saving.
- **For**: work, your admin, leisure. It decides the hours the budget is in view ([Hours](hours.md)). None ticked: your admin.

What a budget did not spend carries over, as an envelope keeps what is left: money set aside in October for a bill due in December is still there in December.

### At a glance

Each budget is a card: its name and period, the verdict on its own line (*On track*, *Ahead of its pace*, *Behind its pace*, with by how much), then one figure per line: what came so far, what is expected by the end of the period, the balance to reach, what was carried in, and what is set aside for later.

The verdict does not assume that money flows evenly. What is known in advance (a rent, a planned bill) counts in full. The rest is weighed against the part of the period gone: "40% of the month gone; 35% of what it still needs has come in." After three periods, the expected end comes with a range, *from a bad month to a good one*.

### A budget, open

A budget opens on its movements, newest first, and its balance by **Days**, **Weeks**, **Months** or **Years**, the planned ones dotted.

**Add a movement**: **Once**, or **Recurring** (every month or every year, on a day). An amount is negative for money out (`-650`) and positive for money in (`1200`). **An estimate (food, cash), not a fixed amount** says it is spent little by little, and real spending is taken from it rather than added on top.

A right click on a line: **Change this line…**, **Delete this line** (asked once).

**Take this budget out** keeps its lines in the file, as they were written; they no longer count.

## Reserves

**New reserve**: its **Name**, its **Balance** on a day, the amount it should **never** go **below**, and the **days** money asked from it takes **to arrive** (0 for a savings account you can move from at once, longer for others).

Each reserve shows its balance today, what moved this month, its balance at the end of the year, and how long it lasts at this pace, above its floor.

Which reserve covers which budget is written in the budget file: at the end of each period, a budget's shortfall is taken from its reserve.

## Mail about money

Below the budgets, the mail about money, newest first. Sioul reads, in English and French, what a message says: a payment received or made, an order, a bill, a refund; the amount; who paid, or was paid.

Each line says where it stands: *counted*, *the same payment as* another message (an order and its payment count once), or waiting for you, with **Add** and **Not a payment**. A line from someone new and not verified says to check it is genuine first. Forged mail, spam and blocked senders are never read as payments.

Rules in the budget file can send a sender's or a word's payments to a budget by themselves.

## Bank accounts

<figure markdown="span">
  [![Bank accounts on the Budgets page: two cards, each with its balance from the bank's file, the budgets it fills, the reserves that top it up, its last movements folded, and Take in an export…, Rules… and Edit; below, a payment that took another amount than expected, this week's payments, and the month ahead as a plain line.](../assets/screens/bank-accounts.png){ loading=lazy }](../assets/screens/bank-accounts.png "Open the picture at full size")
  <figcaption>Each account, the budgets it fills, this week's payments and the month ahead.</figcaption>
</figure>

**New bank account**: its **Name**, its **Kind** (a bank, PayPal, Stripe, something else), the budgets it **Fills**, the amount it is **Kept above**, and the reserves it is **Topped up by**, in your order.

**Take in an export…**, on its card: the file your bank gives (OFX or QFX, CAMT.053, or its CSV), PayPal's activity download, or Stripe's balance history. Reading the same file twice adds nothing. Nothing is fetched from the bank itself: you download the file, Sioul reads it.

### Where each movement goes

In this order:

1. **Your hand**: **Where it goes**, on a movement's row, places it in a budget, or in none.
2. **The same payment as a line already counted**, from mail or by hand: counted once.
3. **Money moved between your own accounts**: in no budget, since the other side is read.
4. **The first of your rules that holds** (**Rules…**, on the account's card): words in the label, money in or out, and where it goes: a budget, the recurring payment it stands for, a reserve, another account. A rule without words takes every movement its way.
5. **The recurring payment it stands for**, by its name or its amount, near its day: the real amount replaces the planned one.
6. **The account's first budget**.

Each movement says why it went where it did.

### Topped up in time

Each account is carried forward a month. On the first day it would go below its floor, Sioul says what to move, from which reserve, and by when, counting the days that reserve takes: "Savings: ask for €590 by Thursday 15 October; it takes about ten days, and the rent leaves on 1 November." When asking today is already too late, it says so.

## The bank watch

A debit that stops does not say so. Sioul holds your bank's movements against the payments you expect, and says, in sentences:

- **what has not left**: "Phone (€19.99) has not left the account. Expected around 12 September, not in your bank's movements.";
- **what changed**: "Electricity took €71.40 on 5 September, where €62 was expected.";
- **what may not be held**: "The account may not hold the rent on 1 November. €579.99 will be missing then.", with the reserve that covers it when one does.

**This week**: the payments of the next seven days, in one line, here and at the top of the Porch: "This week: Electricity €62 (Mon). The account holds them." **The month ahead**: the balance as a plain line, zero dashed.

A reminder comes once, a week after a payment that did not leave, and five working days before a day the account would not hold a payment.

## Contracts and subscriptions

What you are bound to: rent, energy, phone and internet, insurances, health cover, subscriptions, hosting, the bank. Forgotten subscriptions are part of what people with ADHD call the "ADHD tax"; knowing what an insurance covers matters before a legal step.

**Add a contract**: with whom, your number with them, the recurring payment that pays it, since when, when it renews (each month, each year, or no renewal), the notice it needs, how to stop it (its cancel page, or where to write), and what it covers.

- **Where it stands**, in a sentence: "Renews on Tuesday 1 December; to stop it, the notice must leave by 2 October."
- **The usual notice** for its kind is proposed, with the rule it comes from (in France). Yours is what your contract says.
- **A reminder**, once, two weeks before the last day a notice can leave, for contracts that renew each year.
- **Stop it…**: **Open its cancel page**, or **Write the letter**: a formal draft to its address, with your number, asking for a written confirmation. You read it, sign it and send it. **It has ended** keeps it, faded, as a record.

Recurring payments with no contract yet are listed below, each with **Note it**. A message can become one with **Keep as a contract…**, in its menu.

Sioul never shows offers, comparisons or "better deals": only facts about your contracts.

## Where it is kept

In plain files at the root of your notes folder: `sioul-budgets.toml` (budgets, reserves, lines, rules, bank accounts), `sioul-bank.toml` (the bank's movements), `sioul-contracts.toml`. Readable, editable by hand, versioned with git if you like. Nothing is sent anywhere.

In free time, only the budgets for leisure are in view. See [Hours](hours.md).

## Why it works this way

Arrears grow silently: a debit stops, nothing says so, and nobody looks when accounts go badly (Olafsson & Pagel 2017). A payment reminder that ignores the balance can push an account into overdraft (Medina 2021). So Sioul reads the bank's own movements, and says when the account will not hold a payment, before it happens.
