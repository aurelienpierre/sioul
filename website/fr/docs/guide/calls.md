---
description: Les appels sur un téléphone Android avec Sioul – qui peut sonner, et quand. Un appel de quelqu’un qui ne peut pas vous joindre maintenant est refusé simplement et va sur votre messagerie ; ce qui sonne toujours ; le filtrage des appels, à part, pour les numéros que personne ne connaît ; J’attends un appel ; qui a appelé, listé calmement sur le Porche ensuite.
---

# Les appels : qui peut sonner, et quand {#calls-who-may-ring-and-when}

Sur un téléphone Android, Sioul peut filtrer vos appels. Un appel de quelqu’un qui ne peut pas vous joindre maintenant est refusé simplement, comme vous le feriez à la main, et votre opérateur l’envoie sur votre messagerie : rien ne sonne, rien ne s’affiche pendant ce temps. Ensuite, le Porche dit qui a appelé, à un moment où ces personnes peuvent vous joindre.

## Le mettre en place {#setting-it-up}

1. **Paramètres ▸ Ce téléphone** (sur le téléphone) dit ce que Sioul fait des appels, ce qu’il ne fait jamais, et ce qui sonne toujours.
2. **Laisser Sioul filtrer les appels** : Android demande « Définir Sioul comme appli numéro de l’appelant et spam par défaut ? ». Répondez oui. Cela ne donne à Sioul aucune autre autorisation. Pour arrêter plus tard : **Applications par défaut d’Android** sur la même page, puis « Aucune ».
3. **Autoriser la lecture des contacts**, si Android ne l’a pas encore demandé : sans elle, Android fait sonner les appels de vos contacts sans demander à Sioul, et Sioul ne filtre que les autres.
4. **Qui peut vous appeler, et quand** : Paramètres ▸ Ce qui vous joint ▸ Par personne ▸ **Appels** ([Ce qui vous joint, et quand](notifications.md#by-person)). Chaque ligne des personnes que vous connaissez (vos contacts sûrs, neutres et restreints) sonne aux moments que dit sa ligne, à travers le travail, les démarches, les loisirs, les repas, le sommeil, la pause, le temps libre, et les deux couches, du temps pour vous et « Ne pas déranger » ; tant que vous êtes [joignable](hours.md#work-now), chacune de ces lignes sonne. Les autres vont sur la messagerie. Les numéros absents de vos contacts et les numéros masqués ont quitté cette grille : ils relèvent du filtrage des appels, à part ([Le filtrage des appels](#the-call-screening)).
5. **Les personnes qui doivent toujours passer** (une aidante, le cabinet de votre médecin) : mettez-les dans votre liste Passent toujours, dans Ce qui vous joint ▸ Exceptions, ou depuis leur fiche, **Comment … vous joint** ([La fiche d’une personne](notifications.md#a-persons-sheet)). Leurs appels sonnent à tout moment, sommeil et pauses compris, sauf si vous les avez bloquées.
6. **Où vont les appels refusés** : **Vérifier \*#67#** et **Vérifier \*#61#** ouvrent l’application Téléphone avec le code tapé ; c’est vous qui appuyez sur appeler. La plupart des opérateurs envoient les appels refusés sur la messagerie.

L’annonce de votre messagerie peut demander d’envoyer plutôt un SMS.

Android 9 ne laisse aucune application filtrer les appels, et Sioul ne peut pas non plus le mettre en silence : il faut Android 10. Le mode « Ne pas déranger » d’Android, réglé dans ses paramètres, peut y garder les appels silencieux ([Ne pas déranger sur tous vos appareils](notifications.md#do-not-disturb-on-every-device)).

## Ce qui sonne toujours {#what-always-rings}

- Les numéros d’urgence, et les secours qui rappellent (en France depuis le 0 800 112 112, ou depuis le numéro d’urgence lui-même).
- Tous les appels pendant un jour après que vous avez appelé un numéro d’urgence, si Android montre à Sioul votre appel sortant (ce qui ne peut pas s’essayer sans en appeler un).
- Un deuxième appel du même numéro en moins de 15 minutes, sauf si vous l’avez bloqué, ou si le filtrage des appels l’écarte quoi qu’il arrive (un automate d’appel, un numéro qui ne peut pas être celui d’un appelant, un numéro que votre réseau n’a pas pu vérifier).
- Les personnes qui passent toujours, sauf si vous les avez bloquées.
- Tout appel que Sioul ne peut pas trancher à temps.

« Ne pas déranger » vient ensuite : tant que l’un des modes de Sioul est activé (une pause, le temps libre, son interrupteur), les appels que Sioul laisse sonner le traversent, ceux de vos contacts tant que ce téléphone filtre les appels, et un deuxième appel en moins de 15 minutes. Les numéros que personne ne connaît ne le traversent jamais, sauf si vous attendez un appel. Votre propre « Ne pas déranger », réglé dans Android, laisse passer qui vous y avez choisi ([Ce que fait chaque système](notifications.md#what-each-system-does)).

## Le filtrage des appels {#the-call-screening}

Les appels des numéros que personne ne connaît (dans aucun de vos carnets d’adresses, ceux de Sioul comme celui du téléphone) et des numéros masqués relèvent du filtrage des appels, à part de « Ne pas déranger » : ni « Ne pas déranger » ni aucun de vos états ne l’arrête. Sur le téléphone, Paramètres ▸ Ce téléphone ▸ Appels ▸ **Filtrage des appels** :

- **Filtrer les appels des numéros que personne ne connaît** : activé, à toute heure d’habitude. Arrêté, ces numéros sonnent quand vos contacts le peuvent : tant que vous êtes [joignable](hours.md#work-now), à toute heure.
- **Quand** : à toute heure, ou seulement en dehors de vos heures de travail et de démarches.
- **Les numéros absents de vos contacts** et **Les numéros masqués** : sur votre messagerie, ils sonnent pendant vos heures de travail et de démarches (le standard d’un hôpital appelle souvent d’un numéro masqué), ou ils sonnent. Tant que vous ne changez aucun réglage du filtrage, ils suivent ce que disaient vos lignes Appels des numéros absents de vos contacts et des numéros masqués ; le premier réglage que vous changez fixe les deux, et aucun préréglage de Ce qui vous joint ne les change.
- **Les appels automatisés** (depuis 2023, une machine ne peut présenter qu’un numéro des racines que l’Arcep réserve aux machines), **les numéros qui ne peuvent pas être ceux d’un appelant** (le plan de numérotation français interdit les numéros commençant par 08 9 comme numéro présenté) et **les numéros que votre réseau n’a pas pu vérifier** : sur votre messagerie à toute heure, un deuxième appel aussi, même quand vous attendez un appel ; ou chacun comme les autres numéros que personne ne connaît.
- **Les numéros que vous avez appelés ces 14 derniers jours** sonnent comme vos contacts, pour que quelqu’un que vous avez appelé puisse vous rappeler. Le téléphone garde les numéros que vous appelez 14 jours, et seulement tant que ce réglage est activé.

Ce que le filtrage laisse sonner ne sonne jamais quand vos contacts attendent : sous « Ne pas déranger », la nuit, pendant une pause ou le temps libre, les numéros que personne ne connaît vont aussi sur votre messagerie, sauf si vous attendez un appel. Les personnes que vous connaissez sonnent comme le disent leurs lignes.

## J’attends un appel {#expecting-a-call}

<span id="let-every-call-through"></span>Quand vous attendez l’appel d’un numéro que vous ne connaissez peut-être pas : un hôpital, une livraison, quelqu’un qui vous rappelle. Les numéros que personne ne connaît, les numéros masqués et les numéros que vous avez appelés ces derniers jours sonnent alors, même quand vos contacts attendent ; vos numéros bloqués, les appels automatisés, les numéros qui ne peuvent pas être ceux d’un appelant et les numéros que votre réseau n’a pas pu vérifier restent à l’écart. Les personnes que vous connaissez sonnent toujours comme le disent leurs lignes : jusqu’au 19 octobre 2026, c’était **Laisser passer tous les appels**, qui faisait sonner tous les appels, ceux des personnes que vous connaissez la nuit compris ; ce n’est plus le cas. Sur la notification discrète de Sioul sur le téléphone, celle qui dit ce qu’est le moment (son canal : « Appareils à jour ») : **J’attends un appel, 1 h**, ou **J’attends un appel** jusqu’à ce que vous l’arrêtiez ; **Filtrer à nouveau** y met fin. Dans la fenêtre, le bouton du téléphone dans la ligne d’état, sur chacun de vos appareils : un clic, pour une heure ; un clic droit, ou un appui long, choisit jusqu’à quand.

## Ensuite, sur le Porche {#afterwards-on-the-porch}

« Pendant votre sommeil, un numéro absent de vos contacts a appelé à 09:30. » Chaque appel attend que la ligne Appels de son auteur le laisse vous joindre, ou vos moments de travail et de démarches pour une ligne qui ne sonne jamais ; jamais compté, sans pastille, jamais notifié. Dessous : **Répondre par SMS**, **Rappeler** et **Vu** ; son **⋮** (ou un appui long, un clic droit, la touche Menu) contient **Ajouter aux contacts**, **Bloquer**, **Comment cette personne vous joint…** et **Pourquoi ?**. Quand le même numéro a rappelé et que cet appel a sonné, la ligne le dit : vous vous êtes peut-être déjà parlé. Sioul ne voit pas votre messagerie, alors il le dit : « Un message a peut-être été laissé. » Le journal d’appels du téléphone liste aussi chaque appel refusé, avec le nom de Sioul.

Si votre opérateur peut vous envoyer chaque message par courriel avec son fichier son (Free le peut : Espace Abonné, Messagerie vocale ▸ Notification, avec fichier audio), nommez le domaine d’où viennent ses courriels dans Paramètres ▸ Mots ▸ **Messagerie vocale par courriel** ([Paramètres](settings.md#words)) : la ligne dit alors « Un message a été laissé (0:42). » avec **Écouter**. Le courriel reste parmi vos courriels.

### Sur vos ordinateurs {#on-your-computers}

Les appels refusés par votre téléphone attendent aussi sur le Porche de vos ordinateurs, aux mêmes moments, dès que vos appareils partagent ([Partager entre vos appareils](sharing.md), la partie **Appels**) : « Pendant votre sommeil, un numéro absent de vos contacts a appelé votre téléphone à 09:30. » **Vu** sur un appareil retire la ligne de tous, avec dix secondes pour annuler. Sur un ordinateur, **Répondre par SMS** et **Rappeler** passent par l’application de votre système pour les liens de téléphone (KDE Connect les confie à votre téléphone), sinon **Copier le numéro** ; **Ajouter aux contacts** ouvre une nouvelle fiche dans Contacts, le numéro rempli. Pour garder les appels hors du Porche de vos ordinateurs : le ⚙ du Porche ▸ Appels ▸ **Appels refusés par vos téléphones**. Pour garder la liste hors d’un ordinateur, éteignez-y la partie **Appels**.

Les appels d’une personne depuis un mois, sonnés ou refusés, sont sur sa fiche : Contacts ▸ sa fiche ▸ **Comment … vous joint**, ou **Comment cette personne vous joint…** sur la ligne d’un appel. Chaque appareil garde un appel un mois, puis le retire.

## Ce que Sioul ne fait jamais {#what-sioul-never-does}

Il ne décroche jamais, n’enregistre jamais un appel, n’écoute jamais, et n’envoie aucun numéro nulle part : aucun serveur, aucune recherche. La liste des appels filtrés par votre téléphone ne voyage que vers vos propres appareils, scellée, tant que la partie **Appels** du partage est activée. Les SMS sont retenus comme les messages des autres applications, selon qui écrit, quand Sioul a l’accès aux notifications d’Android ([Les notifications des autres applications, sur un téléphone](notifications.md#other-apps-on-a-phone)) ; les numéros bloqués d’Android arrêtent les appels et les SMS d’un numéro dans toutes les applications.
