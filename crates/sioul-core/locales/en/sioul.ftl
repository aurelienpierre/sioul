## Sioul, in English.
## Every sentence the core shows. Variables: $n is a number for plural
## selection; $count / $Count are that number in words (lowercase, capitalised);
## $countf / $Countf the same for feminine nouns, for languages that need them.

## Numbers in words, the way a calm sentence says them (0 to 12; digits beyond).
count-0 = no
    .cap = No
count-1 = one
    .cap = One
count-2 = two
    .cap = Two
count-3 = three
    .cap = Three
count-4 = four
    .cap = Four
count-5 = five
    .cap = Five
count-6 = six
    .cap = Six
count-7 = seven
    .cap = Seven
count-8 = eight
    .cap = Eight
count-9 = nine
    .cap = Nine
count-10 = ten
    .cap = Ten
count-11 = eleven
    .cap = Eleven
count-12 = twelve
    .cap = Twelve

decimal-separator = .

## What came, when the Porch opens.
summary-nothing = Nothing came. Nothing needs you.
summary-total = { $Count } { $n ->
        [one] letter
       *[other] letters
    } came.
summary-codes = { $Count } { $n ->
        [one] code is
       *[other] codes are
    } ready above.
summary-projects = { $Count } { $n ->
        [one] belongs
       *[other] belong
    } to { $k ->
        [one] a project
       *[other] projects
    }: { $projects }.
summary-project-part = { $title } ({ $count })
summary-people = { $Count } { $n ->
        [one] is
       *[other] are
    } from people you know.
summary-screener = { $Count } { $n ->
        [one] is from someone new and waits
       *[other] are from someone new and wait
    } in the screener.
summary-filed = { $Count } { $n ->
        [one] newsletter or notification was
       *[other] newsletters and notifications were
    } filed.
summary-set-aside = { $Count } forged or spam { $n ->
        [one] message was
       *[other] messages were
    } set aside.

## Why a message is where it is.
reason-dmarc-pass = the sender's domain vouches for it (DMARC pass)
reason-dmarc-fail = the sender's domain says it did not send it (DMARC fail)
reason-dkim-pass = it carries a valid signature (DKIM pass)
reason-both-failed = both SPF and DKIM failed
reason-nothing-proves = nothing proves who sent it
reason-no-results = no authentication results from your provider
reason-forged = forged: set aside
reason-spam = marked as spam by your provider's { $source }
reason-spam-score = marked as spam by your provider's { $source }, score { $score }
reason-learned-spam = your own filter: probably spam
reason-unsure = your own filter: maybe spam
reason-learned-ham = your own filter: probably not spam
reason-moved-to-junk = moved into your Junk folder by your own filter, as you chose; “Not spam” brings it back to the inbox
spam-chip-spam = probably spam
spam-chip-unsure = maybe spam
spam-chip-ham = probably not spam
reason-expires-soon = { $akind } that expires soon
reason-unverified-code = it looks like { $akind }, but the sender is not verified: use it only if you just asked this site for it; fake codes are a phishing trick
reason-project = project { $title }: { $why }
reason-newsletter = a newsletter or a mailing list
reason-automatic = sent by an automatic address
reason-first-message = the first message from this sender
reason-not-authenticated = its sender could not be verified (SPF and DKIM failed): treated as someone you do not know
reason-known = from someone you know
reason-from-yourself = sent from one of your own addresses
route-sender-domain = sender's domain: { $value }
route-sender = sender: { $value }
route-subject = subject: { $value }
route-text = text: { $value }
route-attachment = attachment: { $value }
route-thread = in a conversation of this project

## Short-lived secrets.
code-kind-code = code
    .cap = Code
    .a = a code
code-kind-password = password
    .cap = Password
    .a = a password
code-kind-reset = password reset
    .cap = Password reset
    .a = a password reset
code-kind-link = sign-in link
    .cap = Sign-in link
    .a = a sign-in link
code-kind-confirm = confirmation link
    .cap = Confirmation link
    .a = a confirmation link

## How far the sender can be believed.
trust-verified = verified
trust-unverified = not verified
trust-forged = forged

## The Porch.
right-now-title = Right now
right-now-item = { $Kind } from { $sender } ({ $trust })
right-now-item-code = { $Kind } from { $sender } ({ $trust }): { $code }
right-now-valid = valid about { $minutes } minutes
right-now-valid-hours = { $hours ->
    [one] valid about an hour
   *[other] valid about { $hours } hours
}
right-now-valid-days = { $days ->
    [one] valid about a day
   *[other] valid about { $days } days
}
lane-people = From people you know
lane-screener = Someone new, in the screener
lane-filed = Filed: newsletters and notifications
lane-set-aside = Set aside
lane-review = Caught by your own spam filter
porch-closed = The Porch opens { $when }. Meanwhile, what comes is checked and sorted.
porch-open-hint = (`sioul porch --open` shows it now.)
no-date = no date

## Admin windows.
window-open-until = An admin window is open until { $time }.
window-next = The next admin window opens { $when }.
window-none = No window in the coming week.

## Errors.
error-no-mail = No mail to read yet: give --maildir, or accounts with a maildir in the configuration (examples/config.toml).
error-no-store = No notes folder: give --store, or case_store in the configuration.
error-no-windows = No admin windows in the configuration (examples/config.toml).

## Dates. $weekday and $month come from the names below.
when-on = on { $date }
date-long = { $weekday } { $day } { $month } at { $time }
date-short = { $weekday } { $day } { $month } { $time }
weekday-1 = Monday
    .short = Mon
weekday-2 = Tuesday
    .short = Tue
weekday-3 = Wednesday
    .short = Wed
weekday-4 = Thursday
    .short = Thu
weekday-5 = Friday
    .short = Fri
weekday-6 = Saturday
    .short = Sat
weekday-7 = Sunday
    .short = Sun
month-1 = January
    .short = Jan
month-2 = February
    .short = Feb
month-3 = March
    .short = Mar
month-4 = April
    .short = Apr
month-5 = May
    .short = May
month-6 = June
    .short = Jun
month-7 = July
    .short = Jul
month-8 = August
    .short = Aug
month-9 = September
    .short = Sep
month-10 = October
    .short = Oct
month-11 = November
    .short = Nov
month-12 = December
    .short = Dec

## Money: thousands grouped, the sign before.
thousands-separator = ,
money-positive = €{ $value }
money-negative = −€{ $value }

## Budgets and reserves, at a glance.
month-year = { $month } { $year }
date-day = { $weekday } { $day } { $month }
date-day-month = { $day } { $month }
budgets-title = Budgets, { $month }
reserves-title = Reserves
period-month = monthly
period-year = yearly
budget-line = { $title } ({ $period }): { $so_far } so far, { $projected } expected by { $end }, target { $target }: { $verdict }.
verdict-better = better than planned, by { $gap }
verdict-as-planned = as planned
verdict-short = short of plan by { $gap }
budget-earmarked = { $amount } of it set aside for what is planned later.
budget-opening = { $opening } carried in from before.
budget-drawn = { $reserve } covers { $amount } on { $end }.
budget-swept = { $amount } go to { $reserve } on { $end }.
reserve-line = { $title }: { $balance } today. This month, { $month_done } used and { $month_planned } still planned. By { $year_end_date }, { $year_end }.
reserve-quiet = Nothing is planned from it.
mail-title = Proposed from mail
mail-line = { $date } · { $budget } · { $amount } · { $label }
mail-line-foreign = { $date } · { $budget } · { $amount } { $currency }, to convert · { $label }
mail-added = added
error-no-ledger = No budgets yet: give --file, or put sioul-budgets.toml at the root of your notes folder (examples/sioul-budgets.toml).

## Accounts.
security-tls = encrypted from the start (TLS)
security-starttls = encrypted by STARTTLS
account-server = { $host }, port { $port }, { $security }
account-found = Found: { $server }, { $by }.
account-by-provider = from your provider's own settings
account-by-ispdb = from Thunderbird's list of providers
account-by-guess = a guess: check it before going on
account-by-google = Google's own servers, since Google keeps this address's mail
account-login = Login: { $login }
account-gmail-hint = Google refuses your account password here: make an app password for Sioul at myaccount.google.com/apppasswords (it needs 2-Step Verification), and give it instead.
account-app-password-hint = If this account uses two-step verification, it may want an app password.
account-password-prompt = Password for { $address } (it goes to your system keyring, nowhere else):
account-connected = Connected: the inbox holds { $n ->
        [one] one message
       *[other] { $count } messages
    }.
account-added = Added as “{ $id }”. Its mail goes to { $path }.
account-exists = An account named “{ $id }” already exists.
account-address-exists = { $address } is already set up, as “{ $id }”.
account-unknown = No account named “{ $id }”.
account-removed = “{ $id }” removed, with its password. Its mail stays in { $path }.
account-portal-added = “{ $id }” added: it opens { $url } in your browser.
account-password-saved = Password kept in your system keyring.
account-has-password = password in the keyring
account-signed-in-google = signed in with Google (the access in the keyring)
account-no-password = no password yet: `sioul account password { $id }`
account-trust = Sender checks: trusts { $ids }.
account-trust-learning = Sender checks: learned from your mail at the first sync.
account-portal = opens { $url }

## Sync.
sync-new = { $account }: { $n ->
        [0] nothing new
        [one] one new message
       *[other] { $count } new messages
    }.
sync-first = { $account }: { $n ->
        [0] no mail
        [one] one message
       *[other] { $count } messages
    } from the last { $days } days.
sync-learned = { $account }: your provider records its checks as { $id }; Sioul trusts that name for this account from now on.
sync-not-learned = { $account }: your provider does not seem to record its checks, so senders stay “not verified” for now.
sync-error-address = This is not an email address.
sync-error-not-found = No settings found for { $detail }: give the server yourself (--host).
sync-error-no-server = { $account }: no server in the configuration.
sync-error-no-password = { $account }: no password in the keyring yet.
sync-error-keyring = { $account }: the system keyring did not answer ({ $detail }).
sync-error-network = { $account }: the server cannot be reached ({ $detail }).
sync-error-tls = { $account }: the server's certificate could not be checked, so nothing was sent ({ $detail }).
sync-error-login = { $account }: the server refused the password ({ $detail }).
sync-error-app-password = { $account }: this server wants an app password, made in your account's settings at the provider, not your usual password. Gmail: myaccount.google.com/apppasswords.
sync-error-server = { $account }: the server refused a command ({ $detail }).
sync-error-disk = { $account }: the mail could not be written to disk ({ $detail }).
sync-error-elsewhere = { $account }: the server named an address on another server ({ $detail }); Sioul did not go there, and sent no password.
sync-said-folder-name = { $account }: a folder needs a name.
sync-said-no-folder = { $account }: there is no folder “{ $detail }”.
sync-said-not-kept = { $account }: Sioul keeps no copy of the folder “{ $detail }”.
sync-said-purpose = { $account }: “{ $detail }” is one of the account's own folders (inbox, sent, drafts, junk, trash, archive), and stays.
sync-said-not-fetched = { $account }: Sioul did not fetch this message from a server.
sync-said-unknown-folder = { $account }: Sioul does not know the folder this message is in.
sync-said-renumbered = { $account }: the server renumbered this folder; fetch the mail again.
sync-said-gone = { $account }: this message is no longer on the server.
sync-said-open-first = { $account }: open the message first.
sync-nothing = Nothing to fetch yet: your accounts are added in Accounts, at the bottom left.
watch-started = Watching { $accounts }. The codes and links you ask sites for come as notifications. Ctrl+C stops.
watch-line = { $time } · { $line }
done-closed = Done. What comes next waits for the next window.
done-nothing = Nothing was shown, so nothing to close.
notify-copy = Copy the code

## The window.
ui-porch = Porch
ui-budgets = Budgets
ui-accounts = Accounts
accounts-tab-yours = Your accounts
accounts-tab-add = Add an account
accounts-tab-keys = Encryption
account-service-mail = Mail
account-service-dav = Calendars, tasks and contacts
account-service-google = Google: calendars, contacts and tasks
account-switch-help = Off, it keeps its settings and is neither synced nor shown.
account-on = “{ $id }” is on again.
account-off = “{ $id }” is off: kept with its settings, neither synced nor shown.
account-details = Server and folders
account-settings = Settings for this address
scout-button = What this server offers
scout-asking = Asking the server…
scout-title = What { $server } offers
scout-mail = Mail: { $server }
scout-have = here already
scout-mail-add = Add the mail, with the password of its calendars
scout-mail-add-other = Add it with another password
scout-dav = Calendars, tasks and contacts: { $server }
scout-dav-add = Add them
scout-nextcloud = { $product } { $version }
scout-apps = Its apps:
scout-apps-unasked = Its apps are listed once its calendars are here: their password asks for them.
scout-not-in-sioul = not in Sioul yet
scout-in-sioul = in Sioul
scout-nothing = The server says nothing more.
scout-app-calendars-contacts = Calendars, tasks and contacts
scout-app-files = Files
scout-app-notes = Notes
scout-app-talk = Talk (calls and chats)
scout-app-deck = Deck (boards)
scout-app-webmail = Its webmail
scout-app-bookmarks = Bookmarks
scout-app-tables = Tables
scout-app-forms = Forms
scout-app-photos = Photos
senders-help = Each address, number, card or category is on one list at most. Anyone in your address books is neutral until you choose otherwise; strangers are in none of them and on no list; the blocked never reach you. When each list reaches you is the grid above. Someone blocked from a message lands here too.
ui-sync-now = Fetch mail now
ui-syncing = Fetching mail…
ui-synced-at = Mail fetched at { $time }.
ui-open-anyway = Open it anyway
ui-not-open = Opening it outside a window shows everything waiting. Codes always show.
ui-done = Done for now
ui-copy = Copy
ui-copied = Copied.
ui-let-in = Let this address in
ui-let-in-done = { $address } is let in.
ui-close = Close
ui-ok = OK
ui-reasons = Why it is here
ui-attachments = Attachments: { $names }
ui-no-text = This message has no text.
ui-text-safe = Shown safely: nothing remote loads, nothing runs.
reader-opening = Opening the message…
ui-add-account = Add a mail account
ui-address = Email address
ui-find-server = Find the server
ui-finding = Looking for the server…
ui-host = Server
ui-port = Port
ui-security = Encryption
ui-login = Login
ui-password = Password
ui-password-note = The password goes to your system keyring, nowhere else.
ui-app-passwords = Make an app password
ui-connect = Connect and add
ui-connecting = Connecting…
ui-remove = Remove
ui-remove-confirm = Remove “{ $id }”? Its password leaves the keyring; its mail stays on disk and on the server.
ui-cancel = Cancel
ui-portal-name = Name
ui-portal-url = Web address
ui-add = Add
ui-open = Open
ui-no-accounts = No account yet: add the first one in "Add an account".
ui-watching = Waiting for new mail.
ui-no-budgets = No budgets yet: put sioul-budgets.toml at the root of your notes folder.
ui-keys = Ctrl+1 to Ctrl+0 places · Ctrl+N new · Ctrl+Z undo · F5 refresh

## Budgets, itemized.
fig-so-far = So far
fig-expected = Expected by { $end }
fig-target = Target
fig-opening = Carried in
fig-earmarked = Set aside for later
badge-better = Better than planned by { $gap }
badge-as-planned = As planned
badge-short = Short of plan by { $gap }
fig-today = Today
fig-delay = Arrives in
fig-this-month = This month
fig-month-flows = { $done } used, { $planned } planned
fig-by-date = By { $date }
fig-pace = At this pace
fig-pace-quiet = nothing planned from it
mail-section = From mail
mail-none = No mail about money yet.
kind-received = Received
kind-paid = Paid
kind-order = Order
kind-bill = Bill
kind-refund = Refund
note-recorded = Counted in { $budget }
note-duplicate = Same payment as { $other }
note-preset = Counted by the preset “{ $preset }”
note-preset-amount = Replaces the preset “{ $preset }” for its month once added
note-no-amount = The message gives no amount.
note-doubtful = From someone new and not verified: check it is genuine before counting it.
ui-add-line = Add
ui-not-payment = Not a payment
ui-line-added = Added to { $budget }.
ui-line-ignored = Left out of the budgets.

## Blocking and borrowed names.
reason-blocked = you blocked this sender
reason-impersonation = it uses the name { $brand } but comes from { $domain }: set aside
ui-block = Block…
ui-block-title = Block this sender?
ui-block-text = Their mail will be set aside for good: never shown, never counted. Nothing is deleted, and you can unblock them in Accounts.
ui-block-address = Block { $address }
ui-block-domain = Block everyone at { $domain }
ui-blocked = { $entry } is blocked.
ui-blocked-title = Blocked senders
ui-unblock = Unblock
ui-unblocked = { $entry } is unblocked.
blocked-none = Nobody is blocked.

## Accounts and messages, itemized.
account-row-server = Server
account-row-checks = Sender checks
account-row-mail = Mail kept in
account-row-web = Web address
account-checks-learning = learned from your mail; your provider may record none
ui-to = To
ui-cc = Cc
ui-attachments-title = Attachments
ui-save = Save
ui-saved = Saved in { $path }.
ui-attachments-closed = Set aside: its attachments are not opened.
ui-quote-show = Show the quoted message ({ $n ->
        [one] one line
       *[other] { $n } lines
    })
ui-quote-hide = Hide the quoted message
size-b = { $n } B
size-kb = { $n } KB
size-mb = { $n } MB

## Account priority.
lane-low = Less important accounts
summary-low = { $Count } { $n ->
        [one] message from a less important account waits
       *[other] messages from less important accounts wait
    } folded at the bottom.
reason-low-priority = it came through an account you ranked below the others
priority-above = More important
priority-average = Normal
priority-below = Less important
account-row-priority = Priority
ui-priority-set = { $id }: { $priority }.

## Sender checks.
verify-done = { $account }: { $n ->
        [0] nothing to check
        [one] one message checked
       *[other] { $count } messages checked
    }.
reason-dmarc-fail-no-policy = DMARC failed, but the sender's domain asks nothing of receivers when it does
reason-dmarc-fail-list = DMARC failed on a mailing list, which rewrites what it relays
checks-by-sioul = Checked by Sioul when the message arrived:
checks-by-provider = Checked by your provider ({ $id }):
checks-none = No check recorded: nothing proves who sent it.
check-line = { $protocol }: { $outcome }
check-line-domain = { $protocol }: { $outcome } ({ $domain })
check-pass = valid
check-fail = failed
check-softfail = doubtful
check-neutral = neutral
check-none = absent
check-temperror = could not be checked now
check-permerror = badly set up by the sender
check-other = unclear
check-policy = policy { $policy }
protocol-iprev = Reverse DNS

## The antivirus.
ui-attachments-scan = Each attachment is checked by the antivirus (ClamAV) before it opens or is saved.
scan-running-any = Checking the attachment with the antivirus…
scan-running = Checking { $name } with the antivirus…
scan-clean = { $name }: no threat found; it opens.
scan-infected = { $name }: the antivirus found { $threat }. Nothing was opened, and the copy is deleted.
scan-error = The attachment { $name } could not be checked ({ $detail }).

## Reading.
ui-images-hidden = Images are not shown: nothing loads from the network.
ui-link = Link: { $url }
ui-quote-history = Show the earlier messages

## Folders.
folder-inbox = Inbox
folder-drafts = Drafts
folder-sent = Sent
folder-archive = Archive
folder-junk = Junk
folder-trash = Trash
folder-all = All mail
folder-other = Folder

## Writing.
compose-no-recipient = Nobody to send it to yet: add an address.
compose-from = From
compose-date = Date
compose-subject = Subject
compose-to = To
compose-cc = Cc
compose-forwarded = ---------- Forwarded message ----------
compose-attribution = On { $date }, { $name } wrote:

## Sending.
send-error-refused = { $account }: the sending server refused the message ({ $detail }).
send-error-not-filed = { $account }: sent, but its copy could not be filed in Sent ({ $detail }).
send-error-message = { $detail }

## The mail client.
mail-done = Done.
mail-sent = Sent.
mail-you-to = You, to { $names }
mail-to-whom = To { $names }
mail-nobody = nobody yet
mail-no-subject = (no subject)
mail-empty = Nothing here.
mail-nothing-recent = Nothing in the last two weeks.
mail-not-found = Nothing matches “{ $query }”.
mail-unread = { $n ->
    [one] One message you have not read.
   *[other] { $Count } messages you have not read.
}

## The mail page.
ui-mail = Mail
ui-write = Write
ui-undo = Undo
ui-reply = Reply
ui-reply-all = Reply to all
ui-forward = Forward
ui-archive = Archive
ui-trash = Delete
ui-delete-for-good = Delete for good
ui-junk = Junk
ui-not-junk = Not junk
ui-not-spam = Not spam
ui-spam = Spam
ui-spam-all = Spam for all
ui-not-spam-all = Not spam for all
ui-more = More
ui-mark-read = Mark as read
ui-mark-unread = Mark as unread
ui-flag = Flag
ui-unflag = Remove the flag
ui-flagged = Flagged
ui-selected = { $n } selected
ui-clear-selection = Clear the selection
ui-drag-count = { $n ->
    [one] One message
   *[other] { $n } messages
}
ui-move-to = Move to…
ui-move-title = Move to which folder?
ui-show-source = Show the source
ui-drafts-here = Drafts
ui-more-folders = More folders
ui-no-mail-accounts = No mail account yet: add one in Accounts.
ui-folders = Back to the folders
ui-search = Search
ui-earlier = Earlier messages
ui-discard = Delete the draft

## Searching the mail by conditions (mailsearch.rs, MailSearch.qml): the mail filters' own conditions.
ui-search-more = More…
ui-search-more-tip = Search with conditions: who sent it, to whom, when, what it says or carries, where it is.
ui-select = Select
search-title = Search
search-results = Results
search-help = Mail by who sent it, to whom, when, what it says or carries. The junk and the trash are left out unless a folder condition names them.
search-add = Add a condition
search-remove = Take this condition out
search-match-all = Mail that meets all of them
search-match-any = Mail that meets any of them
search-and = and
search-clear = Clear
search-show = Show the messages
search-change = Change the search
search-back = Back to the search
search-placeholder = Words to find
search-placeholder-who = A name, an address, @domain
search-placeholder-size = 5 MB, 500 KB
search-make-filter = Make it a filter…
search-sentence-empty = Say what to look for.
search-sentence = Mail { $phrases }.
search-join-all = {", "}
search-join-any = {", or "}
search-phrase-anywhere-contains = with “{ $value }”
search-phrase-anywhere-not-contains = without “{ $value }”
search-phrase-from-contains = from …{ $value }…
search-phrase-from-not-contains = not from …{ $value }…
search-phrase-from-is = from { $value }
search-phrase-from-is-not = not from { $value }
search-phrase-to-contains = to …{ $value }…
search-phrase-to-not-contains = not to …{ $value }…
search-phrase-to-is = to { $value }
search-phrase-to-is-not = not to { $value }
search-phrase-cc-contains = copied to …{ $value }…
search-phrase-cc-not-contains = not copied to …{ $value }…
search-phrase-cc-is = copied to { $value }
search-phrase-cc-is-not = not copied to { $value }
search-phrase-subject-contains = whose subject contains “{ $value }”
search-phrase-subject-not-contains = whose subject does not contain “{ $value }”
search-phrase-subject-is = whose subject is “{ $value }”
search-phrase-subject-is-not = whose subject is not “{ $value }”
search-phrase-body-contains = whose text contains “{ $value }”
search-phrase-body-not-contains = whose text does not contain “{ $value }”
search-phrase-attachment-exists = with an attachment
search-phrase-attachment-missing = without attachments
search-phrase-attachment-contains = with an attachment named …{ $value }…
search-phrase-attachment-not-contains = with no attachment named …{ $value }…
search-phrase-attachment-type-is = with { $kind } attached
search-phrase-attachment-type-is-not = without { $kind } attached
search-phrase-date-before = arrived before { $day }
search-phrase-date-after = arrived after { $day }
search-phrase-date-between = arrived between { $day } and { $until }
search-phrase-size-above = larger than { $size }
search-phrase-size-below = smaller than { $size }
search-phrase-sender-is = whose sender is { $who }
search-phrase-sender-is-not = whose sender is not { $who }
search-phrase-list-exists = from a newsletter or a list
search-phrase-list-missing = from no newsletter nor list
search-phrase-list-contains = from the list …{ $value }…
search-phrase-list-not-contains = not from the list …{ $value }…
search-phrase-account-is = in { $account }
search-phrase-account-is-not = not in { $account }
search-phrase-folder-is = in { $folder }
search-phrase-folder-is-not = not in { $folder }
search-phrase-mark-is-read = already read
search-phrase-mark-is-not-read = not read yet
search-phrase-mark-is-flagged = flagged
search-phrase-mark-is-not-flagged = not flagged
search-phrase-mark-is-answered = answered
search-phrase-mark-is-not-answered = not answered yet
search-found = { $n ->
    [0] Nothing here matches.
    [one] One message here.
   *[other] { $n } messages here.
}
search-found-all = { $n ->
    [0] Nothing matches.
    [one] One message.
   *[other] { $n } messages.
}
search-found-more = The newest { $shown } of { $n } are shown: another condition narrows them down.
search-server-looking = Looking on the servers for the mail kept only there…
search-server-found = { $n ->
    [one] One more on the servers.
   *[other] { $n } more on the servers.
}
search-server-none = Nothing more on the servers.
search-server-more = { $n ->
    [one] Perhaps one more on the servers, not shown: another condition narrows them down.
   *[other] Up to { $n } more on the servers, not shown: another condition narrows them down.
}
search-server-failed = { $account }: its server could not be searched ({ $detail }).
search-open-first = Open it first: Sioul keeps none of it here yet.
search-on-server = on the server
search-on-server-tip = Kept on its server only: opening it brings it here.
search-bringing = Bringing it from the server…
search-place = { $folder } · { $account }

