---
description: Votre administratif reste utilisable sans Sioul. Sioul relie des protocoles et des fichiers standard (IMAP, CalDAV, CardDAV, WebDAV, notes en Markdown, fichiers PDF, images, audio) et n’a aucun format de fichier privé ; il rassemble seulement la gestion de votre administratif et les liens entre les choses, dans une même application sur chaque système ; ses extensions changent des services fermés en agendas et en carnets d’adresses ordinaires que chacun peut partager. Ce qui ouvre chaque type de données sans Sioul, ce qui est propre à Sioul et comment le sortir, et là où ce n’est pas encore vrai.
---

# Votre administratif reste utilisable sans Sioul {#without-sioul}

L’interopérabilité est le socle de Sioul. Quatre choses en découlent :

- **Sioul relie ce qui est standard.** Vos courriels arrivent par IMAP et partent par SMTP ; votre agenda et vos tâches sont sur un serveur CalDAV, vos contacts sur un serveur CardDAV ; le partage entre vos appareils peut passer par un serveur WebDAV, comme votre Nextcloud. Vos notes sont des fichiers Markdown, vos papiers et vos courriers des fichiers PDF et des images, un mémo vocal un fichier audio.
- **Il n’y a aucun format de fichier privé.** Par conception, tout reste lisible et modifiable par les autres programmes qui traitent les mêmes formats standard.
- **Sioul ne rassemble que la gestion de votre administratif, et les liens entre les choses.** Vos courriels, votre agenda, vos tâches, vos contacts, vos notes, vos papiers et votre argent se retrouvent dans une même application, qui garde les liens entre eux, et cette application est la même sur chaque système que vous utilisez : Linux, Windows, macOS et Android (iOS pas encore). Vos données elles-mêmes restent à leurs places standard.
- **Les extensions changent des services fermés en agendas et en carnets d’adresses standard.** Une extension lit un service qui n’offre aucun standard, comme l’emploi du temps d’un établissement scolaire ou les réservations d’un salon, et écrit ce qu’elle lit dans un agenda ou un carnet d’adresses à vous, sur votre propre serveur. Vous pouvez partager cet agenda avec des personnes qui n’utilisent pas Sioul : elles le lisent et, si vous leur permettez de le modifier, y écrivent avec leurs propres programmes, comme pour n’importe quel agenda ([plus bas](#a-calendar-an-extension-fills)).

Si Sioul s’arrêtait demain, vos courriels, votre agenda, vos contacts, vos notes et vos papiers seraient toujours là où ils sont, et les programmes de votre choix les ouvriraient. Là où ce n’est pas encore vrai est dit simplement à la fin de cette page ([plus bas](#where-it-does-not-hold-yet)).

## Ce qui ouvre vos données sans Sioul {#what-opens-your-data}

| Quoi | Gardé sous la forme de | Ouvert sans Sioul par |
|---|---|---|
| Courriels | sur votre serveur de courriel (IMAP), et sur votre appareil, un fichier par message (Maildir) | n’importe quel logiciel de courriel : Thunderbird, l’application de courriel de votre téléphone, le webmail de votre fournisseur ; sur un ordinateur, les logiciels qui lisent le Maildir, comme notmuch ou mutt |
| Agenda et tâches | sur votre serveur d’agendas (CalDAV), et sur votre appareil, un fichier par événement ou par tâche (iCalendar, `.ics`) | n’importe quel logiciel d’agenda : Thunderbird, l’agenda de votre téléphone avec DAVx⁵, Agenda et Tâches de Nextcloud, Tasks.org ; sur un ordinateur, khal |
| Contacts | sur votre serveur de contacts (CardDAV), et sur votre appareil, un fichier par fiche (vCard, `.vcf`) | n’importe quel carnet d’adresses : celui de votre téléphone, de Thunderbird, de Nextcloud ; sur un ordinateur, khard |
| Notes | un dossier de fichiers Markdown que vous choisissez, hors des dossiers propres à Sioul | Obsidian, Nextcloud Notes, n’importe quel éditeur de texte |
| Papiers et courriers papier | les fichiers tels que vous les avez donnés (PDF, images), dans des dossiers à vous (`Documents/Papers` et `Documents/Letters` sauf si vous en avez choisi d’autres) ; le texte d’un courrier, lu sur sa numérisation, en texte brut | n’importe quel gestionnaire de fichiers, lecteur de PDF ou visionneuse d’images |
| Les factures que vous émettez | un fichier PDF/A-3, qui porte sa facture électronique Factur-X (la norme européenne EN 16931) quand la facture a ce que cette norme demande (en franchise en base de TVA sans numéro de TVA, pas encore), vérifié par un validateur extérieur chaque fois que la façon dont Sioul fait les factures change | n’importe quel lecteur de PDF ; les logiciels de comptabilité et les plateformes de facturation électronique qui lisent Factur-X |
| Vos SMS | sur votre téléphone, dans le stockage propre d’Android | l’application de messages de votre téléphone, et les applications qui sauvegardent vos SMS |
| Les lignes de votre banque | l’export de votre banque (OFX, CAMT.053 ou CSV), que Sioul lit | votre banque, un tableur |

Sioul garde une copie de vos courriels, de votre agenda et de vos contacts sur chaque appareil, pour fonctionner sans attendre le réseau. Le serveur garde l’original, et chacun des autres programmes que vous utilisez sur le même compte voit la même boîte, le même agenda et les mêmes contacts : ce que chacun voit de ce que Sioul a fait est dans [Fonctionne avec](compatibility.md#other-apps-on-the-same-accounts).

## Ce qui est propre à Sioul, et comment le sortir {#what-is-sioul-s-own}

Certaines choses n’existent que parce que Sioul les garde pour vous : vos projets et les courriels qui arrivent seuls à chacun, les budgets, les mouvements d’argent et les contrats, les stocks, les fiches de vos clients, le temps passé sur les tâches, vos médicaments, vos proches, les liens entre les choses, vos règles et vos filtres, vos réglages, vos brouillons. Sioul les garde dans des fichiers texte simples (TOML, un format fait pour être lu et écrit à la main), jamais dans une base de données :

- **Où** : sous Linux, `~/.config/sioul/` (les réglages), `~/.local/share/sioul/` (les données ; les listes de Sioul dans son dossier `records/`) et `~/.local/state/sioul/` ; de même sous Windows et macOS, dans les dossiers propres au système ([Vie privée et sécurité](privacy-security.md#what-stays-on-your-computer)). Chacun contient des fichiers que vous pouvez ouvrir, copier et garder.
- **Dans vos événements, vos tâches et vos contacts**, Sioul écrit quelques lignes à lui (leur nom commence par `X-SIOUL-` : le temps gardé autour d’un rendez-vous, le projet d’une tâche, ses liens). La norme des agendas demande à chaque programme de garder de telles lignes ; un autre programme montre un événement ou une tâche ordinaire, avec son heure, son titre et son lieu.
- **En ligne de commande**, sur un ordinateur, sans la fenêtre : `sioul invoices export` donne toutes les factures en tableau, ou une facture sous forme de son PDF ; `sioul receipts --csv` le livre des recettes ; `sioul movements --json` chaque mouvement d’argent ; `sioul links <adresse> --chain --json` les liens d’une chose ; `sioul projects` vos projets.
- **Ce qui n’est que de la mécanique** n’a rien à sortir : la table du filtre anti-spam se refait par son entraînement, et les caches se refont à partir du reste.

Chaque type de données, où il vit, son format et le moyen de le sortir, vérifié ligne à ligne dans le code de Sioul : [la note pour les développeurs](https://sioul.aurelienpierre.com/dev/formats.html) (en anglais).

## Entre vos appareils {#between-your-devices}

Vos courriels, et les agendas et carnets d’adresses de vos serveurs, voyagent par ces serveurs, jamais par le partage de Sioul : tous les programmes les y voient.

Ce que seul Sioul garde voyage entre vos appareils par le [dossier de partage](sharing.md), scellé de bout en bout sur votre appareil avant de partir : aucun serveur, et personne qui détient le dossier, ne peut le lire. Ce qu’il contient, ce sont les mêmes fichiers simples : quand votre autre appareil ouvre un changement, il l’écrit dans les mêmes fichiers TOML, et vos notes et vos papiers dans les mêmes fichiers Markdown, PDF et images. Chacun de vos appareils garde donc vos données dans les formats ouverts ci-dessus (vos SMS à part : [plus bas](#where-it-does-not-hold-yet)), et n’importe lequel de vos ordinateurs est un moyen de les sortir (un téléphone les garde dans son stockage privé : [plus bas](#where-it-does-not-hold-yet)). Le dossier scellé, seul, ne s’ouvre que dans Sioul, avec votre phrase de passe : gardez au moins un appareil, ou une sauvegarde des dossiers d’un appareil.

Vos notes et vos papiers sont des fichiers simples sur votre serveur quand votre propre application de synchronisation porte leur dossier (le client de Nextcloud, Syncthing, Dropbox). Portés par le partage de Sioul à la place (sur un téléphone dont l’application de synchronisation ne porte pas le dossier de notes), ils y sont scellés comme le reste.

## Un agenda qu’une extension remplit {#a-calendar-an-extension-fills}

Une extension, comme celle de Planity ou de Skolengo, écrit ses événements dans un agenda à vous, que vous choisissez dans [Paramètres ▸ Extensions](settings.md#extensions). C’est un agenda ordinaire sur votre serveur : partagez-le depuis Agenda ⚙ ▸ Vos agendas ([Vos agendas sur leurs serveurs](agenda.md#agendas-on-their-servers)) ou depuis la page de votre serveur : les personnes à qui vous permettez de le modifier le lisent et y écrivent avec n’importe quel logiciel d’agenda, sans Sioul ; n’importe qui d’autre peut s’abonner à son lien en lecture seule. Choisissez un agenda de votre serveur plutôt qu’un agenda de cet appareil seulement, que personne d’autre ne peut atteindre.

Sioul relit le service à son démarrage, puis toutes les trois heures tant que sa fenêtre est ouverte, et sur un téléphone pendant son étape en arrière-plan. À chaque fois, pour les jours auxquels le service a répondu :

- **Un événement que quelqu’un a changé** (son titre, une note, son heure) reste tel qu’il l’a laissé, tant que le service ne change pas cet événement. Quand le service le change (l’établissement déplace le cours), Sioul le réécrit depuis le service, et le changement est remplacé. La description de l’événement le dit : « une modification faite ici est remplacée quand Skolengo change cet évènement ».
- **Un événement que quelqu’un a supprimé** revient, tant que le service l’a encore : l’emploi du temps est celui de l’établissement.
- **Un événement que le service n’a plus** est retiré, même si quelqu’un l’a changé.
- **Un événement dont quelqu’un a fait une réunion**, en invitant des participants, est laissé tel quel à partir de là : l’extension ne change jamais une réunion, puisque son serveur préviendrait chaque participant.
- **Les événements que d’autres ajoutent eux-mêmes à cet agenda** ne sont jamais lus comme ceux de l’extension, jamais changés, jamais retirés.

Sioul reconnaît ses événements à leur identifiant, que tout programme garde, et note sur chacun, dans une ligne à lui, la version du service qu’il a écrite. La plupart des programmes gardent cette ligne, comme le demande la norme des agendas ; si un programme l’enlève, un autre de vos appareils qui fait tourner l’extension peut réécrire l’événement depuis le service, et le changement fait dans ce programme est remplacé.

Un carnet d’adresses qu’une extension remplit fonctionne de la même façon, plus doucement : Sioul ajoute seulement à une fiche ce qui lui manque, n’enlève jamais une ligne, ne remplace jamais ce que quelqu’un a changé, et ne supprime jamais une fiche.

**Rien ne retourne au service.** Un changement fait dans l’agenda, dans Sioul ou dans un autre programme, reste dans l’agenda seulement, jusqu’à ce que le service change cet événement (plus haut) : Sioul n’écrit ni à Planity ni à Skolengo, et la modification dans les deux sens n’est pas construite. Pour changer une réservation ou un cours pour de bon, changez-le auprès du service.

## Sur quels systèmes {#on-which-systems}

Linux, Windows, macOS et Android : GitHub construit Sioul pour chacun d’eux à chaque changement de son code, et passe ses tests sous Linux, Windows et macOS. Il sert tous les jours sous Linux et sur un téléphone Android. iOS : pas encore. [Installer](install.md)

## Là où ce n’est pas encore vrai {#where-it-does-not-hold-yet}

- **L’archive de vos SMS que garde Sioul**, sur chacun de vos appareils, est scellée, pour qu’une copie soit illisible sans la clé de votre partage. L’application de messages de votre téléphone reste la référence de vos SMS ; mais un SMS que vous supprimez sur le téléphone ne reste que dans l’archive de Sioul, et aucun autre programme ne l’ouvre.
- **Les brouillons** restent dans Sioul : ils ne sont pas écrits dans le dossier Brouillons de votre serveur, donc vos autres logiciels de courriel ne les voient pas.
- **Vos notes et vos papiers sur un serveur** n’y sont des fichiers simples que quand votre propre application de synchronisation porte leur dossier : Sioul ne les écrit pas encore lui-même en fichiers simples sur un serveur WebDAV. Portés par le partage de Sioul, ils y sont scellés, et Nextcloud Notes ou Obsidian ailleurs ne peuvent pas les ouvrir ([plus haut](#between-your-devices)).
- **Sur un téléphone Android**, les fichiers propres à Sioul sont dans son stockage privé, que les autres applications ne peuvent pas ouvrir. Vos notes y vont aussi tant que vous n’avez pas choisi de dossier de notes : choisissez-en un dans le stockage partagé du téléphone (dans Documents, par exemple) depuis la ⚙ de la page Notes, et les autres applications peuvent ouvrir vos notes. Les dossiers des papiers, des courriers et des factures peuvent y être aussi (choisis dans Papiers ▸ ⚙, Courrier ▸ ⚙ et Paramètres ▸ Factures). La ligne de commande (plus haut) est celle d’un ordinateur : un téléphone n’en a pas.
- **Le dossier de partage scellé** ne s’ouvre que dans Sioul, avec votre phrase de passe ([plus haut](#between-your-devices)).
- **Vos règles et vos filtres de courriel** ne tournent que dans Sioul ; un autre programme garde leur fichier mais ne les fait pas tourner.
- **Prévu** : Sioul scellera ses propres fichiers sur chaque appareil (ses réglages, ses listes et ses registres, ses copies de vos boîtes de courriel, de vos agendas et de vos carnets d’adresses), avec une commande pour les sortir en fichiers standard ; vos notes, vos papiers et vos courriers resteront des fichiers simples, en dehors, pour Obsidian, Nextcloud Notes et votre système.
