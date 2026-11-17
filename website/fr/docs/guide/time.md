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
- Un numéro de facture n’est jamais donné deux fois, même avec plusieurs appareils : un seul appareil numérote les factures, et Sioul attend plutôt que de risquer un numéro en double. Sioul note le dernier numéro donné, si bien qu’un numéro reste donné même si son fichier est supprimé.
- Une facture émise n’est jamais changée : un avoir qui la cite la corrige.
- Le temps déjà facturé ne peut plus être changé, ni facturé de nouveau.
- Les tableurs pour votre comptable ne peuvent pas porter de formule cachée : un titre de tâche ou un nom de client venu d’un courriel de quelqu’un est écrit comme du simple texte.
- Un paiement compte une fois, quoi qu’aient fait vos appareils : la case Payée cochée sur deux appareils, le même export de la banque pris sur deux, ou le même paiement tapé sur deux compte une fois ; et quand votre banque montre le virement d’une facture que vous avez déjà marquée payée, ce virement compte à la place de la case Payée, au lieu de s’y ajouter.
- Quand vos appareils partagent, votre temps et vos factures voyagent scellés, par un dossier que votre propre synchronisation transporte : le dossier et son serveur voient que quelque chose a changé, jamais quoi ([Le partage](sharing.md)).

## La page Temps {#the-time-page}

**Semaine**, **Mois** ou **Année**, pour **Tous les projets** ou un seul ; **Aujourd’hui** et les flèches ◂ ▸ font avancer ou reculer dans le temps :

- une barre par jour (par mois, sur une année), empilée par projet ;
- les heures de chaque projet, et ce qui reste à facturer, en heures et en argent à son taux ;
- puis chaque plage de temps, de la plus récente à la plus ancienne.

## D’où vient le temps {#where-time-comes-from}

