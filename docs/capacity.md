# What a day holds: costs, gain and time in the plan

The plan learns three things from your own record:
- how long your tasks take against your first guess;
- how much a day holds for you, cost by cost;
- how much of a day should give back.

It lays each day with them, and says in words why a day holds what it holds.

Nothing is guessed from behaviour alone ([design.md](design.md)): what a day holds is learned only from the days whose outcome you gave yourself, and from the ratings you gave; a day you did not say counts for nothing. This is a planning aid. It makes no health claim. The evidence for pacing is thin, most thresholds below are informed guesses, and people differ more than any default (criterion 51 of [research/capacity-budget.md](research/capacity-budget.md)).

Each rule below is marked:
- **R**: from research. The criterion and its study are named; the criteria are in [capacity-budget.md](research/capacity-budget.md) (CB), [wellbeing-gain.md](research/wellbeing-gain.md) (G) and [time-estimation.md](research/time-estimation.md) (TE).
- **G**: a guess. A default to tune, named in the code as a constant.
- **V**: to check over time. Says which data, which signal, and when (last section).

Code:
- `crates/sioul-core/src/capacity.rs`: ratings, loads, ratios, the day's free time, budgets, the gain minimum, the balance and reasons in words;
- `demands.rs`: costs, gain, heaviness, felt ratings;
- `plan.rs`: days;
- `dayview.rs`: the day laid out;
- `timelog.rs`: the type of time records;
- `tasks.rs`: the first estimate, `set_felt`;
- `crates/sioul-app/src/capacity.rs`: the window's side.

## 1. What you give, and where it is kept

| What | Where | Written as |
|---|---|---|
| Four costs (thinking, feelings, anxiety, body and senses) and a gain, 0–10 each, unsaid until said | task panel, event form | `X-SIOUL-COST:COGNITIVE=3;EMOTIONAL=5;ANXIETY=7;BODY=2`, `X-SIOUL-GAIN:6` |
| How it was, said after the task, optional | "How was it?" on finishing, the task panel | `X-SIOUL-FELT-COST;X-SIOUL-ON=20261006:ANXIETY=3`, `X-SIOUL-FELT-GAIN;X-SIOUL-ON=20261006:6`: one rating per day, the newest five kept in the task |
| The first estimate | written by Sioul when a task first gets an estimate | `X-SIOUL-ESTIMATE-FIRST:PT30M` |
| Heavy or light | computed from the costs once one is rated, else chosen | `X-SIOUL-ENERGY:HEAVY` (no line for usual) |
| How a time record's minutes were known | the timer, the Time page | `kind = "measured" / "typed" / "corrected"` in `time/YYYY-MM.toml` |
| Dropped, apart from deleting | the task panel | `STATUS:CANCELLED` |
| How the day went: too much / about right / too empty | the end of the work day, the evening's review | `reviews/YYYY-MM.toml` ([reviews.md](reviews.md)) |
| Days learned from, even days, the start, time for you | Tasks ⚙, "What a day holds" | `[planning]` in `config.toml` |

All of it travels as the rest does:
- costs, felt ratings and the first estimate inside the task or the event, through CalDAV;
- time records, reviews and settings through the sharing's "time" and "settings" parts, sealed.

Every device holding the same record plans the same day: nothing learned is stored, it is replayed from the record each time (section 6).

**Details:**
- **Body and senses** is the fourth cost: physical effort, standing, sensory load, noise, crowds. R: CB4 (NICE NG206; Chu 2018: crashes after physical and sensory load; Raymaker 2020: sensory load in autistic burnout).
- **The first estimate is written once.** At creation when an estimate is given, or the first time a task without one gets one. Sioul never changes it. A task estimated before this existed has none, ever: its first guess is unknown, and its ratio uses the current estimate. R: TE1, TE32: a later estimate hides the gap.
- **Felt ratings are written only by "How was it?"** (`tasks::set_felt`). The form's save never writes nor clears them.
  - A value the person does not move stays unsaid. It is never copied from the forecast, which would bring back the forecast's bias.
  - R: CB6–7 (van Gog 2012; Rachman 1994: anxiety over-predicted beforehand), G5–G6.
