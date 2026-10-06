## Sioul, en français.
## Les variables : $n est un nombre pour choisir le pluriel ; $count / $Count
## ce nombre en lettres (minuscule, majuscule) ; $countf / $Countf au féminin.

## Les nombres en lettres (0 à 12 ; en chiffres au-delà).
count-0 = aucun
    .cap = Aucun
    .f = aucune
    .fcap = Aucune
count-1 = un
    .cap = Un
    .f = une
    .fcap = Une
count-2 = deux
    .cap = Deux
count-3 = trois
    .cap = Trois
count-4 = quatre
    .cap = Quatre
count-5 = cinq
    .cap = Cinq
count-6 = six
    .cap = Six
count-7 = sept
    .cap = Sept
count-8 = huit
    .cap = Huit
count-9 = neuf
    .cap = Neuf
count-10 = dix
    .cap = Dix
count-11 = onze
    .cap = Onze
count-12 = douze
    .cap = Douze

decimal-separator = ,

## Ce qui est arrivé, quand le Porche ouvre.
summary-nothing = Rien n’est arrivé. Rien n’a besoin de vous.
summary-total = { $Countf } { $n ->
        [one] lettre est arrivée
       *[other] lettres sont arrivées
    }.
summary-codes = { $Count } { $n ->
        [one] code est prêt
       *[other] codes sont prêts
    } ci-dessus.
summary-cases = { $Countf } { $n ->
        [one] concerne
       *[other] concernent
    } { $k ->
        [one] un projet
       *[other] des projets
    } : { $cases }.
summary-case-part = { $title } ({ $countf })
summary-people = { $Countf } { $n ->
        [one] vient de quelqu’un que vous connaissez
       *[other] viennent de personnes que vous connaissez
    }.
summary-screener = { $Countf } { $n ->
        [one] vient d’un nouvel expéditeur et attend votre accord
       *[other] viennent de nouveaux expéditeurs et attendent votre accord
    }.
summary-filed = { $Countf } { $n ->
        [one] lettre d’information ou notification a été rangée
       *[other] lettres d’information et notifications ont été rangées
    }.
summary-set-aside = { $Count } { $n ->
        [one] message falsifié ou indésirable a été mis
       *[other] messages falsifiés ou indésirables ont été mis
    } de côté.

## Pourquoi un message est là où il est.
reason-dmarc-pass = le domaine de l’expéditeur s’en porte garant (DMARC valide)
reason-dmarc-fail = le domaine de l’expéditeur dit ne pas l’avoir envoyé (DMARC en échec)
reason-dkim-pass = il porte une signature valide (DKIM valide)
reason-both-failed = SPF et DKIM ont tous deux échoué
reason-nothing-proves = rien ne prouve qui l’a envoyé
reason-no-results = votre fournisseur n’a pas laissé de résultats d’authentification
reason-forged = falsifié : mis de côté
reason-spam = marqué comme indésirable par le { $source } de votre fournisseur
reason-spam-score = marqué comme indésirable par le { $source } de votre fournisseur, score { $score }
reason-expires-soon = { $akind } qui expire bientôt
reason-unverified-code = on dirait { $akind }, mais l’expéditeur n’est pas vérifié : ne vous en servez que si vous venez de le demander à ce site ; les faux codes sont une ruse d’hameçonnage
reason-case = projet { $title } : { $why }
reason-newsletter = une lettre d’information ou une liste de diffusion
reason-automatic = envoyé par une adresse automatique
reason-first-message = le premier message de cet expéditeur
reason-known = de quelqu’un que vous connaissez
reason-from-yourself = envoyé depuis l’une de vos adresses
route-sender-domain = domaine de l’expéditeur : { $value }
route-sender = expéditeur : { $value }
route-subject = objet : { $value }
route-text = texte : { $value }
route-attachment = pièce jointe : { $value }
route-thread = dans une conversation de ce projet

## Les secrets de courte durée.
code-kind-code = code
    .cap = Code
    .a = un code
code-kind-password = mot de passe
    .cap = Mot de passe
    .a = un mot de passe
code-kind-reset = réinitialisation de mot de passe
    .cap = Réinitialisation de mot de passe
    .a = une réinitialisation de mot de passe
code-kind-link = lien de connexion
    .cap = Lien de connexion
    .a = un lien de connexion
code-kind-confirm = lien de confirmation
    .cap = Lien de confirmation
    .a = un lien de confirmation

## Jusqu’où croire l’expéditeur.
trust-verified = vérifié
trust-unverified = non vérifié
trust-forged = falsifié

## Le Porche.
right-now-title = Tout de suite
right-now-item = { $Kind } de { $sender } ({ $trust })
right-now-item-code = { $Kind } de { $sender } ({ $trust }) : { $code }
right-now-valid = valable environ { $minutes } minutes
right-now-valid-hours = { $hours ->
    [one] valable environ une heure
   *[other] valable environ { $hours } heures
}
right-now-valid-days = { $days ->
    [one] valable environ un jour
   *[other] valable environ { $days } jours
}
lane-people = De personnes que vous connaissez
lane-screener = Nouveaux expéditeurs, en attente de votre accord
lane-filed = Rangé : lettres d’information et notifications
lane-set-aside = Mis de côté
porch-closed = Le Porche ouvre { $when }. D’ici là, ce qui arrive est vérifié et trié.
porch-open-hint = (`sioul porch --open` l’affiche maintenant.)
no-date = sans date

## Les fenêtres administratives.
window-open-until = Un créneau pour vos démarches est ouvert jusqu’à { $time }.
window-next = Le prochain créneau pour vos démarches ouvre { $when }.
window-none = Aucune fenêtre dans la semaine qui vient.

## Les erreurs.
error-no-mail = Pas encore de courrier à lire : indiquez --maildir, ou des comptes avec un maildir dans la configuration (examples/config.toml).
error-no-store = Pas de dossier de notes : indiquez --store, ou case_store dans la configuration.
error-no-windows = Pas de fenêtres administratives dans la configuration (examples/config.toml).

## Les dates.
when-on = le { $date }
date-long = { $weekday } { $day ->
        [1] 1er
       *[other] { $day }
    } { $month } à { $time }
date-short = { $weekday } { $day ->
        [1] 1er
       *[other] { $day }
    } { $month } { $time }
weekday-1 = lundi
    .short = lun.
weekday-2 = mardi
    .short = mar.
weekday-3 = mercredi
    .short = mer.
weekday-4 = jeudi
    .short = jeu.
weekday-5 = vendredi
    .short = ven.
weekday-6 = samedi
    .short = sam.
weekday-7 = dimanche
    .short = dim.
month-1 = janvier
    .short = janv.
month-2 = février
    .short = févr.
month-3 = mars
    .short = mars
month-4 = avril
    .short = avr.
month-5 = mai
    .short = mai
month-6 = juin
    .short = juin
month-7 = juillet
    .short = juil.
month-8 = août
    .short = août
month-9 = septembre
    .short = sept.
month-10 = octobre
    .short = oct.
month-11 = novembre
    .short = nov.
month-12 = décembre
    .short = déc.

## L’argent : les milliers groupés, le signe devant.
thousands-separator = {"\u202F"}
money-positive = { $value }{"\u00A0"}€
money-negative = −{ $value }{"\u00A0"}€

## Les budgets et les réserves, d’un coup d’œil.
month-year = { $month } { $year }
date-day = { $weekday } { $day ->
        [1] 1er
       *[other] { $day }
    } { $month }
date-day-month = { $day ->
        [1] 1er
       *[other] { $day }
    } { $month }
budgets-title = Budgets, { $month }
reserves-title = Réserves
period-month = mensuel
period-year = annuel
budget-line = { $title } ({ $period }) : { $so_far } jusqu’ici, { $projected } attendus au { $end }, objectif { $target } : { $verdict }.
verdict-better = mieux que prévu, de { $gap }
verdict-as-planned = comme prévu
verdict-short = en dessous du prévu de { $gap }
budget-earmarked = dont { $amount } mis de côté pour ce qui est prévu plus tard.
budget-opening = { $opening } reportés d’avant.
budget-drawn = { $reserve } couvre { $amount } le { $end }.
budget-swept = { $amount } vont à { $reserve } le { $end }.
reserve-line = { $title } : { $balance } aujourd’hui. Ce mois-ci, { $month_done } utilisés et { $month_planned } encore prévus. Au { $year_end_date }, { $year_end }.
reserve-quiet = Rien n’est prévu depuis cette réserve.
mail-title = Proposé par le courrier
mail-line = { $date } · { $budget } · { $amount } · { $label }
mail-line-foreign = { $date } · { $budget } · { $amount } { $currency }, à convertir · { $label }
mail-added = ajouté
error-no-ledger = Pas encore de budgets : indiquez --file, ou placez sioul-budgets.toml à la racine de votre dossier de notes (examples/sioul-budgets.toml).

## Les comptes.
security-tls = chiffré dès le départ (TLS)
security-starttls = chiffré par STARTTLS
account-server = { $host }, port { $port }, { $security }
account-found = Trouvé : { $server }, { $by }.
account-by-provider = d’après les réglages publiés par votre fournisseur
account-by-ispdb = d’après la liste de fournisseurs de Thunderbird
account-by-guess = une supposition : vérifiez-la avant de continuer
account-login = Identifiant : { $login }
account-gmail-hint = Gmail demande ici un mot de passe d’application, pas votre mot de passe habituel : myaccount.google.com/apppasswords (la validation en deux étapes doit être activée).
account-app-password-hint = Si ce compte utilise la validation en deux étapes, il peut demander un mot de passe d’application.
account-password-prompt = Mot de passe pour { $address } (il va dans le trousseau de votre système, nulle part ailleurs) :
account-connected = Connexion réussie : la boîte de réception contient { $n ->
        [one] un message
       *[other] { $count } messages
    }.
account-added = Ajouté sous le nom « { $id } ». Son courrier va dans { $path }.
account-exists = Un compte nommé « { $id } » existe déjà.
account-address-exists = { $address } est déjà configurée, sous le nom « { $id } ».
account-unknown = Aucun compte nommé « { $id } ».
account-removed = « { $id } » retiré, avec son mot de passe. Son courrier reste dans { $path }.
account-portal-added = « { $id } » ajouté : il ouvre { $url } dans votre navigateur.
account-password-saved = Mot de passe gardé dans le trousseau de votre système.
account-has-password = mot de passe dans le trousseau
account-no-password = pas encore de mot de passe : `sioul account password { $id }`
account-trust = Vérification des expéditeurs : fait confiance à { $ids }.
account-trust-learning = Vérification des expéditeurs : apprise de votre courrier à la première relève.
account-portal = ouvre { $url }

## La relève du courrier.
sync-new = { $account } : { $n ->
        [0] rien de nouveau
        [one] un nouveau message
       *[other] { $count } nouveaux messages
    }.
sync-first = { $account } : { $n ->
        [0] aucun message
        [one] un message
       *[other] { $count } messages
    } des { $days } derniers jours.
sync-learned = { $account } : votre fournisseur consigne ses vérifications sous le nom { $id } ; Sioul fait désormais confiance à ce nom pour ce compte.
sync-not-learned = { $account } : votre fournisseur ne semble pas consigner ses vérifications, les expéditeurs restent donc « non vérifiés » pour l’instant.
sync-error-address = Ce n’est pas une adresse électronique.
sync-error-not-found = Aucun réglage trouvé pour { $detail } : indiquez le serveur vous-même (--host).
sync-error-no-server = { $account } : pas de serveur dans la configuration.
sync-error-no-password = { $account } : pas encore de mot de passe dans le trousseau.
sync-error-keyring = { $account } : le trousseau du système n’a pas répondu ({ $detail }).
sync-error-network = { $account } : le serveur est injoignable ({ $detail }).
sync-error-tls = { $account } : le certificat du serveur n’a pas pu être vérifié, rien n’a donc été envoyé ({ $detail }).
sync-error-login = { $account } : le serveur a refusé le mot de passe ({ $detail }).
sync-error-app-password = { $account } : ce serveur demande un mot de passe d’application, créé dans les réglages de votre compte chez le fournisseur, et non votre mot de passe habituel. Gmail : myaccount.google.com/apppasswords.
sync-error-server = { $account } : le serveur a refusé une commande ({ $detail }).
sync-error-disk = { $account } : le courrier n’a pas pu être écrit sur le disque ({ $detail }).
sync-nothing = Rien à relever pour l’instant : les comptes s’ajoutent dans Comptes, en bas à gauche.
watch-started = Surveillance de { $accounts }. Les codes et les liens que vous demandez aux sites arrivent en notification. Ctrl+C arrête.
watch-line = { $time } · { $line }
done-closed = C’est fait. Ce qui arrive maintenant attend la prochaine fenêtre.
done-nothing = Rien n’était affiché, donc rien à fermer.
notify-copy = Copier le code

## La fenêtre.
ui-porch = Porche
ui-budgets = Budgets
ui-accounts = Comptes
accounts-tab-yours = Vos comptes
accounts-tab-add = Ajouter un compte
accounts-tab-senders = Expéditeurs
accounts-tab-keys = Chiffrement
account-service-mail = Courrier
account-service-dav = Agendas, tâches et contacts
account-service-google = Google : agendas, contacts et tâches
account-switch-help = Éteint, il garde ses réglages et n’est ni synchronisé ni montré.
account-on = « { $id } » est rallumé.
account-off = « { $id } » est éteint : gardé avec ses réglages, ni synchronisé ni montré.
account-details = Serveur et dossiers
account-settings = Réglages de cette adresse
scout-button = Ce que ce serveur offre
scout-asking = Le serveur est interrogé…
scout-title = Ce que { $server } offre
scout-mail = Courrier : { $server }
scout-have = déjà là
scout-mail-add = Ajouter le courrier, avec le mot de passe de ses agendas
scout-mail-add-other = L’ajouter avec un autre mot de passe
scout-dav = Agendas, tâches et contacts : { $server }
scout-dav-add = Les ajouter
scout-nextcloud = { $product } { $version }
scout-apps = Ses applications :
scout-apps-unasked = Ses applications sont listées une fois ses agendas ici : leur mot de passe les demande.
scout-not-in-sioul = pas encore dans Sioul
scout-in-sioul = dans Sioul
scout-nothing = Le serveur n’en dit pas plus.
scout-app-calendars-contacts = Agendas, tâches et contacts
scout-app-files = Fichiers
scout-app-notes = Notes
scout-app-talk = Talk (appels et discussions)
scout-app-deck = Deck (tableaux)
scout-app-webmail = Son webmail
scout-app-bookmarks = Marque-pages
scout-app-tables = Tables
scout-app-forms = Formulaires
scout-app-photos = Photos
senders-help = Qui peut vous écrire, et quand, quelle que soit l’adresse où il écrit : quatre listes, chacune avec les moments où son courrier vient. Quelqu’un bloqué depuis un message arrive ici aussi.
ui-sync-now = Relever le courrier
ui-syncing = Relève du courrier…
ui-synced-at = Courrier relevé à { $time }.
ui-open-anyway = L’ouvrir quand même
ui-not-open = L’ouvrir hors fenêtre montre tout ce qui attend. Les codes s’affichent toujours.
ui-done = Terminé pour l’instant
ui-copy = Copier
ui-copied = Copié.
ui-let-in = Accepter cette adresse
ui-let-in-done = { $address } est acceptée.
ui-close = Fermer
ui-ok = OK
ui-reasons = Pourquoi il est ici
ui-attachments = Pièces jointes : { $names }
ui-no-text = Ce message n’a pas de texte.
ui-text-safe = Affiché sans risque : rien de distant ne se charge, rien ne s’exécute.
ui-add-account = Ajouter un compte de courrier
ui-address = Adresse électronique
ui-find-server = Trouver le serveur
ui-finding = Recherche du serveur…
ui-host = Serveur
ui-port = Port
ui-security = Chiffrement
ui-login = Identifiant
ui-password = Mot de passe
ui-password-note = Le mot de passe va dans le trousseau de votre système, nulle part ailleurs.
ui-app-passwords = Créer un mot de passe d’application
ui-connect = Se connecter et ajouter
ui-connecting = Connexion…
ui-remove = Retirer
ui-remove-confirm = Retirer « { $id } » ? Son mot de passe quitte le trousseau ; son courrier reste sur le disque et sur le serveur.
ui-cancel = Annuler
ui-portal-name = Nom
ui-portal-url = Adresse web
ui-add = Ajouter
ui-open = Ouvrir
ui-no-accounts = Pas encore de compte : ajoutez le premier dans « Ajouter un compte ».
ui-watching = En attente de nouveau courrier.
ui-no-budgets = Pas encore de budgets : placez sioul-budgets.toml à la racine de votre dossier de notes.
ui-keys = Ctrl+1 à Ctrl+0 lieux · Ctrl+N nouveau · Ctrl+Z annuler · F5 actualiser

## Les budgets, détaillés.
fig-so-far = Jusqu’ici
fig-expected = Attendu au { $end }
fig-target = Objectif
fig-opening = Report
fig-earmarked = Mis de côté pour plus tard
badge-better = Mieux que prévu de { $gap }
badge-as-planned = Comme prévu
badge-short = En dessous du prévu de { $gap }
fig-today = Aujourd’hui
fig-delay = Arrive en
fig-this-month = Ce mois-ci
fig-month-flows = { $done } utilisés, { $planned } prévus
fig-by-date = Au { $date }
fig-pace = À ce rythme
fig-pace-quiet = rien n’est prévu dessus
mail-section = Depuis le courrier
mail-none = Pas encore de courrier sur de l’argent.
kind-received = Reçu
kind-paid = Payé
kind-order = Commande
kind-bill = Facture
kind-refund = Remboursement
note-recorded = Compté dans { $budget }
note-duplicate = Même paiement que { $other }
note-preset = Compté par le prévu « { $preset } »
note-preset-amount = Remplace le prévu « { $preset } » pour son mois une fois ajouté
note-no-amount = Le message ne donne pas de montant.
note-doubtful = D’un expéditeur nouveau et non vérifié : vérifiez qu’il est authentique avant de le compter.
ui-add-line = Ajouter
ui-not-payment = Pas un paiement
ui-line-added = Ajouté à { $budget }.
ui-line-ignored = Laissé hors des budgets.

## Les blocages et les noms empruntés.
reason-blocked = vous avez bloqué cet expéditeur
reason-impersonation = il prend le nom { $brand } mais vient de { $domain } : mis de côté
ui-block = Bloquer…
ui-block-title = Bloquer cet expéditeur ?
ui-block-text = Son courrier sera mis de côté pour de bon : jamais affiché, jamais compté. Rien n’est supprimé, et vous pouvez le débloquer dans Comptes.
ui-block-address = Bloquer { $address }
ui-block-domain = Bloquer tout { $domain }
ui-blocked = { $entry } est bloqué.
ui-blocked-title = Expéditeurs bloqués
ui-unblock = Débloquer
ui-unblocked = { $entry } est débloqué.
blocked-none = Personne n’est bloqué.

## Les comptes et les messages, détaillés.
account-row-server = Serveur
account-row-checks = Vérification des expéditeurs
account-row-mail = Courrier gardé dans
account-row-web = Adresse web
account-checks-learning = apprise de votre courrier ; votre fournisseur n’en consigne peut-être aucune
ui-to = À
ui-cc = Cc
ui-attachments-title = Pièces jointes
ui-save = Enregistrer
ui-saved = Enregistré dans { $path }.
ui-attachments-closed = Mis de côté : ses pièces jointes ne s’ouvrent pas.
ui-quote-show = Afficher le message cité ({ $n ->
        [one] une ligne
       *[other] { $n } lignes
    })
ui-quote-hide = Masquer le message cité
size-b = { $n } o
size-kb = { $n } Ko
size-mb = { $n } Mo

## La priorité des comptes.
lane-low = Comptes moins importants
summary-low = { $Countf } { $n ->
        [one] lettre d’un compte moins important attend
       *[other] lettres de comptes moins importants attendent
    }, repliées en bas.
reason-low-priority = il vient d’un compte que vous avez classé sous les autres
priority-above = Plus important
priority-average = Normal
priority-below = Moins important
account-row-priority = Priorité
ui-priority-set = { $id } : { $priority }.

## Les vérifications des expéditeurs.
verify-done = { $account } : { $n ->
        [0] rien à vérifier
        [one] un message vérifié
       *[other] { $count } messages vérifiés
    }.
reason-dmarc-fail-no-policy = DMARC échoue, mais le domaine de l’expéditeur ne demande rien quand c’est le cas
reason-dmarc-fail-list = DMARC échoue sur une liste de diffusion, qui réécrit ce qu’elle relaie
checks-by-sioul = Vérifié par Sioul à l’arrivée du message :
checks-by-provider = Vérifié par votre fournisseur ({ $id }) :
checks-none = Aucune vérification consignée : rien ne prouve qui l’a envoyé.
check-line = { $protocol } : { $outcome }
check-line-domain = { $protocol } : { $outcome } ({ $domain })
check-pass = valide
check-fail = échoue
check-softfail = douteux
check-neutral = neutre
check-none = absent
check-temperror = impossible à vérifier pour l’instant
check-permerror = mal configuré par l’expéditeur
check-other = incertain
check-policy = politique { $policy }
protocol-iprev = DNS inverse

