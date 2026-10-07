# Areas and hours: what comes when

Two questions decide what Sioul brings forward, and they are separate:
- **Who may reach you, and when**: five states, the same for mail, calls and other apps' messages (strangers, in none of your address books and on no list; the blocked, never; and for the people in your address books safe, neutral, restricted), your Always through people, and the one matrix of what reaches you, a row per state on each channel ([attention.md](attention.md)). That is about people.
- **What a thing is for, and whether now is for it**: areas and the five times, this page. That is about sources (an address, a site, a chat) and tasks.

They meet in one place, mail: your safe senders' mail comes at the times ticked for them, to any of your addresses; the others' mail (neutral, restricted, strangers) comes at the times ticked for their row, to an address for what now is for. Calls and other apps' messages follow their own matrices alone. The codes and links you just asked a site for, and what you send yourself, come at once, whatever the time.

This is a health matter before it is a filter. Work that reaches the evening keeps people from recovering, and detaching from work after hours is what recovery needs most (Sonnentag & Fritz 2007, 2015). The risk is highest for people who work from home or for themselves, for whom nothing else marks the end of the day. Admin left to spread over every evening weighs too, even when nobody is working.

## Areas: three switches
A source or a task is for **work**, for **your own admin** (bills, letters, offices, health errands), or for **leisure** (friends, family, chats, what you enjoy). These are switches, not a choice of one: an address for everything personal is for admin and leisure; a freelancer's bank account, used for both, is for work and admin; a chat used for work and for friends is for both.

`sioul_core::areas::Area`, written in files as `work`, `admin`, `leisure`, joined by `+` (`admin+leisure`). The older words still read: `personal` is admin and leisure, `mixed` is work and admin.

