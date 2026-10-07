---
description: Le client de courrier de Sioul – dossiers, lecture, écriture en Markdown, dix secondes pour annuler, OpenPGP, invitations – sans compteurs ni pastilles.
---

# Le courrier {#mail}

Le Porche est fait pour ce qui est nouveau. La page Courrier, pour quand vous choisissez de regarder : chaque adresse, chaque dossier, tout le courrier gardé. Elle garde le calme du Porche : pas de compteur de non-lus, pas de pastilles, pas de rouge.

<figure markdown="span">
  [![La page Courrier : une adresse dépliée avec ses dossiers (Boîte de réception, Envoyés, Brouillons, Archives, Indésirables, Corbeille) et une autre repliée en dessous ; au milieu, la boîte de réception : « Sept messages que vous n’avez pas lus. », puis une ligne par message avec qui, quoi et quand, un petit point devant ceux qui ne sont pas lus ; un champ de recherche et la case Temps réel en haut.](../assets/screens/fr/mail.png){ loading=lazy }](../assets/screens/fr/mail.png "Ouvrir l’image en grand")
  <figcaption>Les dossiers à gauche, les deux dernières semaines au milieu, aucun compteur.</figcaption>
</figure>

## Les dossiers {#folders}

- **À gauche**, chaque adresse avec ses dossiers principaux : Boîte de réception, Envoyés, Brouillons, Archives, Indésirables, Corbeille. Les autres sont repliés sous **Autres dossiers**.
- **Un petit point** marque un dossier qui a du nouveau. Combien, c’est dit en toutes lettres quand vous l’ouvrez.
- **Un dossier montre ses deux dernières semaines.** **Messages plus anciens** ouvre le reste, pour qu’aucune liste ne soit sans fin.
- **Chercher**, en haut d’un dossier : par expéditeur, destinataire et objet. **Plus de critères…**, à côté, cherche partout, par conditions ([plus bas](#searching)).
- **Par conversation**, dans le ⚙ de la page Courrier : un message et ses réponses ensemble, sous le plus récent ; vos propres réponses viennent des Envoyés.

Un clic droit sur un dossier propose **En garder une copie ici** ou **Ne plus en garder de copie** (le serveur garde tous les messages dans les deux cas), **Nouveau dossier…**, et **Supprimer ce dossier vide**, pour un dossier vide à vous.

## Chercher {#searching}

Le champ en haut d’un dossier trouve un expéditeur, un destinataire ou un objet dans ce dossier. Quand il en faut plus, **Plus de critères…**, à côté, ouvre la recherche par conditions à la place des dossiers. Rien de tout cela ne se montre avant.

<figure markdown="span">
  [![La page Courrier pendant une recherche : à gauche, à la place des dossiers, « Recherche », ce qu’elle parcourt, « Le courrier qui les remplit toutes », puis deux conditions, De contient « no-reply » et Jour d’arrivée après une date, Ajouter une condition, Effacer et En faire un filtre… ; au milieu, « Résultats », la phrase « Le courrier de …no-reply…, arrivé après le … », combien ont été trouvés, puis les messages, chacun avec l’endroit où il est, comme « Boîte de réception · noa@example.com ».](../assets/screens/fr/mail-search.png){ loading=lazy }](../assets/screens/fr/mail-search.png "Ouvrir l’image en grand")
  <figcaption>La recherche à la place des dossiers ; les résultats dans la liste habituelle, chacun disant où il est.</figcaption>
</figure>

- **Une condition d’abord**, Partout, avec ce que vous aviez tapé dans la recherche du dossier. **Ajouter une condition** pour une autre : l’expéditeur, les destinataires, l’objet, le texte, une pièce jointe (il y en a une, aucune, une avec un nom), le type de pièce jointe (un PDF, une image…), le jour d’arrivée, la taille, qui est l’expéditeur pour vous, une lettre d’information ou une liste, l’adresse, le dossier, et s’il est lu, suivi ou répondu. Majuscules et accents n’y changent rien.
- Avec deux conditions ou plus, choisissez **Le courrier qui les remplit toutes** ou **qui en remplit au moins une**.
- **Les résultats** prennent la place de la liste, les plus récents d’abord, chacun disant où il est. Un message s’ouvre à côté comme dans un dossier : vous allez et venez entre les résultats et les messages. Au-dessus, une phrase dit ce qui est cherché : « Le courrier de …@banque.example…, arrivé après le 3 juin, avec une pièce jointe. »
- **Partout** : chaque dossier de chaque adresse, sauf les indésirables et la corbeille, à moins de les nommer dans une condition Dossier.
- **Sur les serveurs aussi** : Sioul garde ici votre courrier récent, et fait venir le plus ancien selon la place sur le disque ; les dossiers gardés sur le serveur seulement ne sont pas ici du tout. Un instant après que vous avez fini de taper (tout de suite avec ++"Entrée"++), Sioul demande le reste à vos serveurs, et le dit : « Recherche sur les serveurs… », puis « 3 de plus sur les serveurs. », ou pourquoi un serveur n’a pas pu être interrogé. Ces messages disent « sur le serveur » ; en ouvrir un le fait d’abord venir ici. Un serveur tient plus aux accents que Sioul : écrivez-les comme les messages les écrivent.
- **Effacer** revient au dossier.
- **En faire un filtre…**, à côté, fait de la recherche un filtre de courrier : son éditeur s’ouvre dans le ⚙ de la page Courrier, pour choisir ce qu’il fait de ce courrier à son arrivée.

Les conditions sont celles des [filtres du courrier](#filters), avec les mêmes mots.

## Filtres {#filters}

Les filtres agissent sur le courrier qui arrive, sur son serveur : dans un dossier, archivé, en indésirable, suivi, marqué comme lu, à la corbeille, ou avec un mot-clé. Ils sont dans le ⚙ de la page Courrier, sous **Filtres** : une seule liste pour toutes vos adresses, chaque filtre en une phrase.

<figure markdown="span">
  [![La page Courrier, ses réglages ouverts à droite, sous Filtres : un court paragraphe sur ce que font les filtres, « Montrer les filtres de : Toutes les adresses », puis quatre filtres, chacun un interrupteur et une phrase : Banque, « De contient « @banquedesberges.example.org » → dans « Archive », marqué comme lu, et aucun autre filtre » ; « Venu d’une lettre d’information ou d’une liste et arrivé un samedi ou dimanche → marqué comme lu » ; « De contient « @bonnes-affaires.example.com » → dans les indésirables », seulement pour noa.ferrand@example.org ; Factures, inactif et estompé ; puis Ajouter un filtre, et Les appliquer aux boîtes de réception….](../assets/screens/fr/mail-filters.png){ loading=lazy }](../assets/screens/fr/mail-filters.png "Ouvrir l’image en grand")
  <figcaption>Chaque filtre en une phrase, avec son interrupteur ; inactif, il est gardé et ne fait rien.</figcaption>
</figure>

- **Ajouter un filtre** l’ouvre sur place, avec une condition et une action. Choisissez ce que lit la condition (De, À, Cc, Répondre à, Objet, Texte, Partout, une pièce jointe, son type, qui est l’expéditeur pour vous, une lettre d’information ou une liste, le jour ou l’heure d’arrivée, la taille), comment elle compare (contient, ne contient pas, est, n’est pas ; avant, après, entre ; plus gros, plus petit), et ce qu’elle cherche. **Ajouter une condition** pour une autre ; à partir de deux, choisissez si **toutes les conditions sont vraies** ou si **l’une d’elles est vraie**. La casse et les accents ne comptent pas, comme dans la recherche.
- **Alors** : le déplacer dans un dossier, l’archiver, le marquer comme indésirable, le suivre, le marquer comme lu, le mettre à la corbeille, ou lui ajouter un mot-clé. **Ajouter une action** pour une autre.
- **Une phrase** dit le filtre à mesure que vous le composez, et ce qui manque s’il manque quelque chose : « Choisissez le dossier où il déplace les messages. » Chaque changement est gardé aussitôt.
- Repliés sous une ligne : **Sur quelles adresses** il agit (toutes, ou certaines), et **Une fois qu’il agit, les filtres suivants ne sont pas demandés**.
- **L’essayer sur les boîtes de réception** compte ce qu’il prend dans vos boîtes de réception maintenant, lu ou non, et en nomme quelques-uns.

<figure markdown="span">
  [![Le filtre Banque ouvert sur place sous sa phrase : son nom ; Si : De, contient, « @banquedesberges.example.org » ; Ajouter une condition ; Alors : Le déplacer dans un dossier, Archive, et Le marquer comme lu, chacun avec × ; Ajouter une action ; dépliés, Sur quelles adresses : Toutes les adresses, et Une fois qu’il agit, les filtres suivants ne sont pas demandés, cochés tous les deux ; L’essayer sur les boîtes de réception, Terminé, Supprimer ce filtre ; puis « Dans vos boîtes de réception, il prend un des 28 messages. » et le message qu’il prend.](../assets/screens/fr/mail-filter-editor.png){ loading=lazy }](../assets/screens/fr/mail-filter-editor.png "Ouvrir l’image en grand")
  <figcaption>Un filtre ouvert : sa condition, ses actions, et ce qu’il prend dans vos boîtes de réception maintenant.</figcaption>
</figure>

- **L’ordre** compte : le premier filtre qui déplace un message décide où il va. ▴ et ▾ demandent un filtre plus tôt ou plus tard. Avec plusieurs adresses, **Montrer les filtres de** montre ceux d’une adresse.
- **Quand ils agissent** : sur le courrier qui arrive non lu dans une boîte de réception, sur le premier de vos appareils à le relever (cet ordinateur, votre téléphone en arrière-plan, `sioul watch`). Cet appareil marque le message sur son serveur, pour que vos autres appareils n’y touchent plus. Vos filtres vont sur vos autres appareils avec vos réglages.
- **Ce qu’ils ne touchent jamais** : le courrier que le Porche met de côté, un code que vous avez demandé, ce que votre filtre à indésirables a retenu. Rien n’est supprimé pour de bon : la corbeille le garde.
- **Pas annoncé** : un message qu’un filtre sort de la boîte de réception, ou marque comme lu, n’est pas notifié, et n’attend pas sur le Porche. Quand un filtre ne peut pas agir sur un message (un dossier manquant, le serveur qui refuse), le message est notifié comme tout nouveau courrier, et la ligne d’état dit pourquoi.
- **Les appliquer aux boîtes de réception…** les applique à tout ce qui est dans vos boîtes de réception maintenant, le courrier lu aussi. Sioul dit d’abord ce qui changerait ; **Les appliquer maintenant** le fait au bout de dix secondes, avec **Annuler**.
- **Depuis une recherche** : **En faire un filtre…**, à côté du bouton Effacer de la recherche, fait un filtre de ses conditions et l’ouvre à la fin de la liste, pour que vous choisissiez ce qu’il fait.

## Lire {#reading}

Un message s’ouvre à droite ; les dossiers se replient pendant que vous lisez. Son expéditeur, jusqu’où il est vérifié, ses pièces jointes et les messages précédents qu’il cite s’affichent comme sur [le Porche](porch.md#reading-a-message).

<figure markdown="span">
  [![Un message ouvert à côté de la liste : au-dessus, Répondre, Transférer, Ajouter, Lier à…, Archiver et Supprimer ; puis son expéditeur, son adresse marquée « vérifié », à qui et quand, l’objet ; puis son texte, la signature estompée, et en bas « Affiché sans risque : rien de distant ne se charge, rien ne s’exécute. »](../assets/screens/fr/mail-reader.png){ loading=lazy }](../assets/screens/fr/mail-reader.png "Ouvrir l’image en grand")
  <figcaption>Un message : qui l’a envoyé et jusqu’où c’est vérifié, puis son texte, rendu sûr.</figcaption>
</figure>

Au-dessus du message, toujours au même endroit :

- **Répondre**, **Répondre à tous** (seulement quand il y a d’autres personnes à qui répondre), **Transférer** ;
- **Ajouter ▾** (une tâche, un événement, une note, une réponse, l’expéditeur dans vos contacts) et **Lier à…** (tout le reste : une tâche, une note, un projet) ;
- **Archiver**, **Supprimer**, **Indésirable** ;
- **Se désabonner**, sur une lettre d’information ou le message d’une liste qui dit comment la quitter ([plus bas](#unsubscribing)).

Archiver, supprimer et mettre aux indésirables se font tout de suite, avec **Annuler** dans la ligne d’état pendant dix secondes ; le serveur n’est prévenu qu’après. Il n’y a pas de « Voulez-vous vraiment ? ». Dans la corbeille, **Supprimer définitivement** supprime pour de bon, avec les mêmes dix secondes.

Ce que vous marquez **Indésirable**, ou **Pas indésirable** dans le dossier des indésirables, apprend à votre propre filtre à son prochain apprentissage ([plus bas](#your-own-spam-filter)).

Un clic droit sur un message (ou son ⋮, ou la touche Menu) donne le reste : marquer comme lu ou non lu, suivre, **Déplacer vers…**, **Voir la source**, bloquer l’expéditeur, **Comment cette personne vous joint…** (sa liste, Passe toujours, ce qui vous joint de sa part : [La fiche d’une personne](notifications.md#a-persons-sheet)), **Garder comme contrat…**.

Ouvrir un message le marque comme lu, comme le fait tout logiciel de courrier.

### Plusieurs à la fois {#several-at-once}

- ++ctrl++ + clic ajoute un message à la sélection ou l’en retire ; ++"Maj"++ + clic prend tout depuis le dernier cliqué ; ++ctrl+a++ prend tout ; ++"Échap"++, rien. Sur un écran tactile, un appui long ouvre le menu d’un message : **Sélectionner**, puis un toucher en choisit d’autres, ou en retire un.
- Une barre les marque alors comme lus, les archive, les supprime ou les déplace, sous un seul **Annuler**, quel que soit leur nombre.
- **Glissez-les** sur n’importe quel dossier de n’importe quelle adresse : le dossier sous le pointeur est entouré. Depuis les résultats d’une recherche aussi, et c’est ainsi qu’on trie une boîte chargée : les dossiers reviennent à la place de la recherche le temps du glisser. Vers une autre adresse, un message n’est retiré de la première qu’une fois que le second serveur l’a reçu.
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

Le bouton reste en retrait, estompé, sur un message falsifié, mis de côté comme indésirable, hostile, dans les indésirables, ou d’un expéditeur que Sioul ne peut pas vérifier : y répondre dirait à son expéditeur que votre adresse est lue, ou toucherait quelqu’un d’autre. Son info-bulle, ou un toucher sur un téléphone, dit pourquoi.

Une liste quittée montre **Abonnement arrêté**, et depuis quand. Si ses messages continuent d’arriver, bloquez l’expéditeur : ⋮ ▸ **Comment cette personne vous joint…** ▸ **Bloqué**. Les listes quittées sont dans le ⚙ de la page Courrier, sous **Listes quittées**, sur cet appareil.

### Les invitations {#invitations}

Un message qui apporte une invitation la montre au-dessus du texte : **Accepter**, **Peut-être**, **Décliner**. L’événement va dans votre agenda et votre réponse part vers la personne qui organise. Un événement publié a **Ajouter à mon agenda** ; un événement annulé, **Le retirer de mon agenda**.

### Les pièces jointes {#attachments}

Chaque pièce jointe est vérifiée par l’antivirus avant de s’ouvrir ou d’être enregistrée. **Garder dans les papiers** la range dans vos [papiers](papers.md) : vérifiée, puis gardée, et vous dites ce que c’est.

## Écrire {#writing}

**Écrire** est toujours au même endroit. Vous écrivez en Markdown, et les retours à la ligne sont gardés comme vous les tapez, comme dans une messagerie instantanée : une ligne vide commence un paragraphe. **Aperçu** montre le message tel qu’il apparaîtra.

- **Cc, Cci** sont repliés jusqu’à ce que vous en ayez besoin. Quand vous avez plusieurs adresses, **De** choisit celle qui envoie ; une réponse part de l’adresse à laquelle elle répond.
- **Joindre**, ou déposer des fichiers sur la fenêtre. **Un papier**, à côté, joint l’un de vos [papiers](papers.md) ; un papier qui a expiré, ou plus vieux que ce qui est demandé d’habitude, le dit dans la liste.
- **Votre signature** est placée sous le texte quand le brouillon commence, après la ligne habituelle « -- », pour que vous voyiez ce qui part. Votre nom et votre signature se règlent par adresse dans Comptes (**Nom et signature…**).
- **En répondant**, la fenêtre dit « Sous votre texte : le message de Camille du …, cité ». La citation est ajoutée quand le message part.
- **Les brouillons s’enregistrent pendant que vous tapez**, sur cet appareil. Fermer la fenêtre ne perd rien : le brouillon attend dans Brouillons, à gauche.

**Envoyer** (ou ++ctrl+"Entrée"++) attend dix secondes, avec **Annuler**, avant que le message parte. Puis une copie va dans Envoyés. Rien n’est jamais envoyé sans vous.

Le message part en HTML pour qui lit le HTML, et en texte brut, le Markdown lui-même, pour les autres.

### Depuis d’autres applications {#from-other-apps}

- **Sur un téléphone**, Sioul est dans le menu de partage d’Android : partagez une photo, un PDF, un lien ou un texte vers **Sioul**, et un nouveau message s’ouvre avec, depuis votre adresse habituelle. Vos adresses y sont aussi, chacune à part (Android 10 et plus récent) : choisissez-en une, et le message part d’elle. Un appui long sur l’icône de Sioul les montre aussi, **Écrire depuis …**.
- **Les fichiers** sont copiés dans Sioul à leur arrivée. La fenêtre dit « Ajout des 2 fichiers… » jusqu’à ce qu’ils soient là, et **Envoyer** les attend. Les copies s’en vont une fois le message envoyé ou supprimé.
- **Les liens de courrier** (`mailto:`), dans un navigateur ou une autre application, ouvrent un nouveau message dans Sioul, avec son adresse, son objet et son texte, quand Sioul est votre application de courrier. Sur un téléphone, Android demande la première fois quelle application les ouvre. Sous Linux, choisissez Sioul comme programme de courrier parmi les applications par défaut des réglages de votre bureau (KDE, GNOME), ou avec `xdg-mime default com.aurelienpierre.Sioul.desktop x-scheme-handler/mailto`. Un lien cliqué pendant que Sioul est ouvert s’ouvre dedans.
- **Rien n’est envoyé** avant que vous appuyiez sur **Envoyer**. Les autres applications ne voient rien de vos comptes, sinon les adresses du menu de partage.

### Signer et chiffrer {#signing-and-encrypting}

Quand vous avez une clé OpenPGP (créée ou importée dans [Comptes ▸ Chiffrement](accounts.md#encryption)), la fenêtre d’écriture a deux petits interrupteurs : **Signer** et **Chiffrer**. **Chiffrer** fonctionne quand la clé de chaque destinataire est connue ; sinon il dit pour qui la clé manque, et **Chercher leurs clés** interroge le Web Key Directory de leur domaine, puis keys.openpgp.org (seulement quand vous le demandez, puisque cela dit à un serveur à qui vous écrivez).

Un message chiffré est déchiffré quand vous l’ouvrez ; une signature est vérifiée et dite sous l’expéditeur : « Chiffré · Signé par … », ou « La signature ne correspond pas au texte », en couleurs chaudes, jamais en rouge. Vos messages portent votre clé publique (Autocrypt), pour que les personnes qui vous répondent puissent chiffrer.


Un message que votre [clé de sécurité](accounts.md#security-key) signe est signé quand vous appuyez sur **Envoyer**, avant les dix secondes d’**Annuler** : un bandeau en bas de la fenêtre d’écriture demande le code PIN de la clé, et dit combien d’essais il reste quand certains ont été perdus ; puis, quand la clé demande un toucher, il dit **Touchez votre clé de sécurité** pendant qu’elle clignote. Non branchée, il le dit, et reprend dès qu’elle arrive. **Envoyer sans signature** et **Pas maintenant** restent là tout du long, et **Annuler** jette le message signé et rouvre le brouillon. Le code PIN reste en mémoire quinze minutes après son dernier usage, jamais écrit nulle part ; il est oublié quand vous retirez la clé, fermez Sioul, ou choisissez **Oublier le code PIN maintenant** dans Comptes.

Un message chiffré pour votre clé de sécurité n’est jamais ouvert parce qu’il est affiché : la ligne sous l’expéditeur dit « Chiffré pour votre clé de sécurité », avec **Ouvrir avec votre clé de sécurité**. Une fois ouvert, il s’ouvre de nouveau sans la clé, ses pièces jointes aussi, jusqu’à la fermeture de Sioul.
## Les réglages {#settings}

Le ⚙ en haut de la page Courrier :

- **Par conversation** : les messages groupés avec leurs réponses.
- **Relever toutes les** : la fréquence à laquelle les dossiers autres que la boîte de réception sont relevés. La boîte de réception arrive dès que le serveur signale du nouveau.
- **Police**, **Taille**, **Interligne** : la lecture des messages. Les trois mêmes sont dans le ⚙ du Porche, et derrière **Aa** dans les Notes.
- **Listes quittées** : chaque liste quittée depuis un message, quand et comment, dès qu’il y en a une.
- **Filtres** : ce qui est fait au courrier qui arrive, sur son serveur, selon des conditions ([plus haut](#filters)).

### Votre filtre à indésirables {#your-own-spam-filter}

Sous **Votre filtre à indésirables** : ce qu’il fait de chacun de ses avis sur le courrier d’un inconnu (probablement indésirable, peut-être indésirable, probablement pas : le déplacer dans le dossier Indésirables sur le serveur à son arrivée, le signaler, ou rien), à quel point il doit être sûr pour chacun, et sur un ordinateur **Entraîner maintenant**, pour apprendre de tout votre courrier, avec ce que le dernier apprentissage a mesuré. Ce qu’il signale attend sur le Porche, dans **Retenu par votre filtre à indésirables**, et reste dans son dossier ici ; ce qu’il déplace, vous le trouvez aussi dans le dossier Indésirables ([le Porche](porch.md#spam-and-your-own-filter)). Chaque réglage, en entier : [Paramètres](settings.md#your-own-spam-filter).

Les réglages propres à chaque adresse (à quoi elle sert, jusqu’où elle remonte, à quel rythme elle est relevée, sa protection) sont sur sa fiche dans [Comptes](accounts.md#your-accounts).

**Temps réel**, en haut de la page, relève chaque dossier de chaque adresse toutes les minutes, pour un code ou un mot de passe que vous attendez.

## Pendant le calme {#in-quiet-time}

En dehors des heures de travail, les adresses du travail se replient, sans leurs points : « Le courrier du travail se repose jusqu’au retour du travail. Il est là si vous le cherchez. » Pendant le sommeil, toutes les adresses se replient : « Pendant le sommeil, le courrier se repose : rien ne notifie, et le Porche ne montre que ce que vos listes laissent passer maintenant. Le reste est là si vous le cherchez. » Voir [Heures](hours.md).

## Pas encore là {#not-there-yet}

Vider la corbeille ou les indésirables d’un coup, copier un message dans un dossier, l’enregistrer comme fichier, garder les brouillons sur le serveur, envoyer plus tard. C’est prévu.
