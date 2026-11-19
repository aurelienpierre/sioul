---
description: "Bien démarrer avec Sioul : les quatre idées sur lesquelles il repose, puis l’installer, ajouter vos courriels, vos agendas, vos tâches et vos contacts, régler vos heures, un premier regard sur chaque page, et relier un second appareil."
---

# Bien démarrer : l’essentiel {#getting-started}

Cette page est le chemin le plus court. Elle donne les quatre idées sur lesquelles Sioul repose, met en place sa base (courriels, agendas, tâches et contacts), regarde chacun une fois, puis relie un second appareil. Elle s’en tient au cas courant, et chaque section finit par la page qui dit le reste.

Rien n’est à faire d’un coup. Chaque étape sert à elle seule, Sioul fonctionne avec ce que vous lui donnez, et tout ce que vous déplacez, supprimez ou envoyez peut s’annuler pendant dix secondes.

## Quatre idées à connaître d’abord {#four-ideas}

Sioul fait des choses familières, des courriels, un agenda, des tâches et des contacts, dans un ordre qui ne l’est pas : il part de ce que vous pouvez donner, et ajuste le reste autour. Quatre idées expliquent presque tout ce que vous verrez.

1. **Vous décidez de ce qui passe, et quand.** Vous dites une fois quelles heures sont pour le travail et lesquelles sont pour vos démarches ; tout le reste du temps est aux loisirs. Vous dites aussi à quoi sert chaque chose : cette adresse sert au travail, cet agenda est personnel. Dès lors, le courriel d’un client attend vos heures de travail, celui des impôts vos heures de démarches, et le travail de personne n’atteint votre soirée.
2. **Être affiché n’est pas être dérangé.** Ce qui arrive n’entre pas tout droit. Cela attend sous le **Porche**, vérifié et trié, que vous regardiez. Seul ce que vous avez permis peut vous interrompre, comme le code que vous venez de demander à un site. Sioul n’affiche ni compteur de non-lus, ni pastilles, ni rouge.
3. **Une seule étape suivante, et rien n’est en retard.** Sioul dispose vos tâches lui-même, autour de vos événements, de vos repas et de votre repos, et montre la seule étape à faire maintenant, avec sa raison. Une date passée n’est pas une dette : le plan repart d’aujourd’hui.
4. **Sioul garde les liens.** Un courriel, la tâche qu’il demande, le rendez-vous et la personne qui a écrit font une seule affaire. Chacun ouvre les autres : vous n’avez pas à vous rappeler où sont les choses.

Vos données restent les vôtres d’un bout à l’autre. Vos courriels, vos agendas et vos contacts restent sur les serveurs de vos propres fournisseurs, tout le reste reste sur vos appareils, et rien ne parvient au développeur.

[À quoi sert Sioul, et pour qui](../index.md)

<figure markdown="span">
  [![Le Porche aux heures de travail. En haut, « Ouvert jusqu’à 17:00. » et ses réglages, Montrer tous les courriels maintenant en dessous ; la carte pour rejoindre vos autres appareils ; puis ses sections, chacune un titre avec son « ? » et son chevron : Tout de suite, un code d’un expéditeur vérifié avec Copier ; les courriels d’un projet ; les personnes que vous connaissez ; De votre téléphone ; Sites ; Argent, chaque paiement avec son montant à droite ; chaque ligne avec Marquer comme lu et Écarter à côté.](../assets/screens/fr/porch.png){ loading=lazy }](../assets/screens/fr/porch.png "Ouvrir l’image en grand")
  <figcaption>Les codes en haut, puis chaque section, une ligne par nouveauté.</figcaption>
</figure>

## La mise en place {#set-it-up}

Quatre étapes. Les deux premières vous donnent une boîte aux lettres qui marche : vous pouvez vous arrêter là, et revenir un autre jour.

### 1. Installer Sioul {#install-sioul}

