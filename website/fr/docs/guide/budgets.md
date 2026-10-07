---
description: Budgets, réserves et comptes bancaires dans Sioul – dans les temps ou non, dit en mots, jamais en rouge ; les paiements lus dans le courrier, les fichiers d’export de votre banque lus sur votre appareil, une veille discrète sur ce qui n’est pas sorti, et vos contrats avec leurs dates de préavis.
---

# Les budgets et les comptes bancaires {#budgets-and-bank-accounts}

## En bref {#in-short}

L’argent va et vient par courrier : factures, reçus, avis de paiement, versements, avis d’impôt. La page Budgets vous dit où en est votre argent, en phrases, et sépare trois choses :

- les **comptes bancaires**, là où est l’argent : un compte courant, PayPal, Stripe ;
- les **réserves**, là où il est mis de côté : un Livret A, une assurance vie ;
- les **budgets**, ce à quoi il sert : vos charges (loyer, énergie, courses), vos loisirs, votre travail, un projet.

Sioul lit les fichiers d’export de votre banque et le courrier qui parle de paiements, les compare aux paiements que vous attendez, et dit calmement quand l’un n’est pas sorti, ou quand un compte risque de ne pas tenir le suivant, assez tôt pour virer de l’argent. Il ne se connecte jamais à votre banque, ne montre jamais de rouge, et ne montre jamais d’offres.

<figure markdown="span">
  [![La page Budgets, « Budgets, octobre 2026 », avec Nouveau budget : trois budgets (travail, charges du foyer, loisirs), chacun sur une carte avec son nom et sa période, « Dans les temps » sur sa propre ligne, une phrase comme « 16 % du mois écoulé ; 51 % de ce qu’il peut dépenser est dépensé. », puis Jusqu’ici, Attendu au 31 octobre et Objectif, un par ligne.](../assets/screens/fr/budgets.png){ loading=lazy }](../assets/screens/fr/budgets.png "Ouvrir l’image en grand")
  <figcaption>Chaque budget, son verdict en mots, un chiffre par ligne.</figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

- **Sioul ne se connecte jamais à votre banque** et ne garde aucun mot de passe bancaire : vous téléchargez le fichier de votre banque, et Sioul le lit sur votre appareil.
- **Vos budgets, les mouvements de votre banque et vos contrats sont des fichiers lisibles** dans votre propre dossier de notes. Il n’y a pas de serveur de Sioul.
- **Entre vos appareils**, ils voyagent par la synchronisation de votre dossier, ou scellés par le partage de Sioul une fois **Projets et argent** activé : le dossier et son serveur n’en peuvent rien lire ([Le partage](sharing.md)).
- **Un courriel n’est lu comme un paiement qu’après avoir passé les vérifications de Sioul** : le courrier falsifié, les indésirables, un expéditeur qui emprunte le nom d’une banque ou d’une boutique, et les expéditeurs bloqués ne sont jamais lus comme des paiements. Un paiement venu de quelqu’un de nouveau et non vérifié demande de vérifier d’abord : un faux « votre paiement » est une ruse courante.
- **Un agent d’IA**, seulement si vous en connectez un, lit un résumé de vos budgets, ne déplace jamais d’argent, et voit les numéros de compte masqués.
- **Pas d’offres**, pas de comparatifs, pas de « meilleures affaires », pas de publicité, pas de mesure d’audience.

## Les budgets {#budgets}

**Nouveau budget** :

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

