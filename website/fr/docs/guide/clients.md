---
description: Sioul pour les indépendants, les consultants et les micro-entrepreneurs – un projet par client, ses courriels qui lui arrivent tout seuls, son temps compté pendant le travail, et la facture faite de ce temps, numérotée sans trou. Une seule fenêtre, vos propres fichiers, pas d’abonnement.
---

# Travailler pour des clients {#working-for-clients}

Indépendants, consultants, micro-entrepreneurs : vous vendez votre temps. D’habitude, il y faut un outil de suivi du temps, un tableur des heures, un service de facturation, et votre boîte de courriel ouverte à côté, avec votre mémoire pour tenir le tout.

Dans Sioul, c’est une seule boucle, dans une seule fenêtre. Les courriels du client arrivent à son projet. Le travail se compte pendant que vous le faites. La facture est faite de ce qui a été compté, et attend dans votre budget jusqu’à son paiement.

<figure markdown="span">
  [![La page Projets : trois projets listés à gauche, avec Nouveau projet ; l’un ouvert à droite : « Pour un client », son taux, Tableau, Liste, Gantt, Noter du temps et Faire la facture ; ses tâches ouvertes et faites, le temps noté et à facturer ; « Les courriels qui arrivent ici tout seuls », replié ; ses factures, l’une marquée Payée ; puis « Sur une ligne de temps », ce qui vient et ce qui s’est passé avant.](../assets/screens/fr/projects.png){ loading=lazy }](../assets/screens/fr/projects.png "Ouvrir l’image en grand")
  <figcaption>Le projet d’un client : ses tâches, son temps, ses courriels, ses factures, sur une ligne de temps.</figcaption>
</figure>

## La boucle, du premier courriel à la facture payée {#the-loop-from-the-first-mail-to-the-paid-invoice}

