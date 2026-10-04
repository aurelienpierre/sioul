---
description: Utiliser Sioul sur plusieurs ordinateurs – ce qui voyage par vos serveurs, par votre dossier de notes, et par le journal scellé de Sioul à travers un dossier Nextcloud, Dropbox ou Syncthing, chiffré de bout en bout.
---

# Partager entre vos ordinateurs {#sharing-between-your-computers}

Sioul sur un ordinateur de bureau et sur un portable : l’essentiel de ce que vous voyez voyage déjà tout seul. Ce que Sioul est seul à garder peut voyager aussi, scellé par une phrase de passe, par un dossier que votre service de synchronisation transporte déjà : Nextcloud, Dropbox, Syncthing. Aucun serveur à nous entre les deux.

## Ce qui voyage, et comment {#what-travels-and-how}

| Quoi | Comment cela rejoint vos autres ordinateurs |
|---|---|
| Courrier, contacts, événements et tâches, avec ce que Sioul écrit dans vos tâches (étapes, attentes, liens, types) | par leurs propres serveurs, comme avec n’importe quel programme |
| Votre dossier de notes : notes, projets, budgets, papiers, lettres scannées, images et mémos vocaux | par la synchronisation du dossier lui-même |
| Ce que Sioul garde sur cet ordinateur seul : vos réglages et vos comptes (sans les mots de passe), qui peut vous écrire, les liens entre les choses, le temps passé, les brouillons, les factures, les médicaments et les prises, les journées de votre montre, les listes gardées sur cet ordinateur seulement, l’endroit où le Porche a été fermé | par le journal scellé de Sioul, à travers un dossier que vous choisissez |

**Jamais partagé** : l’endroit où sont les choses sur chaque ordinateur (chacun garde ses propres dossiers), la façon dont le texte se lit sur cet écran, la disposition des pages, les notifications du navigateur de cet ordinateur, les caches, et vos propres clés OpenPGP (copiez-les à la main). Les mots de passe restent dans le trousseau de chaque ordinateur : un compte qui arrive d’un autre ordinateur demande son mot de passe une fois.

## La mise en place {#setting-it-up}

Sur le premier ordinateur :

1. Ouvrez **Paramètres ▸ Votre dossier et le partage**, et trouvez **Entre vos ordinateurs**.
2. **Dossier** : choisissez un dossier à l’intérieur de celui que votre synchronisation transporte, par exemple un nouveau dossier `Sioul` dans votre dossier Nextcloud.
3. **Phrase de passe**, puis **Encore une fois** : quelques mots que vous n’oublierez pas, au moins 12 caractères.
4. **Partager**.

Sur l’autre ordinateur, choisissez le même dossier (là où votre synchronisation le place sur cet ordinateur). Sioul voit qu’un autre ordinateur partage par ce dossier, et demande la phrase de passe choisie là-bas. Puis **Partager**.

La première fois, une copie de ce qu’avait cet ordinateur est gardée de côté, au cas où. Puis les réglages de l’autre ordinateur arrivent, et ce que seul celui-ci avait part vers l’autre : deux ordinateurs réglés chacun de leur côté finissent avec les réglages du premier et les listes des deux.

Ensuite, les changements s’échangent chaque minute, et quand vous choisissez **Tout actualiser** ou **Échanger maintenant**. Le panneau dit par quel dossier vous partagez, avec combien d’autres ordinateurs, et quand ils ont donné des nouvelles pour la dernière fois. **Arrêter le partage** y met fin ; chaque ordinateur garde ses propres fichiers.

Votre dossier de notes voyage par sa propre synchronisation, pas par Sioul. S’il ne semble pas être dans un dossier synchronisé, le panneau le dit : votre autre ordinateur ne verrait pas vos notes et vos projets. Déplacés dans un dossier synchronisé (et choisis à nouveau dans Paramètres), ils voyagent aussi.

## Scellé {#sealed}

Chaque changement est chiffré sur votre ordinateur avant d’être écrit dans le dossier (XChaCha20-Poly1305), avec une clé tirée de votre phrase de passe (Argon2id). Le dossier, et le serveur qui le transporte, voient quel ordinateur a écrit, quand, et combien ; jamais quoi : ni le nom des choses, ni leurs valeurs.

La phrase de passe se tape une fois sur chaque ordinateur, et reste dans son trousseau. Une phrase fausse est signalée tout de suite.

!!! warning "Une phrase de passe perdue ne se retrouve pas"
    Personne ne peut la retrouver pour vous : elle n’est jamais envoyée nulle part. Si elle est perdue, arrêtez le partage sur chaque ordinateur et recommencez avec un nouveau dossier. Rien n’est perdu sur vos ordinateurs.

## Quand deux ordinateurs changent la même chose {#when-two-computers-change-the-same-thing}

Le changement le plus récent l’emporte, un réglage, une ligne, une entrée à la fois :

- un réglage changé sur chaque ordinateur garde le plus récent ;
- du temps noté sur chaque ordinateur : les deux restent ;
- un expéditeur laissé entrer sur un ordinateur puis bloqué plus tard sur l’autre finit bloqué partout.

Un fichier de réglages à moitié écrit, ou abîmé à la main, n’est jamais lu comme vidé : rien n’en est retiré ailleurs.

## Certaines choses, un ordinateur à la fois {#some-things-one-computer-at-a-time}

- **Les médicaments** ne sont rappelés que par l’ordinateur devant lequel vous êtes, pour qu’une prise ne soit pas rappelée deux fois. Une prise notée part tout de suite vers les autres.
- **La notification regroupée des sites** vient sur l’ordinateur devant lequel vous êtes.
- **Les factures** sont numérotées sur un seul ordinateur, pour qu’un numéro ne soit jamais donné deux fois. Un autre ordinateur dit où elles se font, et propose **Faire les factures sur cet ordinateur**. Voir [Le temps et les factures](time.md#on-several-computers).

## Pas encore là {#not-there-yet}

Un serveur de base de données au lieu d’un dossier, pour qui en préfère un ; les brouillons dans le dossier Brouillons de votre serveur de courrier, pour que d’autres programmes de courrier les voient ; les téléphones. Le format est simple et documenté, pour qu’ils puissent venir.
