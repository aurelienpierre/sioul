---
description: "Le client de courrier de Sioul : chaque adresse et chaque dossier, chaque message vérifié à son arrivée et affiché sans risque, les pièces jointes passées à votre antivirus, la recherche, les filtres, votre propre filtre à indésirables, OpenPGP et dix secondes pour annuler, sans compteurs ni pastilles."
---

# Le courrier {#mail}

## En bref {#in-short}

La page Courrier garde tout votre courrier, de chaque adresse, pour quand vous choisissez de regarder ; ce qui est nouveau attend d’abord sur [le Porche](porch.md). Sioul vérifie lui-même chaque message à son arrivée, et vous dit avant que vous le lisiez s’il vient vraiment de l’adresse qu’il affiche. Rien dans un message ne se charge ni ne s’exécute, et une pièce jointe passe par l’antivirus de votre système, quand il en a un, avant de s’ouvrir. Vous écrivez en texte simple, en Markdown si vous le voulez, et tout ce que vous envoyez, déplacez ou supprimez peut s’annuler pendant dix secondes. Sioul ne montre ni compteur de non-lus, ni pastilles, ni rouge.

<figure markdown="span">
  [![La page Courrier : une adresse dépliée avec ses dossiers (Boîte de réception, Envoyés, Brouillons, Archives, Indésirables, Corbeille) et une autre repliée en dessous ; au milieu, la boîte de réception : « Sept messages que vous n’avez pas lus. », puis une ligne par message avec qui, quoi et quand, un petit point devant ceux qui ne sont pas lus ; un champ de recherche et la case Temps réel en haut.](../assets/screens/fr/mail.png){ loading=lazy }](../assets/screens/fr/mail.png "Ouvrir l’image en grand")
  <figcaption>Les dossiers à gauche, les deux dernières semaines au milieu, aucun compteur.</figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

Sans rien régler :

