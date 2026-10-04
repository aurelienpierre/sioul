# Areas and hours: what comes when

Two questions decide what Sioul brings forward, and they are separate:
- **Who may reach you**: the senders' lists, safe, neutral, blocked ([porch.md](porch.md)). That is about people and communication.
- **What a thing is for, and whether these hours are for it**: areas and hours, this page. That is about sources (an address, a site, a chat) and tasks.

They meet in one place. Mail from a sender you marked safe, and a verified code, always come, whatever the hours. Everything else comes when its area fits the hours.

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

Mail addresses are the one place nothing is mixed for you: asking what each address is for is what separates a business address from a personal one.

### Hours of several kinds at once
Hours may overlap: admin hours inside free time on a Saturday afternoon, say. Then what each of them brings comes, all together (`Time::Several`): your admin and your leisure, not work; the status line says "Admin and free time until 16:00", the first of them to close.

### "Work now"
A box beside "Real time" on the Porch, and in the status line's menu on every page: work shown whatever the hours, as in working hours. It ends when you untick it, when Sioul closes (Sioul takes it back on starting too, after a crash), or once the next working day is over: today's working hours when they are not over yet, else the next working day's, past any time off; without working hours, at midnight (`quiet::end_of_next_workday`, `Overrides::work_now`). The status line says until when.

### When no hours are set
The Porch asks, in a card, for the hours not set yet (work, your admin, free time), with "Set my hours", which opens Settings at them, and "Leave as is", which stops asking.

## Hours: work, admin, free time, and the rest
Settings ▸ Hours has three weeks: **working hours**, **hours for your admin**, and **free time**. Each day of each can be on or off, from a start to an end (`[[window]]`, with `kind = "admin"` or `"leisure"`; working hours have no kind). Hours set for none of them are **personal time**: not work, and no assumption on admin or leisure. Time off and a day closed early ("Done for today") are free time. "A little longer" is work time.

| Hours | Comes forward | Waits |
|---|---|---|
| Work | work; admin too, until admin has hours of its own; a call to an office, always (offices keep business hours) | leisure |
| Admin | admin; work too, while work has no hours | leisure |
| Free time | leisure, and nothing else | work and admin, set or not |
| Personal time (evenings, days off) | admin and leisure; work too, while work has no hours | a call to an office (closed) |
| No hours set at all | everything, as before any were set | nothing |

`sioul_core::areas::in_view` is the whole rule. `sioul_core::quiet::mode` says what the hours are now and until when; the status line says it: "Admin time until 19:00: offices, bills, letters."

### Mail
Codes and safe senders always come (`quiet::mail_in_view`). Otherwise mail comes when its address's area fits the hours. In free time, an address that is also for admin or work shows only what your safe senders write, because the rest of it may be a bill or a client. Budgets follow the same rule, by what each is for (your admin when unsaid).

### Sites
The sites for these hours are listed; the others fold under one line, "For other hours: 3", opened with a click. A site's notifications wait until a gathered notification falls in its hours (see [sites.md](sites.md)). A site in real time, and a call, come at once, but only while its area fits the hours.

### Tasks
The task pages keep what fits (`quiet::QuietTasks`). The next step is never one that waits for something still open, and never a call to an office that is closed now. A task needing an office uses its office's own opening hours when it has them ([tasks.md](tasks.md)).

## What is not decided for you
- **A bank account used for both.** It is for work and admin: it comes in work time and in admin time, not in free time. Sioul does not split its movements between business and personal. Your accountant decides that, and in France a micro-entrepreneur needs a separate account once turnover passes €10,000 two years running. Keeping the business account apart keeps its alerts out of your evenings.
- **Admin hours inside office hours.** If your admin hours are all in the evening, calls to offices still come in working hours, when offices answer. Give admin a slot during the day (Tuesday 14:00–16:00) if you would rather keep work time for work.
- **No hours at all.** Nothing changes until you set some. Setting only admin hours sets nothing aside for work: work still comes outside them, except in free time.
