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
summary-cases = { $Count } { $n ->
        [one] belongs
       *[other] belong
    } to { $k ->
        [one] a project
       *[other] projects
    }: { $cases }.
summary-case-part = { $title } ({ $count })
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
reason-expires-soon = { $akind } that expires soon
reason-unverified-code = it looks like { $akind }, but the sender is not verified: use it only if you just asked this site for it; fake codes are a phishing trick
reason-case = project { $title }: { $why }
reason-newsletter = a newsletter or a mailing list
reason-automatic = sent by an automatic address
reason-first-message = the first message from this sender
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
account-login = Login: { $login }
account-gmail-hint = Gmail wants an app password here, not your usual one: myaccount.google.com/apppasswords (two-step verification must be on).
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
accounts-tab-senders = Senders
accounts-tab-keys = Encryption
account-service-mail = Mail
account-service-dav = Calendars, tasks and contacts
account-service-google = Google: calendars, contacts and tasks
account-switch-help = Off, it keeps its settings and is neither synced nor shown.
account-on = “{ $id }” is on again.
account-off = “{ $id }” is off: kept with its settings, neither synced nor shown.
account-details = Server and folders
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
senders-help = Who may write to you, and when, whatever the address it writes to: the safe at any hour, the neutral in working hours, the blocked never. Someone blocked from a message lands here too.
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
ui-reasons = Why it is here
ui-attachments = Attachments: { $names }
ui-no-text = This message has no text.
ui-text-safe = Shown safely: nothing remote loads, nothing runs.
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
scan-unavailable = No antivirus answered, so nothing was opened. Install ClamAV: sudo dnf install clamav clamav-update, then sudo freshclam. ({ $detail })
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

