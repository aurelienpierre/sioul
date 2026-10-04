---
description: Le temps passé et les factures dans Sioul – le temps du minuteur de concentration et celui noté à la main, ce qui reste à facturer, un export en tableur, et des factures numérotées sans trou.
---

# Le temps et les factures {#time-and-invoices}

Le temps passé se note pendant que vous travaillez, et devient des factures pour les projets que vous menez pour des clients.

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

Un clic droit sur du temps noté à la main : **Modifier ce temps**, ou **Enlever ce temps**. Le temps déjà sur une facture reste tel qu’il a été facturé.

### Facturé ou non {#billed-or-not}

Le travail pour un client se facture. Une tâche peut dire autre chose, sous **Facturé** dans son panneau : *Comme son projet*, *Son temps se facture*, ou *Non facturé*.

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
