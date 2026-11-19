---
description: Budgets, réserves et comptes bancaires dans Sioul – chaque compte jusqu’à sa prochaine entrée et chaque budget en mots, jamais en rouge ; les paiements lus dans les courriels, les fichiers d’export de votre banque lus sur votre appareil, ce qui arrive semaine par semaine, et vos contrats avec leurs dates de préavis.
---

# Les budgets et les comptes bancaires {#budgets-and-bank-accounts}

## En bref {#in-short}

L’argent va et vient par courriel : factures, reçus, avis de paiement, versements, avis d’impôt. La page Budgets vous dit où en est votre argent, en phrases, et sépare trois choses :

- les **comptes bancaires**, là où est l’argent : un compte courant, PayPal, Stripe ;
- les **réserves**, là où il est mis de côté : un Livret A, une assurance vie ;
- les **budgets**, ce à quoi il sert : vos charges (loyer, énergie, courses), vos loisirs, votre travail, un projet.

Sioul lit les fichiers d’export de votre banque et les courriels qui parlent de paiements, les compare aux paiements que vous attendez, et commence par chaque compte jusqu’à sa prochaine entrée : s’il couvre chaque paiement, ce qui reste à vivre, son point le plus bas. Il dit calmement quand un paiement n’est pas sorti, ou quand un compte risque de ne pas tenir le suivant, assez tôt pour virer de l’argent. Il ne se connecte jamais à votre banque, ne montre jamais de rouge, et ne montre jamais d’offres.

