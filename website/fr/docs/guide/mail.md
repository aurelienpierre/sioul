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
- **Chercher**, en haut d’un dossier : par expéditeur, destinataire et objet.
- **Par conversation**, dans le ⚙ de la page Courrier : un message et ses réponses ensemble, sous le plus récent ; vos propres réponses viennent des Envoyés.

Un clic droit sur un dossier propose **En garder une copie ici** ou **Ne plus en garder de copie** (le serveur garde tous les messages dans les deux cas), **Nouveau dossier…**, et **Supprimer ce dossier vide**, pour un dossier vide à vous.

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

Un clic droit sur un message (ou son ⋮, ou la touche Menu) donne le reste : marquer comme lu ou non lu, suivre, **Déplacer vers…**, **Voir la source**, bloquer l’expéditeur, **Son courrier** (comme le disent ses catégories, sûr, neutre, restreint, bloqué), **Garder comme contrat…**.

Ouvrir un message le marque comme lu, comme le fait tout logiciel de courrier.

### Plusieurs à la fois {#several-at-once}

- ++ctrl++ + clic ajoute un message à la sélection ou l’en retire ; ++"Maj"++ + clic prend tout depuis le dernier cliqué ; ++ctrl+a++ prend tout ; ++"Échap"++, rien.
- Une barre les marque alors comme lus, les archive, les supprime ou les déplace, sous un seul **Annuler**.
- Les messages peuvent être glissés sur n’importe quel dossier de n’importe quelle adresse. Vers une autre adresse, un message n’est retiré de la première qu’une fois que le second serveur l’a reçu.

### Se désabonner {#unsubscribing}

Une lettre d’information ou le message d’une liste qui dit comment la quitter a **Se désabonner** au-dessus. Un clic, et dix secondes plus tard Sioul fait ce que la liste demande :

- il **prévient le serveur de la liste**, quand elle le propose et que sa signature prouve que la demande vient bien d’elle : le « en un clic » que proposent la plupart des lettres d’information ;
- sinon il **envoie à la liste le message qu’elle demande**, depuis l’adresse où la lettre est arrivée ; le message va dans Envoyés comme les autres ;
- sinon, quand seule une page web peut le faire, il **ouvre cette page** dans votre navigateur, tout de suite : vous terminez là-bas.

**Annuler** reste dans la ligne d’état pendant ces dix secondes. Ensuite la ligne dit « Désabonnement de Lettres & Pixels fait. », ou pourquoi pas. Le message lui-même reste où il est.

Le bouton reste en retrait, estompé, sur un message falsifié, mis de côté comme indésirable, hostile, dans les indésirables, ou d’un expéditeur que Sioul ne peut pas vérifier : y répondre dirait à son expéditeur que votre adresse est lue, ou toucherait quelqu’un d’autre. Son info-bulle, ou un toucher sur un téléphone, dit pourquoi.

Une liste quittée montre **Abonnement arrêté**, et depuis quand. Si ses messages continuent d’arriver, bloquez l’expéditeur : ⋮ ▸ **Son courrier** ▸ **Bloqué**. Les listes quittées sont dans le ⚙ de la page Courrier, sous **Listes quittées**, sur cet appareil.

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

## Les réglages {#settings}

Le ⚙ en haut de la page Courrier :

- **Par conversation** : les messages groupés avec leurs réponses.
- **Relever toutes les** : la fréquence à laquelle les dossiers autres que la boîte de réception sont relevés. La boîte de réception arrive dès que le serveur signale du nouveau.
- **Police**, **Taille**, **Interligne** : la lecture des messages. Les trois mêmes sont dans le ⚙ du Porche, et derrière **Aa** dans les Notes.
- **Listes quittées** : chaque liste quittée depuis un message, quand et comment, dès qu’il y en a une.

Les réglages propres à chaque adresse (à quoi elle sert, jusqu’où elle remonte, à quel rythme elle est relevée, sa protection) sont sur sa fiche dans [Comptes](accounts.md#your-accounts).

**Temps réel**, en haut de la page, relève chaque dossier de chaque adresse toutes les minutes, pour un code ou un mot de passe que vous attendez.

## Pendant le calme {#in-quiet-time}

En dehors des heures de travail, les adresses du travail se replient, sans leurs points : « Le courrier du travail se repose jusqu’au retour du travail. Il est là si vous le cherchez. » Pendant le sommeil, toutes les adresses se replient : « Pendant le sommeil, le courrier se repose : rien ne notifie, et le Porche ne montre que ce que vos listes laissent passer maintenant. Le reste est là si vous le cherchez. » Voir [Heures](hours.md).

## Pas encore là {#not-there-yet}

Vider la corbeille ou les indésirables d’un coup, copier un message dans un dossier, l’enregistrer comme fichier, garder les brouillons sur le serveur, envoyer plus tard. C’est prévu.
