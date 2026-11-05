# Persons

Admin is done for others too: a child's identity card, the tax notice of a couple, the post of a parent you help. Whoever does it carries it ([research.md](research.md), finding 55). So Sioul keeps a person for each of the people your admin is done for, and papers point at them: you, a child, a parent you help, a partner, family, someone you help, and a child's other parent. A person is not a contact card: a card is for reaching someone, kept on a CardDAV server and read by every app of a phone; a person is who papers and agendas are *for*, kept sealed on your own devices (findings 57, 60). A baby is a person with no card; one human with a home card and a work card is one person with two.

Code: `crates/sioul-core/src/persons.rs`, `crates/sioul-app/src/persons.rs`, `qml/Household.qml`, `qml/PersonPage.qml`, `qml/PersonDialog.qml`; the papers' "For" in `crates/sioul-app/src/papers.rs` and `qml/PaperDialog.qml`. Research: [research/persons.md](research/persons.md).

## What a person holds
Only these, so that nothing about anyone is kept that their papers do not already say (finding 56):

| Field | Example | Why |
|---|---|---|
| `id` | a UUID made once | the same on every device; the name of the person's table |
| `name` | "Léa" | what Sioul shows |
| `full_name` | "Léa Martin" | how papers write it, to match them |
| `relation` | `child` | who they are to you, below |
| `born` | 2014-05-12 | the ages French papers follow (12, 16, 18, 20, 21, 25); optional |
| `lives_with` | true | the CAF's household, a person under your roof for taxes; optional |
| `cards` | contact UIDs | the cards that are this person |
| `of` | a child's id | a child's other parent: whose |
| `since`, `until` | dates | the relation's period; `until` past, it ended |
| `agents` | false | open to AI agents; closed until opened |

Never a field: a social security, identity or tax number, a health fact, a capacity, a location. Numbers stay inside the paper that bears them.

**Relations**, said from your side, in RFC 6350's words where they exist (`me`, `child`, `parent`, `kin`), Sioul's own where none fits: `partner` (RFC 6350's "spouse" leaves out PACS partners and cohabitants), `helped` (someone you help), `other-parent` (a child's other parent: outside the household, never the holder of a paper), and unsaid ("In your household", « Proche »). In French the relation is a « type ». A word a later version writes reads as unsaid, and is kept as written.

## Where it lives
`$XDG_DATA_HOME/sioul/persons.toml`, beside `links.toml`, not in the notes folder: a sync app may carry that folder to a server in clear, and Sioul's own sharing is sealed. Format 1; every table keeps what a later version writes. Each person is a table named by their id, its fields inside.

```toml
# The people your admin is done for (docs/persons.md).
format = 1
names_read = 1

[person.6f0a3c2e-…]
name = "Léa"
full_name = "Léa Martin"
relation = "child"
born = 2014-05-12
lives_with = true
agents = false

[person.9b1d…]
name = "Sam"
relation = "other-parent"
of = ["6f0a3c2e-…"]
cards = ["c-sam-1"]
agents = false
```