Téléchargez le paquet de votre système sur [la page des versions](https://github.com/aurelienpierre/sioul/releases/latest).

| Système | Paquet |
|---|---|
| Windows 10 et 11 | `sioul-<version>-setup.exe` |
| macOS 13 et suivants | `Sioul-<version>-macos-universal.dmg` |
| Linux | `Sioul-<version>-x86_64.AppImage`, ou `Sioul-<version>.flatpak` |
| Android 9 et suivants, téléphones et tablettes | `Sioul-<version>-android-arm64.apk` |

Sioul n’est dans aucune boutique, et ses paquets pour Windows et macOS ne sont pas encore signés : chaque système vous demande donc de confirmer la première ouverture. [Installer](install.md) dit quoi presser sur chacun. Sioul est jeune : il sert tous les jours sous Linux et sur Android, et il est peu essayé ailleurs.

Ouvrez ensuite Sioul depuis votre menu d’applications. Il parle français et anglais, comme votre système.

### 2. Ajouter vos courriels {#add-your-mail}

1. Ouvrez **Comptes** (l’icône de la personne, en bas de la colonne de gauche), puis l’onglet **Ajouter un compte**.
2. Tapez votre **adresse électronique** et choisissez **Trouver le serveur**. Sioul dit ce qu’il a trouvé, et d’où cela vient.
3. Tapez votre **mot de passe**. Il va dans le trousseau de votre système, et nulle part ailleurs.
4. Choisissez **Se connecter et ajouter**. Sioul essaie le mot de passe avant de garder quoi que ce soit, puis relève vos courriels récents.

Trois fournisseurs ne prennent pas de mot de passe ordinaire. Pour Gmail, Sioul propose un mot de passe d’application, ou la page de connexion de Google ; pour Yahoo Mail, un mot de passe d’application, créé sur la page de sécurité de votre compte Yahoo ; pour Outlook.com, Hotmail et Microsoft 365, la page de connexion de Microsoft. [Premiers pas](first-steps.md#gmail-and-google-workspace) décrit les trois.

Relever ne change rien sur votre serveur de courriel. Sioul n’y écrit que lorsque vous agissez : archiver, supprimer et déplacer se font dix secondes après votre demande, pour qu’**Annuler** puisse les arrêter.

Dites ensuite à quoi sert l’adresse. Sur sa carte, dans **Vos comptes**, dépliez **Réglages de cette adresse** et cochez **À quoi sert cette adresse** : travail, démarches, loisirs, ou plusieurs. Tant que vous ne l’avez pas dit, une adresse compte pour le travail, afin qu’elle n’atteigne jamais vos soirées.

### 3. Ajouter vos agendas, vos tâches et vos contacts {#add-your-calendars-tasks-and-contacts}

Ils viennent du serveur qui les garde déjà.

- **Un serveur CalDAV et CardDAV** (Nextcloud, Fastmail, iCloud, votre hébergeur) : dans **Comptes ▸ Ajouter un compte**, sous « Ajouter des contacts et agendas », tapez votre adresse et votre mot de passe, puis **Se connecter et ajouter**. Vos carnets d’adresses et vos agendas arrivent ; les tâches viennent avec les agendas qui les contiennent. Quand vos courriels sont chez le même fournisseur, **Ce que ce serveur offre**, sur la carte de l’adresse, les ajoute en un clic.
- **Google** : sous « Agendas, contacts et tâches Google », tapez votre adresse Google et choisissez **Se connecter avec Google**. Vous vous connectez sur la page de Google, dans votre navigateur.
- **Sans serveur** : Sioul peut garder des agendas, des listes de tâches et des contacts sur cet appareil seulement. À votre première tâche, **Créer la liste** crée une liste en un clic.

Sioul n’atteint pas les agendas et les contacts de Microsoft : Microsoft ne les offre pas par les standards ouverts que Sioul parle.

### 4. Régler vos heures {#set-your-hours}

Dans **Paramètres ▸ Heures**, deux semaines sont à remplir : **Heures de travail** et **Heures pour vos démarches**. Chaque jour est actif ou non, avec une plage d’heures ou plusieurs. Tout le reste du temps est aux loisirs, sans rien à régler ; vos repas et votre sommeil habituels se règlent dans le même onglet, sous **Repas, repos et sommeil**.

C’est cette étape qui rend Sioul calme. Sans heures, tout arrive à toute heure, comme dans les autres logiciels de courriel. Tant qu’elles ne sont pas réglées, le Porche le demande une fois, avec **Régler mes heures** et **Laisser ainsi**.

### Puis laissez-le ouvert {#then-leave-it-open}

Sioul est fait pour rester ouvert toute la journée. Fermer sa fenêtre le range dans la zone de notification, où il continue de relever les courriels et de tenir vos rappels. **Quitter Sioul** est dans le menu de son icône, ou ++ctrl+q++. [Garder Sioul ouvert](first-steps.md#keep-sioul-open)

## Un premier regard {#a-first-look}

La colonne de gauche porte les lieux, une icône chacun. Sur un téléphone, ils glissent depuis la gauche, derrière ☰. Cinq d’entre eux font la base.

| Lieu | Touche | Ce qu’il contient |
|---|---|---|
| **Porche** | ++ctrl+1++ | ce qui est arrivé, en attente de ses heures |
| **Tâches** | ++ctrl+2++ | la seule étape suivante, et le plan derrière elle |
| **Courriels** | ++ctrl+3++ | tous vos courriels, pour quand vous choisissez de regarder |
| **Agenda** | ++ctrl+5++ | ce qui vient, à partir d’aujourd’hui |
| **Contacts** | ++ctrl+6++ | les personnes à qui vous avez affaire, et qui peut vous joindre |

### Le Porche : ce qui est arrivé {#the-porch}

Le Porche est l’endroit où les nouveaux courriels de toutes vos adresses attendent que vous regardiez. Sioul vérifie d’abord chaque courriel, authentique ou falsifié, et le range dans une file : les personnes que vous connaissez, un projet, les infolettres, ce qui est mis de côté.

- **Il ouvre à vos heures**, et dit jusqu’à quand : « Ouvert jusqu’à 17:00. » En dehors, il dit quand il ouvre de nouveau, et rien d’autre de vos courriels.
- **Les codes et les liens de connexion que vous venez de demander à un site arrivent tout de suite**, quelle que soit l’heure, dans **Tout de suite**, avec **Copier**.
- **Le premier courriel d’un inconnu attend votre accord.** **Accepter cette adresse** envoie ses courriels suivants avec ceux des personnes que vous connaissez.
- **Chaque ligne** a **Marquer comme lu** et **Écarter**, qui valent sur tous vos appareils. **Terminé pour l’instant** ferme le Porche sur ce qu’il montrait. Rien n’est supprimé, et le fermer ne marque rien comme lu.

La suite : [Le Porche](porch.md)

### Les courriels : tout, quand vous choisissez de regarder {#mail}

La page **Courriels** garde tous vos courriels. Elle s’ouvre sur **Toutes les adresses** : une seule liste pour toutes vos adresses, les plus récents d’abord, chaque courriel une fois, avec un petit point devant ce que vous n’avez pas lu, et rien de compté.

- **Un clic ouvre un courriel** à côté de la liste. Sioul dit, avant que vous le lisiez, s’il vient vraiment de l’adresse qu’il affiche. Rien dans un courriel ne se charge ni ne s’exécute.
- **Écrire** commence un courriel, en texte simple ou en Markdown. **Envoyer** (++ctrl+enter++) attend dix secondes, avec **Annuler**, avant que le courriel parte. Les brouillons s’enregistrent pendant que vous tapez.
- **Les courriels affichés** suivent vos heures : les adresses du travail aux heures de travail, celles de vos démarches aux heures de démarches. Le bouton à côté des pastilles (**Tout**, **Non lus**, **Envoyés**, **Indésirables**) montre un autre domaine quand vous le voulez.

La suite : [Les courriels](mail.md)

### L’agenda : ce qui vient {#agenda}

L’**Agenda** s’ouvre sur **Ce qui vient** : aujourd’hui en entier, puis les jours qui contiennent quelque chose, en une liste calme. **Jour**, **Semaine** et **Mois** sont à un choix de là.

- **Un nouvel événement** : **Nouveau ▾ ▸ Un événement**, ou un double appui sur un jour ou une heure. Il demande un titre, quand, et où.
- **Avant** et **Après**, dans son formulaire, gardent le temps de se préparer, d’y aller et d’en revenir. Votre plan garde ce temps libre, et l’événement dit quand partir.
- **Un seul rappel, discret**, vient un quart d’heure avant l’événement et son temps pour y aller. Rien n’est jamais en retard : le passé ne se montre que si vous y retournez.

Vos événements restent sur votre serveur d’agenda, comme des événements standards : votre téléphone et vos autres logiciels les voient.

La suite : [L’agenda](agenda.md)

### Les tâches : une seule étape suivante {#tasks}

La page **Tâches** s’ouvre sur **Maintenant** : une étape, pourquoi elle vient maintenant, et celle d’après. Elle a cinq boutons : **Commencer**, **Fait**, **Pas maintenant**, **Détails** et **Qu’est-ce qui rend cela difficile ?**

<figure markdown="span">
  [![La page Tâches sur « Maintenant ». En haut, les vues Maintenant, La journée, Liste, Tableau et Calendrier, puis Routines et trois choix : tous les projets, tous les types, toutes les catégories. Au milieu, une carte, « L’étape suivante » : son titre, le projet dont elle fait partie, pourquoi elle vient maintenant, à peu près combien de temps elle prend et où vous en étiez, avec Commencer, Fait, Pas maintenant et Qu’est-ce qui rend cela difficile ? En dessous, « Ensuite » et l’étape d’après, sa date demandée en jours restants ; puis « Autres choix » et « Faites cette semaine », repliés.](../assets/screens/fr/tasks-now.png){ loading=lazy }](../assets/screens/fr/tasks-now.png "Ouvrir l’image en grand")
  <figcaption>Maintenant : une étape, pourquoi elle vient maintenant, et celle d’après.</figcaption>
</figure>

- **Une nouvelle tâche** : **Nouvelle tâche**, ou une ligne en haut de la page. `Appeler les impôts demain ~15m {30/10}` crée une tâche qui peut commencer demain, prend un quart d’heure, et est demandée pour le 30 octobre.
- **Pas maintenant** est respecté sans discussion : l’étape revient demain, et l’étape suivante vient d’autre chose.
- **Comment est la journée ?** En haut de la page, **Clair**, **Brume** ou **Brouillard** : une journée difficile en tient moins. C’est vous qui le dites ; rien n’est deviné de ce que vous faites.
- **La journée** dispose aujourd’hui : vos événements, vos repas, et les étapes que le plan donne aujourd’hui, aux heures faites pour elles.

Vos tâches sont des tâches standards, sur votre serveur d’agenda elles aussi.

La suite : [Les tâches](tasks.md)

### Les contacts : qui peut vous joindre {#contacts}

La page **Contacts** liste les personnes à qui vous avez affaire, leurs noms d’abord ; quelques lettres en trouvent une. Depuis un courriel, **Ajouter aux contacts** fait de son expéditeur un contact en un clic.

Les contacts font ici plus que garder des adresses. Chaque personne est sur une liste, et la liste décide quand ses courriels, ses appels et ses messages vous parviennent.

| Liste | Qui s’y trouve |
|---|---|
| **Sûrs** | la famille, les amis, les collègues choisis ; personne d’autre que vous n’y met quelqu’un |
| **Vos contacts** | toute personne de vos carnets d’adresses, tant que vous ne choisissez pas autrement |
| **Restreints** | les personnes que vous préférez n’entendre qu’aux moments que vous choisissez |
| **Bloqués** | jamais, par aucun canal |
| **Inconnus** | tous les autres ; leur premier courriel attend votre accord, sous le Porche |

**Comment cette personne vous joint**, sur la fiche d’un contact, montre sa liste et la change. Ce que fait chaque liste à chaque moment de votre journée est dans **Paramètres ▸ Ce qui vous joint**, dit en phrases.

La suite : [Les contacts](contacts.md) · [Ce qui vous joint, et quand](notifications.md)

## Un second appareil {#a-second-device}

Chaque appareil garde toutes ses données, et fonctionne sans les autres. Trois choses les tiennent accordés, chacune à sa façon.

| Quoi | Comment cela parvient à vos autres appareils |
|---|---|
| Les courriels, les contacts, les événements et les tâches | par leurs propres serveurs, comme avec n’importe quel logiciel |
| Ce que Sioul garde sur l’appareil : vos paramètres, vos comptes (jamais leurs mots de passe), vos heures, qui peut vous joindre, les liens entre les choses, les brouillons, et ses propres listes (projets, budgets…) une fois sorties de votre dossier de notes | par le **partage** de Sioul : un dossier qui porte des changements scellés |
| Votre dossier de notes | par sa propre application de synchronisation, ou par le partage une fois que vous l’activez |

Une application de synchronisation que vous utilisez déjà porte le dossier du partage (Nextcloud, Dropbox, Syncthing, Google Drive, OneDrive, ou une autre), ou bien Sioul le porte lui-même quand vous avez un compte Nextcloud. Chaque changement est chiffré sur votre appareil avant d’y être écrit, avec une clé faite d’une phrase de passe que vous choisissez : le dossier et son serveur ne peuvent rien lire. Il n’y a aucun serveur à nous.

### Sur le premier appareil : commencer le partage {#start-sharing}

1. Ouvrez **Paramètres ▸ Partage**, et trouvez **Entre vos appareils**.
2. **Dossier** : choisissez un dossier dans celui que votre application de synchronisation porte, par exemple un nouveau dossier `Sioul` dans votre dossier Nextcloud. Avec un compte Nextcloud dans Sioul, vous pouvez choisir à la place **Sioul le tient à jour avec votre Nextcloud**, sans application de synchronisation.
3. **Phrase de passe**, puis **Encore une fois** : quelques mots que vous n’oublierez pas, au moins 12 caractères.
4. **Partager**.

!!! warning "Une phrase de passe perdue ne se retrouve pas"
    Elle n’est jamais envoyée nulle part : personne ne peut donc la retrouver pour vous. Si elle est perdue, arrêtez le partage sur chaque appareil et recommencez avec un nouveau dossier. Rien n’est perdu sur vos appareils.

### Sur le nouvel appareil : rejoindre {#join}

Installez Sioul et ouvrez-le. Son Porche propose **Rejoindre vos autres appareils**, par trois chemins :

- **Se connecter à Murena**, ou **Se connecter à un autre Nextcloud** : la page de connexion de votre Nextcloud s’ouvre dans votre navigateur, puis Sioul y trouve votre dossier de partage.
- **J’utilise une application de synchronisation** : choisissez le même dossier, tel que votre application de synchronisation le montre sur cet appareil.

Tapez votre phrase de passe, une fois, et choisissez **Rejoindre**. Un premier échange apporte vos paramètres, vos comptes et vos heures. Les mots de passe ne voyagent jamais : **Reste à vous connecter sur cet appareil** liste les comptes qui demandent le leur, les courriels surtout. Donnez à chacun son mot de passe dans **Comptes**.

Sur un ordinateur qui partage déjà, **Ajouter un appareil**, dans le même panneau, dit ce dont le nouvel appareil a besoin.

### À quoi vous attendre {#what-to-expect}

- **Vos règles vous suivent.** Les heures, les listes et le « ne pas déranger » réglés sur un appareil valent sur les autres, le plus souvent en quelques minutes.
- **Chaque appareil choisit ce qui voyage depuis lui**, partie par partie, sous **Ce qui voyage depuis cet appareil**.
- **Mettez vos appareils à jour ensemble quand vous le pouvez.** Des appareils sur des versions différentes continuent de partager, mais une partie dont la forme est plus récente que ce qu’un appareil sait lire attend sur cet appareil jusqu’à sa mise à jour.
- **Sur Android**, une application de synchronisation n’apporte souvent les changements que toutes les demi-heures. Quand le dossier est sur votre Nextcloud, Sioul y lit lui-même les nouvelles de vos autres appareils, chaque minute tant qu’il est ouvert.

La suite : [Partager entre vos appareils](sharing.md)

## Ensuite {#next}

- [Aller plus loin](going-further.md) : votre filtre à indésirables et vos filtres de courriel, les thèmes et les autres étiquettes, les tâches qui s’attendent, et les personnes pour qui vous faites des démarches.
- [Premiers pas](first-steps.md) : la fenêtre et ses touches, la connexion de chaque fournisseur, le dossier des notes, les sites que vous épinglez, et Sioul sur un téléphone.
- [Les heures](hours.md) et [Ce qui vous joint, et quand](notifications.md) : les deux réglages qui façonnent tout le reste.
