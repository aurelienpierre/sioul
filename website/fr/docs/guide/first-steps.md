---
description: "La fenêtre, ses touches, et les premières choses à régler dans Sioul : courrier, agendas et contacts, Google, votre dossier de notes, vos heures, vos sites."
---

# Premiers pas {#first-steps}

Rien n’est à régler d’un coup. Chaque étape ci-dessous est utile seule, et Sioul fonctionne avec ce que vous lui donnez. Ouvrez-le depuis votre menu d’applications, ou avec `sioul-app` dans un terminal.

Sioul parle anglais ou français, selon la langue de votre système, sauf si vous en choisissez une autre dans **Paramètres ▸ Affichage ▸ Langue**.

## La fenêtre {#the-window}

<figure markdown="span">
  [![La fenêtre de Sioul : à gauche, Nouveau, puis les lieux, du Porche à Santé, et trois icônes en bas (Comptes, Paramètres, Tout actualiser) ; à droite, le Porche ; tout en bas, la ligne d’état avec les touches, le bouton des sons et la météo.](../assets/screens/fr/porch.png){ loading=lazy }](../assets/screens/fr/porch.png "Ouvrir l’image en grand")
</figure>

**À gauche**, de haut en bas :

- **Nouveau ▾** crée une chose de n’importe quel type : un message, une tâche, un événement, un contact, une note, un projet, du temps passé, un mouvement de budget, un papier ; et **Où j’en suis…** laisse une ligne sur où vous en êtes ([Les tâches](tasks.md#starting-and-stopping)).
- **Les lieux** : Porche, Tâches, Courrier, Sites, Agenda, Contacts, Notes, Projets, Temps, Budgets, Papiers, Santé.
- **Trois icônes**, à part en bas : Comptes (une personne), Paramètres (des curseurs), et Tout actualiser, qui relève à nouveau, d’un coup, le courrier, l’agenda, les tâches et les contacts.

**En bas**, la ligne d’état dit en une phrase ce qui s’est passé en dernier. Après chaque déplacement, suppression ou envoi, « Annuler » y attend dix secondes. Pendant le calme, elle dit quand le travail revient ; une fois les heures du jour finies, elle propose de clore la journée de travail, et le soir la journée. À son extrémité droite se trouvent les touches, le bouton des sons ([des sons pour se concentrer ou se reposer](tasks.md#sounds)) et la météo d’un lieu que vous choisissez.

**Sur chaque page**, le ⚙ au bout de la première ligne contient les réglages propres à cette page, chacun avec une phrase sur ce qu’il change ; ils sont enregistrés aussitôt. Là où l’on lit du texte long (un message, une note), « Aa » règle la police, sa taille et l’interligne. Un clic droit, la touche Menu, ou un appui long sur un écran tactile, sur n’importe quel élément donne ce qui n’est pas en vue.

### Touches {#keys}

Tout se fait au clavier : Tab pour se déplacer, Entrée pour choisir, Échap pour revenir.

| Touches | Ce qu’elles font |
|---|---|
| ++ctrl+1++ à ++ctrl+9++, ++ctrl+0++ | les dix premiers lieux, dans l’ordre de la liste : Porche, Tâches, Courrier, Sites, Agenda, Contacts, Notes, Projets, Temps, Budgets |
| ++ctrl+n++ | Nouveau ▾ |
| ++ctrl+z++ | Annuler, tant que c’est proposé |
| ++f5++ | Tout actualiser |
| ++ctrl+enter++ | Envoyer, dans la fenêtre de rédaction |

### Sur un téléphone {#on-a-phone}

Une version Android est à l’essai ([Installer](install.md#on-android)). Sur un téléphone, ou dans une fenêtre de moins de 720 pixels de large :

- **Les lieux** glissent depuis la gauche, derrière ☰ ; une barre en haut nomme la page.
- **Un volet à la fois** : une page montre sa liste, puis ce que vous ouvrez sur tout l’écran ; **Retour**, sur la barre ou celui d’Android, revient en arrière.
- **Les menus** s’ouvrent par un appui long sur un écran tactile, là où une souris ferait un clic droit.

Ce qui change sur un téléphone :

- **Les notifications** ne viennent que pour les prises de médicaments ([La santé](health.md#reminders)) et pour le temps qui court ([Le temps](time.md#where-time-comes-from)). Un code que vous avez demandé s’affiche sur le Porche ; les autres rappels n’y viennent pas encore.
- **Le courrier** est relevé tant que Sioul est ouvert : Android l’arrête en arrière-plan.
- **Les dossiers** : le **Choisir…** d’un réglage ouvre la liste des dossiers du téléphone, propre à Sioul, avec **Autoriser l’accès aux fichiers** quand Android n’a pas encore donné cet accès à Sioul.
- **Les sites** s’ouvrent dans votre navigateur ([Les sites](sites.md)) ; les PDF s’ouvrent dans une autre application, avec **Ouvrir avec…**.
- **Le courrier papier** n’est pas lu, et les pièces jointes ne passent pas par un antivirus : les programmes dont Sioul se sert pour cela sur un ordinateur n’existent pas sur un téléphone.
- **Vos autres appareils** partagent avec lui par un dossier que l’application de synchronisation du téléphone transporte ([Le partage](sharing.md)). Les mots de passe ne voyagent jamais : chaque compte demande le sien, une fois ([Les comptes](accounts.md#an-account-from-your-other-device)).

## Ajouter votre courrier {#add-your-mail}

1. Ouvrez **Comptes** (l’icône en forme de personne), puis l’onglet **Ajouter un compte**.
2. Sous « Ajouter un compte de courrier », tapez votre **adresse électronique** et choisissez **Trouver le serveur**.
3. Sioul dit ce qu’il a trouvé et d’où : les réglages de votre fournisseur lui-même, la liste des fournisseurs de Thunderbird, ou une supposition, dite comme telle, à vérifier avant d’aller plus loin.
4. Tapez votre **mot de passe**. Il va dans le trousseau de votre système, nulle part ailleurs.
5. Choisissez **Se connecter et ajouter**. Sioul essaie le mot de passe avant de garder quoi que ce soit, puis relève votre courrier récent.

Gmail, et d’autres fournisseurs quand la validation en deux étapes est activée, demandent un **mot de passe d’application** au lieu de votre mot de passe habituel : vous le créez dans les réglages de votre compte chez le fournisseur (pour Gmail, sur myaccount.google.com/apppasswords). Quand le fournisseur est connu, le formulaire propose **Créer un mot de passe d’application**, qui ouvre la bonne page.

Relever le courrier ne change rien sur votre serveur de courrier. Sioul n’y écrit que lorsque vous agissez : ouvrir un message le marque comme lu, comme dans tout logiciel de courrier ; archiver, supprimer et déplacer se font dix secondes après votre demande, pour que « Annuler » puisse les arrêter.

Ensuite, sur la fiche de l’adresse dans **Vos comptes**, dépliez **Réglages de cette adresse** et cochez **À quoi sert cette adresse** : travail, vos démarches, loisirs, ou plusieurs. Tant que vous ne l’avez pas dit, une adresse compte comme du travail, pour que son courrier n’atteigne jamais vos soirées. Voir [Les heures](hours.md).

## Ajouter vos agendas, tâches et contacts {#add-your-calendars-tasks-and-contacts}

Depuis un serveur CalDAV et CardDAV : Nextcloud, Fastmail, iCloud, votre hébergeur.

1. Dans **Comptes ▸ Ajouter un compte**, sous « Ajouter des contacts et agendas », tapez votre adresse et votre mot de passe.
2. Si Sioul ne trouve pas le serveur à partir de l’adresse, dépliez **Adresse du serveur, s’il ne se trouve pas** et donnez-la.
3. Choisissez **Se connecter et ajouter**. Vos carnets d’adresses et vos agendas arrivent ; les tâches viennent avec les agendas qui les contiennent.

Votre courrier est déjà ajouté ? Sur sa fiche dans **Vos comptes**, **Ce que ce serveur offre** interroge le serveur : ses agendas et ses contacts, et pour un Nextcloud, sa version et ses applications. Un clic les ajoute.

## Ajouter Google {#add-google}

Google n’accepte aucun mot de passe venant d’autres programmes : vous vous connectez sur la page de Google elle-même.

1. Dans **Comptes ▸ Ajouter un compte**, sous « Agendas, contacts et tâches Google », tapez votre adresse Google.
2. Choisissez **Se connecter avec Google**. Votre navigateur s’ouvre sur la page de Google ; Sioul attend sa réponse cinq minutes au plus.
3. Connectez-vous, et autorisez ce que Sioul demande : vos agendas, vos contacts et vos tâches.

Sioul garde l’accès dans le trousseau de votre système. Google garde moins de choses qu’un serveur ouvert : ce qu’il ne garde pas apparaît grisé dans Sioul, jamais caché, avec la raison. Ce que Sioul lit et écrit dans votre compte Google, et comment retirer l’accès : [Politique de confidentialité](../privacy.md#google-calendars-contacts-and-tasks).

!!! note "Si Sioul demande une clé Google"
    Une copie de Sioul construite sans sa propre clé Google demande la vôtre. **Faire votre clé Google** déplie les étapes dans Comptes : un projet gratuit dans Google Cloud, un quart d’heure environ, une fois. Vous pouvez aussi choisir **Utiliser une clé Google à moi** à tout moment.

Le courrier de Google s’ajoute comme n’importe quel compte de courrier, plus haut, avec un mot de passe d’application.

## Choisir votre dossier de notes {#choose-your-notes-folder}

Vos notes sont un dossier de fichiers Markdown : un coffre Obsidian fonctionne tel quel. Sioul garde aussi vos projets, vos budgets, vos papiers et vos lettres scannées dans ce dossier, pour qu’ils voyagent avec lui jusqu’à vos autres appareils.

Dans **Paramètres ▸ Votre dossier et le partage**, choisissez **Le dossier des notes**. Sioul le lit et y fait des liens ; il ne se l’approprie jamais. Voir [Les notes](notes.md).

## Régler vos heures {#set-your-hours}

Dans **Paramètres ▸ Heures** : vos heures de travail et vos heures pour vos démarches ; tout le reste du temps, ce sont vos loisirs, et les repas et le sommeil viennent de la page Santé. Sans heures, tout arrive à toute heure, comme dans les autres logiciels de courrier. Tant qu’elles ne sont pas réglées, le Porche demande une fois, avec **Régler mes heures** et **Laisser ainsi**. Voir [Les heures](hours.md).

## Épingler les sites que vous consultez {#pin-the-websites-you-check}

Les messageries sécurisées de votre banque, de l’Assurance maladie, des impôts ; une discussion ; un appel vidéo.

Sur la page **Sites**, **Sites courants ▾** en liste environ 400, par pays, ou **Épingler un site** en trouve un par un mot (« banque », « ameli »), ou prend n’importe quelle adresse à la main. Vous vous connectez une fois ; le site garde votre session. Voir [Les sites](sites.md).

## Garder Sioul ouvert {#keep-sioul-open}

Tant que sa fenêtre est ouverte, Sioul garde chaque boîte de réception ouverte sur le serveur : un code ou un lien de connexion que vous avez demandé à un site vous parvient en quelques secondes, en une seule notification discrète, à n’importe quelle heure.

Les rappels peuvent aussi venir fenêtre fermée : dans **Paramètres ▸ Rappels et notifications**, cochez **Fenêtre de Sioul fermée**. Un petit programme de veille démarre alors avec votre session ; il ne relève pas le courrier.

## Ensuite {#next}

- [Le Porche](porch.md), où le nouveau courrier attend.
- [Les tâches](tasks.md), et l’étape suivante.
- [Les heures](hours.md), et le calme.