| Thing | Where its area is set | Unsaid |
|---|---|---|
| A mail address | Accounts, on its card: "What this address is for" (`[[account]] area`) | work: an address you did not classify never reaches your evenings |
| A budget | its form ("Edit"): "For" (`[[budget]] area`) | your admin; the older `personal` reads as leisure |
| A site | the site's ⋮ ▸ For (`area`) | by its type: secure mailboxes and client areas (offices, banks, suppliers) are admin; chats, social networks and dating, leisure; video calls and other sites, admin and leisure |
| A task | the task's panel, "For" (`X-SIOUL-AREA`) | by its tags and projects: work's (`[quiet] work`, a client's project, GitHub), leisure's (joy, family, friends…), both personal ones (health), else admin |
| An app's notifications, on a phone | Settings ▸ What reaches you ▸ Exceptions, under the app: "For" ([android.md](android.md#notifications-from-other-apps)) | any time: the matrix decides alone ([attention.md](attention.md)) |

Mail addresses are the one place nothing is mixed for you: asking what each address is for is what separates a business address from a personal one.

## Five times
What now is for is one of five times (`sioul_core::areas::Time`, decided by `sioul_core::quiet::mode`):

| Time | What it is | Set where |
|---|---|---|
| **Work** | working hours; "A little longer" and "Work now" too | Settings ▸ Hours |
| **Admin** | hours for your own admin | Settings ▸ Hours |
| **Meals** | each meal, from getting it ready to its end | Health: meals, and the day's changes ([health.md](health.md)) |
| **Sleep** | the night, from winding down to waking; each nap with its minutes to come back | Health: the night, naps |
| **Leisure** | every other time: evenings, days without hours, time off, a day closed early ("Done for today"), Free time | nothing to set |

**Which comes first**, when they overlap: the pause, then sleep, then Free time, then meals, then the hours ([pauses.md](pauses.md)). The pause (« En pause ») holds everything Sioul shows, whatever the time, as sleep does (`Reason::Paused`), with its own column in the matrices of who may reach you; Free time (« Temps libre ») is leisure whatever the hour, until you come back or the night begins, and the working time it took moves the end of today's work later, within limits (`Reason::FreeTime`, `Reason::Extended`). A meal inside working hours is a meal while it lasts. "Work now" may last over a night: the night stays sleep. "Done for today" is leisure until work comes back; "A little longer" and "Work now" are work. Time off is leisure. Work and admin hours at once bring what each of them brings, all together (`Time::Several`), until the first of them closes.

**The hour before bed is sleep's.** Health keeps the wind-down free, and the programmes it comes from make that hour free of what keeps the mind going (Harvey et al. 2021): a work message or a notification is exactly that. The status line says it apart: "Winding down: nothing disturbs until 07:00." The night's own notice at its time, the start of the wind-down, still comes: it is the one that says the night begins, and it offers to close the day ([reviews.md](reviews.md)). From the wind-down, the status line offers nothing more.

**A block taken out for one day** (off that day, on the Health page) is no meal or sleep that day; one made quiet that day (no notice) still is.

**No working or admin hours at all**: everything comes, as before any were set (`Time::Any`); meals and sleep keep their time when Health sets them. Leisure lets only your safe senders through: someone who has not set hours yet would see no other mail. Mail, calls and other apps' messages read such a time as working and admin hours together: the more open of a row's two cells (`attention::Now::of`, [attention.md](attention.md), Q14).

**No night set**: nights are leisure, and nothing keeps notifications away while you sleep. The Porch asks for it in a card, "Set my night" (the Health page, at its meals and night) or "Leave as is", which stops asking.

### What each time brings

| Time | Comes forward | Waits |
|---|---|---|
| Work | work; admin too, until admin has hours of its own; a call to an office, always (offices keep business hours) | leisure |
| Admin | admin; work too, while work has no hours | leisure |
| Leisure | leisure, and nothing else | work and admin |
| Meals | leisure: a meal is a break | work and admin, tasks for them |
| Sleep | leisure, if you open Sioul; no notification of any kind but doses | tasks, projects, time behind one sentence; every notification |
| No hours set at all | everything, as before any were set | nothing |

`sioul_core::areas::in_view` is the whole rule for areas, the attention model's `Attention::mail` for mail ([attention.md](attention.md)); the status line says what now is and until when: "Admin time until 19:00: offices, bills, letters.", "Meal until 13:00.", "Leisure until 22:00: what you enjoy.", "Sleep: nothing disturbs until 07:00.", and for the two pauses "Free time: only your safe senders, doses and codes reach you. Work comes back when you do." and "Paused."

### "Work now"
A box beside "Real time" on the Porch, and in the status line's menu on every page: work shown whatever the hours, as in working hours. It ends when you untick it, when Sioul closes (Sioul takes it back on starting too, after a crash), or once the next working day is over: today's working hours when they are not over yet, else the next working day's, past any time off; without working hours, at midnight (`quiet::end_of_next_workday`, `Overrides::work_now`). The status line says until when. Meals and sleep still come first.

### When no hours are set
The Porch asks, in a card, for the hours not set yet (work, your admin), with "Set my hours", which opens Settings at them, and "Leave as is", which stops asking.

## Hours: work and admin
Settings ▸ Hours has two weeks: **working hours** and **hours for your admin**. Each day of each can be on or off, from a start to an end (`[[window]]`, with `kind = "admin"`; working hours have no kind). One sentence says that leisure is every other time, meals and sleep coming from Health, with a button to them. Time off and a day closed early are leisure; "A little longer" is work.

**Leisure is not set.** Leisure is every time not given to work, admin, a meal or sleep; "Free time" is now the name of a pause you take ([pauses.md](pauses.md)). Hours an older Sioul set as leisure (`kind = "leisure"`, its "free time") stay in the file, untouched, read without a word and left aside (`Config::week_hours`); saving the other weeks keeps them.

### Mail
Who may reach you when is the one matrix of [attention.md](attention.md): for mail, a row per state, strangers included, and Always through above them. The address it came to narrows it (`Attention::decide`, step 8): your safe senders' mail, and your Always through people's, comes to any address; the others' (neutral, restricted, strangers) only to an address for what now is for, unless the two never meet in your week (a stranger writing to an address for leisure, strangers coming only in work and admin time; a restricted sender writing to your personal address): then their row alone decides, so that no mail waits for good. While admin has no hours of its own, working hours take its cells too; while work has none, admin hours take work's, as areas are lent. Budgets follow the areas, by what each is for (your admin when unsaid).

### Sites
The sites for now are listed; the others fold under one line, "Other hours: 3", opened with a click. A site's notifications wait until a gathered notification falls in its hours (see [sites.md](sites.md)). A site in real time, and a call, come at once, but only while its area fits now, and never while you sleep.

### Sleep
While you sleep (`Mode::sleeps`): the Porch shows codes and what your lists let through now, a message about a project among the people you know, no money and no paper letters; every address rests on the Mail page; only leisure sites are listed; Tasks, Projects and Time wait behind the status line's sentence and **Show anyway**, Tasks keeping a field to note a thought for later. **Almost nothing notifies**, as usual (the matrix's sleep column, [attention.md](attention.md)): no site, no new mail (your safe senders' is shown, not told), no message, no call but your Always through people's, no reminder before a date (but an event's own reminders when the event itself is in the night: you chose it, [reminders.md](reminders.md)), no pause to move, no meal or nap notice. The codes you just asked for come (you asked, and a code lasts minutes); **On the Porch only** keeps them there. What waits comes at waking: reminders while they still make sense, new mail that waited ("The Porch opens: …"), sites' notifications at the next gathered time. Dose reminders come all the same: you set their times, and a dose at 05:00 is meant to wake you. Settings ▸ What reaches you ▸ Sioul's own, the doses' sleep cell: At once (the default) or Later; later, a dose's reminder comes at waking, and so does the question on doses due while Sioul was closed. Sioul never silences a dose without that choice.

### Tasks
The task pages keep what fits (`quiet::QuietTasks`): what is for leisure in leisure and during a meal, none during sleep. The next step is never one that waits for something still open, and never a call to an office that is closed now. A task needing an office uses its office's own opening hours when it has them ([tasks.md](tasks.md)). The plan gives tasks room in working and admin hours; leisure has no hours, so what is only for leisure takes no room and waits for none.

## What is not decided for you
- **A bank account used for both.** It is for work and admin: it comes in work time and in admin time, not in leisure. Sioul does not split its movements between business and personal. Your accountant decides that, and in France a micro-entrepreneur needs a separate account once turnover passes €10,000 two years running. Keeping the business account apart keeps its alerts out of your evenings.
- **Admin hours inside office hours.** If your admin hours are all in the evening, calls to offices still come in working hours, when offices answer. Give admin a slot during the day (Tuesday 14:00–16:00) if you would rather keep work time for work.
- **No hours at all.** Nothing changes until you set some. Setting only admin hours keeps work in them (work comes in admin hours while it has none of its own); outside them, leisure.