## Undo, ten seconds.
undo-archived = Archived.
undo-trashed = Moved to the trash.
undo-deleted = Deleted for good.
undo-junked = Moved to junk.
undo-not-junk = Back in the inbox.
undo-moved = Moved to { $folder }.
undo-many-archived = { $n } messages archived.
undo-many-trashed = { $n } messages moved to the trash.
undo-many-deleted = { $n } messages deleted for good.
undo-many-junked = { $n } messages moved to junk.
undo-many-not-junk = { $n } messages back in the inbox.
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
dav-conflict = Changed on both sides: the server’s version is kept, yours is in { $path }.
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
pgp-made = Your key for { $address } is made; the keyring keeps its passphrase.
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
ui-encryption-note = Sign and encrypt your messages. Your keys stay on this computer, their passphrases in the system keyring; whom you write to gets your key with each message (Autocrypt).
ui-export-key = Save the public key
ui-make-key = Make a key for { $address }
ui-import-key = Import a key…
ui-key-passphrase = The key’s passphrase, if it has one
ui-import = Import
ui-others-keys = Keys of others
pgp-expires = until { $date }
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
task-no-case = Without a project
task-weather-haze-note = Haze: the plan keeps less for today.
task-weather-fog-note = Fog: only small steps are shown. One step is a full day.
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
how-case = Project
how-in-case = In the project
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
task-mode-timeline = Timeline
task-capture-hint = A task in one line: “Call the tax office tomorrow ~15m #project {"{"}date asked{"}"}”
chip-start = from
chip-due = date asked
chip-estimate = minutes
chip-case = project
chip-tag = tag
task-first-list = Tasks live in a task list, on your calendar server or only here. One is made in a click:
task-default-list = Tasks
task-make-list = Make the list
task-all-cases = Every project
task-weather = How is today?
task-weather-clear = Clear
task-weather-haze = Haze
task-weather-fog = Fog
task-start = Start
task-done = Done
task-not-now = Not now
task-open-again = Open again
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
task-by-case = By project
task-by-list = By list
task-show-done = Done too
task-title = Title
task-steps = Steps
task-add-step = A step, in one line
task-waits-title = Waits for
task-waits-add = Waits for… (a task’s title)
task-waits-remove = No longer waits for it
task-frees-one = Frees: { $title }
task-field-start = Can start from
task-field-due = Date asked
task-field-estimate = Takes about
task-field-case = Project
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
set-windows-help = On these days and hours, work can reach you. Outside them, work rests: your own admin and what you enjoy, as the hours below say. The codes and links you ask sites for, and what you send yourself, always come at once.
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
set-quiet-personal-help = Tasks of these categories, and projects marked as yours, are yours outside work: those of leisure (joy, family, friends, leisure) come in free time, the others (health, personal) in any hours of yours. A task with none of these and none of work's is your admin.
set-office-hours = When offices are open
set-office-hours-help = A task marked “Needs an open office” is proposed only then, and planned only on these days.
set-windows-add = Add a window
set-windows-none = No window: the Porch is always open.
set-known = Senders you know
set-known-help = Their mail goes straight to “From people you know”. Anyone else waits in the screener until you let them in. An address, or @domain for everyone there.
set-blocked = Blocked senders
set-blocked-help = Their mail is set aside for good: never shown, never counted. An address, or @domain for everyone there.
set-senders-add = Add an address or @domain
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
set-account-shield-ai-help = An AI (Claude) reads new mail of this address to say its tone and topic, more finely than word lists. The text leaves this computer for that.
set-day-start = The day starts at
set-day-start-help = The hour the day and week planning open on.
set-task-estimate = A task without an estimate counts
set-task-estimate-help = Minutes the plan counts for a task whose length is not said.
set-task-list = New tasks go into
set-task-list-help = The list a task typed in one line goes into.
set-task-list-first = The first list made for tasks
set-case-store = The notes folder
set-case-store-help = Your folder of Markdown files, read as a vault: your notes, and beside them your projects (sioul-cases.toml) and budgets (sioul-budgets.toml). Sioul links to it and never owns it.
set-notes-folder = New notes go into
set-notes-folder-help = A folder inside the notes folder, for notes made from mail and events.
set-reading-family = Font
set-reading-family-help = For long text: notes, mail, a task's notes. Empty: the desktop's.
set-reading-size = Size
set-reading-size-help = The size of long text.
set-reading-spacing = Line spacing
set-reading-spacing-help = Air between lines: 1.5 reads more easily than tight lines.
set-map-geocode = Place contacts on the map
set-map-geocode-help = Their postal addresses are sent to OpenStreetMap's geocoder (Nominatim), once each, one a second; the places found are kept on this computer.
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
set-invoice-name-help = Printed at the top of your invoices.
set-invoice-address = Your address
set-invoice-address-help = On lines, as on an envelope.
set-invoice-siret = SIRET
set-invoice-siret-help = Or the business number where you are; empty until you are registered.
set-invoice-vat = VAT line
set-invoice-vat-help = For a French micro-entrepreneur: « TVA non applicable, art. 293 B du CGI ».
set-invoice-prefix = Invoice numbers start with
set-invoice-prefix-help = Numbers follow each other: 2026-001, 2026-002…
set-invoice-currency = Currency
set-invoice-currency-help = A three-letter code: EUR, USD, CHF.
set-invoice-payment = Payment details
set-invoice-payment-help = Printed at the bottom: IBAN, terms, late fees.
set-invoice-folder = Invoices go into
set-invoice-folder-help = The folder of their PDFs; "Documents/Invoices" in your home when empty.
set-invoice-rate = An hour costs
set-invoice-rate-help = The fee when a project does not set its own.
set-theme = Colours
set-theme-help = Light or dark, or as the system has them. The icons follow at the next start.
set-theme-system = The system's
set-theme-light = Light
set-theme-dark = Dark
set-ai-group = The AI shield
set-ai-key = Key for Anthropic's API
set-ai-key-help = Kept in the system keyring, never in a file. Each new message to a shielded address that allows the AI is sent once to Anthropic (model Claude Haiku), with its subject; the answer (tone, topic, one neutral line) stays on this computer. A key is made at console.anthropic.com.
set-ai-key-kept = A key is kept.
set-ai-key-none = No key yet: the word lists judge alone.
set-ai-key-keep = Keep
set-ai-key-forget = Forget the key
set-mail-threads = By conversation
set-mail-threads-help = A message and its answers together, under the newest; yours come from Sent. Opened with the small arrow on the left.
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
link-kind-case = Projects
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
project-note-changed = Note changed
project-time-noted = { $time } noted
project-invoice = Invoice { $number }
project-no-title = A project needs a name.
project-gone = This project is no longer there.
invoice-nothing = No billable time is waiting to be billed.
invoice-gone = This invoice is no longer there.
invoice-folder-name = Invoices
invoice-budget-line = Invoice { $number }, { $client }
invoice-budget-origin = Invoice { $number }, made by Sioul: expected until paid.
time-no-minutes = How long? Some minutes, at least.
time-no-project-chosen = For which project, or which task?
time-bad-day = Which day? It reads as 2026-10-05.
time-billed-stays = This time is on an invoice: it stays.
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
project-kind-case = Yours
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
set-account-area = What this address is for
set-account-area-help = Its mail comes in the hours for what it is for: work, your admin, leisure, any of them together. Nothing ticked: work, so that it never reaches your evenings. Senders you marked safe, and codes, always come.
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
health-every-day = every day at { $times }
health-every-days = { $days ->
    [2] every other day at { $time }, from { $from }
   *[other] every { $days } days at { $time }, from { $from }
}
health-every-hours = every { $hours } hours, from { $from }
health-next-refill = pharmacy from { $day }
health-next-renew = renew the prescription from { $day }
health-no-name = A name is needed.
health-taken = Taken
health-errand-refill = Pharmacy: { $title }
health-errand-renew = Doctor: renew the prescription for { $title }
health-category = health
health-list = Health
ui-health = Health
health-local = Your medicines, prescriptions and doses stay on this computer. The pharmacy and renewal errands become tasks in the list chosen below, so your phone has them: their titles name the medicine.
health-today = Today
health-today-none = No medicine today.
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
health-moving-help = From when Sioul opens, a notification on your desktop says it is time to move, even with the window hidden. During a focus session, the session itself pauses for a few minutes of moving and stretching; one click starts it again.
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
weather-tip-hours = The weather at { $place }: now, then hour by hour for the next four hours. Click for the days to come, and to change the place.
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
done-button-help = Today’s tasks move to their next day; work rests until it comes back.
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
lane-about-case = Mail that matches this project's routes.
lane-about-public = To { $address }, from someone you have not let in, read first and sorted by topic: work first.
lane-about-people = From senders you let in.
lane-about-screener = From someone new: let them in, or block them.
lane-about-filed = Newsletters and automatic senders, to read when you like.
lane-about-low = From addresses you ranked less important.
lane-about-set-aside = Forged, borrowing a name, spam, or blocked. Nothing is deleted.
lane-about-hostile = Insults, harassment or threats to your public address. Their words stay hidden.
rule-order = Each message goes to the first lane that takes it, in this order: set aside, hostile, codes, projects, what you send yourself, your public address, less important addresses, newsletters, someone new, people you know.
rule-people = Your list of senders you know holds { $n } entries. "Let in", in a message from someone new, adds them.
rule-screener = A sender who is not in your list waits here, unless a project or the newsletter rule takes the message first.
rule-filed = A message that says it comes from a mailing list (List-Id, List-Unsubscribe, Precedence: bulk) is a newsletter. A sender whose address holds one of these words is automatic: { $words }.
rule-low-none = No address is ranked less important yet: an address's rank is set on its card in Accounts.
rule-low = Addresses ranked less important: { $addresses }. Their codes still come at once.
rule-forged = Forged: the sender's domain says it did not send it (DMARC), or every check failed.
rule-borrowed = Borrowing a name: the name shown claims a brand, or your own domain, that the address does not belong to.
rule-spam = Spam: your provider's filter says so.
rule-blocked = Blocked: from your list of blocked senders; such mail is never shown, never counted.
rule-case-none = This project has no route yet: no mail comes here by itself.
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
rule-where-blocked = Blocked senders are in Accounts, under “Senders”.
rule-where-routes = Its routes are set on its page in Projects.
hostile-someone = Someone
hostile-someone-at = Someone at { $domain }
hostile-subject = A message set aside as hostile
ui-lane-help = How mail lands here
set-filed-words = Words of automatic senders
set-filed-words-help = A sender whose address holds one of these words is automatic: its mail is filed with the newsletters.
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
sync-error-google-again = { $account }: Google asks you to sign in again.
sync-error-google-testing = { $account }: Google ended the sign-in after seven days, as it does while your Google project is in testing. In Google Cloud: Google Auth Platform ▸ Audience ▸ Publish app; then sign in again: it lasts after that.
google-no-collections = Google makes no calendar or address book from another program: make it on Google's pages.
google-fixed-collection = Google renames and deletes its calendars and address books on its own pages only.
google-tasks-greyed = Google Tasks does not keep this.
task-limited = This list is on Google Tasks: what Google does not keep is greyed (a start day, a length, a project, a kind, office hours, categories, what it takes, repeating, waiting for another task, steps of steps).

