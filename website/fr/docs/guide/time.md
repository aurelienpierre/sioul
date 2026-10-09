---
description: Le temps passé et les factures dans Sioul – le temps du minuteur de concentration et celui noté à la main, gardé pour facturer les clients et pour apprendre combien de temps les choses prennent vraiment ; ce qui reste à facturer, un export en tableur, et des factures numérotées sans trou, le tout dans des fichiers sur vos appareils.
---

# Le temps et les factures {#time-and-invoices}

## En bref {#in-short}

Sioul note le temps que vous passez, pendant que vous travaillez ou après coup, à la main, et le garde pour deux usages. Pour les clients, il devient des factures : ce qui reste à facturer reste en vue, chaque heure n’est facturée qu’une fois, et les numéros de facture se suivent sans trou. Pour vous, mis à côté de vos estimations, il montre combien de temps les choses prennent vraiment, et le plan de vos journées en tient compte ([Combien de temps prennent les choses](#how-long-things-take)). Votre temps et vos factures restent dans des fichiers lisibles, sur vos propres appareils.

<figure markdown="span">
  [![La page Temps sur « Semaine » : une barre par jour, empilée par projet en quelques couleurs calmes ; dessous, les heures de chaque projet et ce qui reste à facturer, puis chaque plage de temps, de la plus récente à la plus ancienne.](../assets/screens/fr/time.png){ loading=lazy }](../assets/screens/fr/time.png "Ouvrir l’image en grand")
  <figcaption>Une semaine de temps, par projet, et ce qui reste à facturer.</figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

- Votre temps, vos clients et vos factures restent dans des fichiers lisibles sur vos appareils. Pas de compte à ouvrir, pas d’abonnement, pas de serveur de Sioul.
- Un numéro de facture n’est jamais donné deux fois, même avec plusieurs appareils : un seul appareil numérote les factures, et Sioul attend plutôt que de risquer un numéro en double.
- Le temps déjà facturé ne peut plus être changé, ni facturé de nouveau.
- Le tableur pour votre comptable ne peut pas porter de formule cachée : un titre de tâche venu du courrier de quelqu’un est écrit comme du simple texte.
- Quand vos appareils partagent, votre temps et vos factures voyagent scellés, par un dossier que votre propre synchronisation transporte : le dossier et son serveur voient que quelque chose a changé, jamais quoi ([Le partage](sharing.md)).

## La page Temps {#the-time-page}

**Semaine**, **Mois** ou **Année**, pour **Tous les projets** ou un seul ; **Aujourd’hui** et les flèches ◂ ▸ font avancer ou reculer dans le temps :

- une barre par jour (par mois, sur une année), empilée par projet ;
- les heures de chaque projet, et ce qui reste à facturer, en heures et en argent à son taux ;
- puis chaque plage de temps, de la plus récente à la plus ancienne.

## D’où vient le temps {#where-time-comes-from}

- **Le minuteur de concentration** : les minutes d’une [séance de concentration](tasks.md#starting-and-stopping) comptent pour sa tâche, et pour le projet de la tâche.
- **Noter du temps**, sur cette page, sur la page d’un projet, ou avec **Nouveau ▾ ▸ Du temps passé** : une réunion, un appel, un travail fait loin du minuteur. **Pour** (un projet, ou une tâche), **Combien de temps** (`1h30`, `45m`, `90`), **Quand**, **Ce que c’était**, et **Ne pas facturer** quand il ne doit pas l’être.

Pendant une séance de concentration, sous Linux et sur un téléphone, une notification la montre : sa tâche, depuis quand, et **Pause** (**Reprendre** une fois en pause) et **Arrêter**, qui font ce que font les boutons de la fenêtre de concentration. Sur un bureau Linux, elle reste parmi les notifications du bureau tant que Sioul est ouvert (dans KDE Plasma, sous la cloche une fois sa bulle partie), et s’en va quand Sioul se ferme. Sur un téléphone, un chronomètre y tourne, et ses boutons marchent avec Sioul en arrière-plan ou fermé. Mise en pause ou arrêtée ailleurs, dans la fenêtre de concentration ou sur un autre appareil, elle suit. Sous Windows et macOS, seule la fenêtre de concentration montre le temps qui court, pour l’instant.

Cliquez sur n’importe quel moment, chronométré ou noté à la main, pour le changer : sa tâche, son projet, son jour, de quand à quand, et ce que c’était ; le temps que le minuteur a gardé après une pause manquée se corrige ainsi. Un clic droit propose aussi **Enlever ce temps**. Le temps déjà sur une facture reste tel qu’il a été facturé.

Un minuteur oublié est coupé quand vous l’arrêtez : au-delà du double du temps choisi (ou de ce temps et une demi-heure de plus, si c’est plus long), seul le temps choisi compte ; une séance sans fin fixée compte trois heures au plus.

Chaque moment dit discrètement comment ses minutes ont été connues :

- *chronométré* ;
- *noté à la main* ;
- *chronométré, puis corrigé*, une fois que vous avez changé la durée d’un moment chronométré, ou que Sioul a coupé un minuteur resté lancé ;
- *on ne sait pas comment*, pour le temps noté avant que Sioul ne le garde.

### Facturé ou non {#billed-or-not}

Le travail pour un client se facture. Une tâche peut dire autre chose, sous **Facturé** dans son panneau : *Comme son projet*, *Son temps se facture*, ou *Non facturé*.

## Combien de temps prennent les choses {#how-long-things-take}

Le **Prend environ** d’une tâche est une estimation. Le temps noté pour elle est ce qu’elle a pris. Sioul met les deux côte à côte, et corrige vos estimations dans le plan seulement : chaque tâche prend sa durée corrigée dans le plan et dans la journée, tandis que **Prend environ** reste tel que vous l’avez écrit, et le minuteur part de lui. Seules les tâches finies lui apprennent, les récentes le plus ; les minutes notées à la main comptent moins que celles qui sont chronométrées, parce que les durées dont on se souvient sont moins sûres. Comment il apprend, en détail : [plus bas](#how-the-plan-learns-from-your-time).

Il ne vous note jamais, ne montre jamais rien en retard, ne remplit jamais **Prend environ** à votre place, et ne vous compare jamais à personne.

Chronométrer les loisirs, c’est à vous de voir. Les trous ne posent pas de problème : le temps non noté est laissé de côté, il ne compte contre rien.

## Un tableur du temps facturable {#a-spreadsheet-of-billable-time}

Le bouton d’export, en haut à droite de la page Temps : **Le temps facturable, en tableur (CSV)…**, pour un projet et une période (cette semaine, la semaine dernière, ce mois-ci, le mois dernier, depuis toujours, ou d’un jour à un autre). Une ligne par tâche, avec ses heures, le taux horaire et le montant, puis le total. En français, le fichier utilise `;` et la virgule décimale, comme les tableurs français les lisent. Un titre de tâche venu du courrier de quelqu’un est écrit comme du simple texte : un tableur ne l’exécute jamais comme une formule.

## Les factures {#invoices}

Sur la page d’un projet, ou à côté du projet sur la page Temps, **Faire la facture** quand du temps facturable attend.

- **Les numéros.** Avec le préfixe habituel, l’année, les numéros repartent de 001 chaque mois de janvier : 2026-001, 2026-002. Avec votre propre préfixe, ils continuent après le dernier, d’une année sur l’autre. Un numéro n’est jamais donné deux fois, et une facture ne peut pas être supprimée depuis Sioul.
- **Les lignes** : une par tâche (ou par temps noté sans tâche), avec ses heures au taux du projet.
- **Facturé une fois** : le temps facturé porte le numéro de sa facture ; la facture suivante le laisse de côté, et ce temps ne peut plus être changé.
- **Ce qu’elle imprime** : votre nom ou votre raison sociale, votre adresse et votre SIRET ; le nom et l’adresse postale du client ; son numéro ; le jour où elle est émise, la date ou la période de la prestation (le premier et le dernier jour des séances qu’elle facture, ou leur seul jour), et le jour où le paiement est dû ; le projet ; chaque ligne avec ses heures, son taux et son montant, puis le total ; votre mention de TVA (« TVA non applicable, art. 293 B du CGI » pour une micro-entreprise) ; vos modalités de paiement ; et une ligne sur le retard de paiement : des pénalités au taux de trois fois le taux d’intérêt légal, l’indemnité forfaitaire de 40 € pour frais de recouvrement, et pas d’escompte pour paiement anticipé. « EI » après votre nom n’est pas ajouté pour vous : voir [Pas encore là](#not-there-yet).
- **Le PDF** (A4) va dans `Documents/Factures` de votre dossier personnel, ou dans le dossier choisi. **Imprimer à nouveau** le réécrit.
- **Votre budget** : le total est attendu dans le budget du projet, payable à trente jours. **Payée** inscrit le paiement à la date du jour, et l’argent compte.

Votre nom, votre adresse, vos numéros, la devise, les modalités de paiement et le taux horaire se règlent une fois, dans [Paramètres ▸ Factures](settings.md#invoices).

### Sur plusieurs appareils {#on-several-computers}

Un numéro de facture ne doit jamais être donné deux fois. Quand vous [partagez entre vos appareils](sharing.md), un seul appareil numérote les factures : un autre dit « Les factures sont numérotées sur … », et **Faire les factures sur cet appareil** les reprend, après une minute et demie, une fois que vos autres appareils le savent. Si votre dossier de partage ne peut pas être écrit, ou si un autre appareil ne s’est pas manifesté depuis quelques minutes, Sioul attend plutôt que de risquer un numéro en double.

## Pendant le calme {#in-quiet-time}

En dehors des heures de travail, la page Temps attend derrière **Montrer quand même**. Voir [Les heures](hours.md).

## Pas encore là {#not-there-yet}

- **« EI » après votre nom.** Le nom d’un entrepreneur individuel doit être précédé ou suivi des mots « entrepreneur individuel » ou « EI » (service-public.gouv.fr, fiche F31808). Sioul ne l’ajoute pas de lui-même : écrivez-le juste après votre nom dans [Paramètres ▸ Factures](settings.md#invoices), comme le dit l’aide du champ (« Camille Exemple EI »).
- **La facture électronique.** À partir du 1er septembre 2027, une micro-entreprise française, en franchise de TVA aussi, doit émettre ses factures aux entreprises françaises sous forme électronique, par une plateforme agréée, en UBL, en CII ou dans un format mixte comme Factur-X, avec le SIREN du client. À partir de la même date, les données des ventes aux particuliers et à l’étranger doivent être transmises (e-reporting). Sioul ne fait que des factures en PDF, pour l’instant.
- Les devis, les lignes au forfait, les frais, les montants de TVA et les avoirs.
- Le temps qui court dans les notifications de Windows et de macOS : seule la fenêtre de concentration le montre là.

## Pour aller plus loin {#going-further}

### Comment le plan apprend de votre temps {#how-the-plan-learns-from-your-time}

- **Votre première estimation est gardée.** La première fois qu’une tâche reçoit une durée, Sioul la garde avec la tâche et ne la change jamais, même quand vous changez **Prend environ** ensuite. Une tâche qui avait une durée avant que cela existe n’en a pas de gardée ; sa durée actuelle sert à la place.
- **Seules les tâches finies lui apprennent**, chacune avec son temps passé face à sa première estimation, tous ses moments additionnés.
    - Les minutes chronométrées comptent pleinement, corrigées aussi.
    - Les minutes notées à la main comptent pour un quart, parce que les durées dont on se souvient sont moins sûres.
    - Les tâches abandonnées ne comptent jamais.
- **Les tâches récentes comptent le plus** : le poids d’une tâche diminue de moitié tous les quatre ou cinq jours.
- **Peu de tâches, peu de correction.** Tant qu’environ neuf tâches ne sont pas derrière elle, la correction penche vers « un peu plus long que prévu ».
- **Selon ce à quoi servent les tâches** : le travail, vos démarches et les loisirs ont chacun leur correction, rapprochée de la vôtre tant qu’ils n’ont pas assez de tâches à eux.
- **Le plan s’en sert ; vous voyez la vôtre.** Chaque tâche prend sa durée corrigée dans le plan et dans la journée, tandis que **Prend environ** reste tel que vous l’avez écrit, et le minuteur part de lui.
- **La marge est celle de la journée, pas de chaque tâche.** Chaque jour garde un peu de temps libre après sa dernière étape pour les étapes qui débordent : à peu près de quoi couvrir une journée qui va moins bien que d’habitude, jamais plus du tiers de la journée. Une fois les premières tâches d’aujourd’hui finies, une journée qui avance plus lentement en garde davantage. L’étape suivante garde toujours une part d’aujourd’hui.
- **Sur demande**, dans le panneau d’une tâche, une ligne comme « Les tâches de ce type prennent d’habitude environ 1,3× la première estimation ; le plan en tient déjà compte », seulement une fois que assez de tâches sont derrière elle.

Le détail, avec ce qui vient de la recherche et ce qui est une supposition : [les notes de conception (en anglais)](https://aurelienpierre.github.io/sioul/dev/capacity.html).

### En ligne de commande {#from-the-command-line}

`sioul focus start <task> [--minutes 25]`, `sioul focus status` et `sioul focus stop [--done] [--note "…"]` mènent le minuteur de concentration depuis un terminal ; ses minutes comptent comme celles de la fenêtre.

## Comparé à d’autres applications {#compared-with-other-apps}

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

**Le temps et les factures**

| | Sioul | Toggl Track | Clockify | Harvest | Kimai | Freebe |
|---|---|---|---|---|---|---|
| Un minuteur, et du temps noté à la main | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Un minuteur oublié est rattrapé | en partie¹ | ✓² | ✓² | ? | en partie³ | ? |
| Facturable ou non, avec des taux horaires | ✓ | ✓⁴ | ✓⁴ | ✓ | ✓ | ✓ |
| Ce qui reste à facturer, par projet | ✓ | ✗⁵ | en partie⁶ | ✓ | en partie⁷ | ? |
| Des factures faites du temps | ✓ | en partie⁸ | ✓⁹ | ✓ | ✓ | ✓ |
| Chaque heure facturée une fois, jamais deux | ✓ | ✗⁵ | ✓ | ✓ | ✓ | ? |
| Des numéros de facture qui se suivent d’eux-mêmes | ✓¹⁰ | ✗¹¹ | ✓¹² | ✓¹² | ✓ | ✓ |
| Devis, frais ou montants de TVA | ✗ | en partie¹³ | ✓ | ✓ | ✓ | ✓¹⁴ |
| Un tableur pour votre comptable | ✓ | ✓ | ✓ | ? | ✓ | ? |

1. Coupé quand vous l’arrêtez : au-delà du double du temps choisi (ou de ce temps et une demi-heure), le temps choisi compte ; sans fin fixée, trois heures au plus. Rien ne surveille si vous êtes devant votre ordinateur.
2. La détection d’inactivité : Toggl Track dans ses applications de bureau et son extension de navigateur ; Clockify sur Mac, Windows et dans Chrome.
3. Une entrée la plus longue, de 8 heures sauf si l’administrateur la change.
4. Les taux facturables à partir de l’offre Starter de Toggl Track et de l’offre Basic de Clockify.
5. Toggl Track ne marque pas le temps comme facturé ; son aide propose d’étiqueter à la main les entrées facturées.
6. Les entrées facturées portent une marque « Invoiced » dans le rapport détaillé ; un total de ce qui reste à facturer n’est pas décrit.
7. Les enregistrements pas encore exportés sont ce que prend la facture suivante ; un montant restant à facturer n’est pas décrit.
8. Un PDF fait d’un rapport, chaque champ modifiable ; la facture n’est pas gardée dans Toggl Track.
9. À partir de l’offre Standard.
10. Jamais donnés deux fois, aussi entre vos appareils : un seul appareil les numérote. Avec le préfixe habituel, la série repart chaque année.
11. Le numéro de facture se tape à la main.
12. Automatique par défaut ; il peut se changer.
13. Un champ de taxe, tapé sur la facture.
14. Devis, avoirs et factures d’acompte.

**La France, et où vivent vos données**

| | Sioul | Toggl Track | Clockify | Harvest | Kimai | Freebe |
|---|---|---|---|---|---|---|
| Les mentions légales françaises imprimées pour vous | en partie¹ | ? | ? | ? | ?² | ✓ |
| Des factures électroniques pour la France (Factur-X, UBL, CII) | ✗³ | ? | ? | ? | en partie⁴ | ✓ |
| Fonctionne sans connexion | ✓ | ✓⁵ | ✓⁵ | ? | ? | ? |
| Votre temps sur vos propres appareils, sans compte | ✓ | ✗ | ✗ | ✗ | en partie⁶ | ✗ |
| Logiciel libre | ✓ | ✗⁷ | ✗⁷ | ✗⁷ | ✓ | ✗⁷ |

1. Imprimés : le SIRET, la mention d’exonération de TVA, la date ou la période de la prestation, les pénalités de retard, l’indemnité de 40 €, pas d’escompte pour paiement anticipé. Pas imprimés pour vous : « EI » (tapez-le après votre nom), et le SIREN du client, que les micro-entreprises doivent donner à partir du 1er septembre 2027.
2. Ses modèles de facture sont à modifier vous-même ; aucun modèle français n’a été confirmé.
3. Sioul fait des factures en PDF. À partir du 1er septembre 2027, une micro-entreprise française doit émettre ses factures aux entreprises françaises sous forme électronique, par une plateforme agréée.
4. Avec une extension payante, 99 € par an : Factur-X, ZUGFeRD, XRechnung, UBL et CII.
5. Les applications de bureau de Toggl Track ; Clockify sur Mac, Windows, Linux, Android et iOS. Les deux envoient le temps à leurs serveurs une fois de nouveau en ligne.
6. Sur votre propre serveur, ou dans le nuage de Kimai.
7. Un service hébergé, selon ses propres conditions ; aucune licence libre n’est publiée pour lui.

D’autres en font plus pour la facturation elle-même : la détection d’inactivité (Toggl Track, Clockify) ; les devis, les frais, la TVA, les avoirs et les liens de paiement (Clockify, Harvest, Kimai, Freebe) ; les factures électroniques de la réforme française (Freebe, et Kimai avec une extension) ; les déclarations à l’URSSAF (Freebe) ; les équipes, les validations et les intégrations (les quatre services commerciaux). Sioul garde ce qui reste à facturer, ne facture chaque heure qu’une fois, numérote sans double même d’un appareil à l’autre, donne ce même temps au plan de vos journées, et garde le tout sur vos propres appareils.

??? info "Sources"
    Toutes lues le 8 octobre 2026.

    - **Toggl Track**, son aide sur les factures et sur l’application de bureau pour Windows, sa page des applications de bureau et ses prix : <https://support.toggl.com/creating-invoices-in-toggl-track>, <https://support.toggl.com/en-us/article/toggl-track-desktop-app-for-windows-5w1y5/>, <https://toggl.com/track/toggl-desktop/>, <https://toggl.com/track/pricing/>
    - **Clockify**, son aide sur les factures et sur la facturation du temps suivi, ses pages sur la facturation et les applications, et ses prix : <https://clockify.me/help/projects/invoicing>, <https://clockify.me/help/projects/invoicing-tracked-time-expenses>, <https://clockify.me/features/invoicing>, <https://clockify.me/apps>, <https://clockify.me/pricing>
    - **Harvest**, sa page sur la facturation, la documentation de son API (rapport du non-facturé, factures, entrées de temps), ses applications et ses prix : <https://www.getharvest.com/features/invoicing>, <https://help.getharvest.com/api-v2/reports-api/reports/uninvoiced-report/>, <https://help.getharvest.com/api-v2/invoices-api/invoices/invoices/>, <https://help.getharvest.com/api-v2/timesheets-api/timesheets/time-entries/>, <https://www.getharvest.com/apps>, <https://www.getharvest.com/pricing>
    - **Kimai**, sa documentation sur les factures, les exports et les feuilles de temps, son extension de facture électronique et son dépôt : <https://www.kimai.org/documentation/invoices.html>, <https://www.kimai.org/documentation/export.html>, <https://www.kimai.org/documentation/timesheet.html>, <https://www.kimai.org/store/invoice-bundle.html>, <https://github.com/kimai/kimai>
    - **Freebe**, sa page d’accueil, ses pages sur le temps et sur les factures, et ses prix : <https://www.freebe.me/>, <https://www.freebe.me/fonctionnalite/gestion-du-temps>, <https://www.freebe.me/fonctionnalite/facture-client>, <https://www.freebe.me/tarifs>
    - **Le droit français**, la fiche de service-public.gouv.fr sur les mentions obligatoires d’une facture (mise à jour le 11 août 2026), le BOFiP sur la numérotation des factures, la page d’impots.gouv.fr sur la facturation électronique (modifiée le 26 mai 2026), et l’actualité de service-public.gouv.fr du 2 septembre 2026 : <https://entreprendre.service-public.gouv.fr/vosdroits/F31808>, <https://bofip.impots.gouv.fr/bofip/140-PGP.html/identifiant=BOI-TVA-DECLA-30-20-20-10-20131018>, <https://www.impots.gouv.fr/professionnel/je-decouvre-la-facturation-electronique>, <https://entreprendre.service-public.gouv.fr/actualites/A18953>

## Côté technique {#for-technical-readers}

- **Le temps noté** : du TOML, un fichier par mois (`~/.local/share/sioul/time/2026-10.toml` sous Linux), la séance en cours dans `time/running.toml`, pour que fermer la fenêtre ne perde rien. Chaque enregistrement dit comment ses minutes ont été connues (`kind` : mesuré, tapé, corrigé).
- **Un minuteur oublié** : une séance au-delà de `max(2 × choisi, choisi + 30)` minutes compte le temps choisi ; une séance sans fin fixée compte 180 minutes au plus ; l’une ou l’autre est alors marquée corrigée.
- **Les factures** : un enregistrement chacune (`invoices/<numéro>.toml`), écrit en entier puis renommé à sa place, qui garde vos coordonnées telles qu’elles étaient ce jour-là ; un enregistrement qui ne se lit plus garde quand même son numéro. Le PDF est en A4, avec des marges de 18 mm, écrit par le `QPdfWriter` de Qt à partir d’un `QTextDocument`.
- **La numérotation** : le plus grand numéro du préfixe, plus un, sur trois chiffres. Le préfixe par défaut est l’année suivie d’un tiret : cette série repart donc chaque année ; un préfixe à vous continue simplement.
- **Facturé une fois** : chaque séance facturée porte le numéro de sa facture, et une nouvelle facture laisse de côté toute séance qui en a un.
- **Un seul appareil numérote** : un bail dans le dossier de partage (`leases/invoices/<ordinateur>.lease`, scellé comme les enregistrements), renouvelé chaque minute et valable cinq ; un autre appareil n’agit qu’après 90 secondes d’attente, et seulement si les autres appareils se sont manifestés récemment ([Le partage](sharing.md#some-things-one-device-at-a-time)).
- **Le tableur** : les guillemets de la RFC 4180 ; une cellule qui commence par `=`, `+`, `-`, `@`, une tabulation ou un retour chariot reçoit une apostrophe devant, pour qu’un tableur ne l’exécute jamais comme une formule (injection CSV) ; `;` et la virgule décimale en français.
- **Le partage** : la partie **Temps** porte le temps noté, la séance en cours et les revues du jour ; **Brouillons et factures** porte les enregistrements des factures. Chaque enregistrement est scellé avec XChaCha20-Poly1305 sous une clé tirée de votre phrase de passe par Argon2id (64 Mio, 3 passes) et gardée dans le trousseau de chaque appareil ([Le partage](sharing.md#what-it-protects-and-what-it-cannot-hide)).
- **Combien de temps prennent les choses, en chiffres** : y = ln(temps passé ÷ première estimation) sur les tâches finies ; les poids diminuent de moitié tous les 4,5 jours ; les minutes tapées pèsent un quart ; rapproché de 1,1× tant qu’il n’y a pas environ neuf tâches, et par domaine, de la vôtre ; robuste aux valeurs aberrantes (trois écarts médians mis à l’échelle, tenu entre ×8 et ÷8) ; chaque jour garde comme temps libre le 85e centile d’une simulation à graine, au plus un tiers de la journée ([les notes de conception, en anglais](https://aurelienpierre.github.io/sioul/dev/capacity.html)).
