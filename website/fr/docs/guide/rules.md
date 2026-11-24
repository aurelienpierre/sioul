---
description: Les règles dans Sioul – « quand ceci arrive, faire cela », une seule liste dans Paramètres ▸ Règles ; une règle faite d’un courriel en deux appuis, ce qu’elle aurait fait ces 30 derniers jours montré avant de l’enregistrer ; chaque action dite une fois avec Annuler ; ce qu’une règle ne fait jamais.
---

# Les règles {#rules}

## En bref {#in-short}

Une règle dit ce que fait Sioul quand quelque chose arrive, en une phrase : « Quand un courriel arrive du domaine impots.example avec « avis » dans son objet : le rattacher au projet Impôts 2025. ». **Paramètres ▸ Règles** les tient toutes en une seule liste, groupées sous ce qui arrive (« Quand un courriel arrive ») ; les règles qui partagent le même « quand » sont montrées ensemble sous lui, chacune par ce qu’elle fait. Chacune de vos règles a son interrupteur, **Modifier**, **✕**, et ce qu’elle a fait dernièrement. **Ajouter une règle…** en commence une de rien ou d’un courriel.

Une règle agit sur ce qui vient après son enregistrement, jamais sur ce qui était là avant. Chaque chose qu’elle fait est dite une fois dans la ligne d’état, avec **Annuler**.

## Protégé par défaut {#what-is-protected}

- **Une règle n’envoie jamais** de courriel, de SMS ni de réponse, **ne déplace jamais d’argent** (un paiement qu’elle compte est une trace à vous, pas un paiement), **ne supprime jamais** rien, ne change jamais une réunion qui a d’autres invités, ni une liste ou un agenda que vous pouvez seulement lire, n’agit jamais sur la santé, et ne change jamais un réglage ni une autre règle. L’éditeur ne le propose pas ; une règle écrite à la main qui le demande est refusée, et le dit.
- **Une règle sur les courriels ne voit que ce que le Porche laisse entrer** : jamais ce qu’il a mis de côté comme dangereux, ni ce que votre filtre à indésirables retient. Une condition sur l’expéditeur d’un courriel ne vaut que pour un expéditeur que quelque chose prouve (la signature de son serveur, votre propre carnet d’adresses) : n’importe qui peut écrire « De : votre banque » dans un courriel.
- **Une fois, sur chaque appareil** : ce qu’une règle crée porte une marque tirée de la règle et du courriel, pour que votre téléphone et votre ordinateur, voyant le même courriel, le créent une seule fois.
- **Rien à valider** : une règle agit, ou elle est éteinte. Ce qu’elle a fait se défait d’un appui, et **Pas pour celui-ci** l’écarte de cette chose pour de bon.
- **Pas de boucle** : une règle n’agit jamais sur ce qu’elle a fait elle-même, et une chaîne de règles (l’une agissant sur ce qu’une autre a fait) s’arrête à trois étapes.

## La liste {#the-list}

Chaque ligne est une phrase. Vos propres règles ont, de gauche à droite, leur interrupteur (éteinte, une règle reste dans la liste et ne fait rien), la phrase, **Modifier** et **✕** ; sur un téléphone, **Modifier** et **Retirer** sont sous la phrase. **✕** retire la règle tout de suite, avec **Annuler** dans la ligne d’état un moment. Sous une règle qui a agi, **Ce qu’elle a fait dernièrement** déplie ses cinq dernières actions, chacune avec son moment, **Annuler** (remis comme avant, tant que rien ne l’a changé depuis) et **Pas pour celui-ci**.

Les règles qui existent déjà ailleurs dans Sioul sont dans la même liste, dites de la même façon, pour que tout ce qui se fait de lui-même soit en un seul endroit :

- **les routes d’un projet**, et celles qui font une tâche de chaque courriel, gardées sur la page du projet ;
- **les règles de courriel d’un budget**, les courriels qui deviennent une ligne d’un budget ;
- **les filtres des courriels**, gardés dans Courriels ⚙ ▸ Filtres, où leur ordre est gardé aussi ;
- **les expéditeurs d’un thème**, « Toujours ce thème pour cet expéditeur », gardés dans Courriels ⚙ ▸ Thèmes.

Chacune dit où elle est gardée, et son lien l’ouvre là ; elle s’y change, comme avant, et s’applique comme avant. Rien d’elle n’est copié dans la liste.

## Faire une règle {#making-a-rule}

**Ajouter une règle…** propose ses départs : **Partir de rien**, ou **D’un courriel**, l’un des 30 derniers jours, trouvé par quelques mots de son objet ou de son expéditeur (les courriels que le Porche a mis de côté n’en sont pas). Partir d’un préréglage, l’une des règles qui viennent avec Sioul, viendra dans une version suivante, et la carte le dit. L’éditeur s’ouvre alors, une carte en deux parties :

