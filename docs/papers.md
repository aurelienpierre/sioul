# Papers

The same papers are asked again and again (an identity card, the last tax notice, bank details, rent receipts less than three months old, a health attestation), and an expired one blocks what depends on it: a lease, a cover, a trip. Remembering that a passport ends in March is time-based remembering, the kind that fails (Landsiedel et al. 2017). So Sioul keeps the papers, says where each stands, and reminds when renewing should start.

Code: `crates/sioul-core/src/papers.rs`, `crates/sioul-app/src/papers.rs`, `qml/PapersPage.qml`, `qml/PaperDialog.qml`.

## Where they live
`sioul-papers.toml` at the root of the case store, the files in its `papers/` folder: they travel with your projects and notes, by the same sync. A file chosen elsewhere (a scan in Downloads) is copied there when the paper is saved, under a free name.

```toml
[[paper]]
id = "passeport"
kind = "passport"
title = "Passeport"
file = "papers/Passeport.pdf"
issued = 2016-12-21
until = 2026-12-20
holder = ""          # in a household: whose
renewal = "…"        # the task made to renew it
added = 2026-10-03
```

## What each kind knows
| Kind | Renewing starts | Usually asked |
|---|---|---|
| Identity card, passport | 3 months before it ends (an appointment, then the making); an end proposed 10 years after the issue (adults, cards made from 2021) | |
| Residence permit | 4 months before | |
| Driving licence | 2 months before | |
| Health cover (complémentaire santé solidaire, mutual) | 3 months before: the CSS does not renew by itself (2 to 4 months before, ameli) | |
| Insurance certificate | 1 month before | |
| Warranty, proof of purchase | 1 month before; an end proposed 2 years after the purchase (the legal guarantee of conformity) | |
| Rent receipt, payslip, attestation | | less than three months old |
| Tax notice | | the last one (about a year) |
| Bank details, health card, other | | |

- **Where it stands**, in words, never in red: "valid until 20 December", "valid until 20 December: time to renew it", "ended on …", "from 30 June, older than what is usually asked". A date in another year carries its year.
- **The reminder** ([reminders.md](reminders.md)): once, when renewing starts, at the next working hours: "Passport: valid until Sunday 20 December", with what renewing takes. Not for a paper added after renewing began (you just saw it), nor once its renewal is planned.
- **Plan the renewal**: a task in your usual list (your phone has it), from when renewing starts (or today) to the day it ends, tied to the paper, with the steps known for its kind (France: the pre-request on ants.gouv.fr and a town hall that takes them; the ANEF site for a residence permit; ameli for the CSS).

## How papers come in
- **From a mail**: "Keep in papers" beside an attachment: the antivirus checks it (or, without one, you are asked, as for opening), it is copied into `papers/`, and the form opens with a name and a kind guessed from the file's name and the subject ("avis_impot_2026.pdf": a tax notice; "quittance": a rent receipt); you say the dates.
- **From a file**: "Add a paper" (or New ▸ A paper), then its file: a scan, a photo.
- Nothing is typed twice: the form asks only what cannot be guessed. Reading the dates on a scan comes with letters read by OCR (next).

## Going out
- **In a mail**: "A paper" beside "Attach" in the writing window: the papers with a file; one older than what is usually asked, or ended, says so in the list.
- Taken out of the wallet, a paper's file stays where it is.

## Tested
`cargo test -p sioul-core papers`: where each kind stands (within its renewal time, valid, ended, old, fresh, undated), ends proposed from the issue, kinds guessed from names, a wallet kept and read again with its files under free names. `reminders`: a passport's renewal reminder. In the window: the page with five papers, the form, a renewal planned (the task in the list, tied to the paper).
