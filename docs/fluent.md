# Strings: Fluent

Every sentence Sioul shows is a *message* in a Fluent file, one file per language: `crates/sioul-core/locales/en/sioul.ftl` and `crates/sioul-core/locales/fr/sioul.ftl`, built into the programs. This page shows how to read and write those files, how the code asks for a message, and how to add one in both languages. Why Fluent, and the house conventions (counts in words, `$n`, `$count`, French typography), are in [Languages](i18n.md); they are shown here in use, not repeated.

[Project Fluent](https://projectfluent.org/) is Mozilla's translation system. Its [syntax guide](https://projectfluent.org/fluent/guide/) teaches the whole language; Sioul uses a small part of it, below. The code reads the files with the `fluent-bundle` crate, in `crates/sioul-core/src/i18n.rs`.

## Reading a `.ftl` file
- **A message** is a name, `=`, and its text. The name is lowercase words joined by hyphens, starting with the part of Sioul it belongs to (`ui-` the window, `task-` tasks, `set-` Settings, `share-` the sharing, `dnd-` do-not-disturb…):

    ```ftl
    stopped-done = Done
    ```

- **A variable** is written `{ $name }`, and the code gives its value:

    ```ftl
    stopped-at = Where you stopped, { $when }
    ```

- **A selector** chooses words by a value, often a number. Each variant is in brackets; the one marked `*` is the default, and is required. A number is matched by the language's plural categories (`one`, `other`…), or exactly (`[1]`):

    ```ftl
    summary-total = { $Count } { $n ->
            [one] letter
           *[other] letters
        } came.
    ```

    A message on several lines continues on lines that start with spaces.

- **Attributes** are other forms of one message, each on its own line under it, after a dot. Sioul uses them for the forms of a word the code picks: the capitalised and feminine numbers (`.cap`, `.f`, `.fcap`), a word with its article (`.a`), short names (`.short`):

    ```ftl
    code-kind-code = code
        .cap = Code
        .a = a code
    ```

- **Comments**: `##` heads a group of messages (`## Dates. $weekday and $month come from the names below.`); `#` before a message is about that message alone.
- **Invisible characters** that matter (a space alone, as the thousands' separator) are written as a quoted literal with an escape, so that they can be seen. In French: `thousands-separator = {"\u202F"}`, a narrow no-break space, and `money-positive = { $value }{"\u00A0"}€`, a no-break space. In prose, the French file holds the characters themselves (`’`, and U+202F before `:`).

Sioul's files use no other part of Fluent: no *terms* (messages whose name starts with `-`), no functions such as `NUMBER()`. Unicode isolation marks around variables are off (`set_use_isolating(false)` in `i18n.rs`): they showed as stray characters in terminals, and no right-to-left language is translated yet.

## Each language its own grammar
The code gives the facts; each language's file decides the words. The same message, in `en/sioul.ftl`:

```ftl
summary-total = { $Count } { $n ->
        [one] letter
       *[other] letters
    } came.
```

and in `fr/sioul.ftl`:

```ftl
summary-total = { $Countf } { $n ->
        [one] lettre est arrivée
       *[other] lettres sont arrivées
    }.
```

French takes the feminine number (`$Countf`: "Une lettre", "Deux lettres") and agrees the verb; English needs neither. The code gave both the same arguments (`Translator::counted`): `$n`, the number for the selector, and the number in words, `$count`, `$Count`, `$countf`, `$Countf` ([i18n.md](i18n.md#conventions)).

The plural categories are each language's own, from Unicode's CLDR: in English `one` is 1 alone, in French it is 0 and 1. A language can also match a value exactly. French writes the first of the month "1er":

```ftl
date-long = { $weekday } { $day ->
        [1] 1er
       *[other] { $day }
    } { $month } à { $time }
```

## How the code asks for a message
**From QML**, through the bridge object, always by a literal name:
- `sioul.text("stopped-done")`: a message without arguments;
- `sioul.textWith("stopped-at", "when", card.stopped.when)`: one argument, by name and value;
- `sioul.textArgs("overlap-today", JSON.stringify({ first: …, second: … }))`: several, as a JSON object.

**From the window's Rust** (`crates/sioul-app/src/backend.rs`):
- `tr()` is the session's `Translator`, made once (the configuration's language, else the system's), so a change of language takes effect when Sioul starts again;
- `tr().text("done-closed", None)` for a message without arguments;
- `say("sync-held-back", &[("account", account.id.clone()), ("free", …)])` with arguments. `textWith` and `textArgs` go through `say` too.

**From the core**, a function takes `tr: &Translator` and calls `tr.text(id, Some(&args))`. The arguments are a `FluentArgs`, filled with `args.set("name", value)`; `tr.counted(n)` returns one already holding `$n` and the number in words. `Translator` also has `when`, `date`, `day`, `money`, `decimal` and others, which build dates and amounts from the messages for weekdays, months and separators. The command line has its translator in `Session` (`s.tr`), sync makes its own (`sioul_sync::translator`).

Three things to know:
- **Numbers given as text become numbers.** QML gives every argument as text; `say` turns a text that is a whole number (`"2"`) into a number, so that `[one]` and `[other]` choose by it. A text such as `"007"` stays as it is.
- **A missing message shows its name.** `Translator::text` takes the English message when the language lacks one, and the message's name itself when English lacks it too. A name on the screen (`stopped-donne`) is a mistyped or missing message.
- **Names built at run time** (`format!("weekday-{}", …)` in `i18n.rs`, `"area-" + kind` in QML) and names kept in lists (`NewMenu.qml`'s `kinds`, with `"new-stopped"`) cannot be checked by `tools/check-messages.py`, which looks for literal names in calls. Write names literally in `text(…)` calls wherever the code allows it.

## Adding a message in both languages
1. **Find its place.** Messages are grouped by part, in the same order in both files. Look for its neighbours: `grep -n '^stopped-' crates/sioul-core/locales/*/sioul.ftl`.
2. **Name it** with its part's prefix and a few literal words; make sure the name is free in both files.
3. **Write it in English** in `en/sioul.ftl`, plainly and literally ([research.md](research.md), §10): no idioms, buttons that say what they do, small numbers in words, nothing that commands or counts what was not done ([tasks.md](tasks.md)).
4. **Write it in French** in `fr/sioul.ftl`, at the same place, with French typography: a narrow no-break space (U+202F) before `:` `;` `!` `?` and inside « », the apostrophe `’`, a comma in decimals. Copying the space from a neighbouring message is the simplest way to type it.
5. **Ask for it by its literal name** in QML or Rust.
6. **Check**:
    - `python3 tools/check-messages.py`: every name the code asks for exists in each language, and French typography holds; it exits with 1 when something is missing, and CI runs it. `--unused` lists the messages no literal call asks for, and the prefixes of names built at run time;
    - `cargo test -p sioul-core i18n`: every file parses, every language has every English message (`every_language_parses_and_has_every_message`), and no message is defined twice (`no_message_twice`, since Fluent would silently keep the first).

## Adding a language
- **A folder and a file**: `crates/sioul-core/locales/<language>/sioul.ftl`, with every message of the English file.
- **One line in `LANGUAGES`** (`crates/sioul-core/src/i18n.rs`), which builds the file into the programs: `("fr", include_str!("../locales/fr/sioul.ftl"))`.
- **The messages the code reads for the language itself**: the numbers in words (`count-0` to `count-12`, with their attributes as the language needs them), `decimal-separator`, `thousands-separator`, `money-positive` and `money-negative`, the weekdays and months, and `qt-locale`, the locale Qt formats dates with in the window (`en_GB`, `fr_FR`).
- **The tests read `LANGUAGES`, and the checker every folder of `locales/`**: both cover the new language once it is there. The checker's typography rules are French's alone.

How translations are drafted and reviewed, and Weblate, which supports Fluent: [i18n.md](i18n.md#translating).

## What Fluent does not do here
- **The detectors** read the languages of the mail you receive, whatever the interface's language: one-time codes, dates in letters, the shield's word lists. Their words are in the code that detects (`crates/sioul-core/src/codes.rs`, `shield.rs`), not in the `.ftl` files ([i18n.md](i18n.md#detection-is-not-the-interfaces-language)).
- **Android's own few words**: what Android shows before Sioul's Rust has said anything. Two are Android resources in English only (`android/package/res/values/`: the card's description in the launcher's list of widgets, and the tile's name, "Pause"), since Qt's Gradle template keeps the English resources alone ([android.md](android.md#the-card-on-the-home-screen)). The Java keeps a few more in English and French, chosen by the phone's language: a channel's name or a reminder's words before Rust gave them, the card before Rust wrote it, a share that failed before Rust could be asked (`DoseAlarms.java`, `WakeAlarms.java`, `EventAlarms.java`, `StepService.java`, `HomeCard.java`, `ShareActivity.java`). Everything else on Android is worded by Rust, through Fluent, and handed to Java ready to show.