- **Time records written before kinds existed read as "unknown"** and count as typed. The Time page says "not known how". R: TE4, TE15 (Roy, Christenfeld & McKenzie 2005: typed durations are memories; Johnson & Disney 1999).
- **A timed record whose minutes you change becomes "corrected"**: on the Time page, or when Sioul cuts a timer left running all night. It still counts as timed: the case TE13 asks to catch at the source. G: corrected weighs as timed.

## 2. Heaviness

- **A task is as heavy as its highest cost**, whichever it is, body included:
  - 3 or less: light;
  - 4 to 6: usual;
  - 7 or more: heavy;
  - "gives back" when the gain is 5 or more and no cost is above 3.
  - R: CB9 (Schmeck 2015; Redelmeier & Kahneman 1996: remembered load follows peaks).
  - G: the thresholds 3, 7, 5 (`Level::LIGHT_TOP`, `HEAVY_FROM`, `GIVES_FROM`).
- **Once a cost is rated, the word is computed**, shown rather than chosen, and written to `X-SIOUL-ENERGY` so that older versions and other views agree. A file whose word disagrees with its costs is set right at its next save. No cost rated: the word chosen stands.
- **What gives back takes no room and is offered, never planned** (as before), unless it has a date asked: a date makes it a duty, so the plan lays it like a light step. G: the date guard; without it, a dated admin task rated gain 5 and cost 2 would vanish from the plan.
- **The plan's heaviness uses the ratings it plans with** (section 3): a call forecast at anxiety 9 and felt at 4 the last five times is planned as usual.

## 3. The ratings the plan uses

For each cost and the gain, in this order (`FeltIndex::rates`):
1. the median of the last five ratings said after the same item: the same task (a repeating one keeps its own five), or tasks with the same title, accents and case aside;
2. else the forecast;
3. else, for what the forecast leaves unsaid, the median of the last five said after tasks of the same kind (call, write…);
4. else unsaid.

R: G6 (median of the last five after-ratings), CB6 (after-task ratings for learning). G: the order item, forecast, kind; five.

## 4. Load

- **The load of an item is its rating × its hours**, every item counting at least half an hour, so a ten-minute dreaded call is not free.
  - Per cost: only the costs rated.
  - In total: the item's heaviness rating, its highest cost when any is rated, else its word: light 2, usual 4, heavy 7, gives back 2.
  - R: CB10 (session-RPE: Foster 2001; Haddad 2017), CB11.
  - G: 30 minutes; 2 / 4 / 7 / 2.
- **The total is the highest, not the sum of the costs.** The research's total is the sum (CB28), itself an inference (CB-Q7). But an unrated item has only a word: with the sum, a rated item would weigh up to four times an unrated one of the same word. The total stays on the heaviness you chose: "the highest rating over all cost axes". G.
- **A day's load**:
  - tasks by the minutes worked on them that day (their records);
  - a task finished that day without any record, at its corrected length;
  - time noted for a project alone, as usual work;
  - events by their length, with their own ratings, else as usual;
  - a task's time block (an event pinning a task: [tasks.md](tasks.md), "Pinned to a time") never as an event: it is the task's time, counted with the task, by its records like any task; a block over without the task done counts for nothing;
  - meals, naps and sleep never (they are needs, G4).
  - A task rated after it that day counts with that day's rating.
  - R: CB10, CB12. G: events without ratings counted as usual (biased; said here).

## 5. How long things take: your estimate corrected

- **The ratio** is y = ln(time spent / first estimate), with all records summed (TE7, TE14):
  - on finished tasks only, never on dropped or open ones (TE14);
  - with the first estimate, else the current one (a task estimated before it was kept).
- **Weights**:
  - by days since it was finished, halving every 4.5 days (TE8: 4–5);
  - times how its minutes were known: timed (measured or corrected) 1, typed or unknown 0.25, mixed by minutes. G: 0.25 (TE15: "little").
  - Robust: y is pulled in to three scaled median deviations of its median from five tasks on, and always held within ×8 and ÷8. Real overruns stay; a timer left running all night stops ruling (TE13: Leys 2013, Huber 1964).
  - Tasks older than 90 days are not looked at. G.
