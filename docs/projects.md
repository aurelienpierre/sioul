# Projects, time and invoices

A project is any matter you follow, one list for all of them (`sioul-projects.toml` in your notes folder, [notes-folder.md](notes-folder.md)): the umbrella over its tasks (REFID), events, mail (its routes), notes, time and budget lines. Projects were once "cases" beside "projects"; they are one thing, with one name, and "for a client" is a choice in its form. What was written under the old name still reads ([notes-folder.md](notes-folder.md#its-first-name)). Made, renamed, given its routes and taken out from the Projects page (its form, "Its mail"); taken out, its tasks, notes, mail and time stay where they are. The Porch's settings only choose which projects have a lane there (Porch ⚙, "Projects shown here"). A project **for a client** (`kind = "project"`) has a client, an hourly rate and the budget its invoices are expected in, and its time can be billed.

```toml
[[project]]
id = "lumen"
title = "Studio Lumen — website"
kind = "project"
client = "Studio Lumen"        # or "sioul:contact/<UID>": its client profile, else the card's postal address
rate = 60.0                    # else [invoice] rate
budget = "work"                # invoices expected there until paid
```

## A project's page (Projects, Ctrl+8)
- **One line of time**: what is coming first (dates asked of its open tasks, events), then what happened, newest first (tasks done, mail by its routes or tied by hand, notes of its record or tied to it, time noted, invoices). Mail comes by its routes on sender and subject (matched again each time), by being tied to the project (when it arrived, or by hand), and by conversation: the whole of a conversation one of its messages belongs to ([notes-folder.md](notes-folder.md#routes)). Each message is checked as the Porch checks it, each time the page is shown (`porch::Gate`, which reads the file and asks `porch::admission`, the function the Porch's own lanes use): what the Porch sets aside (a blocked sender, forged, a borrowed name, spam no route protects, hostile) never shows, whether a route, a tie Sioul made or a conversation brings it, and a conversation is followed through the messages let in only, so a forged reply joins nothing; mail nothing authenticates comes by no route on its sender's address or domain, and says "whose address could not be verified"; a message you tied by hand stays, as you tied it. The agents' `list_projects` reads the same page.
- **Its tasks** as a board, a list or a calendar: the Tasks page, filtered to the project.
- **"Note time"**: a meeting, a call, work away from the timer; for a project alone or a task.
- **"Make the invoice"**, when billable time waits: a draft of that time, issued in the same click, or refused with what is missing ("Invoices", below).

## Time (Ctrl+9)
A week, a month or a year: a bar per day (per month over a year) stacked by project, each project's hours and what is left to bill, in hours and in money at its rate, then each stretch of time, newest first.
- **Billed or not, task by task**: a task's form says "Billed": as its project says (work for a client is billed), its time is billed, or not billed (`X-SIOUL-BILLABLE` in the task, kept by CalDAV servers; greyed on Google Tasks).
- **As a spreadsheet**: the export button on the Time page writes a project's billable time as CSV: what (a line per task, or per note), hours, hourly rate, amount, then the total; this week, last week, this month, last month, all time, or from one day to another. In French, `;` between fields and a decimal comma, as French spreadsheets read them. Nothing else goes in: the full invoice is the invoice's (below). Time comes from the focus timer (a task's time counts for its project: the first of its projects that is for a client, else its first project) and from what is noted by hand (`$XDG_DATA_HOME/sioul/time/<month>.toml`, sessions with `project`, `unbilled`, `invoice`). Any stretch not billed, timed or noted by hand, changes with a click: its task, its project, its day, from when to when, what it was (`qml/TimeDialog.qml`; moved to another month, it is written there first); its right click takes it out. Billed time stays as it was billed.

## Invoices
Invoices, credit notes and deposit invoices you issue (`crates/sioul-core/src/invoice/`: `record.rs`, `life.rs`, `series.rs`, `totals.rs`, `presets.rs`, `ids.rs`, `html.rs`; client profiles in `clients.rs`). The invoicing's phase 1 (November 2026) built the model and its life, described here. Later: the issued file, a PDF/A-3 carrying its Factur-X XML, kept beside its record with its hash (phase 2); the window's Money ▸ Invoices, the editor and Settings ▸ Invoices by country (phase 3); the books, the receipts book from invoice payments, the declarations and the thresholds (phase 4). What the design rests on: [research/invoicing.md](research/invoicing.md), findings 76 to 79 of [research.md](research.md).

- **One record, in EN 16931's terms** (`record.rs`, `format = 2`): each field is one of the European standard's business terms (BT-xx), so that one record prints its page in any language and becomes an electronic invoice without translation. Its kind is UNTDID 1001's code (BT-3): 380 an invoice, 381 a credit note, 386 a deposit invoice. It holds its series and number, its issue day, the service's day or period (`period_from`, `period_to`: the first and last days billed; service-public.gouv.fr, sheet F31808), its due day and terms, its currency (ISO 4217) and language, its project, its client as chosen (`client_ref`), the issuer and the client frozen as they were at issue, its lines, its totals, the invoice it corrects (BG-3), the deposit invoices it deducts, the client's purchase order (BT-13), its notes with their subject codes (UNTDID 4451), the late-payment rate it printed (in basis points), its payments and the day it was sent; `file`, `file_sha256` and `record_sha256` wait for phase 2's kept file.
- **Where it stands is read, never typed** (`Invoice::status`, `left`, `credited`): draft, issued, sent, partly paid, paid, credited, from its number, the day it was sent, its payments and the credit notes that cite it. Nothing is "overdue": what is unpaid is "not paid yet".
- **Lines** (BG-25): a description and details; a quantity in thousandths of a UN/ECE unit (Recommendation 20: `HUR`, `MIN`, `DAY`, `C62`, `MON`, `KMT`, `LS`); a unit price in cents for `base` units (BT-149); a discount; a VAT category (UNTDID 5305: `S`, `Z`, `E`, `AE`, `K`, `G`, `O`) and its rate in basis points; an exemption's reason; goods or a service; the amount; the time it bills, by session. **Lines from time** come as before, one per task (or per note, for time noted without a task), in the order first worked on, as minutes at the project's hourly price with a base of 60, so that 25 minutes at €60 an hour make €25.00 exactly, as format 1 computed it.
- **Totals** (`totals.rs`), computed once in integer cents as EN 16931 checks them (BR-CO-10 to 17): each line to the cent, half away from zero (Factur-X §7.1.8); per category and rate its base and its tax; then without tax, the tax, with tax, the deposits deducted (BT-113) and the amount due (BT-115). No float in a sum: quantities are thousandths, rates basis points; a project's rate, still a float in `sioul-projects.toml`, becomes cents at that boundary.
- **Drafts** in `$XDG_DATA_HOME/sioul/invoices/drafts/<id>.toml`, beside the records, so that they travel with them: a draft has no number, and its id is when it was made and six letters (`20261116-143005-k3x9qa`), never a number. It changes freely (`life::change_draft`, which the editor of phase 3 calls): `life::refresh` takes the issuer as the settings say it, each line's tax as the scheme asks, the legal lines the preset prints for this client, the late-payment rate, the due day and the totals; your own notes stay. A draft is made from a project's time not billed yet (`life::from_time`), for a client (`life::for_client`), blank, as a credit note or a deposit invoice.
- **Checked before issue** (`life::checks`): what the preset asks and the draft lacks, in plain words and in the invoice's language. Required items refuse the issue; recommended ones are said only ("The client's SIREN: asked on invoices to French businesses from September 2027."). Identifiers are checked by their keys (`ids.rs`: a SIREN and a SIRET by Luhn, La Poste's own rule too; a French VAT number by its key). A table that issued invoices before may now be refused where its preset requires what it lacks (your SIREN, a French business client's address): a change of behaviour, said here.
- **Issued all or nothing** (`life::issue`): checked first; refused, in words, on a device whose sharing is on but leaves out the `drafts` part, which carries the invoices ("Turn on Drafts and invoices in Sharing to issue invoices on this device, or issue them on the device that has it."), since the others' numbers may not be known there; refused when some of its time was billed meanwhile. Then, under the series journal's lock: numbered, its issuer and client frozen, kept as `invoices/<number>.toml` (never over another file: of two numbers that make one file name, "F:1" and "F/1", the second is refused), the journal written; its time marked (a credit note's cancelled time freed); a planned line in the project's budget, tied to `sioul:invoice/<number>` and due on its due day (none for a credit note); the draft taken out. A step that fails undoes those before it. **Make the invoice** on the Projects page stays one click: a draft of the project's time, issued at once; refused, it says what is missing and leaves no draft behind (`crates/sioul-app/src/projects.rs`, `make_invoice`).
- **Numbers** (`series.rs`): a pattern and its reset make a sequence. A pattern holds `{YYYY}`, `{YY}`, `{MM}`, and one counter, `{N}` or `{N:k}` (k digits at least), the rest as written; `reset = "yearly"` (then the year is in the pattern) or `"never"`. `[invoice] pattern` when set, else the older `prefix`: empty, `{YYYY}-{N:3}` from 1 again each January (2026-001); a prefix of your own and three digits, going on from the last whatever the year (F-041, F-042). Each number is one more than the largest of the series journal, the records (those that no longer read too) and every device's claim (`series::next`). The journal, `invoices/series.toml`, keeps each sequence's last number and its day, written in the same step as the record, under its lock: a record deleted by hand no longer frees its number, and a number of the journal's range with no record here is said ("2026-004 has no record on this device", `series::missing`), never given again. The lease's claim carries each sequence's last number too (`sioul_sync::lease::Claim::series`, from the journal at each renewal; [database.md](database.md#leases-one-computer-at-a-time)), so that a device whose records have not come yet never gives one of them again. Records list in their natural order, 2026-999 before 2026-1000.
- **Credit notes** (`life::credit`): a draft citing the invoice's number and day (BG-3), with all its lines (to cancel it) or some (to correct them); in the invoices' series, unless `credit_pattern` gives them one of their own; no payment terms and no late-payment lines. Issued, it settles what it credits: an invoice credited whole reads "credited", one credited in part has the rest left to pay. It frees for billing again the time of each line it cancels whole, and all of the invoice's time when it credits the whole invoice (a format-1 invoice credited whole: all its time). The budget line is not changed by a credit note.
- **Deposits and final invoices** (`life::deposit`, `cite_deposits`): a deposit invoice (386) bills an amount up front, excluding tax; a final invoice cites the deposit invoices it deducts, by number, day and amount with tax, and its amount due is what is left.
- **Payments**: each `{date, amount, means, movement}`, `movement` the address of the bank line's movement it was matched with, empty when said by hand. **Paid**, on the Projects page or the invoice's card in the agenda, writes a payment of what is left, by transfer, dated today (`life::pay_rest`), and its budget line follows ("Budget", below); **Undo** takes that payment off again; **Not paid** takes off the payments said by hand, never those the bank's matching wrote. **From the bank**: a bank line that the matching joins to an invoice's planned line, when an export is taken in, writes a payment on that invoice (`movements::acts::resolve`: the bank's day and amount, by transfer, the movement's address), once per movement; the window says it in the status line with Undo, which takes it off again, and nothing asks to confirm it ([accounting.md](accounting.md#payments-on-your-invoices)). Payments add up: one smaller than what is left makes the invoice partly paid.
- **Its life written in place** (`invoice::write_life`): an issued record is never written whole again; its payments and the day it was sent are written into its file with `toml_edit`, every other line kept as it is.
- **Format-1 records** (before November 2026, no `format` key) read as issued invoices of kind 380 with time lines (`record::V1`): their SIRET an identifier, their VAT line the franchise's mention (category `E`), or no VAT (`O`) without one. They are never rewritten: their payments and `sent` are written in place, with `paid` for an older Sioul, and they print as they always did, in the window's language. A record that no longer reads still holds its number, and is never written over.
- **Its own language and currency**: an invoice keeps its language (`en` or `fr`: the client profile's, else `[invoice] language`, else the window's) and its currency (the profile's, the settings', the preset's, else euros), prints in them whatever the window speaks, and shows its amounts in its currency in every view (the project's page, the agenda's dated card, `sioul invoices`, the agents), where a euro sign showed before whatever the currency.
- **The page and the PDF**: the record's page (`html.rs`) prints the issuer with its mark and identifiers, the client with its identifiers and delivery address, the nature of the operation, the date or period of the service, each line's quantity, unit and unit price, the VAT per rate when liable, the totals, the deposits deducted, the legal lines and how to pay. The window prints it to an A4 PDF through Qt's text document and PDF writer, in `[invoice] folder`, else `~/Documents/Invoices` (Factures in French); "Print again" renders the record again, until phase 2 keeps the file issued.
- **Budget**: the amount due is expected in the project's budget, a planned line on the due day (the terms: the client profile's, the settings', the preset's, else 30 days), tied to `sioul:invoice/<number>`; ticking "Paid" dates it today and makes it real, with "Undo" for ten seconds; unticked, it is planned again on the day it had, its due day when that is not known (`budget::unsettle_line`), where it was in the ledger, so that Paid then not paid leaves the budget as it was.
- **From the command line** (`crates/sioul-cli/src/invoices.rs`): `sioul invoices [list] [--client <words>] [--year <year>] [--status <status>]` (drafts first, then the invoices, newest first, each with where it stands; a record that no longer reads, and a number with no record here, said), `show <number or draft>` (its lines, totals and payments, and for a draft what it still lacks), `export [--year <year>]` (every invoice as CSV, in French with `;` and decimal commas) or `export --html <number>` (its page to print), `draft <project>` (a draft of the project's time not billed yet; nothing numbered), `issue <draft>` (you at the keyboard, under the window's rules: one device numbers at a time, never one whose sharing leaves invoices out), `credit <number> [--line <n>]…` (a draft credit note, all lines or those given), `pay <number> [--day <day>] [--amount <amount>] [--means transfer|card|cash|cheque|direct-debit|other]` (a payment received; what is left when no amount is given).
- **Agents**: `invoices` reads drafts and invoices, `draft_invoice` writes drafts only, when you ask; neither issues, sends nor records a payment ([mcp.md](mcp.md)).

### Countries: presets as data
- **One file per country**, `crates/sioul-core/data/invoice-presets/*.toml`, built into Sioul (`presets.rs`): France (`fr`), Germany (`de`), Belgium (`be`), the United Kingdom (`uk`), Switzerland (`ch`), and `elsewhere` for any country (no legal claims: every identifier and line your own). Spain (`es`) and Italy (`it`) are listed as not supported, each with why (certified, chained invoicing software, VERI\*FACTU; the state's exchange system, SdI): issuing under them is refused with that sentence, rather than a template offered that would be illegal there. No country has a code path of its own.
- **What a preset says**: its labels in English and French; its legal forms (and the mark a form prints after the name), its income regimes; the identifier kinds its invoices carry and how each is checked; what is required or recommended before issue (of the issuer or the client; for business, consumer or public clients; where the client is, at home, in another EU member state or abroad; for goods or services; under which scheme and legal form; from or until a day), with the sentence said when it is missing; its VAT schemes, the franchise's category, reason code and dated mention, its dated rates, exempt lines, the reverse charge; its legal lines with their EN 16931 subject codes, each with its source in the file's comments; its late-payment terms, its payment terms, its activities, thresholds and contribution rates, each dated (the books read them from phase 4).
- **Three settings apart**: the legal form (France: an entrepreneur individuel, micro-entrepreneur included, or EURL, SASU, SARL, SAS, SA; the other presets: a sole trader or a company), the income regime for the books (France: micro or réel), and the VAT scheme, dated (`none`, not registered; `franchise`, the small-business exemption with its mention; `liable`), since none implies another: an EI micro-entrepreneur liable for VAT prints « EI » and VAT per rate; a company under the franchise prints the franchise's mention, its form and its capital. Sioul prints « EI » after a French entrepreneur individuel's name (C. com. R526-27), whatever the VAT, unless the name says it already, at its start, at its end, or in words (`life::marked_name`). A French company's capital ("SASU au capital de 1 000 €") and its RCS city are required.
- **VAT by the scheme of the day of issue** (`life::default_tax`): not registered, `O`; the franchise, `E` with its mention (France: "TVA non applicable, article 293 B du CGI", reason code `VATEX-FR-FRANCHISE`), and no VAT ever shown (art. 293 E CGI); liable, `S` at the preset's standard rate, your own rate, category or written exemption kept on a line. A service to a business in another member state takes the reverse charge whatever the scheme, `AE` with `VATEX-EU-AE`, "Autoliquidation" or "Reverse charge", and asks for both VAT numbers. The rates shipped (France 20, 10, 5.5 and 2.1 %; Germany 19 and 7 %; Belgium 21, 12 and 6 %; the United Kingdom 20, 5 and 0 %; Switzerland 8.1, 2.6 and 3.8 %) were not part of the research, but for Switzerland's 8.1 %: they are the rates as known in mid-2026, each file says so, and they are to be checked.
- **France's lines**: for business and public clients only, "Escompte pour paiement anticipé : néant.", the late-payment penalties at the rate kept with the invoice (by default the ECB's main refinancing rate in force on 1 January or 1 July, plus 10 points: 12.40 % for an invoice of the second half of 2026), and the €40 recovery indemnity (C. com. L441-9, L441-10, D441-5); none of them for a consumer, nor on a credit note. A late-payment rate of your own below three times the legal interest rate, and terms over 60 days to a business, are said before issue. Liable for VAT with that option, "Option pour le paiement de la taxe d'après les débits"; your professional insurance, when you give it.
- **Left out on purpose**, as the research could not verify them: Belgium's franchise mention (two wordings in use: you write yours, and a required check says so), Belgium's €25,000 threshold (secondary sources only), a French artisan's RNE mention, a German late-payment wording (§ 288 BGB), and the CIBS article for the French franchise's mention from 2027 (L. 223-3 or L. 233-3; the CGI's wording stays valid until 30 June 2028 and is the default). None is printed as law.
- **Older tables** (`life::preset_of`, `legal_form_of`, `regime_of`, `scheme_on`): a table with no `preset` whose VAT line cites article 293 B reads as France, an entrepreneur individuel, the micro regime and the franchise, its own VAT words kept as the mention; any other as `elsewhere`, with no VAT scheme, its VAT line and the old late-payment sentence printed as lines (`older-vat`, `older-late`; the latter taken out with `[[invoice.line]] id = "older-late"`, `off = true`). The old `siret` becomes an identifier: a SIREN for nine digits under France, else a SIRET; a kind its preset does not know prints as its name in capitals ("SIRET 12-3456789"), so that an invoice elsewhere keeps its words. The French older table now prints the preset's lines for business clients (the ECB's rate plus 10 points, €40, no escompte; none for consumers) in place of the old fixed sentence, which gave three times the legal interest rate as the rate: a correction.

```toml
[invoice]
preset = "fr"                  # fr, de, be, uk, ch, elsewhere
legal_form = "ei"              # France: ei, eurl, sasu, sarl, sas, sa; elsewhere: sole, company
regime = "micro"               # France: micro, reel
name = "Camille Exemple"       # « EI » is added after it
address = "2 place de l’Exemple"
postcode = "69001"
city = "Lyon"
country = "FR"
email = "camille@example.org"
language = "fr"                # en, fr; a client profile's first
terms_days = 30                # the preset's when unsaid
pattern = "{YYYY}-{N:3}"       # else from `prefix`, as before
reset = "yearly"               # yearly, never
# credit_pattern = "AV-{YYYY}-{N:3}"   # credit notes in a series of their own
# penalty_rate = 12.4          # percent; else the preset's default
# capital = "1 000 €"          # a company's, as printed
# insurance = "…"              # insurer, contact, coverage, as printed
# options = ["debits"]         # France: VAT on debits
# activity = ["bnc"]           # for the books, read from phase 4

[[invoice.id]]
kind = "siret"                 # a kind of the preset's: siren, siret, rcs, vat, other…
value = "…"

[[invoice.tax]]
from = "2026-01-01"
scheme = "franchise"           # none, franchise, liable
# mention = "…"                # the franchise's, when not the preset's

[[invoice.bank]]
holder = "Camille Exemple EI"
iban = "…"
bic = "…"

[[invoice.line]]               # a line of your own
text = "Merci de votre confiance."

[[invoice.line]]               # a preset's line, in your words (or `off = true`)
id = "escompte"
text = "Escompte pour paiement anticipé : 2 % à 8 jours."
```

The Settings page shows the keys it showed before (name, address, SIRET, VAT line, prefix, currency, payment details, folder, hourly rate); Settings ▸ Invoices by country comes in phase 3.

### Clients
- **Client profiles** in the notes folder's `sioul-clients.toml`, beside `sioul-projects.toml` (`clients.rs`), one `[[client]]` each, tied to its contact by address (`sioul:contact/<uid>`), so that a profile travels with the projects and never adds fields to a card on someone's server: whether it is a business, a consumer or a public body; its legal name; its billing address and, when different, its delivery address, both structured with their country; its identifiers (a SIREN, a VAT number); the language, currency and payment terms of its invoices; the address they are mailed to; a purchase order to print.
- **Found from a project**: a project's client (`client = "sioul:contact/<uid>"`, or a name) finds its profile by the contact it names, else by the profile's name or id; with none, it bills as before: the card's organisation and name, its first postal address and first email address, or the name as written (`clients::resolve`). No new `sioul:client/` address. The client is frozen into the invoice at issue as its profile says then.
- **Written by hand** in phase 1; Contacts and Projects show and change them in phase 3, through `clients::save` and `remove`, which keep the file's comments and order. They travel with the `projects` part of the sharing, once every device's Sioul knows the file ([database.md](database.md)).

```toml
[[client]]
id = "lumen"
contact = "sioul:contact/<UID>"
kind = "business"              # business, consumer, public
name = "Studio Lumen SARL"
language = "fr"
currency = "EUR"
terms_days = 30
email = "factures@example.org"
order = "PO-77"

[client.address]
lines = ["1 rue de l’Exemple"]
postcode = "75002"
city = "Paris"
country = "FR"

[[client.ids]]
kind = "siren"
value = "…"
```

### Later
- **Phase 2, the issued file**: a PDF/A-3 written by Sioul in Rust, carrying its Factur-X XML (EN 16931 profile) and, on euro invoices with an IBAN, an EPC QR code to pay by transfer; kept beside its record as `<number>.pdf`, with the SHA-256 of both, so that "Print again" gives back that very file; a validator run in CI over sample invoices of each preset.
- **Phase 3, the window**: Money ▸ Invoices (every invoice and draft, by date, with where each stands, filters by client, year and status), the editor (any line, a draft kept as it is typed, the checks and a preview), Settings ▸ Invoices by preset, client profiles in Contacts and Projects, "Write the mail" (a draft to the client with the PDF attached, which writes `sent` once it leaves), "Paid…" with the day and the means, "Credit note…", "Deposit…".
- **Phase 4, the books**: the receipts book from invoice payments, the declarations per category with their Porch items, the thresholds and their notices, the reminder of the European declaration of services.
- **Open points**: a credit note leaves the budget line as it was; a match the matching finds later, when the books are made again rather than at an export's import, is not written on the invoice yet.

## Budgets at their pace
A budget's verdict no longer compares a projection made only of what is scheduled with its target, which said "short" on the 3rd of every month. What is known in advance (presets, planned lines) counts in full; the rest, what simply happens, is weighed against the part of the period gone: ten days into thirty, a third of what the period still needs should have come in (or no more than a third of what it may spend should be spent). Within the one tolerance of every verdict, 5 % of what the period moves, it is on track ([accounting.md](accounting.md), "On track, without assuming money flows evenly").

The end of the period is estimated:
- with fewer than three past periods: the difference of today carried to the end (no extrapolation of three days into a month);
- with three or more: what is known, plus the median past period's unscheduled movements for the part left; the worst and best past periods give a range (the 10th and 90th percentiles from ten periods on).

A budget opens on its ledger (lines and recurring movements of the period, newest first), its balance by day, week, month or year (planned movements dotted), and "Add a movement": once, or each month or year (`[[preset]]`, written in place, comments kept).
