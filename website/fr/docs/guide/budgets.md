---
description: Budgets, réserves et comptes bancaires dans Sioul – dans les temps ou non, dit en mots, les paiements lus dans le courrier, les relevés de la banque rangés selon vos règles, une veille discrète sur ce qui n’est pas sorti, et vos contrats.
---

# Les budgets et les comptes bancaires {#budgets-and-bank-accounts}

L’argent va et vient par courrier : factures, reçus, avis de paiement, versements, avis d’impôt. Sioul sépare trois choses :

- les **comptes bancaires**, là où est l’argent : un compte courant, PayPal, Stripe ;
- les **réserves**, là où il est mis de côté : un Livret A, une assurance vie ;
- les **budgets**, ce à quoi il sert : vos charges (loyer, énergie, courses), vos loisirs, votre travail, un projet.

Vous voyez ainsi d’un regard quels budgets sont positifs ou négatifs, lesquels sont dans les temps, et combien de temps tient chaque réserve. Jamais en rouge.

<figure markdown="span">
  [![La page Budgets, « Budgets, octobre 2026 », avec Nouveau budget : trois budgets (travail, charges du foyer, loisirs), chacun sur une carte avec son nom et sa période, « Dans les temps » sur sa propre ligne, une phrase comme « 16 % du mois écoulé ; 51 % de ce qu’il peut dépenser est dépensé. », puis Jusqu’ici, Attendu au 31 octobre et Objectif, un par ligne.](../assets/screens/fr/budgets.png){ loading=lazy }](../assets/screens/fr/budgets.png "Ouvrir l’image en grand")
  <figcaption>Chaque budget, son verdict en mots, un chiffre par ligne.</figcaption>
</figure>

## Les budgets {#budgets}

**Nouveau budget** :

- **Nom**.
- **Compté par** : le mois ou l’année.
- **Solde à atteindre** à la fin de chaque période : 0 pour l’équilibre ; au-dessus, c’est de l’épargne.
- **Pour** : le travail, vos démarches, les loisirs. Cela décide des heures où le budget est en vue ([Les heures](hours.md)). Rien de coché : vos démarches.

Ce qu’un budget n’a pas dépensé est reporté, comme une enveloppe garde ce qui reste : l’argent mis de côté en octobre pour une facture de décembre est encore là en décembre.

### D’un regard {#at-a-glance}

Chaque budget est une carte : son nom et sa période, le verdict sur sa propre ligne, avec l’écart (*Dans les temps*, *En avance sur son rythme de 40 €*, *25 € de retard sur son rythme*), puis un chiffre par ligne : ce qui est venu jusqu’ici, ce qui est attendu à la fin de la période, le solde à atteindre, ce qui a été reporté, et ce qui est mis de côté pour plus tard.

Le verdict ne suppose pas que l’argent arrive de façon régulière. Ce qui est connu d’avance (un loyer, une facture prévue) compte en entier. Le reste est rapporté à la part de la période écoulée : « 40 % du mois écoulé ; 35 % de ce qu’il lui faut encore est arrivé. » Après trois périodes, la fin attendue vient avec une fourchette, *d’un mauvais mois à un bon*.

### Un budget, ouvert {#a-budget-open}

Un budget s’ouvre sur ses mouvements, du plus récent au plus ancien, et sur son solde par **Jours**, **Semaines**, **Mois** ou **Années**, ce qui est prévu en pointillés.

**Ajouter un mouvement** : **Une fois**, ou **Régulier** (chaque mois ou chaque année, à un jour donné). Un montant est négatif pour une sortie (`-650`) et positif pour une entrée (`1200`). **Une estimation (courses, liquide), pas un montant fixe** dit que la somme se dépense petit à petit : les vraies dépenses sont prises dessus au lieu de s’y ajouter.

Un clic droit sur une ligne : **Modifier cette ligne…**, **Supprimer cette ligne** (avec une confirmation).

**Retirer ce budget** garde ses lignes dans le fichier, telles qu’elles ont été écrites ; elles ne comptent plus.

## Les réserves {#reserves}

