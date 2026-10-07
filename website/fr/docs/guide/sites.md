---
description: Les sites dans Sioul – les messageries sécurisées des banques et des organismes, les discussions et les appels vidéo, connectés une fois, leurs notifications gardées pour vos heures ; votre clé de sécurité et Bitwarden à l’intérieur ; comparés aux autres applications qui réunissent des services web dans une fenêtre.
---

# Les sites {#sites}

## En bref {#in-short}

Certains courriers ne quittent jamais leur site : la messagerie sécurisée d’une banque, celle de l’hôpital, des impôts, de l’assurance maladie. Les discussions, les appels vidéo et les sites de rencontre vivent aussi dans le navigateur. La page Sites les garde épinglés et connectés, à part de votre navigateur habituel, et retient leurs notifications jusqu’aux heures que vous leur gardez. Votre clé de sécurité marche à l’intérieur, et Bitwarden remplit leurs identifiants quand vous le demandez.

<figure markdown="span">
  [![La page Sites : au-dessus de la liste, un entonnoir, un bouton de tri et un bouton qui replie la liste ; les sites de ces heures-ci (une discussion, avec un point pour ses nouvelles, et des appels vidéo), puis «  Autres heures : 3  » ouvert sur une banque, les impôts et une assurance maladie, estompés ; Épingler un site, Sites courants et les réglages de la page en bas.](../assets/screens/fr/sites.png){ loading=lazy }](../assets/screens/fr/sites.png "Ouvrir l’image en grand")
  <figcaption>Messageries sécurisées et discussions, connectées une fois.</figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

- Vos sites vivent à part de votre navigateur habituel, dans un navigateur propre à Sioul : ce qu’ils gardent (connexions, cookies, fichiers) reste là.
- Un site ne s’épingle qu’à une adresse `https://` : ce que vous y tapez est chiffré en chemin.
- Le micro, la caméra et le partage de l’écran restent coupés pour les banques, les organismes et les autres messageries sécurisées tant que vous ne les activez pas ; un site qui les demande alors est refusé, et la ligne d’état dit où les activer. L’endroit où vous êtes n’est jamais donné à aucun site.
- L’icône de chaque site est demandée au site lui-même, jamais à un service d’icônes qui apprendrait quels sites vous gardez.
- Votre clé de sécurité vous connecte au site qui la demande, que la boîte de dialogue de Sioul nomme ; ses secrets ne quittent jamais la clé.
- Bitwarden : Sioul lit votre coffre lui-même et n’y écrit jamais. Votre mot de passe principal et vos clés ne sont jamais gardés, et un identifiant n’est rempli que quand vous le demandez.

## Épingler un site {#pinning-a-site}

- **Sites courants ▾** ouvre les sites que l’on garde le plus souvent, environ 400 : pour la France, le Canada (avec le Québec) et les États-Unis (État par État là où cela compte), par groupe : démarches, banques, énergie, téléphone et internet ; et, quel que soit votre pays, discussions, appels vidéo, réseaux sociaux, rencontres. Chaque groupe commence par **Tous**, qui épingle tout le groupe d’un coup ; un site déjà épinglé est coché et laissé tel quel. Ailleurs, épinglez vos organismes et votre banque à la main.
- **Épingler un site** en trouve un par un mot («  banque  », «  ameli  », «  discussion  »), remplit son nom, son adresse, son type et ce à quoi il sert en un clic, ou prend n’importe quel site à la main : un nom et une adresse qui commence par `https://`.

Vous vous connectez une fois, sur le site lui-même. Sioul garde la connexion ; il ne garde jamais lui-même le mot de passe du site.

Sur un téléphone, la page Sites liste vos sites et ouvre chacun dans votre navigateur : Sioul n’y garde aucune connexion et n’y regroupe aucune notification. Les sites épinglés sur un ordinateur viennent avec les réglages quand vous [partagez entre vos appareils](sharing.md).

## La liste {#the-list}

- **Les sites de ces heures-ci** viennent d’abord. Les autres se replient sous une ligne, «  Autres heures : 3  », qui s’ouvre d’un clic. Voir [Les heures](hours.md).
- **Dans votre ordre** : le menu d’un site ▸ **Monter**, **Descendre**. Le bouton de tri les montre plutôt **groupés par type**.
- **L’entonnoir** («  Quels sites  ») filtre selon ce à quoi sert un site (travail, vos démarches, loisirs), selon son type, ou selon l’une de vos catégories ; **Montrer tous les sites** annule le filtre.
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
- **Nom et adresse…** : son nom, son adresse et vos propres catégories («  Banque  », «  Santé  ») ;
- **Retirer ce site** : il quitte Sioul ; rien ne change sur le site lui-même.

