---
description: Les tâches dans Sioul – une seule étape suivante avec sa raison, des tâches qui s’attendent les unes les autres, la journée disposée autour de vos événements, de vos repas et de votre repos, rien en retard, la concentration sans pression, et la fin de la journée.
---

# Les tâches {#tasks}

## En bref {#in-short}

Un plan écrit en liste est un mur : chaque ligne est là d’un coup, et l’étape suivante doit être retrouvée à chaque fois. Sioul fait du plan des tâches qui s’attendent les unes les autres, et montre la seule étape à faire maintenant, avec sa raison. Il dispose votre journée de lui-même, autour de vos événements, de vos repas et de votre repos, dans les heures que vous gardez pour chaque type de travail, et rien n’est jamais en retard. Vous dites comment est la journée, clair, brume ou brouillard, et une journée difficile en tient moins. Vos tâches sont des tâches standard sur votre propre serveur d’agenda, que votre téléphone et vos autres programmes voient.

<figure markdown="span">
  [![La page Tâches sur « Maintenant ». En haut, les vues Maintenant, La journée, Liste, Tableau et Calendrier, puis Routines et trois choix : tous les projets, tous les types, toutes les catégories. Au milieu, une carte, « L’étape suivante » : son titre, le projet dont elle fait partie, pourquoi elle vient maintenant, à peu près combien de temps elle prend et où vous en étiez, avec Commencer, Fait, Pas maintenant et Qu’est-ce qui rend cela difficile ? En dessous, « Ensuite » et l’étape d’après, sa date demandée en jours restants ; puis « Autres choix » et « Faites cette semaine », repliés.](../assets/screens/fr/tasks-now.png){ loading=lazy }](../assets/screens/fr/tasks-now.png "Ouvrir l’image en grand")
  <figcaption>Maintenant : une étape, pourquoi elle vient maintenant, et celle d’après.</figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

- Vos tâches ne vont qu’au serveur d’agenda que vous avez choisi, toujours par une connexion chiffrée : Sioul refuse l’adresse d’un serveur qui n’est pas chiffrée.
- Votre mot de passe reste dans le trousseau de votre système, jamais dans un fichier que Sioul écrit. Avec Google, vous vous connectez sur la page de Google elle-même ; Sioul ne voit jamais votre mot de passe Google.
- Comment s’est passée chaque journée, le temps que vous avez passé et où vous en étiez restent dans des fichiers lisibles sur vos appareils. Entre vos appareils, ils voyagent scellés, par un dossier que votre propre synchronisation transporte : le dossier et son serveur voient que quelque chose a changé, jamais quoi ([Le partage](sharing.md)).
- Une liste peut vivre sur cet appareil seulement. Elle ne rejoint alors vos autres appareils que par ce partage scellé.
- Ce que vous notez sur une tâche (ce qu’elle demande, ce qu’elle apporte, comment ça s’est passé) fait partie de la tâche : c’est donc gardé sur votre serveur d’agenda, comme le reste de la tâche. Choisissez un serveur à qui vous le confiez, ou une liste gardée sur cet appareil.
- GitHub, si vous l’activez, est seulement lu, avec un jeton à vous gardé dans votre trousseau : rien n’est écrit sur GitHub.
- Il n’y a pas de serveur de Sioul. Rien de vos tâches ni de vos journées ne va ailleurs que là où vous l’avez réglé.

## Maintenant {#now}

**Maintenant** montre une étape, pourquoi elle vient maintenant, et celle d’après :

- « Sa date est le 30 octobre : encore trois semaines. »
- « Elle libère deux autres étapes. »
- « Rien ne passe avant elle. »

Quand plusieurs étapes se valent, Sioul en choisit une et le dit ; il ne vous laisse pas un choix à faire. Deux autres choix attendent derrière **Autres choix**. Repliés en dessous : ce qui est commencé, ce que vous pourriez faire **si vous en avez envie**, et ce qui a été fait cette semaine.

Cinq boutons : **Commencer**, **Fait**, **Pas maintenant**, **Détails** (les détails de la tâche, rien ne démarre) et **Qu’est-ce qui rend cela difficile ?**

Finir dit ce que cela a changé, une fois, dans la ligne d’état : « Fait. Cela libère : Envoyer la lettre recommandée. »

## Une nouvelle tâche {#a-new-task}

**Nouvelle tâche**, à côté de la ligne en haut de la page, ouvre à droite le formulaire complet d’une nouvelle tâche (sur un téléphone, il prend la page), son titre d’abord. **Nouveau ▾ ▸ Une tâche** fait de même où que vous soyez, tout comme **Ajouter ▾ ▸ Une tâche** sur un message, un événement, une note, un contact ou un projet, son titre et son lien déjà donnés.

- Tous les champs sont là d’un coup : la liste, les dates, le temps qu’elle prend, ce qu’elle attend, le temps avant et après, ce qu’elle demande et ce qu’elle apporte, les notes.
- La tâche est créée dès qu’elle a un titre : Entrée, ou un autre champ. Ensuite, chaque champ est gardé au fur et à mesure, comme pour toute tâche.
- Fermer le formulaire garde ce que vous avez tapé : avec un titre, la tâche est créée ; sans titre, rien ne l’est. Une tâche créée par erreur se supprime comme une autre, avec **Annuler**.

## Créer une tâche en une ligne {#making-a-task-in-one-line}

La ligne en haut de la page prend une tâche en une phrase, où que vous soyez sur la page :

```
Appeler les impôts demain ~15m #impôts {30/10}
```

En fin de ligne, en français ou en anglais :

| Écrivez | Pour |
|---|---|
| demain, vendredi, semaine prochaine, dans 3 jours, 30/10, 30 octobre (ou en anglais : *tomorrow*, *friday*, *in 3 days*…) | le jour où elle peut commencer |
| `~15m` | le temps qu’elle prend |
| `#impôts` | un projet ou une étiquette |
| `{30/10}` | la date demandée, de l’extérieur : une échéance |
| `@appel`, `@écrire`, `@enligne`, `@dehors`, `@lire`, `@réfléchir`, `@faire` | son type |

Ce que Sioul a compris s’affiche en petites pastilles avant que vous appuyiez sur Entrée. Un premier mot qui le dit clairement donne aussi le type : « Appeler… » est un appel. « Demander… » ne dit rien, car ce peut être un appel ou un message. Entrée crée la tâche et l’ouvre à droite.

Les tâches vivent dans une liste de tâches, sur votre serveur d’agenda ou seulement sur cet appareil. La première fois, **Créer la liste** en crée une en un clic.

## Rien n’est en retard {#nothing-is-overdue}

- Un jour de début passé ne dit rien : la tâche est simplement libre.
- Une date demandée de l’extérieur se dit en temps restant : « Pour le 30 octobre : encore trois semaines ». Une fois passée : « Date demandée : le 2 octobre ». Jamais en rouge, jamais comptée.
- Le plan part toujours d’aujourd’hui. Un nouveau jour commence sans retard accumulé.

**Pas maintenant** est respecté sans discussion. Il remet l’étape à demain, avec les étapes qui y sont liées (celles qui l’attendent, ou qui font partie de la même tâche), pour que l’étape suivante vienne d’autre chose.

Rien ne compte ce que vous n’avez pas fait : ni série, ni score, ni nombre de reports. **Faites cette semaine**, replié sous Maintenant, montre ce qui a été fait.

## Comment est aujourd’hui ? {#how-is-today}

En haut de la page : **Clair**, **Brume** ou **Brouillard**. Il n’y a que vous pour le savoir ; rien n’est deviné d’après ce que vous faites.

- **Brume** garde moins pour aujourd’hui.
- **Brouillard** ne montre que les petites étapes. Une étape fait une journée complète.