- **Yours**: μ = w·ȳ + (1 − w)·ln 1.1, with w = min(1, n / 9), where n counts the weights of the tasks behind it (typed ones count little). Your ratio stands on its own from nine tasks. Before that it is pulled toward 1.1× (σ 0.5). R: TE9. G: 1.1, 9, the linear pull.
- **Per kind, pulled toward yours**: μ_k = w_k·ȳ_k + (1 − w_k)·μ, with w_k = n_k / (n_k + 5). The kind is the task's area (work, admin, leisure):
  - three groups, always known;
  - they differ in what a task is (focused work, forms and calls);
  - the task kinds (call, write…) are often unsaid and number seven or more, and dozens of kinds with two tasks each add noise (TE10, time-estimation.md 5.5).
  - R: TE10 (Efron & Morris 1975). G: 5, the area.
- **The plan lays each task at its corrected length**: your estimate × exp(μ_k), for a task with an estimate of its own.
  - You still see your own estimate; the corrected one is never written back nor pre-filled.
  - The focus timer and fog's small steps use yours.
  - R: TE16, TE32.
  - G: the correction applies to your estimate as it stands, revised or not. V4.
- **On request, in the task panel**: "Tasks like this usually take about 1.3× the first guess; the plan already allows for it."
  - Only when the kind's ratio stands on five tasks or more and differs from 1× by 15 % or more.
  - A property of the plan and of the kind, never of you.
  - R: TE25–26 (Kluger & DeNisi 1996, Hattie & Timperley 2007). G: 5, 15 %.

## 6. The day's free time

- **Each day keeps free time for steps running long**: the 85th percentile of the day's total less the sum of its steps' corrected lengths. It is unassigned, after the last step, within the day's hours, never added to a step.
  - The total comes from a simulation of 2000 days: one effect shared by the day's steps, d ~ N(0, 0.3 σ²), each step its own, ε ~ N(0, 0.7 σ²).
  - σ is 0.5 widened by √(1 + 1/n) for n finished tasks (√2 with none): thin data gets a wider interval, so a higher percentile in effect (TE20). From ten tasks, it is the robust spread of y around its kind, widened the same way.
  - The draws are seeded by the date and the steps' UIDs: the same day and steps give the same free time on every device.
- **Fitted as the day fills, never sized for steps that are gone.**
  - As each step goes into a day, the plan checks that the day still holds its steps plus their free time, cutting a long step to the part that fits.
  - The check uses the Fenton–Wilkinson form of the same model: a lognormal matched to the mean and variance of the day's total, the shared effect included. It is cheap enough to run at each step (Fenton 1960; TE18).
  - Once every step is laid, the free time kept is the seeded simulation's, on the steps actually laid. It is never more than the day has left.
  - An earlier version sized each day's free time on a first layout, then laid the days again. A long step then left the day while its free time stayed: three hours kept free for nothing, and the next step gone from today.
- **Never more than a third of a day's room.** With thin data the 85th percentile of a long step is about its own length again; the cap keeps two thirds of the day for steps. When it binds, the margin is simply smaller, and nothing is said. G: a third.
- **The next step keeps part of today.** When the free time and the slot for you would leave the plan's next step nothing today, it takes what is left of today's room, as before free time was kept, so that Now and the day agree. This holds for a started step and any other next step. Only a step that cannot be cut (its margins) goes whole to the first day that holds it.
  - R: TE16–18 (Halkjelsvik & Jørgensen 2018: add means, not medians; Trietsch 2012: activities move together), TE20 (Meeker 2017), Critical Chain's shared buffer.
  - G: P85 (TE17: 80–90), 0.3, 2000, a third, the next step's exception.
- **Today is weighed by what was finished today.** Their y less their kind's gives today's effect, pulled toward an ordinary day: d̂ = Σ r / (n + 0.7/0.3), its spread narrowed as it should. Today's free time is then simulated around d̂: a slow morning keeps more of the afternoon free. A fast morning lays nothing more. R: TE19. G: only slowing.

## 7. What a day holds: budgets

- **One budget per cost and one in total**, in load units (section 4).
- **The start**, before your days say more: "As now" by default.
  - "As now": your fullest weekday's hours of usual work plus two heavy hours fit under the 85 % fill: B₀ = (4·H + 2·3) / 0.85, H in hours. Nothing changes on day one.
  - Each cost starts at the total: a cost of an item never passes its highest, so a cost's budget binds only once your days have lowered it.
  - "Lighter" is three quarters, "much lighter" half: CB24's "about half of a recent full day", the advice to start pacing at half (#MEAction guide).
  - There is no "fuller": "as now" leaves the hours as the only limit already.
  - R: CB24. G: the formula, 0.75, 0.5.