**Ajouter un mouvement** : **Une fois**, ou **Régulier** (chaque mois ou chaque année, à un jour donné). Un montant est négatif pour une sortie (`-650`) et positif pour une entrée (`1200`). Pour ce qui se dépense petit à petit, comme les courses, voir [Les dépenses du quotidien](#daily-spending).

Un clic droit sur une ligne : **Modifier cette ligne…**, **Supprimer cette ligne** (avec une confirmation).

**Retirer ce budget** garde ses lignes dans le fichier, telles qu’elles ont été écrites ; elles ne comptent plus.

## Les réserves {#reserves}

**Nouvelle réserve** : son **Nom**, son **Solde** à une date, **Jamais sous** : le montant sous lequel elle ne doit jamais descendre, et **Jours pour arriver** : le temps que met l’argent demandé pour arriver sur votre compte (0 pour un Livret A, dont l’argent arrive tout de suite, une dizaine pour une assurance vie).

Chaque réserve montre son solde aujourd’hui, ce qui a bougé ce mois-ci, son solde à la fin de l’année, et combien de temps elle tient à ce rythme, au-dessus de son plancher.

Quelle réserve couvre quel budget s’écrit dans le fichier des budgets : à la fin de chaque période, ce qui manque à un budget est pris sur sa réserve.

## Le courrier qui parle d’argent {#mail-about-money}

Sous les budgets, le courrier qui parle d’argent, du plus récent au plus ancien. Sioul lit, en français et en anglais, ce que dit un message : un paiement reçu ou fait, une commande, une facture, un remboursement ; le montant ; qui a payé, ou qui a été payé.

Chaque ligne dit où elle en est : *Compté dans* un budget, *Même paiement que* tel autre message (une commande et son paiement ne comptent qu’une fois), ou en attente de vous, avec **Ajouter** et **Pas un paiement**. Une ligne venue d’un expéditeur nouveau et non vérifié demande de vérifier d’abord que le message est authentique. Le courrier falsifié, les indésirables et les expéditeurs bloqués ne sont jamais lus comme des paiements. Un montant dans une autre monnaie est marqué « à convertir » : Sioul ne le convertit pas encore.

Des règles dans le fichier des budgets peuvent envoyer d’elles-mêmes vers un budget les paiements d’un expéditeur, ou ceux qui portent un mot.

## Les comptes bancaires {#bank-accounts}

<figure markdown="span">
  [![Les comptes bancaires sur la page Budgets : deux comptes, chacun sur une carte avec son solde d’après le fichier de la banque, les budgets qu’il remplit, les réserves qui le renflouent, ses derniers mouvements repliés, et Prendre un export…, Règles… et Modifier ; dessous, un paiement qui a pris un autre montant que prévu, les paiements de la semaine, et le mois qui vient en une simple ligne.](../assets/screens/fr/bank-accounts.png){ loading=lazy }](../assets/screens/fr/bank-accounts.png "Ouvrir l’image en grand")
  <figcaption>Chaque compte, les budgets qu’il remplit, les paiements de la semaine et le mois qui vient.</figcaption>
</figure>

**Nouveau compte bancaire** : son **Nom**, son **Type** (une banque, PayPal, Stripe, autre chose), **Remplit** : les budgets qu’il remplit, **Gardé au-dessus de** : le montant sous lequel il ne doit pas descendre, et **Renfloué par** : les réserves qui le renflouent, dans votre ordre.

**Prendre un export…**, sur sa carte : le fichier de relevé que donne votre banque (OFX ou QFX, CAMT.053, ou son CSV), le téléchargement d’activité de PayPal, ou l’historique du solde de Stripe. Lire deux fois le même fichier n’ajoute rien. Rien n’est demandé à la banque elle-même : vous téléchargez le fichier, Sioul le lit. Chaque mouvement va ensuite dans un budget selon votre choix, vos règles ou le paiement qu’il représente, et dit pourquoi ([Où va chaque mouvement](#where-each-movement-goes)).

### Renfloué à temps {#topped-up-in-time}

Le solde de chaque compte est projeté sur le mois qui vient. Au premier jour où il passerait sous son plancher, Sioul dit quoi verser, depuis quelle réserve, et d’ici quand, en comptant les jours que met cette réserve : « Assurance vie : demandez 590 € d’ici le jeudi 15 octobre ; il faut environ 10 jours, et le loyer part le 1er novembre. » Quand demander aujourd’hui est déjà trop tard, il le dit.

## La veille bancaire {#the-bank-watch}

Un prélèvement qui s’arrête ne le dit pas. Sioul compare les mouvements de votre banque aux paiements que vous attendez, et dit, en phrases :

- **ce qui n’est pas sorti** : « Téléphone (19,99 €) n’est pas sorti du compte. Attendu vers le 12 septembre, absent des mouvements de votre banque. » ;
- **ce qui a changé** : « Électricité a pris 71,40 € le 5 septembre, là où 62 € était attendu. » ;
- **ce que le compte risque de ne pas tenir** : « Le compte risque de ne pas tenir le loyer le 1er novembre. Il manquera alors 579,99 €. », avec la réserve qui le couvre, s’il y en a une.

**Cette semaine** : les paiements des sept prochains jours, en une ligne, ici et en haut du Porche : « Cette semaine : Électricité 62 € (lun.). Le compte les tient. » **Le mois qui vient** : le solde en une simple ligne, le zéro en tirets.

**Des rappels**, une fois chacun, au début du travail : deux jours ouvrés avant un paiement prévu (une facture, un impôt), en disant si le compte le tiendra ; une semaine après un paiement qui n’est pas sorti ; cinq jours ouvrés avant un jour où le compte ne tiendrait pas un paiement. Ils ne viennent que d’un ordinateur, quand Sioul est ouvert ou, si vous le demandez, fenêtre fermée (Paramètres ▸ [Rappels](settings.md#reminders) ; pas encore sous Windows). Un téléphone ne les dit pas.

## Contrats et abonnements {#contracts-and-subscriptions}

Ce qui vous engage : loyer, énergie, téléphone et internet, assurances, complémentaire santé, abonnements, hébergement, banque. Les abonnements oubliés font partie de ce que les personnes avec un TDAH appellent la « taxe TDAH » ; avant une démarche en justice, il importe de savoir ce que couvre une assurance.

**Ajouter un contrat** : avec qui, votre numéro chez eux, le paiement régulier qui le paie, depuis quand, quand il se renouvelle (chaque mois, chaque année, ou pas de renouvellement), le préavis qu’il demande, comment l’arrêter (sa page de résiliation, ou où écrire), et ce qu’il couvre.

- **Où il en est**, en une phrase : « Se renouvelle le mardi 1er décembre ; pour l’arrêter, le préavis doit partir au plus tard le 2 octobre. »
- **Le préavis habituel** pour son type est proposé, avec la règle d’où il vient (en France). Le vôtre est celui que dit votre contrat.
- **Un rappel**, une fois, deux semaines avant le dernier jour où un préavis peut partir, pour les contrats qui se renouvellent chaque année. Comme les rappels sur l’argent, il ne vient que d’un ordinateur.
- **L’arrêter…** : **Ouvrir sa page de résiliation**, ou **Écrire la lettre** : un brouillon de lettre de résiliation, à son adresse, avec votre numéro, qui demande une confirmation écrite. Vous la relisez, la signez et l’envoyez. **Il est fini** garde le contrat, estompé, comme trace.

Les paiements réguliers sans contrat noté sont listés dessous, chacun avec **Le noter**. Un message peut en devenir un avec **Garder comme contrat…**, dans son menu.

Sioul ne montre jamais d’offres, de comparatifs ni de « meilleures affaires » : seulement des faits sur vos contrats.

## Où tout est gardé {#where-it-is-kept}

Dans des fichiers texte simples, à la racine de votre dossier de notes : `sioul-budgets.toml` (budgets, réserves, lignes, règles, comptes bancaires), `sioul-bank.toml` (les mouvements de la banque), `sioul-contracts.toml`. Lisibles, modifiables à la main, versionnés avec git si vous voulez. Ils voyagent avec votre dossier de notes, ou scellés par le partage de Sioul une fois **Projets et argent** activé dans le partage ([Le partage](sharing.md)) ; rien n’est envoyé nulle part ailleurs.

Pendant les loisirs, un repas et le sommeil, seuls les budgets de loisirs sont en vue. Voir [Les heures](hours.md).

## Pour aller plus loin {#going-further}

### Où va chaque mouvement {#where-each-movement-goes}

Dans cet ordre :

1. **Votre choix** : **Où il va**, sur la ligne d’un mouvement, le place dans un budget, ou dans aucun.
2. **Le même paiement qu’une ligne déjà comptée**, venue du courrier ou ajoutée à la main : compté une fois.
3. **Un virement entre vos propres comptes** : dans aucun budget, puisque l’autre côté est lu aussi.
4. **La première de vos règles qui tient** (**Règles…**, sur la carte du compte) : des mots du libellé, une entrée ou une sortie, et la destination : un budget, le paiement régulier qu’il représente, une réserve, un autre compte. Une règle sans mots prend tous les mouvements de son sens.
5. **Le paiement régulier qu’il représente**, par son nom ou son montant, près de son jour : le vrai montant remplace celui qui était prévu.
6. **Le premier budget du compte**.

Chaque mouvement dit pourquoi il est allé là.

### Les dépenses du quotidien {#daily-spending}

**Une estimation (courses, liquide), pas un montant fixe**, sur une ligne régulière, dit que la somme se dépense petit à petit : un paiement par carte qui ne représente rien d’autre est pris dessus, l’estimation à qui il reste le plus d’abord, et seule la dépense au-delà compte en plus. Les vraies dépenses sont prises sur l’estimation, jamais ajoutées par-dessus.

### Télécharger le fichier de votre banque {#downloading-your-banks-file}

Le site de votre banque peut être gardé dans [Sites](sites.md) : Sioul remplit votre identifiant depuis Bitwarden, et votre clé de sécurité répond là où votre banque en accepte une. Le fichier que vous y téléchargez n’est alors qu’à un **Prendre un export…** de distance.

### Dans le terminal {#in-the-terminal}

`sioul budgets` affiche les budgets, les réserves et les paiements que propose le courrier, comme la page les dit.

### Avec un agent d’IA {#with-an-ai-agent}

Si vous connectez un agent d’IA ([Avec un agent d’IA](ai-agent.md)), il peut lire ce que dit cette page : les budgets, les réserves, le dernier solde des comptes bancaires et ce que la veille a remarqué. Il ne peut ni déplacer d’argent ni changer une ligne, et il voit les numéros de compte masqués.

## Pourquoi cela marche ainsi {#why-it-works-this-way}

Les impayés grossissent en silence : un prélèvement s’arrête, rien ne le dit, et personne ne regarde ses comptes quand ils vont mal (Olafsson & Pagel 2017). Un rappel de paiement qui ignore le solde peut faire passer un compte à découvert (Medina 2021). Sioul lit donc les mouvements de la banque elle-même, et dit quand le compte ne tiendra pas un paiement, avant que cela arrive.

## Comparé à d’autres applications {#compared-with-other-apps}

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

**Pour tout le monde**

| | Sioul | YNAB | Actual | Firefly III | GnuCash | Bankin’ |
|---|---|---|---|---|---|---|
| Des budgets selon ce à quoi sert l’argent ; ce qui n’est pas dépensé, gardé | ✓ | ✓ | ✓ | en partie¹ | en partie² | en partie³ |
| Où en est chaque budget, au rythme de la période, en mots | ✓ | en partie⁴ | ✗ | ✗ | en partie⁵ | ✗ |
| Les paiements attendus, rapprochés des mouvements de la banque | ✓ | ✓ | ✓ | ✓ | ✓ | en partie⁶ |
| Dit quand un paiement attendu n’est pas sorti, ou a pris un autre montant | ✓ | ✗ | en partie⁷ | en partie⁸ | ✗ | ✗ |
| Dit à l’avance qu’un compte ne tiendra pas un paiement, et quoi virer d’ici quand | ✓ | ✗ | ✗ | ✗ | ✗ | en partie⁹ |
| Des paiements lus dans votre courrier, comptés une fois | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Des contrats : renouvellement, date limite de préavis, lettre de résiliation | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Lit les fichiers d’export de votre banque | ✓ | ✓¹⁰ | ✓ | ✓¹¹ | ✓ | ✗ |
| Se connecte à votre banque de lui-même | ✗¹² | ✓ | ✓ | ✓¹¹ | en partie¹³ | ✓ |
| Rapports et graphiques | en partie¹⁴ | ✓ | ✓ | ✓ | ✓ | en partie¹⁵ |
| Placements, plusieurs monnaies, comptabilité en partie double | ✗¹⁶ | en partie¹⁷ | en partie¹⁸ | ✓ | ✓ | en partie¹⁹ |

1. Selon le type de budget : un montant fixe par période disparaît à la suivante ; « add money … every month » le fait grandir.
2. Un budget fixe des montants prévus par compte et par période, face aux montants réels, dans des rapports.
3. Des budgets par catégorie de dépenses ; le report n’est pas décrit.
4. Des barres de progression en couleurs face à un objectif ; du rouge quand les dépenses dépassent ce qui est disponible.
5. Un rapport de budget met le prévu face au réel.
6. Une prévision du solde de fin de mois.
7. Une échéance dont la date est passée sans son paiement s’affiche « missed » (d’après son code source) ; les montants rapprochés « approximately », à 7,5 % près.
8. Les abonnements ont un montant minimum et maximum, et un encadré de la page d’accueil « will tell you how you’re doing ».
9. Une notification avant un découvert, et la prévision de fin de mois.
10. Sur le web et sur un iPad, pas sur les téléphones.
11. Par le Data Importer, installé à côté.
12. Par choix : vous téléchargez le fichier de votre banque, et Sioul le lit sur votre appareil.
13. Les banques allemandes par HBCI/FinTS, et les banques qui parlent OFX.
14. Le solde de chaque budget par jours, semaines, mois ou années.
15. L’historique des budgets, et ce mois-ci face au mois dernier, dans Bankin’ Plus.
16. Un montant dans une autre monnaie est marqué, pas converti.
17. Les comptes de placement gardés comme comptes de suivi ; une autre monnaie dans un budget à part.
18. Les rapports comprennent les comptes de placement.
19. Comptes d’épargne et de titres, dans Bankin’ Plus.

**Côté technique**

| | Sioul | YNAB | Actual | Firefly III | GnuCash | Bankin’ |
|---|---|---|---|---|---|---|
| Où vivent les données de votre argent | des fichiers lisibles dans votre dossier de notes | les serveurs de YNAB | votre appareil et votre serveur | votre serveur | un fichier sur votre ordinateur | les serveurs de Bankin’, en Europe |
| Demande un compte chez l’éditeur | non | oui | non | non | non | oui |
| Chiffré de bout en bout entre vos appareils | ✓¹ | ✗² | en partie³ | — | — | ✗ |
| Formats d’export lus | OFX, QFX, CAMT.053, CSV, PayPal, Stripe | CSV, OFX, QFX, QIF | CSV, QIF, OFX, QFX, CAMT | CSV, CAMT.053 | QIF, OFX, QFX, CSV, MT940, MT942, CAMT.052 et d’autres | — |
| Un fichier lu deux fois n’ajoute rien | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| Connexions bancaires par | aucune | Plaid, MX | Enable Banking, SimpleFIN, Pluggy.ai, Akahu ; GoCardless⁴ | GoCardless, Enable Banking, SimpleFIN, FinTS et d’autres | AqBanking (HBCI/FinTS), OFX | la sienne (Bridge, agréée par l’ACPR) |
| Logiciel libre | ✓ GPL-3.0+ | ✗ | ✓ MIT | ✓ AGPL-3.0 | ✓ GPL | ✗ |

1. XChaCha20-Poly1305, avec une clé tirée de votre phrase de passe par Argon2id ; une fois **Projets et argent** activé.
2. Chiffré au repos et pendant le transport sur les serveurs de YNAB ; le chiffrement de bout en bout n’est pas mentionné.
3. Facultatif ; les jetons de synchronisation bancaire ne sont pas couverts, et la copie sur votre appareil n’est pas chiffrée.
4. Indiqué comme « not accepting new accounts ».

D’autres en font plus par endroits : YNAB, Actual, Firefly III et Bankin’ se connectent d’eux-mêmes à la banque, et GnuCash le fait pour les banques allemandes ; GnuCash et Firefly III tiennent une comptabilité en partie double, des placements et plusieurs monnaies ; les quatre applications de budget font des rapports plus complets, et leurs applications pour téléphone ont plus d’usage derrière elles. Ce que Sioul ajoute, c’est la veille en phrases, le courrier lu comme des paiements et compté une fois, et vos contrats avec leurs dates de préavis, le tout sans connexion bancaire.

??? info "Sources"
    Toutes lues le 8 octobre 2026.

    - **YNAB**, son centre d’aide (glossaire, barres de progression, import de fichiers, import direct en Europe, opérations programmées, dépassements, et son index), sa page sur la sécurité et ses prix : <https://support.ynab.com/en_us/ynab-glossary-a-guide-BJd80SORq>, <https://support.ynab.com/en_us/progress-bars-a-guide-SkDEhot09>, <https://support.ynab.com/en_us/file-based-import-a-guide-Bkj4Sszyo>, <https://support.ynab.com/en_us/direct-import-in-europe-Syae1z_A9>, <https://support.ynab.com/en_us/scheduled-transactions-a-guide-BygrAIFA9>, <https://support.ynab.com/en_us/overspending-in-ynab-a-guide-ryWoxEyi>, <https://support.ynab.com/llms.txt>, <https://ynab.com/security>, <https://www.ynab.com/pricing>
    - **Actual Budget**, sa documentation sur les budgets, l’import, la synchronisation bancaire, les règles, les échéances, la synchronisation, l’installation et les rapports, et son dépôt (`schedules.ts` pour « missed ») : <https://actualbudget.org/docs/budgeting/>, <https://actualbudget.org/docs/transactions/importing>, <https://actualbudget.org/docs/advanced/bank-sync>, <https://actualbudget.org/docs/budgeting/rules/>, <https://actualbudget.org/docs/schedules>, <https://actualbudget.org/docs/getting-started/sync>, <https://actualbudget.org/docs/install/>, <https://actualbudget.org/docs/reports/>, <https://github.com/actualbudget/actual>
    - **Firefly III**, sa documentation sur les budgets, les abonnements, l’import de fichiers et les fournisseurs de données, et son dépôt : <https://docs.firefly-iii.org/how-to/firefly-iii/finances/budgets/>, <https://docs.firefly-iii.org/explanation/financial-concepts/budgets/>, <https://docs.firefly-iii.org/explanation/financial-concepts/subscriptions/>, <https://docs.firefly-iii.org/how-to/data-importer/import/file/>, <https://docs.firefly-iii.org/tutorials/data-importer/data-providers/>, <https://github.com/firefly-iii/firefly-iii>
    - **GnuCash**, sa page des fonctions, son manuel sur l’import et les préférences, et le chapitre de son guide sur les budgets : <https://www.gnucash.org/features.phtml>, <https://www.gnucash.org/docs/v5/C/gnucash-manual/trans-import.html>, <https://www.gnucash.org/docs/v5/C/gnucash-manual/set-prefs.html>, <https://code.gnucash.org/docs/C/gnucash-guide/chapter_budgets.html>
    - **Bankin’**, sa propre description dans l’App Store d’Apple (son centre d’aide n’a pas pu être lu), sa page Plus et Pro et sa page d’accueil : <https://apps.apple.com/fr/app/bankin-la-meilleure-app-pour-g%C3%A9rer-mon-argent/id447040033>, <https://bankin.com/fr/pluspro.html>, <https://bankin.com>

## Côté technique {#for-technical-readers}

**Les fichiers.** `sioul-budgets.toml` (budgets, paiements réguliers, lignes, réserves, couvertures, règles du courrier, comptes bancaires, leurs règles et vos choix au cas par cas), `sioul-bank.toml` (les mouvements de chaque compte par l’identifiant de la banque, son solde le plus récent), `sioul-contracts.toml` : du TOML à la racine de votre dossier de notes, écrit en gardant ses commentaires. Les messages dont vous avez dit qu’ils ne sont pas des paiements sont listés dans `~/.local/state/sioul/money.toml` sous Linux. Les montants sont gardés en centimes entiers, pour que les sommes ne dérivent jamais, et les montants écrits à la façon de chaque pays sont lus (« 1 234,56 € », « €1,234.56 », « (62.00) »).

**Les exports.** OFX 1.x (SGML) et 2.x (XML), par le `FITID` de chaque mouvement ; camt.053 de l’ISO 20022, des versions 001.02 à 001.08 (les écritures comptabilisées, le solde de clôture comptabilisé, les noms sous `Pty`) ; le CSV d’une banque, son en-tête trouvé par ses mots, avec `;` ou `,`, la virgule décimale, débit et crédit séparés ou non, et un solde écrit au-dessus du tableau ; l’activité de PayPal (nette de frais ; les lignes en attente, refusées et pour mémoire laissées de côté, ainsi que celles dans une autre monnaie que la plupart du fichier) ; l’historique du solde de Stripe. Un mouvement sans identifiant en reçoit un, fait de son jour, de son montant et de son libellé : un fichier lu deux fois n’ajoute rien.

**Le rythme.** Ce qui est prévu (paiements réguliers, lignes planifiées) compte en entier. Pour le reste, Sioul compare ce qui est entré ou sorti jusqu’ici à la part de la période écoulée : à dix jours sur trente, un tiers de ce qu’il faut encore à la période devrait être arrivé, ou pas plus d’un tiers de ce qu’elle peut dépenser dépensé. À moins de 5 % de ce que la période fait bouger (et jamais plus près que 10), le budget est dans les temps. La fourchette vient des douze dernières périodes qui ont eu des mouvements : du plus bas au plus haut à partir de trois, du 10e au 90e centile à partir de dix.

**La veille.** Un paiement attendu est cherché de trois jours avant sa date à sept jours après, par un mot de son nom, ou par son montant (à 1 % près, à trois jours près). « A pris un autre montant » veut dire plus de 5 % et 2 € d’écart. Le solde est projeté 31 jours en avant, les dépenses du quotidien réparties sur leurs jours. Un renflouement est arrondi à la dizaine au-dessus et demandé autant de jours à l’avance que sa réserve en met.

**Le courrier.** Seul le courrier qui a passé les vérifications d’authentification est lu pour y trouver des paiements ([Chaque message vérifié](privacy-security.md#every-message-checked)) : jamais le courrier mis de côté (falsifié, indésirable, un nom emprunté, un expéditeur bloqué), ni la file à examiner. Les paiements sont lus par des règles, en français et en anglais, dans l’objet et le début du texte ; un capital social dans des mentions légales n’est jamais pris pour un montant.

**Scellé entre vos appareils.** Avec **Projets et argent** activé, chaque changement des trois fichiers est scellé avec XChaCha20-Poly1305 (un nonce aléatoire de 192 bits chacun), lié à l’appareil qui l’a écrit, à sa place et à son heure, sous une clé tirée de votre phrase de passe par Argon2id (64 Mio, trois passes) et gardée dans le trousseau de chaque appareil ([Le partage](sharing.md#what-it-protects-and-what-it-cannot-hide)).

**L’agent d’IA.** Son outil `budgets` lit par la même vue que la fenêtre, n’écrit rien, et masque les IBAN (« [IBAN …0189] »).

**Les contrats.** Les préavis habituels proposés sont ceux de la France : trois mois pour un locataire (un mois en meublé ou en zone tendue) ; un mois pour les assurances et les complémentaires santé après leur première année (loi Hamon) ; dix jours au plus pour le téléphone et internet après douze mois ; aucun pour l’énergie (art. L224-13 du code de la consommation). En ligne, un bouton de résiliation est obligatoire depuis le 1er juin 2023 (loi 2022-1158).
