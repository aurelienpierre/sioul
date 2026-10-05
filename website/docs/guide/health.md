---
description: Health in Sioul - medicines reminded once and quietly, prescriptions and their errands, meals, rest and sleep kept free, what a watch measured without any score, pauses to move, and a daily limit on chats.
---

# Health

Medicines to take, prescriptions to renew, meals, rest and sleep kept free in your plan, a pause to move, a limit on chats. Kept on this computer; nothing counts what was missed, nothing turns red.

<figure markdown="span">
  [![The Health page: a sentence saying medicines, prescriptions and doses stay on this computer; today's doses, one marked "Taken at 07:41", the others each with Taken; the medicines, each with when it is taken; a prescription, with when to go to the pharmacy and when to renew it, "Fetched today", and the list errands go to; then the pause to move while focusing, every 45 minutes.](../assets/screens/health.png){ loading=lazy }](../assets/screens/health.png "Open the picture at full size")
  <figcaption>Today's doses first, then the medicines and prescriptions.</figcaption>
</figure>

The page puts the most needed first: doses due while Sioul was closed, then today's doses, then the medicines and the prescriptions, then what your watch says (once one is set up), then the pauses.

## Medicines

**Add a medicine**: its **Name**, its **Dose** ("one tablet"), and **When**:

- **At set times each day**: "08:00, 20:00";
- **Every few days**, at a time, from a day: "every other day at 08:00";
- **Every few hours**, from a time: "every 6 hours, from 18:30".

Then **Until** a day, or for as long as it goes; the **Prescription** it comes from; **Paused for now**.

### Today

Each dose of the day, with **Taken**. A dose taken says when; one click takes it back. A dose not marked is simply not marked.

**Taken late**: more than half an hour past its time, **Taken…** asks when you took it (now, unless you change it), or lets you say it was not taken.

**Two kinds of medicines.** Those taken at set times of the day keep their times. Those taken every few hours keep the hours between two doses, which the body needs to clear one before the next: each dose you mark, on time, early or late, sets the next one that many hours after it. Taking the mark back puts the doses back.

### Reminders

One desktop notification per dose, within half an hour of its time, without sound, with **Taken**. Never repeated. Reminders come in quiet time too: they are yours.

If Sioul was closed at the time, a dose of the last twelve hours that was neither marked nor reminded is asked about on the Porch at the next start, and on this page: with **Taken…** (when you took it) and **Not taken**. It is a question about the past, never a reminder to take one now.

### On several computers

Only the computer you are at reminds you. A dose marked taken on one goes to the others at once. For that, share between your computers ([Sharing](sharing.md)); without it, the page says the doses are known to this computer only.

**When Sioul can't tell.** A dose taken twice can harm, so Sioul never says a dose was not taken unless it knows. Your devices exchange through a folder another program syncs, sometimes late: a phone's sync can bring files only every half hour. When Sioul has not heard from one of your devices since the dose was due, or part of what it wrote could not be read, or this device's own record of doses could not be read, it says so under the dose, on the Porch and on this page: "Sioul can't tell whether it was taken: your laptop was last heard on Monday 5 October at 07:52. Check before taking it." A reminder in that case first waits a few minutes for news, then comes titled **Check first**. Look at the other device, or at your pill box, before taking the dose.

## Meals, rest and sleep

Times kept free, set before any work: Sioul plans no task in them, and plans the rest of the day around them.

- **Meals**: three to begin with (breakfast, lunch, dinner), each at the time you choose, with the minutes it takes to get it ready and to eat. **Add a meal or a snack** for more. If two meals are planned more than four hours apart, it is said there, quietly.
- **Naps**: when, how long, and a few minutes to come back after it.
- **The night**: bedtime, waking, and the time to wind down before bed.

Name each as you like, choose its weekdays, and turn its notices off if you would rather not have them: it stays kept free.

**Today**: today's meals, naps and night come first on the page, each with **15 min later** (as often as you like), **Move to…** a time, and **Not today**, for today only, without a word asked. A meal that would fall in an event moves after it by itself, today only, with the time to come back from it (the event's **After**); never earlier than you moved it.

**Notices**: two at most for each, each once. First, a quarter of an hour before (you choose), "No new big task" with its name and time, so you do not start something you would have to leave. Then one at the time. Each has **Options…**: later, at another time, not today, and one line on where you stopped, shown again when you are back. Moving a meal never brings more notices. Nothing during a meeting, nothing for one not today, nothing when one passes. A notice shows only a name and a time.

Nothing about what you eat or how you sleep is asked or recorded: no counts, no history. The way it works follows research on how people who struggle with eating want to be invited to eat ([the research notes](../dev/research/meal-prompts.md)).

## Prescriptions

**Add a prescription**: what it is for, who wrote it, until when it is valid, how many days the pharmacy gives at a time, and when it was last fetched.

Then Sioul makes the errands, once each:

- two days before the medicines run out, a task "Pharmacy: …";
- two weeks before the prescription ends, a task "Doctor: renew the prescription for …".

They go into the list you choose under **Errands go to**, so that your phone has them. **Fetched today** counts the next visit from today.

!!! note "Errands are tasks"
    The errands' titles name the medicine, and they go to your task list, on your calendar server when the list is there. Choose a list on this computer only if you would rather keep them here.

## Your watch

What a Garmin watch measured, read from its own files, never through a Garmin account: nothing goes to a server.

**To set it up**, in the Health page's ⚙, **Its files come to**: a folder where Gadgetbridge's exports (the open companion app for Android) or Garmin's export files arrive. Or plug the watch in: when your desktop shows its `GARMIN` folder, Sioul reads it.

The page then says, in words: when the watch last gave data, the day's steps, the resting heart rate with its usual, last night's sleep (its length and its hours), Body Battery, the week's averages; then today's curves, plain. No goal, no streak, no score, and no colour as a grade.

**Gentle offers between tasks**, on unless you untick them: a pause after sitting long, a short break, a walk when there is room for one, or calling it a day when the reserve is low. Only at a natural stop (a task done, a focus session ended), never in quiet time, at most six a day, and an offer you declined rests longer each time. A notification about it says no number.

In the morning, the Tasks page may say one line, never a notification: after a short night, "Shorter sessions today, and the hardest task early, or tomorrow?"; when the resting heart rate stands well above its usual, "Your body may be fighting something. A lighter day?". Each comes with **A lighter day**, which sets [the day's weather](tasks.md#how-is-today) to haze or fog.

## Moving

**A pause to move**, every 45 minutes unless you change it: a quiet notification says it is time to move and stretch, even with the window hidden. During a focus session, the pause is offered, never imposed: **Pause now** pauses the session and lets you note in one line where you stopped; **Not now** goes on, and asks again later. If you miss it, the session keeps counting. **Back to it** starts it again.

## Chats

**A limit a day on chats**, off unless you turn it on. Once the minutes you chose are used (counted while a chat is in front of you), the chats in [Sites](sites.md) are covered, muted and silent, for the time you chose. Then they come back by themselves, whatever happens.

## Where it is kept

On this computer, in two files of Sioul's own folders: what you enter, and the doses marked. They go nowhere, unless you share between your computers: then they travel sealed, through your own synced folder ([Sharing](sharing.md)).