- **Quand** : ce qui est surveillé et ce qui lui arrive (dans cette version, un courriel qui arrive), puis ses conditions. Chaque condition est ce qu’elle lit (le domaine de son expéditeur, son objet, ce que Sioul y reconnaît, son projet…), comment elle compare (est, contient, a les mots…) et une valeur, dont le champ suit ce qu’elle est : des mots séparés par des virgules, un domaine, un projet, un thème, un type de pièce jointe, des jours, une heure. À partir de deux conditions, **Toutes les conditions** ou **Une seule suffit**.
- **Alors** : ce qu’elle fait. Dans cette version : rattacher le courriel à un projet, faire une tâche d’un modèle, lui donner un thème, vous le dire, et garder ses pièces jointes dans Papiers. Chaque détail est soit choisi ici, soit pris du courriel (« son objet », « son projet »). Ce qui vient dans une version suivante est dans les listes, grisé, avec « (plus tard) ». Garder des pièces jointes dans Papiers demande lesquelles (ses PDF d’abord), le type du papier, son nom, où `{sender}`, `{month}`, `{year}`, `{subject}` ou `{file}` tiennent la place de ceux du courriel, et pour qui il est ; chaque fichier est gardé une fois, seulement quand l’antivirus l’a trouvé sain, et rattaché à son courriel.

Au-dessus des deux parties, la règle se lit en une phrase à mesure que vous la changez, la même phrase que diront la liste et la ligne d’état. En dessous, **Ce qu’elle aurait fait ces 30 derniers jours** : combien de courriels, les premiers par leur jour, leur expéditeur et leur objet, et ce qu’elle aurait fait de chacun. **Aussi pour ces courriels, une fois chacun** n’est pas coché : cochez-le, et l’enregistrement le fait pour eux aussi. **Enregistrer** garde la règle ; elle agit dès lors.

## D’un courriel {#from-a-message}

Sur n’importe quel courriel, dans le menu ⋮ du lecteur ou le menu de sa ligne sur le Porche : **Faire une règle de ceci…**. L’éditeur s’ouvre avec son « quand » rempli d’après ce courriel : le domaine de son expéditeur (seulement quand quelque chose prouve l’expéditeur), ce que Sioul y reconnaît (un code, une facture, une commande…), son projet s’il en a un. Choisissez ce qu’elle fait, regardez ce qu’elle aurait fait, et enregistrez. Un courriel que le Porche a mis de côté n’a pas cette entrée. **Faire une tâche des courriels comme celui-ci…**, **En faire un filtre…** et **Toujours ce thème pour cet expéditeur** restent où ils sont.

## Préréglages {#presets}

Le second onglet, **Préréglages**, tient les règles qui viennent avec Sioul : ce qu’il fait de lui-même, dit de la même façon, groupé par ce qui arrive. Chacune est une ligne de mots, avec son interrupteur là où un réglage l’active ou l’éteint aujourd’hui ; **Détails** montre ce qui peut encore se changer (les réglages qu’elle a aujourd’hui, changés là et nulle part ailleurs). Chacune est encore faite comme Sioul l’a toujours fait, et sa carte le dit : ses détails deviendront les vôtres, et un préréglage pourra être copié pour en faire le vôtre, dans une version suivante.

## Pourquoi c’est ainsi {#why-it-works-this-way}

- **Une seule liste au lieu de six endroits.** Avant les règles, « quand tel courriel arrive, faire cela » se disait en six endroits, chacun avec sa propre façon de dire « quels courriels » (les routes d’un projet, les règles de courriel d’un budget, les filtres des courriels, les expéditeurs d’un thème, les routes qui font des tâches, l’argent lu dans les courriels). Une seule liste, une phrase pour chacune, montre toute la vérité de ce qui se fait de lui-même ; les endroits qui les gardaient les gardent encore tant que chacun de vos appareils ne connaît pas les règles.
- **Une phrase par règle.** Une règle qu’on peut lire à voix haute est une règle qu’on peut vérifier. Les mêmes mots dans la liste, l’éditeur et la ligne d’état : rien n’est dit de deux façons.
- **Une action par règle.** Une chose arrive, une chose est faite, un seul Annuler. Les règles qui partagent un « quand » sont montrées ensemble, pour que « Quand un courriel de votre opérateur apporte une facture » se lise une fois au-dessus de tout ce qu’elle fait.
- **Désormais, avec le passé montré d’abord.** Une règle qui agirait d’un coup sur des années de courriels vous surprendrait ; montrer ce qu’elle aurait fait ces 30 derniers jours, et laisser le choix non coché, garde la surprise de votre côté.
- **Faite pour converger, pas pour se déclencher.** Une règle est vérifiée comme un état à atteindre (pour chaque courriel qui y répond, son résultat existe), pour qu’un appareil endormi rattrape son retard, et que deux appareils ne le fassent jamais deux fois.
- **Ce que Sioul décide compte jusqu’à ce que vous le corrigiez.** Pas de file de suggestions à approuver : une règle agit, et son Annuler et **Pas pour celui-ci** sont toujours à un appui.
