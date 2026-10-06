---
description: Ce avec quoi Sioul fonctionne, fonction par fonction – serveurs de courrier et d’agenda, Google, les applications que vous utilisez déjà sur les mêmes comptes, Obsidian et Nextcloud Notes, applications de synchronisation, sites web, Bitwarden, OpenPGP, montres, antivirus, agents d’IA et systèmes ; ce qui a été testé, ce qui est attendu, et les limites connues.
---

# Fonctionne avec {#works-with}

Sioul parle des normes ouvertes : il fonctionne donc avec la plupart des serveurs, et à côté des programmes que vous utilisez déjà. Cette page dit, fonction par fonction, ce que chacune demande, et à quel point c’est sûr :

- **Testé** : essayé avec le serveur, le programme ou l’appareil nommé.
- **Attendu** : construit selon la norme qu’ils suivent, pas encore essayé avec eux.
- **Limite** : ce qui ne marche pas, ou moins bien, et pourquoi.
- **Non pris en charge** : pas construit.

Rien ici ne va au-delà de ce qui est construit. Les normes derrière chaque ligne, et où chaque essai est consigné : [la note de conception](https://aurelienpierre.github.io/sioul/dev/compatibility.html) (en anglais).

## En bref {#in-short}

| Domaine | Fonctionne avec | À quel point c’est sûr |
|---|---|---|
| [Courrier](#mail) | tout serveur IMAP et SMTP qui accepte un mot de passe sur une connexion chiffrée ; Gmail avec un mot de passe d’application | **Testé** avec un serveur de test, et une vraie boîte aux lettres utilisée chaque jour ; **attendu** ailleurs. **Non pris en charge** : Outlook.com, Hotmail, Microsoft 365 |
| [Agendas, tâches et contacts](#calendars-tasks-and-contacts) | tout serveur CalDAV et CardDAV en HTTPS : Nextcloud (celui de Murena aussi), Radicale, Fastmail, Posteo, mailbox.org, iCloud, celui de votre hébergeur | **Testé** avec Radicale, et pour trouver les serveurs de Murena ; **attendu** avec les autres. **Limites** : les rappels d’iCloud ; Baïkal doit être réglé sur l’identification Basic |
| [Google](#google) | agendas, contacts et Google Tasks, connexion sur la page de Google | **Attendu** : essayé seulement face à une imitation de Google Tasks |
| [Les autres applications sur les mêmes comptes](#other-apps-on-the-same-accounts) | Thunderbird et les applications de courrier des téléphones ; Nextcloud Tasks, Tasks.org ; DAVx⁵ avec OpenTasks ou jtx Board | **Attendu**, d’après leur code. **Limite** : une tâche liée à une autre par une attente, changée sur un téléphone par DAVx⁵ |
| [Les notes](#notes) | Obsidian, Nextcloud Notes, tout éditeur Markdown | **Testé** sur des fichiers écrits comme chacun les écrit |
| [Partager entre vos appareils](#sharing-between-your-devices) | toute application qui garde un dossier en accord : Nextcloud, l’eDrive de Murena, Syncthing, Dropbox, Google Drive, OneDrive… | **Testé** avec un simulateur de la façon dont se comportent les applications de synchronisation, et avec eDrive sur un téléphone ; **attendu** avec les autres |
| [Les sites](#sites) | les sites web qui fonctionnent dans Chrome ou Chromium : messageries sécurisées, discussions, appels ; clés de sécurité | **Attendu** ; vu fonctionner avec Proton Mail. **Limites** : partager l’écran pendant un appel ; sur un téléphone, les sites s’ouvrent dans votre navigateur |
| [Identifiants et clés](#logins-and-keys) | Bitwarden (son cloud, votre propre serveur, Vaultwarden) ; OpenPGP avec GnuPG et les autres logiciels de courrier | GnuPG **testé** dans les deux sens ; Bitwarden **testé** jusqu’à sa connexion, pas avec un vrai coffre |
| [Votre montre](#your-watch) | les fichiers d’une montre Garmin : depuis la montre, depuis Gadgetbridge, ou depuis l’export de Garmin | **Testé** sur des fichiers faits à la main, pas sur une vraie montre |
| [Antivirus et courrier scanné](#antivirus-and-scanned-letters) | ClamAV, Microsoft Defender, Tesseract et Poppler | **Testé** sur Linux ; Defender **attendu** |
| [Agents d’IA](#ai-agents) | Claude Code, Claude Desktop, les autres clients MCP qui lancent un programme | **Testé** seul, pas encore dans ces clients |
| [GitHub](#github) | vos tickets et pull requests, lus avec un jeton | **Testé** face à une imitation de GitHub |
| [Systèmes](#systems) | Linux (AppImage, Flatpak), Windows 10 et 11, macOS 13 et suivants ; Android, en essai | Linux utilisé chaque jour ; Windows et macOS construits et testés par GitHub, pas encore lancés par une personne ; Android essayé sur un téléphone |

## Courrier {#mail}

| Fonction | Ce qu’il faut | Testé avec | Attendu avec | Limites |
|---|---|---|---|---|
| Trouver votre serveur | votre adresse | celui de Murena, à la main | les fournisseurs qui publient leurs réglages, et ceux de la liste de Thunderbird | un serveur trouvé en devinant est dit comme une supposition, à vérifier |
| Recevoir et garder le courrier | IMAP sur une connexion chiffrée (port 993, ou 143 avec STARTTLS), votre mot de passe | GreenMail, un serveur de test ; une vraie boîte aux lettres utilisée chaque jour | Murena, Fastmail, Posteo, mailbox.org, celui de votre hébergeur, votre propre Dovecot ; iCloud Mail et Yahoo avec un mot de passe d’application | relever ne change rien sur le serveur ; aucune connexion non chiffrée |
| Les codes en quelques secondes | IDLE, que la plupart des serveurs ont | — | la plupart des serveurs | sans lui, Sioul regarde toutes les deux minutes |
| Archiver, supprimer, indésirable, déplacer | MOVE, sinon UIDPLUS ; des dossiers marqués par leur usage | GreenMail : archiver, supprimer | la plupart des serveurs | sans MOVE ni UIDPLUS, l’original reste marqué comme supprimé jusqu’à ce qu’un autre programme le retire ; les dossiers non marqués sont trouvés par leur nom, et une Corbeille, des Indésirables ou des Archives manquants sont créés |
| Ce que les autres programmes ont changé | — | — | tout serveur | lu, signalé, déplacé ou supprimé ailleurs : vu à chaque passage, en demandant les marques de chaque message, ce qui est plus lent pour les très gros dossiers |
| Envoyer | SMTP sur le port 465 (chiffré) ou 587 (STARTTLS), votre mot de passe | GreenMail | le serveur d’envoi de votre fournisseur | une copie va dans Envoyés (Gmail range la sienne) ; aucune connexion non chiffrée |
| Gmail | un mot de passe d’application, qui demande la validation en deux étapes de Google | son serveur atteint à la main ; son refus d’un mot de passe habituel reconnu, dans les tests | Gmail | « Tous les messages » sert d’archive |
| Le courrier chiffré | OpenPGP, en PGP/MIME | GnuPG 2.4, dans les deux sens | Thunderbird, Proton, tout programme qui lit le PGP/MIME | voir [Identifiants et clés](#logins-and-keys) |

**Non pris en charge** :

- **Les adresses Outlook.com, Hotmail et Live** : depuis le 16 septembre 2024, Microsoft n’accepte plus des autres logiciels de courrier que sa propre page de connexion, et Sioul n’en a pas pour le courrier. **Microsoft 365**, adresses professionnelles et scolaires : de même.
- **Gmail sans mot de passe d’application**, pour la même raison : la connexion par Google est construite pour les agendas, les contacts et les tâches, pas pour Gmail.
- Les serveurs JMAP, POP3, et les protocoles propres à Exchange.
- **Proton Mail par son Bridge** : pas essayé. Sioul ne fait confiance qu’aux certificats auxquels votre système fait confiance : celui du Bridge devrait d’abord être ajouté à votre système. Proton Mail fonctionne comme [site](#sites).

### La même boîte aux lettres dans d’autres programmes {#the-same-mailbox-in-other-programs}

Thunderbird, l’application de courrier de votre téléphone et le webmail voient la même boîte aux lettres :

- Relever le courrier ne marque rien comme lu. Ouvrir un message dans Sioul le marque comme lu sur le serveur, comme dans tout logiciel de courrier.
- Archiver, supprimer, indésirable et déplacer arrivent sur le serveur dix secondes après votre geste, comme de vrais déplacements : les autres programmes les voient. **Indésirable** et **Pas indésirable** marquent aussi le message pour le filtre anti-spam du serveur.
- Ce que font les autres programmes (lu, signalé, déplacé, supprimé) revient dans Sioul à chaque passage.
- Sioul ne crée un dossier Corbeille, Indésirables, Archives ou Envoyés que si le serveur n’en a pas, et ne supprime que les dossiers vides que vous avez créés.
- Répondre depuis Sioul ne marque pas le message comme répondu sur le serveur.
- Les brouillons restent sur votre appareil : les autres programmes ne les voient pas.

## Agendas, tâches et contacts {#calendars-tasks-and-contacts}

| Fonction | Ce qu’il faut | Testé avec | Attendu avec | Limites |
|---|---|---|---|---|
| Trouver votre compte | votre adresse ; sinon l’adresse du serveur, donnée une fois | ceux de Murena, à la main ; Radicale | les serveurs qui publient les adresses de la norme ; Fastmail, iCloud, Posteo, mailbox.org et les hébergeurs cPanel sont connus | une redirection vers un autre domaine n’est pas suivie : donnez l’adresse du serveur |
| Les événements et les contacts, dans les deux sens | CalDAV et CardDAV en HTTPS, votre mot de passe | Radicale, un serveur de test | Nextcloud (celui de Murena aussi), Fastmail, Posteo, mailbox.org, SOGo, celui de votre hébergeur ; iCloud avec un mot de passe d’application | aucun serveur non chiffré. **Baïkal** : réglez son « WebDAV authentication type » sur Basic ; celui par défaut, Digest, n’est pas pris en charge |
| Seulement ce qui a changé | les jetons de synchronisation, sinon l’étiquette de chaque élément | Radicale ; une imitation dans les tests | la plupart des serveurs | sans jetons de synchronisation, l’étiquette de chaque élément est comparée à chaque passage, ce qui est plus lent |
| Les tâches | des listes de tâches | — | les serveurs dont les agendas acceptent les tâches, celui de Nextcloud parmi eux | **iCloud** : les rappels mis à niveau depuis iOS 13 ne sont accessibles à aucun programme CalDAV. **Google** : par Google Tasks ([plus bas](#google)) |
| Étapes, attentes, liens, types, coûts | un serveur qui garde ce qu’on lui donne, lignes inconnues comprises | — | les serveurs qui gardent les tâches telles qu’on les envoie, celui de Nextcloud parmi eux | les autres applications peuvent en perdre une partie en enregistrant une tâche ([plus bas](#other-apps-on-the-same-accounts)) |
| Une heure donnée à une étape pour un jour | une ligne propre à Sioul dans la tâche | — | — | les autres applications ne la montrent pas. Ce n’est pas le début de la tâche : les serveurs qui vérifient (celui de Nextcloud) refusent un début avec une heure quand la date demandée n’en a pas. **À venir** : un événement dans un agenda, lié à la tâche, que toute application d’agenda montre |
| Nouvelles listes, nouveaux agendas et carnets d’adresses ; les renommer | les requêtes de la norme pour les créer et les renommer | — | Nextcloud et la plupart des serveurs | pas chez Google |
| Les catégories des contacts | les catégories de la fiche elle-même | des fiches écrites comme Nextcloud les écrit | Nextcloud Contacts (ses groupes) ; DAVx⁵ réglé pour garder les groupes en catégories | les groupes gardés comme fiches à part (la façon d’Apple, et l’autre réglage de DAVx⁵) ne sont pas lus comme catégories : un tel groupe apparaît comme une fiche |
| Les invitations | reçues par courrier ; votre réponse repart par courrier | Radicale, une invitation acceptée | le logiciel de courrier de l’organisateur, quel qu’il soit | inviter des personnes depuis Sioul : non pris en charge. La copie dans votre agenda ne garde pas votre réponse : d’autres applications peuvent la montrer sans réponse |
| Changé à deux endroits à la fois | l’étiquette de chaque élément | une imitation dans les tests | tout serveur | la version du serveur l’emporte ; la vôtre est mise de côté, et la ligne d’état dit où |

## Google {#google}

| Fonction | Ce qu’il faut | Testé avec | Attendu avec | Limites |
|---|---|---|---|---|
| Se connecter | la page de Google, dans votre navigateur | ses morceaux, dans les tests | Google | pas encore essayé avec Google lui-même. Avec une clé Google à vous laissée « en test », Google met fin à chaque connexion au bout de sept jours ([la note de conception](https://aurelienpierre.github.io/sioul/dev/google.html#your-own-google-key), en anglais) |
| Les agendas | le CalDAV de Google | — | Google | aucun agenda créé, renommé ou supprimé depuis Sioul ; pas de tâches dans un agenda Google ; Google ajoute vos rappels par défaut à chaque événement, et décale les événements écrits sans fuseau horaire |
| Les contacts | le CardDAV de Google | — | Google | Google garde l’ancienne version des vCard : les libellés, les anniversaires sans année et les champs plus récents sont perdus, ce qui est dit avant qu’un contact n’y aille ; aucun nouveau carnet d’adresses |
| Les tâches | Google Tasks | une imitation de Google Tasks | Google Tasks | Google garde un titre, des notes, fait ou non, un jour, et un niveau d’étapes ; le reste est grisé dans le formulaire de la tâche, en disant pourquoi |
| Gmail | un mot de passe d’application | voir [Courrier](#mail) | | |

## Les autres applications sur les mêmes comptes {#other-apps-on-the-same-accounts}

Vos tâches, vos événements et vos contacts sont standard : toute application sur le même serveur les montre. Ce qu’une application garde des lignes propres à Sioul quand elle enregistre une tâche dépend d’elle. Ceci a été lu dans leur code source le 6 octobre 2026, pas encore essayé avec Sioul :

| Application | Étapes | Attentes | Liens, types, coûts, ressentis | À quel point c’est sûr |
|---|---|---|---|---|
| Nextcloud Tasks, sur le web | montrées comme sous-tâches | gardées | gardés | **attendu** |
| Tasks.org, avec sa propre synchronisation CalDAV | montrées comme sous-tâches | gardées | gardés | **attendu** |
| Tasks.org ou OpenTasks, par DAVx⁵ | montrées comme sous-tâches | montrées comme des étapes, et réécrites comme des étapes quand le téléphone change la tâche | gardés | **limite** |
| jtx Board, par DAVx⁵ | montrées comme sous-tâches | perdues quand le téléphone change la tâche | gardés | **limite** |
| L’agenda d’Android, par DAVx⁵ (événements) | — | — | gardés | **attendu** |
| Thunderbird, Calendrier et Rappels d’Apple, Evolution, KOrganizer | pas vérifié | pas vérifié | pas vérifié | une application qui réécrit une tâche à partir de ce qu’elle comprend seulement perd le reste |
| Google Tasks | un niveau | pas gardées | pas gardés | **limite** : grisé dans Sioul, en disant pourquoi |

!!! warning "Les attentes sur un téléphone, par DAVx⁵"
    Avec OpenTasks, Tasks.org ou jtx Board par DAVx⁵, changer sur le téléphone une tâche liée à une autre par une attente (l’une ou l’autre), même la cocher comme faite, change l’attente en étape, ou la perd. En attendant que cela change, modifiez ces tâches dans Sioul, dans Nextcloud Tasks, ou dans Tasks.org avec sa propre synchronisation.

## Les notes {#notes}

Entièrement compatibles avec un coffre Obsidian et avec Nextcloud Notes, côte à côte : [Les notes](notes.md#the-same-folder-as-obsidian-and-nextcloud-notes).

| Fonction | Ce qu’il faut | Testé avec | Attendu avec | Limites |
|---|---|---|---|---|
| Un coffre Obsidian | le coffre choisi comme dossier de notes de Sioul | des fichiers écrits comme Obsidian les écrit (liens wiki, images incluses, étiquettes, en-tête, alias, sa corbeille) | Obsidian, sur l’ordinateur et sur le téléphone, par votre synchronisation | Sioul ne lit jamais les réglages propres d’Obsidian (`.obsidian`) |
| Nextcloud Notes | le dossier Notes de votre Nextcloud, synchronisé sur l’appareil | des fichiers écrits comme Nextcloud Notes les écrit (`.txt` ou `.md`, catégories en dossiers) | Nextcloud Notes sur le web et dans ses applications pour téléphone | Sioul lit le dossier synchronisé, pas l’application Notes de Nextcloud : le dossier doit être sur l’appareil |
| Tout autre éditeur | des fichiers Markdown simples | — | tous | une note changée ailleurs pendant qu’elle est ouverte dans Sioul est gardée à côté, « (conflit …) », jamais écrasée |

## Partager entre vos appareils {#sharing-between-your-devices}

Le partage n’a besoin que d’un dossier qu’une application de synchronisation garde en accord entre vos appareils : les nouveaux fichiers, et les fichiers qui grandissent, doivent arriver un jour chez les autres. Rien n’a besoin d’être renommé, effacé ni verrouillé. Comment cela marche : [Partager entre vos appareils](sharing.md).

| Fonction | Ce qu’il faut | Testé avec | Attendu avec | Limites |
|---|---|---|---|---|
| Le dossier de partage, sur un ordinateur | un dossier que votre application de synchronisation garde en accord | un simulateur de la façon dont les applications de synchronisation transportent les fichiers : chaque changement dans les deux sens (comme le client de Nextcloud, Dropbox, Google Drive et OneDrive), en retard et dans le désordre, des copies plus anciennes remises, des fichiers gardés en ligne seulement | le client de Nextcloud (celui de Murena aussi), ownCloud, Syncthing, Dropbox, Google Drive, OneDrive, iCloud Drive, pCloud, Seafile | fichiers gardés en ligne seulement : réglez le dossier de partage pour qu’il reste « toujours sur cet appareil » |
| Le dossier de partage, sur un téléphone | l’accès d’Android à tous les fichiers ; un dossier que l’application de synchronisation garde sur le téléphone | l’eDrive de Murena 1.9.2, sur /e/OS : le dossier est arrivé à son examen complet suivant | l’application de Nextcloud, Syncthing, FolderSync, Autosync | eDrive ne transporte que Documents, Pictures, Music… : partagez par `Documents/Sioul`. Il n’efface jamais ce qui a été effacé ailleurs : comptez la place deux fois. Seul eDrive est prié de regarder tout de suite ; les autres applications apportent les changements à leur rythme, souvent toutes les demi-heures |
| Les notes et les papiers, transportés par Sioul | un dossier de notes qu’aucune application de synchronisation ne transporte | le simulateur | — | refusé là où une application de synchronisation transporte déjà le dossier de notes. Reconnus : Nextcloud, Dropbox, Syncthing, et les dossiers nommés ownCloud, Sync, OneDrive, pCloudDrive ou Seafile. **Pas reconnus** : Google Drive, Insync, rclone, MEGA : laissez Notes désactivé là où ils transportent vos notes |
| Changé sur deux appareils | — | le simulateur | — | les deux versions sont gardées, l’une nommée « (conflit …) » ; les copies en conflit des applications de synchronisation ne sont jamais lues |

## Les sites {#sites}

Les sites tournent dans Qt WebEngine, le moteur de Chromium, dans un profil à eux, à part de votre navigateur habituel.

| Fonction | Ce qu’il faut | Testé avec | Attendu avec | Limites |
|---|---|---|---|---|
| Les sites web | — | l’application web de Proton Mail, utilisée | les sites qui fonctionnent dans Chrome ou Chromium | se connecter avec Google dans un site peut être refusé : Google bloque les connexions dans les navigateurs intégrés |
| Rester connecté | les cookies et le stockage du site, gardés | — | la plupart des sites | — |
| Les notifications | celles du site | — | tout site qui envoie des notifications | gardées pour vos heures ([Les sites](sites.md#notifications-at-your-pace)) |
| Les appels | le micro, la caméra et le haut-parleur, permis site par site | — | les appels des discussions et des sites de visioconférence qui fonctionnent dans Chrome | **partager l’écran ne fonctionne pas encore** (trouvé le 6 octobre 2026) ; le plein écran n’est pas disponible |
| Les clés de sécurité (FIDO2, WebAuthn) | une clé USB comme une YubiKey ; sur Linux, un Qt WebEngine construit avec udev (celui de Fedora l’est) | — | GitHub, Google, Proton, Bitwarden | les clés d’accès gardées dans un téléphone ou dans le système : pas sur Linux ni macOS. Sur Windows, la boîte de dialogue de Windows elle-même demande. Un simple toucher ne montre rien dans Sioul |
| Les téléchargements et les PDF | — | — | — | les téléchargements vont dans votre dossier de téléchargements ; un PDF s’ouvre dans la vue du site |
| Les identifiants de Bitwarden | voir [Identifiants et clés](#logins-and-keys) | | | |
| Sur un téléphone | — | — | — | les sites s’ouvrent dans votre navigateur : aucune connexion gardée par Sioul, aucune notification rassemblée |

## Identifiants et clés {#logins-and-keys}

| Fonction | Ce qu’il faut | Testé avec | Attendu avec | Limites |
|---|---|---|---|---|
| Les identifiants de Bitwarden | votre compte Bitwarden : bitwarden.com, bitwarden.eu, votre propre serveur ou Vaultwarden, en HTTPS | le cloud de Bitwarden, jusqu’à sa connexion ; son déchiffrement vérifié sur les propres valeurs de test de Bitwarden | un vrai coffre, et une clé de sécurité pour l’ouvrir | lecture seule : rien n’est écrit dans votre coffre. Duo comme deuxième étape : non pris en charge. Sur un téléphone : pas de clé de sécurité |
| Les mots de passe de vos comptes | le trousseau de votre système | Linux, utilisé chaque jour | le Gestionnaire d’identification de Windows, le Trousseau d’accès de macOS, le KeyStore d’Android | les mots de passe ne voyagent jamais entre vos appareils |
| Le courrier chiffré (OpenPGP) | votre clé, faite dans Sioul ou importée de GnuPG | GnuPG 2.4, dans les deux sens : signé, chiffré, falsifié | Thunderbird, Proton, tout programme qui lit le PGP/MIME ; l’annuaire de clés web de leur domaine (WKD), keys.openpgp.org | Sioul garde ses propres clés et ne lit ni n’écrit jamais celles de GnuPG. Les clés gardées sur une carte à puce ou une clé de sécurité : non pris en charge. Vos clés secrètes restent sur leur appareil |

## Votre montre {#your-watch}

| Fonction | Ce qu’il faut | Testé avec | Attendu avec | Limites |
|---|---|---|---|---|
| Une montre Garmin | ses propres fichiers (FIT) : depuis son dossier `GARMIN` quand votre bureau le montre, depuis les exports de Gadgetbridge, ou depuis l’export de Garmin | des fichiers faits à la main | les montres Garmin ; Gadgetbridge avec une montre Garmin | jamais par un compte Garmin. Les montres récentes, sur KDE, demandent kio-fuse ; sans lui, copiez les dossiers de la montre à la main ([La santé](health.md#your-watch)) |
| Les autres montres | — | — | — | **non pris en charge** : seuls les fichiers de Garmin sont lus ; pas la base de données propre de Gadgetbridge, Apple Santé, Health Connect ni Fitbit |

## Antivirus et courrier scanné {#antivirus-and-scanned-letters}

| Fonction | Ce qu’il faut | Testé avec | Attendu avec | Limites |
|---|---|---|---|---|
| Les pièces jointes vérifiées, sur Linux et macOS | ClamAV, des paquets de votre système ou de Homebrew | Linux sans ClamAV : Sioul demande d’abord, et donne la commande qui l’installe | ClamAV, son démon ou son scanner | le Flatpak n’atteint pas le ClamAV de votre système : là, Sioul demande d’abord. Aucun antivirus sur un téléphone |
| Les pièces jointes vérifiées, sur Windows | Microsoft Defender, ou un autre antivirus qui répond à l’interface d’analyse de Windows (AMSI) | — | l’antivirus de Windows | construit, jamais lancé sur Windows pour l’instant |
| Le courrier scanné, lu | Tesseract et Poppler | Tesseract, à la main, sur une lettre française dessinée en image | Linux, Windows et macOS avec les deux installés | le Flatpak ne contient ni l’un ni l’autre : s’il lit les scans n’est pas vérifié. Pas sur un téléphone |

## Agents d’IA {#ai-agents}

| Fonction | Ce qu’il faut | Testé avec | Attendu avec | Limites |
|---|---|---|---|---|
| Un agent sur cet ordinateur | un client MCP qui lance `sioul mcp` | le serveur seul, dans ses tests et à la main | Claude Code, Claude Desktop, les autres clients MCP qui lancent un programme ; des modèles locaux par un tel client | pas encore essayé dans Claude Code ni Claude Desktop |
| Un agent sur Internet | — | — | — | **non pris en charge** : ChatGPT n’atteint les serveurs que par Internet |
| La lecture par l’IA d’une adresse protégée | votre clé Anthropic | une imitation du service d’Anthropic | Anthropic | seulement pour une adresse que vous protégez, quand vous le permettez ([Le Porche](porch.md#a-public-address-protected)) |

Comment en connecter un, et ce qu’il peut voir : [Avec un agent d’IA](ai-agent.md).

## GitHub {#github}

| Fonction | Ce qu’il faut | Testé avec | Attendu avec | Limites |
|---|---|---|---|---|
| Vos tickets et vos pull requests comme tâches | un jeton à droits fins, en lecture seule | une imitation de GitHub | github.com | GitHub refusait les recherches faites avec un jeton à droits fins jusqu’à la correction de Sioul du 6 octobre 2026, pas essayée avec GitHub depuis. Rien n’est écrit sur GitHub. GitHub Enterprise Server, GitLab, Codeberg : non pris en charge |

## Systèmes {#systems}

| | Linux | Windows | macOS | Android |
|---|---|---|---|---|
| Paquet | AppImage, Flatpak, ou depuis les sources | un installateur, Windows 10 et 11 (64 bits) | une image disque, macOS 13 et suivants | un APK, installé à la main (Android 9 et suivants, 64 bits) |
| À quel point c’est sûr | utilisé chaque jour, sur Fedora | construit et testé par GitHub à chaque changement, pas encore lancé par une personne | de même | essayé sur un téléphone (Android 12) |
| Les mots de passe sont gardés dans | votre trousseau (GNOME Keyring, KWallet) | le Gestionnaire d’identification | le Trousseau d’accès | le KeyStore d’Android |
| Les rappels, fenêtre fermée | oui | pas encore | oui | les prises et le réveil |
| Les notifications | oui, avec des boutons ; le temps qui court | oui | oui | les prises, le temps qui court, le réveil |
| Les pièces jointes vérifiées | ClamAV, s’il est installé (pas dans le Flatpak) | Microsoft Defender | ClamAV, de Homebrew | non |
| Le courrier scanné, lu | Tesseract et Poppler, s’ils sont installés | de même | de même, de Homebrew | non |
| Les sites | dans Sioul | dans Sioul | dans Sioul | dans votre navigateur |
| Les clés de sécurité dans les sites | clés USB, leur code PIN demandé par Sioul | la boîte de dialogue de Windows | clés USB pas vérifiées ; pas de Touch ID | celles de votre navigateur |

Sur Ubuntu 24.04 et suivants, l’AppImage fait tourner les sites sans le bac à sable de Chromium : préférez-y le Flatpak ([Installer](install.md#download)). **Non pris en charge** : iPhone et iPad.

## Pas encore vérifié {#not-checked-yet}

À essayer, et bienvenu dans les [tickets GitHub](https://github.com/aurelienpierre/sioul/issues) une fois essayé :

- Une synchronisation complète de tâches avec étapes, attentes et liens sur Nextcloud (celui de Murena aussi), Fastmail, iCloud, ou Baïkal réglé sur Basic.
- Des attentes changées sur un téléphone par DAVx⁵, pour confirmer ce que dit son code ; ce que gardent Thunderbird et les applications d’Apple.
- Une invitation acceptée sur un serveur qui envoie lui-même les invitations (Nextcloud, Google, iCloud).
- Google lui-même, et GitHub lui-même depuis la correction du 6 octobre 2026.
- Une clé de sécurité sur GitHub, Google et Proton dans les sites ; un appel avec le micro et la caméra ; un PDF montré par un site.
- Un site qui garde sa connexion dans sa page, comme Discord, qui reste connecté après la fermeture de Sioul.
- Bitwarden avec un vrai coffre.
- Windows et macOS, lancés par une personne.
- Le courrier scanné dans le Flatpak ; Proton Mail Bridge ; une vraie montre Garmin ; Claude Code et Claude Desktop.
