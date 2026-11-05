# Mail topics

A topic is a standing part of life, such as Health, Tax, Choir, Book club or Consulting, that Sioul writes on a message as a quiet word. A message has one topic at most, and most mail has none. Topics are never folders: mail stays where it is, and a topic is a way to look at the one mailbox, as a filter and a search condition. What else a message relates to stays outside topics: the person it concerns ([persons.md](persons.md)), its area ([areas.md](areas.md)), its project ([projects.md](projects.md)) and the money it moves.

The evidence behind these choices is in [research/mail-topics.md](research/mail-topics.md), and the rule it gives is finding 61 of [research.md](research.md).

## Measured first

Topics are measured on your own mail before anything shows in the window. Two commands read the spam filter's corpus, the mail `sioul spam fetch` keeps on this computer ([spam-filter.md](spam-filter.md)), and change nothing: no message, table or setting.

### `sioul topics seeds`

It lists the topics Sioul would propose from what you already sorted. Nothing is asked: each of these is a seed, a rule written before, which names past messages.

- **Your folders**, a Gmail label being one. The server's own folders (the inbox, the archive, junk, Gmail's `[Gmail]/…`), folders of fewer than 20 messages and folders unused for a year seed nothing.
- **Your projects**: the mail their routes, ties and conversations take ([projects.md](projects.md)). Forged mail seeds no project.
- **Your contacts' categories**: the mail of the addresses on the cards of a category, when Sioul's checks find the sender authenticated, as the Porch does.

Seeds that share senders join into one topic: a folder and a category, a folder and a project, two projects such as "Taxes 2024" and "Taxes 2025". Two folders never join, nor two categories, since you kept them apart. A topic is named after its folder, else its category, as you wrote it; else after its largest project, its years left out.

A message two topics name goes to the more specific one, the one whose seeds name fewer messages, and the larger topic says how many it gave. A topic exists once it has mail: at least 20 messages from at least 3 senders, or 50 from fewer, such as a single office. Below that it waits. Twelve topics are proposed at most, the largest.

`--json` gives the same as data. Each topic has an id, an ASCII slug of its name (`impots` for "Impôts").

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

`--quick` trains the threshold alone, with the sender's words, at the first bucket count. `--threads`, `--dim` and `--epochs` change fastText's settings (all cores, 100 and 25 by default). The commands print names and numbers, never a message.

## The table

What every device would read is `sioul_core::topics::table`, format 1: each word fastText knows under a 64-bit hash, with one number per class, the sum of its rows' scalars; each character n-gram bucket with its scalars; the end of the line's; the temperature; each topic's sure threshold. A message's scores are the mean of its rows' scalars, counted as the spam filter's table counts them, and its probabilities their softmax. The scores are kept in f16 or f32; the file says which, and ends with a checksum. Nothing of a message's text is in it.
