# Sioul's own spam filter

A spam filter trained on your own mail, on your computer, when you ask; folded into a small table that every device reads, the phone too; never applied to people you know, codes, projects or your own mail; always saying why. Code: `crates/sioul-core/src/spam/` (what every device runs: the tokenizer, the header features, the table, the verdict, the label log), `crates/sioul-learn/` (the training, on a computer only: the corpus, the labels, fastText, the SVM, the calibration, the evaluation, the export), `crates/sioul-core/src/porch.rs` (where the verdict goes), `crates/sioul-app/src/spam.rs` and `qml/SpamFilter.qml` (the settings and "Train now"), `crates/sioul-cli/src/spam.rs` (`sioul spam`). The Porch's side: [porch.md](porch.md), "Spam"; the sharing's: [database.md](database.md), "Parts".

## Why
- **Before it, spam was your provider's word alone**, read from its `X-Spam-*` headers ([porch.md](porch.md), "Trust"). That word ranked above everything but blocked and forged mail: a provider's flag set aside a safe sender's message and a real code. Nothing learned from your own "Junk" and "Not junk".
- **Only your own mail teaches well.** Filters trained on other people's mail did about a hundred times worse than one-person filters in the TREC spam tracks; the labels you give, near the threshold, matter more than the choice of model (the research beside the build: `spam-filter/literature.md`, kept with the plan).
- **"It always says why"** ([design.md](design.md)): a linear model names what weighed, word by word and header by header.
- **The owner's design**, from Virtual Secretary's experience ([virtual-secretary.md](virtual-secretary.md)): a fastText language model trained on the desktop on your mail; a linear SVM on header features and the message's mean word vector; the phone gets the folded table; never spam for people you know, codes, project routes; two thresholds, spam, unsure, ham, yours to set; manual tags only spam or ham, folders and tags taken as they are; one model for every account; training on demand, from the mail settings.