# GitHub, once asked: issues and pull requests as tasks.
set-code-group = Code
set-github = GitHub issues and pull requests as tasks
set-github-help = For those who work on GitHub: what is yours comes into a "GitHub" list on this computer, every thirty minutes, and GitHub's notification mail is tied to it. Nothing is written to GitHub.
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
note-not-here = Not on this computer yet: it may still be syncing (Nextcloud, Dropbox).
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
settings-tab-reminders = Reminders and notifications
settings-tab-files = Your folder and sharing
settings-tab-invoices = Invoices
ui-refresh-all = Refresh everything: mail, agenda, tasks, contacts (F5)
ui-refreshing = Refreshing…
budget-add-budget = Budget
set-look-group = Language and appearance
set-hours-group = Working hours and quiet time
set-hours-elsewhere = Working hours, hours for your admin, free time and days off are in Settings: the sliders icon at the bottom of the left column.
porch-hours-none = Your hours are not set: work, your admin and leisure all come at any hour.
porch-hours-some = Not set yet: { $which }.
porch-hours-why = Each kind of hours brings its own things forward and lets the rest wait: work in working hours, offices and bills in admin hours, nothing but rest in free time.
porch-hours-set = Set my hours
porch-hours-leave = Leave as is
site-column-narrow = Fold the list to its icons
site-column-widen = Show the sites' names
set-senders-group = Who may write to you, and when
set-safe = Safe: at any hour
set-safe-help = Friends, chosen colleagues, chosen family: their mail reaches you in quiet time too. An address, or a pattern with *: *@example.org for everyone there, *@*.example.org for its subdomains. Forged mail never counts as theirs.
set-neutral = Neutral: during working hours
set-neutral-help = Everyone not named elsewhere is neutral: their mail waits for working hours. Name someone here to keep them neutral inside a safe domain.
set-blocked-all = Blocked: never
set-blocked-all-help = Spam and harassment: set aside for good, never shown. The most precise entry wins: an address marked safe stays safe in a domain blocked here. Nobody is blocked for sharing a server or a domain with someone else.
sender-now-safe = { $entry } is safe: their mail reaches you at any hour.
sender-now-neutral = { $entry } is neutral: their mail waits for working hours.
sender-now-blocked = { $entry } is blocked: their mail is set aside for good.
sender-standing = Their mail
sender-safe = Safe: at any hour
sender-neutral = Neutral: during working hours
sender-blocked = Blocked: never
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
health-list-here = on this computer only
scan-ask = No antivirus is installed on this computer, so { $name } will not be checked. Open it only if you trust it. To have files checked: { $hint }
scan-open-anyway = Open it unchecked
scan-save-anyway = Save it unchecked
scan-unavailable-short = No antivirus answered ({ $detail }).
sounds-nature = Nature, made here
sounds-waves = Waves on a beach
sounds-rain = Rain
sounds-wind = Wind in the trees
sounds-crickets = Crickets at night
sounds-storm = A distant storm
watch-title = Your watch
watch-none = Nothing from a watch yet. Choose the folder its files come to (Gadgetbridge's exports, or Garmin's export ZIPs), or plug in your watch: when the desktop shows its GARMIN folder, Sioul reads it. Nothing leaves this computer; no Garmin account is used.
watch-folder = Its files come to
watch-folder-choose = Choose a folder…
watch-offers = Gentle offers between tasks (a pause, a walk)
watch-synced = Last data: { $when }.
watch-steps = { $steps } steps today
watch-resting = Resting heart rate { $bpm } bpm
watch-resting-usual = Resting heart rate { $bpm } bpm (usually { $usual })
watch-slept = Slept { $time }, { $from }–{ $to }
watch-hours = { $h } h { $m }
watch-battery = Body Battery { $level }
watch-week = This week, a night: { $sleep }; a day: { $steps } steps
watch-curves = Today: heart rate, stress as Garmin scores it, Body Battery
watch-imported = { $count ->
        [one] One file from the watch read.
       *[other] { $count } files from the watch read.
    }