Chaque paiement garde ce que le payeur a payé, les frais pris en chemin et ce qui est arrivé sur votre compte. La page a cinq onglets : **Aperçu**, sur lequel elle s’ouvre toujours et qui suit vos heures ; **Budgets**, chaque budget et chaque compte ; **Mouvements**, chaque mouvement, du plus récent au plus ancien ; **Courriels**, chaque courriel qui parle d’argent que vous avez gardé ; **Factures**, chaque facture et chaque brouillon que vous avez faits ([Le temps et les factures](time.md#invoices)). Sur un téléphone, faites glisser la ligne des onglets de côté pour atteindre les derniers.

<figure markdown="span">
  [![La page Budgets, « Budgets, novembre 2026 », avec son filtre par domaine et Nouveau budget : le compte principal en phrases (jusqu’à la prochaine entrée, s’il couvre chaque paiement, le reste à vivre et par jour, le point le plus bas), son solde à venir en marches d’escalier, puis chaque budget en mots : ce qui est payé, ce qui est encore à venir, ce qui reste pour les courses, comment son mois devrait finir.](../assets/screens/fr/budgets.png){ loading=lazy }](../assets/screens/fr/budgets.png "Ouvrir l’image en grand")
  <figcaption>Chaque compte jusqu’à sa prochaine entrée, chaque budget en mots.</figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

- **Sioul ne se connecte jamais à votre banque** et ne garde aucun mot de passe bancaire : vous téléchargez le fichier de votre banque, et Sioul le lit sur votre appareil.
- **Vos budgets, les mouvements de votre banque et vos contrats sont des fichiers lisibles** sur votre appareil, dans le propre dossier de Sioul ; tant que vous n’y avez pas déplacé les listes de Sioul, ils sont dans votre dossier de notes ([Où Sioul garde ses listes](sharing.md#where-sioul-keeps-its-lists)). Il n’y a pas de serveur de Sioul.
- **Entre vos appareils**, ils voyagent scellés par le partage de Sioul, dans sa partie **Projets et argent** : le dossier et son serveur n’en peuvent rien lire. Avant le déplacement, la synchronisation de votre dossier de notes peut les transporter à sa place ([Le partage](sharing.md)).
- **Un courriel n’est lu comme un paiement qu’après avoir passé les vérifications de Sioul** : les courriels falsifiés, les indésirables, un expéditeur qui emprunte le nom d’une banque ou d’une boutique, et les expéditeurs bloqués ne sont jamais lus comme des paiements. Un paiement venu de quelqu’un de nouveau et non vérifié demande de vérifier d’abord : un faux « votre paiement » est une ruse courante.
- **Un agent d’IA**, seulement si vous en connectez un, lit un résumé de vos budgets, ne déplace jamais d’argent, et voit les numéros de compte masqués.
- **Pas d’offres**, pas de comparatifs, pas de « meilleures affaires », pas de publicité, pas de mesure d’audience.

## Les budgets {#budgets}

**Nouveau budget** :

- **Nom**.
- **Compté par** : le mois ou l’année.
- **Sa période commence** : le 1er, ou avec une entrée, votre salaire ou une prestation mensuelle : d’une paie à la suivante, comme vous la vivez.
- **Solde à atteindre** à la fin de chaque période : 0 pour l’équilibre ; au-dessus, c’est de l’épargne.
- **Pour** : le travail, vos démarches, les loisirs. Cela décide des heures où le budget est en vue ([Les heures](hours.md)). Rien de coché : vos démarches.
- **Mettre de côté pour les impôts**, pour le travail : une part de chaque entrée, en pour cent, mise de côté dès son arrivée pour la déclaration, **chaque mois** ou **chaque trimestre**. Rien tant que vous ne la fixez pas. Payée avant le jour de la déclaration, **Le même que…** sur sa ligne la joint à la ligne de la banque qui l’a payée : comptée une fois, dite payée.

Ce qu’un budget n’a pas dépensé est reporté, comme une enveloppe garde ce qui reste : l’argent mis de côté en octobre pour une facture de décembre est encore là en décembre.

### D’un regard {#at-a-glance}

La page commence par chaque compte, en quelques phrases, jusqu’à sa prochaine entrée : « Jusqu’à l’arrivée de Salaire le samedi 28 novembre, le compte couvre chaque paiement. Reste à vivre : 460 €, environ 25 € par jour. Le point le plus bas arrive le jeudi 26 novembre, après Loyer. » Le paiement d’un client qui peut venir plus tard est dit avec son doute (« Si Client A arrive d’ici le vendredi 20 novembre, … »), et ce que ferait le compte sans lui aussi, quand cela change la réponse. Sans entrée attendue : « À votre rythme habituel, le compte et Livret A tiennent environ 24 semaines. » Un seul graphique montre le solde à venir jusqu’à la prochaine entrée, le zéro en tirets ; les phrases n’en ont jamais besoin. Sioul ne se connecte pas à votre banque : il lit les exports que vous prenez.

C’est l’**Aperçu**, l’onglet sur lequel la page s’ouvre toujours, et le seul qui suit vos heures. Chaque compte dit quand le fichier de sa banque a été lu pour la dernière fois : « Dernière mise à jour bancaire : 3 novembre. » ; quand cela date de plus longtemps que vous n’en prenez d’habitude (une semaine, tant que Sioul ne connaît pas votre rythme), il ajoute, une fois : « Des mouvements depuis peuvent manquer. » Le reste à vivre est ce qui reste après les paiements que Sioul connaît, jusqu’à votre prochaine entrée, d’après les mouvements pris jusqu’ici : jamais une promesse, ni une allocation. Le graphique nomme toujours son compte, « Ma banque, jusqu’au 28 novembre », et le nom du compte au-dessus en choisit un autre. Un budget de travail qui déclare dit son chiffre d’affaires à côté de ce qu’il met de côté : « Chiffre d’affaires encaissé ce trimestre : 6 940 €, à déclarer avant le 31 janvier. »

Chaque budget dit ensuite ce qui reste, en mots : « Payé : Loyer, Énergie. » « Encore à venir : Téléphone, 19,99 €, jeudi 12 novembre. » « Courses : reste 180 € jusqu’au 27 novembre. » Puis une phrase sur la fin de sa période, avec l’épargne qui couvre un manque : « D’ici le 30 novembre, il devrait finir comme prévu. » Ni pourcentage, ni couleur, ni barre.

**Par domaine**, comme dans le Courrier : le filtre en haut montre ce que disent vos heures (les budgets du travail pendant vos heures de travail, ceux des démarches pendant vos heures de démarches, ceux des loisirs pendant votre temps libre, aux repas et la nuit), et vous pouvez en cocher d’autres un moment ; **Selon vos heures** les remet. Ce qu’il laisse de côté est dit en une ligne : « Les budgets de travail attendent vos heures de travail. » Un compte qui ne tiendrait pas un paiement s’affiche toujours.

### Ce qui arrive {#what-is-coming}

Chaque paiement et chaque entrée attendus sur les six semaines qui viennent, ou jusqu’à l’entrée d’après, semaine par semaine : son jour, qui, le montant ou « environ », son budget, où il en est (« prévu le… », « annoncé pour le… », « pas encore définitif », « s’il arrive d’ici le… »), et ce qui le dit, ensemble : la facture du courriel, le courrier, le contrat, la ligne de la banque. **En mois** pose les mêmes sur un calendrier, à côté de la liste, sur un ordinateur. Les dépenses inhabituelles de l’année passée (une réparation, un voyage) sont comptées dans la prévision comme une ligne à part, environ tant par mois.

Un appui long sur un écran tactile, un clic droit ou **⋯** à la souris : **Où il va…**, **Répartir…** entre deux budgets, **Pas le même paiement** à côté de celui que Sioul croit sans doute le même, **Le même que…**, et ce qui le dit, ouvert.

### Un budget, ouvert {#a-budget-open}

Un budget s’ouvre sur ses mots et la fin de sa période, puis sur ses mouvements, du plus récent au plus ancien, et sur son solde par **Jours**, **Semaines**, **Mois** ou **Années**, ce qui est prévu en pointillés.

**Ajouter un mouvement** : **Une fois**, ou **Régulier** (chaque mois ou chaque année, à un jour donné). Un montant est négatif pour une sortie (`-650`) et positif pour une entrée (`1200`). Pour ce qui se dépense petit à petit, comme les courses, voir [Les dépenses du quotidien](#daily-spending).

Un clic droit sur une ligne : **Modifier cette ligne…**, **Supprimer cette ligne** (avec une confirmation). Si la ligne a changé dans le fichier entre-temps (sur un autre appareil, ou à la main), rien n’est changé, et Sioul le dit : regardez-la de nouveau.

Un budget peut être **Pour** une personne de vos proches (l’argent de poche d’un enfant, un compte que vous tenez pour un parent que vous aidez) : choisi dans son formulaire, il se montre sur sa page ([Vos proches](contacts.md#household)).

**Retirer ce budget** garde ses lignes dans le fichier, telles qu’elles ont été écrites ; elles ne comptent plus.

## Les réserves {#reserves}

**Nouvelle réserve** : son **Nom**, son **Solde** à une date, **Jamais sous** : le montant sous lequel elle ne doit jamais descendre, et **Jours pour arriver** : le temps que met l’argent demandé pour arriver sur votre compte (0 pour un Livret A, dont l’argent arrive tout de suite, une dizaine pour une assurance vie).

Chaque réserve montre son solde aujourd’hui, ce qui a bougé ce mois-ci, son solde à la fin de l’année, combien de temps elle tient à ce rythme au-dessus de son plancher, et combien de semaines de vos propres dépenses elle couvre.

Quelle réserve couvre quel budget s’écrit dans le fichier des budgets : à la fin de chaque période, ce qui manque à un budget est pris sur sa réserve.

## Les mouvements {#movements}

**Mouvements** liste chaque mouvement jusqu’à aujourd’hui, du plus récent au plus ancien, groupé par jour (ce qui est encore à venir est dans **Ce qui arrive** de l’Aperçu, ou sous **Filtres** avec un jour plus tard) : la recherche en haut (qui, votre note, le budget, le montant), puis chaque ligne avec son jour, qui, ce qui a bougé avec son signe, son budget et où il en est, en mots. **Filtres**, repliés, le restreignent par compte, budget, jours, entrées ou sorties, montant, ce à quoi il sert, et ce qui en a parlé (la banque, un courriel, un paiement régulier, votre propre ligne) ; ceux qui servent s’affichent en mots sous la recherche, chacun avec sa ×. Comme le Courrier, il commence par les domaines que disent vos heures, et une recherche trouve tout, à toute heure. Pas de total courant ni de compteur ; une somme ne s’affiche que quand les filtres posent une question qui en a une : « Entrées : 2 340 € payés, 2 281 € arrivés. »

Une ligne ouverte montre tout : ce qui a été payé, chaque frais et où il a été lu, le jour du paiement, ce qui en a parlé (chacun s’ouvre), ce à quoi il est lié, et « Signalé le 5 novembre ; modifié le 7 novembre ». Son menu (un appui long, un clic droit, **⋯**) : **Modifier…**, **Où il va…**, **Partager…**, **Le même que…**, **Pas le même**, **Lier à…**, et son retrait. **Nouveau mouvement** est en haut : qui, ce qui est arrivé ou parti, le budget, le jour, et s’il est encore à venir ; sous **Ce qui a été payé, les frais, une note…**, ce que le payeur a payé ou le prix, ses frais, le compte, le jour du paiement quand l’argent est arrivé plus tard sur le compte, et une note. **Lier à…**, sur sa ligne, le lie ensuite à un contact, une tâche ou un projet.

Ce qui peut changer dépend de ce dont le mouvement est fait, et le formulaire le dit :

- **votre propre ligne** : tout ; **Supprimer**, demandé une fois ;
- **un paiement régulier** : le montant et le jour de cette fois-ci, son budget, une note ; **Pas cette fois** pour un mois ;
- **la ligne de la banque** : son budget, ce qui a été payé et ses frais, le jour du paiement, une note, qui ; jamais son montant ni son jour, qui sont ceux de la banque ; **Laisser de côté** pour une ligne d’un mauvais fichier ;
- **une facture que vous avez envoyée**, ou **une part mise de côté pour les impôts** : changées là où elles sont faites, la facture ou le budget.

Chaque changement dit son effet en une ligne, « Maison a 12 € de plus pour novembre 2026. », avec **Annuler**.

### Ce qui a été payé, les frais et ce qui est arrivé {#fees}

Un paiement par PayPal, Stripe, SumUp ou une plateforme comme Malt vous arrive moins des frais, souvent plus tard et avec d’autres. Sioul garde les trois montants, et dit les frais dans la ligne de leur paiement : « Client A · payé 520 € · frais 15,43 € ». Il lit chaque frais dans le relevé le plus exact qui soit : ce que vous avez écrit, l’export du prestataire, une facture de frais, la ligne de frais du courriel, ou la différence d’un versement. Un tarif publié (PayPal France : 2,90 % plus 0,35 €) ne sert, dit « environ », que pour ce qui doit encore venir. Quand les chiffres ne tombent pas juste, la ligne garde ce qui est arrivé et dit que les frais ne sont pas encore connus : Sioul n’en invente jamais.

### Les versements {#payouts}

Avec l’export de PayPal ou de Stripe pris, un versement est un virement entre vos comptes, qui dit les paiements qu’il regroupe. Sans lui, la ligne de la banque au nom du prestataire (« VIR PAYPAL… ») regroupe les paiements dont ses courriels ont parlé depuis le précédent, les plus anciens d’abord, autant que son montant le permet : « regroupe 3 paiements », la différence étant leurs frais. Un paiement pas encore sur votre banque attend le versement suivant. Si Sioul les a mal appariés, **Pas un versement** le défait.

## Les courriels qui parlent d’argent {#mail-about-money}

**Courriels** garde chaque courriel qui parle d’argent que vous avez gardé, sur chaque adresse, les envoyés compris : les factures reçues et envoyées, les reçus, les avis de paiement et de versement, les factures à payer, les avis d’impôt, les factures de frais des plateformes. Chaque courriel est lu pour l’argent une fois, pour toujours, sur votre appareil, et seulement quand son expéditeur, son objet ou sa pièce jointe rend l’argent possible. Sioul lit, en français et en anglais, ce que dit un courriel : un paiement reçu ou fait, une commande, une facture, un remboursement ; le montant, et les frais quand il les donne ; qui a payé, ou qui a été payé.

Du plus récent au plus ancien, la recherche en haut et les filtres repliés, comme dans Mouvements, chaque ligne dit ce qu’il est devenu : *le même paiement que* un mouvement déjà compté (une facture et son prélèvement, un reçu et son paiement par carte, que le courriel rejoint de lui-même, ou *sans doute* le même quand Sioul n’en est pas sûr) ; *fait partie* d’un versement ; *compté* dans un budget ; *pas compté*, comme vous l’avez dit ; ou *lié à aucun mouvement*, avec **Ajouter** et **Pas un paiement**. Lié ou non est un filtre à part, jamais celui par défaut, jamais compté. **Pas un paiement** cesse aussi de compter les courriels du même type de cet expéditeur (son objet, ses nombres mis à part), dit une fois sur sa ligne avec **Le compter à nouveau**. Quand le courriel d’un paiement nomme l’un de vos contacts, il lui est lié. Une ligne venue d’un expéditeur nouveau et non vérifié demande de vérifier d’abord que le courriel est authentique, et ne rejoint rien d’elle-même. Les courriels falsifiés, les indésirables et les expéditeurs bloqués ne sont jamais lus comme des paiements. Un montant dans une autre monnaie est marqué « à convertir » : Sioul ne le convertit pas encore.

Des règles dans le fichier des budgets peuvent envoyer d’elles-mêmes vers un budget les paiements d’un expéditeur, ou ceux qui portent un mot.

**Un paiement…**, dans le menu d’un courriel, dans la liste du Courrier et sur la carte d’un courrier papier, fait de n’importe quel courriel ou courrier un paiement, quoi que Sioul y ait lu : un court formulaire rempli de ce qu’il dit (qui, le montant, le jour), n’importe quel budget. Le courriel ou le courrier en reste la source.

## La déclaration {#the-declaration}

**Pour la déclaration**, dans Mouvements ou dans Factures, montre vos livres pour la période d’une déclaration, dès qu’un budget de travail met de côté pour les impôts (ou dit à quel rythme il déclare) ou dès que vous avez émis une facture : toutes vos factures et chaque budget de travail qui déclare, ensemble. Sur le Porche, **Ouvrir la déclaration** sur la ligne d’une déclaration l’ouvre à cette période. Les flèches mènent à la période d’avant et à la période d’après ; **Retour** revient aux mouvements, ou à Factures quand vous en venez. De haut en bas :

- **Quoi déclarer, et pour quand** : le chiffre d’affaires encaissé dans le trimestre ou le mois, hors TVA, pour chacune de vos activités (chaque ligne d’une facture dans l’activité dont elle relève : un repas servi ou une nuit d’hébergement est une vente pour l’URSSAF, même s’il est un service pour la TVA), avec les cotisations estimées aux taux de l’URSSAF du jour où chaque paiement est arrivé, le versement libératoire si vous l’avez choisi, et la contribution à la formation professionnelle (CFP) : « Du 1er octobre 2026 au 31 décembre 2026 : 6 940 € encaissés hors TVA, à déclarer au plus tard le 1er février 2027. » Une période où rien n’a été encaissé se déclare aussi : « Rien d’encaissé du 1er juillet 2026 au 30 septembre 2026 : déclarez 0 € au plus tard le 2 novembre 2026. » Un paiement dans une autre devise attend ce qu’il a donné en euros, et le dit, jamais « 0 € » : « 1 000 $ US reçus : dites ce que cela a donné en euros. » Une période où vous avez rendu plus que vous n’avez encaissé déclare 0 €, et dit que ce qui est reparti au-delà n’est pas déduit : Sioul ne reporte rien sur une autre période. Un jour qui tombe un week-end passe au lundi, comme l’URSSAF le décale ; les jours fériés ne sont pas comptés, si bien que le jour dit peut venir un jour ou deux avant celui de l’URSSAF, jamais après. Une fois dit le jour où votre activité a commencé, votre première déclaration regroupe vos premiers mois, comme l’URSSAF le demande : en déclarant chaque mois, le mois où vous avez commencé et les trois suivants, au plus tard à la fin du quatrième mois après lui (un début en mars : au plus tard le 31 juillet) ; chaque trimestre, le trimestre où vous avez commencé et le suivant (un début en avril : au plus tard le 31 octobre) ([Paramètres ▸ Factures](settings.md#invoices)). Rien n’est dit des périodes d’avant le début de vos livres : le jour où vous avez commencé, sinon votre première facture ou la première recette de votre budget de travail. Les cotisations sont une estimation : l’URSSAF les calcule quand vous déclarez, sur son propre site.
- **L’argent rendu** : un remboursement sur un avoir compte avec l’argent qu’il rend, le dernier paiement de la facture d’abord : un remboursement de 1 000 € sur une facture payée 900 € en septembre et 100 € en octobre rend d’abord les 100 € d’octobre, puis 900 € de ceux de septembre. Chaque part compte avec son propre paiement. Rendu dans la même période que cet argent, il baisse cette période. Rendu plus tard, il baisse la période où l’argent est arrivé, tant que cette période n’est pas déclarée : pas dite **Déclarée**, et son jour pour déclarer pas passé. Après, il ne baisse aucune période, et la déclaration de sa propre période dit de corriger celle que vous avez faite : « La fiche de l’Urssaf dit de corriger la déclaration déjà faite pour cet argent (Troisième trimestre 2026) : 300 € hors TVA rendus le 8 octobre 2026, comptés dans aucune période ici. » Cela suit la fiche de l’URSSAF sur le chiffre d’affaires telle qu’un résumé de recherche l’a donnée : vérifiez-la auprès de l’URSSAF avant de vous y fier ([Pourquoi cela marche ainsi](#why-it-works-this-way)).
- **Quand vous êtes redevable de la TVA** : la TVA collectée, mois par mois (ou trimestre par trimestre, si vous l’avez réglé ainsi) et taux par taux : sur les paiements reçus pour des services, au jour de la facture pour des biens, et pour des services aussi quand vous avez opté pour les débits. « Sioul prépare votre déclaration de TVA ; il ne dépose rien. »
- **Les services à des entreprises d’autres pays de l’UE** : la déclaration de chaque mois (DES), avec les factures qu’elle couvre et son jour ([Le Porche](porch.md#the-sections)).
- **Les totaux du trimestre et de l’année**, jusqu’au jour montré tant que la période court (« Quatrième trimestre 2026, jusqu’au 31 octobre 2026 : 2 080 € encaissés. »), et cette année, **où en est chaque seuil** : « 30 100 € comptés cette année pour le seuil de la franchise en base de TVA pour les services, 37 500 €. » L’approcher et le dépasser se disent chacun une fois ; ce qui vous demande de faire quelque chose (la TVA dès aujourd’hui ou à partir du 1er janvier, la fin du régime micro) reste jusqu’à ce que ce soit fait ([Ce qui vous joint](notifications.md#sioul-s-own)).
- **Déclarée** : une fois la période d’une déclaration finie, et celle de chaque DES aussi, la vue montre **Déclarée**. Appuyez dessus quand vous avez déclaré sur le site de l’URSSAF (ou des douanes pour la DES) : la ligne d’état dit « Notée comme déclarée. », avec **Annuler**, et la ligne se lit alors « Déclaration : Quatrième trimestre 2026, déclarée le 3 février 2027 », avec **Pas déclarée** si vous avez appuyé trop tôt, qui a **Annuler** aussi. C’est votre parole seulement : rien n’est envoyé nulle part. Dite sur un appareil, elle vaut sur tous, et la déclaration quitte le Porche ; quand deux appareils disent autre chose, la dernière dite compte : une parole dite sur un appareil qui a déjà reçu celle de l’autre est la plus récente, quoi que disent leurs horloges, et deux appareils qui n’ont pas encore échangé suivent leurs horloges.
- **Votre livre des recettes**, en dessous : chaque paiement reçu, à son jour : le client (celui de la facture, pas les mots de votre banque), ce que vous avez vendu (des biens, des services ou les deux), la facture, le moyen de paiement (les espèces dites à part ; une recette qu’aucune facture ne décrit est en espèces quand ses mots le disent, « espèces » ou « cash ») et, quand vous êtes redevable de la TVA, le montant hors TVA. Un paiement qu’aucune facture ne décrit vient toujours de votre budget de travail, à ce que le client a payé, au jour où il a été payé, même s’il est arrivé plus tard sur votre banque. Chaque euro reçu par votre banque compte une fois, que votre banque ait montré un paiement, que vous ayez marqué la facture **Payée**, ou les deux ; ce qu’une ligne de banque a apporté au-delà des factures qu’elle a payées (un client qui a trop payé, un versement d’un prestataire de paiement qui porte d’autres paiements) est une recette à elle, « au-delà de la facture 2026-031 ». Un remboursement versé sur un avoir est une ligne à lui, en dessous de zéro, au jour où l’argent est reparti ; les déclarations le comptent comme le dit **L’argent rendu**, plus haut.

**Enregistrer le livre des recettes…** écrit le livre en fichier de tableur (CSV : une colonne pour les espèces, les biens et les services à part quand vous vendez les deux, le total de chaque trimestre après ses jours et celui de l’année en dernier) et en PDF. Un paiement dont le montant payé n’est pas encore connu le dit, et les totaux se lisent alors « au moins » ; un paiement dans une autre devise compte une fois que vous avez dit ce qui vous est parvenu en euros, la ligne de ces euros sur votre banque étant alors ce même paiement, jamais compté de nouveau ; tant que vous ne l’avez pas dit, la déclaration, le Porche et l’Aperçu disent ce qui attend ([Payée, et les avoirs](time.md#paid-and-credit-notes)). Chaque déclaration attend aussi sur le Porche à partir du jour où vous pouvez la faire, jusqu’à ce que vous la disiez **Déclarée** ou l’écartiez ([Le Porche](porch.md#the-sections)). La phrase de l’Aperçu, à côté d’un budget de travail qui déclare (le premier, quand plusieurs déclarent), dit la même chose, d’après vos livres : « Chiffre d’affaires encaissé ce trimestre, hors TVA : 6 940 €, à déclarer au plus tard le 1er février 2027. »

## Les comptes bancaires {#bank-accounts}

<figure markdown="span">
  [![Les comptes bancaires sur la page Budgets : deux comptes, chacun sur une carte avec son solde d’après le fichier de la banque, les budgets qu’il remplit, les réserves qui le renflouent, ses derniers mouvements, chacun disant où il est allé et pourquoi, avec ⋯ à côté, et Prendre un export…, Règles… et Modifier ; dessous, un paiement qui a pris un autre montant que prévu.](../assets/screens/fr/bank-accounts.png){ loading=lazy }](../assets/screens/fr/bank-accounts.png "Ouvrir l’image en grand")
  <figcaption>Chaque compte, les budgets qu’il remplit, et où est allé chaque mouvement.</figcaption>
</figure>

La carte d’un prestataire de paiement dit comment ce qu’il reçoit arrive à votre banque : « Ce que vous recevez attend sur PayPal jusqu’à ce que vous le versiez sur Ma banque. »

**Nouveau compte bancaire** : son **Nom**, son **Type** (une banque, PayPal, Stripe, SumUp, Wise, Revolut, un autre prestataire ou une autre plateforme que Sioul connaît, autre chose), **Remplit** : les budgets qu’il remplit, **Gardé au-dessus de** : le montant sous lequel il ne doit pas descendre, et **Renfloué par** : les réserves qui le renflouent, dans votre ordre.

**Prendre un export…**, sur sa carte : le fichier de relevé que donne votre banque (OFX ou QFX, CAMT.053, ou son CSV), le téléchargement d’activité de PayPal, l’historique du solde de Stripe, ou le relevé de Revolut ou de Wise ; ce que chaque ligne dit que le payeur a payé, et les frais pris, est lu aussi. Lire deux fois le même fichier n’ajoute rien. Rien n’est demandé à la banque elle-même : vous téléchargez le fichier, Sioul le lit. Un paiement que le fichier dit pas encore définitif (une empreinte de carte) attend à part que la ligne de la banque arrive. Chaque mouvement va ensuite dans un budget selon votre choix, vos règles ou le paiement qu’il représente, et dit pourquoi ([Où va chaque mouvement](#where-each-movement-goes)).

**Où il va**, sur la ligne d’un mouvement, propose tous les budgets, quelle que soit l’heure. Déplacé, le mouvement dit ce que cela change (« Maison a 12 € de plus pour octobre 2026. ») et demande une fois si les prochains paiements de ce bénéficiaire y vont aussi ; **Oui, désormais** écrit une règle, que vous retrouvez dans **Règles…**. **⋯** à côté le répartit entre deux budgets, dit qu’il n’est **Pas le même paiement** que celui auquel Sioul l’a joint, ou qu’il est **Le même que…** un autre ; chaque ligne dit ce qui le dit aussi.

### Renfloué à temps {#topped-up-in-time}

Le solde de chaque compte est projeté jusqu’à sa prochaine entrée. Au premier paiement qu’il ne tiendrait pas au-dessus de son plancher, Sioul dit quoi verser, depuis quelle réserve, et d’ici quand, en comptant les jours que met cette réserve : « Assurance vie : demandez 590 € d’ici le jeudi 15 octobre ; il faut environ 10 jours, et le loyer part le 1er novembre. » Quand demander aujourd’hui est déjà trop tard, il le dit.

## Ce que les mouvements ont trouvé {#the-bank-watch}

Un prélèvement qui s’arrête ne le dit pas. Sioul compare les mouvements de votre banque aux paiements que vous attendez, et dit, en phrases, dans l’onglet **Budgets** :

- **ce qui n’est pas sorti** : « Téléphone (19,99 €) n’est pas sorti du compte. Attendu vers le 12 septembre, absent des mouvements de votre banque. » ;
- **ce qui a changé** : « Électricité a pris 71,40 € le 5 septembre, là où 62 € était attendu. » ;
- **ce que le compte risque de ne pas tenir** : un compte qui ne tiendrait pas un paiement avant sa prochaine entrée, avec ce qui le couvre ([Renfloué à temps](#topped-up-in-time)), dit quelle que soit l’heure.

**Des rappels**, une fois chacun, au début du travail : deux jours ouvrés avant (**Paiements prévus : jours travaillés avant**, dans le ⚙ de la page, en haut à droite ; 0 pour aucun) un paiement que vous faites vous-même (une facture prévue, un impôt, la facture d’un courrier, un paiement régulier coché **Vous le payez vous-même**) ou une facture que vous envoyez (**Vous envoyez sa facture**), en disant si le compte le tiendra ; une semaine après un paiement qui n’est pas sorti ; cinq jours ouvrés avant un jour où le compte ne tiendrait pas un paiement, un prélèvement aussi. Un prélèvement n’a pas de rappel sinon. Ils ne viennent que d’un ordinateur, quand Sioul est ouvert ou, si vous le demandez, fenêtre fermée (Paramètres ▸ [Rappels](settings.md#reminders) ; pas encore sous Windows). Un téléphone ne les dit pas ; une tablette, si, comme un ordinateur.

## Contrats et abonnements {#contracts-and-subscriptions}

Ce qui vous engage : loyer, énergie, téléphone et internet, assurances, complémentaire santé, abonnements, hébergement, banque. Les abonnements oubliés font partie de ce que les personnes avec un TDAH appellent la « taxe TDAH » ; avant une démarche en justice, il importe de savoir ce que couvre une assurance.

**Ajouter un contrat** : avec qui, votre numéro chez eux, le paiement régulier qui le paie, depuis quand, quand il se renouvelle (chaque mois, chaque année, ou pas de renouvellement), le préavis qu’il demande, comment l’arrêter (sa page de résiliation, ou où écrire), et ce qu’il couvre.

- **Où il en est**, en une phrase : « Se renouvelle le mardi 1er décembre ; pour l’arrêter, le préavis doit partir au plus tard le 2 octobre. »
- **Le préavis habituel** pour son type est proposé, avec la règle d’où il vient (en France). Le vôtre est celui que dit votre contrat.
- **Un rappel**, une fois, deux semaines avant le dernier jour où un préavis peut partir, pour les contrats qui se renouvellent chaque année. Comme les rappels sur l’argent, il ne vient que d’un ordinateur.
- **L’arrêter…** : **Ouvrir sa page de résiliation**, ou **Écrire la lettre** : un brouillon de lettre de résiliation, à son adresse, avec votre numéro, qui demande une confirmation écrite. Vous la relisez, la signez et l’envoyez. **Il est fini** garde le contrat, estompé, comme trace.

Les paiements réguliers sans contrat noté sont listés dessous, chacun avec **Le noter**. Un courriel peut en devenir un avec **Garder comme contrat…**, dans son menu.

Sioul ne montre jamais d’offres, de comparatifs ni de « meilleures affaires » : seulement des faits sur vos contrats.

## Où tout est gardé {#where-it-is-kept}

Dans des fichiers texte simples, parmi les propres listes de Sioul, dans `records/` du dossier de données de Sioul (`~/.local/share/sioul/records/` sous Linux) : `sioul-budgets.toml` (budgets, réserves, lignes, règles, comptes bancaires), `sioul-bank.toml` (les mouvements de la banque), `sioul-movements.toml` (ce que Sioul a appris de vos paiements : lesquels sont les mêmes, les empreintes pas encore définitives, vos réponses), `sioul-contracts.toml`. Lisibles, modifiables à la main, versionnés avec git si vous voulez. Tant que vous n’avez pas sorti les listes de Sioul de votre dossier de notes, ils sont à sa racine ([Où Sioul garde ses listes](sharing.md#where-sioul-keeps-its-lists)). Ils voyagent scellés par le partage de Sioul, dans sa partie **Projets et argent** (avant le déplacement, la synchronisation de votre dossier de notes peut les transporter à sa place : [Le partage](sharing.md)) ; rien n’est envoyé nulle part ailleurs.

Les budgets s’affichent par domaine selon vos heures, comme dans le Courrier ([D’un regard](#at-a-glance)) ; tous les budgets restent proposés pour un paiement, à toute heure. Voir [Les heures](hours.md).

## Pour aller plus loin {#going-further}

### Où va chaque mouvement {#where-each-movement-goes}

Dans cet ordre :

1. **Votre choix** : **Où il va**, sur la ligne d’un mouvement, le place dans un budget, ou dans aucun.
2. **Le même paiement qu’une ligne déjà comptée**, venue d’un courriel ou ajoutée à la main : compté une fois.
3. **Un virement entre vos propres comptes** : dans aucun budget, puisque l’autre côté est lu aussi.
4. **La première de vos règles qui tient** (**Règles…**, sur la carte du compte) : des mots du libellé, une entrée ou une sortie, et la destination : un budget, le paiement régulier qu’il représente, une réserve, un autre compte. Une règle sans mots prend tous les mouvements de son sens.
5. **Le paiement régulier qu’il représente**, par son nom ou son montant, près de son jour : le vrai montant remplace celui qui était prévu.
6. **Le premier budget du compte**.

Chaque mouvement dit pourquoi il est allé là.

Les budgets comptent chaque paiement une fois. Sioul reconnaît quel paiement est une ligne de la banque, une ligne déjà comptée, un paiement régulier, la facture d’un courrier, une facture que vous avez envoyée, d’abord par ce qu’ils partagent (le mandat d’un prélèvement, le numéro d’une facture ou d’une commande), puis par le montant, le jour et le nom du bénéficiaire, chaque type avec sa marge : un prélèvement autour de son jour, un paiement par carte à quelques pour cent près pour un pourboire, le virement d’un client des semaines après la facture, un remboursement jusqu’à huit semaines plus tard. Ce qui n’est que sans doute le même, il le dit. Deux factures réglées par un seul virement, une commande débitée en plusieurs fois, la somme mensuelle d’une carte à débit différé et un achat en plusieurs fois sont reconnus ensemble, pour que rien ne compte deux fois.

### Les dépenses du quotidien {#daily-spending}

**Une estimation (courses, liquide), pas un montant fixe**, sur une ligne régulière, dit que la somme se dépense petit à petit : un paiement par carte qui ne représente rien d’autre est pris dessus, l’estimation à qui il reste le plus d’abord, et seule la dépense au-delà compte en plus. Les vraies dépenses sont prises sur l’estimation, jamais ajoutées par-dessus.

### Télécharger le fichier de votre banque {#downloading-your-banks-file}

Le site de votre banque peut être gardé dans [Sites](sites.md) : Sioul remplit votre identifiant depuis Bitwarden, et votre clé de sécurité répond là où votre banque en accepte une. Le fichier que vous y téléchargez n’est alors qu’à un **Prendre un export…** de distance.

### Dans le terminal {#in-the-terminal}

`sioul budgets` affiche ce que dit la page, tous domaines à la fois : chaque compte jusqu’à sa prochaine entrée, chaque budget en mots, ce qui arrive semaine par semaine, les réserves, et les paiements que proposent les courriels. `sioul movements` liste chaque mouvement avec les mêmes filtres (`--from`, `--to`, `--account`, `--budget`, `--in`, `--out`, `--min`, `--max`, `--area`, `--search`, `--json`) ; `sioul movement show`, `add`, `change`, `remove` et `back` en font, en changent et en retirent un, comme la fenêtre ; `sioul money-mail` liste les courriels qui parlent d’argent et ce qu’ils sont devenus ; `sioul receipts --csv` écrit le livre des recettes de la période de la déclaration, et `sioul books` dit vos déclarations, votre TVA, vos seuils et vos DES ([Le temps et les factures](time.md#from-the-command-line)).

### Avec un agent d’IA {#with-an-ai-agent}

Si vous connectez un agent d’IA ([Avec un agent d’IA](ai-agent.md)), il peut lire ce que dit cette page : les phrases de chaque compte, chaque budget en mots, ce qui arrive et les réserves, chaque mouvement et les courriels qui parlent d’argent, dans les budgets que vous lui avez ouverts ; et vos livres, les paiements des seuls projets et budgets que vous lui avez ouverts, les seuils seulement quand rien ne lui est caché. Quand vous le demandez, il peut écrire, changer ou retirer un mouvement comme la fenêtre, chaque fois dit dans la fenêtre avec **Annuler** ; ce sont des relevés, jamais des paiements : il ne peut pas déplacer d’argent, et il voit les numéros de compte masqués.

## Pourquoi cela marche ainsi {#why-it-works-this-way}

L’argent ne s’écoule pas également sur un mois : les dépenses montent quand la paie arrive, et les factures la suivent (Stephens 2003 ; Gelman et al. 2014) ; les personnes qui tiennent un budget comptent selon leur paie plutôt que selon le calendrier (Zhang et al. 2022). La page part donc du solde d’aujourd’hui et de votre prochaine rentrée d’argent : si le compte tient chaque paiement d’ici là, ce qui reste pour les dépenses du quotidien, et le point le plus bas.

Les mêmes chiffres, montrés en rouge, font attendre le pire (Bazley, Cronqvist & Mormann 2021), et les leçons de gestion d’argent s’effacent en moins de deux ans (Fernandes, Lynch & Netemeyer 2014). Chaque budget dit donc en mots simples ce qui est payé et ce qui reste à venir, sans couleur, barre ni pourcentage, et sans mot de comptabilité à apprendre.

Quand on prévoit son propre argent, on compte ce qui rentrera et on oublie ce qui sortira, surtout les dépenses inhabituelles (Berman et al. 2016 ; Sussman & Alter 2012). Sioul fait donc la prévision lui-même : la liste de ce qui vient donne chaque paiement attendu, entrées et sorties, et les dépenses inhabituelles de l’année passée ont leur propre ligne. Une prévision qui dit son doute sert mieux, et garde la confiance quand elle se trompe (Joslyn & LeClerc 2012) : une rentrée d’argent pas encore sûre est montrée avec sa condition.

Les impayés grossissent en silence : un prélèvement s’arrête, rien ne le dit, et personne ne regarde ses comptes quand ils vont mal (Olafsson & Pagel 2017). Les alertes bancaires ont réduit d’un quart les frais de découvert quand elles venaient d’office (Caflisch et al. 2018), et peu de personnes s’y inscrivent d’elles-mêmes. Sioul lit donc les mouvements de la banque elle-même, et dit quand le compte ne tiendra pas un paiement, avant que cela arrive, à votre prochaine fenêtre d’administration, quels que soient les budgets que l’heure montre.

Un rappel aide le plus quand il nomme ce à quoi sert l’argent (Karlan et al. 2016), et un rappel qui ignore le solde peut faire passer un compte à découvert (Medina 2021). Un paiement que vous faites à la main a donc un seul rappel, quelques jours avant, qui dit si le compte le tiendra.

Un paiement est souvent annoncé par un courriel, une lettre ou une facture, puis passé par la banque. Sioul les garde ensemble comme un seul paiement et les relie lui-même : tenir des relevés à la main pèse sur beaucoup de personnes (Rudberg Selin, Unéus & Knudsen 2026), alors rien n’attend votre vérification, et un lien qui se trompe se défait d’un geste.

La plupart des personnes qui tiennent un budget répartissent leur argent en types à elles, de deux à un par facture (Zhang et al. 2022), et une dépense qui n’entre dans aucun est celle qui s’échappe (Heath & Soll 1996). Les budgets sont donc les types que vous choisissez, Sioul range chaque paiement, et déplacer un paiement dit ce que cela change.

Les revenus irréguliers sont courants : dans une année de journaux tenus par des ménages, le revenu mensuel variait d’environ 40 % (Hannagan & Morduch 2015), et passer un mois maigre avec une grosse dépense demandait environ six semaines de revenu net (Farrell, Greig & Yu 2019). Les réserves se comptent donc en semaines de vos propres dépenses, et un budget de travail peut mettre de côté une part de chaque paiement pour les impôts dès qu’il arrive, comme l’argent mis à part dans une enveloppe est gardé (Soman & Cheema 2011).

Un micro-entrepreneur déclare ce que les clients ont payé, avant tout frais (CGI art. 102 ter), encaissé quand la plateforme ou le prestataire l’a reçu, même s’il arrive plus tard sur la banque (BOFiP, BOI-BNC-BASE-20-10-10 §80). Alors chaque paiement garde ce qui a été payé, ses frais et ce qui est arrivé, et le jour où il a été payé ; les budgets comptent ce qui est arrivé, la déclaration ce qui a été payé, et le livre des recettes liste chaque paiement, jamais le versement qui les a regroupés. Chaque comptabilité qui traite bien les frais en fait une part du paiement (hledger, GnuCash), et d’un versement un virement entre vos propres comptes ; les applications qui n’en ont pas vous laissent partager chaque paiement à la main.

La loi demande à un micro-entrepreneur un livre des recettes dans l’ordre des jours où elles arrivent, chacune avec son client, ce qui a été vendu, le moyen de paiement (les espèces à part) et sa facture, totalisé chaque trimestre et chaque année (Code de commerce, L123-28 ; CGI, art. 50-0, 102 ter et 286 ; BOFiP, BOI-BIC-DECLA-30-30), et l’URSSAF demande chaque mois ou chaque trimestre ce qui a été encaissé, une période sans rien aussi, pour un jour qu’elle décale après un week-end (autoentrepreneur.urssaf.fr). Alors le livre se fait à partir des paiements de vos factures, au jour où chacun est arrivé, et chaque période dit quoi déclarer et pour quand, une période à zéro aussi, avec les cotisations en estimation : l’URSSAF calcule, Sioul prépare, et un jour pour déclarer est un jour, jamais « en retard ».

La fiche de l’URSSAF « Déterminer mon chiffre d’affaires » dit qu’un remboursement fait avant la déclaration la baisse, et qu’un remboursement fait après demande de corriger la déclaration déjà faite. La recherche de Sioul n’a pu lire cette fiche qu’à travers le résumé d’un moteur de recherche, car le site de l’URSSAF refusait la lecture automatique : il reste à une personne de la lire pour confirmer la règle (constat 82 de [ce que dit la recherche, en anglais](https://sioul.aurelienpierre.com/dev/research.html)). Un remboursement compte donc avec l’argent qu’il rend tant que vous n’avez pas dit cette période **Déclarée** et que son jour n’est pas passé, et après, la déclaration suivante dit quoi corriger : rien de déjà déclaré n’est baissé dans votre dos. Sioul ne voit pas ce que vous avez envoyé à l’URSSAF : **Déclarée** est votre parole, dite une fois sur l’un de vos appareils.

Une petite perte pèse moins dite avec le gain plus grand qui l’accompagne (Thaler 1985 ; théorie, au soutien mitigé). Alors les frais se disent dans la ligne de leur paiement, jamais comme une ligne ou un total à part.

On retrouve les choses en faisant défiler le passé récent et en cherchant, bien plus qu’en classant (Whittaker et al. 2011 ; Bergman et al. 2008), et consulter ses derniers mouvements est ce qu’on fait le plus avec sa banque sur un téléphone (Federal Reserve 2016). Alors Mouvements et Courriels s’ouvrent sur les plus récents, la recherche en haut, les filtres repliés, sans rien compter.

## Comparé à d’autres applications {#compared-with-other-apps}

En novembre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

**Pour tout le monde**

| | Sioul | YNAB | Actual | Firefly III | GnuCash | Bankin’ |
|---|---|---|---|---|---|---|
| Des budgets selon ce à quoi sert l’argent ; ce qui n’est pas dépensé, gardé | ✓ | ✓ | ✓ | en partie¹ | en partie² | en partie³ |
| Où en est chaque budget, en mots : payé, à venir, ce qui reste | ✓ | en partie⁴ | ✗ | ✗ | en partie⁵ | ✗ |
| Les paiements attendus, rapprochés des mouvements de la banque | ✓ | ✓ | ✓ | ✓ | ✓ | en partie⁶ |
| Dit quand un paiement attendu n’est pas sorti, ou a pris un autre montant | ✓ | ✗ | en partie⁷ | en partie⁸ | ✗ | ✗ |
| Dit à l’avance qu’un compte ne tiendra pas un paiement, et quoi virer d’ici quand | ✓ | ✗ | ✗ | ✗ | ✗ | en partie⁹ |
| Des paiements lus dans vos courriels, comptés une fois | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Des contrats : renouvellement, date limite de préavis, lettre de résiliation | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Lit les fichiers d’export de votre banque | ✓ | ✓¹⁰ | ✓ | ✓¹¹ | ✓ | ✗ |
| Se connecte à votre banque de lui-même | ✗¹² | ✓ | ✓ | ✓¹¹ | en partie¹³ | ✓ |
| Rapports et graphiques | en partie¹⁴ | ✓ | ✓ | ✓ | ✓ | en partie¹⁵ |
| Placements, plusieurs monnaies, comptabilité en partie double | ✗¹⁶ | en partie¹⁷ | en partie¹⁸ | ✓ | ✓ | en partie¹⁹ |
| Où en est le compte jusqu’à votre prochaine rentrée d’argent : ce qui reste, le point le plus bas | ✓ | ✗ | ✗ | ? | en partie²⁰ | en partie⁶ |
| Ce qui vient : chaque paiement attendu, entrées et sorties, par date | ✓ | en partie²¹ | en partie²² | ? | en partie²³ | en partie²⁴ |
| Un paiement avec tout ce qui en parle : courriel, lettre, contrat, ligne de banque | ✓ | en partie²⁵ | ✗ | en partie²⁶ | en partie²⁷ | ? |

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
14. Le solde de chaque budget par jours, semaines, mois ou années, et le solde à venir jusqu’à la prochaine rentrée d’argent.
15. L’historique des budgets, et ce mois-ci face au mois dernier, dans Bankin’ Plus.
16. Un montant dans une autre monnaie est marqué, pas converti.
17. Les comptes de placement gardés comme comptes de suivi ; une autre monnaie dans un budget à part.
18. Les rapports comprennent les comptes de placement.
19. Comptes d’épargne et de titres, dans Bankin’ Plus.
20. Un solde « Projected Minimum » dans la barre de résumé de chaque registre.
21. Les opérations programmées, en haut du registre de chaque compte.
22. Les échéances dans le registre, jusqu’à une durée que vous choisissez.
23. Les opérations programmées, créées ou rappelées un nombre de jours choisi à l’avance.
24. Les opérations à venir que certaines banques envoient (ING France, Crédit Mutuel).
25. Une photo par opération ; pas de PDF.
26. Des pièces jointes, sur les opérations entre autres.
27. Un fichier ou une adresse web lié par opération.

**Côté technique**

| | Sioul | YNAB | Actual | Firefly III | GnuCash | Bankin’ |
|---|---|---|---|---|---|---|
| Où vivent les données de votre argent | des fichiers lisibles sur votre appareil | les serveurs de YNAB | votre appareil et votre serveur | votre serveur | un fichier sur votre ordinateur | les serveurs de Bankin’, en Europe |
| Demande un compte chez l’éditeur | non | oui | non | non | non | oui |
| Chiffré de bout en bout entre vos appareils | ✓¹ | ✗² | en partie³ | — | — | ✗ |
| Formats d’export lus | OFX, QFX, CAMT.053, CSV, PayPal, Stripe | CSV, OFX, QFX, QIF | CSV, QIF, OFX, QFX, CAMT | CSV, CAMT.053 | QIF, OFX, QFX, CSV, MT940, MT942, CAMT.052 et d’autres | — |
| Un fichier lu deux fois n’ajoute rien | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| Une ligne de banque rapprochée de ce qui était attendu | une référence d’abord (l’identifiant de la banque, un mandat SEPA, un numéro de facture), puis le montant, l’écart en jours et le nom : sûr, probable ou à part | le même montant à 10 jours près | une échéance à 2 jours près ; un import par le même montant à 7 jours près, le même bénéficiaire d’abord | une règle, le montant entre le minimum et le maximum d’un abonnement | un score du montant, de la date, du numéro de chèque et des mots | ? |
| Connexions bancaires par | aucune | Plaid, MX | Enable Banking, SimpleFIN, Pluggy.ai, Akahu ; GoCardless⁴ | GoCardless, Enable Banking, SimpleFIN, FinTS et d’autres | AqBanking (HBCI/FinTS), OFX | la sienne (Bridge, agréée par l’ACPR) |
| Logiciel libre | ✓ GPL-3.0+ | ✗ | ✓ MIT | ✓ AGPL-3.0 | ✓ GPL | ✗ |

1. XChaCha20-Poly1305, avec une clé tirée de votre phrase de passe par Argon2id ; une fois **Projets et argent** activé.
2. Chiffré au repos et pendant le transport sur les serveurs de YNAB ; le chiffrement de bout en bout n’est pas mentionné.
3. Facultatif ; les jetons de synchronisation bancaire ne sont pas couverts, et la copie sur votre appareil n’est pas chiffrée.
4. Indiqué comme « not accepting new accounts ».

D’autres en font plus par endroits : YNAB, Actual, Firefly III et Bankin’ se connectent d’eux-mêmes à la banque, et GnuCash le fait pour les banques allemandes ; GnuCash et Firefly III tiennent une comptabilité en partie double, des placements et plusieurs monnaies ; les quatre applications de budget font des rapports plus complets, et leurs applications pour téléphone ont plus d’usage derrière elles ; PocketSmith, hors du tableau, prévoit le solde de chaque compte pour chaque jour, des années à l’avance. Ce que Sioul ajoute, c’est le compte dit jusqu’à votre prochaine rentrée d’argent, chaque paiement gardé avec le courriel, la lettre et le contrat qui en parlent, la veille en phrases, et vos contrats avec leurs dates de préavis, le tout sans connexion bancaire.

**Les frais, les versements et les courriels qui parlent d’argent**

| | Sioul | hledger | GnuCash | Firefly III | YNAB | Actual | PocketSmith |
|---|---|---|---|---|---|---|---|
| Un paiement garde ce qui a été payé, ses frais et ce qui est arrivé | ✓ | ✓¹ | ✓² | en partie³ | en partie⁴ | en partie⁴ | ? |
| Le versement d’un prestataire est un virement entre vos comptes | ✓ | ✓¹ | ✓² | ✓ | ✓⁵ | ✓⁵ | ? |
| Un versement lié aux paiements qu’il regroupe, sans le fichier du prestataire | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | ? |
| Chaque courriel qui parle d’argent gardé, chacun disant ce qu’il est devenu | ✓ | ✗ | ✗ | ✗ | ✗ | ✗ | en partie⁶ |
| Le chiffre d’affaires à déclarer, au jour du paiement, et le livre des recettes fait des paiements de vos factures | ✓⁷ | ? | ? | ? | ? | ? | ? |

1. Ses règles pour PayPal écrivent trois écritures par paiement : le brut, le net, les frais ; le retrait est un virement entre deux comptes.
2. Une opération de plusieurs parts, les retenues d’un paiement parmi elles.
3. Une opération partagée en plusieurs parts ; pas de champ pour des frais.
4. Un partage à la main, avec une ligne de frais ; pas de champ pour des frais.
5. Un virement est une paire d’opérations entre deux de vos comptes.
6. Sa boîte de pièces jointes garde les reçus en attente de leur opération.
7. Les règles de la France : la déclaration de chaque période par activité, avec son jour et les cotisations estimées ; pour qui est redevable de la TVA, la TVA collectée ; les seuils de la franchise en base de TVA et les plafonds du régime micro, chaque étape dite une fois ; la DES. Préparé, jamais déposé.

??? info "Sources"
    Lues le 8 octobre 2026 ; les lignes sur la prochaine rentrée d’argent, ce qui vient, les sources d’un paiement et le rapprochement, le 4 novembre 2026.

    - **YNAB**, son centre d’aide (glossaire, barres de progression, import de fichiers, import direct en Europe, opérations programmées, dépassements, et son index), sa page sur la sécurité et ses prix : <https://support.ynab.com/en_us/ynab-glossary-a-guide-BJd80SORq>, <https://support.ynab.com/en_us/progress-bars-a-guide-SkDEhot09>, <https://support.ynab.com/en_us/file-based-import-a-guide-Bkj4Sszyo>, <https://support.ynab.com/en_us/direct-import-in-europe-Syae1z_A9>, <https://support.ynab.com/en_us/scheduled-transactions-a-guide-BygrAIFA9>, <https://support.ynab.com/en_us/overspending-in-ynab-a-guide-ryWoxEyi>, <https://support.ynab.com/llms.txt>, <https://ynab.com/security>, <https://www.ynab.com/pricing>
    - **Actual Budget**, sa documentation sur les budgets, l’import, la synchronisation bancaire, les règles, les échéances, la synchronisation, l’installation et les rapports, et son dépôt (`schedules.ts` pour « missed ») : <https://actualbudget.org/docs/budgeting/>, <https://actualbudget.org/docs/transactions/importing>, <https://actualbudget.org/docs/advanced/bank-sync>, <https://actualbudget.org/docs/budgeting/rules/>, <https://actualbudget.org/docs/schedules>, <https://actualbudget.org/docs/getting-started/sync>, <https://actualbudget.org/docs/install/>, <https://actualbudget.org/docs/reports/>, <https://github.com/actualbudget/actual>
    - **Firefly III**, sa documentation sur les budgets, les abonnements, l’import de fichiers et les fournisseurs de données, et son dépôt : <https://docs.firefly-iii.org/how-to/firefly-iii/finances/budgets/>, <https://docs.firefly-iii.org/explanation/financial-concepts/budgets/>, <https://docs.firefly-iii.org/explanation/financial-concepts/subscriptions/>, <https://docs.firefly-iii.org/how-to/data-importer/import/file/>, <https://docs.firefly-iii.org/tutorials/data-importer/data-providers/>, <https://github.com/firefly-iii/firefly-iii>
    - **GnuCash**, sa page des fonctions, son manuel sur l’import et les préférences, et le chapitre de son guide sur les budgets : <https://www.gnucash.org/features.phtml>, <https://www.gnucash.org/docs/v5/C/gnucash-manual/trans-import.html>, <https://www.gnucash.org/docs/v5/C/gnucash-manual/set-prefs.html>, <https://code.gnucash.org/docs/C/gnucash-guide/chapter_budgets.html>
    - **Bankin’**, sa propre description dans l’App Store d’Apple (son centre d’aide n’a pas pu être lu), sa page Plus et Pro et sa page d’accueil : <https://apps.apple.com/fr/app/bankin-la-meilleure-app-pour-g%C3%A9rer-mon-argent/id447040033>, <https://bankin.com/fr/pluspro.html>, <https://bankin.com>
    - **Ajoutées le 5 novembre 2026**, pour les frais, les versements et les courriels qui parlent d’argent : le manuel de hledger 1.19, l’exemple de PayPal, <https://hledger.org/1.19/csv.html> ; le guide de GnuCash, les opérations simples et partagées, par un résumé de recherche, <https://code.gnucash.org/docs/C/gnucash-guide/txns-registers-txntypes.html> ; les opérations partagées de Firefly III, telles que les documente son client généré, <https://docs.rs/crate/firefly-iii/latest/source/docs/TransactionSplit.md> ; les opérations partagées de YNAB, par un résumé de recherche, <https://support.ynab.com/en_us/split-transactions-a-guide-SJLEKwY0q.md> ; les virements et les partages d’Actual, dans sa documentation lue le 4 novembre ; les pièces jointes de PocketSmith, lues le 4 novembre pour la recherche sur les budgets.
    - **Ajoutées le 4 novembre 2026** : YNAB sur le rapprochement et les photos, <https://support.ynab.com/en_us/approving-and-matching-transactions-a-guide-ByYNZaQ1i>, <https://support.ynab.com/en_us/add-context-to-transactions-HyCpTR4bg> ; le rapprochement d’Actual dans son dépôt, `packages/loot-core/src/server/accounts/sync.ts` ; les pièces jointes de Firefly III, par un résumé de moteur de recherche, <https://docs.firefly-iii.org/api/storeattachment/> ; le registre de GnuCash, <https://www.gnucash.org/docs/v5/C/gnucash-manual/gui-acct-reg.html>, ses opérations programmées et ses documents liés, par un résumé de moteur de recherche, <https://code.gnucash.org/docs/C/gnucash-manual/trans-sched-slr.html>, <https://code.gnucash.org/docs/C/gnucash-manual/trans-assoc.html>, et son rapprochement, `gnucash/import-export/import-backend.cpp` dans <https://github.com/Gnucash/gnucash> ; Bankin’ sur les opérations à venir, par un résumé de moteur de recherche, <https://support.bankin.com/hc/fr/articles/360027356511-Op%C3%A9rations-%C3%A0-venir> ; la prévision de PocketSmith, <https://www.pocketsmith.com/features/budgets-and-planning>

## Côté technique {#for-technical-readers}

**Les fichiers.** `sioul-budgets.toml` (budgets, paiements réguliers, lignes, réserves, couvertures, règles des courriels, comptes bancaires, leurs règles et vos choix au cas par cas), `sioul-bank.toml` (les mouvements de chaque compte par l’identifiant de la banque, son solde le plus récent), `sioul-contracts.toml` : du TOML dans `records/` du dossier de données de Sioul (à la racine de votre dossier de notes tant que les listes de Sioul n’ont pas été déplacées), écrit en gardant ses commentaires. Les courriels dont vous avez dit qu’ils ne sont pas des paiements sont listés dans `~/.local/state/sioul/money.toml` sous Linux. Les montants sont gardés en centimes entiers, pour que les sommes ne dérivent jamais, et les montants écrits à la façon de chaque pays sont lus (« 1 234,56 € », « €1,234.56 », « (62.00) »).

**Les exports.** OFX 1.x (SGML) et 2.x (XML), par le `FITID` de chaque mouvement ; camt.053 de l’ISO 20022, des versions 001.02 à 001.08 (les écritures comptabilisées, le solde de clôture comptabilisé, les noms sous `Pty`) ; le CSV d’une banque, son en-tête trouvé par ses mots, avec `;` ou `,`, la virgule décimale, débit et crédit séparés ou non, et un solde écrit au-dessus du tableau ; l’activité de PayPal (le net de chaque ligne, avec son brut et ses frais quand ils tombent juste, gardés à côté dans `sioul-movements.toml` ; les lignes en attente, refusées et pour mémoire laissées de côté, ainsi que celles dans une autre monnaie que la plupart du fichier) ; l’historique du solde de Stripe (son Amount et ses frais aussi) ; le relevé de Revolut (ses frais en plus de son montant) et celui de Wise (ses frais totaux compris dedans), comme le dit le solde courant de chaque fichier ; les frais d’un camt.053 quand la banque dit qu’ils sont compris dans l’écriture. Un mouvement sans identifiant en reçoit un, fait de son jour, de son montant et de son libellé : un fichier lu deux fois n’ajoute rien.

**D’une entrée à la suivante.** Le solde d’un compte est le dernier de la banque, avec les lignes lues après lui, projeté avec chaque mouvement attendu (prévu, annoncé, une empreinte de carte) jusqu’à sa prochaine entrée : la première entrée qui n’est ni un virement entre vos comptes ni un remboursement, d’au moins un cinquième de la plus grosse à venir et de 50 € ; une entrée régulière ou annoncée est sûre, une ligne prévue ou une facture à vous est dite avec son doute. Le reste à vivre est le point le plus bas au-dessus de ce que le compte garde, moins la part des dépenses inhabituelles de l’année passée pour ces jours : les paiements qui ne représentent aucun paiement régulier, trois fois le paiement habituel et 50 € au moins, par mois. Sans entrée sous deux mois, le compte et son épargne sont comptés en semaines de vos paiements des treize dernières semaines. La fin d’un budget est jugée au rythme du milieu de ses périodes passées dès qu’il en a trois avec des mouvements non prévus ; avant cela, ce qui est entré ou sorti au-delà du prévu est rapporté à la part de la période écoulée, et l’avance ou le retard d’aujourd’hui est reporté jusqu’à la fin. À moins de 5 % de ce que la période fait bouger (et jamais plus près que 10), il finit comme prévu.

**La veille.** Un paiement n’est pas vu quand ses jours (de trois avant à sept après) sont passés dans ce que couvrent les exports de son compte, sans ligne pour lui. « A pris un autre montant » veut dire plus de 5 % et 2 € d’écart. Un renflouement est arrondi à la dizaine au-dessus et demandé autant de jours à l’avance que sa réserve en met. Le filtre des budgets est gardé sur chaque appareil, dans `~/.local/state/sioul/budgets-view.toml`, et une copie des mouvements d’argent faite à partir de vos fichiers dans `~/.cache/sioul/money-books.json`, refaite dès qu’un fichier change : ni l’un ni l’autre n’est partagé, et les supprimer ne fait rien perdre.

**Les courriels.** Seuls les courriels qui ont passé les vérifications d’authentification sont lus pour y trouver des paiements ([Chaque courriel vérifié](privacy-security.md#every-message-checked)) : jamais les courriels mis de côté (falsifiés, indésirables, un nom emprunté, un expéditeur bloqué), ni la file à examiner. Les paiements sont lus par des règles, en français et en anglais, dans l’objet et le début du texte ; un capital social dans des mentions légales n’est jamais pris pour un montant. Chaque courriel est lu pour l’argent une fois, pour toujours, dans un index de l’argent sur l’appareil (`~/.cache/sioul/money-index/`, jamais partagé), ouvert seulement quand le domaine de son expéditeur est celui d’un prestataire, qu’une règle des courriels le nomme, que son objet porte un mot de paiement ou un montant, ou qu’une pièce jointe est nommée comme une facture ; sur 20 000 courriels inventés, sa première lecture a pris 1,4 s sur un ordinateur.

**Scellé entre vos appareils.** Avec **Projets et argent** activé, chaque changement des trois fichiers est scellé avec XChaCha20-Poly1305 (un nonce aléatoire de 192 bits chacun), lié à l’appareil qui l’a écrit, à sa place et à son heure, sous une clé tirée de votre phrase de passe par Argon2id (64 Mio, trois passes) et gardée dans le trousseau de chaque appareil ([Le partage](sharing.md#what-it-protects-and-what-it-cannot-hide)).

**L’agent d’IA.** Ses outils `budgets`, `movements`, `get_movement` et `money_mail` lisent par les mêmes vues que la fenêtre, dans les budgets ouverts aux agents, et masquent les IBAN (« [IBAN …0189] ») ; `add_movement`, `update_movement`, `delete_movement` et `same_movement` écrivent comme le font les actes de la fenêtre, chaque fois dit dans la fenêtre avec Annuler, jamais un paiement. Son outil `books` lit les livres, en laissant de côté et en comptant ce qui lui est caché.

**Les livres.** Faits des paiements de vos factures, chacun lu à un seul endroit, ceux d’un avoir en dessous de zéro, le « payée » d’une facture faite avant novembre 2026 au jour de sa ligne de banque quand votre banque l’a montrée, son remboursement placé d’après ce jour aussi, et des recettes de vos budgets de travail qu’aucune facture ne décrit, un mouvement dont vient le paiement d’une facture compté une fois ; les déclarations, le récapitulatif de TVA, les seuils et la DES suivent les règles de la France, gardées en données datées (`crates/sioul-core/data/invoice-presets/fr.toml`), qu’un taux ou un seuil à vous dans `[invoice]` remplace à partir d’un jour ([Paramètres ▸ Factures](settings.md#invoices)). Le jour pour déclarer est le dernier jour du mois qui suit la période, décalé après un week-end ; celui de la DES, le dixième jour de semaine du mois suivant. Une déclaration ou une DES que vous avez dite **Déclarée** est écrite aussi dans les fichiers de vie de vos factures (`[[declared]]`, avec le jour où vous l’avez dite ; **Pas déclarée** la reprend, chaque parole datée après la dernière que son appareil a lue pour cette déclaration, quoi que dise son horloge), si bien qu’elle quitte le Porche sur chaque appareil, et qu’un remboursement d’après elle est dit comme une correction. L’étape d’un seuil, une fois dite, est écrite dans les fichiers de vie de vos factures (`[[told]]`), qui voyagent avec **Brouillons et factures**, si bien qu’aucun appareil ne la redit comme nouvelle : le partage activé, seul un ordinateur qui partage **Brouillons et factures** les dit, une fois ses fichiers échangés depuis qu’il a démarré. Ce qui vous demande de faire quelque chose est redit à chaque démarrage de Sioul, jusqu’à ce que ce soit fait. Seuls les seuils de la France sont suivis : ceux de l’Allemagne ne le sont pas, tant que la recherche ne dit pas comment ils se comptent.

**Les contrats.** Les préavis habituels proposés sont ceux de la France : trois mois pour un locataire (un mois en meublé ou en zone tendue) ; un mois pour les assurances et les complémentaires santé après leur première année (loi Hamon) ; dix jours au plus pour le téléphone et internet après douze mois ; aucun pour l’énergie (art. L224-13 du code de la consommation). En ligne, un bouton de résiliation est obligatoire depuis le 1er juin 2023 (loi 2022-1158).
