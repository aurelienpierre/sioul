---
description: Les notes dans Sioul – votre propre dossier de fichiers Markdown, le même qu’un coffre Obsidian ou que vos notes Nextcloud, lié à vos tâches, à vos courriels et à vos événements, avec des images, des PDF et des mémos vocaux ; rien de chargé ni d’exécuté à l’ouverture d’une note ; comparé aux autres applications de notes.
---

# Les notes {#notes}

## En bref {#in-short}

Vos notes sont un dossier de fichiers Markdown qui reste à vous. Ce peut être votre coffre Obsidian, ou le dossier que Nextcloud Notes garde dans votre Nextcloud : Sioul le lit là où il est, n’importe rien, et vous pouvez continuer à y écrire avec n’importe quel autre programme. Une note peut être liée à une tâche, un courriel, un événement ou une personne, et une ligne avec une case vide devient une tâche en un clic. Les images, les PDF et les mémos vocaux sont à côté de vos notes et s’ouvrent sur place. Vos notes voyagent avec la synchronisation que vous utilisez déjà, ou scellées, avec le partage de Sioul.

## Protégé par défaut {#what-is-protected}

- Vos notes restent de simples fichiers dans votre propre dossier, sur votre appareil. Sioul n’a pas de serveur à lui, et n’envoie vos notes nulle part où vous ne les avez pas envoyées.
- Ouvrir une note ne charge rien depuis internet. Une image venue du web s’affiche comme un lien vers elle : personne n’apprend que vous lisez la note. Rien de ce qui est écrit dans une note ne peut s’exécuter : le HTML s’affiche comme le texte qu’il est, sauf de simples marques comme le gras et l’italique.
- Un lien dans une note ouvre une page web, une adresse électronique, un numéro de téléphone ou un fichier de cet appareil. Un programme, un script ou un installateur ne se lance jamais depuis une note : son dossier s’ouvre à la place. Un lien vers un dossier partagé du réseau n’est pas suivi.
- Quand le partage de Sioul transporte vos notes, chacune est scellée sur votre appareil avant de le quitter. Le serveur du dossier partagé voit la taille de chaque fichier scellé, jamais son nom ni ce qu’il dit.
- Quand un changement venu d’un autre appareil remplace une note ici, la note telle qu’elle était est gardée sur cet appareil, pour être remise : **Paramètres ▸ Partage ▸ Voir les versions précédentes** ([Remettre une version précédente](sharing.md#putting-back-an-older-version)).

## Le même dossier qu’Obsidian et que Nextcloud Notes {#the-same-folder-as-obsidian-and-nextcloud-notes}

Choisissez votre coffre Obsidian, ou le dossier **Notes** que Nextcloud Notes garde dans votre Nextcloud, comme dossier de notes de Sioul. Les deux programmes continuent de marcher à côté de Sioul, sur les mêmes fichiers : rien n’est importé, rien n’est converti. Une fois les listes de Sioul déplacées dans son propre dossier et ses anciens fichiers retirés, aucune ne reste parmi vos notes ([Où Sioul garde ses listes](sharing.md#where-sioul-keeps-its-lists)).

- **Ce qu’écrit Obsidian** se lit comme Obsidian le lit : les `[[liens]]` entre notes, les images montrées par `![[image.png]]`, les `#étiquettes`, les propriétés en haut d’une note (son titre, ses étiquettes et ses autres noms), les cases à cocher, et les notes qui renvoient à celle-ci. Une note renommée dans Sioul emmène avec elle les liens des autres notes. Une note mise à la corbeille va dans le dossier `.trash` du coffre, comme le fait la corbeille propre à Obsidian.
- **Ce qu’écrit Nextcloud Notes** se lit comme le Markdown qu’il est : ses catégories sont des dossiers, et ses notes, en `.txt` ou en `.md`, gardent leur nom et leur extension. Les nouvelles notes faites dans Sioul sont en `.md`, que Nextcloud Notes montre aussi, sur le web et dans ses applications pour téléphone.
- **Ce que Sioul ajoute reste du Markdown simple** : une note liée à une tâche, un courriel ou un événement contient un lien ordinaire, ou une ligne en haut, que les autres programmes montrent comme un lien ou laissent de côté.
- **Jamais à lui** : Sioul n’écrit que les notes que vous écrivez ou faites à partir de lui. Il ne lit jamais le dossier de réglages d’Obsidian, la corbeille, ni aucun dossier caché.

Une partie de ce que dessine Obsidian, Sioul le montre comme le texte que c’est : les encadrés, les surlignages, les notes de bas de page, les formules et les diagrammes. Une note intégrée dans une autre (`![[une autre note]]`) s’affiche comme un lien vers elle ; les images s’affichent sur place.

<figure markdown="span">
  [![La page Notes : un champ de recherche, Nouvelle note et les autres boutons, puis Voir et Type sur une ligne au-dessus de toutes les notes dans leurs dossiers, Projets déplié ; une note ouverte au milieu, son chemin et ses étiquettes sous son titre, Modifier et Aa à côté, son texte avec des liens et l’image d’un croquis ; à droite, Ajouter et Lier à…, ce qui lui est lié (des tâches, un courriel, d’autres notes) avec Les voir tous, et ses lignes non cochées.](../assets/screens/fr/notes.png){ loading=lazy }](../assets/screens/fr/notes.png "Ouvrir l’image en grand")
  <figcaption>Une note, en lecture, avec ce qui lui est lié sur le côté.</figcaption>
</figure>

## Votre dossier de notes {#your-notes-folder}

Vos notes sont de simples fichiers Markdown dans un dossier à vous, sur cet appareil, que d’autres logiciels, un éditeur de notes parmi eux, peuvent ouvrir et synchroniser : Sioul s’y lie et ne les possède jamais. Tant que cet appareil n’en a pas, la page Notes le demande à sa place, en une phrase, avec **Choisir un dossier…**, qui ouvre le sélecteur de dossiers de votre système (sur Android, celui de Sioul, qui demande d’abord l’accès d’Android à tous vos fichiers, avec **Autoriser l’accès aux fichiers**) ; la page se remplit une fois un dossier choisi. Un dossier réglé mais illisible (sur un disque qui n’est pas branché, déplacé ou renommé) se dit de la même façon, avec son chemin et **Choisir de nouveau…** ; rebranché, la page se remplit d’elle-même en moins d’une minute. Jamais une liste vide à sa place. Ensuite, le dossier se change dans les réglages de la page (⚙), **Le dossier des notes**, sa première ligne ; il est propre à cet appareil, chacun de vos appareils disant où est sa copie. **Paramètres ▸ Partage** dit de quel dossier partent vos notes. Le dossier ne garde que vos notes : Sioul garde ses propres listes (les projets, les budgets, les listes de vos papiers et de vos courriers) dans son propre dossier, toujours des fichiers texte simples que vous pouvez modifier à la main, et les fichiers de vos papiers et de vos courriers dans des dossiers à eux. Tant que vous ne les y avez pas déplacés, une fois, sur chaque appareil, ils sont dans le dossier de notes à côté de vos notes ; après le déplacement, les anciens fichiers y restent jusqu’à ce que vous les retiriez, une fois que chaque appareil a fait le déplacement (**Paramètres ▸ Partage ▸ Les retirer**). Tant qu’ils y sont, les PDF et les images de vos papiers et de vos courriers apparaissent dans la liste de vos notes, dans leurs dossiers `papers` et `letters` ([Où Sioul garde ses listes](sharing.md#where-sioul-keeps-its-lists)). Là où aucune synchronisation ne transporte le dossier de notes, le partage de Sioul peut transporter vos notes, si vous y activez **Notes** ([Le partage](sharing.md)).

## Trouver une note {#finding-a-note}

La recherche vient d’abord : **Chercher dans les notes** trouve une note par son titre, son dossier ou ses étiquettes dès que vous tapez, puis, après une courte pause, par les mots qu’elle contient. Chaque mot tapé compte, où qu’il soit dans la note, majuscules et accents à part. Les notes trouvées par leur titre viennent d’abord, puis celles trouvées par leur texte, les plus récemment changées en tête, chacune avec la ligne qui contient vos mots. Tant que Sioul cherche encore dans les textes, la liste le dit. Seules les notes de texte sont lues : un PDF ou un son est trouvé par son nom, et la liste le dit quand l’un d’eux aurait pu répondre.

Sous la recherche, une ligne porte deux choix, que Sioul garde sur cet appareil tels que vous les laissez :

- **Voir** : **Par dossier**, comme la page s’ouvre : les notes **changées récemment** (les dix dernières, repliées sous leur titre jusqu’à ce que vous l’ouvriez), puis toutes les notes dans leurs dossiers, repliés jusqu’à ce que vous les ouvriez. **Par modification** : une seule liste, la plus récente d’abord, sous Aujourd’hui, Cette semaine, Ce mois-ci, Plus tôt cette année, puis chaque année d’avant. **Par création** : de même, selon le jour où chaque note a été créée, celles dont le jour n’est pas connu à la fin, sous **Date inconnue**.
- **Type** : **Tous**, **Texte**, **Son**, **PDF** ou **Autres** (images et documents de bureautique). Il restreint de même les dossiers, les listes datées et ce que trouve la recherche ; un dossier qui ne contient rien de ce type n’est pas montré.

Aucun classement ne vous est demandé.

**Quand une note a été créée.** Sioul le lit dans la note elle-même, quand ses propriétés, en haut, le disent : `created: 2026-10-05`, sinon `date:`, un jour ou un moment (`2026-10-05T09:12`). Sinon, il prend le jour où, d’après votre système, le fichier de la note a été créé sur cet appareil. Un fichier qu’un programme de synchronisation a apporté (Nextcloud, Dropbox, Syncthing, le partage de Sioul) a été créé, pour chaque appareil, le jour où il y est arrivé, et une version plus récente apportée ensuite arrive à nouveau. Quand Sioul enregistre une note, il garde le jour où elle a d’abord été créée, bien qu’il en écrive chaque fois un nouveau fichier. Là où le système ne le dit pas (sur Android, entre autres), la note est rangée sous **Date inconnue** : Sioul ne prend jamais le jour de la dernière modification d’une note pour celui de sa création. Une ligne `created:` dans les propriétés d’une note garde son jour quoi qu’il arrive à son fichier.

## Lire et écrire {#reading-and-writing}

Une note s’ouvre pour être lue. Ses liens se suivent d’un clic, où qu’ils mènent : une autre note, une tâche, un courriel, un événement, un contact, une page web.

- **Modifier** passe à l’écriture, en Markdown ; **Enregistrer** (ou Ctrl+S) garde le texte. **Lire** revient à la lecture, la note enregistrée.
- Pendant la lecture, une sélection faite à la souris ou au doigt reste dans un seul paragraphe, élément de liste, tableau ou bloc de code. **Ctrl+A** sélectionne toute la note, et **Ctrl+C** copie alors tout son texte, en Markdown.
- `[[Une autre note]]` renvoie à une note par son nom, où qu’elle soit dans le dossier. Les liens Markdown, les `#étiquettes` et les propriétés en haut d’une note marchent aussi.
- Chaque note montre ce qui renvoie vers elle, et ce à quoi elle est liée : tâches, courriels, événements, personnes, projets. **Lier à…** la lie à quelque chose qui existe déjà ; **Ajouter ▾** fait une tâche, un événement ou un courriel lié à elle.
- **Aa** règle la police, sa taille et l’interligne, pour la lecture sur cet appareil.
- **Plan** : une note qui a deux titres ou plus les montre, chacun en retrait selon son niveau. Là où la fenêtre est assez large pour la colonne à côté de la note, ils sont en haut de celle-ci, au-dessus de ce à quoi la note est liée ; sur un téléphone ou dans une fenêtre étroite, ils se replient en haut de la note, sous **Plan**, fermé jusqu’à ce que vous l’ouvriez. Un titre touché amène sa place en haut de la note, lue ou en cours d’écriture (le curseur va alors à sa ligne) ; dans le pli, celui-ci se referme. Pendant la lecture, le titre de la partie en haut de la note est teinté. Pendant que vous écrivez, le plan suit votre texte après une courte pause.

Une note changée sur un autre appareil, venue par le partage, s’affiche aussitôt, comme sur les pages Santé, Papiers et Temps ; pendant que vous y tapez, ce que vous avez tapé reste tel quel, et une ligne sous son titre dit qu’une version plus récente est arrivée. Si la note a changé ailleurs pendant que vous écriviez (sur un autre appareil, par une application de synchronisation, dans un autre programme), votre version n’est jamais enregistrée par-dessus : elle est gardée à côté, sous « Plan (conflict 2026-10-05 21.50) » (ce mot anglais, tel quel), et reste ouverte ; la ligne d’état le dit. L’autre version garde le nom. Sur un petit écran, la note ouverte prend toute la page, et **Retour** garde la note, enregistrée, et la ferme.

Une ligne comme `- [ ] Demander l’attestation`, non cochée, est listée à côté de la note (sous elle sur un écran étroit), sous **Lignes non cochées**, à un clic de devenir une tâche : **En faire une tâche**. La tâche reste liée à sa note.

## Nouvelles notes {#new-notes}

**Nouvelle note**, ou **Nouveau ▾ ▸ Une note**, demande son titre. Chaque nouvelle note va dans un dossier de vos notes, réglé dans le ⚙ de la page Notes, sous le dossier des notes : **Les nouvelles notes vont dans** (`notes`, sauf si vous le changez). Les notes que vous faites depuis ailleurs y vont aussi :

- depuis un courriel : avec son objet pour titre, en commençant par qui l’a écrit et ses premières lignes citées, le courriel lié à elle ;
- depuis un événement : datée, avec la liste des personnes invitées, liée dans les deux sens, puis **Envoyer aux personnes invitées** ;
- depuis une tâche, liée à elle.

Pour faire une note ailleurs, faites un clic droit sur son dossier (sur un écran tactile, un appui long) : **Nouvelle note ici…**. Le même menu propose **Nouveau dossier dedans…**, **Renommer le dossier…** et **Retirer ce dossier vide**. **Nouveau dossier**, le bouton à côté de **Nouvelle note**, fait un dossier en haut de vos notes.

## Images, PDF et sons {#pictures-pdfs-and-sounds}

Ce sont aussi des notes, listées à côté des fichiers Markdown, et ouvertes sur place : une image ajustée à la page, les pages d’un PDF, un son avec lecture, pause et position. Elles peuvent être liées à des tâches et à des projets comme toute note. **Ouvrir avec…** ouvre le fichier dans le programme que votre système a pour lui. Sur Android, les pages d’un PDF ne s’affichent pas dans Sioul : **Ouvrir avec…** le confie au lecteur de PDF du téléphone ou de la tablette.

Les documents bureautiques (un texte, un tableur, une présentation, un livre numérique : `odt`, `ods`, `odp`, `odg`, `doc`, `docx`, `xls`, `xlsx`, `ppt`, `pptx`, `rtf`, `epub`), dont ceux qu’un site a proposés et que vous avez rangés dans Notes ([Sites](sites.md#downloads)), sont listés aussi, par leur nom, et se lient comme toute note ; **Ouvrir avec…** les ouvre dans un autre programme. Aucun autre type de fichier n’est une note.

Les pages d’un PDF prennent la largeur ; Ctrl et la molette, ou deux doigts, les agrandissent. Son texte se sélectionne et se copie. Un lien vers une autre page y va ; un lien vers le web ou vers une adresse dit d’abord où il mène, sous les pages (« Ouvre example.org : https://… »), et ne s’ouvre qu’avec **Ouvrir**, une adresse dans la fenêtre d’écriture de Sioul, comme un lien dans un courriel. Tout autre type de lien (vers un fichier de votre ordinateur, un script) n’est pas suivi, et la ligne le dit.

`![[scan.png]]` montre l’image dans une note.

**Enregistrer un mémo vocal** lance l’enregistrement ; le même bouton, qui affiche alors **Arrêter** et la durée enregistrée, le termine. Le mémo va dans un dossier `memos`, à l’intérieur du dossier où vont les nouvelles notes, et s’ouvre comme une note. Le micro n’est ouvert que pendant l’enregistrement.

## Renommer et supprimer {#renaming-and-deleting}

Un clic droit sur une note, ou un appui long sur un écran tactile :

- **Renommer…** : les notes qui renvoient vers elle, et les tâches qui lui sont liées, suivent le nouveau nom. Un dossier renommé emmène ses notes, et leurs liens suivent aussi.
- **Mettre à la corbeille** : la note va dans le `.trash` du dossier, avec **Annuler**.

## Sur plusieurs appareils {#on-several-computers}

Vos notes voyagent avec la synchronisation de leur dossier : Nextcloud, Dropbox, Syncthing. Là où aucune synchronisation ne transporte le dossier, sur Android par exemple, le partage de Sioul les transporte, scellées, fichier par fichier, une fois **Notes** activé dans le partage. Sioul ne transporte pas un dossier qu’une application de synchronisation transporte déjà : deux transporteurs déferaient les changements l’un de l’autre. Voir [Partager entre vos appareils](sharing.md).

Sur Android, Sioul garde son propre dossier de notes : activez **Notes** dans le partage, sur le téléphone ou la tablette et sur un ordinateur, et donnez à Sioul l’accès d’Android à tous vos fichiers ([Les notes et les papiers](sharing.md#notes-and-papers)).

Une note liée qui n’est pas encore arrivée apparaît estompée, avec « Pas encore sur cet appareil : sa synchronisation est peut-être en cours (Nextcloud, Dropbox). »

## Pour aller plus loin {#going-further}

- **Les liens écrits dans la note** : quand vous liez une note à quelque chose, Sioul écrit son adresse en haut de la note, entre deux lignes `---`, sous `links:` (ou `task:`, `event:`, `mail:`, `contact:`). Vous pouvez les y écrire vous-même, ou les retirer ; les adresses sont dans [Côté technique](#for-technical-readers).
- **Les liens vers un titre, ou avec d’autres mots** : `[[Plan#Budget]]`, qui nomme un titre de la note comme Obsidian l’écrit, ouvre la note à ce titre (ses mots, majuscules à part, ou tels qu’une page web les écrit, `#budget-2026` ; un titre introuvable ouvre la note à son début), et `[[Plan|le plan]]` montre d’autres mots pour le lien, comme dans Obsidian.
- **Les étiquettes avec une barre oblique** : `#démarches/courriers` se lit comme une seule étiquette, comme Obsidian écrit ses étiquettes imbriquées ; chercher « démarches » la trouve.
- **Les versions précédentes** : avant qu’un changement venu d’un autre appareil remplace ou retire une note ici, la note telle qu’elle était est gardée sur cet appareil. Le plus souvent, elle l’est comme un renvoi vers sa copie dans le dossier de partage, qui garde une note remplacée deux semaines : le renvoi part au bout de 12 jours, si peu de versions qu’il y ait. Une note dont la copie a quitté le dossier est copiée ici à la place, comme les autres fichiers : ses 20 dernières copies, et toutes celles des 30 derniers jours. Une note effacée sur un autre appareil est gardée comme une copie elle aussi, déplacée ici, ce qui ne prend aucune place : sa copie dans le dossier de partage le quitte alors dès que chacun de vos appareils a lu qu’elle était effacée. Les modifications que vous faites sur cet appareil ne créent pas de version ; votre application de synchronisation ou votre serveur peuvent garder les leurs (Nextcloud le fait).
- **Des sons pour travailler** : les sons que vous mettez dans un dossier `sounds` (ou `sons`) de vos notes sont proposés par le bouton des sons pendant que vous travaillez ([Tâches](tasks.md#sounds)).

## Comparé à d’autres applications {#compared-with-other-apps}

**Un dossier que d’autres programmes lisent aussi.** Les notes de Sioul sont là pour se tenir à côté de vos tâches, de vos courriels et de vos journées. Parmi ces applications, seuls Sioul et Obsidian ouvrent un coffre Obsidian là où il est. Sioul lie une note à vos tâches, à vos courriels et à vos événements, dans les deux sens, et fait une tâche d’une ligne avec une case ; OneNote en fait une partie avec Outlook, sous Windows, et Joplin fait une note d’un courriel transféré à son service payant. Son partage scellé emploie XChaCha20-Poly1305 et Argon2id, comme Standard Notes, à travers n’importe quelle application de synchronisation et sans compte. D’autres font plus ailleurs : Sioul ne cherche pas les mots à l’intérieur de vos notes, n’a ni dessin, ni écriture à la main, ni texte lu dans les images, ne peut pas partager une note avec d’autres personnes, et ne garde les versions précédentes que quand un changement venu d’un autre appareil remplace une note.

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

=== "Au quotidien"

    | | Sioul | Obsidian | Joplin | Nextcloud Notes | Standard Notes | OneNote |
    |---|---|---|---|---|---|---|
    | Vos notes sont de simples fichiers, dans un dossier que vous choisissez | ✓ | ✓ | ✗ | ✓ | ✗ | ✗ |
    | Ouvre un coffre Obsidian là où il est, liens et étiquettes compris | ✓ | ✓ | ✗¹ | ✗² | ✗ | ✗ |
    | Des liens entre notes, et les notes qui renvoient à celle-ci | ✓ | ✓ | en partie³ | ✗ | ✓ | en partie⁴ |
    | Une note liée à une tâche, un courriel ou un événement | ✓ | ✗⁵ | en partie⁶ | ✗ | ✗ | en partie⁷ |
    | Une ligne avec une case transformée en tâche | ✓ | ✗ | ✗ | ✗ | ✗ | ✓⁷ |
    | Images, PDF, et son enregistré dans l’application | ✓ | ✓ | ✓ | en partie⁸ | en partie⁹ | ✓ |
    | Trouve une note par les mots qu’elle contient | ✓¹⁰ | ✓ | ✓ | ? | ? | ✓ |
    | Dessin, écriture à la main, texte lu dans les images | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
    | Partager des notes avec d’autres personnes | ✗ | en partie¹¹ | en partie¹² | en partie¹³ | en partie¹⁴ | ✓ |

    1. Joplin importe un dossier de fichiers Markdown dans sa propre base de données ; il n’ouvre pas le dossier là où il est.
    2. Nextcloud Notes ouvre les fichiers `.md`, sans les `[[liens]]`, les images intégrées ni les autres noms.
    3. Des liens par le numéro d’une note ; les notes qui renvoient à celle-ci seulement par des greffons faits par d’autres.
    4. Des liens vers des pages et des sections ; ce qui renvoie à une page n’est pas décrit.
    5. Pas parmi les fonctions propres à Obsidian ; des greffons faits par d’autres peuvent l’ajouter.
    6. Un courriel transféré à Joplin Cloud devient une note, avec ses formules payantes ; une note peut être une tâche avec une alarme.
    7. Avec Outlook, dans OneNote pour Windows : les détails d’une réunion copiés dans une page, et des mots transformés en tâche Outlook, cochée dans l’un ou l’autre.
    8. Images et fichiers ; pas d’enregistrement de son.
    9. Des fichiers avec la formule Professional seulement ; pas d’enregistrement de son.
    10. Dans le texte de ses notes en Markdown, après leur titre, leur dossier et leurs étiquettes ; un PDF ou un son par son nom seulement. Un [agent d’IA](ai-agent.md) relié à Sioul cherche dans les mêmes textes.
    11. Un coffre partagé par Obsidian Sync, payé par chaque membre ; pas d’écriture à plusieurs dans une note au même moment.
    12. Un carnet partagé par les formules payantes de Joplin Cloud.
    13. En partageant le dossier Notes dans les Fichiers de Nextcloud.
    14. Un lien en lecture seule par Listed, qui ferme le 31 décembre 2026.

=== "Technique"

    | | Sioul | Obsidian | Joplin | Nextcloud Notes | Standard Notes | OneNote |
    |---|---|---|---|---|---|---|
    | Marche avec la synchronisation que vous utilisez déjà | ✓¹ | ✓ | en partie² | en partie³ | ✗⁴ | ✗⁵ |
    | Chiffré de bout en bout entre vos appareils | ✓⁶ | ✓⁷ | ✓⁸ | ✗ | ✓⁹ | en partie¹⁰ |
    | Les deux versions gardées quand une note change sur deux appareils | ✓ | ✓¹¹ | ✓ | en partie¹² | ✓ | ? |
    | Les versions précédentes gardées, pour les remettre | en partie¹³ | ✓¹⁴ | ✓¹⁵ | ✓¹⁶ | ✓¹⁷ | ? |
    | Logiciel libre | ✓ GPL-3.0+ | ✗¹⁸ | ✓ AGPL-3.0+ | ✓ AGPL-3.0+ | ✓ AGPL-3.0 | ✗ |

    1. Votre dossier de notes voyage avec votre propre application de synchronisation (Nextcloud, Dropbox, Syncthing…), ou scellé avec le partage de Sioul, à travers le dossier de n’importe quelle application de synchronisation.
    2. Nextcloud, WebDAV, Dropbox, OneDrive, S3 ou un dossier, toujours dans le format propre à Joplin, que les autres programmes ne lisent pas comme des notes.
    3. Nextcloud seulement, sur votre propre serveur ou celui d’un fournisseur.
    4. Son propre serveur, gratuit, ou un serveur que vous faites tourner vous-même.
    5. OneDrive ou SharePoint.
    6. Quand le partage de Sioul transporte vos notes : XChaCha20-Poly1305, sa clé faite à partir de votre phrase de passe par Argon2id. Un dossier que transporte votre propre application de synchronisation est protégé comme cette application le protège.
    7. Avec Obsidian Sync, payant : AES-256-GCM, sa clé faite par scrypt.
    8. Quand vous l’activez : AES-256-GCM, sa clé faite par PBKDF2-HMAC-SHA512.
    9. Toujours actif, gratuit : XChaCha20-Poly1305, sa clé faite par Argon2id.
    10. Un mot de passe sur une section, dans l’application de bureau ; le chiffrement employé n’est pas dit.
    11. Avec Obsidian Sync : fusionnées, ou gardées en copie en conflit.
    12. Son application web montre les deux versions et demande laquelle garder ; son application Android garde la dernière écrite.
    13. Sur chaque appareil, quand un changement venu d’un autre appareil remplace une note : gardée 12 jours comme un renvoi vers la copie du dossier de partage, ou, cette copie partie, copiée (les 20 dernières, et toutes celles de 30 jours) ; une note effacée sur un autre appareil, copiée. Vos propres modifications n’en créent pas.
    14. Sur chaque appareil toutes les 5 minutes, gardées 7 jours, gratuitement (récupération de fichiers) ; sur le serveur avec Obsidian Sync, payant.
    15. Toutes les 10 minutes, gardées 90 jours, sur chaque appareil, gratuitement.
    16. Sur le serveur, comme Nextcloud garde les versions de chaque fichier.
    17. Sur l’appareil ; sur le serveur avec les formules payantes.
    18. Libre d’usage pour tout but, mais son code n’est pas un logiciel libre. L’application Android de Nextcloud Notes est sous GPL-3.0-or-later.

**Apple Notes**, sur les appareils d’Apple et sur le site d’iCloud, garde les notes dans sa propre base et les synchronise par iCloud. Il relie les notes entre elles (ce qui renvoie à une note n’est pas décrit), enregistre du son avec une transcription écrite, laisse plusieurs personnes écrire dans une note à la fois, et chiffre de bout en bout quand la protection avancée des données est activée. Son guide ne décrit pas de versions précédentes ; une note supprimée reste 30 jours dans « Suppressions récentes ».

??? info "Sources (en anglais)"
    - Aide d’Obsidian : comment Obsidian garde les données, <https://help.obsidian.md/data-storage>, lu le 8 octobre 2026.
    - Aide d’Obsidian : les liens internes, <https://help.obsidian.md/links>, lu le 8 octobre 2026.
    - Aide d’Obsidian : intégrer des fichiers, <https://help.obsidian.md/embeds>, lu le 8 octobre 2026.
    - Aide d’Obsidian : les alias, <https://help.obsidian.md/aliases>, lu le 8 octobre 2026.
    - Aide d’Obsidian : les propriétés, <https://help.obsidian.md/properties>, lu le 8 octobre 2026.
    - Aide d’Obsidian : les greffons, <https://help.obsidian.md/plugins>, lu le 8 octobre 2026.
    - Aide d’Obsidian : les rétroliens, <https://help.obsidian.md/plugins/backlinks>, lu le 8 octobre 2026.
    - Aide d’Obsidian : la syntaxe de mise en forme, <https://help.obsidian.md/syntax>, lu le 8 octobre 2026.
    - Aide d’Obsidian : l’enregistreur audio, <https://help.obsidian.md/plugins/audio-recorder>, lu le 8 octobre 2026.
    - Aide d’Obsidian : la recherche, <https://help.obsidian.md/plugins/search>, lu le 8 octobre 2026.
    - Aide d’Obsidian : travailler à plusieurs sur un coffre partagé, <https://help.obsidian.md/sync/collaborate>, lu le 8 octobre 2026.
    - Aide d’Obsidian : synchroniser ses notes entre appareils, <https://help.obsidian.md/sync-notes>, lu le 8 octobre 2026.
    - Aide d’Obsidian : la sécurité et la vie privée de Sync, <https://help.obsidian.md/sync/security>, lu le 8 octobre 2026.
    - Aide d’Obsidian : les problèmes de Sync (conflits), <https://help.obsidian.md/sync/troubleshoot>, lu le 8 octobre 2026.
    - Aide d’Obsidian : la récupération de fichiers, <https://help.obsidian.md/plugins/file-recovery>, lu le 8 octobre 2026.
    - Aide d’Obsidian : l’historique des versions de Sync, <https://help.obsidian.md/sync/version-history>, lu le 8 octobre 2026.
    - Aide d’Obsidian : les réglages (« Deleted files »), <https://obsidian.md/help/settings>, lu le 8 octobre 2026.
    - Obsidian : la licence, <https://obsidian.md/license>, lu le 8 octobre 2026.
    - Obsidian : les prix, <https://obsidian.md/pricing>, lu le 8 octobre 2026.
    - Aide d’Obsidian : l’installation, <https://help.obsidian.md/install>, lu le 8 octobre 2026.
    - Joplin : questions et réponses, <https://joplinapp.org/help/faq>, lu le 8 octobre 2026.
    - Joplin : importer et exporter, <https://joplinapp.org/help/apps/import_export>, lu le 8 octobre 2026.
    - Joplin : lier une note, <https://joplinapp.org/help/apps/link_to_note>, lu le 8 octobre 2026.
    - Greffons de Joplin : Easy Backlinks, <https://joplinapp.org/plugins/plugin/com.tuibyte.EasyBacklinks/>, lu le 8 octobre 2026.
    - Joplin : des courriels aux notes, <https://joplinapp.org/help/apps/email_to_note>, lu le 8 octobre 2026.
    - Joplin : les tâches, <https://joplinapp.org/help/apps/to-dos>, lu le 8 octobre 2026.
    - Joplin : les pièces jointes, <https://joplinapp.org/help/apps/attachments>, lu le 8 octobre 2026.
    - Joplin : l’historique des versions de l’application Android, <https://joplinapp.org/help/about/changelog/android>, lu le 8 octobre 2026.
    - Joplin : la recherche, <https://joplinapp.org/help/apps/search>, lu le 8 octobre 2026.
    - Joplin : l’outil de dessin, <https://joplinapp.org/help/apps/drawing_tool>, lu le 8 octobre 2026.
    - Joplin : la reconnaissance de texte (OCR), <https://joplinapp.org/help/apps/ocr>, lu le 8 octobre 2026.
    - Joplin : partager un carnet, <https://joplinapp.org/help/apps/share_notebook>, lu le 8 octobre 2026.
    - Joplin : les formules, <https://joplinapp.org/plans/>, lu le 8 octobre 2026.
    - Joplin : la synchronisation, <https://joplinapp.org/help/apps/sync/>, lu le 8 octobre 2026.
    - Joplin : le chiffrement de bout en bout, <https://joplinapp.org/help/apps/sync/e2ee>, lu le 8 octobre 2026.
    - Joplin : la spécification du chiffrement, <https://joplinapp.org/help/dev/spec/e2ee/native_encryption>, lu le 8 octobre 2026.
    - Joplin : les conflits, <https://joplinapp.org/help/apps/conflict>, lu le 8 octobre 2026.
    - Joplin : l’historique des notes, <https://joplinapp.org/help/apps/note_history>, lu le 8 octobre 2026.
    - Joplin : la licence, <https://github.com/laurent22/joplin/blob/dev/LICENSE>, lu le 8 octobre 2026.
    - Joplin : l’installation, <https://joplinapp.org/help/install>, lu le 8 octobre 2026.
    - Nextcloud Notes : son README, <https://github.com/nextcloud/notes>, lu le 8 octobre 2026.
    - Nextcloud Notes : le code de son éditeur Markdown, <https://github.com/nextcloud/notes/blob/main/src/components/EditorMarkdownIt.vue>, lu le 8 octobre 2026.
    - Nextcloud Notes : son historique des versions, <https://github.com/nextcloud/notes/blob/main/CHANGELOG.md>, lu le 8 octobre 2026.
    - Nextcloud Notes pour Android : questions et réponses, <https://github.com/nextcloud/notes-android/blob/main/FAQ.md>, lu le 8 octobre 2026.
    - Nextcloud Notes : son API, <https://github.com/nextcloud/notes/blob/main/docs/api/v1.md>, lu le 8 octobre 2026.
    - Manuel de Nextcloud : le chiffrement de bout en bout, <https://docs.nextcloud.com/server/latest/user_manual/en/files/using_e2ee.html>, lu le 8 octobre 2026.
    - Manuel de Nextcloud : le chiffrement des fichiers sur le serveur, <https://docs.nextcloud.com/server/latest/user_manual/en/files/encrypting_files.html>, lu le 8 octobre 2026.
    - Nextcloud Notes : le code de son éditeur simple (conflits), <https://github.com/nextcloud/notes/blob/main/src/components/NotePlain.vue>, lu le 8 octobre 2026.
    - Manuel de Nextcloud : les versions des fichiers, <https://docs.nextcloud.com/server/latest/user_manual/en/files/version_control.html>, lu le 8 octobre 2026.
    - Nextcloud Notes : les informations de l’application (licence), <https://github.com/nextcloud/notes/blob/main/appinfo/info.xml>, lu le 8 octobre 2026.
    - Nextcloud Notes pour Android : son README, <https://github.com/nextcloud/notes-android/blob/main/README.md>, lu le 8 octobre 2026.
    - Standard Notes : les sauvegardes, <https://standardnotes.com/help/14/how-do-i-create-and-import-backups-of-my-standard-notes-data>, lu le 8 octobre 2026.
    - Standard Notes : l’aide, <https://standardnotes.com/help>, lu le 8 octobre 2026.
    - Standard Notes : lier une autre note, <https://standardnotes.com/help/84/can-i-create-a-link-to-another-note>, lu le 8 octobre 2026.
    - Standard Notes : les fonctions, <https://standardnotes.com/features>, lu le 8 octobre 2026.
    - Standard Notes : les fichiers chiffrés, <https://standardnotes.com/help/36/how-do-i-attach-encrypted-files-to-my-notes>, lu le 8 octobre 2026.
    - Standard Notes : chercher dans une note, <https://standardnotes.com/help/70/how-do-i-search-inside-a-note>, lu le 8 octobre 2026.
    - Standard Notes : travailler à plusieurs sur une note, <https://standardnotes.com/help/50/can-i-collaborate-with-others-on-a-note>, lu le 8 octobre 2026.
    - Standard Notes : partager une note privée, <https://standardnotes.com/help/17/how-do-i-share-a-private-note>, lu le 8 octobre 2026.
    - Standard Notes : faire tourner son propre serveur, <https://standardnotes.com/help/47/can-i-self-host-standard-notes>, lu le 8 octobre 2026.
    - Standard Notes : la spécification du chiffrement, <https://github.com/standardnotes/app/blob/main/packages/snjs/specification.md>, lu le 8 octobre 2026.
    - Standard Notes : doublons et conflits, <https://standardnotes.com/help/33/how-do-i-clear-duplicates>, lu le 8 octobre 2026.
    - Standard Notes : l’historique des versions, <https://standardnotes.com/help/26/how-do-i-enable-note-version-history>, lu le 8 octobre 2026.
    - Standard Notes : les formules, <https://standardnotes.com/plans>, lu le 8 octobre 2026.
    - Standard Notes : la licence, <https://github.com/standardnotes/app/blob/main/LICENSE>, lu le 8 octobre 2026.
    - OneNote : les tâches de base sous Windows, <https://support.microsoft.com/en-us/onenote/onenote-help-and-learning/basic-tasks-in-onenote-on-windows>, lu le 8 octobre 2026.
    - OneNote : les détails d’une réunion Outlook dans une page, <https://support.microsoft.com/en-us/onenote/onenote-help-and-learning/insert-outlook-meeting-details-into-onenote>, lu le 8 octobre 2026.
    - OneNote : des tâches Outlook faites dans OneNote, <https://support.microsoft.com/en-us/onenote/onenote-help-and-learning/create-outlook-tasks-in-onenote>, lu le 8 octobre 2026.
    - OneNote : les notes audio et vidéo, <https://support.microsoft.com/en-us/onenote/onenote-help-and-learning/record-audio-or-video-notes>, lu le 8 octobre 2026.
    - OneNote : les notes manuscrites, <https://support.microsoft.com/en-us/onenote/onenote-help-and-learning/take-handwritten-notes-in-onenote>, lu le 8 octobre 2026.
    - OneNote : copier le texte des images (OCR), <https://support.microsoft.com/en-us/OneNote/onenote-help-and-learning/copy-text-from-pictures-and-file-printouts-using-ocr-in-onenote>, lu le 8 octobre 2026.
    - OneNote : partager un bloc-notes, <https://support.microsoft.com/en-us/OneNote/onenote-help-and-learning/how-to-share-a-onenote-notebook>, lu le 8 octobre 2026.
    - OneNote : synchroniser un bloc-notes, <https://support.microsoft.com/en-us/OneNote/onenote-help-and-learning/sync-a-notebook-in-onenote>, lu le 8 octobre 2026.
    - OneNote : un mot de passe sur une section, <https://support.microsoft.com/en-us/onenote/onenote-help-and-learning/protect-your-notes-with-a-password>, lu le 8 octobre 2026.
    - Notes pour Mac : les liens, <https://support.apple.com/guide/notes/add-links-apde615d29c2/mac>, lu le 8 octobre 2026.
    - Notes pour Mac : enregistrer et transcrire du son, <https://support.apple.com/guide/notes/record-and-transcribe-audio-apdb5106e334/mac>, lu le 8 octobre 2026.
    - Apple : vue d’ensemble de la sécurité des données iCloud, <https://support.apple.com/en-us/102651>, lu le 8 octobre 2026.
    - Notes pour Mac : supprimer une note, <https://support.apple.com/guide/notes/delete-a-note-not5585d71a8/mac>, lu le 8 octobre 2026.
    - Notes pour Mac : partager et travailler à plusieurs, <https://support.apple.com/guide/notes/share-notes-and-collaborate-apd4e6e2c9a6/mac>, lu le 8 octobre 2026.
    - Notes pour Mac : importer, exporter et imprimer, <https://support.apple.com/guide/notes/import-export-and-print-notes-not201900c07/mac>, lu le 8 octobre 2026.

## Côté technique {#for-technical-readers}

- **Markdown** : CommonMark avec les tableaux, le texte barré et les listes de tâches (pulldown-cmark). Le HTML brut s’affiche tel qu’il est écrit, sauf douze marques de mise en forme simples, sans attributs (`<b>`, `<i>`, `<u>`, `<s>`, `<em>`, `<strong>`, `<del>`, `<sub>`, `<sup>`, `<mark>`, `<kbd>`, `<br>`). Une image ne s’affiche que depuis cet appareil ou le dossier des notes, refaite à partir de son adresse et de sa taille ; une image distante devient un lien. La note est dessinée par le texte enrichi de Qt, pas par un moteur web : aucun script ne peut s’exécuter.
- **Les liens** : `[[nom]]`, `[[nom#titre]]` et `[[nom|texte affiché]]` se trouvent comme Obsidian les trouve : une note dont le chemin finit par ce nom, le même dossier d’abord, puis le chemin le plus court, puis les `aliases` des notes. Les liens Markdown sont relatifs à la note.
- **Les adresses** : une note est `sioul:note/<chemin>` (une IRI, RFC 3987), un courriel `mid:<Message-ID>` (RFC 2392), une tâche, un événement ou un contact `sioul:task/<UID>`, `sioul:event/<UID>`, `sioul:contact/<UID>`. Chaque lien est écrit dans la chose qui peut le porter : les propriétés d’une note (son en-tête : `title`, `tags`, `aliases`, et les liens vers les tâches, les événements, les courriels, les contacts et les brouillons) ; les lignes de lien et de relation d’une tâche ou d’un événement (`X-SIOUL-LINK`, `RELATED-TO` ; le `LINK` de la RFC 9253 lu aussi), que CalDAV transporte vers vos autres appareils et que les autres programmes gardent ; sinon un petit fichier local, `links.toml`.
- **Les liens ouverts** : `http`, `https`, `mailto`, `tel`, et `file:///` sur cet appareil ; un lien `file:///` vers un programme ouvre son dossier. Les autres schémas (`smb:`, `file://serveur/`, les gestionnaires propres à un bureau) sont refusés, et la ligne d’état le dit.
- **Les fichiers** : les dossiers cachés, `.obsidian`, `.trash`, `.git`, `node_modules` et `target` ne sont jamais lus. Une note s’écrit à côté de sa place, puis est renommée à sa place, pour qu’une panne ne laisse jamais une moitié de note ; un chemin qui sortirait du dossier des notes est refusé. Les noms qu’un système refuse, dont les noms de périphériques de Windows (`CON`, `NUL`, `AUX`…), sont changés, pour que les notes passent d’un système à l’autre. Le texte d’une note reçoit une empreinte à son ouverture ; si le fichier a changé au moment où vous enregistrez, votre version va à côté.
- **Le partage scellé** : chaque note est compressée, puis scellée avec XChaCha20-Poly1305 en morceaux de 1 Mio, chaque morceau lié à son fichier, à sa place et au fait qu’il soit le dernier, pour que des morceaux ne puissent être ni échangés, ni coupés, ni ajoutés. Son nom dans le dossier est un HMAC-SHA-256 de l’empreinte de son contenu, sous une clé dérivée par HKDF-SHA-256 : les noms ne disent rien, pas même que deux dossiers contiennent le même fichier. La clé vient de votre phrase de passe (12 caractères au moins) par Argon2id (version 1.3, 64 Mio, 3 passes, 1 voie) et reste dans le trousseau du système, ou dans le KeyStore d’Android sur un téléphone ou une tablette. Les fichiers de plus de 64 Mo restent sur leur appareil. Une suppression atteint les autres appareils au bout de dix minutes ; beaucoup de fichiers disparus d’un coup d’un dossier toujours là sont gardés deux semaines sur vos autres appareils, puis les quittent d’eux-mêmes, sauf si vous les remettez. Le serveur du dossier voit toujours quel appareil a écrit (un identifiant tiré au hasard), quand, et la taille de chaque fichier scellé. Voir [Ce que cela protège, et ce que cela ne peut pas cacher](sharing.md#what-it-protects-and-what-it-cannot-hide).
- **Les versions précédentes** : une note ou un papier gardé comme un renvoi vers sa copie dans le dossier de partage, 12 jours, quel que soit leur nombre ; les copies (les autres fichiers, les notes dont la copie a quitté le dossier, et les notes ou papiers qu’un autre appareil a effacés, déplacés là) les 20 dernières de chaque fichier et toutes celles des 30 derniers jours, au plus 1 Gio ou un vingtième du disque, la plus petite des deux limites ; gardées sur cet appareil seulement et jamais partagées.
- **Les dossiers synchronisés reconnus** : Sioul ne transporte pas un dossier de notes qui se trouve dans un dossier Nextcloud, ownCloud, Dropbox, Syncthing, OneDrive, pCloud ou Seafile, puisque cette application le transporte déjà ; une fois les listes de Sioul déplacées, il en va de même pour le dossier des papiers et celui des courriers. Les propres listes de Sioul ne sont plus alors dans le dossier de notes : elles voyagent par le partage de Sioul seul, jamais refusées.