## L’antivirus.
ui-attachments-scan = Chaque pièce jointe est vérifiée par l’antivirus (ClamAV) avant de s’ouvrir ou d’être enregistrée.
scan-running-any = Vérification de la pièce jointe par l’antivirus…
scan-running = Vérification de { $name } par l’antivirus…
scan-clean = { $name } : aucune menace trouvée ; ouverture.
scan-infected = { $name } : l’antivirus a trouvé { $threat }. Rien n’a été ouvert, et la copie est effacée.
scan-unavailable = Aucun antivirus n’a répondu, donc rien n’a été ouvert. Installez ClamAV : sudo dnf install clamav clamav-update, puis sudo freshclam. ({ $detail })
scan-error = La pièce jointe { $name } n’a pas pu être vérifiée ({ $detail }).

## La lecture.
ui-images-hidden = Les images ne sont pas affichées : rien ne se charge depuis le réseau.
ui-link = Lien : { $url }
ui-quote-history = Afficher les messages précédents

## Les dossiers.
folder-inbox = Boîte de réception
folder-drafts = Brouillons
folder-sent = Envoyés
folder-archive = Archives
folder-junk = Indésirables
folder-trash = Corbeille
folder-all = Tout le courrier
folder-other = Dossier

## L’écriture.
compose-no-recipient = Personne à qui l’envoyer pour l’instant : ajoutez une adresse.
compose-from = De
compose-date = Date
compose-subject = Objet
compose-to = À
compose-cc = Cc
compose-forwarded = ---------- Message transféré ----------
compose-attribution = Le { $date }, { $name } a écrit :

## L’envoi.
send-error-refused = { $account } : le serveur d’envoi a refusé le message ({ $detail }).
send-error-not-filed = { $account } : envoyé, mais sa copie n’a pas pu être rangée dans les Envoyés ({ $detail }).
send-error-message = { $detail }

## Le client de courrier.
mail-done = C’est fait.
mail-sent = Envoyé.
mail-you-to = Vous, à { $names }
mail-to-whom = À { $names }
mail-nobody = personne encore
mail-no-subject = (sans objet)
mail-empty = Rien ici.
mail-nothing-recent = Rien ces deux dernières semaines.
mail-not-found = Rien ne correspond à « { $query } ».
mail-unread = { $n ->
    [one] Un message que vous n’avez pas lu.
   *[other] { $Count } messages que vous n’avez pas lus.
}

## La page du courrier.
ui-mail = Courrier
ui-write = Écrire
ui-undo = Annuler
ui-reply = Répondre
ui-reply-all = Répondre à tous
ui-forward = Transférer
ui-archive = Archiver
ui-trash = Supprimer
ui-delete-for-good = Supprimer définitivement
ui-junk = Indésirable
ui-not-junk = Pas indésirable
ui-more = Plus
ui-mark-read = Marquer comme lu
ui-mark-unread = Marquer comme non lu
ui-flag = Suivre
ui-unflag = Ne plus suivre
ui-flagged = Suivi
ui-selected = { $n ->
    [one] Un sélectionné
   *[other] { $n } sélectionnés
}
ui-clear-selection = Tout désélectionner
ui-drag-count = { $n ->
    [one] Un message
   *[other] { $n } messages
}
ui-move-to = Déplacer vers…
ui-move-title = Déplacer vers quel dossier ?
ui-show-source = Voir la source
ui-drafts-here = Brouillons
ui-more-folders = Autres dossiers
ui-no-mail-accounts = Pas encore de compte de courrier : ajoutez-en un dans Comptes.
ui-folders = Revenir aux dossiers
ui-search = Chercher
ui-earlier = Messages plus anciens
ui-discard = Supprimer le brouillon