watch-offer-move = Five minutes on your feet?
watch-offer-move-text = Before the next one. A walk to the window counts.
watch-offer-pause = Time for a short pause?
watch-offer-pause-text = Stand, look far away, breathe out slowly. Five minutes.
watch-offer-low-reserve = Your reserve is low.
watch-offer-low-reserve-text = Move the rest to tomorrow and call it a day?
watch-offer-low-reserve-action = Done for today
watch-offer-walk = Room for a walk?
watch-offer-walk-text = Twenty minutes, easy, outside, if you like.
watch-morning-short-night = A short night. Shorter sessions today, and the hardest task early, or tomorrow?
watch-morning-strain = Your body may be fighting something. A lighter day?
watch-morning-lighter = A lighter day

## Sharing with your other computers (Parameters)
share-title = Between your computers
share-help = Mail, contacts, the agenda and tasks on a server already reach your other computers, and projects and notes travel with their folder. What Sioul keeps on this computer alone (settings, who may write to you, ties, time, drafts, invoices, health, the watch, lists kept here) can travel too, through a folder your sync carries (Nextcloud, Dropbox, Syncthing), sealed with a passphrase: that folder's server never reads it.
share-off = Not shared: all of it stays on this computer.
share-on = Shared through { $folder }.
share-others = { $count ->
    [one] With one other computer (last news: { $when }).
   *[other] With { $count } other computers (last news: { $when }).
}
share-alone = No other computer yet: on the other one, choose the same folder and type the same passphrase.
share-last = Last exchange here: { $when }.
share-outside = Your projects and notes ({ $store }) do not seem to be in a folder your sync carries: the other computer would not see them. Moved into one (and chosen again in Settings ▸ Your folder and sharing), they travel too.
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
share-passphrase-hint = A few words you will not forget, at least 12 characters, typed once on each computer and kept in its keyring. Never sent anywhere: lost, it cannot be found again (start over with a new folder; nothing here is lost).
share-passphrase-known = Another computer shares through this folder: type the passphrase chosen there.
share-start = Share
share-now = Exchange now
share-stop = Stop sharing
share-short = At least 12 characters: a few words make a good passphrase.
share-differ = The two passphrases differ.
share-wrong = This is not the passphrase chosen on your other computer.
share-no-folder = Choose a folder first.
share-other-seal = A computer in this folder seals with another passphrase: its changes are left aside.
share-unreadable = { $file } could not be read: what came for it waits until it can be.
share-key-missing = Type the passphrase again on this computer: the keyring no longer holds it.
share-not-shared = Not shared: how pages are laid out on this screen, the folders each computer keeps its files in, this computer's browser notices, your own PGP keys (copy them by hand), caches.
share-found = Already shared by your other devices (pick one):
share-files-access = To read the folder your sync app carries (eDrive, Syncthing, FolderSync…), Sioul needs Android's access to your files.
share-files-allow = Allow access to files

