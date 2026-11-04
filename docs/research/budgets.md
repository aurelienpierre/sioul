# Budgets over time: money coming in and going out

**The question**: how should Sioul show when money comes in and goes out, to a person who knows little accounting and for whom admin hurts, once money movements are objects of their own (tied to contracts, mails, paper letters and bank lines, matched across them, movable between budgets) and budgets are filtered by area (work, admin, leisure) at the hours each is admitted?

Researched on 4 November 2026 for the budgets batch: the literature, thirteen apps' help pages, and record-linkage and reconciliation methods. Nothing here is built; section 6 proposes.

**How evidence is graded**, as in [life-admin.md](life-admin.md): **meta** (meta-analysis or systematic review), **RCT** (randomised, in the field or the lab), **study** (observational or quasi-experimental, its kind said), **qualitative**, **grey** (reports by organisations), **theory**. For apps and web pages: **vendor** (the maker's page, read), **review** (a third party's page), **snippet** (known only from a search summary, because the page refused automated reading or was not opened: unverified). Full references are in section 7.

## 1. Summary

- **Why the current page confuses.** "n % of the month gone, m % of the budget spent" assumes money flows evenly through a calendar month. It does not: spending rises when income arrives and bills cluster after it (Stephens 2003; Shapiro 2005; Gelman et al. 2014), and people budget by their pay period (Zhang et al. 2022). A fixed bill reads as 100 % spent on its first day, and the two percentages leave the comparison to the person.
- **What answers the worry is shaped by time**: until the next income, will the account hold every payment, what is left for everyday spending, where is the lowest point; then what is coming, and when; then, apart, how long the reserves last.
- **People look away when it goes badly** (Karlsson et al. 2009; Olafsson & Pagel 2017), so the facts must reach them unasked, once, in words. Alerts on by default lower fees (Caflisch et al. 2018); a reminder works best when it names the bill (Karlan et al. 2016) and checks the balance (Medina 2021); repeated contact harms (Murray & Bond 2023).
- **People forecast income and forget expenses**, the unusual ones most (Berman et al. 2016; Sussman & Alter 2012): Sioul should forecast from the data, irregular spending included, and say its doubt.
- **The apps** show the future as a balance line ahead (PocketSmith, Simplifi, GnuCash, the French aggregators), one "left to spend" number (Monarch, Simplifi, Copilot, YNAB) or a calendar of bills (Monarch, Lunch Money, PocketSmith). They match by amount, then a window of 2 to 14 days, then the payee. None ties the mail or letter announcing a payment to its bank line; most mark lateness in red or by counts.
- **Matching** should use exact keys first (the SEPA mandate reference, invoice references, France's e-invoices from 2026–2027), then a score with three outcomes (Fellegi & Sunter 1969), and know its traps: card holds, deferred-debit cards, orders charged in parts, schedules against invoices, refunds, currencies, identical amounts.

## 2. The literature

### 2.1 Mental accounting, budgets, envelopes
- **Money is kept in labelled mental accounts**, by source and use, each with its period, and is treated as less fungible than economics assumes (Thaler 1985, theory; Thaler 1999, review).
- **Budgets constrain, and the atypical escapes them.** People set limits per category, track spending against them and buy less once one is spent; purchases atypical of a category are less often counted against it (Heath & Soll 1996, study).
- **How people really budget.** In a representative US survey (N = 3,826), 65.6 % budget, at every income level, over 90 % of them by categories (from two to one per utility); 60.7 % budget by the month, but periods follow pay (52.7 % of the weekly-paid budget by the week, against 24.6 % of all); people in worse financial health check their budgets less often (Zhang et al. 2022, study, with Australian bank data).
- **Earmarked money, split in two, is kept.** Over 14 weeks with 146 households of labourers paid weekly in cash in rural India (baseline saving 0.75 % of income), savings sealed in two envelopes beat savings in one (about 414 against 241 rupees), more so with the children's photograph on them (Soman & Cheema 2011, RCT).
- **A labelled place helps; a label that locks money away can hurt.** In Kenya, a safe box at home raised health savings by 66 %; earmarking it for emergencies helped, earmarking it for preventive health lowered savings, through the liquidity it cost (Dupas & Robinson 2013, RCT).
- **Ambiguous expenses go where there is room**: a dinner out becomes "social" once "food" is spent; clear categories and being accountable to someone make this harder (Cheema & Soman 2006, study; full text not read).
- **Rule.** Budgets need no teaching, and their kinds are the person's own. A budget's period follows its income. Money set aside for a known bill helps if it stays reachable, as Sioul's reserves do. Moves between budgets are the person's; Sioul files atypical movements itself, since those escape.

### 2.2 Attention to money, and avoiding it
- **The ostrich effect.** Swedish and US investors looked at their portfolios more in rising markets than in flat or falling ones (Karlsson et al. 2009, study); 1.1 million Vanguard savers logged in 9.5 % less after a market fall (Sicherman et al. 2016, study); in an Icelandic aggregator's data, logins rose after payday and fell as spending and overdrafts grew (Olafsson & Pagel 2017, study, working paper). Not everyone hides: active traders looked more after rises and falls alike (Gherzi et al. 2014, study).
- **Easy access lowers fees, and attention fades.** After a finance app came out in Iceland, its users' fees for insufficient funds fell (38.4 % within two years, by a summary; unverified), from access rather than from alerts (Carlin et al. 2023, study). In the US, survey questions about overdrafts lowered overdraft fees that month, for up to two years with repeats, most among the less educated (Stango & Zinman 2014, study).
- **Rule.** The person looks least when it matters most: a payment that will not pass, or money missing on a day, must reach them unasked, at their admin hours, once, in words. Looking must feel safe: what is fine first, then the one thing to do.

### 2.3 Payday cycles and liquidity
- **Spending follows income, even expected income.** Households living on Social Security spent more on quickly consumed goods after the check (Stephens 2003, study); food-stamp recipients' caloric intake fell 10–15 % over the benefit month (Shapiro 2005, study); in a mobile app's data (about 75,000 users), spending rose on the day pay arrived and stayed high for at least four days, most among people with little liquidity, partly because bills are paid then (Gelman et al. 2014, study).
- **When pay is late, bills are delayed.** When the 2013 US shutdown delayed federal pay (about 40 % of a paycheck, repaid within two weeks), spending fell sharply, and people bridged by delaying mortgage and card payments; the least liquid struggled most (Gelman et al. 2020, study, natural experiment).
- **Being short is not being less able.** Low-income US households surveyed at random before or after payday were more present-biased about money before it, but no worse at cognitive tasks, risk or the quality of decisions (Carvalho et al. 2016, RCT).
- **Rule.** The stretch from one income to the next is the unit of time that matters; a calendar month hides it for anyone paid on the 28th, by benefits or by clients. Bills that can move are the person's to move to after the money arrives, never Sioul's.

### 2.4 Bill reminders
- **A reminder that names the purpose works.** Monthly reminders to new savers in Bolivia, Peru and the Philippines raised balances by 6 %; in Peru, reminders that named the goal raised savings by about 16 %, and those that did not had no effect (Karlan et al. 2016, RCT).
- **A text three days before did as much as a discount.** With 1,121 borrowers of a Ugandan lender, a monthly SMS three days before each instalment raised on-time payment by 7–9 %, as much as a cash-back worth 25 % of interest (Cadena & Schoar 2011, RCT, working paper).
- **A reminder blind to the balance moves the cost**: credit-card reminders in Brazil cut late fees by about 14 % and raised overdraft fees by about 9 % (Medina 2021, RCT).
- **Alerts work when they come by default.** In UK bank data, text alerts or the app cut unarranged overdraft charges by 5–8 %, both together by 24 %, yet only 3–8 % of customers had registered (FCA 2015, study); automatic enrolment (1.5 million customers, six banks) cut those charges by 25 % (Caflisch et al. 2018, study); encouraging people to register reached about 10 %, with no fall in their charges (FCA 2018, RCT; summary read).
- **When unwell, bills wait, and more contact hurts.** Among people with mental health problems in the UK, 74 % put off paying bills when unwell and 71 % avoided contacting creditors; a bill's "you must contact us immediately" can feel "too hard so it goes on a pile" (Evans & Acton 2017, citing Holkar & Mackenzie 2016, grey). A quarter of people behind on payments were contacted every one or two days; 49 % felt harassed (Murray & Bond 2023, grey).
- **Rule.** One reminder, a few days ahead, that names the bill and says whether the account will hold it; on without signing up, within the person's hours; never repeated.

### 2.5 Financial anxiety and presentation
- **Financial anxiety turns the eyes away.** A ten-item scale matched implicit measures (a dot-probe task showed mostly avoidance of financial words) and was distinct from depression and general anxiety (Shapiro & Burchell 2012, study).
- **Two worries, not one**: stress about managing money now, and security about the future; together they predicted overall well-being about as much as job satisfaction, health and relationships combined (Netemeyer et al. 2018, study).
- **Red changes expectations.** In eight experiments (about 1,450 people), the same data in red rather than black cut risk-taking by 15–30 % and made losses seem likely to continue (Bazley et al. 2021, RCT).
- **Teaching does not stick.** Financial education explained about 0.1 % of the variance in financial behaviour, less in low-income samples, and its effect was negligible 20 months on; the authors advise help given "just in time", at the decision (Fernandes et al. 2014, meta, 201 studies).
- **Rule.** No red: the evidence is direct for money. Answer the daily worry first and the future apart. Every term explains itself in the sentence where it stands; a budgets page is not a lesson in accounting.

### 2.6 Forecasting one's own balance
- **Income counts; expenses are forgotten.** Across 27 studies (8,418 participants), people forecasting their financial slack gave 2.9 times more weight to changes in income than to changes in expenses, more so for the distant future (Berman et al. 2016, study).
- **Spending is underestimated, mostly through the unusual.** A savings goal lowers the forecast, not the spending (Peetz & Buehler 2009, study); people spend nearly twice what they expected on exceptional expenses, because each one looks unique (Sussman & Alter 2012, study); predictions anchor on the typical week, the mode of a skewed distribution, below its mean (about $100 a week too low), and asking why this week might differ made them more accurate (Howard et al. 2022, RCT, ten studies, N = 6,044).
- **Uncertainty shown is used better and trusted more.** A forecast given as a probability improved lay people's decisions over a single value and kept their trust after it erred (Joslyn & LeClerc 2012, RCT); for predictions on a phone, quantile dotplots gave better estimates than densities (Kay et al. 2016, study), and they and cumulative plots improved decisions over a single point or text (Fernandes et al. 2018, RCT).
- **Rule.** Sioul makes the forecast from data, never from the person's guesses, counts expenses as carefully as income, adds the past year's irregular spending as an estimate of its own, says "about", states an uncertain income with its condition, and keeps the lowest point in view rather than an average.

### 2.7 Irregular income
- **Volatility is common, and stability is preferred.** In the US Financial Diaries (235 low- and moderate-income households followed for a year), monthly income varied by 39 % (coefficient of variation), with about two and a half months a year more than 25 % below the household's average and as many above (Hannagan & Morduch 2015, study). In a survey of 7,845 US adults, 92 % chose financial security over moving up the income ladder (Pew 2015, grey).
- **A buffer of about six weeks.** In bank data for about six million US families, riding out an income dip and a spending spike together took a median of about six weeks of take-home income in liquid savings, which 65 % did not have (Farrell et al. 2019, grey, administrative data).
- **Freelancers want less finance, not more.** In a week-long field study with 16 gig workers, some preferred a plain tracker to finance apps ("more of the financial crap" they did not like dealing with), wanted earnings set against expenses, and found taxes "a nightmare" (Hsieh et al. 2025, qualitative).
- **Rule.** With irregular income the question is how long the money lasts and what is still to come in: the payments expected, each with its doubt, and the buffer counted in weeks of the person's own spending. A share of each payment received, set aside for taxes on arrival, is an earmark.

## 3. The apps

Help pages read on 4 November 2026; **snippet** marks what is known only from search summaries, where a help centre refused automated reading. For each: time ahead, the model of a transaction, how a bank line is matched to what was planned, documents. Sources in section 7.

- **YNAB** (vendor). *Time*: no balance ahead; money is assigned to categories until "Ready to Assign" is zero, each showing what is Available; Targets say what a category needs by a date; "Age of Money" counts the days from money arriving to its being spent. Overspending is red ("Immediate action is needed!"). *Model*: scheduled, cleared, reconciled; splits; transfers; imports await approval. *Matching*: the same amount within 10 days, one to one; the bank's amount wins, the person's payee, memo and date stay. *Documents*: one photo, no PDF.
- **Actual Budget** (open source; docs and code). *Time*: schedules in the register, up to a chosen "upcoming length". *Model*: schedules with an exact, approximate (±7.5 %) or ranged amount, moved off weekends but not holidays; rules, splits, transfers. *Matching*: a schedule takes a transaction within 2 days of its date; an import matches by the bank's id, else the same amount within 7 days either side, same payee first, nearest date first, one to one, and the entered transaction takes the bank's date. *Documents*: none.
- **Monarch Money** (vendor, partly snippet). *Time*: income minus fixed, non-monthly and goals gives one "Monthly Flex Number"; non-monthly categories roll over; bars green under 50 %, yellow to 99 %, red at 100 %; a calendar of recurring bills (upcoming blue, paid with a green tick), alerts three days ahead. *Matching*: not documented. *Documents*: attachments; receipt scanning reads merchant, amount, date.
- **Copilot Money** (vendor). *Time*: recurring bills can count "as paid at the beginning of the month", leaving what is free for the unplanned. *Model*: regular, income, internal transfer (outside budgets). *Matching*: name, amount range and date ("Spotify from $9 to $10 around the 23rd"). *Documents*: none found.
- **Lunch Money** (vendor). *Time*: a calendar of daily income, spending and cash flow; no balance ahead stated. *Model*: recurring items, fixed or a range, suggested from "a clear pattern of payee and amount" until accepted; splits, also spread over time. *Matching*: payee, amount and a date window (the 1st to the 5th for a bill around the 3rd), plus the account if given. *Documents*: files.
- **PocketSmith** (vendor). *Time*: the fullest calendar: budgets are events, and each account's balance is forecast for every day ahead ("up to 60 years") from today's real balance, with "what-if" scenarios. *Model*: transactions are the past, budgets the future; future-dated transactions "can result in incorrect balances". *Matching*: none of a line to a plan. *Documents*: files, and an Attachment Inbox where receipts wait for their transaction.
- **Simplifi** (vendor, partly snippet). *Time*: "Available to spend" is income minus bills, subscriptions, planned spending, savings goals and recurring transfers, minus other spending; Projected Cash Flow draws the balance ahead from reminders and expected refunds. *Matching*: a download links to the next reminder of its series, by payee, then by an amount rule (any, exact, a range); Quicken says the exact criteria "aren't typically made known". *Documents*: attachments.
- **Firefly III** (open source; snippet). *Time*: budgets per period; subscriptions weekly to yearly. *Model*: withdrawal, deposit, transfer; splits; piggy banks. *Matching*: a subscription's minimum and maximum amount and a rule; imports deduplicated by a content hash or an identifier. *Documents*: attachments.
- **Goodbudget** (vendor). *Time*: envelopes, scheduled transactions. *Matching*: an import's identity is its name, date and amount; scheduled entries duplicate the bank's line, so the help suggests deleting them; since 2025, a merge by hand keeps "the bank-provided information". *Documents*: none found.
- **HomeBank** (open source). *Time*: scheduled transactions; late ones show a warning with a count up to 10, then "+10"; no balance ahead. *Matching*: duplicates only, by OFX id or identical fields within a 0-day gap by default; no merge. *Documents*: none.
- **GnuCash** (open source; manual and code). *Time*: scheduled transactions created or reminded days ahead; a "Future Minimum" column, each account's lowest balance from today on; a forecast report. *Matching*: a score. Amount: the same +3, within a fee threshold (2.00) +2, else −5; date: the same day +3, within 4 days +2, beyond 14 −5; the same check number +4; the same memo or description +2. Shown from 1, proposed as new at 3 or less, cleared at 6 or more; accounts guessed by Bayesian matching of words. *Documents*: one linked file or URL.
- **Bankin'** (French aggregator; snippet). *Time*: an end-of-month balance from the bank's own "opérations à venir", only with banks that send them. *Matching, documents*: none documented.
- **Linxo** (French aggregator; reviews). *Time*: a 30-day forecast from detected recurring operations, with an overdraft alert threshold (Premium). *Matching, documents*: none documented.

| App | Balance ahead | Upcoming | One "left" number | Planned against bank line | Documents |
|---|---|---|---|---|---|
| YNAB | no | scheduled | Available, Ready to Assign | same amount, ±10 days | one photo |
| Actual | no | schedules | Available | ±2 days (±7.5 % if approximate); import ±7 days, payee first | none |
| Monarch | no | calendar, alert 3 days ahead | Flex Number | not documented | yes, scanning |
| Copilot | no | recurrings | left after recurrings | name, amount range, date | none found |
| Lunch Money | not stated | calendar | budget | payee, amount or range, date window | yes |
| PocketSmith | daily, years ahead, what-if | calendar | no | none: plans and actuals apart | yes, receipt inbox |
| Simplifi | daily projected | reminders | Available to spend | next reminder; payee, then amount rule | yes |
| Firefly III | no | subscriptions | budget available | rule; amount between min and max | yes |
| Goodbudget | no | scheduled | envelopes | merge by hand | none found |
| HomeBank | no | scheduled, late count | budget | duplicates only, gap 0 days | none |
| GnuCash | Future Minimum, report | since last run | no | score: amount, ±4/14 days, number, words | one link |
| Bankin' | end of month (bank's data) | bank's upcoming | no | not documented | none found |
| Linxo | 30 days (Premium) | detected recurring | no | not documented | none found |