## La fenêtre d’écriture.
ui-to-placeholder = Adresses, séparées par des virgules
ui-cc-bcc = Cc, Cci
ui-bcc = Cci
ui-bcc-note = Invisibles des autres destinataires
ui-attach = Joindre
ui-preview = Aperçu
ui-edit = Modifier
ui-markdown-hint = Markdown : **gras**, *italique*, - listes, [lien](https://…)
ui-send = Envoyer
ui-writing = Nom et signature…
ui-your-name = Votre nom, tel que les destinataires le voient
ui-signature = Signature
ui-signature-note = En Markdown ; elle s’ajoute sous ce que vous écrivez.
ui-writing-saved = { $id } : nom et signature enregistrés.
compose-title-new = Nouveau message
compose-title-reply = Réponse à { $names }
compose-title-forward = Transfert
compose-saved = Enregistré à { $time }.
compose-below-quote = Sous votre texte : le message de { $name } du { $date }, cité.
compose-below-forward = Sous votre texte : le message de { $name } du { $date }, avec ses pièces jointes.
compose-kept = Le brouillon est gardé dans Brouillons.
compose-not-a-file = { $path } n’est pas un fichier.
compose-bad-address = « { $entry } » n’est pas une adresse.

## Annuler, dix secondes.
undo-archived = Archivé.
undo-trashed = Mis à la corbeille.
undo-deleted = Supprimé définitivement.
undo-junked = Mis dans les indésirables.
undo-not-junk = De retour dans la boîte de réception.
undo-moved = Déplacé vers { $folder }.
undo-many-archived = { $n } messages archivés.
undo-many-trashed = { $n } messages mis à la corbeille.
undo-many-deleted = { $n } messages supprimés définitivement.
undo-many-junked = { $n } messages mis dans les indésirables.
undo-many-not-junk = { $n } messages de retour dans la boîte de réception.
undo-many-moved = { $n } messages déplacés vers { $folder }.
undo-moved-across = { $n ->
    [one] Déplacé
   *[other] { $n } messages déplacés
} vers { $folder } de { $account }.
undo-sending = Envoi dans dix secondes.
undo-discarded = Brouillon supprimé.
undo-done = Annulé.
undo-send-undone = Pas envoyé : le brouillon est de nouveau ouvert.
undo-too-late = Trop tard pour annuler : c’est fait.
mail-message-gone = Ce message n’est plus là ; relevez le courrier.
mail-draft-gone = Ce brouillon n’est plus là.
mail-not-sent = Pas envoyé ; le brouillon attend dans Brouillons. { $detail }
mail-no-account = Aucun compte de courrier pour envoyer : ajoutez-en un dans Comptes.

## Contacts et agendas.
agenda-today = Aujourd’hui
agenda-tomorrow = Demain
agenda-all-day = Toute la journée
agenda-from = À partir de { $time }
agenda-until = Jusqu’à { $time }
agenda-untitled = (sans titre)
agenda-nothing = Rien de prévu ces deux prochaines semaines.
contacts-none = Pas encore de contact.
account-row-dav = Contacts et agendas
dav-found = Contacts : { $contacts } · Agendas : { $calendars }
dav-none = aucun
dav-added = Compte « { $id } » ajouté : ses contacts et agendas arrivent.
dav-report = { $account } : { $sent } envoyés, { $received } reçus, { $removed } retirés.
dav-conflict = Modifié des deux côtés : la version du serveur est gardée, la vôtre est dans { $path }.
dav-nothing = Pas encore de compte de contacts et agendas : sioul dav add vous@exemple.org.
dav-no-book = Pas encore de carnet d’adresses où l’ajouter.
dav-no-calendar = Pas encore d’agenda où l’ajouter.
contact-no-name = Un contact a besoin d’un nom ou d’une adresse.
contact-saved = Contact enregistré.
event-no-title = Un événement a besoin d’un titre.
event-saved = Événement enregistré.
undo-contact-deleted = Contact supprimé.
undo-event-deleted = Événement supprimé.
undo-occurrence-skipped = Cette fois-ci est retirée.
agenda-gone = Cet événement n’est plus là.
dav-synced-at = Contacts et agendas synchronisés à { $time }.
invitation-answering = Envoi de votre réponse…
invitation-accepted = Accepté : il est dans votre agenda, et votre réponse est envoyée.
invitation-tentative = Peut-être : il est dans votre agenda, et votre réponse est envoyée.
invitation-declined = Décliné : votre réponse est envoyée.
invitation-add = Ajouté à votre agenda.
invitation-remove = Retiré de votre agenda.
invitation-subject-accepted = Accepté : { $summary }
invitation-subject-tentative = Peut-être : { $summary }
invitation-subject-declined = Décliné : { $summary }
invitation-sentence-accepted = { $name } accepte : { $summary }.
invitation-sentence-tentative = { $name } viendra peut-être : { $summary }.
invitation-sentence-declined = { $name } décline : { $summary }.

## L’agenda et les contacts dans la fenêtre.
ui-agenda = Agenda
ui-contacts = Contacts
ui-new-contact = Nouveau contact
ui-new-event = Nouvel événement
ui-edit-event = Modifier l’événement
ui-call = Appeler
ui-delete = Supprimer
ui-more-details = Plus
ui-open-contact = Ouvrir le contact
ui-add-contact = Ajouter aux contacts
ui-add-dav = Ajouter des contacts et agendas
ui-add-dav-note = Depuis un serveur CardDAV et CalDAV : Nextcloud et Murena, Fastmail, iCloud, votre hébergeur. Le mot de passe va dans le trousseau du système.
ui-dav-server = Adresse du serveur, s’il ne se trouve pas
agenda-next = Les deux prochaines semaines
agenda-from-day = À partir du { $day }
agenda-week-of = Semaine du { $day }
agenda-mode-agenda = Ce qui vient
agenda-mode-day = Jour
agenda-mode-week = Semaine
agenda-mode-month = Mois
agenda-earlier = Plus tôt
agenda-later = Plus tard
agenda-free = Rien de prévu.
agenda-repeats = se répète
agenda-organizer = Organisé par { $name }
agenda-delete-this = Cette fois seulement
agenda-delete-all = Toutes les fois
answer-accepted = a accepté
answer-declined = a décliné
answer-tentative = peut-être
answer-needs-action = pas encore répondu
event-title = Titre
event-starts = Début
event-ends = Fin
event-where = Lieu
event-repeat = Se répète
event-calendar = Agenda
event-pick-day = Choisir le jour
repeat-none = Ne se répète pas
repeat-daily = Tous les jours
repeat-weekly = Toutes les semaines
repeat-monthly = Tous les mois
repeat-yearly = Tous les ans
contact-name = Nom
contact-emails = Adresses électroniques
contact-phones = Numéros de téléphone
contact-org = Organisation
contact-title = Fonction
contact-address = Adresse
contact-birthday = Anniversaire
contact-notes = Notes
contact-web = Site web
contact-book = Carnet d’adresses
contact-add-email = Une autre adresse
contact-add-phone = Un autre numéro
contact-add-address = Une autre adresse postale
contact-categories = Catégories
contact-category-add = catégorie
contact-category-remove = Retirer la catégorie { $category }
contact-category-used = Catégories utilisées
contacts-category-all = Toutes les catégories
contacts-none-in = Aucun contact dans { $category }.
set-phone-region = Pays des numéros de téléphone écrits sans indicatif
set-phone-region-help = Un numéro écrit sans son pays, comme 06 08 12 34 56, est lu comme un numéro de ce pays, pour retrouver le même numéro écrit +33 6 08 12 34 56. Seulement pour comparer : les fiches gardent leurs numéros tels qu’ils sont écrits.
set-phone-region-usual = Comme le dit votre système : { $country }
set-phone-region-none = Aucun : ces numéros sont comparés tels qu’ils sont écrits
dup-open = Doublons
dup-title = Doublons
dup-looking = Recherche des doublons…
dup-region = Pays des numéros écrits sans indicatif : { $country }. Les réglages (⚙) le changent.
dup-region-none = Les numéros écrits sans leur pays sont comparés tels qu’ils sont écrits ; les réglages (⚙) leur en donnent un.
dup-within-title = Sur une même fiche
dup-within-none = Aucune fiche ne contient deux fois un numéro ou une adresse.
dup-within-some = { $n ->
    [one] Une fiche contient deux fois un numéro ou une adresse. Cochée, elle en garde un de chaque :
   *[other] { $Countf } fiches contiennent deux fois un numéro ou une adresse. Celles qui sont cochées en gardent un de chaque :
}
dup-removed = { $value }, le même que { $kept }
dup-clean = Retirer les doublons
dup-cleaned = { $n ->
    [one] Doublons retirés d’une fiche.
   *[other] Doublons retirés de { $countf } fiches.
}
dup-pairs-title = Deux fiches, une seule personne ?
dup-pairs-none = Aucune paire de fiches ne semble être une seule personne.
dup-pairs-some = { $n ->
    [one] Une paire de fiches pourrait être une seule personne.
   *[other] { $Countf } paires de fiches pourraient être une seule personne, montrées une à la fois.
}
dup-share = Les deux ont { $what }.
dup-share-name = le même nom
dup-share-phone = le numéro { $value }
dup-share-email = l’adresse { $value }
dup-in-book = Dans { $book }
dup-keep-name = Nom gardé
dup-merge-what = Fusionner garde la fiche du nom gardé, avec son organisation, son anniversaire et sa photo quand elle en a, et ajoute ce que seule l’autre contient : numéros, adresses, sites web, catégories, notes. L’autre fiche est ensuite supprimée, ici et sur son serveur.
dup-merge = Fusionner
dup-not-same = Pas la même personne
dup-later = Plus tard
dup-merged = Fusionnées en une fiche : { $names }.
dup-kept-apart = Gardées à part, plus jamais proposées : { $names }.
dup-done-title = Fait récemment
dup-done-help = Chaque changement est gardé trente jours, pour pouvoir l’annuler.
dup-done-clean = { $n ->
    [one] Doublons retirés de { $names }
   *[other] Doublons retirés de { $countf } fiches
}
dup-done-merge = Fusionnées : { $names }
dup-undone-aside = Remis comme avant. Une fiche modifiée depuis est gardée à part dans { $folder }.
dup-changed = Ces fiches ont changé entre-temps : regardez-les à nouveau.
country-fr = France
country-re = La Réunion
country-yt = Mayotte
country-gp = Guadeloupe
country-mq = Martinique
country-gf = Guyane
country-be = Belgique
country-ch = Suisse
country-lu = Luxembourg
country-de = Allemagne
country-at = Autriche
country-nl = Pays-Bas
country-gb = Royaume-Uni
country-ie = Irlande
country-es = Espagne
country-pt = Portugal
country-it = Italie
country-dk = Danemark
country-no = Norvège
country-se = Suède
country-fi = Finlande
country-pl = Pologne
country-gr = Grèce
country-us = États-Unis
country-ca = Canada
country-au = Australie
country-nz = Nouvelle-Zélande
country-jp = Japon
country-in = Inde
country-ma = Maroc
country-dz = Algérie
country-tn = Tunisie
country-sn = Sénégal
country-ci = Côte d’Ivoire
label-work = travail
label-home = domicile
label-cell = portable
label-fax = fax
label-pager = bipeur
label-text = SMS
label-video = vidéo
label-main = standard
invite-cancelled = Annulé : { $summary }
invite-known = Dans votre agenda.
invite-accept = Accepter
invite-maybe = Peut-être
invite-decline = Décliner
invite-add = Ajouter à mon agenda
invite-remove = Le retirer de mon agenda
# The locale Qt formats dates with.
qt-locale = fr_FR
agenda-no-calendar = Pas encore d’agenda : ajoutez vos contacts et agendas dans Comptes.
contacts-no-book = Pas encore de carnet d’adresses : ajoutez vos contacts et agendas dans Comptes.

## OpenPGP.
pgp-encrypted = Chiffré
pgp-not-opened = Chiffré, et aucune de vos clés ne l’ouvre
pgp-signed-by = Signé par { $signer }
pgp-signed-unknown = Signé avec une clé que Sioul ne connaît pas
pgp-signature-bad = La signature ne correspond pas au texte
pgp-no-own-key = Aucune clé à vous pour { $address } pour signer : créez-en une ou importez-la dans Comptes.
pgp-missing-key = Pas encore de clé pour { $address } : cherchez-la, ou envoyez sans chiffrer.
pgp-making = Création de votre clé…
pgp-made = Votre clé pour { $address } est créée ; le trousseau garde sa phrase secrète.
pgp-wrong-passphrase = Cette phrase secrète n’ouvre pas la clé : rien n’a été importé.
pgp-imported = { $n ->
    [one] Une clé importée.
   *[other] { $Count } clés importées.
}
pgp-no-key = Cette clé n’est plus là.
pgp-removed = Clé retirée.
pgp-looking = Recherche de leurs clés…
pgp-lookup-all = Toutes les clés sont trouvées : le message peut être chiffré.
pgp-lookup-some = Aucune clé trouvée pour { $missing }.
pgp-lookup-failed = Les serveurs de clés ne répondent pas ({ $detail }).
ui-sign = Signer
ui-encrypt = Chiffrer
ui-no-key-for = Pas encore de clé pour { $addresses }
ui-look-for-keys = Chercher leurs clés
ui-encryption = Chiffrement (OpenPGP)
ui-encryption-note = Signez et chiffrez vos messages. Vos clés restent sur cet appareil, leurs phrases secrètes dans le trousseau du système ; les personnes à qui vous écrivez reçoivent votre clé avec chaque message (Autocrypt).
ui-export-key = Enregistrer la clé publique
ui-make-key = Créer une clé pour { $address }
ui-import-key = Importer une clé…
ui-key-passphrase = La phrase secrète de la clé, s’il y en a une
ui-import = Importer
ui-others-keys = Clés des autres
pgp-expires = jusqu’au { $date }
ui-history = Jusqu’où remontent le courrier et l’agenda
ui-history-note = Le courrier plus ancien arrive au prochain relevé ; « Tout » peut prendre du temps et de la place.
ui-history-set = Le courrier et l’agenda remontent maintenant à { $weeks } semaines.
ui-history-everything = Le courrier et l’agenda remontent maintenant jusqu’au début.
history-1 = 1 semaine
history-2 = 2 semaines
history-4 = 1 mois
history-13 = 3 mois
history-26 = 6 mois
history-52 = 1 an
history-0 = Tout

## Tâches : les mots des pages de tâches (docs/tasks.md). Jamais « en retard »,
## « manqué » ; une date demandée se dit en temps qui reste ; rien n’ordonne.
task-untitled = (sans titre)
task-estimate-minutes = Environ { $minutes } minutes
task-estimate-hour = Environ une heure
task-estimate-hours = Environ { $hours } h { $minutes }
task-left-today = c’est aujourd’hui
task-left-tomorrow = c’est demain
task-left-days = encore { $count } jours
task-left-weeks = encore { $countf } { $n ->
        [one] semaine
       *[other] semaines
    }
task-left-months = encore { $count } mois
task-due = Pour le { $date } : { $left }
task-date-asked = Date demandée : le { $date }
task-waits-for = Attend : { $titles }
task-waits-until = Attend le { $date }
task-steps-left = { $Countf } { $n ->
        [one] étape reste
       *[other] étapes restent
    } sur { $total }
task-steps-total = Ses étapes font { $estimate } en tout.
task-part-of = Fait partie de : { $title }
task-tight = À ce rythme, le plan finit après le { $date }. La faire plus tôt, la réduire ou la confier garderait la date.
task-done-on = Faite le { $date }
task-unblocks = Libère { $n ->
        [one] une autre étape
       *[other] { $countf } autres étapes
    }
task-stopped = Là où vous en étiez : { $text }
task-spent = { $minutes } minutes jusqu’ici
task-session = { $date } : { $minutes } minutes
task-no-case = Sans projet
task-weather-haze-note = Brume : le plan garde moins pour aujourd’hui.
task-weather-fog-note = Brouillard : seules les petites étapes s’affichent. Une étape fait une journée complète.
# Ce que tient une journée (docs/capacity.md) : des mots seulement, jamais un nombre.
capacity-ratio-longer = Les tâches de ce type prennent d’habitude environ { $factor }× la première estimation ; le plan en tient déjà compte.
capacity-ratio-shorter = Les tâches de ce type prennent d’habitude moins que la première estimation, environ { $factor }× ; le plan les pose ainsi.
capacity-reason-after-bad = Aujourd’hui tient un peu moins : hier était trop après une journée pleine.
capacity-reason-before-heavy = Aujourd’hui tient un peu moins : { $title } est demain.
capacity-reason-after-heavy = Aujourd’hui tient un peu moins, après { $title } hier.
capacity-reason-more = Les journées tiennent un peu plus qu’il y a deux semaines : les jours se sont bien passés.
capacity-reason-less = Les journées tiennent moins qu’il y a deux semaines.
capacity-said-tomorrow-less = Demain tiendra un peu moins : aujourd’hui était trop après une journée pleine.
capacity-said-held = Ce que tient une journée reste tel quel pour un temps : plusieurs jours ont été trop.
capacity-scale-cognitive = réflexion
capacity-scale-emotional = émotions
capacity-scale-anxiety = anxiété
capacity-scale-body = corps et sens
capacity-scale-total = la journée
capacity-gain-slot = Du temps pour vous
capacity-gain-suggestion = Peut-être : { $title }
capacity-slack = Gardé libre, si des étapes prennent plus longtemps
task-why-started = Vous l’avez commencée.
task-why-due = Sa date est le { $date } : { $left }.
task-why-chain = { $title }, qui l’attend, a une date : le { $date }.
task-why-unblocks = Elle libère { $n ->
        [one] une autre étape
       *[other] { $countf } autres étapes
    }.
task-why-free = Rien ne passe avant elle.
task-picked = Choisie pour vous parmi des étapes égales.
task-why-chain-part = { $title }, qui l’attend, fait partie de « { $whole } », dont la date est le { $date }.
task-only-tied = Rien d’autre que ce que vous avez remis n’est libre maintenant : celle-ci y est liée.
task-office-hours = Demande un bureau ouvert
task-office-hours-help = Proposée seulement quand les bureaux sont ouverts (réglages des tâches), et prévue seulement ces jours-là.
task-office-closed = Les étapes qui demandent un bureau ouvert reviennent { $when }.
task-all-aside = Les étapes mises de côté reviennent demain.
task-nothing-until = Rien n’est nécessaire aujourd’hui. L’étape suivante vient le { $date } : { $title }.
task-nothing-ready = Rien n’est nécessaire aujourd’hui.
task-wip = { $Countf } choses sont commencées. En finir ou en garer une ?
task-loop = Ces étapes s’attendent l’une l’autre : { $titles }. L’une d’elles doit passer d’abord.
task-done-frees = Cela libère : { $titles }.
task-done-all-steps = Toutes les étapes de { $title } sont faites.
board-ready = Libres de commencer
board-doing = Commencées
board-waiting = En attente
board-done = Faites
timeline-empty = Rien d’ouvert à placer.
timeline-reaches = Le plan va jusqu’au { $date }.

## Comment une chose est liée à celle ouverte.
how-note = Ses notes
how-notes-of = Notes sur
how-source = Faite à partir de
how-made = A donné
how-waits-for = Attend
how-unblocks = Libère
how-part-of = Fait partie de
how-step = Une étape
how-contact = Qui
how-involves = Les concerne
how-case = Projet
how-in-case = Dans le projet
how-mentions = Mentionne
how-mentioned-by = Mentionnée dans
how-answers = Répond à
how-answered-by = Réponse
how-link = Lié
task-now-title = L’étape suivante
task-then = Ensuite
task-not-found = Aucune tâche ne correspond à « { $what } ».
task-ambiguous = Plusieurs tâches correspondent à « { $what } » : { $titles }. Donnez son UID.
task-no-such-list = Aucune liste « { $list } » ne prend de tâches (voir sioul tasks lists).
task-no-list = Pas encore de liste de tâches : créez-en une avec sioul tasks new-list <compte> <nom>.
task-no-title = Une tâche a besoin d’un titre.
task-read-only = Cette tâche est dans une liste en lecture seule.
task-done-said = Fait.
import-unknown-key = « { $key } », nommée par { $task }, n’est pas dans le fichier.
focus-already = Une séance tourne déjà, sur { $title }.
focus-started = Concentration sur { $title } pendant { $minutes } minutes.
focus-status = { $task } : { $elapsed } minutes jusqu’ici, encore { $left }.
focus-none = Aucune séance en cours.
focus-stopped = { $n ->
        [one] Une minute, gardée.
       *[other] { $n } minutes, gardées.
    }
note-not-found = Aucune note à { $path }.
task-no-list-yet = Pas encore de liste de tâches. La page Tâches peut en créer une.
task-gone = Cette tâche n’est plus là.
task-set-aside = Mise de côté jusqu’à demain.
undo-task-deleted = Tâche supprimée.
note-untitled = Note sans titre
note-from-mail = De { $sender } :
note-guests = Personnes invitées :
note-gone = Cette note n’est plus là.
task-prepare = Préparer : { $title }

## Les pages des tâches, des notes et de la concentration.
ui-tasks = Tâches
ui-notes = Notes
ui-make-task = En faire une tâche
ui-make-note = En faire une note
ui-make-event = En faire un événement
related-title = Lié à cela
task-mode-now = Maintenant
task-mode-list = Liste
task-mode-board = Tableau
task-mode-timeline = Calendrier
task-capture-hint = Une tâche en une ligne : « Appeler les impôts demain ~15m #projet {"{"}date demandée{"}"} »
chip-start = à partir du
chip-due = date demandée
chip-estimate = minutes
chip-case = projet
chip-tag = étiquette
task-first-list = Les tâches vivent dans une liste de tâches, sur votre serveur d’agenda ou seulement ici. Une liste se crée en un clic :
task-default-list = Tâches
task-make-list = Créer la liste
task-all-cases = Tous les projets
task-weather = Comment est aujourd’hui ?
task-weather-clear = Clair
task-weather-haze = Brume
task-weather-fog = Brouillard
task-start = Commencer
task-done = Fait
task-not-now = Pas maintenant
task-open-again = Rouvrir
# Plus à faire finalement (STATUS:CANCELLED) : gardée, barrée, hors du plan.
task-drop = Abandonner
# Les détails d’une tâche : les horaires de son bureau non donnés, les habituels.
task-office-usual-short = horaires habituels des bureaux
task-drop-help = Plus à faire finalement : gardée, barrée, hors du plan. « Rouvrir » la ramène.
# Le temps que prennent les choses de ce genre, comparé à la première estimation, montré sur demande (ratio_line).
task-ratio-ask = Combien de temps ça prend, d’habitude
task-hard = Qu’est-ce qui rend cela difficile ?
task-hard-how = Je ne sais pas par où commencer
task-hard-big = C’est trop gros
task-hard-dread = Cela me pèse
task-hard-boring = C’est ennuyeux
task-hard-energy = Pas d’énergie aujourd’hui
task-other-choices = Autres choix
task-started = Commencées
task-done-week = Faites cette semaine
task-search = Chercher dans les tâches
task-by-case = Par projet
task-by-list = Par liste
task-show-done = Faites aussi
task-title = Titre
task-steps = Étapes
task-add-step = Une étape, en une ligne
task-waits-title = Attend
task-waits-add = Attend… (le titre d’une tâche)
task-waits-remove = Ne l’attend plus
task-frees-one = Libère : { $title }
task-field-start = Peut commencer le
task-field-due = Date demandée
task-field-estimate = Prend environ
task-field-case = Projet
task-field-tags = Étiquettes
task-field-repeat = Revient
task-field-list = Liste
task-field-notes = Notes, en Markdown
task-write = Écrire un courriel
task-make-note = En faire une note
focus-title = Concentration
focus-two = Juste deux minutes
focus-for = { $minutes } minutes
focus-open-ended = Sans fin fixée
focus-left = Encore { $minutes } min
focus-so-far = { $minutes } min jusqu’ici
focus-two-left = Encore deux minutes : le moment de trouver où s’arrêter.
focus-over = Le temps choisi est passé.
focus-paused = En pause.
focus-pause = Pause
focus-resume = Reprendre
focus-plus-five = Cinq minutes de plus
focus-keep-going = Continuer
focus-stop-counts = S’arrêter ici, cela compte
focus-stop = Arrêter
focus-breadcrumb = La prochaine fois, commencer par…
focus-done = Fait
focus-keep = Garder
# Le temps qui court, dans les notifications du système : depuis quand, avec le temps choisi ou sans fin ; en pause, le temps jusqu’ici.
focus-notification-planned = Depuis { $time }, { $minutes } min choisies
focus-notification-open = Depuis { $time }, sans fin fixée
focus-notification-paused = En pause, { $minutes } min jusqu’ici
# Son canal sur Android, tel que les réglages d’Android le nomment.
focus-notification-channel = Minuteur de concentration
note-search = Chercher dans les notes
note-new = Nouvelle note
note-title = Son titre
note-no-store = Pas encore de dossier de notes : il se choisit dans Paramètres ▸ Votre dossier et le partage.
note-none-open = Une note s’ouvre ici.
note-recent = Changées récemment
note-read = Lire
note-save = Enregistrer
note-send-guests = Envoyer aux personnes invitées
note-open-lines = Lignes non cochées
note-make-task = En faire une tâche
task-group-joy = Si vous en avez envie
task-group-someday = Quand vous le direz
task-joy = Si vous en avez envie

## Réglages, chacun là où il s’applique (une phrase chacun).
ui-settings = Réglages de cette page
set-like-others = Comme les autres adresses
set-windows = Heures de travail
set-windows-help = Ces jours et à ces heures, le travail peut vous joindre. En dehors, il se repose : vos démarches viennent à leurs propres heures, plus bas ; tout le reste du temps, ce sont vos loisirs. Les codes et les liens que vous demandez aux sites, et ce que vous vous envoyez, arrivent toujours tout de suite.
set-sorting = Comment le courrier est trié
set-accounts-elsewhere = À quoi sert chaque adresse, jusqu’où elle remonte, à quel rythme elle est relevée et son bouclier se règlent sur sa fiche dans Comptes : l’icône de personne en bas de la colonne de gauche.
set-task-areas-group = À quoi sert une tâche
set-files-group = Votre dossier
set-invoice-group = Factures
set-time-off = Congés
set-time-off-help = Vacances, arrêt maladie : calme du premier jour au dernier, comme un jour sans travail.
set-time-off-label = Vacances, un mot dessus
set-time-off-add = Ajouter
set-quiet-personal = Ce qui est à vous
set-quiet-personal-help = Les tâches de ces catégories, et les projets marqués comme à vous, sont à vous hors du travail : celles de loisirs (joy, famille, amis, loisirs) viennent pendant les loisirs et les repas, les autres (santé, perso) à tous vos moments. Une tâche sans aucune de ces catégories ni celles du travail est une démarche.
set-office-hours = Quand les bureaux sont ouverts
set-office-hours-help = Une tâche marquée « Demande un bureau ouvert » n’est proposée qu’à ces heures, et prévue que ces jours-là.
set-windows-add = Ajouter une fenêtre
set-windows-none = Aucune fenêtre : le Porche est toujours ouvert.
set-known = Expéditeurs connus
set-known-help = Leur courrier va directement dans « De personnes que vous connaissez ». Les autres attendent dans le filtre d’accueil jusqu’à ce que vous les laissiez entrer. Une adresse, ou @domaine pour tout le monde là-bas.
set-blocked = Expéditeurs bloqués
set-blocked-help = Leur courrier est écarté pour de bon : jamais montré, jamais compté. Une adresse, ou @domaine pour tout le monde là-bas.
set-senders-add = Ajouter une adresse ou un @domaine
set-fetch-minutes = Relever toutes les
set-fetch-minutes-help = La boîte de réception arrive dès que le serveur signale du nouveau ; les autres dossiers sont relevés à ce rythme.
set-account-name = Votre nom
set-account-name-help = Tel que les destinataires le voient dans la ligne De.
set-account-signature = Signature
set-account-signature-help = Ajoutée sous ce que vous écrivez, en Markdown.
set-account-priority = Importance
set-account-priority-help = Moins importante : son courrier ne passe pas par le filtre d’accueil et attend replié en bas du Porche. Plus importante : il passe en premier.
set-account-history = Jusqu’où remonter
set-account-history-help = Combien de semaines du courrier de cette adresse les dossiers montrent.
set-account-fetch = Relever toutes les
set-account-fetch-help = 0 suit les autres adresses. Une adresse publique peut être relevée moins souvent : deux fois par jour, c’est 720.
set-account-shield = Protégée contre le harcèlement
set-account-shield-help = Pour une adresse publique qui peut amener des insultes : son courrier est lu d’abord ; les messages hostiles sont écartés sans montrer leur texte, le reste est résumé par sujet.
set-account-shield-ai = Laisser l’IA le lire d’abord
set-account-shield-ai-help = Une IA (Claude) lit le nouveau courrier de cette adresse pour en dire le ton et le sujet, plus finement que des listes de mots. Le texte quitte cet appareil pour cela.
set-day-start = La journée commence à
set-day-start-help = L’heure à laquelle s’ouvrent le planning du jour et de la semaine.
set-task-estimate = Une tâche sans durée compte
set-task-estimate-help = Les minutes que le plan compte pour une tâche dont la durée n’est pas dite.
set-planning-group = Ce que tient une journée
set-planning-start = Partir de
set-planning-start-help = Tant que vos journées n’en disent pas plus, combien le plan remplit une journée. Comme maintenant : vos heures pleines, deux étapes lourdes au plus. Plus léger, ou bien plus léger : pour reprendre après une période difficile, ou avec une maladie qui limite l’énergie.
set-planning-start-as-now = Comme maintenant
set-planning-start-lighter = Plus léger
set-planning-start-much-lighter = Bien plus léger
set-planning-window = Jours dont le plan apprend
set-planning-window-help = Combien de jours passés le plan regarde pour apprendre ce que tient une journée pour vous. Seuls comptent les jours dont vous avez dit qu’ils étaient de trop, à peu près bien ou trop vides.
set-planning-window-unit = jours
set-planning-even = Des journées égales
set-planning-even-help = La charge de la semaine répartie pour que chaque jour en tienne à peu près autant, plutôt que des journées pleines et d’autres vides. Cela aide avec une maladie qui limite l’énergie.
set-planning-gain-slots = Du temps pour vous
set-planning-gain-slots-help = Deux moments par jour, l’un après l’étape la plus lourde, l’autre le soir, gardés libres et calmes : remplissez-les comme vous voulez, ou laissez-les vides.
set-task-list = Les nouvelles tâches vont dans
set-task-list-help = La liste où va une tâche tapée en une ligne.
set-task-list-first = La première liste faite pour les tâches
set-case-store = Le dossier des notes
set-case-store-help = Votre dossier de fichiers Markdown, lu comme un coffre : vos notes, et à côté vos projets (sioul-cases.toml) et vos budgets (sioul-budgets.toml). Sioul s’y lie et ne le possède jamais.
set-notes-folder = Les nouvelles notes vont dans
set-notes-folder-help = Un dossier à l’intérieur du dossier des notes, pour les notes faites à partir du courrier et des événements.
set-reading-family = Police
set-reading-family-help = Pour le texte long : notes, courrier, notes d’une tâche. Vide : celle du bureau.
set-reading-size = Taille
set-reading-size-help = La taille du texte long.
set-reading-spacing = Interligne
set-reading-spacing-help = De l’air entre les lignes : 1,5 se lit plus facilement que des lignes serrées.
set-map-geocode = Placer les contacts sur la carte
set-map-geocode-help = Leurs adresses postales sont envoyées au géocodeur d’OpenStreetMap (Nominatim), une fois chacune, une par seconde ; les lieux trouvés restent sur cet appareil.
map-show = Carte
map-list = Liste
map-allow = Pour placer vos contacts, leurs adresses postales sont envoyées au géocodeur d’OpenStreetMap, une fois chacune. Rien d’autre ne part avec.
map-allow-button = Les placer
map-waiting = { $n ->
    [one] Une adresse pas encore placée.
   *[other] { $n } adresses pas encore placées.
}
map-locating = Placement des adresses, une par seconde…
map-place = Les placer maintenant
map-none = Aucun contact n’a encore d’adresse placée.
set-map-tiles = Tuiles de carte
set-map-tiles-help = D’où viennent les images de la carte, en https://…/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png. Vide : celles d’OpenStreetMap, demandées avec mesure et gardées.
set-invoice-name = Votre nom ou raison sociale
set-invoice-name-help = Imprimé en haut de vos factures.
set-invoice-address = Votre adresse
set-invoice-address-help = Sur plusieurs lignes, comme sur une enveloppe.
set-invoice-siret = SIRET
set-invoice-siret-help = Ou le numéro d’entreprise là où vous êtes ; vide tant que l’immatriculation n’est pas faite.
set-invoice-vat = Mention de TVA
set-invoice-vat-help = Pour une micro-entreprise : « TVA non applicable, art. 293 B du CGI ».
set-invoice-prefix = Les numéros de facture commencent par
set-invoice-prefix-help = Les numéros se suivent : 2026-001, 2026-002…
set-invoice-currency = Devise
set-invoice-currency-help = Un code de trois lettres : EUR, USD, CHF.
set-invoice-payment = Modalités de paiement
set-invoice-payment-help = Imprimées en bas : IBAN, délai, pénalités de retard.
set-invoice-folder = Les factures vont dans
set-invoice-folder-help = Le dossier de leurs PDF ; « Documents/Factures » dans votre dossier personnel s’il est vide.
set-invoice-rate = Une heure coûte
set-invoice-rate-help = Le tarif quand un projet ne fixe pas le sien.
set-theme = Couleurs
set-theme-help = Claires ou sombres, ou comme celles du système. Les icônes suivent au prochain démarrage.
set-theme-system = Celles du système
set-theme-light = Claires
set-theme-dark = Sombres
set-ai-group = Le bouclier IA
set-ai-key = Clé de l’API d’Anthropic
set-ai-key-help = Gardée dans le trousseau du système, jamais dans un fichier. Chaque nouveau message vers une adresse protégée qui permet l’IA est envoyé une fois à Anthropic (modèle Claude Haiku), avec son objet ; la réponse (ton, sujet, une ligne neutre) reste sur cet appareil. Une clé se crée sur console.anthropic.com.
set-ai-key-kept = Une clé est gardée.
set-ai-key-none = Pas encore de clé : les listes de mots jugent seules.
set-ai-key-keep = Garder
set-ai-key-forget = Oublier la clé
set-mail-threads = Par conversation
set-mail-threads-help = Un message et ses réponses ensemble, sous le plus récent ; les vôtres viennent des Envoyés. S’ouvre par la petite flèche à gauche.
ui-conversation = { $n } messages : montrer ou replier la conversation
link-made = Lié à « { $title } ».
link-undone = Le lien est défait.
link-is-plan = Ce lien est une étape ou une attente : il se change dans la tâche.
link-in-text = Ce lien est écrit dans le texte de la note : modifiez la note pour l’enlever.
link-is-guest = Cette personne est invitée à l’événement : cela se change dans l’événement.
link-not-found = Ce lien n’existe plus.
link-not-made = Cela n’a pas pu être fait.
ui-add-new = Ajouter
ui-link-existing = Lier à…
add-task = Une tâche
add-event = Un événement
add-note = Une note
add-mail = Un message
add-reply = Une réponse
add-contact = L’expéditeur, dans vos contacts
link-title = Lier « { $title } » à…
link-search = Quelques mots de son titre
link-kind-all = Tout
link-kind-task = Tâches
link-kind-event = Événements
link-kind-mail = Courrier
link-kind-note = Notes
link-kind-contact = Contacts
link-kind-budget = Lignes de budget
link-kind-case = Projets
link-kind-draft = Brouillons
link-kind-web = Web
link-kind-file = Fichiers
link-kind-other = Autre
link-already = Déjà lié
link-nothing = Rien ne correspond.
link-undo = Défaire ce lien
task-nothing-filtered = Rien de ce type n’est libre maintenant.
task-quiet-empty = Rien ne vous attend maintenant.
task-kind = Type
task-kind-help = Ce que la faire demande : des tâches du même type se font mieux ensemble. Dans une ligne : « @appel », « @écrire », « @enligne », « @dehors », « @lire », « @réfléchir », « @faire ».
task-kind-none = Sans type
task-kind-call = Un appel
task-kind-write = Écrire
task-kind-online = En ligne, un formulaire
task-kind-out = Sortir
task-kind-read = Lire
task-kind-think = Y réfléchir
task-kind-make = Faire, fabriquer
task-filter-all = Tous les types
task-filter-any-category = Toutes les catégories
chip-kind = Type
invoice-to = Facturé à
invoice-title = Facture n° { $number }
invoice-dated = Émise le { $date }
invoice-due = À payer avant le { $date }
invoice-project = Projet : { $project }
invoice-what = Travail
invoice-time = Temps
invoice-rate = Taux horaire
invoice-amount = Montant
invoice-total = Total
invoice-siret = SIRET { $siret }
invoice-late = En cas de retard de paiement, pénalités au taux de trois fois le taux d’intérêt légal et indemnité forfaitaire de 40 € pour frais de recouvrement (Code de commerce, art. L441-10 et D441-5). Pas d’escompte pour paiement anticipé.
time-no-project = Sans projet
time-gone-project = Un projet qui n’existe plus
time-week-of = Semaine du { $day }
time-nothing-week = Rien de noté cette semaine.
time-nothing-month = Rien de noté ce mois-ci.
time-nothing-year = Rien de noté cette année.
project-done = Fait
project-asked = Date demandée
project-from = De { $sender }
project-note-changed = Note modifiée
project-time-noted = { $time } noté
project-invoice = Facture { $number }
project-no-title = Un projet a besoin d’un nom.
project-gone = Ce projet n’existe plus.
invoice-nothing = Aucun temps facturable n’attend d’être facturé.
invoice-gone = Cette facture n’existe plus.
invoice-folder-name = Factures
invoice-budget-line = Facture { $number }, { $client }
invoice-budget-origin = Facture { $number }, faite par Sioul : attendue jusqu’à son paiement.
time-no-minutes = Combien de temps ? Quelques minutes, au moins.
time-no-project-chosen = Pour quel projet, ou quelle tâche ?
time-bad-day = Quel jour ? Il s’écrit 2026-10-05.
time-billed-stays = Ce temps est sur une facture : il reste.
# Comment les minutes d’un moment ont été connues, dit discrètement sur la page Temps.
time-kind-measured = chronométré
time-kind-typed = noté à la main
time-kind-corrected = chronométré, puis corrigé
time-kind-unknown = on ne sait pas comment
time-bad-hours = Les heures s’écrivent 09:00 et 10:30.
time-gone = Ce moment n’est plus là : il a peut-être été changé ailleurs.
time-field-from = De
time-field-to = À
time-field-task = Tâche
time-no-task = Pas de tâche
time-own-project = Le projet de la tâche
time-lasts = { $time }
ui-projects = Projets
ui-time = Temps
project-new = Nouveau projet
project-edit = Projet
project-none = Pas encore de projet. Un projet, c’est toute affaire que vous suivez : un travail pour un client, son temps facturable, ou une affaire à vous (logement, santé, un voyage).
project-choose = Un projet, à gauche.
project-field-title = Nom
project-field-billable = Pour un client : son temps se facture à un taux horaire
project-field-billable-help = Le temps noté dessus peut se facturer ; sans cela, le projet est à vous : logement, santé, un voyage.
project-field-client = Pour
project-field-client-hint = Pour qui
project-field-rate = Une heure coûte
project-field-rate-hint = Le taux des réglages des factures s’il est vide
project-field-budget = Factures attendues dans
project-no-budget = Aucun budget
project-field-status = Où il en est
project-status-open = Ouvert
project-status-waiting = En attente
project-status-closed = Fermé
project-kind-project = Pour un client
project-kind-case = À vous
project-rate = { $rate } de l’heure
project-board = Tableau
project-list = Liste
project-calendar = Calendrier
project-open-tasks = { $n ->
    [one] une tâche ouverte
   *[other] { $n } tâches ouvertes
}
project-tasks-count = { $open ->
    [0] Aucune tâche ouverte
    [one] Une tâche ouverte
   *[other] { $open } tâches ouvertes
}
project-tasks-done = { $done } faites
project-time-all = { $time } notées
project-to-bill = { $time } à facturer
project-to-bill-amount = { $time } à facturer
project-invoices = Factures
project-timeline = Sur une ligne de temps
project-coming = À venir
project-before = Avant
invoice-make = Faire la facture
invoice-paid = Payée
invoice-print = Imprimer à nouveau
invoice-written = La facture est dans { $path }.
invoice-not-written = La facture n’a pas pu être écrite dans { $path }.
time-note = Noter du temps
time-field-project = Pour
time-field-length = Combien de temps
time-field-length-hint = 1h30, 45m, 90
time-field-day = Quand
time-field-note = Ce que c’était
time-field-note-hint = Une réunion, un appel…
time-field-unbilled = Ne pas facturer
time-period-week = Semaine
time-period-month = Mois
time-period-year = Année
time-all-projects = Tous les projets
time-total = { $time } en tout
time-on-invoice = Sur la facture { $number }
time-remove = Enlever ce temps
badge-pace-better = En avance sur son rythme de { $gap }
badge-pace-on-track = Dans les temps
badge-pace-short = { $gap } de retard sur son rythme
pace-needs-month = { $gone } % du mois écoulé ; { $part } % de ce qu’il lui faut encore est arrivé.
pace-needs-year = { $gone } % de l’année écoulée ; { $part } % de ce qu’il lui faut encore est arrivé.
pace-may-spend-month = { $gone } % du mois écoulé ; { $part } % de ce qu’il peut dépenser est dépensé.
pace-may-spend-year = { $gone } % de l’année écoulée ; { $part } % de ce qu’il peut dépenser est dépensé.
pace-nothing-needed-month = { $gone } % du mois écoulé ; il ne faut rien de plus.
pace-nothing-needed-year = { $gone } % de l’année écoulée ; il ne faut rien de plus.
fig-range = D’un mauvais mois à un bon
fig-range-value = { $low } à { $high }
budget-in = Entrées
budget-out = Sorties
budget-step-day = Jours
budget-step-week = Semaines
budget-step-month = Mois
budget-step-year = Années
budget-movements = Mouvements
budget-planned = prévu
budget-recurring = chaque période
budget-add = Ajouter un mouvement
budget-add-once = Une fois
budget-add-recurring = Régulier
budget-add-label = Quoi
budget-add-amount = Montant
budget-add-amount-hint = -650 en sortie, 1200 en entrée
budget-add-date = Quand
budget-add-every = Chaque
budget-add-every-month = mois, le jour
budget-add-every-year = année, le jour
budget-add-estimate = Une estimation (courses, liquide), pas un montant fixe
budget-add-from = À partir du
budget-add-until = Jusqu’au
budget-added = Ajouté à { $budget }.
budget-add-bad-amount = Un montant, positif pour une entrée, négatif pour une sortie.
budget-back = Budgets
ui-sites = Sites
ui-menu = Menu
ui-back = Retour
ui-phone-account = Depuis les comptes du téléphone…
ui-phone-account-note = Android prête l’adresse d’un compte, jamais son mot de passe : Sioul trouve ses serveurs et demande le mot de passe une fois.
account-password = Mot de passe…
account-password-title = Mot de passe de { $account }
account-password-help = Les mots de passe ne voyagent jamais entre vos appareils : celui-ci est gardé dans le trousseau de cet appareil, une fois accepté par son serveur.
account-password-vault = Depuis Bitwarden…
account-password-vault-missing = Votre compte Bitwarden n’est pas réglé ici : Sites ▸ ⚙, son adresse, et son serveur quand ce n’est pas bitwarden.com. Il vient de vos autres appareils avec les réglages.
account-password-from = Depuis Bitwarden : { $name }.
account-password-use = Utiliser
account-password-keep = Garder
account-password-testing = Le serveur est interrogé…
account-password-kept = { $account } : mot de passe gardé ; la synchronisation reprend.
bitwarden-unlock-help-password = Votre mot de passe principal ouvre le coffre ici, par Sioul lui-même ; il n’est pas gardé, et le coffre reste ouvert jusqu’à la fermeture de Sioul.
site-none = Pas encore de site. Une messagerie sécurisée (celle d’une banque, d’un hôpital, des impôts), une discussion, tout site à garder sous la main : connecté une fois, gardé ici.
site-choose = Un site, à gauche.
site-android = Ici, les sites s’ouvrent dans votre navigateur : Qt WebEngine, qui les garde dans Sioul sur un ordinateur, n’existe pas pour Android.
site-add = Épingler un site
site-field-name = Nom
site-field-url = Adresse
site-field-kind = Ce que c’est
site-kind-mailbox = Espaces personnels
site-kind-chat = Discussions
site-kind-dating = Rencontres et amitié
site-kind-other = Autres sites
site-background = Le garder ouvert, pour en être prévenu
site-back = Retour
site-reload = Recharger
site-mute = Couper ses sons
site-realtime-help = Ses notifications tout de suite, jusqu’à ce que vous décochiez ; sinon elles attendent le Porche.
site-share = Partager dans l’appel
site-share-screens = Tout un écran
site-share-windows = Une fenêtre
site-fill-login = Remplir l’identifiant
site-downloading = Téléchargement de { $file } dans vos téléchargements.
site-notified = Du nouveau sur { $site }
site-notified-more = et { $n } de plus
site-open = Ouvrir { $site }
bitwarden-missing = Votre compte Bitwarden se règle dans les réglages de cette page (⚙) : son adresse, et son serveur quand ce n’est pas bitwarden.com.
bitwarden-unlock-help = Votre clé de sécurité, ou votre mot de passe principal, ouvre le coffre ici, par Sioul lui-même ; rien n’en est gardé, et le coffre reste ouvert jusqu’à la fermeture de Sioul.
bitwarden-open = Ouvrir
bitwarden-code = Code
bitwarden-code-0 = Le code de votre application d’authentification.
bitwarden-code-1 = Le code que Bitwarden vient de vous envoyer par courriel.
bitwarden-code-3 = Touchez votre YubiKey : elle tape son code.
bitwarden-code-8 = Votre code de récupération.
bitwarden-factor-0 = Application d’authentification
bitwarden-factor-1 = Courriel
bitwarden-factor-3 = YubiKey
bitwarden-factor-8 = Code de récupération
bitwarden-factor-unsupported = Votre compte demande Duo, que Sioul ne sait pas encore prendre : ajoutez une clé de sécurité ou une application d’authentification comme seconde étape dans Bitwarden.
bitwarden-new-device = Un nouvel appareil : Bitwarden vous a envoyé un code par courriel.
set-bitwarden-email = Compte Bitwarden
set-bitwarden-email-help = Son adresse. Le coffre est lu ici, par Sioul lui-même, seulement quand vous remplissez le formulaire d’un site, et seulement avec votre mot de passe principal, jamais gardé.
set-bitwarden-server = Serveur Bitwarden
set-bitwarden-server-help = Vide pour bitwarden.com ; bitwarden.eu ; ou l’adresse de votre propre serveur (Vaultwarden aussi).
bitwarden-unlock = Déverrouiller Bitwarden
bitwarden-password = Mot de passe principal
bitwarden-refused = Bitwarden ne s’est pas déverrouillé : le mot de passe est peut-être mal tapé.
bitwarden-locked = Bitwarden est verrouillé.
bitwarden-none = Bitwarden ne garde aucun identifiant pour ce site.
until-tomorrow = demain à { $time }
day-tomorrow = demain
mail-resting = Le courrier du travail se repose jusqu’au retour du travail. Il est là si vous le cherchez.
mail-rest = Pendant le sommeil, le courrier se repose : rien ne notifie, et le Porche ne montre que ce que vos listes laissent passer maintenant. Le reste est là si vous le cherchez.
set-account-area = À quoi sert cette adresse
set-account-area-help = Son courrier vient aux moments de ce à quoi elle sert : travail, vos démarches, loisirs, plusieurs à la fois. Rien de coché : le travail, pour qu’il n’atteigne jamais vos soirées. Le courrier de vos expéditeurs sûrs vient à n’importe quelle adresse, aux moments cochés pour eux ; les codes viennent toujours.
area-work = Travail
area-personal = À vous, démarches ou loisirs
project-field-personal = À vous, hors travail
project-field-personal-help = Ses tâches restent en vue pendant le calme, avec la famille et les amis.
project-routes = Le courrier qui arrive ici tout seul
project-resting = Les projets de travail se reposent jusqu’au retour du travail.
ui-show-anyway = Montrer quand même
folder-server-only = Gardé sur le serveur seulement : clic droit pour en garder une copie ici.
folder-keep = En garder une copie ici
folder-forget = Ne plus en garder de copie
folder-forget-ask = Sa copie ici s’en va ; le serveur garde tous les messages, et en garder de nouveau une copie la ramène entière.
folder-delete = Supprimer ce dossier vide
folder-delete-ask = Supprimé sur le serveur, pour tous les appareils. Seul un dossier vide peut l’être : déplacez d’abord ses messages.
folder-new = Nouveau dossier…
folder-made = Dossier « { $name } » créé sur le serveur.
folder-deleted = Dossier « { $name } » supprimé sur le serveur.
folder-not-empty = « { $name } » contient encore des messages : déplacez-les d’abord.
sync-held-back = Le courrier plus ancien de { $account } attend : le disque garde sa réserve ({ $free } Go libres). Il viendra quand il y aura de la place.
note-as-list = Liste
note-as-tree = Dossiers
note-all = Toutes les notes
note-open-elsewhere = Ouvrir avec…
audio-play = Lire
audio-pause = Pause
audio-position = Position
note-memo = Enregistrer un mémo vocal
note-memo-stop = Arrêter · { $time }
budget-new = Nouveau budget
budget-edit = Le budget
budget-field-title = Nom
budget-field-period = Compté par
budget-period-month = Mois
budget-period-year = Année
budget-field-target = Solde à atteindre
budget-field-area = Pour
budget-field-area-help = Les heures où il est en vue (Paramètres, heures). Rien de coché : vos démarches.
budget-area-line = Pour : { $areas }
budget-field-target-hint = 0 pour l’équilibre
budget-remove = Retirer ce budget
budget-remove-yes = Le retirer
budget-remove-ask = Ses lignes restent dans le fichier, telles qu’écrites ; elles ne comptent plus nulle part.
budget-line-remove = Supprimer cette ligne
budget-line-remove-ask = La ligne quitte le fichier des budgets. Ce à quoi elle était liée reste.
note-rename = Renommer…
note-trash = Mettre à la corbeille
note-trashed = « { $title } » mise dans la corbeille des notes (.trash).
project-remove = Retirer de la liste
project-remove-ask = Ses tâches, notes, courriels et temps restent où ils sont ; ils ne sont plus rassemblés sous lui.
time-change = Modifier ce temps
site-remove = Retirer ce site
site-remove-ask = Il quitte Sioul : sa page, ses notifications. Rien ne change sur le site lui-même.
set-task-lists = Listes de tâches
set-task-lists-help = Renommées ici, et sur le serveur à la prochaine synchronisation. Une liste vide peut être supprimée ; une liste qui contient des tâches reste.
set-calendars = Agendas
set-calendars-help = Renommés ici, et sur le serveur à la prochaine synchronisation. Un agenda vide peut être supprimé ; un agenda qui contient des événements reste.
set-address-books = Carnets d’adresses
set-address-books-help = Renommés ici, et sur le serveur à la prochaine synchronisation. Un carnet vide peut être supprimé ; un carnet qui contient des contacts reste.
set-collections-remove = Le supprimer, ici et sur le serveur
word-and = et
health-every-days = { $days ->
    [2] un jour sur deux à { $time }, à partir du { $from }
   *[other] tous les { $days } jours à { $time }, à partir du { $from }
}
health-every-hours = toutes les { $hours } heures ; prochaine : { $next }
health-next-refill = pharmacie à partir du { $day }
health-renew-by = à renouveler d’ici le { $day }
health-valid-until = valable jusqu’au { $day }
health-until = jusqu’au { $day }
health-covers = pour { $names }
health-no-name = Un nom est nécessaire.
health-taken = Pris
health-errand-refill = Pharmacie : { $title }
health-errand-renew = Médecin : renouveler l’ordonnance de { $title }
health-category = santé
health-list = Santé
ui-health = Santé
health-local = Vos médicaments, ordonnances et prises restent sur cet appareil. Les passages à la pharmacie et les renouvellements deviennent des tâches dans la liste choisie plus bas, pour que votre téléphone les ait : leur titre nomme le médicament.
health-taken-at = Pris à { $time }
health-medicines = Médicaments
health-add-medicine = Ajouter un médicament
health-medicine = Le médicament
health-paused = en pause
health-prescriptions = Ordonnances
health-add-prescription = Ajouter une ordonnance
health-prescription = L’ordonnance
health-refilled = Retiré aujourd’hui
health-moving = Bouger
health-moving-every = Une pause pour bouger pendant la concentration, toutes les
health-minutes = minutes
health-minutes-a-day = minutes par jour
health-moving-help = Dès l’ouverture de Sioul, une notification sur votre bureau dit qu’il est temps de bouger, même la fenêtre cachée. Pendant une session de concentration, elle demande d’abord : Faire la pause, quelques minutes pour bouger et s’étirer, ou Pas maintenant ; un clic relance la session.
health-chats = Discussions
health-chats-limit = Une limite par jour pour les discussions
health-chats-after = Couvertes après
health-chats-for = Pendant
health-chats-help = Une fois leur temps passé, les discussions sont couvertes, muettes et silencieuses ; elles reviennent après le temps choisi, quoi qu’il arrive.
health-field-name = Nom
health-field-dose = Dose
health-field-dose-hint = 50 mg, un comprimé
health-field-when = Quand
health-every-day-choice = À heures fixes chaque jour
health-every-days-choice = Tous les quelques jours
health-every-hours-choice = Toutes les quelques heures
health-field-times = À
health-field-every-days = Tous les … jours
health-field-at = à
health-field-from = À partir du
health-field-every-hours = Toutes les … heures
health-field-from-time = à partir de
health-field-until = Jusqu’au
health-field-prescription = Ordonnance
health-no-prescription = Aucune
health-pause = En pause pour l’instant
health-remove = Le retirer
health-remove-medicine-ask = Plus de rappel pour lui. Ce qui a été marqué pris le reste.
health-remove-prescription-ask = Ses médicaments restent ; ils ne lui sont plus liés.
health-field-what = Quoi
health-field-what-hint = Vitamine D 1000 UI
health-field-prescriber = Prescrite par
health-field-valid-until = Valable jusqu’au
health-field-refill-days = La pharmacie donne
health-days-at-a-time = jours à la fois
health-field-last-refill = Dernier retrait
health-field-note = Note
focus-move = Le moment de bouger et de s’étirer quelques minutes. La session attend ici.
focus-move-back = Y retourner
focus-move-ask = Le moment de bouger et de s’étirer quelques minutes ? La session continue tant que vous ne la mettez pas en pause.
focus-move-now = Faire la pause
focus-move-not-now = Pas maintenant
stopped-hint = Où j’en suis, en une ligne, pour quand je reviens
stopped-title = Où vous en étiez
stopped-done = C’est fait
stopped-at = Où vous en étiez, { $when }
site-chats-covered = Les discussions se reposent pour l’instant : leur temps du jour est passé. Elles reviennent d’elles-mêmes.
weather-now = Maintenant
weather-this-morning = Ce matin
weather-this-afternoon = Cet après-midi
weather-this-evening = Ce soir
weather-tonight = Cette nuit
weather-tomorrow-morning = Demain matin
weather-tomorrow-afternoon = Demain après-midi
weather-tomorrow-evening = Demain soir
weather-tomorrow-night = La nuit suivante
weather-rain-chance = { $chance } %
weather-clear = Ciel dégagé
weather-mainly-clear = Plutôt dégagé
weather-partly-cloudy = Partiellement nuageux
weather-overcast = Couvert
weather-fog = Brouillard
weather-drizzle = Bruine
weather-freezing-drizzle = Bruine verglaçante
weather-rain = Pluie
weather-heavy-rain = Forte pluie
weather-freezing-rain = Pluie verglaçante
weather-snow = Neige
weather-showers = Averses
weather-snow-showers = Averses de neige
weather-thunderstorm = Orage
weather-hail = Orage avec grêle
weather-unknown = Inconnu
weather-credit = Données météo d’Open-Meteo.com
weather-choose = Choisir un lieu pour la météo
weather-change = Changer de lieu
weather-search = Une ville, un village
weather-privacy = Demande à Open-Meteo, avec les seules coordonnées de ce lieu.
weather-none = Pas encore de prévision.
weather-tip = Le temps qu’il fait à { $place }. Cliquez pour les heures et les jours à venir, et pour changer de lieu.
weather-tip-hours = Le temps à { $place } : maintenant, puis heure par heure pour les quatre prochaines heures. Cliquez pour les jours à venir, et pour changer de lieu.
sounds-title = Sons
sounds-focus = Pour se concentrer
sounds-white = Bruit blanc
sounds-pink = Bruit rose
sounds-brown = Bruit brun
sounds-calm = Vos enregistrements
sounds-calm-none = Les enregistrements mis dans un dossier « sounds » de vos notes viennent ici : pluie, mer, vent, grillons. Des enregistrements libres sont listés dans la documentation de Sioul (docs/sounds.md).
sounds-volume = Volume
sounds-stop = Arrêter
webauth-title = Clé de sécurité pour { $site }
webauth-account = Lequel des comptes de la clé ?
webauth-pin = Le code PIN de la clé ({ $left } essais restants).
webauth-pin-set = La clé n’a pas encore de code PIN : choisissez-en un, de { $length } caractères au moins.
webauth-pin-change = La clé demande un nouveau code PIN, de { $length } caractères au moins.
webauth-pin-field = Code PIN
webauth-pin-again = Le même code PIN, encore
webauth-pin-uv-locked = La vérification propre à la clé est bloquée : le code PIN est nécessaire.
webauth-pin-wrong = Pas ce code PIN.
webauth-pin-too-short = Trop court.
webauth-pin-invalid = Le code PIN contient des caractères que la clé ne prend pas.
webauth-pin-same = Le nouveau code PIN est l’ancien.
webauth-touch = Touchez de nouveau votre clé.
webauth-go = Continuer
webauth-retry = Réessayer
webauth-failed-timeout = La clé n’a pas été touchée à temps.
webauth-failed-not-registered = Cette clé n’est pas enregistrée pour ce site.
webauth-failed-already-registered = Cette clé est déjà enregistrée pour ce site.
webauth-failed-soft-block = Trop de codes PIN faux pour l’instant : débranchez la clé puis rebranchez-la.
webauth-failed-hard-block = Trop de codes PIN faux : la clé doit être réinitialisée avant de resservir.
webauth-failed-removed = La clé a été retirée pendant la demande de son code PIN.
webauth-failed-no-resident = La clé ne peut pas garder la connexion de ce site sur elle-même.
webauth-failed-no-uv = La clé ne peut pas vérifier qui vous êtes (ni code PIN, ni empreinte).
webauth-failed-no-blob = La clé ne peut pas garder ce que ce site demande.
webauth-failed-no-algorithm = La clé et le site n’ont aucune façon de signer en commun.
webauth-failed-full = La clé est pleine : une ancienne connexion doit partir d’abord.
webauth-failed-denied = Refusé sur la clé.
webauth-failed-cancelled = Annulé.
task-list-no-tasks = Cette liste ne garde pas de tâches.
task-list-greyed = Ne garde pas de tâche
task-move-ask = { $place } ne garderait pas : { $list }. La déplacer quand même ?
contact-moved = Contact déplacé.
contact-gone = Ce contact n’est plus là.
contact-read-only = Ce contact ne peut qu’être lu ici.
contact-book-move-ask = { $place } ne garderait pas : { $list }. Le déplacer quand même ?
move-anyway = Déplacer
loss-waits = ce qu’elle attend
loss-start = son jour de début
loss-due-time = l’heure de sa date
loss-repeat = sa répétition
loss-priority = son ordre
loss-kind = son type
loss-estimate = sa durée
loss-links = ses liens
loss-contacts = les personnes qui y sont liées
loss-categories = ses catégories
loss-margins = le temps gardé avant et après
loss-costs = ce qu’elle coûte et ce qu’elle apporte
loss-labels = les libellés de ses adresses et numéros
loss-birthday-no-year = un anniversaire sans son année
loss-vcard4 = ce que seul vCard 4.0 contient (genre, date anniversaire, relations)
mode-quiet = Le travail revient { $until }.
mode-time-off = Congés{ $label ->
    [none] {""}
   *[other] {""} : { $label }
}. Le travail revient { $until }.
mode-back-to-plan = Reprendre le plan du jour
mode-work-30 = Travailler encore une demi-heure
mode-work-60 = Travailler encore une heure
mode-work-120 = Travailler encore deux heures
mode-work-240 = Travailler encore quatre heures
quiet-title = Le reste de la journée
quiet-time-off-title = Congés
closing-put-away = Le travail est rangé jusqu’à { $back }.
# Après elle, sur l’écran une fois la journée de travail close (DoneDialog.qml, docs/reviews.md).
closing-yours = Le reste de la journée est à vous.
closing-done = Fait aujourd’hui : { $list }.
closing-worked = Avancé : { $list }.
closing-rest = Tout le reste a sa place, à partir de { $day }, dans l’ordre habituel.
closing-starts-with = { $day } commence par :
closing-change-step = Changer
closing-step-hint = Un repère et un geste : « Après le petit-déjeuner, ouvrir le formulaire »
closing-deadline = La date demandée pour « { $title } » est { $day }.
closing-ask-time = Demander un délai
closing-leave = Laisser pour { $day }
closing-resting = Le courrier et les tâches se reposent. Les codes à usage unique arrivent toujours, ainsi que les messages des proches que vous avez choisis.
closing-plan = S’arrêter avant d’être à bout fait partie du plan.
first-step-line = { $day }. Le plan commence par : { $step }
day-today = aujourd’hui
task-note-for-later = À noter pour le retour du travail : une ligne, hors de vue jusque-là
task-noted-for = Noté pour { $day }.
set-task-kinds = Types de tâches
set-task-kinds-help = Chaque type rassemble les tâches qui se font de la même façon : les appels ensemble, les formulaires ensemble. Renommez-en un sur place, retirez-le ou ajoutez les vôtres ; les tâches gardent le type qu’elles ont.
set-task-kinds-new = Un nouveau type
set-task-categories = Catégories
set-task-categories-help = Les catégories de vos tâches, les plus utilisées d’abord. Un nouveau nom la renomme sur toutes les tâches qui l’ont ; en retirer une l’enlève de toutes.
set-task-categories-remove = L’enlever de toutes les tâches
mode-working-late = Le travail reste en vue jusqu’à { $until }.
mode-usual-hours = Revenir aux heures habituelles
mode-work-now = Travailler maintenant
mode-work-now-help = Le travail montré quelles que soient les heures : jusqu’à ce que vous décochiez, que Sioul se ferme, ou que la prochaine journée de travail soit finie.
mode-work-now-line = Le travail est montré jusqu’à { $until }, à votre demande.
done-button = Fini pour aujourd’hui
done-button-help = Un mot sur la journée, si vous voulez ; puis les tâches du jour passent à leur prochain jour, et le travail se repose jusqu’à son retour.
first-step-clear = Ranger
set-language = Langue
set-language-help = La langue de toutes les phrases de Sioul.
set-language-system = Celle du système
set-language-fr = Français
set-language-en = English
realtime-on = Temps réel : chaque dossier est relevé toutes les minutes, et le Porche reste ouvert.
realtime-off = Temps réel arrêté : retour au rythme habituel.
ui-realtime = Temps réel
ui-realtime-help = Chaque dossier de chaque adresse, toutes les minutes, et le Porche ouvert : pour un code ou un mot de passe que vous attendez.
reason-hostile = lu comme hostile (insultes, harcèlement ou menaces) : ses mots ne sont pas montrés
reason-public = à votre adresse publique, au sujet de : { $topic }
topic-work = travail
topic-support = une question
topic-press = la presse
topic-thanks = des remerciements
topic-donation = un don
topic-other = autre chose
tone-calm = calme
tone-hostile = hostile
shield-none = Aucune adresse n’est protégée : mettre « shield = true » sur un compte, ou dans les réglages du Courrier.
shield-read = L’IA a lu { $n } { $n ->
    [one] message
   *[other] messages
} ; { $left } restent pour les prochains tours.
shield-words = Mots qui ont pesé : { $words }
tone-rude = grossier
porch-open-until = Ouvert jusqu’à { $time }.
porch-opened-anyway = Ouvert en dehors d’une fenêtre.
lane-public = Votre adresse publique, { $address }
lane-hostile = Hostile, écarté
lane-about-case = Le courrier qui correspond aux règles de ce projet.
lane-about-public = À { $address }, de personnes que vous n’avez pas laissées entrer, lu d’abord et trié par sujet : le travail en premier.
lane-about-people = Des expéditeurs que vous avez laissés entrer.
lane-about-screener = De quelqu’un de nouveau : laissez entrer, ou bloquez.
lane-about-filed = Lettres d’information et expéditeurs automatiques, à lire quand vous voulez.
lane-about-low = Des adresses que vous avez classées moins importantes.
lane-about-set-aside = Falsifié, empruntant un nom, indésirable ou bloqué. Rien n’est supprimé.
lane-about-hostile = Insultes, harcèlement ou menaces envoyés à votre adresse publique. Leurs mots restent cachés.
rule-order = Chaque message va dans la première file qui le prend, dans cet ordre : écarté, hostile, codes, projets, ce que vous vous envoyez, votre adresse publique, adresses moins importantes, lettres d’information, quelqu’un de nouveau, personnes que vous connaissez.
rule-people = Votre liste d’expéditeurs connus compte { $n } entrées. « Laisser entrer », dans un message de quelqu’un de nouveau, les y ajoute.
rule-screener = Un expéditeur qui n’est pas dans votre liste attend ici, sauf si un projet ou la règle des lettres d’information prend le message d’abord.
rule-filed = Un message qui dit venir d’une liste de diffusion (List-Id, List-Unsubscribe, Precedence: bulk) est une lettre d’information. Un expéditeur dont l’adresse contient l’un de ces mots est automatique : { $words }.
rule-low-none = Aucune adresse n’est encore classée moins importante : le rang d’une adresse se règle sur sa fiche dans Comptes.
rule-low = Adresses classées moins importantes : { $addresses }. Leurs codes arrivent quand même tout de suite.
rule-forged = Falsifié : le domaine de l’expéditeur dit ne pas l’avoir envoyé (DMARC), ou toutes les vérifications ont échoué.
rule-borrowed = Empruntant un nom : le nom affiché se réclame d’une marque, ou de votre propre domaine, auquel l’adresse n’appartient pas.
rule-spam = Indésirable : le filtre de votre fournisseur le dit.
rule-blocked = Bloqué : de votre liste d’expéditeurs bloqués ; ce courrier n’est jamais montré, jamais compté.
rule-case-none = Ce projet n’a pas encore de règle : aucun courrier n’y arrive de lui-même.
rule-route = { $parts }.
rule-part-domains = De { $list } (et ses sous-domaines)
rule-part-addresses = De { $list }
rule-part-subject = avec { $list } dans le sujet
rule-part-text = avec { $list } dans le texte
rule-and = , et{" "}
rule-public = Lu avant que vous ne le voyiez, par des listes de mots en français et en anglais ; le courrier hostile va dans « Hostile ». Relevé toutes les { $minutes } minutes.
rule-public-ai = Une IA le lit aussi, comme vous l’avez permis, et en dit le ton et le sujet plus finement.
rule-hostile = Les insultes qui vous visent, le harcèlement et les menaces arrivent ici. Le nom de l’expéditeur, le sujet et le texte restent cachés jusqu’à ce que vous choisissiez de les lire.
rule-where-shield = Le bouclier d’une adresse se règle sur sa fiche dans Comptes.
rule-where-rank = Le rang d’une adresse se règle sur sa fiche dans Comptes.
rule-where-blocked = Les expéditeurs bloqués sont dans Comptes, sous « Expéditeurs ».
rule-where-routes = Ses règles se choisissent sur sa page dans Projets.
hostile-someone = Quelqu’un
hostile-someone-at = Quelqu’un chez { $domain }
hostile-subject = Un message écarté comme hostile
ui-lane-help = Comment le courrier arrive ici
set-filed-words = Mots des expéditeurs automatiques
set-filed-words-help = Un expéditeur dont l’adresse contient l’un de ces mots est automatique : son courrier est rangé avec les lettres d’information.
set-words-add = Ajouter un mot
set-routes = Les règles de ce projet
set-routes-help = Le courrier va dans ce projet quand une règle correspond : chaque ligne remplie d’une règle doit correspondre. Plusieurs mots ou domaines sur une ligne : l’un d’eux suffit. Une réponse dans la conversation d’un message du dossier le suit.
set-route = Règle
set-route-domains = Des domaines
set-route-addresses = Des adresses
set-route-subject = Mots dans le sujet
set-route-text = Mots dans le texte
set-route-attachments = Mots dans le nom d’une pièce jointe
set-route-add = Ajouter une règle
set-route-comma = Séparés par des virgules
hostile-gate = Ce message a été écarté comme hostile.
hostile-gate-help = Ses mots ne sont pas montrés. Il peut attendre, aller à quelqu’un de confiance, ou partir. Le lire est votre choix, au moment qui vous convient.
hostile-forward = Transférer à quelqu’un de confiance
hostile-read = Le lire quand même
ui-choose = Choisir…
set-font-desktop = Celle du bureau
ui-reading = Lecture du texte

# Google : agendas et contacts par son CalDAV et son CardDAV, tâches par Google Tasks.
ui-add-google = Agendas, contacts et tâches Google
ui-add-google-note = Google ne prend aucun mot de passe d’un autre programme : vous vous connectez sur la page de Google, dans votre navigateur. Sans clé Google dans cette copie de Sioul, ou pour prendre la vôtre, vous la faites une fois, dans votre propre projet Google. Google garde moins qu’un serveur ouvert ; ce qu’il ne garde pas apparaît grisé.
ui-add-google-built-in = Google ne prend aucun mot de passe d’un autre programme : vous vous connectez sur la page de Google, dans votre navigateur, et Sioul garde l’accès dans le trousseau du système. Google garde moins qu’un serveur ouvert ; ce qu’il ne garde pas apparaît grisé.
ui-google-own-key = Utiliser une clé Google à moi
ui-google-steps = Faire votre clé Google (une fois, un quart d’heure environ)
ui-google-steps-text =
    1. Sur [console.cloud.google.com](https://console.cloud.google.com), créez un projet nommé Sioul.
    2. Dans *API et services → Bibliothèque*, activez **CalDAV API**, **CardDAV API** et **Google Tasks API**.
    3. Dans *Google Auth Platform* : *Branding*, un nom et votre adresse ; *Audience*, Externe, et vous-même comme utilisateur test ; *Accès aux données*, les champs d’application `…/auth/calendar`, `…/auth/carddav` et `…/auth/tasks`.
    4. *Clients → Créer un client → Application de bureau*. Copiez son identifiant et son secret tout de suite : Google ne montre le secret qu’une fois.
    5. *Audience → Publier l’application*. Si elle reste en test, Google coupe l’accès tous les sept jours.
    6. Collez les deux ici et connectez-vous. Google dit que l’application n’est pas validée : c’est la vôtre. Choisissez *Paramètres avancés*, puis *Accéder à Sioul*.

    Google supprime une clé inutilisée pendant six mois ; Sioul s’en sert à chaque synchronisation, elle reste.
ui-google-client-id = Identifiant client
ui-google-client-secret = Secret client
ui-google-sign-in = Se connecter avec Google
ui-google-waiting = Votre navigateur est ouvert sur la page de Google. Sioul attend ici.
ui-google-again = Se reconnecter
google-timeout = Google n’a pas répondu en cinq minutes : rien n’a changé.
google-denied = Google n’a donné aucun accès : rien n’a changé.
google-state = Une réponse est arrivée qui n’était pas pour Sioul : rien n’a changé.
google-other = Votre navigateur s’est connecté en tant que { $address }, pas à l’adresse donnée : rien n’a changé, et cet accès est rendu à Google.
google-cancelled = Arrêté : rien n’a changé.
sync-error-google-again = { $account } : Google demande de vous reconnecter.
sync-error-google-testing = { $account } : Google a mis fin à la connexion au bout de sept jours, comme il le fait tant que votre projet Google est en test. Dans Google Cloud : Google Auth Platform ▸ Audience ▸ Publier l’application ; puis reconnectez-vous : elle dure ensuite.
google-no-collections = Google ne crée ni agenda ni carnet d’adresses depuis un autre programme : créez-le sur les pages de Google.
google-fixed-collection = Google ne renomme et ne supprime ses agendas et carnets d’adresses que sur ses propres pages.
google-tasks-greyed = Google Tasks ne garde pas ceci.
task-limited = Cette liste est sur Google Tasks : ce que Google ne garde pas est grisé (un jour de début, une durée, un projet, un type, les heures de bureau, des catégories, ce que ça coûte, la répétition, l’attente d’une autre tâche, les étapes d’étapes).

# GitHub, une fois demandé : tickets et pull requests en tâches.
set-code-group = Code
set-github = Tickets et pull requests GitHub en tâches
set-github-help = Pour qui travaille sur GitHub : ce qui vous revient arrive dans une liste « GitHub » sur cet appareil, toutes les trente minutes, et le courrier de notification de GitHub y est lié. Rien n’est écrit sur GitHub.
set-github-token = Jeton GitHub
set-github-token-help = Un jeton à droits fins qui lit les tickets et les pull requests, rien de plus, gardé dans le trousseau du système. « Créer un jeton sur GitHub » ouvre la page de GitHub avec ses droits remplis : choisissez les dépôts (tous les vôtres, ou certains), créez-le, et collez-le ici.
set-github-token-kept = Un jeton est gardé.
set-github-token-none = Pas encore de jeton : rien ne vient de GitHub.
set-github-token-forget = Oublier le jeton
set-github-token-make = Créer un jeton sur GitHub
set-github-assigned = Qui vous sont attribués
set-github-assigned-help = Les tickets et pull requests dont vous êtes l’un des responsables.
set-github-reviews = Votre relecture demandée
set-github-reviews-help = Les pull requests dont la relecture vous est demandée nommément.
set-github-created = Ouverts par vous
set-github-created-help = Vos propres tickets et pull requests, tant qu’ils sont ouverts.
set-github-mentioned = Qui vous mentionnent
set-github-mentioned-help = Là où quelqu’un a écrit votre nom. Souvent nombreux : désactivé sauf si vous les voulez.
sync-error-github-token = GitHub a refusé le jeton : un nouveau se met dans les réglages des tâches, « Code ».
note-not-here = Pas encore sur cet appareil : sa synchronisation est peut-être en cours (Nextcloud, Dropbox).
ui-new = Nouveau
new-mail = Un message
new-task = Une tâche
new-event = Un événement
new-contact = Un contact
new-note = Une note
new-movement = Un mouvement de budget
new-time = Du temps passé
new-project = Un projet
ui-parameters = Paramètres
settings-tab-look = Affichage
settings-tab-hours = Heures
settings-tab-reminders = Rappels et notifications
settings-tab-files = Votre dossier et le partage
settings-tab-invoices = Factures
ui-refresh-all = Tout actualiser : courrier, agenda, tâches, contacts (F5)
ui-refreshing = Actualisation…
budget-add-budget = Budget
set-look-group = Langue et apparence
set-hours-group = Heures de travail et calme
set-hours-elsewhere = Les heures de travail, de démarches et les congés sont dans Paramètres : l’icône à curseurs en bas de la colonne de gauche. Les repas et le sommeil sont sur la page Santé.
porch-hours-none = Vos heures ne sont pas réglées : travail et démarches arrivent à toute heure.
porch-hours-some = Pas encore réglé : { $which }.
porch-hours-why = Chaque sorte d’heures amène ce qui lui revient et fait attendre le reste : le travail aux heures de travail, organismes et factures aux heures de démarches ; tout le reste du temps, ce sont vos loisirs.
porch-hours-set = Régler mes heures
porch-hours-leave = Laisser ainsi
site-column-narrow = Replier la liste sur ses icônes
site-column-widen = Afficher les noms des sites
set-senders-group = Qui peut vous écrire, et quand
set-safe = Sûrs
set-safe-help = Les amis, les collègues et la famille que vous choisissez. Une adresse, ou un motif avec * : *@example.org pour tout le monde là-bas, *@*.example.org pour ses sous-domaines. Un courrier falsifié ne compte jamais comme le leur.
set-neutral = Neutres
set-neutral-help = Toute personne qu’aucune liste ne nomme est neutre, les inconnus compris. Nommez quelqu’un ici pour le garder neutre dans un domaine ou une catégorie mis sur une autre liste.
set-blocked-all = Bloqués
set-blocked-all-help = Le spam et le harcèlement : mis de côté pour de bon, jamais montrés, jamais notifiés. L’entrée la plus précise l’emporte : une adresse marquée sûre le reste dans un domaine bloqué ici. Personne n’est bloqué pour partager un serveur ou un domaine avec quelqu’un d’autre.
sender-now-safe = { $entry } est sûr : son courrier vient aux moments cochés pour les sûrs.
sender-now-neutral = { $entry } est neutre : son courrier vient aux moments cochés pour les neutres.
sender-now-blocked = { $entry } est bloqué : son courrier est mis de côté pour de bon.
sender-standing = Son courrier
sender-unverified = L’expéditeur de ce message n’est pas vérifié : l’adresse peut appartenir à quelqu’un d’autre, elle ne change donc aucune liste.
sender-unverified-short = non vérifié
set-quiet-work = Ce qui est du travail
set-quiet-work-help = Les tâches de ces catégories, des projets pour un client et celles de GitHub sont du travail : elles viennent seulement aux heures de travail. Une tâche peut aussi dire à quoi elle sert, dans son panneau.
task-tag-add = étiquette
task-tag-remove = Retirer l’étiquette { $tag }
note-folder-new = Nouveau dossier
note-folder-new-inside = Nouveau dossier dedans…
note-new-here = Nouvelle note ici…
note-folder-rename = Renommer le dossier…
note-folder-remove = Retirer ce dossier vide
note-folder-remove-full = Retirer (le vider d’abord)
note-folder-not-empty = Ce dossier contient des notes ou des dossiers : déplacez-les ou mettez-les à la corbeille d’abord.
site-edit = Nom et adresse…
site-https-only = Seulement une adresse https:// : la connexion à un site ne circule jamais en clair.
budget-line-change = Modifier cette ligne…
set-projects-group = Projets
set-porch-projects = Projets montrés ici
set-porch-projects-help = Chaque projet coché a sa file sur le Porche. Le courrier des autres reste sur leur page dans Projets, où l’on crée les projets, les renomme et règle leurs routes.
loss-billable = si son temps se facture
time-export = Le temps facturable, en tableur (CSV)…
time-export-project = Projet
time-export-range = Période
time-export-from = Du
time-export-to = Au
time-export-save = Enregistrer…
time-range-this-week = Cette semaine
time-range-last-week = La semaine dernière
time-range-this-month = Ce mois-ci
time-range-last-month = Le mois dernier
time-range-all = Depuis toujours
time-range-custom = D’un jour à un autre
time-csv-what = Quoi
time-csv-hours = Heures
time-csv-rate = Taux horaire
time-csv-amount = Montant
time-csv-total = Total
time-exported = Enregistré dans { $path }.
task-field-billable = Facturé
task-billable-project = Comme son projet
task-billable-yes = Son temps se facture
task-billable-no = Non facturé
health-errands-list = Les démarches vont dans
health-list-here = sur cet appareil seulement
scan-ask = Aucun antivirus n’est installé sur cet appareil : { $name } ne sera pas vérifié. Ne l’ouvrez que si vous lui faites confiance. Pour que les fichiers soient vérifiés : { $hint }
scan-open-anyway = L’ouvrir sans vérification
scan-save-anyway = L’enregistrer sans vérification
scan-unavailable-short = Aucun antivirus n’a répondu ({ $detail }).
sounds-nature = La nature, faite ici
sounds-waves = Les vagues sur une plage
sounds-rain = La pluie
sounds-wind = Le vent dans les arbres
sounds-crickets = Les grillons la nuit
sounds-storm = Un orage au loin
watch-title = Votre montre
watch-none = Rien d’une montre pour l’instant. Choisissez le dossier où arrivent ses fichiers (les exports de Gadgetbridge, ou les ZIP d’export de Garmin), ou branchez votre montre : quand le bureau montre son dossier GARMIN, Sioul le lit. Rien ne quitte cet appareil ; aucun compte Garmin n’est utilisé.
watch-folder = Ses fichiers arrivent dans
watch-folder-choose = Choisir un dossier…
watch-offers = Des propositions douces entre deux tâches (une pause, une marche)
watch-synced = Dernières données : { $when }.
watch-steps = { $steps } pas aujourd’hui
watch-resting = Fréquence cardiaque au repos { $bpm } bpm
watch-resting-usual = Fréquence cardiaque au repos { $bpm } bpm (d’habitude { $usual })
watch-slept = Dormi { $time }, { $from }–{ $to }
watch-hours = { $h } h { $m }
watch-battery = Body Battery { $level }
watch-week = Cette semaine, une nuit : { $sleep } ; un jour : { $steps } pas
watch-curves = Aujourd’hui : fréquence cardiaque, stress selon Garmin, Body Battery
watch-imported = { $count ->
        [one] Un fichier de la montre lu.
       *[other] { $count } fichiers de la montre lus.
    }
watch-offer-move = Cinq minutes debout ?
watch-offer-move-text = Avant la suite. Aller jusqu’à la fenêtre compte.
watch-offer-pause = Le temps d’une courte pause ?
watch-offer-pause-text = Se lever, regarder loin, souffler lentement. Cinq minutes.
watch-offer-low-reserve = Votre réserve est basse.
watch-offer-low-reserve-text = Remettre le reste à demain et finir la journée ?
watch-offer-low-reserve-action = Fini pour aujourd’hui
watch-offer-walk = De la place pour marcher ?
watch-offer-walk-text = Vingt minutes, tranquillement, dehors, si vous voulez.
watch-morning-short-night = Une nuit courte. Des séances plus courtes aujourd’hui, et la tâche la plus dure tôt, ou demain ?
watch-morning-strain = Votre corps lutte peut-être contre quelque chose. Une journée plus légère ?
watch-morning-lighter = Une journée plus légère

## Partage entre vos appareils (Paramètres)
share-title = Entre vos appareils
share-help = Le courrier, les contacts, l’agenda et les tâches sur un serveur rejoignent déjà vos autres appareils. Le reste peut voyager par un dossier que votre synchronisation transporte (Nextcloud, Dropbox, Syncthing), scellé par une phrase de passe, pour que son serveur ne le lise jamais : ce que Sioul garde sur cet appareil (réglages, qui peut vous écrire, temps, brouillons, factures, santé, montre, listes gardées ici) et, quand aucune synchronisation ne les transporte, vos notes, vos projets et vos papiers.
share-off = Pas de partage : tout reste sur cet appareil.
share-on = Partagé par { $folder }.
share-others = { $count ->
    [one] Avec un autre appareil (dernières nouvelles : { $when }).
   *[other] Avec { $count } autres appareils (dernières nouvelles : { $when }).
}
share-alone = Aucun autre appareil pour l’instant : sur l’autre, choisissez le même dossier et tapez la même phrase de passe.
share-last = Dernier échange ici : { $when }.
share-outside = Vos notes et vos projets ({ $store }) ne semblent pas être dans un dossier que votre synchronisation transporte : l’autre appareil ne les verrait pas. Allumez Notes, et Projets et argent, ci-dessous pour les faire voyager par ce dossier, scellés ; ou déplacez-les dans un dossier synchronisé (et choisissez-le à nouveau dans Paramètres ▸ Votre dossier et le partage).
share-phones = Un téléphone ne voit ce dossier que si son application de synchronisation le transporte, et certaines n’en transportent que quelques-uns : l’eDrive de Murena transporte Documents (avec Pictures, Music…), pas le reste de votre cloud. Pour atteindre un tel téléphone, partagez par un dossier dans Documents : arrêtez le partage, puis choisissez-en un là.
folder-not-on-device = Ce dossier n’est pas sur cet appareil : choisissez-en un où votre application de synchronisation garde ses fichiers (Documents, avec l’eDrive de Murena).
folder-browser-title = Choisir un dossier
folder-browser-up = Remonter
folder-browser-choose = Choisir ce dossier
folder-browser-shared = partagé par vos autres appareils
folder-browser-empty = Aucun dossier dedans.
folder-browser-unreadable = Sioul ne peut pas lire ce dossier.
share-folder = Dossier
share-choose = Choisir…
share-passphrase = Phrase de passe
share-again = Encore une fois
share-passphrase-hint = Quelques mots que vous n’oublierez pas, au moins 12 caractères, tapés une fois sur chaque appareil et gardés dans son trousseau. Jamais envoyée nulle part : perdue, elle ne se retrouve pas (recommencez avec un nouveau dossier ; rien d’ici n’est perdu).
share-passphrase-known = Un autre appareil partage par ce dossier : tapez la phrase de passe choisie là-bas.
share-start = Partager
share-now = Échanger maintenant
share-stop = Arrêter le partage
share-short = Au moins 12 caractères : quelques mots font une bonne phrase de passe.
share-differ = Les deux phrases de passe diffèrent.
share-wrong = Ce n’est pas la phrase de passe choisie sur votre autre appareil.
share-no-folder = Choisissez d’abord un dossier.
share-other-seal = Un appareil de ce dossier scelle avec une autre phrase de passe : ses changements sont laissés de côté.
share-unreadable = { $file } n’a pas pu être lu : ce qui est venu pour lui attend qu’il puisse l’être.
share-key-missing = Tapez à nouveau la phrase de passe sur cet appareil : le trousseau ne la garde plus.
share-not-shared = Pas partagé : la disposition des pages sur cet écran, les dossiers où chaque appareil garde ses fichiers, les notifications du navigateur de cet appareil, vos propres clés PGP (copiez-les à la main), les caches.
share-found = Déjà partagé par vos autres appareils (choisissez-en un) :
share-files-access = Pour lire le dossier que votre application de synchronisation transporte (eDrive, Syncthing, FolderSync…), Sioul a besoin de l’accès d’Android à vos fichiers.
share-files-allow = Autoriser l’accès aux fichiers
share-parts = Ce qui voyage depuis cet appareil
share-parts-help = Chaque appareil choisit pour lui-même. Une partie éteinte ici reste telle quelle sur vos autres appareils : rien n’y est retiré. Rallumée, elle les rejoint comme le ferait un nouvel appareil.
share-part-settings = Réglages et comptes
share-part-settings-carries = Vos réglages et vos comptes (jamais leurs mots de passe), les liens entre les choses, où le Porche a été fermé, les courriels dont vous avez dit qu’ils ne sont pas des paiements.
share-part-senders = Expéditeurs
share-part-senders-carries = Qui peut vous écrire (connus, bloqués, sûrs, neutres, restreints), ce que le bouclier a lu, les clés publiques des autres.
share-part-health = Santé
share-part-health-carries = Les médicaments, les ordonnances et les prises.
share-part-time = Temps
share-part-time-carries = Le temps noté, la séance en cours, les choix du jour, où vous vous êtes arrêté, travailler tard ou fini pour aujourd’hui.
share-part-drafts = Brouillons et factures
share-part-drafts-carries = Les courriels en cours d’écriture, les factures faites.
share-part-projects = Projets et argent
share-part-projects-carries = De votre dossier de notes : les projets et leurs routes de courrier, les budgets, les comptes et mouvements bancaires, les contrats.
share-part-watch = Montre
share-part-watch-carries = Les journées de votre montre.
share-part-lists = Listes gardées ici
share-part-lists-carries = Les agendas et contacts gardés sur cet appareil seulement.
share-part-notes = Notes
share-part-notes-carries = Votre dossier de notes : les notes, leurs images, PDF et mémos, les lettres numérisées. Chaque fichier scellé à part, seul ce qui a changé envoyé ; les fichiers de plus de 64 Mo restent.
share-part-papers = Papiers
share-part-papers-carries = Le portefeuille de papiers et ses fichiers.
share-part-carried = Votre dossier de notes ({ $store }) est déjà transporté par une application de synchronisation : transporté ici aussi, les deux déferaient les changements l’une de l’autre. Pour le partager ici, déplacez-le dans un dossier qu’aucune synchronisation ne transporte, ou laissez-le à cette synchronisation.
share-part-sent = Dernier envoi : { $when }.
share-part-received = Dernière réception : { $when }.
share-part-quiet = Rien d’échangé pour l’instant.
share-conflict = Deux appareils ont changé le même fichier : les deux versions sont gardées, l’autre sous le nom « { $copy } ».
share-conflict-gone = Un fichier changé ici a été retiré sur un autre appareil : ce qui a changé est gardé sous le nom « { $copy } ».
share-damaged = { $file } est arrivé abîmé d’un autre appareil : votre copie ici reste telle quelle jusqu’à ce qu’une entière arrive.
share-too-big = { $file } dépasse 64 Mo : il reste sur cet appareil.
share-history-help = Avant qu’un changement venu d’un autre appareil soit écrit dans un fichier ici, le fichier tel qu’il était est gardé sur cet appareil : les 20 dernières versions de chaque fichier, et toutes celles des 30 derniers jours. Elles ne sont jamais partagées.
share-history-show = Voir les versions précédentes
share-history-hide = Cacher les versions précédentes
share-history-empty = Rien de gardé pour l’instant.
share-versions = { $count ->
    [one] Une version
   *[other] { $count } versions
}
share-put-back = Remettre
share-put-back-done = { $file } est revenu comme il était ({ $when }). Le fichier tel qu’il était juste avant est gardé dans la liste aussi.
share-history-more = { $count ->
    [one] Un fichier de plus, changé avant : tapez une partie de son nom pour le trouver.
   *[other] { $count } fichiers de plus, changés avant : tapez une partie d’un nom pour les trouver.
}
share-history-filter = Trouver un fichier par son nom
share-put-back-whole = { $file } revient comme il était ({ $when }) ; le fichier tel qu’il est maintenant est gardé dans la liste.
share-put-back-nothing = { $file } contient déjà ce que contenait cette version : rien à remettre.
share-put-back-entries = { $file } comme il était ({ $when }) : { $changed ->
    [one] une entrée revient à ce qu’elle était
   *[other] { $changed } entrées reviennent à ce qu’elles étaient
}, { $returning ->
    [one] une retirée depuis revient
   *[other] { $returning } retirées depuis reviennent
} ; { $kept ->
    [one] une ajoutée depuis reste
   *[other] { $kept } ajoutées depuis restent
}, ici et sur vos autres appareils.
share-putting-back = Remise en cours…
share-estimate = { $count ->
    [one] Un fichier
   *[other] { $count } fichiers
}, { $size } en tout, voyageraient depuis cet appareil, chacun scellé à part ; { $big ->
    [0] aucun n’est trop gros.
    [one] un de plus de 64 Mo reste.
   *[other] { $big } de plus de 64 Mo restent.
} Allumer ?
share-estimating = Calcul de ce qui voyagerait…
share-switch-on = Allumer
share-vanished = { $count ->
    [one] Un fichier est parti
   *[other] { $count } fichiers sont partis
} d’un coup de { $folder } ici : rien n’en est retiré sur vos autres appareils tant que vous ne le dites pas (un disque non monté, un dossier déplacé, un accès retiré auraient le même air).
share-vanished-confirm = Les retirer partout
share-older-copy = Une copie plus ancienne a été remise ici à la main (sa date est antérieure à la dernière version vue) : cette version est revenue, la copie ancienne gardée à côté sous le nom « { $copy } ». Pour envoyer une version plus ancienne, utilisez Remettre ci-dessous.
share-missing = { $file } a changé sur un autre appareil il y a un jour, mais son contenu n’est pas arrivé : l’application de synchronisation transporte-t-elle encore le dossier de partage ?
share-no-room = { $file } attend : cet appareil manque de place pour lui (et pour la copie gardée avant de l’écrire).
share-name-clash = { $file } attend : un autre fichier ici n’en diffère que par la casse ou les accents, ce que le stockage de cet appareil prend pour un seul.
share-refused = { $file } n’est pas écrit : sa place passe par un lien, ou tombe dans le dossier de partage ou dans celui de Sioul.
share-not-text = { $file } reste ici : son nom n’est pas un texte que Sioul peut transporter.
share-emptied = { $file } est vide ici : rien n’en est retiré sur vos autres appareils pendant dix minutes.
share-files-unreadable = Les notes et les papiers attendent : sans l’accès d’Android à tous vos fichiers, Sioul ne voit pas ceux faits par d’autres applications, et les croirait partis.

## Rappels avant les dates
reminder-event = { $when } · { $what }
reminder-asked = Demandé pour { $date }
reminder-wait = L’attente après « { $before } » est finie.
reminder-payment = Prévu le { $date }.
remind-nothing = Rien à rappeler dans les deux semaines qui viennent.
remind-told = dit
remind-running = Les rappels sont déjà surveillés sur cet appareil.
remind-watching = Rappels surveillés : chacun vient une fois, en notification discrète. Ctrl+C arrête.
reminder-open = Ouvrir
reminders-closed-unavailable = Pas encore sous Windows : les rappels viennent quand la fenêtre de Sioul est ouverte.
reminders-closed-no-command = La commande « sioul » n’est pas installée à côté de Sioul : les rappels viennent quand la fenêtre est ouverte.
set-reminders-group = Rappels
set-reminders-events = Événements, le jour travaillé d’avant
set-reminders-events-help = Une demi-heure avant la fin du travail, le jour travaillé qui précède un événement : quoi, quand, où. Les alarmes que porte un événement sont dites à leur heure aussi.
set-reminders-asked = Dates demandées : jours travaillés avant
set-reminders-asked-help = Au début du travail, tant de jours travaillés avant une date demandée ; 0 pour aucun. Un rappel par date, jamais répété.
set-reminders-waits = Une attente finie
set-reminders-waits-help = Quand une attente après une étape faite est finie (une réponse due), une fois, quand le travail est là.
set-reminders-payment = Paiements prévus : jours travaillés avant
set-reminders-payment-help = Au début du travail, tant de jours travaillés avant un paiement prévu (une facture, un impôt) ; 0 pour aucun. Les prélèvements qui partent seuls ne sont pas rappelés : la veille sur l’argent dit quand le compte ne les tiendra pas.
set-reminders-closed = Fenêtre de Sioul fermée
set-reminders-closed-help = Votre session lance un petit veilleur (« sioul remind --watch ») qui dit les rappels quand la fenêtre est fermée ; rien d’autre ne tourne. Chaque rappel ne vient toujours qu’une fois.
paper-reminder = { $title } : valable jusqu’au { $date }
paper-renew-identity = Le renouvellement prend des semaines (un rendez-vous, puis la fabrication) : une étape maintenant ?
paper-renew-health = Elle ne se renouvelle pas seule : redemandez-la avant la fin.
paper-renew-home = Un nouveau devrait venir ; demandez-le sinon.
paper-renew-money = Un nouveau devrait venir ; demandez-le sinon.
paper-renew-warranty = Sa garantie finit : quelque chose à signaler avant ?
paper-renew-other = Il finit bientôt.
ui-papers = Papiers
new-paper = Un papier
papers-help = Les papiers demandés encore et encore, chacun avec son fichier et sa durée. Gardés dans le dossier des notes (papers/) : ils voyagent avec lui. Un rappel vient quand l’un est à renouveler.
papers-add = Ajouter un papier
papers-none = Aucun papier pour l’instant. Ajoutez-en un depuis un fichier (un scan, une photo), ou gardez une pièce jointe d’un courrier avec « Garder dans les papiers ».
papers-new = Un nouveau papier
papers-change = Un papier
papers-kind = Quoi
papers-title = Nom
papers-title-hint = Passeport, Quittance septembre…
papers-file = Fichier
papers-choose = Choisir…
papers-issued = Délivré le
papers-until = Valable jusqu’au
papers-until-hint = Pour un passeport, une carte d’identité ou une garantie, une fin est proposée depuis la délivrance quand aucune n’est donnée. Quittances, bulletins de paie et attestations n’ont pas de fin : ils sont dits plus vieux que trois mois.
papers-holder = À qui
papers-holder-hint = Dans un foyer : à qui est ce papier
papers-notes = Notes
papers-remove = Retirer
papers-remove-ask = Ce papier quitte le portefeuille. Son fichier reste où il est.
papers-file-missing = Son fichier n’est pas encore sur cet appareil : la synchronisation n’est peut-être pas finie.
papers-plan-renewal = Prévoir le renouvellement
papers-renewal-planned = Renouvellement prévu
papers-keep = Garder dans les papiers
papers-keep-help = Vérifiée par l’antivirus, puis gardée dans le portefeuille de papiers : vous dites ce que c’est.
papers-kept = { $name } est dans vos papiers : dites ce que c’est.
papers-attach = Un papier
papers-none-to-attach = Aucun papier avec un fichier pour l’instant
papers-no-store = Les papiers vivent dans le dossier des notes : choisissez-le d’abord (Paramètres ▸ Votre dossier et le partage).
papers-no-title = Un nom, s’il vous plaît : « Passeport », « Quittance septembre ».
papers-bad-date = « { $date } » ne se lit pas comme une date.
papers-no-file = { $path } est introuvable.
papers-gone = Ce papier n’est plus dans le portefeuille.
papers-list = Papiers
paper-valid = valable jusqu’au { $date }
paper-renew = valable jusqu’au { $date } : le moment de le renouveler
paper-ended = fini le { $date }
paper-fresh = du { $date }
paper-old = du { $date }, plus vieux que ce qui est demandé d’habitude
paper-renew-task = Renouveler : { $title }
paper-family-identity = Identité
paper-family-health = Santé
paper-family-home = Logement
paper-family-money = Argent
paper-family-warranty = Garanties
paper-family-other = Autres papiers
paper-kind-identity = Carte d’identité
paper-kind-passport = Passeport
paper-kind-residence = Titre de séjour
paper-kind-driving = Permis de conduire
paper-kind-health-card = Carte Vitale ou attestation de droits
paper-kind-health-cover = Complémentaire santé (CSS, mutuelle)
paper-kind-insurance = Attestation d’assurance
paper-kind-tax-notice = Avis d’impôt
paper-kind-rent-receipt = Quittance de loyer
paper-kind-bank-details = RIB
paper-kind-payslip = Bulletin de paie
paper-kind-certificate = Attestation, certificat
paper-kind-warranty = Garantie, preuve d’achat
paper-kind-other = Autre
paper-steps-identity = Une pré-demande sur ants.gouv.fr, puis un rendez-vous dans une mairie qui les prend (n’importe laquelle). Apporter l’ancienne carte, une photo de moins de six mois, un justificatif de domicile.
paper-steps-passport = Une pré-demande sur ants.gouv.fr avec un timbre fiscal acheté en ligne, puis un rendez-vous dans une mairie qui les prend (n’importe laquelle). Apporter l’ancien passeport, une photo de moins de six mois, un justificatif de domicile.
paper-steps-residence = La demande sur le site de l’ANEF (administration-etrangers-en-france.interieur.gouv.fr), dès quatre mois avant la fin.
paper-steps-driving = Sur ants.gouv.fr.
paper-steps-health-cover = La redemander sur votre compte ameli (ou par le formulaire), deux à quatre mois avant la fin : elle ne se renouvelle pas seule.
time-budget = Jusqu’au { $date } : environ { $need } d’étapes, { $room } de place.
time-budget-short = Jusqu’au { $date } : environ { $need } d’étapes, { $room } de place. Demander plus de temps, ou alléger le plan ?
task-field-energy = Ce que ça coûte
task-energy-usual = Comme d’habitude
task-energy-light = Léger
task-energy-heavy = Lourd
task-energy-rest = Ça recharge (une marche, de la musique)
task-field-before = Avant : y aller, se préparer
task-field-after = Après : revenir
task-margins-hint = Gardé libre dans le plan autour, jamais compté comme une pause.
task-field-cognitive = Réflexion demandée
task-field-emotional = Émotions remuées
task-field-anxiety = Anxiété suscitée
task-field-body = Corps et sens
task-field-gain = Ce que ça apporte
task-rating-unsaid = —
# Une note en curseur : non dite tant qu’aucune valeur n’est donnée (RatingSlider.qml).
rating-none = Non noté
rating-clear = Effacer la note
# Ce qui était prévu, à côté d’une note ressentie pas encore donnée.
rating-foreseen = prévu : { $value }
# Les mots sous chaque curseur, à 0, 5 et 10 (docs/research/capacity-budget.md, critères 2, 3, 5 ; wellbeing-gain.md, G1).
rating-cognitive-0 = sans y penser
rating-cognitive-5 = attention soutenue
rating-cognitive-10 = toute ma concentration
rating-emotional-0 = rien à cacher ni à porter
rating-emotional-5 = des émotions à contenir
rating-emotional-10 = beaucoup à cacher ou à porter
rating-anxiety-0 = aucune appréhension
rating-anxiety-5 = appréhension avant ou après
rating-anxiety-10 = angoisse bien avant et après
rating-gain-0 = ne m’a rien apporté
rating-gain-5 = un peu de repos, de plaisir ou de fierté
rating-gain-10 = m’a nettement apporté repos, joie ou fierté
rating-body-0 = rien de physique ni de sensoriel
rating-body-5 = debout, bruit ou lumière qui fatiguent un peu
rating-body-10 = épuisant : foule, bruit, longue station debout
# Ce que ça coûte, dès qu’un coût est noté : calculé, pas choisi (« Lourd, d’après les curseurs »).
task-energy-computed = { $level }, d’après les curseurs
# Les détails d’une tâche, puis son formulaire.
ui-details = Détails
# Après une tâche faite, si vous voulez le dire (FeltRatings.qml) : jamais redemandé, jamais compté.
felt-ask = Comment ça s’est passé ?
felt-hint = Les marques pâles sont ce que vous aviez prévu. Notez ce que vous voulez dire ; le reste demeure en blanc.
felt-kept = Gardé comme vous l’avez ressenti, à côté de ce que vous aviez prévu.
# « Qu’est-ce qui rend cela difficile ? » : l’appréhension ou l’ennui, deux minutes proposées ; rien ne démarre sans vous.
task-hard-two = Deux minutes suffisent pour commencer.
# Le formulaire d’une nouvelle tâche : créée dès que son titre est donné.
ui-new-task = Nouvelle tâche
task-new-title = Titre de la nouvelle tâche
task-new-made-when = Créée dès qu’elle a un titre ; chaque champ est ensuite gardé au fur et à mesure.
task-new-tied = Liée à : { $title }
task-why-heavy-fog = Une lourde, un jour de brouillard : rien de plus léger n’est libre. Ses deux premières minutes suffisent peut-être.
task-rest-offer = Après quelque chose de lourd, quelque chose qui recharge :
task-mode-day = La journée
day-all-day = Toute la journée : { $what }
day-margin = Autour : { $what }
overlap-today = Aujourd’hui, deux événements en même temps : { $first } et { $second }. Le temps d’y aller et d’en revenir compte.
overlap-day = { $day }, deux événements en même temps : { $first } et { $second }.
overlap-open = Ouvrir « { $title } »
overlap-set-aside = Ne plus en parler
day-more = { $count ->
    [one] Une étape de plus d’aujourd’hui ne tient pas avant la fin de la journée : elle garde sa place dans le plan.
   *[other] { $count } étapes de plus d’aujourd’hui ne tiennent pas avant la fin de la journée : elles gardent leur place dans le plan.
}
day-empty = Rien à poser aujourd’hui : aucun événement, aucune étape que le plan donne aujourd’hui.
day-now = maintenant
# Déplacer en faisant glisser : la frise de la page Santé, le jour et la semaine de l’agenda, la journée des Tâches.
drag-done = { $what } : { $time }.
drag-done-plain = C’est fait.
day-let-plan = Laisser le plan la placer
day-let-plan-done = Le plan place de nouveau « { $title } ».
agenda-read-only = Ce calendrier ne peut qu’être lu.
agenda-move-which = Déplacé à { $time } : cette fois seulement, ou toutes les fois ?
agenda-move-set-days = Sa règle fixe ses jours : déplacez cette fois seulement, ou changez sa répétition dans son formulaire.
health-readable-done = Terminé
health-readable-help = Un appui long prend un repas, un repos ou la nuit ; faites-le ensuite glisser. Par l’un de ses bords, seule cette fin change.
routine-admin = Le créneau des démarches
routine-admin-porch = Le Porche : ce qui est venu
routine-admin-next = L’étape suivante : { $title }
routine-admin-stop = Où vous vous arrêtez, en une ligne
routine-no-title = Un nom, s’il vous plaît.
routine-no-steps = Une étape par ligne, avec ses minutes : « 10 min Ouvrir le Porche ».
routines = Routines
routines-help = Des étapes jouées une à une, la suivante dite avant qu’elle vienne. Une par ligne, avec ses minutes : « 10 min Ouvrir le Porche », « Faire un thé 5 ».
routines-new = Une nouvelle routine
routines-play = Jouer
routines-change = Changer
routines-remove = Retirer
routines-title = Nom
routines-steps = Étapes
routines-auto = L’étape suivante commence seule quand le temps est fini
routines-builtin = Faite de ce qui est là maintenant : le Porche, l’étape suivante du plan, une ligne pour revenir.
routine-next = Ensuite : { $title }
routine-last = La dernière étape.
routine-left = encore { $minutes } min
routine-over = Son temps est fini : fait, ou quelques minutes de plus ?
routine-done = Fait
routine-skip = Passer
routine-more = +5 min
routine-pause = Pause
routine-resume = Reprendre
routine-stop = Arrêter
routine-open = Ouvrir
routine-finished = C’était tout.
contract-reminder = { $title } se renouvelle le { $date }
contract-reminder-notice = Pour l’arrêter, le préavis doit partir au plus tard le { $by }.
contract-reminder-free = Il peut être arrêté jusque-là.
contracts = Contrats et abonnements
contracts-add = Ajouter un contrat
contracts-none = Aucun contrat noté pour l’instant. Loyer, énergie, téléphone, assurances, abonnements : chacun avec sa date de renouvellement et comment l’arrêter.
contracts-new = Un nouveau contrat
contracts-change = Un contrat
contracts-title-hint = Assurance habitation, Électricité, Téléphone…
contracts-party = Avec
contracts-reference = Votre numéro chez eux
contracts-preset = Payé par
contracts-started = Depuis
contracts-renews = Se renouvelle le
contracts-every-none = Pas de renouvellement
contracts-every-month = chaque mois
contracts-every-year = chaque année
contracts-notice = Préavis
contracts-days = jours
contracts-cancel = Pour l’arrêter
contracts-cancel-hint = Sa page de résiliation (https://…), ou où écrire
contracts-covers = Ce qu’il couvre
contracts-covers-hint = Responsabilité civile, protection juridique, bris de glace…
contracts-covers-line = Couvre : { $covers }
contracts-ended = Fini le
contracts-remove-ask = Ce contrat quitte la liste. Rien n’est envoyé à personne.
contracts-stop = L’arrêter…
contracts-stop-page = Ouvrir sa page de résiliation
contracts-stop-letter = Écrire la lettre
contracts-stop-ended = Il est fini
contracts-suggestions = Paiements réguliers sans contrat noté :
contracts-note-it = Le noter
contracts-keep = Garder comme contrat…
contract-cost-month = { $amount } par mois
contract-cost-year = { $amount } par an
contract-ended = Fini le { $date }.
contract-renews-notice = Se renouvelle le { $date } ; pour l’arrêter, le préavis doit partir au plus tard le { $by }.
contract-renews-late = Se renouvelle le { $date } ; le préavis pour ce renouvellement ne peut plus partir à temps.
contract-renews = Se renouvelle le { $date } ; il peut être arrêté jusque-là.
contract-open-notice = Pas de date de renouvellement ; pour l’arrêter, { $days } jours de préavis.
contract-open = Pas de date de renouvellement ; il peut être arrêté à tout moment.
contract-letter-reference = n° { $reference }
contract-letter-subject = Résiliation : { $title } { $reference }
contract-letter-body = Madame, Monsieur,

    Par la présente, je vous demande de résilier mon contrat { $title } { $reference } à la première date possible.

    Je vous remercie de bien vouloir me confirmer par écrit cette résiliation et sa date d’effet.

    Veuillez agréer, Madame, Monsieur, mes salutations distinguées.
contract-kind-rent = Loyer
contract-kind-energy = Énergie
contract-kind-telecom = Téléphone, internet
contract-kind-insurance = Assurance
contract-kind-health = Complémentaire santé
contract-kind-subscription = Abonnement
contract-kind-hosting = Hébergement, domaine
contract-kind-bank = Banque
contract-kind-other = Autre
contract-rule-rent = Un locataire donne trois mois de préavis, un mois pour un meublé ou en zone tendue ; votre bail dit lequel.
contract-rule-energy = L’électricité et le gaz se changent ou s’arrêtent à tout moment, sans frais.
contract-rule-telecom = Après les douze premiers mois d’engagement, téléphone et internet s’arrêtent à tout moment, avec au plus dix jours de préavis.
contract-rule-insurance = Après sa première année, une assurance habitation, auto ou affinitaire s’arrête à tout moment ; elle finit un mois plus tard (loi Hamon). Avant, comme le contrat le dit, souvent deux mois avant l’échéance.
contract-rule-health = Après sa première année, une complémentaire santé s’arrête à tout moment ; elle finit un mois plus tard.
contract-rule-subscription = En ligne, un abonnement s’arrête aussi facilement qu’il se prend : un bouton de résiliation en trois clics depuis juin 2023.
contract-rule-hosting = Comme les conditions du prestataire le disent ; un domaine non renouvelé est perdu après son délai de grâce.
contract-rule-bank = Un compte courant se ferme à tout moment ; une nouvelle banque peut le faire pour vous (mobilité bancaire).
contract-rule-other = Comme le contrat le dit.
reminder-payment-held = Prévu le { $date } ; le compte le tiendra.
reminder-payment-short = Prévu le { $date } ; il manquera alors { $short } sur le compte.
money-missed-out = { $label } ({ $amount }) n’est pas sorti du compte
money-missed-in = { $label } ({ $amount }) n’est pas arrivé
money-missed-why = Attendu vers le { $date }, absent des mouvements de votre banque. Un prélèvement qui s’arrête ne le dit pas : à regarder.
money-short = Le compte risque de ne pas tenir { $label } le { $date }
money-short-reserve = Il manquera alors { $short } ; { $reserve } le couvre.
money-short-ask = Il manquera alors { $short }. Mettre de l’argent avant, ou demander une date plus tard ?
bank = La banque
bank-import = Prendre un export…
bank-files = Exports bancaires
bank-balance = { $amount } le { $date }, d’après le fichier de votre banque.
bank-coming = Cette semaine :
bank-forecast = Le mois qui vient, avec ce qui est attendu :
bank-accounts = Comptes bancaires
bank-accounts-help = Là où est vraiment votre argent : un compte courant, PayPal, Stripe. Chacun prend ses propres exports, remplit les budgets que vous choisissez, et vos réserves le renflouent dans votre ordre. Tant qu’aucun n’est déclaré, un export pris ne nourrit que la veille ci-dessous. Gardé dans le dossier de vos projets (sioul-budgets.toml, sioul-bank.toml) ; rien n’est envoyé nulle part.
bank-account-new = Nouveau compte bancaire
bank-account-edit = Compte bancaire
bank-account-title = Nom
bank-account-title-hint = Ma banque, PayPal…
bank-account-kind = Type
bank-kind-bank = Une banque
bank-kind-paypal = PayPal
bank-kind-stripe = Stripe
bank-kind-other = Autre chose
bank-account-fills = Remplit
bank-account-fills-help = Ses mouvements vont à ces budgets, selon vos règles et les paiements réguliers qu’ils représentent ; le premier prend le reste.
bank-account-first = le reste
bank-account-earlier = Plus tôt dans l’ordre
bank-account-floor = Gardé au-dessus de
bank-account-topped-by = Renfloué par
bank-account-remove = Le retirer
bank-account-remove-yes = Le retirer pour de bon
bank-account-remove-ask = Ses mouvements restent dans sioul-bank.toml et ses règles dans le fichier des budgets ; ses budgets ne les comptent plus.
bank-account-no-export = Aucun export pris encore : « Prendre un export… ».
bank-account-fills-line = Remplit { $budgets }.
bank-account-fills-none = Il ne remplit encore aucun budget : ses mouvements ne comptent nulle part. « Modifier » pour choisir.
bank-account-topped = Renfloué par { $reserves }.
bank-account-topped-floor = Gardé au-dessus de { $floor }, renfloué par { $reserves }.
bank-rules = Règles…
bank-rules-title = Où vont les mouvements de { $account }
bank-rules-help = Un mouvement dont le libellé contient un des mots d’une règle va où la règle le dit ; une règle sans mots prend tous les mouvements de son sens (toutes les sorties vers un budget). La première règle qui tient l’emporte. Mots : séparés par des virgules, sans tenir compte des majuscules ni des accents. L’argent passé entre vos propres comptes est reconnu d’abord, quand les exports des deux côtés sont lus. Sans règle, un mouvement va avec le paiement régulier qu’il représente, sinon au premier budget du compte.
bank-rules-none = Aucune règle encore.
bank-rule-words = Mots du libellé, séparés par des virgules
bank-rule-direction-any = Entrée ou sortie
bank-rule-direction-debit = Sortie
bank-rule-direction-credit = Entrée
bank-rule-to-reserve = Avec { $reserve }
bank-rule-everywhere = tous les comptes
bank-rule-here = ce compte
bank-movements = { $count ->
    [one] Son dernier mouvement
   *[other] Ses { $count } derniers mouvements
}
bank-movement-auto = Selon les règles
bank-movement-where = Où il va
bank-place-reserve = Avec { $reserve }, par { $budget }
bank-place-transfer = Passé avec { $account }
bank-place-known = Compté d’après le courrier ({ $label })
bank-place-none = Dans aucun budget
bank-why-hand = votre choix
bank-why-mail = le même paiement qu’une ligne venue du courrier
bank-why-rule = par une règle
bank-why-between = entre vos comptes
bank-why-preset = un paiement régulier
bank-why-first = son premier budget
bank-why-unknown = rien ne dit où
bank-unplaced = { $count ->
    [one] Un mouvement ne va dans aucun budget : choisissez les budgets que ce compte remplit (« Modifier »).
   *[other] { $count } mouvements ne vont dans aucun budget : choisissez les budgets que ce compte remplit (« Modifier »).
}
money-topup-now = { $reserve } : { $amount } à verser avant le { $date }, quand { $payment } part ; l’argent arrive tout de suite.
money-topup-ask = { $reserve } : demandez { $amount } d’ici le { $ask } ; il faut environ { $days } jours, et { $payment } part le { $date }.
money-topup-late = { $reserve } : demandez { $amount } aujourd’hui ; il faut environ { $days } jours, l’argent arrivera donc après { $payment } ({ $date }).
reserve-new = Nouvelle réserve
reserve-edit = Réserve
reserve-title = Nom
reserve-title-hint = Livret A, assurance vie…
reserve-balance = Solde
reserve-as-of = Au
reserve-as-of-hint = 2026-10-01 ; aujourd’hui si vide
reserve-floor = Jamais sous
reserve-delay = Jours pour arriver
reserve-delay-help = Le temps que met l’argent demandé pour arriver sur votre compte : 0 pour un Livret A, une dizaine de jours pour une assurance vie. Les renflouements sont demandés d’autant à l’avance.
reserve-delay-short = { $days ->
    [one] un jour
   *[other] { $days } jours
}
reserve-at-once = tout de suite
bank-imported = { $read ->
    [one] Un mouvement lu
   *[other] { $read } mouvements lus
}, { $new ->
    [0] aucun nouveau.
    [one] un nouveau.
   *[other] { $new } nouveaux.
}
bank-unreadable = { $file } ne se lit pas comme un export bancaire (OFX, CAMT.053 ou CSV).
money-changed = { $label } a pris { $actual } le { $date }, là où { $expected } était attendu.
bank-week-held = Cette semaine : { $list }. Le compte les tient.
bank-week-short = Cette semaine : { $list }. Le compte risque de ne pas tous les tenir.
bank-attention = { $count ->
    [one] Une chose d’argent à regarder.
   *[other] Quelques choses d’argent à regarder.
}
letters = Courrier papier
letters-no-ocr = Des scans attendent dans le dossier du courrier, non lus : aucun programme ici ne les lit. Pour les lire : { $hint }
letters-unread = Son texte n’a pas pu être lu ({ $problem }) ; son scan est là.
letters-scan = Voir le scan
letters-task = Une tâche pour cette date
letters-task-made = Sa tâche
letters-event = Dans l’agenda
letters-no-project = Aucun projet
letters-done = Fini, classé
letter-amount = Demande { $amount }.
letter-deadline = Au plus tard le { $date } (« { $why } »).
letter-appointment = Un rendez-vous : { $when }.
letter-registered = Envoyée en recommandé avec accusé de réception.
letter-dated = Datée du { $date }.
letter-reference = Votre numéro : { $reference }.
letter-unknown-sender = Une lettre
letter-scan = Le scan
letter-event = Rendez-vous : { $sender }
letter-kind-formal-notice = une mise en demeure
letter-kind-decision = une décision
letter-kind-tax-notice = un avis d’impôt
letter-kind-reminder = une relance
letter-kind-bill = une facture
letter-kind-appointment = un rendez-vous
letter-kind-acknowledgment = un accusé de réception
letter-kind-attestation = une attestation
letter-kind-contract = un contrat
letter-kind-other = une lettre
letter-task-formal-notice = Répondre à la mise en demeure de { $sender } { $amount }
letter-task-decision = Décision de { $sender } : la contester ou non
letter-task-tax-notice = Avis d’impôt : payer ou vérifier { $amount }
letter-task-reminder = { $sender } : payer la relance { $amount }
letter-task-bill = Payer { $sender } { $amount }
letter-task-appointment = Préparer le rendez-vous avec { $sender }
letter-task-acknowledgment = { $sender } : vérifier ce qu’ils ont reçu
letter-task-attestation = Classer l’attestation de { $sender }
letter-task-contract = Lire le contrat de { $sender }
letter-task-other = Répondre à { $sender }
set-letters-group = Courrier papier
set-letters-inbox = Où arrivent les scans
set-letters-inbox-help = Un dossier où vous, un scanner ou une personne qui vous aide déposez le courrier en PDF ou en photos (l’appli de scan d’un téléphone qui s’y synchronise convient aussi). Chaque scan est lu par Sioul sur un ordinateur (Tesseract, Poppler ; un téléphone ne les lit pas) et attend la fenêtre du Porche comme une carte : qui, quoi, combien, pour quand.
password-show = Afficher le mot de passe
password-hide = Masquer le mot de passe
set-passwords-shown = Afficher les mots de passe pendant la saisie
set-passwords-shown-help = Chaque champ de mot de passe, de phrase de passe ou de clé montre ce que vous tapez dès le départ, sur cet appareil ; l’œil au bout de chaque champ l’affiche ou le masque à tout moment.
bitwarden-factor-7 = Clé de sécurité (YubiKey, FIDO2)
bitwarden-code-7 = Deuxième étape : votre clé de sécurité.
bitwarden-key-use = Utiliser la clé de sécurité
bitwarden-key-failed = La clé de sécurité n’a pas servi : { $error }
bitwarden-code-sent = Le code a été envoyé à votre adresse électronique.
bitwarden-code-again = Renvoyer le code
bitwarden-code-refused = Ce code n’a pas été accepté : réessayez, ou choisissez une autre étape.
bitwarden-no-key = Bitwarden ne propose aucune clé de sécurité pour votre compte. Une YubiKey apparaît ici quand Bitwarden la connaît comme clé de sécurité (FIDO2 WebAuthn, ouverte à tous les comptes), ou comme « YubiKey OTP » tant que le compte a Premium : sans lui, Bitwarden la cache à toutes les applications. Voyez la connexion en deux étapes dans les réglages de sécurité de votre compte, dans le coffre web de Bitwarden. Les codes gardés sur la YubiKey par Yubico Authenticator vont dans « Application d’authentification ».
bitwarden-passkey = Ouvrir avec ma clé de sécurité
bitwarden-passkey-help = Votre clé et son code PIN, sans le mot de passe principal, quand Bitwarden connaît la clé comme clé d’accès utilisée pour le chiffrement.
bitwarden-password-or = Ou votre mot de passe principal :
bitwarden-passkey-no-secret = Votre clé a répondu, mais sans le secret qui ouvre le coffre (WebAuthn PRF) : utilisez votre mot de passe principal.
bitwarden-passkey-no-vault = Bitwarden vous connecte avec cette clé d’accès mais n’ouvre pas le coffre avec elle. Dans le coffre web de Bitwarden, Paramètres ▸ Sécurité ▸ Mot de passe principal, sous « Se connecter avec une clé d’accès », choisissez « Configurer le chiffrement » à côté d’elle. D’ici là, utilisez votre mot de passe principal.
bitwarden-passkey-unknown = Bitwarden ne connaît pas cette clé comme clé d’accès d’un compte. Ajoutez-la dans le coffre web de Bitwarden, Paramètres ▸ Sécurité ▸ Mot de passe principal, sous « Se connecter avec une clé d’accès » ; ou utilisez votre mot de passe principal.
bitwarden-passkey-other = Cette clé d’accès est celle d’un autre compte Bitwarden que celui réglé dans les réglages de cette page (⚙) : le coffre n’a pas été ouvert.
bitwarden-passkey-mismatch = Le secret de votre clé n’a pas ouvert le coffre : utilisez votre mot de passe principal.
bitwarden-key-waiting = Touchez votre clé de sécurité quand elle clignote ; branchez-la d’abord si elle ne l’est pas.
bitwarden-key-waiting-passkey = Votre clé est demandée : branchez-la si elle ne l’est pas, tapez son code PIN quand Sioul le demande, puis touchez-la quand elle clignote.
bitwarden-key-stop = Arrêter
bitwarden-key-not-allowed = La clé n’a pas servi à temps, ou la demande a été annulée.
bitwarden-choose = Choisir un identifiant…
bitwarden-choose-title = Quel identifiant ?
bitwarden-choose-site = Pour { $site } :
bitwarden-choose-none = Bitwarden ne garde aucun identifiant pour { $site } : cherchez parmi tous.
bitwarden-choose-search = Chercher dans tous vos identifiants
bitwarden-choose-found = Dans votre coffre :
bitwarden-choose-nothing = Rien trouvé.
bitwarden-choose-other-site = pour { $site }
bitwarden-choose-fill = Remplir
task-office-open = Ouvert du
task-office-to = au
task-office-and = et
task-office-usual = Les heures habituelles des bureaux, tant que celles-ci ne sont pas changées.
task-field-area = Pour
task-area-tags = Selon ses étiquettes
health-missed = Pendant que Sioul était fermé
health-missed-body = Avez-vous pris { $doses } ?
health-missed-open = Répondre
health-missed-question = Prévus pendant que Sioul était fermé, et notés nulle part où Sioul peut voir : les avez-vous pris ?
health-not-taken = Pas pris
health-taken-when = Pris…
dose-taken-title = Pris en retard
dose-taken-due = { $name }, prévu à { $due }.
dose-taken-when = Pris à
dose-next-after = La prise suivante vient { $hours } heures après l’heure où vous avez pris celle-ci : les heures entre deux prises sont gardées.
dose-time-wrong = Une heure, comme 09:30.
dose-doubt = Sioul ne peut pas savoir si elle a été prise : { $why }. Vérifiez avant de la prendre.
dose-doubt-now = Des prises notées sur vos autres appareils peuvent manquer ici : { $why }.
dose-doubt-record = le registre des prises de cet appareil n’a pas pu être lu { $when }
dose-doubt-never = { $name } n’a jamais dit jusqu’où va son registre (un Sioul plus ancien ?)
share-other-device = un autre de vos appareils
dose-doubt-closed = { $name } s’est fermé { $when }, et ses nouvelles peuvent tarder à arriver
dose-doubt-open = { $name } a été entendu pour la dernière fois { $when }
dose-doubt-broken = une partie de ce qu’a écrit { $name } n’a pas pu être lue
dose-check-title = À vérifier d’abord : { $dose }
dose-alarm-fallback = Une prise est prévue : ouvrez Sioul pour vérifier.
dose-alarm-late = Pris en retard : touchez pour dire à quelle heure.
dose-notifications-off = Android n’affiche pas les notifications de Sioul, ou son canal « Doses » est coupé : les rappels de prises ne peuvent pas s’afficher sur ce téléphone.
dose-alarms-inexact = Android ne laisse pas Sioul poser d’alarmes exactes : les rappels peuvent arriver en retard, jusqu’à une heure. Paramètres ▸ Applications ▸ Sioul ▸ Alarmes et rappels.
dose-record-broken = Le registre des prises ne peut pas être écrit. Sioul a mis l’ancien de côté et le reconstruit depuis vos autres appareils ; notez ailleurs ce que vous prenez en attendant.
need-meal-0 = Petit déjeuner
need-meal-1 = Déjeuner
need-meal-2 = Dîner
need-meal-n = Repas { $n }
need-nap = Sieste
need-sleep = Se préparer à dormir
need-heads-up = Pas de nouvelle grosse tâche
need-at = { $name } à { $time }
need-later = Plus tard
need-open = Options…
need-later-n = { $minutes } min plus tard
need-move-to = Déplacer à
need-move = Déplacer
need-unskip = Finalement aujourd’hui
stopped-note = Où j’en suis…
new-stopped = Où j’en suis…
need-gap = Plus de quatre heures entre { $from } et { $to }.
needs-title = Repas, repos et sommeil
needs-help = Des heures gardées libres : aucune tâche n’y est prévue. Un avis vient avant, sur le travail, puis un à l’heure ; rien d’autre n’est dit, et rien n’est noté.
needs-meals = Repas
needs-naps = Siestes
needs-sleep = La nuit
needs-add-meal = Ajouter un repas ou un en-cas
needs-add-nap = Ajouter une sieste
needs-at = À
needs-eat = Minutes pour manger
needs-prep = Minutes pour le préparer
needs-nap-minutes = Minutes
needs-after = Minutes pour revenir
needs-bed = Coucher
needs-wake = Lever
needs-wind-down = Minutes pour se préparer
needs-notices = Avis
needs-not-today = Pas aujourd’hui
needs-heads-up = Avis avant, en minutes
needs-later-by = « Plus tard » décale de, en minutes
needs-remove = Retirer
needs-name-hint = Nom, comme vous voulez
# The Health page's day and week (HealthPage.qml): each day's own meals, naps and nights.
need-meal-detail = le repas à { $at }
need-nap-detail = puis { $minutes } min pour revenir
need-night-detail = se préparer à dormir, coucher à { $bed }
need-changed = changé pour ce jour
need-added = ce jour seulement
need-pushed = déplacé après un événement
need-quiet = pas d’avis ce jour-là ; toujours gardé libre
need-off = retiré de ce jour
need-added-meal = Repas
need-added-nap = Repos
need-change-times = Changer ses heures…
need-not-that-day = Pas ce jour-là
need-unskip-day = Finalement ce jour-là
need-as-usual = Comme d’habitude
need-remove-today = Retirer d’aujourd’hui
need-remove-day = Retirer de ce jour-là
need-put-back = Remettre
need-add = Ajouter un repas ou un repos…
need-add-title = Un repas ou un repos, { $day }
need-kind-meal = Un repas
need-kind-nap = Un repos
need-form-from = De
need-form-to = À
need-form-help = Pour ce jour seulement : les heures habituelles restent telles quelles, dans les réglages de cette page.
need-menu = Plus pour { $name }
need-past = Ce jour est passé : il reste tel qu’il était.
need-times-wrong = Deux heures, comme 12:30, la fin après le début.
need-too-short = Quelques minutes au moins.
need-other-day = Cela le ferait passer à un autre jour.
health-day-empty = Les repas, le repos et le sommeil se règlent dans les réglages de cette page (⚙).
health-day-nothing = Rien de prévu ce jour-là.
health-errand-day = { $what } (dans vos tâches à partir de ce jour)
health-alone = Ces prises ne sont connues que de cet appareil. Si Sioul tourne aussi sur un autre, partagez entre eux (Paramètres ▸ Votre dossier et le partage) : une prise notée sur l’un compte alors sur tous, et seul celui où vous êtes vous la rappelle.
health-unchecked = Vos autres appareils ont été entendus pour la dernière fois à { $time } : une prise notée là-bas n’apparaît peut-être pas encore ici.
health-unchecked-yet = Vos autres appareils n’ont pas encore été entendus : une prise notée là-bas n’apparaît peut-être pas encore ici.
health-reminded-there = Les rappels viennent sur { $computer }, l’appareil où vous êtes.
invoice-unchecked = Votre dossier de partage ne peut pas être écrit pour l’instant : vos autres appareils ne sauraient rien de cette facture. Réessayez quand il sera joignable.
invoice-elsewhere = Les factures sont numérotées sur { $computer } : un appareil à la fois, pour qu’un numéro ne soit jamais donné deux fois.
invoice-take = Faire les factures sur cet appareil
invoice-settling = Dans une minute et demie : vos autres appareils apprennent d’abord que les factures se font ici.
invoice-others-silent = Un autre appareil n’a pas été entendu depuis quelques minutes : sa synchronisation est peut-être en retard. Les factures attendent qu’il le soit, pour qu’aucun numéro ne soit donné deux fois.
invoice-here = Les factures se font désormais sur cet appareil.
site-kind-video = Appels vidéo
site-area = Pour
area-admin = Vos démarches
area-leisure = Loisirs
area-mixed = Travail et démarches
site-move-up = Monter
site-move-down = Descendre
site-link = Lier à…
site-sort-category = Groupés par type
site-sort-own = Dans votre ordre
site-presets-find = Trouver un site : une banque, un organisme, une discussion…
site-presets-everyone = Pour tous : discussions, appels, rencontres
site-presets-whole-country = Tout le pays
site-presets-all = Tous les types
site-presets-kept = déjà là
site-presets-calls = appels
site-presets-none = Aucun ici : ajoutez-le à la main ci-dessous.
site-by-hand = Ou à la main :
site-later = Autres heures : { $count }
link-kind-site = Sites
mode-admin = Temps des démarches jusqu’à { $until } : organismes, factures, courriers.
mode-leisure = Temps libre jusqu’à { $until } : ce qui vous plaît.
set-windows-admin = Heures pour vos démarches
set-windows-admin-help = Vos démarches viennent alors : organismes, factures, courriers, courses de santé. Une fois ces heures réglées, les démarches ne viennent plus pendant les heures de travail, sauf les appels à un bureau, qui gardent les heures de bureau.
set-reminders-gather = Notifications des sites regroupées
set-reminders-gather-help = Ce que vos sites notifient attend, puis vient en une seule notification aux heures ci-dessous, pour les sites de ces heures-là. Un site en temps réel, et un appel, viennent tout de suite.
set-reminders-gathered = Regroupées à
set-reminders-gathered-help = Des heures de la journée, comme 09:00 : trois par jour ont le plus aidé dans un essai en conditions réelles.
sites-gathered = Vos sites ont des nouvelles
sites-gathered-open = Ouvrir le Porche
site-microphone = Micro, pour les appels
site-camera = Caméra, pour les appels
site-screen = Partage de l’écran, pour les appels
site-devices = Périphériques d’appel
site-devices-help = La caméra, le micro et le haut-parleur des appels dans chaque site, pris dans la liste de votre système. Un site qui vous laisse choisir dans ses propres réglages garde son choix.
site-device-camera = Caméra
site-device-microphone = Micro
site-device-speaker = Haut-parleur
site-device-system = Celui du système
site-permission-off-microphone = { $site } a demandé le micro : il est coupé pour ce site (⋮ ▸ Micro).
site-permission-off-camera = { $site } a demandé la caméra : elle est coupée pour ce site (⋮ ▸ Caméra).
site-permission-off-screen = { $site } a demandé à partager l’écran : c’est coupé pour ce site (⋮ ▸ Partage de l’écran).
site-kind-social = Réseaux sociaux
site-filter = Quels sites
site-field-categories = Catégories
site-categories-help = Banque, Santé…, séparées par des virgules
folder-new-button = Nouveau dossier
folder-new-title = Nouveau dossier dans { $account }
folder-new-make = Le créer
collection-here = sur cet appareil seulement
health-move = Un moment pour bouger
health-move-body = Levez-vous, étirez-vous, regardez au loin une minute.
site-presets = Sites courants
site-presets-pin-all = Tous ({ $count })
site-presets-by-region = Par région
site-pinned = Sites épinglés : { $count }
site-filter-clear = Montrer tous les sites
site-filter-any-area = Pour tout
site-filter-any-kind = Tous les types
site-filter-any-category = Toutes les catégories
site-filter-for = À quoi il sert
site-filter-type = Type
site-filter-category = Catégorie

## Le réveil au lever, sonné par un téléphone (crates/sioul-app/src/wake.rs) ; Java dit les cinq premiers, donnés à l’avance.
wake-channel = Réveil
wake-ringing = Réveil
wake-stop = Arrêter
wake-later = { $minutes } min plus tard
wake-again = Sonne de nouveau à { $time }
wake-title = Réveil au lever
wake-help = Sonne sur votre téléphone (Sioul pour Android) à la fin de la nuit, les matins cochés ; une nuit changée sur la page sonne à son propre lever. Le mode Ne pas déranger d’Android laisse passer les alarmes.
wake-next = Prochain réveil : { $when }.
wake-exact-off = Android ne laisse pas Sioul sonner à l’heure : le réveil ne peut pas sonner tant que « Alarmes et rappels » n’est pas autorisé.
wake-allow-exact = Autoriser les alarmes…
wake-screen-off = Android peut ne pas allumer l’écran quand il sonne ; il sonne quand même.
wake-allow-screen = Autoriser le plein écran…
wake-notifications-off = Les notifications de Sioul sont coupées dans Android : le réveil en a besoin pour montrer Arrêter.
wake-allow-notifications = Les activer…
wake-at = réveil à { $time }
wake-none = sans réveil
wake-skip = Pas de réveil à { $time }
wake-unskip = Réveil à { $time } finalement
wake-try = Essayer le réveil
wake-try-rings = Sonne dans une dizaine de secondes, comme un vrai réveil ; rien d’autre ne change.
wake-try-screen-off = Android ne laisse pas Sioul montrer le réveil par-dessus l’écran de verrouillage : autorisez le plein écran pour l’essayer.
wake-try-phone = Le réveil ne sonne que sur un téléphone.
wake-try-failed = Android n’a pas répondu : rien n’a été réglé. Réessayez dans un instant.

## Combien de temps une réserve tient à ce rythme, par son nombre de mois (0 : déjà à son plancher).
fig-pace-months = { $months ->
    [0] déjà à son plancher
    [one] environ un mois
   *[other] environ { $months } mois
}
reserve-lasts-months = { $months ->
    [0] À ce rythme, elle est déjà à son plancher.
    [one] À ce rythme, elle tient environ un mois.
   *[other] À ce rythme, elle tient environ { $months } mois.
}
attachment-program = { $name } est un programme, un script ou un installeur : Sioul n’en lance pas depuis un courriel, même vérifié. Enregistrez-le si vous lui faites confiance.
budget-origin-mail = D’un courriel : « { $subject } » de { $sender }, ajouté par Sioul le { $date }.
budget-origin-hand = Ajouté à la main dans Sioul le { $date }.
note-outside-notes = { $path } n’est pas dans le dossier de vos notes.
setting-unknown-key = Sioul n’a pas de réglage nommé « { $key } ».
reminders-entry-name = Rappels de Sioul
reminders-entry-comment = Rappels avant les dates, fenêtre de Sioul fermée
bank-rule-gone = Cette règle n’existe plus.
budget-line-gone = Cette ligne des budgets n’existe plus.
mail-no-such-folder = { $account } n’a pas de dossier nommé « { $folder } » ; sioul mail folders { $account } les liste.
mail-not-in-accounts = { $file } n’est pas dans le courrier de l’un de vos comptes.
mail-move-needs-to = Quel dossier ? Nommez-le avec --to.
list-read-only-mark = (lecture seule)
import-bad-key = « { $key } » ne peut pas être une clé : les clés nomment des fichiers, donc ni « / », ni « \ », ni « : » dedans.
antivirus-hint-windows = Sécurité Windows ▸ Protection contre les virus et menaces
antivirus-hint-packages = ClamAV, depuis les paquets de votre système
ocr-hint-windows = Tesseract (github.com/UB-Mannheim/tesseract) et Poppler
ocr-hint-packages = Tesseract et Poppler, depuis les paquets de votre système
google-page-granted = Sioul a l’accès. Vous pouvez fermer cet onglet.
google-page-denied = Google n’a donné aucun accès : rien n’a changé. Vous pouvez fermer cet onglet.
google-page-foreign = Cette réponse n’était pas pour Sioul. Vous pouvez fermer cet onglet.
ui-all-files = Tous les fichiers
## Formulaires, notes et mémos : montants illisibles, boutons, liens, micro.
amount-unreadable = « { $text } » n’est pas un montant : écrivez-le en chiffres, comme 1200 ou -650,50.
note-make = Créer la note
note-folder-make = Créer le dossier
ui-rename = Renommer
audio-cannot-play = Ce son ne peut pas être lu ici ({ $why }).
note-memo-failed = Le mémo n’a pas pu être enregistré ({ $why }).
note-memo-no-microphone = Le système ferme le micro à Sioul : il s’ouvre dans les réglages de confidentialité du système.
note-link-not-found = « { $path } » n’est pas dans votre dossier de notes.
note-changed-elsewhere = Cette note a changé ailleurs pendant que vous écriviez : la vôtre est gardée à côté, sous « { $path } ».
note-link-kept = Les liens de ce type ne s’ouvrent pas depuis une note : { $url }
right-now-unverified = Son expéditeur n’est pas vérifié : ne vous en servez que si vous venez de le demander à ce site.
link-program = Un programme, un script ou un installeur : c’est son dossier qui s’ouvre, pour le lancer de là si vous lui faites confiance.
task-spent-time = { $minutes ->
        [one] { $minutes } minute jusqu’ici
       *[other] { $minutes } minutes jusqu’ici
    }
task-session-time = { $date } : { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }
hours-range-add = Une autre plage horaire ce jour-là
hours-range-remove = Retirer cette plage horaire
site-news-line = { $title } : { $text }

## Ce qui vient quand : cinq moments, quatre listes, ne pas déranger (docs/areas.md, docs/porch.md)

mode-meal = Repas jusqu’à { $until }.
mode-sleep = Sommeil : rien ne dérange jusqu’à { $until }.
mode-wind-down = Avant de dormir : rien ne dérange jusqu’à { $until }.
mode-nap = Sieste : rien ne dérange jusqu’à { $until }.
quiet-meal-title = Le repas
set-hours-leisure = Repas et sommeil, sur la page Santé
set-hours-leisure-help = Les loisirs, c’est tout le reste du temps : ni travail ni démarches. Les repas et le sommeil se règlent sur la page Santé : un repas de sa préparation à sa fin, la nuit du moment de se détendre au réveil, les siestes. Pendant le sommeil, rien ne dérange.
set-doses-in-sleep = Prises pendant le sommeil
set-doses-in-sleep-help = Pendant le sommeil (la nuit, du moment de se détendre au réveil ; une sieste), aucune notification ne vient. Le rappel d’une prise vient quand même : c’est vous qui avez réglé son heure. En silence, il vient au réveil.
set-doses-in-sleep-remind = Rappeler
set-doses-in-sleep-silent = Rester silencieux
set-reach = Quand vient le courrier de chaque liste
set-reach-help = Coché : leur courrier vient alors. Décoché : il attend, jamais perdu, un moment coché. Les codes et les liens que vous demandez aux sites, et ce que vous vous envoyez, arrivent toujours tout de suite ; les bloqués, jamais. Pendant le sommeil, rien ne notifie.
reach-work = Travail
reach-admin = Démarches
reach-leisure = Loisirs
reach-meals = Repas
reach-sleep = Sommeil
reach-word-work = travail
reach-word-admin = démarches
reach-word-leisure = loisirs
reach-word-meals = repas
reach-word-sleep = sommeil
reach-any = à tout moment
reach-never = jamais
sender-list-times = { $list } : { $times }
sender-list-safe = Sûrs
sender-list-neutral = Neutres
sender-list-restricted = Restreints
sender-list-blocked = Bloqués
sender-one-safe = Sûr
sender-one-neutral = Neutre
sender-one-restricted = Restreint
sender-one-blocked = Bloqué
sender-categories = Comme le disent ses catégories
sender-no-list = Aucune liste
sender-from-address = { $list } : son propre choix.
sender-from-category = { $list }, comme le dit la catégorie { $name }.
sender-from-domain = { $list }, comme le dit { $name }.
sender-from-default = { $list } : aucune liste ne le nomme.
sender-now-restricted = { $entry } est restreint : son courrier ne vient qu’aux moments cochés pour les restreints.
sender-now-categories = { $entry } : comme le disent ses catégories.
set-restricted = Restreints
set-restricted-help = Ceux dont vous préférez n’avoir des nouvelles qu’à des moments choisis : un client exigeant, quelqu’un dont le courrier pèse. Leur courrier attend les moments cochés plus haut. Une adresse, ou un motif avec *.
set-sender-categories-group = Les catégories de vos contacts
set-sender-categories-note = Le choix propre à une personne passe d’abord, puis les catégories de sa fiche, puis le domaine de son adresse ; toutes les autres sont neutres. Rien ne va sur une liste tout seul.
set-sender-category = Une catégorie de vos contacts
set-sender-category-help = Toute personne dont la fiche est dans cette catégorie, sauf si son propre choix dit autre chose.
porch-night-none = Votre nuit n’est pas réglée : rien n’éloigne les notifications pendant que vous dormez.
porch-night-why = Dites quand vous vous couchez et vous levez, sur la page Santé : du moment de se détendre au réveil, rien ne dérange, sauf les prises que vous demandez.
porch-night-set = Régler ma nuit

# Les deux rituels : la fin de la journée de travail, la fin de la journée (docs/reviews.md, DayReview.qml).
review-work-title = La fin de la journée de travail
review-night-title = La fin de la journée
review-felt-label = La journée a été :
review-felt-light = Légère
review-felt-usual = Habituelle
review-felt-heavy = Lourde
review-felt-gave-back = Ressourçante
review-mix-label = Le mélange :
review-mix-too-much = Trop plein
review-mix-about-right = Juste ce qu’il faut
review-mix-too-empty = Trop vide
review-memo-label = Une note, si vous voulez :
review-memo-hint = Ce que vous voulez garder de la journée
review-close-work = Clore la journée de travail
review-close-night = Clore la journée
review-not-now = Pas maintenant
review-back-title = Vos notes des derniers jours
# Comment la journée a commencé et s’est passée, en mots, seulement ce qui a été dit (reviews.rs, `morning_lines`).
review-morning = Ce matin : { $weather }.
review-morning-later = Ce matin : { $weather } ; plus tard, { $later }.
review-weather-clear = clair
review-weather-haze = brume
review-weather-fog = brouillard
review-plan-haze = Le plan a gardé une journée plus légère.
review-plan-fog = Le plan s’en est tenu aux petites étapes.
review-plan-lighter = Le plan a allégé le reste de la journée.
review-plan-roomier = Le plan a redonné de la place au reste de la journée.
# Une revue en mots (`said_words`) : « lourde ; le mélange : trop plein ».
review-felt-word-light = légère
review-felt-word-usual = habituelle
review-felt-word-heavy = lourde
review-felt-word-gave-back = ressourçante
review-mix-word-too-much = trop plein
review-mix-word-about-right = juste ce qu’il faut
review-mix-word-too-empty = trop vide
review-said-both = { $felt } ; le mélange : { $mix }
review-said-mix = le mélange : { $mix }
review-said-note = une note
review-work-said = À la fin du travail : { $words }.
review-first-step = { $day } commence par : { $step }
# Un jour passé en une ligne (`back_line`).
review-back-morning = Matin : { $weather }
review-back-work = Fin du travail : { $words }
review-back-night = La journée : { $words }
# Ce que la journée a coûté et apporté, en mots seulement, jamais un nombre (`capacity::day_balance`).
review-balance-title = Ce que la journée a demandé et donné :
review-scale = { $scale } : charge { $level } pour vous.
review-scale-cognitive = Réflexion
review-scale-emotional = Émotions
review-scale-anxiety = Anxiété
review-scale-body = Corps et sens
review-level-light = légère
review-level-usual = habituelle
review-level-heavy = lourde
review-gain-some = Du temps vous a ressourcé.
review-gain-little = Peu de temps vous a ressourcé.
# La ligne d’état, quand la journée de travail ou la journée peut être close ; rien pendant le sommeil.
review-offer-work = Les heures de travail sont finies pour aujourd’hui.
review-offer-night = Avant de dormir, la journée peut être close.
# Une notification discrète à la fin des dernières heures de travail ou de démarches du jour, une fois.
review-notice-title = Les heures de travail sont finies
review-notice-text = La journée de travail peut être close, avec un mot sur elle si vous voulez.
review-closed-night = La journée est close.
review-kept = Vos mots sont gardés.
# Sur la journée de la page Santé.
review-line-said = Comment s’est passée la journée :
