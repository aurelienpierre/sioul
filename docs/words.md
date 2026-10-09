# The words Sioul looks for

Every list of words a recogniser matches text against lives outside the code, in language and country packs, and each person can add words and take shipped ones away in their configuration. The recognisers include one-time codes and sign-in mail, approvals on the phone, bills and payments, bank exports, paper letters, papers, contracts, automatic senders, reply prefixes, quoted history, folder names, borrowed brand names, calls in sites, voicemail by mail, the shield of a public address, task categories, quick capture and the spam filter's tokenizer. Code: `crates/sioul-core/src/words.rs` (the packs, the merge, your changes, the typed lists, the writers), the packs in `crates/sioul-core/data/words/` (`en.toml`, `fr.toml`, `de.toml`, `es.toml`, `it.toml`, `international.toml`, `countries/FR.toml`), the migration proof in `crates/sioul-core/tests/fixtures/words-golden.json`. Each recogniser reads its lists from a `Words` value (below); none keeps a list of its own.

## Why
- **Until 8 October 2026 every list was code**, about 2,300 strings in some 130 lists, almost all in French and English, and a few fitted one person's services rather than everyone's. A German or Spanish reader got nothing, and a word nobody could change sent a message to the wrong lane for good. The owner asked on 7 October that every service- and language-dependent string be configurable, "inited with sensible defaults".
- **The design he approved** (8 October): your changes live in config.toml as differences, never as a copy of a list, so that a newer Sioul's words still reach you; grammar words (months, "your … code", stop words) stay in the packs and are never shown in Settings; the languages read are written once into the configuration and change only by your hand; one country setting, several allowed; the spam filter's words travel inside its table; a word both added and taken away counts as added (you put it back).

## Three layers
1. **Language packs**, shipped: `data/words/<language>.toml`, a file per language, like the `.ftl` files.
2. **Country packs**, shipped: `data/words/countries/<code>.toml`, the names of one country (its public bodies, its brands, its banks' approval services); and `data/words/international.toml`, always in use: the words of mail programs, servers and apps in any language (automatic senders, folder names, reply prefixes, quoted-message openings, call notifications, a provider's spam marks), brands of every country, the mail providers millions share.
3. **Your changes**, in config.toml under `[words]`.

A list as Sioul uses it is the packs' words in order (the languages', then the countries', then the international pack), less the shipped words you took away, plus yours. Words are compared folded (`words::folded`, `text::fold`): trimmed, lowercase, accents aside; each word is kept once, its first spelling first.

**The packs in use**: the languages of `[words] languages`, else the interface's and English; the countries of `[words] countries`, else `[contacts] region`, else the locale's country (`LC_ALL`, `LC_TELEPHONE`, `LANG`); and the international pack. A configuration written before these words (no `[words]` table) gets `languages = ["fr", "en"]` and `countries = ["FR"]` written at the first start of this Sioul on each device (`words::keep_first_once`, below), so that nothing changes until its owner changes it.

**Newer packs without a new build**: a pack in `$XDG_DATA_HOME/sioul/words/`, then in each of `$XDG_DATA_DIRS`' `sioul/words/`, under the same file name, wins over the built-in one when its `checked` date is newer, as the site presets do (`presets.rs`). Only packs Sioul ships are looked for there; your own changes never go there.

## A pack
```toml
# Words Sioul looks for in English (docs/words.md): a list for each thing it recognises.
language = "en"          # or: country = "FR"; the international pack has neither
checked = "2026-10-08"   # the newest copy of a pack wins

# One-time codes, passwords, password resets, sign-in links and addresses to confirm (codes.rs).
[codes]
# Names a code: a code right beside it is taken.
code = ["verification code", "security code", "one-time code"]

[letters.senders]
# A named list: a name shown, then the words that say it.
CAF = ["caisse d allocations familiales", "caf"]
```
- Each table is a recogniser, each array a list; the paths are those of the `Words` structs (`codes.code`, `letters.kinds.tax_notice`, `capture.weekdays.friday`). A path the code does not know is ignored, and a test refuses it in a shipped pack.
- **Named lists** (`words::MAPS`: `brands.brands`, `letters.senders`, `letters.months`, `letters.numbers`, `letters.fixed_delays`, `capture.months`) are tables of names, each with its words, in order.
- Words are matched whole, capitals and accents aside, unless the list's comment says otherwise. Pack words mostly keep the code's old spelling, without accents; accents may be added, since both sides are folded. The voicemail's `caller_leads` ("du", "de :") are matched as whole words, a letter or a digit on neither side, so a word you add there, trimmed as every addition is, works as the shipped ones do.
- Every comment says what the list does, in one line, for the person reading the pack to translate it.

