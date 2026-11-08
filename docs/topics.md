# Mail topics

A topic is a standing part of life, such as Health, Tax, Choir, Book club or Consulting, that Sioul writes on a message as a quiet word. A message has one topic at most, and most mail has none. Topics are never folders: mail stays where it is, and a topic is a way to look at the one mailbox, as a filter and a search condition. What else a message relates to stays outside topics: the person it concerns ([persons.md](persons.md)), its area ([areas.md](areas.md)) and the money it moves. Topics and projects ([projects.md](projects.md)) are independent: a project is something to complete, a topic a category, and neither holds the other. A project's mail has topics like any other mail, and a project says nothing of a message's topic.

The evidence behind these choices is in [research/mail-topics.md](research/mail-topics.md), and the rule it gives is finding 61 of [research.md](research.md).

## Your word

Topics start with you: the mail kept here rarely holds enough sorting of its own for a model to learn from, so a topic tags mail by your acts and rules until it has enough (20 messages from 3 senders, or 50 from one office), and nothing asks you to tag anything.

- **Topic…** on a message gives it a topic, changes it or takes it off: in the reader, under the sender's line; in a message's menu (right click, ⋮, or a long press on a phone); and in the bar above a selection, for several messages at once, chosen with Ctrl and Shift, or on a phone by **Select** in a row's menu, then a tap on each. The same in the mailbox and in each address's folders. The topic the messages have is ticked, when they all have the same. **New topic…** makes the topic and gives it in one act.
- **Rename or remove…**, at the end of the same menu (8 November 2026): each topic's name becomes a field, Enter renames it, **Remove** beside it takes it away (its word leaves every message; Undo puts it back); **Done** goes back to giving. The topics are made, renamed and removed where mail is read, and Mail ▸ ⚙ ▸ Topics stays for their seeds, their senders' rules and the groups Sioul noticed.
- **Always <topic> for this sender** and **Never…**, in the same menu once the message has a topic: the sender's mail takes the topic, or never does, unless you said otherwise of a message.
- **A conversation follows** the first of its messages you gave a topic: a reply takes it, by its References and In-Reply-To.
- **The order of words** (`sioul_core::topics::rules`): your word on the message (a topic, or none), your rule for its sender, its conversation, the topic's seeds (its folders, its contacts' categories), then a started preset's rules (below). "Never" keeps a topic from the last three. A sender counts only when Sioul's checks authenticate the message, so forged mail never borrows a sender's topic.
- Each act is said in one sentence with **Undo**, which writes your word on each message back as it was.

Your word is kept in a log per device, `$XDG_STATE_HOME/sioul/topics/labels/<device>.jsonl`, a line per act (a message's topic by its Message-ID, a sender's rule by its address), never rewritten; the newest line wins. The logs travel sealed with the settings, once every device has the store, as the persons' store waits for every device. The topics themselves are `[[topic]]` tables in the configuration:

```toml
[[topic]]
id = "choir"
name = "Chorale"
seeds = { folders = ["Chorale"], categories = ["Chorale"] }
```

## Topic presets

Sioul ships topics to start with, in `presets/topics.json`: Bank, Taxes, Health, Insurance, Housing, Phone and internet, School and childcare, Administration, Orders and deliveries, Travel and transport, Online services, Social networks and Newsletters. Each has its name in English, French, German, Spanish and Italian, its area, whether agents start closed (Health does), and its rules:

- **the site presets' categories and types it takes** (`presets/sites.json`: a bank's site is Bank's, a social network's is Social networks'); a site of several categories goes to the earliest topic in the file that takes one of them, so that the tax office, an administration of taxes, is Taxes';
- **sender domains** beyond the site presets, each with a public source; a domain of the site presets is never listed again;
- **List-Id patterns**, `*` for any list or newsletter (Newsletters).

A preset's rules hold only for a sender Sioul's checks verify, so forged mail never takes its topic. They come after your own words and your topics' folders and categories: its own domains and List-Id patterns first, then a site's category, then any list, so that a bank's newsletter is the bank's.

**Day one**: once the mail kept here holds a hundred messages, the presets that match at least five of them start as topics, once; the Porch says which, in one card, until you put it away. The others stay offered in Mail ▸ ⚙ ▸ Topics, one tap each. A preset started is your own topic (`[[topic]]` with its `preset`): rename it, remove it; an update of the presets never changes it back, and only its rules follow the file. As with the site presets, the newest file by its `checked` date wins, the copy built in or one in `$XDG_DATA_HOME/sioul/presets/topics.json`; `tools/check-topic-presets.py` checks that each domain is well formed and sourced, that none sits in two topics or among the site presets, and that each category and type exists.

## Where topics show

