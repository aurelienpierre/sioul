---
description: Les paramètres de Sioul – langue et couleurs, vos heures, rappels et notifications, votre dossier et le partage, factures ; et les réglages propres à chaque page, là où ils s’appliquent.
---

# Les paramètres {#settings}

**Paramètres** est l’icône à curseurs en bas de la colonne de gauche. On y trouve ce qui concerne Sioul tout entier. Ce qui concerne une seule page est sur cette page, derrière son ⚙ (plus bas).

Chaque réglage dit en une phrase ce qu’il change, et s’enregistre tout de suite. Rien à confirmer, rien à appliquer.

La page Paramètres a sept onglets ; un téléphone en ajoute deux, **Appels** et **Autres applications**.

## Affichage {#display}

- **Langue** : celle du système, English ou Français. Toutes les phrases de Sioul la suivent.
- **Couleurs** : claires, sombres, ou celles du système. Les icônes suivent au prochain démarrage.
- **Afficher le nom des lieux à côté de leur icône** : les lieux, à gauche de la fenêtre, montrent leur nom à côté de leur icône, dans une colonne plus large, pour qui lit plus facilement des mots que des icônes. Sans ce réglage, leurs icônes seules ; le nom de chacun s’affiche quand le pointeur s’y arrête, quand le clavier l’atteint, ou par un appui long sur un écran tactile. Il suit vos réglages sur vos autres appareils, quand vous les [partagez](sharing.md). Le bouton à l’extrémité gauche de la barre de titre, au-dessus des lieux, ou ++f9++, le change aussi.
- **Afficher les mots de passe pendant la saisie** : chaque champ de mot de passe, de phrase de passe ou de clé montre ce que vous tapez dès le départ, sur cet appareil. Sans ce réglage, l’œil au bout de chaque champ affiche ou masque ce que vous avez tapé, à tout moment.
- **Détails sur l’écran d’accueil** (sur un téléphone) : la carte de Sioul sur l’écran d’accueil du téléphone nomme les premières lettres qui attendent sur le Porche, avec leur expéditeur et leur objet, un code que vous venez de demander à un site, une prise prévue, et le titre de la prochaine étape ([Premiers pas](first-steps.md#on-a-phone)). Sans ce réglage, elle ne dit que ce à quoi sert ce moment, combien de lettres sont arrivées, et qu’une prochaine étape attend : pour un téléphone dont d’autres voient l’écran d’accueil. Ce téléphone seulement.

## Heures {#hours}

<figure markdown="span">
  [![L’onglet Heures des Paramètres : « Heures de travail », chaque jour de la semaine coché ou non, avec ses plages horaires, d’un début à une fin, et une phrase sur ce qu’elles font ; puis « Heures pour vos démarches », réglées de la même façon.](../assets/screens/fr/settings-hours.png){ loading=lazy }](../assets/screens/fr/settings-hours.png "Ouvrir l’image en grand")
  <figcaption>Deux semaines : travail, vos démarches. Puis les congés.</figcaption>
</figure>

- **Heures de travail** : ces jours et à ces heures, le travail peut vous joindre. En dehors, il se repose.
- **Heures pour vos démarches** : vos démarches viennent alors (organismes, factures, courriers, courses de santé).
- **Repas et sommeil, sur la page Santé** : une phrase, et un bouton vers eux. Les loisirs, c’est tout le reste du temps, ni travail ni démarches : il n’y a rien à régler pour eux. Les repas et le sommeil se règlent sur la page [Santé](health.md#meals-rest-and-sleep), et pendant le sommeil rien ne dérange.
- **Congés** : vacances, arrêt maladie, calme du premier jour au dernier, comme un jour sans travail, avec un mot dessus.

Chaque jour de chaque semaine est actif ou non, avec une plage horaire ou plusieurs : **+** en ajoute une, **×** en retire une. Ce que chaque type d’heures apporte, et ce qui attend : [Les heures](hours.md).

## Rappels et notifications {#reminders-and-notifications}

Chaque rappel vient une fois, en notification discrète, sans son, jamais répété. Se souvenir « le 30 » de ce qui était prévu est ce qui échoue le plus souvent, avec l’autisme et avec le TDAH (Landsiedel, Williams & Abbot-Smith 2017 ; Altgassen, Kretschmer & Kliegel 2014), et les rappels aident là où c’est la mémoire qui fait défaut (Jamieson et al. 2014).

- **Avant un événement** : un rappel discret, tant de temps avant chaque événement : aucun, 5, 10, 15 (sauf si vous le changez) ou 30 minutes, 1 ou 2 heures. Il est compté avant le temps de se préparer et d’y aller : un événement à 14:00 avec 30 minutes pour y aller est rappelé à 13:15. Chaque événement peut dire le sien : **Rappel**, dans son formulaire et dans ses détails ([Agenda](agenda.md#reminders)). Pas pour les journées entières, ni pour les agendas que vous lisez seulement. Sur un téléphone, le réglage dit si Android laisse ces rappels venir à l’heure.
- **Événements, le jour travaillé d’avant** : une demi-heure avant la fin du travail, le jour travaillé qui précède un événement : quoi, quand, où. Les alarmes que porte un événement sont dites à leur heure aussi ; celle qui tombe à moins de cinq minutes du rappel de Sioul n’est dite qu’une fois.
- **Dates demandées : jours travaillés avant** : au début du travail, tant de jours travaillés avant la date demandée d’une tâche (2, sauf si vous changez ce nombre) ; 0 pour aucun.
- **Une attente finie** : quand une attente après une étape faite est finie (une réponse due), une fois, quand le travail est là.
- **Paiements prévus : jours travaillés avant** : au début du travail, tant de jours travaillés avant un paiement prévu (une facture, un impôt) ; 0 pour aucun. Le rappel dit si le compte le tiendra.
- **Fenêtre de Sioul fermée** : votre session lance un petit veilleur qui dit les rappels quand la fenêtre est fermée ; rien d’autre ne tourne, aucun courrier n’est relevé. Il faut pour cela la commande `sioul` installée à côté de Sioul ([Installer](install.md#into-your-application-menu)). Pas encore sous Windows.
- **Nouveau courrier : notifier aux heures où il peut venir** : une seule notification discrète pour chaque lot de courrier qui peut vous joindre maintenant, et une quand le courrier qui attendait peut venir : « Le Porche ouvre : trois lettres vous attendent. » Activé d’origine. Voir [Ce que Sioul vous dit](notifications.md#what-sioul-tells-you-and-when).
- **Avec les lettres d’information** : les lettres d’information et les listes de diffusion, rangées sur le Porche, sont dites aussi. Les expéditeurs automatiques (une facture de no-reply) le sont de toute façon. Désactivé d’origine.
- **Notifications des sites regroupées**, et **Regroupées à** : ce que vos sites notifient attend, puis vient en une seule notification à ces moments-là : 09:00, 13:00 et 18:00, sauf si vous en réglez d’autres. Sur un téléphone, les notifications des automates des autres applications reviennent alors. Voir [Les heures de regroupement](notifications.md#the-gathered-times).
- **Quand chacune vient** : une grille de ce que fait chaque sorte de notification à chaque moment, les sortes en lignes, les moments en colonnes (vos heures, le sommeil, les pauses, un moment de temps pour vous, « Ne pas déranger ») : tout de suite, plus tard, aux heures de regroupement, si son événement tombe dans ce moment, pas du tout ; pendant « Ne pas déranger », votre liste. Les prises pendant le sommeil et une pause sont de ses cases. Certaines cases sont fixes, pour votre sécurité. Voir [Quand chacune vient](notifications.md#when-each-comes).

Les prises de médicaments sont rappelées depuis la page [Santé](health.md), et les papiers à renouveler depuis [Papiers](papers.md). Sur un téléphone, les prises, les événements et le nouveau courrier sont dits ; les autres rappels viennent sur un ordinateur ([Sur un téléphone](first-steps.md#on-a-phone)). Ce qui vient quand, et ce qui attend : [Les notifications et « Ne pas déranger »](notifications.md).

## Pauses {#pauses}

Le temps libre et la pause, préparés un jour calme : qui vous joint en temps libre, jusqu’où la fin du travail peut se décaler, si le mouvement est proposé ; pour la pause, vos contacts favoris, ce qui vous aide, votre ligne, le guide de respiration, ce que tient le reste de la journée ensuite, de quel pays sont les numéros, et ce que peut le mode Ne pas déranger de cet appareil ([Ce que fait chaque système](notifications.md#what-each-system-does)) ; **Essayer l’écran de pause**. Voir [Les pauses](pauses.md).

## Ne pas déranger {#do-not-disturb}

Un seul « Ne pas déranger » pour tous vos appareils : **L’interrupteur dans la ligne d’état** ; ce qui l’active de soi-même, **Pendant que je me concentre sur une tâche**, **Pendant les pauses**, **Pendant mon sommeil** ; **Les personnes de ma liste passent**, et la liste, la même sur tous vos appareils ; ce que le système de cet appareil laisse faire à Sioul ; sur un téléphone, qui, sur votre liste, y est en favori, et **Garder ce téléphone à jour en arrière-plan**. Ce qui vient pendant ce temps, sorte par sorte : **Quand chacune vient**, dans Rappels et notifications. Voir [Ne pas déranger sur tous vos appareils](notifications.md#do-not-disturb-on-every-device).

## Appels {#calls}

Sur un téléphone : Sioul comme appli numéro de l’appelant et spam d’Android, pour qu’un appel de quelqu’un qui ne peut pas vous joindre maintenant aille sur votre messagerie ; ce que Sioul fait des appels, ce qu’il ne fait jamais, et ce qui sonne toujours ; la lecture des contacts ; où va un appel refusé ; les SMS. Voir [Les appels](calls.md).

## Autres applications {#other-apps}

Sur un téléphone, les notifications des autres applications retenues jusqu’à leur heure, comme votre courrier : les messages des personnes selon [qui peut vous joindre](accounts.md#senders), les notifications des automates aux heures de regroupement. L’accès (l’**Accès aux notifications** d’Android, en deux étapes pour Sioul installé depuis un fichier), **Retenir les notifications des autres applications jusqu’à leur heure**, les applications qui ont sonné avant que Sioul les retienne, avec **Rendre silencieux**, et un choix pour chaque application, conversation et site. Voir [Les notifications des autres applications, sur un téléphone](notifications.md#other-apps-on-a-phone).

## Votre dossier et le partage {#your-folder-and-sharing}

- **Le dossier des notes** : votre dossier de fichiers Markdown, lu comme un coffre : vos notes, et à côté vos projets, budgets, papiers et lettres. Sioul s’y lie ; il ne le possède jamais.
- **Entre vos appareils** : partager avec vos autres appareils ce que Sioul garde sur celui-ci, partie par partie, scellé par une phrase de passe ; vos notes et vos papiers aussi, si vous les activez. Voir [Partager entre vos appareils](sharing.md).

## Factures {#invoices}

Ce qui est imprimé sur les factures que vous faites depuis [Temps](time.md#invoices) et [Projets](projects.md) :

- **Votre nom ou raison sociale**, **Votre adresse** (sur plusieurs lignes, comme sur une enveloppe) ;
- **SIRET**, ou le numéro d’entreprise là où vous êtes ; vide tant que votre entreprise n’est pas immatriculée ;
- **Mention de TVA** : pour une micro-entreprise en France, « TVA non applicable, art. 293 B du CGI » ;
- **Les numéros de facture commencent par** : les numéros se suivent ensuite, 2026-001, 2026-002… ;
- **Devise** : un code de trois lettres, comme EUR, USD, CHF ;
- **Modalités de paiement** : imprimées en bas (coordonnées bancaires, délai, pénalités de retard) ;
- **Les factures vont dans** : le dossier de leurs PDF, `Documents/Factures` dans votre dossier personnel s’il est vide ;
- **Une heure coûte** : le tarif quand un projet ne fixe pas le sien.

## Les réglages propres à chaque page {#each-pages-own-settings}

Un réglage, un seul endroit. Ce qui appartient à une page est derrière le ⚙ au bout de la première rangée de cette page :

| Page | Derrière son ⚙ |
|---|---|
| [Porche](porch.md#the-porchs-settings) | les projets qui y sont montrés, où arrivent les scans, la lecture d’un message, comment le courrier est trié |
| [Courrier](mail.md#settings) | par conversation, à quel rythme les dossiers sont relevés, votre propre filtre à indésirables, la lecture d’un message, les listes quittées |
| [Tâches](tasks.md#the-tasks-settings) | heures de bureau, types, catégories, listes de tâches, où vont les nouvelles tâches, ce qui est du travail et ce qui est à vous, GitHub |
| [Agenda](agenda.md#the-agenda-settings) | agendas, l’heure à laquelle commence la journée |
| [Contacts](contacts.md#the-contacts-settings) | carnets d’adresses, la carte |
| [Notes](notes.md#new-notes) | où vont les nouvelles notes |
| [Sites](sites.md#logins-from-bitwarden) | votre compte Bitwarden |
| [Santé](health.md#your-watch) | le dossier de votre montre, les propositions douces |

Les réglages propres à chaque adresse sont sur sa fiche dans [Comptes](accounts.md#a-mail-address). La lecture du texte long (la police, sa taille et l’interligne) est dans le ⚙ des pages où se lisent les messages, Courrier et Porche, et derrière **Aa** dans les Notes : un seul réglage, montré là où il sert.

### Votre filtre à indésirables {#your-own-spam-filter}

Dans le ⚙ de la page Courrier, sous **Votre filtre à indésirables**. Ce qu’il fait sur le Porche : [Les indésirables, et votre propre filtre](porch.md#spam-and-your-own-filter).

<figure markdown="span">
  [![Les réglages de la page Courrier, « Votre filtre à indésirables » : Ce qu’il fait, « Montrer son avis seulement » ; Indésirable à partir de, un curseur à 95 % ; Peut-être indésirable à partir de, à 50 % ; Son apprentissage : « En service : appris par noa-bureau le mardi 6 octobre, sur 4 210 messages légitimes et 655 indésirables », ce qu’il a mesuré alors, Entraîner maintenant et ce qu’il fait, puis ce dont il apprend et la place qui reste sur le disque.](../assets/screens/fr/settings-spam.png){ loading=lazy }](../assets/screens/fr/settings-spam.png "Ouvrir l’image en grand")
  <figcaption>Ce qu’il fait, ses deux seuils, et sur un ordinateur son apprentissage.</figcaption>
</figure>

- **Ce qu’il fait** : **Éteint** ; **Montrer son avis seulement**, là où il commence : « peut-être indésirable » ou « probablement indésirable » à côté du message d’un inconnu, rien de déplacé ; **Mettre l’indésirable de côté** : ce dont il est sûr va dans Mis de côté, d’où **Pas indésirable** le fait revenir.
- **Indésirable à partir de** : à quel point il doit être sûr pour dire indésirable le message d’un inconnu, 95 % sauf si vous le changez. Plus haut : moins de vos messages pris pour indésirables, plus d’indésirables laissés passer.
- **Peut-être indésirable à partir de** : de là jusqu’à l’autre, un message garde sa file avec « peut-être indésirable » à côté ; 50 % sauf si vous le changez. Chaque curseur s’arrête avant l’autre : « peut-être » reste sous « indésirable ».
- **Son apprentissage**, sur un ordinateur. **Entraîner maintenant** télécharge ce dont l’apprentissage a besoin, de chaque dossier de chaque adresse (les en-têtes et le début de chaque texte, jamais les pièces jointes), puis apprend de votre courrier : vos dossiers des indésirables, et ce que vous avez marqué indésirable ou pas indésirable, lui apprennent le plus. La première fois est longue. Pendant ce temps, il dit où il en est (« Messages téléchargés : 1 200 sur environ 15 000 », puis chaque étape), et Sioul reste utilisable ; **Arrêter** garde ce qui est venu, et la fois suivante reprend de là. Sa nouvelle table ne remplace celle en service que si elle ne prend pas plus de vos messages pour indésirables. Dessous, le dernier apprentissage : quand, si sa table a remplacé celle en service et pourquoi, sur combien de messages, et sur votre courrier le plus récent, quelle part en a été prise pour indésirable et quelle part des indésirables a été repérée, chacune avec sa marge ; puis ce dont il apprend, et la place qui reste sur le disque.
- **Sa table**, sur un téléphone, qui ne l’entraîne jamais : l’ordinateur qui l’a entraîné, et quand, avec ce qu’il a mesuré alors. Elle arrive par votre dossier, scellée (la partie **Filtre à indésirables**, dans [Le partage](sharing.md#what-travels-from-this-device)).

Le mode et les seuils voyagent vers vos autres appareils avec vos réglages. Ce que l’apprentissage lit, et ce qu’il garde : [Vie privée et sécurité](privacy-security.md#your-own-spam-filter).

## Où les réglages sont gardés {#where-settings-are-kept}

Dans un seul fichier de texte brut, `config.toml`, dans votre dossier de configuration (`~/.config/sioul/` sous Linux). Vous pouvez le lire et le modifier à la main : quand Sioul l’écrit, vos commentaires restent.