## Reminders before dates
reminder-event = { $when } · { $what }
reminder-asked = Asked for { $date }
reminder-wait = The wait after “{ $before }” is over.
reminder-payment = Planned for { $date }.
remind-nothing = Nothing to remind in the next two weeks.
remind-told = told
remind-running = Reminders are already watched on this computer.
remind-watching = Reminders watched: each one comes once, as a quiet notification. Ctrl+C stops.
reminder-open = Open
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
papers-help = The papers asked again and again, each with its file and how long it holds. Kept in your projects' folder (papers/): they travel with it. A reminder comes when one should be renewed.
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
papers-file-missing = Its file is not on this computer yet: it may still be syncing.
papers-plan-renewal = Plan the renewal
papers-renewal-planned = Renewal planned
papers-keep = Keep in papers
papers-keep-help = Checked by the antivirus, then kept in the papers wallet: you say what it is.
papers-kept = { $name } is in your papers: say what it is.
papers-attach = A paper
papers-none-to-attach = No paper with a file yet
papers-no-store = Papers live in your projects' folder: choose it first (Settings ▸ Your folder and sharing).
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
task-why-heavy-fog = A heavy one, on a foggy day: nothing lighter is free. Its first two minutes may be enough.
task-rest-offer = After something heavy, something that gives back:
task-mode-day = The day
day-all-day = All day: { $what }
day-more = { $count ->
    [one] One more step of today does not fit before the end of the day: it keeps its place in the plan.
   *[other] { $count } more steps of today do not fit before the end of the day: they keep their place in the plan.
}
day-empty = Nothing laid out today: no event, no step the plan gives today.
day-now = now
routine-admin = The admin window
routine-admin-porch = The Porch: what came
routine-admin-next = The next step: { $title }
routine-admin-stop = Where you stopped, in a line
routine-no-title = A name, please.
routine-no-steps = One step a line, with its minutes: “10 min Open the Porch”.
routines = Routines
routines-help = Steps played one at a time, the next one said before it comes. One a line, with its minutes: “10 min Open the Porch”, “Make tea 5”.
routines-new = A new routine
routines-play = Play
routines-change = Change
routines-remove = Take out
routines-title = Name
routines-steps = Steps
routines-auto = The next step starts by itself when the time is up
routines-builtin = Made from what is there now: the Porch, the plan's next step, a line to come back to.
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
bank-accounts-help = Where your money actually is: a current account, PayPal, Stripe. Each takes its own exports, fills the budgets you choose, and is topped up by your reserves in your order. Until one is declared, an export taken in only feeds the watch below. Kept in your projects' folder (sioul-budgets.toml, sioul-bank.toml); nothing is sent anywhere.
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
set-letters-inbox-help = A folder where you, a scanner or a helper puts the post as PDF or photos (a phone's scanner app syncing there works too). Each scan is read on this computer (Tesseract, Poppler) and waits for the Porch's window as a card: who, what, how much, by when.
password-show = Show the password
password-hide = Hide the password
set-passwords-shown = Show passwords as you type
set-passwords-shown-help = Every password, passphrase and key field shows what you type from the start, on this computer; the eye at the end of each field shows or hides it at any time.
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
bitwarden-choose-site = For { $site }:
bitwarden-choose-none = Bitwarden keeps no login for { $site }: search them all.
bitwarden-choose-search = Search all your logins
bitwarden-choose-found = In your vault:
bitwarden-choose-nothing = Nothing found.
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
health-missed-question = Due while Sioul was closed on all your computers: did you take them?
health-not-taken = Not taken
health-alone = These doses are known to this computer only. If Sioul also runs on another computer, share between them (Settings ▸ Your folder and sharing): a dose marked on one then counts on all, and only the one you are at reminds you.
health-unchecked = Your other computers were last heard from at { $time }: a dose marked there may not show here yet.
health-unchecked-yet = Your other computers are not heard from yet: a dose marked there may not show here yet.
health-reminded-there = Reminders come on { $computer }, the computer you are at.
invoice-unchecked = Your sharing folder cannot be written now: your other computers would not know of this invoice. Try again once it is reachable.
invoice-elsewhere = Invoices are numbered on { $computer }: one computer at a time, so that a number is never given twice.
invoice-take = Make invoices on this computer
invoice-settling = In a minute and a half: your other computers first learn that invoices are made here.
invoice-others-silent = Another computer has not been heard from for a few minutes: its sync may be late. Invoices wait until it is heard from, so that no number is given twice.
invoice-here = Invoices are made on this computer from now on.
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
mode-leisure = Free time until { $until }: rest, and what you enjoy.
mode-admin-leisure = Admin and free time until { $until }: offices and bills, and what you enjoy.
set-windows-admin = Hours for your admin
set-windows-admin-help = Your own admin comes forward then: offices, bills, letters, health errands. Once these are set, admin no longer comes in working hours, except calls to an office, which keep office hours.
set-windows-leisure = Free time
set-windows-leisure-help = Rest and leisure only: friends, family, chats, what you enjoy; neither work nor admin. Hours set for nothing are personal time: admin or leisure as you like, never work.
set-reminders-gather = Sites' notifications gathered
set-reminders-gather-help = What your sites notify waits, then comes in one notification at the times below, for the sites of those hours. A site in real time, and a call, come at once.
set-reminders-gathered = Gathered at
set-reminders-gathered-help = Times of the day, as 09:00: three a day helped most in a field trial.
sites-gathered = Your sites have news
sites-gathered-open = Open the Porch
site-microphone = Microphone, for calls
site-camera = Camera, for calls
site-screen = Sharing the screen, for calls
site-devices = Devices for calls
site-devices-help = The camera, microphone and speaker calls in every site use, from your system's list. A site that lets you choose in its own settings keeps its choice.
site-device-camera = Camera
site-device-microphone = Microphone
site-device-speaker = Speaker
site-device-system = The system's own
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
collection-here = on this computer only
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
budget-line-gone = This line of the budgets is no longer there.
mail-no-such-folder = { $account } has no folder named “{ $folder }”; sioul mail folders { $account } lists them.
mail-not-in-accounts = { $file } is not in the mail of one of your accounts.
mail-move-needs-to = Which folder? Name it with --to.
list-read-only-mark = (read only)
import-bad-key = “{ $key }” cannot be a key: keys name files, so no “/”, “\” or “:” in them.
antivirus-hint-windows = Windows Security ▸ Virus & threat protection
antivirus-hint-packages = ClamAV, from your system's packages
ocr-hint-windows = Tesseract (github.com/UB-Mannheim/tesseract) and Poppler
ocr-hint-packages = Tesseract and Poppler, from your system's packages
google-page-granted = Sioul has the access. You can close this tab.
google-page-denied = Google gave no access: nothing changed. You can close this tab.
google-page-foreign = This answer was not for Sioul. You can close this tab.
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