- **Le minuteur de concentration** : les minutes d’une [séance de concentration](tasks.md#starting-and-stopping) comptent pour sa tâche, et pour le projet de la tâche.
- **Noter du temps**, sur cette page, sur la page d’un projet, ou avec **Nouveau ▾ ▸ Du temps passé** : une réunion, un appel, un travail fait loin du minuteur. **Pour** (un projet, ou une tâche), **Combien de temps** (`1h30`, `45m`, `90`), **Quand**, **Ce que c’était**, et **Ne pas facturer** quand il ne doit pas l’être.

Pendant une séance de concentration, sous Linux et sur Android, une notification la montre : sa tâche, depuis quand, et **Pause** (**Reprendre** une fois en pause) et **Arrêter**, qui font ce que font les boutons de la fenêtre de concentration. Sur un bureau Linux, elle reste parmi les notifications du bureau tant que Sioul est ouvert (dans KDE Plasma, sous la cloche une fois sa bulle partie), et s’en va quand Sioul se ferme. Sur Android, un chronomètre y tourne, et ses boutons marchent avec Sioul en arrière-plan ou fermé. Mise en pause ou arrêtée ailleurs, dans la fenêtre de concentration ou sur un autre appareil, elle suit. Sous Windows et macOS, seule la fenêtre de concentration montre le temps qui court, pour l’instant.

Cliquez sur n’importe quel moment, chronométré ou noté à la main, pour le changer : sa tâche, son projet, son jour, de quand à quand, et ce que c’était ; le temps que le minuteur a gardé après une pause manquée se corrige ainsi. Un clic droit propose aussi **Enlever ce temps**. Le temps déjà sur une facture reste tel qu’il a été facturé.

Un minuteur oublié est coupé quand vous l’arrêtez : au-delà du double du temps choisi (ou de ce temps et une demi-heure de plus, si c’est plus long), seul le temps choisi compte ; une séance sans fin fixée compte trois heures au plus.

Chaque moment dit discrètement comment ses minutes ont été connues :

- *chronométré* ;
- *noté à la main* ;
- *chronométré, puis corrigé*, une fois que vous avez changé la durée d’un moment chronométré, ou que Sioul a coupé un minuteur resté lancé ;
- *on ne sait pas comment*, pour le temps noté avant que Sioul ne le garde.

### Facturé ou non {#billed-or-not}

Le travail pour un client se facture. Une tâche peut dire autre chose, sous **Facturé** dans son panneau : *Comme son projet*, *Son temps se facture*, ou *Non facturé*.

## Combien de temps prennent les choses {#how-long-things-take}

Le **Prend environ** d’une tâche est une estimation. Le temps noté pour elle est ce qu’elle a pris. Sioul met les deux côte à côte, et corrige vos estimations dans le plan seulement : chaque tâche prend sa durée corrigée dans le plan et dans la journée, tandis que **Prend environ** reste tel que vous l’avez écrit, et le minuteur part de lui. Seules les tâches finies lui apprennent, les récentes le plus ; les minutes notées à la main comptent moins que celles qui sont chronométrées, parce que les durées dont on se souvient sont moins sûres. Comment il apprend, en détail : [plus bas](#how-the-plan-learns-from-your-time).

Il ne vous note jamais, ne montre jamais rien en retard, ne remplit jamais **Prend environ** à votre place, et ne vous compare jamais à personne.

Chronométrer les loisirs, c’est à vous de voir. Les trous ne posent pas de problème : le temps non noté est laissé de côté, il ne compte contre rien.

## Un tableur du temps facturable {#a-spreadsheet-of-billable-time}

Le bouton d’export, en haut à droite de la page Temps : **Le temps facturable, en tableur (CSV)…**, pour un projet et une période (cette semaine, la semaine dernière, ce mois-ci, le mois dernier, depuis toujours, ou d’un jour à un autre). Une ligne par tâche, avec ses heures, le taux horaire et le montant, puis le total. En français, le fichier utilise `;` et la virgule décimale, comme les tableurs français les lisent. Un titre de tâche venu d’un courriel de quelqu’un est écrit comme du simple texte : un tableur ne l’exécute jamais comme une formule.

## Les factures {#invoices}

Sur la page d’un projet, ou à côté du projet sur la page Temps, **Faire la facture** quand du temps facturable attend. Sioul fait la facture de ce temps et l’émet du même clic. S’il manque quelque chose que demandent les règles de votre pays (votre SIREN, l’adresse d’un client professionnel…), Sioul dit quoi, en mots simples, et ne fait rien : ajoutez-le, puis refaites la facture. Ce qui est un conseil plutôt qu’une obligation (le SIREN du client avant septembre 2027, une facture qui ne porte pas encore de données électroniques, un IBAN avec un chiffre faux) est dit dans la ligne d’état au moment où la facture est émise ; l’éditeur de factures à venir le dira avant. L’unité de chaque ligne est une unité que Sioul connaît (une heure, une minute, un jour, une unité, un mois, un kilomètre, un forfait), aucun prix unitaire n’est sous zéro (moins à payer, c’est un avoir), et un pays s’écrit en deux lettres ou par son nom ; l’écriture de droite à gauche (hébreu, arabe) ne s’imprime pas encore : Sioul demande des lettres latines.

- **Les numéros.** Avec le modèle habituel, l’année puis trois chiffres, les numéros repartent de 001 chaque mois de janvier : 2026-001, 2026-002. Avec votre propre préfixe, ils continuent après le dernier, d’une année sur l’autre. Sioul note le dernier numéro donné dans chaque série : un numéro n’est jamais donné deux fois, même si son fichier est supprimé à la main, et un numéro sans facture sur cet appareil est signalé, jamais redonné. Une facture ne peut pas être supprimée depuis Sioul.
- **Les lignes** : une par tâche (ou par temps noté sans tâche), avec son temps au taux du projet, au centime près.
- **Facturé une fois** : le temps facturé est listé dans sa facture, et porte son numéro ; la facture suivante le laisse de côté, et ce temps ne peut plus être changé. Ce que listent vos factures émises fait foi : avant chaque nouvelle facture, Sioul marque de nouveau votre temps d’après elles, pour qu’une même heure ne soit jamais facturée deux fois, même après un plantage.
- **Ce qu’elle imprime**, comme le disent [les règles de votre pays](#your-countrys-rules) : votre nom, suivi de « EI » pour un entrepreneur individuel en France, votre adresse et vos identifiants (votre SIREN ou votre SIRET en France) ; le nom, l’adresse et les identifiants du client, et l’adresse de livraison des biens quand elle est autre ; son numéro ; le jour où elle est émise, la date ou la période de la prestation (le premier et le dernier jour des séances qu’elle facture, ou leur seul jour ; pour une facture qui n’est pas faite de temps, le jour où elle est émise, sauf si un jour a été donné), et le jour où le paiement est dû ; ce qu’elle vend (des prestations de services, des biens, ou les deux) ; chaque ligne avec sa quantité, son unité, son prix unitaire et son montant ; la TVA par taux quand vous y êtes assujetti, et les totaux ; la mention de la franchise en base de TVA quand vous en relevez (« TVA non applicable, article 293 B du CGI » en France), ou, pour une entreprise à l’étranger, les mots qui disent où la TVA est due ; pour un client professionnel, les mentions sur le retard de paiement (en France : pas d’escompte pour paiement anticipé, des pénalités au taux de la BCE majoré de 10 points, gardé avec la facture, et l’indemnité forfaitaire de 40 € pour frais de recouvrement), aucune pour un particulier, ni pour un organisme public, dont les règles propres échappent aux sources de Sioul ; vos modalités de paiement et vos propres lignes ; sur une facture en euros avec votre IBAN, un code QR qu’une application bancaire scanne pour remplir le virement.
- **Dans sa propre langue et sa propre devise** : le français ou l’anglais, et la devise de la fiche du client, sinon celle de vos réglages, quelle que soit la langue de la fenêtre. Ses montants s’affichent dans cette devise partout dans Sioul.
- **Le fichier.** Une facture émise est un PDF/A-3 (A4), la forme de PDF faite pour être gardée, qui porte aussi la facture en données électroniques (Factur-X, dans le profil de la norme européenne) : la même facture, lue par un programme. Sioul le fait une fois, quand la facture est émise, le garde à côté de la facture avec son empreinte, et en met une copie dans `Documents/Factures` de votre dossier personnel, ou dans le dossier choisi. **Imprimer à nouveau** rend ce fichier-là, jamais un nouveau : un fichier changé ou absent depuis est refait tel qu’il a été émis, quand Sioul le peut ; sinon rien n’est remis, et Sioul dit où sont la facture et son fichier. Si le dossier des copies ne peut pas être écrit (un disque débranché), la facture est émise quand même, et la ligne d’état le dit. Si une émission s’est arrêtée en chemin (l’ordinateur éteint), la suivante met son fichier de côté dans le dossier `leftovers` des factures, et le dit une fois. Il va sur vos autres appareils avec vos factures. Les factures faites avant novembre 2026 s’impriment comme elles l’ont toujours fait.
- **Le code QR**, sur les factures en euros avec un IBAN : le montant dû, le titulaire et l’IBAN de votre compte, et le numéro de la facture comme référence du virement, les mêmes qu’imprimés à côté. `qr = false` dans vos réglages l’enlève ([Paramètres ▸ Factures](settings.md#invoices)).
- **Votre budget** : le montant dû est attendu dans le budget du projet le jour de son échéance : trente jours plus tard, sauf si vos réglages ou la fiche du client disent autre chose. Vos budgets comptent en euros : une facture dans une autre devise n’y est pas attendue, pour l’instant.

Votre nom, votre adresse, vos numéros, la devise, les modalités de paiement et le taux horaire se règlent une fois, dans [Paramètres ▸ Factures](settings.md#invoices).

### Payée, et les avoirs {#paid-and-credit-notes}

- **Payée**, sur la page d’un projet ou sur la carte de la facture dans l’agenda, dit que la facture est payée, par virement, à la date du jour, en couvrant tout ce qui reste à payer, et l’argent compte dans le budget ; **Annuler** revient en arrière. Une fois la case décochée, ce que vous aviez indiqué à la main est retiré ; ce que votre banque a montré reste.
- **Depuis votre banque** : quand vous prenez l’export de votre banque (**Prendre un export…**), un virement que Sioul rapproche de vos factures les paie, au jour de la banque : une facture, ou plusieurs quand un seul virement les paie ensemble, réparti entre elles, de la plus ancienne à la plus récente. Une facture ne reçoit jamais plus que ce qui lui reste à payer, et ce qui est arrivé en plus est dit. Une facture que vous avez déjà marquée **Payée** ne reçoit pas de second paiement : le virement compte, et **Payée** ne couvre que ce qu’il laisse. Un paiement que vous avez inscrit avec son montant prend pour lui le virement de ce montant exact, sinon le plus proche en jours, et garde son propre jour ; un paiement indiqué en espèces ne le fait jamais. Par PayPal ou Stripe, ce que le client a payé compte, au jour où il l’a payé, quand l’export du prestataire de paiement le dit, sa commission à part ; quand seul ce qui est arrivé sur votre compte est connu, un montant inférieur à ce que demande la facture d’au plus la commission habituelle du prestataire la paie quand même, et la différence est dite comme cette commission. La ligne d’état le dit (« Payée, d’après votre banque : facture 2026-004. »), avec **Annuler**, qui reprend ces paiements et garde désormais ce virement à part des factures ; **Pas le même paiement**, sur ce mouvement dans [Les budgets](budgets.md), fait de même plus tard. Il n’y a rien à confirmer. Sioul rapproche un virement d’une facture dont l’argent est attendu dans un budget, le champ **Factures attendues dans** du projet, et dans la devise de la facture seulement : un virement en euros ne paie jamais une facture en dollars.
- **Payée en partie** : les paiements s’additionnent ; tant qu’ils n’atteignent pas le montant dû, la facture n’est pas encore payée, et `sioul invoices` dit ce qui reste. Un paiement n’est jamais plus grand que ce qui reste à payer. Sioul ne dit jamais d’une facture qu’elle est en retard.
- **Une facture erronée** n’est jamais changée : un avoir qui la cite la corrige, avec toutes ses lignes pour l’annuler, ou certaines pour les corriger, chaque ligne avec la TVA dont elle a été facturée. Il prend le numéro suivant de la même série. Émis, il règle ce qu’il crédite, et le temps de chaque ligne qu’il annule en entier peut être facturé de nouveau. Il ne crédite jamais plus que ce qui reste à créditer sur la facture : un avoir au-delà, comme un second avoir pour une facture déjà annulée en entier, est refusé, et les avoirs déjà émis sont nommés. Un avoir n’est jamais payé ni impayé : il dit quelle facture il crédite. Quand cette facture a été payée au-delà de ce qu’elle demande désormais, l’avoir dit aussi ce que vous avez à rembourser (« 120,00 € à rembourser »), et a une case **Remboursé** ; `sioul invoices pay <avoir> --amount 50` dit une part remboursée. L’argent que la facture devait apporter à votre budget baisse du montant de l’avoir, ou s’en va quand la facture est annulée en entier, pour que le prochain virement du client ne lui soit jamais rapproché. Une facture annulée en partie et pas payée n’est pas « payée en partie » : il lui reste simplement moins à payer. Pour l’instant, les avoirs se font [en ligne de commande](#from-the-command-line).
- **Les acomptes** : les factures d’acompte, et la facture finale qui les déduit, viendront avec l’éditeur de factures ([Pas encore là](#not-there-yet)) ; aucun bouton, aucune commande ni aucun agent d’IA ne les fait encore.

### Les règles de votre pays {#your-countrys-rules}

Ce qu’une facture doit dire dépend de votre pays, et en France de trois choses qui ne s’impliquent pas l’une l’autre : votre forme juridique (entrepreneur individuel, micro-entrepreneur compris, ou société), votre régime d’imposition (micro ou réel), et votre régime de TVA (la franchise en base, ou redevable de la TVA), qui peut changer à un jour donné. La facture d’un micro-entrepreneur redevable de la TVA porte « EI » et la TVA par taux ; celle d’une société en franchise porte la mention de la franchise, sa forme et son capital.

Sioul connaît les règles de la France, de l’Allemagne, de la Belgique, du Royaume-Uni et de la Suisse, et **Ailleurs**, pour tout autre pays : aucune affirmation juridique, chaque identifiant et chaque ligne sont les vôtres. Les règles de chaque pays sont des données que Sioul lit, avec leurs sources :

- **L’Espagne et l’Italie ne sont pas prises en charge**, et Sioul dit pourquoi plutôt que d’offrir un modèle qui y serait illégal : l’Espagne demande un logiciel de facturation certifié, aux factures chaînées et signées (VERI\*FACTU) ; les factures italiennes passent par le système d’échange de l’État (SdI), dans son propre format.
- **La Belgique** : entre entreprises belges, les factures sont des factures électroniques structurées, envoyées par Peppol depuis 2026, et un PDF par courriel ne suffit plus. Sioul n’atteint pas Peppol : il fait le PDF, et envoyer la facture par votre point d’accès vous revient. En franchise, écrivez-en vous-même la mention : Sioul ne connaît pas la formulation exacte qu’exige la loi.
- **La France, redevable de la TVA, avec des clients particuliers** : un logiciel qui enregistre leurs paiements doit être certifié ou attesté (article 286 I 3° bis du CGI), et Sioul n’a ni l’un ni l’autre. En franchise, rien n’est demandé.
- **La France, la mention de la franchise** : le droit de la TVA passe dans un nouveau code, le CIBS, le 1er janvier 2027, et l’article à y citer n’est pas encore tranché (L. 223-3 ou L. 233-3). « TVA non applicable, article 293 B du CGI » reste valable jusqu’au 30 juin 2028 et est la mention par défaut ; vous pouvez en écrire une autre.
- **Les taux de TVA** : Sioul livre ceux de chaque pays tels qu’ils étaient connus à la mi-2026 ; vérifiez-les avant de vous y fier.
- **Des clients dans d’autres pays** : là où la TVA est due dépend de l’endroit où est le client, de ce qu’il est une entreprise ou non, et de ce que vous vendez, et les règles de votre pays placent chaque ligne. En France, une prestation de services à une entreprise d’un autre pays de l’UE porte « Autoliquidation » et les deux numéros de TVA, en franchise aussi ; redevable de la TVA, des biens vendus à une telle entreprise portent « Exonération, art. 262 ter I du CGI » et les deux numéros de TVA ; une prestation de services à une entreprise hors de l’UE porte « TVA non applicable – article 259-1 du CGI ». Là où les sources de Sioul ne donnent pas les mots (une exportation de biens ; dans les autres pays, tous les cas sauf l’autoliquidation de l’UE, comme une prestation de services à une entreprise hors de l’UE, en Allemagne et en Belgique sous leur régime de franchise des petites entreprises aussi), Sioul vous demande de les écrire sur la ligne avant que la facture soit émise. Un numéro de TVA est vérifié selon les règles de son propre pays.
- **Le pays du client** vient de sa fiche, sinon de la dernière ligne de l’adresse de son contact quand elle nomme un pays, dans l’une des langues courantes (« Belgique », « Deutschland », « Maroc »), sinon des deux lettres par lesquelles commence son numéro de TVA. Une adresse dont la dernière ligne est un code postal et une ville est prise pour une adresse de votre propre pays, et Sioul le dit avant que la facture soit émise. Quand cette ligne ne nomme aucun pays que Sioul connaisse, ou qu’il n’y a aucune adresse, Sioul demande le pays du client avant que la facture soit émise, partout où les règles de votre pays placent la TVA selon lui. Un client à Monaco est facturé comme un client en France pour la TVA, et son adresse dit Monaco. Le pays s’imprime par son nom, dans la langue de la facture.

En attendant que Paramètres ▸ Factures les montre par pays, ces choix se font dans le fichier de configuration ([Paramètres ▸ Factures](settings.md#invoices)). Les réglages des factures faits avant continuent de fonctionner comme avant : une mention de TVA qui cite l’article 293 B veut dire la France, le régime micro et la franchise ; toute autre mention veut dire Ailleurs, ses lignes imprimées comme avant. Avec une mention qui cite l’article 293 B, votre forme juridique est demandée une fois, dans Paramètres ▸ Factures ▸ **Forme juridique**, avant que votre prochaine facture soit émise : une société en franchise ne doit pas porter « EI », si bien que « EI » n’est imprimé qu’une fois que vous avez choisi *Entrepreneur individuel (micro-entrepreneur compris)*. Si vos modalités de paiement parlent déjà de pénalités de retard, comme le demandait l’aide d’avant de Sioul, Sioul vous demande de garder soit vos mots, soit ses propres mentions, pour qu’une facture n’indique jamais deux taux ([Paramètres ▸ Factures](settings.md#invoices)).

### Vos clients {#your-clients}

Un client peut avoir une fiche, dans `sioul-clients.toml` dans votre dossier de notes, liée à son contact : une entreprise, un particulier ou un organisme public ; la dénomination ; l’adresse de facturation et, quand elle est autre, l’adresse de livraison ; ses identifiants (un SIREN, un numéro de TVA) ; la langue, la devise et le délai de paiement de ses factures ; l’adresse où les envoyer ; un bon de commande à imprimer. Un projet dont le **Pour** nomme ce contact, ou le nom de la fiche, facture avec elle. Sans fiche, une facture prend le nom du contact et sa première adresse postale, comme avant, et facture le client comme une entreprise, quoi que porte le contact ; seule une fiche qui dit `kind = "consumer"` facture un particulier, dont l’adresse est alors conseillée plutôt qu’exigée, car un particulier peut demander qu’elle ne soit pas imprimée. `sioul invoices show` dit lequel. Jusqu’à son émission, un brouillon suit la fiche de son client, ou sa fiche de contact : une fiche changée entre-temps compte. Pour l’instant, les fiches s’écrivent à la main dans ce fichier (un exemple dans [les notes de conception, en anglais](https://sioul.aurelienpierre.com/dev/projects.html#clients)) ; Contacts et Projets les montreront plus tard.

### Sur plusieurs appareils {#on-several-computers}

Un numéro de facture ne doit jamais être donné deux fois. Quand vous [partagez entre vos appareils](sharing.md), un seul appareil numérote les factures : un autre dit « Les factures sont numérotées sur … », et **Faire les factures sur cet appareil** les reprend, après une minute et demie, une fois que vos autres appareils le savent. Si votre dossier de partage ne peut pas être écrit, ou si un autre appareil ne s’est pas manifesté depuis quelques minutes, Sioul attend plutôt que de risquer un numéro en double.

Chaque appareil dit aussi aux autres le dernier numéro qu’il a donné dans chaque série, juste après chaque facture et quand Sioul se ferme, si bien qu’un appareil dont les factures ne sont pas encore arrivées ne redonne jamais l’un de ces numéros. Un appareil qui partage, mais laisse de côté **Brouillons et factures**, n’émet aucune facture : il le dit, et vous les émettez sur un appareil qui les partage. Un appareil dont le partage ne peut pas lire sa clé, par exemple quand le trousseau est verrouillé, n’en émet pas non plus : il attend de pouvoir la lire. Le partage allumé, `sioul invoices issue` ne numérote une facture que tant que la fenêtre de Sioul tourne sur cet ordinateur et y garde les factures, selon les mêmes règles que la fenêtre, et dit aussitôt le nouveau numéro à vos autres appareils.

Un paiement compte une fois, quoi qu’aient fait vos appareils : la case **Payée** cochée sur deux appareils, le même export de la banque pris sur les deux, ou le même paiement (son jour, son montant et son moyen) tapé sur les deux avant qu’ils échangent compte une fois. Chaque appareil écrit ce qui arrive à vos factures dans un fichier à lui, et une facture émise n’est elle-même jamais réécrite. Si deux appareils donnaient malgré tout un même numéro au même moment, `sioul invoices list` le dit, et une ligne calme sur le Porche aussi, pendant vos heures de travail : émettez un avoir pour l’une des deux factures, puis facturez-la à nouveau. Les fiches des clients voyagent avec vos projets, une fois que chacun de vos appareils a un Sioul qui les connaît.

## Pendant le calme {#in-quiet-time}

En dehors des heures de travail, la page Temps attend derrière **Montrer quand même**. Voir [Les heures](hours.md).

## Pas encore là {#not-there-yet}

- **Les factures dans la fenêtre.** Un seul endroit pour toutes les factures et tous les brouillons (Argent ▸ Factures) ; un éditeur pour toute ligne (un forfait, des jours, des frais, des biens), qui dit ce qui manque pendant que vous tapez ; Paramètres ▸ Factures par pays ; les fiches des clients dans Contacts et Projets ; **Écrire le courriel**, le PDF joint ; **Payée…** avec le jour et le moyen de paiement ; **Avoir…** et **Acompte…**. D’ici là, un brouillon fait en ligne de commande ou par un [agent d’IA](ai-agent.md) s’émet en ligne de commande.
- **L’envoi des factures électroniques.** À partir du 1er septembre 2027, une micro-entreprise française, en franchise de TVA aussi, doit émettre ses factures aux entreprises françaises sous forme électronique, par une plateforme agréée, en UBL, en CII ou dans un format mixte comme Factur-X, avec le SIREN du client ; à partir de la même date, les données des ventes aux particuliers et à l’étranger doivent être transmises (e-reporting). Chaque facture que Sioul émet porte déjà sa Factur-X, que la plupart des plateformes acceptent en fichier ; l’envoi par votre plateforme depuis Sioul est prévu avant cette date.
- **En franchise de TVA sans numéro de TVA**, le PDF de vos factures ne porte pas encore de données Factur-X : la norme européenne y demande un numéro de TVA ou fiscal, et la règle française qui dit quel numéro donner à la place reste à lire. Sioul le dit dans la ligne d’état au moment où il émet la facture ; le PDF est une facture complète, comme la loi le demande jusqu’en septembre 2027.
- **Les livres.** Le livre des recettes fait des paiements de vos factures, les déclarations à l’URSSAF par catégorie, et un mot quand vous approchez des seuils de la franchise de TVA ou du régime micro.
- Les devis.
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

Le détail, avec ce qui vient de la recherche et ce qui est une supposition : [les notes de conception (en anglais)](https://sioul.aurelienpierre.com/dev/capacity.html).

### En ligne de commande {#from-the-command-line}

`sioul focus start <task> [--minutes 25]`, `sioul focus status` et `sioul focus stop [--done] [--note "…"]` mènent le minuteur de concentration depuis un terminal ; ses minutes comptent comme celles de la fenêtre.

Vos factures, depuis un terminal :

```
sioul invoices [list] [--client <mots>] [--year 2026] [--status draft|issued|sent|partly-paid|paid|credited|credit-note|to-pay-back]
sioul invoices show <numéro ou brouillon>
sioul invoices export [--year 2026] | --html <numéro> | --pdf <numéro>
sioul invoices draft <projet>
sioul invoices issue <brouillon>
sioul invoices discard <brouillon>
sioul invoices credit <numéro> [--line 2]…
sioul invoices pay <numéro> [--day 2026-11-16] [--amount 120.50] [--means transfer|card|cash|cheque|direct-debit|other]
```

`list` donne d’abord vos brouillons, puis vos factures, de la plus récente à la plus ancienne, chacune avec où elle en est ; la liste dit aussi un numéro donné deux fois, un fichier écrit par un Sioul plus récent, et un brouillon qui est une copie qu’a faite votre application de synchronisation d’un autre brouillon. `show` donne les lignes, les totaux et les paiements d’une facture, son client et s’il est facturé comme une entreprise, un particulier ou un organisme public, le jour ou la période de la prestation, et pour un brouillon ce qui lui manque encore avant de pouvoir l’émettre. `export` écrit toutes les factures en tableur (le nom d’un client comme du simple texte, jamais comme une formule), la page à imprimer d’une facture (`--html`), ou met son fichier, tel qu’émis, là où vont vos factures et dit où (`--pdf`). `draft` fait un brouillon du temps d’un projet pas encore facturé : rien n’est numéroté. `issue` numérote un brouillon et le garde avec son fichier, comme le fait la fenêtre : seulement sur l’appareil qui numérote vos factures, et, le partage allumé, seulement tant que la fenêtre de Sioul y tourne ([Sur plusieurs appareils](#on-several-computers)) ; un brouillon n’est jamais émis deux fois, ni la copie d’un brouillon émis depuis. `discard` jette un brouillon : rien n’était numéroté. `credit` fait un brouillon d’avoir, de toutes les lignes d’une facture ou de celles données, que `issue` émet ensuite, jamais pour plus que ce qui reste à créditer. `pay` inscrit un paiement reçu, ou, sur un avoir, de l’argent remboursé : ce qui reste, sauf si vous donnez un montant, et jamais plus que ce qui reste.

## Pourquoi cela fonctionne ainsi {#why-it-works-this-way}

Votre estimation de la durée d’une tâche porte une information, avec un biais : l’erreur typique est de 20 à 30 %, les tâches courtes surestimées, les longues sous-estimées, et le total d’une journée déborde (Halkjelsvik et Jørgensen 2012). On se souvient mal de la durée des tâches passées, et voir les temps mesurés de tâches semblables aide (Roy, Christenfeld et McKenzie 2005). Votre estimation est donc gardée et corrigée dans le plan, jamais remplacée ; les minutes chronométrées comptent plus que celles dont on se souvient ; et la marge est celle de la journée, pas de chaque tâche.

Le droit français donne à chaque facture un numéro d’une série continue au moment où elle est émise, et une facture émise n’est jamais changée : un avoir qui cite son numéro et sa date la corrige (CGI, annexe II, art. 242 nonies A ; CGI, art. 289 ; BOFiP, BOI-TVA-DECLA-30-20-20-10 et -20). Un brouillon n’a donc pas de numéro, l’émission le numérote et le fige, le dernier numéro donné est noté à part des factures elles-mêmes, et une facture erronée trouve sa réponse dans un avoir. Les pays diffèrent par ce qu’une facture doit dire, pas par ce dont elle est faite : la norme européenne de la facture électronique, EN 16931, nomme chacune de ses parties, et les factures électroniques de la France et de l’Allemagne portent exactement ces parties (Factur-X 1.09.2). Un seul enregistrement garde donc une facture dans les termes de cette norme, les règles de chaque pays sont des données, et l’Espagne et l’Italie, dont les factures demandent un logiciel certifié ou le canal de l’État, sont dites hors d’atteinte plutôt que dotées d’un modèle qui y serait illégal.

Un micro-entrepreneur déclare ce que ses clients ont payé, quand ils l’ont payé (URSSAF ; CGI, art. 50-0 et 102 ter), et ce que la loi demande sur une facture à un professionnel (les pénalités, l’indemnité de 40 €) ne l’est pas sur une facture à un particulier (code de commerce, L441-10). Un paiement que montre votre banque est donc inscrit sur sa facture de lui-même, avec Annuler, sans rien attendre de vous ; les mentions pour les professionnels ne vont qu’aux professionnels ; et une facture pas encore payée est dite telle, jamais « en retard », jamais en rouge : les mêmes chiffres en rouge font attendre le pire (Bazley, Cronqvist et Mormann 2021), et l’on détourne les yeux de ses comptes quand ils vont mal (Olafsson et Pagel 2017).

Une facture se garde telle qu’elle a été émise, dix ans, et une entreprise française la recevra bientôt en données (code de commerce, L123-22 ; CGI, art. 289). Le fichier fait à l’émission est donc celui qui est gardé et rendu, jamais un nouveau rendu dans les mots du jour ; c’est un PDF/A-3, fait pour rester lisible, qui porte la même facture en données Factur-X, comme la France et l’Allemagne la lisent ; et là où manque la règle qu’une personne doit lire (quel numéro donne une franchise sans numéro de TVA), Sioul laisse les données de côté et le dit plutôt que deviner. Le code QR de paiement porte les données du virement pour que rien ne soit retapé, et le nom du titulaire du compte lui-même, pour que la vérification du bénéficiaire par la banque du payeur n’alerte pas (EPC069-12 ; règlement (UE) 2024/886).

Plus de détails dans [ce que dit la recherche (en anglais)](https://sioul.aurelienpierre.com/dev/research.html), constats 24 et 76 à 80, et dans [Time estimation (en anglais)](https://sioul.aurelienpierre.com/dev/research/time-estimation.html) et [Invoices (en anglais)](https://sioul.aurelienpierre.com/dev/research/invoicing.html).

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
| Devis, frais ou montants de TVA | en partie¹³ | en partie¹⁴ | ✓ | ✓ | ✓ | ✓¹⁵ |
| Avoirs et factures d’acompte | en partie¹⁶ | ? | ? | ? | ? | ✓¹⁵ |
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
10. Jamais donnés deux fois, aussi entre vos appareils : un seul appareil les numérote, et le dernier numéro donné est noté. Avec le modèle habituel, la série repart chaque année.
11. Le numéro de facture se tape à la main.
12. Automatique par défaut ; il peut se changer.
13. La TVA par taux quand vous y êtes assujetti ; les lignes autres que du temps (un forfait, des jours, des frais) seulement dans le brouillon d’un agent d’IA pour l’instant ; pas de devis.
14. Un champ de taxe, tapé sur la facture.
15. Devis, avoirs et factures d’acompte.
16. Un avoir qui cite la facture, fait en ligne de commande pour l’instant ; les factures d’acompte, et la facture finale qui les déduit, viendront avec l’éditeur.

**La France, et où vivent vos données**

| | Sioul | Toggl Track | Clockify | Harvest | Kimai | Freebe |
|---|---|---|---|---|---|---|
| Les mentions légales françaises imprimées pour vous | ✓¹ | ? | ? | ? | ?² | ✓ |
| Des factures électroniques pour la France (Factur-X, UBL, CII) | en partie³ | ? | ? | ? | en partie⁴ | ✓ |
| Fonctionne sans connexion | ✓ | ✓⁵ | ✓⁵ | ? | ? | ? |
| Votre temps sur vos propres appareils, sans compte | ✓ | ✗ | ✗ | ✗ | en partie⁶ | ✗ |
| Logiciel libre | ✓ | ✗⁷ | ✗⁷ | ✗⁷ | ✓ | ✗⁷ |

1. D’après vos réglages et la fiche du client : « EI » après le nom d’un entrepreneur individuel, le SIREN ou le SIRET, la forme, le capital et le registre d’une société, la mention de la franchise de TVA ou la TVA par taux, le SIREN et le numéro de TVA du client, l’adresse de livraison, la date ou la période de la prestation, la nature de l’opération ; pour les clients professionnels seulement, pas d’escompte pour paiement anticipé, les pénalités de retard et l’indemnité de 40 €. Ce qui manque est dit avant que la facture soit émise.
2. Ses modèles de facture sont à modifier vous-même ; aucun modèle français n’a été confirmé.
3. Chaque facture émise est un PDF/A-3 qui porte sa Factur-X (profil EN 16931), gardé tel qu’émis ; en franchise de TVA sans numéro de TVA, le PDF seul pour l’instant. À partir du 1er septembre 2027, une micro-entreprise française doit émettre ses factures aux entreprises françaises sous forme électronique, par une plateforme agréée : l’y envoyer depuis Sioul viendra plus tard.
4. Avec une extension payante, 99 € par an : Factur-X, ZUGFeRD, XRechnung, UBL et CII.
5. Les applications de bureau de Toggl Track ; Clockify sur Mac, Windows, Linux, Android et iOS. Les deux envoient le temps à leurs serveurs une fois de nouveau en ligne.
6. Sur votre propre serveur, ou dans le nuage de Kimai.
7. Un service hébergé, selon ses propres conditions ; aucune licence libre n’est publiée pour lui.

D’autres en font plus pour la facturation elle-même : la détection d’inactivité (Toggl Track, Clockify) ; les devis, les frais et les liens de paiement, avec la TVA et les avoirs faits dans leur fenêtre (Clockify, Harvest, Kimai, Freebe) ; les factures électroniques de la réforme française (Freebe, et Kimai avec une extension) ; les déclarations à l’URSSAF (Freebe) ; les équipes, les validations et les intégrations (les quatre services commerciaux). Sioul garde ce qui reste à facturer, ne facture chaque heure qu’une fois, numérote sans double même d’un appareil à l’autre, imprime ce que demande votre pays d’après des règles gardées comme des données, inscrit sur chaque facture les paiements que montre votre banque, donne ce même temps au plan de vos journées, et garde le tout sur vos propres appareils.

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
- **Les factures** : un enregistrement chacune (`invoices/<numéro>.toml`), dans les termes de la norme européenne EN 16931 (format 2 depuis novembre 2026 : types 380, 381 et 386, montants en centimes entiers, arrondis au plus loin de zéro), écrit une seule fois et plus jamais ensuite, jamais par-dessus un autre fichier, qui garde vos coordonnées et celles du client telles qu’elles étaient ce jour-là. Ce qui lui arrive ensuite est gardé à part, chaque appareil dans son propre fichier (`invoices/life/<appareil>.toml`) : ses paiements, chacun avec un identifiant fait de ce qu’il dit (tapé à la main : la facture, le jour, le montant et le moyen ; venu de la banque : le compte, le jour et le montant de la ligne, et une empreinte de son libellé), « Payée » à un jour donné, qui couvre ce que les autres laissent, le jour de son envoi, et ce qui a été repris ; lu comme l’union de tous, chaque identifiant une fois, si bien que deux appareils ne changent jamais un même fichier et qu’un paiement indiqué sur deux compte une fois. Les brouillons dans `invoices/drafts/<id>.toml`, nommés par le moment où ils ont été faits (des lettres, des chiffres et des tirets), jamais par un numéro. Un enregistrement fait avant novembre 2026 se lit tel qu’il est et n’est jamais écrit : son propre « payée » compte comme « Payée », qui couvre ce que les autres paiements laissent ; un enregistrement qui ne se lit plus garde quand même son numéro ; un fichier écrit par un Sioul plus récent se lit là où il le peut, sans jamais être réécrit. Le fichier émis, `invoices/<numéro>.pdf`, est un PDF/A-3b écrit en Rust (krilla), en A4 avec des marges de 18 mm, Liberation Sans incorporée, son XML Factur-X joint (`factur-x.xml`, relation Alternative, profil EN 16931) et, en euros, le code QR de l’EPC dessiné en carrés ; son SHA-256 est dans l’enregistrement, et imprimer à nouveau le copie tel quel. Les factures d’avant novembre 2026 sont imprimées par le `QPdfWriter` de Qt à partir d’un `QTextDocument`, comme avant ; des factures d’exemple de chaque pays sont vérifiées en CI par Mustang (veraPDF, les règles d’EN 16931 et celles de la France).
- **La numérotation** : un modèle (`{YYYY}-{N:3}` par défaut) et son recommencement, chaque année ou jamais ; chaque numéro vaut un de plus que le plus grand du journal des séries (`invoices/series/<appareil>.toml`, celui de chaque appareil, le dernier numéro de chaque série, lu comme le plus grand de tous), des enregistrements et du bail de chaque appareil. Un préfixe vide donne l’année et trois chiffres, qui repartent chaque année ; un préfixe à vous continue simplement.
- **Émise tout ou rien** : le numéro, l’enregistrement et son fichier, le journal, le temps marqué et la ligne du budget (pour un avoir, celle de la facture créditée, baissée) s’écrivent ensemble ; une étape qui échoue défait celles d’avant.
- **Les pays** : un fichier de règles par pays (`crates/sioul-core/data/invoice-presets/`), lu par le même code : les identifiants et leur vérification (un SIREN ou un SIRET par sa clé de Luhn), ce qui est exigé, les régimes et les taux de TVA, les mentions avec leurs codes de sujet EN 16931, les délais de paiement, les seuils datés.
- **Facturé une fois** : les enregistrements émis listent les séances qu’ils facturent, et une nouvelle facture laisse de côté toute séance qu’un enregistrement facture ; chaque séance porte aussi le numéro de sa facture, une marque refaite d’après les enregistrements avant chaque émission ; un avoir libère les séances des lignes qu’il annule en entier.
- **Un seul appareil numérote** : un bail dans le dossier de partage (`leases/invoices/<ordinateur>.lease`, scellé comme les enregistrements), renouvelé chaque minute et valable cinq, qui porte le dernier numéro de chaque série, mis à jour aussi juste après chaque émission et quand Sioul se ferme ; un autre appareil n’agit qu’après 90 secondes d’attente, et seulement si les autres appareils se sont manifestés récemment ; la ligne de commande ne numérote que tant que la fenêtre de son ordinateur tient le bail, renouvelé dans les deux dernières minutes et jamais fermé, selon les mêmes règles que la fenêtre ([Le partage](sharing.md#some-things-one-device-at-a-time)).
- **Les tableurs** (le temps facturable, et `sioul invoices export`) : les guillemets de la RFC 4180 ; une cellule qui commence par `=`, `+`, `-`, `@`, une tabulation ou un retour chariot reçoit une apostrophe devant, pour qu’un tableur ne l’exécute jamais comme une formule (injection CSV) ; `;` et la virgule décimale en français.
- **Le partage** : la partie **Temps** porte le temps noté, la séance en cours et les revues du jour ; **Brouillons et factures** porte les enregistrements des factures et ce qui leur arrive ensuite, leurs brouillons et les journaux des séries ; **Projets et argent** porte les fiches des clients (`sioul-clients.toml`), une fois que le Sioul de chaque appareil les connaît. Chaque enregistrement est scellé avec XChaCha20-Poly1305 sous une clé tirée de votre phrase de passe par Argon2id (64 Mio, 3 passes) et gardée dans le trousseau de chaque appareil ([Le partage](sharing.md#what-it-protects-and-what-it-cannot-hide)).
- **Combien de temps prennent les choses, en chiffres** : y = ln(temps passé ÷ première estimation) sur les tâches finies ; les poids diminuent de moitié tous les 4,5 jours ; les minutes tapées pèsent un quart ; rapproché de 1,1× tant qu’il n’y a pas environ neuf tâches, et par domaine, de la vôtre ; robuste aux valeurs aberrantes (trois écarts médians mis à l’échelle, tenu entre ×8 et ÷8) ; chaque jour garde comme temps libre le 85e centile d’une simulation à graine, au plus un tiers de la journée ([les notes de conception, en anglais](https://sioul.aurelienpierre.com/dev/capacity.html)).
