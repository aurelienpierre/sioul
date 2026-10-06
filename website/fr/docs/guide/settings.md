---
description: Les paramètres de Sioul – langue et couleurs, vos heures, rappels et notifications, votre dossier et le partage, factures ; et les réglages propres à chaque page, là où ils s’appliquent.
---

# Les paramètres {#settings}

**Paramètres** est l’icône à curseurs en bas de la colonne de gauche. On y trouve ce qui concerne Sioul tout entier. Ce qui concerne une seule page est sur cette page, derrière son ⚙ (plus bas).

Chaque réglage dit en une phrase ce qu’il change, et s’enregistre tout de suite. Rien à confirmer, rien à appliquer.

La page Paramètres a cinq onglets.

## Affichage {#display}

- **Langue** : celle du système, English ou Français. Toutes les phrases de Sioul la suivent.
- **Couleurs** : claires, sombres, ou celles du système. Les icônes suivent au prochain démarrage.
- **Afficher les mots de passe pendant la saisie** : chaque champ de mot de passe, de phrase de passe ou de clé montre ce que vous tapez dès le départ, sur cet appareil. Sans ce réglage, l’œil au bout de chaque champ affiche ou masque ce que vous avez tapé, à tout moment.

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

- **Évènements, le jour travaillé d’avant** : une demi-heure avant la fin du travail, le jour travaillé qui précède un événement : quoi, quand, où. Les alarmes que porte un événement sont dites à leur heure aussi.
- **Dates demandées : jours travaillés avant** : au début du travail, tant de jours travaillés avant la date demandée d’une tâche (2, sauf si vous changez ce nombre) ; 0 pour aucun.
- **Une attente finie** : quand une attente après une étape faite est finie (une réponse due), une fois, quand le travail est là.
- **Paiements prévus : jours travaillés avant** : au début du travail, tant de jours travaillés avant un paiement prévu (une facture, un impôt) ; 0 pour aucun. Le rappel dit si le compte le tiendra.
- **Fenêtre de Sioul fermée** : votre session lance un petit veilleur qui dit les rappels quand la fenêtre est fermée ; rien d’autre ne tourne, aucun courrier n’est relevé. Il faut pour cela la commande `sioul` installée à côté de Sioul ([Installer](install.md#into-your-application-menu)). Pas encore sous Windows.
- **Notifications des sites regroupées**, et **Regroupées à** : ce que vos sites notifient attend, puis vient en une seule notification à ces moments-là (09:00, 13:00 et 18:00, sauf si vous en réglez d’autres), pour les sites dont les heures sont en cours. Un site en temps réel, et un appel, viennent tout de suite.
- **Prises pendant le sommeil** : **Rappeler** ou **Rester silencieux**. Pendant le sommeil (la nuit, du moment de se détendre au réveil ; une sieste), aucune notification ne vient ; le rappel d’une prise vient quand même, puisque c’est vous qui avez réglé son heure, sauf si vous choisissez **Rester silencieux** : il vient alors au réveil. Voir [Les heures](hours.md#sleep).

Les prises de médicaments sont rappelées depuis la page [Santé](health.md), et les papiers à renouveler depuis [Papiers](papers.md). Sur un téléphone, seules les prises sont rappelées pour l’instant ([Sur un téléphone](first-steps.md#on-a-phone)).

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
| [Porche](porch.md#the-porchs-settings) | les projets qui y sont montrés, où arrivent les scans, comment le courrier est trié |
| [Courrier](mail.md#settings) | par conversation, à quel rythme les dossiers sont relevés |
| [Tâches](tasks.md#the-tasks-settings) | heures de bureau, types, catégories, listes de tâches, où vont les nouvelles tâches, ce qui est du travail et ce qui est à vous, GitHub |
| [Agenda](agenda.md#the-agenda-settings) | agendas, l’heure à laquelle commence la journée |
| [Contacts](contacts.md#the-contacts-settings) | carnets d’adresses, la carte |
| [Notes](notes.md#new-notes) | où vont les nouvelles notes |
| [Sites](sites.md#logins-from-bitwarden) | votre compte Bitwarden |
| [Santé](health.md#your-watch) | le dossier de votre montre, les propositions douces |

Les réglages propres à chaque adresse sont sur sa fiche dans [Comptes](accounts.md#a-mail-address). Partout où se lit un long texte (un message, une note), **Aa** règle la police, sa taille et l’interligne.

## Où les réglages sont gardés {#where-settings-are-kept}

Dans un seul fichier de texte brut, `config.toml`, dans votre dossier de configuration (`~/.config/sioul/` sous Linux). Vous pouvez le lire et le modifier à la main : quand Sioul l’écrit, vos commentaires restent.
