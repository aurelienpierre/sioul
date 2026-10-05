---
description: Les notes dans Sioul – votre propre dossier de fichiers Markdown, le même qu’un coffre Obsidian ou que vos notes Nextcloud, avec des liens dans les deux sens, des images, des PDF, des sons et des mémos vocaux.
---

# Les notes {#notes}

Vos notes sont votre propre dossier de fichiers Markdown. Sioul le lit comme Obsidian lit un coffre, et s’y lie ; il ne le possède jamais. Vous pouvez continuer à y écrire avec n’importe quel autre programme.

## Le même dossier qu’Obsidian et que Nextcloud Notes {#the-same-folder-as-obsidian-and-nextcloud-notes}

Les notes de Sioul sont entièrement compatibles avec un coffre Obsidian et avec Nextcloud Notes. Choisissez votre coffre, ou le dossier **Notes** que Nextcloud Notes garde dans votre Nextcloud, comme dossier de notes de Sioul, et continuez à vous servir des deux côte à côte : les mêmes fichiers, rien d’importé, rien de converti.

- **Comme Obsidian les lit** : les `[[liens wiki]]`, avec `#titre` et `|texte affiché`, trouvés par leur nom comme Obsidian les trouve (le même dossier d’abord, puis le chemin le plus court, puis les alias) ; les `![[images]]` montrées en place ; les liens Markdown ; les `#étiquettes` ; l’en-tête (titre, étiquettes, alias) ; les cases `- [ ]` ; et les notes qui renvoient à celle-ci. Une note mise à la corbeille va dans le `.trash` du coffre, comme dans Obsidian. Renommée dans Sioul, une note emmène avec elle les liens des autres notes.
- **Comme Nextcloud Notes les écrit** : ses catégories sont des dossiers, et ses notes, enregistrées en `.txt` par défaut ou en `.md`, sont lues et modifiées comme le Markdown qu’elles sont, chacune gardant son nom et son extension. Les nouvelles notes faites dans Sioul sont en `.md`, que Nextcloud Notes montre aussi, sur le web et dans ses applications pour téléphone.
- **Ce que Sioul ajoute reste du Markdown simple** : une note liée à une tâche, un courriel ou un événement contient un lien ordinaire (`sioul:task/…`, `mid:…`), ou une ligne d’en-tête, que les autres programmes montrent comme un lien ou laissent simplement de côté.
- **Jamais à lui** : Sioul n’écrit que les notes que vous écrivez ou faites à partir de lui, et ne lit jamais le dossier `.obsidian` d’Obsidian, le `.trash`, `.git` ni aucun dossier caché.

<figure markdown="span">
  [![La page Notes : Nouvelle note et les autres boutons au-dessus de la liste de toutes les notes, chacune avec son dossier ; une note ouverte au milieu, son chemin et ses étiquettes sous son titre, son texte avec des liens et l’image d’un croquis ; à droite, ce qui lui est lié (des tâches, un message, d’autres notes) et ses lignes non cochées, chacune avec un bouton qui en fait une tâche.](../assets/screens/fr/notes.png){ loading=lazy }](../assets/screens/fr/notes.png "Ouvrir l’image en grand")
  <figcaption>Une note, en lecture, avec ce qui lui est lié sur le côté.</figcaption>
</figure>

## Votre dossier de notes {#your-notes-folder}

Il se choisit dans **Paramètres ▸ Votre dossier et le partage ▸ Le dossier des notes**. À côté de vos notes, le même dossier garde vos projets, vos budgets, vos papiers et vos lettres scannées, pour qu’ils voyagent tous ensemble quand le dossier est synchronisé avec vos autres ordinateurs. Là où aucune synchronisation ne le transporte, le partage de Sioul peut les transporter, si vous les y activez ([Le partage](sharing.md)).

## Trouver une note {#finding-a-note}

La recherche vient d’abord : **Chercher dans les notes**. Dessous, les notes **changées récemment**, puis **toutes les notes**, en une **Liste** ou en **Dossiers**, repliés jusqu’à ce que vous les ouvriez. Sioul se souvient de votre choix. Aucun classement ne vous est demandé.

## Lire et écrire {#reading-and-writing}

Une note s’ouvre pour être lue. Ses liens se suivent d’un clic, où qu’ils mènent : une autre note, une tâche, un message, un événement.

- **Modifier** passe à l’écriture ; **Enregistrer** garde le texte.
- `[[Une autre note]]` renvoie à une note par son nom, où qu’elle soit dans le dossier. Les liens Markdown, les `#étiquettes` et l’en-tête YAML (front matter) marchent aussi.
- Chaque note montre ce qui renvoie vers elle, et ce à quoi elle est liée : tâches, messages, événements, personnes, projets.
- **Aa** règle la police, sa taille et l’interligne, pour la lecture.
- **Ouvrir avec…** ouvre la note dans un autre programme.

Si la note a changé ailleurs pendant que vous écriviez (sur un autre appareil, par une application de synchronisation, dans un autre programme), votre version n’est jamais enregistrée par-dessus : elle est gardée à côté, sous « Plan (conflict 2026-10-05 21.50) » (ce mot anglais, tel quel), et reste ouverte ; la ligne d’état le dit. L’autre version garde le nom. Sur un téléphone, **Retour** garde la note, enregistrée, et la ferme.

Une ligne comme `- [ ] Demander l’attestation`, non cochée, est à un clic de devenir une tâche : **En faire une tâche**. La tâche reste liée à sa note.

## Nouvelles notes {#new-notes}

**Nouvelle note**, ou **Nouveau ▾ ▸ Une note**, demande son titre. Une note peut aussi se faire depuis un message (elle commence par qui l’a écrit), depuis un événement (datée, avec la liste des personnes invitées, puis **Envoyer aux personnes invitées**), ou depuis une tâche.

Les notes faites depuis le courrier et les événements vont dans un dossier de vos notes, réglé dans le ⚙ de la page Notes : **Les nouvelles notes vont dans**.

Un clic droit sur un dossier : **Nouvelle note ici…**, **Nouveau dossier dedans…**, **Renommer le dossier…**, **Retirer ce dossier vide**.

## Images, PDF et sons {#pictures-pdfs-and-sounds}

Ce sont aussi des notes, listées à côté des fichiers Markdown, et ouvertes sur place : une image ajustée à la page, les pages d’un PDF, un son avec lecture, pause et position. Elles peuvent être liées à des tâches et à des projets comme toute note.

`![[scan.png]]` montre l’image dans une note.

**Enregistrer un mémo vocal** enregistre dans le dossier `memos` de vos notes, et l’ouvre comme une note. Le micro n’est ouvert que pendant l’enregistrement.

## Renommer et supprimer {#renaming-and-deleting}

Un clic droit sur une note :

- **Renommer…** : les notes qui renvoient vers elle, et les tâches qui lui sont liées, suivent le nouveau nom.
- **Mettre à la corbeille** : la note va dans le `.trash` du dossier, comme le fait Obsidian, avec **Annuler**.

## Sur plusieurs ordinateurs {#on-several-computers}

Vos notes voyagent avec la synchronisation de leur dossier : Nextcloud, Dropbox, Syncthing. Là où aucune synchronisation ne transporte le dossier, celui d’un téléphone par exemple, le partage de Sioul peut les transporter, fichier par fichier, une fois **Notes** activé dans le partage. Voir [Partager entre vos appareils](sharing.md).

Une note liée qui n’est pas encore arrivée apparaît estompée, avec « Pas encore sur cet ordinateur : sa synchronisation est peut-être en cours ».
