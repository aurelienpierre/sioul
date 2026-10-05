---
description: Le temps passé et les factures dans Sioul – le temps du minuteur de concentration et celui noté à la main, gardé pour facturer les clients et pour apprendre combien de temps les choses prennent vraiment ; ce qui reste à facturer, un export en tableur, et des factures numérotées sans trou.
---

# Le temps et les factures {#time-and-invoices}

Le temps passé se note pendant que vous travaillez. Il est gardé pour deux usages :

- **facturer** : les heures que vous travaillez pour des clients deviennent des factures ;
- **prévoir** : mis à côté de vos estimations, il dit combien de temps les choses prennent vraiment. Le plan s’en servira ensuite : voir [Combien de temps prennent les choses](#how-long-things-take).

<figure markdown="span">
  [![La page Temps sur « Semaine » : une barre par jour, empilée par projet en quelques couleurs calmes ; dessous, les heures de chaque projet et ce qui reste à facturer, puis chaque plage de temps, de la plus récente à la plus ancienne.](../assets/screens/fr/time.png){ loading=lazy }](../assets/screens/fr/time.png "Ouvrir l’image en grand")
  <figcaption>Une semaine de temps, par projet, et ce qui reste à facturer.</figcaption>
</figure>

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

### Facturé ou non {#billed-or-not}

Le travail pour un client se facture. Une tâche peut dire autre chose, sous **Facturé** dans son panneau : *Comme son projet*, *Son temps se facture*, ou *Non facturé*.

## Combien de temps prennent les choses {#how-long-things-take}

Le **Prend environ** d’une tâche est une estimation. Le temps noté pour elle est ce qu’elle a pris. Les deux ensemble disent comment vos estimations se comparent au temps que les choses prennent vraiment.

**Aujourd’hui**, le relevé est là : la fenêtre de concentration, le temps de cette page, chaque ligne modifiable. Le plan ne s’en sert pas encore : il prévoit chaque tâche selon son **Prend environ**, tel que vous l’avez écrit.

**Ensuite**, le plan s’en servira pour corriger vos estimations, dans le plan seulement :

- il compare le temps passé au temps estimé, sur les deux dernières semaines environ, pour chaque type de tâche ;
- il dimensionne le plan avec ce rapport : si les lettres ont pris le double de votre estimation, la prochaine lettre reçoit le double de la sienne dans le plan ;
- la marge pour l’imprévu reste sur la journée entière, pas sur chaque tâche.

Il ne vous note jamais, ne montre jamais rien en retard, ne remplit jamais **Prend environ** à votre place, et ne vous compare jamais à personne.

Chronométrer les loisirs, c’est à vous de voir. Les trous ne posent pas de problème : le temps non noté est laissé de côté, il ne compte contre rien.

## Un tableur du temps facturable {#a-spreadsheet-of-billable-time}

Le bouton d’export, en haut à droite de la page Temps : **Le temps facturable, en tableur (CSV)…**, pour un projet et une période (cette semaine, la semaine dernière, ce mois-ci, le mois dernier, depuis toujours, ou d’un jour à un autre). Une ligne par tâche, avec ses heures, le taux horaire et le montant, puis le total. En français, le fichier utilise `;` et la virgule décimale, comme les tableurs français les lisent.

## Les factures {#invoices}

Sur la page d’un projet, ou à côté du projet sur la page Temps, **Faire la facture** quand du temps facturable attend.

- **Les numéros** se suivent dans l’année et ne sont jamais réutilisés : 2026-001, 2026-002, ou avec votre propre préfixe.
- **Les lignes** : une par tâche (ou par temps noté sans tâche), avec ses heures au taux du projet.
- **Facturé une fois** : le temps facturé porte le numéro de sa facture ; la facture suivante le laisse de côté.
- **Les mentions que demande la loi française** : votre nom, votre adresse et votre SIRET, la mention de TVA (« TVA non applicable, art. 293 B du CGI » pour une micro-entreprise), les modalités de paiement, les pénalités de retard et l’indemnité forfaitaire de 40 € pour frais de recouvrement.
- **Le PDF** (A4) va dans `Documents/Factures` de votre dossier personnel, ou dans le dossier choisi. **Imprimer à nouveau** le réécrit.
- **Votre budget** : le total est attendu dans le budget du projet, payable à trente jours. **Payée** inscrit le paiement à la date du jour, et l’argent compte.

Votre nom, votre adresse, vos numéros, la devise, les modalités de paiement et le taux horaire se règlent une fois, dans [Paramètres ▸ Factures](settings.md#invoices).

### Sur plusieurs ordinateurs {#on-several-computers}

Un numéro de facture ne doit jamais être donné deux fois. Quand vous [partagez entre vos ordinateurs](sharing.md), un seul ordinateur numérote les factures : un autre dit « Les factures sont numérotées sur … », et **Faire les factures sur cet ordinateur** les reprend, après une minute et demie, une fois que vos autres ordinateurs le savent. Si votre dossier de partage ne peut pas être écrit, ou si un autre ordinateur ne s’est pas manifesté depuis quelques minutes, Sioul attend plutôt que de risquer un numéro en double.

## Pendant le calme {#in-quiet-time}

En dehors des heures de travail, la page Temps attend derrière **Montrer quand même**. Voir [Les heures](hours.md).