**Together.** None ties the mail or letter that announced a payment to its bank line (PocketSmith's receipt inbox comes nearest), and most mark lateness in red or by counts, which Sioul refuses.

## 4. Presenting it to someone who is not an accountant

### 4.1 Three questions, a few numbers
- **"Will I be all right until the next money comes in?"** Three facts answer it. *Until when*: the next income and its day, or, with irregular income, the next payment expected and its doubt; Monzo's Trends runs "from one payday to the next". *What is left after the bills before then*: the money in the account minus the bills that leave before that day, named; it is Monzo's "left to spend", Simplifi's "Available to spend", Monarch's flex number and the French *reste à vivre*, the money left once fixed charges are paid. For everyday spending alone it can be said per day, since daily spending is roughly even and bills are not. *The lowest point*: the lowest balance before the next income, its day and the payment that causes it (GnuCash's Future Minimum; Sioul's "short" sentence).
- **"What is coming, and when?"** A list in date order of what is expected in and out, with who, how much (or "about"), the budget, its source and whether it has happened. The US consumer agency's coaching toolkit lays bills on a monthly "bill calendar", and Monarch, Lunch Money and PocketSmith show calendars; no study comparing a grid with a list was found. A list reads aloud, fits a phone and groups by week; a month grid can sit beside it.
- **"How does the year look?"**, apart and after: the reserves and how long they last at the person's own pace, what is set aside for later bills, income still expected.
- **At most one chart**: the balance ahead as a step line to the next income, zero dashed, the floor plain, incomes and large bills marked, an uncertain income as a lighter second line; never red, and never needed to understand the sentences.

### 4.2 What to drop
- **The share of the month gone beside the share of the budget spent.** It makes the person compare two percentages, assumes money flows evenly (section 2.3), and says nothing about a fixed bill (rent paid on the 1st reads as 100 % spent on day one). Monarch's bars coloured by the share spent and YNAB's red are the same idea.
- **Accounting states as words**: debit and credit (they mean opposite things on a bank statement and in a ledger), cleared, posted, reconciled, accrual, "Ready to Assign".

### 4.3 Words
A review of 45 top-rated budgeting apps found that all track transactions but a third have no envelope budgets, and that their concepts of "accounts" and "transactions" overlap and confuse; its authors call for a more nuanced vocabulary (Alenazi & Sas 2023, study, functional review). Proposed words:

| Instead of | Say | In French |
|---|---|---|
| credit, debit | money in, money out | entrées, sorties |
| scheduled, forecast | expected on Friday 5 December | prévu le vendredi 5 décembre |
| pending, hold | not final yet: the shop may take another amount | pas encore définitif |
| cleared, posted | has left the account; has arrived | passé sur le compte ; arrivé |
| available to spend | left for everyday spending until the 28th | reste à vivre jusqu'au 28 |
| projected minimum | the lowest point: €140 on Wednesday 26 | le point le plus bas : 140 € le mercredi 26 |
| transfer | moved between your own accounts | entre vos comptes |
| matched, reconciled | the same payment, told by the bank and by the mail | le même paiement |
| sinking fund, earmark | set aside for the insurance in March | mis de côté pour l'assurance de mars |
| overspent | €30 more than planned | 30 € de plus que prévu |

## 5. Matching and deduplication

### 5.1 Block, score, assign
- **Record linkage** compares pairs field by field; each agreement weighs by how much likelier it is among true matches than among non-matches, and two thresholds split pairs into matches, non-matches and possible matches (Fellegi & Sunter 1969, theory). Only candidates that share a key are compared ("blocking": the same account and direction, an amount close enough, a date window), then scored and classified (Christen 2012; Elmagarmid et al. 2007, reviews).
- **Names**: Jaro–Winkler did well on short names, and TF-IDF with Jaro–Winkler between tokens ("SoftTFIDF") did best overall (Cohen et al. 2003, study, benchmarks).
- **Invoices**: Odoo's reconciliation models find the invoice's reference in the bank line's label (or by regular expression) and the partner's name, accept a payment slightly lower than the invoice within a tolerance (never a higher one), leave the rest of a partial payment open, and can validate alone when every condition holds (vendor).

### 5.2 Keys stronger than similarity, in France
- **SEPA direct debits** carry the creditor's identifier (ICS) and the mandate's unique reference (RUM, up to 35 characters). The creditor must give the date and amount at least 14 calendar days ahead unless agreed otherwise, and the notice carries the ICS and RUM; an authorised debit can be refunded within 8 weeks, an unauthorised one within 13 months (Banque de France; IEDOM; official, snippet). A letter or schedule stating its RUM, the bank line carrying it, and a later refund can be tied exactly.
- **Card lines carry a cut name**: Stripe caps the descriptor at 22 characters, a prefix of 2 to 10 plus a suffix (vendor, snippet), and processors put their own name first ("PAYPAL *", "SUMUP *").
- **Work invoices become data.** Since 1 September 2026 every French business must be able to receive electronic invoices; small and micro-enterprises, the self-employed included, must issue them from 1 September 2027, as Factur-X (a PDF carrying its data), UBL or CII (impots.gouv.fr; official, snippet). A self-employed person's account must be dedicated to the work once receipts pass €10,000 two years running (Service-public; official, snippet): such an account tells the area.

### 5.3 Traps
- **Holds and pending lines**: petrol stations, hotels and car rentals place holds; between pending and posted "their name and amount may change" (a tip), some pending lines "simply disappear", and posting takes one to five business days, rarely fourteen (Plaid, vendor).
- **Deferred-debit cards** (*carte à débit différé*, common in France) take about a month of card payments in one sum on a fixed day (Crédit Agricole, vendor, snippet).
- **One order, several charges**: Amazon charges each shipment as it leaves, the charges adding up to the order (Amazon Pay, vendor, snippet); the reverse, one client transfer for two invoices or one PayPal withdrawal for several sales; instalments ("en 3 fois") spread one purchase over months.
- **Schedules, not invoices**: energy, water and taxes paid monthly (*mensualisation*, *échéancier*) match the schedule; the yearly invoice settles the difference.
- **Refunds** come later, partial, sometimes under the processor's name; **currencies** settle at the network's rate, sometimes with a separate fee line; **dates** differ between booking and value, and debits move off weekends and holidays.
- **Equal by chance**: two subscriptions at €9.99, two coffees on one day. HomeBank's exact comparison flags such lines within one file as duplicates: lines with distinct bank ids are never duplicates of each other, and deduplication runs across sources, not within one bank's export.

## 6. Recommendations for Sioul

1. **Count from income to income, not by the calendar month.** The budgets page opens with one sentence per bank account shown: "Until your pay on Friday 28 November, the account holds every payment. €340 is left for everyday spending, about €20 a day. The lowest point is €140 on Wednesday 26, after the rent." A budget's period starts on its income's day. With no income expected: "At your usual pace, the account and the Livret A last about seven weeks." *Evidence*: 2.3; Zhang et al. 2022; Farrell et al. 2019.
2. **Replace "n % of the month gone, m % spent" with what remains, named.** A budget of fixed bills says what is paid and what is still to come ("Paid: rent, energy. Still to come: phone, €19.99, Monday 12"); one with an everyday estimate says what is left and until when ("€180 left for food until the 28th"). The verdict (better, as planned, short) stays as one sentence about the period's end, with the reserve that covers it. No percentage, colour or bar. *Evidence*: Heath & Soll 1996; Bazley et al. 2021; 4.2.
3. **One list: "What is coming".** Every movement expected over the next six weeks, or until the income after next, in date order and grouped by week: who, the amount or "about", the budget, its sources, its state in words; income as visible as bills; a month grid on demand. *Evidence*: Berman et al. 2016; 4.1; PocketSmith, Monarch, Lunch Money.
4. **Movements: one object, several sources, five states.** *Planned* (from a contract or preset: exact, "about" or a range), *announced* (a mail, letter, invoice or debit notice gave its amount and day), *not final yet* (a card hold), *done* (the bank's line), *not seen* (its day passed with no line: said once, as "missed" is now). Each keeps its sources and documents, one budget or parts across budgets, and its budget's area; a transfer between the person's own accounts is one movement with two sides, in no budget. Never "late", "overdue" or a count. *Evidence*: section 3; Plaid on holds; Sioul's rule on saying doubt.
5. **Match with keys first, then a score; never ask the person to validate.**
   - *Keys*: the bank's id within its own exports; the SEPA RUM and ICS; an invoice, order or tax reference in the bank line; Factur-X or UBL data.
   - *Candidates*: same direction, same account when known; an amount tolerance by kind (exact for debits and transfers, a few percent for card payments with tips, holds or another currency, the planned range for "about" bills); a window by kind (around the notified day for a debit, off weekends and holidays; up to 14 days after a card receipt; weeks after an invoice for a client's transfer; up to 8 weeks for a refund).
   - *Score*: amount, days apart, reference, and the payee's name normalised (bank prefixes such as "CB", "PRLV SEPA", "VIR", card numbers, dates, processors' prefixes removed) and compared by token similarity with the names its contract and sender are known by.
   - *Assignment* one to one, best score first, nearest date on a tie; three outcomes: sure, linked as "the same payment" with its sources; probable, linked as "probably the same as …" with "Not the same" beside it; otherwise apart. One-to-many only for known patterns (a deferred-debit card's sum, an order charged in parts, instalments, two invoices paid at once), as "probably together".
   - An automatic link counts until the person corrects it, and each correction teaches a name: no queue to review.
   - Consent stays where it is: a mail or letter that matches a movement already counted (a bank line, a contract's payment) joins it as a source without a question; one that matches nothing is proposed as now ("Add", "Not a payment"), and one from a sender not verified says to check it first.
   *Evidence*: Fellegi & Sunter 1969; Cohen et al. 2003; GnuCash, Actual, YNAB, Odoo; 5.2–5.3.
6. **Moving between budgets.** A movement moves or splits from its row; the page says the effect ("Home holds €58 more for November") and asks once whether this payee's next movements go there too: the person's act is the consent. *Evidence*: Cheema & Soman 2006; Zhang et al. 2022; Heath & Soll 1996.
7. **Areas filter budgets, never the facts about an account.** Budgets show by area at the hour, as decided; an account's area follows the budgets it fills. A shortfall on an account concerns every area: it is said in the next admin window and in its one reminder whatever the filter, and the page says in words what the hour leaves out ("Work budgets are kept for your work hours"), never how many. *Evidence*: 2.2; Caflisch et al. 2018.
8. **Forecast as Sioul, and say the doubt.** The forecast counts the past twelve months' irregular spending as its own line ("unusual spending, about €60 a month"); an income not yet certain is said with its condition ("if Client A pays by the 20th"), and a second line appears only when it changes the answer. *Evidence*: 2.6.
9. **Reminders come from movements.** A movement the person must act on (a bill paid by hand, an invoice to send, a notice to give) gets one reminder a few days ahead that names it and says whether the account holds it; automatic debits get none unless the account will not hold them. *Evidence*: 2.4.
10. **Work money: set taxes aside on arrival, count weeks of cover.** An income in a work budget sets aside its share for taxes and contributions at once, as a planned movement on the declaration's day; reserves are said in weeks of the person's own spending. *Evidence*: Soman & Cheema 2011; Dupas & Robinson 2013; 2.7.
11. **Words**: the table of 4.3, in English and French; never debit, credit, cleared or pending. *Evidence*: Alenazi & Sas 2023; Fernandes et al. 2014.
12. **Documents travel with the movement.** The mail's invoice, the scanned letter, the contract and the bank line show as one row's sources; a receipt that arrives before its bank line waits on the movement as *announced*, with nothing to file. *Evidence*: PocketSmith's attachment inbox; the Taskmaster lesson in [design.md](../design.md).

## 7. Sources

Read or checked on 4 November 2026; every DOI below resolved at Crossref with matching title, venue and year.

**Literature**
- Alenazi, M. & Sas, C. (2023). Evaluating budgeting apps: limited support for budgeting compared to tracking. *36th International BCS Human-Computer Interaction Conference*. https://eprints.lancs.ac.uk/id/eprint/200745/
- Bazley, W. J., Cronqvist, H. & Mormann, M. (2021). Visual finance: the pervasive effects of red on investor behavior. *Management Science* 67(9), 5616–5641. doi:10.1287/mnsc.2020.3747
- Berman, J. Z., Tran, A. T. K., Lynch, J. G. Jr. & Zauberman, G. (2016). Expense neglect in forecasting personal finances. *Journal of Marketing Research* 53(4), 535–550. doi:10.1509/jmr.15.0101
- Cadena, X. & Schoar, A. (2011). Remembering to pay? Reminders vs. financial incentives for loan payments. NBER Working Paper 17020. https://www.nber.org/papers/w17020
- Caflisch, A., Grubb, M. D., Kelly, D. et al. (2018). Sending out an SMS: the impact of automatically enrolling consumers into overdraft alerts. FCA Occasional Paper 36. https://www.fca.org.uk/node/49441
- Carlin, B., Olafsson, A. & Pagel, M. (2023). Mobile apps and financial decision making. *Review of Finance* 27(3), 977–996. doi:10.1093/rof/rfac040 ; summary: https://business.rice.edu/wisdom/can-mobile-banking-app-really-save-you-money
- Carvalho, L. S., Meier, S. & Wang, S. W. (2016). Poverty and economic decision-making: evidence from changes in financial resources at payday. *American Economic Review* 106(2), 260–284. doi:10.1257/aer.20140481
- Cheema, A. & Soman, D. (2006). Malleable mental accounting: the effect of flexibility on the justification of attractive spending and consumption decisions. *Journal of Consumer Psychology* 16(1), 33–44. doi:10.1207/s15327663jcp1601_6
- Christen, P. (2012). *Data matching*. Springer. doi:10.1007/978-3-642-31164-2
- Cohen, W. W., Ravikumar, P. & Fienberg, S. E. (2003). A comparison of string distance metrics for name-matching tasks. *IJCAI-03 Workshop on Information Integration on the Web*, 73–78. https://www.cs.cmu.edu/~wcohen/postscript/ijcai-ws-2003.pdf
- Dupas, P. & Robinson, J. (2013). Why don't the poor save more? Evidence from health savings experiments. *American Economic Review* 103(4), 1138–1171. doi:10.1257/aer.103.4.1138
- Elmagarmid, A. K., Ipeirotis, P. G. & Verykios, V. S. (2007). Duplicate record detection: a survey. *IEEE Transactions on Knowledge and Data Engineering* 19(1), 1–16. doi:10.1109/TKDE.2007.250581
- Evans, K. & Acton, R. (2017). *Fintech for good*. Money and Mental Health Policy Institute (citing Holkar, M. & Mackenzie, P. (2016), *Money on your mind*). https://www.moneyandmentalhealth.org/wp-content/uploads/2017/07/Fintech-for-good-report.pdf
- Farrell, D., Greig, F. & Yu, C. (2019). *Weathering volatility 2.0: a monthly stress test to guide savings*. JPMorgan Chase Institute. https://www.jpmorganchase.com/institute/all-topics/financial-health-wealth-creation/report-weathering-volatility-2-a-monthly-stress-test-to-guide-saving
- FCA (2015). Occasional Paper 10: Message received? The impact of annual summaries, text alerts and mobile apps on consumer banking behaviour. https://www.fca.org.uk/publications/occasional-papers/occasional-paper-no-10-message-received-impact-annual-summaries-text
- FCA (2018). Occasional Paper 40: Time to act: a field experiment on overdraft alerts. https://www.fca.org.uk/publications/occasional-papers/occasional-paper-no-40-time-act-field-experiment-overdraft-alerts
- Fellegi, I. P. & Sunter, A. B. (1969). A theory for record linkage. *Journal of the American Statistical Association* 64(328), 1183–1210. doi:10.1080/01621459.1969.10501049
- Fernandes, D., Lynch, J. G. Jr. & Netemeyer, R. G. (2014). Financial literacy, financial education, and downstream financial behaviors. *Management Science* 60(8), 1861–1883. doi:10.1287/mnsc.2013.1849
- Fernandes, M., Walls, L., Munson, S. et al. (2018). Uncertainty displays using quantile dotplots or CDFs improve transit decision-making. *CHI '18*. doi:10.1145/3173574.3173718
- Gelman, M., Kariv, S., Shapiro, M. D. et al. (2014). Harnessing naturally occurring data to measure the response of spending to income. *Science* 345(6193), 212–215. doi:10.1126/science.1247727
- Gelman, M., Kariv, S., Shapiro, M. D. et al. (2020). How individuals respond to a liquidity shock: evidence from the 2013 government shutdown. *Journal of Public Economics* 189, 103917. doi:10.1016/j.jpubeco.2018.06.007
- Gherzi, S., Egan, D., Stewart, N. et al. (2014). The meerkat effect: personality and market returns affect investors' portfolio monitoring behaviour. *Journal of Economic Behavior & Organization* 107, 512–526. doi:10.1016/j.jebo.2014.07.013
- Hannagan, A. & Morduch, J. (2015). Income gains and month-to-month income volatility: household evidence from the US Financial Diaries. NYU Wagner working paper. https://wagner.nyu.edu/impact/research/publications/income-gains-and-month-month-income-volatility-evidence-us-financial
- Heath, C. & Soll, J. B. (1996). Mental budgeting and consumer decisions. *Journal of Consumer Research* 23(1), 40–52. doi:10.1086/209465
- Howard, R. C., Hardisty, D. J., Sussman, A. B. & Lukas, M. F. (2022). Understanding and neutralizing the expense prediction bias: the role of accessibility, typicality, and skewness. *Journal of Marketing Research* 59(2), 435–452. doi:10.1177/00222437211068025
- Hsieh, J., Zhang, A., Surati, S. et al. (2025). Gig2Gether: data-sharing to empower, unify and demystify gig work. *CHI '25*. https://arxiv.org/abs/2502.04482
- Joslyn, S. L. & LeClerc, J. E. (2012). Uncertainty forecasts improve weather-related decisions and attenuate the effects of forecast error. *Journal of Experimental Psychology: Applied* 18(1), 126–140. doi:10.1037/a0025185
- Karlan, D., McConnell, M., Mullainathan, S. & Zinman, J. (2016). Getting to the top of mind: how reminders increase saving. *Management Science* 62(12), 3393–3411. doi:10.1287/mnsc.2015.2296
- Karlsson, N., Loewenstein, G. & Seppi, D. (2009). The ostrich effect: selective attention to information. *Journal of Risk and Uncertainty* 38(2), 95–115. doi:10.1007/s11166-009-9060-6
- Kay, M., Kola, T., Hullman, J. R. & Munson, S. A. (2016). When (ish) is my bus? User-centered visualizations of uncertainty in everyday, mobile predictive systems. *CHI '16*, 5092–5103. doi:10.1145/2858036.2858558
- Medina, P. C. (2021). Side effects of nudging: evidence from a randomized intervention in the credit card market. *Review of Financial Studies* 34(5), 2580–2607. doi:10.1093/rfs/hhaa108
- Murray, T. & Bond, N. (2023). *Debts and despair*. Money and Mental Health Policy Institute. https://www.moneyandmentalhealth.org/publications/debts-and-despair/
- Netemeyer, R. G., Warmath, D., Fernandes, D. & Lynch, J. G. Jr. (2018). How am I doing? Perceived financial well-being, its potential antecedents, and its relation to overall well-being. *Journal of Consumer Research* 45(1), 68–89. doi:10.1093/jcr/ucx109
- Olafsson, A. & Pagel, M. (2017). The ostrich in us: selective attention to financial accounts, income, spending, and liquidity. NBER Working Paper 23945. https://www.nber.org/papers/w23945
- Peetz, J. & Buehler, R. (2009). Is there a budget fallacy? The role of savings goals in the prediction of personal spending. *Personality and Social Psychology Bulletin* 35(12), 1579–1591. doi:10.1177/0146167209345160
- Pew Charitable Trusts (2015). Pew survey shows Americans' financial worries cloud optimism (press release, 26 February). https://www.pewtrusts.org/en/about/news-room/press-releases-and-statements/2015/02/26/pew-survey-shows-americans-financial-worries-cloud-optimism
- Shapiro, G. K. & Burchell, B. J. (2012). Measuring financial anxiety. *Journal of Neuroscience, Psychology, and Economics* 5(2), 92–103. doi:10.1037/a0027647
- Shapiro, J. M. (2005). Is there a daily discount rate? Evidence from the food stamp nutrition cycle. *Journal of Public Economics* 89(2–3), 303–325. doi:10.1016/j.jpubeco.2004.05.003
- Sicherman, N., Loewenstein, G., Seppi, D. J. & Utkus, S. P. (2016). Financial attention. *Review of Financial Studies* 29(4), 863–897. doi:10.1093/rfs/hhv073
- Soman, D. & Cheema, A. (2011). Earmarking and partitioning: increasing saving by low-income households. *Journal of Marketing Research* 48 (special issue), S14–S22. doi:10.1509/jmkr.48.SPL.S14
- Stango, V. & Zinman, J. (2014). Limited and varying consumer attention: evidence from shocks to the salience of bank overdraft fees. *Review of Financial Studies* 27(4), 990–1030. doi:10.1093/rfs/hhu008
- Stephens, M. Jr. (2003). "3rd of tha month": do Social Security recipients smooth consumption between checks? *American Economic Review* 93(1), 406–422. doi:10.1257/000282803321455386
- Sussman, A. B. & Alter, A. L. (2012). The exception is the rule: underestimating and overspending on exceptional expenses. *Journal of Consumer Research* 39(4), 800–814. doi:10.1086/665833
- Thaler, R. H. (1985). Mental accounting and consumer choice. *Marketing Science* 4(3), 199–214. doi:10.1287/mksc.4.3.199
- Thaler, R. H. (1999). Mental accounting matters. *Journal of Behavioral Decision Making* 12(3), 183–206. doi:10.1002/(SICI)1099-0771(199909)12:3<183::AID-BDM318>3.0.CO;2-F
- Zhang, C. Y., Sussman, A. B., Wang-Ly, N. & Lyu, J. K. (2022). How consumers budget. *Journal of Economic Behavior & Organization* 204, 69–88. doi:10.1016/j.jebo.2022.09.025

**Apps** (help pages; "snippet" as in section 3)
- YNAB: https://support.ynab.com/en_us/approving-and-matching-transactions-a-guide-ByYNZaQ1i.md ; https://support.ynab.com/en_us/ynab-glossary-a-guide-BJd80SORq.md ; https://support.ynab.com/en_us/add-context-to-transactions-HyCpTR4bg.md
- Actual Budget: https://actualbudget.org/docs/schedules ; https://actualbudget.org/docs/transactions/importing ; `packages/loot-core/src/server/accounts/sync.ts` at https://github.com/actualbudget/actual
- Monarch: https://www.monarch.com/flex-budgeting-simplify-your-spending-with-just-one-number ; https://help.monarch.com/hc/en-us/articles/4890751141908 (snippet) ; https://help.monarch.com/hc/en-us/articles/44244210547860-Receipt-and-Image-Imports-with-Receipt-Scanning (snippet)
- Copilot: https://help.copilot.money/en/articles/3971267-transaction-types ; https://help.copilot.money/en/articles/10761907-transactions-faq ; https://intercom.help/copilotmoney/en/articles/3760068-create-recurrings (snippet)
- Lunch Money: https://support.lunchmoney.app/finances/recurring-items/creating-recurring-items ; https://support.lunchmoney.app/finances/recurring-items/recurring-transactions ; https://lunchmoney.app/features/calendar/
- PocketSmith: https://www.pocketsmith.com/features/budgets-and-planning ; https://learn.pocketsmith.com/article/1535-using-budgets-instead-of-future-dated-transactions ; https://learn.pocketsmith.com/article/1276-attaching-files-and-photos-to-transactions ; https://learn.pocketsmith.com/article/931-attachment-inbox (snippet)
- Simplifi: https://support.simplifi.quicken.com/en/articles/3357429-using-projected-cash-flow ; https://help.simplifimoney.com/en/articles/4212702-using-the-simplifi-spending-plan (snippet) ; https://support.simplifi.quicken.com/en/articles/9174873-amount-matching-for-recurring-transactions (snippet) ; https://support.simplifi.quicken.com/en/articles/3348103-managing-transactions-in-quicken-simplifi (snippet)
- Firefly III: https://docs.firefly-iii.org/how-to/firefly-iii/finances/subscriptions/ ; https://docs.firefly-iii.org/references/data-importer/duplicate-detection/ ; https://docs.firefly-iii.org/api/storeattachment/ (all snippet)
- Goodbudget: https://goodbudget.com/blog/2025/10/match-duplicate-transactions-on-web-and-iphone/ ; https://goodbudget.com/help/automatic-bank-sync/will-goodbudget-match-my-automatically-imported-transactions-with-my-scheduled-ones/ (snippet)
- HomeBank: https://gethomebank.org/help/use-auto_addition.html ; https://gethomebank.org/help/use-import.html (snippet) ; https://answers.launchpad.net/homebank/+question/671369
- GnuCash: `gnucash/import-export/import-backend.cpp` and `gnucash/gschemas/org.gnucash.GnuCash.dialogs.import.generic.gschema.xml.in`, stable branch of https://github.com/Gnucash/gnucash ; https://code.gnucash.org/docs/C/gnucash-manual/trans-sched-slr.html ; https://code.gnucash.org/docs/C/gnucash-manual/trans-assoc.html ; https://lists.gnucash.org/pipermail/gnucash-user/2017-March/069505.html
- Bankin': https://support.bankin.com/hc/fr/articles/360027356511-Op%C3%A9rations-%C3%A0-venir (snippet)
- Linxo: https://selectra.info/finance/guides/compte-bancaire/linxo-avis (review)

**Other pages**
- Monzo, "A better way to see how your money's trending is here": https://monzo.com/blog/balance-in-trends (vendor, snippet)
- La Banque Postale, "Comment calculer son reste à vivre ?": https://www.labanquepostale.fr/particulier/accompagner/actualites-et-conseils/actus/calcul-reste-a-vivre.html (vendor, snippet)
- Consumer Financial Protection Bureau, *Your Money, Your Goals* toolkit: https://www.consumerfinance.gov/consumer-tools/educator-tools/your-money-your-goals/toolkit/ (grey, snippet)
- Odoo 18 documentation, "Reconciliation models": https://www.odoo.com/documentation/18.0/applications/finance/accounting/bank/reconciliation_models.html
- Banque de France, "Qu'est-ce qu'un prélèvement SEPA ?": https://www.banque-france.fr/en/node/2530 ; IEDOM, *Migration SEPA*: https://www.iedom.fr/IMG/pdf/bdf_migration_sepa-3.pdf (snippet)
- impots.gouv.fr, "Je découvre la facturation électronique": https://www.impots.gouv.fr/professionnel/je-decouvre-la-facturation-electronique (snippet)
- Service-public, "Compte bancaire du micro-entrepreneur": https://entreprendre.service-public.gouv.fr/vosdroits/F35991 (snippet)
- Stripe, "Statement descriptors": https://docs.stripe.com/docs/connect/statement-descriptors (snippet)
- Plaid, "Transaction states": https://plaid.com/docs/transactions/transactions-data/
- Crédit Agricole, "Carte à débit différé": https://www.credit-agricole.fr/particulier/conseils/magazine/tout-un-mag/carte-a-debit-differe-l-essayer-c-est-l-adopter.html (snippet)
- Amazon Pay, "Frequently asked questions": https://pay.amazon.com/help/69871 (snippet)