**Nouvelle réserve** : son **Nom**, son **Solde** à une date, **Jamais sous** : le montant sous lequel elle ne doit jamais descendre, et **Jours pour arriver** : le temps que met l’argent demandé pour arriver sur votre compte (0 pour un Livret A, dont l’argent arrive tout de suite, une dizaine pour une assurance vie).

Chaque réserve montre son solde aujourd’hui, ce qui a bougé ce mois-ci, son solde à la fin de l’année, et combien de temps elle tient à ce rythme, au-dessus de son plancher.

Quelle réserve couvre quel budget s’écrit dans le fichier des budgets : à la fin de chaque période, ce qui manque à un budget est pris sur sa réserve.

## Le courrier qui parle d’argent {#mail-about-money}

Sous les budgets, le courrier qui parle d’argent, du plus récent au plus ancien. Sioul lit, en français et en anglais, ce que dit un message : un paiement reçu ou fait, une commande, une facture, un remboursement ; le montant ; qui a payé, ou qui a été payé.

Chaque ligne dit où elle en est : *Compté dans* un budget, *Même paiement que* tel autre message (une commande et son paiement ne comptent qu’une fois), ou en attente de vous, avec **Ajouter** et **Pas un paiement**. Une ligne venue d’un expéditeur nouveau et non vérifié demande de vérifier d’abord que le message est authentique. Le courrier falsifié, les indésirables et les expéditeurs bloqués ne sont jamais lus comme des paiements.

Des règles dans le fichier des budgets peuvent envoyer d’elles-mêmes vers un budget les paiements d’un expéditeur, ou ceux qui portent un mot.

## Les comptes bancaires {#bank-accounts}

<figure markdown="span">
  [![Les comptes bancaires sur la page Budgets : deux comptes, chacun sur une carte avec son solde d’après le fichier de la banque, les budgets qu’il remplit, les réserves qui le renflouent, ses derniers mouvements repliés, et Prendre un export…, Règles… et Modifier ; dessous, un paiement qui a pris un autre montant que prévu, les paiements de la semaine, et le mois qui vient en une simple ligne.](../assets/screens/fr/bank-accounts.png){ loading=lazy }](../assets/screens/fr/bank-accounts.png "Ouvrir l’image en grand")
  <figcaption>Chaque compte, les budgets qu’il remplit, les paiements de la semaine et le mois qui vient.</figcaption>
</figure>

**Nouveau compte bancaire** : son **Nom**, son **Type** (une banque, PayPal, Stripe, autre chose), **Remplit** : les budgets qu’il remplit, **Gardé au-dessus de** : le montant sous lequel il ne doit pas descendre, et **Renfloué par** : les réserves qui le renflouent, dans votre ordre.

**Prendre un export…**, sur sa carte : le fichier de relevé que donne votre banque (OFX ou QFX, CAMT.053, ou son CSV), le téléchargement d’activité de PayPal, ou l’historique du solde de Stripe. Lire deux fois le même fichier n’ajoute rien. Rien n’est demandé à la banque elle-même : vous téléchargez le fichier, Sioul le lit.

### Où va chaque mouvement {#where-each-movement-goes}

Dans cet ordre :

1. **Votre choix** : **Où il va**, sur la ligne d’un mouvement, le place dans un budget, ou dans aucun.
2. **Le même paiement qu’une ligne déjà comptée**, venue du courrier ou ajoutée à la main : compté une fois.
3. **Un virement entre vos propres comptes** : dans aucun budget, puisque l’autre côté est lu aussi.
4. **La première de vos règles qui tient** (**Règles…**, sur la carte du compte) : des mots du libellé, une entrée ou une sortie, et la destination : un budget, le paiement régulier qu’il représente, une réserve, un autre compte. Une règle sans mots prend tous les mouvements de son sens.
5. **Le paiement régulier qu’il représente**, par son nom ou son montant, près de son jour : le vrai montant remplace celui qui était prévu.
6. **Le premier budget du compte**.

Chaque mouvement dit pourquoi il est allé là.

### Renfloué à temps {#topped-up-in-time}

