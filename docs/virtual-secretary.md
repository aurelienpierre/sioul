# Virtual Secretary, with Sioul

[Virtual Secretary](https://github.com/aurelienpierreeng/VirtualSecretary) sorts mail with Python filters (`00-imap-spam.py`, `01-imap-taxes.py`…) that cross-check mail, contacts and calendars. Sioul keeps those filters working, in two modes.

## Mode A: Virtual Secretary keeps sorting on the server
Nothing changes on its side: it moves messages into IMAP folders and tags them. Sioul reads where a message was put, and turns the folder into a proposal (`rules::FolderMapping` in the core):

```toml
[[virtual_secretary.map]]
folder = "INBOX.Money.Taxes"
project = "taxes-2025"     # `case`, as written before, reads too

[[virtual_secretary.map]]
folder = "INBOX.Services.Notifications"
shelf = "notifications"

[[virtual_secretary.map]]
folder = "INBOX.spam"
junk = true
```
Sioul does not move mail on the server while Virtual Secretary does, so the two never fight.

## Mode B: Virtual Secretary's filters run inside Sioul
- **How it runs**: a separate Python process loads the filter files unchanged, with globals `imap`, `smtp` and `carddav` that behave like Virtual Secretary's. They talk to Sioul's core over JSON-RPC on standard input and output.
- **Why a separate process**: Python stays out of the Rust core, and the projects keep separate licences.
- **What it turns into**: the filters' actions become Sioul proposals, listed below.

| Virtual Secretary | In Sioul |
|---|---|
| `imap.get_objects(mailbox, n)` | the account's messages (the inbox is the Porch's arrivals) |
| `imap.run_filters(filter, action, runs)` | each card is filtered; how many times each filter ran on a message is kept in Sioul's database instead of hidden log files |
| `email.is_in(query, field)`, `get_sender()`, `headers`, `email["X-…"]`, `attachments`, `get_body()`, `get_date()`, `age()` | the same, from the card |
| `email.ip`, `email.domains` | the server route from `Received` |
| `spf_pass()`, `dkim_pass()`, `arc_pass()`, `authenticity_score()`, `is_authentic()` | Sioul's trust results |
| `is_newsletter()`, `is_mailing_list()` | the card's list detection |
| `move(folder)` | a proposal to file or assign a project, through the folder mapping |
| `tag(k)`, `untag(k)` | a label |
| `spam(folder)` | junk: set aside, and the classifier learns |
| `delete()` | the trash, recoverable for 30 days |
| `mark_as_read/answered/important` | flags |
| `query_referenced_emails()`, `query_replied_email()` | thread lookups in Sioul's store (later) |
| `smtp` (autoresponders) | drafts in the Outbox: Sioul never sends without you |

- **"LEARN" filters** (read-only, heavy) run on demand.
- **Permissions**: every filter starts as a proposer, and you allow a filter to act alone, filter by filter.

## What the spam filter took from Virtual Secretary
Sioul's own spam filter ([spam-filter.md](spam-filter.md)) started from Virtual Secretary's code, read on 7 October 2026 in its public repository (`src/core/nlp.py`, `patterns.py`, `utils.py`, `language.py`, `src/protocols/imap_object.py`, `examples/`, `docs/`). Why the filter departs from it, with the evidence: [research/spam-filter.md](research/spam-filter.md).