## What it learns from
### The corpus
Sioul's sync keeps your mail, but whole messages since forever rarely fit a disk, and a provider purges its Junk folder (Gmail after a month). Training keeps its own copy of what it needs, per message, in `$XDG_DATA_HOME/sioul/spam/corpus/<account>/` (`sioul_learn::corpus`):
- where it is (account, folder and its role, UIDVALIDITY, UID), when the server received it (INTERNALDATE), its flags and keywords (`$Junk`, `$NotJunk`), its size;
- its header block (at most 32 KB), its MIME structure from BODYSTRUCTURE (the parts' types and names, attachments included, never their content);
- the text the Porch reads of it: the start of its text part, and its HTML when it has no text part or only a stand-in ("view it in your browser"), which part being which as mail-parser says for the Porch, so that training reads a message as the Porch does.

Every folder of every address, since forever, but Trash, Drafts, Sent and Gmail's All Mail. Read with Sioul's own IMAP session (EXAMINE, BODY.PEEK): nothing changes on the server. Incremental: where each folder stopped is kept after every batch, so a download stopped goes on there; a folder the server renumbered is read again. A record is never deleted when the server deletes its message: that is how junk outlives the provider's purge. Gzipped JSON lines, a few kilobytes a message. It never fills the disk: it stops before the free space falls under 1 GB, and says so.

### The labels
One label per message, spam or ham, from what is already there (`sioul_learn::labels`):
- **spam**: in a Junk-role folder (`folders::Role::Junk`: `\Junk`, or a name such as Junk, Spam, Courrier indésirable), or carrying `$Junk`;
- **ham**: in any other folder kept, or carrying `$NotJunk`;
- **what you did** beats both: the label log, the newest entry winning;
- **one message, one label**: copies (the same Message-ID in several folders or accounts) become one; when they disagree, the log beats the keywords, which beat the Junk folder, which beats the other folders; copies that disagree at the same level are left out.

The filter's own verdicts are never labels. The label log is `$XDG_STATE_HOME/sioul/spam/labels.jsonl`, one line per act ("Junk", "Not junk", "Not spam", blocking a sender from one of their messages): when, the account, the folder, UIDVALIDITY, UID, Message-ID, spam or ham, which act; never a word of the message ([porch.md](porch.md), "Trust"). The keywords `$Junk` and `$NotJunk`, set by Sioul or another mail client, are kept in each message's file name, so a correction made on the phone or in Thunderbird reaches the trainer through the server.

**A Junk folder is mostly your provider's own filing**: a model trained on it alone learns to copy the provider. Your own acts, logged and kept as keywords, are the labels that teach it something the provider did not know.

## Reading a message: the tokenizer
`spam::tokenize`, version 1 (`TOKENIZER`, stamped in the table: a table of another version is refused). It ports Virtual Secretary's pipeline, rewritten for Rust's `regex` (no look-around), and is the same code on the computer that trains and the phone that judges:
1. **Text**: the subject and the Porch's excerpt (the text part, the HTML read as text after a stand-in), quotes and signatures kept.
2. **Normalization**: invisible characters out, NFKC, lookalike letters folded (Cyrillic, Greek, Armenian, only in words holding a Latin letter or made only of lookalikes), lowercase, Virtual Secretary's table (accents, ligatures, spaces, dashes, quotes, fractions), the remaining non-ASCII out, spaces collapsed.
3. **Cleanup**: runs of `.`, `-`, `?` collapsed; a character repeated ten times and more, and long base64 blocks, out.
4. **Placeholders**: Virtual Secretary's 36 (an address, a link, a server's address, a date, a time, a price, a file of each kind, numbers with their units, a long number…) and two more, `_PHONE_` and `_IBAN_` (checked by its mod-97). A link also gives its domain (eTLD+1) to the header features.
5. **Splitting**: words on spaces and punctuation; French elisions split and dropped (l', d', qu', n', s', j', c', m', t', lorsqu', puisqu', jusqu', quelqu').
6. **Each word**: number words become `_NUMBER_`, five digits and more `_HASH_`, Virtual Secretary's stop words go, its French–English suffix stemmer cuts the rest ("lottery" reads "loteri").

Where it departs from Virtual Secretary, on purpose, is listed in `tokenize.rs` (every accent stripped, not only French ones; €, £ and ° kept as words; look-around rewritten; alternatives longest first). Tests: Virtual Secretary's `test-patterns.py` inputs as assertions, elisions, invisible characters, lookalikes.

## The header features
`spam::features`, version 1 (`FEATURES`), 41 named numbers, each explained by its name; read from the header block, the message's parts and links, and your provider's authentication results (`features::provider_results`: the account's trusted ids but Sioul's own stamp, since the training corpus keeps messages as the server holds them, without that stamp; the lanes, forgeries and senders still read Sioul's stamp first), never from the headers' order:
- **authentication**: SPF, DKIM and DMARC each pass, fail or none; ARC pass, fail; aligned (the sender verified);
- **domains**: Reply-To elsewhere than From, Reply-To at a free mailbox, the Message-ID made elsewhere, a display name showing another domain, links pointing elsewhere;
- **time**: dated ahead of its arrival, or well before (against INTERNALDATE, clipped), no Date, no Message-ID;
- **the way it came**: the number of `Received` hops, a server that gave no name;
- **its shape**: HTML only, images with almost no text, an archive, a program, an office document, a PDF or an image attached, the text's length in four steps, the number of links;
- **bulk**: `List-Unsubscribe`, `List-Id`, `Precedence: bulk` or `list`. Bulk marks gray mail, not spam: the model weighs it;
- **the provider's verdict**: seen, flagged, its score clipped, as one explicit input, only as your provider wrote it at arrival (`trust::read_spam_verdict`). `X-Spam*` and `X-Rspamd*` never enter as words.

Left out, because they would leak the label or how the mail was collected: To and Cc, the account itself, dates as such.

## The model, and how it folds
Trained on a computer (`sioul_learn::train`):
1. **The language model**: fastText (the `fasttext` crate 0.8.0, pure Rust, pinned) skip-gram on the tokenized corpus, ham and spam of every account: 100 dimensions, character n-grams of 3 to 6, 200 000 buckets, words seen 5 times and more, 5 epochs. Kept in `$XDG_DATA_HOME/sioul/spam/language.bin`, on this computer only.
2. **A message's vector**: the plain mean of its words' vectors (fastText's `get_word_vector`: a word's own row and its n-grams'; an unknown word, its n-grams' alone), never normalized, so that the model folds. The header features follow it; each dimension is standardized with the training messages' mean and deviation.
3. **The classifier**: a linear SVM (L2-regularized, squared hinge, dual coordinate descent as liblinear's: Hsieh et al. 2008), our own code; ham weighs 5 (calling ham spam costs five times more); its cost C chosen among 0.01, 0.1, 1 and 10 by the AUC on the newest fifth of the learning messages.
4. **The calibration**: Platt's A and B, fitted on scores each message got from an SVM trained on older mail only (forward folds), so that a probability means what it says on mail not seen.

**The fold.** Everything above is linear in the word vectors. With `u = w / σ` the SVM's embedding weights over the deviations, each vocabulary word scores `s_w = u · v_w` and each n-gram bucket `s_b = u · row_b`; the mean of a message's word scores is then exactly the SVM's term for the mean of its vectors. A message's decision value is `f = bias + (mean of its words' scores, 0 without words) − text_mean + Σ wₕ·(xₕ − μₕ)`, a word missing from the table scoring the mean of its n-grams' buckets, as fastText builds the vector of a word it never saw; its probability of being spam is `p = 1 / (1 + exp(A·f + B))`. The n-gram hashing is fastText's own (32-bit FNV-1a over sign-extended bytes of `<word>`, n-grams of whole characters), checked against `fasttext`'s `get_subwords` on the computer.

**The table** (`spam::table`, `$XDG_DATA_HOME/sioul/spam/table.bin`): each vocabulary word's score under a 64-bit hash of the word (FNV-1a), never the word; each bucket's score; the header weights and means; the bias, `text_mean`, Platt's A and B; the tokenizer's and features' versions, the n-grams' sizes; what the settings say of the training (when, on which device, from how many messages, its aggregate numbers). About 2 MB for 100 000 words. Checksummed, written whole or not at all. Before it is exported, the table's score must equal the full model's within 1e-4 on every test message, read back from its own bytes as every device reads it: else nothing is exported.

**The why**: each word's share of `f` and each header feature's, sorted; the verdict keeps the five words or placeholders and the three header features that pushed most toward spam (`spam::Why`). The words are said as the filter reads them: stems ("loteri"), placeholders by what they stand for ("a price").

## Where it acts: never on people you know
The verdict is the Porch's (`porch::triage`). In order:
1. **Blocked, forged, a borrowed name**: set aside as before, whatever any filter says.
2. **Protected, from every spam verdict, your provider's and Sioul's own**: mail from anyone but a stranger (safe, neutral or restricted: in your address books, on a list, let in from the screener); a code or a password you asked for; mail a project's routes take (a thread pulled into a project loses the filter's word too); your own verified mail; a message you said is not spam, here, on another device or in another mail client (`spam::labels::said_ham`: `$NotJunk`, or this device's label log). Mail nothing authenticates (SPF and DKIM failed, no DMARC or ARC vouching) has a stranger for its sender whatever the address it shows, and no protection.
3. **Your provider's verdict**, for a stranger's mail only: set aside, as before.
4. **Sioul's own**, for a stranger's mail only, when a table is there and the filter is not off.

A shielded address's mail found rude or hostile never gets the filter's word beside it: its why would show the words the shield hides.

## Two thresholds, three modes
- **Spam**, from `threshold_spam` (95 % unless you change it): when it acts, set aside, with "your own filter: probably spam (97%) — a price, a link; replies go to another domain"; when it only says, the message stays in its lane with the quiet word "probably spam".
- **Maybe spam**, from `threshold_unsure` (50 %) up to the spam threshold: the message stays in its lane, with the quiet word "maybe spam", whatever the mode.
- **Below**: nothing.

The modes (`[spam] mode`): **Off**, nothing said; **Show its verdict only** (`say`, the default: until you choose, a new table never moves mail); **Set spam aside** (`act`). One filter for every account. In Mail ▸ ⚙, "Your own spam filter": the mode, the two thresholds as percentages (each slider stops a point short of the other: unsure always below spam), then the training's block.

**What you see**: the word beside the subject, quiet, how sure and why when the pointer rests on it; the reason, in full, in the Reader's "Why it is here"; **Not spam** in the Reader for every message the filter called spam or maybe spam, and for mail set aside as spam: `$NotJunk` on the server, a line in the label log, the message back in its lane or its word taken away, for good, ten seconds to undo. **Junk** stays as always. Nothing is deleted; nothing moves on the server by itself.

**Notifications**: mail set aside is never told ([porch.md](porch.md), "Notifications"); a message left in its lane with a word is told as its lane says. One-click unsubscribing is refused for mail called spam (answering it says your address is read), offered for "maybe spam".

## Training on demand
On a computer, in Mail ▸ ⚙ ("Train now") or with `sioul spam train`; never on a schedule for now, never on a phone. The steps (`train::run`):
1. the corpus brought up to date from every address;
2. each message's label, words and header features, read as the Porch reads mail;
3. fastText learns your mail's words;
4. each message's vector;
5. the oldest 80 % to learn from, the newest 20 % to test on, by INTERNALDATE; the SVM's cost chosen;
6. the calibration;
7. the numbers, on the newest 20 %;
8. the table folded, checked, and compared with the one in place;
9. kept: the same model learned again from every message, the newest included (spam changes, and the filter that runs must have seen this month's), calibrated the same way, folded, checked, written. The numbers said are those of steps 5 to 7.

**The table changes only when no worse**: the new one replaces the one in place only if it takes no more ham for spam, at your spam threshold, than the one in place, on those of the newest 20 % the one in place never learned from (those that came after its own training: on older mail it would be judged on what it was taught). It replaces it too when there is none, when the one in place is another version's, and when nothing came since it was trained ("nothing tells them apart"). The one replaced, when this Sioul can use it, stays beside it as `table.prev.bin`. What happened, and why, goes to `$XDG_STATE_HOME/sioul/spam/trained.toml` (numbers only), which the settings say: the reason, the comparison, what the table in use learned from.

**In the window** (`crates/sioul-app/src/spam.rs`): on a thread of its own, below the window's priority (Linux: `setpriority`, which fastText's threads take), so the computer stays usable while every core learns. Its progress is said as it goes, at most a few times a second: "Messages fetched: 1,200 of about 15,000 (home · INBOX)", each folder's count summed over the addresses, the ones not reached yet counted from what the last download left on their server; then each step, with its count when it has one ("Learning your mail's words" has none: fastText says nothing until done). **Stop** stops it at its next step; what was downloaded is kept, and the next time goes on there. The block also says the last training (what it did with the table and why, the two tables compared on the newest messages the one in place never saw; what the table in use learned from, again from every message; the numbers of the same model learned from the oldest 80 %, at both thresholds with their intervals; the thresholds of then when you changed yours since), and the corpus (messages kept, from how many addresses, its size, the last download, the room left on the disk, the addresses it could not read). The window's Porch reads the new table at once.

**From the command line**: `sioul spam fetch [--account]` (the corpus only), `train [--no-fetch]`, `eval` (the table in place on the newest fifth), `status` (the corpus, the providers' verdicts as Sioul reads them, the last training), `why <file>` (below).

## Evaluation
Aggregates only (`sioul_learn::eval`), on the newest 20 %, by time, never by chance: a filter is used on tomorrow's mail, and spam drifts.
- the messages per label and per account, learned from and tested on;
- **ham taken for spam** and **spam caught**, at each of your two thresholds, each with its 95 % exact interval (Clopper–Pearson);
- the share called "maybe spam";
- the area under the ROC curve.

Nothing per message, except `sioul spam why <file>`: the probability, the score, the words and header facts that weighed. It is for you, in your terminal: the MCP server offers none of this ([ai.md](ai.md)).

**What the numbers cannot tell**: labels from Junk folders are mostly the provider's filing; the corpus's headers carry the provider's authentication results, where the Porch reads Sioul's own stamp first (a small skew); a test on your newest mail says how the filter did then, not how mail will change.

## Privacy
- **The corpus and the language model stay on the computer that made them**: never in a shared part (no store of the sharing covers them: tested), never shown to an AI agent (no MCP tool reads them, and the evaluation names no message), never in the repository nor its tests (tests train on invented mail at reserved domains). The language model holds the vocabulary of your mail in plain text; the corpus, the start of every message.
- **The table travels, sealed**: hashes and numbers, no word in plain text; still made from your mail (a guessed word can be tested against a hash), it is private as notes are: sealed in the sharing, never published.
- **The label log**: where and when, never what.
- **Nothing goes to any server**: training reads your mail over IMAP, read-only, and writes local files.

## The phone
- **It never trains**: `sioul-learn` and fastText are dependencies of the window and the command line on computers only (`[target.'cfg(not(target_os = "android"))'.dependencies]`); `cargo tree --target aarch64-linux-android -p sioul-app` lists neither.
- **Its table comes through the sharing**, the part "Spam filter" (on by default on every device): sealed apart, in every exchange, the background step's too ([database.md](database.md), "Parts"). The one it replaces, when this Sioul can use it, is kept as `table.prev.bin`; a table this Sioul refuses (another version's tokenizer or features, damaged) leaves the one before in use (`Table::cached`), and the settings say so.
- **Judging**: the window's Porch and the background process (`:steps`, new mail's notifications) read the same file through `Config::mail_sources` → `spam::Filter::of` → `Table::cached`: read once per process (two megabytes, about two and a half in memory), read again only when its size or time changes; each message's verdict kept per message file and per table (20 000 at most). No table, or the filter off: the Porch as before.
- **Its settings**: the mode and the thresholds travel with your settings; Mail ▸ ⚙ says where its table comes from ("In use: trained by desk on Tuesday 6 October at 10:00, from…") and what it measured then.

## Sharing the table
One store, `files/spam/table.bin` (`share::SPAM_TABLE`), in its own part, sealed apart as notes are (`Shape::Files`): the record says the content's hash, size and time; the content goes once, as a sealed file in `blobs/`. Not whole in a record (`Shape::Whole`): its two megabytes (some 3.6 MB once sealed in a line) would go again in every round its computer starts, and with Sioul keeping the folder itself the current round goes up whole at each change, every minute while a computer writes. Unlike notes and papers, it is read in every exchange, quick ones and the phone's background step included: one file of Sioul's own, quick to read, never in a shared storage ("All files access" is not needed). With Sioul keeping the folder itself, a device's own records go up before its sealed files, and the others' sealed files come down last, so that no large file on a slow line holds back the doses' answers ([database.md](database.md), "Kept in step by Sioul itself").

## Not built
- **Training by itself**, at night or when enough new labels came: on demand only, for now.
- **A sparse model to measure against**: a linear model on hashed character n-grams is what the one-person filters of the TREC tracks were best with; no study tests an unsupervised fastText centroid with an SVM, in time order, at a low false-positive rate. Both are cheap in Rust: to measure on your own mail before trusting the centroid alone.
- **The why in the words of the message**: it says the stems the filter read ("loteri"), not "lottery".
- **Labels from what you do elsewhere**: mail answered, flagged, tied to a task or a project, as ham.
- **Tokenizer v2**: Virtual Secretary's photography units fire on everyday text ("10 to 20" read as a file size): consistent between training and judging, to drop in a next version.

## Tested
- `sioul-core`: the tokenizer (Virtual Secretary's inputs, elisions, invisible characters, lookalikes), the features, the table (a score by hand, written and read back, refused when damaged or of another version, the last good one in use when the newest is refused), the verdict (`porch::tests::spam_never_touches_who_you_know`, `the_learned_verdict_says_or_acts`), the label log.
- `sioul-learn`: the SVM against hand-solved optima, Platt, Clopper–Pearson, the corpus appended and read back after a crash, the labels' precedence, a whole training on invented mail (AUC, the fold within 1e-4, the replacement rule four ways), fastText's hashing against the crate's own; the corpus download against GreenMail ([building.md](building.md)).
- `sioul-sync`: the table from the computer to the phone in quick exchanges only, byte for byte, the one before kept, nothing of it readable in the folder, never the language model nor the corpus (`share::tests::the_spam_table_travels_alone_in_every_exchange`); a sealed file failing or slow, going up or coming down, never holding back a device's records (`remote::tests::a_slow_or_failing_sealed_file_never_holds_back_the_records`).
