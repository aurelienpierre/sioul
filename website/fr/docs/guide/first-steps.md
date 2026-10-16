---
description: "La fenêtre, ses touches, et les premières choses à régler dans Sioul : courriels, agendas et contacts, Google, votre dossier de notes, vos heures, vos sites."
---

# Premiers pas {#first-steps}

Rien n’est à régler d’un coup. Chaque étape ci-dessous est utile seule, et Sioul fonctionne avec ce que vous lui donnez. Ouvrez-le depuis votre menu d’applications, ou avec `sioul-app` dans un terminal.

Sioul parle anglais ou français, selon la langue de votre système, sauf si vous en choisissez une autre dans **Paramètres ▸ Affichage ▸ Langue**.

## La fenêtre {#the-window}

<figure markdown="span">
  [![La fenêtre de Sioul : en haut, sur toute la largeur, sa barre de titre, la ligne d’état avec les touches, le bouton des sons, la météo, Temps libre et Pause, puis les boutons de la fenêtre ; à gauche, une étroite colonne d’icônes, Nouveau (un plus), puis les lieux, du Porche à Santé, celui affiché marqué, et trois icônes en bas (Comptes, Paramètres, Tout actualiser) ; à droite, le Porche.](../assets/screens/fr/porch.png){ loading=lazy }](../assets/screens/fr/porch.png "Ouvrir l’image en grand")
</figure>