## Your changes
```toml
[words]
languages = ["fr", "en", "de"]
countries = ["FR"]

[words.codes.code]
add = ["Bestätigungscode"]
remove = ["code temporaire"]

[words.brands.brands]
add = { "Ma Banque" = ["mabanque.example"] }   # a named list: names and their words
remove = ["Banque Exemple"]                      # names
```
- `add` and `remove` of one list; a word in both is there (`apply_one`).
- **Older settings** still win while written and while `[words]` says nothing of the same list: `filed_words` is `senders.automatic`, `[quiet] personal` and `work` are `tasks.personal` and `tasks.work` (`older_settings`). The Settings rows "quiet.work" and "quiet.personal" show those lists and keep writing `[quiet]`.
- **The writers** (`words.rs`), each through `config::read_document`/`write_document`, comments kept: `write_list(config_path, path, wanted, shipped)` writes the difference between the list you want and the packs' (`compared(config).shipped`), and takes the changes out when you want the packs' list; `write_names(config_path, path, wanted, shipped)` does the same for a named list (a shipped name given fewer words is taken away and added back with those wanted); `reset(config_path, recogniser)` takes out every change of a recogniser, `reset_lists(config_path, paths)` those of some lists; `write_setting(config_path, "languages" | "countries", values)`; `keep_first(config_path)` and `keep_first_once(config_path, marker)`.

## Settings ▸ Words
- **The tab** (`WordsTab.qml`, in `ParametersPage.qml` after What reaches you): on top, the languages read and the countries, ticked (`words.languages`, `words.countries`; at least one language); then one line per thing recognised, in four groups (mail; money and papers; the phone; tasks and a public address), each opening a page of its own, on a computer as on a phone: what its words do, its lists as word rows (`SettingRow.qml`: chips with ×, a field to add one), "Back to the defaults".
- **Its rows** (`settings::words_rows`, section "words"): for each line of `settings::WORD_LINES`, a note keyed `words.about.<line>` (its sentence in `label`, its counts in `help`, its group's name in `unit`), then a `Kind::Words` row per list, keyed `words.<list>`, showing the list as Sioul uses it, your own words first. A named list shows one chip per name, "Name: word, word"; a brand needs its domains, a body named alone is looked for by its name. Only the words that say what a thing is are shown; the grammar around them stays in the packs.
- **Writing** (`settings::apply`): `words.<list>` is written as its difference from the packs (`write_list`, `write_names`); `words.reset.<line>` takes that line's lists back (`reset_lists`); `words.languages` and `words.countries` through `write_setting`. The automatic senders' older whole list (`filed_words`) is taken out the first time that list is written, or reset; the Filed lane's ⚙ now holds a link to the line (`settings:words.senders.automatic`), so the list has one place.
- **The first start** (`sioul-app`'s `run`): `words::keep_first_once` writes `languages = ["fr", "en"]` and `countries = ["FR"]` into a configuration without `[words]`, once on each device (a marker, `words-first-start`, in Sioul's data folder): a configuration from before keeps what it read, one made later by this Sioul is left to its interface's language and English.
- The calls' setup on the phone links to the voicemail line (`settings:words.voicemail.operators`).

## In the code
- `Words` (and one struct per recogniser: `Codes`, `Approvals`, `MoneyWords`, `Payments`, `BankWords`, `AccountWords`, `LetterWords`, `PaperWords`, `ContractWords`, `SenderWords`, `Replies`, `Quotes`, `FolderWords`, `BrandWords`, `CallWords`, `VoicemailWords`, `ShieldWords`, `TaskWords`, `CaptureWords`, `SpamWords`, `OcrWords`), every field a list in its pack's order (`#[serde(default)]`: a list no pack in use holds is empty).
- `Words::of(&config)`: the packs in use, the newest copy of each, then your changes and the older settings; made once for each configuration (a small cache keyed by the languages, the countries and the `[words]` table).
- `Words::builtin()` / `Words::builtin_ref()`: the built-in packs for French, English and France, without changes: what the tests read, and what a caller without a configuration reads.
- `words::current()` / `words::set_current()`: the words of the configuration this program loaded last, for readers deep in other crates that see no configuration (a server's folder names, the reply prefixes, quoted history, calls in sites, task categories, quick capture, the shield). The desktop app (`backend::load_config`), the command line (`main.rs`, the spam filter's weekly run) and the MCP server (each session) call `set_current` when they read the configuration; tests never do, so `current()` is the built-in words there.
- **One pattern for the recognisers**: a function that takes the lists (`codes::detect(&words.codes, …)`, `money::find_amount(&words.money, …)`, `payments::detect(&words, …)`, `bank::read(&words.bank, …)`, `letters::read(&words, …)`, `papers::Kind::guess(&words, …)`, `contracts::Kind::guess(&words, …)`, `lookalike::impersonation(&words.brands, …)`, `appnotes::classify(…, &words)`, `voicemail::read(&words.voicemail, …)`, `folders::role_in`, `reading::parts_with`, `sites::is_call_with`, `shield::assess_with`, `capture::capture_with`, `capture::parse_day_with`, `tokenize::tokens_with`), and where a caller has no configuration, a function of the same name without `_with`/`_in` that reads `current()`. The Porch's triage takes them from its `Context` (`Context.words`, from `Source.words`; none: the built-in words).
- The OCR's Tesseract models are `words.ocr.tesseract` (`sioul_sync::ocr::text_of`, `install_hint`): the languages in use decide which models a scan is read with.

## The spam filter's words
The tokenizer's words (a provider's subject marks, month and key names, French elisions, number words, stop words: `SpamWords`) shape the words the model learns, so a table must be read with the words it was trained with, on every device.
- `tokenize::Lexicon` holds the words and the patterns made from them; `tokens_with(&lexicon, …)` reads with them, `tokens(…)` with the built-in ones.
- Training reads every message with `train::Options::lexicon` (`Options::of(config)`: the spam lists of the languages in use and your changes), and the table keeps them in its JSON metadata (`table::Meta::lexicon`). `spam::score`, the evaluation and `sioul spam why` read with the table's own (`Table::lexicon`). A change of words therefore counts from the next training, everywhere at once, and never drifts between devices.
- **A training's comparison reads each table with its own words** (`train::against_current`): the new table its training's, the table in place its own (`Table::lexicon`, the built-in French and English words when it keeps none). When the two differ, the test messages the table in place never learned from are read again from the corpus with its words, and one that can no longer be read is left out for both tables; so a change of words never makes the table in place look worse than it is, and a new table replaces it only when it takes no more ham for spam on the same messages (`the_table_in_place_reads_with_its_own_words`).
- A table made before 8 October 2026 keeps no words and reads with the built-in French and English ones, which are what made it. So **a change to the spam lists of `fr.toml` or `en.toml` changes how such tables read**: the weekly training replaces them within a week, but when in doubt raise `TOKENIZER`, which makes every older table refused, as a change of a tokenizer step always did.
- The shortcut's modifier keys ("ctrl", "maj"…), the single letter after them, the ordinal suffixes, the units and the stemmer stay in the tokenizer's code. The header feature `reply_to_freemail` keeps its own frozen list of shared providers (`spam/features.rs`, `FREE_MAIL`, versioned with `FEATURES`): a provider you add to `brands.shared` never shifts what a table learned.

