---
description: Les sites dans Sioul – les messageries sécurisées des banques et des organismes, les discussions et les appels vidéo, connectés une fois, leurs notifications gardées pour vos heures ; clés de sécurité et identifiants Bitwarden.
---

# Les sites {#sites}

Certains courriers ne quittent jamais leur site : la messagerie sécurisée d’une banque, celle de l’hôpital, des impôts, de l’assurance maladie. Les discussions vivent aussi dans le navigateur, comme les appels vidéo et les sites de rencontre. La page Sites les garde épinglés, connectés une fois, et fait de leurs notifications quelque chose qui vous attend.

<figure markdown="span">
  [![La page Sites : au-dessus de la liste, un entonnoir, un bouton de tri et un bouton qui replie la liste ; les sites de ces heures-ci (une discussion, avec un point pour ses nouvelles, et des appels vidéo), puis « Autres heures : 3 » ouvert sur une banque, les impôts et une assurance maladie, estompés ; Épingler un site, Sites courants et les réglages de la page en bas.](../assets/screens/fr/sites.png){ loading=lazy }](../assets/screens/fr/sites.png "Ouvrir l’image en grand")
  <figcaption>Messageries sécurisées et discussions, connectées une fois.</figcaption>
</figure>

## Épingler un site {#pinning-a-site}

- **Sites courants ▾** ouvre les sites que l’on garde le plus souvent, environ 400 : par pays (et par État ou par province là où cela compte), puis par groupe : démarches, banques, énergie, téléphone et internet ; et, pour tous, discussions, appels vidéo, réseaux sociaux, rencontres. Chaque groupe commence par **Tous**, qui épingle tout le groupe d’un coup ; un site déjà épinglé est coché et laissé tel quel.
- **Épingler un site** en trouve un par un mot (« banque », « ameli », « discussion »), remplit son nom, son adresse, son type et ce à quoi il sert en un clic, ou prend n’importe quel site à la main : un nom et une adresse `https://`.

Vous vous connectez une fois, sur le site lui-même. Sioul garde la connexion ; il ne garde jamais lui-même le mot de passe du site.

Sur un téléphone, la page Sites liste vos sites et ouvre chacun dans votre navigateur : Sioul n’y garde aucune connexion et n’y regroupe aucune notification. Les sites épinglés sur un ordinateur viennent avec les réglages quand vous [partagez entre vos appareils](sharing.md).

## La liste {#the-list}

- **Les sites de ces heures-ci** viennent d’abord. Les autres se replient sous une ligne, « Autres heures : 3 », qui s’ouvre d’un clic. Voir [Les heures](hours.md).
- **Dans votre ordre** : le menu d’un site ▸ **Monter**, **Descendre**. Le bouton de tri les montre plutôt **groupés par type**.
- **L’entonnoir** (« Quels sites ») filtre selon ce à quoi sert un site (travail, vos démarches, loisirs), selon son type, ou selon l’une de vos catégories ; **Montrer tous les sites** annule le filtre.
- **L’icône de chaque site** est demandée au site lui-même, jamais à un service d’icônes qui apprendrait quels sites vous gardez.
- **La liste se replie sur ses icônes** quand vous voulez plus de place pour le site.

## Un site ouvert {#a-site-open}

Quand un site est ouvert, une rangée au-dessus de lui porte : retour, recharger, **Temps réel** (ses notifications tout de suite), **Couper ses sons**, **Remplir l’identifiant** (depuis Bitwarden, plus bas) et le menu du site, ⋮.

## Le menu d’un site {#a-sites-menu}

Le ⋮ du site ouvert, ou un clic droit sur n’importe quel site de la liste :

- **son type** : espaces personnels (messageries sécurisées, espaces clients), discussions, appels vidéo, réseaux sociaux, rencontres et amitié, autres sites ;
- **Le garder ouvert, pour en être prévenu** : les discussions restent ouvertes par défaut ;
- **Pour** ▸ travail, vos démarches, loisirs, ou plusieurs (si vous ne dites rien, selon le type du site) ;
- **Micro, pour les appels**, **Caméra, pour les appels**, **Partage de l’écran, pour les appels** : activés pour les discussions, les appels vidéo et les sites de rencontre, coupés pour les autres jusqu’à ce que vous les activiez ;
- **Périphériques d’appel…** : la caméra, le micro et le haut-parleur des appels, pris dans la liste de votre système ;
- **Monter**, **Descendre**, **Lier à…** une tâche, une note, un contact ou un projet ;
- **Nom et adresse…** : son nom, son adresse et vos propres catégories (« Banque », « Santé ») ;
- **Retirer ce site** : il quitte Sioul ; rien ne change sur le site lui-même.

<figure markdown="span">
  [![Le menu d’un site ouvert, de haut en bas : les six types, « Discussions » coché ; Le garder ouvert, pour en être prévenu ; Pour ; Micro, Caméra et Partage de l’écran, pour les appels, cochés tous les trois ; Périphériques d’appel… ; Monter ; Descendre ; Lier à… ; Nom et adresse… ; Retirer ce site.](../assets/screens/fr/sites-menu.png){ loading=lazy }](../assets/screens/fr/sites-menu.png "Ouvrir l’image en grand")
  <figcaption>Tout ce qui concerne un site, dans son menu.</figcaption>
</figure>

## Les notifications à votre rythme {#notifications-at-your-pace}

Les notifications de chaque site sont acceptées, puis gardées par Sioul ; le site lui-même ne les montre jamais.