- **Chaque message est vérifié à son arrivée.** Vous voyez s’il est *vérifié*, *non vérifié* ou *falsifié* avant de le lire. Le courrier falsifié, et celui qui emprunte le nom d’une banque ou d’un service public, attendent à part sur le Porche, avec la raison ([Lire](#reading)).
- **Rien dans un message ne se charge ni ne s’exécute** : personne n’apprend que vous l’avez ouvert, et chaque lien montre où il mène avant de s’ouvrir ([Lire](#reading)).
- **Les pièces jointes passent par l’antivirus de votre ordinateur** avant de s’ouvrir, et un programme joint n’est jamais lancé. Sans antivirus, Sioul demande d’abord ; sur un téléphone, qui n’en a pas que Sioul puisse appeler, il dit que le fichier n’est pas vérifié ([Les pièces jointes](#attachments)).
- **Votre courrier voyage chiffré.** Sioul n’a aucun réglage pour parler en clair à un serveur de courrier.
- **Rien ne part sans vous**, et vous avez dix secondes pour changer d’avis. Aucun accusé de lecture n’est jamais envoyé. Il n’y a pas de serveur Sioul : votre courrier va de votre appareil à votre fournisseur, et nulle part ailleurs, sauf si vous choisissez de l’envoyer quelque part ([Vie privée et sécurité](privacy-security.md)).

Une fois que vous les avez mis en place :

- **Votre propre filtre à indésirables**, entraîné sur votre ordinateur à partir de votre courrier ; votre courrier ne va à aucun service de filtrage ([Votre filtre à indésirables](#your-own-spam-filter)).
- **Le courrier signé et chiffré** avec OpenPGP, votre clé dans le trousseau de votre système ou sur une clé de sécurité ([Signer et chiffrer](#signing-and-encrypting)).

## Les dossiers {#folders}

- **À gauche**, chaque adresse avec ses dossiers principaux : Boîte de réception, Envoyés, Brouillons, Archives, Indésirables, Corbeille. Les autres sont repliés sous **Autres dossiers**.
- **Un petit point** marque un dossier qui a du nouveau. Combien, c’est dit en toutes lettres quand vous l’ouvrez.
- **Un dossier montre ses deux dernières semaines.** **Messages plus anciens** ouvre le reste, pour qu’aucune liste ne soit sans fin.
- **Chercher**, en haut d’un dossier : par expéditeur, destinataire et objet. **Plus de critères…**, à côté, cherche partout, par conditions ([plus bas](#searching)).
- **Par conversation**, dans le ⚙ de la page Courrier : un message et ses réponses ensemble, sous le plus récent ; vos propres réponses viennent des Envoyés.

Un clic droit sur un dossier propose **En garder une copie ici** ou **Ne plus en garder de copie** (le serveur garde tous les messages dans les deux cas), **Nouveau dossier…**, et **Supprimer ce dossier vide**, pour un dossier vide à vous.

## Lire {#reading}

Un message s’ouvre à droite ; les dossiers se replient pendant que vous lisez.

<figure markdown="span">
  [![Un message ouvert à côté de la liste : au-dessus, Répondre, Transférer, Ajouter, Lier à…, Archiver et Supprimer ; puis son expéditeur, son adresse marquée « vérifié », à qui et quand, l’objet ; puis son texte, la signature estompée, et en bas « Affiché sans risque : rien de distant ne se charge, rien ne s’exécute. »](../assets/screens/fr/mail-reader.png){ loading=lazy }](../assets/screens/fr/mail-reader.png "Ouvrir l’image en grand")
  <figcaption>Un message : qui l’a envoyé et jusqu’où c’est vérifié, puis son texte, rendu sûr.</figcaption>
</figure>

**D’abord, qui l’a envoyé, et jusqu’où c’est prouvé.** Sioul vérifie lui-même chaque message à son arrivée, en demandant au domaine de l’expéditeur si le message est bien le sien :

- **vérifié** : le domaine s’en porte garant ;
- **non vérifié** : rien ne prouve ni l’un ni l’autre, et la raison est dite ;
- **falsifié** : le domaine dit qu’il ne l’a pas envoyé, et demande qu’un tel courrier soit tenu à part. Le courrier falsifié est mis de côté sur le Porche, avec la raison, et jamais supprimé.

Un message qui emprunte le nom d’une banque, d’un service public ou de votre propre domaine, même écrit avec des lettres qui se ressemblent, est mis de côté lui aussi. Survoler le bouclier à côté de l’expéditeur montre chaque vérification ([comment un expéditeur est vérifié](#how-a-sender-is-checked)).

**Puis son texte, affiché sans risque.** Le courrier HTML garde ses paragraphes, ses listes, son texte en gras et ses liens, et rien d’autre : pas d’images, pas de styles, pas de scripts. Rien ne se charge depuis le réseau et rien ne s’exécute : aucune image de pistage n’apprend que vous avez ouvert le message. Les messages précédents qu’une réponse cite sont repliés sous **Afficher les messages précédents** ; une signature est estompée.

**Chaque lien dit où il mène avant de s’ouvrir** : son site en gras et son adresse complète, sous le message, tant que le pointeur est dessus. Sur un écran tactile, le premier toucher montre l’adresse et le second l’ouvre. Seuls les liens web et de courrier s’ouvrent ; un lien de courrier ouvre un nouveau message dans Sioul, vers cette seule adresse.

Au-dessus du message, toujours au même endroit :

- **Répondre**, **Répondre à tous** (seulement quand il y a d’autres personnes à qui répondre), **Transférer** ;
- **Ajouter ▾** (une tâche, un événement, une note, une réponse, l’expéditeur dans vos contacts) et **Lier à…** (tout le reste : une tâche, une note, un projet) ;
- **Archiver**, **Supprimer**, **Indésirable** ;
- **Se désabonner**, sur une lettre d’information ou le message d’une liste qui dit comment la quitter ([plus bas](#unsubscribing)).

Archiver, supprimer et mettre aux indésirables se font tout de suite, avec **Annuler** dans la ligne d’état pendant dix secondes ; le serveur n’est prévenu qu’après. Il n’y a pas de « Voulez-vous vraiment ? ». Dans la corbeille, **Supprimer définitivement** supprime pour de bon, avec les mêmes dix secondes.

Ce que vous marquez **Indésirable**, ou **Pas indésirable** dans le dossier des indésirables, apprend à votre propre filtre à son prochain apprentissage ([plus bas](#your-own-spam-filter)).

Un clic droit sur un message (ou son ⋮, ou la touche Menu) donne le reste : marquer comme lu ou non lu, suivre, **Déplacer vers…**, **Voir la source**, bloquer l’expéditeur, **Comment cette personne vous joint…** (sa liste, Passe toujours, ce qui vous joint de sa part : [La fiche d’une personne](notifications.md#a-persons-sheet)), **Garder comme contrat…**.

Ouvrir un message le marque comme lu, comme le fait tout logiciel de courrier. Sioul n’envoie jamais d’accusé de lecture.

### Plusieurs à la fois {#several-at-once}

- ++ctrl++ + clic ajoute un message à la sélection ou l’en retire ; ++"Maj"++ + clic prend tout depuis le dernier cliqué ; ++ctrl+a++ prend tout ; ++"Échap"++, rien. Sur un écran tactile, un appui long ouvre le menu d’un message : **Sélectionner**, puis un toucher en choisit d’autres, ou en retire un.
- Une barre les marque alors comme lus, les archive, les supprime ou les déplace, sous un seul **Annuler**, quel que soit leur nombre.
- **Glissez-les** sur n’importe quel dossier de n’importe quelle adresse, avec une souris ou un pavé tactile : le dossier sous le pointeur est entouré. Depuis les résultats d’une recherche aussi, et c’est ainsi qu’on trie une boîte chargée : les dossiers reviennent à la place de la recherche le temps du glisser. Vers une autre adresse, un message n’est retiré de la première qu’une fois que le second serveur l’a reçu.
- **Déplacer vers…**, dans la barre ou dans le menu d’un message (clic droit, ⋮, ou la touche Menu), fait de même au clavier, et sur un téléphone.

<figure markdown="span">
  [![Les résultats d’une recherche, tous choisis, tenus au-dessus des dossiers : les dossiers sont revenus à gauche à la place de la recherche, Archives entouré sous l’étiquette « 7 messages » ; au-dessus de la liste, « 7 sélectionnés » avec Marquer comme lu, Archiver, Supprimer, Déplacer vers… et ×.](../assets/screens/fr/mail-search-drag.png){ loading=lazy }](../assets/screens/fr/mail-search-drag.png "Ouvrir l’image en grand")
  <figcaption>Trier depuis une recherche : les messages choisis, glissés sur un dossier.</figcaption>
</figure>

### Se désabonner {#unsubscribing}

Une lettre d’information ou le message d’une liste qui dit comment la quitter a **Se désabonner** au-dessus. Un clic, et dix secondes plus tard Sioul fait ce que la liste demande :

- il **prévient le serveur de la liste**, quand elle le propose et que sa signature prouve que la demande vient bien d’elle : le « en un clic » que proposent la plupart des lettres d’information ;
- sinon il **envoie à la liste le message qu’elle demande**, depuis l’adresse où la lettre est arrivée ; le message va dans Envoyés comme les autres ;
- sinon, quand seule une page web peut le faire, il **ouvre cette page** dans votre navigateur, tout de suite : vous terminez là-bas.

**Annuler** reste dans la ligne d’état pendant ces dix secondes. Ensuite la ligne dit « Désabonnement de Lettres & Pixels fait. », ou pourquoi pas. Le message lui-même reste où il est.

Le bouton reste en retrait, estompé, sur un message falsifié, mis de côté comme indésirable ou dit probablement indésirable par votre propre filtre, dans le dossier des indésirables, qui emprunte un nom, hostile, d’un expéditeur que vous avez bloqué, ou quand rien ne prouve que l’adresse qu’il donne appartient à la liste : y répondre dirait à son expéditeur que votre adresse est lue, ou toucherait quelqu’un d’autre. Son info-bulle, ou un toucher sur un téléphone, dit pourquoi.

Une liste quittée montre **Abonnement arrêté**, et depuis quand. Si ses messages continuent d’arriver, bloquez l’expéditeur : ⋮ ▸ **Comment cette personne vous joint…** ▸ **Bloqué**. Les listes quittées sont dans le ⚙ de la page Courrier, sous **Listes quittées**, sur cet appareil.

### Les invitations {#invitations}

Un message qui apporte une invitation la montre au-dessus du texte : **Accepter**, **Peut-être**, **Décliner**. L’événement va dans votre agenda et votre réponse part vers la personne qui organise. Un événement publié a **Ajouter à mon agenda** ; un événement annulé, **Le retirer de mon agenda**.

### Les pièces jointes {#attachments}

Les pièces jointes sont repliées sous le message, une ligne chacune, avec leur type, leur nom et leur taille.

- **L’antivirus de votre système vérifie chacune avant qu’elle s’ouvre ou soit enregistrée** : Microsoft Defender sous Windows, ClamAV sous Linux et macOS quand il est installé. S’il trouve une menace, rien ne s’ouvre et la copie est effacée.
- **Un programme, un script, un raccourci ou un installateur n’est jamais lancé depuis un courriel**, vérifié ou non : enregistrez-le si vous lui faites confiance, et lancez-le vous-même.
- **Sans antivirus**, Sioul ne refuse pas : il dit que le fichier ne sera pas vérifié, demande avant de l’ouvrir (**L’ouvrir sans vérification**), et dit comment en avoir un.
- **Sur un téléphone**, qui n’a pas d’antivirus que Sioul puisse appeler, les pièces jointes ne sont pas vérifiées, et Sioul le dit dans le message et après chaque ouverture, plutôt que de demander chaque fois. Une pièce jointe s’ouvre dans l’application que vous choisissez (Android demande laquelle quand aucune n’est réglée pour ce type de fichier), qui peut lire ce seul fichier et rien d’autre de Sioul ; les installateurs d’Android (`.apk`) ne s’ouvrent jamais, comme les programmes. **Enregistrer…** demande à Android où mettre le fichier.
- **Sous Windows et macOS, les fichiers que vous ouvrez ou enregistrez portent la marque du système qui dit qu’ils viennent d’Internet**, pour que le système les traite avec son soin habituel : Office les ouvre en mode protégé, macOS les vérifie avant qu’ils se lancent.
- **Les pièces jointes du courrier mis de côté ne s’ouvrent pas.**
- **Garder dans les papiers** en range une dans vos [papiers](papers.md) : vérifiée, puis gardée, et vous dites ce que c’est.

## Écrire {#writing}

**Écrire** est toujours au même endroit. Vous écrivez en Markdown, et les retours à la ligne sont gardés comme vous les tapez, comme dans une messagerie instantanée : une ligne vide commence un paragraphe. **Aperçu** montre le message tel qu’il apparaîtra.

- **Cc, Cci** sont repliés jusqu’à ce que vous en ayez besoin. Quand vous avez plusieurs adresses, **De** choisit celle qui envoie ; une réponse part de l’adresse à laquelle elle répond.
- **Joindre**, ou déposer des fichiers sur la fenêtre. **Un papier**, à côté, joint l’un de vos [papiers](papers.md) ; un papier qui a expiré, ou plus vieux que ce qui est demandé d’habitude, le dit dans la liste.
- **Votre signature** est placée sous le texte quand le brouillon commence, après la ligne habituelle « -- », pour que vous voyiez ce qui part. Votre nom et votre signature se règlent par adresse dans Comptes (**Nom et signature…**).
- **En répondant**, la fenêtre dit « Sous votre texte : le message de Camille du …, cité ». La citation est ajoutée quand le message part.
- **Les brouillons s’enregistrent pendant que vous tapez.** Fermer la fenêtre ne perd rien : le brouillon attend dans Brouillons, à gauche. Les brouillons restent sur vos appareils : ils rejoignent vos autres appareils scellés quand vous [partagez entre eux](sharing.md), et ne vont jamais dans le dossier Brouillons de votre serveur.

**Envoyer** (ou ++ctrl+"Entrée"++) attend dix secondes, avec **Annuler**, avant que le message parte. Puis une copie va dans Envoyés. Rien n’est jamais envoyé sans vous.

Le message part en HTML pour qui lit le HTML, et en texte brut, le Markdown lui-même, pour les autres.

### Depuis d’autres applications {#from-other-apps}

- **Sur un téléphone**, Sioul est dans le menu de partage d’Android : partagez une photo, un PDF, un lien ou un texte vers **Sioul**, et un nouveau message s’ouvre avec, depuis votre adresse habituelle. Vos adresses y sont aussi, chacune à part (Android 10 et plus récent) : choisissez-en une, et le message part d’elle. Un appui long sur l’icône de Sioul les montre aussi, **Écrire depuis …**.
- **Les fichiers** sont copiés dans Sioul à leur arrivée. La fenêtre dit « Ajout des 2 fichiers… » jusqu’à ce qu’ils soient là, et **Envoyer** les attend. Les copies s’en vont une fois le message envoyé ou supprimé.
- **Les liens de courrier** (`mailto:`), dans un navigateur ou une autre application, ouvrent un nouveau message dans Sioul, avec son adresse, son objet et son texte, quand Sioul est votre application de courrier. Sur un téléphone, Android demande la première fois quelle application les ouvre. Sous Linux, choisissez Sioul comme programme de courrier parmi les applications par défaut des réglages de votre bureau (KDE, GNOME), ou avec `xdg-mime default com.aurelienpierre.Sioul.desktop x-scheme-handler/mailto`. Un lien cliqué pendant que Sioul est ouvert s’ouvre dedans. Pas encore sous Windows ni macOS.
- **Rien n’est envoyé** avant que vous appuyiez sur **Envoyer**. Les autres applications ne voient rien de vos comptes, sinon les adresses du menu de partage.

### Signer et chiffrer {#signing-and-encrypting}

Quand vous avez une clé OpenPGP (créée ou importée dans [Comptes ▸ Chiffrement](accounts.md#encryption)), la fenêtre d’écriture a deux petits interrupteurs, **Signer** et **Chiffrer**.

- **Signer** permet aux personnes à qui vous écrivez de vérifier que le message est de vous et qu’il n’a pas été changé en chemin.
- **Chiffrer** rend le message lisible seulement par elles et par vous. Cela fonctionne quand Sioul connaît la clé de chaque destinataire ; sinon il dit pour qui la clé manque, et **Chercher leurs clés** les demande. Sioul ne cherche que quand vous appuyez, puisque chercher dit à un serveur à qui vous écrivez.
- **L’objet n’est pas chiffré** : quiconque transporte le message peut le lire. Gardez-le neutre.
- Vos messages portent votre clé publique, pour que les personnes qui vous répondent puissent chiffrer.

Un message chiffré est déchiffré quand vous l’ouvrez ; une signature est vérifiée et dite sous l’expéditeur : « Chiffré · Signé par … », ou « La signature ne correspond pas au texte », en couleurs chaudes, jamais en rouge.

Sur un ordinateur, votre clé peut vivre sur une clé de sécurité, comme une YubiKey, qui ne la donne jamais ([plus bas](#your-key-on-a-security-key)).

## Pendant le calme {#in-quiet-time}

En dehors des heures de travail, les adresses du travail se replient, sans leurs points : « Le courrier du travail se repose jusqu’au retour du travail. Il est là si vous le cherchez. » Pendant le sommeil, toutes les adresses se replient : « Pendant le sommeil, le courrier se repose : rien ne notifie, et le Porche ne montre que ce que vos listes laissent passer maintenant. Le reste est là si vous le cherchez. » Voir [Heures](hours.md).

## Pas encore là {#not-there-yet}

Vider la corbeille ou les indésirables d’un coup, copier un message dans un dossier, l’enregistrer comme fichier, garder les brouillons sur le serveur, envoyer plus tard, un objet chiffré, révoquer ou prolonger une clé OpenPGP, les clés de sécurité sur un téléphone, les noms de domaine qui en imitent d’autres. C’est prévu. Sioul n’a pas S/MIME, ni de connexion Microsoft pour le courrier : les adresses Outlook.com, Hotmail et Microsoft 365 ne peuvent donc pas être ajoutées.

## Pour aller plus loin {#going-further}

### Chercher {#searching}

Le champ en haut d’un dossier trouve un expéditeur, un destinataire ou un objet dans ce dossier. Quand il en faut plus, **Plus de critères…**, à côté, ouvre la recherche par conditions à la place des dossiers. Rien de tout cela ne se montre avant.

<figure markdown="span">
  [![La page Courrier pendant une recherche : à gauche, à la place des dossiers, « Recherche », ce qu’elle parcourt, « Le courrier qui les remplit toutes », puis deux conditions, De contient « no-reply » et Jour d’arrivée après une date, Ajouter une condition, Effacer et En faire un filtre… ; au milieu, « Résultats », la phrase « Le courrier de …no-reply…, arrivé après le … », combien ont été trouvés, puis les messages, chacun avec l’endroit où il est, comme « Boîte de réception · noa@example.com ».](../assets/screens/fr/mail-search.png){ loading=lazy }](../assets/screens/fr/mail-search.png "Ouvrir l’image en grand")
  <figcaption>La recherche à la place des dossiers ; les résultats dans la liste habituelle, chacun disant où il est.</figcaption>
</figure>

- **Une condition d’abord**, Partout, avec ce que vous aviez tapé dans la recherche du dossier. **Ajouter une condition** pour une autre : l’expéditeur, les destinataires, l’objet, le texte, une pièce jointe (il y en a une, aucune, une avec un nom), le type de pièce jointe (un PDF, une image…), le jour d’arrivée, la taille, qui est l’expéditeur pour vous, une lettre d’information ou une liste, l’adresse, le dossier, et s’il est lu, suivi ou répondu. Majuscules et accents n’y changent rien.
- Avec deux conditions ou plus, choisissez **Le courrier qui les remplit toutes** ou **qui en remplit au moins une**.
- **Les résultats** prennent la place de la liste, les plus récents d’abord, chacun disant où il est. Un message s’ouvre à côté comme dans un dossier : vous allez et venez entre les résultats et les messages. Au-dessus, une phrase dit ce qui est cherché : « Le courrier de …@banque.example…, arrivé après le 3 juin, avec une pièce jointe. »
- **Partout** : chaque dossier de chaque adresse, sauf les indésirables et la corbeille, à moins de les nommer dans une condition Dossier.
- **Sur les serveurs aussi** : Sioul garde ici votre courrier récent, et fait venir le plus ancien selon la place sur le disque ; les dossiers gardés sur le serveur seulement ne sont pas ici du tout. Un instant après que vous avez fini de taper (tout de suite avec ++"Entrée"++), Sioul demande le reste à vos serveurs, et le dit : « Recherche sur les serveurs… », puis « 3 de plus sur les serveurs. », ou pourquoi un serveur n’a pas pu être interrogé. Ces messages disent « sur le serveur » ; en ouvrir un le fait d’abord venir ici. Interroger un serveur ne marque rien comme lu. Un serveur tient plus aux accents que Sioul : écrivez-les comme les messages les écrivent.
- **Effacer** revient au dossier.
- **En faire un filtre…**, à côté, fait de la recherche un filtre de courrier : son éditeur s’ouvre dans le ⚙ de la page Courrier, pour choisir ce qu’il fait de ce courrier à son arrivée.

Les conditions sont celles des [filtres du courrier](#filters), avec les mêmes mots.

### Filtres {#filters}

Les filtres agissent sur le courrier qui arrive, sur son serveur : dans un dossier, archivé, en indésirable, suivi, marqué comme lu, à la corbeille, ou avec un mot-clé. Ils sont dans le ⚙ de la page Courrier, sous **Filtres** : une seule liste pour toutes vos adresses, chaque filtre en une phrase.

<figure markdown="span">
  [![La page Courrier, ses réglages ouverts à droite, sous Filtres : un court paragraphe sur ce que font les filtres, « Montrer les filtres de : Toutes les adresses », puis quatre filtres, chacun un interrupteur et une phrase : Banque, « De contient « @banquedesberges.example.org » → dans « Archive », marqué comme lu, et aucun autre filtre » ; « Venu d’une lettre d’information ou d’une liste et arrivé un samedi ou dimanche → marqué comme lu » ; « De contient « @bonnes-affaires.example.com » → dans les indésirables », seulement pour noa.ferrand@example.org ; Factures, inactif et estompé ; puis Ajouter un filtre, et Les appliquer aux boîtes de réception….](../assets/screens/fr/mail-filters.png){ loading=lazy }](../assets/screens/fr/mail-filters.png "Ouvrir l’image en grand")
  <figcaption>Chaque filtre en une phrase, avec son interrupteur ; inactif, il est gardé et ne fait rien.</figcaption>
</figure>

- **Ajouter un filtre** l’ouvre sur place, avec une condition et une action. Choisissez ce que lit la condition (De, À, Cc, Répondre à, Objet, Texte, Partout, une pièce jointe, son type, qui est l’expéditeur pour vous, une lettre d’information ou une liste, le jour ou l’heure d’arrivée, la taille), comment elle compare (contient, ne contient pas, est, n’est pas ; avant, après, entre ; plus gros, plus petit), et ce qu’elle cherche. **Ajouter une condition** pour une autre ; à partir de deux, choisissez si **toutes les conditions sont vraies** ou si **l’une d’elles est vraie**. La casse et les accents ne comptent pas, comme dans la recherche.
- **Alors** : le déplacer dans un dossier, l’archiver, le marquer comme indésirable, le suivre, le marquer comme lu, le mettre à la corbeille, ou lui ajouter un mot-clé. **Ajouter une action** pour une autre. Les filtres ne transfèrent pas le courrier et n’y répondent pas.
- **Une phrase** dit le filtre à mesure que vous le composez, et ce qui manque s’il manque quelque chose : « Choisissez le dossier où il déplace les messages. » Chaque changement est gardé aussitôt.
- Repliés sous une ligne : **Sur quelles adresses** il agit (toutes, ou certaines), et **Une fois qu’il agit, les filtres suivants ne sont pas demandés**.
- **L’essayer sur les boîtes de réception** compte ce qu’il prend dans vos boîtes de réception maintenant, lu ou non, et en nomme quelques-uns.

<figure markdown="span">
  [![Le filtre Banque ouvert sur place sous sa phrase : son nom ; Si : De, contient, « @banquedesberges.example.org » ; Ajouter une condition ; Alors : Le déplacer dans un dossier, Archive, et Le marquer comme lu, chacun avec × ; Ajouter une action ; dépliés, Sur quelles adresses : Toutes les adresses, et Une fois qu’il agit, les filtres suivants ne sont pas demandés, cochés tous les deux ; L’essayer sur les boîtes de réception, Terminé, Supprimer ce filtre ; puis « Dans vos boîtes de réception, il prend un des 28 messages. » et le message qu’il prend.](../assets/screens/fr/mail-filter-editor.png){ loading=lazy }](../assets/screens/fr/mail-filter-editor.png "Ouvrir l’image en grand")
  <figcaption>Un filtre ouvert : sa condition, ses actions, et ce qu’il prend dans vos boîtes de réception maintenant.</figcaption>
</figure>

- **Chaque filtre** de la liste a son interrupteur (inactif, il est gardé et ne fait rien), sa phrase, ▴ et ▾ pour le demander plus tôt ou plus tard, et le crayon pour le modifier. **L’ordre** compte : le premier filtre qui déplace un message décide où il va. Avec plusieurs adresses, **Montrer les filtres de** montre ceux d’une adresse.
- **Quand ils agissent** : sur le nouveau courrier qui arrive non lu dans une boîte de réception, une fois, sur celui de vos appareils qui le relève le premier (cet ordinateur, votre téléphone en arrière-plan, `sioul watch`) ; vos autres appareils n’y touchent plus ensuite. Vos filtres vont sur vos autres appareils avec vos réglages.
- **Ce qu’ils ne touchent jamais** : le courrier que le Porche met de côté, un code que vous avez demandé, ce que votre filtre à indésirables a retenu. Rien n’est supprimé pour de bon : la corbeille le garde.
- **Pas annoncé** : un message qu’un filtre sort de la boîte de réception, ou marque comme lu, n’est pas notifié, et n’attend pas sur le Porche. Quand un filtre ne peut pas agir sur un message (un dossier manquant, le serveur qui refuse), le message est notifié comme tout nouveau courrier, et la ligne d’état dit pourquoi.
- **Les appliquer aux boîtes de réception…** les applique à tout ce qui est dans vos boîtes de réception maintenant, le courrier lu aussi. Sioul dit d’abord ce qui changerait ; **Les appliquer maintenant** le fait au bout de dix secondes, avec **Annuler**.

<figure markdown="span">
  [![Sous les filtres, après Les appliquer aux boîtes de réception… : « Dans vos boîtes de réception, un message changerait, lu ou non. Les codes que vous avez demandés, le courrier mis de côté et ce que votre filtre à indésirables garde à revoir restent comme ils sont. », puis chaque filtre qui agirait avec son nombre, « Un message : Banque (De contient …) », et deux boutons, Les appliquer maintenant et Pas maintenant.](../assets/screens/fr/mail-filters-run.png){ loading=lazy }](../assets/screens/fr/mail-filters-run.png "Ouvrir l’image en grand")
  <figcaption>Les appliquer aux boîtes de réception : ce qui changerait est dit d’abord.</figcaption>
</figure>

- **Depuis une recherche** : **En faire un filtre…**, à côté du bouton Effacer de la recherche, fait un filtre de ses conditions et l’ouvre à la fin de la liste, pour que vous choisissiez ce qu’il fait.

### Votre clé sur une clé de sécurité {#your-key-on-a-security-key}

Sur un ordinateur, votre clé OpenPGP peut rester sur une clé de sécurité (une YubiKey, une Nitrokey), qui signe et déchiffre elle-même et ne donne jamais la clé. Elle se met en place dans [Comptes ▸ Clé de sécurité](accounts.md#security-key).

Un message que votre clé de sécurité signe est signé quand vous appuyez sur **Envoyer**, avant les dix secondes d’**Annuler** : un bandeau en bas de la fenêtre d’écriture demande le code PIN de la clé, et dit combien d’essais il reste quand certains ont été perdus ; puis, quand la clé demande un toucher, il dit de la toucher maintenant et de garder le doigt dessus une seconde ou deux, car la clé attend environ quinze secondes et un toucher bref n’est souvent pas pris. Non branchée, il le dit, et reprend dès qu’elle arrive. **Envoyer sans signature** et **Pas maintenant** restent là tout du long, et **Annuler** jette le message signé et rouvre le brouillon. Le code PIN reste en mémoire quinze minutes après son dernier usage, jamais écrit ni journalisé nulle part ; il est oublié quand vous retirez la clé, fermez Sioul, ou choisissez **Oublier le code PIN maintenant** dans Comptes.

Un message chiffré pour votre clé de sécurité n’est jamais ouvert parce qu’il est affiché : la ligne sous l’expéditeur dit « Chiffré pour votre clé de sécurité », avec **Ouvrir avec votre clé de sécurité**. Une fois ouvert, il s’ouvre de nouveau sans la clé, ses pièces jointes aussi, jusqu’à la fermeture de Sioul.

Pas encore sur un téléphone.

### Les réglages {#settings}

Le ⚙ en haut de la page Courrier :

- **Par conversation** : les messages groupés avec leurs réponses.
- **Relever toutes les** : la fréquence à laquelle les dossiers autres que la boîte de réception sont relevés. La boîte de réception arrive dès que le serveur signale du nouveau.
- **Police**, **Taille**, **Interligne** : la lecture des messages. Les trois mêmes sont dans le ⚙ du Porche, et derrière **Aa** dans les Notes.
- **Listes quittées** : chaque liste quittée depuis un message, quand et comment, dès qu’il y en a une.
- **Filtres** : ce qui est fait au courrier qui arrive, sur son serveur, selon des conditions ([plus haut](#filters)).
- **Votre filtre à indésirables** : plus bas.

Les réglages propres à chaque adresse (à quoi elle sert, jusqu’où elle remonte, à quel rythme elle est relevée, sa protection) sont sur sa fiche dans [Comptes](accounts.md#your-accounts).

**Temps réel**, en haut de la page, relève chaque dossier de chaque adresse toutes les minutes, pour un code ou un mot de passe que vous attendez.

#### Votre filtre à indésirables {#your-own-spam-filter}

Sous **Votre filtre à indésirables**, dans le ⚙ de la page Courrier. Il ne juge que le courrier des inconnus, jamais celui de quelqu’un que vous connaissez, un code que vous avez demandé, le courrier d’un projet ni le vôtre. Ce qu’il fait sur le Porche : [Les indésirables, et votre propre filtre](porch.md#spam-and-your-own-filter).

<figure markdown="span">
  [![Les réglages de la page Courrier, « Votre filtre à indésirables » : Ce qu’il fait de chaque avis, trois rangées de boutons ronds, Probablement indésirable, Peut-être indésirable et Probablement pas indésirable, chacune avec Déplacer dans les indésirables, Signaler seulement et Ne rien faire ; Indésirable à partir de, un curseur à 95 % ; Peut-être indésirable à partir de, à 50 % ; Son apprentissage : « En service : appris par noa-bureau le mardi 6 octobre, sur 4 210 messages légitimes et 655 indésirables », ce qu’il a mesuré alors, Entraîner maintenant et ce qu’il fait.](../assets/screens/fr/settings-spam.png){ loading=lazy }](../assets/screens/fr/settings-spam.png "Ouvrir l’image en grand")
  <figcaption>Ce qu’il fait de chaque avis, ses deux seuils, et sur un ordinateur son apprentissage.</figcaption>
</figure>

- **Ce qu’il fait de chaque avis** : pour ce qu’il trouve **probablement indésirable**, **peut-être indésirable** et **probablement pas indésirable**, l’un des trois :
    - **Déplacer dans les indésirables** : à son arrivée, le message va dans le dossier Indésirables de son adresse, **sur le serveur**, où vos autres logiciels de courrier le voient aussi ; il attend sur le Porche, dans **Retenu par votre filtre à indésirables**, où **Pas indésirable** le ramène dans la boîte de réception. Seulement le courrier qui arrive : ce qui est déjà dans la boîte de réception y reste, signalé ;
    - **Signaler seulement** : il reste où il est, signalé, et y attend aussi ;
    - **Ne rien faire** : il va dans sa file, comme tout message.

    Tant que vous n’avez pas choisi : le probablement indésirable et le douteux signalés, rien pour le reste. Rien de ce qu’il signale ou déplace ne donne jamais de notification, sur aucun appareil.
- **Indésirable à partir de** : à quel point il doit être sûr pour dire probablement indésirable le message d’un inconnu, 95 % sauf si vous le changez. Plus haut : moins de vos messages pris pour indésirables, plus d’indésirables laissés passer.
- **Peut-être indésirable à partir de** : de là jusqu’à l’autre, le message d’un inconnu est peut-être indésirable ; en dessous, probablement pas ; 50 % sauf si vous le changez. Chaque curseur s’arrête avant l’autre : « peut-être » reste sous « indésirable ».
- **Son apprentissage**, sur un seul ordinateur : la première fois à la main, avec **Entraîner maintenant**, puis de lui-même une fois par semaine (plus bas). Entraînez-le sur celui-là seulement (deux ordinateurs qui entraînent chacun le leur, ce n’est pas prévu). **Entraîner maintenant** télécharge ce dont l’apprentissage a besoin, de chaque dossier de chaque adresse sauf la corbeille, les brouillons et les envoyés (les en-têtes, le nom des pièces jointes et le début de chaque texte, jamais les pièces jointes elles-mêmes), sans rien changer sur le serveur, puis apprend de votre courrier : vos dossiers des indésirables lui apprennent le plus ; ce qu’il a retenu lui-même compte tel quel (ce qu’il a déplacé ou trouvé probablement indésirable, comme indésirable ; un peut-être indésirable, seulement une fois que vous l’avez dit) ; et ce que vous avez dit indésirable ou pas, sur chacun de vos appareils, l’emporte toujours. La première fois est longue. Pendant ce temps, il dit où il en est (« Messages téléchargés : 1 200 sur environ 15 000 », puis chaque étape), et Sioul reste utilisable ; **Arrêter** garde ce qui est venu, et la fois suivante reprend de là. Sa nouvelle table ne remplace celle en service que si elle ne prend pas plus de vos messages pour indésirables. Dessous, le dernier apprentissage : quand, si sa table a remplacé celle en service et pourquoi, sur combien de messages, et sur votre courrier le plus récent, quelle part en a été prise pour indésirable et quelle part des indésirables a été repérée, chacune avec sa marge ; puis ce dont il apprend, la place qui reste sur le disque, et l’apport extérieur que vous avez importé, s’il y en a, avec ce que le filtre y a mesuré.
- **Se réentraîner de lui-même une fois par semaine, quand cet ordinateur est branché et inactif** : actif tant que vous ne le désactivez pas, sur l’ordinateur qui a fait la table en service ; sur vos autres ordinateurs l’interrupteur est grisé, et une phrase dit quel ordinateur s’en charge. Une fois par semaine au plus après le dernier apprentissage, à la main ou de lui-même ; jamais sur batterie ni en économie d’énergie, seulement après 15 minutes sans personne devant l’ordinateur ou avec sa session verrouillée, jamais pendant une session de concentration. Il tourne à la priorité la plus basse, sur la moitié du processeur ; sur une connexion limitée il apprend de ce qu’il a, sans télécharger d’abord. Débranché ou en économie d’énergie pendant qu’il tourne, il s’arrête et réessaie plus tard. Il ne vous notifie jamais : une ligne sous l’interrupteur dit quand il s’est réentraîné de lui-même la dernière fois, et si sa table a remplacé celle en service.
- **L’apport extérieur** : du courrier étiqueté ailleurs (les archives d’un ancien filtre), importé une fois sur cet ordinateur avec `sioul spam import <fichier>`, donne au filtre plus de mots à apprendre et une référence pour le mesurer ; il reste sur cet ordinateur, à part de votre courrier, et `sioul spam import --remove <nom>` le retire. Son format : [les notes des développeurs, « Outside material » (en anglais)](https://aurelienpierre.github.io/sioul/dev/spam-filter.html#outside-material).
- **Sa table**, sur un téléphone, qui ne l’entraîne jamais : l’ordinateur qui l’a entraîné, et quand, avec ce qu’il a mesuré alors. Elle arrive par votre dossier, scellée (la partie **Filtre à indésirables**, dans [Le partage](sharing.md#what-travels-from-this-device)), avec ce que vous avez dit indésirable ou pas sur chaque appareil, et ce que son filtre y a retenu.

Ce qu’il fait de chaque avis, les seuils et s’il se réentraîne de lui-même voyagent vers vos autres appareils avec vos réglages. Ce que l’apprentissage lit, et ce qu’il garde : [Vie privée et sécurité](privacy-security.md#your-own-spam-filter).

## Comparé à d’autres applications {#compared-with-other-apps}

En octobre 2026, d’après la documentation de chaque application.

✓ documenté · en partie, avec une note · ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) · ? pas confirmé · — sans objet.

Thunderbird est le logiciel libre de courrier pour ordinateur, avec une application Android ; Gmail, Outlook et Apple Mail sont les plus utilisés ; Proton Mail est la référence du courrier chiffré ; FairEmail, une application Android libre qui montre les vérifications de chaque message, rejoint le tableau technique.

=== "Au quotidien"

    | | Sioul | Thunderbird | Gmail | Outlook | Apple Mail | Proton Mail |
    |---|---|---|---|---|---|---|
    | Dit sur chaque message s’il vient vraiment de son expéditeur | ✓ | ✗¹ | en partie² | en partie³ | ✗⁴ | en partie⁵ |
    | Met de côté un expéditeur qui emprunte le nom d’une banque ou d’une marque | ✓⁶ | ✗⁷ | en partie⁸ | en partie⁹ | ✗ | en partie¹⁰ |
    | Images et autres contenus distants pas chargés tant que vous ne le demandez pas | ✓¹¹ | ✓ | en partie¹² | en partie¹³ | en partie¹⁴ | en partie¹⁵ |
    | Pièces jointes analysées contre les virus | ✓¹⁶ | en partie¹⁷ | ✓¹⁸ | ✓¹⁸ | ✗¹⁹ | en partie²⁰ |
    | Un filtre à indésirables qui apprend de votre courrier, sur votre appareil | ✓²¹ | ✓ | ✗¹⁸ | ✗¹⁸ | en partie²² | ✗¹⁸ |
    | Filtres de courrier | ✓²³ | ✓ | ✓²⁴ | en partie²⁵ | en partie²⁶ | ✓²⁷ |
    | Cherche dans chaque dossier, et sur le serveur le courrier absent de l’appareil | ✓ | ✓ | ✓ | ✓ | en partie²⁸ | en partie²⁹ |
    | Se désabonner en un clic | ✓³⁰ | en partie³¹ | ✓ | en partie³² | en partie³³ | ✓ |
    | Annuler l’envoi | ✓³⁴ | ✗ | ✓³⁵ | ✓³⁶ | ✓³⁷ | ✓³⁸ |
    | Envoi programmé, rappel plus tard | ✗³⁹ | en partie⁴⁰ | ✓ | ✓⁴¹ | ✓ | ✓⁴² |
    | Courrier signé et chiffré | ✓⁴³ | ✓⁴⁴ | en partie⁴⁵ | en partie⁴⁶ | en partie⁴⁷ | ✓⁴⁸ |
    | Adresses Outlook.com, Hotmail et Microsoft 365 | ✗⁴⁹ | ✓ | en partie⁵⁰ | ✓ | ✓ | ✗⁵¹ |

    1. Thunderbird : seulement par une extension (DKIM Verifier) ; rien sur Android.
    2. Gmail : un point d’interrogation sur un message qui n’est pas authentifié, et « Envoyé par » et « Signé par » dans ses détails, sur un ordinateur et sur Android ; pas sur un iPhone ni un iPad.
    3. Outlook : un « ? » dans l’image de l’expéditeur quand il ne peut pas le vérifier, et « via » quand l’expéditeur réel est un autre.
    4. Apple Mail n’affiche rien ; iCloud Mail vérifie SPF et DKIM, et applique DMARC, sur les serveurs d’Apple.
    5. Proton Mail : un avertissement quand un message échoue aux vérifications de son domaine ; rien n’est dit quand il les passe.
    6. Sioul : une cinquantaine de marques et de services publics, français pour la plupart, et vos propres domaines, comparés à travers leurs lettres qui se ressemblent ; les noms de domaine qui en imitent d’autres ne sont pas encore vérifiés.
    7. La détection des escroqueries de Thunderbird vérifie les liens (un texte qui nomme un autre site que celui du lien), pas les noms.
    8. Gmail : une adresse « très semblable à celle d’un expéditeur connu » est l’une des raisons d’un indésirable ; un bandeau avertit d’une escroquerie envoyée depuis l’un de vos contacts.
    9. Outlook : seulement avec Defender pour Office 365, pour les entreprises.
    10. Proton Mail : PhishGuard signale les « adresses peut-être usurpées » ; la confirmation des liens avertit des lettres qui se ressemblent dans les liens.
    11. Sioul n’affiche jamais une image à l’intérieur d’un message, même si vous le demandez.
    12. Gmail affiche les images tout de suite, par les serveurs de Google, qui cachent votre appareil et l’endroit où vous êtes ; un réglage le fait demander d’abord.
    13. Outlook : Outlook.com les charge par le relais de Microsoft ; Outlook classique et les applications pour téléphone peuvent les bloquer.
    14. Apple Mail les charge de façon privée, par deux relais (Protection de la confidentialité dans Mail), ou, sur un Mac, les bloque si vous le choisissez.
    15. Proton Mail les charge par son relais et retire les traqueurs connus.
    16. Sioul : l’antivirus de votre ordinateur (Microsoft Defender sous Windows ; ClamAV sous Linux et macOS, s’il est installé) ; un téléphone n’en a pas, et Sioul dit que le fichier n’a pas été vérifié.
    17. Thunderbird laisse cela à l’antivirus de l’ordinateur, à qui il permet de mettre un message en quarantaine.
    18. Sur les serveurs du fournisseur.
    19. macOS vérifie un programme la première fois qu’il est ouvert.
    20. Proton Mail : sur ses serveurs, pour le courrier qui n’est pas chiffré de bout en bout.
    21. Sioul apprend sur un ordinateur, d’abord quand vous le demandez, puis de lui-même une fois par semaine ; le téléphone se sert de ce qu’il a appris.
    22. Apple Mail sur un Mac apprend de ce que vous marquez comme indésirable ou non ; sur un iPhone, les serveurs d’iCloud filtrent le courrier iCloud.
    23. Les filtres de Sioul ne transfèrent pas le courrier et n’y répondent pas.
    24. Les filtres de Gmail se créent sur un ordinateur.
    25. Le nouvel Outlook n’applique aucune règle aux comptes Gmail, Yahoo ou iCloud.
    26. Apple Mail : des règles sur un Mac ; sur un iPhone, seulement les règles d’iCloud, pour les adresses iCloud.
    27. Proton Mail : un seul filtre actif avec l’offre gratuite ; des filtres Sieve aussi.
    28. Apple Mail cherche dans chaque boîte ; la recherche sur le serveur n’est pas dans sa documentation.
    29. Proton Mail ne cherche dans le texte des messages qu’après avoir construit un index local, dans un navigateur.
    30. Sioul le refuse pour le courrier falsifié, les indésirables et les noms empruntés, pour ne jamais confirmer votre adresse à un expéditeur qui ment.
    31. Thunderbird : une entrée de menu écrit le message que demande la liste ; pas de « en un clic ».
    32. Outlook : la page Abonnements d’Outlook.com.
    33. Apple Mail : un bandeau sur un Mac ; sur un iPhone, le nettoyage d’iCloud Mail, pour les adresses iCloud.
    34. Sioul : dix secondes, non réglables.
    35. Gmail : de 5 à 30 secondes.
    36. Outlook : jusqu’à 30 secondes ; jusqu’à 120 sur un Mac.
    37. Apple Mail : 10 secondes par défaut, jusqu’à 30, ou désactivé.
    38. Proton Mail : de 0 à 20 secondes, 10 par défaut.
    39. Sioul : prévu.
    40. Thunderbird : « Envoyer plus tard » garde le message dans la boîte d’envoi jusqu’à ce que vous l’envoyiez ; pas de rappel plus tard.
    41. Outlook : l’envoi programmé ne fonctionne pas pour les comptes IMAP ou POP.
    42. Proton Mail : des heures de votre choix avec les offres payantes.
    43. Sioul : OpenPGP, votre clé dans votre trousseau ou sur une clé de sécurité ; l’objet reste lisible.
    44. Thunderbird : OpenPGP et S/MIME.
    45. Gmail : S/MIME et chiffrement côté client avec certaines offres Workspace ; pas d’OpenPGP.
    46. Outlook : S/MIME avec un certificat, en général fourni par votre organisation, et « Chiffrer » pour les abonnés Microsoft 365 Personnel et Famille ; pas d’OpenPGP.
    47. Apple Mail : S/MIME, sur un Mac et sur un iPhone ou un iPad géré par une organisation ; pas d’OpenPGP.
    48. Proton Mail : de bout en bout entre adresses Proton ; OpenPGP ou un mot de passe avec les autres.
    49. Sioul : Microsoft n’accepte que sa propre page de connexion, que Sioul n’a pas encore pour le courrier.
    50. Gmail : dans ses applications pour téléphone ; Gmail sur le web cesse de relever les autres comptes en janvier 2027.
    51. Proton Mail : les adresses Proton ; un compte Gmail peut s’y relier.

=== "Technique"

    | | Sioul | Thunderbird | FairEmail | Proton Mail | Gmail | Outlook |
    |---|---|---|---|---|---|---|
    | SPF, DKIM et DMARC vérifiés par l’application elle-même | ✓¹ | ✗² | en partie³ | —⁴ | —⁴ | —⁴ |
    | Le résultat de chaque vérification affiché sur le message | ✓ | ✗² | ✓⁵ | ✗⁶ | en partie⁷ | ✗⁸ |
    | Noms de domaine qui en imitent d’autres signalés | ✗⁹ | ✗ | ✗ | en partie¹⁰ | en partie¹¹ | en partie¹² |
    | OpenPGP intégré | ✓¹³ | ✓¹⁴ | en partie¹⁵ | ✓ | ✗¹⁶ | ✗ |
    | Autocrypt | ✓¹⁷ | en partie¹⁸ | en partie¹⁹ | ✗ | — | — |
    | Les clés des autres prises sur leur domaine (Web Key Directory) | ✓²⁰ | ✓²¹ | ✗²² | ✓²³ | — | — |
    | Votre clé OpenPGP sur une clé de sécurité | en partie²⁴ | en partie²⁵ | en partie²² | ✗²⁶ | — | — |
    | L’objet chiffré aussi | ✗²⁷ | ✓²⁸ | ✗ | ✗²⁹ | ✗³⁰ | ✗ |
    | S/MIME | ✗ | ✓ | ✓³¹ | ✗ | en partie³² | ✓³³ |
    | Comptes Google et Microsoft connectés sur leur propre page (OAuth) | en partie³⁴ | ✓ | ✓³⁵ | — | — | ✓ |
    | Aucun serveur de l’éditeur entre vous et votre fournisseur | ✓ | ✓³⁶ | ✓³⁷ | —³⁸ | —³⁸ | ✗³⁹ |
    | Logiciel libre | ✓⁴⁰ | ✓⁴¹ | ✓⁴² | en partie⁴³ | ✗⁴⁴ | ✗⁴⁵ |

    1. Sioul vérifie aussi ARC et le DNS inverse du serveur d’envoi, à l’arrivée de chaque message, par le DNS de votre système (`mail-auth`, de Stalwart).
    2. Thunderbird : seulement par une extension (DKIM Verifier).
    3. FairEmail affiche ce que votre serveur a noté ; il ne vérifie DKIM lui-même que par une option de débogage, et ajoute des vérifications de MX et de listes de blocage.
    4. Le fournisseur vérifie sur ses propres serveurs.
    5. FairEmail : un bouclier, et une bande colorée quand une vérification a échoué.
    6. Proton Mail : un avertissement seulement quand un message échoue.
    7. Gmail : « Envoyé par » et « Signé par » dans les détails du message ; l’en-tête complet sous **Afficher l’original**.
    8. Outlook : un « ? » quand l’expéditeur ne peut pas être vérifié.
    9. Sioul : prévu ; les noms empruntés avec des lettres qui se ressemblent sont vérifiés (l’autre onglet).
    10. Proton Mail avertit des lettres qui se ressemblent dans les liens.
    11. Gmail : une adresse qui imite celle d’un expéditeur connu est l’une des raisons d’un indésirable ; les domaines qui en imitent d’autres sont un réglage d’administrateur des comptes d’entreprise.
    12. Outlook : dans Defender pour Office 365, pour les entreprises.
    13. Sioul : Sequoia (RFC 9580), PGP/MIME (RFC 3156).
    14. Thunderbird : intégré depuis la version 78.
    15. FairEmail : par l’application OpenKeychain.
    16. Google dit que Gmail ne peut pas lire le contenu d’un courrier chiffré avec PGP.
    17. Sioul : envoyé avec chaque message dès que vous avez une clé ; lu seulement dans le courrier qui n’est pas falsifié.
    18. Thunderbird : envoyé quand votre clé est jointe, ou toujours si vous le choisissez ; les clés peuvent en être importées ; Autocrypt Gossip depuis la 115.7.
    19. FairEmail : envoyé avec le courrier signé ou chiffré ; les clés qu’il lit vont à OpenKeychain.
    20. Sioul : quand vous appuyez sur **Chercher leurs clés**, puis keys.openpgp.org.
    21. Thunderbird : et keys.openpgp.org.
    22. FairEmail : laissé à OpenKeychain.
    23. Proton Mail : ses serveurs cherchent les clés.
    24. Sioul : une carte OpenPGP (YubiKey, Nitrokey) par le service de cartes à puce du système, sur un ordinateur seulement ; essayée jusqu’ici avec une carte logicielle, pas encore avec une vraie clé.
    25. Thunderbird : par GnuPG, installé et réglé à part ; signer et déchiffrer seulement.
    26. Proton Mail : ses clés de sécurité servent à se connecter au compte.
    27. Sioul : prévu.
    28. Thunderbird : par défaut depuis la version 91 ; on peut le désactiver.
    29. Proton Mail : « l’objet des messages Proton Mail n’est pas chiffré de bout en bout ».
    30. Gmail : le chiffrement côté client laisse l’objet, les destinataires et les heures.
    31. FairEmail : signer et chiffrer sont des fonctions payantes.
    32. Gmail : S/MIME hébergé, avec certaines offres Workspace.
    33. Outlook : avec un certificat, en général fourni par votre organisation.
    34. Sioul : Google, avec une clé Google à vous ; pas de connexion Microsoft.
    35. FairEmail : pas dans la version de F-Droid.
    36. Thunderbird : le courrier va directement à votre fournisseur ; Mozilla apprend le domaine de votre adresse quand vous l’ajoutez, et reçoit de la télémétrie tant que vous ne la désactivez pas.
    37. FairEmail : une page de FairEmail peut relayer la connexion OAuth quand Android ne le peut pas.
    38. C’est votre fournisseur.
    39. Outlook : vos autres comptes peuvent être synchronisés par le cloud de Microsoft, qui garde une copie de leur courrier (les applications pour téléphone, le nouvel Outlook pour Mac, et les comptes Gmail et Yahoo dans le nouvel Outlook pour Windows).
    40. Sioul : GPL-3.0-or-later.
    41. Thunderbird : MPL 2.0 ; Apache 2.0 sur Android.
    42. FairEmail : GPL-3.0-or-later, certaines fonctions payantes.
    43. Proton Mail : ses applications sont libres ; ses serveurs ne le sont pas.
    44. Gmail : Google concède ses logiciels pour un usage personnel, pas pour les transmettre.
    45. Outlook : Microsoft interdit de copier ou de distribuer ses logiciels.

??? info "Sources (en anglais)"
    Les articles d’aide de Mozilla ont été lus dans les copies de l’Internet Archive, support.mozilla.org refusant ce jour-là la lecture automatique.

    - Thunderbird, contenu distant dans les messages : <https://support.mozilla.org/en-US/kb/remote-content-in-messages>, lu le 8 octobre 2026.
    - Thunderbird, courrier indésirable : <https://support.mozilla.org/en-US/kb/thunderbird-and-junk-spam-messages>, lu le 8 octobre 2026.
    - Thunderbird, détection des escroqueries : <https://support.mozilla.org/en-US/kb/thunderbirds-scam-detection>, lu le 8 octobre 2026.
    - Thunderbird, réglages de vie privée (antivirus) : <https://support.mozilla.org/en-US/kb/privacy-panel-settings-in-thunderbird>, lu le 8 octobre 2026.
    - Thunderbird, filtres : <https://support.mozilla.org/en-US/kb/organize-your-messages-using-filters>, lu le 8 octobre 2026.
    - Thunderbird, recherches enregistrées : <https://support.mozilla.org/en-US/kb/using-saved-searches>, lu le 8 octobre 2026.
    - Thunderbird, OpenPGP, mode d’emploi et questions (OpenPGP, Autocrypt, WKD, objets chiffrés) : <https://support.mozilla.org/en-US/kb/openpgp-thunderbird-howto-and-faq>, lu le 8 octobre 2026.
    - Thunderbird, cartes à puce : <https://wiki.mozilla.org/Thunderbird:OpenPGP:Smartcards>, lu le 8 octobre 2026.
    - Thunderbird, raccourcis clavier (Envoyer plus tard) : <https://support.mozilla.org/en-US/kb/keyboard-shortcuts-thunderbird>, lu le 8 octobre 2026.
    - Thunderbird, notes de version 115.0, 115.7.0, 142.0 et 145.0 : <https://www.thunderbird.net/en-US/thunderbird/releases/>, lu le 8 octobre 2026.
    - Thunderbird, avis de confidentialité : <https://www.mozilla.org/en-US/privacy/thunderbird/>, lu le 8 octobre 2026.
    - Thunderbird, à propos (licence) : <https://www.thunderbird.net/en-US/about/>, lu le 8 octobre 2026.
    - Thunderbird, extension DKIM Verifier : <https://addons.thunderbird.net/en-US/thunderbird/addon/dkim-verifier/>, lu le 8 octobre 2026.
    - Thunderbird pour Android, OpenPGP : <https://support.mozilla.org/en-US/kb/openpgp-thunderbird-android-howto>, lu le 8 octobre 2026.
    - Gmail, authentification : <https://support.google.com/mail/answer/180707>, lu le 8 octobre 2026.
    - Gmail, indésirables et raisons affichées : <https://support.google.com/mail/answer/1366858>, lu le 8 octobre 2026.
    - Gmail, « Ce message pourrait être une escroquerie » : <https://support.google.com/mail/answer/1074268>, lu le 8 octobre 2026.
    - Gmail, images : <https://support.google.com/mail/answer/145919>, lu le 8 octobre 2026.
    - Gmail, antivirus : <https://support.google.com/mail/answer/25760>, lu le 8 octobre 2026.
    - Gmail, filtres : <https://support.google.com/mail/answer/6579>, lu le 8 octobre 2026.
    - Gmail, recherche : <https://support.google.com/mail/answer/7190> et <https://support.google.com/mail/answer/6593>, lu le 8 octobre 2026.
    - Gmail, désabonnement : <https://support.google.com/mail/answer/15433283>, lu le 8 octobre 2026.
    - Gmail, annuler l’envoi : <https://support.google.com/mail/answer/2819488>, lu le 8 octobre 2026.
    - Gmail, envoi programmé : <https://support.google.com/mail/answer/9214606>, lu le 8 octobre 2026.
    - Gmail, rappel plus tard : <https://support.google.com/mail/answer/7622010>, lu le 8 octobre 2026.
    - Gmail, S/MIME : <https://support.google.com/mail/answer/6330403>, lu le 8 octobre 2026.
    - Gmail, chiffrement côté client : <https://support.google.com/mail/answer/13317990>, lu le 8 octobre 2026.
    - Gmail, PGP : <https://support.google.com/transparencyreport/answer/7381230>, lu le 8 octobre 2026.
    - Gmail, autres comptes : <https://support.google.com/mail/answer/16604719> et <https://support.google.com/mail/answer/17101213>, lu le 8 octobre 2026.
    - Google, conditions d’utilisation (licence des logiciels) : <https://policies.google.com/terms>, lu le 8 octobre 2026.
    - Outlook, hameçonnage et comportements suspects : <https://support.microsoft.com/en-us/outlook/mail/phishing-and-suspicious-behavior-in-outlook>, lu le 8 octobre 2026.
    - Outlook, politiques anti-hameçonnage de Defender pour Office 365 : <https://learn.microsoft.com/en-us/defender-office-365/anti-phishing-policies-about>, lu le 8 octobre 2026.
    - Outlook, protection des images externes dans Outlook.com : <https://support.microsoft.com/en-us/office/external-image-protection-in-outlook-com-43c0c17e-8fd1-41c6-93fe-ffe54638e82b>, lu le 8 octobre 2026.
    - Outlook, bloquer les images externes : <https://support.microsoft.com/en-us/outlook/what-is-block-external-images-in-my-settings>, lu le 8 octobre 2026.
    - Outlook, téléchargement des images dans Outlook classique : <https://support.microsoft.com/en-us/office/block-or-unblock-automatic-picture-downloads-in-email-messages-15e08854-6808-49b1-9a0a-50b81f2d617a>, lu le 8 octobre 2026.
    - Outlook, sécurité avancée d’Outlook.com (pièces jointes) : <https://support.microsoft.com/en-us/office/advanced-outlook-com-security-for-microsoft-365-subscribers-882d2243-eab9-4545-a58a-b36fee4a46e2>, lu le 8 octobre 2026.
    - Outlook, filtre du courrier indésirable : <https://support.microsoft.com/en-us/outlook/filter-junk-email-and-spam-in-outlook>, lu le 8 octobre 2026.
    - Outlook, règles : <https://support.microsoft.com/en-us/outlook/mail/manage-email-messages-by-using-rules-in-outlook>, lu le 8 octobre 2026.
    - Outlook, recherche : <https://support.microsoft.com/en-us/outlook/getstarted/how-to-search-in-outlook>, lu le 8 octobre 2026.
    - Outlook, abonnements : <https://support.microsoft.com/en-us/outlook/how-to-manage-email-subscriptions-in-outlook-com>, lu le 8 octobre 2026.
    - Outlook, annuler l’envoi : <https://support.microsoft.com/en-us/outlook/mail/how-to-recall-an-email-in-outlook-requirements-limitations-steps>, lu le 8 octobre 2026.
    - Outlook pour Mac, annuler l’envoi : <https://support.microsoft.com/en-us/outlook/undo-send-in-outlook-for-mac>, lu le 8 octobre 2026.
    - Outlook, envoi programmé : <https://support.microsoft.com/en-us/outlook/mail/delay-or-schedule-sending-email-messages-in-outlook>, lu le 8 octobre 2026.
    - Outlook, rappel plus tard : <https://support.microsoft.com/en-us/outlook/officeweb/organize-your-inbox>, lu le 8 octobre 2026.
    - Outlook, S/MIME : <https://support.microsoft.com/en-us/outlook/mail/set-up-outlook-to-use-s-mime-encryption>, lu le 8 octobre 2026.
    - Outlook, « Chiffrer » pour Personnel et Famille : <https://support.microsoft.com/en-us/outlook/send-encrypted-messages-with-a-microsoft-365-personal-or-family-subscription>, lu le 8 octobre 2026.
    - Outlook, ajouter des comptes : <https://support.microsoft.com/en-us/outlook/getstarted/add-an-email-account-to-outlook-for-windows>, lu le 8 octobre 2026.
    - Outlook, ajouter un compte Gmail : <https://support.microsoft.com/en-us/outlook/getstarted/add-a-gmail-account-to-outlook-for-windows>, lu le 8 octobre 2026.
    - Outlook, synchroniser d’autres comptes par le cloud de Microsoft : <https://support.microsoft.com/en-us/outlook/getstarted/sync-your-account-in-outlook-to-the-microsoft-cloud>, lu le 8 octobre 2026.
    - Contrat de services Microsoft (licence des logiciels) : <https://www.microsoft.com/en-us/servicesagreement>, lu le 8 octobre 2026.
    - Apple Mail, vérifications d’iCloud Mail : <https://support.apple.com/en-us/102322>, lu le 8 octobre 2026.
    - Apple Mail, protéger la confidentialité du courrier sur un Mac : <https://support.apple.com/guide/mail/protect-email-privacy-mlhlp1205/mac>, lu le 8 octobre 2026.
    - Apple, les relais de la Protection de la confidentialité dans Mail : <https://www.apple.com/legal/privacy/data/en/mail-privacy-protection/>, lu le 8 octobre 2026.
    - Apple, protection contre les logiciels malveillants dans macOS : <https://support.apple.com/guide/security/protecting-against-malware-sec469d47bd8/web>, lu le 8 octobre 2026.
    - Apple Mail, courrier indésirable sur un Mac : <https://support.apple.com/guide/mail/reduce-junk-mail-mlhlp1065/mac>, lu le 8 octobre 2026.
    - Apple, courrier indésirable dans iCloud : <https://support.apple.com/guide/icloud/manage-junk-mail-mm6b1a2ced/icloud>, lu le 8 octobre 2026.
    - Apple Mail, règles et boîtes aux lettres intelligentes sur un Mac : <https://support.apple.com/guide/mail/automatically-sort-incoming-emails-mlhlp1190/mac>, lu le 8 octobre 2026.
    - Apple, règles d’iCloud Mail sur un iPhone : <https://support.apple.com/guide/iphone/icloud-mail-rules-automatically-apply-iph02be4f1c8/ios>, lu le 8 octobre 2026.
    - Apple Mail, recherche : <https://support.apple.com/guide/iphone/search-for-email-iphb2eab8035/ios> et <https://support.apple.com/guide/mail/search-for-emails-mlhlp1003/mac>, lu le 8 octobre 2026.
    - Apple, nettoyage d’iCloud Mail : <https://support.apple.com/guide/iphone/automatically-clean-up-icloud-mail-iphb48813489/ios>, lu le 8 octobre 2026.
    - Apple Mail, annuler l’envoi : <https://support.apple.com/guide/iphone/unsend-email-with-undo-send-iph0e7288015/ios>, lu le 8 octobre 2026.
    - Apple Mail, envoyer plus tard : <https://support.apple.com/guide/iphone/send-email-iph742b6abb1/ios>, lu le 8 octobre 2026.
    - Apple Mail, rappel plus tard : <https://support.apple.com/guide/iphone/check-your-email-iph461684497/ios>, lu le 8 octobre 2026.
    - Apple, S/MIME sur les appareils gérés : <https://support.apple.com/guide/deployment/mail-payload-settings-dep9c14bfc5/web>, lu le 8 octobre 2026.
    - Apple, comptes de courrier sur un iPhone : <https://support.apple.com/guide/iphone/set-up-mail-contacts-and-calendar-accounts-ipha0d932e96/ios>, lu le 8 octobre 2026.
    - Proton Mail, avertissement d’échec d’authentification : <https://proton.me/support/email-has-failed-its-domains-authentication-requirements-warning>, lu le 8 octobre 2026.
    - Proton Mail, fonctions de sécurité (PhishGuard) : <https://proton.me/mail/security>, lu le 8 octobre 2026.
    - Proton Mail, liens homographes : <https://proton.me/support/homograph-attacks>, lu le 8 octobre 2026.
    - Proton Mail, protection contre les traqueurs : <https://proton.me/support/email-tracker-protection>, lu le 8 octobre 2026.
    - Proton Mail, images : <https://proton.me/support/protonmail-images>, lu le 8 octobre 2026.
    - Proton Mail, politique de confidentialité (analyse antivirus) : <https://proton.me/mail/privacy-policy>, lu le 8 octobre 2026.
    - Proton Mail, filtrage des indésirables : <https://proton.me/support/spam-filtering>, lu le 8 octobre 2026.
    - Proton Mail, filtres : <https://proton.me/support/email-inbox-filters>, lu le 8 octobre 2026.
    - Proton Mail, filtres Sieve : <https://proton.me/support/sieve-advanced-custom-filters>, lu le 8 octobre 2026.
    - Proton Mail, recherche : <https://proton.me/support/search> et <https://proton.me/support/search-message-content>, lu le 8 octobre 2026.
    - Proton Mail, désabonnement : <https://proton.me/support/auto-unsubscribe>, lu le 8 octobre 2026.
    - Proton Mail, annuler l’envoi : <https://proton.me/support/undo-send>, lu le 8 octobre 2026.
    - Proton Mail, envoi programmé : <https://proton.me/support/schedule-email-send>, lu le 8 octobre 2026.
    - Proton Mail, rappel plus tard : <https://proton.me/support/snooze-emails>, lu le 8 octobre 2026.
    - Proton Mail, chiffrement et objets : <https://proton.me/support/proton-mail-encryption-explained>, lu le 8 octobre 2026.
    - Proton Mail, PGP avec les autres : <https://proton.me/support/how-to-use-pgp>, lu le 8 octobre 2026.
    - Proton Mail, WKD : <https://proton.me/blog/security-updates-2019>, lu le 8 octobre 2026.
    - Proton Mail, courrier protégé par un mot de passe : <https://proton.me/support/password-protected-emails>, lu le 8 octobre 2026.
    - Proton, clés de sécurité : <https://proton.me/support/2fa-security-key>, lu le 8 octobre 2026.
    - Proton Mail, relier Gmail : <https://proton.me/blog/proton-mail-connect-gmail>, lu le 8 octobre 2026.
    - Proton, code ouvert : <https://proton.me/community/open-source>, lu le 8 octobre 2026.
    - Proton Mail, applications : <https://proton.me/mail/download>, lu le 8 octobre 2026.
    - FairEmail, FAQ (sections 12, 92, 111, 163 et 181), README, PRIVACY et CHANGELOG : <https://github.com/M66B/FairEmail>, lu le 8 octobre 2026.

## Côté technique {#for-technical-readers}

### Les adresses qui fonctionnent {#which-addresses-work}

Tout serveur IMAP et SMTP qui accepte un mot de passe par une connexion chiffrée : TLS dès le premier octet (ports 993 et 465, comme le préfère la RFC 8314) ou STARTTLS (143 et 587), jamais une connexion en clair ; un serveur qui ne propose pas STARTTLS est refusé. Les certificats sont vérifiés avec ceux de votre système (rustls, TLS 1.2 et 1.3). IMAP4rev2 (RFC 9051) et IMAP4rev1, avec IDLE (RFC 2177, renouvelé toutes les cinq minutes), MOVE, UIDPLUS et SPECIAL-USE ; chaque adresse est gardée en Maildir, dont vous seul pouvez lire les fichiers sous Linux et macOS.

Gmail et Google Workspace acceptent un mot de passe d’application, ou **Se connecter avec Google** avec une clé Google à vous (OAuth 2.0 pour les applications natives, PKCE, SASL XOAUTH2), pas encore essayé avec Google lui-même ([Gmail et Google Workspace](first-steps.md#gmail-and-google-workspace)). Les adresses Outlook.com, Hotmail et Microsoft 365 ne peuvent pas être ajoutées : Microsoft n’accepte que sa propre page de connexion, et Sioul n’en a pas encore pour le courrier. JMAP, POP3 et les protocoles propres à Exchange ne sont pas pris en charge. Plus de détails dans [Fonctionne avec](compatibility.md#mail).

### Comment un expéditeur est vérifié {#how-a-sender-is-checked}

Quand un message est relevé, Sioul le vérifie lui-même, par le DNS de votre système (`mail-auth`, de Stalwart), en vingt secondes au plus :

- **SPF** (RFC 7208), sur le serveur qui a remis le message à votre fournisseur : la première adresse publique en partant du haut des lignes `Received`, puisque chaque ligne en dessous pourrait être écrite par n’importe qui ;
- **DKIM** (RFC 6376), chaque signature contre la clé que publie son domaine, à l’arrivée, puisque les clés changent et que certaines signatures expirent en quelques jours ;
- **DMARC** (RFC 7489), avec la politique du domaine lui-même ;
- **ARC** (RFC 8617), pour le courrier transféré, et le **DNS inverse** du serveur d’envoi.

Les résultats sont écrits en tête du message gardé, dans un en-tête `Authentication-Results` (RFC 8601), sous un nom que seule votre copie de Sioul utilise, terminé par `.invalid`, pour qu’aucun expéditeur ne puisse écrire des résultats qui passeraient pour ceux de Sioul. Sioul se fie d’abord à eux, puis à l’en-tête de votre fournisseur, dont il apprend le nom dans votre courrier. Un message trop gros pour être relevé en entier est vérifié sur ses en-têtes : DKIM et ARC ne sont alors pas dits, et DMARC ne l’est que s’il passe.

- **Vérifié** : DMARC passe pour le domaine affiché dans De, ou une signature DKIM valide de ce même domaine. La signature d’un autre domaine ne prouve rien de l’expéditeur.
- **Falsifié** : DMARC échoue, et le domaine demande aux destinataires d’en tenir compte (`p=quarantine` ou `p=reject`). Avec `p=none`, ou par une liste de diffusion, qui réécrit ce qu’elle relaie, l’expéditeur est seulement non vérifié. Aucune liste ne relaie vos codes : un code qui échoue à DMARC sous une politique est falsifié, en-têtes de liste ou non.
- **Non authentifié** : SPF et DKIM ont tous deux échoué, et ni DMARC ni une chaîne ARC scellée par votre fournisseur ou l’un de vos domaines ne s’en porte garant. Rien ne prouve qu’il vient de l’adresse qu’il affiche : son expéditeur compte comme un inconnu pour chaque règle du Porche (filtre d’accueil, jugement des filtres à indésirables, aucune file de projet par l’adresse).

Survoler le bouclier montre chaque vérification, son résultat, le domaine concerné, et qui l’a faite : Sioul à l’arrivée, ou votre fournisseur. L’avis de votre fournisseur sur les indésirables (les en-têtes de SpamAssassin ou de rspamd) n’est lu que dans les lignes qu’il a écrites lui-même, au-dessus de celle où le message est entré.

### Les noms empruntés {#borrowed-names}

Une signature prouve un domaine, pas le nom écrit devant l’adresse. Sioul compare le nom affiché avec une cinquantaine de marques et de services publics, français pour la plupart (les impôts, l’assurance maladie, la CAF, l’ANTAI, France Travail, La Poste et les transporteurs de colis, les fournisseurs d’énergie, les opérateurs de téléphone, les banques) à côté de grandes plateformes (Amazon, PayPal, Netflix, Apple, Google, Microsoft), et avec vos propres domaines, après avoir réduit les deux à un squelette de lettres qui se ressemblent (la détection des confusables d’Unicode, UTS #39 : l pour I, 0 pour O, rn pour m, les lettres cyrilliques). Un nom qui en revendique un depuis une adresse hors de ses domaines est mis de côté ; un expéditeur que vous connaissez, ou que vous avez laissé entrer, garde le nom qu’il utilise. Les grands fournisseurs partagés, comme gmail.com, ne sont le domaine de personne. Les noms de domaine qui en imitent d’autres ne sont pas encore vérifiés.

### Le courrier affiché sans risque {#mail-shown-safely}

Le HTML est nettoyé avec une liste de ce qui est permis (`ammonia`) : la structure du texte et les liens seulement ; les styles, les scripts et l’en-tête de la page retirés avec leur contenu, les commentaires aussi ; aucun attribut sauf l’adresse d’un lien ; seulement les liens `http`, `https` et `mailto` ; les liens relatifs refusés, puisqu’ils nommeraient un fichier ou un partage de votre ordinateur. Les images ne sont jamais chargées, et aucun réglage ne les charge. Le site d’un lien est lu après tout `nom@` placé devant pour tromper (`https://banque.example@autre.example` affiche autre.example), et un lien `mailto:` ouvre un brouillon vers sa seule adresse, sans l’objet ni le texte qu’il remplirait.

### L’antivirus {#the-antivirus}

Sous Windows, l’Antimalware Scan Interface (AMSI) passe le fichier à l’antivirus que Windows fait tourner, Microsoft Defender par défaut ; ce chemin suit la séquence documentée par Microsoft et est construit par les compilations automatiques du projet, mais n’a pas encore tourné sur un ordinateur Windows. Sous Linux et macOS, ClamAV quand le système l’a : son démon (`clamdscan --fdpass`), sinon son analyseur avec les signatures du système, sinon avec des signatures que Sioul tient à jour une fois par jour avec le `freshclam` du système. Rien de ClamAV n’est fourni avec Sioul. Un Flatpak ne peut pas joindre le ClamAV du système, et demande. Les programmes (`.exe`, `.js`, `.lnk`, `.desktop`, `.iso`…) ne sont jamais qu’enregistrés. Le nom d’une pièce jointe est nettoyé avant d’être écrit : aucun dossier dedans, et sous Windows aucun nom de périphérique ni flux caché. Les fichiers ouverts ou enregistrés portent la Mark of the Web de Windows (`Zone.Identifier`, zone 3) ou l’attribut de quarantaine de macOS (`com.apple.quarantine`). Sur Android, rien n’est analysé : Android n’offre aucun moyen de faire analyser un fichier par une autre application, et ClamAV n’y tourne pas. La pièce jointe va à l’application que vous choisissez par une adresse `content://` prêtée en lecture seule, pour ce seul fichier ; son type vient de son nom, jamais du message, et n’est jamais celui de l’installateur d’Android.

### OpenPGP {#openpgp}

Sioul utilise Sequoia (OpenPGP, RFC 9580) avec sa cryptographie en pur Rust et sa politique standard, la même sur chaque système. Les messages partent en PGP/MIME (RFC 3156) : signés seuls en `multipart/signed`, sinon en `multipart/encrypted` avec la signature à l’intérieur, chiffrés pour chaque destinataire et pour vous, pour que vous puissiez les relire dans Envoyés. Le PGP en ligne est lu aussi. Un message déchiffré ne vit qu’en mémoire. Une clé créée dans Sioul est en Curve25519 (EdDSA pour signer, ECDH pour chiffrer), valable trois ans, sa phrase de passe tirée au hasard et gardée dans votre trousseau. Les clés des autres viennent de leurs messages (Autocrypt, jamais d’un courrier falsifié, pour qu’un message falsifié ne puisse pas glisser une clé pour quelqu’un à qui vous écrivez), d’un fichier, ou, quand vous le demandez, du Web Key Directory de leur domaine, puis de keys.openpgp.org, en HTTPS. Vos propres clés restent sur l’appareil où elles ont été créées ou importées ; les clés publiques des autres voyagent vers vos autres appareils, scellées. Pas encore : les en-têtes protégés (l’objet reste lisible), révoquer ou prolonger une clé, S/MIME.

### Les clés de sécurité {#security-keys}

Une carte OpenPGP (spécification 3.4 : une YubiKey, une Nitrokey), jointe par le service de cartes à puce du système (PC/SC : pcscd sous Linux, le service Carte à puce sous Windows, CryptoTokenKit sous macOS). La carte signe une empreinte ou ouvre une clé de session elle-même ; ses clés privées ne la quittent jamais, et chaque signature qu’elle fait est vérifiée avec votre certificat avant usage. Le code PIN est tapé dans Sioul, gardé chiffré en mémoire, oublié quinze minutes après son dernier usage, quand la clé est retirée ou que Sioul se ferme, et jamais écrit ni journalisé : Sioul n’installe aucun journal, et plafonne la journalisation dès son démarrage, puisque les traces de la bibliothèque de la carte contiendraient le code PIN. La carte est remise à zéro après chaque opération, pour qu’aucun autre programme ne la trouve déverrouillée. Sioul ne demande jamais le code PIN d’administration et ne crée ni ne charge jamais de clés sur la carte. Essayé jusqu’ici avec une carte logicielle ; un essai avec une vraie YubiKey est la prochaine étape. Pas sur les téléphones.

### Le filtre à indésirables {#the-spam-filter}

Un classifieur fastText, entraîné sur un ordinateur à partir de votre propre courrier : les mots, et des faits tirés des en-têtes, dont les résultats SPF, DKIM, DMARC, ARC et DNS inverse de Sioul lui-même. Il est calibré (Platt), puis réduit en une petite table de mots sous forme d’empreintes, sans aucun mot en clair, que chaque appareil lit, votre téléphone aussi, et qui voyage scellée. Une nouvelle table ne remplace celle en service que si elle ne prend pas plus de bon courrier pour indésirable sur votre courrier le plus récent. Son apprentissage lit votre courrier sans rien changer sur le serveur, et garde ce qu’il a lu sur cet ordinateur. Plus de détails : [Vie privée et sécurité](privacy-security.md#your-own-spam-filter) et [les notes des développeurs (en anglais)](https://aurelienpierre.github.io/sioul/dev/spam-filter.html).

### Se désabonner en un clic {#unsubscribing-in-one-click}

Le « en un clic » (RFC 8058) ne sert que si une signature DKIM valide couvre à la fois `List-Unsubscribe` et `List-Unsubscribe-Post`, et appartient au domaine de De ou à celui de l’adresse elle-même ; il envoie le seul formulaire en HTTPS, vers un nom sur Internet, sans cookie ni rien de vous, ne suit les redirections qu’au sein du même site, et abandonne au bout de quinze secondes. Sinon, Sioul envoie le message que la liste demande (RFC 2369), depuis votre adresse, ou ouvre la page de la liste. Deux en-têtes `List-Unsubscribe`, ou une adresse inutilisable, et il ne fait rien.

### La recherche et les filtres sur le serveur {#search-and-filters-on-the-server}

Une recherche interroge les serveurs avec `UID SEARCH` pour les dossiers et les jours que cet appareil n’a pas, vérifie ici ce qu’ils trouvent sur ses en-têtes, et ne marque jamais rien comme lu (`BODY.PEEK`). Chez Gmail, les pièces jointes sont trouvées avec la recherche propre à Gmail. Avant qu’un filtre agisse, l’appareil marque le message du mot-clé `$SioulFiltered`, avec CONDSTORE (RFC 7162) quand le serveur le propose, pour que deux de vos appareils n’agissent jamais sur le même message. Les serveurs sans mots-clés (Exchange, Outlook.com) ne peuvent pas porter la marque ; là, ce qu’un filtre fait deux fois laisse le message comme une fois.

### Ce que Sioul écrit, et ce qu’il envoie {#what-sioul-writes-and-what-it-sends}

Relever ne change rien sur le serveur (`EXAMINE`, `BODY.PEEK`). Sioul n’y écrit que quand vous agissez, ou quand vos filtres et votre filtre à indésirables font ce que vous leur avez demandé : une marque de lecture quand vous ouvrez un message, un déplacement dix secondes après votre demande, une copie dans Envoyés, un mot-clé (`$Junk`, `$NotJunk`, `$SioulFiltered`). Quand il envoie, il se présente comme `[127.0.0.1]`, pour que le nom de votre ordinateur n’entre pas dans les lignes `Received` du message ; il n’écrit pas de `X-Mailer`, et crée le Message-ID sous le domaine de votre propre adresse. Il n’envoie jamais d’accusé de lecture.