<figure markdown="span">
  [![Le menu d’un site ouvert, de haut en bas : les six types, «  Discussions  » coché ; Le garder ouvert, pour en être prévenu ; Pour ; Micro, Caméra et Partage de l’écran, pour les appels, cochés tous les trois ; Périphériques d’appel… ; Monter ; Descendre ; Lier à… ; Nom et adresse… ; Retirer ce site.](../assets/screens/fr/sites-menu.png){ loading=lazy }](../assets/screens/fr/sites-menu.png "Ouvrir l’image en grand")
  <figcaption>Tout ce qui concerne un site, dans son menu.</figcaption>
</figure>

## Les notifications à votre rythme {#notifications-at-your-pace}

Les notifications de chaque site sont acceptées, puis gardées par Sioul ; le site lui-même ne les montre jamais.

- Du site devant vous : rien de plus.
- D’un site en **Temps réel** (sa case, toujours en vue) : une notification sur le bureau, tout de suite.
- **Un appel** («  appel entrant  ») vient tout de suite, sauf si les sons du site sont coupés. Un appel manqué attend comme le reste.
- Tout le reste attend sur [le Porche](porch.md#above-the-lanes), en une ligne «  Du nouveau sur  » suivie du nom du site. Ouvrir le site efface ses nouvelles.

À heures fixes (09:00, 13:00 et 18:00, sauf si vous en choisissez d’autres dans [Paramètres ▸ Rappels](settings.md#reminders)), une seule notification rassemble les sites qui ont du nouveau, avec **Ouvrir le Porche**. Trois par jour ont le plus aidé dans un essai en conditions réelles (Fitz et al. 2019).

Un site dont les heures ne sont pas venues garde ses notifications, temps réel et appels compris, jusqu’à ce qu’elles viennent.

Un courriel venu du domaine d’un site («  vous avez un nouveau message dans votre espace sécurisé  ») reçoit un bouton au-dessus de lui : **Ouvrir** le site.

## Les appels {#calls}

Les discussions, les appels vidéo et les sites de rencontre ont le micro et la caméra. Quand un appel commence, Sioul demande les périphériques choisis dans **Périphériques d’appel…**, et fait passer le son de l’appel par le haut-parleur choisi. **Partager dans l’appel** vous laisse choisir tout un écran, une fenêtre, ou rien. Quand un site demande le micro alors qu’il est coupé, la ligne d’état le dit, avec l’endroit où l’activer. L’endroit où vous êtes n’est jamais donné à aucun site.

## Les clés de sécurité {#security-keys}

Quand un site demande votre clé de sécurité (une YubiKey ou une autre clé FIDO2, pour se connecter à GitHub, Google ou Proton), la boîte de dialogue propre à Sioul nomme le site qui la demande, vous laisse choisir parmi les comptes de la clé quand elle en garde plusieurs, demande son code PIN quand elle en a un (ou un nouveau, quand la clé le demande), dit quand la toucher, et dit en mots simples pourquoi cela a échoué (clé non enregistrée, code PIN bloqué, trop tard), avec **Réessayer**. Les clés d’accès gardées sur la clé fonctionnent. Celles gardées dans un téléphone ou dans le système ne fonctionnent pas, sous Linux et macOS ; sous Windows, la boîte de dialogue de Windows prend la clé.

## Les identifiants de Bitwarden {#logins-from-bitwarden}

Sioul remplit les identifiants depuis votre coffre Bitwarden, lu par Sioul lui-même : rien d’autre à installer.

1. Dans le ⚙ de la page Sites : l’adresse de votre compte Bitwarden, et son serveur quand ce n’est pas bitwarden.com (bitwarden.eu, ou le vôtre, Vaultwarden aussi).
2. Sur la page de connexion d’un site, **Remplir l’identifiant**.
3. La première fois dans une session, ouvrez le coffre : avec **votre clé de sécurité** seule (quand Bitwarden la connaît comme clé d’accès utilisée pour le chiffrement), ou avec votre mot de passe principal, puis la deuxième étape que demande votre compte (une clé de sécurité, le code d’une application d’authentification, un code par courriel, le code d’une YubiKey, un code de récupération). Duo ne peut pas servir ici. Le coffre reste ensuite ouvert jusqu’à la fermeture de Sioul.

Sioul remplit tout de suite l’identifiant fait pour le site devant vous, quand c’est le seul. Quand un site a plusieurs identifiants, ou aucun, la liste de vos identifiants s’ouvre ; **Choisir un identifiant…** l’ouvre à tout moment. Un identifiant fait pour un autre site n’est rempli que si vous le choisissez, et la liste dit pour quel site il a été fait, pour qu’un site qui en imite un autre se remarque. Deux champs trouvent les identifiants, chacun facultatif, les deux ensemble quand les deux sont remplis :

- **Site** : le domaine du site pour commencer, sa propre adresse en premier. Changez-le pour une connexion sur un autre site (accounts.google.com, pour un site qui se connecte avec Google), ou tapez des mots : «  ameli  » trouve ameli.fr et assure.ameli.fr, pas camelia.com ; un identifiant sans site se trouve par son nom. Jamais par un nom d’utilisateur : «  gmail  » trouve les identifiants de Gmail, pas tous ceux qui ont une adresse Gmail.
- **Nom d’utilisateur** : n’importe quelle partie, seul ou avec un site.

L’identifiant choisi en dernier sur un site vient en premier la fois suivante, son domaine dans Site quand il a été fait pour un autre. Tab passe d’un champ à l’autre, Entrée prend le premier identifiant (ou celui où vous êtes allé avec les flèches), Échap ferme.

Pour le mot de passe d’un compte de courrier (**Mot de passe…** ▸ **Depuis Bitwarden…**, dans la page Comptes), Nom d’utilisateur porte l’adresse du compte et Site reste vide : l’adresse d’un serveur de courrier (imap.gmail.com) est rarement celle où votre fournisseur garde l’identifiant (accounts.google.com).

Sur une page qui demande un code à usage unique, **Remplir l’identifiant** écrit le code que donne en ce moment le secret gardé dans votre coffre.

Votre mot de passe principal et vos clés ne sont jamais gardés. Les identifiants restent en mémoire tant que le coffre est ouvert, jusqu’à la fermeture de Sioul. Rien n’est jamais écrit dans votre coffre.

## Où c’est gardé {#where-it-is-kept}

L’adresse et les choix de chaque site sont dans les réglages de Sioul. Les cookies et les fichiers des sites vivent dans un profil de navigateur propre à Sioul, à part de votre navigateur habituel. Leurs notifications attendent dans le dossier d’état de Sioul jusqu’à ce que vous regardiez.

Quand Sioul se ferme, il ferme chaque site comme un navigateur ferme ses onglets : un site qui garde votre connexion dans la page ouverte, comme Discord, la retrouve la fois suivante.

## Pour aller plus loin {#going-further}

- **Ce à quoi sert un site** (⋮ ▸ **Pour** : travail, vos démarches, loisirs, ou plusieurs) décide des heures où il vient en avant ([Les heures](hours.md)). Si vous ne dites rien, il suit son type : les messageries sécurisées sont vos démarches ; les discussions, les réseaux sociaux et les rencontres sont des loisirs.
- **Les personnes de vos listes** : quand la notification d’une discussion nomme quelqu’un que vos contacts connaissent, la ligne de cette personne dans [Ce qui vous joint](notifications.md#by-person) peut la retenir plus longtemps, jamais moins : les notifications de discussion d’une personne bloquée ne viennent jamais.
- **Une limite par jour pour les discussions**, sur la page [Santé](health.md#chats) : une fois les minutes choisies écoulées, les discussions sont couvertes et silencieuses pendant le temps choisi.
- **Lier à…** lie un site à une tâche, une note, un contact ou un projet ; le lien ouvre le site.
- **Les fenêtres surgissantes**, comme une fenêtre de connexion ou celle d’un appel, s’ouvrent dans une fenêtre à elles, avec les droits du site qui les a ouvertes.
- **Un seul profil pour tous les sites** : chaque site garde sa propre connexion, mais deux comptes du même service (deux numéros WhatsApp) ne peuvent pas être ouverts côte à côte.
- **Vos sites épinglés voyagent en une seule liste** avec vos réglages : épinglez des sites sur un appareil à la fois, car deux appareils qui épinglent chacun un site avant de s’échanger leurs changements ne gardent que la liste la plus récente.
- **Votre propre liste de sites courants** : la liste est un simple fichier (`sites.json`) que chacun peut corriger sans reconstruire Sioul ; une copie dans `~/.local/share/sioul/presets/` est prise quand elle est plus récente que celle de Sioul. Voir [les notes des développeurs, «  Presets  » (en anglais)](https://aurelienpierre.github.io/sioul/dev/sites.html#presets).
- **Les limites** : se connecter avec Google à l’intérieur d’un site peut être refusé, car Google bloque les connexions dans les navigateurs qu’il prend pour des navigateurs intégrés ; un appel ne peut pas passer en plein écran. Ce qui a été essayé, et avec quels sites : [Fonctionne avec](compatibility.md#sites).

## Comparé à d’autres applications {#compared-with-other-apps}

**Des notifications pour vos heures, et les organismes prêts à épingler.** Les applications qui réunissent des services web dans une fenêtre, et un navigateur avec des conteneurs, comparés. Aucune autre documentation ne décrit des notifications retenues jusqu’à vos heures puis données ensemble à heures fixes : Ferdium et Rambox les coupent selon un horaire, Wavebox pendant un temps fixé. Sioul a les espaces sécurisés des banques, des assurances et des organismes publics prêts à épingler pour la France, le Canada et les États-Unis ; il remplit les identifiants de Bitwarden sans rien à installer ; sa propre boîte de dialogue demande le code PIN d’une clé de sécurité sous Linux, là où Electron, sur lequel Ferdium et Rambox sont construits, refuse une telle demande ; et il garde vos sites à jour entre vos ordinateurs sans compte chez une entreprise. D’autres font plus ailleurs : ils ouvrent deux comptes d’un même service côte à côte, prennent les extensions de navigateur et les bloqueurs de publicité, et le téléphone de Sioul ne fait qu’ouvrir vos sites dans son navigateur.

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

=== "Au quotidien"

    | | Sioul | Ferdium | Rambox | Shift | Wavebox | Firefox + conteneurs |
    |---|---|---|---|---|---|---|
    | Notifications retenues pour vos heures, puis données ensemble à heures fixes | ✓ | en partie¹ | en partie² | ✗³ | en partie⁴ | ? |
    | Les espaces sécurisés des banques, des assurances et des organismes publics, prêts à épingler | ✓⁵ | ✗⁶ | ✗⁶ | ? | ? | — |
    | Une liste toute faite de services à épingler | ✓⁷ | ✓⁷ | ✓⁷ | ✓⁷ | ✓⁷ | — |
    | Micro, caméra et partage de l’écran permis site par site | ✓ | en partie⁸ | en partie⁸ | ✓ | ✓⁹ | ✓ |
    | Deux comptes d’un même service, côte à côte | ✗¹⁰ | ✓ | ✓¹¹ | ✓ | ✓ | ✓ |
    | Extensions de navigateur, blocage des publicités | ✗ | ✗ | ✓¹¹ | ✓ | ✓¹² | ✓ |
    | Sur un téléphone | en partie¹³ | ✗ | ✗ | ✗ | ✗ | en partie¹⁴ |

    1. Une plage «  ne pas déranger  » chaque jour, d’une heure à une autre ; pas de regroupement à heures fixes.
    2. Des heures de travail pour chaque espace de travail, en dehors desquelles ses notifications sont coupées, et un mode Focus ; pas de regroupement à heures fixes.
    3. Des notifications permises ou bloquées application par application.
    4. Un mode Focus pendant un temps fixé, par groupe d’applications.
    5. Pour la France, le Canada (avec le Québec) et les États-Unis ; ailleurs, ajoutez les vôtres à la main.
    6. Aucun parmi ses services tout faits ; n’importe quel site peut s’ajouter à la main.
    7. Sioul environ 400 ; Ferdium 441 et Rambox 782, comptés dans leurs listes ; Wavebox plus de 2 500 et Shift 4 500, d’après eux.
    8. Les notifications et le son service par service, mais pas de réglage de la caméra ni du micro service par service (Rambox les permet pour toute l’application, dans les réglages du système).
    9. La caméra et le micro, site par site.
    10. Un seul profil de navigateur garde tous les sites : chacun garde sa propre connexion, mais pas deux comptes du même service.
    11. Avec la formule Pro.
    12. N’importe quelle extension de Chrome ; pas de bloqueur de publicité à lui.
    13. Vos sites sont listés, et chacun s’ouvre dans le navigateur de votre téléphone.
    14. Firefox tourne sous Android et iOS ; ses conteneurs non.

=== "Technique"

    | | Sioul | Ferdium | Rambox | Shift | Wavebox | Firefox + conteneurs |
    |---|---|---|---|---|---|---|
    | Une clé de sécurité (FIDO2) avec son code PIN, dans les sites | ✓¹ | en partie² | en partie³ | en partie⁴ | ✓⁵ | ✓⁶ |
    | Identifiants remplis depuis Bitwarden | ✓⁷ | ✗⁸ | ✓⁹ | ✓¹⁰ | ✓¹⁰ | ✓¹⁰ |
    | Vos sites sur vos autres ordinateurs, sans compte chez une entreprise | ✓¹¹ | ✓¹² | ✗¹³ | ✗¹³ | ✗¹³ | ✗¹⁴ |
    | Logiciel libre | ✓ GPL-3.0+ | ✓ Apache-2.0 | ✗ | ✗ | ✗ | ✓ MPL-2.0 |

    1. La boîte de dialogue propre à Sioul sous Linux et macOS, avec le code PIN de la clé ; sous Windows, la boîte de dialogue de Windows. Attendu avec GitHub, Google, Proton et Bitwarden ([Fonctionne avec](compatibility.md#sites)).
    2. Electron, sur lequel Ferdium est construit, refuse une demande quand la clé demande son code PIN. Ferdium a ajouté une autre bibliothèque pour les clés d’accès sous Linux dans sa version 7.2.0 ; un signalement ouvert dit qu’une clé FIDO2 ne connecte pas à Microsoft Teams là.
    3. Son aide dit que les clés de sécurité marchent sous Windows, pas entièrement sous macOS ni sur certains systèmes Linux.
    4. Pour l’authentification unique, sous Windows seulement ; décrite comme ne marchant pas en deuxième étape.
    5. «  Exactement comme dans n’importe quel autre navigateur  » ; le code PIN de la clé n’est pas mentionné.
    6. Les clés FIDO2 USB sous macOS et Linux depuis Firefox 114, avec leur code PIN.
    7. Lu par Sioul lui-même, rien à installer, jamais écrit ; le coffre s’ouvre aussi avec votre seule clé de sécurité.
    8. Ses responsables l’ont refusé comme hors de son but.
    9. Par l’extension de Bitwarden, avec la formule Pro.
    10. Par l’extension de Bitwarden.
    11. Scellés, à travers un dossier que transporte votre propre application de synchronisation.
    12. Avec le serveur de Ferdium, que vous faites tourner vous-même.
    13. Avec un compte chez Rambox, Shift ou Wavebox.
    14. Avec un compte Mozilla.

**Les autres applications.** Franz (Windows, macOS, Linux) marche comme Ferdium et Rambox : chaque service dans un conteneur à lui, environ 75 services tout faits, le remplissage par 1Password avec sa formule Pro, un interrupteur qui coupe tous les services, et pas d’heures pour les notifications. Station n’a plus de site pour son produit ; sa dernière version stable date du 1er décembre 2024. Choisir la caméra, le micro et le haut-parleur des appels, ce que fait Sioul, est laissé hors des tableaux : la documentation des autres applications ne dit pas comment elles le font.

??? info "Sources (en anglais)"
    - Ferdium : sa page d’accueil, <https://ferdium.org/>, lu le 8 octobre 2026.
    - Ferdium : questions et réponses, <https://ferdium.org/faq>, lu le 8 octobre 2026.
    - Ferdium : les chaînes anglaises de son interface (version 7.2.3), <https://github.com/ferdium/ferdium-app/blob/develop/src/i18n/locales/en-US.json>, lu le 8 octobre 2026.
    - Ferdium : ses services tout faits (recettes), <https://github.com/ferdium/ferdium-recipes/tree/main/recipes>, lu le 8 octobre 2026.
    - Ferdium : la version 7.2.0, <https://github.com/ferdium/ferdium-app/releases/tag/v7.2.0>, lu le 8 octobre 2026.
    - Ferdium : ticket 2539, une clé FIDO2 et Microsoft Teams sous Linux, <https://github.com/ferdium/ferdium-app/issues/2539>, lu le 8 octobre 2026.
    - Ferdium : ticket 1468, Bitwarden, refusé comme hors de son but, <https://github.com/ferdium/ferdium-app/issues/1468>, lu le 8 octobre 2026.
    - Ferdium : ticket 135, les extensions de navigateur, <https://github.com/ferdium/ferdium-app/issues/135>, lu le 8 octobre 2026.
    - Ferdium : son serveur, <https://github.com/ferdium/ferdium-server>, lu le 8 octobre 2026.
    - Ferdium : son code et sa licence, <https://github.com/ferdium/ferdium-app>, lu le 8 octobre 2026.
    - Electron : demande de fusion 54355, une demande WebAuthn annulée quand la clé demande son code PIN (fusionnée le 25 septembre 2026), <https://github.com/electron/electron/pull/54355>, lu le 8 octobre 2026.
    - Rambox : les fonctions, <https://rambox.app/features/>, lu le 8 octobre 2026.
    - Rambox : les prix, <https://rambox.app/pricing/>, lu le 8 octobre 2026.
    - Rambox : sa liste d’applications, <https://rambox.app/apps/>, lu le 8 octobre 2026.
    - Aide de Rambox : profils et sessions, <https://support.rambox.app/support/solutions/articles/42000094794-profiles-session-management>, lu le 8 octobre 2026.
    - Aide de Rambox : les espaces de travail, <https://support.rambox.app/support/solutions/articles/42000027659-how-to-create-and-configure-workspaces->, lu le 8 octobre 2026.
    - Aide de Rambox : le mode Focus, <https://support.rambox.app/support/solutions/articles/42000027999-focus-mode>, lu le 8 octobre 2026.
    - Aide de Rambox : clés d’accès et authentification à deux facteurs, <https://support.rambox.app/support/solutions/articles/42000110024-passkeys-and-two-factor-authentication-2fa-doesn-t-work->, lu le 8 octobre 2026.
    - Aide de Rambox : les extensions, <https://support.rambox.app/support/solutions/articles/42000092789-extensions-how-to-add-and-configure-them>, lu le 8 octobre 2026.
    - Aide de Rambox : partager sa caméra, son micro ou son écran, <https://support.rambox.app/support/solutions/articles/42000093043-issues-sharing-your-camera-microphone-or-screen->, lu le 8 octobre 2026.
    - Rambox : questions et réponses, <https://rambox.app/faqs/>, lu le 8 octobre 2026.
    - Aide de Rambox : comment marche Rambox, <https://support.rambox.app/support/solutions/articles/42000081279-how-does-rambox-work->, lu le 8 octobre 2026.
    - Rambox : son contrat de licence, <https://rambox.app/eula/>, lu le 8 octobre 2026.
    - Aide de Shift : cookies et partitions, <https://support.shift.com/hc/en-us/articles/39174004970004-How-Cookies-and-Partitions-are-Handled-in-Shift>, lu le 8 octobre 2026.
    - Aide de Shift : plusieurs comptes, <https://support.shift.com/hc/en-us/articles/39236976528916-How-to-Log-Into-Multiple-Accounts-in-Shift>, lu le 8 octobre 2026.
    - Shift : ses applications, <https://shift.com/apps/>, lu le 8 octobre 2026.
    - Aide de Shift : les notifications, <https://support.shift.com/hc/en-us/articles/39195849411220-Managing-notifications-in-Shift-Browser>, lu le 8 octobre 2026.
    - Aide de Shift : authentification à deux facteurs et authentification unique, <https://support.shift.com/hc/en-us/articles/39170248336020-Using-2FA-and-SSO-in-Shift>, lu le 8 octobre 2026.
    - Aide de Shift : les extensions, <https://support.shift.com/hc/en-us/articles/28587706143252-How-to-add-extensions-in-Shift>, lu le 8 octobre 2026.
    - Aide de Shift : le module Blocker, <https://support.shift.com/hc/en-us/articles/45561439081748-All-about-Shift-Browser-s-Blocker-Add-on>, lu le 8 octobre 2026.
    - Aide de Shift : les permissions de la caméra, du micro et du partage de l’écran, <https://support.shift.com/hc/en-us/articles/39167776055316-How-to-Fix-Camera-Microphone-and-Screen-Sharing-Permissions>, lu le 8 octobre 2026.
    - Aide de Shift : les réglages de synchronisation, <https://support.shift.com/hc/en-us/articles/35017689014676-How-to-manage-your-Sync-settings-in-Shift-Browser>, lu le 8 octobre 2026.
    - Aide de Shift : sur un téléphone ou une tablette, <https://support.shift.com/hc/en-us/articles/38736611711380-Is-Shift-available-on-mobile-or-tablet>, lu le 8 octobre 2026.
    - Shift : les prix, <https://shift.com/pricing/>, lu le 8 octobre 2026.
    - Wavebox : les fonctions, <https://wavebox.io/features>, lu le 8 octobre 2026.
    - Wavebox : les prix, <https://wavebox.io/pricing>, lu le 8 octobre 2026.
    - Wavebox : le mode Focus, <https://hub.wavebox.io/focus-mode/>, lu le 8 octobre 2026.
    - Wavebox : les clés d’accès, <https://hub.wavebox.io/passkeys/>, lu le 8 octobre 2026.
    - Wavebox : les gestionnaires de mots de passe, <https://hub.wavebox.io/password-manager/>, lu le 8 octobre 2026.
    - Wavebox : les permissions du micro et de la caméra, <https://hub.wavebox.io/microphone-camera-permissions/>, lu le 8 octobre 2026.
    - Wavebox : la synchronisation du profil, <https://hub.wavebox.io/profile-sync/>, lu le 8 octobre 2026.
    - Wavebox : le bloqueur de publicités, <https://hub.wavebox.io/ad-blocker/>, lu le 8 octobre 2026.
    - Wavebox : le téléchargement, <https://wavebox.io/download>, lu le 8 octobre 2026.
    - Wavebox : son ancien dépôt, sans fichier de licence, <https://github.com/wavebox/waveboxapp>, lu le 8 octobre 2026.
    - Firefox Multi-Account Containers : la page du module, <https://addons.mozilla.org/en-US/firefox/addon/multi-account-containers/>, lu le 8 octobre 2026.
    - Firefox Multi-Account Containers : ses détails par l’API des modules, <https://addons.mozilla.org/api/v5/addons/addon/multi-account-containers/>, lu le 8 octobre 2026.
    - Firefox : les politiques d’entreprise, les permissions, <https://github.com/mozilla/policy-templates/blob/master/docs/index.md>, lu le 8 octobre 2026.
    - Firefox 114 : les notes de version (clés FIDO2 par USB), <https://www.firefox.com/en-US/firefox/114.0/releasenotes/>, lu le 8 octobre 2026.
    - Bitwarden pour Firefox : la page du module, <https://addons.mozilla.org/en-US/firefox/addon/bitwarden-password-manager/>, lu le 8 octobre 2026.
    - Firefox Multi-Account Containers : ses chaînes anglaises, <https://github.com/mozilla-l10n/multi-account-containers-l10n/blob/main/en/messages.json>, lu le 8 octobre 2026.
    - Firefox : sous Android et iOS, <https://www.mozilla.org/en-US/firefox/browsers/mobile/>, lu le 8 octobre 2026.
    - Mozilla : les licences, <https://www.mozilla.org/en-US/foundation/licensing/>, lu le 8 octobre 2026.
    - Firefox : les fonctions, <https://www.mozilla.org/en-US/firefox/features/>, lu le 8 octobre 2026.
    - Franz : sa page d’accueil, <https://meetfranz.com/>, lu le 8 octobre 2026.
    - Franz : les prix, <https://meetfranz.com/pricing>, lu le 8 octobre 2026.
    - Franz : les fonctions, <https://meetfranz.com/features>, lu le 8 octobre 2026.
    - Franz : son dépôt (Franz 5), <https://github.com/meetfranz/franz>, lu le 8 octobre 2026.
    - Station : ses versions, <https://github.com/getstation/desktop-app/releases>, lu le 8 octobre 2026.
    - Station : son dépôt, <https://github.com/getstation/desktop-app>, lu le 8 octobre 2026.

## Côté technique {#for-technical-readers}

- **Le navigateur** : Qt WebEngine (Chromium). Chaque site vit dans un seul profil persistant propre à Sioul, `sioul-sites`, à part de votre navigateur habituel : ses cookies gardés, un cache disque de 512 Mo au plus, ses permissions enregistrées. Sioul ne pose pas de filtre de cookies à lui. L’agent utilisateur et les indications du client («  client hints  ») taisent le nom de Qt, car certaines discussions et la connexion de Google refusent les navigateurs qu’elles prennent pour des navigateurs intégrés.
- **Les permissions** : les notifications sont accordées à chaque site et recueillies par Sioul ; le presse-papiers, en lecture et en écriture, est accordé à chaque site, car Qt WebEngine ne demande qu’une permission pour les deux ; le micro, la caméra et la capture de l’écran suivent les interrupteurs du site, et une demande faite quand ils sont coupés est refusée et dite dans la ligne d’état ; tout le reste (la position, les polices de l’ordinateur, le verrouillage du pointeur) est refusé.
- **Les adresses** : un site ne s’épingle qu’à une adresse `https://` : Sioul refuse toute autre, car la connexion à un site ne doit jamais circuler en clair.
- **Les clés de sécurité** : WebAuthn par la propre demande de Qt WebEngine (le code FIDO de Chromium, par USB HID). Sous Linux, cela marche avec un Qt WebEngine construit avec udev et les règles de systemd pour les clés, sans règle à ajouter ; le Flatpak a besoin de `--device=all`. Un script d’une ligne retire `PublicKeyCredential.getClientCapabilities()` de chaque page, car Qt 6.10 et 6.11 n’y répondent jamais (QTBUG-149575) et les sites attendraient sans fin. Sous Windows, la boîte de dialogue de Windows prend la clé.
- **Les périphériques d’appel** : un script dans chaque page demande la caméra et le micro que vous avez choisis par leur nom (`getUserMedia` avec un `deviceId` idéal), et envoie chaque son vers le haut-parleur que vous avez choisi (`setSinkId`) ; une page n’apprend ces noms qu’une fois qu’un micro ou une caméra lui est permis.
- **Les icônes** : le `<link rel="icon">` du site lui-même, le plus grand, sinon `/favicon.ico`, par HTTPS seulement, redirections comprises ; gardées une semaine dans `~/.cache/sioul/favicons/`.
- **Les notifications** : gardées dans `~/.local/state/sioul/site-notices.toml`, les 200 plus récentes, 300 caractères chacune ; ce qui vous joint et quand suit [Ce qui vous joint, et quand](notifications.md).
- **Bitwarden, comme le font ses propres clients** : la clé principale est faite à partir de votre mot de passe principal par PBKDF2-SHA256 ou Argon2id, comme le dit votre compte, les réglages du serveur tenus dans les bornes de Bitwarden lui-même (PBKDF2, de 5 000 à 2 000 000 d’itérations ; Argon2id, au moins 2 passes, de 16 à 1 024 Mio, de 1 à 16 voies), pour qu’un serveur ne puisse pas rendre votre mot de passe moins cher à deviner. La connexion envoie une empreinte du mot de passe, jamais le mot de passe. La clé de l’utilisateur s’ouvre par des clés étirées par HKDF ; chaque élément AES-256-CBC est vérifié par son HMAC-SHA256 avant que quoi que ce soit soit déchiffré, et les éléments des comptes plus récents sont lus aussi (COSE : XChaCha20-Poly1305, AES-256-GCM) ; la clé d’une organisation passe par votre clé RSA (RSA-OAEP).
- **Votre seule clé de sécurité** ouvre le coffre par l’extension PRF de WebAuthn, salée comme les applications de Bitwarden la salent, qui ouvre la clé que Bitwarden garde pour cette clé d’accès ; la clé d’accès d’un autre compte Bitwarden est refusée. En deuxième étape, la clé répond à la propre page de clé de sécurité de Bitwarden, tenue invisible dans la boîte de dialogue, car une clé ne signe que pour l’adresse du coffre lui-même. Les codes à usage unique suivent RFC 6238 (SHA-1, SHA-256, SHA-512, et les cinq caractères de Steam).
- **Ce qui part vers Bitwarden** : HTTPS seulement, et seulement la version du serveur, la préconnexion, la connexion, les options de la clé d’accès, le code par courriel quand vous choisissez cette étape, et le coffre lui-même (`/sync`). Rien n’écrit dans le coffre. Le mot de passe principal et les clés vivent en mémoire et sont effacés une fois lâchés ; le trousseau ne garde que le jeton «  se souvenir de cet appareil  » ; l’identifiant choisi en dernier pour chaque site est gardé par l’identifiant de son élément, sans aucun secret.
- **Le remplissage** : l’identifiant passe comme des données dans un court script, jamais comme du code, et s’écrit dans le champ du mot de passe de la page et dans le champ du nom qui le précède, comme le ferait la frappe, ou dans le champ du code à usage unique. Un identifiant n’est rempli tout de suite que s’il est le seul fait pour le domaine enregistrable de la page.