- **Shared** between your devices in the "settings" part, sealed, each field of each person an entry, so that a name changed on the phone and a birth date added on the computer both stay, before the two first meet as after ([database.md](database.md), "Parts"). A person is a table rather than an element of a `[[person]]` list because the sharing carries a list's element whole: two devices changing two fields of one person in the same exchanges would keep only one. It travels once every other device says it has the store (`devices::Entry::persons`), so that an older Sioul never holds a record it would keep waiting.
- **Never on a contact server**: a person links to cards by their UIDs, and no `RELATED` line is written into a card. No server keeps a relation as a reference every client follows (finding 60), and an address book is read by every app of a phone. Writing one by your choice was left out: Sioul writes cards in vCard 3.0, which has no `RELATED` (Google's server takes only 3.0), and whether Nextcloud and a phone's address book keep such a line cannot be shown by a stand-in server here, only by those servers themselves.
- **You** are always a person, `me`, with nothing to fill. A paper with no holder is yours, as it always was.

## What points at a person
Each tie lives where every version of Sioul keeps it.

- **A paper**: a tie in `links.toml`, `sioul:paper/<id>` → `sioul:person/<id>`, how `holder`, one per person (a joint tax notice, a lease of two). The paper's `holder` still holds the names, joined by ", ", which older versions show; an older Sioul rewrites the wallet with the fields it knows only, so the tie could not live there.
- **A paper's text, with no tie**: matched at each read, by name or full name, case and accents aside. A text joining names ("Alex, Sam et Léa") is cut at ", ", " et ", " and ", " & " when the whole names nobody. Empty, one of your accounts' names, or a word for you ("moi", "me", "myself", « moi-même »), it is yours. A child's other parent is never matched as a holder. What names nobody stays text, shown as written.
- **An agenda**: `person = "<id>"` in `[agenda.calendars."<place>"]`, beside `name`, which keeps the person's name for older versions; unsaid, `name` is matched to a person of the agenda's relation (a child's agenda to a child, a partner's to a partner, another parent's to a child's other parent). A child's agenda's other parent is `parent_person`, beside `parent`; the address their invitations go to stays the agenda's `parent_address`. Each agenda is then shown by its person's name now, and the filter groups agendas by person: two agendas of one child are one group, however their names were spelt (`agendas::Agenda::with_persons`).
- **Custody days and holder lines**: a whole day "With Sam" (« Avec Sam ») counts for the other parent by any of their person's names, case and accents aside (`holders::custody_of`); a holder line whose address is an agenda's other parent's shows that person's name now.
- **A letter**: a tie in `links.toml`, `sioul:letter/<id>` → `sioul:person/<id>`, how `to`, written when you choose on its card ("For…"); else its addressee, the name its address block gives (`letters::addressee_of`: the name line of a block ending with a postcode and a town, "Madame" and the like taken off, the block with a civility first), matched to you by your accounts' names and to persons by their full name (or name), its words in any order ("MARTIN Léa"), case and accents aside (`Persons::addressed`). A block naming nobody here gives nobody: the letter is yours, as before.
- **A legal measure or a procuration**: a paper of the kind `procuration` or `protection` (a mandat de protection future, an habilitation familiale, a curatelle, a tutelle: one kind, its words in the title), "For" the person it concerns, its `until` the measure's end; renewing starts four months before, since a review must be asked of the judge before the end or the measure lapses (service-public.gouv.fr, F2120). Never a mark on the person (finding 56). An older Sioul knows no such kind: the wallet writes them `kind = "other"`, `measure = "protection"` beside, so that it still reads every paper ([papers.md](papers.md)).
- **A budget**: `sioul:person/<id>` among the budget's own `links` in `sioul-budgets.toml`, which every version keeps, one person at a time, chosen in its form's **For** (`budget::person_of`).
- **A site you open for them** ("Odile's ameli account, which you open for Odile"): a tie in `links.toml`, `sioul:site/<id>` → `sioul:person/<id>`, how `for`, one person at a time, chosen in the site's **Name and address…** (`persons::tie_site`). The site is opened in your own browser profile, by you; Sioul keeps no one else's password unasked.
- **A contact card**: its UID in the person's `cards`, one person's at a time. A child's other parent is the one card holding an agenda's address for them, when exactly one does; never a card matched by first name.
- **By address** (a holder line on a child's event, a sender): an address on one of a person's cards, or the address an agenda gives for a child's other parent of that name.

## Read from old texts, once
Agendas and papers named people before persons existed. On each device, once, while `persons.toml` does not say `names_read = 1`:

1. Each agenda said a child's, a partner's or another parent's, with a name, gives a person (`child`, `partner`, `other-parent`); a child's agenda's other parent gives one too, whose `of` names that child. Two agendas of one child give one person.
2. Each paper's holder text that names nobody yet gives a person of an unsaid relation, except your names and the empty text, which are you, and the names said to be no person (`not_persons`).
3. The ids of persons read so are name-based: a UUID, version 5, of the relation and the name, case and accents aside, in Sioul's own namespace (`holders::NAMESPACE`). A phone and a computer reading apart make the same persons with the same ids, and the sharing joins them as one; a child and an other parent of the same first name stay two.
4. Nothing else is written: no tie, no key, no line. A tie is written only when you choose someone in a form.
5. `names_read = 1` is written last. It stays on each device; the sharing leaves it out.

What is read so counts until corrected, with no list to review (findings 6, 11).

## In the window
- **The household**, at the head of the Contacts page (« Proches »): one row of chips, you first, then children (eldest first), parents, partners, family, people you help, those unsaid, and after a thin gap children's other parents; "Add a person" at the end. A chip opens the person's page in the place of a card (on a phone, the page); right click, or a long press, opens its menu (Open, Rename). One "?" says what a person is.
- **A person's page**: their name and one line ("Child · 12 · lives with you", « Enfant · 12 ans · vit avec vous », the age from the date of birth); their papers with where each stands, and "Add a paper for Léa"; their agendas, each child's with **Who does what** (for a child's other parent, the children's agendas they share); the cards that are them, each one click from its card, and "Link a card…"; their letters; **Money**, the budgets for them and the sites you open for them; at the foot, quiet, **What Sioul keeps about Léa…**, **Ends…** (anyone but you and a child), **Same as…**, **This is not a person**, and "Open to AI agents" (not for you: you are always open, as before). A section's title shows only when the section holds something; its button to add one always does.
- **The form**: a name, how papers write it, the relation, a date of birth, "Lives with you", and for a child's other parent, whose. A name is unique within the household. A new name rewrites the papers' holder texts and the agendas' `name` and `parent` that named the old one or hold the person's key, so that older versions show the new one (`persons::rename_holders`, `persons::renamed_agendas`).
- **Agenda ⚙ ▸ Your agendas**: "Whose it is" and "The other parent" are person choosers: nobody said (or the name an older text gave, as written), the persons of the right relation, then **New person…**, whose form opens with the relation and, for an other parent, the child filled in; the person saved is chosen.
- **An agenda's role gives its person** (5 November 2026): a child's, a partner's or another parent's agenda whose name names nobody by its key, made with **New agenda…** (its name typed), given that role, or written by an older version, finds that person or makes them, and keeps their key (`persons::tie_role`, `Persons::for_role`): one of that relation by name or full name; else, for a role that holds papers, one of the household so named, whose relation stays theirs unless it was unsaid (read from a paper), when it becomes the role's, so that two of the household never share a name; else a new person of that relation, their id name-based as the first read makes them, so that two devices make the same person. A child's agenda's other parent alike. A key that names someone stays; a name said to be no person makes nobody. The window ties every agenda so as it starts, once nothing is left to tie writing nothing.
- **Shown at once everywhere**: the store, written, says so to the window, and so does an agenda's role (`sioul_core::changes`, [client.md](client.md#every-view-shows-a-change-at-once)): the household, an open person's page, Agenda ⚙'s choosers, and the papers' and letters' pages read them again, whether the change was made here or came from another device.
- **A contact's card**: "This is a person in your household…" links the card to a person, or makes one of it; once linked, the button opens that person's page.
- **Papers**: the form's "For" (« Pour ») chooses you, persons of the household (several at once), or "New person…"; the Papers page's chips, "Everyone" first, show one person's papers. A paper whose holder text names nobody here (written by an older Sioul) offers **Make a person of "…"** on its card, once, in place.
- **Letters**: a letter's card says "For Léa" when its address block names someone of the household, and **For…** chooses, each person ticked or not.
- **What Sioul keeps about Léa…**: one PDF of their papers (kind, dates, where each stands), agendas and letters, and a folder with the papers' files, saved where you choose (`<folder>/<name>/`). Nothing is sent: handing it over is your act (findings 58, 59).
- **The hand-over lines**, on a child's page, each once, in an admin window: at 16, "From 16, Léa can have a carte Vitale and a reimbursement account in Léa’s own name."; at 18, "Léa is 18: the papers are Léa’s own, and what Sioul keeps can be handed over.", with **What Sioul keeps about Léa…** beside. Shown, a line is said for good, on every device (`Person::handed`). Never a reminder, a badge or a count.
- **Ends…** says the relation's end (`until`; today when unsaid): the person leaves the household's row, their papers and agendas stay. For a partner, it offers to save their own papers (theirs alone; those of two stay, as the household's past) in a folder you choose, to hand back, then to take them out of the wallet.
- **Same as…** joins two persons (Léa and "Léa M."): the one whose page is open keeps its id and takes what the other held and it lacked (a full name, a date of birth, cards), the other's ties (papers, letters) and agendas' keys move to it, and the texts that named the other (papers' holders, agendas' `name` and `parent`) say its name.
- **This is not a person** ("Banque") takes them out and keeps their name among those that are no person (`not_persons`), so that no later read makes them again; their papers keep their texts, their ties and agendas' keys go.

## What agents see
Through `sioul mcp` ([mcp.md](mcp.md)):
- **Each person is closed until opened** on their page (`agents = true`); you are always open. A closed person's papers, those of a name that is nobody's, and a paper of two when either is closed, are left out of every list and counted, never named; asked for by address, they are refused, as a closed project's things are.
- **Letters**: a letter for a closed person (by its ties, else its addressee) is left out and counted, as their papers are.
- **The `persons` tool** gives the persons opened to agents, each with their name, who they are to you, their address, and what links to them by address: their papers and letters (each as its own consent lets it through), their agendas by place, their budgets and the sites you open for them. Never a date of birth, "lives with you" or a card; those not opened are counted, never named.
- **Opened**, an agent gets a paper's holder names and their addresses (`for`: `sioul:person/<id>`), never a date of birth, "lives with you" or cards.
- **A closed person's agendas**: a child's, a partner's or another parent's agenda whose person is closed, or that names nobody here, has its events and tasks left out and counted (`persons::Persons::closed_agendas`, `Consent::keep_agendas`). Your own stand-ins of a child's events stay, being your own time.
- **Never, whatever the switch**: health papers (a health card, a health cover) of anyone but you; a child's events and tasks of the health and dentist kinds, and your stand-ins of them (`RELATED-TO` the child's event), kept as health is. Data about a minor sent to a model's provider leaves the household exemption (finding 57).

## What Sioul never does about a person
No location, "last seen", activity or supervision of anyone; no reading of a dependent's messages, mail or accounts; no mark that an adult cannot decide; no total or history of what was done for whom; nothing of a person on anyone else's device or server (findings 56, 58).

## Tested
`cargo test -p sioul-core persons`: the store read and written with what a later version adds, dates bare; names matched case and accents aside, joined texts cut, you by your names; old texts read once into the same persons on two devices, two agendas of one child one person, an other parent and a holder of one name two; a paper's ties before its text; what agents never get; addresses finding persons and one card; the household's order; an agenda's person by its key, else by its name, two agendas of one child one group, closed agendas, a new name written where the old one stood (`agendas_name_their_persons_by_key_then_by_name_and_follow_a_new_name`). `holders::tests`: custody days by a person's names. `cargo test -p sioul-sync quiet_tests::the_persons_store_waits_for_a_device_that_has_it`: kept to a device until the other says it has the store, then joined field by field, a date of birth said before the join kept, two fields changed on two devices in the same exchanges both kept. `cargo test -p sioul-cli a_closed_person_is_never_shown`: planted words of a closed child, a paper tied by `links.toml`, a lease of two, a health card of an opened parent, a text naming nobody, a closed child's agenda's event, an opened child's dentist appointment and your stand-in of it, in no answer of any tool; the opened child's school fair comes. `budget::tests::budgets_made_changed_and_taken_out`: a budget for a person among its links, the others kept. `persons::tests::a_site_is_opened_for_one_person`. `cargo test -p sioul-app --test pdf`: "What Sioul keeps about…" written by `PdfWriter`, a PDF whose text names the person, their papers, agendas and letters, a word like markup kept a word. `cargo test -p sioul-app persons::`: a person saved, a name taken refused, a paper for a new person with its tie and holder text, a rename followed by the paper and an agenda, a person chosen for an agenda, a card linked and unlinked, an agenda's role finding or making its person (`an_agenda_s_role_finds_or_makes_its_person`; in core, `persons::tests::an_agenda_s_role_finds_or_makes_its_person`). `tools/qml-test.sh tst_household.qml`: the household's row, a person's page with its agendas and Who does what, and the person chooser, at a phone's width; a person made elsewhere shown at once.