- **No learned spam filter to reuse.** Virtual Secretary's SVM (scikit-learn's `SVC`, an RBF kernel, C = 15, libsvm's Platt probabilities) sorts mail into folders, one label per IMAP folder, from the mean word vector of the subject and the plain text, with no header; its documentation reports 87 to 90 % accuracy on 6,500 French and English messages, measured on one random hold-out of 5 %, accuracy only (`docs/index.md`). Its fastText model was trained for a web search engine, never on mail. Spam is handled by rules (below). The classifier's path has been broken at its HEAD since May 2026 (`Pool` is used in `nlp.py` without its import).
- **The text pipeline, which Sioul ported** (`spam::tokenize`):
  1. normalisation: lower case, NFKC, then a table of replacements (apostrophes, French accents only, spaces, dashes, decorations, ligatures, ellipses, guillemets, fractions, arrows), then every remaining character outside ASCII dropped;
  2. whitespace: runs of blank lines become one paragraph break, and a single line break between two words a space (a PDF's wrapped line);
  3. a prefilter over the whole text: runs of dots, dashes and question marks collapsed; a character repeated ten times or more, BBCode, markup within brackets and base64 runs of 256 characters or more taken out;
  4. 36 placeholders, in an order that carries meaning: addresses and handles (`_USER_`), links (`_URL_`), IP addresses, files by kind (code, database, image, document, text, archive, program), dates and times, keyboard shortcuts, paths, numbers with their units (photographic ones among them: exposure, shutter speed, sensitivity, aperture), ordinals, prices, resolutions; then compound words joined by underscores;
  5. words split (BlingFire), then each word trimmed of punctuation; a run of six hexadecimal characters or more becomes `_HASH_`, a number `_NUMBER_`; stop words go; and a stemmer of its own maps French and English onto shared roots (action, acteur and actor to "act") by ordered suffix rules with minimum lengths.

  Sioul's tokenizer keeps that design, rewritten for Rust's `regex` crate, which has no look-around, where nearly every pattern of Virtual Secretary is guarded by look-behind and look-ahead. It adds `_PHONE_` and `_IBAN_` (Virtual Secretary has no placeholder for phone numbers: a run of five digits or more becomes a fixed number, then `_HASH_`), splits French elisions, and strips every accent rather than French ones only ([spam-filter.md](spam-filter.md), "Reading a message: the tokenizer").
- **Pitfalls found in its code and history**, which the port avoids, or keeps on purpose for parity:
  - regular expressions that blew up: patterns split because merging them "triggers infinite loop", a guard added so that the unit patterns "run within decent runtimes", timeouts of 15 seconds per pattern and 10 per word. Rust's `regex` runs in linear time, but it can match differently from a backtracking engine;
  - the order of the pipeline carries meaning: addresses before links (links hold addresses), file names before paths, dates before partial paths, shortcuts before paths ("f4"), long runs of digits before units;
  - `~` becomes `-` before paths are found, so `~/x` paths are missed;
  - the hash pattern runs before the number pattern, so numbers of six digits or more, and words made only of the letters a to f ("decade", "facade"), become `_HASH_`;
  - NFKC before the ASCII table turns "½" into "12", and "™" into a capitalised "TM" after lower-casing;
  - French stop words keep their accents while the text has lost them, so they never match;
  - one random hold-out of 5 %, without a seed, and accuracy only: no basis for spam thresholds, since spam drifts in time;
  - an RBF SVM's file holds its support vectors, "a few dozens of MB";
  - `get_body` fails on a message with neither a text nor an HTML part;
  - its tests print rather than assert; Sioul turned the inputs of `src/tests/test-patterns.py` into assertions.
- **Its authenticity score, and the weaknesses Sioul's checks avoid** (`src/protocols/imap_object.py`): SPF, DKIM and ARC each score from −2 to 2, summed, so that "one valid check compensates one fail"; the failures of mail older than six months are forgiven, since keys and DNS records change. Its weaknesses: no DMARC and no alignment, so a DKIM pass for any domain makes a forged From "authentic"; SPF passes when any hop's address is allowed, internal relays included, instead of the address that reached your provider; the sender's MX hosts stand in for its HELO; a domain without MX is taken for a forgery; any address starting with "192." is taken for a private one. Sioul checks SPF, DKIM, DMARC with alignment and ARC with `mail-auth`, on the server that handed the message to your provider ([porch.md](porch.md), "Trust"); its spam filter reads old mail's failures as unknown beside a signature, a rule close to Virtual Secretary's six months ([spam-filter.md](spam-filter.md), "The header features"). Virtual Secretary's documentation says that "basic SPF and DKIM checks get rid of 90% of spam emails"; by 2023, 89 % of unwanted mail passed SPF, DKIM or DMARC (Cloudflare), so Sioul's filter reads a pass as almost no evidence of good mail ([research/spam-filter.md](research/spam-filter.md), Q2).
- **Its public spam example** (`examples/common/00-imap-spam.py`, 2022): addresses and IP addresses on allow lists are not spam, those on deny lists are; `X-Spam-Flag: YES` is spam; `Precedence: bulk` without `List-Unsubscribe` is spam. It reads `email.ip`, which the message object does not have, so it fails on every message.
- **Two of its rules are not rules in Sioul**: in Virtual Secretary, a message without a Message-ID, and bulk mail without `List-Unsubscribe`, are spam. In Sioul both are facts the filter weighs, learned from your mail rather than fixed: "no Message-ID" is one of its header features, and `List-Unsubscribe`, `List-Id` and `Precedence` are features that mark gray mail, not spam ([spam-filter.md](spam-filter.md), "The header features").