- Du site devant vous : rien de plus.
- D’un site en **Temps réel** (sa case, toujours en vue) : une notification sur le bureau, tout de suite.
- **Un appel** (« appel entrant ») vient tout de suite, sauf si les sons du site sont coupés. Un appel manqué attend comme le reste.
- Tout le reste attend sur [le Porche](porch.md#above-the-lanes), en une ligne « Du nouveau sur » suivie du nom du site. Ouvrir le site efface ses nouvelles.

À heures fixes (09:00, 13:00 et 18:00, sauf si vous en choisissez d’autres dans [Paramètres ▸ Rappels](settings.md#reminders)), une seule notification rassemble les sites qui ont du nouveau, avec **Ouvrir le Porche**. Trois par jour ont le plus aidé dans un essai en conditions réelles (Fitz et al. 2019).

Un site dont les heures ne sont pas venues garde ses notifications, temps réel et appels compris, jusqu’à ce qu’elles viennent.

Un courriel venu du domaine d’un site (« vous avez un nouveau message dans votre espace sécurisé ») reçoit un bouton au-dessus de lui : **Ouvrir** le site.

## Les appels {#calls}

Les discussions, les appels vidéo et les sites de rencontre ont le micro et la caméra. Quand un appel commence, Sioul demande les périphériques choisis dans **Périphériques d’appel…**, et fait passer le son de l’appel par le haut-parleur choisi. **Partager dans l’appel** vous laisse choisir tout un écran, une fenêtre, ou rien. Quand un site demande le micro alors qu’il est coupé, la ligne d’état le dit, avec l’endroit où l’activer. L’endroit où vous êtes n’est jamais donné à aucun site.

## Les clés de sécurité {#security-keys}

Quand un site demande une clé de sécurité (WebAuthn, FIDO2 : une YubiKey, pour GitHub ou Google), la boîte de dialogue propre à Sioul demande lequel des comptes de la clé, demande son code PIN quand elle en a un, dit de toucher la clé, et dit en mots simples pourquoi cela a échoué (clé non enregistrée, code PIN bloqué, délai dépassé), avec **Réessayer**. Les clés d’accès gardées sur la clé fonctionnent. Celles gardées dans un téléphone ou dans le système ne fonctionnent pas, sous Linux et macOS.

## Les identifiants de Bitwarden {#logins-from-bitwarden}

Sioul remplit les identifiants depuis votre coffre Bitwarden, lu par Sioul lui-même : rien d’autre à installer.

1. Dans le ⚙ de la page Sites : l’adresse de votre compte Bitwarden, et son serveur quand ce n’est pas bitwarden.com (bitwarden.eu, ou le vôtre, Vaultwarden aussi).
2. Sur la page de connexion d’un site, **Remplir l’identifiant**.
3. La première fois dans une session, ouvrez le coffre : **Ouvrir avec ma clé de sécurité** (quand Bitwarden connaît la clé comme clé d’accès utilisée pour le chiffrement), ou votre mot de passe principal, puis la deuxième étape que demande votre compte (une clé de sécurité, le code d’une application d’authentification, un code par courriel, un code de récupération).

L’identifiant est écrit dans les champs de la page, seulement sur le site auquel il appartient, seulement quand vous le demandez. Quand un site a plusieurs identifiants, ou aucun, la liste de vos identifiants s’ouvre ; **Choisir un identifiant…** l’ouvre à tout moment. Deux champs les trouvent, chacun facultatif, les deux ensemble quand les deux sont remplis :

- **Site** : le domaine du site pour commencer, sa propre adresse en premier. Changez-le pour une connexion sur un autre site (accounts.google.com, pour un site qui se connecte avec Google), ou tapez des mots : « ameli » trouve ameli.fr et assure.ameli.fr, pas camelia.com ; un identifiant sans site se trouve par son nom. Jamais par un nom d’utilisateur : « gmail » trouve les identifiants de Gmail, pas tous ceux qui ont une adresse Gmail.
- **Nom d’utilisateur** : n’importe quelle partie, seul ou avec un site.

L’identifiant choisi en dernier sur un site vient en premier la fois suivante, son domaine dans Site quand il a été fait pour un autre. Un identifiant fait pour un autre domaine dit lequel, pour qu’un site qui en imite un autre se remarque. Tab passe d’un champ à l’autre, Entrée prend le premier identifiant (ou celui où vous êtes allé avec les flèches), Échap ferme.

Pour le mot de passe d’un compte de courrier (**Mot de passe…** ▸ **Depuis Bitwarden…**, dans la page Comptes), Nom d’utilisateur porte l’adresse du compte et Site reste vide : l’adresse d’un serveur de courrier (imap.gmail.com) est rarement celle où votre fournisseur garde l’identifiant (accounts.google.com).

Sur une page qui demande un code à usage unique, **Remplir l’identifiant** écrit le code que donne en ce moment le secret gardé dans votre coffre.

Votre mot de passe principal et vos clés ne sont jamais gardés. Les identifiants restent en mémoire tant que le coffre est ouvert, jusqu’à la fermeture de Sioul. Rien n’est jamais écrit dans votre coffre.

## Où c’est gardé {#where-it-is-kept}

L’adresse et les choix de chaque site sont dans les réglages de Sioul. Les cookies et les fichiers des sites vivent dans un profil de navigateur propre à Sioul, à part de votre navigateur habituel. Leurs notifications attendent dans le dossier d’état de Sioul jusqu’à ce que vous regardiez.

Quand Sioul se ferme, il ferme chaque site comme un navigateur ferme ses onglets : un site qui garde votre connexion dans la page ouverte, comme Discord, la retrouve la fois suivante.