Le solde de chaque compte est projeté sur le mois qui vient. Au premier jour où il passerait sous son plancher, Sioul dit quoi verser, depuis quelle réserve, et d’ici quand, en comptant les jours que met cette réserve : « Assurance vie : demandez 590 € d’ici le jeudi 15 octobre ; il faut environ 10 jours, et le loyer part le 1er novembre. » Quand demander aujourd’hui est déjà trop tard, il le dit.

## La veille bancaire {#the-bank-watch}

Un prélèvement qui s’arrête ne le dit pas. Sioul compare les mouvements de votre banque aux paiements que vous attendez, et dit, en phrases :

- **ce qui n’est pas sorti** : « Téléphone (19,99 €) n’est pas sorti du compte. Attendu vers le 12 septembre, absent des mouvements de votre banque. » ;
- **ce qui a changé** : « Électricité a pris 71,40 € le 5 septembre, là où 62 € était attendu. » ;
- **ce que le compte risque de ne pas tenir** : « Le compte risque de ne pas tenir le loyer le 1er novembre. Il manquera alors 579,99 €. », avec la réserve qui le couvre, s’il y en a une.

**Cette semaine** : les paiements des sept prochains jours, en une ligne, ici et en haut du Porche : « Cette semaine : Électricité 62 € (lun.). Le compte les tient. » **Le mois qui vient** : le solde en une simple ligne, le zéro en tirets.

Un rappel vient une fois, une semaine après un paiement qui n’est pas sorti, et cinq jours ouvrés avant un jour où le compte ne tiendrait pas un paiement.

## Contrats et abonnements {#contracts-and-subscriptions}

Ce qui vous engage : loyer, énergie, téléphone et internet, assurances, complémentaire santé, abonnements, hébergement, banque. Les abonnements oubliés font partie de ce que les personnes avec un TDAH appellent la « taxe TDAH » ; avant une démarche en justice, il importe de savoir ce que couvre une assurance.

**Ajouter un contrat** : avec qui, votre numéro chez eux, le paiement régulier qui le paie, depuis quand, quand il se renouvelle (chaque mois, chaque année, ou pas de renouvellement), le préavis qu’il demande, comment l’arrêter (sa page de résiliation, ou où écrire), et ce qu’il couvre.

- **Où il en est**, en une phrase : « Se renouvelle le mardi 1er décembre ; pour l’arrêter, le préavis doit partir au plus tard le 2 octobre. »
- **Le préavis habituel** pour son type est proposé, avec la règle d’où il vient (en France). Le vôtre est celui que dit votre contrat.
- **Un rappel**, une fois, deux semaines avant le dernier jour où un préavis peut partir, pour les contrats qui se renouvellent chaque année.
- **L’arrêter…** : **Ouvrir sa page de résiliation**, ou **Écrire la lettre** : un brouillon de lettre de résiliation, à son adresse, avec votre numéro, qui demande une confirmation écrite. Vous la relisez, la signez et l’envoyez. **Il est fini** garde le contrat, estompé, comme trace.

Les paiements réguliers sans contrat noté sont listés dessous, chacun avec **Le noter**. Un message peut en devenir un avec **Garder comme contrat…**, dans son menu.

Sioul ne montre jamais d’offres, de comparatifs ni de « meilleures affaires » : seulement des faits sur vos contrats.

## Où tout est gardé {#where-it-is-kept}

Dans des fichiers texte simples, à la racine de votre dossier de notes : `sioul-budgets.toml` (budgets, réserves, lignes, règles, comptes bancaires), `sioul-bank.toml` (les mouvements de la banque), `sioul-contracts.toml`. Lisibles, modifiables à la main, versionnés avec git si vous voulez. Rien n’est envoyé nulle part.

Pendant le temps libre, seuls les budgets de loisirs sont en vue. Voir [Les heures](hours.md).

## Pourquoi cela marche ainsi {#why-it-works-this-way}

Les impayés grossissent en silence : un prélèvement s’arrête, rien ne le dit, et personne ne regarde ses comptes quand ils vont mal (Olafsson & Pagel 2017). Un rappel de paiement qui ignore le solde peut faire passer un compte à découvert (Medina 2021). Sioul lit donc les mouvements de la banque elle-même, et dit quand le compte ne tiendra pas un paiement, avant que cela arrive.