## Tests
- **The migration proof**: `the_packs_give_back_the_lists_of_before` merges the shipped packs for French, English and France and compares every list, as folded sets, with `tests/fixtures/words-golden.json`: the lists of v0.0.3 (the code on 7 October 2026), the words that left the packs as one person's taken out. Every list of those packs must be one of the golden file's: a new list goes into the golden file with its shipped words.
- A recogniser's test reads a word added in `[words]` (German, Dutch, an invented brand or operator) and shows the packs alone do not know it: codes, approvals, payments, contracts, folder names, reply prefixes, quoted history, borrowed names, calls in sites, voicemail, quick capture, the shield and the spam lexicon have one; a recogniser moved later should too.
- **Name the languages in a test's configuration** (`[words] languages = ["fr", "en"]`, `countries = ["FR"]`): without them, `Words::of` reads the locale, French and English on a French desktop but English alone on CI (`C.UTF-8`) and on Windows, and a test that passes here fails there. Run the tests once with `LANG`, `LC_ALL` and `LC_MESSAGES` unset.
- **Invented names only**: a test never names a person's real bank, operator or subscription (`operator-mobile.example`, `Exemplestream`, `pixelmill`, `banque-exemple.example`). The payments' tests read `words::with_processor()`, the packs plus one processor's public receipt wording, added as its users would add it.