La brume et le brouillard abaissent ce que tient la journée autant que sa place ([Ce que tient une journée](#what-a-day-holds)). Dire comment est aujourd’hui ne peut que rendre la journée plus légère, jamais plus pleine.

Une tâche peut aussi dire **ce que ça coûte**, dans son panneau : *léger*, *comme d’habitude*, *lourd*, ou *ça recharge* (une marche, de la musique). Un jour clair prend deux tâches lourdes, un jour de brume une seule, un jour de brouillard aucune. Ce qui recharge ne prend pas de place dans le plan et n’est jamais imposé : après une étape lourde, cela vous est proposé. Dès qu’un de ses coûts est noté ([plus bas](#what-it-costs-and-what-it-gives-back)), le mot suit les notes au lieu d’être choisi.

## Commencer, et s’arrêter {#starting-and-stopping}

Commencer est le plus difficile, alors Sioul y aide. **Commencer** propose d’abord **Juste deux minutes**, puis 15, 25 ou 45 minutes, ou **Sans fin fixée**.

La fenêtre de concentration reste au premier plan pendant que vous travaillez ailleurs. Un disque se vide, d’une couleur neutre, sans tic-tac et sans son. Sous Linux et sur un téléphone, une notification montre aussi le temps qui court, avec **Pause** et **Arrêter** ([Temps](time.md#where-time-comes-from)).

- Pendant qu’elle tourne : **Pause** (**Reprendre** une fois en pause), **Cinq minutes de plus** quand la séance a une fin, et **Arrêter**.
- Deux minutes avant la fin : « Encore deux minutes : le moment de trouver où s’arrêter. »
- À la fin : **Continuer**, ou **S’arrêter ici, cela compte**.
- S’arrêter propose une ligne, **La prochaine fois, commencer par…**, montrée quand la tâche revient.

Le temps est noté pour la tâche, et pour son projet : pour le facturer, et pour apprendre combien de temps les choses prennent vraiment ([Temps](time.md)). Un minuteur oublié est coupé quand vous l’arrêtez, et le moment le dit ([Temps](time.md#where-time-comes-from)). Toutes les 45 minutes, sauf si vous le changez dans [Santé](health.md#moving), une pause pour bouger est proposée : **Faire la pause**, avec une ligne sur où vous en êtes, ou **Pas maintenant** ; manquée, la séance continue de compter.

**Où vous en étiez.** Chaque fois que quelque chose vous interrompt (une pause, un repas, la nuit, la fin de la journée, un minuteur arrêté), vous pouvez laisser une ligne sur où vous en étiez ; **Nouveau ▾ ▸ Où j’en suis…** en laisse une à tout moment. Elle revient en haut du Porche et de la page Tâches, et dans la fenêtre de concentration, jusqu’à ce que vous appuyiez sur **C’est fait**. Quand vous [partagez entre vos appareils](sharing.md), vos autres appareils la montrent aussi.

### Qu’est-ce qui rend cela difficile ? {#what-makes-it-hard}

Cinq réponses : *Je ne sais pas par où commencer*, *C’est trop gros*, *Cela me pèse*, *C’est ennuyeux*, *Pas d’énergie aujourd’hui*. Chacune apporte une aide : les détails de la tâche ouverts, avec ses notes et ses messages ; une étape ajoutée ; deux minutes proposées (**Juste deux minutes**, sous la carte : le temps ne démarre que si vous appuyez) ; ou une journée plus légère. Seul **Commencer** fait démarrer le temps.

## La journée {#the-day}

**La journée** dispose aujourd’hui heure par heure : vos heures selon ce à quoi elles servent (travail, vos démarches, loisirs), les événements à leur heure, vos repas et vos siestes ([Santé](health.md#meals-rest-and-sleep)), et les étapes que le plan donne aujourd’hui dans les heures prévues pour elles, à partir de maintenant, avec une pause entre elles. Le temps gardé avant et après une tâche ou un événement s’affiche à part, « Autour : … ». Une ligne montre l’heure qu’il est ; l’étape en cours est mise en évidence, la suivante entourée d’un trait. Ce que vous avez fini aujourd’hui reste là où cela s’est terminé, coché ✓ et estompé.

La journée suit l’horloge : la ligne avance avec les minutes, et la journée est disposée à nouveau à partir de maintenant toutes les cinq minutes et chaque fois que vous revenez dans Sioul, avec ce que vos autres appareils ont marqué ou noté entre-temps, et aussitôt que quelque chose change ce qu’elle tient ([Le plan](#the-plan)).

C’est une disposition à regarder, jamais un emploi du temps : rien n’est écrit dans les tâches ; une heure à laquelle vous fixez une étape est un événement de votre agenda ([plus bas](#pinned-to-a-time)). Ce qui ne tient pas avant la fin de la journée garde sa place dans le plan, et une ligne le dit.

**Déplacer une étape à la main** : faites-la glisser vers une autre heure d’aujourd’hui. Elle y est fixée ([plus bas](#pinned-to-a-time)) : son créneau va dans votre agenda, aussi long que l’étape que vous avez fait glisser, et la journée dispose les autres étapes autour. Son heure s’affiche en couleur. Clic droit (ou gardez le doigt dessus puis lâchez) pour **Laisser le plan la placer**, qui retire le créneau. Rien d’autre de la tâche ne change : son jour de début et sa date demandée restent tels que vous les avez réglés. Vous pouvez aussi faire glisser un repas ou une sieste (comme sur la page [Santé](health.md#the-day-and-the-week)) et un événement d’un agenda où vous pouvez écrire (comme dans l’[Agenda](agenda.md#moving-an-event-by-hand)). Pendant que vous faites glisser, les nouvelles heures s’affichent, de cinq en cinq minutes ; lâchez, et **Annuler** attend dix secondes. À la souris, appuyez et faites glisser ; sur un écran tactile, gardez le doigt jusqu’à ce que le bloc se soulève, puis faites-le glisser ; un simple balayage fait défiler.

La journée répartit ce qu’elle demande :

- jamais deux étapes lourdes d’affilée ;
- après une étape lourde, une légère, ou d’abord un quart d’heure de pause ;
- deux étapes qui pèsent sur la même chose (réflexion, émotions, anxiété, le corps) sont séparées quand une autre peut venir entre elles.

Elle garde deux moments de **Du temps pour vous**, d’une demi-heure chacun :

- l’un juste après l’étape ou le rendez-vous le plus lourd de la journée ;
- l’autre le soir, une fois vos heures finies.

Ils sont à vous, à remplir ou à laisser vides, et calmes : les messages des sites et les pauses pour bouger attendent qu’ils finissent. Les prises, les repas, les codes demandés, les appels et l’alarme d’un événement passent toujours. Une fois que vous avez dit que certaines choses vous rechargeaient bien ([Comment ça s’est passé ?](#how-was-it)), l’une d’elles peut être proposée, une autre chaque jour. **Du temps pour vous** se désactive dans le ⚙ des tâches.

Après la dernière étape, un peu de temps est **Gardé libre, si des étapes prennent plus longtemps** ([Combien de temps prennent les choses](time.md#how-long-things-take)). Quand quelque chose a changé ce que tient aujourd’hui, une ligne au-dessus de la journée dit pourquoi, en mots.

<figure markdown="span">
  [![La journée : une colonne d’heures avec une fine bande qui marque les heures de travail, de démarches et de loisirs, une ligne à l’heure actuelle, les étapes du jour l’une après l’autre avec leurs horaires, et un appel vidéo à 14:30.](../assets/screens/fr/tasks-day.png){ loading=lazy }](../assets/screens/fr/tasks-day.png "Ouvrir l’image en grand")
  <figcaption>La journée : les événements, et les étapes qui trouvent place autour.</figcaption>
</figure>

## Liste, Tableau, Calendrier {#list-board-timeline}

À un clic de Maintenant :

- **Liste** : chaque tâche ouverte dans l’ordre du plan, une tâche plus grande suivie de ses étapes, groupées **Par projet** ou **Par liste**, avec une recherche, et **Faites aussi** sur demande. Les tâches facultatives viennent en dernier.
- **Tableau** : *Libres de commencer*, *Commencées*, *En attente*, *Faites* (les deux dernières semaines). Les cartes se déplacent par glisser-déposer, à la souris ou au pavé tactile ; sur un écran tactile, glisser fait défiler le tableau. « En attente » est décidé par ce que chaque tâche attend, et chaque carte dit quoi. Au-delà de trois tâches commencées, une ligne demande : « En finir ou en garer une ? »
- **Calendrier** : chaque tâche ouverte sur ses jours, la date demandée en petit losange, les jours sans place grisés. Un projet, ou tous. Le bouton **Calendrier** d’un projet ouvre cette vue, limitée au projet.

<figure markdown="span">
  [![La Liste : un champ de recherche, « Par projet » et « Faites aussi » en haut ; les tâches groupées par projet, une tâche plus grande suivie de ses étapes, chacune avec le temps qu’elle prend, la date demandée en jours restants et ce qu’elle attend ; les tâches sans projet en dessous, et celles « Quand vous le direz » en dernier.](../assets/screens/fr/tasks-list.png){ loading=lazy }](../assets/screens/fr/tasks-list.png "Ouvrir l’image en grand")
  <figcaption>La liste, par projet.</figcaption>
</figure>

Trois choix en haut de la page, **Tous les projets**, **Tous les types** et **Toutes les catégories**, ne montrent que les tâches d’un projet, d’un type ou d’une catégorie, dans chaque vue, Maintenant compris : « le prochain appel ». Le plan reste entier : un appel qui attend un message attend toujours. Sioul se souvient de vos choix.

## Une tâche, ouverte {#a-task-open}

Une tâche s’ouvre à droite avec ses détails, depuis n’importe quelle vue : un clic dessus, **Détails** dans son menu (clic droit, ou appui long sur un écran tactile), ou **Détails** sur la carte de Maintenant. Rien ne démarre sans vous.

- **Commencer**, **Fait**, **Pas maintenant**, **Faire à…** ([fixée à une heure](#pinned-to-a-time)) et **Abandonner** (plus à faire finalement : gardée, barrée, hors du plan ; **Rouvrir** la ramène), puis ce qui compte, en mots : sa date, sa durée, ce qu’elle attend, où vous en étiez, le temps passé ; et, quand il y a quelque chose à dire, **Combien de temps ça prend, d’habitude**, comparé à vos premières estimations ;
- ses étiquettes, **ses étapes** (chacune se coche ici ; leurs minutes s’additionnent), **ce qu’elle attend**, chaque attente avec son délai quand elle en a un (« deux semaines après elle »), et ce qu’elle libère ;
- ses champs en mots, ceux qui sont dits : **Peut commencer le**, **Avant** et **Après**, ce qu’elle demande et ce qu’elle apporte, **Ce que ça coûte**, **Projet**, **Facturé**, **Type**, **Pour**, **Demande un bureau ouvert**, **Revient**, **Liste** ; ses notes ;
- **Lié à cela** : le courrier d’où elle vient, ses notes, les personnes, les brouillons, le projet. **Écrire un courriel** commence un message aux personnes qu’elle concerne ; **En faire une note** commence une note liée à elle ; **Lier à…** y lie tout le reste.

**Modifier**, en haut, change les détails en son formulaire, et **Détails** revient aux détails. Chaque champ est gardé au fur et à mesure :

- **ses étapes**, et une de plus en une ligne (« Une étape, en une ligne ») ; **ce qu’elle attend** (« Attend… » : une autre tâche, trouvée par son titre), et, pour la prochaine attente ajoutée, **Puis attendre** : un délai en jours ou en semaines une fois l’autre faite (« la réponse arrive sous deux semaines » ; 0, aucun) ;
- sous **Plus** : **Peut commencer le**, **Date demandée**, **Prend environ**, **Projet**, **Étiquettes**, **Revient** (la répétition), **Avant** et **Après** (le temps de se préparer, d’y aller et d’en revenir : gardé libre autour d’elle dans votre plan, jamais compté comme une pause ; la journée ne coupe jamais une telle tâche en morceaux), ce qu’elle demande et ce qu’elle apporte, **Ce que ça coûte**, **Pour** (travail, vos démarches, loisirs : voir [Heures](hours.md)), **Facturé** ([Temps](time.md)), **Demande un bureau ouvert**, **Liste**, et **Notes, en Markdown**. Choisir une autre **Liste** y déplace la tâche ; quand cette liste ne garderait pas tout (Google Tasks garde moins), Sioul dit quoi, et demande d’abord.

Tout peut devenir une tâche : un message, une ligne d’une note, un événement à préparer.

## La fin de la journée {#the-end-of-the-day}

**Fini pour aujourd’hui** termine la journée de travail, tôt ou non. Il ouvre d’abord une courte feuille :

- ce que vous avez dit du matin, et ce que le plan en a fait : « Ce matin : brume. Le plan a gardé une journée plus légère. » ;
- **La journée a été :** légère, habituelle, lourde ou ressourçante ;
- **Le mélange :** trop plein, juste ce qu’il faut ou trop vide ;
- une note si vous voulez, en Markdown, avec **Aperçu** ;
- vos notes des derniers jours, repliées, avec les mots que vous aviez alors.

Chaque réponse est facultative. **Clore la journée de travail** la clôt sans réponse comme avec ; **Pas maintenant** ne change rien. Rien ne demande s’il faut s’arrêter, et rien ne compare ce qui était prévu à ce qui a été fait. « Annuler » attend dans la ligne d’état.

Puis un écran court, lu en dix secondes, dit :

- quand le travail revient, et que le reste de la journée est à vous ;
- ce qui a été fait ou avancé, s’il y en a ;
- que tout le reste a sa place ;
- la première étape au retour du travail, que vous pouvez dire à votre façon (« Après le petit-déjeuner, ouvrir le formulaire ») ;
- une date demandée d’ici là, s’il y en a une, avec **Demander un délai**, qui écrit un brouillon aux personnes que la tâche concerne, à relire et à envoyer vous-même (rien ne part tout seul), ou que vous laissez ;
- et ce qui passe encore : les codes à usage unique, et les expéditeurs que vos listes laissent alors passer.

Ce qui n’a pas été fait n’est pas compté. Le jour travaillé suivant s’ouvre sur cette première étape. D’ici là, le travail se repose ([le calme](hours.md#quiet-time)). Une pensée qui vient le soir peut se noter en une ligne ; elle attend, hors de vue, le retour du travail.

**À la fin de vos heures**, la ligne d’état propose **Clore la journée de travail**, et une notification discrète dit « Les heures de travail sont finies », une fois, jamais pendant le sommeil ni pendant une réunion. Une journée finie sans le bouton se clôt comme d’habitude ; sa revue reste proposée jusqu’au soir.

**À toute heure**, deux boutons en bas des lieux closent la journée de travail, **Clore la journée de travail** (une liste cochée), et toute la journée, **Clore toute la journée** (une lune), l’un sous l’autre. La ligne d’état ne propose chacun qu’à son heure ; ces boutons sont toujours là, sur un téléphone dans le tiroir des lieux.

**Avant de dormir**, **Clore toute la journée** revient sur toute la journée : travail, démarches et loisirs ([Santé](health.md#before-sleep)).

Vos réponses restent sur vos appareils, dans des fichiers lisibles, et voyagent scellées avec la partie **Temps** du partage. Le mélange est ce dont le plan apprend : quelles journées se sont bien passées, lesquelles ont été trop pleines. Rien n’est montré comme un score.

## Où vivent les tâches {#where-tasks-live}

Les tâches sont des tâches standard, dans des listes de tâches sur votre serveur d’agenda : votre téléphone et vos autres programmes les voient. Sioul écrit ce qui les relie (étapes, attentes, liens, types) dans les termes de la norme elle-même, pour qu’un programme qui garde ce dont il ne se sert pas le garde aussi. Une tâche fixée à une heure a son créneau dans un agenda, comme un événement ordinaire, lié à la tâche dans les deux sens.

Peu d’autres applications de tâches comprennent les attentes entre tâches. La plupart montrent une attente comme une étape de l’autre tâche, et certaines, quand vous y changez une telle tâche, changent l’attente en étape pour de bon, ou la perdent. Quelles applications gardent quoi : [Fonctionne avec](compatibility.md#other-apps-on-the-same-accounts).

Google Tasks garde moins : une liste là-bas grise ce que Google ne garde pas (un jour de début, une durée, un projet, un type, l’attente d’une autre tâche…), en disant pourquoi. Une liste peut aussi vivre seulement sur cet appareil.

## Pendant le calme, et pendant le sommeil {#in-quiet-time-and-while-you-sleep}

Pendant le calme ([Les heures](hours.md#quiet-time)), les pages de tâches ne gardent que ce qui convient au moment présent : pendant les loisirs et un repas, ce qui est à vous ; le travail attend son retour.

Pendant le sommeil ([Les heures](hours.md#sleep) : la nuit, du moment de se détendre au réveil ; une sieste), la page attend derrière une phrase, « Sommeil : rien ne dérange jusqu’à 07:00. », et **Montrer quand même**. Un champ y note une pensée pour plus tard, en une ligne, hors de vue jusqu’au retour du travail. **Nouveau ▾ ▸ Une tâche** montre quand même la page, le formulaire d’une nouvelle tâche ouvert. Les rappels des tâches attendent aussi, jusqu’au réveil.

## Pour aller plus loin {#going-further}

### Fixée à une heure {#pinned-to-a-time}

**Faire à…**, dans les détails d’une tâche, la fixe à un jour et une heure : son créneau, un événement à part entière, va dans votre agenda, « Tâches planifiées », créé la première fois sur le compte de la liste de la tâche (sur cet appareil pour une liste gardée ici ; le ⚙ des tâches peut en nommer un autre). Il dure ce que le plan donne à la tâche, sauf si vous en décidez autrement. Il s’affiche dans tout agenda, celui de votre téléphone aussi, et la tâche et son créneau sont liés dans les deux sens. Ses détails montrent alors une épingle et quand, sous son titre : « Fixée : jeudi 8 octobre, 10:00–10:30 ». Un clic là ouvre le créneau dans l’agenda ; son menu (clic droit, appui long sur un écran tactile, ou **⋯**) propose **Déplacer…** et **Laisser le plan la placer**. Dans la liste et sur le tableau, sa ligne montre une petite épingle et son heure ; Maintenant montre l’épingle quand l’étape suivante est fixée, et la journée en marque son étape.

- **Le plan place la tâche dans son créneau**, quoi qu’il y ait là, et autour ce qui tient encore. Le créneau compte une fois, comme la tâche : jamais comme un événement en plus.
- **Déplacez-le** avec **Changer…**, en faisant glisser l’étape dans [la journée](#the-day), ou en faisant glisser le créneau dans l’[agenda](agenda.md#tasks-pinned-to-a-time) ou dans tout autre programme d’agenda : la tâche suit, à la synchronisation suivante pour un autre programme. Supprimez le créneau n’importe où, et le plan replace la tâche.
- **Laisser le plan la placer** retire le créneau ; **Annuler** attend dix secondes.
- **Le créneau passe et la tâche n’est pas faite** : le plan la replace. Le créneau reste dans votre agenda tel qu’il était ; rien n’est dit, rien n’est compté.
- **Faite ou abandonnée** : un créneau encore à venir part, son temps libéré ; un créneau en cours finit à ce moment-là ; un créneau passé reste, la trace du moment où le travail a été fait. Une tâche supprimée emporte ses créneaux à venir, et **Annuler** ramène les deux.
- **Une tâche qui revient** : un créneau fixe un seul de ses tours ; le tour suivant est planifié comme d’habitude.
- **Pas d’alarme dans le créneau**, sauf si vous en demandez une dans le ⚙ des tâches : Sioul vous le rappelle comme tout événement, et un téléphone vous le rappellerait deux fois.
- **Le créneau garde le titre de la tâche**, changé avec elle.
- **Vos repas** se décalent après un créneau comme après tout événement.

### Ce qu’elle demande, et ce qu’elle apporte {#what-it-costs-and-what-it-gives-back}

Quatre tuiles, dans le formulaire d’une tâche et dans celui d’un événement, deux par deux (l’une sous l’autre sur un téléphone) : **Réflexion**, **Émotions**, **Inquiétude** et **Corps et sens**, puis **Ce que ça apporte** en dessous. Chacune prend un nombre de 0 à 10, comme vous le ressentez, et pose en petit une question sur ce qui compte :

| Tuile | Ce qu’elle demande |
|---|---|
| **Réflexion** | Décider, beaucoup de choses à garder en tête, quelque chose de nouveau à apprendre ? |
| **Émotions** | Parler à quelqu’un, le regard des autres, la peur de décevoir, un souvenir douloureux ? |
| **Inquiétude** | Une échéance, une réponse que vous attendez, le risque de vous tromper ? |
| **Corps et sens** | Bouger, rester debout, le bruit, la foule, les écrans, un trajet ? |
| **Ce que ça apporte** | Est-ce que ça vous apportera quelque chose : du repos, de la joie, du sens ? |

- **Un appui donne une valeur.** Chaque jauge a une petite case pour 0, puis dix cases pour 1 à 10 : touchez celle que vous voulez dire. Touchez-la encore et la tuile dit de nouveau **non noté**. 0 est une note, pas un blanc : il dit *rien*.
- **Au clavier**, Tab passe d’une tuile à l’autre ; un chiffre donne 0 à 9, **+** ou **=** donne 10, les flèches montent ou descendent d’un cran, et Suppr efface.
- **Un mot à côté de chaque nombre** dit ce qu’il vaut, comme sur l’échelle qui sert à noter l’effort dans le sport et dans la gestion de l’énergie : *rien*, *très léger*, *léger*, *modéré* (3 et 4), *lourd* (5 et 6), *très lourd* (7 à 9), *au maximum*. Ce que ça apporte va de *rien* à *pleinement*, en passant par *un peu*, *modérément*, *nettement* et *beaucoup*.
- **Ce que vous avez dit avant.** Si vous avez dit comment cette tâche s’est passée les dernières fois, ou des tâches de son type ([Comment ça s’est passé ?](#how-was-it)), les tuiles non notées montrent ces valeurs en pâle, et une ligne dit d’où elles viennent. **C’est bien ça** les prend toutes ; un appui sur une tuile ne règle que celle-là. Rien de pâle n’est gardé tant que vous ne l’avez pas choisi.
- Chaque valeur est gardée dès que vous la donnez. Dès qu’un coût est noté, **Ce que ça coûte** suit les notes, montré plutôt que choisi : *léger* quand aucun coût ne dépasse 3, *comme d’habitude* jusqu’à 6, *lourd* à partir de 7 ; *ça recharge* quand l’apport est de 5 ou plus et qu’aucun coût ne dépasse 3. C’est ainsi que le plan compte les tâches lourdes qu’un jour peut tenir ([Comment est aujourd’hui ?](#how-is-today)).

Les émotions comptent quand il faut les cacher ou les porter, pas seulement quand elles sont tristes ; l’inquiétude compte l’appréhension avant et après, pas seulement pendant ; le corps et les sens comptent la station debout, le bruit, la lumière et la foule. Les questions demandent ce qui remue les émotions plutôt que de quelle émotion il s’agit, pour qui a du mal à les nommer ([ce que dit la recherche, en anglais](https://aurelienpierre.github.io/sioul/dev/research.html)).

### Comment ça s’est passé ? {#how-was-it}

Quand une tâche est faite, la ligne d’état propose **Comment ça s’est passé ?** un moment ; les détails d’une tâche faite le proposent aussi. Cela ouvre les mêmes tuiles, une fine marque sur chaque jauge là où vous l’aviez prévu. Seul ce que vous touchez est gardé, comme ressenti, à côté de ce que vous aviez prévu ; le reste demeure non noté, jamais recopié de la prévision, et rien de pâle n’y est proposé. Ignorez-le et rien ne change : ce n’est jamais redemandé, jamais compté.

### Les bureaux ont des horaires {#offices-have-hours}

Une tâche marquée **Demande un bureau ouvert** n’est proposée que pendant l’ouverture des bureaux (du lundi au vendredi, de 09:00 à 17:00, sauf si vous le changez dans le ⚙ de la page Tâches) et n’est prévue que ces jours-là. Un samedi, l’étape suivante est une étape que vous pouvez faire. Une tâche peut aussi avoir les horaires de son propre bureau, avec une pause de midi.

### Si vous en avez envie, et quand vous le direz {#if-you-want-and-when-you-say-so}

Une tâche étiquetée `joy` (ce mot anglais, tel quel) est offerte sous **Si vous en avez envie**, jamais proposée comme étape suivante. Une tâche étiquetée `someday` (de même) attend à la fin de la liste, sous **Quand vous le direz**. Aucune des deux ne prend de place dans le plan.

### Le plan {#the-plan}

- **L’ordre** : une tâche ne passe jamais avant ce qu’elle attend. Des tâches qui s’attendent en boucle sont signalées, calmement : « Ces étapes s’attendent l’une l’autre : … L’une d’elles doit passer d’abord. »
- **L’étape suivante**, parmi les tâches libres de commencer : celle que vous avez commencée ; puis celle dont la date vient le plus tôt, en comptant le travail qui attend derrière elle ; puis votre propre ordre ; puis celle qui en libère le plus ; puis la plus petite.
- **Les jours** : chaque tâche va dans les premiers jours qui ont de la place. La place, ce sont vos heures, chaque type pour ses propres tâches : les heures de travail au travail, les heures pour vos démarches à vos démarches ; les loisirs n’ont pas d’heures : ce qui n’est que pour eux ne prend pas de place et n’attend rien ([Heures](hours.md)). Les événements en sont retirés, avec leur **Avant** et leur **Après**, et cinq minutes avant et après chacun ; chaque étape laisse cinq minutes après elle. L’**Avant** et l’**Après** d’une tâche prennent de la place avec elle. Vos repas, vos siestes et votre nuit sont gardés libres aussi ([Santé](health.md#meals-rest-and-sleep)). Une étape d’une heure au plus n’est jamais coupée ; une plus longue est coupée en morceaux d’au moins un quart d’heure. Une tâche avec un **Avant** ou un **Après** n’est jamais coupée : elle va entière sur un jour dont la place la tient, ou a un jour à elle. Sans aucune heure réglée, la place va du lundi au vendredi, de 09:00 à 17:00. Chaque tâche prend sa durée corrigée ([Combien de temps prennent les choses](time.md#how-long-things-take)), et chaque jour garde un peu de temps libre pour les étapes qui débordent, et une demi-heure pour vous. La place d’aujourd’hui commence maintenant, et la brume ou le brouillard la réduisent. Les jours de repos, et aujourd’hui une fois la journée close, n’en ont pas.
- **Vos dates restent les vôtres.** Les jours que donne le plan sont recalculés à chaque fois, et jamais écrits dans vos tâches. Seul ce que vous réglez est gardé : un jour de début, une date demandée, un ordre, et les heures auxquelles vous fixez des tâches, comme événements de votre agenda.
- **Toujours à jour** : le plan est refait aussitôt que quelque chose dont il dépend change (une tâche, sa durée estimée ou le temps noté pour elle, un événement, un repas ou la nuit, vos heures ou vos jours de repos, la météo du jour, **Fini pour aujourd’hui**, ce qu’apportent vos autres appareils ou une synchronisation), ainsi qu’au démarrage de Sioul, toutes les douze heures et à minuit.
- **Quand une date ne tiendra pas**, la tâche le dit une fois : « À ce rythme, le plan finit après le 30 octobre. La faire plus tôt, la réduire ou la confier garderait la date. » Près d’une date demandée, Maintenant dit ce qui tient : « Jusqu’au mercredi 7 octobre : environ 30 min d’étapes, 3 h de place. »

Un rappel vient, une fois, deux jours travaillés avant une date demandée, et quand une attente est finie. Ces deux rappels viennent d’un ordinateur, quand Sioul est ouvert ou, si vous le demandez, fenêtre fermée ; un téléphone ne les dit pas ([Paramètres](settings.md#reminders)).

### Ce que tient une journée {#what-a-day-holds}

Le plan apprend aussi, de vos propres journées, ce que tient une journée pour vous : en réflexion, en émotions, en anxiété, en corps et sens, et en tout. C’est une aide pour planifier, rien de plus : il ne mesure rien de votre santé et ne dit rien de vous.

- **D’où il part.** Tant que vos journées n’en disent pas plus, une journée tient ce qu’elle tenait avant : vos heures de travail habituel, et deux étapes lourdes. **Plus léger** ou **Bien plus léger**, dans le ⚙ des tâches, partent en dessous : pour reprendre après une période difficile, ou avec une maladie qui limite l’énergie.
- **D’où il apprend.** Des journées dont vous avez dit la fin ([La fin de la journée](#the-end-of-the-day)) : de trop, à peu près bien, trop vide. Une journée que vous n’avez pas dite ne compte pour rien ; elle n’est jamais prise pour légère. Ce qu’a tenu chaque journée se compte avec les coûts que vous avez donnés, ou ce que vous avez ressenti quand vous avez dit **Comment ça s’est passé ?**, multipliés par le temps que cela a pris, une demi-heure au moins pour chaque chose.
- **Comment il bouge.**
    - Une journée dite **de trop** après une journée pleine abaisse aussitôt ce que tient une journée, d’un cinquième, sur ce qui était plein.
    - Il ne monte qu’après une semaine de journées pleines qui se sont bien passées, d’un dixième au plus.
    - Trois journées de trop en une semaine le gardent où il est pour un temps.
    - Aucune journée n’est planifiée plus lourde que la plus lourde qui s’est bien passée dans le dernier mois, et un peu plus.
    - Les journées légères qui se sont bien passées disent seulement « au moins cela » : elles ne l’abaissent jamais.
- **Comment le plan s’en sert.** Chaque journée est remplie un peu en dessous de ce qu’elle tient, après ses rendez-vous. Les jours avant et après un rendez-vous lourd en tiennent moins. Une étape trop lourde pour ce que tient encore une journée va à un autre jour ; une plus lourde que n’importe quelle journée a un jour à elle.
- **Des journées égales**, dans le ⚙ des tâches : la charge de la semaine répartie pour que chaque jour en tienne à peu près autant, plutôt que des journées pleines et d’autres vides. Désactivé sauf si vous l’activez. Avec une maladie qui limite l’énergie, voir aussi [Santé](health.md#even-days).
- **Ce qui recharge** a aussi un minimum, appris des journées que vous avez dites à peu près bien. Les moments de temps pour vous restent quoi qu’il soit, et la fin de la journée dit en mots si la journée a rechargé.
- **En mots seulement.** Quand une règle change aujourd’hui, une ligne au-dessus de la journée dit pourquoi : « Aujourd’hui tient un peu moins : hier était trop après une journée pleine. » Jamais un nombre, un pourcentage, une jauge ni du rouge.

Comment chaque règle est réglée, quels nombres viennent de la recherche et lesquels sont des suppositions encore à vérifier : [les notes de conception (en anglais)](https://aurelienpierre.github.io/sioul/dev/capacity.html).

### Routines {#routines}

Une **routine** est une suite que vous refaites souvent, dans le même ordre : se préparer à sortir, commencer la journée de travail, vos heures de démarches. Vous en écrivez les étapes une fois, chacune avec ses minutes ; Sioul les joue ensuite une à une sur un minuteur et dit l’étape suivante avant qu’elle vienne : l’ordre et l’heure ne sont plus à garder en tête. Des adultes autistes qui ont du mal à démarrer et à changer d’activité citent les routines parmi ce qui aide, « pour ne pas avoir à y penser autant » (Buckle et al. 2021). Une routine n’est pas une tâche : rien n’est planifié, compté ni en retard, et vous pouvez vous arrêter à n’importe quelle étape.

**Routines**, en haut de la page des tâches, les liste ; une nouvelle s’écrit une étape par ligne, avec ses minutes : « 10 min Ouvrir le Porche », « Faire un thé 5 ».

Une routine se joue dans une petite fenêtre au premier plan : l’étape, son temps qui se vide d’une couleur neutre, « Ensuite : … », et la routine en points. **Fait**, **Ouvrir** (ce que l’étape ouvre), **+5 min**, **Passer**, **Pause**, **Arrêter**. Rien de tout cela n’est compté.

Quand le temps d’une étape est fini, la suivante ne commence seule que si la routine le dit. Sinon elle attend, et demande sans un son : « Son temps est fini : fait, ou quelques minutes de plus ? »

**La fenêtre d’administration** est une routine faite de ce qui est là maintenant : le Porche, l’étape suivante du plan, et une ligne pour revenir.

### Sons {#sounds}

Le bouton des sons, dans la ligne d’état, joue au choix, jamais de lui-même :

- **Pour se concentrer** : bruit blanc, rose ou brun ;
- **La nature, faite ici** : les vagues sur une plage, la pluie, le vent dans les arbres, les grillons la nuit, un orage au loin ;
- **Vos enregistrements** : tout son mis dans un dossier `sounds` de vos notes (en Ogg, Opus, FLAC ou WAV, qui bouclent sans blanc).

Un seul à la fois, en boucle sans raccord audible, avec un fondu de cinq secondes au début et à la fin. Le bruit a un peu aidé l’attention des personnes ayant des traits de TDAH, et l’a un peu gênée chez les autres (Nigg et al. 2024) : il est proposé, pas imposé. D’une étude à l’autre, les sons de la nature ont fait baisser le stress et amélioré l’humeur (Buxton et al. 2021).

### Les réglages des tâches {#the-tasks-settings}

Le ⚙ en haut de la page :

- **Quand les bureaux sont ouverts**.
- **Types de tâches** : renommés, retirés ou ajoutés ; les tâches gardent le type qu’elles ont.
- **Catégories** : renommées sur toutes les tâches qui les ont, ou retirées de toutes.
- **Listes de tâches** : renommées, ici et sur le serveur ; une liste vide peut être supprimée.
- **Une tâche sans durée compte** tant de minutes.
- **Les nouvelles tâches vont dans** : la liste où va une nouvelle tâche, tapée en une ligne ou dans son formulaire (où une autre peut se choisir).
- **Les créneaux vont dans** : l’agenda des tâches que vous fixez à une heure ; sauf si vous en choisissez un, « Tâches planifiées », créé la première fois avec la liste de chaque tâche (sur cet appareil pour une liste gardée ici, ou pour un compte Google).
- **Une alarme dans chaque créneau** : désactivée sauf si vous l’activez ; cinq minutes avant lui, dans les créneaux faits ou déplacés à partir de là.
- **Ce que tient une journée** ([plus haut](#what-a-day-holds)) :
    - **Partir de** : *Comme maintenant*, *Plus léger* ou *Bien plus léger* ;
    - **Jours dont le plan apprend** : 28 sauf si vous changez, de 14 à 90 ;
    - **Des journées égales** : désactivé sauf si vous l’activez ;
    - **Du temps pour vous** : activé sauf si vous le désactivez.
- **Ce qui est du travail** et **Ce qui est à vous** : les catégories qui disent à quoi sert une tâche ([Heures](hours.md)).
- **Code**, en dernier : **Tickets et pull requests GitHub en tâches**, désactivé sauf si vous l’activez. Les vôtres arrivent dans une liste « GitHub » sur cet appareil, toutes les trente minutes ; rien n’est écrit sur GitHub.

### Ailleurs dans Sioul {#elsewhere-in-sioul}

- **Un agent d’IA que vous connectez** ([Avec un agent d’IA](ai-agent.md)) peut lister vos tâches, en ajouter une et en marquer une faite, dans les fichiers de cet ordinateur ; ce qu’il fait de ce qu’il lit dépend de l’agent.
- **En ligne de commande** : `sioul tasks` et `sioul focus` font ce que fait la page ([plus bas](#from-the-command-line)).

## Pourquoi cela fonctionne ainsi {#why-it-works-this-way}

- Une seule étape suivante, petite, avec un quand : les plans « Quand X, je fais Y » augmentent le passage à l’acte (Gollwitzer & Sheeran 2006). Trop d’options pèsent surtout quand choisir est difficile (Chernev, Böckenholt & Goodman 2015).
- Rien en retard : se pardonner d’avoir procrastiné a réduit la procrastination ensuite (Wohl, Pychyl & Bennett 2010).
- Découper une tâche permet de la commencer, et rend son estimation plus juste (Kruger & Evans 2004).
- Le temps rendu visible, sans tic-tac : des minuteurs visuels ont aidé des enfants avec un TDAH à gérer le temps (Wennberg et al. 2018).
- Une ligne pour noter où l’on s’est arrêté libère l’esprit pour la suite (Leroy & Glomb 2018).
- Certains jours, on peut moins, et il n’y a que vous pour savoir lesquels (Raymaker et al. 2020 ; Chen, Meng & Nie 2026).
- Clore le travail en donnant une place à ce qui reste aide à le laisser le soir (Smit 2016), et une liste pour le lendemain écrite au coucher a aidé à s’endormir plus vite (Scullin et al. 2018). Une réponse par jour sur la façon dont il s’est passé est ce qu’il faut à un plan pour apprendre ce qu’une journée peut tenir, plutôt que ce que vous faites d’habitude.

Plus de détails dans [ce que dit la recherche (en anglais)](https://aurelienpierre.github.io/sioul/dev/research.html), constats 6 à 23.

## Comparé à d’autres applications {#compared-with-other-apps}

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

Things 3 et OmniFocus, qui ne tournent que sur les appareils d’Apple (OmniFocus aussi sur le web), sont décrits en une phrase chacun sous le premier tableau. Ce que font les autres programmes CalDAV des attentes entre tâches est [côté technique](#waits-in-other-caldav-programs).

**Vos tâches**

| | Sioul | Todoist | TickTick | Microsoft To Do | Tasks.org |
|---|---|---|---|---|---|
| Une étape suivante, avec pourquoi elle vient maintenant | ✓ | ✗ | en partie¹ | en partie² | ✗ |
| Des tâches qui en attendent d’autres | ✓ | ✗³ | ✗ | ✗ | ✗⁴ |
| Un jour de début, distinct de la date demandée | ✓ | ✗⁵ | en partie⁶ | ✗ | ✓ |
| Planifie votre journée de lui-même | ✓ | ✗ | en partie⁷ | ✗ | ✗ |
| Une tâche en une ligne tapée | ✓ | ✓ | ✓ | en partie⁸ | en partie⁹ |
| Vues en tableau et en frise chronologique | ✓ | en partie¹⁰ | ✓¹¹ | ✗ | ✗ |
| Un minuteur de concentration | ✓ | ✗¹² | ✓ | ✗ | en partie¹³ |
| Rien de compté contre vous : ni retard en rouge, ni points | ✓ | ✗¹⁴ | ✗¹⁵ | ? | ✗¹⁶ |
| Une revue de fin de journée | ✓ | ✗ | en partie¹⁷ | ✗¹⁸ | ✗ |
| Des rappels selon le lieu | ✗ | ✓¹⁹ | ✓²⁰ | ✗²¹ | ✓ |
| Listes partagées, tâches confiées à d’autres | ✗²² | ✓ | ✓ | ✓ | en partie²³ |
| Des tâches standard sur un serveur que vous choisissez (CalDAV) ; logiciel libre | ✓ | ✗ | ✗²⁴ | ✗²⁵ | ✓ |
| Des tâches chiffrées de bout en bout quand elles se synchronisent | en partie²⁶ | ✗²⁷ | ✗²⁷ | ? | en partie²⁸ |

1. Les « Suggested Tasks » rangent les tâches en quatre groupes où choisir : ajoutées récemment, reportées, en retard depuis longtemps, à venir.
2. Les suggestions de « Ma journée », où choisir ; qu’une raison soit donnée n’est pas confirmé.
3. Seulement par un module d’un tiers, listé dans son annuaire.
4. Demandé sur son outil de suivi depuis 2020.
5. « Todoist does not support start dates that hide your tasks until their start date is due. »
6. Une tâche peut avoir une heure de début et une heure de fin (Premium) ; elle n’est pas cachée jusque-là.
7. Son assistant d’IA peut placer des tâches dans votre temps libre quand vous le demandez, et ne change rien avant que vous confirmiez (Premium).
8. Les mots-dièse deviennent des étiquettes ; les dates tapées dans le titre ne sont pas confirmées.
9. Étiquettes, priorité et dates simples, d’après son propre code ; une lecture plus complète est une demande ouverte.
10. Un tableau, et un calendrier dans l’offre Pro ; pas de frise.
11. La frise (Timeline) est une vue Premium.
12. Sa propre page renvoie à d’autres applications pour un minuteur.
13. Un minuteur par tâche qui note le temps passé ; pas de Pomodoro.
14. Les dates dépassées en rouge ; des points Karma perdus pour les tâches en retard de cinq jours (Karma peut se désactiver).
15. Un groupe « En retard » ; des points de réussite peuvent se perdre quand des tâches sont retardées.
16. Un filtre « En retard » ; les dates dépassées dans la couleur des erreurs (d’après son propre code).
17. Un rappel quotidien pour faire le point et planifier, et un « Summary » de ce qui a été fait.
18. « Ma journée » se vide à minuit ; il n’y a pas de revue.
19. Dans l’offre Pro, sur téléphone.
20. Sur téléphone (son aide dit iOS et Android ; sa page de fonctions dit iOS seulement).
21. Demandé sur le forum de Microsoft lui-même depuis 2019.
22. Sioul n’a pas de partage ni de répartition des tâches à lui.
23. Des listes partagées par son nuage, ou par un serveur Nextcloud, ownCloud ou sabre/dav ; pas de répartition.
24. Il peut montrer des agendas CalDAV ; ses propres tâches ne sont pas offertes en CalDAV.
25. Les tâches vivent dans Exchange Online.
26. Une liste gardée sur cet appareil voyage scellée entre vos appareils ; une liste sur votre serveur CalDAV y est gardée comme le veut la norme, envoyée par une connexion chiffrée mais lisible par qui gère le serveur.
27. Chiffrées pendant le transport et au repos sur les serveurs de l’entreprise ; le chiffrement de bout en bout n’est pas dans ses pages sur la sécurité.
28. Avec EteSync seulement.

Things 3 garde une date de début à part de l’échéance, n’a pas de points, et pas de dépendances : son propre conseil pour une tâche bloquée est une étiquette « Waiting ». OmniFocus a des projets séquentiels, où seule la première action est disponible et les suivantes attendent, des dates de report et prévues à part des échéances, et une synchronisation chiffrée de bout en bout ; les éléments en retard s’affichent en rouge.

**Planifier votre journée**

| | Sioul | Motion | Reclaim.ai | Sunsama | Tiimo |
|---|---|---|---|---|---|
| Place vos tâches dans la journée de lui-même, autour de vos événements | ✓ | ✓ | en partie¹ | en partie² | ✗³ |
| Replanifie de lui-même quand quelque chose change | ✓ | ✓ | ✓ | en partie⁴ | ✗⁵ |
| Des heures pour chaque type de travail (travail, personnel…) | ✓ | ✓ | ✓ | ✓ | ✗ |
| De la place gardée pour les repas, les pauses et le repos | ✓⁶ | en partie⁷ | ✓⁸ | en partie⁹ | ✗ |
| Des tâches qui en attendent d’autres | ✓ | ✓ | ✗ | ✗ | ✗ |
| Une étape suivante, avec pourquoi elle vient maintenant | ✓ | en partie¹⁰ | en partie¹¹ | ✗ | en partie¹² |
| Un plan plus léger les jours difficiles | ✓¹³ | ? | en partie¹⁴ | en partie¹⁵ | en partie¹⁶ |
| Apprend combien de temps vos tâches prennent vraiment | ✓ | ✗ | ✗ | ✓ | ✗ |
| Une revue de fin de journée | ✓ | ✗ | en partie¹⁷ | ✓ | en partie¹⁸ |
| Dates manquées et tâches non faites jamais comptées ni montrées en rouge | ✓ | ✗¹⁹ | ? | ✗²⁰ | ✗²¹ |
| Un minuteur visuel ; des routines jouées pas à pas | ✓ | ✗ | ✗ | en partie²² | ✓ |
| Logiciel libre ; vos données sur vos appareils et vos serveurs (CalDAV) | ✓ | ✗²³ | ✗²³ | ✗²³ | ✗²³ |

1. Reclaim 1.0 place les tâches. Reclaim 2.0, par défaut pour les nouveaux comptes depuis mai 2026, recommande des tâches et ne place que des blocs de concentration, d’habitudes et de marge.
2. Sur demande, pour une tâche ou plusieurs, dans vos plages horaires et autour de vos événements.
3. Son Co-planner propose des heures que vous enregistrez ; la planification entièrement automatique a été écartée à dessein.
4. Seulement ses propres blocs de tâches, quand ils se chevauchent ou finissent tôt ; les réunions ne bougent jamais.
5. Quand vous le demandez au Co-planner.
6. Vos repas, vos siestes et votre nuit, une pause après chaque étape, un quart d’heure après une étape lourde, et deux moments de temps pour vous.
7. « A 15-minute break every 2 hours » et le temps de trajet ; le déjeuner comme un événement fixe.
8. Une habitude de déjeuner dans une plage, des pauses entre les tâches, de quoi souffler après les réunions, des marges pour les trajets.
9. Un battement avant chaque tâche planifiée et des rappels de pause ; pas de règle pour le déjeuner ni pour les trajets.
10. Son agenda montre ce qui vient ensuite ; des raisons ne sont données que pour les tâches qu’il n’a pas pu placer.
11. Reclaim 2.0 suggère trois à cinq tâches « Relevant Now », sans raison pour chacune.
12. Son Co-planner suggère une tâche à faire maintenant.
13. Vous dites comment est la journée (clair, brume ou brouillard), et ce que tient une journée s’apprend de ce que vos soirées disent des journées.
14. Vous pouvez demander à son assistant quelque chose de plus léger.
15. Une limite de charge quotidienne en heures, avec des avertissements.
16. Son Co-planner demande où en est votre énergie ; un journal d’humeur sur iOS.
17. Une revue nocturne de l’agenda et un résumé du matin (2.0) ; un rapport hebdomadaire.
18. Un point « Review today », où vous choisissez ce qui bouge et pouvez noter votre humeur.
19. Un « Past Due » en rouge.
20. Les tâches sont reportées à minuit, avec un compteur des jours de report.
21. Des séries et des trophées, sur iOS.
22. Un minuteur de tâche, un mode concentration et une vue Pomodoro ; pas de routines pas à pas.
23. Des agendas Google, Outlook ou iCloud, pas de CalDAV ; vos données dans le nuage de l’entreprise ; un service propriétaire.

D’autres en font plus par endroits : les rappels selon le lieu, le partage et la répartition des tâches (Todoist, TickTick, Microsoft To Do) ; des tâches chiffrées de bout en bout entre les appareils (Tasks.org avec EteSync, OmniFocus) ; la pause de midi et les trajets protégés de Reclaim.ai dans des agendas partagés ; la limite de charge en heures de Sunsama ; les minuteurs visuels de Tiimo à chaque étape ; des applications sur chaque plateforme et de nombreuses intégrations (Todoist). Sioul prévient aussi, une fois, quand une date demandée ne tiendra pas, comme Motion et Reclaim 2.0 : la ligne sur le rouge et les comptes porte sur la couleur et le compte, pas sur cet avertissement.

??? info "Sources"
    Toutes lues le 8 octobre 2026.

    - **Todoist**, les articles de son centre d’aide sur les dates de début, les dates et heures, Karma, les créneaux, l’ajout rapide, la vue en tableau et les rappels selon le lieu, sa page sur le Pomodoro, la fiche du module Ganttify et ses offres : <https://www.todoist.com/help/articles/does-todoist-support-start-dates-qhqlgZhk>, <https://www.todoist.com/help/articles/introduction-to-dates-and-time-q7VobO>, <https://www.todoist.com/help/todoist/features/introduction-to-karma-OgWkWy>, <https://todoist.com/help/articles/time-blocking-in-todoist-d6Pf1uTpc>, <https://www.todoist.com/help/articles/use-task-quick-add-in-todoist-va4Lhpzz>, <https://www.todoist.com/help/articles/use-the-board-layout-in-todoist-AiAVsyEI>, <https://www.todoist.com/productivity-methods/pomodoro-technique>, <https://www.todoist.com/help/articles/use-location-reminders-in-todoist-uGcwH2AJ6>, <https://www.todoist.com/integrations/apps/ganttify>, <https://www.todoist.com/help/account-and-billing/plans/todoist-plans-pricing-and-billing-faq-Vq2z0HWL6>
    - **TickTick**, son centre d’aide (ses 96 articles parcourus) et sa page des offres : <https://help.ticktick.com/articles/7401564165023727616>, <https://help.ticktick.com/articles/7055782010496745472>, <https://help.ticktick.com/articles/7503016104470511616>, <https://help.ticktick.com/articles/7055782381696843776>, <https://help.ticktick.com/articles/7082302534169133056>, <https://help.ticktick.com/articles/7056594711640801280>, <https://help.ticktick.com/articles/7055782395743567872>, <https://help.ticktick.com/articles/7209388126463066112>, <https://ticktick.com/upgrade>
    - **Microsoft To Do**, les pages d’aide de Microsoft sur « Ma journée », To Do dans Outlook, une tâche glissée dans le calendrier, la répartition dans les listes partagées, et la demande de rappels selon le lieu sur le forum de Microsoft : <https://support.microsoft.com/en-us/todo/my-day-and-suggestions>, <https://support.microsoft.com/en-us/outlook/calendar/manage-tasks-with-to-do-in-outlook>, <https://support.microsoft.com/en-us/outlook/calendar/drag-a-task-to-your-calendar-with-to-do-in-outlook>, <https://support.microsoft.com/en-us/todo/assign-tasks-in-shared-lists>, <https://support.microsoft.com/en-us/todo/welcome-to-microsoft-to-do>, <https://techcommunity.microsoft.com/discussions/to-doinsiders_android/feature-request-location-based-reminders/401454>
    - **Tasks.org**, sa documentation sur la synchronisation et les lieux, ses prix, son billet sur les listes partagées, son outil de suivi et son code source : <https://tasks.org/docs/sync>, <https://tasks.org/docs/location>, <https://tasks.org/pricing>, <https://tasks.org/blog/shared-lists-are-coming>, <https://github.com/tasks/tasks/issues/1202>, <https://github.com/tasks/tasks>
    - **Things 3**, les articles d’aide de Cultured Code : <https://culturedcode.com/things/support/articles/2803579/>, <https://culturedcode.com/things/support/articles/2237050/>
    - **OmniFocus**, le manuel de référence d’OmniFocus 4, version 4.9.3 (« Project Type », « Dates and Times », « Encrypted Sync », « Overdue ») : <https://support.omnigroup.com/documentation/omnifocus/universal/4.9.3/en/print>
    - **Motion**, tout son centre d’aide (176 pages, depuis son index), dont le fonctionnement de la planification automatique, les étapes et blocages des projets, les états des tâches, et ses prix et téléchargements : <https://www.usemotion.com/help/llms.txt>, <https://www.usemotion.com/help/time-management/auto-scheduling/reference-auto-scheduling/how-auto-scheduling-works-behind-the-scenes>, <https://www.usemotion.com/help/project-management/projects/reference-projects/project-stages-and-blockers>, <https://www.usemotion.com/help/project-management/task/reference-tasks/task-states-and-task-types>, <https://www.usemotion.com/pricing>, <https://www.usemotion.com/download>
    - **Reclaim.ai**, tout son centre d’aide (223 articles, depuis son index), dont la présentation des tâches de la version 2.0, les heures et les marges, et ses prix : <https://help.reclaim.ai/llms.txt>, <https://help.reclaim.ai/en/articles/16558552-reclaim-2-0-tasks-overview>, <https://help.reclaim.ai/en/articles/3600766-set-your-working-meeting-personal-custom-hours>, <https://help.reclaim.ai/en/articles/4281992-buffer-time-overview-travel-decompression-and-tasks-habit-breaks>, <https://reclaim.ai/pricing>
    - **Sunsama**, son manuel (105 pages), dont les créneaux, la planification quotidienne, les durées prévues et réelles, le report des tâches, et ses prix : <https://help.sunsama.com/>, <https://help.sunsama.com/docs/usage-guides/timeboxing/timeboxing-auto-scheduling>, <https://help.sunsama.com/docs/usage-guides/daily-planning>, <https://help.sunsama.com/docs/usage-guides/tasks/planned-and-actual-times>, <https://help.sunsama.com/docs/getting-started/basics/task-rollover-and-recurring-tasks-the-basics>, <https://www.sunsama.com/pricing>
    - **Tiimo**, sa FAQ et ses guides, son article sur la conception du Co-planner, et sa page sur les séries : <https://www.tiimoapp.com/faq>, <https://www.tiimoapp.com/resource-hub/ai-co-planner-design>, <https://www.tiimoapp.com/faq/focus-timer>, <https://www.tiimoapp.com/product/streaks>

## Côté technique {#for-technical-readers}

### Dans les normes {#in-the-standards}

Chaque tâche est un `VTODO` (RFC 5545 §3.6.2) dans une liste de tâches CalDAV, un fichier chacune, changé ligne à ligne, pour que ce qu’un autre programme a écrit revienne exactement et qu’un formulaire enregistré sans changement n’écrive rien.

| Quoi | Écrit comme |
|---|---|
| Une étape d’une tâche plus grande | `RELATED-TO;RELTYPE=PARENT:<UID>`, dans l’étape |
| Attend une autre tâche | `RELATED-TO;RELTYPE=DEPENDS-ON:<UID>` (RFC 9253 §5), dans la tâche qui attend |
| Attend, avec un délai (« la réponse vient sous deux semaines ») | `RELATED-TO;RELTYPE=FINISHTOSTART;GAP=P14D:<UID>`, dans la tâche qui vient d’abord (RFC 9253 §4). Écrit par `sioul tasks import`, et par **Puis attendre** dans le formulaire. Lu de tout programme, `NEXT` aussi |
| Peut commencer le ; la date demandée | `DTSTART` ; `DUE` |
| Le temps qu’elle prend | `ESTIMATED-DURATION` (draft-ietf-calext-ical-tasks) |
| Son type ; « demande un bureau ouvert » | `CONCEPT` avec une URI tag (RFC 9253 §8.1, RFC 4151) |
| Son projet | `REFID` (RFC 9253 §8.3) |
| Ses notes, le courrier d’où elle vient, ses brouillons, un événement | `LINK` avec un `LINKREL` `describedby`, `via` ou `related` ; un message en `mid:` (RFC 2392) |
| Les personnes et les bureaux concernés | `CONTACT;ALTREP="sioul:contact/<UID>"` |
| Coûts, apport, notes ressenties, première estimation, temps avant et après, domaine, horaires du bureau, facturé | `X-SIOUL-COST`, `X-SIOUL-GAIN`, `X-SIOUL-FELT-COST`, `X-SIOUL-FELT-GAIN`, `X-SIOUL-ESTIMATE-FIRST`, `X-SIOUL-BEFORE`, `X-SIOUL-AFTER`, `X-SIOUL-AREA`, `X-SIOUL-OFFICE-HOURS`, `X-SIOUL-BILLABLE` |
| Fixée à une heure | un `VEVENT` dans un agenda, lié dans les deux sens : un `LINK` avec un `LINKREL` en URI tag dans chacun, et `X-SIOUL-TASK` dans l’événement |

Retirer une attente dans Sioul retire aussi un `FINISHTOSTART` ou un `NEXT` qu’un autre programme a écrit pour la même paire dans l’autre tâche.

Les nouvelles listes se créent avec `MKCALENDAR` (RFC 4791 §5.3.1) et se renomment avec `PROPPATCH`. Google Tasks passe par l’API REST de Google, puisque Google ne sert aucune tâche en CalDAV.

### Les attentes dans les autres programmes CalDAV {#waits-in-other-caldav-programs}

La RFC 9253 (août 2022) a ajouté au `RELATED-TO` d’iCalendar le type `DEPENDS-ON` et quatre types de relations temporelles (`FINISHTOSTART` et ses semblables, avec un `GAP` facultatif), ainsi que trois nouvelles propriétés : `LINK`, `CONCEPT` et `REFID`. Peu de programmes de tâches s’en servent encore. cfait, un logiciel libre pour le terminal, le bureau et Android, écrit `DEPENDS-ON` depuis novembre 2025. Aucune des dix-sept applications de tâches CalDAV établies qui ont été vérifiées n’a d’attentes entre tâches, et des demandes restent ouvertes depuis des années : depuis 2003 pour Thunderbird, 2017 pour Nextcloud Tasks, 2020 pour Tasks.org. Ce qui n’a été trouvé que dans Sioul, parmi les projets examinés, c’est l’ensemble : `DEPENDS-ON` et `FINISHTOSTART` avec `GAP`, chacun dans le sens que lui donne la RFC 9253 ; `LINK`, `CONCEPT` et `REFID` ; et un plan qui s’organise autour des attentes.

En octobre 2026, d’après le code et l’outil de suivi de chaque programme, sans essai avec Sioul :

| Programme | Plateformes | Étapes (sous-tâches) | Attentes entre tâches |
|---|---|---|---|
| Sioul | Linux, Windows, macOS, Android | ✓ | ✓ |
| cfait | Linux, FreeBSD, Windows, macOS, Android | ✓ | ✓¹ |
| Tasks.org | Android | ✓ | ✗² |
| Nextcloud Tasks | web (dans Nextcloud) | ✓ | ✗³ |
| jtx Board | Android | ✓ | ✗ |
| OpenTasks | Android | en partie⁴ | ✗ |
| Thunderbird | Linux, Windows, macOS | ✗⁵ | ✗⁵ |
| KOrganizer, Merkuro | Linux | ✓ | ✗⁶ |
| Errands | Linux | ✓ | ✗⁷ |
| Vikunja | web | ✓ | en partie⁸ |
| Rappels d’Apple | iOS, iPadOS, macOS | ?⁹ | ✗ |

1. `DEPENDS-ON`, dans la tâche qui attend, comme Sioul l’écrit, depuis la version 0.1.7 (novembre 2025) ; une attente faite dans l’un devrait donc se voir dans l’autre.
2. Demandé depuis 2020. Par DAVx⁵, un type de relation de la RFC 9253 a été réécrit en parent (signalé en 2023).
3. Demandé depuis 2017, et `DEPENDS-ON` nommément depuis juin 2026.
4. Dans son stockage, pas dans ses écrans ; la FAQ de DAVx⁵ dit qu’il ne semble plus développé.
5. Sous-tâches et dépendances demandées depuis 2003. Thunderbird garde telle quelle une relation qu’il ne montre pas.
6. Une demande de dépendances a été refusée comme hors sujet en 2004. KCalendarCore, sous les deux, lit chaque `RELATED-TO` comme le parent.
7. Lit chaque `RELATED-TO` comme le parent, et en réécrit une seule ligne simple quand il enregistre la tâche.
8. Des relations de blocage et de précédence dans Vikunja ; en CalDAV, il n’écrit que parent et enfant, et lit tout autre type comme un parent.
9. Des sous-tâches dans iCloud ; comment elles passent en CalDAV n’est pas documenté.

Vérifiés aussi, avec au plus des étapes et sans attentes : GNOME Evolution, GNOME Endeavour, Planify, Chiri, 2Do (ses « dépendances » sont des titres de tâches écrits dans une note), Fantastical, todoman et org-caldav. Deux petits outils récents portent aussi des dépendances en CalDAV : caldawarrior, un pont pour Taskwarrior (`DEPENDS-ON`), et tasknotes-caldav, une extension de TaskNotes pour Obsidian, qui écrit les types temporels avec `GAP` dans la tâche qui attend, à l’inverse de la RFC 9253 : Sioul lirait une telle attente à l’envers.

**Ce que les autres programmes font d’une attente.** La RFC 5545 (§3.2.15) demande à un programme qui ne connaît pas un type de relation de le lire comme `PARENT` : c’est pourquoi la plupart des applications montrent une attente comme une sous-tâche. KCalendarCore (sous KOrganizer et Merkuro) et Errands lisent chaque `RELATED-TO` comme le parent. DAVx⁵, le pont habituel sur Android, réécrit les types de la RFC 9253 en `PARENT` (pour OpenTasks et Tasks.org) ou les retire (pour jtx Board) quand il renvoie une tâche. Une attente faite dans Sioul peut donc devenir une sous-tâche, ou disparaître, dès qu’une autre application enregistre cette tâche ([Fonctionne avec](compatibility.md#other-apps-on-the-same-accounts)). Google ne sert aucune tâche en CalDAV.

??? info "Sources"
    Toutes lues le 8 octobre 2026, dans le dépôt, la documentation ou l’outil de suivi de chaque programme.

    - **RFC 9253**, « Support for iCalendar Relationships » (août 2022), §4, §5, §6.2, §8 : <https://www.rfc-editor.org/rfc/rfc9253.txt>
    - **RFC 5545**, §3.2.15 (`RELTYPE`) : <https://www.rfc-editor.org/rfc/rfc5545.txt>
    - **cfait**, sa spécification, `src/model/adapter.rs` et son journal des changements (0.1.7), et sa page F-Droid : <https://github.com/trougnouf/cfait>, <https://f-droid.org/packages/com.trougnouf.cfait/>
    - **Tasks.org**, sa documentation sur la synchronisation, `iCalendar.kt`, et les tickets 1202 et 2527 : <https://tasks.org/docs/sync/>, <https://github.com/tasks/tasks>, <https://github.com/tasks/tasks/issues/1202>, <https://github.com/tasks/tasks/issues/2527>
    - **Nextcloud Tasks**, `src/models/task.js`, et les tickets 131 et 3188 : <https://github.com/nextcloud/tasks>, <https://github.com/nextcloud/tasks/issues/131>, <https://github.com/nextcloud/tasks/issues/3188>
    - **jtx Board**, `Relatedto.kt` et son contrat : <https://github.com/TechbeeAT/jtxBoard>
    - **OpenTasks**, son contrat des tâches, et la FAQ de DAVx⁵ sur les tâches : <https://github.com/dmfs/opentasks>, <https://davx5.com/faq/tasks/advanced-task-features>
    - **DAVx⁵**, la correspondance des relations dans synctools (`RelationsBuilder.kt`, `RelationsHandler.kt`, `RelatedToHandler.kt`) : <https://github.com/bitfireAT/davx5-ose>
    - **Thunderbird**, `CalRelation.sys.mjs` et le bogue 194863 : <https://github.com/thunderbird/thunderbird-desktop>, <https://bugzilla.mozilla.org/show_bug.cgi?id=194863>
    - **KOrganizer, Merkuro et KCalendarCore**, `incidence.h`, `icalformat_p.cpp`, `incidencewrapper.cpp`, et le bogue KDE 37011 : <https://invent.kde.org/frameworks/kcalendarcore>, <https://github.com/KDE/korganizer>, <https://github.com/KDE/merkuro>, <https://bugs.kde.org/show_bug.cgi?id=37011>
    - **Errands**, `errands/lib/data.py` : <https://github.com/mrvladus/Errands>
    - **Vikunja**, ses pages sur les relations entre tâches et sur CalDAV, et `pkg/caldav/caldav.go` : <https://vikunja.io/help/task-relations/>, <https://vikunja.io/help/caldav/>, <https://github.com/go-vikunja/vikunja>
    - **Rappels d’Apple**, son guide d’utilisation : <https://support.apple.com/guide/reminders/welcome/mac>
    - **GNOME Evolution**, `e-cal-model.c` et le ticket 838 : <https://github.com/GNOME/evolution>, <https://gitlab.gnome.org/GNOME/evolution/-/issues/838>
    - **GNOME Endeavour**, ses sources et les tickets 347 et 488 : <https://gitlab.gnome.org/World/Endeavour>
    - **Planify**, `core/Objects/Item.vala` : <https://github.com/alainm23/planify>
    - **Chiri**, `src/lib/ical/vtodo.ts` : <https://github.com/chiriapp/chiri>
    - **2Do**, ses pages sur la synchronisation CalDAV et les liens entre tâches : <https://www.2doapp.com/docs/macos/sync-with-caldav>, <https://www.2doapp.com/docs/macos/task-links>
    - **Fantastical**, son aide : <https://flexibits.com/fantastical/help>
    - **todoman**, `todoman/model.py` et le ticket 568 : <https://github.com/pimutils/todoman>, <https://github.com/pimutils/todoman/issues/568>
    - **org-caldav**, `org-caldav.el` : <https://github.com/dengste/org-caldav>
    - **caldawarrior**, son README : <https://github.com/alexandrebarsacq/caldawarrior>
    - **tasknotes-caldav**, `src/caldav/vtodoRelations.ts` : <https://codeberg.org/schobernoise/tasknotes-caldav>
    - **Google Agenda**, son guide CalDAV (« Doesn’t support VTODO or VJOURNAL data ») : <https://developers.google.com/workspace/calendar/caldav/v2/guide>

### Comment le plan est fait {#how-the-plan-is-made}

- **L’ordre** : le tri topologique de Kahn sur les tâches ouvertes ; les boucles trouvées comme composantes fortement connexes (Tarjan 1972) et signalées, calmement. Les tâches liées par des attentes ou par le fait d’être des étapes d’une même tâche forment un courant (union-find) : **Pas maintenant** met de côté tout un courant pour la journée.
- **L’étape suivante**, parmi les tâches libres de commencer : celle qui est commencée ; puis celle dont le début au plus tard vient d’abord (sa date demandée, moins le travail qui attend derrière elle) ; puis votre ordre (`PRIORITY`) ; puis celle qui en libère le plus ; puis la plus petite.
- **Les jours** : chaque tâche va dans le premier jour qui a de la place dans les heures de son type, après ce qu’elle attend (et son délai) ; les événements, les repas, les siestes et la nuit sont retirés d’abord, avec cinq minutes autour de chaque événement et après chaque étape. Chaque jour est rempli à 85 % de ce qu’il tient, coût par coût et en tout ; la brume garde 60 % de la place d’aujourd’hui, le brouillard 30 %.
- **Les durées** : chaque tâche est placée avec sa durée corrigée ([Combien de temps prennent les choses](time.md#how-long-things-take)) ; chaque jour garde comme temps libre le 85e centile d’une simulation à graine de ses étapes (un effet commun à la journée, un par étape), au plus un tiers de la journée. La graine vient de la date et des tâches : chaque appareil dispose la même journée.
- **Ce que tient une journée** est rejoué à partir de vos journées à chaque fois, jamais gardé : le même historique donne le même plan sur chaque appareil. Ses règles, et quels nombres sont des suppositions : [les notes de conception (en anglais)](https://aurelienpierre.github.io/sioul/dev/capacity.html).
- Le plan est refait au démarrage, toutes les douze heures, à minuit, et aussitôt après tout changement dont il dépend, hors du fil de la fenêtre.

### La sécurité, précisément {#security-precisely}

- CalDAV seulement en HTTPS : une adresse `http://` est refusée. TLS par rustls, avec les certificats racines de votre système.
- Les mots de passe dans le trousseau du système : le Secret Service (KWallet, GNOME Keyring) sous Linux, le Trousseau d’accès sous macOS, le Gestionnaire d’identification sous Windows, le KeyStore d’Android.
- Google : OAuth 2.0 pour les applications natives (RFC 8252) avec PKCE (RFC 7636, S256), par une adresse de bouclage sur un port au hasard ; le jeton de rafraîchissement dans le trousseau, le jeton d’accès en mémoire seulement.
- Le partage entre vos appareils : chaque enregistrement scellé avec XChaCha20-Poly1305 (un nonce aléatoire de 24 octets chacun, lié à sa place), sous une clé tirée de votre phrase de passe par Argon2id (64 Mio, 3 passes) et gardée dans le trousseau de chaque appareil. La partie **Temps** porte le temps noté, la séance en cours, les revues du jour, la météo du jour et où vous en étiez ; **Listes gardées ici** porte les listes de tâches gardées sur cet appareil ([Le partage](sharing.md#what-it-protects-and-what-it-cannot-hide)).
- Les fichiers, sous Linux : les revues dans `~/.local/share/sioul/reviews/`, le temps dans `~/.local/share/sioul/time/`, en TOML lisible ; ailleurs, les dossiers de données du système.
- GitHub, quand vous l’activez : son API REST, en lecture seule, avec un jeton à portée limitée gardé dans le trousseau ; les tickets arrivent dans une liste gardée sur cet appareil.

### En ligne de commande {#from-the-command-line}

```
sioul tasks [now]                      the next step, why, and the one after it
sioul tasks list [--by project|list] [--done] [words]
sioul tasks board | timeline [--project <id>] | show <task>
sioul tasks add "Call the tax office tomorrow ~15m #taxes" [--list <account/id>] [--parent <task>] [--after <task>]
sioul tasks done | start | not-now <task>
sioul tasks weather clear|haze|fog
sioul tasks lists | new-list <account|local> <name> [--events]
sioul tasks import <file.toml> [--dry-run] [--no-sync]
sioul focus start <task> [--minutes 25] | status | stop [--done] [--note "…"]
```

Une tâche se nomme par son UID ou le début de son titre. `sioul tasks import` prend un plan écrit dans un fichier TOML, attentes avec délai comprises : [son format (en anglais)](https://aurelienpierre.github.io/sioul/dev/tasks.html#importing).
