---
description: Les projets dans Sioul – toute affaire que vous suivez, pour un client ou pour vous, qui rassemble ses tâches, ses événements, son courrier, ses notes, son temps et ses factures sur une ligne de temps.
---

# Les projets {#projects}

Un projet, c’est toute affaire que vous suivez : un travail pour un client, dont le temps se facture, ou une affaire à vous, comme le logement, la santé, une déclaration d’impôts ou un voyage. Il rassemble ce qui lui appartient : tâches, événements, courrier, notes, temps passé, factures.

<figure markdown="span">
  [![La page Projets : trois projets listés à gauche, avec Nouveau projet ; l’un ouvert à droite : « Pour un client », son taux, Tableau, Liste, Calendrier, Noter du temps et Faire la facture ; ses tâches ouvertes et faites, le temps noté et à facturer ; « Le courrier qui arrive ici tout seul », replié ; ses factures, l’une marquée Payée ; puis « Sur une ligne de temps », ce qui vient et ce qui s’est passé avant.](../assets/screens/fr/projects.png){ loading=lazy }](../assets/screens/fr/projects.png "Ouvrir l’image en grand")
  <figcaption>Un projet : ce qui vient, puis ce qui s’est passé.</figcaption>
</figure>

## Créer un projet {#making-a-project}

**Nouveau projet** (ou **Nouveau ▾ ▸ Un projet**) :

- **Nom**.
- **Pour un client : son temps se facture à un taux horaire**. Puis : **Pour** (pour qui), **Une heure coûte** (sinon le taux de [Paramètres ▸ Factures](settings.md#invoices)), et **Factures attendues dans** : le budget où l’argent est attendu jusqu’au paiement. Quand **Pour** nomme l’un de vos [contacts](contacts.md) (son nom, ou son organisation), les factures prennent son adresse postale.
- Sans cela, le projet est à vous. **À vous, hors travail** garde ses tâches en vue pendant le calme, avec la famille et les amis.
- **Où il en est** : ouvert, en attente ou fermé.

**Retirer de la liste** enlève un projet. Ses tâches, notes, courriels et temps restent où ils sont ; ils ne sont plus rassemblés sous lui.

## La page d’un projet {#a-projects-page}

À gauche, tous les projets, avec leurs tâches ouvertes et le temps qui reste à facturer. À droite, le projet ouvert :

- **Sur une ligne de temps** : d’abord ce qui est **À venir** (les dates demandées de ses tâches ouvertes, les événements), puis ce qui s’est passé **Avant**, du plus récent au plus ancien : tâches faites, courrier, notes modifiées, temps noté, factures.
- **Ses tâches**, en **Tableau**, en **Liste** ou en **Calendrier** : la page [Tâches](tasks.md), limitée à ce projet.
- **Noter du temps** : une réunion, un appel, un travail fait loin du minuteur.
- **Faire la facture**, quand du temps facturable attend d’être facturé. Voir [Le temps et les factures](time.md).
- **Ajouter ▾** et **Lier à…**, au bas : quelque chose de nouveau lié au projet, ou un lien vers ce qui existe, comme le contact du client.

## Le courrier qui arrive ici tout seul {#mail-that-comes-here-by-itself}

**Le courrier qui arrive ici tout seul**, sur la page du projet, donne au projet ses règles. Un message va au projet quand une règle correspond ; chaque ligne remplie dans une règle doit correspondre :

- **Des domaines** : le domaine de l’expéditeur, ou l’un de ses sous-domaines ;
- **Des adresses** ;
- **Mots dans le sujet** ;
- **Mots dans le texte** ;
- **Mots dans le nom d’une pièce jointe** (« devis » prend `Devis_2026.pdf`).

Plusieurs mots ou domaines sur une ligne : n’importe lequel d’entre eux. Les mots se comparent entiers, sans tenir compte des majuscules ni des accents.

Une réponse dans une conversation du projet va avec lui, comme un message que vous y liez à la main (**Lier à…**). Chaque message rangé garde sa raison, montrée sur demande : « domaine de l’expéditeur : example.org ».

Sur le Porche, un projet peut avoir sa propre file : **Projets montrés ici**, dans le ⚙ du Porche.

## Où vivent les projets {#where-projects-live}

Dans un seul fichier, `sioul-cases.toml`, à la racine de votre dossier de notes : lisible, et modifiable à la main si vous voulez. Il voyage avec votre dossier de notes, ou par le partage de Sioul une fois **Projets et argent** activé dans le partage ([Le partage](sharing.md)). Les tâches et les événements d’un projet portent son identifiant dans un champ standard, pour que les autres programmes d’agenda gardent le regroupement.

## Pendant le calme {#in-quiet-time}

Les projets pour des clients se reposent en dehors des heures de travail : « Les projets de travail se reposent jusqu’au retour du travail. » **Montrer quand même** les montre malgré tout. Pendant le sommeil, toute la page attend derrière une phrase et **Montrer quand même**. Voir [Les heures](hours.md).