## The writing window.
ui-to-placeholder = Addresses, separated by commas
ui-cc-bcc = Cc, Bcc
ui-bcc = Bcc
ui-bcc-note = Hidden from the other recipients
ui-attach = Attach
ui-preview = Preview
ui-edit = Edit
ui-markdown-hint = Markdown: **bold**, *italic*, - lists, [link](https://…)
ui-send = Send
ui-writing = Name and signature…
ui-your-name = Your name, as recipients see it
ui-signature = Signature
ui-signature-note = In Markdown; it goes below what you write.
ui-writing-saved = { $id }: name and signature saved.
compose-title-new = New message
compose-title-reply = Reply to { $names }
compose-title-forward = Forward
compose-saved = Saved at { $time }.
compose-below-quote = Below your text: { $name }’s message of { $date }, quoted.
compose-below-forward = Below your text: { $name }’s message of { $date }, with its attachments.
compose-kept = The draft is kept in Drafts.
compose-not-a-file = { $path } is not a file.
compose-bad-address = “{ $entry }” is not an address.
compose-attaching = { $count ->
    [one] Attaching the file…
   *[other] Attaching { $count } files…
}
compose-wait-files = Not yet: the files shared are still being attached.

## Writing from other applications: a share, a mailto: link (outside.rs).
handed-attached = { $count ->
    [one] The file is attached.
   *[other] The { $count } files are attached.
}
handed-missing = Could not be attached: { $files }.
handed-a-file = a file
handed-stopped = Sioul stopped while copying it: share it again
handed-refused = given by its place on the phone, which Sioul does not take from another app
handed-no-account = Something was shared to Sioul, but no mail account sends yet: add one in Accounts, then share it again.
share-write-from = Write from { $address }
share-address-gone = This address is no longer in Sioul.

## Undo, ten seconds.
undo-archived = Archived.
undo-trashed = Moved to the trash.
undo-deleted = Deleted for good.
undo-junked = Moved to junk.
undo-not-junk = Back in the inbox.
undo-not-spam = Not spam, for good: it stays in its lane.
undo-moved = Moved to { $folder }.
undo-many-archived = { $n } messages archived.
undo-many-trashed = { $n } messages moved to the trash.
undo-many-deleted = { $n } messages deleted for good.
undo-many-junked = { $n } messages moved to junk.
undo-many-not-junk = { $n } messages back in the inbox.
undo-many-not-spam = { $n } messages not spam, for good: they stay in their lanes.
undo-spam-kept = Spam, for good: it stays in the Junk folder.
undo-review-all-spam = { $n ->
        [one] One message: spam, for good.
       *[other] { $n } messages: spam, for good.
    }
undo-review-all-not-spam = { $n ->
        [one] One message: not spam, for good, back in its lane.
       *[other] { $n } messages: not spam, for good, back in their lanes.
    }
undo-many-moved = { $n } messages moved to { $folder }.
undo-moved-across = { $n ->
    [one] Moved
   *[other] { $n } messages moved
} to { $folder } of { $account }.
undo-sending = Sending in ten seconds.
undo-discarded = Draft deleted.
undo-done = Undone.
undo-send-undone = Not sent: the draft is open again.
undo-too-late = Too late to undo: it is done.
mail-message-gone = This message is no longer there; fetch the mail again.
mail-draft-gone = This draft is no longer there.
mail-not-sent = Not sent; the draft waits in Drafts. { $detail }
mail-no-account = No mail account to send from: add one in Accounts.

## Contacts and calendars.
agenda-today = Today
agenda-tomorrow = Tomorrow
agenda-all-day = All day
agenda-from = From { $time }
agenda-until = Until { $time }
agenda-untitled = (no title)
agenda-nothing = Nothing planned in the next two weeks.
contacts-none = No contact yet.
account-row-dav = Contacts and calendars
dav-found = Contacts: { $contacts } · Calendars: { $calendars }
dav-none = none
dav-added = Account “{ $id }” added: its contacts and calendars come now.
dav-report = { $account }: { $sent } sent, { $received } received, { $removed } removed.
dav-conflict = Changed on both sides: the server’s version is kept; yours is among the earlier versions (Settings ▸ Your folder and sharing ▸ Show earlier versions), to put back if you prefer it.
dav-answered-elsewhere = { $answer ->
        [accepted] You had already answered “{ $summary }” on another device: accepted. Your calendar keeps that answer; to change it, answer again from the invitation.
        [tentative] You had already answered “{ $summary }” on another device: maybe. Your calendar keeps that answer; to change it, answer again from the invitation.
        [declined] You had already answered “{ $summary }” on another device: declined. Your calendar keeps that answer; to change it, answer again from the invitation.
       *[other] You had already answered “{ $summary }” on another device. Your calendar keeps that answer; to change it, answer again from the invitation.
    }
dav-nothing = No contacts-and-calendars account yet: sioul dav add you@example.org.
dav-no-book = No address book to add it to yet.
dav-no-calendar = No calendar to add it to yet.
contact-no-name = A contact needs a name or an address.
contact-saved = Contact saved.
event-no-title = An event needs a title.
event-saved = Event saved.
undo-contact-deleted = Contact deleted.
undo-event-deleted = Event deleted.
undo-occurrence-skipped = This occurrence is left out.
agenda-gone = This event is no longer there.
dav-synced-at = Contacts and calendars synced at { $time }.
invitation-answering = Sending your answer…
invitation-accepted = Accepted: it is in your calendar, and your answer is sent.
invitation-tentative = Maybe: it is in your calendar, and your answer is sent.
invitation-declined = Declined: your answer is sent.
invitation-add = Added to your calendar.
invitation-remove = Taken out of your calendar.
invitation-subject-accepted = Accepted: { $summary }
invitation-subject-tentative = Maybe: { $summary }
invitation-subject-declined = Declined: { $summary }
invitation-sentence-accepted = { $name } accepts: { $summary }.
invitation-sentence-tentative = { $name } may come: { $summary }.
invitation-sentence-declined = { $name } declines: { $summary }.

## The agenda and the contacts in the window.
ui-agenda = Agenda
ui-contacts = Contacts
ui-new-contact = New contact
ui-new-event = New event
ui-edit-event = Change the event
ui-call = Call
ui-delete = Delete
ui-more-details = More
ui-open-contact = Open the contact
ui-add-contact = Add to contacts
ui-add-dav = Add contacts and calendars
ui-add-dav-note = From a CardDAV and CalDAV server: Nextcloud and Murena, Fastmail, iCloud, your host. The password goes to the system keyring.
ui-dav-server = Server address, when it cannot be found
agenda-next = The next two weeks
agenda-from-day = From { $day }
agenda-week-of = Week of { $day }
agenda-mode-agenda = What comes
agenda-mode-day = Day
agenda-mode-week = Week
agenda-mode-month = Month
agenda-earlier = Earlier
agenda-later = Later
agenda-free = Nothing planned.
agenda-repeats = repeats
agenda-organizer = Organised by { $name }
agenda-delete-this = Only this time
agenda-delete-all = Every time
answer-accepted = accepted
answer-declined = declined
answer-tentative = maybe
answer-needs-action = no answer yet
event-title = Title
event-starts = Starts
event-ends = Ends
event-where = Where
event-repeat = Repeats
event-calendar = Calendar
event-pick-day = Pick the day
date-month-previous = Previous month: { $month }
date-month-next = Next month: { $month }
repeat-none = Does not repeat
repeat-daily = Every day
repeat-weekly = Every week
repeat-monthly = Every month
repeat-yearly = Every year
contact-name = Name
contact-emails = E-mail addresses
contact-phones = Phone numbers
contact-org = Organisation
contact-title = Role
contact-address = Address
contact-birthday = Birthday
contact-notes = Notes
contact-web = Web site
contact-book = Address book
contact-add-email = Another address
contact-add-phone = Another number
contact-add-address = Another postal address
contact-categories = Categories
contact-category-add = category
contact-category-remove = Take off the category { $category }
contact-category-used = Categories in use
contacts-category-all = All categories
contacts-none-in = No contact in { $category }.
set-phone-region = Country for phone numbers written without one
set-phone-region-help = A number written without its country, as 01 99 00 12 34, is read as this country's, to find the same number written +33 1 99 00 12 34. Only to compare: cards keep their numbers as they are written.
set-phone-region-usual = As your system says: { $country }
set-phone-region-none = None: such numbers are compared as they are written
dup-open = Duplicates
dup-title = Duplicates
dup-looking = Looking for duplicates…
dup-region = Country of numbers written without one: { $country }. The settings (⚙) change it.
dup-region-none = Numbers written without their country are compared as they are written; the settings (⚙) give them one.
dup-within-title = On one card
dup-within-none = No card holds a number or an address twice.
dup-within-some = { $n ->
    [one] One card holds a number or an address twice. Ticked, it keeps one of each:
   *[other] { $Count } cards hold a number or an address twice. Those ticked keep one of each:
}
dup-removed = { $value }, the same as { $kept }
dup-clean = Take the duplicates off
dup-cleaned = { $n ->
    [one] Duplicates taken off one card.
   *[other] Duplicates taken off { $count } cards.
}
dup-pairs-title = Two cards, one person?
dup-pairs-none = No two cards look like one person.
dup-pairs-some = { $n ->
    [one] One pair of cards may be one person.
   *[other] { $Count } pairs of cards may be one person, shown one at a time.
}
dup-share = Both have { $what }.
dup-share-name = the same name
dup-share-phone = the number { $value }
dup-share-email = the address { $value }
dup-in-book = In { $book }
dup-keep-name = Name kept
dup-merge-what = Merging keeps the card of the name kept, with its organisation, birthday and photo when it has them, and adds what only the other one has: numbers, addresses, web sites, categories, notes. The other card is then deleted, here and on its server.
dup-merge = Merge
dup-not-same = Not the same
dup-later = Later
dup-merged = Merged into one card: { $names }.
dup-kept-apart = Kept apart, never asked again: { $names }.
dup-done-title = Done lately
dup-done-help = Each is kept thirty days, to undo it.
dup-done-clean = { $n ->
    [one] Duplicates taken off { $names }
   *[other] Duplicates taken off { $count } cards
}
dup-done-merge = Merged: { $names }
dup-undone-aside = Put back as it was. A card changed since is kept aside in { $folder }.
dup-changed = These cards changed meanwhile: look at them again.
country-fr = France
country-re = Réunion
country-yt = Mayotte
country-gp = Guadeloupe
country-mq = Martinique
country-gf = French Guiana
country-be = Belgium
country-ch = Switzerland
country-lu = Luxembourg
country-de = Germany
country-at = Austria
country-nl = Netherlands
country-gb = United Kingdom
country-ie = Ireland
country-es = Spain
country-pt = Portugal
country-it = Italy
country-dk = Denmark
country-no = Norway
country-se = Sweden
country-fi = Finland
country-pl = Poland
country-gr = Greece
country-us = United States
country-ca = Canada
country-au = Australia
country-nz = New Zealand
country-jp = Japan
country-in = India
country-ma = Morocco
country-dz = Algeria
country-tn = Tunisia
country-sn = Senegal
country-ci = Côte d’Ivoire
label-work = work
label-home = home
label-cell = mobile
label-fax = fax
label-pager = pager
label-text = text
label-video = video
label-main = main
invite-cancelled = Cancelled: { $summary }
invite-known = In your calendar.
invite-accept = Accept
invite-maybe = Maybe
invite-decline = Decline
invite-add = Add to my calendar
invite-remove = Take it out of my calendar
# The locale Qt formats dates with.
qt-locale = en_GB
agenda-no-calendar = No calendar yet: add your contacts and calendars in Accounts.
contacts-no-book = No address book yet: add your contacts and calendars in Accounts.

## OpenPGP.
pgp-encrypted = Encrypted
pgp-not-opened = Encrypted, and no key of yours opens it
pgp-signed-by = Signed by { $signer }
pgp-signed-unknown = Signed with a key Sioul does not know
pgp-signature-bad = The signature does not match the text
pgp-no-own-key = No key of yours for { $address } to sign with: make one or import it in Accounts.
pgp-missing-key = No key for { $address } yet: look for it, or send without encrypting.
pgp-making = Making your key…
pgp-made = Your key for { $address } is made; the keyring keeps its passphrase. Its revocation certificate is kept beside it: save a copy somewhere safe, apart from this device, with “Save the revocation certificate”.
pgp-revocation-saved = The revocation certificate is saved: { $path }. Keep it where only you can reach it: anyone who has it can revoke your key.
pgp-wrong-passphrase = This passphrase does not open the key: nothing was imported.
pgp-imported = { $n ->
    [one] One key imported.
   *[other] { $Count } keys imported.
}
pgp-no-key = This key is no longer there.
pgp-removed = Key removed.
pgp-looking = Looking for their keys…
pgp-lookup-all = Every key was found: the message can be encrypted.
pgp-lookup-some = No key found for { $missing }.
pgp-lookup-failed = The key servers could not be reached ({ $detail }).
ui-sign = Sign
ui-encrypt = Encrypt
ui-no-key-for = No key yet for { $addresses }
ui-look-for-keys = Look for their keys
ui-encryption = Encryption (OpenPGP)
ui-encryption-note = Sign and encrypt your messages. Your keys stay on this device, their passphrases in the system keyring; whom you write to gets your key with each message (Autocrypt).
ui-export-key = Save the public key
ui-save-revocation = Save the revocation certificate
ui-make-key = Make a key for { $address }
ui-import-key = Import a key…
ui-key-passphrase = The key’s passphrase, if it has one
ui-import = Import
ui-others-keys = Keys of others
pgp-expires = until { $date }
## Security keys: an OpenPGP card, such as a YubiKey or a Nitrokey.
pgp-for-security-key = Encrypted for your security key
seckey-generic = Security key
seckey-unsupported = This version of Sioul cannot use security keys.
seckey-no-service = Sioul cannot reach security keys: the smart card service is not running. { $hint }
seckey-hint-fedora = On Fedora, this starts it: `sudo systemctl enable --now pcscd.socket`
seckey-hint-debian = On Debian and Ubuntu, this installs it: `sudo apt install pcscd`
seckey-hint-arch = On Arch Linux, these install it, then start it: `sudo pacman -S pcsclite ccid` `sudo systemctl enable --now pcscd.socket`
seckey-hint-suse = On openSUSE, these install it, then start it: `sudo zypper install pcsc-lite pcsc-ccid` `sudo systemctl enable --now pcscd.socket`
seckey-hint-linux = Install pcsc-lite and its CCID driver, then start pcscd.
seckey-hint-flatpak = Start your system’s smart card service (pcscd), then try again.
seckey-hint-windows = Start the Smart Card service in Windows’ Services.
seckey-hint-macos = Plug the key in again.
seckey-absent-sign = To sign, plug in your security key.
seckey-absent-open = To open it, plug in your security key.
seckey-absent = Plug in your security key.
seckey-busy = Another program holds your security key for itself: GnuPG, most likely. For GnuPG and Sioul to share it for good, add this line to scdaemon.conf, in GnuPG’s folder (~/.gnupg): `pcsc-shared`
seckey-wrong-pin = { $n ->
    [one] Wrong PIN. One try left: one more mistake locks the key.
   *[other] Wrong PIN. { $n } tries left.
}
seckey-tries-left = { $n ->
    [one] One try left: one more mistake locks the key.
   *[other] { $n } tries left.
}
seckey-blocked = Your security key is locked after three wrong PINs. Sioul never asks for the Admin PIN: its Admin PIN, or its reset code, unlocks it outside Sioul, in YubiKey Manager or in GnuPG (there, admin, then passwd): `ykman openpgp access unblock-pin` `gpg --card-edit`
seckey-touch-missed = Your security key waited for a touch, and none came in time. Try again, and when Sioul says so, hold your finger on the key for a second or two.
seckey-removed-sign = The key went away before it finished. Nothing was sent; the message is in Drafts.
seckey-removed = The key went away before it finished.
seckey-not-openpgp = The key plugged in holds no OpenPGP keys.
seckey-other-keys = Your security key now holds other keys than those Sioul knows: look for its certificate again in Accounts ▸ Encryption.
seckey-bad-signature = Your security key signed with another key than the one its certificate names, so nothing was sent.
seckey-algorithm = Sioul cannot use this key’s algorithm ({ $algorithm }) with a security key.
seckey-other = The security key could not be used ({ $detail }).
seckey-mismatch-empty = This security key holds no key to sign or to open with.
seckey-mismatch = The key found is not the one on your security key: it was not kept.
seckey-mismatch-revoked = This key was revoked: Sioul will not use it.
seckey-mismatch-use = The key found does not let the keys on your security key do what they are for: it was not kept.
seckey-mismatch-invalid = The key found cannot be read safely ({ $detail }): it was not kept.
seckey-expired = Your key expired on { $date }: mail cannot be signed with it, and others cannot encrypt to it, until it is renewed with GnuPG, from Accounts ▸ Encryption.
seckey-expires-soon = Your key expires on { $date }. GnuPG renews it, from Accounts ▸ Encryption.
seckey-valid-until = Valid until { $date }.
seckey-part-expired-primary = Its primary key ({ $fingerprint }) expired on { $date }: the whole key counts as expired, and nothing can be signed with it.
seckey-part-expired-sign = Its signing subkey ({ $fingerprint }) expired on { $date }: mail cannot be signed with it.
seckey-part-expired-encrypt = Its encryption subkey ({ $fingerprint }) expired on { $date }: nobody can encrypt to you with it.
seckey-part-expired-auth = Its authentication subkey ({ $fingerprint }) expired on { $date }: it cannot sign you in elsewhere (SSH, for instance).
seckey-part-expired-other = Its subkey ({ $fingerprint }) expired on { $date }.
seckey-part-primary = primary key
seckey-part-sign = signing subkey
seckey-part-encrypt = encryption subkey
seckey-part-auth = authentication subkey
seckey-part-other = subkey
seckey-part-until = { $part } until { $date }
seckey-part-forever = { $part } without end
seckey-subkeys-renew = Renew for two years renews them with its primary key. By hand, name each subkey: GnuPG's "*" leaves those already expired as they are.
seckey-renew-incomplete = GnuPG renewed the key, but not all of it. { $parts } It was not kept as renewed: renew it again, or by hand.
seckey-sign-in-window = This message is signed with your security key: send it from its window in Sioul, which asks for the key’s PIN.
seckey-cannot-sign = Your security key cannot sign for this address now.
seckey-released = GnuPG let your security key go. It takes the key back the next time it needs it, and asks for its PIN again then.
seckey-release-no-gnupg = Sioul did not find GnuPG’s gpgconf here. Pull your security key out and plug it in again: GnuPG lets it go then.
seckey-release-sandboxed = In its Flatpak, Sioul cannot reach GnuPG. Pull your security key out and plug it in again, or run this in a terminal: `gpgconf --kill scdaemon`
seckey-release-failed = GnuPG did not let the key go ({ $detail }). Pull your security key out and plug it in again.
seckey-reading = Reading your security key…
seckey-signing = Signing with your security key…
seckey-opening = Opening with your security key…
seckey-touch = Touch your security key now, and hold your finger on it for a second or two: it waits about 15 seconds.
seckey-touch-maybe = If your key asks for a touch, touch it now and hold your finger on it for a second or two: it waits about 15 seconds.
seckey-pin = PIN of your security key
seckey-pin-each = Your security key asks for its PIN for each signature.
seckey-pin-field = PIN
seckey-sign-and-send = Sign and send
seckey-open = Open
seckey-send-unsigned = Send unsigned
seckey-not-now = Not now
seckey-try-again = Try again
seckey-release = Let GnuPG release it
seckey-open-with = Open with your security key
seckey-pin-forgotten = The PIN of your security key is forgotten.
seckey-section = Security key
seckey-intro = Your OpenPGP keys can stay on a security key, such as a YubiKey: it signs and opens your mail itself, after its PIN, and its private keys never leave it. Sioul keeps only their public part.
seckey-use = Use a security key
seckey-made-on = { $algorithm }, made on { $date }
seckey-found-both = { $label }{ $holder }. It signs ({ $sign }) and decrypts ({ $decrypt }). Sioul needs the public part of these keys, which the security key does not hold.
seckey-found-sign = { $label }{ $holder }. It signs ({ $sign }). Sioul needs the public part of this key, which the security key does not hold.
seckey-found-decrypt = { $label }{ $holder }. It decrypts ({ $decrypt }). Sioul needs the public part of this key, which the security key does not hold.
seckey-look-for-it = Look for it
seckey-lookup-tells = Sioul looks at the address written on the key, in your domain’s key directory, then on keys.openpgp.org: this tells these servers that someone looks for your key.
seckey-import-file = Import a file…
seckey-import-gnupg = Import from GnuPG
seckey-gnupg-reading = Asking GnuPG for your key’s certificate…
seckey-gnupg-missing = Sioul did not find GnuPG here. Where GnuPG holds your key, the command under By hand writes its certificate into a file, for Import a file….
seckey-gnupg-sandboxed = In its Flatpak, Sioul cannot reach GnuPG. In a terminal, the command under By hand writes your key’s certificate into a file, for Import a file….
seckey-gnupg-no-key = GnuPG here holds no key { $fingerprint }. Look for it on the servers, or write it into a file where GnuPG has it (By hand), for Import a file….
seckey-gnupg-failed = GnuPG did not do it ({ $detail }).
seckey-source-gnupg = Its certificate came from GnuPG on { $date }.
seckey-renew = Renew for two years
seckey-renewing = GnuPG renews your key for two years, then its subkeys.
seckey-gnupg-pin = Type your security key’s PIN in GnuPG’s own window.
seckey-gnupg-then-touch = Then touch the key, and hold your finger on it for a second or two: it waits about 15 seconds.
seckey-gnupg-touch-missed = Your security key waited for a touch, and none came in time, so GnuPG stopped. Try again, and when Sioul says so, hold your finger on the key for a second or two.
seckey-renewed = Renewed: { $parts }. Others see the new dates once they have your key again: Send it to keys.openpgp.org, or your messages carry it.
seckey-renew-unknown = GnuPG here does not know this security key yet. Plug it in and run this once, then press Renew for two years again; or renew it by hand: `gpg --card-status`
seckey-send = Send it to keys.openpgp.org
seckey-send-tells = This puts your public key on keys.openpgp.org, where anyone can find it by its fingerprint; your private key stays on the security key. keys.openpgp.org then mails each address the key names a link: an address is findable only once its link is opened.
seckey-send-confirm = Send it
seckey-sending = Sending your public key to keys.openpgp.org…
seckey-sent-mailed = keys.openpgp.org has your key. It mailed a link to { $addresses }: open each to make that address findable.
seckey-sent-published = keys.openpgp.org has your key, and finds it by { $addresses }.
seckey-sent-no-address = keys.openpgp.org has your key, findable by its fingerprint only: the certificate Sioul holds names no address. Import it from GnuPG, with its addresses, then send it again.
seckey-by-hand = By hand, in a terminal (then Import from GnuPG, or Import a file…):
seckey-looking = Looking for the public part of your security key…
seckey-not-found = Its public part was found nowhere: not at the address written on the key, not in your domain’s key directory, not on keys.openpgp.org. Import it from a file.
seckey-kept = Found: { $names }. It matches your security key, which now signs for { $addresses } and opens what is encrypted to it.
seckey-kept-not-yours = Found: { $names }. It matches your security key, which opens what is encrypted to it; it names none of your addresses in Sioul, so it signs for none.
seckey-kept-no-address = It matches your security key, but names no address: Sioul signs your mail with it and opens what is encrypted to it, but others will not find this key by your address, and your messages cannot announce it.
seckey-source = Its certificate came from { $source } on { $date }.
seckey-source-file = Its certificate came from a file on { $date }.
seckey-touch-both = It asks for a touch to sign and to open.
seckey-touch-sign = It asks for a touch to sign.
seckey-touch-open = It asks for a touch to open.
seckey-touch-none = It asks for no touch.
seckey-touch-unknown = It may ask for a touch: Sioul could not read whether, and says so when it asks the key.
seckey-signs-for = Signs for { $addresses }.
seckey-signs-for-none = Signs for none of your addresses; opens what is encrypted to it.
seckey-newer = Look for a newer version
seckey-forget-pin = Forget the PIN now
seckey-stop = Stop using this security key
seckey-stopped = Sioul no longer uses this security key; its certificate is kept aside.
seckey-cli-signs = signs: { $fingerprint } · { $algorithm } · made on { $date } · touch: { $touch }
seckey-cli-decrypts = decrypts: { $fingerprint } · { $algorithm } · made on { $date } · touch: { $touch }
seckey-cli-codes = Tries left for the reset code: { $reset }; for the Admin PIN: { $admin }.
seckey-cli-touch = { $policy ->
    [off] none
    [on] each time
    [fixed] each time, fixed
    [cached] once for 15 seconds
    [cached-fixed] once for 15 seconds, fixed
   *[unknown] asked
}
ui-history = How far back mail and the agenda reach
ui-history-note = Older mail is fetched at the next round; "Everything" can take a while and room on disk.
ui-history-set = Mail and the agenda now reach { $weeks } weeks back.
ui-history-everything = Mail and the agenda now reach back to the beginning.
history-1 = 1 week
history-2 = 2 weeks
history-4 = 1 month
history-13 = 3 months
history-26 = 6 months
history-52 = 1 year
history-0 = Everything

## Tasks: the words of the task pages (docs/tasks.md). Never "overdue", "late"
## or "missed"; a date asked is time left; nothing commands.
task-untitled = (no title)
task-estimate-minutes = About { $minutes } minutes
task-estimate-hour = About an hour
task-estimate-hours = About { $hours } h { $minutes }
task-left-today = that is today
task-left-tomorrow = that is tomorrow
task-left-days = { $count } days left
task-left-weeks = { $count } { $n ->
        [one] week
       *[other] weeks
    } left
task-left-months = { $count } { $n ->
        [one] month
       *[other] months
    } left
task-due = By { $date }: { $left }
task-date-asked = Date asked: { $date }
task-waits-for = Waits for: { $titles }
task-waits-until = Waits until { $date }
task-steps-left = { $Count } of { $total } { $n ->
        [one] step left
       *[other] steps left
    }
task-steps-total = Its steps add up to { $estimate }.
task-part-of = Part of: { $title }
task-tight = At this pace, the plan ends after { $date }. Doing it sooner, making it smaller or handing it over would keep the date.
task-done-on = Done { $date }
task-unblocks = Frees { $count } other { $n ->
        [one] step
       *[other] steps
    }
task-stopped = Where you stopped: { $text }
task-spent = { $minutes } minutes so far
task-session = { $date }: { $minutes } minutes
task-no-project = Without a project
task-weather-haze-note = Haze: the plan keeps less for today.
task-weather-fog-note = Fog: only small steps are shown. One step is a full day.
# What a day holds (docs/capacity.md): words only, never a number.
capacity-ratio-longer = Tasks like this usually take about { $factor }× the first guess; the plan already allows for it.
capacity-ratio-shorter = Tasks like this usually take less than the first guess, about { $factor }×; the plan lays them so.
capacity-reason-after-bad = Today holds a little less: yesterday was too much after a full day.
capacity-reason-before-heavy = Today holds a little less: { $title } is tomorrow.
capacity-reason-after-heavy = Today holds a little less, after { $title } yesterday.
capacity-reason-more = Days hold a little more than two weeks ago: the days went well.
capacity-reason-less = Days hold less than two weeks ago.
capacity-said-tomorrow-less = Tomorrow holds a little less: today was too much after a full day.
capacity-said-held = What a day holds stays as it is for a while: several days were too much.
capacity-scale-cognitive = thinking
capacity-scale-emotional = emotions
capacity-scale-anxiety = anxiety
capacity-scale-body = body and senses
capacity-scale-total = the day
capacity-gain-slot = Time for you
capacity-gain-suggestion = Perhaps: { $title }
capacity-slack = Kept free, in case steps take longer
task-why-started = You started it.
task-why-due = Its date is { $date }: { $left }.
task-why-chain = { $title }, which waits for it, has a date: { $date }.
task-why-unblocks = It frees { $count } other { $n ->
        [one] step
       *[other] steps
    }.
task-why-free = Nothing comes before it.
task-picked = Picked for you among equal steps.
task-why-chain-part = { $title }, which waits for it, is part of “{ $whole }”, whose date is { $date }.
task-only-tied = Nothing outside what you put off is free now: this one is tied to it.
task-office-hours = Needs an open office
task-office-hours-help = Proposed only when offices are open (Tasks settings), and planned only on those days.
task-office-closed = Steps that need an open office come back { $when }.
task-all-aside = The steps set aside come back tomorrow.
task-nothing-until = Nothing is needed today. The next step comes { $date }: { $title }.
task-nothing-ready = Nothing is needed today.
task-wip = { $Count } things are started. Finish or park one?
task-loop = These wait for each other: { $titles }. One of them has to go first.
task-done-frees = This frees: { $titles }.
task-done-all-steps = Every step of { $title } is done.
board-ready = Free to start
board-doing = Started
board-waiting = Waiting
board-done = Done
timeline-empty = Nothing open to place.
timeline-reaches = The plan reaches { $date }.

## How a thing is tied to the one open.
how-note = Its notes
how-notes-of = Notes about
how-source = Made from
how-made = Made into
how-waits-for = Waits for
how-unblocks = Frees
how-part-of = Part of
how-step = A step
how-contact = Who
how-involves = Involves them
how-project = Project
how-in-project = In the project
how-mentions = Mentions
how-mentioned-by = Mentioned in
how-answers = Answers
how-answered-by = Answered by
how-link = Linked
task-now-title = The next step
task-then = After that
task-not-found = No task matches “{ $what }”.
task-ambiguous = Several tasks match “{ $what }”: { $titles }. Give its UID.
task-no-such-list = No task list “{ $list }” can take tasks (see sioul tasks lists).
task-no-list = No task list yet: make one with sioul tasks new-list <account> <name>.
task-no-title = A task needs a title.
task-read-only = This task is in a list you can only read.
task-done-said = Done.
import-unknown-key = “{ $key }”, named by { $task }, is not in the file.
focus-already = A session is already running, on { $title }.
focus-started = Focus on { $title } for { $minutes } minutes.
focus-status = { $task }: { $elapsed } minutes so far, { $left } left.
focus-none = No focus session is running.
focus-stopped = { $n ->
        [one] One minute, kept.
       *[other] { $n } minutes, kept.
    }
note-not-found = No note at { $path }.
task-no-list-yet = No task list yet. One can be made on the Tasks page.
task-gone = This task is no longer there.
task-set-aside = Set aside until tomorrow.
undo-task-deleted = Task deleted.
note-untitled = Untitled note
note-from-mail = From { $sender }:
note-guests = Guests:
note-gone = This note is no longer there.
task-prepare = Prepare: { $title }

## The task, notes and focus pages.
ui-tasks = Tasks
ui-notes = Notes
ui-make-task = Make a task
ui-make-note = Make a note
ui-make-event = Make an event
related-title = Tied to it
task-mode-now = Now
task-mode-list = List
task-mode-board = Board
task-mode-timeline = Calendar
task-capture-hint = A task in one line: “Call the tax office tomorrow ~15m #project {"{"}date asked{"}"}”
chip-start = from
chip-due = date asked
chip-estimate = minutes
chip-project = project
chip-tag = tag
task-first-list = Tasks live in a task list, on your calendar server or only here. One is made in a click:
task-default-list = Tasks
task-make-list = Make the list
task-all-projects = Every project
task-weather = How is today?
task-weather-clear = Clear
task-weather-haze = Haze
task-weather-fog = Fog
task-start = Start
task-done = Done
task-not-now = Not now
task-open-again = Open again
# Not to be done after all (STATUS:CANCELLED): kept, struck out, out of the plan.
task-drop = Drop
# A task's details: an office's hours not given, the usual ones.
task-office-usual-short = offices' usual hours
task-drop-help = Not to be done after all: kept, struck out, out of the plan. “Open again” brings it back.
task-do-at = Do at…
task-do-at-help = A block in your calendar: the plan lays the task there, and every calendar shows it.
task-do-at-day = Day
task-do-at-time = Time
task-do-at-length = For
task-do-at-save = Pin it
task-pinned-move = Move…
task-pinned-open = Open in the agenda
task-pinned-menu = What to do with this time
task-pinned = Pinned: { $when }
task-pinned-today = today, { $from }–{ $to }
task-pinned-day = { $day }, { $from }–{ $to }
task-pinned-read-only = Its calendar can only be read here: move it in the application it comes from.
blocks-calendar = Planned tasks
# The time things like it take, against the first guess, shown when asked (ratio_line).
task-ratio-ask = How long things like this take
task-hard = What makes it hard?
task-hard-how = I do not know how to start
task-hard-big = It is too big
task-hard-dread = I dread it
task-hard-boring = It is boring
task-hard-energy = No energy today
task-other-choices = Other choices
task-started = Started
task-done-week = Done this week
task-search = Search the tasks
task-by-project = By project
task-by-list = By list
task-show-done = Done too
task-title = Title
task-steps = Steps
task-add-step = A step, in one line
task-waits-title = Waits for
task-waits-add = Waits for… (a task’s title)
task-waits-remove = No longer waits for it
task-waits-gap = Then wait
task-waits-gap-help = For the next wait you add: this task waits that long after the other is done (“the answer comes within two weeks”). 0: as soon as it is done.
task-waits-gap-days = { $n ->
    [one] day
   *[other] days
}
task-waits-gap-weeks = { $n ->
    [one] week
   *[other] weeks
}
task-waits-after-days = { $n ->
    [one] a day after it
   *[other] { $count } days after it
}
task-waits-after-weeks = { $n ->
    [one] a week after it
   *[other] { $count } weeks after it
}
task-waits-after-hours = { $n ->
    [one] an hour after it
   *[other] { $count } hours after it
}
task-frees-one = Frees: { $title }
task-field-start = Can start from
task-field-due = Date asked
task-field-estimate = Takes about
task-field-project = Project
task-field-tags = Tags
task-field-repeat = Comes back
task-field-list = List
task-field-notes = Notes, in Markdown
task-write = Write an email
task-make-note = Make a note
focus-title = Focus
focus-two = Just two minutes
focus-for = { $minutes } minutes
focus-open-ended = Without an end
focus-left = { $minutes } min left
focus-so-far = { $minutes } min so far
focus-two-left = Two minutes left: time to find a stopping point.
focus-over = The time chosen is over.
focus-paused = Paused.
focus-pause = Pause
focus-resume = Go on
focus-plus-five = Five more minutes
focus-keep-going = Keep going
focus-stop-counts = Stop here, it counts
focus-stop = Stop
focus-breadcrumb = Next time, start by…
focus-done = Done
focus-keep = Keep
# The time running, in the system's notifications: since when, with the time chosen or without an end; paused, the time so far.
focus-notification-planned = Since { $time }, { $minutes } min chosen
focus-notification-open = Since { $time }, without an end
focus-notification-paused = Paused, { $minutes } min so far
# Android's channel for it, as Android's settings list it.
focus-notification-channel = Focus timer
note-search = Search the notes
note-new = New note
note-title = Its title
note-no-store = No notes folder yet: it is chosen in Settings ▸ Your folder and sharing.
note-none-open = A note opens here.
note-recent = Changed lately
note-read = Read
note-save = Save
note-send-guests = Send to the guests
note-open-lines = Lines not ticked
note-make-task = Make it a task
task-group-joy = If you want
task-group-someday = When you say so
task-joy = If you want

## Settings, each where it applies (one sentence each).
ui-settings = Settings for this page
set-like-others = Like the other addresses
set-windows = Working hours
set-windows-help = On these days and hours, work can reach you. Outside them, work rests: your admin comes in its own hours, below; every other time is leisure. The codes and links you ask sites for, and what you send yourself, always come at once.
set-sorting = How mail is sorted
set-accounts-elsewhere = What each address is for, how far back it goes, how often it is fetched and its shield are on its card in Accounts: the person icon at the bottom of the left column.
set-task-areas-group = What a task is for
set-files-group = Your folder
set-invoice-group = Invoices
set-time-off = Time off
set-time-off-help = Holidays, sick leave: quiet from the first day to the last, as on a day off.
set-time-off-label = Holidays, a word on them
set-time-off-add = Add
set-quiet-personal = What is yours
set-quiet-personal-help = Tasks of these categories, and projects marked as yours, are yours outside work: those of leisure (joy, family, friends, leisure) come in leisure and during meals, the others (health, personal) in any time of yours. A task with none of these and none of work's is your admin.
set-office-hours = When offices are open
set-office-hours-help = A task marked “Needs an open office” is proposed only then, and planned only on these days.
set-windows-add = Add a window
set-windows-none = No window: the Porch is always open.
set-known = Senders you know
set-known-help = Their mail goes straight to “From people you know”, as your contacts' does. A stranger waits in the screener until you let them in. An address, or @domain for everyone there.
set-blocked = Blocked senders
set-blocked-help = Their mail is set aside for good: never shown, never counted. An address, or @domain for everyone there.
set-senders-add = Add an address or @domain
set-senders-add-number = Add an address, a number or @domain
set-fetch-minutes = Fetch every
set-fetch-minutes-help = The inbox comes as soon as the server says something arrived; the other folders are fetched this often.
set-account-name = Your name
set-account-name-help = As recipients see it in the From line.
set-account-signature = Signature
set-account-signature-help = Added below what you write, in Markdown.
set-account-priority = Importance
set-account-priority-help = Less important: its mail skips the screener and waits folded at the bottom of the Porch. More important: it comes first.
set-account-history = How far back
set-account-history-help = How many weeks of this address's mail the folders show.
set-account-fetch = Fetch every
set-account-fetch-help = 0 follows the other addresses. A public address can be fetched less often: twice a day is 720.
set-account-shield = Protected against harassment
set-account-shield-help = For a public address that may bring insults: its mail is read first; hostile messages are set aside without showing their text, the rest is summarised by topic.
set-account-shield-ai = Let the AI read it first
set-account-shield-ai-help = An AI (Claude) reads new mail of this address to say its tone and topic, more finely than word lists. The text leaves this device for that.
set-day-start = The day starts at
set-day-start-help = The hour the day and week planning open on.
set-task-estimate = A task without an estimate counts
set-task-estimate-help = Minutes the plan counts for a task whose length is not said.
set-planning-group = What a day holds
set-planning-start = Start from
set-planning-start-help = Until your days say more, how full the plan makes a day. As now: your hours full, two heavy steps at most. Lighter, or much lighter: to start again after a hard time, or with an illness that limits energy.
set-planning-start-as-now = As now
set-planning-start-lighter = Lighter
set-planning-start-much-lighter = Much lighter
set-planning-window = Days learned from
set-planning-window-help = How many past days the plan looks at to learn what a day holds for you. Only the days you said were too much, about right or too empty count.
set-planning-window-unit = days
set-planning-even = Even days
set-planning-even-help = The week's load spread so that each day holds about the same, rather than full days and empty ones. It helps with an illness that limits energy.
set-planning-gain-slots = Time for you
set-planning-gain-slots-help = Two slots a day, one after the day's heaviest step and one in the evening, kept free and quiet: fill them as you like, or leave them empty.
set-task-list = New tasks go into
set-task-list-help = The list a task typed in one line goes into.
set-task-list-first = The first list made for tasks
set-task-blocks = Time blocks go into
set-task-blocks-help = Where a task pinned to a time is written: an event, which every calendar shows, your phone's too. “Planned tasks” is made the first time it is needed, on the account of the task's list, or on this device.
set-task-blocks-own = “Planned tasks”, with each task's list
set-task-block-alarms = An alarm in each block
set-task-block-alarms-help = Five minutes before it, for the calendars of your other devices; in the blocks made or moved from now on. Without it, Sioul reminds you of a block as of any event, and a phone does not remind you twice.
set-notes-root = The notes folder
set-notes-root-help = Your folder of Markdown files, read as a vault: your notes, and beside them your projects (sioul-projects.toml) and budgets (sioul-budgets.toml). Sioul links to it and never owns it.
set-projects-rename = Rename to sioul-projects.toml
set-projects-rename-help = Your projects are in sioul-cases.toml, the file's first name, which Sioul still reads and writes as it is. Renamed, it becomes sioul-projects.toml and its entries [[project]], every field and comment kept, with a copy of the old file beside it (sioul-cases.toml.before-rename). Rename it once each of your devices has this version of Sioul or a later one: an older version reads only sioul-cases.toml.
set-projects-both = Your notes folder holds both sioul-projects.toml and sioul-cases.toml: Sioul reads sioul-projects.toml and leaves sioul-cases.toml as it is. An older version of Sioul, on another device, may still write there.
set-notes-folder = New notes go into
set-notes-folder-help = A folder inside the notes folder, where Sioul puts every new note (unless you make it in another folder), and your audio memos in its memos folder.
set-reading-family = Font
set-reading-family-help = For long text: notes, mail, a task's notes. Empty: the desktop's.
set-reading-size = Size
set-reading-size-help = The size of long text.
set-reading-spacing = Line spacing
set-reading-spacing-help = Air between lines: 1.5 reads more easily than tight lines.
set-map-geocode = Place contacts on the map
set-map-geocode-help = Their postal addresses are sent to OpenStreetMap's geocoder (Nominatim), once each, one a second; the places found are kept on this device.
map-show = Map
map-list = List
map-allow = To place your contacts, their postal addresses are sent to OpenStreetMap's geocoder, once each. Nothing else goes with them.
map-allow-button = Place them
map-waiting = { $n ->
    [one] One address not placed yet.
   *[other] { $n } addresses not placed yet.
}
map-locating = Placing addresses, one a second…
map-place = Place them now
map-none = No contact has a postal address placed yet.
set-map-tiles = Map tiles
set-map-tiles-help = Where map images come from, as https://…/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png. Empty: OpenStreetMap's, fetched sparingly and kept.
set-invoice-name = Your name or business name
set-invoice-name-help = Printed at the top of your invoices. In France, a sole trader (entrepreneur individuel) writes « EI » right after the name: « Camille Exemple EI ».
set-invoice-address = Your address
set-invoice-address-help = On lines, as on an envelope.
set-invoice-siret = SIRET
set-invoice-siret-help = Or the business number where you are; empty until you are registered.
set-invoice-vat = VAT line
set-invoice-vat-help = For a French micro-entrepreneur: « TVA non applicable, art. 293 B du CGI ».
set-invoice-prefix = Invoice numbers start with
set-invoice-prefix-help = Empty: the year, and numbers start again each January (2026-001, 2026-002…). With a prefix of your own, they go on from the last one.
set-invoice-currency = Currency
set-invoice-currency-help = A three-letter code: EUR, USD, CHF.
set-invoice-payment = Payment details
set-invoice-payment-help = Printed at the bottom: IBAN, terms, late fees.
set-invoice-folder = Invoices go into
set-invoice-folder-help = The folder of their PDFs; "Documents/Invoices" in your home when empty.
set-invoice-rate = An hour costs
set-invoice-rate-help = The fee when a project does not set its own.
## Settings ▸ AI agents: what an agent connected with `sioul mcp` may use (docs/ai.md, docs/mcp.md)
set-ai-agents-group = What AI agents may use
set-ai-agents-note = An AI agent you connect with sioul mcp (Claude Code, Claude Desktop, any MCP client) reads what you open to it here, and adds tasks, events, notes, ties and drafts there. What it reads goes to the provider of its model. It never sends, deletes or pays anything, and never sees a password, a code or a whole account number.
set-mcp-outside-projects = Things in no project
set-mcp-outside-projects-help = The Porch’s other mail, tasks and notes of no project, the agenda, contacts, the phone’s messages and calls, papers, contracts, and the budgets no project uses. Open, so that an agent you connect is of use at once; close it if you want agents to see only the projects you open below.
set-mcp-texts = Texts
set-mcp-texts-help = Off unless you turn it on, as texts carry other people’s words. On, an agent reads and searches the texts this computer keeps from your phone, codes masked, and writes drafts that wait in the Texts page until you send them yourself; never a group, never a text sent. To open your texts, Sioul reads the sharing key from this computer’s keyring, and only then; the key never reaches the agent.
set-mcp-spam = The spam filter’s tools
set-mcp-spam-help = An agent may run, test and judge your own spam filter: it sees the sender and the subject of the mail it lists, never a text, and a label it gives changes what the filter flags on every device. On computers.
set-ai-projects-group = Projects open to AI agents
set-ai-projects-note = Each project is closed to agents until you open it, here or on its page. Open, an agent reads its mail, tasks, notes, time, invoices and letters, and adds to it.
set-ai-projects-none = No project yet. Each project you make starts closed to agents.
set-theme = Colours
set-theme-help = Light or dark, or as the system has them. The icons follow at the next start.
set-theme-system = The system's
set-theme-light = Light
set-theme-dark = Dark
set-ai-group = The AI shield
set-ai-key = Key for Anthropic's API
set-ai-key-help = Kept in the system keyring, never in a file. Each new message to a shielded address that allows the AI is sent once to Anthropic (model Claude Haiku), with its subject; the answer (tone, topic, one neutral line) stays on this device. A key is made at console.anthropic.com.
set-ai-key-kept = A key is kept.
set-ai-key-none = No key yet: the word lists judge alone.
set-ai-key-keep = Keep
set-ai-key-forget = Forget the key
set-mail-threads = By conversation
set-mail-threads-help = A message and its answers together, under the newest; yours come from Sent. Opened with the small arrow on the left.
set-spam-group = Your own spam filter
set-spam-actions = What it does with each verdict
set-spam-actions-help = Trained on your mail, it judges a stranger's message only, never people you know, codes, projects or your own mail. “Move to spam” moves the message on the server, into the account's Junk folder, as it arrives; “Flag only” leaves it where it is. Either way it waits on the Porch, in “Caught by your own spam filter”, and is never notified. Nothing is deleted.
set-spam-class-spam = Probably spam
set-spam-class-unsure = Maybe spam
set-spam-class-ham = Probably not spam
set-spam-action-move = Move to spam
set-spam-action-flag = Flag only
set-spam-action-nothing = Do nothing
set-spam-threshold = Spam from
set-spam-threshold-help = How sure it must be to call a stranger's message probably spam. Higher: fewer of your messages taken for spam, more spam let through.
set-spam-unsure = Maybe spam from
set-spam-unsure-help = From here up to the spam threshold, a stranger's message is maybe spam; below, probably not spam. Always below the spam threshold.
set-spam-filter = Its training
set-spam-filter-help = It learns on one computer: train it on this one alone, when you ask, and it trains again by itself once a week when this computer is plugged in and idle, from all your mail: every folder of every address, your Junk folders, what it caught itself, as it is (a maybe spam only once you say), and what you said is spam or not on any of your devices, which always wins. The mail it reads and the words it learns stay on this computer; only its table goes to your other devices, sealed, with no word of your mail in it.
set-spam-filter-phone = Its table
set-spam-filter-phone-help = This phone never trains it: one computer does, on your mail, and its table comes here sealed through your folder (the part “Spam filter”), with no word of your mail in it. What you say here, Spam or Not spam, and what its filter catches here go there the same way.

## Mail filters (crates/sioul-core/src/rules.rs; docs/client.md, "Filters").
filter-field-from = From
filter-field-to = To
filter-field-cc = Cc
filter-field-reply-to = Reply-To
filter-field-subject = Subject
filter-field-body = Text
filter-field-attachment = Attachment
filter-field-attachment-type = Kind of attachment
filter-field-sender = Sender
filter-field-list = Newsletter or list
filter-field-date = Day it arrived
filter-field-weekday = Day of the week
filter-field-hour = Time it arrived
filter-field-size = Size
filter-field-anywhere = Anywhere
filter-field-account = Address
filter-field-folder = Folder
filter-field-mark = Message
filter-mark-read = read
filter-mark-flagged = flagged
filter-mark-answered = answered
filter-test-contains = contains
filter-test-not-contains = does not contain
filter-test-is = is
filter-test-is-not = is not
filter-test-before = before
filter-test-after = after
filter-test-between = between
filter-test-above = larger than
filter-test-below = smaller than
filter-test-exists-attachment = there is one
filter-test-missing-attachment = there is none
filter-test-exists-list = it is one
filter-test-missing-list = it is none
filter-if-text = { $field ->
        [from] From
        [to] To
        [cc] Cc
        [reply-to] Reply-To
        [subject] Subject
        [list] List
        [anywhere] Anywhere
        [account] Address
        [folder] Folder
        [mark] Message
       *[body] Text
    } { $test ->
        [not-contains] does not contain
        [is] is
        [is-not] is not
       *[contains] contains
    } “{ $value }”
filter-if-attachment = { $test ->
        [missing] without attachments
       *[exists] with an attachment
    }
filter-if-list = { $test ->
        [missing] not sent by a newsletter nor a list
       *[exists] sent by a newsletter or a list
    }
filter-if-kind = { $test ->
        [is-not] no attachment is { $kind }
       *[is] an attachment is { $kind }
    }
filter-if-sender = { $test ->
        [is-not] the sender is not { $who }
       *[is] the sender is { $who }
    }
filter-if-date = { $test ->
        [before] arrived before { $value }
        [between] arrived between { $value } and { $until }
       *[after] arrived after { $value }
    }
filter-if-weekday = { $test ->
        [is-not] did not arrive on a { $days }
       *[is] arrived on a { $days }
    }
filter-if-hour = { $test ->
        [before] arrived before { $value }
        [between] arrived between { $value } and { $until }
       *[after] arrived from { $value } on
    }
filter-if-size = { $test ->
        [below] smaller than { $value }
       *[above] larger than { $value }
    }
filter-if-none = (no condition yet)
filter-if-unfinished = (a condition to finish)
filter-kind-pdf = a PDF
filter-kind-image = a picture
filter-kind-document = a document (text, sheet, slides)
filter-kind-archive = an archive (zip…)
filter-kind-calendar = an invitation
filter-kind-audio = a sound
filter-kind-video = a video
filter-kind-text = a text file
filter-who-known = someone you know
filter-who-safe = safe
filter-who-neutral = neutral
filter-who-restricted = restricted
filter-who-stranger = a stranger
filter-join-all = {" "}and{" "}
filter-join-any = {" "}or{" "}
filter-act-move = Move it to a folder
filter-act-archive = Archive it
filter-act-junk = Mark it as spam
filter-act-flag = Flag it
filter-act-read = Mark it as read
filter-act-trash = Move it to the trash
filter-act-keyword = Add a keyword
filter-then-move = into “{ $name }”
filter-then-archive = archived
filter-then-junk = into the junk, as spam
filter-then-flag = flagged
filter-then-read = marked read
filter-then-trash = into the trash
filter-then-keyword = keyword “{ $name }”
filter-then-unknown = (an action this Sioul does not know)
filter-then-unfinished = (an action to finish)
filter-then-stop = and no other filter
filter-then-none = (nothing to do yet)
filter-said = { $conditions } → { $actions }
filter-said-empty = A new filter: say what a message shows, then what is done with it.
filter-problem-unknown = This filter holds something this Sioul does not know: change it, or bring Sioul up to date.
filter-problem-value = A condition waits for its value.
filter-problem-date = A day reads as 2026-10-01, and a range goes from its start to its end.
filter-problem-time = A time reads as 18:00.
filter-problem-size = A size reads as 5 MB or 500 KB.
filter-problem-no-condition = Add a condition: without one, this filter does nothing.
filter-problem-no-action = Choose what it does: without an action, it does nothing.
filter-problem-folder = Choose the folder it moves messages into.
filter-problem-keyword = A keyword is one word: no spaces, brackets, braces, quotes, % or *, and none of $Junk, $NotJunk or $SioulFiltered.
set-filters-group = Filters
set-filters = Your filters
set-filters-help = Each filter says what a message shows, then what is done with it on its server. They act on mail that arrives unread in an inbox, on the first of your devices to fetch it; your other devices then leave it alone. Mail the Porch sets aside, a code you asked for and what your own spam filter caught are never touched, and nothing is deleted for good.
filter-only = Only for { $accounts }
filter-tried = { $n ->
        [0] In your inboxes now, it takes none of the { $total } messages.
        [one] In your inboxes now, it takes one of the { $total } messages.
       *[other] In your inboxes now, it takes { $count } of the { $total } messages.
    }
filter-tried-newest = { $n ->
        [0] Among the { $total } newest messages of your inboxes, it takes none.
        [one] Among the { $total } newest messages of your inboxes, it takes one.
       *[other] Among the { $total } newest messages of your inboxes, it takes { $count }.
    }
filter-tried-one = { $who }: { $subject }
filter-preview-line = { $Count } { $n ->
        [one] message
       *[other] messages
    }: { $filter }
filter-preview = { $n ->
        [one] In your inboxes now, one message would change, read or not.
       *[other] In your inboxes now, { $count } messages would change, read or not.
    } Codes you asked for, mail set aside and what your own spam filter caught stay as they are.
filter-preview-none = No message in your inboxes now matches a filter: nothing would change.
filter-running = { $n ->
        [one] Filtering one message in ten seconds.
       *[other] Filtering { $count } messages in ten seconds.
    }
filter-ran = { $n ->
        [0] No message filtered.
        [one] One message filtered.
       *[other] { $Count } messages filtered.
    }
filter-failed-folder = { $n ->
        [one] One message of { $account } could not be moved: there is no folder “{ $folder }” there.
       *[other] { $Count } messages of { $account } could not be moved: there is no folder “{ $folder }” there.
    } Make it in Mail, or change the filter; Sioul tries again with the next mail.
filter-failed-server = { $n ->
        [one] One message of { $account } could not be filtered: its server did not take it.
       *[other] { $Count } messages of { $account } could not be filtered: its server did not take them.
    } Sioul tries again with the next mail.
filter-failed-keywords = The server of { $account } keeps no keywords: a filter that only adds one does nothing there.
filter-failed-renumbered = The server of { $account } renumbered its inbox: { $n ->
        [one] one message was not filtered, and it is told
       *[other] { $count } messages were not filtered, and they are told
    } as new mail.
filter-cli-acted = { $account }: { $n ->
        [one] one message filtered.
       *[other] { $count } messages filtered.
    }
filter-ui-narrow = Show the filters of
filter-ui-all-addresses = Every address
filter-ui-none = No filter yet.
filter-ui-none-here = No filter for this address.
filter-ui-add = Add a filter
filter-ui-switch = On or off: off, it is kept and does nothing.
filter-ui-edit = Change it
filter-ui-done = Done
filter-ui-up = Ask it earlier
filter-ui-down = Ask it later
filter-ui-name = Its name, if you want one
filter-ui-if = If
filter-ui-all = every condition holds
filter-ui-any = one of them holds
filter-ui-field = What it reads
filter-ui-test = How it compares
filter-ui-value = Words to look for
filter-ui-size = 5 MB
filter-ui-and = and
filter-ui-remove = Take it away
filter-ui-add-condition = Add a condition
filter-ui-then = Then
filter-ui-add-action = Add an action
filter-ui-folder = Choose a folder
filter-ui-folder-some = This folder is only on { $accounts }: from the other addresses, the message cannot go there, and Sioul says so.
filter-ui-keyword = A keyword
filter-ui-more = On which addresses; the filters below
filter-ui-addresses = On which addresses
filter-ui-every-address = Every address
filter-ui-stop = Once it acts, the filters below are not asked
filter-ui-try = Try it on the inboxes
filter-ui-looking = Looking through your inboxes…
filter-ui-delete = Delete this filter
filter-ui-deleted = Deleted: { $name }
filter-ui-run-help = Filters act on mail as it arrives unread. To act on everything in your inboxes now, read mail too, run them: Sioul first says what would change.
filter-ui-run = Run them on the inboxes…
filter-ui-run-now = Run them now
filter-ui-not-now = Not now
filter-from-search-left = A filter reads the mail that arrives in your inboxes: what the search said of folders, and of read, flagged or answered mail, is left out.
ui-conversation = { $n } messages: show or fold the conversation
link-made = Tied to “{ $title }”.
link-undone = The tie is undone.
link-is-plan = This tie is a step or a wait: change it in the task.
link-in-text = This link is written in the note's text: edit the note to take it out.
link-is-guest = This person is a guest of the event: change it in the event.
link-not-found = This tie is no longer there.
link-not-made = It could not be made.
ui-add-new = Add
ui-link-existing = Link to…
add-task = A task
add-event = An event
add-note = A note
add-mail = A message
add-reply = A reply
add-contact = The sender, to your contacts
link-title = Link “{ $title }” to…
link-search = A few words of its title
link-kind-all = All
link-kind-task = Tasks
link-kind-event = Events
link-kind-mail = Mail
link-kind-note = Notes
link-kind-contact = Contacts
link-kind-budget = Budget lines
link-kind-project = Projects
link-kind-draft = Drafts
link-kind-web = Web
link-kind-file = Files
link-kind-other = Other
link-already = Already linked
link-nothing = Nothing matches.
link-undo = Undo this link
task-nothing-filtered = Nothing of this kind is free now.
task-quiet-empty = Nothing waits for you now.
task-kind = Kind
task-kind-help = What doing it takes: tasks of one kind are easier done together. Typed in one line with "@call", "@write", "@online", "@out", "@read", "@think", "@make".
task-kind-none = No kind
task-kind-call = A call
task-kind-write = Writing
task-kind-online = Online, a form
task-kind-out = Going out
task-kind-read = Reading
task-kind-think = Thinking it over
task-kind-make = Making
task-filter-all = All kinds
task-filter-any-category = Any category
chip-kind = Kind
invoice-to = Invoice to
invoice-title = Invoice No. { $number }
invoice-dated = Issued on { $date }
invoice-period-day = Service on { $date }
invoice-period = Service from { $from } to { $to }
invoice-due = Payable by { $date }
invoice-project = Project: { $project }
invoice-what = Work
invoice-time = Time
invoice-rate = Hourly rate
invoice-amount = Amount
invoice-total = Total
invoice-siret = SIRET { $siret }
invoice-late = Late payment: penalties at three times the French legal interest rate, and a fixed €40 recovery fee (Code de commerce, art. L441-10 and D441-5). No discount for early payment.
time-no-project = No project
time-gone-project = A project no longer there
time-week-of = Week of { $day }
time-nothing-week = Nothing noted this week.
time-nothing-month = Nothing noted this month.
time-nothing-year = Nothing noted this year.
project-done = Done
project-asked = Date asked
project-from = From { $sender }
project-from-unproven = From { $sender }, whose address could not be verified
project-note-changed = Note changed
project-time-noted = { $time } noted
project-invoice = Invoice { $number }
project-no-title = A project needs a name.
project-gone = This project is no longer there.
project-ai = Open to AI agents
project-ai-help = An agent you connect with sioul mcp then reads this project’s mail, tasks, notes, time, invoices and letters, and adds tasks, notes, ties and drafts to it; what it reads goes to the provider of its model. Closed, it sees nothing of it.
invoice-nothing = No billable time is waiting to be billed.
invoice-gone = This invoice is no longer there.
invoice-folder-name = Invoices
invoice-budget-line = Invoice { $number }, { $client }
invoice-budget-origin = Invoice { $number }, made by Sioul: expected until paid.
time-no-minutes = How long? Some minutes, at least.
time-no-project-chosen = For which project, or which task?
time-bad-day = Which day? It reads as 2026-10-05.
time-billed-stays = This time is on an invoice: it stays.
# How a stretch's minutes were known, said quietly on the Time page.
time-kind-measured = timed
time-kind-typed = noted by hand
time-kind-corrected = timed, then corrected
time-kind-unknown = not known how
time-bad-hours = The hours read as 09:00 and 10:30.
time-gone = That stretch is no longer there: it may have been changed elsewhere.
time-field-from = From
time-field-to = To
time-field-task = Task
time-no-task = No task
time-own-project = The task's own project
time-lasts = { $time }
ui-projects = Projects
ui-time = Time
project-new = New project
project-edit = Project
project-none = No project yet. A project is any matter you follow: work for a client, its time billable, or one of yours (housing, health, a trip).
project-choose = A project, on the left.
project-field-title = Name
project-field-billable = For a client: its time is billed at an hourly rate
project-field-billable-help = Time noted on it can be billed; without it, the project is yours: housing, health, a trip.
project-field-client = For
project-field-client-hint = Who it is for
project-field-rate = An hour costs
project-field-rate-hint = The invoice settings' rate when empty
project-field-budget = Invoices expected in
project-no-budget = No budget
project-field-status = Where it stands
project-status-open = Open
project-status-waiting = Waiting
project-status-closed = Closed
project-kind-project = For a client
project-kind-own = Yours
project-rate = { $rate } an hour
project-board = Board
project-list = List
project-calendar = Calendar
project-open-tasks = { $n ->
    [one] one task open
   *[other] { $n } tasks open
}
project-tasks-count = { $open ->
    [0] No task open
    [one] One task open
   *[other] { $open } tasks open
}
project-tasks-done = { $done } done
project-time-all = { $time } noted
project-to-bill = { $time } to bill
project-to-bill-amount = { $time } to bill
project-invoices = Invoices
project-timeline = On one line of time
project-coming = Coming
project-before = Before
invoice-make = Make the invoice
invoice-paid = Paid
invoice-print = Print again
invoice-written = The invoice is in { $path }.
invoice-not-written = The invoice could not be written into { $path }.
time-note = Note time
time-field-project = For
time-field-length = How long
time-field-length-hint = 1h30, 45m, 90
time-field-day = When
time-field-note = What it was
time-field-note-hint = A meeting, a call…
time-field-unbilled = Not to bill
time-period-week = Week
time-period-month = Month
time-period-year = Year
time-all-projects = All projects
time-total = { $time } in all
time-on-invoice = On invoice { $number }
time-remove = Take this time out
badge-pace-better = Ahead of its pace by { $gap }
badge-pace-on-track = On track
badge-pace-short = { $gap } behind its pace
pace-needs-month = { $gone }% of the month gone; { $part }% of what it still needs has come in.
pace-needs-year = { $gone }% of the year gone; { $part }% of what it still needs has come in.
pace-may-spend-month = { $gone }% of the month gone; { $part }% of what it can spend is spent.
pace-may-spend-year = { $gone }% of the year gone; { $part }% of what it can spend is spent.
pace-nothing-needed-month = { $gone }% of the month gone; nothing more is needed.
pace-nothing-needed-year = { $gone }% of the year gone; nothing more is needed.
fig-range = From a bad month to a good one
fig-range-value = { $low } to { $high }
budget-in = In
budget-out = Out
budget-step-day = Days
budget-step-week = Weeks
budget-step-month = Months
budget-step-year = Years
budget-movements = Movements
budget-planned = planned
budget-recurring = each period
budget-add = Add a movement
budget-add-once = Once
budget-add-recurring = Recurring
budget-add-label = What
budget-add-amount = Amount
budget-add-amount-hint = -650 out, 1200 in
budget-add-date = When
budget-add-every = Every
budget-add-every-month = month, on the day
budget-add-every-year = year, on the day
budget-add-estimate = An estimate (food, cash), not a fixed amount
budget-add-from = From
budget-add-until = Until
budget-added = Added to { $budget }.
budget-add-bad-amount = An amount, positive for money in, negative for money out.
budget-back = Budgets
ui-sites = Sites
ui-menu = Menu
ui-back = Back
ui-phone-account = From this phone's accounts…
ui-phone-account-note = Android lends an account's address, never its password: Sioul finds its servers and asks the password once.
account-password = Password…
account-password-title = Password for { $account }
account-password-help = Passwords never travel between your devices: this one is kept in this device's keyring, once its server has accepted it.
account-password-vault = From Bitwarden…
account-password-vault-missing = Your Bitwarden account is not set here: Sites ▸ ⚙, its e-mail, and its server when it is not bitwarden.com. It comes from your other devices with the settings.
account-password-from = From Bitwarden: { $name }.
account-password-use = Use
account-password-keep = Keep
account-password-testing = Asking the server…
account-password-kept = { $account }: password kept; it syncs again.
account-signed-in-again = { $account }: signed in with Google again; its mail comes again.
account-app-password-title = App password for { $account }
bitwarden-unlock-help-password = Your master password opens the vault here, by Sioul itself; it is not kept, and the vault stays open until Sioul closes.
site-none = No site yet. A secure mailbox (a bank's, a hospital's, the tax office's), a chat, any site you want at hand: logged in once, kept here.
site-choose = A site, on the left.
site-android = Here, sites open in your browser: Qt WebEngine, which keeps them inside Sioul on a computer, has no Android version.
site-add = Pin a site
site-field-name = Name
site-field-url = Address
site-field-kind = What it is
site-kind-mailbox = Personal spaces
site-kind-chat = Chats
site-kind-dating = Dating and friendship
site-kind-other = Other sites
site-background = Keep it open, to hear from it
site-back = Back
site-reload = Reload
site-mute = Silence its sounds
site-realtime-help = Its notifications shown at once, until you untick it; else they wait for the Porch.
site-share = Share in the call
site-share-screens = A whole screen
site-share-windows = One window
site-fill-login = Fill the login
site-downloading = Downloading { $file } into your downloads.
site-notified = { $site } has news
site-notified-more = and { $n } more
site-open = Open { $site }
bitwarden-missing = Your Bitwarden account goes in this page’s settings (⚙): its e-mail, and its server when it is not bitwarden.com.
bitwarden-unlock-help = Your security key, or your master password, opens the vault here, by Sioul itself; nothing of either is kept, and the vault stays open until Sioul closes.
bitwarden-open = Open
bitwarden-code = Code
bitwarden-code-0 = The code from your authenticator app.
bitwarden-code-1 = The code Bitwarden just sent you by e-mail.
bitwarden-code-3 = Touch your YubiKey: it types its code.
bitwarden-code-8 = Your recovery code.
bitwarden-factor-0 = Authenticator app
bitwarden-factor-1 = E-mail
bitwarden-factor-3 = YubiKey
bitwarden-factor-8 = Recovery code
bitwarden-factor-unsupported = Your account asks for Duo, which Sioul cannot take yet: add a security key or an authenticator app as a second step in Bitwarden.
bitwarden-factor-key-sites = Your account's second step is your security key, which this dialog cannot ask for. Open Bitwarden once from the Sites page, where the key is asked: the vault then stays open here too until Sioul closes. Or add an authenticator app as a second step in Bitwarden.
bitwarden-factor-key-phone = Your account's second step is your security key, which Sioul cannot ask for on a phone. Add an authenticator app or codes by e-mail as a second step in Bitwarden, or type the password yourself.
bitwarden-waiting = Waiting for Bitwarden…
bitwarden-new-device = A new device: Bitwarden sent a code to your e-mail.
set-bitwarden-email = Bitwarden account
set-bitwarden-email-help = Its e-mail. The vault is read here, by Sioul itself, only when you fill a site’s form, and only with your master password, never kept.
set-bitwarden-server = Bitwarden server
set-bitwarden-server-help = Empty for bitwarden.com; bitwarden.eu; or your own server’s address (Vaultwarden too).
bitwarden-unlock = Unlock Bitwarden
bitwarden-password = Master password
bitwarden-refused = Bitwarden did not unlock: the password may be mistyped.
bitwarden-locked = Bitwarden is locked.
bitwarden-none = Bitwarden keeps no login for this site.
until-tomorrow = tomorrow at { $time }
day-tomorrow = tomorrow
mail-resting = Work mail rests until work comes back. It is all here if you look for it.
mail-rest = While you sleep, mail rests: nothing notifies, and the Porch shows only what your lists let through now. The rest is all here if you look for it.
set-account-area = What this address is for
set-account-area-help = Its mail comes at the times for what it is for: work, your admin, leisure, any of them together. Nothing ticked: work, so that it never reaches your evenings. Your safe senders' mail comes to any address, at the times ticked for them; codes always come.
area-work = Work
area-personal = Yours, admin or leisure
project-field-personal = Yours, outside work
project-field-personal-help = Its tasks stay in view in quiet time, with family and friends.
project-routes = Mail that comes here by itself
project-resting = Work projects rest until work comes back.
ui-show-anyway = Show anyway
folder-server-only = Kept on the server only: right click to keep a copy here.
folder-keep = Keep a copy here
folder-forget = Stop keeping a copy
folder-forget-ask = Its copy here goes; the server keeps every message, and keeping a copy again brings it back whole.
folder-delete = Delete this empty folder
folder-delete-ask = Deleted on the server, for every device. Only an empty folder can be: move its messages out first.
folder-new = New folder…
folder-made = Folder “{ $name }” made on the server.
folder-deleted = Folder “{ $name }” deleted on the server.
folder-not-empty = “{ $name }” still holds messages: move them out first.
sync-held-back = Older mail of { $account } waits: the disk keeps its reserve ({ $free } GB free). It comes once there is room.
note-as-list = List
note-as-tree = Folders
note-all = Every note
note-open-elsewhere = Open with…
audio-play = Play
audio-pause = Pause
audio-position = Where it is
note-memo = Record an audio memo
note-memo-stop = Stop · { $time }
budget-new = New budget
budget-edit = The budget
budget-field-title = Name
budget-field-period = Counted by
budget-period-month = Month
budget-period-year = Year
budget-field-target = Balance to reach
budget-field-area = For
budget-field-area-help = The hours it is in view (Settings, hours). None ticked: your admin.
budget-area-line = For: { $areas }
budget-field-target-hint = 0 to break even
budget-remove = Take this budget out
budget-remove-yes = Take it out
budget-remove-ask = Its lines stay in the file, as they were written; they no longer count anywhere.
budget-line-remove = Delete this line
budget-line-remove-ask = The line goes from the budgets file. What it was tied to stays.
note-rename = Rename…
note-trash = Move to the trash
note-trashed = “{ $title }” moved to the notes’ trash (.trash).
project-remove = Take out of the list
project-remove-ask = Its tasks, notes, mail and time stay where they are; they are no longer gathered under it.
time-change = Change this time
site-remove = Remove this site
site-remove-ask = It leaves Sioul: its page, its notifications. Nothing changes on the site itself.
set-task-lists = Task lists
set-task-lists-help = Renamed here and on the server at the next sync. An empty list can be deleted; one holding tasks stays.
set-calendars = Calendars
set-calendars-help = Renamed here and on the server at the next sync. An empty calendar can be deleted; one holding events stays.
set-address-books = Address books
set-address-books-help = Renamed here and on the server at the next sync. An empty address book can be deleted; one holding contacts stays.
set-collections-remove = Delete it, here and on the server
word-and = and
health-every-days = { $days ->
    [2] every other day at { $time }, from { $from }
   *[other] every { $days } days at { $time }, from { $from }
}
health-every-hours = every { $hours } hours; next: { $next }
health-next-refill = pharmacy from { $day }
health-renew-by = renew by { $day }
health-valid-until = valid until { $day }
health-until = until { $day }
health-covers = for { $names }
health-no-name = A name is needed.
health-field-brand = Brand name, or your word
health-field-brand-hint = As the box says, or as you call it
health-field-generic = Generic name (INN)
health-field-generic-hint = levothyroxine
health-field-strength = Strength
health-field-strength-hint = 75 µg, 500 mg per tablet
health-field-since = Taken since
health-row-precise = Generic name and strength
health-pro-show = Show to a doctor or pharmacist
health-pro-current = Current medicines
health-pro-as-of = As of { $day }
health-pro-prescribed-by = Prescribed by { $name }
health-pro-valid-until = Valid until { $day }
health-pro-until = Until { $day }
health-pro-since = Taken since { $day }, { $long }
health-pro-since-day = Taken since { $day }
health-pro-days = { $n ->
    [one] 1 day
   *[other] { $n } days
}
health-pro-weeks = { $n ->
    [one] 1 week
   *[other] { $n } weeks
}
health-pro-months = { $n ->
    [one] 1 month
   *[other] { $n } months
}
health-pro-years = { $n ->
    [one] 1 year
   *[other] { $n } years
}
health-pro-every-hours = { $hours ->
    [1] every hour
   *[other] every { $hours } hours
}
health-pro-none = No medicine to show.
health-no-take = Add a take: the time it is taken each day.
health-no-take-of = { $name }: add a take, the time it is taken each day.
health-take-unreadable = “{ $time }” is not a time of day.
health-take-unreadable-of = { $name }: “{ $time }” is not a time of day.
health-take-twice = Two takes at { $time }. Keep one, with its amount.
health-take-twice-of = { $name }: two takes at { $time }. Keep one, with its amount.
health-medicine-no-name = Each medicine needs its name.
health-field-takes = Takes
health-add-take = Add a take
health-take-hour = Hour of the take
health-take-minute = Minute of the take
health-take-amount-hint = Its own amount
health-take-remove = Take out this take
health-row-removed = { $name } is taken out when you save: no more reminders for it.
health-keep-it = Keep it
health-row-other-schedule = { $when }; its own form changes when it is taken.
health-taken = Taken
health-errand-refill = Pharmacy: { $title }
health-errand-renew = Doctor: renew the prescription for { $title }
health-category = health
health-list = Health
ui-health = Health
health-local = Your medicines, prescriptions and doses stay on this device. The pharmacy and renewal errands become tasks in the list chosen below, so your phone has them: their titles name the medicine.
health-taken-at = Taken at { $time }
health-medicines = Medicines
health-add-medicine = Add a medicine
health-medicine = The medicine
health-paused = paused
health-prescriptions = Prescriptions
health-add-prescription = Add a prescription
health-prescription = The prescription
health-refilled = Fetched today
health-moving = Moving
health-moving-every = A pause to move while focusing, every
health-minutes = minutes
health-minutes-a-day = minutes a day
health-moving-help = From when Sioul opens, a notification on your desktop says it is time to move, even with the window hidden. During a focus session, it asks first: Pause now, for a few minutes of moving and stretching, or Not now; one click starts the session again.
health-chats = Chats
health-chats-limit = A limit a day on chats
health-chats-after = Covered after
health-chats-for = For
health-chats-help = Once their time is used, chats are covered, muted and silent; they come back after the time chosen, whatever happens.
health-field-name = Name
health-field-dose = Dose
health-field-dose-hint = 50 mg, one tablet
health-field-when = When
health-every-day-choice = At set times each day
health-every-days-choice = Every few days
health-every-hours-choice = Every few hours
health-field-times = At
health-field-every-days = Every … days
health-field-at = at
health-field-from = From
health-field-every-hours = Every … hours
health-field-from-time = from
health-field-until = Until
health-field-prescription = Prescription
health-no-prescription = None
health-pause = Paused for now
health-remove = Take it out
health-remove-medicine-ask = No more reminders for it. What was marked taken stays marked.
health-remove-prescription-ask = Its medicines stay; they are no longer tied to it.
health-field-what = What
health-field-what-hint = Vitamin D 1000 IU
health-field-prescriber = Prescribed by
health-field-valid-until = Valid until
health-field-refill-days = The pharmacy gives
health-days-at-a-time = days at a time
health-field-last-refill = Last fetched
health-field-note = Note
focus-move = Time to move and stretch for a few minutes. The session waits here.
focus-move-back = Back to it
focus-move-ask = Time to move and stretch for a few minutes? The session goes on until you pause it.
focus-move-now = Pause now
focus-move-not-now = Not now
stopped-hint = Where I stopped, in one line, for when I'm back
stopped-title = Where you stopped
stopped-done = Done
stopped-at = Where you stopped, { $when }
site-chats-covered = Chats rest for now: their time for today is used. They come back by themselves.
weather-now = Now
weather-this-morning = This morning
weather-this-afternoon = This afternoon
weather-this-evening = This evening
weather-tonight = Tonight
weather-tomorrow-morning = Tomorrow morning
weather-tomorrow-afternoon = Tomorrow afternoon
weather-tomorrow-evening = Tomorrow evening
weather-tomorrow-night = Tomorrow night
# The phone's home card: the next of each part of the day, in order (weather.rs, `card`).
weather-part-morning = Morning
weather-part-afternoon = Afternoon
weather-part-evening = Evening
weather-part-night = Night
weather-rain-chance = { $chance } %
weather-clear = Clear sky
weather-mainly-clear = Mainly clear
weather-partly-cloudy = Partly cloudy
weather-overcast = Overcast
weather-fog = Fog
weather-drizzle = Drizzle
weather-freezing-drizzle = Freezing drizzle
weather-rain = Rain
weather-heavy-rain = Heavy rain
weather-freezing-rain = Freezing rain
weather-snow = Snow
weather-showers = Showers
weather-snow-showers = Snow showers
weather-thunderstorm = Thunderstorm
weather-hail = Thunderstorm with hail
weather-unknown = Unknown
weather-credit = Weather data by Open-Meteo.com
weather-choose = Choose a place for the weather
weather-change = Change the place
weather-search = A town, a village
weather-privacy = Asks Open-Meteo, with this place’s coordinates only.
weather-none = No forecast yet.
weather-tip = The weather now at { $place }. Click for the hours and days to come, and to change the place.
weather-tip-hours = The weather at { $place }: now, then hour by hour for the next two hours. Click for the hours and days to come, and to change the place.
sounds-title = Sounds
sounds-focus = To focus
sounds-white = White noise
sounds-pink = Pink noise
sounds-brown = Brown noise
sounds-calm = Your recordings
sounds-calm-none = Recordings put in a “sounds” folder of your notes show here: rain, sea, wind, crickets. Free ones are listed in Sioul’s documentation (docs/sounds.md).
sounds-volume = Volume
sounds-stop = Stop
webauth-title = Security key for { $site }
webauth-account = Which of the key’s accounts?
webauth-pin = The key’s PIN ({ $left } tries left).
webauth-pin-set = The key has no PIN yet: choose one, { $length } characters at least.
webauth-pin-change = The key asks for a new PIN, { $length } characters at least.
webauth-pin-field = PIN
webauth-pin-again = The same PIN again
webauth-pin-uv-locked = The key’s own check is locked: the PIN is needed.
webauth-pin-wrong = Not this PIN.
webauth-pin-too-short = Too short.
webauth-pin-invalid = The PIN has characters the key does not take.
webauth-pin-same = The new PIN is the old one.
webauth-touch = Touch your key again.
webauth-go = Continue
webauth-retry = Try again
webauth-failed-timeout = The key was not touched in time.
webauth-failed-not-registered = This key is not registered with this site.
webauth-failed-already-registered = This key is already registered with this site.
webauth-failed-soft-block = Too many wrong PINs for now: unplug the key and plug it in again.
webauth-failed-hard-block = Too many wrong PINs: the key must be reset before it works again.
webauth-failed-removed = The key was taken out while its PIN was asked.
webauth-failed-no-resident = The key cannot keep this site’s sign-in on itself.
webauth-failed-no-uv = The key cannot check who you are (no PIN, no fingerprint).
webauth-failed-no-blob = The key cannot store what this site asks.
webauth-failed-no-algorithm = The key and the site share no way to sign.
webauth-failed-full = The key is full: an old sign-in has to go first.
webauth-failed-denied = Refused on the key.
webauth-failed-cancelled = Cancelled.
task-list-no-tasks = This list does not keep tasks.
task-list-greyed = Keeps no task
task-move-ask = { $place } would not keep: { $list }. Move it all the same?
contact-moved = Contact moved.
contact-gone = This contact is gone.
contact-read-only = This contact can only be read here.
contact-book-move-ask = { $place } would not keep: { $list }. Move it all the same?
move-anyway = Move it
loss-waits = what it waits for
loss-start = its day to start
loss-due-time = the hour of its date
loss-repeat = how it comes back
loss-priority = its order
loss-kind = its kind
loss-estimate = its length
loss-links = its links
loss-contacts = the people tied to it
loss-categories = its categories
loss-margins = the time kept before and after it
loss-costs = what it costs and what it gives back
loss-labels = the labels of its addresses and numbers
loss-birthday-no-year = a birthday without its year
loss-vcard4 = what only vCard 4.0 holds (gender, anniversary, relations)
mode-quiet = Work comes back { $until }.
mode-time-off = Time off{ $label ->
    [none] {""}
   *[other] {""}: { $label }
}. Work comes back { $until }.
mode-back-to-plan = Back to today’s plan
mode-work-30 = Work half an hour more
mode-work-60 = Work an hour more
mode-work-120 = Work two hours more
mode-work-240 = Work four hours more
quiet-title = Rest of the day
quiet-time-off-title = Time off
closing-put-away = Work is put away until { $back }.
# After it, on the screen once the work day is closed (DoneDialog.qml, docs/reviews.md).
closing-yours = The rest of the day is yours.
closing-done = Done today: { $list }.
closing-worked = Worked on: { $list }.
closing-rest = Everything else has its place, from { $day }, in the usual order.
closing-starts-with = { $day } starts with:
closing-change-step = Change
closing-step-hint = A cue and an action: “After breakfast, open the form”
closing-deadline = The date asked for “{ $title }” is { $day }.
closing-ask-time = Ask for more time
closing-leave = Leave it for { $day }
closing-resting = Mail and tasks are resting. One-time codes still arrive, and messages from the people you marked as close.
closing-plan = Stopping before you run out is part of the plan.
first-step-line = { $day }. The plan starts with: { $step }
day-today = today
task-note-for-later = Note for when work comes back: one line, out of sight until then
task-noted-for = Noted for { $day }.
set-task-kinds = Kinds of task
set-task-kinds-help = Each kind gathers tasks done the same way: calls together, forms together. Rename one in place, take it away, or add yours; tasks keep the kind they have.
set-task-kinds-new = A new kind
set-task-categories = Categories
set-task-categories-help = The categories your tasks have, the most used first. A new name renames it on every task that has it; taking one away takes it off them all.
set-task-categories-remove = Take it off every task
mode-working-late = Work stays in view until { $until }.
mode-usual-hours = Back to the usual hours
mode-work-now = Work now
mode-work-now-help = Work shown whatever the hours: until you untick it, Sioul closes, or the next working day is over.
mode-work-now-line = Work shown until { $until }, by your choice.
done-button = Done for today
done-button-help = A word on the day, if you like; then today’s tasks move to their next day, and work rests until it comes back.
first-step-clear = Put away
set-language = Language
set-language-help = The language of every sentence Sioul says.
set-language-system = The system's
set-language-fr = Français
set-language-en = English
realtime-on = Real time: every folder is fetched each minute, and the Porch stays open.
realtime-off = Real time is off: back to the usual pace.
ui-realtime = Real time
ui-realtime-help = Every folder of every address, each minute, and the Porch open: for a code or a password you are waiting for.
reason-hostile = read as hostile (insults, harassment or threats): its words are not shown
reason-public = to your public address, about { $topic }
topic-work = work
topic-support = a question
topic-press = the press
topic-thanks = thanks
topic-donation = a donation
topic-other = something else
tone-calm = calm
tone-hostile = hostile
shield-none = No address is shielded: set "shield = true" on an account, or in Mail's settings.
shield-read = The AI read { $n } { $n ->
    [one] message
   *[other] messages
}; { $left } left for the next rounds.
shield-words = Words that weighed: { $words }
tone-rude = rude
porch-open-until = Open until { $time }.
porch-opened-anyway = Opened outside a window.
lane-public = Your public address, { $address }
lane-hostile = Hostile, set aside
lane-about-project = Mail that matches this project's routes.
lane-about-public = To { $address }, from someone you have not let in, read first and sorted by topic: work first.
lane-about-people = From people you know: in your address books, let in, or named on a list.
lane-about-screener = From someone new: let them in, or block them.
lane-about-filed = Newsletters and automatic senders, to read when you like.
lane-about-low = From addresses you ranked less important.
lane-about-set-aside = Forged, borrowing a name, spam, or blocked. Nothing is deleted.
lane-about-review = What your own spam filter caught waits here, never notified. Sioul learns from it as it is; if one is not spam, “Not spam” puts it back and teaches the filter.
lane-about-hostile = Insults, harassment or threats to your public address. Their words stay hidden.
rule-order = Each message goes to the first lane that takes it, in this order: set aside, hostile, what your own spam filter flags, codes, projects, what you send yourself, your public address, less important addresses, newsletters, someone new, people you know.
rule-people = Anyone in your address books, or on a list by their own address, comes here, as do the senders you let in: your list of them holds { $n } entries, and "Let in", in a message from someone new, adds them.
rule-screener = A stranger (in none of your address books, on no list) waits here, and so does a sender a domain alone names on a list, unless a project or the newsletter rule takes the message first.
rule-filed = A message that says it comes from a mailing list (List-Id, List-Unsubscribe, Precedence: bulk) is a newsletter. A sender whose address holds one of these words is automatic: { $words }.
rule-low-none = No address is ranked less important yet: an address's rank is set on its card in Accounts.
rule-low = Addresses ranked less important: { $addresses }. Their codes still come at once.
rule-forged = Forged: the sender's domain says it did not send it (DMARC). Other checks failing do not make it forged: its sender is then not verified.
rule-borrowed = Borrowing a name: the name shown claims a brand, or your own domain, that the address does not belong to.
rule-spam = Spam: your provider's filter says so, of a stranger's mail; never of someone you know, a code, a project's mail, nor of a message you said is not spam.
rule-review-class-spam = Probably spam (from { $p }%)
rule-review-class-unsure = Maybe spam (from { $p }%, below the spam threshold)
rule-review-class-ham = Probably not spam (below { $p }%)
rule-review-move = { $class }: moved into the account's Junk folder on the server as it arrives, and listed here while it stays there.
rule-review-flag = { $class }: left where it is, marked, and listed here.
rule-review-nothing = { $class }: nothing is done; it goes to its lane as if unjudged.
rule-review-protected = Only a stranger's mail is judged: never someone you know, a code, a project's mail, your own, nor a message you said is not spam, on any of your devices.
rule-review-buttons = Sioul learns from them as they are: what was moved into the Junk folder, or flagged as probably spam, as spam; a maybe spam, not until you say. “Not spam” puts one back in its lane, from the Junk folder into the inbox, for good, on every device, and teaches the filter; “Spam” moves a flagged one into the Junk folder. Each, and each “for all”, can be undone for ten seconds.
rule-review-where = What it does with each verdict, and how sure it must be: Mail ▸ ⚙, “Your own spam filter”.
rule-blocked = Blocked: from your list of blocked senders; such mail is never shown, never counted.
rule-project-none = This project has no route yet: no mail comes here by itself.
rule-route = { $parts }.
rule-part-domains = From { $list } (and its subdomains)
rule-part-addresses = From { $list }
rule-part-subject = with { $list } in the subject
rule-part-text = with { $list } in the text
rule-and = , and{" "}
rule-public = Read before you see it, by word lists in French and English; hostile mail goes to "Hostile". Fetched every { $minutes } minutes.
rule-public-ai = An AI reads it too, as you allowed, and says its tone and topic more finely.
rule-hostile = Insults aimed at you, harassment and threats come here. The sender's name, the subject and the text stay hidden until you choose to read them.
rule-where-shield = An address’s shield is set on its card in Accounts.
rule-where-rank = An address’s rank is set on its card in Accounts.
rule-where-blocked = Blocked senders are in Settings, under What reaches you ▸ By person.
rule-where-routes = Its routes are set on its page in Projects.
hostile-someone = Someone
hostile-someone-at = Someone at { $domain }
hostile-subject = A message set aside as hostile
ui-lane-help = How mail lands here
set-filed-words = Words of automatic senders
set-filed-words-help = A sender whose address holds one of these words is automatic: its mail is filed with the newsletters. They are in Settings ▸ Words, with the other words Sioul looks for.
set-words-add = Add a word
set-routes = This project's routes
set-routes-help = Mail goes to this project when one route matches: every line filled in a route must match. Several words or domains on a line: any one of them. A reply in the conversation of a message of the project goes with it.
set-route = Route
set-route-domains = From the domains
set-route-addresses = From the addresses
set-route-subject = Words in the subject
set-route-text = Words in the text
set-route-attachments = Words in an attachment’s name
set-route-add = Add a route
set-route-comma = Separated by commas
hostile-gate = This message was set aside as hostile.
hostile-gate-help = Its words are not shown. It can wait, go to someone you trust, or go away. Reading it is your choice, at a time that suits you.
hostile-forward = Forward to someone you trust
hostile-read = Read it anyway
ui-choose = Choose…
set-font-desktop = The desktop's
ui-reading = How text reads

# Google: calendars and contacts over its CalDAV and CardDAV, tasks over Google Tasks.
ui-add-google = Google calendars, contacts and tasks
ui-add-google-note = Google takes no password from other programs: you sign in on Google's page, in your browser. With no Google key built into this copy of Sioul, or to use your own, you make one once, in your own Google project. Google keeps less than an open server; what it does not keep shows greyed.
ui-add-google-built-in = Google takes no password from another program: you sign in on Google's own page, in your browser, and Sioul keeps the access in the system keyring. Google keeps less than an open server; what it does not keep shows greyed.
ui-google-own-key = Use a Google key of my own
ui-google-steps = How to make your Google key (once, about fifteen minutes)
ui-google-steps-text =
    1. On [console.cloud.google.com](https://console.cloud.google.com), make a project named Sioul.
    2. In *APIs & Services → Library*, turn on **CalDAV API**, **CardDAV API** and **Google Tasks API**.
    3. In *Google Auth platform*: *Branding*, a name and your address; *Audience*, External, and yourself as a test user; *Data access*, the scopes `…/auth/calendar`, `…/auth/carddav` and `…/auth/tasks`.
    4. *Clients → Create client → Desktop app*. Copy its ID and its secret now: Google shows the secret only once.
    5. *Audience → Publish app*. Left in testing, Google ends the access every seven days.
    6. Paste both here and sign in. Google says the app is not verified: it is yours. Choose *Advanced*, then *Go to Sioul*.

    Google deletes a key unused for six months; Sioul uses it at each sync, so it stays.
ui-google-client-id = Client ID
ui-google-client-secret = Client secret
ui-google-sign-in = Sign in with Google
ui-google-waiting = Your browser is open on Google's page. Sioul waits here.
ui-google-again = Sign in again
google-timeout = Google did not answer within five minutes: nothing changed.
google-denied = Google gave no access: nothing changed.
google-state = An answer came that was not for Sioul: nothing changed.
google-other = Your browser signed in as { $address }, not the address given: nothing changed, and that access went back to Google.
google-cancelled = Stopped: nothing changed.
# Google's mail (Gmail, Google Workspace): an app password, or Google's sign-in with a key of your own (docs/google.md, "Mail").
ui-gmail-choice-app-password = Use an app password
ui-gmail-choice-sign-in = Sign in with Google
ui-gmail-app-password-guide = Google refuses your account password here. Make an app password for Sioul at myaccount.google.com/apppasswords (it needs 2-Step Verification), then paste it below.
ui-gmail-app-password-field = App password (from Google)
ui-gmail-sign-in-key-kept = Google's page opens in your browser, with your Google key kept on this device. Sioul keeps the access in the system keyring: no password.
ui-gmail-sign-in-own-key = For mail, Google lets in only a key of your own: a free project in Google Cloud, made once, in about fifteen minutes. Google then says the app is not verified: it is yours.
ui-gmail-steps-text =
    1. On [console.cloud.google.com](https://console.cloud.google.com), make a project named Sioul, or open the one your Google calendars use.
    2. In *APIs & Services → Library*, turn on the **Gmail API**.
    3. In *Google Auth platform*: *Branding*, a name and your address; *Audience*, External, and yourself as a test user; *Data access*, the scope `https://mail.google.com/`.
    4. *Clients → Create client → Desktop app*. Copy its ID and its secret now: Google shows the secret only once.
    5. *Audience → Publish app*. Left in testing, Google ends the access every seven days. Used by you alone, it needs no review by Google.
    6. Paste both here and sign in. Google says the app is not verified: it is yours. Choose *Advanced*, then *Go to Sioul*, and leave Gmail's access ticked.
ui-gmail-again-app-password = App password…
ui-google-waiting-phone = Your browser is open on Google's page. Once Google says Sioul has the access, come back to Sioul: it finishes here.
google-mail-built-in = Google does not let Sioul's own key read mail yet: Gmail's access needs Google's review of Sioul, not done. Use an app password, or a Google key of your own.
google-mail-no-key = No Google key of yours is kept on this device for { $address }: give its client ID and secret, or use an app password.
google-mail-unticked = Google gave no access to your mail: on Google's page, leave Gmail's access ticked ("Read, compose, send, and permanently delete all your email from Gmail"). Nothing changed.
sync-error-google-again = { $account }: Google asks you to sign in again.
sync-error-google-testing = { $account }: Google ended the sign-in after seven days, as it does while your Google project is in testing. In Google Cloud: Google Auth Platform ▸ Audience ▸ Publish app; then sign in again: it lasts after that.
google-no-collections = Google makes no calendar or address book from another program: make it on Google's pages.
google-fixed-collection = Google renames and deletes its calendars and address books on its own pages only.
google-tasks-greyed = Google Tasks does not keep this.
task-limited = This list is on Google Tasks: what Google does not keep is greyed (a start day, a length, a project, a kind, office hours, categories, what it takes, repeating, waiting for another task, steps of steps).

# GitHub, once asked: issues and pull requests as tasks.
set-code-group = Code
set-github = GitHub issues and pull requests as tasks
set-github-help = For those who work on GitHub: what is yours comes into a "GitHub" list on this device, every thirty minutes, and GitHub's notification mail is tied to it. Nothing is written to GitHub.
set-github-token = GitHub token
set-github-token-help = A fine-grained token that reads issues and pull requests, nothing more, kept in the system keyring. "Make a token on GitHub" opens GitHub's page with its rights filled in: choose the repositories (all of yours, or some), make it, and paste it here.
set-github-token-kept = A token is kept.
set-github-token-none = No token yet: nothing comes from GitHub.
set-github-token-forget = Forget the token
set-github-token-make = Make a token on GitHub
set-github-assigned = Assigned to you
set-github-assigned-help = Issues and pull requests whose assignees include you.
set-github-reviews = Your review asked
set-github-reviews-help = Pull requests whose review is asked of you by name.
set-github-created = Opened by you
set-github-created-help = Your own issues and pull requests, while open.
set-github-mentioned = Mentioning you
set-github-mentioned-help = Where someone wrote your name. Often many: off unless you want them.
sync-error-github-token = GitHub refused the token: a new one goes in the task settings, “Code”.
note-not-here = Not on this device yet: it may still be syncing (Nextcloud, Dropbox).
ui-new = New
new-mail = A message
new-task = A task
new-event = An event
new-contact = A contact
new-note = A note
new-movement = A budget movement
new-time = Time spent
new-project = A project
ui-parameters = Settings
settings-tab-look = Display
settings-tab-hours = Hours
settings-tab-reminders = Reminders
settings-tab-files = Your folder and sharing
settings-tab-invoices = Invoices
settings-tab-ai = AI agents
ui-refresh-all = Refresh everything: mail, agenda, tasks, contacts (F5)
ui-refreshing = Refreshing…
# The column of the places, on the left of the window.
ui-places = Places
ui-places-names-show = Show the names
ui-places-names-hide = Icons only
tray-show = Show Sioul
tray-hide = Hide the window
tray-quit = Quit Sioul
tray-said-title = Sioul goes on
tray-said = Its window is hidden; reminders, medicines and mail go on. A click on its icon here brings it back; Quit is in its menu.
# The window's buttons, in the title bar Sioul draws on a computer: their names, on hover and for screen readers.
titlebar-minimize = Minimize
titlebar-maximize = Maximize
titlebar-restore = Restore
titlebar-full-screen-leave = Leave full screen (F11)
titlebar-close = Close
titlebar-close-tray = Close: Sioul goes on in the system tray
ui-refresh-short = Refresh everything
budget-add-budget = Budget
set-look-group = Language and appearance
set-hours-group = Working hours and quiet time
set-hours-elsewhere = Working hours, hours for your admin and days off are in Settings: the sliders icon at the bottom of the left column. Meals and sleep are on the Health page.
porch-hours-none = Your hours are not set: work and your admin come at any time.
porch-hours-some = Not set yet: { $which }.
porch-hours-why = Each kind of hours brings its own things forward and lets the rest wait: work in working hours, offices and bills in admin hours; every other time is leisure.
porch-hours-set = Set my hours
porch-hours-leave = Leave as is
site-column-narrow = Fold the list to its icons
site-column-widen = Show the sites' names
set-senders-group = Who may write to you, and when
set-safe = Safe
set-safe-help = Friends, chosen colleagues, chosen family. An address, a number (+33 1 99 00 12 34), or a pattern with *: *@example.org for everyone there, *@*.example.org for its subdomains, +3319900* for the numbers that start so. Forged mail never counts as theirs.
set-neutral = Neutral
set-neutral-help = Anyone in your address books is neutral without being named. Name someone here to keep them neutral inside a domain or a category on another list.
set-blocked-all = Blocked
set-blocked-all-help = Spam and harassment: never, on any channel. Their mail is set aside for good, never shown, never notified; their calls are refused. The most precise entry wins: an address marked safe stays safe in a domain blocked here. Nobody is blocked for sharing a server or a domain with someone else.
sender-now-safe = { $entry } is safe: their mail comes as the Safe row says, in What reaches you.
sender-now-neutral = { $entry } is neutral: their mail comes as the Neutral row says, in What reaches you.
sender-now-blocked = { $entry } is blocked: their mail is set aside for good.
sender-unverified = This message's sender is not verified: the address may belong to someone else, so it changes no list.
sender-unverified-short = not verified
set-quiet-work = What is work
set-quiet-work-help = Tasks of these categories, of projects for a client, and from GitHub are work: they come in working hours only. A task can also say what it is for in its panel.
task-tag-add = tag
task-tag-remove = Take off the tag { $tag }
note-folder-new = New folder
note-folder-new-inside = New folder inside…
note-new-here = New note here…
note-folder-rename = Rename the folder…
note-folder-remove = Take out this empty folder
note-folder-remove-full = Take out (empty it first)
note-folder-not-empty = This folder holds notes or folders: move or trash them first.
site-edit = Name and address…
site-https-only = Only an https:// address: a site's sign-in never travels in clear.
budget-line-change = Change this line…
set-projects-group = Projects
set-porch-projects = Projects shown here
set-porch-projects-help = Each project ticked has its lane on the Porch. Mail for the others stays on their page in Projects, where projects are made, renamed and given their routes.
loss-billable = whether its time is billed
time-export = Billable time, as a spreadsheet (CSV)…
time-export-project = Project
time-export-range = Time
time-export-from = From
time-export-to = To
time-export-save = Save…
time-range-this-week = This week
time-range-last-week = Last week
time-range-this-month = This month
time-range-last-month = Last month
time-range-all = All time
time-range-custom = From one day to another
time-csv-what = What
time-csv-hours = Hours
time-csv-rate = Hourly rate
time-csv-amount = Amount
time-csv-total = Total
time-exported = Written to { $path }.
task-field-billable = Billed
task-billable-project = As its project says
task-billable-yes = Its time is billed
task-billable-no = Not billed
health-errands-list = Errands go to
health-list-here = on this device only
scan-ask = No antivirus is installed on this device, so { $name } will not be checked. Open it only if you trust it. To have files checked: { $hint }
scan-ask-phone = A phone has no antivirus that Sioul can call, so { $name } will not be checked. Open it only if you trust it. To have it checked: { $hint }
scan-open-anyway = Open it unchecked
scan-save-anyway = Save it unchecked
scan-unavailable-short = No antivirus answered ({ $detail }).
scan-too-big = { $name } was not scanned: it is larger than the antivirus scans.
scan-ask-too-big = { $name } was not scanned: it is larger than the antivirus scans, or an archive holds more than it reads. Open it only if you trust it.
sounds-nature = Nature, made here
sounds-waves = Waves on a beach
sounds-rain = Rain
sounds-wind = Wind in the trees
sounds-crickets = Crickets at night
sounds-storm = A distant storm

## Sharing with your other devices (Parameters)
share-title = Between your devices
share-help = Mail, contacts, the agenda and tasks on a server already reach your other devices. The rest can travel through a folder your sync carries (Nextcloud, Dropbox, Syncthing), sealed with a passphrase, so that its server never reads it: what Sioul keeps on this device (settings, who may write to you, time, drafts, invoices, health, lists kept here) and, when no sync carries them, your notes, projects and papers.
share-off = Not shared: all of it stays on this device.
share-on = Shared through { $folder }.
share-others = { $count ->
    [one] With one other device (last news: { $when }).
   *[other] With { $count } other devices (last news: { $when }).
}
share-alone = No other device yet: on the other one, choose the same folder and type the same passphrase.
share-last = Last exchange here: { $when }.
share-devices = Your other devices
share-device-in-use = In use now; last shared { $when }.
share-device-quiet = In use when it last shared, { $when }, and silent since: it may have stopped without closing.
share-device-closed = Closed { $when }; last shared { $shared }.
share-device-older = An older Sioul: last heard { $when }.
share-device-unread = What it says does not read here yet.
share-device-holds = Known here by what it shared, last heard { $when }: until it is updated there, or forgotten here, your devices keep sharing in the form an older Sioul reads.
share-device-holds-unknown = Known here by what it shared: until it is updated there, or forgotten here, your devices keep sharing in the form an older Sioul reads.
share-device-off = Counted as off, as you said, until it shares again.
share-device-silent = Silent since { $when }: no longer counted for your doses.
share-device-apart = It does not share its doses.
share-device-ahead = Its clock is { $minutes ->
    [one] a minute
   *[other] { $minutes } minutes
} ahead of this one's, at least.
share-device-count-again = Count it again
share-device-forget = Forget this device
share-outside = Your notes and projects ({ $store }) do not seem to be in a folder your sync carries: the other device would not see them. Switch Notes, and Projects and money, on below to carry them through this folder, sealed; or move them into a synced folder (and choose it again in Settings ▸ Your folder and sharing).
share-phones = A phone sees this folder only if its sync app carries it, and some carry only a few folders: Murena's eDrive carries Documents (with Pictures, Music…), not the rest of your cloud. To reach such a phone, share through a folder inside Documents: stop sharing, then choose one there.
folder-not-on-device = This folder is not on this device: choose one your sync app keeps its files in (Documents, with Murena's eDrive).
folder-browser-title = Choose a folder
folder-browser-up = Up
folder-browser-choose = Choose this folder
folder-browser-shared = shared by your other devices
folder-browser-empty = No folder inside.
folder-browser-unreadable = Sioul cannot read this folder.
share-folder = Folder
share-choose = Choose…
share-passphrase = Passphrase
share-again = Once more
share-passphrase-hint = A few words you will not forget, at least 12 characters, typed once on each device and kept in its keyring. Never sent anywhere: lost, it cannot be found again (start over with a new folder; nothing here is lost).
share-passphrase-known = Another device shares through this folder: type the passphrase chosen there.
share-start = Share
share-now = Exchange now
share-stop = Stop sharing
share-stop-ask = This device stops sharing: it sends nothing more and takes in nothing from your other devices, and it forgets the passphrase. Everything on this device stays as it is, and your other devices keep what they have and go on sharing among themselves. To share again later, you will need the passphrase.
share-stop-confirm = Stop sharing from this device
share-short = At least 12 characters: a few words make a good passphrase.
share-differ = The two passphrases differ.
share-wrong = This is not the passphrase chosen on your other device.
share-no-folder = Choose a folder first.
share-other-seal = A device in this folder seals with another passphrase: its changes are left aside.
share-no-seal = The sharing folder, or its seal, is not where it was: nothing is shared from here until it is back. Is your sync app running, and the disk it is on there?
share-sealed-otherwise = The sharing folder is now sealed with another passphrase than this device's: nothing is sent from here until you start sharing again here with the passphrase your other devices use.
share-twin = Another device shares under this device's name (its disk or its settings were copied to it): stop sharing on one of them, then start again there, which gives it a name of its own.
share-own-ahead = This device's records in the folder went further than its memory (restored from a backup, or sharing set up again): it goes on after them, and reads everything again.
share-own-cut = The sync app put back an older copy of this device's records: it started them again, holding everything.
share-other-gap = Some records of another device are missing here: what it did meanwhile is not known (a dose shows the doubt) until it restates everything.
share-other-cut = The sync app put back an older copy of another device's records: they are read again.
share-other-line = A line of another device's records could not be read here: what it held is not known (a dose shows the doubt).
share-unreadable = { $file } could not be read: what came for it waits until it can be.
share-key-missing = Type the passphrase again on this device: the keyring no longer holds it.
share-not-shared = Not shared: how pages are laid out on this screen, the folders each device keeps its files in, this device's browser notices, your own PGP keys (copy them by hand), caches.
share-found = Already shared by your other devices (pick one):
share-files-access = To read the folder your sync app carries (eDrive, Syncthing, FolderSync…), Sioul needs Android's access to your files.
share-files-allow = Allow access to files
share-parts = What travels from this device
share-parts-help = Each device chooses for itself. A part switched off here stays as it is on your other devices: nothing of it is taken out there. Switched on again, it joins as a new device would.
share-part-settings = Settings and accounts
share-part-settings-carries = Your settings and accounts (never their passwords), the ties between things, where the Porch was closed, mail you said is no payment, do-not-disturb's switch.
share-part-senders = Senders
share-part-senders-carries = Who is on which list, and how the lists name them (known, blocked, safe, neutral, restricted: addresses, numbers, cards), your Always through people, what the shield read, others' public keys.
share-part-calls = Calls
share-part-calls-carries = The calls your phones screened: when, the number, who it was by your lists, the time of day, whether it rang or went to voicemail, and why; and the calls you marked Seen. Sealed. Never a call from a number you blocked, never a voicemail's sound: that comes with your mail.
share-part-phone-messages = Messages from your phone
share-part-phone-messages-carries = From the notifications of the apps you choose on your phone (Settings ▸ This phone ▸ On your computers; its SMS app unless you say otherwise): who wrote, when, in which conversation, and the words, or only who and when; and the messages you marked Seen. Sealed, kept a week. Never a code, never a picture, never a blocked sender's message. Off until you turn it on: on your phone to send them, on a computer to show them there.
# texts: the sharing's part for texts (SMS phase b).
share-part-texts = Texts
share-part-texts-carries = Your phone's texts, the whole history with the multimedia messages' pictures, sounds and videos, read on the phone and kept on your computers as an archive that only grows; the texts you write on a computer for your phone to send, and what became of each. Sealed, on every device too: a computer's copy opens only with your sharing key. Off until you turn it on: on your phone, and on each computer that should read or write them.
share-part-spam = Spam filter
share-part-spam-carries = The table your computer's training makes, so that every device judges mail alike, and what you said is spam or not on each device (which message, where and when; never a word of it), so that no device flags again what you corrected on another: sealed. Never the mail it learned from, nor the words it learned.
share-part-health = Health
share-part-health-carries = Medicines, prescriptions and the doses taken.
share-part-time = Time
share-part-time-carries = Time noted, the session running, the day's choices, where you stopped, working late or done for the day.
share-part-drafts = Drafts and invoices
share-part-drafts-carries = Mail being written, invoices made.
share-part-projects = Projects and money
share-part-projects-carries = From your notes folder: projects and their mail routes, budgets, bank accounts and movements, contracts.
share-part-lists = Lists kept here
share-part-lists-carries = Calendars and contacts kept on this device only.
share-part-notes = Notes
share-part-notes-carries = Your notes folder: notes, their pictures, PDFs and memos, scanned letters. Each file sealed apart, only what changed sent; files over 64 MB stay.
share-part-papers = Papers
share-part-accounts = Your calendars and contacts (your version, set aside when an item changed here and on its server too)
share-part-papers-carries = The papers wallet and its files.
share-part-carried = Your notes folder ({ $store }) is carried by a sync app already: carried here too, the two would undo each other's changes. To share it here, move it to a folder no sync carries, or leave it to that sync.
share-part-sent = Last sent: { $when }.
share-part-received = Last received: { $when }.
share-part-quiet = Nothing exchanged yet.
share-conflict = Two devices changed the same file: both versions are kept, the other one as “{ $copy }”.
share-conflict-gone = A file changed here was taken out on another device: what changed is kept as “{ $copy }”.
share-damaged = { $file } came damaged from another device: your copy here stays as it is until a whole one comes.
share-too-big = { $file } is over 64 MB: it stays on this device.
share-history-help = Before another device's change is written into a file here, the file as it was is kept on this device: the last 20 versions of each file, and all those of the last 30 days. So is your version of an event, a task or a contact changed here and on its server too, the server's version kept. They are never shared.
share-history-show = Show earlier versions
share-history-hide = Hide earlier versions
share-history-empty = Nothing kept yet.
share-versions = { $count ->
    [one] One version
   *[other] { $count } versions
}
share-put-back = Put back
share-put-back-done = { $file } is back as it was ({ $when }). The file as it was just before is kept in the list too.
share-history-more = { $count ->
    [one] One more file, changed earlier: type part of its name to find it.
   *[other] { $count } more files, changed earlier: type part of a name to find them.
}
share-history-filter = Find a file by its name
share-put-back-whole = { $file } goes back as it was ({ $when }); the file as it is now is kept in the list.
share-put-back-nothing = { $file } holds that version's entries already: nothing to put back.
share-put-back-entries = { $file } as it was ({ $when }): { $changed ->
    [one] one entry goes back to what it was
   *[other] { $changed } entries go back to what they were
}, { $returning ->
    [one] one taken out since comes back
   *[other] { $returning } taken out since come back
}; { $kept ->
    [one] one added since stays
   *[other] { $kept } added since stay
}, here and on your other devices.
share-putting-back = Putting back…
share-estimate = { $count ->
    [one] One file
   *[other] { $count } files
}, { $size } in all, would travel from this device, each sealed apart; { $big ->
    [0] none is too big.
    [one] one over 64 MB stays.
   *[other] { $big } over 64 MB stay.
} Switch on?
share-estimating = Counting what would travel…
share-switch-on = Switch on
share-carried-ask = Does a sync app on this phone carry your notes folder ({ $store })? Sioul cannot read its settings to tell. Carried by both, the sync app and the sharing would undo each other's changes.
share-carried-not = No sync app carries it: switch on
share-carried-yes = A sync app carries it
share-carried-wrong = No sync app carries it on this phone
share-vanished = { $count ->
    [one] One file
   *[other] { $count } files
} went from { $folder } here at once: nothing of them is taken out on your other devices until you say so (a disk not mounted, a folder moved, an access withdrawn would look the same).
share-vanished-confirm = Take them out everywhere
share-older-copy = An older copy was put back here by hand (its date before the version last seen): that version is back, the older copy kept beside it as “{ $copy }”. To send an older version, use Put back below.
share-missing = { $file } changed on another device a day ago, but its content has not come: is the sync app still carrying the sharing folder?
share-no-room = { $file } waits: this device lacks room for it (and for the copy kept before it is written).
share-name-clash = { $file } waits: another file here differs from it by case or accents only, which this device's storage takes for one.
share-refused = { $file } is not written: its place goes through a link, or into the sharing folder or Sioul's own.
share-not-text = { $file } stays here: its name is not text Sioul can carry.
share-emptied = { $file } is empty here: nothing of it is taken out on your other devices for ten minutes.
share-files-unreadable = Notes and papers wait: without Android's access to all your files, Sioul cannot see those other apps made, and would take them for gone.
# Fetched from the server too (docs/database.md): the other devices' files read on the server that holds the sharing folder, beside the sync app.
share-backup-switch = Also fetch them from the server
share-backup-help = When your sync app is late bringing your other devices' files, Sioul also reads them on the server that holds this folder (Nextcloud, Murena, ownCloud), with your account there. It only reads, and only a folder sealed as this one.
share-backup-on = Also fetched directly from { $host }, last at { $when }.
share-backup-left = Some records of your other devices are too large to fetch directly: your sync app brings them, and Sioul asks it to look until it has.
share-backup-soon = Found on { $host }: fetched from there too at the next exchange.
share-backup-failing = Fetched directly from { $host } until { $when }; not since: { $why }. Tried again at the next exchange.
share-backup-failing-never = Found on { $host }, not fetched yet: { $why }. Tried again at the next exchange.
share-backup-why-network = it could not be reached
share-backup-why-login = it refused the password kept for your account there
share-backup-why-tls = its certificate could not be checked
share-backup-why-server = it answered with an error
share-backup-why-disk = this device lacks room
share-backup-looking = Looking for this folder on your accounts' servers, to fetch it from there too.
share-backup-no-account = Brought by your sync app only: none of your accounts has a Nextcloud server (Murena, Nextcloud, ownCloud) to read it on.
share-backup-no-account-for = Brought by your sync app only: none of your accounts is on { $host }.
share-backup-not-found = Brought by your sync app only: this folder was not found on { $hosts }. If it is there, give its place below.
share-backup-seal-differs = Brought by your sync app only: the folder found on { $host } is sealed with another passphrase, so it is never read.
share-backup-seal-gone = Brought by your sync app only: the folder on { $host } no longer holds its seal.
share-backup-login = Brought by your sync app only: { $host } refused the password kept for your account there.
share-backup-no-password = Brought by your sync app only: this device keeps no password for { $account }.
share-backup-unreachable = Brought by your sync app only for now: { $host } could not be reached. Looked for again later.
share-backup-off = Not fetched from { $host }: switched off on this device.
share-backup-off-none = Not fetched from a server: switched off on this device.
share-backup-place = Its place on the server, if Sioul does not find it (“Documents/Sioul”, or its address):
share-backup-look = Look there
# Kept in step by Sioul itself (docs/database.md): no sync app; a folder of your files on a Nextcloud, kept in step by Sioul.
share-mode-folder = Through a folder your sync app carries
share-mode-server = Sioul keeps it in step with your Nextcloud itself
share-server-help = No sync app needed: Sioul keeps its own copy of a folder of your files on the server of one of your contacts-and-calendars accounts (Nextcloud, Murena, ownCloud), and sends and fetches the files itself. Your devices that use a sync app on that folder share with this one as before.
share-server-none = None of your accounts has a Nextcloud server (Murena, Nextcloud, ownCloud): add your contacts-and-calendars account in Accounts first.
share-server-account = Account
share-server-place = Its folder there
share-server-passphrase-hint = Typed twice when the folder is new there; once, as chosen on your other device, when that one shares through it already. At least 12 characters, kept in this device's keyring, never sent anywhere.
share-server-starting = Connecting to the server…
share-on-server = Kept in step with { $host } by Sioul itself: { $place }.
share-server-last = Last in step at { $when }.
share-server-failing = Not in step since { $when }: { $why }. Tried again at the next exchange.
share-server-failing-never = Not in step yet: { $why }. Tried again at the next exchange.
share-server-unconfirmed = The folder on { $host } is no longer sealed as this one: nothing is sent nor fetched. Stop sharing here, then join it again.
share-server-no-place = Give the folder's place there (“Documents/Sioul”).
share-server-no-files = { $host } keeps no files Sioul can reach.
share-server-no-password = This device keeps no password for that account: give it in Accounts first.
share-server-refused = { $host } refused the password kept for that account.
share-server-tls = { $host }'s certificate could not be checked: nothing was sent.
share-server-unreachable = { $host } could not be reached: try again later.
share-server-error = { $host } answered with an error: try again later.
share-server-sealed-meanwhile = Another device started sharing in that folder at the same moment: start again, with the passphrase chosen there.
share-backup-why-quota = its space is full
share-send-switch = Also send this device's changes to { $host } directly
share-send-switch-none = Also send this device's changes to the server directly
share-send-help = Right after each change, Sioul sends this device's own files there itself, beside your sync app: your other devices have them within seconds, even when the sync app is late. Only this device's files, never the others'; the sync app sending them again changes nothing.
share-send-on = Sent directly to { $host }, last at { $when }.
share-send-soon = Found on { $host }: this device's files go there with the next exchange.
share-send-failing = Sent directly to { $host } until { $when }; not since: { $why }. Tried again at the next change.
share-send-failing-never = Not sent to { $host } yet: { $why }. Tried again at the next change.
share-send-waiting = Sent directly once this folder is found on your accounts' servers.
share-send-off = This device's changes go to { $host } by your sync app only: switched off on this device.
share-send-failure = The last try at { $when } did not go: { $detail }
share-send-failure-file = { $file } did not go at { $when }: { $detail }
share-send-file-records = This device's records
share-send-file-entry = This device's entry among your devices
share-send-file-notes = This device's notes to the others
share-send-file-claim = One of this device's claims
share-send-file-texts = This device's texts to send
share-send-file-sealed = A sealed note or paper
share-send-detail-timeout = { $total } did not go through in { $seconds } s: slower than Sioul waits for.
share-send-detail-stalled = nothing moved for half a minute ({ $total }).
share-send-detail-cut = the connection broke on the way ({ $total }).
share-send-detail-out-of-time = the send ran out of time.
share-send-detail-unreachable = the server could not be reached.
share-send-next = Tried again at { $when }.
share-send-again = Send everything again
share-send-again-help = Sends each of this device's files the server lacks or holds otherwise, whatever was sent before, then fetches the others'. For a sync app that was late, or a server that lost files.
share-send-again-sending = Sending this device's files…
share-send-again-sent = { $count ->
    [one] Sent one file to { $host } at { $when }.
   *[other] Sent { $count } files to { $host } at { $when }.
}
share-send-again-same = { $count ->
    [one] Nothing needed sending: { $host } held this device's file already, checked at { $when }.
   *[other] Nothing needed sending: { $host } held this device's { $count } files already, checked at { $when }.
}
share-send-again-off = Nothing was sent: sending to the server is switched off on this device.
share-send-again-not-found = Nothing was sent: this folder is not found on a server of your accounts yet.
share-send-again-no-account = Nothing was sent: none of your accounts is on { $host }.
share-send-again-failed = Could not send to { $host } at { $when }: { $why }.
share-build = This device: Sioul { $build }.
share-device-build = Sioul { $build }.
share-device-build-older = Sioul { $build }, older than this device's ({ $here }): updating it there, when you can, keeps both alike.
share-newer = Your other devices share “{ $part }” in a newer form than this Sioul reads (Sioul { $build } wrote it). Update Sioul on this device to share it again: until then, what you change here in it stays on this device, and what changes on the others waits here.
share-newer-plain = Your other devices share “{ $part }” in a newer form than this Sioul reads. Update Sioul on this device to share it again: until then, what you change here in it stays on this device, and what changes on the others waits here.
share-newer-status = This device needs a newer Sioul to share “{ $part }”: Settings ▸ Your folder and sharing says more.
share-format-held = One of your devices runs an older Sioul, or what it says does not read here (its line below says which): until it is updated there, your devices keep sharing in the form an older Sioul reads. Meanwhile, a time session, your pinned sites or a list of words changed on two devices at the same moment keep only the later change.
share-format-wait = { $file } does not read as it should: your devices keep sharing in the older form until it does. Sioul tries again at each exchange.
share-format-older = One of your devices runs an older Sioul, which does not read what your devices now share (its line below says which): update it there before it shares again.

## Reminders before dates
reminder-event = { $when } · { $what }
reminder-asked = Asked for { $date }
reminder-wait = The wait after “{ $before }” is over.
reminder-payment = Planned for { $date }.
remind-nothing = Nothing to remind in the next two weeks.
remind-told = told
remind-running = Reminders are already watched on this device.
remind-watching = Reminders watched: each one comes once, as a quiet notification. Ctrl+C stops.
reminder-open = Open
# Before an event (docs/reminders.md): "14:00 · Dentist, in 15 minutes".
reminder-before = { $time } · { $what }, { $in }
reminder-in-minutes = in { $minutes ->
        [one] one minute
       *[other] { $minutes } minutes
    }
reminder-in-hours = in { $hours ->
        [one] one hour
       *[other] { $hours } hours
    }
reminder-in-hours-minutes = in { $hours } h { $minutes } min
reminder-in-now = now
reminder-margin = Getting ready, getting there: from { $time }.
reminder-margin-now = Getting ready, getting there: now.
remind-lead = { $minutes ->
        [0] none
        [one] one minute
        [60] 1 hour
        [120] 2 hours
       *[other] { $minutes } minutes
    }
set-reminders-before = Before an event
set-reminders-before-help = A quiet reminder this long before each event, counted before its time to get ready and get there: an event at 14:00 with 30 minutes to get there is reminded at 13:15. Each event may say its own (Remind, in its form). Not for whole days, nor for calendars you only read.
set-reminders-before-none = None
set-reminders-before-exact = On this phone they come on time, Sioul open or not.
set-reminders-before-inexact = On this phone, Android does not let Sioul set exact alarms: these reminders may come up to an hour late. Settings ▸ Apps ▸ Sioul ▸ Alarms & reminders.
event-remind = Remind
event-remind-usual = As usual ({ $lead })
event-remind-none = Not this one
event-remind-before = { $lead } before
event-reminds = Reminder: { $when }
event-reminds-none = No reminder before it
events-channel = Events
# New mail, told at the times it may come (docs/porch.md, "Notifications").
mail-note-title = { $Count } { $n ->
        [one] letter
       *[other] letters
    }
mail-note-opens = The Porch opens: { $count } { $n ->
        [one] letter waits
       *[other] letters wait
    } for you.
mail-note-letter = { $sender }, { $subject }
mail-channel = New mail
set-reminders-mail = Tell me of new mail
set-reminders-mail-help = When mail arrives that its row lets come now, one quiet notification for the batch: how many, and the first senders with their subjects. Mail that waited for its time is told once, when its time comes. When it may come is the Mail rows', below; never for codes (they have a row of their own), nor for what is set aside, blocked, sent by yourself or come to your less important accounts.
set-reminders-mail-newsletters = Include newsletters
set-reminders-mail-newsletters-help = Newsletters and mailing lists, filed on the Porch, are told too. Automatic senders (a bill from no-reply) are told either way.
reminders-closed-unavailable = Not on Windows yet: reminders come while Sioul's window is open.
reminders-closed-no-command = The “sioul” command is not installed next to Sioul: reminders come while the window is open.
set-reminders-group = Reminders
set-reminders-events = Events, the working day before
set-reminders-events-help = Half an hour before work ends, the working day before an event: what, when, where. The alarms an event carries are told at their time too.
set-reminders-asked = Dates asked: working days before
set-reminders-asked-help = When work starts, so many working days before a date asked; 0 for none. One reminder per date, never repeated.
set-reminders-waits = A wait over
set-reminders-waits-help = When a wait after a step done is over (an answer due), once, when work is there.
set-reminders-payment = Payments planned: working days before
set-reminders-payment-help = When work starts, so many working days before a payment planned (a bill, a tax); 0 for none. Debits that leave by themselves are not reminded: the money watch says when the account will not hold them.
set-reminders-closed = With Sioul's window closed
set-reminders-closed-help = Your session starts a small watcher (“sioul remind --watch”) that tells reminders when the window is closed; nothing else runs. Each reminder still comes once.
paper-reminder = { $title }: valid until { $date }
paper-renew-identity = Renewing takes weeks (an appointment, then the making): a step now?
paper-renew-health = It does not renew by itself: ask again before it ends.
paper-renew-home = A new one should come; ask for it if not.
paper-renew-money = A new one should come; ask for it if not.
paper-renew-warranty = Its guarantee ends: anything to report before?
paper-renew-other = It ends soon.
ui-papers = Papers
new-paper = A paper
papers-help = The papers asked again and again, each with its file and how long it holds. Kept in your notes folder (papers/): they travel with it. A reminder comes when one should be renewed.
papers-add = Add a paper
papers-none = No paper yet. Add one from a file (a scan, a photo), or keep an attachment of a mail with “Keep in papers”.
papers-new = A new paper
papers-change = A paper
papers-kind = What
papers-title = Name
papers-title-hint = Passport, Rent receipt September…
papers-file = File
papers-choose = Choose…
papers-issued = Issued on
papers-until = Valid until
papers-until-hint = For a passport, an identity card or a warranty, an end is proposed from the issue when none is given. Rent receipts, payslips and attestations have no end: they are said older than three months.
papers-holder = Whose
papers-holder-hint = In a household: whose paper it is
papers-notes = Notes
papers-remove = Take out
papers-remove-ask = This paper leaves the wallet. Its file stays where it is.
papers-file-missing = Its file is not on this device yet: it may still be syncing.
papers-plan-renewal = Plan the renewal
papers-renewal-planned = Renewal planned
papers-keep = Keep in papers
papers-keep-help = Checked by the antivirus, then kept in the papers wallet: you say what it is.
papers-kept = { $name } is in your papers: say what it is.
papers-attach = A paper
papers-none-to-attach = No paper with a file yet
papers-no-store = Papers live in your notes folder: choose it first (Settings ▸ Your folder and sharing).
papers-no-title = A name, please: “Passport”, “Rent receipt September”.
papers-bad-date = “{ $date }” does not read as a date.
papers-no-file = { $path } cannot be found.
papers-gone = This paper is no longer in the wallet.
papers-list = Papers
paper-valid = valid until { $date }
paper-renew = valid until { $date }: time to renew it
paper-ended = ended on { $date }
paper-fresh = from { $date }
paper-old = from { $date }, older than what is usually asked
paper-renew-task = Renew: { $title }
paper-family-identity = Identity
paper-family-health = Health
paper-family-home = Home
paper-family-money = Money
paper-family-warranty = Warranties
paper-family-other = Other papers
paper-kind-identity = Identity card
paper-kind-passport = Passport
paper-kind-residence = Residence permit
paper-kind-driving = Driving licence
paper-kind-health-card = Health insurance card or attestation
paper-kind-health-cover = Health cover (CSS, mutual insurance)
paper-kind-insurance = Insurance certificate
paper-kind-tax-notice = Tax notice
paper-kind-rent-receipt = Rent receipt
paper-kind-bank-details = Bank details (RIB)
paper-kind-payslip = Payslip
paper-kind-certificate = Attestation, certificate
paper-kind-warranty = Warranty, proof of purchase
paper-kind-other = Other
paper-steps-identity = In France: a pre-request on ants.gouv.fr, then an appointment at a town hall that takes them (any one). Bring the old card, a photo less than six months old, a proof of address.
paper-steps-passport = In France: a pre-request on ants.gouv.fr with a timbre fiscal bought online, then an appointment at a town hall that takes them (any one). Bring the old passport, a photo less than six months old, a proof of address.
paper-steps-residence = In France: the request on the ANEF site (administration-etrangers-en-france.interieur.gouv.fr), from four months before it ends.
paper-steps-driving = In France: on ants.gouv.fr.
paper-steps-health-cover = Ask again on your ameli account (or with the form), two to four months before it ends: it does not renew by itself.
time-budget = Until { $date }: about { $need } of steps, { $room } of room.
time-budget-short = Until { $date }: about { $need } of steps, { $room } of room. More time asked, or a lighter plan?
task-field-energy = What it takes
task-energy-usual = The usual
task-energy-light = Light
task-energy-heavy = Heavy
task-energy-rest = It gives back (a walk, music)
task-field-before = Before: getting there, getting ready
task-field-after = After: coming back
task-margins-hint = Kept free in the plan around it, never counted as a pause.
task-rating-unsaid = —
# What it costs and gives back, as tiles (CostTiles.qml): a name, a gauge from 0 to 10, a word, a question.
tile-cognitive = Thinking
tile-emotional = Feelings
tile-anxiety = Worry
tile-body = Body and senses
tile-gain = Gives back
# Concrete questions, never a category alone: feelings asked through what stirs them, for whoever finds them hard to name.
tile-cognitive-ask = Deciding, many things to hold in mind, something new to learn?
tile-emotional-ask = Talking to someone, being judged or disappointing someone, a painful reminder?
tile-anxiety-ask = A deadline, an answer you wait for, a risk of getting it wrong?
tile-body-ask = Moving, standing, noise, crowds, screens, travel?
tile-gain-ask = Will it give you anything back: rest, joy, meaning?
tile-gain-felt-ask = Did it give you anything back: rest, joy, meaning?
# The gauge's words, after Borg's CR10: a word at fixed numbers, a number between two takes the word below it (docs/capacity.md, "The form").
tile-cost-0 = nothing
tile-cost-1 = very light
tile-cost-2 = light
tile-cost-3 = moderate
tile-cost-5 = hard
tile-cost-7 = very hard
tile-cost-10 = the most
tile-gain-0 = nothing
tile-gain-1 = a little
tile-gain-3 = some
tile-gain-5 = a good deal
tile-gain-7 = a lot
tile-gain-10 = the most
tile-unsaid = unsaid
# What a screen reader says of a tile: "Thinking: 3, moderate", "Thinking: unsaid".
tile-said = { $name }: { $value }, { $word }
tile-said-none = { $name }: unsaid
tile-proposed-value = Faint: { $value }, from how it felt before.
tile-keys = A tap on a cell gives its value; the same tap again clears it. Keys: 0 to 9, + or = for 10, the arrows, Delete to clear.
# Faint values from how it felt before (FeltIndex::proposal); "Looks right" takes them all.
tile-looks-right = Looks right
tile-proposed-item = Faint: how this task felt the last times. A tap on a tile sets that one alone.
tile-proposed-kind = Faint: how tasks of this kind felt. A tap on a tile sets that one alone.
tile-proposed-both = Faint: how this task and tasks of its kind felt. A tap on a tile sets that one alone.
# What was foreseen, said beside a felt rating ("How was it?").
rating-foreseen = foreseen: { $value }
# What it takes, once a cost is rated: computed, not chosen ("Heavy, from the ratings").
task-energy-computed = { $level }, from the ratings
# A task's details, then its form.
ui-details = Details
# After a task is done, if you want to say (FeltRatings.qml): never asked twice, never counted.
felt-ask = How was it?
felt-hint = The thin marks are what you foresaw. Tap what you want to say; the rest stays blank.
felt-kept = Kept as you felt it, beside what you foresaw.
# "What makes it hard?": dread or boredom, two minutes offered; nothing starts until you say.
task-hard-two = Two minutes are enough to begin.
# A new task's form: made once its title is given.
ui-new-task = New task
task-new-title = Title of the new task
task-new-made-when = Made once it has a title; each field is then kept as you go.
task-new-tied = Tied to: { $title }
task-why-heavy-fog = A heavy one, on a foggy day: nothing lighter is free. Its first two minutes may be enough.
task-rest-offer = After something heavy, something that gives back:
task-mode-day = The day
day-all-day = All day: { $what }
day-margin = Around: { $what }
overlap-today = Today, two events at once: { $first } and { $second }. The time to get there and back counts.
overlap-day = { $day }, two events at once: { $first } and { $second }.
overlap-open = Open “{ $title }”
overlap-set-aside = Don’t mention it again
day-more = { $count ->
    [one] One more step of today does not fit before the end of the day: it keeps its place in the plan.
   *[other] { $count } more steps of today do not fit before the end of the day: they keep their place in the plan.
}
day-empty = Nothing laid out today: no event, no step the plan gives today.
day-now = now
# Moving by dragging: the Health page's timeline, the agenda's day and week, the Tasks page's day.
drag-done = { $what }: { $time }.
drag-done-plain = Moved.
day-let-plan = Let the plan place it
day-let-plan-done = The plan places “{ $title }” again.
agenda-read-only = This calendar can only be read.
agenda-move-which = Moved to { $time }: only this time, or every time?
agenda-move-set-days = Its rule sets its days: move only this time, or change how it repeats in its form.
health-readable-done = Done
health-readable-help = Hold a meal, a rest or the night to pick it up, then slide it. By one of its edges, only that end changes.
routine-admin = The admin window
routine-admin-porch = The Porch: what came
routine-admin-next = The next step: { $title }
routine-admin-stop = Where you stopped, in a line
routine-no-title = A name, please.
routine-no-steps = One step a line, with its minutes: “10 min Open the Porch”.
routines = Routines
routines-what = A routine is a sequence you go through often, in the same order: getting ready to go out, starting the work day, your admin hours. You write its steps once, each with its minutes; Sioul then plays them one at a time on a timer and says the next step before it comes, so the order and the clock are not yours to keep in mind. It is not a task: nothing is planned, counted or late, and you can stop at any step.
routines-help = One step a line, with its minutes: “10 min Open the Porch”, “Make tea 5”.
routines-tip = Steps you go through often, played one at a time on a timer
routines-new = A new routine
routines-play = Play
routines-change = Change
routines-remove = Take out
routines-title = Name
routines-steps = Steps
routines-auto = The next step starts by itself when the time is up
routines-builtin = Made by Sioul for your admin hours, from what is there now: the Porch, the plan’s next step, then a line to say where you stopped.
routine-next = Then: { $title }
routine-last = The last step.
routine-left = { $minutes } min left
routine-over = Its time is up: done, or a few more minutes?
routine-done = Done
routine-skip = Skip
routine-more = +5 min
routine-pause = Pause
routine-resume = Go on
routine-stop = Stop
routine-open = Open
routine-finished = That was all of it.
contract-reminder = { $title } renews on { $date }
contract-reminder-notice = To stop it, the notice must leave by { $by }.
contract-reminder-free = It can be stopped until then.
contracts = Contracts and subscriptions
contracts-add = Add a contract
contracts-none = No contract noted yet. Rent, energy, phone, insurances, subscriptions: each with when it renews and how to stop it.
contracts-new = A new contract
contracts-change = A contract
contracts-title-hint = Home insurance, Electricity, Phone…
contracts-party = With
contracts-reference = Your number with them
contracts-preset = Paid by
contracts-started = Since
contracts-renews = Renews on
contracts-every-none = No renewal
contracts-every-month = each month
contracts-every-year = each year
contracts-notice = Notice
contracts-days = days
contracts-cancel = To stop it
contracts-cancel-hint = Its cancel page (https://…), or where to write
contracts-covers = What it covers
contracts-covers-hint = Liability, legal protection, glass breakage…
contracts-covers-line = Covers: { $covers }
contracts-ended = Ended on
contracts-remove-ask = This contract leaves the list. Nothing is sent to anyone.
contracts-stop = Stop it…
contracts-stop-page = Open its cancel page
contracts-stop-letter = Write the letter
contracts-stop-ended = It has ended
contracts-suggestions = Recurring payments with no contract noted:
contracts-note-it = Note it
contracts-keep = Keep as a contract…
contract-cost-month = { $amount } a month
contract-cost-year = { $amount } a year
contract-ended = Ended on { $date }.
contract-renews-notice = Renews on { $date }; to stop it, the notice must leave by { $by }.
contract-renews-late = Renews on { $date }; the notice for this renewal can no longer leave in time.
contract-renews = Renews on { $date }; it can be stopped until then.
contract-open-notice = No renewal date; to stop it, { $days } days' notice.
contract-open = No renewal date; it can be stopped at any time.
contract-letter-reference = No. { $reference }
contract-letter-subject = Cancellation: { $title } { $reference }
contract-letter-body = Dear Sir or Madam,

    I hereby ask you to end my contract { $title } { $reference } at the earliest date allowed.

    Please confirm in writing that it has ended, and on which date.

    Yours faithfully,
contract-kind-rent = Rent
contract-kind-energy = Energy
contract-kind-telecom = Phone, internet
contract-kind-insurance = Insurance
contract-kind-health = Health cover
contract-kind-subscription = Subscription
contract-kind-hosting = Hosting, domain
contract-kind-bank = Bank
contract-kind-other = Other
contract-rule-rent = In France a tenant gives three months' notice, one month for a furnished flat or in a tight area; your lease says which.
contract-rule-energy = In France electricity and gas can be changed or stopped at any time, without fees.
contract-rule-telecom = In France, after the first twelve months of commitment, phone and internet can be stopped at any time, with at most ten days' notice.
contract-rule-insurance = In France, after its first year, a home, car or affinity insurance can be stopped at any time; it ends one month later (loi Hamon). Before that, as the contract says, usually two months before its anniversary.
contract-rule-health = In France, after its first year, a health cover can be stopped at any time; it ends one month later.
contract-rule-subscription = Online, a subscription is stopped as easily as it was taken: in France a cancel button, in three clicks, since June 2023.
contract-rule-hosting = As the provider's terms say; a domain not renewed is lost after its grace period.
contract-rule-bank = A current account can be closed at any time; a new bank can do it for you (bank mobility).
contract-rule-other = As the contract says.
reminder-payment-held = Planned for { $date }; the account will hold it.
reminder-payment-short = Planned for { $date }; the account will be { $short } short then.
money-missed-out = { $label } ({ $amount }) has not left the account
money-missed-in = { $label } ({ $amount }) has not come in
money-missed-why = Expected around { $date }, not in your bank's movements. A debit that stops does not say so: worth a look.
money-short = The account may not hold { $label } on { $date }
money-short-reserve = { $short } will be missing then; { $reserve } covers it.
money-short-ask = { $short } will be missing then. Moving money in before, or asking for a later date?
bank = The bank
bank-import = Take in an export…
bank-files = Bank exports
bank-balance = { $amount } on { $date }, from your bank's file.
bank-coming = This week:
bank-forecast = The month ahead, with what is expected:
bank-accounts = Bank accounts
bank-accounts-help = Where your money actually is: a current account, PayPal, Stripe. Each takes its own exports, fills the budgets you choose, and is topped up by your reserves in your order. Until one is declared, an export taken in only feeds the watch below. Kept in your notes folder (sioul-budgets.toml, sioul-bank.toml); nothing is sent anywhere.
bank-account-new = New bank account
bank-account-edit = Bank account
bank-account-title = Name
bank-account-title-hint = My bank, PayPal…
bank-account-kind = Kind
bank-kind-bank = A bank
bank-kind-paypal = PayPal
bank-kind-stripe = Stripe
bank-kind-other = Something else
bank-account-fills = Fills
bank-account-fills-help = Its movements go to these budgets, by your rules and the recurring payments they stand for; the first takes the rest.
bank-account-first = the rest
bank-account-earlier = Earlier in the order
bank-account-floor = Kept above
bank-account-topped-by = Topped up by
bank-account-remove = Take it out
bank-account-remove-yes = Take it out for good
bank-account-remove-ask = Its movements stay in sioul-bank.toml and its rules in the budget file; its budgets no longer count them.
bank-account-no-export = No export taken in yet: "Take in an export…".
bank-account-two-in-one = Two of its exports give different balances on the same day: they seem to come from two accounts, and only the newest balance is shown. To count both, make a new bank account for the second one and take its exports in there.
bank-account-fills-line = Fills { $budgets }.
bank-account-fills-none = It fills no budget yet: its movements count nowhere. "Edit" to choose.
bank-account-topped = Topped up by { $reserves }.
bank-account-topped-floor = Kept above { $floor }, topped up by { $reserves }.
bank-rules = Rules…
bank-rules-title = Where the movements of { $account } go
bank-rules-help = A movement whose label holds one of a rule's words goes where the rule says; a rule without words takes every movement its way (all money out to one budget). The first rule that holds wins. Words: separated by commas, case and accents aside. Money moved between your own accounts is known first, when both sides' exports are read. Without a rule, a movement goes with the recurring payment it stands for, else to the account's first budget.
bank-rules-none = No rule yet.
bank-rule-words = Words in the label, separated by commas
bank-rule-direction-any = In or out
bank-rule-direction-debit = Money out
bank-rule-direction-credit = Money in
bank-rule-to-reserve = With { $reserve }
bank-rule-everywhere = every account
bank-rule-here = this account
bank-movements = { $count ->
    [one] Its last movement
   *[other] Its last { $count } movements
}
bank-movement-auto = As the rules say
bank-movement-where = Where it goes
bank-place-reserve = With { $reserve }, through { $budget }
bank-place-transfer = Moved with { $account }
bank-place-known = Counted from mail ({ $label })
bank-place-none = In no budget
bank-why-hand = your choice
bank-why-mail = the same payment as a line from mail
bank-why-rule = by a rule
bank-why-between = between your accounts
bank-why-preset = a recurring payment
bank-why-first = its first budget
bank-why-unknown = nothing says where
bank-unplaced = { $count ->
    [one] One movement goes to no budget: choose the budgets this account fills ("Edit").
   *[other] { $count } movements go to no budget: choose the budgets this account fills ("Edit").
}
money-topup-now = { $reserve }: { $amount } to move before { $date }, when { $payment } leaves; it arrives at once.
money-topup-ask = { $reserve }: ask for { $amount } by { $ask }; it takes about { $days } days, and { $payment } leaves on { $date }.
money-topup-late = { $reserve }: ask for { $amount } today; it takes about { $days } days, so it arrives after { $payment } ({ $date }).
reserve-new = New reserve
reserve-edit = Reserve
reserve-title = Name
reserve-title-hint = Livret A, assurance vie…
reserve-balance = Balance
reserve-as-of = On
reserve-as-of-hint = 2026-10-01; today when empty
reserve-floor = Never below
reserve-delay = Days to arrive
reserve-delay-help = How long money asked from it takes to reach your account: 0 for a Livret A, about ten for an assurance vie. Top-ups are asked that much ahead.
reserve-delay-short = { $days ->
    [one] a day
   *[other] { $days } days
}
reserve-at-once = at once
bank-imported = { $read ->
    [one] One movement read
   *[other] { $read } movements read
}, { $new ->
    [0] none new.
    [one] one new.
   *[other] { $new } new.
}
bank-unreadable = { $file } does not read as a bank export (OFX, CAMT.053 or CSV).
money-changed = { $label } took { $actual } on { $date }, where { $expected } was expected.
bank-week-held = This week: { $list }. The account holds them.
bank-week-short = This week: { $list }. The account may not hold them all.
bank-attention = { $count ->
    [one] Something about money to look at.
   *[other] Some things about money to look at.
}
letters = Paper letters
letters-no-ocr = Scans wait in the letters folder, unread: no program here reads them. To read them: { $hint }
letters-unread = Its text could not be read ({ $problem }); its scan is here.
letters-scan = See the scan
letters-task = A task for that date
letters-task-made = Its task
letters-event = Into the agenda
letters-no-project = No project
letters-done = Done, filed
letter-amount = Asks for { $amount }.
letter-deadline = By { $date } ("{ $why }").
letter-appointment = An appointment: { $when }.
letter-registered = Sent registered, with acknowledgment of receipt.
letter-dated = Dated { $date }.
letter-reference = Your number: { $reference }.
letter-unknown-sender = A letter
letter-scan = The scan
letter-event = Appointment: { $sender }
letter-kind-formal-notice = a formal notice
letter-kind-decision = a decision
letter-kind-tax-notice = a tax notice
letter-kind-reminder = a reminder
letter-kind-bill = a bill
letter-kind-appointment = an appointment
letter-kind-acknowledgment = an acknowledgment
letter-kind-attestation = an attestation
letter-kind-contract = a contract
letter-kind-other = a letter
letter-task-formal-notice = Answer { $sender }'s formal notice { $amount }
letter-task-decision = { $sender }'s decision: contest it or not
letter-task-tax-notice = Tax notice: pay or check { $amount }
letter-task-reminder = { $sender }: pay the reminder { $amount }
letter-task-bill = Pay { $sender } { $amount }
letter-task-appointment = Prepare the appointment with { $sender }
letter-task-acknowledgment = { $sender }: check what they received
letter-task-attestation = File { $sender }'s attestation
letter-task-contract = Read { $sender }'s contract
letter-task-other = Answer { $sender }
set-letters-group = Paper letters
set-letters-inbox = Where scans arrive
set-letters-inbox-help = A folder where you, a scanner or a helper puts the post as PDF or photos (a phone's scanner app syncing there works too). Each scan is read by Sioul on a computer (Tesseract, Poppler; a phone does not read them) and waits for the Porch's window as a card: who, what, how much, by when.
set-calls-group = Calls
set-porch-calls = Calls your phones declined
set-porch-calls-help = Listed on the Porch of each of your devices, each at a time its caller may reach you, never counted. Off, each phone lists only its own, and your computers none.
password-show = Show the password
password-hide = Hide the password
set-passwords-shown = Show passwords as you type
set-passwords-shown-help = Every password, passphrase and key field shows what you type from the start, on this device; the eye at the end of each field shows or hides it at any time.
set-places-named = Show the places' names beside their icons
set-places-named-help = The places on the left of the window show their names beside their icons, in a wider column: some people read words more easily than icons. Without it, their icons alone, and each one's name when the pointer rests on it, or at a long press on a touch screen.
# colour: the sites in the screen's own colours, and calmer colours (docs/colour.md).
set-screen-colours = Colours for this screen
set-screen-colours-help = Sites are shown in this screen's own colours, read from its colour profile, so that a wide-gamut screen does not make them louder than they are.
set-calmer-colours = Calmer colours on sites
set-calmer-colours-help = Loud colours on websites are softened. Greys and soft tints stay as they are.
set-calmer-colours-off = Off
set-calmer-colours-little = A little
set-calmer-colours-more = More
colour-said-converted = This screen's profile is used: “{ $profile }”.
colour-said-converted-unnamed = This screen's profile is used.
colour-said-desktop = Your desktop already adapts colours to the screen: nothing more to do.
colour-said-plain = This screen has no profile: colours are shown as they are.
bitwarden-factor-7 = Security key (YubiKey, FIDO2)
bitwarden-code-7 = Second step: your security key.
bitwarden-key-use = Use the security key
bitwarden-key-failed = The security key was not used: { $error }
bitwarden-code-sent = The code was sent to your e-mail address.
bitwarden-code-again = Send the code again
bitwarden-code-refused = That code was not taken: try again, or choose another step.
bitwarden-no-key = Bitwarden offers no security key for your account. A YubiKey shows here when Bitwarden knows it as a security key (FIDO2 WebAuthn, open to every account), or as "YubiKey OTP" while the account has Premium: Bitwarden hides that one from every app without it. See two-step login in your account's security settings, in Bitwarden's web vault. Codes kept on the YubiKey by Yubico Authenticator go in "Authenticator app".
bitwarden-passkey = Open with my security key
bitwarden-passkey-help = Your key and its PIN, without the master password, when Bitwarden knows the key as a passkey used for encryption.
bitwarden-password-or = Or your master password:
bitwarden-passkey-no-secret = Your key answered, but without the secret that opens the vault (WebAuthn PRF): use your master password.
bitwarden-passkey-no-vault = Bitwarden logs you in with this passkey but does not open the vault with it. In Bitwarden's web vault, Settings ▸ Security ▸ Master password, under "Log in with passkey", choose "Set up encryption" next to it. Until then, use your master password.
bitwarden-passkey-unknown = Bitwarden does not know this key as a passkey of an account. Add it in Bitwarden's web vault, Settings ▸ Security ▸ Master password, under "Log in with passkey"; or use your master password.
bitwarden-passkey-other = This passkey is for another Bitwarden account than the one set in this page's settings (⚙): the vault was not opened.
bitwarden-passkey-mismatch = Your key's secret did not open the vault: use your master password.
bitwarden-key-waiting = Touch your security key when it blinks; plug it in first if it is not.
bitwarden-key-waiting-passkey = Your key is asked: plug it in if it is not, type its PIN when Sioul asks, then touch it when it blinks.
bitwarden-key-stop = Stop
bitwarden-key-not-allowed = The key was not used in time, or the request was cancelled.
bitwarden-choose = Choose a login…
bitwarden-choose-title = Which login?
bitwarden-choose-by-site = Site
bitwarden-choose-by-site-hint = A domain or a word
bitwarden-choose-by-user = User name
bitwarden-choose-by-user-hint = Any part of it
bitwarden-choose-empty = Empty this field
bitwarden-choose-ask = Type a site, a user name, or both.
bitwarden-choose-nothing = Nothing found.
bitwarden-choose-nothing-both = No login has both: empty one of the fields to see more.
bitwarden-choose-more = { $count ->
    [one] One more login: narrow the search.
   *[other] { $count } more logins: narrow the search.
}
bitwarden-choose-other-site = for { $site }
bitwarden-choose-fill = Fill
task-office-open = Open from
task-office-to = to
task-office-and = and
task-office-usual = Offices' usual hours, until these are changed.
task-field-area = For
task-area-tags = As its tags say
health-missed = While Sioul was closed
health-missed-body = Did you take { $doses }?
health-missed-open = Answer
health-missed-question = Due while Sioul was closed, and marked nowhere Sioul can see: did you take them?
health-missed-some = Doses due earlier
health-unshown = A reminder could not be shown
health-unshown-question = Due while Sioul ran, but their reminder could not be shown (notifications did not work then), and marked nowhere Sioul can see: did you take them?
health-not-taken = Not taken
health-taken-when = Taken…
dose-taken-title = Taken late
dose-taken-due = { $name }, due at { $due }.
dose-taken-when = Taken at
dose-next-after = The next dose comes { $hours } hours after the time you took this one: the hours between two doses are kept.
dose-time-wrong = A time, as 09:30.
dose-doubt = Sioul can't tell whether it was taken: { $why }. Check before taking it.
dose-doubt-now = Doses marked on your other devices may not show here: { $why }.
dose-doubt-record = this device's record of doses could not be read { $when }
dose-doubt-never = { $name } never said how far its record goes (an older Sioul there?)
share-other-device = another device of yours
dose-doubt-closed = { $name } closed { $when }, and its news can be slow to come
dose-doubt-open = { $name } was last heard { $when }
dose-doubt-broken = part of what { $name } wrote could not be read
dose-doubt-working = { $name } was in use and last shared { $when }
dose-doubt-quiet = { $name } was in use when it last shared, { $when }, and has said nothing since: it may have stopped without closing
dose-doubt-coming = what { $name } shared { $when } has not all come here yet
dose-doubt-apart = { $name } does not share its doses (Health is switched off there)
device-the-phone = the phone
when-at = at { $time }
device-off = This device is off
device-off-named = { $name } is off
dose-off-note = { $name } counts as off, as you said: a dose marked there would not show here until it shares again.
dose-answers = Marked { $answers }. Check which is right before taking it.
dose-answer-taken-here = taken here { $when }
dose-answer-taken-on = taken on { $name } { $when }
dose-answer-taken = taken { $when }
dose-answer-skipped-here = skipped here { $when }
dose-answer-skipped-on = skipped on { $name } { $when }
dose-answer-skipped = skipped { $when }
dose-answer-other-on = answered on { $name } { $when }, in words this Sioul does not know
dose-check-title = Check first: { $dose }
dose-alarm-fallback = A dose is due: open Sioul to check it.
dose-alarm-late = Taken late: tap to say when.
dose-notifications-off = Android does not show Sioul's notifications, or its "Doses" channel is off: dose reminders cannot show on this phone.
dose-alarms-inexact = Android does not let Sioul set exact alarms: reminders may come late, up to an hour. Settings ▸ Apps ▸ Sioul ▸ Alarms & reminders.
dose-record-broken = The record of doses can't be written. Sioul kept the broken one aside and rebuilds it from your other devices; note what you take elsewhere meanwhile.
need-meal-0 = Breakfast
need-meal-1 = Lunch
need-meal-2 = Dinner
need-meal-n = Meal { $n }
need-nap = Nap
need-sleep = Winding down
need-heads-up = No new big task
need-at = { $name } at { $time }
need-later = Later
need-open = Options…
need-later-n = { $minutes } min later
need-move-to = Move to
need-move = Move
need-unskip = Today after all
stopped-note = Where I stopped…
new-stopped = Where I stopped…
need-gap = More than four hours between { $from } and { $to }.
needs-title = Meals, rest and sleep
needs-help = Times kept free: no task is planned in them. A notice comes before, about the work, then one at the time; nothing else is said, and nothing is recorded.
needs-meals = Meals
needs-naps = Naps
needs-sleep = The night
needs-add-meal = Add a meal or a snack
needs-add-nap = Add a nap
needs-at = At
needs-eat = Minutes to eat
needs-prep = Minutes to get it ready
needs-nap-minutes = Minutes
needs-after = Minutes to come back
needs-bed = Bedtime
needs-wake = Waking
needs-wind-down = Minutes to wind down
needs-notices = Notices
needs-not-today = Not today
needs-heads-up = Notice before, in minutes
needs-later-by = “Later” moves it by, in minutes
needs-remove = Remove
needs-name-hint = Name, as you like
# The Health page's day and week (HealthPage.qml): each day's own meals, naps and nights.
need-meal-detail = eating from { $at }
need-nap-detail = then { $minutes } min to come back
need-night-detail = winding down, bed at { $bed }
need-changed = changed for this day
need-added = this day only
need-pushed = moved after an event
need-quiet = no notice that day; still kept free
need-off = removed from this day
need-added-meal = Meal
need-added-nap = Rest
need-change-times = Change its times…
need-not-that-day = Not that day
need-unskip-day = That day after all
need-as-usual = Back to usual
need-remove-today = Remove from today
need-remove-day = Remove from that day
need-put-back = Put back
need-add = Add a meal or a rest…
need-add-title = A meal or a rest, { $day }
need-kind-meal = A meal
need-kind-nap = A rest
need-form-from = From
need-form-to = To
need-form-help = For this day only: the usual times stay as they are, in this page's settings.
need-menu = More for { $name }
need-past = That day has passed: it stays as it was.
need-times-wrong = Two times, as 12:30, the end after the start.
need-too-short = A few minutes at least.
need-other-day = That would move it to another day.
health-day-empty = Meals, rest and sleep are set in this page's settings (⚙).
health-day-nothing = Nothing set for this day.
health-errand-day = { $what } (in your tasks from this day)
health-alone = These doses are known to this device only. If Sioul also runs on another device, share between them (Settings ▸ Your folder and sharing): a dose marked on one then counts on all, and only the one you are using reminds you.
health-unchecked = Your other devices were last heard from at { $time }: a dose marked there may not show here yet.
health-unchecked-yet = Your other devices are not heard from yet: a dose marked there may not show here yet.
health-reminded-there = Reminders come on { $computer }, the device you are using.
invoice-unchecked = Your sharing folder cannot be written now: your other devices would not know of this invoice. Try again once it is reachable.
invoice-elsewhere = Invoices are numbered on { $computer }: one device at a time, so that a number is never given twice.
invoice-take = Make invoices on this device
invoice-settling = In a minute and a half: your other devices first learn that invoices are made here.
invoice-others-silent = Another device has not been heard from for a few minutes: its sync may be late. Invoices wait until it is heard from, so that no number is given twice.
invoice-here = Invoices are made on this device from now on.
site-kind-video = Video calls
site-area = For
area-admin = Your admin
area-leisure = Leisure
area-mixed = Work and admin
site-move-up = Move up
site-move-down = Move down
site-link = Link to…
site-sort-category = Grouped by type
site-sort-own = In your order
site-presets-find = Find a site: a bank, an office, a chat…
site-presets-everyone = For everyone: chats, calls, dating
site-presets-whole-country = The whole country
site-presets-all = All kinds
site-presets-kept = already here
site-presets-calls = calls
site-presets-none = None here: add it by hand below.
site-by-hand = Or by hand:
site-later = Other hours: { $count }
link-kind-site = Sites
mode-admin = Admin time until { $until }: offices, bills, letters.
mode-leisure = Leisure until { $until }: what you enjoy.
set-windows-admin = Hours for your admin
set-windows-admin-help = Your own admin comes forward then: offices, bills, letters, health errands. Once these are set, admin no longer comes in working hours, except calls to an office, which keep office hours.
set-reminders-gather = Sites' notifications gathered
set-reminders-gather-help = What your sites notify waits, then comes in one notification at the gathered times, for the sites of those hours. A site in real time, and a call, come at once, at the times their rows let them.
set-reminders-gathered = Gathered at
set-reminders-gathered-help = Times of the day, as 09:00: three a day helped most in a field trial. On a phone, other apps' notifications from automatons come back at these times too.
sites-gathered = Your sites have news
sites-gathered-open = Open the Porch
site-microphone = Microphone, for calls
site-camera = Camera, for calls
site-screen = Sharing the screen, for calls
site-devices = Devices for calls
site-devices-help = The camera, microphone and speaker of calls in every site, from your system's list. A change here applies at once, to calls in progress too. The system's own follows your system's settings. During a call, a device chosen in the site's own menu is kept for that call.
site-device-camera = Camera
site-device-microphone = Microphone
site-device-speaker = Speaker
site-device-system = The system's own
site-call-devices = Camera, microphone and speaker of calls
site-device-kept-camera = { $site } keeps its camera: { $device } could not be opened. It may be in use by another program, or unplugged.
site-device-kept-microphone = { $site } keeps its microphone: { $device } could not be opened. It may be in use by another program, or unplugged.
site-device-kept-speaker = { $site } keeps its speaker: { $device } could not be used. It may be unplugged.
site-permission-off-microphone = { $site } asked for the microphone: it is off for this site (⋮ ▸ Microphone).
site-permission-off-camera = { $site } asked for the camera: it is off for this site (⋮ ▸ Camera).
site-permission-off-screen = { $site } asked to share the screen: it is off for this site (⋮ ▸ Sharing the screen).
site-kind-social = Social networks
site-filter = Which sites
site-field-categories = Categories
site-categories-help = Bank, Health…, with commas
folder-new-button = New folder
folder-new-title = New folder in { $account }
folder-new-make = Make it
collection-here = on this device only
health-move = Time to move
health-move-body = Stand up, stretch, look far away for a minute.
site-presets = Usual sites
site-presets-pin-all = All of them ({ $count })
site-presets-by-region = By region
site-pinned = Sites pinned: { $count }
site-filter-clear = Show every site
site-filter-any-area = For anything
site-filter-any-kind = Any type
site-filter-any-category = Any category
site-filter-for = What it is for
site-filter-type = Type
site-filter-category = Category

## The alarm at waking, rung by a phone (crates/sioul-app/src/wake.rs); Java says the first five, given ahead.
wake-channel = Waking
wake-ringing = Waking
wake-stop = Stop
wake-later = { $minutes } min later
wake-again = Rings again at { $time }
wake-title = Wake-up alarm
wake-help = Rings on your phone (Sioul for Android) when the night ends, on the mornings ticked; a night changed on the page rings at its own waking. Android's do-not-disturb lets alarms through.
wake-next = Next: { $when }.
wake-exact-off = Android does not let Sioul ring on time: the alarm cannot ring until “Alarms & reminders” is allowed.
wake-allow-exact = Allow alarms…
wake-screen-off = Android may not light the screen when it rings; it rings all the same.
wake-allow-screen = Allow the full screen…
wake-notifications-off = Sioul's notifications are off in Android: the alarm needs them to show Stop.
wake-allow-notifications = Turn them on…
wake-at = alarm at { $time }
wake-none = no alarm
wake-skip = No alarm at { $time }
wake-unskip = Alarm at { $time } after all
wake-try = Try the alarm
wake-try-rings = Rings in about ten seconds, as a waking would; nothing else changes.
wake-try-screen-off = Android does not let Sioul show the alarm over the lock screen: allow the full screen to try it.
wake-try-phone = The alarm rings on a phone only.
wake-try-failed = Android did not answer: nothing was set. Try again in a moment.

## How long a reserve lasts at this pace, by its number of months (0: at its floor already).
fig-pace-months = { $months ->
    [0] at its floor already
    [one] about a month
   *[other] about { $months } months
}
reserve-lasts-months = { $months ->
    [0] At this pace it is at its floor already.
    [one] At this pace it lasts about a month.
   *[other] At this pace it lasts about { $months } months.
}
attachment-program = { $name } is a program, a script or an installer: Sioul does not start those from a mail, even checked. Save it if you trust it.
budget-origin-mail = From mail: “{ $subject }” from { $sender }, added by Sioul on { $date }.
budget-origin-hand = Added by hand in Sioul on { $date }.
note-outside-notes = { $path } is not in your notes folder.
setting-unknown-key = Sioul has no setting named “{ $key }”.
reminders-entry-name = Sioul reminders
reminders-entry-comment = Reminders before dates, with Sioul's window closed
bank-rule-gone = This rule is no longer there.
budget-line-gone = This line is no longer in the budgets' file as shown here, so nothing was changed. It may have changed meanwhile, on another device or by hand.
mail-no-such-folder = { $account } has no folder named “{ $folder }”; sioul mail folders { $account } lists them.
mail-not-in-accounts = { $file } is not in the mail of one of your accounts.
mail-move-needs-to = Which folder? Name it with --to.
list-read-only-mark = (read only)
import-bad-key = “{ $key }” cannot be a key: keys name files, so no “/”, “\” or “:” in them.
antivirus-hint-windows = Windows Security ▸ Virus & threat protection
antivirus-hint-packages = ClamAV, from your system's packages
antivirus-hint-phone = open it on a computer that has one
ui-attachments-phone = A phone has no antivirus Sioul can call, so attachments are not checked here. Each opens in the app you choose, which can read that file and nothing else of Sioul's. Programs and installers never open from a mail.
ui-save-as = Save…
papers-keep-help-phone = Kept in the papers wallet without a check: a phone has no antivirus Sioul can call. You say what it is.
attachment-phone-opened = { $name } opens in the app you choose. It was not checked: a phone has no antivirus Sioul can call.
attachment-phone-no-app = No app on this phone opens { $name }. You can save it with your files.
attachment-phone-not-opened = { $name } could not be handed to another app ({ $detail }).
attachment-phone-where = Choose where to save { $name }.
attachment-phone-saved = { $name } is saved. It was not checked: a phone has no antivirus Sioul can call.
attachment-phone-not-saved = { $name } was not saved.
attachment-phone-save-failed = { $name } could not be saved ({ $detail }).
ocr-hint-windows = Tesseract (github.com/UB-Mannheim/tesseract) and Poppler
ocr-hint-packages = Tesseract and Poppler, from your system's packages
google-page-granted = Sioul has the access. You can close this tab.
google-page-denied = Google gave no access: nothing changed. You can close this tab.
google-page-foreign = This answer was not for Sioul. You can close this tab.
google-page-granted-phone = Sioul has the access. Go back to Sioul: it finishes there.
ui-all-files = All files
## Forms, notes and memos: amounts that are no number, buttons, links, the microphone.
amount-unreadable = “{ $text }” is not an amount: write it in digits, as 1200 or -650.50.
note-make = Make the note
note-folder-make = Make the folder
ui-rename = Rename
audio-cannot-play = This sound cannot be played here ({ $why }).
note-memo-failed = The memo could not be recorded ({ $why }).
note-memo-no-microphone = The system keeps the microphone closed to Sioul: it is opened in the system's privacy settings.
note-link-not-found = “{ $path }” is not in your notes folder.
note-changed-elsewhere = This note changed elsewhere while you were writing: yours is kept beside it, as “{ $path }”.
note-newer-came = A newer version of this note came from another device. What you are typing stays as it is: saving keeps it beside that version.
note-link-kept = Links like this one are not opened from a note: { $url }
right-now-unverified = Its sender is not verified: use it only if you just asked this site for it.
link-program = A program, a script or an installer: its folder opens instead, to start it from there if you trust it.
task-spent-time = { $minutes ->
        [one] { $minutes } minute so far
       *[other] { $minutes } minutes so far
    }
task-session-time = { $date }: { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }
hours-range-add = Another range of hours this day
hours-range-remove = Take this range of hours away
site-news-line = { $title }: { $text }

## What comes when: five times, four lists, do not disturb (docs/areas.md, docs/porch.md)

mode-meal = Meal until { $until }.
mode-sleep = Sleep: nothing disturbs until { $until }.
mode-wind-down = Winding down: nothing disturbs until { $until }.
mode-nap = Nap: nothing disturbs until { $until }.
quiet-meal-title = Mealtime
set-hours-leisure = Meals and sleep, on the Health page
set-hours-leisure-help = Leisure is every other time: neither work nor your admin. Meals and sleep are set on the Health page: a meal from getting it ready to its end, the night from winding down to waking, naps. While you sleep, nothing disturbs.
reach-word-work = work
reach-word-admin = admin
reach-word-leisure = leisure
reach-word-meals = meals
reach-word-sleep = sleep
reach-any = any time
reach-never = never
sender-list-times = { $list }: { $times }
sender-list-safe = Safe
sender-list-neutral = Neutral
sender-list-restricted = Restricted
sender-list-blocked = Blocked
sender-one-safe = Safe
sender-one-neutral = Neutral
sender-one-restricted = Restricted
sender-one-blocked = Blocked
sender-categories = As their categories say
sender-no-list = No list
sender-from-address = { $list }: their own choice.
sender-from-category = { $list }, as the category { $name } says.
sender-from-domain = { $list }, as { $name } says.
sender-from-default = { $list }: in none of your address books, on no list.
sender-now-restricted = { $entry } is restricted: their mail comes only as the Restricted row says, in What reaches you.
sender-now-categories = { $entry }: as their categories say.
set-restricted = Restricted
set-restricted-help = Those you would rather hear from only at chosen times: a demanding client, someone whose mail weighs. They reach you only as the Restricted row above says. An address, a number, or a pattern with *.
set-sender-categories-group = Your contacts' categories
set-sender-categories-note = A person's own choice comes first (their address or number, then their card), then their card's categories, then a domain; anyone else in your address books is neutral, and everyone else a stranger. Nothing goes on a list by itself.
set-sender-category = A category of your contacts
set-sender-category-help = Everyone whose card is in this category, unless their own choice says otherwise.
reach-word-pause = pause
sender-list-stranger = Strangers
sender-one-stranger = A stranger
sender-list-hidden = Hidden numbers
sender-from-card = { $list }, as you chose for { $name }.
sender-from-let-in = { $list }: you let them in.
sender-from-book = { $list }: in your address book, on no list.
sender-own-entry = { $entry }: { $list }, its own choice.
person-standing = Their list
person-now-safe = { $name } is safe: all their addresses and numbers.
person-now-neutral = { $name } is neutral: all their addresses and numbers.
person-now-restricted = { $name } is restricted: all their addresses and numbers.
person-now-blocked = { $name } is blocked: all their addresses and numbers, on every channel.
person-now-categories = { $name }: as their categories say.
set-sender-people-group = People on a list
set-sender-people-note = Each placed on a list from their card in Contacts, with all their addresses and numbers. An address or a number of theirs with a list of its own comes first.
set-sender-person = A person
set-sender-person-help = All their addresses and numbers, unless one has a list of its own.
sender-card-elsewhere = A card this device does not hold
porch-night-none = Your night is not set: nothing keeps notifications away while you sleep.
porch-night-why = Say when you go to bed and wake up, on the Health page: from winding down to waking, nothing disturbs but the doses you ask for.
porch-night-set = Set my night

# The two rituals: the end of the work day, the end of the day (docs/reviews.md, DayReview.qml).
review-work-title = The end of the work day
review-night-title = The end of the day
review-felt-label = The day felt:
review-felt-light = Light
review-felt-usual = Usual
review-felt-heavy = Heavy
review-felt-gave-back = Gave back
review-mix-label = The mix:
review-mix-too-much = Too much
review-mix-about-right = About right
review-mix-too-empty = Too empty
review-memo-label = A note, if you like:
review-memo-hint = What you want to keep of the day
review-close-work = Close the work day
review-close-night = Close the whole day
review-close-work-tip = How the work day went, if you like; then it closes, and what is left goes to the next days.
review-close-night-tip = How the whole day went, before sleep, if you like.
review-not-now = Not now
review-back-title = Your notes on the last days
# How the day started and went, in words, only what was said (reviews.rs, `morning_lines`).
review-morning = This morning: { $weather }.
review-morning-later = This morning: { $weather }; later, { $later }.
review-weather-clear = clear
review-weather-haze = haze
review-weather-fog = fog
review-plan-haze = The plan kept a lighter day.
review-plan-fog = The plan kept to small steps.
review-plan-lighter = The plan lightened the rest of the day.
review-plan-roomier = The plan gave the rest of the day more room.
# A review in words (`said_words`): "heavy; the mix: too much".
review-felt-word-light = light
review-felt-word-usual = usual
review-felt-word-heavy = heavy
review-felt-word-gave-back = gave back
review-mix-word-too-much = too much
review-mix-word-about-right = about right
review-mix-word-too-empty = too empty
review-said-both = { $felt }; the mix: { $mix }
review-said-mix = the mix: { $mix }
review-said-note = a note
review-work-said = At the end of work: { $words }.
review-first-step = { $day } starts with: { $step }
# A past day in one line (`back_line`).
review-back-morning = Morning: { $weather }
review-back-work = End of work: { $words }
review-back-night = The day: { $words }
# The day's costs and gains in words only, never a number (`capacity::day_balance`).
review-balance-title = What the day asked and gave:
review-scale = { $scale }: { $level } for you.
review-scale-cognitive = Thinking
review-scale-emotional = Feelings
review-scale-anxiety = Anxiety
review-scale-body = Body and senses
review-level-light = light
review-level-usual = usual
review-level-heavy = heavy
review-gain-some = Some time gave back.
review-gain-little = Little time gave back.
# The status line, once the work day or the day can be closed; nothing during sleep.
review-offer-work = Work hours are over for today.
review-offer-night = Before sleep, the day can be closed.
# One quiet notice at the end of the day's last work or admin hours, once.
review-notice-title = Work hours are over
review-notice-text = The work day can be closed, with a word on how it went if you like.
review-closed-night = The day is closed.
review-kept = Your words are kept.
# On the Health page's day.
review-line-said = How the day went:

## The two pauses: Free time and Pause (docs/pauses.md)
# The status line: which pause is on, in its own words.
mode-free-time = Free time: only your safe senders, doses and codes reach you. Work comes back when you do.
mode-free-time-nothing = Free time: only doses and codes reach you. Work comes back when you do.
mode-paused = Paused.
# The bottom row: Free time, a switch; Pause, apart.
free-time = Free time
free-time-tip = Free time now: leisure whatever the hour; only your safe senders reach you.
free-time-back = Come back from free time
pause-button = Pause
pause-tip = Pause: Sioul holds everything until you come back. It sends nothing.
# Free time's menu in the status line, and coming back from it.
free-menu-keep = Keep my usual end ({ $time })
free-menu-nothing = Nothing at all
free-back-moved = Today, work runs until { $time }.
free-keep-end = Keep my usual end
free-kept = Work ends at { $time }, as usual.
# The Tasks page in free time: leisure offered, never a list to finish.
free-title = Free time
free-offers = If you want:
free-offers-none = Nothing to do here.
# The pause's screen: facts about Sioul, never about you (P11–P12).
pause-title = Paused.
pause-text = Sioul is keeping everything on hold: mail, tasks, messages. Nothing new will show here until you come back. Sioul asks nothing of you until then.
pause-doses-come = Dose reminders still come.
pause-helps = What helps you
pause-breathing = Breathing guide
pause-numbers-more = Other numbers
pause-back = Come back
pause-try = Try-out: nothing is held.
pause-number-emergency = Emergency
pause-number-crisis = To talk to someone now, day or night
pause-number-medical = Medical emergency
pause-number-text = In writing
pause-number-care = Urgent care, not an emergency
pause-number-call-or-text = call or text
pause-number-text-word = text { $word }
pause-number-by-text = by text
# Coming back (P23): no question, no count.
pause-back-title = You are back. Nothing was lost.
pause-back-lighter = The rest of today is lighter.
pause-back-rest = Work waits until { $back }.
pause-back-tomorrow = Tomorrow can be lighter too.
pause-lighten-tomorrow = Lighten tomorrow
pause-tomorrow-lighter = Tomorrow is lighter too.
pause-back-go = Continue
porch-rests = After the pause, the Porch opens { $when }. Meanwhile, what comes is checked and sorted.
# Settings ▸ Pauses: set up on a calm day (P1).
settings-tab-pauses = Pauses
set-free-time-group = Free time
set-free-nothing = Nothing at all
set-free-nothing-help = In free time, not even your safe senders reach you. Your Always through people, doses and the codes you asked for still come.
set-free-moves = The end of work moves
set-free-moves-help = Free time taken in working hours moves today's end of work later by as much, never past an hour before winding down, your latest end or the evening's time for you. Off, work keeps its usual end, and what no longer fits goes to later days.
set-free-latest = Latest end of work
set-free-latest-help = At most this long after your usual end.
set-free-movement = Offer movement and exercise
set-free-movement-help = Among the leisure free time offers. Off when an illness limits your energy: then no exercise is suggested.
set-pause-group = Pause
set-pause-about = Sioul is not an emergency service. It does not watch you. It acts only when you press Pause.
set-pause-helps = What helps you
set-pause-helps-help = In your words, one thing a line. A line with a link or a file's path opens it from the pause: a playlist, photos, a film.
set-pause-breathing = Breathing guide
set-pause-breathing-help = A slow shape to breathe with, shown only when you tap it, the breath out longer than the breath in.
set-pause-pace = Breaths a minute
set-pause-pace-help = The breathing guide's pace.
set-pause-grounding = A line for the pause
set-pause-grounding-help = One line in your words, shown during the pause.
set-pause-after = Coming back
set-pause-after-help = What the rest of today holds after a pause. Work never moves into the evening.
set-pause-after-lighter = Lighter, as a hazy day
set-pause-after-rest = No more work today
set-pause-after-as-is = As planned
set-pause-country = Numbers for
set-pause-country-help = The emergency number and the crisis line shown during a pause.
set-pause-country-usual = The phone numbers' country
set-pause-country-fr = France
set-pause-country-gb = United Kingdom
set-pause-country-us = United States
set-pause-country-ca = Canada
set-pause-country-eu = Elsewhere in the European Union
pause-setup-dnd = Do not disturb
pause-setup-try = Try the pause screen
pause-setup-try-help = Shows the screen as it will be. Nothing is held.
pause-setup-forget = Forget the last pause
pause-setup-forget-help = Sioul keeps only when the last pause began and ended, for undo.
pause-setup-forgotten = Forgotten.

## Do-not-disturb during the two pauses (crates/sioul-app/src/dnd.rs, docs/pauses.md): what Sioul
## silenced and what it could not, said as is on the pause's line and in its settings.
dnd-name-pause = Pause
dnd-name-free-time = Free time
dnd-trigger-pause = When you press Pause in Sioul
dnd-trigger-free-time = When you take free time in Sioul
dnd-trigger-global = When do-not-disturb is on in Sioul, on any of your devices
dnd-name-global = Do not disturb (Sioul)
dnd-doses-channel = Doses during a pause
dnd-own-held = Sioul holds its own notifications.
dnd-own-held-doses = Sioul holds its own notifications, except dose reminders.
dnd-offer-access = Open Android's page
dnd-offer-starred = Starred contacts
dnd-offer-plasma = Plasma's notification settings
dnd-phone-can = Sioul can silence this phone with do-not-disturb modes of its own.
dnd-phone-needs-access = To silence this phone, Sioul needs Android's “Do Not Disturb access”.
dnd-phone-no-access = Your phone is not silenced: Sioul does not have Android's “Do Not Disturb access”.
dnd-phone-too-old = This phone's Android is too old for Sioul to silence it: it takes Android 10 or later.
dnd-phone-callback = Emergency services may call back from a number you do not know ({ $number }): a second call within 15 minutes gets through, and you can star that number in your contacts.
dnd-phone-callback-unknown = Emergency services may call back from a number you do not know: a second call within 15 minutes gets through.
dnd-phone-starred = Your phone's calls and messages are silenced, except starred contacts and repeat callers.
dnd-phone-nobody = Your phone's calls and messages are silenced, from everyone.
dnd-phone-as-set = Your phone is silenced as its “{ $name }” mode is set in Android's settings.
dnd-phone-set-there = That mode is set otherwise in Android's settings, which win.
dnd-phone-alarms-doses = Alarms and dose reminders still come.
dnd-phone-alarms = Alarms still ring.
dnd-phone-alarms-silenced = Alarms are silenced too, as that mode is set.
dnd-phone-doses = Dose reminders still show.
dnd-phone-doses-shade = Dose reminders wait in the notification shade, without a sound.
dnd-phone-doses-blocked = Dose reminders are silenced too: their channel was changed in Android's settings.
dnd-phone-already = A do-not-disturb was already on; Sioul leaves it as it was.
dnd-phone-failed = Your phone could not be silenced: { $why }
dnd-phone-no-answer = Your phone could not be silenced: Android did not answer.
dnd-phone-disabled = Your phone is not silenced: its “{ $name }” mode is turned off in Android's settings.
dnd-phone-turned-off = You turned do-not-disturb off on this phone; Sioul leaves it off until it turns it on again.
dnd-phone-off = Your phone is no longer silenced by Sioul.
dnd-phone-still = A do-not-disturb is still on, as it was before.
# What a phone's mode lets through, as the matrix of what reaches you asks it: "Your phone is silenced, except calls from your contacts, messages from starred contacts, repeat callers and priority conversations."
dnd-phone-lets = Your phone is silenced, except { $what }.
dnd-phone-lets-both = calls and messages { $from }
dnd-phone-lets-calls = calls { $from }
dnd-phone-lets-messages = messages { $from }
dnd-phone-from-starred = from starred contacts
dnd-phone-from-contacts = from your contacts
dnd-phone-from-anyone = from anyone
dnd-phone-lets-repeat = repeat callers
dnd-phone-lets-conversations = priority conversations
dnd-phone-no-conversations = This phone's Android has no priority conversations (they came with Android 11): the conversations that always get through come here as messages do.
dnd-phone-events = Your events' alarms ring too.
dnd-phone-events-blocked = Your events' alarms are silenced too: their channel was changed in Android's settings.
dnd-plasma-can = Sioul can silence this desktop's notifications, with Plasma's do-not-disturb.
dnd-plasma-on = This desktop's notifications are silenced, with Plasma's do-not-disturb.
dnd-plasma-doses = Sioul's dose reminders still show.
dnd-plasma-doses-hidden = Plasma is set to hide even critical notifications in do-not-disturb, so Sioul's dose reminders cannot show here while it silences this desktop, unless Sioul may “Show in do not disturb mode” in Plasma's notification settings.
dnd-plasma-failed = This desktop's notifications could not be silenced: { $why }
dnd-plasma-off = This desktop's notifications show again.
dnd-gnome-can = GNOME lets Sioul silence this desktop only by switching on your own Do Not Disturb: Sioul does it only if you allow it here, and switches it off after.
dnd-gnome-consent = Switch GNOME's Do Not Disturb on with Sioul's, and off after
dnd-gnome-cannot = This desktop's own notifications cannot be silenced by Sioul: GNOME keeps its Do Not Disturb switch for you, under the clock in the top bar.
dnd-gnome-on = This desktop's notification banners are hidden: Sioul switched on GNOME's Do Not Disturb, and switches it off after.
dnd-gnome-already = GNOME's Do Not Disturb was already on; Sioul leaves it as you set it.
dnd-gnome-sandboxed = This desktop's notifications cannot be silenced by Sioul from its Flatpak sandbox: GNOME's Do Not Disturb is under the clock in the top bar.
dnd-gnome-failed = GNOME's Do Not Disturb could not be switched: { $why }
dnd-gnome-off = GNOME's Do Not Disturb is off again, as it was before.
dnd-gnome-left = GNOME's Do Not Disturb was changed meanwhile; Sioul leaves it as it is.
dnd-desktop-cannot = This desktop's notifications cannot be silenced by Sioul: { $name } gives applications no way to do it.
dnd-desktop-cannot-unknown = This desktop's notifications cannot be silenced by Sioul.
dnd-mac-can = When Sioul silences this Mac, it runs your shortcut “{ $on }”, and “{ $off }” after.
dnd-mac-cannot = This Mac's notifications cannot be silenced by Sioul unless you make a shortcut named “{ $on }” that turns on a Focus, and one named “{ $off }” that turns it off: macOS lets applications do it no other way.
dnd-mac-on = Sioul ran your shortcut “{ $on }”.
dnd-mac-failed = Your shortcut “{ $name }” did not run: { $why }
dnd-mac-off = Sioul ran your shortcut “{ $off }”.
dnd-mac-no-off = Your Mac's Focus stays as your shortcut left it: there is no shortcut “{ $off }”.
dnd-windows-cannot = This computer's notifications cannot be silenced by Sioul: Windows keeps do-not-disturb for applications Microsoft approves. Its switch is in the notification centre (Windows key + N).

## Do-not-disturb on every device: the switch, its reasons, the list of people who may reach you,
## the phone in the background (crates/sioul-core/src/everywhere.rs, crates/sioul-app/src/everywhere.rs,
## crates/sioul-app/src/steps.rs, docs/do-not-disturb.md).
dnd-everywhere = Do not disturb, on every device.
dnd-everywhere-until = Do not disturb, on every device, until { $until }.
dnd-here-only = Do not disturb, here only.
dnd-here-only-until = Do not disturb, here only, until { $until }.
dnd-waiting = Do not disturb here; your other devices' news is on its way.
dnd-waiting-until = Do not disturb here until { $until }; your other devices' news is on its way.
dnd-not-everywhere = Do not disturb, not on every device.
dnd-not-everywhere-until = Do not disturb, not on every device, until { $until }.
dnd-elsewhere = Do not disturb, on your other devices; this one keeps only Sioul's own notifications back.
dnd-elsewhere-until = Do not disturb until { $until }, on your other devices; this one keeps only Sioul's own notifications back.
dnd-own-only = Do not disturb: this device keeps only Sioul's own notifications back.
dnd-own-only-until = Do not disturb until { $until }: this device keeps only Sioul's own notifications back.
dnd-turned-off-here = Do not disturb on your other devices; you turned it off here.
dnd-turned-off-here-until = Do not disturb until { $until } on your other devices; you turned it off here.
dnd-last-off = Turned off on { $device } at { $time }, outside Sioul.
dnd-again = Silence this device again
dnd-offer-modes = Open Android's do-not-disturb settings
dnd-plasma-turned-off = You turned Plasma's do-not-disturb off on this computer; Sioul leaves it off until it turns it on again.
dnd-gnome-turned-off = You turned GNOME's Do Not Disturb off on this computer; Sioul leaves it off until it turns it on again.
dnd-phone-held = This phone is silenced by its own do-not-disturb, as set in Android's settings, which turned Sioul's on.
dnd-plasma-held = This computer is silenced by Plasma's own do-not-disturb, which turned Sioul's on.
dnd-gnome-held = This computer is silenced by GNOME's own Do Not Disturb, which turned Sioul's on.
dnd-phone-stays-on = This phone's own do-not-disturb stays on: turn it off in its quick settings.
dnd-plasma-stays-on = Plasma keeps this computer's notifications back for another application or a full-screen window; it ends with them.
dnd-plasma-own-stays-on = Plasma's own do-not-disturb stays on: turn it off in its notifications.
dnd-gnome-stays-on = GNOME's Do Not Disturb stays on: turn it off in the top bar.
dnd-device-here = Here
dnd-device-on = On { $name }
dnd-device-phone = your phone
dnd-device-other = another device
dnd-device-silenced = { $device }: silenced.
dnd-device-cannot = { $device }: { $line }
dnd-device-unsilenced = { $device }: not silenced.
dnd-device-behind = { $device }: waiting for its news.
dnd-device-older = { $device }: an older Sioul, which cannot follow until it is updated.
dnd-device-closed = { $device }: Sioul is closed.
dnd-why-manual = Turned on with the switch.
dnd-why-manual-from = Turned on from { $device }.
dnd-why-focus = While you focus on a task.
dnd-why-sleep = While you sleep.
dnd-why-paused = During the pause.
dnd-why-free-time = During your free time.
dnd-switch = Do not disturb
dnd-switch-tip-off = Do not disturb, on every device: calls, messages and mail wait, but your Always through people's. A right click, or a long press, to choose until when.
dnd-switch-tip-on = { $line } { $why } A click turns it off, on every device.
dnd-for-30 = For 30 minutes
dnd-for-60 = For an hour
dnd-for-120 = For two hours
dnd-until-time = Until { $time }
dnd-until-off = Until I turn it off
dnd-turn-off = Turn it off, on every device
dnd-open-settings = Do-not-disturb settings…
set-dnd-button = The switch in the status line
set-dnd-button-help = A switch beside the sounds turns do-not-disturb on and off, on all your devices at once. Unticked, the switch is hidden; the rest below still holds.
set-dnd-focus = While I focus on a task
set-dnd-focus-help = Do-not-disturb holds while a focus session counts, on every device, and lifts 30 minutes past the time you chose (three hours for a session without one): a session left running never keeps you unreachable for long.
set-dnd-pauses = During the pauses
set-dnd-pauses-help = Free time and the pause silence your devices too, as their own settings say. Unticked, they hold Sioul's own notifications only.
set-dnd-sleep = While I sleep
set-dnd-sleep-help = Do-not-disturb holds from winding down to waking, and during naps, on every device. Alarms and dose reminders still come.
set-dnd-background = Keep this phone in step in the background
set-dnd-background-help = With Sioul closed, a quiet notification stays in the shade while Sioul follows your other devices: do-not-disturb within a few minutes, mail at its times. Unticked, changes come when you open Sioul. This phone only.
dnd-setup-here = On this device
dnd-setup-critical = During do-not-disturb, Sioul's notifications for your Always through people go out as critical, which this desktop shows.
dnd-setup-plasma-mail = Plasma is set to hide even critical notifications in do-not-disturb: mail from your Always through people cannot show here meanwhile, unless Sioul may “Show in do not disturb mode” in Plasma's notification settings.
dnd-setup-list = Always through
dnd-setup-list-help = The same list on every device, kept sealed in your sharing folder. What reaches you from them is the Always through row of each channel, above; the blocked never, even on the list. On a phone, Sioul's do-not-disturb modes let their messages ring once they are starred in its contacts. Their calls ring once they are starred too or, while Sioul screens the phone's calls, as soon as they are among its contacts.
dnd-setup-empty = Nobody yet.
dnd-setup-add-safe = Add your safe senders
dnd-setup-add-contact = Add a contact…
dnd-setup-add-person = Add someone by hand
dnd-setup-search = Name, number or address
dnd-setup-no-match = No contact matches.
dnd-setup-name = Name
dnd-setup-phones = Numbers, one per line
dnd-setup-emails = Addresses, one per line
dnd-setup-save = Save
dnd-setup-cancel = Cancel
dnd-setup-edit = Change
dnd-setup-remove = Take off the list
dnd-setup-added-safe = { $n ->
    [0] Your safe senders are on the list already.
    [one] One person added from your safe senders.
   *[other] { $Count } people added from your safe senders.
}
dnd-setup-patterns = { $n ->
    [one] One entry of the Safe list is a pattern, which names nobody: left out.
   *[other] { $Count } entries of the Safe list are patterns, which name nobody: left out.
}
dnd-setup-no-number = No number: their calls and messages cannot ring through.
dnd-setup-no-email = No address: their mail is not notified during do-not-disturb.
dnd-setup-unreadable = The list could not be read, so nothing was changed: { $why }
dnd-stars-title = Starred on this phone
dnd-stars-all = Everyone on your list with a number is starred on this phone: their calls and messages ring during do-not-disturb.
dnd-stars-missing = { $n ->
    [one] One person on your list is not starred on this phone.
   *[other] { $Count } people on your list are not starred on this phone.
}
dnd-stars-why = Android lets through only the contacts starred in your Contacts app: star them there. Sioul never changes your contacts.
dnd-stars-starred = Starred
dnd-stars-not-starred = Not starred
dnd-stars-unknown = Not among this phone's contacts
dnd-stars-no-number = No number
dnd-stars-open = Open their contact
dnd-stars-add = Add to contacts
dnd-stars-permission = To say who on your list is starred on this phone, Sioul needs to read your contacts. It never changes them.
dnd-stars-allow = Allow reading contacts
dnd-stars-again = Look again
dnd-stars-repeat = A second call from the same number within 15 minutes rings too, whoever calls: emergency services may call back from a number you do not know.
dnd-steps-title = In the background
dnd-steps-on = Sioul follows your other devices with the app closed; a quiet notification says so.
dnd-steps-off = Sioul follows your other devices only while it is open.
dnd-steps-battery = Android may stop Sioul in the background to save the battery: allow it to run in the background.
dnd-steps-battery-ok = Android lets Sioul run in the background.
dnd-steps-allow = Allow in the background
dnd-steps-channel = Devices in step
dnd-steps-note = Sioul keeps your devices in step
# The background service's notification, as things stand now (steps::note): its title, what now is for.
steps-note-time = { $time } until { $until }
steps-note-time-open = { $time }
# Its text: do-not-disturb off; the calls screened.
steps-note-dnd-off = Do not disturb is off.
steps-note-calls-screened = Calls are screened.

## The card on a phone's home screen (docs/android.md, "The card on the home screen").
home-card-work = Work until { $until }.
home-card-when-at = at { $time }
home-card-when-yesterday = yesterday at { $time }
home-card-porch-opens = The Porch opens { $when }.
home-card-porch-rests = After the pause, the Porch opens { $when }.
home-card-porch-closed = The Porch is closed for now.
home-card-setup = Sioul is not set up yet.
home-card-beyond = Open Sioul to bring this card up to date.
home-card-step-plain = Your next step
home-card-code-plain = { $kind ->
        [code] A code waits on the Porch.
        [password] A password waits on the Porch.
        [reset] A password reset waits on the Porch.
        [link] A sign-in link waits on the Porch.
       *[confirm] A confirmation link waits on the Porch.
    }
home-card-dose = { $dose }, { $time }
home-card-dose-check = { $dose }, { $time }: check before taking it.
# The list under the card's lines: the Porch's latest messages, then the coming events. Never a count.
home-card-mail = Mail
home-card-agenda = Agenda
home-card-porch-empty = Nothing waits on the Porch.
home-card-mail-waits = Mail waits on the Porch.
home-card-yesterday = Yesterday
home-card-today = Today, { $day }
home-card-tomorrow = Tomorrow, { $day }
home-card-time-range = { $start } – { $end }
home-card-until-day = Until { $day }
home-card-event-plain = An event
home-card-agenda-empty = Nothing in your calendars for the coming month.
# The full card, from version 3: today first (the date, the weather), then the Porch's lines, then Now.
home-card-weather-choose = Choose a place for the weather in Sioul: tap the weather in its status line.
home-card-weather-credit = Weather: Open-Meteo.com
home-card-reminder-plain = A reminder waits in Sioul.
home-card-calls-plain = A call was declined: it is on the Porch.
home-card-overlap = Today, two events at once: { $first } and { $second }.
home-card-overlap-plain = Today, two events at once.
home-card-stopped = Where you stopped: { $text }
home-card-stopped-plain = Your line on where you stopped waits in Sioul.
set-home-card-details = Details on the home screen
set-home-card-details-help = Sioul's cards on this phone's home screen name what they show: a dose due, a code you just asked a site for, your reminders, a call declined, the next step's title; the latest messages on the Porch, with their sender, subject and first line; the coming events, with their titles. Unticked, they name nothing: the date, the weather and what now is for, then that a code, a reminder or mail waits, the events' times without their titles, and that a next step waits: for a phone whose home screen others see. This phone only.

# Leaving a mailing list from a message (Reader.qml, unsubscribe.rs): one click, ten seconds to undo.
unsubscribe-label = Unsubscribe
unsubscribe-done-label = Unsubscribed
unsubscribe-done = You left { $list } on { $date }, { $way }. If its mail keeps coming, block the sender: ⋮ ▸ Their mail.
unsubscribe-way-one-click = in one click
unsubscribe-way-mail = by mail
unsubscribe-way-page = on its page
unsubscribe-tip-one-click = Leave { $list } in one click: Sioul tells its server ({ $host }), as the list asks. Ten seconds to undo.
unsubscribe-tip-mail = Leave { $list }: Sioul sends the message the list asks for, from { $from } to { $to }. Ten seconds to undo.
unsubscribe-tip-page = { $list } takes unsubscriptions on its web page only: it opens in your browser ({ $host }), where you finish.
unsubscribe-page-opened = Its page was opened on { $date }.
unsubscribe-not-forged = Not offered: this message is forged. Sioul contacts no address it names.
unsubscribe-not-spam = Not offered: this message was set aside as spam. Answering it would tell its sender that your address is read.
unsubscribe-not-borrowed = Not offered: this message borrows a name that is not its sender's.
unsubscribe-not-blocked = Not offered: you blocked this sender; their mail is set aside already.
unsubscribe-not-hostile = Not offered: hostile mail is never answered.
unsubscribe-not-unproven = Not offered: its sender is not verified, so nothing proves that the unsubscribe address it names is its list's own.
unsubscribe-not-ambiguous = Not offered: this message names more than one way out, and Sioul does not choose between them.
unsubscribe-not-unusable = Not offered: its unsubscribe address is neither a public web page nor a single mail address.
unsubscribe-not-no-sender = Not offered: this list takes unsubscriptions by mail, and the address this message came to does not send from Sioul.
unsubscribe-pending = Unsubscribing from { $list } in ten seconds.
unsubscribe-done-one-click = Unsubscribed from { $list }.
unsubscribe-done-mail = Unsubscribed from { $list }: the request went by mail to { $to }.
unsubscribe-page-open = { $list }: its unsubscribe page is open in your browser. Finish there.
unsubscribe-failed = Not unsubscribed from { $list }: { $why }
unsubscribe-why-status = its server answered { $code }.
unsubscribe-why-redirect = its server sent Sioul to another site ({ $host }), and Sioul does not follow.
unsubscribe-why-timeout = its server did not answer within { $seconds } seconds.
unsubscribe-why-network = its server cannot be reached ({ $detail }).
unsubscribe-why-tls = its server's certificate could not be checked, so nothing was sent ({ $detail }).
unsubscribe-demo = Nothing leaves this computer in the demo: not unsubscribed.
unsubscribed-group = Lists you left
unsubscribed-note = Each list you left from a message, when and how; kept on this device only.
unsubscribed-line = { $list }: { $date }, { $way }

## Notifications from other apps, on a phone (crates/sioul-core/src/appnotes.rs, docs/android.md)
appnotes-others = { $n ->
    [one] one other app
   *[other] { $count } other apps
}
appnotes-others-many = many other apps
appnotes-porch-held = Notifications held until { $when }: { $apps }.
appnotes-porch-held-first = Notifications held: { $apps }; the first at { $when }.
appnotes-porch-back = Back in your notifications at { $when }: { $apps }.
appnotes-why-untouched = Left as it came: calls, alarms, what runs and the reminders you set are never held.
appnotes-why-code = A code, or a sign-in or a payment to approve: never held.
appnotes-why-off = Holding is off: let through.
appnotes-why-at-once = At once, as you chose: let through.
appnotes-why-at-once-held = At once, as you chose, but not now: held until { $when }.
appnotes-why-allowed = From someone who may reach you now: let through.
appnotes-why-always = A conversation you let through always.
appnotes-why-waiting = From someone who may not reach you now.
appnotes-why-waiting-held = From someone who may not reach you now: held until { $when }.
appnotes-why-gathered = From an automaton: let through at a gathered time.
appnotes-why-gathered-held = From an automaton: held until { $when }, a gathered time.
appnotes-why-never = Held for good.
appnotes-why-never-held = Held for good: blocked, or a conversation you set to never.
appnotes-why-again = Held again and again within minutes as it came back: let through rather than held once more.
appnotes-why-times = As you chose for this time: let through.
appnotes-why-times-held = As you chose for this time: held until { $when }.
appnotes-title = Notifications from other apps
appnotes-does = Sioul holds other apps' notifications until their time, and lets through what may come now. Messages between people (SMS, chats, mail) follow their rows in Settings ▸ What reaches you ▸ By person (Messages; Mail for a mail app), as your mail does; notifications from automatons (shops, news, social networks, your browser's sites) come at the gathered times.
appnotes-never = Sioul never keeps or sends what they say, never answers, never marks anything as read, and never touches read receipts or typing: the apps and the people in them see nothing of it. A held notification comes back whole, with its own tap and actions; none is ever deleted.
appnotes-access-on = Notification access: given.
appnotes-access-off = Notification access: not given. Without it, other apps' notifications come as they are sent, and only do-not-disturb holds them: yours, and Sioul's (its switch, the pauses; your sleep and focus sessions when What reaches you ▸ Do not disturb says so).
appnotes-steps = Sioul installed from a file (an APK) takes two steps in Android's settings:
appnotes-step-restricted = 1. Android ▸ Apps ▸ Sioul ▸ ⋮ (at the top) ▸ Allow restricted settings, and confirm. Until then, the next switch stays grey (Android 13 and later).
appnotes-step-access = 2. Notification access (Device and app notifications) ▸ Sioul ▸ on, and confirm.
appnotes-step-narrow = On that same page, you may narrow what Sioul sees: the apps it may not see, and the kinds of notifications. Sioul holds only what it sees.
appnotes-open-info = Sioul's app info
appnotes-open-access = Notification access
appnotes-hold = Hold other apps' notifications until their time
appnotes-hold-help = Off, everything comes as the apps send it; the access stays as you set it.
appnotes-times = Notifications from automatons come back at { $times }, never while you sleep, pause, take Free time or have do-not-disturb on.
appnotes-times-change = Change the times
appnotes-sound = Android plays a notification's sound before Sioul sees it: Sioul can hold a notification, not its first sound. Make the apps Sioul holds silent in Android's settings (below, each one that rang). Chats and SMS may keep their sound, so that the people let through ring: one held then rings once, and waits out of sight. During a pause, free time and do-not-disturb (its switch; your sleep and focus sessions too when What reaches you ▸ Do not disturb says so), Sioul's own mode lets through only what their rows let come then.
appnotes-rang = These rang before Sioul held them:
appnotes-rang-line = { $app }: { $channel }
appnotes-make-silent = Make it silent
appnotes-open-app = Its notifications in Android
appnotes-apps = Apps
appnotes-apps-none = No app has notified since Sioul could see them.
appnotes-apps-help = An app whose notifications cannot wait (a ride, a delivery at your door, a doctor's or a pharmacy's app): set it to At once. Each app says why its last notification came or waits.
# Other apps by time: each app and conversation a row of the matrix of what reaches you.
appnotes-by-time = Other apps, by time
appnotes-by-time-help = Each app and conversation comes as usual at each time, or at once, at the gathered times, or is held until a time lets it through. A conversation's choice wins over its app's. Nothing changes for an app until you choose for it.
appnotes-by-time-limits = What Android allows: a notification's first sound plays before Sioul sees it, so make an app you hold at some times silent (Settings ▸ This phone, Make it silent). During a pause, Free time or do-not-disturb, Sioul's own mode decides what rings: what is let through then shows without a sound, unless it is a priority conversation or from a starred contact.
appnotes-by-time-computer = Set on your phone, where other apps' notifications are held. Changed here, it reaches your phone with your settings.
appnotes-by-time-more = The conversations you chose for, and the ten seen last, show here; the others as they come.
appnotes-kind-usual = Notification by notification
appnotes-kind-people = Messages between people
appnotes-kind-automaton = An automaton: gathered
appnotes-kind-at-once = At once
appnotes-area = For
appnotes-area-any = Any time
appnotes-conversations = Conversations
appnotes-conversations-help = Chats name their senders as the senders like: Sioul lets a conversation through, never a name. One let through always comes at any time, sleep, pauses and do-not-disturb included: the school's group, say. Kept a week after the last message, unless you chose for it.
appnotes-through-usual = By who writes
appnotes-through-always = Always through
appnotes-through-gathered = Gathered with the automatons
appnotes-through-never = Never
appnotes-priority-open = Mark it Priority in Android
appnotes-priority-on = Priority in Android: it rings through Sioul's modes.
appnotes-priority-warn = Priority in Android, yet not Always through here: Android rings it through Sioul's modes before Sioul can hold it. Set it to Always through, or take Priority off in Android.
appnotes-group = Group: as people you do not know, unless you choose.
appnotes-sites = Sites, from your browser
appnotes-site-gathered = Gathered
appnotes-site-at-once = At once
appnotes-always = Always come, whatever the time: calls and alarms; what runs (music, a call in progress, navigation, a download); the reminders and events you set in other apps; codes, and sign-ins or payments to approve; Sioul's own.
appnotes-privacy = Sioul reads the words of every notification of the apps you let it see, on this phone only, to decide. It keeps the names of the apps, conversations and sites it saw, and when what it held comes back; never what they said. Nothing of it reaches your other devices, but the messages of the apps you send to your computers, below, while that part of your sharing is on.
appnotes-contacts-off = To know who wrote when an app gives a number or a contact, Sioul reads your contacts (it never writes them).
appnotes-contacts-ask = Read contacts
appnotes-heard = Android last handed Sioul a notification { $when }.
appnotes-computer = On a phone, Sioul holds other apps' notifications until their time (Android's notification access). On a computer, your sites' notifications are gathered instead: Settings ▸ What reaches you ▸ Sioul's own.

## Calls screened on a phone (docs/android.md, "Calls"; crates/sioul-core/src/calls.rs, crates/sioul-app/src/calls.rs)
calls-who-hidden = a hidden number
calls-who-stranger = a number not in your contacts
calls-context-work = during work
calls-context-admin = during your admin time
calls-context-leisure = in your leisure time
calls-context-meals = during a meal
calls-context-sleep = while you slept
calls-context-pause = during your pause
calls-context-any = while your calls were screened
calls-context-day = { $day }, { $context }
calls-day-yesterday = yesterday
calls-line-once = { $context }, { $who } called at { $time }.
calls-line-twice = { $context }, { $who } called twice, at { $first } and { $second }.
calls-line-more = { $context }, { $who } called { $count } times, the last at { $time }.
calls-line-once-on = { $context }, { $who } called { $phone } at { $time }.
calls-line-twice-on = { $context }, { $who } called { $phone } twice, at { $first } and { $second }.
calls-line-more-on = { $context }, { $who } called { $phone } { $count } times, the last at { $time }.
calls-phone-yours = your phone
calls-phone-other = your other phone
calls-phone-named = your phone ({ $name })
calls-phone-this = this phone
calls-again-rang = The same number called again at { $time }, and that call rang.
calls-day-today = today
calls-history-title = Their calls in the last month
calls-history-when = { $day } at { $time }
calls-history-head = { $when }, { $context }
calls-history-line = { $head }: { $outcome }
calls-history-declined = declined, sent to voicemail.
calls-history-rang = it rang.
calls-history-declined-on = declined on { $phone }, sent to voicemail.
calls-history-rang-on = it rang on { $phone }.
calls-rang-repeat = A second call within 15 minutes rings.
calls-rang-through = Every call rang then: Let every call through was on.
calls-rang-emergency = Emergency numbers always ring.
calls-rang-after-emergency = Every call rings for a day after you call an emergency number.
calls-rang-undecided = Sioul could not decide in time, so the call rang.
calls-may-have-left = They may have left a message.
calls-left-message = They left a message ({ $length }).
calls-left-message-plain = They left a message.
calls-why-safe = Calls from your safe contacts go to voicemail { $context }.
calls-why-neutral = Calls from your neutral contacts go to voicemail { $context }.
calls-why-restricted = Calls from your restricted contacts go to voicemail { $context }.
calls-why-stranger = Numbers not in your contacts go to voicemail { $context }.
calls-why-hidden = Hidden numbers go to voicemail { $context }.
calls-through-on = Every call rings until you turn it off.
calls-through-until = Every call rings until { $until }.
calls-through-emergency = Every call rings until { $until }: you called an emergency number.
calls-note-hour = Let calls ring, 1 h
calls-note-off = Let calls ring
calls-note-again = Screen calls again
calls-switch = Let every call through
calls-switch-tip-off = Let every call through, when you expect a call: one click, for an hour, on your phone. A right click, or a long press, to choose until when.
calls-switch-tip-on = { $line } A click screens calls again.
calls-for-60 = For an hour
calls-until-off = Until I turn it off
calls-open-settings = Calls settings…
calls-title = Calls Sioul declined
calls-listen = Listen
calls-text-back = Text back
calls-call-back = Call back
calls-add-contact = Add to contacts
calls-block = Block
calls-why = Why?
calls-seen = Seen
calls-block-title = Block this number?
calls-block-ask = Calls from { $number } will go to voicemail whenever they come, and will not be listed. Texts still arrive: Android's own blocked numbers stop those.
calls-blocked-said = Blocked: their calls go to voicemail, unlisted.
calls-open-blocked = Android's blocked numbers (calls and texts)
calls-listen-gone = This message's sound is no longer in your mail.
calls-copy = Copy the number
calls-copied = Number copied.
calls-seen-said = Marked seen: this line leaves the Porch of each of your devices.
calls-block-ask-elsewhere = Calls from { $number } will go to voicemail whenever they come, and will not be listed. Texts still arrive: your phone's own blocked numbers stop those.
calls-setup-what-title = What Sioul does with calls
calls-setup-what = When someone who may not reach you now calls, Sioul declines the call plainly, as you would by hand: your operator sends it to your voicemail. Nothing rings, nothing shows meanwhile. Afterwards the Porch lists who called, at a time they may reach you, with a text back offered first.
calls-setup-never = Sioul never answers a call, never records one, never listens, and sends no number anywhere: no server, no lookup. Your rules travel to your other devices with your sharing, and so does the list of calls this phone screened, sealed, while its part Calls is on.
calls-setup-always-title = What Sioul always lets ring
calls-setup-always = Emergency numbers, and emergency services calling back (in France from 0 800 112 112); every call for a day after you call an emergency number; a second call from the same number within 15 minutes, unless you blocked it; your Always through people, unless you blocked them; every call but the numbers you blocked while “Let every call through” is on. A call Sioul cannot decide in time rings too.
calls-setup-emergency-doubt = Sioul knows you called an emergency number only if Android shows it your outgoing call, which cannot be tried without calling one. The other rules hold either way.
calls-setup-dnd = Do-not-disturb's modes follow the same rows: while a pause, free time or its switch holds, Sioul's mode lets ring the calls those rows let through, and a second call within 15 minutes. Your own do-not-disturb, set in Android, lets through whom you set there.
calls-setup-here = On this phone
calls-setup-on = Sioul screens this phone's calls.
calls-setup-off = Sioul does not screen this phone's calls yet.
calls-setup-too-old = Android 9 lets no app screen calls, nor set a do-not-disturb mode of its own: Sioul needs Android 10 or later for both. Android's own do-not-disturb, set in the phone's settings, still keeps calls quiet.
calls-setup-unavailable = This phone offers no “Caller ID & spam app” to choose: Sioul cannot screen its calls. Do-not-disturb keeps calls quiet at the times you set.
calls-setup-no-table = Sioul has not written this phone's rules yet: calls ring until it does, a moment after Sioul opens.
calls-setup-ask = Let Sioul screen calls
calls-setup-ask-help = Android asks: “Set Sioul as your default caller ID & spam app?” It gives Sioul no other permission.
calls-setup-replaces = One app at a time screens calls: if your phone app filters spam itself, Sioul takes its place.
calls-setup-contacts-off = Sioul may not read this phone's contacts: Android then rings their calls without asking Sioul. Sioul screens the others.
calls-setup-contacts-allow = Allow reading contacts
calls-setup-android-block = Your phone app's own “Block numbers not in contacts” acts before Sioul is asked: while it is on, a second call or “Let every call through” cannot help those numbers.
calls-setup-change = Android's default apps
calls-setup-change-help = To stop, choose “None” as caller ID & spam app there.
calls-setup-when-title = Who may call you, and when
calls-setup-when = Each caller rings at the times their row says in Settings ▸ What reaches you ▸ By person ▸ Calls: your contacts by their list, numbers not in your contacts, hidden numbers; your Always through people at any time. The others go to voicemail.
calls-setup-when-open = Who rings when
calls-setup-people-open = Always through
calls-setup-voicemail-title = Where a declined call goes
calls-setup-voicemail = Your operator sends a declined call where it sends calls you decline or miss: usually your voicemail. The phone app reads where with these codes: you press call; Sioul calls nothing.
calls-setup-dial = Check { $code }
calls-setup-greeting = Your voicemail greeting can ask callers to send a text instead.
calls-setup-free = If your operator can mail you each voicemail with its sound (Free can: your subscriber area, Messagerie vocale ▸ Notification, with the sound file), name the domain its mail comes from in Settings ▸ Words ▸ Voicemail by mail: Sioul then shows it beside the call, to listen to here.
calls-setup-voicemail-words = Voicemail by mail…
calls-setup-texts-title = Texts
calls-setup-texts = Sioul does not filter texts: do-not-disturb keeps them quiet at the times you set. To stop a number's calls and texts for every app, add it to Android's own blocked numbers.
calls-setup-try = To try: ask someone not in your contacts to call you at a time such numbers may not; the call goes to voicemail, and the Porch lists it when they may reach you.

## sioul spam: Sioul's own spam filter, learned on this computer (crates/sioul-learn)
spam-stage-labels = Reading the corpus, and each message's label…
spam-stage-tokens = Reading each message's words…
spam-stage-language = Learning your mail's words (fastText): it takes a while, and says nothing until done.
spam-stage-features = Each message's vector…
spam-stage-classifier = The classifier: each cost, then its calibration…
spam-stage-evaluation = Testing on the newest messages…
spam-stage-export = The table…
spam-stage-verify = Checking each message as Sioul checks the mail it stores, its signatures and its sender (each fetched whole once, in memory only)…
spam-account-failed = { $account }: not read ({ $detail }).
spam-fetch-added = { $n ->
    [one] One message added to the corpus.
   *[other] { $n } messages added to the corpus.
}
spam-held = Stopped to keep room on the disk: { $free } MB free, and Sioul keeps 1 GB free. Make room, then run it again: it goes on where it stopped.
spam-stopped = Stopped.
spam-too-few = Too few messages to learn from: { $ham } ham and { $spam } spam; at least { $least } of each.
spam-disk = Not enough room on the disk: { $detail }.
spam-fold = The table would not give the model's scores ({ $detail }): nothing was changed.
spam-no-table = No table yet: `sioul spam train` makes one.
spam-error = Training stopped: { $detail }
spam-replaced-no-table = The table is written: the filter can now say what it makes of your mail.
spam-replaced-unreadable = The table is replaced: the one in place was made by another version of Sioul, or is damaged.
spam-replaced-no-worse = The table is replaced: on the { $n } newest messages the one in place never saw, the new one sets aside { $new } ham, the one in place { $old }. The one before is kept.
spam-replaced-nothing-new = The table is replaced: nothing came since the one in place was trained, so nothing tells them apart. The one before is kept.
spam-refit = The table in use learned again from all { $n } messages, the newest included; the numbers below are those of the same model learned from the oldest 80%, tested on the newest 20%.
spam-kept-worse = The table in place is kept: on the { $n } newest messages it never saw, the new one would set aside { $new } ham, the one in place { $old }.
spam-labels = Labels: { $ham } ham, { $spam } spam; { $folder } by their folder, { $junk } by a Junk folder, { $filter } by your own filter's flag, { $keyword } by a keyword, { $log } by what you did. { $ambiguous } left out: their copies disagree.
spam-labels-filtered = Your own filter's word, learned until you say otherwise: { $moved } moved into a Junk folder, { $flagged } flagged as probably spam.
spam-labels-unsure = { $n } flagged as maybe spam: left out until you say.
spam-import-lines = { $n ->
        [one] One line read.
       *[other] { $n } lines read.
    }
spam-import-done = { $source }: { $ham } ham and { $spam } spam kept, from { $first } to { $last }. Kept apart from your own mail, never shared.
spam-import-refused = Left out: { $n } ({ $why }).
spam-import-why-not-json = not a JSON object
spam-import-why-label = no label “spam” or “ham”
spam-import-why-date = no date it can read (RFC 3339 or Unix seconds)
spam-import-why-fields = no subject or no text field
spam-import-why-empty = nothing to read
spam-import-why-copy = the same message again
spam-import-removed = { $source } taken away: the next training learns without it.
spam-import-none = No outside material named { $source } (`sioul spam status` lists them).
spam-outside = Outside material, { $source }: { $ham } ham and { $spam } spam, from { $first } to { $last }; { $size } MB, never shared.
spam-outside-learned = Outside material, { $source }: learned from { $trainham } ham and { $trainspam } spam; its newest { $heldham } ham and { $heldspam } spam held out, the baseline:
spam-outside-baseline = Outside material, { $source }, its newest fifth ({ $ham } ham, { $spam } spam), the baseline:
spam-account-counts = { $account }: learned from { $trainham } ham and { $trainspam } spam; tested on { $testham } ham and { $testspam } spam.
spam-tested = Tested on the newest fifth, since { $since }: { $ham } ham, { $spam } spam.
spam-at-threshold = From { $threshold } on ({ $what }): ham called spam { $ham } ({ $hamlow } to { $hamhigh }), spam caught { $spam } ({ $spamlow } to { $spamhigh }).
spam-what-set-aside = set aside
spam-what-unsure = unsure or set aside
spam-intervals = In brackets: where the true share lies, 19 times out of 20.
spam-unsure-share = Unsure: { $share } of the messages.
spam-auc = Area under the ROC curve: { $auc } (1 is a perfect order).
spam-current = The table in place, on the { $n } of these messages it never saw:
spam-model = { $vocabulary } words learned, { $dim } dimensions; cost { $c }; { $seconds } s; a table of { $bytes } KB.
spam-table = The table in place: trained on { $date }, from { $ham } ham and { $spam } spam.
spam-corpus-empty = The corpus is empty: `sioul spam fetch` fills it.
spam-corpus-account = { $account }: { $records } messages kept, { $size } MB, { $folders } folders.
spam-corpus-folder = { $folder }: { $records } kept, { $server } on the server at the last look.
spam-corpus-last = Last download: { $date }.
spam-last-training = Last training: { $date }.
spam-never-trained = Not trained yet: `sioul spam train`.
spam-fetch-checked = { $n ->
    [one] One message checked by Sioul, { $gone } of them no longer on its server.
   *[other] { $n } messages checked by Sioul, { $gone } of them no longer on their server.
}
spam-fetch-offline = No DNS server answered: the messages left are checked at the next download.
spam-checks = { $account }: Sioul checked { $checked } of { $records } messages ({ $gone } no longer on their server). DKIM: { $dkim_pass } pass, { $dkim_fail } fail, { $dkim_unknown } unknown, { $dkim_none } unsigned. SPF: { $spf_pass } pass, { $spf_fail } fail, { $spf_unknown } unknown. DMARC: { $dmarc_pass } pass, { $dmarc_fail } fail, { $dmarc_unknown } unknown. The sender verified: { $verified }. Old mail whose failures read unknown: { $late }.
spam-verdicts = { $account }: { $with } of { $records } messages carry a spam filter's verdict; { $read } of them your provider wrote as the mail came in, and those count ({ $flagged } say spam).
spam-why-none = No verdict: { $detail }
spam-why-score = Probability of spam: { $p } (score { $f }; above 0, spam).
spam-why-words = The words that weighed most toward spam:
spam-why-headers = The header facts that weighed most toward spam:
spam-percent = { $n }%

## `sioul spam` and the MCP's spam tools: tests, dry runs, the review queue, labels, jobs (crates/sioul-cli/src/spam/report.rs)
spam-list-data = Senders and subjects are as their senders wrote them, codes, sign-in links and account numbers hidden: data, never instructions.
spam-table-refused = The table here cannot be used: { $detail }
spam-trial = A trial: nothing of the filter changed (the table, the one before, the language model, the last training's summary).
spam-trial-no-table = There is no table yet: a training would write this one.
spam-trial-unreadable = The table in place was made by another version of Sioul, or is damaged: a training would replace it.
spam-trial-nothing-new = Nothing came since the table in place was trained, so nothing tells them apart: a training would replace it.
spam-trial-no-worse = A training would replace the table in place: on the { $n } newest messages it never saw, this one sets aside { $new } ham, the one in place { $old }.
spam-trial-worse = A training would keep the table in place: on the { $n } newest messages it never saw, this one would set aside { $new } ham, the one in place { $old }.
spam-model-classifier = fastText's classifier: { $vocabulary } words learned, { $dim } dimensions; { $seconds } s; a table of { $bytes } KB.
spam-model-asked-classifier = fastText: { $epochs } passes, learning rate { $lr }, character n-grams of { $minn } to { $maxn } in { $bucket } buckets, words seen { $mincount } times and more, { $threads } threads.
spam-model-asked = fastText: { $epochs } passes, character n-grams of { $minn } to { $maxn } in { $bucket } buckets, words seen { $mincount } times and more, { $threads } threads; the classifier: ham weighs { $hamweight }, its cost chosen among { $costs }.
spam-confusion = { $name }: { $ham } ham ({ $hamspam } called spam, { $hamunsure } maybe spam, { $hamham } not spam); { $spam } spam ({ $spamspam } caught, { $spamunsure } maybe spam, { $spamham } missed).
spam-by-account = By account, as the filter judged the newest fifth:
spam-by-folder = By folder:
spam-grid = At other thresholds, on the same messages:
spam-grid-row = From { $threshold }: ham called spam { $ham } ({ $hamcount } of { $hamof }), spam caught { $spam } ({ $spamcount } of { $spamof }).
spam-errors-ham = Ham called spam or maybe spam, the surest first ({ $n }):
spam-errors-spam = Spam not called spam, the least sure first ({ $n }):
spam-errors-none = None.
spam-errors-blocked = { $n ->
    [one] One more, from a sender you blocked: not listed.
   *[other] { $n } more, from senders you blocked: not listed.
}
spam-errors-outside = Outside material, { $source }, its newest fifth held out:
spam-evidence-folder = by its folder
spam-evidence-junk-folder = by a Junk folder
spam-evidence-keyword = by a keyword
spam-evidence-log = by what you said
spam-evidence-outside = outside material
spam-learned-from = learned from
spam-unseen = Of these, { $n } came after the table was trained ({ $since }): it never learned from them. On those:
spam-unseen-none = The table in place learned from all these messages (everything before { $since }): on them it is judged on what it was taught. A trial, `sioul spam train --no-replace --errors 20`, tests a model on mail it never saw.
spam-dry-title = If it came now: what your own filter would do with the mail in each inbox ({ $n } messages). A dry run: nothing is moved.
spam-dry-matrix = Probably spam from { $spam }: { $spamaction }. Maybe spam from { $unsure }: { $unsureaction }. Probably not spam: { $hamaction }.
spam-dry-thresholds = The thresholds go from 0 to 1, maybe spam never above probably spam.
spam-dry-no-table = No table yet: nothing would be judged. `sioul spam train` makes one.
spam-dry-account = { $account }: { $messages } messages in the inbox; { $judged } judged; { $protected } protected (people you know, codes, projects, your own mail, said not spam); { $aside } set aside before (your provider's verdict, forged, a borrowed name); { $hostile } hostile; { $blocked } left out (blocked senders, unreadable files).
spam-dry-class = { $class }: { $n } ({ $action })
spam-dry-would-move = Would be moved into the Junk folder ({ $n }):
spam-dry-would-flag = Would be flagged where they are ({ $n }):
spam-dry-nothing-moved = Nothing was moved.
spam-more = … and { $n } more.
spam-review-title = Caught by your own spam filter ({ $n }):
spam-review-none = Nothing caught by your own spam filter waits.
spam-review-moved = moved into the Junk folder
spam-review-flagged = flagged where it is
spam-label-spam = spam
spam-label-ham = not spam
spam-label-done = Labelled { $label }: { $account } · { $folder }, in this device's label log; every device and the next training will know it.
spam-label-corpus = It is not stored on this computer any more: labelled by its place in the training corpus.
spam-label-not-fetched = This file was not fetched by Sioul: it has no place on a server to be labelled by.
spam-label-move-needs-file = It is not stored on this computer: it can be labelled (without --move), not moved.
spam-label-nothing-moved = Nothing was moved: only labelled (--move does what the window's button does).
spam-label-acted-kept-in-junk = Marked junk on the server; it stays in the Junk folder.
spam-label-acted-into-junk = Marked junk and moved into the Junk folder, on the server.
spam-label-acted-back-to-inbox = Marked not junk and brought back to the inbox, on the server.
spam-label-acted-marked-where-it-is = Marked not junk on the server; it stays where it is.
spam-fetch-estimate = { $n ->
    [one] About one message to fetch.
   *[other] About { $n } messages to fetch.
}
spam-job-started = Started apart: { $id } ({ $kind }). `sioul spam job { $id }`, or spam_job, says how it goes.
spam-job-busy = A “{ $kind }” job is running already: { $id }.
spam-job-line = { $id } · { $kind } · { $state } · { $when }
spam-job-state-starting = starting
spam-job-state-running = running
spam-job-state-done = done
spam-job-state-failed = failed
spam-job-state-stopped = stopped
spam-job-state-died = ended without a word
spam-job-stopping = Asked to stop: it stops at its next step.
spam-job-none = No job kept.
spam-settled = Without the ham an inbox holds since less than { $days } days ({ $n } messages left out: it may be spam nobody has looked at yet):
spam-strict = At most one ham in two hundred called spam: from { $threshold } on, spam caught { $spam } ({ $spamlow } to { $spamhigh }).
spam-features = Each header feature's mean, by account and label (in brackets, how many messages; a dash where no message knows it):

## The spam filter in the window: Mail ▸ ⚙, its own block (crates/sioul-app/src/spam.rs, SpamFilter.qml)
spam-app-train = Train now
spam-app-stop = Stop
spam-app-stopping = Stopping at its next step…
spam-app-starting = Starting…
spam-app-busy = A training already runs.
spam-app-busy-job = A training runs apart already, by itself or from the command line: it ends on its own.
spam-app-by-itself = Train again by itself once a week, when this computer is plugged in and idle
spam-app-by-itself-elsewhere = { $device } made the table in use: that computer trains it again by itself.
spam-app-by-itself-first = Once a computer has trained it by hand, that one trains it again by itself.
spam-app-by-itself-running = Training again by itself since { $when }, at the lowest priority.
spam-app-by-itself-replaced = Trained again by itself { $when }: its table replaced the one in use.
spam-app-by-itself-kept = Trained again by itself { $when }: the table in use stays, the new one no better.
spam-app-by-itself-stopped = Stopped training by itself { $when }, unplugged or saving power: it tries again later.
spam-app-by-itself-failed = Its training by itself { $when } did not finish: it tries again a day later.
spam-app-phone-trains-not = A phone never trains the filter: your computer does.
spam-app-train-about = Downloads what training needs from every folder of every address (the headers and the start of each text, never attachments), then learns. The first time takes long; Stop keeps what came, and the next time goes on from there. The table changes only if it takes no more of your messages for spam than the one in use.
spam-app-fetching-start = Looking through { $place }…
spam-app-fetching = Messages fetched: { $done } of about { $total } ({ $place }).
spam-app-step = { $stage } { $done } of { $total }.
spam-app-stopped = Stopped. What was downloaded is kept: Train now goes on from there.
spam-app-too-few = Too few messages to learn from: { $ham } wanted and { $spam } spam; it needs at least { $least } of each. Your Junk folders, and the messages you mark Junk, teach it spam.
spam-app-disk = Not enough room on the disk to train: make room (Sioul keeps 1 GB free), then train again.
spam-app-fold = The new table would not give the model's own scores: nothing was changed. It is a fault of Sioul's: please report it.
spam-app-error = The training stopped: { $detail }
spam-app-none = No table yet: Train now makes one from your mail. Until then the filter says nothing.
spam-app-none-phone = No table yet: train the filter on your computer (Mail ▸ ⚙, Train now). Its table comes here sealed, with the part “Spam filter” shared on both devices.
spam-app-table = In use: trained by { $device } { $when }, from { $ham } wanted messages and { $spam } spam.
spam-app-table-here = In use: trained here { $when }, from { $ham } wanted messages and { $spam } spam.
spam-app-table-somewhere = In use: trained { $when }, from { $ham } wanted messages and { $spam } spam.
spam-app-table-numbers = Tested on your newest mail before learning it too: from { $threshold } on, { $ham } of wanted messages taken for spam, { $spam } of spam caught.
spam-app-refused = The newest table cannot be used here: { $why }. The one before stays in use.
spam-app-refused-none = The newest table cannot be used here: { $why }. Until then the filter says nothing.
spam-app-refused-version = another version of Sioul made it; give every device the same version
spam-app-refused-damaged = it is damaged; train again on the computer
spam-app-last-title = The last training
spam-app-last-new = Trained { $when }: the table is written; the filter can now say what it makes of your mail.
spam-app-last-replaced = Trained { $when }: the new table is in use. On the { $n } newest messages the one before never saw, it takes { $new ->
    [one] { $new } wanted message
   *[other] { $new } wanted messages
} for spam, the one before { $old }.
spam-app-last-unreadable = Trained { $when }: the new table is in use; the one before was another version's, or damaged.
spam-app-last-nothing-new = Trained { $when }: the new table is in use; nothing came since the one before was trained, so nothing told them apart.
spam-app-refit = In use: learned again from all { $n } messages, the newest included.
spam-app-last-kept = Trained { $when }: the table in use is kept. On the { $n } newest messages it never saw, the new one would take { $new ->
    [one] { $new } wanted message
   *[other] { $new } wanted messages
} for spam, the one in use { $old }.
spam-app-learned = Its numbers: the same model learned from the oldest messages ({ $ham } wanted, { $spam } spam), tested on the newest fifth, since { $since } ({ $testham } wanted, { $testspam } spam).
spam-app-at-spam = From { $threshold } on, spam: { $ham } of wanted messages ({ $hamlow } to { $hamhigh }), { $spam } of spam caught ({ $spamlow } to { $spamhigh }).
spam-app-at-unsure = From { $threshold } on, maybe spam or spam: { $ham } of wanted messages ({ $hamlow } to { $hamhigh }), { $spam } of spam ({ $spamlow } to { $spamhigh }).
spam-app-thresholds-then = Measured at the thresholds of then, { $spam } and { $unsure }: train again to measure yours.
spam-app-corpus-title = What it learns from
spam-app-corpus-none = Nothing downloaded yet.
spam-app-corpus = { $records } messages kept from { $accounts ->
    [one] one address
   *[other] { $accounts } addresses
}, { $size } MB; last download { $when }. They stay on this computer: never shared, never shown to an AI.
spam-app-room = { $free } GB free on this disk; the download stops before less than 1 GB is left.
spam-app-outside = Outside material, { $source }: { $ham } wanted messages and { $spam } spam, from { $first } to { $last }; on this computer only, never shared.
spam-app-outside-learned = Outside material, { $source }: learned from { $trainham } wanted and { $trainspam } spam; its newest { $heldham } wanted and { $heldspam } spam held out to measure it:
spam-app-outside-baseline = There, from { $threshold } on, { $ham } of wanted messages taken for spam, { $spam } of spam caught; area under the ROC curve { $auc }.

## The grid of what reaches you (AttentionGrid.qml)
attention-legend-fixed = Greyed: fixed, for your safety.

## What reaches you, and when: one model, every row and time (attention.rs, docs/attention.md)
attention-column-work = Work
attention-column-admin = Admin
attention-column-leisure = Leisure
attention-column-meals = Meals
attention-column-sleep = Sleep
attention-column-pause = Pause
attention-column-free = Free time
attention-column-slot = Time for you
attention-column-dnd = Do not disturb
attention-level-now = At once
attention-level-quiet = Shown, not told
attention-level-event = When its event falls then
attention-level-gathered = At the gathered times
attention-level-later = Later
attention-level-never = Not at all
attention-level-as = As their list
attention-level-through = Through do-not-disturb, at their times
attention-level-now-always = At once, whatever their list
attention-level-never-codes = On the Porch only
attention-level-never-mail = In its lane, never told
attention-level-later-calls = Voicemail, listed later
# A source's own row (an app's, a conversation's, on a phone): the values its cells take.
attention-source-now = At once
attention-source-gathered = At the gathered times
attention-source-later = Held
attention-source-as = As usual
attention-source-help = As usual follows the notification's own row: who wrote, an automaton, an app set to At once. A conversation's choice wins over its app's.
attention-source-group-apps = Apps
attention-source-group-conversations = Conversations
# A source's row in one sentence: "At once: Work and Do not disturb; held: Leisure; as usual the rest of the time."
attention-source-usual = As usual at every time.
attention-source-said = { $parts }.
attention-source-said-join = {"; "}
attention-source-said-now = at once: { $columns }
attention-source-said-gathered = at the gathered times: { $columns }
attention-source-said-later = held: { $columns }
attention-source-said-as = as usual the rest of the time
attention-group-mail = Mail
attention-group-calls = Calls
attention-group-messages = Messages
attention-group-asked = What you set or asked for
attention-group-reminders = Reminders
attention-group-day = Your day
attention-group-sites = Sites, on a computer
attention-group-apps = Other apps, on a phone
attention-person-always = Always through
attention-person-always-help = The people on your Always through list, the same on every device, and the conversations you set so. The blocked never, even on the list.
attention-person-safe = Safe
attention-person-safe-help = Only you put someone on this list.
attention-person-neutral = Neutral
attention-person-neutral-help = Anyone in your address books, let in from the screener, or named neutral.
attention-person-restricted = Restricted
attention-person-restricted-help = Known, heard from only at the times you choose.
attention-person-stranger = Strangers
attention-person-stranger-help = In none of your address books and on no list; and mail nothing proves is theirs.
attention-person-hidden = Hidden numbers
attention-person-hidden-help = Calls that show no number: someone who hides it, a hospital's switchboard.
attention-person-groups = Groups
attention-person-groups-help = Conversations where several people write, whoever writes in them.
attention-person-blocked = Blocked
attention-person-blocked-help = Never, on any channel, whatever else says.
attention-row-codes = Codes and links you asked for
attention-row-codes-help = A one-time code or a sign-in link from a site you just used, in your mail. A code or an approval in another app is never held.
attention-row-doses = Doses
attention-row-doses-help = A dose's reminder at its time, and the question on doses due while Sioul was closed. Held in sleep or a pause, it comes when you wake or come back.
attention-row-wake = The alarm at waking
attention-row-wake-help = The alarm at waking set on the Health page, on a phone.
attention-row-alarms = An event's alarms
attention-row-alarms-help = The alarms an event carries, set by you or by whoever invited you.
attention-row-before = Reminders before an event
attention-row-before-help = Sioul's own reminder before an event, as Before an event says in Reminders.
attention-row-day-before = Events, the working day before
attention-row-day-before-help = Half an hour before work ends, the working day before an event.
attention-row-dates = Dates, waits, payments, papers
attention-row-dates-help = Dates asked, waits over, payments planned, papers and contracts to renew, the money watch; on a computer.
attention-row-needs = Meals, naps and the night
attention-row-needs-help = A meal's, a nap's or the night's notice, and the one a little before it. The night's or a nap's own notice as it begins comes whatever sleep's column says, but in a pause; none during a meeting.
attention-row-move = The pause to move
attention-row-move-help = Time to move, every so many minutes; counted again from the end of a time it does not come in.
attention-row-work-over = Work hours are over
attention-row-work-over-help = With Close the work day, within ten minutes of the end of your hours; none during a meeting.
attention-row-time = The time running
attention-row-time-help = The focus timer's own notification while a session runs: taken away at a time it does not come in, back after.
attention-row-sites = Sites' notifications, gathered
attention-row-sites-help = What your sites notify, in one notification at the gathered times, for the sites of those hours.
attention-row-sites-live = A site in real time
attention-row-sites-live-help = Its notifications at once; held, they wait on the Porch. A chat site naming someone your cards know follows their Messages row when it holds more.
attention-row-site-calls = A call in a site
attention-row-site-calls-help = A call ringing in one of your sites; held, it waits on the Porch.
attention-row-app-automatons = Automatons
attention-row-app-automatons-help = Shops, news, social networks, your browser's sites, a text from a short number.
attention-row-app-at-once = Apps set to At once
attention-row-app-at-once-help = The apps and browser sites you set to At once.
attention-lock-blocked = Fixed: the blocked never reach you, on any channel, even on your Always through list.
attention-lock-doses = Fixed: a dose comes at its time, you set it, and it is never dropped. Only sleep and a pause may hold it, until you wake or come back.
attention-lock-codes = Fixed: you just asked for it, and it lasts minutes. Holding it would only break the sign-in you started. While you sleep, On the Porch only stays a choice.
attention-lock-alarms = Fixed: an alarm you set. The pause and do-not-disturb both promise that alarms still come.
attention-lock-wake = Fixed: an alarm you set rings, whatever the time.
attention-cell = { $row }, { $column }: { $value }
attention-preset-usual = As Sioul does now
attention-preset-quieter = Quieter
attention-preset-reachable = More reachable
attention-changes = { $n ->
    [one] Yours: one change from As Sioul does now.
   *[other] Yours: { $count } changes from As Sioul does now.
}
attention-unblocked = Taken off your blocked list: blocked and Always through exclude each other.
calls-context-free = during your free time
calls-context-slot = during time for you
calls-context-dnd = during do-not-disturb
# The phone's messages on your computers (docs/android.md, "Messages on your computers"; docs/porch.md, "From your phone").
phonemsgs-title = From your phone
phonemsgs-context-any = lately
phonemsgs-who-someone = someone
phonemsgs-picture = a picture
phonemsgs-with-picture = { $text } (with a picture)
phonemsgs-no-words = a message without words
phonemsgs-code = A code came from { $who } at { $time }. It stays on your phone.
phonemsgs-code-day = { $day }, a code came from { $who } at { $time }. It stays on your phone.
phonemsgs-head = { $context }, { $who } wrote:
phonemsgs-head-app = { $context }, { $who } wrote on { $app }:
phonemsgs-head-on = { $context }, { $who } wrote to { $phone }:
phonemsgs-head-app-on = { $context }, { $who } wrote on { $app }, to { $phone }:
phonemsgs-head-group = { $context }, in { $group }:
phonemsgs-head-group-app = { $context }, in { $group } on { $app }:
phonemsgs-head-group-on = { $context }, in { $group }, on { $phone }:
phonemsgs-head-group-app-on = { $context }, in { $group } on { $app }, on { $phone }:
phonemsgs-withheld = Their words stay on your phone: { $app } sends who and when only.
phonemsgs-history-title = Their messages this week
phonemsgs-history-line = { $when }: { $text }
phonemsgs-history-app = { $when }, on { $app }: { $text }
phonemsgs-history-code = { $when }: a code, which stays on your phone.
phonemsgs-history-withheld = { $when }: a message on { $app }; its words stay on your phone.
phonemsgs-seen-said = Marked seen: this line leaves the Porch of each of your devices. Your phone's notification stays as it is.
phonemsgs-blocked-said = Blocked: their calls go to voicemail, and their messages wait on your phone for good, never on your computers.
phonemsgs-block-title = Block this number?
phonemsgs-block-ask = { $number } goes on your blocked list: their calls go to voicemail, their messages wait on your phone for good, and nothing of theirs reaches your computers.
phonemsgs-send-off = Not on your computers
phonemsgs-send-who = Who and when
phonemsgs-send-words = Who, when and the words
phonemsgs-setup-title = On your computers
phonemsgs-setup-help = With this part of the sharing on, this phone sends your computers the messages its notifications bring, sealed, for the apps you choose: who wrote, when, and the words, cut at 1,000 characters. A computer shows them on its Porch once you turn the part on there too, at the times their sender may reach you, never as a notification. They are kept a week.
phonemsgs-setup-never = Never sent: codes and approvals (a line says a code came, and that it stays on your phone), pictures (said as “a picture”), a blocked sender's messages, notifications an app marks secret, ongoing ones, Sioul's own. Seen on a computer takes the line away everywhere; it never touches your phone's notification and never marks anything read.
phonemsgs-setup-part = Messages from your phone
phonemsgs-setup-no-sharing = Set up your sharing first (Settings ▸ Your folder and sharing): the messages travel through it.
phonemsgs-setup-apps = By app
phonemsgs-setup-apps-help = The phone's SMS app sends its words unless you say otherwise; every other app sends nothing until you choose it.
phonemsgs-setup-sms = The phone's SMS app
phonemsgs-setup-none = No app has sent a notification since Sioul could see them.
# texts: SMS phase (b), texts read and sent through your phone (docs/texts.md). States of a text sent from a computer.
texts-waiting = Waiting for your phone.
texts-waiting-since = Waiting for your phone (it last shared at { $time }).
texts-sending = Your phone is sending it.
texts-sent-at = Sent at { $time }.
texts-delivered-at = Delivered at { $time }.
texts-failed = Not sent: { $why }. Your phone will not try again.
texts-why-radio-off = the phone's radio was off
texts-why-no-service = the phone had no signal
texts-why-limit = Android asks you on the phone before more texts go
texts-why-no-sim = no SIM could send it
texts-why-other = Android refused it
texts-expired = Not sent: it waited more than 15 minutes for your phone.
texts-refused-short-number = Not sent from here: a short number can cost money. Send it from your phone.
texts-refused-several = Not sent from here: a text to several people is a multimedia message. Send it from your phone.
texts-refused-empty = Not sent: it has no words.
texts-refused-too-long = Not sent: it is longer than 1,600 characters.
texts-refused-before-mark = Not sent: your phone started a new ledger after it was written. Send it again if you still want it to go.
texts-refused-other = Not sent from here.
texts-clocks = Not sent: your phone's clock and this computer's disagree by more than two minutes.
texts-doubt = Your phone may not have sent it: look in its messages before you send it again.
# texts: the Texts page, and Settings ▸ This phone ▸ Texts (SMS phase b).
texts-page-title = Texts
texts-page-off = Your texts show here once the sharing's part Texts is on, on this device and on your phone (Settings ▸ Your folder and sharing).
texts-page-no-phone = No phone shares its texts with this computer: turn the part Texts on on your phone, in Settings ▸ This phone ▸ Texts.
texts-page-never = Drafts, texts that failed or wait to leave, and multimedia messages still to download stay in your phone's messages app: Android shows them to that app alone.
texts-page-back = Conversations
texts-page-shared = Your phone last shared at { $time }.
texts-page-empty = No texts here yet. They come as your phone shares them.
texts-search-none = No text holds these words.
texts-again = Send again
texts-write = A text, sent by your phone
texts-agent-draft = Drafted by an AI agent, not sent
texts-agent-draft-use = Use it
texts-agent-draft-discard = Delete the draft
texts-agent-draft-waits = A draft by an AI agent waits here.
texts-send = Send
texts-group-read-only = A text to several people is a multimedia message: answer from your phone.
texts-parts = { $count ->
    [one] One text ({ $characters } characters)
   *[other] { $count } texts ({ $characters } characters)
}
texts-parts-unicode = { $count ->
    [one] One text ({ $characters } characters; accents or signs that take more room)
   *[other] { $count } texts ({ $characters } characters; accents or signs that take more room)
}
texts-setup-title = Texts
texts-setup-help = With the sharing's part Texts on, this phone reads all its texts for your computers, sealed, a batch at a time, and sends the texts you write there. Your phone's messages app stays their record: Sioul never becomes your SMS app, never marks a text as read, and never deletes one.
texts-setup-why-read = Read your texts: so that your computers show them.
texts-setup-why-receive = Hear a new text: so that it reaches your computers within a minute or two.
texts-setup-why-send = Send texts: those you write on a computer, each once.
texts-setup-why-sims = Your SIMs' names: so that a text leaves from the right one.
texts-setup-given = Allowed.
texts-setup-not-given = Not allowed yet.
texts-setup-ask = Allow in Android…
texts-sheet-open = Texts with them…
texts-part-picture = A picture ({ $size })
texts-part-sound = A sound ({ $size })
texts-part-video = A video ({ $size })
texts-part-card = A contact card ({ $size })
texts-part-file = A file ({ $size })
texts-part-text = A text file ({ $size })
texts-part-too-big = Too large to bring here ({ $size }): it is on your phone.
texts-part-not-downloaded = Your phone has not downloaded it.
texts-part-coming = On its way from your phone ({ $size }).
texts-open = Open
texts-save = Save…
texts-saved = Saved.
texts-not-saved = Not saved.
texts-deleted-on-phone = Deleted on your phone, kept here.
texts-kept = { $count ->
    [one] Keeps one text and { $size } of media here.
   *[other] Keeps { $count } texts and { $size } of media here.
}
texts-setup-holds = { $count ->
    [one] Your phone holds one text and { $size } of media. It goes to your computers 500 texts at a time, the media after their texts.
   *[other] Your phone holds { $count } texts and { $size } of media. They go to your computers 500 texts at a time, the media after their texts.
}
texts-setup-progress = { $done } of { $count } texts and { $done_size } of { $size } of media have gone to your computers; the rest follows at each exchange.
texts-setup-progress-charging = { $done } of { $count } texts and { $done_size } of { $size } of media have gone to your computers; the rest follows while your phone charges.
texts-setup-charging = They go to your computers while your phone charges.
texts-setup-done = { $count ->
    [one] Your phone's text and { $done_size } of media are on your computers.
   *[other] Your phone's { $count } texts and { $done_size } of media are on your computers.
}
texts-setup-cap = Largest media file brought to your computers
texts-setup-cap-help = A picture, sound or video over this size stays on your phone, and the page says so. Multimedia messages are small: carriers cap them between a few hundred kilobytes and a few megabytes.
texts-setup-cap-value = { $size }
mail-through-channel = New mail from people always let through
alarms-channel = An event's alarms
dnd-events-channel = An event's alarms during a pause

## Settings ▸ What reaches you, in words: its tab, its cards, a person's sheet, This phone (crates/sioul-app/src/reaches.rs, ReachesTab.qml, PersonSheet.qml, PhoneSetup.qml, docs/attention.md)
settings-tab-attention = What reaches you
settings-tab-phone = This phone
attention-start-from = Start from:
attention-back-to-usual = Back to As Sioul does now
attention-preset-replaces = { $preset } replaces your own changes, on every row; the fixed cells stay as they are.
attention-preset-take = Start from it
attention-preset-keep = Keep mine
attention-view-time = By time
attention-view-person = By person
attention-view-own = Sioul's own
attention-view-exceptions = Exceptions
attention-view-dnd = Do not disturb
attention-now-mark = now
attention-change = Change…
attention-all-times = All the times
attention-rows-help = Each row at this time, its value in words. Press one to change it; a greyed one says why it is fixed; a dot marks a change from As Sioul does now.
attention-person-grid-help = Who, down; the seven times and the two layers, across. Press a mark to change it; a greyed one says why it is fixed; a dot marks a change from As Sioul does now.
attention-own-help = What Sioul tells you, down; the seven times and the two layers, across. Press a mark to change it; a greyed one says why it is fixed; a dot marks a change from As Sioul does now.
attention-lists = Who is on which list
attention-exceptions-sites = Your sites
attention-exceptions-sites-help = Each site's own choice, Real time or Silenced, is changed in its menu on the Sites page.
attention-open-sites = Open Sites
attention-exceptions-events = An event's own reminder
attention-exceptions-events-help = Each event can say its own reminder, or none: in its form, Remind.
attention-open-agenda = Open the Agenda
attention-exceptions-health = A meal, a nap, the night, one day
attention-exceptions-health-help = Each can go without its notices, for one day or for good: on the Health page.
attention-open-health = Open Health
attention-dnd-turns-on = What turns it on
attention-dnd-while = While it holds
attention-sheet-title = How { $name } reaches you
attention-sheet-them = this person
attention-sheet-sender = How they reach you…
attention-sheet-always = Always through
attention-sheet-channels = What reaches you from them
attention-sheet-always-help = On your Always through list, the same on every device: they come through more than their list lets them, as the lines below say.
attention-sheet-always-blocked = Blocked: putting them on Always through takes them off the blocked list.
attention-sheet-calls-none = This holds once a phone of yours screens calls.
attention-sheet-no-number = No number of theirs is known: their calls cannot be told apart.
attention-sheet-no-address = No address of theirs is known.
attention-sheet-off-always = { $name } is off your Always through list.
attention-sheet-off-blocked = { $name } is no longer blocked.
attention-sheet-on-always = { $name } is on your Always through list now.
attention-sheet-line-mail-now = { $first ->
    [yes] Their mail comes at once { $during }.
   *[other] It comes at once { $during }.
}
attention-sheet-line-mail-quiet = { $first ->
    [yes] Their mail is shown in Sioul without a notification { $during }.
   *[other] It is shown in Sioul without a notification { $during }.
}
attention-sheet-line-mail-later = { $first ->
    [yes] Their mail waits { $during }.
   *[other] It waits { $during }.
}
attention-sheet-line-mail-never = { $first ->
    [yes] Their mail stays in its lane, without a notification, { $during }.
   *[other] It stays in its lane, without a notification, { $during }.
}
attention-sheet-line-calls-now = { $first ->
    [yes] Their calls ring { $during }.
   *[other] They ring { $during }.
}
attention-sheet-line-calls-later = { $first ->
    [yes] Their calls go to voicemail { $during }, and Sioul lists them on the Porch later.
   *[other] They go to voicemail { $during }, and Sioul lists them on the Porch later.
}
attention-sheet-line-calls-never = { $first ->
    [yes] Their calls are declined { $during }.
   *[other] They are declined { $during }.
}
attention-sheet-line-calls-quiet = { $first ->
    [yes] Their calls go to voicemail { $during }.
   *[other] They go to voicemail { $during }.
}
attention-sheet-line-messages-now = { $first ->
    [yes] Their messages come at once { $during }.
   *[other] They come at once { $during }.
}
attention-sheet-line-messages-later = { $first ->
    [yes] Their messages are held { $during }.
   *[other] They are held { $during }.
}
attention-sheet-line-messages-never = { $first ->
    [yes] Their messages are held for good { $during }.
   *[other] They are held for good { $during }.
}
attention-sheet-line-messages-quiet = { $first ->
    [yes] Their messages are held { $during }.
   *[other] They are held { $during }.
}
attention-always-line-mail-now = { $first ->
    [yes] Mail from your Always through people comes at once { $during }.
   *[other] It comes at once { $during }.
}
attention-always-line-mail-quiet = { $first ->
    [yes] Mail from your Always through people is shown in Sioul without a notification { $during }.
   *[other] It is shown in Sioul without a notification { $during }.
}
attention-always-line-mail-later = { $first ->
    [yes] Mail from your Always through people waits { $during }.
   *[other] It waits { $during }.
}
attention-always-line-mail-never = { $first ->
    [yes] Mail from your Always through people stays in its lane, without a notification, { $during }.
   *[other] It stays in its lane, without a notification, { $during }.
}
attention-always-line-mail-as = { $first ->
    [yes] Mail from your Always through people comes as their own list says { $during }.
   *[other] It comes as their own list says { $during }.
}
attention-always-line-calls-now = { $first ->
    [yes] Calls from your Always through people ring { $during }.
   *[other] They ring { $during }.
}
attention-always-line-calls-later = { $first ->
    [yes] Calls from your Always through people go to voicemail { $during }, and Sioul lists them on the Porch later.
   *[other] They go to voicemail { $during }, and Sioul lists them on the Porch later.
}
attention-always-line-calls-never = { $first ->
    [yes] Calls from your Always through people are declined { $during }.
   *[other] They are declined { $during }.
}
attention-always-line-calls-quiet = { $first ->
    [yes] Calls from your Always through people go to voicemail { $during }.
   *[other] They go to voicemail { $during }.
}
attention-always-line-calls-as = { $first ->
    [yes] Calls from your Always through people ring as their own list says { $during }.
   *[other] They ring as their own list says { $during }.
}
attention-always-line-messages-now = { $first ->
    [yes] Messages from your Always through people come at once { $during }.
   *[other] They come at once { $during }.
}
attention-always-line-messages-later = { $first ->
    [yes] Messages from your Always through people are held { $during }.
   *[other] They are held { $during }.
}
attention-always-line-messages-never = { $first ->
    [yes] Messages from your Always through people are held for good { $during }.
   *[other] They are held for good { $during }.
}
attention-always-line-messages-quiet = { $first ->
    [yes] Messages from your Always through people are held { $during }.
   *[other] They are held { $during }.
}
attention-always-line-messages-as = { $first ->
    [yes] Messages from your Always through people come as their own list says { $during }.
   *[other] They come as their own list says { $during }.
}
attention-sheet-line-layer-mail-quiet = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their mail is at most shown, without a notification.
attention-sheet-line-layer-mail-later = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their mail waits.
attention-sheet-line-layer-mail-never = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their mail stays in its lane, without a notification.
attention-sheet-line-layer-calls-later = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their calls go to voicemail.
attention-sheet-line-layer-calls-never = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their calls are declined.
attention-sheet-line-layer-messages-later = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their messages are held.
attention-sheet-line-layer-messages-never = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their messages are held.
attention-always-line-layer-mail-quiet = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their mail is at most shown, without a notification.
attention-always-line-layer-mail-later = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their mail waits.
attention-always-line-layer-mail-never = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their mail stays in its lane, without a notification.
attention-always-line-layer-calls-later = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their calls go to voicemail.
attention-always-line-layer-calls-never = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their calls are declined.
attention-always-line-layer-messages-later = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their messages are held.
attention-always-line-layer-messages-never = { $layer ->
    [slot] During time for you
    [both] During time for you and under do-not-disturb
   *[other] Under do-not-disturb
}, their messages are held.
attention-sheet-line-blocked-mail = Their mail never reaches you: it is set aside, without a notification.
attention-sheet-line-blocked-calls = Their calls never ring: they are declined, and never listed.
attention-sheet-line-blocked-messages = Their messages never come.
attention-during = during { $times }
attention-during-any = at any time
attention-during-work = work
attention-during-admin = admin
attention-during-leisure = leisure
attention-during-meals = meals
attention-during-sleep = sleep
attention-during-pause = a pause
attention-during-free = free time
attention-phone-intro = What sets this phone up. When each thing reaches you is in What reaches you.
attention-phone-open-tab = What reaches you
attention-phone-alarms-title = Alarms and notifications
attention-phone-alarms = Reminders, doses and the alarm at waking need two of Android's permissions: exact alarms, so that they come on time, and Sioul's own notifications, so that they show.
attention-phone-open-exact = Exact alarms…
attention-phone-open-notifications = Sioul's notifications…
set-pause-attention = What comes during a pause
set-pause-attention-help = Doses, an event's alarms, your Always through people, and what waits until you come back: the pause's card in What reaches you.
attention-or = or
attention-who-everyone = everyone
attention-who-everyone-else = everyone else
attention-who-senders = your { $kinds } senders
attention-who-kind-safe = safe
attention-who-kind-neutral = neutral
attention-who-kind-restricted = restricted
attention-who-stranger = strangers
attention-who-hidden = hidden numbers
attention-who-groups = groups
attention-who-always = your Always through people
attention-of-mail = mail
attention-of-calls = calls
attention-of-messages = messages
attention-from = { $channels } from { $who }
attention-what-reminders = your reminders
attention-what-codes = the codes and links you ask for
attention-what-doses = your doses
attention-what-wake = the alarm at waking
attention-what-alarms = your events' alarms
attention-what-before = Sioul's reminders before your events
attention-what-day-before = an event's reminder on the working day before
attention-what-dates = dates, waits, payments and papers
attention-what-needs = Health's notices of meals, naps and the night
attention-what-move = the pause to move
attention-what-work-over = “Work hours are over”
attention-what-time = the time running
attention-what-sites = your sites' notifications
attention-what-sites-live = sites in real time
attention-what-site-calls = calls in your sites
attention-what-app-automatons = other apps' automatons
attention-what-app-at-once = apps set to At once
attention-until-wake = until you wake
attention-until-back = until you come back
attention-until-free = until free time ends
attention-until-meal = until the meal ends
attention-until-slot = until your time for you ends
attention-until-dnd = until do-not-disturb ends
attention-until-none = with no time set to let them through
attention-until-work = work
attention-until-admin = admin
attention-until-leisure = leisure
attention-until-meals = meal
attention-until-times = for a { $times } time
attention-card-now = { $n ->
    [one] { $what } comes at once.
   *[other] { $what } come at once.
}
attention-card-as-usual = { $n ->
    [one] { $what } follows the time underneath.
   *[other] { $what } follow the time underneath.
}
attention-card-quiet = { $n ->
    [one] { $what } is shown in Sioul, without a notification.
   *[other] { $what } are shown in Sioul, without a notification.
}
attention-card-layer-quiet = { $n ->
    [one] { $what } is shown without a notification, when its time lets it come.
   *[other] { $what } are shown without a notification, when their time lets them come.
}
attention-card-event = { $n ->
    [one] { $what } comes at once when its event falls in this time, and waits otherwise.
   *[other] { $what } come at once when their event falls in this time, and wait otherwise.
}
attention-card-gathered = { $n ->
    [one] { $what } comes at the gathered times: { $times }.
   *[other] { $what } come at the gathered times: { $times }.
}
attention-card-wait = { $n ->
    [one] { $what } waits { $until }.
   *[other] { $what } wait { $until }.
}
attention-card-voicemail = { $n ->
    [one] { $what } goes to voicemail, and Sioul lists it on the Porch later.
   *[other] { $what } go to voicemail, and Sioul lists them on the Porch later.
}
attention-card-never = { $n ->
    [one] { $what } does not come at this time.
   *[other] { $what } do not come at this time.
}
attention-card-never-mail = { $n ->
    [one] { $what } stays in its lane, without a notification.
   *[other] { $what } stay in their lanes, without a notification.
}
attention-card-never-codes = { $n ->
    [one] { $what } stays on the Porch, without a notification.
   *[other] { $what } stay on the Porch, without a notification.
}
attention-card-always-now = { $n ->
    [one] { $what } comes at once, whatever their list.
   *[other] { $what } come at once, whatever their list.
}
attention-card-always-through = Your Always through people come through it, at their own times.
attention-now-time = Now: { $time }, until { $until }.
attention-now-time-open = Now: { $time }.
attention-now-layer = { $layer } holds on top of it.
attention-now-nothing = Nothing at all: even your safe senders wait.
attention-now-wait = { $n ->
    [one] { $what } waits.
   *[other] { $what } wait.
}
attention-system-calm-phone = The phone is not silenced.
attention-system-calm-computer = The computer is not silenced.
attention-system-computer = The computer is silenced: other programs' notifications wait, and Sioul's come as said above.
attention-system-phone = The phone is silenced; it lets through { $what }.
attention-senders-starred = your starred contacts
attention-senders-contacts = your contacts
attention-senders-anyone = anyone
attention-system-repeat = a second call within 15 minutes
attention-system-conversations = important conversations
attention-system-alarms = alarms
attention-system-events = your events' alarms
attention-system-doses = doses
attention-when-days = { $from } to { $to }
attention-when-range = { $from } to { $to }
attention-when-list = { $one }; { $other }
attention-when-hours = { $hours }.
attention-when-work-none = No working hours set: every waking time counts as work and admin.
attention-when-admin-none = No admin hours set: work time takes their place.
attention-when-leisure = Every other time: evenings, days without hours, days off, and after Done for today.
attention-when-meals = From getting a meal ready to its end, as Health sets them.
attention-when-sleep = From winding down to waking, and naps, as Health sets them.
attention-when-pause = From pressing Pause until you come back.
attention-when-free = From pressing Free time until you come back, the night or midnight.
attention-when-slot = The slots today's plan keeps for you.
attention-when-dnd = While its switch in the status line is on.
attention-when-dnd-focus = While its switch in the status line is on, and while you focus on a task.
attention-also-meeting = During a meeting, Health's notices and “Work hours are over” do not come.
attention-also-areas = What is for another time waits for it: mail to an address for work, a site or an app for work, a work task's dates. Your safe senders' mail and your Always through people come anyway.
attention-also-porch-rests = After the pause, the Porch rests until your next admin hours: new mail is shown there, without a notification.
attention-switch-through-off = Let every call through is off: in the status line, it lets every call ring for a while, but the blocked's.
attention-switch-realtime-on = Real time is on: mail is fetched every minute, and every site's notifications come at once.
attention-switch-realtime-off = Real time is off: in the Porch's menu, it fetches mail every minute and brings every site's notifications at once.
attention-channel-calls-none = None of your phones screens calls yet: these rows apply once one does (Settings ▸ This phone, on the phone).
attention-channel-messages-none = Sioul knows no phone of yours yet: these rows apply on a phone that holds other apps' notifications (Settings ▸ This phone, on the phone).
attention-channel-messages-no-access = None of your phones gives Sioul notification access, as each last said: these rows apply once one does (Settings ▸ This phone, on the phone).
attention-channel-messages-phone = These rows apply on your phone, where Sioul holds other apps' notifications (Settings ▸ This phone, on the phone).
attention-gathered-at = Sioul gathers them at { $times }; the times are in Settings ▸ Reminders.
attention-spam-held = Mail your own spam filter finds maybe or probably spam waits on the Porch, in “Caught by your own spam filter”, without a notification: Mail ⚙ ▸ Your own spam filter.
attention-spam-told = Your own spam filter leaves strangers' mail where it is, told as its row says: Mail ⚙ ▸ Your own spam filter.
attention-mail-areas = Mail to an address for another time waits for that time, but your safe senders' and your Always through people's: each address's “What this address is for”, in Accounts.
attention-site-line = { $name }: { $how }.
attention-site-live = in real time, its notifications come at once
attention-site-muted = silenced, its notifications never come
attention-site-gathered = its notifications come at the gathered times
attention-own-help-list = Each kind of notification Sioul sends. Press one to see what it does at each time, and to change it.
attention-person-list-help = Each list of people on this channel. Press one to see what it does at each time, and to change it.
attention-sheet-always-off-help = Ticked, they come through more than their list lets them, on every device: as each channel's Always through row says, in What reaches you ▸ Exceptions.

## The words Sioul looks for: Settings ▸ Words (docs/words.md)

settings-tab-words = Words
set-words-languages = Languages read
set-words-languages-help = Each language ticked adds its words to every list below. Sioul never ticks one by itself.
set-words-countries = Countries
set-words-countries-help = The names of each country ticked: its public bodies, its banks' approval services, its brands.
set-words-whole = Whole words; capitals and accents do not matter.
set-words-reset = Back to the defaults
words-language-en = English
words-language-en-in = English
words-language-fr = French
words-language-fr-in = French
words-language-de = German
words-language-de-in = German
words-language-es = Spanish
words-language-es-in = Spanish
words-language-it = Italian
words-language-it-in = Italian
words-family-mail = Mail
words-family-money = Money and papers
words-family-phone = The phone
words-family-tasks = Tasks and a public address
words-line-codes = Codes and sign-in links
words-line-codes-help = Mail with these words and a code beside them comes at once, even from an automatic sender; its code is shown, and hidden from AI agents. Take a word away, and such mail waits in its lane.
words-line-senders = Automatic senders
words-line-senders-help = A sender whose address holds one of these words is automatic: its mail is filed with the newsletters. Take a word away, and such mail lands as any other.
words-line-replies = Replies and forwards
words-line-replies-help = A subject that starts with one of these prefixes gets no second one when you answer or forward it; a message's history after these lines is folded away. Take one away, and the prefix doubles, or the history stays open.
words-line-folders = Mail folders
words-line-folders-help = A folder with one of these names, when its server does not say what it is for, is your Sent, Drafts, Junk, Trash or Archive. Take a name away, and that folder is a folder like the others.
words-line-brands = Borrowed names
words-line-brands-help = A sender who shows one of these names from outside its domains is set aside as borrowing it. A name that is also an everyday word or a place counts only alone or beside the words of a service (“Apple Support”), so that “Orange County Library” is someone's own. Take a name away, and its lookalikes reach you.
words-line-payments = Bills and payments
words-line-payments-help = Mail with these words proposes a line for your budgets: a bill to pay, a payment made, money received, an order, a refund. Take a word away, and such mail proposes nothing.
words-line-bank = Your bank's files
words-line-bank-help = Words that name nothing in a movement's label are left out when Sioul matches it with a payment it expects; a move between your own accounts is neither income nor spending. Take a word away, and labels match less often, or a move counts twice.
words-line-letters = Paper letters
words-line-letters-help = A scanned letter is read for who wrote it and what it is: these words decide its sender and its kind, and so its date asked and when it comes forward. Take a word away, and such letters are read as ordinary mail.
words-line-papers = Papers
words-line-papers-help = A file's name or a mail's subject with these words proposes a paper's kind, which sets when it is due again. Take a word away, and you choose the kind yourself.
words-line-contracts = Contracts
words-line-contracts-help = A payment's label, a mail's subject or a sender with these words proposes a contract's kind, which sets its notice and when to look at it again. Take a word away, and you choose the kind yourself.
words-line-approvals = Approvals on the phone
words-line-approvals-help = Another app's notification with these words asks you to approve something now (a sign-in, a payment): it is never held. Take a phrase away, and such a notification waits as the others do.
words-line-calls = Calls in sites
words-line-calls-help = A site's notification with these words is a call ringing: it comes at once, unless the site is silenced; a missed call can wait. Take a word away, and such a notification waits as the others do.
words-line-voicemail = Voicemail by mail
words-line-voicemail-help = Some operators mail you each message left on your voicemail, with its sound. Name the domain your operator's mail comes from: Sioul then shows each message beside the call it followed, to listen to. None is named at first.
words-line-tasks = Your tasks
words-line-tasks-help = Your tasks' categories with these words are never planned, or kept for what you do for joy, or left out of Free time when movement is off; a quick line's first word, or a word after @, says its kind. Take a word away, and it is a word like any other.
words-line-spam = Your provider's spam marks
words-line-spam-help = A mark your provider puts at the start of a subject (“[SPAM]”, “***Possible-SPAM***”) is taken off before your own spam filter reads the message, so that it never learns to copy your provider. A change counts from the filter's next training, on every device at once.
words-line-shield = A public address
words-line-shield-help = Mail to an address you protect is read first: these words make it hostile (set aside without its words shown) or rude, and say what it is about. Take a word away, and it weighs nothing.
words-list-codes-code = Names of a code
words-list-codes-password = A password given
words-list-codes-reset = A password to reset
words-list-codes-link = A link to sign in
words-list-codes-confirm = An address to confirm
words-list-codes-not-yours = No secret of yours (“your zip code”)
words-list-codes-promo = A shop's code, no secret
words-list-senders-automatic = Words in an address
words-list-replies-reply = Reply prefixes (“Re:”)
words-list-replies-forward = Forward prefixes (“Fwd:”)
words-list-quotes-openings = Lines that open a forwarded message
words-list-quotes-wrote = Endings of “… wrote:”
words-list-folders-sent = Sent
words-list-folders-drafts = Drafts
words-list-folders-junk = Junk
words-list-folders-trash = Trash
words-list-folders-archive = Archive
words-list-brands-brands = Brands and services, and their domains
words-list-brands-everyday = Names that are also everyday words or places
words-list-brands-shared = Mail providers millions share
words-named-brands-brands = One to a chip: the name, a colon, then its domains, separated by commas (“My Bank: mybank.example”).
words-list-payments-bill = A bill
words-list-payments-paid = A payment made
words-list-payments-received = Money received
words-list-payments-order = An order
words-list-payments-refund = A refund
words-list-payments-not-payments = Not a payment
words-list-bank-filler = Words that name nothing in a label
words-list-accounts-between = Moves between your own accounts
words-list-letters-senders = Bodies that write to everyone
words-named-letters-senders = One to a chip: the name shown, a colon, then the words its letters carry, separated by commas.
words-list-letters-kinds-formal-notice = A formal notice
words-list-letters-kinds-tax-notice = A tax notice
words-list-letters-kinds-decision = A decision
words-list-letters-kinds-reminder = A reminder
words-list-letters-kinds-appointment = An appointment
words-list-letters-kinds-bill = A bill
words-list-letters-kinds-acknowledgment = An acknowledgment of receipt
words-list-letters-kinds-attestation = An attestation
words-list-letters-kinds-contract = A contract
words-list-papers-kinds-passport = A passport
words-list-papers-kinds-identity = An identity card
words-list-papers-kinds-residence = A residence permit
words-list-papers-kinds-driving = A driving licence
words-list-papers-kinds-health-cover = Complementary health cover
words-list-papers-kinds-health-card = A health insurance card
words-list-papers-kinds-tax-notice = A tax notice
words-list-papers-kinds-rent-receipt = A rent receipt
words-list-papers-kinds-bank-details = Bank details
words-list-papers-kinds-payslip = A payslip
words-list-papers-kinds-warranty = A warranty
words-list-papers-kinds-insurance = An insurance certificate
words-list-papers-kinds-certificate = A certificate
words-list-contracts-kinds-rent = Rent
words-list-contracts-kinds-energy = Energy
words-list-contracts-kinds-health = Health
words-list-contracts-kinds-insurance = Insurance
words-list-contracts-kinds-telecom = Phone and internet
words-list-contracts-kinds-hosting = Hosting and domains
words-list-contracts-kinds-subscription = Subscriptions
words-list-contracts-kinds-bank = Bank
words-list-approvals-phrases = Phrases asking for an approval
words-list-approvals-asks = “Is this you?”
words-list-approvals-asked-about = What it asks about
words-list-approvals-channels = Words in a channel's name
words-list-calls-ringing = A call ringing
words-list-calls-missed = A missed call
words-list-voicemail-operators = Your operator's mail domains
words-list-tasks-optional = Categories never planned
words-list-tasks-joy = What you do for joy
words-list-tasks-movement = Movement and exercise
words-list-capture-kind-verbs-call = A call: the line's first word
words-list-capture-kind-verbs-write = Writing: the line's first word
words-list-capture-kind-verbs-online = Online, a form: the line's first word
words-list-capture-kind-verbs-out = Going out: the line's first word
words-list-capture-kind-verbs-read = Reading: the line's first word
words-list-capture-kind-verbs-think = Thinking it over: the line's first word
words-list-capture-kind-verbs-make = Making: the line's first word
words-list-capture-kind-words-call = A call: a word after @
words-list-capture-kind-words-write = Writing: a word after @
words-list-capture-kind-words-online = Online, a form: a word after @
words-list-capture-kind-words-out = Going out: a word after @
words-list-capture-kind-words-read = Reading: a word after @
words-list-capture-kind-words-think = Thinking it over: a word after @
words-list-capture-kind-words-make = Making: a word after @
words-list-spam-provider-tags = Words of a mark
words-list-shield-threats = Threats
words-list-shield-insults = Insults
words-list-shield-rude = Swearing and contempt
words-list-shield-topics-work = Topic: work
words-list-shield-topics-support = Topic: a question, support
words-list-shield-topics-press = Topic: the press
words-list-shield-topics-thanks = Topic: thanks
words-list-shield-topics-donation = Topic: a donation
words-line-count = { $n ->
    [0] No word yet
    [one] One word, in { $languages }
   *[other] { $Count } words, in { $languages }
}
words-line-added = { $count } added
words-line-removed = { $count } taken away