1. **Un projet par client.** **Nouveau ▾ ▸ Un projet**, puis **Pour un client : son temps se facture à un taux horaire** : pour qui, ce que coûte une heure, et le budget où son argent est attendu. Quand **Pour** nomme l’un de vos contacts (son nom, ou son organisation), la facture prend la fiche du client quand il en a une ([Vos clients](time.md#your-clients)), sinon l’adresse postale du contact. **Lier à…**, au bas du projet, y lie le contact du client, avec tout ce qui en fait partie : une note, un événement, un site. [Les projets](projects.md#making-a-project)
2. **Ses courriels arrivent tout seuls.** Donnez au projet le domaine ou les adresses du client, des mots d’un objet, ou du nom d’une pièce jointe (« devis ») : les courriels qui correspondent y sont rangés à leur arrivée, chacun avec la raison de sa venue. Les réponses suivent leur conversation. Sioul vérifie d’abord qui a envoyé chaque courriel : un courriel falsifié n’est jamais rangé dans le projet d’un client, même quand son adresse ressemble à la sienne. [Les courriels qui arrivent ici tout seuls](projects.md#mail-that-comes-here-by-itself)
3. **Le travail tient dans vos heures de travail.** Les tâches du projet, en tableau, en liste ou en diagramme de Gantt ; la prochaine étape, placée dans les heures que vous gardez pour le travail. [Les tâches](tasks.md)
4. **Le temps se compte pendant que vous travaillez.** Les minutes d’une séance de concentration vont à sa tâche, et au projet de la tâche. Une réunion, un appel, un travail fait loin du minuteur : **Noter du temps**, `1h30`. Une tâche peut rester hors de la facture. [D’où vient le temps](time.md#where-time-comes-from)
5. **Ce qui reste à facturer reste en vue** : pour chaque projet, en heures et en argent, sur la page Temps et à côté du projet. [La page Temps](time.md#the-time-page)
6. **La facture, faite de ce temps.** **Faire la facture** : une ligne par tâche, au taux du projet ; ce que demandent les règles de votre pays, vérifié d’abord, et dit quand quelque chose manque ; un numéro qui suit le dernier et n’est jamais donné deux fois ; le temps facturé marqué comme tel, pour que la facture suivante le laisse de côté ; un PDF/A-3 qui porte sa Factur-X, gardé tel qu’émis, prêt à envoyer. [Les factures](time.md#invoices)
7. **Jusqu’au paiement.** Le total est attendu dans le budget du projet le jour de son échéance, à trente jours sauf si vous avez dit autre chose. Quand vous prenez l’export de votre banque, le virement du client est inscrit sur la facture de lui-même, avec Annuler ; **Payée** le dit à la main. Une facture erronée se corrige par un avoir, jamais en la changeant. [Payée, et les avoirs](time.md#paid-and-credit-notes), [Les budgets](budgets.md)

<figure markdown="span">
  [![La page Temps sur « Semaine » : une barre par jour, empilée par projet en quelques couleurs calmes ; dessous, les heures de chaque projet et ce qui reste à facturer, puis chaque plage de temps, de la plus récente à la plus ancienne.](../assets/screens/fr/time.png){ loading=lazy }](../assets/screens/fr/time.png "Ouvrir l’image en grand")
  <figcaption>La semaine, par projet, et ce qui reste à facturer.</figcaption>
</figure>

## Réglé une fois {#set-once}

**Paramètres ▸ Factures** : votre nom ou votre raison sociale, votre adresse, votre SIRET ou votre numéro d’entreprise, la mention de TVA, le début des numéros de facture, votre monnaie, vos coordonnées de paiement, votre taux horaire. Chaque facture les porte ensuite. [Paramètres ▸ Factures](settings.md#invoices)

Les règles de votre pays font le reste : Sioul connaît celles de la France, de l’Allemagne, de la Belgique, du Royaume-Uni et de la Suisse, et **Ailleurs** vous laisse chaque ligne ; l’Espagne et l’Italie ne sont pas prises en charge, et Sioul dit pourquoi. En France, votre forme juridique, votre régime d’imposition et votre régime de TVA sont trois réglages à part, faits pour l’instant dans le fichier de configuration. [Les règles de votre pays](time.md#your-countrys-rules)

Pour un micro-entrepreneur, la facture imprime d’elle-même « EI » après votre nom, votre SIREN ou votre SIRET, « TVA non applicable, article 293 B du CGI » en franchise en base de TVA (la TVA par taux une fois que vous y êtes assujetti), la date ou la période de la prestation, et ce qu’elle vend, des prestations de services ou des biens. Pour un client professionnel, elle ajoute « Escompte pour paiement anticipé : néant », les pénalités de retard au taux de la BCE majoré de 10 points et l’indemnité forfaitaire de 40 € pour frais de recouvrement ; pour un particulier, rien de tout cela. Pour une prestation de services à une entreprise d’un autre pays de l’UE, elle imprime « Autoliquidation » et les deux numéros de TVA, en franchise aussi ; à une entreprise hors de l’UE, « TVA non applicable – article 259-1 du CGI ». Avant d’émettre, Sioul dit ce qui manque, comme le SIREN d’un client professionnel, que les micro-entreprises doivent donner à partir du 1er septembre 2027. Hors de France, la mention de TVA et les coordonnées de paiement sont à écrire à votre façon.

Chaque client peut avoir une fiche : sa dénomination, ses adresses de facturation et de livraison, ses identifiants, la langue, la devise et le délai de paiement de ses factures. Sans fiche, un client est facturé comme une entreprise ; une fiche dit quand le client est un particulier. [Vos clients](time.md#your-clients)

Avec le préfixe habituel, l’année, les numéros de facture repartent chaque mois de janvier ; avec votre propre préfixe, ils continuent après le dernier ([Les factures](time.md#invoices)).

## Pour votre comptable {#for-your-accountant}

**Le temps facturable, en tableur (CSV)** : un projet, une période (ce mois-ci, le mois dernier, d’un jour à un autre), une ligne par tâche avec ses heures, son taux et son montant, puis le total. Un titre venu d’un courriel de quelqu’un est écrit comme du simple texte, jamais comme une formule. [Un tableur du temps facturable](time.md#a-spreadsheet-of-billable-time)

## Sur votre ordinateur de bureau et votre portable {#on-your-desktop-and-your-laptop}

Le temps noté sur un appareil est sur les autres une minute plus tard, et les factures aussi : scellés de bout en bout, par un dossier que votre synchronisation transporte déjà (Nextcloud, Dropbox, Syncthing). Aucun serveur à nous entre les deux. Les projets voyagent avec votre dossier de notes, ou par ce même partage là où aucune synchronisation ne transporte ce dossier ; les tâches et les événements, par le serveur de votre agenda. Un seul appareil numérote les factures, pour qu’un numéro ne soit jamais donné deux fois. [Partager entre vos appareils](sharing.md)

Sur un téléphone ou une tablette : Sioul pour Android sort avec chaque version depuis la 0.0.2, pour les téléphones et tablettes 64 bits sous Android 9 ou plus récent ([Installer](install.md#on-android)). Il partage avec vos ordinateurs par le même dossier, quand son application de synchronisation le transporte ([Sur un téléphone ou une tablette](first-steps.md#on-a-phone)).

## Et puis le travail se repose {#and-then-it-rests}

En dehors de vos heures de travail, les projets des clients et la page Temps se reposent : « Les projets de travail se reposent jusqu’au retour du travail. » Être payé à l’heure n’oblige pas à travailler tous les soirs. [Les heures](hours.md)

## Pas encore là {#not-there-yet}

- **Les factures dans la fenêtre** : une liste de toutes les factures et de tous les brouillons, un éditeur pour toute ligne (un forfait, des frais, des biens), Paramètres ▸ Factures par pays, les fiches des clients dans Contacts. Pour l’instant, **Faire la facture** facture des heures ; les lignes d’autres sortes viennent du brouillon d’un [agent d’IA](ai-agent.md), émis en ligne de commande. Les devis ne se font pas encore.
- **La facture électronique.** À partir du 1er septembre 2027, une micro-entreprise française, en franchise de TVA aussi, doit émettre ses factures aux entreprises françaises sous forme électronique, par une plateforme agréée, avec le SIREN du client. Chaque facture que Sioul émet porte déjà sa Factur-X, dans le PDF/A-3 gardé tel qu’émis, que la plupart des plateformes acceptent en fichier ; l’envoi par votre plateforme depuis Sioul viendra plus tard ([Le temps et les factures](time.md#not-there-yet)).
- **Les livres** : le livre des recettes fait des paiements de vos factures, les déclarations à l’URSSAF par catégorie, et un mot quand vous approchez d’un seuil.

## Pourquoi cela fonctionne ainsi {#why-it-works-this-way}

- Les personnes qui travaillent pour elles-mêmes déclarent et paient leurs propres impôts, là où un employeur le fait pour ses salariés (Milanez & Bratta 2019), et le coût des obligations pèse le plus sur les plus petites entreprises (Evans 2003). Les personnes handicapées travaillent plus souvent pour elles-mêmes : 9,1 % de celles qui avaient un emploi aux États-Unis en 2025, contre 5,9 % des autres (U.S. Bureau of Labor Statistics 2026). Alors Sioul porte aussi les démarches d’une petite entreprise, du premier courriel du client à la facture payée.
- Avec des revenus irréguliers, ce qui compte est combien de temps l’argent dure : le revenu mensuel a varié de 39 % sur une année de carnets de ménages (Hannagan & Morduch 2015). Alors un budget de travail peut mettre de côté une part de chaque paiement pour les impôts dès son arrivée, et les réserves se comptent en semaines de vos propres dépenses.

Plus de détails dans [ce que dit la recherche (en anglais)](https://sioul.aurelienpierre.com/dev/research.html), constats 31 et 44.

## Comparé à d’autres applications {#compared-with-other-apps}

Toggl Track, Clockify, Harvest, Kimai et Freebe en font plus pour les devis, les frais et la TVA, et Freebe fait déjà les factures électroniques que la France va demander. Sioul garde toute la boucle, des courriels du client à la facture payée, dans une seule fenêtre et dans des fichiers sur vos appareils. Les tableaux, en octobre 2026 : [Le temps et les factures](time.md#compared-with-other-apps) et [Les projets](projects.md#compared-with-other-apps).

## À vous {#yours}

Votre temps, vos clients et vos factures restent sur votre appareil, en fichiers ordinaires : pas de compte à ouvrir, pas d’abonnement, pas de serveur à nous. Sioul est un logiciel libre (GPL-3.0 ou ultérieure). [Vie privée et sécurité](privacy-security.md)
