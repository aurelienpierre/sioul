---
description: Les notes dans Sioul – votre propre dossier de fichiers Markdown, le même qu’un coffre Obsidian ou que vos notes Nextcloud, lié à vos tâches, à votre courrier et à vos événements, avec des images, des PDF et des mémos vocaux ; rien de chargé ni d’exécuté à l’ouverture d’une note ; comparé aux autres applications de notes.
---

# Les notes {#notes}

## En bref {#in-short}

Vos notes sont un dossier de fichiers Markdown qui reste à vous. Ce peut être votre coffre Obsidian, ou le dossier que Nextcloud Notes garde dans votre Nextcloud : Sioul le lit là où il est, n’importe rien, et vous pouvez continuer à y écrire avec n’importe quel autre programme. Une note peut être liée à une tâche, un message, un événement ou une personne, et une ligne avec une case vide devient une tâche en un clic. Les images, les PDF et les mémos vocaux sont à côté de vos notes et s’ouvrent sur place. Vos notes voyagent avec la synchronisation que vous utilisez déjà, ou scellées, avec le partage de Sioul.

## Protégé par défaut {#what-is-protected}

- Vos notes restent de simples fichiers dans votre propre dossier, sur votre appareil. Sioul n’a pas de serveur à lui, et n’envoie vos notes nulle part où vous ne les avez pas envoyées.
- Ouvrir une note ne charge rien depuis internet. Une image venue du web s’affiche comme un lien vers elle : personne n’apprend que vous lisez la note. Rien de ce qui est écrit dans une note ne peut s’exécuter : le HTML s’affiche comme le texte qu’il est, sauf de simples marques comme le gras et l’italique.
- Un lien dans une note ouvre une page web, une adresse de courrier, un numéro de téléphone ou un fichier de cet ordinateur. Un programme, un script ou un installateur ne se lance jamais depuis une note : son dossier s’ouvre à la place. Un lien vers un dossier partagé du réseau n’est pas suivi.
- Quand le partage de Sioul transporte vos notes, chacune est scellée sur votre appareil avant de le quitter. Le serveur du dossier partagé voit la taille de chaque fichier scellé, jamais son nom ni ce qu’il dit.
- Quand un changement venu d’un autre appareil remplace une note ici, la note telle qu’elle était est gardée sur cet appareil, pour être remise : **Paramètres ▸ Votre dossier et le partage ▸ Voir les versions précédentes** ([Remettre une version précédente](sharing.md#putting-back-an-older-version)).

## Le même dossier qu’Obsidian et que Nextcloud Notes {#the-same-folder-as-obsidian-and-nextcloud-notes}

Choisissez votre coffre Obsidian, ou le dossier **Notes** que Nextcloud Notes garde dans votre Nextcloud, comme dossier de notes de Sioul. Les deux programmes continuent de marcher à côté de Sioul, sur les mêmes fichiers : rien n’est importé, rien n’est converti.

- **Ce qu’écrit Obsidian** se lit comme Obsidian le lit : les `[[liens]]` entre notes, les images montrées par `![[image.png]]`, les `#étiquettes`, les propriétés en haut d’une note (son titre, ses étiquettes et ses autres noms), les cases à cocher, et les notes qui renvoient à celle-ci. Une note renommée dans Sioul emmène avec elle les liens des autres notes. Une note mise à la corbeille va dans le dossier `.trash` du coffre, comme le fait la corbeille propre à Obsidian.
- **Ce qu’écrit Nextcloud Notes** se lit comme le Markdown qu’il est : ses catégories sont des dossiers, et ses notes, en `.txt` ou en `.md`, gardent leur nom et leur extension. Les nouvelles notes faites dans Sioul sont en `.md`, que Nextcloud Notes montre aussi, sur le web et dans ses applications pour téléphone.
- **Ce que Sioul ajoute reste du Markdown simple** : une note liée à une tâche, un message ou un événement contient un lien ordinaire, ou une ligne en haut, que les autres programmes montrent comme un lien ou laissent de côté.
- **Jamais à lui** : Sioul n’écrit que les notes que vous écrivez ou faites à partir de lui. Il ne lit jamais le dossier de réglages d’Obsidian, la corbeille, ni aucun dossier caché.

Une partie de ce que dessine Obsidian, Sioul le montre comme le texte que c’est : les encadrés, les surlignages, les notes de bas de page, les formules et les diagrammes. Une note intégrée dans une autre (`![[une autre note]]`) s’affiche comme un lien vers elle ; les images s’affichent sur place.

<figure markdown="span">
  [![La page Notes : Nouvelle note et les autres boutons au-dessus de la liste de toutes les notes, chacune avec son dossier ; une note ouverte au milieu, son chemin et ses étiquettes sous son titre, son texte avec des liens et l’image d’un croquis ; à droite, ce qui lui est lié (des tâches, un message, d’autres notes) et ses lignes non cochées, chacune avec un bouton qui en fait une tâche.](../assets/screens/fr/notes.png){ loading=lazy }](../assets/screens/fr/notes.png "Ouvrir l’image en grand")
  <figcaption>Une note, en lecture, avec ce qui lui est lié sur le côté.</figcaption>
</figure>

## Votre dossier de notes {#your-notes-folder}

Il se choisit dans **Paramètres ▸ Votre dossier et le partage ▸ Le dossier des notes**. À côté de vos notes, le même dossier garde vos projets, vos budgets, vos papiers et vos lettres scannées, pour qu’ils voyagent tous ensemble quand le dossier est synchronisé avec vos autres appareils. Là où aucune synchronisation ne le transporte, le partage de Sioul peut les transporter, si vous les y activez ([Le partage](sharing.md)).

## Trouver une note {#finding-a-note}

La recherche vient d’abord : **Chercher dans les notes** trouve une note par son titre, son dossier ou ses étiquettes ; elle ne cherche pas les mots à l’intérieur des notes. Dessous viennent les notes **changées récemment**, puis **toutes les notes**, en une **Liste** ou en **Dossiers**, repliés jusqu’à ce que vous les ouvriez. Sioul se souvient de votre choix. Aucun classement ne vous est demandé.

## Lire et écrire {#reading-and-writing}

Une note s’ouvre pour être lue. Ses liens se suivent d’un clic, où qu’ils mènent : une autre note, une tâche, un message, un événement, un contact, une page web.

- **Modifier** passe à l’écriture, en Markdown ; **Enregistrer** (ou Ctrl+S) garde le texte.
- `[[Une autre note]]` renvoie à une note par son nom, où qu’elle soit dans le dossier. Les liens Markdown, les `#étiquettes` et les propriétés en haut d’une note marchent aussi.
- Chaque note montre ce qui renvoie vers elle, et ce à quoi elle est liée : tâches, messages, événements, personnes, projets.
- **Aa** règle la police, sa taille et l’interligne, pour la lecture sur cet appareil.
- **Ouvrir avec…** ouvre la note dans un autre programme.

Si la note a changé ailleurs pendant que vous écriviez (sur un autre appareil, par une application de synchronisation, dans un autre programme), votre version n’est jamais enregistrée par-dessus : elle est gardée à côté, sous «  Plan (conflict 2026-10-05 21.50)  » (ce mot anglais, tel quel), et reste ouverte ; la ligne d’état le dit. L’autre version garde le nom. Sur un téléphone, la note ouverte prend toute la page, et **Retour** garde la note, enregistrée, et la ferme.

Une ligne comme `- [ ] Demander l’attestation`, non cochée, est à un clic de devenir une tâche : **En faire une tâche**. La tâche reste liée à sa note.

## Nouvelles notes {#new-notes}

**Nouvelle note**, ou **Nouveau ▾ ▸ Une note**, demande son titre. Chaque nouvelle note va dans un dossier de vos notes, réglé dans le ⚙ de la page Notes : **Les nouvelles notes vont dans** (`notes`, sauf si vous le changez). Les notes que vous faites depuis ailleurs y vont aussi :

- depuis un message : avec son objet pour titre, en commençant par qui l’a écrit et ses premières lignes citées, le message lié à elle ;
- depuis un événement : datée, avec la liste des personnes invitées, liée dans les deux sens, puis **Envoyer aux personnes invitées** ;
- depuis une tâche, liée à elle.

Pour faire une note ailleurs, faites un clic droit sur son dossier : **Nouvelle note ici…**. Le même menu propose **Nouveau dossier dedans…**, **Renommer le dossier…** et **Retirer ce dossier vide**.

## Images, PDF et sons {#pictures-pdfs-and-sounds}

Ce sont aussi des notes, listées à côté des fichiers Markdown, et ouvertes sur place : une image ajustée à la page, les pages d’un PDF, un son avec lecture, pause et position. Elles peuvent être liées à des tâches et à des projets comme toute note.

`![[scan.png]]` montre l’image dans une note.

**Enregistrer un mémo vocal** enregistre dans un dossier `memos`, à l’intérieur du dossier où vont les nouvelles notes, et ouvre le mémo comme une note. Le micro n’est ouvert que pendant l’enregistrement.

## Renommer et supprimer {#renaming-and-deleting}

Un clic droit sur une note :

- **Renommer…** : les notes qui renvoient vers elle, et les tâches qui lui sont liées, suivent le nouveau nom. Un dossier renommé emmène ses notes, et leurs liens suivent aussi.
- **Mettre à la corbeille** : la note va dans le `.trash` du dossier, avec **Annuler**.

## Sur plusieurs appareils {#on-several-computers}

Vos notes voyagent avec la synchronisation de leur dossier : Nextcloud, Dropbox, Syncthing. Là où aucune synchronisation ne transporte le dossier, celui d’un téléphone par exemple, le partage de Sioul les transporte, scellées, fichier par fichier, une fois **Notes** activé dans le partage. Sioul ne transporte pas un dossier qu’une application de synchronisation transporte déjà : deux transporteurs déferaient les changements l’un de l’autre. Voir [Partager entre vos appareils](sharing.md).

Sur un téléphone, Sioul garde son propre dossier de notes : activez **Notes** dans le partage, sur le téléphone et sur un ordinateur, et donnez à Sioul l’accès d’Android à tous vos fichiers ([Les notes et les papiers](sharing.md#notes-and-papers)).

Une note liée qui n’est pas encore arrivée apparaît estompée, avec «  Pas encore sur cet appareil : sa synchronisation est peut-être en cours  ».

## Pour aller plus loin {#going-further}

- **Les liens écrits dans la note** : quand vous liez une note à quelque chose, Sioul écrit son adresse en haut de la note, entre deux lignes `---`, sous `links:` (ou `task:`, `event:`, `mail:`, `contact:`). Vous pouvez les y écrire vous-même, ou les retirer ; les adresses sont dans [Côté technique](#for-technical-readers).
- **Les liens vers un titre, ou avec d’autres mots** : `[[Plan#Budget]]` mène à un titre de la note, et `[[Plan|le plan]]` montre d’autres mots pour le lien, comme dans Obsidian.
- **Les étiquettes avec une barre oblique** : `#démarches/courriers` se lit comme une seule étiquette, comme Obsidian écrit ses étiquettes imbriquées ; chercher «  démarches  » la trouve.
- **Les versions précédentes** : avant qu’un changement venu d’un autre appareil remplace ou retire une note ici, la note telle qu’elle était est gardée sur cet appareil : ses 20 dernières versions, et toutes celles des 30 derniers jours. Les modifications que vous faites sur cet appareil ne créent pas de version ; votre application de synchronisation ou votre serveur peuvent garder les leurs (Nextcloud le fait).
- **Des sons pour travailler** : les sons que vous mettez dans un dossier `sounds` de vos notes sont proposés par le bouton des sons pendant que vous travaillez ([Tâches](tasks.md#sounds)).

## Comparé à d’autres applications {#compared-with-other-apps}

**Un dossier que d’autres programmes lisent aussi.** Les notes de Sioul sont là pour se tenir à côté de vos tâches, de votre courrier et de vos journées. Parmi ces applications, seuls Sioul et Obsidian ouvrent un coffre Obsidian là où il est. Sioul lie une note à vos tâches, à votre courrier et à vos événements, dans les deux sens, et fait une tâche d’une ligne avec une case ; OneNote en fait une partie avec Outlook, sous Windows, et Joplin fait une note d’un courrier transféré à son service payant. Son partage scellé emploie XChaCha20-Poly1305 et Argon2id, comme Standard Notes, à travers n’importe quelle application de synchronisation et sans compte. D’autres font plus ailleurs : Sioul ne cherche pas les mots à l’intérieur de vos notes, n’a ni dessin, ni écriture à la main, ni texte lu dans les images, ne peut pas partager une note avec d’autres personnes, et ne garde les versions précédentes que quand un changement venu d’un autre appareil remplace une note.

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

=== "Au quotidien"

    | | Sioul | Obsidian | Joplin | Nextcloud Notes | Standard Notes | OneNote |
    |---|---|---|---|---|---|---|
    | Vos notes sont de simples fichiers, dans un dossier que vous choisissez | ✓ | ✓ | ✗ | ✓ | ✗ | ✗ |
    | Ouvre un coffre Obsidian là où il est, liens et étiquettes compris | ✓ | ✓ | ✗¹ | ✗² | ✗ | ✗ |
    | Des liens entre notes, et les notes qui renvoient à celle-ci | ✓ | ✓ | en partie³ | ✗ | ✓ | en partie⁴ |
    | Une note liée à une tâche, un message ou un événement | ✓ | ✗⁵ | en partie⁶ | ✗ | ✗ | en partie⁷ |
    | Une ligne avec une case transformée en tâche | ✓ | ✗ | ✗ | ✗ | ✗ | ✓⁷ |
    | Images, PDF, et son enregistré dans l’application | ✓ | ✓ | ✓ | en partie⁸ | en partie⁹ | ✓ |
    | Trouve une note par les mots qu’elle contient | ✗¹⁰ | ✓ | ✓ | ? | ? | ✓ |
    | Dessin, écriture à la main, texte lu dans les images | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
    | Partager des notes avec d’autres personnes | ✗ | en partie¹¹ | en partie¹² | en partie¹³ | en partie¹⁴ | ✓ |

    1. Joplin importe un dossier de fichiers Markdown dans sa propre base de données ; il n’ouvre pas le dossier là où il est.
    2. Nextcloud Notes ouvre les fichiers `.md`, sans les `[[liens]]`, les images intégrées ni les autres noms.
    3. Des liens par le numéro d’une note ; les notes qui renvoient à celle-ci seulement par des greffons faits par d’autres.
    4. Des liens vers des pages et des sections ; ce qui renvoie à une page n’est pas décrit.
    5. Pas parmi les fonctions propres à Obsidian ; des greffons faits par d’autres peuvent l’ajouter.
    6. Un courrier transféré à Joplin Cloud devient une note, avec ses formules payantes ; une note peut être une tâche avec une alarme.
    7. Avec Outlook, dans OneNote pour Windows : les détails d’une réunion copiés dans une page, et des mots transformés en tâche Outlook, cochée dans l’un ou l’autre.
    8. Images et fichiers ; pas d’enregistrement de son.
    9. Des fichiers avec la formule Professional seulement ; pas d’enregistrement de son.
    10. Sioul trouve une note par son titre, son dossier et ses étiquettes.
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
    13. Sur chaque appareil, quand un changement venu d’un autre appareil remplace une note : ses 20 dernières versions, et toutes celles de 30 jours. Vos propres modifications n’en créent pas.
    14. Sur chaque appareil toutes les 5 minutes, gardées 7 jours, gratuitement (récupération de fichiers) ; sur le serveur avec Obsidian Sync, payant.
    15. Toutes les 10 minutes, gardées 90 jours, sur chaque appareil, gratuitement.
    16. Sur le serveur, comme Nextcloud garde les versions de chaque fichier.
    17. Sur l’appareil ; sur le serveur avec les formules payantes.
    18. Libre d’usage pour tout but, mais son code n’est pas un logiciel libre. L’application Android de Nextcloud Notes est sous GPL-3.0-or-later.

**Apple Notes**, sur les appareils d’Apple et sur le site d’iCloud, garde les notes dans sa propre base et les synchronise par iCloud. Il relie les notes entre elles (ce qui renvoie à une note n’est pas décrit), enregistre du son avec une transcription écrite, laisse plusieurs personnes écrire dans une note à la fois, et chiffre de bout en bout quand la protection avancée des données est activée. Son guide ne décrit pas de versions précédentes ; une note supprimée reste 30 jours dans «  Suppressions récentes  ».

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
    - Aide d’Obsidian : les réglages («  Deleted files  »), <https://obsidian.md/help/settings>, lu le 8 octobre 2026.
    - Obsidian : la licence, <https://obsidian.md/license>, lu le 8 octobre 2026.
    - Obsidian : les prix, <https://obsidian.md/pricing>, lu le 8 octobre 2026.
    - Aide d’Obsidian : l’installation, <https://help.obsidian.md/install>, lu le 8 octobre 2026.
    - Joplin : questions et réponses, <https://joplinapp.org/help/faq>, lu le 8 octobre 2026.
    - Joplin : importer et exporter, <https://joplinapp.org/help/apps/import_export>, lu le 8 octobre 2026.
    - Joplin : lier une note, <https://joplinapp.org/help/apps/link_to_note>, lu le 8 octobre 2026.
    - Greffons de Joplin : Easy Backlinks, <https://joplinapp.org/plugins/plugin/com.tuibyte.EasyBacklinks/>, lu le 8 octobre 2026.
    - Joplin : du courrier aux notes, <https://joplinapp.org/help/apps/email_to_note>, lu le 8 octobre 2026.
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

- **Markdown** : CommonMark avec les tableaux, le texte barré et les listes de tâches (pulldown-cmark). Le HTML brut s’affiche tel qu’il est écrit, sauf douze marques de mise en forme simples, sans attributs (`<b>`, `<i>`, `<u>`, `<s>`, `<em>`, `<strong>`, `<del>`, `<sub>`, `<sup>`, `<mark>`, `<kbd>`, `<br>`). Une image ne s’affiche que depuis cet ordinateur ou le dossier des notes, refaite à partir de son adresse et de sa taille ; une image distante devient un lien. La note est dessinée par le texte enrichi de Qt, pas par un moteur web : aucun script ne peut s’exécuter.
- **Les liens** : `[[nom]]`, `[[nom#titre]]` et `[[nom|texte affiché]]` se trouvent comme Obsidian les trouve : une note dont le chemin finit par ce nom, le même dossier d’abord, puis le chemin le plus court, puis les `aliases` des notes. Les liens Markdown sont relatifs à la note.
- **Les adresses** : une note est `sioul:note/<chemin>` (une IRI, RFC 3987), un message `mid:<Message-ID>` (RFC 2392), une tâche, un événement ou un contact `sioul:task/<UID>`, `sioul:event/<UID>`, `sioul:contact/<UID>`. Chaque lien est écrit dans la chose qui peut le porter : les propriétés d’une note (son en-tête : `title`, `tags`, `aliases`, et les liens vers les tâches, les événements, le courrier, les contacts et les brouillons) ; les lignes `LINK` ou `RELATED-TO` d’une tâche ou d’un événement, que CalDAV transporte vers vos autres appareils et que les autres programmes gardent ; sinon un petit fichier local, `links.toml`.
- **Les liens ouverts** : `http`, `https`, `mailto`, `tel`, et `file:///` sur cet ordinateur ; un lien `file:///` vers un programme ouvre son dossier. Les autres schémas (`smb:`, `file://serveur/`, les gestionnaires propres à un bureau) sont refusés, et la ligne d’état le dit.
- **Les fichiers** : les dossiers cachés, `.obsidian`, `.trash`, `.git`, `node_modules` et `target` ne sont jamais lus. Une note s’écrit à côté de sa place, puis est renommée à sa place, pour qu’une panne ne laisse jamais une moitié de note ; un chemin qui sortirait du dossier des notes est refusé. Les noms qu’un système refuse, dont les noms de périphériques de Windows (`CON`, `NUL`, `AUX`…), sont changés, pour que les notes passent d’un système à l’autre. Le texte d’une note reçoit une empreinte à son ouverture ; si le fichier a changé au moment où vous enregistrez, votre version va à côté.
- **Le partage scellé** : chaque note est compressée, puis scellée avec XChaCha20-Poly1305 en morceaux de 1 Mio, chaque morceau lié à son fichier, à sa place et au fait qu’il soit le dernier, pour que des morceaux ne puissent être ni échangés, ni coupés, ni ajoutés. Son nom dans le dossier est un HMAC-SHA-256 de l’empreinte de son contenu, sous une clé dérivée par HKDF-SHA-256 : les noms ne disent rien, pas même que deux dossiers contiennent le même fichier. La clé vient de votre phrase de passe (12 caractères au moins) par Argon2id (version 1.3, 64 Mio, 3 passes, 1 voie) et reste dans le trousseau du système, ou dans le KeyStore d’Android sur un téléphone. Les fichiers de plus de 64 Mo restent sur leur appareil. Une suppression atteint les autres appareils au bout de dix minutes ; beaucoup de fichiers disparus d’un coup sont retenus jusqu’à ce que vous confirmiez. Le serveur du dossier voit toujours quel appareil a écrit (un identifiant tiré au hasard), quand, et la taille de chaque fichier scellé. Voir [Ce que cela protège, et ce que cela ne peut pas cacher](sharing.md#what-it-protects-and-what-it-cannot-hide).
- **Les versions précédentes** : les 20 dernières versions de chaque fichier et toutes celles des 30 derniers jours, au plus 1 Gio ou un vingtième du disque, la plus petite des deux limites, gardées sur cet appareil seulement et jamais partagées.
- **Les dossiers synchronisés reconnus** : Sioul ne transporte pas un dossier de notes qui se trouve dans un dossier Nextcloud, ownCloud, Dropbox, Syncthing, OneDrive, pCloud ou Seafile, puisque cette application le transporte déjà.