## Adding a list, a language, a country
- **A list**: a field in the recogniser's struct in `words.rs` (with a doc comment), its words in each language pack (a comment above each list), the recogniser reading it, a test with an added word, and the list in the golden file.
- **German, Spanish and Italian** (`de.toml`, `es.toml`, `it.toml`, 8 October 2026) hold the cheap lists only: code phrases and their grammar, approvals, automatic senders' address words, reply and forward prefixes, quoted-message openings, header names and "wrote:" endings, folder names, quick capture's days, months and "tomorrow", the tokenizer's month names, Tesseract's model. Their letters, papers, bank exports, payments, shield, task words and stop words wait for real examples and a reader of the language. Folding reads their accents (`text::fold_char`: ì, ò, ñ, ã, õ, å, ø, ý, one character for one; "ß" stays).
- **A language**: a copy of `en.toml` named for its code, `language` and `checked` set, its lists translated where the wording is public and stable (code phrases and their grammar, approvals, automatic senders' words, reply and forward prefixes, quoted-message openings and "wrote:" endings, folder names, call notifications, weekdays and months, quick capture's words, number and stop words, Tesseract's model name); added to `SHIPPED` in `words.rs`. Letters, papers, bank exports and payments in a language wait for real examples: a guessed legal phrase does more harm than none.
- **A country**: `countries/<code>.toml` with `country` and `checked`, and only complete lists (every national mobile operator, the main bank networks, every body that writes to everyone), never one person's providers.
- **Three rules keep shipped words from being one person's**: a phrase ships when it is the common wording of a kind of sender, seen from more than one service or in public documentation, and a test fixture (an invented message in that wording) backs its list; a name ships only inside a list complete for its country and category; before a release, someone other than the pack's author checks it for tailored words.

## What stays in code, and why
- **Protocols and file formats**: header names and values (`List-Id`, `Precedence: bulk`, `X-Spam-Flag`, `Auto-Submitted`), IMAP keywords and dates, MIME types and file extensions, iCalendar and vCard properties, Android notification categories, the notes' front-matter keys; and Sioul's own identifiers in config files (the area and time words of `Area::parse` and `Time::parse`, the attention matrix's columns, the task kinds' ids), whose French synonyms are a tolerance for hand-edited files, not recognition of anyone's text.
- **Mail clients' HTML quote markers** (`reading.rs`): markup of Gmail, Apple Mail, Thunderbird and Outlook, which change with their releases.
- **"unsubscribe"** as the subject and body of an unsubscribe mail: list servers read that word.
- **Services' machine words**: servers' English error replies, the GitHub integration's addresses, the sync folders looked for, the Android app lists (`BOOK_APPS`, `SMS_APPS`, `MAIL_APPS`; they deserve a setting of their own later).
- **Domain rules**: `tokenize::registrable`'s and `sites.rs`'s second-level domains, an approximation of the Public Suffix List.
- **Units and abbreviations typed by you**, shared by the languages and read as notation: a routine's step lengths (`routines.rs`: "m", "min", "mn", "h"), a filter's sizes (`rules.rs`: "kb", "ko", "mb", "mo"…), the days of office hours (`window.rs`: "mo-fr", "lu-ve", which are also the configuration's identifiers), quick capture's "~15m" and "~1h30", the "h" of "9 h 15" in letters.
- **Output**: the folder names Sioul creates when a role has none ("Junk", "Trash", "Archive", "Sent"), Sioul's own invoices' legal mentions (a country setting later), interface words outside the `.ftl` files.
- **Gaps, not lists**: out-of-office replies and parcels have no recogniser yet; when they come, they start in packs.
- **The sync tools' conflicted copies** (`share.rs`, `made_by_tools`): the names Nextcloud, Syncthing, Dropbox and others give a copy in conflict. They are the tools' own, in their own languages, and a French "copie en conflit" is not among them yet.
- **The one-time-code field of a site's form** (`sites.rs`): the pattern by which the site filler finds where a code goes, which follows the forms' own markup (`autocomplete="one-time-code"`, names and ids) rather than a language.

## Not built yet
The approved design goes further than what ships. These parts wait:
- **Why a message went where it went**: the Porch's reason naming the word that matched, with a link to its list; a "Taken away" or "Put back" line under a list; refusing a word under two characters, and a warning when you add a stop word.
- **The languages of your mail**: noticing a language that makes up a share of the last messages (5 % of the last 500, at least 20) and offering its pack. Today a new configuration reads its interface's language and English, and one from before keeps French and English.
- **Your changes, keyed**: a list's `add` and `remove` travel through the sharing as one entry per list, so two devices changing the same list at the same moment keep the later list whole, not both changes.
- **Country rules that are numbers**: the papers' durations, contracts' notice, numbering plans, crisis lines (which you could add to but never take away), the masked social security number, which currency "$" means. Today they are code, as are Sioul's own invoices' legal mentions.
- **Calls in sites**: their words still match inside a word, unlike the rule above; whole words come with the next change to that recogniser.
- **A second reader for the packs**: the rule that someone other than a pack's author reads it for tailored words before a release has no tool yet; the French, English and France packs were read by their author alone.