- **Replayed, day by day, each time.** From the first day with an outcome (at most 180 days back), over the days of the record. A day without an outcome weighs nothing: it is neither light nor full. R: CB13, CB15 (Moussa 2019, REDI; Benson 2021: missing is not zero).
- **On each day with an outcome, in this order** (`capacity::learn`):
  1. **A drop.**
     - When the day was too much, look at the two days before it. A cost that had used 80 % or more of that day's budget, and was not blamed yet, falls by a fifth at once. That day is then blamed: two bad days after one full day lower it once.
     - The total is treated the same way. Never below a quarter of the start.
     - R: CB20 (NICE NG206: crashes come hours to days later; Chu 2018; Chiu & Jain 1989, multiplicative decrease).
     - G: two days back (research: 1–3), 80 %, a fifth, a quarter, blaming once.
  2. **A freeze.**
     - Three days too much within seven hold every rise back.
     - The freeze lifts after seven days with an outcome and none too much.
     - A new baseline may stay lower: nothing brings the old one back.
     - R: CB21 (NICE 1.11.15). G: the lift rule.
  3. **What full good days showed.** Fine days (about right or too empty) that used 70 % or more of their budget count, each at most 1.2 × that budget. A lighter day that went fine says only "at least this much". Their mean is weighted by days, halving every five, within the window (28 days by default, 14 to 90, your setting). R: CB15a (Tobin 1958, censored values), CB16–17 (Muth 1960; Visible's 14-day baseline), CB23. G: 70 %, 1.2.
  4. **A rise.**
     - When not frozen, toward that mean, a tenth at most, and only:
       - after seven days with an outcome, all fine, all within the window, using 80 % or more of their budget on average;
       - and a week after the cost's last change.
     - What was learned weighs in proportion to the days with an outcome in the window: full weight from fourteen days. Before that, it is pulled toward the start.
     - R: CB19 (NICE: never a fixed increase, only after stability; WHO's pacing phases: ≥7 days each; Levitt 1971; Chiu & Jain 1989), CB24–25.
     - G: a tenth a week, 80 %, fourteen days.
  5. **Never down from good days.** Good days below the budget never lower it; it falls only through the drop. R: CB15b.
- **The cap.** No day is planned above the heaviest fine day of the last 30 plus a tenth, per cost and in total, once three such days are known on that cost. R: CB22 (Frandsen 2025: a single session 10 % past the longest of 30 days raised injury). G: three days.

## 8. The plan's days

- **Each day is filled up to 85 % of each budget and of the total**, after what its events already hold. R: CB27–28. G: 85 % (CB-Q18).
- **Today goes by its weather**, which only ever lowers it (CB37):
  - clear 100 %, haze 60 %, fog 30 %, of both its room and its budgets;
  - fog stays the day said bad. CB36's bad day is about 60 %, but haze already is, so fog goes lower, as its room already did.
  - G: fog 30 %.
- **After a pause** ([pauses.md](pauses.md)): the rest of today, and tomorrow if you ask, hold no more than a hazy day, 60 % of the room and the budgets and one heavy step, the morning's weather unchanged (`pause::today_level`). R: CB36 (the bad day, about 60 %), P23.
- **Free time's moved end** ([pauses.md](pauses.md)): today's room grows by the stretch the end of work moved, for light steps only, laid there first; the budgets do not grow (hours, not capacity). R: GP9, GP12–GP13; O: light steps only.
- **The days before and after an event whose highest cost is 7 or more** take one heavy task fewer and hold three quarters of their budgets. R: CB34 (#MEAction "radical rest"; NICE 1.11.4 pre-emptive rest; Chu 2018). G: 7, three quarters, one fewer.
- **A step too heavy for what the day still holds** goes to a later day. A long step that can be cut takes the part the budget lets in, a quarter of an hour at least. A step heavier than an ordinary empty day's budget gets a day of its own: the first day with no other task.
- **A task with margins is never cut**, owner's decision:
  - the plan lays it whole on the first day whose room holds it with its margins;
  - longer than any day's room, it gets a day of its own;
  - the day lays it whole in a gap that holds it, else not today.
- **A task pinned to a time** (its time block, an event: [tasks.md](tasks.md), "Pinned to a time") is laid on its block's day first, whatever the room or the budgets: the block's time, its margins and a pause around it leave that day's room once, as an event's would, and the task is counted there at its block's minutes, its own load and heaviness, never an event's besides. Laid first, it is what each other step of that day is weighed against: the budgets, the heavy steps a day takes and the free time kept for steps running long all see it (the free time grows with its minutes too). Longer than its block, its rest goes on from the next day. The time budget until a date asked counts its block once (`plan::cushion`). A step given a time today by a drag before blocks (`X-SIOUL-AT`, read still) stays today, at its time, as before.
- **Half an hour of each day with two hours of room or more** is kept for the time-for-you slot after the day's costliest block (section 9). G.
- **Even days**, off by default.
  - With them, the plan is made once to see the coming week's load, events included, then again with each day held to the week's mean on each cost, never above 85 % of its budget.
  - For energy-limiting illness, where a full day then an empty one is the boom and bust to avoid.
  - R: CB35 (NICE; Andrews 2012). G: the seven-day mean.

## 9. The day laid out

- **Order**:
  - never two heavy steps in a row: when the next step in the plan's order is heavy after a heavy one, one of the next three that is not, and is free to go, comes first;
  - two steps weighing most on the same cost (4 or more) are kept apart the same way;
  - nothing goes before what it waits for.
  - R: CB29–30 (Meijman & Mulder 1998; Albulescu 2022; NICE 1.11.4 "alternate and vary"; de Jonge & Dormann 2006). G: three, the dominant cost from 4.
- **Breaks**: after a heavy step, a light one or a break of a quarter of an hour. A usual or heavy step waits fifteen minutes past it; a light one only the usual five. R: CB29 (Albulescu 2022: more than ten minutes after demanding tasks). G: 15.
- **Time for you**: two slots a day, half an hour each, or a quarter of an hour when that is all a gap holds.
  - The first comes right after the day's costliest block, a task or an event (by load, section 4), within its hours when they hold it, else in the free time after it. It is looked for within two hours.
  - The second comes in the evening: the first free gap after today's hours (18:00 on a day without hours), before the night.
  - They are yours to fill or leave empty: shown "Time for you", with at most one line ("Perhaps: Walk by the river") taken from your own items said to give back well (felt gain 6 or more, no felt cost above 3), rotated by day.
  - They are quiet: the messages of sites and the pauses to move wait (`hours::quiet_slot`). Doses, meals and sleep, codes asked for, calls and an event's alarm still come.
  - Off in Tasks ⚙.
  - R: G11 (de Bloom 2017), G15 (Fredrickson 2000), G18–18b (Howe 2022; Killingsworth & Gilbert 2010), G19 (van Roekel 2017), G21.
  - G: 30 and 15 minutes, two hours, 18:00, what stays audible, 6 and 3, the rotation.
- **The free time kept** (section 6) shows after the last step: "Kept free, in case steps take longer". Never a countdown nor a target (TE24).
- **Why**: at most two lines above the day, only when a rule changed it:
  - "Today holds a little less: yesterday was too much after a full day."
  - "Today holds a little less: the hearing is tomorrow." / "…after the hearing yesterday."
  - "Days hold a little more (or less) than two weeks ago", only past 30 %.
  - R: CB45, CB48.

## 10. What gives back: the minimum

- **Good hours of a day**: Σ gain / 10 × hours over its items with a gain (said after, else forecast), each counting two hours at most. Slots left empty are not counted in the record: whether they were kept is not known. R: G8. G: two hours; slots not counted.
- **The minimum**:
  - the 25th percentile of good hours over good days (about right) in the window, weighted by days, halving every five;
  - held between half a good hour and four;
  - one good hour while fewer than five good days are known.
  - It is never learned downward from a bad stretch: a lower value waits while three of the last seven days with an outcome were not about right.
  - R: G9–G11 (Sonnentag 2018, the recovery paradox), G10.
  - G: the 25th percentile, the bounds, one hour, the stretch test.
- **Used for**:
  - the evening's balance (below / around / above it, in words, never shown as a number);
  - the slots, which exist from the first day whatever the minimum.
  - G: "around" is within a quarter.
- **The day's balance** (`capacity::Record::day_balance`, for the evening's review):
  - each cost and the total: "light" up to half of the budget, "heavy" above it, "usual" between; "" when nothing of it was recorded that day (CB12). A day said too much is never called light;
  - the gain: below, around, above, or "" when nothing carried a gain;
  - at most two lines on what the plan does next ("Tomorrow holds a little less: …").
  - G: half, the whole.

## 11. Never shown

- No score, no percentage, no number of load, budget or good hours, no gauge, no fuel tank, no red, no "over budget" (CB45–49, G20).
- No ratio as a grade: "about 1.3×", on request, as a property of the plan (TE25, TE31).
- No "late", no count of what was not done, no streak (CB47).
- No suggestion of social activity by default, no "good for you" (G21–22).
- Nothing inferred about a condition from the record (TE30).

## 12. To check over time

Each check names its data, the signal to look at, and when to look.

1. **The 85 % fill and the start.**
   - Data: replayed loads against budgets, and the reviews.
   - Signal: the share of days said too much.
   - When: after four weeks of reviews. Above about one day in five without drops settling, lower the fill or the start.
2. **Drops and rises.**
   - Data: `Learned::changes`.
   - Signal: a drop, a rise and a drop again within two weeks, repeated: boom and bust inside Sioul.
   - When: after six to eight weeks.
3. **Felt against forecast.**
   - Data: pairs of forecast and felt rating per cost.
   - Signal: the mean gap (anxiety expected over-predicted).
   - When: from twenty pairs. A large, stable gap would justify correcting forecasts per kind (CB7, not built).
4. **The ratio.**
   - Data: finished, timed tasks.
   - Signal: the share of days whose real total stayed under the day's 85th percentile, expected near 85 %; the error of the median length (pinball loss).
   - When: from thirty tasks. Also whether correcting a revised estimate over-pads.
5. **Typed against timed.**
   - Data: the share of typed minutes.
   - Signal: mostly typed means the ratio stays near its prior. That is a fact to say in the guide, never a nag.
6. **The gain minimum.**
   - Data: good hours on about-right and too-empty days.
   - Signal: do they separate?
   - When: after five good days in four weeks.
7. **Time for you.**
   - Signal: whether the slots are turned off, or the plan's days felt fuller.
   - When: weekly. It is the only observable.
8. **Unrated defaults.**
   - Data: felt ratings of items that were unrated before.
   - Signal: their median highest cost against 2 / 4 / 7, and events against 4.
   - When: from ten ratings per word.
9. **Blame window and 70 %.**
   - Data: bad days and the loads of the three days before.
   - Signal: which day's load best predicts them.
   - When: after twenty bad days, if ever.
10. **Free time's cap.**
    - Data: days where the third bound the free time, and those days' outcomes.
    - Signal: whether days whose free time was cut end "too much" more often than others.
    - When: after four weeks of reviews and twenty timed days.
11. **Speed.**
    - The record is replayed and two calendar spans read each time the plan is made (events kept five minutes).
    - Signal: a slow Tasks page on a long record. If so, keep the replay per day.

## 13. Open

- **Good hours, not built.** The hardest and most anxious steps early in your own good hours (CB31) waits for research. It is not approximated by your working hours. Open:
  - who declares good hours, and how they move with an illness and its treatment;
  - chronotype against fatigue rising through the day (Kratz 2017);
  - the conflict between hard first (KC 2020, Berns 2006) and a small easy start for ADHD initiation (CB-Q8).
  - See [roadmap.md](roadmap.md).
- **Correcting forecasts per kind** with the felt-against-forecast gap (CB7): not built, V3 first.
- **A weekly minimum** for meaning, connection and time outdoors (G-Q3): not built.
- **Rating rarely**: asking "How was it?" the first few times, then about one time in five (G5), is the panel's to pace. Here every rating counts the same.
- **Per-weekday budgets** (CB-Q17): one budget for every day today.
- **Events sent by others carry no ratings**: counted as usual, which biases the load.
- **Ratings may not survive other applications** that drop unknown properties when they save; to test with Nextcloud Tasks and others.
- **Nothing here has been tested with Sioul's users.** Whether it lightens admin is unknown.
