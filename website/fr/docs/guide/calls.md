---
description: Les appels sur un téléphone Android avec Sioul – qui peut sonner, et quand. Un appel de quelqu’un qui ne peut pas vous joindre maintenant est refusé simplement et va sur votre messagerie ; ce qui sonne toujours ; laisser passer tous les appels quand vous en attendez un ; qui a appelé, listé calmement sur le Porche ensuite.
---

# Les appels : qui peut sonner, et quand {#calls-who-may-ring-and-when}

Sur un téléphone Android, Sioul peut filtrer vos appels. Un appel de quelqu’un qui ne peut pas vous joindre maintenant est refusé simplement, comme vous le feriez à la main, et votre opérateur l’envoie sur votre messagerie : rien ne sonne, rien ne s’affiche pendant ce temps. Ensuite, le Porche dit qui a appelé, à un moment où ces personnes peuvent vous joindre.

## Le mettre en place {#setting-it-up}

1. **Paramètres ▸ Ce téléphone** (sur le téléphone) dit ce que Sioul fait des appels, ce qu’il ne fait jamais, et ce qui sonne toujours.
2. **Laisser Sioul filtrer les appels** : Android demande « Définir Sioul comme appli numéro de l’appelant et spam par défaut ? ». Répondez oui. Cela ne donne à Sioul aucune autre autorisation. Pour arrêter plus tard : **Applications par défaut d’Android** sur la même page, puis « Aucune ».
3. **Autoriser la lecture des contacts**, si Android ne l’a pas encore demandé : sans elle, Android fait sonner les appels de vos contacts sans demander à Sioul, et Sioul ne filtre que les autres.
4. **Qui peut vous appeler, et quand** : Paramètres ▸ Ce qui vous joint ▸ Par personne ▸ **Appels** ([Ce qui vous joint, et quand](notifications.md#by-person)). Chaque ligne (vos contacts sûrs, neutres et restreints, les numéros absents de vos contacts, les numéros masqués) sonne aux moments que dit sa ligne, à travers le travail, les démarches, les loisirs, les repas, le sommeil, la pause, le temps libre, et les deux couches, du temps pour vous et « Ne pas déranger ». Les autres vont sur la messagerie.
5. **Les personnes qui doivent toujours passer** (une aidante, le cabinet de votre médecin) : mettez-les dans votre liste Passent toujours, dans Ce qui vous joint ▸ Exceptions, ou depuis leur fiche, **Comment … vous joint** ([La fiche d’une personne](notifications.md#a-persons-sheet)). Leurs appels sonnent à tout moment, sommeil et pauses compris, sauf si vous les avez bloquées.
6. **Où vont les appels refusés** : **Vérifier \*#67#** et **Vérifier \*#61#** ouvrent l’application Téléphone avec le code tapé ; c’est vous qui appuyez sur appeler. La plupart des opérateurs envoient les appels refusés sur la messagerie.

L’annonce de votre messagerie peut demander d’envoyer plutôt un SMS.

Android 9 ne laisse aucune application filtrer les appels, et Sioul ne peut pas non plus le mettre en silence : il faut Android 10. Le mode « Ne pas déranger » d’Android, réglé dans ses paramètres, peut y garder les appels silencieux ([Ne pas déranger sur tous vos appareils](notifications.md#do-not-disturb-on-every-device)).

## Ce qui sonne toujours {#what-always-rings}

- Les numéros d’urgence, et les secours qui rappellent (en France depuis le 0 800 112 112, ou depuis le numéro d’urgence lui-même).
- Tous les appels pendant un jour après que vous avez appelé un numéro d’urgence, si Android montre à Sioul votre appel sortant (ce qui ne peut pas s’essayer sans en appeler un).
- Un deuxième appel du même numéro en moins de 15 minutes, sauf si vous l’avez bloqué.
- Les personnes qui passent toujours, sauf si vous les avez bloquées.
- Tous les appels tant que **Laisser passer tous les appels** est activé, sauf les numéros que vous avez bloqués.
- Tout appel que Sioul ne peut pas trancher à temps.

« Ne pas déranger » vient ensuite : tant que l’un des modes de Sioul est activé (une pause, le temps libre, son interrupteur), les appels que Sioul laisse sonner le traversent, ceux de vos contacts tant que ce téléphone filtre les appels, et un deuxième appel en moins de 15 minutes. Votre propre « Ne pas déranger », réglé dans Android, laisse passer qui vous y avez choisi ([Ce que fait chaque système](notifications.md#what-each-system-does)).

## Laisser passer tous les appels {#let-every-call-through}

Quand vous attendez un appel. Sur la notification du téléphone « Sioul tient vos appareils à jour » : **Laisser sonner 1 h**, ou **Laisser sonner** jusqu’à ce que vous l’arrêtiez ; **Filtrer à nouveau** y met fin. Dans la fenêtre, le bouton du téléphone dans la ligne d’état, sur chacun de vos appareils : un clic laisse sonner tous les appels pendant une heure ; un clic droit, ou un appui long, choisit jusqu’à quand.

## Ensuite, sur le Porche {#afterwards-on-the-porch}

« Pendant votre sommeil : un numéro absent de vos contacts a appelé à 09:30. » Chaque appel attend que la ligne Appels de son auteur le laisse vous joindre, ou vos moments de travail et de démarches pour une ligne qui ne sonne jamais ; jamais compté, sans pastille. Dessous : **Répondre par SMS**, **Rappeler**, **Ajouter aux contacts**, **Bloquer**, **Pourquoi ?**, **Vu**. Sioul ne voit pas votre messagerie, alors il le dit : « Un message a peut-être été laissé. » Le journal d’appels du téléphone liste aussi chaque appel refusé, avec le nom de Sioul.

Chez Free, réglez votre messagerie pour recevoir chaque message par courriel avec son fichier son (Espace Abonné : Messagerie vocale ▸ Notification, avec fichier audio) : la ligne dit alors « Un message a été laissé (0:42). » avec **Écouter**. Le courriel reste dans votre courrier.

## Ce que Sioul ne fait jamais {#what-sioul-never-does}

Il ne décroche jamais, n’enregistre jamais un appel, n’écoute jamais, et n’envoie aucun numéro nulle part : aucun serveur, aucune recherche. Les SMS sont retenus comme les messages des autres applications, selon qui écrit, quand Sioul a l’accès aux notifications d’Android ([Les notifications des autres applications, sur un téléphone](notifications.md#other-apps-on-a-phone)) ; les numéros bloqués d’Android arrêtent les appels et les SMS d’un numéro dans toutes les applications.