**En haut**, sur toute la largeur de la fenêtre, la ligne d’état est sa barre de titre. Elle dit en une phrase ce qui s’est passé en dernier. Après chaque déplacement, suppression ou envoi, « Annuler » y attend dix secondes. Pendant le calme, elle dit quand le travail revient ; une fois les heures du jour finies, elle propose de clore la journée de travail, et le soir la journée. Plus loin se trouvent les touches, [Ne pas déranger](notifications.md#do-not-disturb-on-every-device), le bouton des sons ([des sons pour se concentrer ou se reposer](tasks.md#sounds)), la météo d’un lieu que vous choisissez, et à son extrémité [**Temps libre** et **Pause**](pauses.md).

- **Son extrémité gauche**, au-dessus des lieux, porte le bouton qui affiche le nom des lieux à côté de leur icône, dans une colonne plus large, ou ne garde que leurs icônes (++f9++) : le même choix que **Paramètres ▸ Affichage ▸ Afficher le nom des lieux à côté de leur icône**.
- **Les boutons de la fenêtre**, réduire, agrandir ou restaurer, et fermer, sont du côté où votre système les met, dans son ordre : KDE Plasma, GNOME et Xfce le disent dans leurs réglages ; à gauche sur un Mac, à droite sous Windows.
- **Faites-la glisser** là où rien d’autre ne se trouve, la phrase comprise, pour déplacer la fenêtre ; **double-cliquez** dessus pour agrandir la fenêtre ou lui rendre sa taille. Les bords de la fenêtre la redimensionnent.

**À gauche**, une étroite colonne d’icônes, de haut en bas :

- **Nouveau ▾**, le **+**, crée une chose de n’importe quel type : un courriel, une tâche, un événement, un contact, une note, un projet, du temps passé, un mouvement de budget, un papier, le scan d’une lettre papier (sur un ordinateur : [Le Porche](porch.md#paper-letters)) ; et **Où j’en suis…** laisse une ligne sur où vous en êtes ([Les tâches](tasks.md#starting-and-stopping)).
- **Les lieux**, une icône chacun : Porche (une corbeille à courrier), Tâches (une liste à cocher), Courriels (une enveloppe), Sites (un globe et ses méridiens), Agenda (un calendrier), Contacts (une personne et des lignes), Notes (un bloc-notes), Projets (un dossier), Temps (une horloge), Budgets (un portefeuille), Papiers (une carte), Santé (un cœur). Le lieu affiché est marqué d’une teinte douce et d’une courte barre.
- **À part en bas** : **Clore la journée de travail** (une liste cochée) et, dessous, **Clore toute la journée** (une lune), à toute heure ([la fin de la journée](tasks.md#the-end-of-the-day)) ; puis trois icônes, côte à côte quand les noms s’affichent : Comptes (une personne), Paramètres (des curseurs), et Tout actualiser, qui relève à nouveau, d’un coup, les courriels, l’agenda, les tâches et les contacts.

Le nom de chaque icône, avec sa touche, s’affiche quand le pointeur s’y arrête, quand le clavier l’atteint, ou par un appui long sur un écran tactile.

**Sur chaque page**, le ⚙ au bout de la première ligne contient les réglages propres à cette page, chacun avec une phrase sur ce qu’il change ; ils sont enregistrés aussitôt. La lecture du texte long (la police, sa taille et l’interligne) est dans le ⚙ des Courriels et du Porche, et derrière « Aa » dans les Notes. Un clic droit, la touche Menu, ou un appui long sur un écran tactile, sur n’importe quel élément donne ce qui n’est pas en vue.

### Touches {#keys}

Tout se fait au clavier : Tab pour se déplacer, Entrée pour choisir, Échap pour revenir. Sur un ordinateur, la ligne d’état rappelle les principales touches quand rien d’autre n’a besoin de sa place ; jamais sur un téléphone, tenu droit ou de côté.

| Touches | Ce qu’elles font |
|---|---|
| ++ctrl+1++ à ++ctrl+9++, ++ctrl+0++ | les dix premiers lieux, dans l’ordre de la liste : Porche, Tâches, Courriels, Sites, Agenda, Contacts, Notes, Projets, Temps, Budgets |
| ++ctrl+n++ | Nouveau ▾ |
| ++ctrl+z++ | Annuler, tant que c’est proposé |
| ++f5++ | Tout actualiser |
| ++f9++ | Le nom des lieux à côté de leur icône, ou leurs icônes seules ; dans une fenêtre étroite, sortir les lieux ou les ranger |
| ++f11++ | Plein écran, ou en sortir |
| ++ctrl+q++ | Quitter Sioul (fermer la fenêtre la masque dans la zone de notification) |
| ++ctrl+enter++ | Envoyer, dans la fenêtre de rédaction |

### Sur un téléphone {#on-a-phone}

Sioul sur un téléphone : [Installer](install.md#on-android). Sur un téléphone, ou dans une fenêtre de moins de 720 pixels de large :

- **Les lieux** glissent depuis la gauche, derrière ☰, avec leur nom à côté de leur icône ; une barre en haut nomme la page.
- **La ligne d’état** est en bas de l’écran d’un téléphone, chacun de ses boutons réduit à son icône : un appui long sur l’un dit ce qu’il est. Quand sa phrase est coupée, un appui dessus la montre en entier.
- **Un volet à la fois** : une page montre sa liste, puis ce que vous ouvrez sur tout l’écran ; **Retour**, sur la barre ou celui d’Android, revient en arrière.
- **Les menus** s’ouvrent par un appui long sur un écran tactile, là où une souris ferait un clic droit.

Ce qui change sur un téléphone :

- **Les notifications** viennent pour les prises de médicaments ([La santé](health.md#reminders)), le temps qui court ([Le temps](time.md#where-time-comes-from)), les événements ([L’agenda](agenda.md#reminders)) et les nouveaux courriels à leurs heures ([Le Porche](porch.md#new-mail-told-at-its-times)). Un code que vous avez demandé s’affiche sur le Porche ; les autres rappels n’y viennent pas encore.
- **Les appels** peuvent être filtrés une fois que vous laissez Sioul être « l’appli numéro de l’appelant et spam » d’Android : qui peut sonner, et quand, comme le dit sa ligne dans Ce qui vous joint ; les autres vont sur votre messagerie ([Les appels](calls.md)).
- **Les notifications des autres applications** peuvent être retenues jusqu’à leur heure, une fois que vous donnez à Sioul l’accès aux notifications d’Android : les messages des personnes selon qui les écrit, le reste aux heures de regroupement ([Paramètres ▸ Ce téléphone](settings.md#this-phone)).
- **Les courriels** sont relevés tant que Sioul est ouvert : Android l’arrête en arrière-plan.
- **Le partage** : Sioul est dans le menu de partage d’Android, et chacune de vos adresses aussi ; les liens de courriel s’ouvrent dedans ([Les courriels](mail.md#from-other-apps)).
- **Les dossiers** : le **Choisir…** d’un réglage ouvre la liste des dossiers du téléphone, propre à Sioul, avec **Autoriser l’accès aux fichiers** quand Android n’a pas encore donné cet accès à Sioul.
- **Les sites** s’ouvrent dans votre navigateur ([Les sites](sites.md)) ; les PDF s’ouvrent dans une autre application, avec **Ouvrir avec…**.
- **Le courrier papier** n’est pas lu, et les pièces jointes ne passent pas par un antivirus : les programmes dont Sioul se sert pour cela sur un ordinateur n’existent pas sur un téléphone.
- **Vos autres appareils** partagent avec lui par un dossier que l’application de synchronisation du téléphone transporte, ou que Sioul tient lui-même à jour avec votre Nextcloud ; un nouveau téléphone les rejoint par la connexion de Murena, ou d’un autre Nextcloud, depuis **Rejoindre vos autres appareils** sur le Porche ([plus bas](#join-your-other-devices)). Les mots de passe ne voyagent jamais : chaque compte demande le sien, une fois ([Les comptes](accounts.md#an-account-from-your-other-device)).
- **Des cartes sur l’écran d’accueil** : les widgets de Sioul (un appui long sur l’écran d’accueil, **Widgets**). La carte complète, « Sioul », commence par aujourd’hui : la date, le temps qu’il fait et, sur la même ligne, celui de chacune des quatre prochaines heures, les prochains matin, après-midi, soir et nuit, puis les sept prochains jours, au lieu choisi pour la météo (touchez la météo dans la ligne d’état de Sioul pour en choisir un). En dessous, ce que le Porche a pour vous, en une courte ligne chacun : une prise prévue, jamais comme « pas prise » (« vérifiez avant de la prendre » quand un autre appareil peut savoir), un code que vous venez de demander à un site, les rappels de dates, d’attentes, de paiements et de papiers, un appel que Sioul a refusé, deux événements en même temps. Enfin, ce à quoi sert ce moment et, pendant le travail ou les démarches, la prochaine étape. Jamais de compte. Un appui sur une prise, un code ou un appel ouvre le Porche ; sur un rappel, ce qu’il concerne ; sur l’étape, **Maintenant**. Les courriels et vos événements ont leurs propres cartes, « Sioul — Courriels » et « Sioul — Agenda », dans la même liste de widgets, à placer et ordonner comme vous voulez : les derniers courriels du Porche (qui a écrit, en gras tant que vous ne l’avez pas lu, quand, l’objet, une ligne du texte ; en dehors des heures choisies pour les courriels, seulement quand le Porche ouvre), et vos prochains événements, jour par jour, chacun dans la couleur de son agenda. Sans le réglage **Détails sur l’écran d’accueil** ([Paramètres](settings.md#display)), elles ne nomment aucun expéditeur, objet, événement, code, prise, rappel, appel ni étape ; la date et la météo restent.

## Rejoindre vos autres appareils {#join-your-other-devices}

Vous utilisez déjà Sioul sur un autre appareil ? Sur un appareil qui ne partage pas encore, la première carte du Porche, **Rejoindre vos autres appareils**, amène ici ses réglages, ses comptes et vos heures : **Se connecter à Murena**, ou **Se connecter à un autre Nextcloud**, ouvre dans votre navigateur la page de connexion de votre Nextcloud ; Sioul y trouve ensuite votre dossier de partage et demande votre phrase de passe, une fois. **J’utilise une application de synchronisation** mène au dossier que votre application de synchronisation transporte. Les étapes ci-dessous sont alors en grande partie faites : il reste le mot de passe de chaque compte de courriel, listé une fois la connexion faite ([Le partage](sharing.md#a-new-device-through-your-nextclouds-sign-in)). **Laisser ainsi** range la carte ; **Paramètres ▸ Votre dossier et le partage** propose la même chose à tout moment.

## Ajouter vos courriels {#add-your-mail}

1. Ouvrez **Comptes** (l’icône en forme de personne), puis l’onglet **Ajouter un compte**.
2. Sous « Ajouter un compte de courriel », tapez votre **adresse électronique** et choisissez **Trouver le serveur**.
3. Sioul dit ce qu’il a trouvé et d’où : les réglages de votre fournisseur lui-même, la liste des fournisseurs de Thunderbird, ou une supposition, dite comme telle, à vérifier avant d’aller plus loin.
4. Tapez votre **mot de passe**. Il va dans le trousseau de votre système, nulle part ailleurs.
5. Choisissez **Se connecter et ajouter**. Sioul essaie le mot de passe avant de garder quoi que ce soit, puis relève vos courriels récents.

Certains fournisseurs demandent un **mot de passe d’application** au lieu de votre mot de passe habituel quand la validation en deux étapes est activée : vous le créez dans les réglages de votre compte chez le fournisseur. Gmail et Google Workspace : ci-dessous.

Outlook.com, Hotmail, Live, MSN et Microsoft 365 ne prennent aucun mot de passe : vous vous connectez sur la page de Microsoft elle-même, ci-dessous.

Relever les courriels ne change rien sur votre serveur de courriel. Sioul n’y écrit que lorsque vous agissez : ouvrir un courriel le marque comme lu, comme dans tout logiciel de courriel ; archiver, supprimer et déplacer se font dix secondes après votre demande, pour que « Annuler » puisse les arrêter.

Ensuite, sur la fiche de l’adresse dans **Vos comptes**, dépliez **Réglages de cette adresse** et cochez **À quoi sert cette adresse** : travail, vos démarches, loisirs, ou plusieurs. Tant que vous ne l’avez pas dit, une adresse compte comme du travail, pour que ses courriels n’atteignent jamais vos soirées. Voir [Les heures](hours.md).

### Gmail et Google Workspace {#gmail-and-google-workspace}

Google refuse le mot de passe de votre compte dans les autres logiciels de courriel. Sioul reconnaît les courriels de Google à leur adresse (gmail.com, googlemail.com) ou aux serveurs qui reçoivent les courriels de votre domaine (Google Workspace), et propose deux façons au lieu d’un mot de passe :

- **Utiliser un mot de passe d’application** : créez pour Sioul un mot de passe d’application sur myaccount.google.com/apppasswords (il demande la validation en deux étapes ; **Créer un mot de passe d’application** ouvre la page dans votre navigateur), collez-le sous **Mot de passe d’application (de Google)**, puis **Se connecter et ajouter**.
- **Se connecter avec Google** : la page de Google elle-même s’ouvre dans votre navigateur. Pour les courriels, Google n’accepte qu’une clé Google à vous, pas celle de Sioul : un projet gratuit dans Google Cloud, fait une fois (**Faire votre clé Google** déplie les étapes), celui de vos agendas Google si vous l’avez fait. Google dit ensuite que l’application n’est pas validée : c’est la vôtre ; choisissez *Paramètres avancés*, puis *Accéder à Sioul*, et laissez cochée la case de l’accès à Gmail. Publiez le projet (*Audience → Publier l’application*) : s’il reste en test, Google coupe l’accès tous les sept jours.

Une fois votre clé gardée sur cet appareil, **Se connecter avec Google** est la façon choisie d’abord, et ne demande rien d’autre. Sur un téléphone, quand Google dit que Sioul a l’accès, revenez à Sioul : il termine là. Si Android a fermé Sioul entre-temps, rien n’a changé : reconnectez-vous.

### Outlook.com, Hotmail et Microsoft 365 {#outlook-and-microsoft-365}

Microsoft ne prend aucun mot de passe des autres logiciels de courriel, mots de passe d’application compris. Sioul reconnaît les courriels de Microsoft à leur adresse (outlook.com, hotmail.com, live.com, msn.com et leurs versions par pays, comme hotmail.fr) ou aux serveurs qui reçoivent les courriels de votre domaine (Microsoft 365), et les connecte plutôt sur la page de Microsoft elle-même :

1. Après **Trouver le serveur**, choisissez **Se connecter avec Microsoft**. La page de Microsoft s’ouvre dans votre navigateur, votre adresse déjà remplie. Choisissez votre compte si Microsoft demande lequel, connectez-vous comme Microsoft le demande (votre mot de passe, votre téléphone, votre clé de sécurité), et acceptez ce que Sioul demande : lire et envoyer vos courriels, et rester connecté.
2. Sioul attend la réponse de Microsoft cinq minutes au plus, puis ouvre votre boîte avec l’accès donné avant de rien garder. Si seule s’ouvre la boîte du compte connecté sur la page de Microsoft, pas celle de votre adresse (un alias, ou un autre de vos comptes choisi là), Sioul nomme les deux et demande avant de l’utiliser. L’accès reste dans le trousseau de votre système, et rien ne part chez le développeur de Sioul.

Ce que chaque sorte d’adresse demande :

- **Outlook.com, Hotmail, Live, MSN** : Outlook.com garde les logiciels de courriel dehors tant que vous ne les laissez pas entrer, une fois. Dans les paramètres d’Outlook.com, sous Courrier ▸ Transfert et IMAP, activez l’accès IMAP des appareils et des applications, puis enregistrez ; **La page de Microsoft à ce sujet** ouvre les explications de Microsoft. Si Sioul dit que la boîte reste fermée alors que c’est activé, confirmez que la connexion est la vôtre sur account.live.com/activity.
- **Microsoft 365, professionnel ou scolaire** : votre organisation doit peut-être approuver Sioul une fois, car par défaut Microsoft ne laisse les employés approuver l’accès aux courriels que pour quelques logiciels de son choix. Si la page de Microsoft dit que seul un administrateur le peut, dépliez **Ce que fait votre administrateur**, copiez-le et envoyez-le-lui : le lien qui approuve Sioul pour toute votre organisation, et les deux interrupteurs de votre boîte (IMAP, et le SMTP authentifié pour l’envoi).
- **Une adresse Microsoft 365 que Sioul ne reconnaît pas** (les courriels de votre organisation passent d’abord par un autre service) : quand **Trouver le serveur** ne trouve rien, choisissez **Cette adresse est chez Microsoft**.
- **Votre propre application Microsoft** : dans une copie de Sioul qui n’en porte pas, le formulaire le dit et demande l’ID d’application (client) de la vôtre. Inscrivez-en une, une fois, dans Microsoft Entra (**Comment inscrire votre application** déplie les étapes) ; elle ne demande aucun secret. Dans une copie qui en porte une, **Utiliser une application à moi** est là si vous préférez la vôtre.

Sur un téléphone, revenez à Sioul dès que la page de Microsoft dit que c’est fait : la réponse de Microsoft dure environ une minute. Si elle est arrivée trop tard, Sioul le dit ; se reconnecter va plus vite la deuxième fois.

Sioul n’atteint pas les agendas et les contacts de Microsoft : Microsoft les propose par sa propre interface, pas par CalDAV ou CardDAV.

## Ajouter vos agendas, tâches et contacts {#add-your-calendars-tasks-and-contacts}

Depuis un serveur CalDAV et CardDAV : Nextcloud, Fastmail, iCloud, votre hébergeur.

1. Dans **Comptes ▸ Ajouter un compte**, sous « Ajouter des contacts et agendas », tapez votre adresse et votre mot de passe.
2. Si Sioul ne trouve pas le serveur à partir de l’adresse, dépliez **Adresse du serveur, s’il ne se trouve pas** et donnez-la.
3. Choisissez **Se connecter et ajouter**. Vos carnets d’adresses et vos agendas arrivent ; les tâches viennent avec les agendas qui les contiennent.

Votre compte de courriel est déjà ajouté ? Sur sa fiche dans **Vos comptes**, **Ce que ce serveur offre** interroge le serveur : ses agendas et ses contacts, et pour un Nextcloud, sa version et ses applications. Un clic les ajoute.

## Ajouter Google {#add-google}

Google n’accepte aucun mot de passe venant d’autres programmes : vous vous connectez sur la page de Google elle-même.

1. Dans **Comptes ▸ Ajouter un compte**, sous « Agendas, contacts et tâches Google », tapez votre adresse Google.
2. Choisissez **Se connecter avec Google**. Votre navigateur s’ouvre sur la page de Google ; Sioul attend sa réponse cinq minutes au plus.
3. Connectez-vous, et autorisez ce que Sioul demande : vos agendas, vos contacts et vos tâches.

Sioul garde l’accès dans le trousseau de votre système. Google garde moins de choses qu’un serveur ouvert : ce qu’il ne garde pas apparaît grisé dans Sioul, jamais caché, avec la raison. Ce que Sioul lit et écrit dans votre compte Google, et comment retirer l’accès : [Politique de confidentialité](../privacy.md#google-calendars-contacts-and-tasks).

!!! note "Si Sioul demande une clé Google"
    Une copie de Sioul construite sans sa propre clé Google demande la vôtre. **Faire votre clé Google** déplie les étapes dans Comptes : un projet gratuit dans Google Cloud, un quart d’heure environ, une fois. Vous pouvez aussi choisir **Utiliser une clé Google à moi** à tout moment.

Les courriels de Google s’ajoutent dans le formulaire des courriels, plus haut : avec un mot de passe d’application, ou connectés avec Google ([Gmail et Google Workspace](#gmail-and-google-workspace)).

## Choisir votre dossier de notes {#choose-your-notes-folder}

Vos notes sont un dossier de fichiers Markdown : un coffre Obsidian fonctionne tel quel. Sioul garde aussi vos projets, vos budgets, vos papiers et vos lettres scannées dans ce dossier, pour qu’ils voyagent avec lui jusqu’à vos autres appareils.

Dans **Paramètres ▸ Votre dossier et le partage**, choisissez **Le dossier des notes**. Sioul le lit et y fait des liens ; il ne se l’approprie jamais. Voir [Les notes](notes.md).

## Régler vos heures {#set-your-hours}

Dans **Paramètres ▸ Heures** : vos heures de travail et vos heures pour vos démarches ; tout le reste du temps, ce sont vos loisirs, et les repas et le sommeil viennent de la page Santé. Sans heures, tout arrive à toute heure, comme dans les autres logiciels de courriel. Tant qu’elles ne sont pas réglées, le Porche demande une fois, avec **Régler mes heures** et **Laisser ainsi**. Voir [Les heures](hours.md).

## Épingler les sites que vous consultez {#pin-the-websites-you-check}

Les messageries sécurisées de votre banque, de l’Assurance maladie, des impôts ; une discussion ; un appel vidéo.

Sur la page **Sites**, **Sites courants ▾** en liste environ 400, par pays, ou **Épingler un site** en trouve un par un mot (« banque », « ameli »), ou prend n’importe quelle adresse à la main. Vous vous connectez une fois ; le site garde votre session. Voir [Les sites](sites.md).

## Garder Sioul ouvert {#keep-sioul-open}

Sur un ordinateur, Sioul s’ouvre agrandi ; ++f11++ le met en plein écran, et l’en sort. Son icône dans la zone de notification affiche ou masque la fenêtre : fermer la fenêtre la masque là, et Sioul continue derrière, courriels, rappels et médicaments compris. **Quitter Sioul** est dans le menu de l’icône, ou ++ctrl+q++ ; ce menu a aussi **Clore la journée de travail** et **Clore toute la journée**. Sans zone de notification (GNOME sans son extension), fermer la fenêtre quitte Sioul.

Tant que sa fenêtre est ouverte, Sioul garde chaque boîte de réception ouverte sur le serveur : un code ou un lien de connexion que vous avez demandé à un site vous parvient en quelques secondes, en une seule notification discrète, à n’importe quelle heure.

Les rappels peuvent aussi venir fenêtre fermée : dans **Paramètres ▸ Rappels**, cochez **Fenêtre de Sioul fermée**. Un petit programme de veille démarre alors avec votre session ; il ne relève pas les courriels.

## Ensuite {#next}

- [Le Porche](porch.md), où les nouveaux courriels attendent.
- [Les tâches](tasks.md), et l’étape suivante.
- [Les heures](hours.md), et le calme.
- [Les notifications et « Ne pas déranger »](notifications.md) : ce que Sioul vous dit et quand, et ce qui attend.
- [Fonctionne avec](compatibility.md) : les serveurs, les applications et les systèmes avec lesquels Sioul fonctionne, et jusqu’où chacun a été essayé.