- **On a mail row**, a quiet word after the subject, when the row has no louder word (spam's, forged mail's); no colour, no count. In the mailbox and, since 8 November 2026, in each address's folders too: a folder's received mail (its inbox, your own folders, the archive) is read as the mailbox reads it (`topics::rules::Lookup::tagged`, once a look, only when there are topics), never Sioul's doubt there, since a folder has no topic filter.
- **In the reader**, the topic and why: "Chorale · given by you", "· by its sender", "· by its conversation", "· from the folder Chorale".
- **The Topic filter**, beside the filter by area once there are topics (on a phone, in the folders' sheet): "All topics", or one topic's mail alone. A topic is a way to look at the one mailbox, never a place: nothing moves.
- **Mail ▸ ⚙ ▸ Topics**: each topic with its seeds in words and its senders' rules (× takes one off), **Rename** and **Remove** (its word leaves every message; Undo puts it back), and **Add a topic**. On a computer, **Groups Sioul noticed**: **Look for groups** gathers the mail no word of yours names yet by its words (NMF, below), each group shown by its senders' names (the most frequent few), its size and two subjects (codes and links masked); **Name it** makes a topic and gives it to the whole group in one act. What it shows (`sioul_learn::groups::noticed`), so that a small mailbox shows nothing misleading:
  - no group before a thousand messages are kept here: "Groups come once more mail is kept here" (a mailbox of 525 measured on 5 November 2026 gave groups that mixed unrelated kinds of sender);
  - then five groups at most, the largest first; none whose coherence (NPMI of its words) is under 0.3, their words seldom meeting; none whose mail the presets started already tag at 50 % or more (before day one, those day one would start), since those topics take it already;
  - a group of one sender is no group: it becomes an offer, **Always a topic…**, which gives that sender the rule "Always <topic>" (the topic made when none has that name), three offers at most, persons too since you choose the topic (a choir's leader is a person), none for a sender with a rule of yours already, none with a name proposed;
  - a group's name to start from comes only from a topic preset or a site category that holds 60 % of it or more, in your language, with one sentence on how; else none, and **Name it** asks for one. Never a sender's name: a group of mixed senders named after one of them misleads (on that mailbox, an AI service beside a social network and a person), and topics are categories while persons are their own dimension.

Topics and projects ([projects.md](projects.md)) are independent: a project is something to complete, a topic a category. A project's mail has topics like any other.

## Measured first

Topics are measured on your own mail from the command line too. Three commands read every message Sioul keeps on this computer (the training reads the corpus instead: below, "The model"), each once (copies in several folders or accounts, by Message-ID, are one): every folder of every account's Maildir, Sent and the archives included, through each folder's index, so that no message is read again for what its headers say. Junk, Trash and Drafts are never read. What the spam filter's labels call spam stays out, though its copy is elsewhere: the spam filter's corpus ([spam-filter.md](spam-filter.md)) serves for its labels alone, with your own "Spam" and "Not spam". A `$Junk` keyword keeps a message out too, and so does forged mail. The commands change no message, table or setting; reading a folder adds to its index what it lacks, as the window does.

### `sioul topics seeds`

It lists the topics Sioul would propose from what you already sorted, by the messages' headers alone. Nothing is asked: each of these is a seed, a rule written before, which names past messages.

- **Your folders**, a Gmail label being one. The server's own folders (the inbox, Sent, the archive, Gmail's `[Gmail]/…`), folders of fewer than 20 messages and folders unused for a year seed nothing.
- **Your contacts' categories**: the mail of the addresses on the cards of a category, when Sioul's checks find the sender authenticated, as the Porch does. For mail of yours (in Sent, or from one of your addresses), its recipients' categories.
- **The topic presets' mail** (below), named in your language: a preset joins a folder or a category of its name or its senders. Their matches come last, by count, the senders proven, with how each preset took them (its own senders, their sites, as lists), and those of five messages or more, which would start on day one.

A folder and a category join into one topic when they share senders, or have one name. Two folders never join, nor two categories, since you kept them apart. A topic is named after its folder, else its category, as you wrote it.

A message two topics name goes to the more specific one, the one whose seeds name fewer messages, and the larger topic says how many it gave. A topic exists once it has mail: at least 20 messages from at least 3 senders, or 50 from fewer, such as a single office. Below that it waits. Twelve topics are proposed at most, the largest.

It says first what each source brought: the messages read and those of yours, the folders and files, what was left out (junk, trash and draft folders, spam, forged mail), and how many messages the folders and the categories name, yours among them. `--json` gives the same as data. Each topic has an id, an ASCII slug of its name (`impots` for "Impôts").

### `sioul topics eval`

It trains fastText's classifier over the proposed topics, softmax over all of them at once, and tests it in time order. A message is read as one line: its sender's words first, then the spam filter's words of the message.

- **The sender's words** are added only when Sioul's own checks proved who sent it (DMARC, or a DKIM signature of the sender's domain): its address under a hash (`from:` and 12 hexadecimal digits), its domain (`from-dom:`) and its mailing list (`list:` and its List-Id). Forged mail never borrows a sender's topic.
- **Time order**: the oldest 80 % of the mail is learned, the newest 20 % tested. Within the learned part, its newest fifth, scored by a model of the rest, sets each topic's sure threshold, and fits softmax's temperature when a class "none" was learned. Without one, fastText's own probabilities are kept: fitted on seeded mail alone, where every message has a topic, the temperature would only make them surer, mail no seed names included.
- **The bands**: a topic is sure from the lowest probability whose precision against the seeds reaches 0.9, never under 0.7; maybe from 0.5; below, the message has no topic. A topic that never reaches that precision is never sure. By default a sure topic on mail no seed names counts against its precision, which makes it a floor, since the seeds miss the mail you never filed; `--seeded-precision` counts only confusions between topics.
- **Two ways to say "no topic"**: a class "none" learned from a share of the mail no seed names (each share of `--none-weights`, 0.1, 0.25 and 0.5 by default, tried on the learned part's newest fifth, the best kept), or the top topic's probability under its sure threshold alone. The better on the test is kept; the threshold, the simpler, when they are within half a point of macro F1.
- **The sender's words and the bucket count**: the way kept is trained again without the sender's words, and at each other count of `--buckets` (50 000 and 200 000 by default, the first being the table's). The sender's words stay unless leaving them out does better; the fewest buckets within half a point of the best are kept.
- **The numbers**, per topic and in all, against the seeds: precision, also among seeded mail alone, recall, coverage (the share of mail with a sure topic), macro F1, and the same for senders the learned part never had. They are kept as numbers only, never shown as a score in the window.
- **The fold**: the way kept is folded into the table every device would read, in f32 and in f16. The f32 table must give fastText's own probabilities within 10⁻⁴; the command says how far the f16 one is, how many topics or bands it changes on the tested mail, and the table's size in each.
- **Churn**: the same model learned from the mail a week older, and the share of its sure topics on the tested mail that the newer one changes.
- **The alternative**: with `--model2vec <folder>`, a nearest centroid on a static embedding, tested the same way. The folder is one of Model2Vec's, such as Minish Lab's potion-multilingual-128M (`model.safetensors`, 512 MB, and `tokenizer.json`), fetched by you; Sioul reads it and downloads nothing. A message's vector is the mean of its tokens' rows, so the centroid folds into one number per piece and topic. Sioul's reading of its tokenizer is close to Hugging Face's, not the same: the precompiled NFKC table is left out, which composed text does not notice.

`--quick` trains the threshold alone, with the sender's words, at the first bucket count. `--threads`, `--dim` and `--epochs` change fastText's settings (all cores, 100 and 25 by default).

### `sioul topics groups`

It gathers the same mail into groups, four ways side by side, so that you can see which gives groups you recognise. On a first real mailbox, NMF's groups read as categories a person recognises, the senders' graph gave groups of one sender each, better as "Always <topic> for this sender", and LDA mixed unrelated senders: Mail ▸ ⚙ ▸ Topics uses NMF. A group is made of its messages' subjects and own text, what they quote, their attributions and signatures left out, without the tokenizer's placeholders (`_URL_`, `_PRICE_`…) nor HTML's and CSS's words; the spam filter keeps them all.

- **The senders**: the correspondents Sioul's checks proved, and the people you write to, two of them tied each time they meet in a conversation or among one message's recipients; the communities of that graph, by Louvain's modularity. A group is the mail of a community's senders.
- **Model2Vec and k-means**, with `--model2vec <folder>`: each message's static embedding, gathered by k-means, k chosen between 4 and 16 by the silhouette.
- **LDA**, latent Dirichlet allocation by collapsed Gibbs sampling, and **NMF**, the factorisation of the messages' TF-IDF, as baselines, with `--k` topics (else k-means' k, else 10).

Each group shows its size, its senders, yours among its messages, the words that say it (c-TF-IDF for the first two, each model's own for the others), two of its subjects with codes and sign-in links hidden, its coherence (the NPMI of its words over all the mail, from −1 to 1) and the seed topic most of it is in. Each way shows its mean coherence and its agreement with the seeds: the share of seeded mail in its groups whose group's main seed is its own. Groups under `--least` messages (10) are not shown.

The commands print names and numbers, and subjects only in `groups`.

## The model

Trained on every device in the spam filter's weekly run, after its own training ([spam-filter.md](spam-filter.md), "Every device trains its own"), by hand too (`sioul topics train`; `sioul_learn::topics::model::train_here`):

- **From the device's corpus since forever** (`topics::read_corpus`), not from the mail kept here: every message the spam filter's download brought to this device, each once (its copies in several folders or accounts, by Message-ID, are one), its folders named as you read them; a device keeps a few hundred messages (525 measured on 5 November 2026), its corpus thousands. Left out as the measuring commands leave them out: Junk, Trash and Drafts, what the spam filter's labels call spam or a `$Junk` keyword marks, forged mail by Sioul's own checks of it. Sent is not in the corpus, so your own mail does not teach topics. A header block cut at 32 KB is read with its fields fetched alone ([spam-filter.md](spam-filter.md), "The corpus", format 2).
- **Deterministic**: one thread, the spam filter's fixed seed, and the corpus read in a fixed order (by Message-ID), so that two devices holding the same corpus and the same labels make the same table at every training, whatever table each has in use (`model::tests::two_devices_with_the_same_corpus_make_the_same_table`: a first training, then a second with another table in use on each). Your word, your topics and your rules travel as before; what a device guesses never does, and never teaches its own next table either (below, "Labels"). The churn against the table in use is said in what the training did, never written in the table. Whether the new table replaces the one in use is judged against that one, so two devices whose tables in use differ (one kept the last table another device sent) may take the same new table a week apart; once both took it, they stay alike.

- **Labels** by the order of words, each at full weight: your word on a message, your rules for senders, conversations, the topics' seeds and the presets' rules. Only those: never a table's own guesses, sure or not (until 7 November 2026 the last table's sure topics were learned from too, half of them; with every device training its own, each device would then have learned from its own lineage of tables, and the devices' topics would have drifted apart: review of 7 November 2026, F4).
- **The gate**, topic by topic: a topic enters the table once it has 20 messages from 3 senders, or 50 from fewer (one office), counting those words. Below it, the topic has no class at all and its mail is left out of the training, never learned as "no topic": your words and its rules sort it, and Mail ▸ ⚙ ▸ Topics says so ("Sorted by your tags and senders until it has more mail"). With no topic through the gate, there is no table.
- **The guards**: in a topic of three senders or more, one sender makes at most 20 % of its examples; one conversation at most 20 % in any topic; the newest are kept.
- **The model**: fastText's softmax over the topics through the gate and a class "no topic" learned from a quarter of the mail nothing names; learned from the oldest 80 % in time order, its temperature and sure thresholds fitted on the newest fifth of that, tested on the newest 20 %, then folded in f16 (below).
- **Replaced only when no worse**: on the newest mail neither table learned from (after the table in use was made, else the whole test), the new table's F1 of sure topics against the labels must be within half a point of the one in use for every topic both know, and it must change fewer than 2 % of the sure topics the one in use shows on mail nothing else names. The first table needs neither. What the training did is said topic by topic, with the outcome.
- **Judging, on every device** (`sioul_core::topics::judging`): the device's own table, `$XDG_DATA_HOME/sioul/topics/table.bin`. While a device does not learn (an older Sioul, a new device not trained yet, one whose switch is off or whose training the guard stopped), the newest table travels in the spam part beside the spam filter's table (`files/topics/table.bin`), sealed apart as that table is, once every device has the topics' store; while every device learns, neither table travels, and a device that learns never takes another's in ([spam-filter.md](spam-filter.md), "Sharing the table and the label logs"). New mail is judged after each fetch, on a phone in its background step too; the mail kept here, 2,000 messages at a look on a computer and 500 at a step on a phone, when a table arrives. What a table judged stays on the device, a line per message by its Message-ID (`$XDG_CACHE_HOME/sioul/topics/judged/`), never shared; a new table starts anew.
- **The model's word comes last** (`rules::with_model`): under your word, your rules, the conversation, the seeds and the presets; never over your "no topic" nor a sender's "Never". A sure topic shows on the row; a doubt ("maybe") only under its topic's filter, as "Perhaps <topic>", and never counts as a label. The reader says "Sioul's guess".
- **Search**: "Topic is" and "Topic is not" in More… read a message's topic as the rows show it, sure; a doubt is no topic there. A server's search cannot say a topic.

## The table

What every device would read is `sioul_core::topics::table`, format 1: each word fastText knows under a 64-bit hash, with one number per class, the sum of its rows' scalars; each character n-gram bucket with its scalars; the end of the line's; the temperature; each topic's sure threshold. A message's scores are the mean of its rows' scalars, counted as the spam filter's table counts them, and its probabilities their softmax. The scores are kept in f16 or f32; the file says which, and ends with a checksum. Nothing of a message's text is in it.
