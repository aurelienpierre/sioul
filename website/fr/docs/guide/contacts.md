---
description: Les contacts dans Sioul – les noms d’abord, les détails repliés ; les doublons trouvés et fusionnés quand vous le dites, chaque fusion annulable pendant trente jours ; la carte de tout le monde, placée seulement si vous le permettez ; qui vous joint, et quand ; comparés aux autres carnets d’adresses.
---

# Les contacts et la carte {#contacts-and-the-map}

## En bref {#in-short}

Les contacts réunissent les personnes à qui vous avez affaire : leur nom d’abord, leurs détails repliés jusqu’à ce que vous les ouvriez. Sioul trouve la même personne écrite deux fois et fusionne les fiches quand vous le dites, avec trente jours pour annuler chaque fusion. Il montre sur une seule carte toutes les personnes qui ont une adresse postale, une fois que vous l’avez permis. Les carnets d’adresses se créent, se renomment, se fusionnent et se suppriment depuis le ⚙ de la page ; plusieurs contacts à la fois se déplacent, reçoivent un groupe ou s’importent, et chaque changement s’annule d’un seul coup. Chaque personne est dans un cercle (Sûrs, Vos contacts, Restreints, Tous les autres pour les inconnus, ou Bloqués ; quelques-unes passent aussi toujours), et ce cercle décide quand ses courriels, ses appels et ses messages vous joignent ; un courriel falsifié ne peut pas prendre sa place. Vos contacts restent sur votre propre serveur, chez Google ou sur cet appareil, et Sioul ne les joint que par une connexion chiffrée. En tête de la page, vos proches : les personnes pour qui vous faites des démarches, chacune avec ses papiers.

<figure markdown="span">
  [![La page Contacts : vos proches en une rangée avec Ajouter une personne, puis un champ de recherche, des boutons pour un nouveau contact, la carte, les doublons, une sélection et un import, et les filtres au-dessus d’une liste de noms, chacun avec une adresse ou un numéro dessous ; un contact ouvert à droite, avec sa fonction et son organisation, une adresse électronique avec Écrire, un numéro de téléphone avec Appeler, « Vos contacts : dans votre carnet d’adresses, sur aucune liste. », C’est une personne de vos proches… et Comment Iris Calvet vous joint, la fiche client avec Facturer ce client, puis ce qui lui est lié : deux tâches et deux événements.](../assets/screens/fr/contacts.png){ loading=lazy }](../assets/screens/fr/contacts.png "Ouvrir l’image en grand")
  <figcaption>Un contact : comment le joindre, et ce qui le concerne.</figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

- Vos contacts restent là où vous les gardez : sur votre propre serveur de contacts (un Nextcloud, par exemple), chez Google, ou sur cet appareil. Sioul n’a pas de serveur à lui, et n’envoie vos contacts nulle part ailleurs.
- Sioul ne joint votre serveur que par une connexion chiffrée, et refuse celle qui ne l’est pas. Votre mot de passe reste dans le trousseau de votre système, jamais dans un fichier.
- La carte n’envoie rien tant que vous ne l’avez pas permis. Ensuite, elle n’envoie que des adresses postales, une à la fois, au géocodeur d’OpenStreetMap, et une adresse qu’il ne sait pas lire à un ou deux autres services d’adresses ([La carte](#the-map)) : jamais un nom, un numéro ni une adresse électronique.
- Personne ne peut emprunter une place sur vos listes. Un courriel qui montre l’adresse d’un contact sûr mais échoue aux vérifications de cette adresse compte comme celui d’un inconnu, et un courriel franchement falsifié est mis de côté. Quelqu’un que vous avez bloqué reste bloqué, quoi que prouve son courriel.
- Qui est sur quelle liste, et les appels que vos téléphones ont filtrés, voyagent entre vos appareils scellés par votre phrase de passe : le serveur du dossier partagé ne les lit jamais.
- Vos proches restent sur vos propres appareils, scellés quand ils voyagent : jamais écrits dans un contact, jamais envoyés à un serveur, chacun fermé aux agents d’IA tant que vous ne l’ouvrez pas ([Vos proches](#household)).

## La liste {#the-list}

Les noms viennent d’abord, avec une ligne sous chacun, et un champ de recherche est en haut : quelques lettres d’un nom, d’une organisation, d’une adresse électronique ou d’un numéro de téléphone suffisent à trouver un contact. La recherche ne regarde pas les adresses postales.

Au-dessus de la liste, des filtres la resserrent, chacun gardé sur cet appareil jusqu’à ce que vous le changiez : les carnets d’adresses montrés (tous, un ou plusieurs, dès que vous en avez plus d’un), un groupe ou **Dans aucun groupe**, et si une fiche a ou n’a pas de numéro de téléphone, d’adresse électronique, d’adresse postale ou de civilité (M., Mme…). Ils s’ajoutent à la recherche, et la ligne dit combien se montrent : « 132 sur 840 ». **Tout montrer** retire les filtres ; la recherche reste jusqu’à ce que vous l’effaciez, et n’est jamais gardée sur le disque. Sur un téléphone, les filtres se replient derrière un seul bouton **Filtres**, qui dit combien sont mis (« Filtres (2) »), avec **Tout montrer** et le compte à côté : le premier contact vient juste après une ligne.

Une civilité que la fiche garde dans son propre champ se montre en grisé avant le nom.

## Vos proches {#household}

En tête de la page, **Proches** est une rangée de noms : vous d’abord, puis les personnes pour qui vous faites des démarches, un enfant, un parent que vous aidez, un ou une partenaire, de la famille, une personne que vous aidez, et après un petit espace l’autre parent d’un enfant. Un nom ouvre la page de cette personne, à la place d’une fiche ; sur un petit écran, elle prend la page. Un clic droit, ou un appui long sur un écran tactile, propose **Ouvrir**, **Renommer** et **Ce n’est pas une personne**. **Ajouter une personne**, au bout de la rangée, en ajoute une.

Quand Sioul ajoute des proches d’après ce que nommaient déjà vos agendas et vos papiers (l’agenda d’un enfant, le titulaire d’un papier), le Porche dit une fois qui, et d’après quoi, pendant vos heures de démarches ; il n’y a rien à répondre.

Une personne n’est pas un contact. Un contact est une fiche pour joindre quelqu’un ; une personne est celle pour qui sont vos papiers. Un bébé est une personne sans fiche ; quelqu’un qui a une fiche personnelle et une fiche professionnelle est une seule personne avec deux fiches.

**La page d’une personne** montre son nom et une ligne, par exemple « Enfant · 12 ans · vit avec vous », puis :

- **Papiers** : les papiers qui sont les siens, chacun avec où il en est, comme sur la page des [Papiers](papers.md#papers), et **Ajouter un papier pour Léa** ;
- **Agendas** : ses agendas, celui d’un enfant avec **Qui fait quoi** ; pour l’autre parent d’un enfant, les agendas des enfants qu’il ou elle partage ;
- **Lettres** : les lettres qui lui sont adressées ;
- **Argent** : les budgets pour cette personne et les sites que vous ouvrez pour elle ;
- **Contacts** : les fiches qui sont elle, chacune ouvrant sa fiche, avec **Délier**, et **Lier un contact…** ;
- en bas, discrets : **Ce que Sioul garde sur Léa…**, un PDF de ses papiers, agendas et lettres et un dossier des fichiers de ses papiers, enregistrés où vous voulez, pour les lui remettre (ses budgets et les sites que vous ouvrez pour cette personne n’y sont pas) ; **Fin…**, pour toute personne sauf un enfant : le jour où cela a pris fin, et pour un ou une partenaire, ses propres papiers enregistrés dans un dossier pour les lui rendre, puis retirés de vos papiers si vous le dites ; **Même personne que…**, pour ne faire qu’une de deux personnes (« Léa » et « Léa M. ») ; **Ce n’est pas une personne**, pour un nom lu sur un papier qui n’est personne (« Banque »), plus jamais refait ;
- **Ouvert aux agents d’IA**, éteint tant que vous ne l’allumez pas. Votre propre page n’a pas cet interrupteur : vous êtes ouvert comme avant.

Sur la page d’un enfant, une fois chacune et pendant vos heures de démarches, une ligne dit ce qui lui revient : à 16 ans, une carte Vitale et un compte de remboursement possibles à son nom ; à 18 ans, ses papiers qui lui reviennent, avec **Ce que Sioul garde sur Léa…** à côté. Un papier dont le nom pour quelqu’un ne nomme personne ici propose **Faire une personne de « … »** sur sa fiche.

**Modifier** change son nom, la façon dont les papiers l’écrivent (« Léa Martin »), son type, sa date de naissance, si elle vit avec vous, et pour l’autre parent d’un enfant, de quel enfant. Un nouveau nom s’écrit aussi dans les papiers qui portaient l’ancien, pour qu’un appareil resté sur un Sioul plus ancien le montre aussi.

**Sur la fiche d’un contact**, **C’est une personne de vos proches…** lie la fiche à l’une d’elles, ou en fait une nouvelle personne. Une fois liée, le bouton ouvre plutôt sa page.

**D’où elles viennent.** La première fois, Sioul lit les noms que vos agendas et vos papiers donnaient déjà : l’agenda d’un enfant donne cet enfant, et son autre parent ; un papier « pour Odile » donne Odile. Un papier sans nom, ou à votre nom, est le vôtre. Ces personnes comptent dès lors ; une erreur se corrige sur sa page. Rien d’autre n’est écrit.

## Un contact {#a-contact}

Un contact s’ouvre à droite ; sur un téléphone, il prend la page :

- sous son nom, sa fonction et son organisation, puis ses **catégories**, de petites étiquettes : un clic sur l’une d’elles montre la liste de cette catégorie ;
- ses **adresses électroniques**, chacune avec **Écrire**, et ses **numéros de téléphone**, chacun avec **Appeler**, qui donne le numéro à votre téléphone, ou sur un ordinateur au programme qui répond aux liens de téléphone ;
- sur la fiche d’un bureau, à côté de ses numéros, ses **Horaires d’ouverture** : ce qui est vrai maintenant, ses horaires jour par jour, et **Les chercher…**, **Modifier**, **Rappeler…** ([Horaires d’ouverture](#opening-hours), plus bas) ; sur une autre fiche, sous **Plus** ;
- **Comment cette personne vous joint** : qui elle est pour vous, en une phrase (« Vos contacts : dans votre carnet d’adresses, sur aucune liste. »), et sa fiche ([plus bas](#how-they-reach-you)) ;
- une petite carte avec une épingle à son adresse, une fois celle-ci placée ([La carte](#the-map)) ;
- replié sous **Plus** : ses adresses postales, son anniversaire, ses notes et ses sites web, puis le carnet d’adresses où se trouve la fiche. **Plus** ne se montre que sur une fiche qui a l’un des quatre premiers. Une fiche d’un carnet en lecture seule ne propose pas **Modifier**, et son carnet d’adresses ne s’y change pas ;
- à côté de chaque adresse postale, **Itinéraire**, qui ouvre le chemin comme le fait le lieu d’un événement ([Agenda](agenda.md#status-map-directions)) : dans votre application de cartes quand ce système en a une pour les liens `geo:`, avec le point où la carte l’a placée quand elle l’a, sinon sur la page d’OpenStreetMap, ou celle d’Apple sur un Mac ;
- **Lié à cela** : les tâches qui la nomment, les événements où elle est invitée, les notes qui la mentionnent, et ce que vous lui avez lié à la main. Un courriel s’y montre quand vous le lui avez lié, de même que chaque courriel qu’elle vous a envoyé et dont Sioul a prouvé qu’il vient d’elle ; la [recherche des courriels](mail.md#searching) trouve le reste, dont ce que vous lui avez envoyé.
- **Facturer ce client**, qui ouvre un brouillon de facture pour ce contact dans Budgets ▸ Factures, et **Fiche client…** : ce qu’impriment ses factures (une entreprise, un particulier ou un organisme public, la dénomination, les adresses, un SIREN ou un numéro de TVA, la langue, la devise et le délai de ses factures), gardé parmi les propres listes de Sioul à côté de vos projets, jamais dans la fiche sur votre serveur de contacts. Une fois faite, la fiche se montre en une ligne au-dessus d’eux, « Fiche client : Une entreprise · Belgique · BE0123456789 · factures en français » ; sans fiche encore, le bouton dit **Faire sa fiche client…**, et part de la fiche de contact ([Vos clients](time.md#your-clients)).

**Pour le changer**, **Modifier**, puis **Enregistrer** : il est modifié sur place. Sous le nom, **Civilité, prénoms, nom de famille** ouvre les parties du nom : sa civilité (M., Mme…), ses prénoms, son nom de famille et son suffixe, selon lesquels les autres programmes trient et s’adressent. Les parties que vous tapez sont gardées telles que vous les avez tapées ; une fiche qui n’en donne aucune les reçoit de son nom, une civilité au début allant à la civilité. Sur Android, **Retour** quitte le formulaire sans enregistrer. **Pour en ajouter un**, **Nouveau contact** au-dessus de la liste (un **+** seul à côté d’une fiche ouverte, et sur un téléphone), ou **Nouveau ▾ ▸ Un contact**. Il va dans le premier carnet d’adresses modifiable, dans l’ordre où Contacts ⚙ les montre ; pour le garder ailleurs, déplacez-le une fois enregistré. Depuis un courriel, **Ajouter aux contacts** fait de l’expéditeur un contact en un clic.

**Pour en supprimer un**, un clic droit sur son nom dans la liste (un appui long sur un écran tactile), puis **Supprimer** : la suppression attend dix secondes, avec **Annuler**. Une fiche d’un carnet d’adresses en lecture seule n’est jamais modifiée ni supprimée : le menu de sa ligne le dit.

**Pour en déplacer un** vers un autre carnet d’adresses, y compris d’un compte à l’autre, choisissez ce carnet sous **Plus ▸ Carnet d’adresses**, sur sa fiche, qui montre le carnet où elle est. Quand l’autre endroit ne garderait pas quelque chose (Google garde moins qu’un serveur ouvert), ou quand le déplacement change qui peut vous joindre, Sioul dit quoi, et demande avant de le déplacer. Pour en déplacer plusieurs, ou une fiche qui ne montre pas **Plus**, sélectionnez-les ([Plusieurs contacts à la fois](#many-at-once)).

Quand vous écrivez un courriel, les adresses se complètent depuis vos contacts.

## Horaires d’ouverture {#opening-hours}

Les horaires d’un bureau s’écrivent une fois, sur sa fiche, à côté du numéro que vous appelez, et chaque tâche qui concerne ce bureau les suit.

- **Où** : sur la fiche d’une organisation (une mairie, un centre des impôts, un cabinet, une boutique), ou de toute fiche qui a des horaires, la partie **Horaires d’ouverture** vient juste sous ses numéros ; sur une autre fiche, elle est sous **Plus**, avec **Ajouter des horaires d’ouverture** et un lien, **C’est un bureau, une boutique ou un cabinet**, qui marque la fiche comme celle d’une organisation, comme les autres programmes le lisent (son nom écrit comme son organisation, ou `KIND:org` en vCard 4.0) ; tout le reste de la fiche reste comme il était. Marquée par erreur, la fiche est remise comme avant avec **Annuler** dans la ligne d’état, ou plus tard, pendant trente jours, sous **Fait récemment** dans Contacts ▸ **Doublons**.
- **Ce qu’elle dit** : en une ligne, ce qui est vrai maintenant (« Ouvert jusqu’à 12:00 », « Fermé : ouvre lundi à 9:00 », « Fermé pour le jour férié : ouvre demain à 9:00 ») ; puis ses horaires jour par jour (« Du lundi au vendredi : 9:00–12:00, 14:00–17:00 »), et « Fermé les jours fériés. », les jours fériés que Sioul connaît (Réglages ▸ Heures), ou, quand Sioul n’en connaît encore aucun, où ajouter leur agenda ; sous **Au téléphone**, quand il répond au téléphone à d’autres heures, celles-là ; sa ligne libre, pour ce que Sioul ne sait pas compter (« Fermé en août, sur rendez-vous le jeudi »), montrée et jamais comptée ; et d’où viennent les horaires : « Saisis par vous le 20 novembre 2026 », ou « De l’annuaire de l’administration (service-public.gouv.fr), vérifiés le… », ou « D’OpenStreetMap (© les contributeurs d’OpenStreetMap), vérifiés le… ». Des horaires vérifiés il y a plus d’un an le disent, doucement, et proposent **Les chercher à nouveau…**.
- **Modifier** (**Ajouter des horaires d’ouverture** sur une fiche qui n’en a pas) ouvre la semaine à remplir, une ligne par jour, vide quand c’est fermé, ou la même chose saisie en bref au-dessus : les jours, puis les heures, « lu-ve 9-12, 14-17 ; sa 9-12 » (les jours en deux lettres, en français ou en anglais, « mo-fr 9-12, 14-17 », ou en toutes lettres ; les heures en « 9 », « 9:30 » ou « 9h30 », chaque plage après la précédente) ; dessous, ce que Sioul y lit. Un jour que Sioul ne sait pas compter garde ce que vous avez saisi dans son champ, le dit dessous, et va dans la ligne libre quand vous enregistrez. « Di » est dimanche parmi des jours français et mardi parmi des jours allemands : écrit à côté de jours anglais, Sioul ne devine pas, et le dit. **Ouvert les jours fériés**, décoché, garde le bureau fermé les jours fériés que Sioul connaît ; « 24/7 » le coche. **Répond au téléphone à d’autres heures** ajoute celles-là, saisies de la même façon. Ce que Sioul ne sait pas compter va dans la ligne libre quand vous enregistrez ; rien de ce qui est saisi n’est perdu. Enregistrée sans que ses horaires changent, la fiche n’est pas écrite.
- **Les chercher…** n’est proposé que sur une fiche qui dit être celle d’une organisation, ou qui a déjà des horaires ou une ligne libre (les saisir, c’est dire que c’est celle d’un bureau). Il ouvre ce qui serait envoyé, le nom du bureau et où il est, que vous pouvez corriger, et à qui : l’annuaire de l’administration (service-public.gouv.fr) pour un bureau dont l’adresse est en France (un code postal suivi de « France », ou votre région la France), et le service de recherche de la carte, celui d’OpenStreetMap sauf si vous en avez choisi un autre dans les réglages des Contacts. Avec les mots partent le nom et la version de Sioul, et vers la carte la langue de la fenêtre. Rien ne part avant que vous appuyiez sur **Chercher** ; alors une question part vers chacun, les deux en même temps. La réponse est une courte liste, **Est-ce celui-ci ?**, chaque bureau avec son adresse et ses horaires, sa source nommée. **Celui-ci** écrit ses horaires sur la fiche, avec leur source et le jour, et « Ouvert les jours fériés » quand la source le dit ; ce qu’il dit que Sioul ne sait pas compter s’ajoute à la ligne libre. Un numéro ou une adresse qui manque à la fiche est proposé ensuite (**Ajouter son numéro … à la fiche**), jamais écrit tout seul. Rien de trouvé ne change rien, et une recherche n’ôte jamais d’horaires à une fiche. La fiche d’une personne ne propose aucune recherche : le nom d’une personne n’est jamais envoyé nulle part. Google n’est pas une source : ses conditions ne laissent pas garder sur une fiche les horaires qu’il donne ; des horaires lus sur sa page peuvent être saisis en bref.
- **Rappeler…** crée tout de suite la tâche « Appeler ‹nom› » : un appel, ce bureau attaché, proposée quand il répond. Un appui, aucun formulaire ; un second appui aussitôt ne crée rien de plus. Sur une fiche qui n’est pas celle d’un bureau et n’a pas d’horaires, c’est un simple appel, proposé à toute heure.
- **Les tâches qui les suivent** : une tâche qui demande un bureau ouvert suit les horaires saisis sur elle, sinon ceux de la fiche du premier bureau qu’elle concerne qui en a (ceux du téléphone pour un appel), sinon les horaires habituels des bureaux (⚙ des tâches). Modifier la fiche modifie chaque tâche de ce bureau. Un appel ou une sortie à qui l’on donne un bureau dont la fiche a des horaires est coché **Demande un bureau ouvert** tout seul, comme les appels et sorties déjà liés à un bureau quand vous donnez pour la première fois des horaires à sa fiche sur cet appareil (saisis ou cherchés ; jamais quand la fiche arrive d’un autre appareil, où la coche a déjà été faite ou non) ; décochez-le et il le reste. Une tâche décochée dit qu’elle ne suit pas les horaires de son bureau, avec **Les suivre** ([Tâches](tasks.md#offices-have-hours)).
- **Un bureau à l’étranger** : ses horaires se lisent sur l’horloge de votre appareil, et les jours fériés sont ceux de vos agendas de jours fériés ; pour un bureau d’un autre pays ou d’un autre fuseau horaire, saisissez ses horaires tels qu’ils tombent sur votre horloge, et ses jours de fermeture dans la ligne libre.
- **Où ils sont gardés** : sur la fiche, dans des lignes propres à Sioul, que les autres programmes et un Sioul plus ancien gardent telles qu’elles sont. La fiche d’un carnet que vous ne pouvez que lire montre ses horaires, et dit qu’ils ne peuvent pas y être écrits. Les carnets de Google pourraient ne pas les garder : y déplacer une telle fiche le dit d’abord.

## Comment on vous joint {#how-they-reach-you}

Chaque personne est sur une liste, et la même liste vaut pour ses courriels, ses appels et ses messages dans les autres applications :

- **Sûrs** : la famille, les amis, les collègues que vous choisissez. Vous seul y mettez quelqu’un.
- **Vos contacts** (autrefois « Neutres ») : toute personne de vos carnets d’adresses, tant que vous n’en décidez pas autrement.
- **Restreints** : les personnes dont vous préférez n’avoir des nouvelles qu’aux moments que vous choisissez.
- **Bloqués** : jamais, sur aucun canal. Leurs courriels sont mis de côté pour de bon, et leurs appels sont refusés.
- **Inconnus** (« Tous les autres » dans Ce qui vous joint) : toutes les personnes qui ne sont dans aucun de vos carnets d’adresses ni sur aucune liste. Leur premier courriel attend sur [le Porche](porch.md#the-lanes), dans le filtre des nouveaux expéditeurs, jusqu’à ce que vous les laissiez entrer.

Votre choix pour la personne passe d’abord ; puis ses catégories, quand une liste en nomme une (« Amis » sont sûrs) ; puis son carnet d’adresses. **Comment … vous joint** ouvre sa fiche : sa liste et pourquoi, **Sa liste** pour la choisir (**Comme le disent ses groupes**, ou l’une des quatre listes), **Passe toujours** pour les quelques personnes qui doivent toujours passer, ce qui vous joint de sa part à chaque moment de votre journée, **Leurs propres heures** pour ses appels et ses messages, ses appels du mois, sonnés ou refusés, et ses messages de la semaine quand un de vos téléphones les partage ([La fiche d’une personne](notifications.md#a-persons-sheet)). Quand les courriels et les messages de chaque liste vous joignent, c’est à vous de le changer, dans Paramètres ▸ [Ce qui vous joint](notifications.md#by-person) ; les appels se règlent pour chaque téléphone, dans Paramètres ▸ Appareils ([À chaque téléphone ses appels](calls.md#calls-per-phone)). Par défaut, par exemple, les courriels d’un de vos contacts viennent pendant vos heures de travail et de démarches, et l’appel d’un inconnu part sur votre messagerie dès qu’un de vos téléphones filtre les appels.

## Les catégories {#categories}

Les catégories sont les groupes que montre Nextcloud Contacts : « Famille », « Amis », « Voisins » ; une fiche peut en avoir plusieurs. Dans le formulaire, sous **Catégories**, × en retire une, et le champ qui les suit en ajoute une, choisie parmi celles qu’ont déjà vos fiches ou tapée. « amis » et « Amis » sont une seule catégorie, écrite comme vos fiches l’ont écrite d’abord. Les filtres, les changements de plusieurs contacts et Ce qui vous joint les appellent des groupes : c’est la même chose.

Elles sont enregistrées dans la fiche elle-même : Nextcloud, votre téléphone et les autres programmes les voient, et une fiche enregistrée dans Sioul garde celles qu’elle avait. Une liste peut nommer une catégorie (Paramètres ▸ [Ce qui vous joint](notifications.md#who-is-on-which-list), sous **Groupes**) : ses personnes vous joignent alors comme le dit cette liste, par courriel, par téléphone et par les messages des autres applications, sauf si vous avez choisi autre chose pour elles sur leur fiche. Rien ne va sur une liste tout seul, famille et amis compris.

## Les carnets d’adresses {#address-books}

Un carnet d’adresses est l’endroit où des fiches sont gardées, et avec qui elles sont partagées : un sur votre Nextcloud, celui de Google, un sur cet appareil seulement. Contacts ⚙ ▸ **Carnets d’adresses** les montre chacun avec l’endroit où il est gardé, combien de contacts il contient, à qui il est (« À vous », « Partagé avec vous par Sam ») et s’il est modifiable. Une ligne s’ouvre sur ce qu’on peut en faire ; ce que vous choisissez ou tapez dans une ligne reste quand une synchronisation redessine la liste :

- **Nouveau carnet d’adresses…** : sur un compte dont le serveur en crée, ou sur cet appareil seulement. Il est créé sur le serveur à la prochaine synchronisation.
- **Renommer** : ici, et sur le serveur à la prochaine synchronisation, avec **Annuler**.
- **Fusionner avec…** : d’abord la question, qui dit combien de contacts vont dans quel carnet, ce que ce carnet ne garderait pas, et qui le voit (« “Professionnel” est partagé avec bureau »). Puis tous les contacts vont dans l’autre carnet, et le carnet vidé est supprimé, ici et sur son serveur une fois qu’une réponse entière de son serveur le montre vide. Un carnet auquel quelqu’un a ajouté des contacts sur son serveur entre-temps est gardé, et revient ici avec eux, en disant pourquoi. **Annuler** les ramène tous, le carnet recréé quand il était sur cet appareil seulement.
- **Exporter…**, sur un ordinateur : ses contacts en un fichier vCard, là où vous le choisissez : la copie à garder avant un grand changement, ou pour un autre programme. **Avec eux… ▸ Exporter…** fait de même pour une sélection. Un téléphone ne le propose pas encore.
- **Supprimer…** : un carnet vide part tout de suite. Un carnet qui contient des contacts demande où ils vont : déplacés vers un autre carnet (avec la même question qu’une fusion), ou supprimés avec lui, après une copie gardée sur cet appareil. Le compte est celui de cet appareil : sur son serveur, le carnet part pour tous ceux avec qui il est partagé, et un carnet qui contient des contacts ajoutés là-bas depuis la dernière synchronisation est gardé et revient ici. **Annuler** le fait revenir tant que son serveur ne l’a pas supprimé ; ensuite, la copie est là pour être importée.
- **Partager**, sur un Nextcloud : un utilisateur ou un groupe de ce serveur, pour lire ou pour modifier les contacts ; la ligne dit qui l’a, et **Arrêter** le reprend.
- **Ne plus montrer**, pour un carnet que quelqu’un d’autre partage avec vous : il quitte cet appareil, et reste le sien.
- **Ses groupes décident qui peut me joindre** : activé pour un carnet que vous seul pouvez modifier, et pour un carnet dont le serveur ne dit ni à qui il est ni avec qui il est partagé (sa ligne le dit : « Ce serveur ne dit pas qui peut modifier ce carnet ») ; désactivé, jusqu’à ce que vous l’activiez, pour un carnet que quelqu’un d’autre peut modifier (un que vous partagez avec d’autres pour le modifier, ou celui de quelqu’un d’autre), parce que quelqu’un qui peut le modifier pourrait sinon y ajouter une fiche dans un groupe qu’une liste laisse passer. Désactivé, ses contacts restent les vôtres, leurs groupes sont montrés et filtrés comme avant, et la place d’une fiche elle-même sur une liste compte toujours ; seuls ses groupes sont tenus à l’écart des listes. Quand ceux qui peuvent modifier un carnet changent (partagé pour modification ici ou ailleurs, une personne ajoutée ou retirée), une notification le dit une fois (une seule, qui nomme chaque carnet, quand plusieurs cessent ensemble), et sa ligne aussi, avec comment réactiver ses groupes. **Ajouter vos expéditeurs sûrs**, sous la liste Passent toujours ([Passent toujours](notifications.md#always-through)), lit les groupes de la même façon. Un groupe tapé sur une fiche d’un tel carnet dit, sous les groupes, qu’il ne décide pas là qui peut vous joindre, avec le chemin vers l’interrupteur.

Google garde un seul carnet d’adresses : Sioul le dit sur sa ligne et ne propose rien que Google refuserait. Les contacts y entrent et en sortent comme dans tout autre.

## Plusieurs contacts à la fois {#many-at-once}

**Sélectionner**, au-dessus de la liste, donne une case à chaque ligne ; **Sélectionner les 132 montrés** prend tout ce que montrent les filtres. Un autre filtre ou une autre recherche ne laisse cochées que les fiches encore montrées, et **Terminé** range les cases. Sur un téléphone, la barre tient en une ligne : le compte est lui-même une case, qui prend tout ce que montrent les filtres, ou rien une fois tous cochés. Puis **Avec eux…** :

- **Déplacer vers un carnet d’adresses…** : chaque fiche garde son identifiant, et rien de ce qui la désigne ne change (une tâche, une personne, une liste qui la nomme) ; deux fiches d’un même identifiant n’arrivent jamais dans un même carnet. La question dit qui voit ce carnet. Là où son serveur a son propre déplacement, la fiche y est déplacée, un changement pas encore envoyé partant d’abord ; sinon elle est copiée, et l’ancienne copie n’est supprimée qu’une fois la nouvelle arrivée. Une fiche que quelqu’un a changée sur le serveur avant que votre déplacement l’atteigne est gardée dans les deux carnets, telle que vous l’avez laissée et telle qu’il l’a changée, et c’est dit.
- **Ajouter à un groupe…** et **Retirer d’un groupe…** : un groupe déjà utilisé, ou un nouveau tapé.
- **Mettre la civilité dans son champ…** : une civilité au début d’un nom (M., Mr, Mme, Mlle, Dr, Me, Pr, Mrs, Ms, Miss, Monsieur, Madame…) va dans le champ de civilité de la fiche, et le nom montré la perd. Chaque fiche montre son nom avant et après, avec sa propre case : cochée quand le mot ne peut être qu’une civilité (suivi d’un point, ou Mme, Mlle, Mrs, Ms, Miss suivis d’un nom), décochée quand il peut faire partie d’un nom ou d’une entreprise (« Me », « Pr », « M » sans point, « Dr » sur la fiche d’une entreprise) ; vous cochez ce que vous voulez. Un nom de deux personnes (« M. et Mme Dupont », « Mr and Mrs Smith ») n’est jamais changé, et c’est dit. Le reste de la ligne du nom (ses autres noms, ses paramètres) reste tel quel. **Les ajouter aussi à un groupe** leur donne un groupe dans le même pas.
- **Exporter…**, sur un ordinateur : les fiches sélectionnées en un fichier vCard, là où vous le choisissez ([Les carnets d’adresses](#address-books)). Aucune fiche n’est changée.
- **Supprimer…**

Chacune dit combien de fiches elle changera avant de commencer, et pourquoi certaines ne le peuvent pas (« son carnet d’adresses est en lecture seule »). Elle avance avec sa progression et un bouton **Arrêter**, et finit en nommant ce qu’elle n’a pas pu faire, fiche par fiche. Une fiche changée depuis qu’elle a été comptée reste telle quelle, et c’est dit. **Annuler**, dans la ligne d’état, reprend tout le changement pendant dix secondes ; ensuite, Contacts ⚙ ▸ Changements de nombreux contacts montre ceux des trente derniers jours, chacun avec **Annuler…**, demandé d’abord, ou la raison pour laquelle il ne peut plus l’être. Annuler remet chaque fiche touchée tant qu’elle est telle que le changement l’a laissée ; une fiche changée, déplacée ou supprimée depuis reste telle quelle, et est nommée.

## Importer des contacts {#importing}

**Importer…**, au-dessus de la liste (une icône : son nom se montre quand le pointeur s’y arrête), lit un fichier vCard (`.vcf`, comme l’écrivent Google, un téléphone ou un Nextcloud) ou un fichier CSV (`.csv`, comme l’exporte un tableur ou un carnet d’adresses). Sur un ordinateur, un fichier `.vcf` ouvert avec Sioul arrive dans la même fenêtre (un fichier `.csv` par « Ouvrir avec »). Sur Android, **Importer…** prend le fichier partout où va le sélecteur de fichiers d’Android (Sioul lit une copie qu’Android lui donne, retirée une fois lue) ; ouvrir le fichier avec Sioul depuis l’endroit où il est (Fichiers, un message) marche aussi. Le fichier est lu sur cet appareil ; rien n’est envoyé nulle part. Un fichier de plus de 32 Mo n’est pas lu. La fenêtre dit comment elle l’a lu : son encodage, et chaque limite atteinte (20 000 fiches ou lignes, 200 colonnes, une fiche de plus de 2 Mo, une cellule de plus de 64 Ko), un guillemet jamais fermé, une ligne avec plus de cellules que de colonnes, chacun en une phrase.

- **Pour un fichier CSV, comment ses colonnes sont séparées** (points-virgules, virgules ou tabulations : trouvé par sa première ligne, et à vous de le changer), puis **quelle colonne est quoi** : chaque colonne se voit proposer un champ (civilité, prénoms, autres prénoms, nom de famille, le nom entier, suffixe, organisation, fonction, portable, téléphone, adresse électronique, rue, code postal, ville, région, pays, l’adresse postale entière, anniversaire, note, groupes, site web) ou **Laisser de côté**, deviné d’après son titre en français ou en anglais et montré avec ses premières valeurs. Les exports de Google et d’Outlook sont reconnus. Les colonnes peuvent être gardées sous un nom, gardées quand vous appuyez sur **Importer**, et reviennent d’elles-mêmes pour un fichier aux mêmes titres.
- **Où ils vont** : un carnet d’adresses modifiable, ou un nouveau créé dans le même pas, et des groupes ajoutés à chaque nouveau contact (à ceux déjà là aussi, seulement si vous cochez la case).
- **Ceux déjà là** sont trouvés par leur identifiant, ou par le même nom avec la même adresse électronique ou le même numéro, deux civilités différentes étant deux personnes (« M. Martin » et « Mme Martin ») : jamais écrits deux fois. Le même numéro ou la même adresse sous un autre nom est une autre personne (un foyer, un standard), importée comme sa propre fiche et comptée. Une fiche dont l’identifiant est déjà là sous un nom clairement autre est listée comme « même identifiant, autre nom », laissée telle quelle, avec sa propre case pour la compléter quand même. Un contact déjà là reste exactement tel qu’il est : le compte les liste un par un avec ce que le fichier leur ajouterait, et **Les compléter avec ce que le fichier ajoute**, décoché d’abord, l’ajoute. Rien de ce qu’ils contiennent n’est jamais remplacé. Deux fiches au seul même nom sont gardées toutes deux, pour que [les doublons](#duplicates) les proposent. Les groupes que le fichier donne à ses nouveaux contacts sont listés, avec leur nombre ; l’un d’eux qu’une de vos listes lit n’est gardé que si vous cochez « Garder aussi les groupes qui décident qui peut vous joindre » ; la même case décide de ce que « Les compléter » donne à un contact déjà là.
- **Les compter** dit ce qui va se passer avant que rien ne soit écrit : combien de lignes ont été lues comme contacts, les nouveaux, ceux déjà là, ceux trouvés deux fois dans le fichier, ceux importés à part, les numéros réécrits, les anniversaires qui se lisent de deux façons (gardés tels qu’écrits dans la note de la fiche), et les lignes qui ne se lisent pas. **Importer** le fait avec sa progression, et s’annule d’un seul coup avec **Annuler**.

Les numéros écrits sans leur indicatif sont écrits avec (« +33 6… »), selon le pays choisi dans les réglages des contacts, quand vous cochez la case (cochée d’abord pour un fichier CSV) ; le compte dit combien. Une colonne de civilité va au champ de civilité ; une civilité au début d’un nom y va aussi quand vous cochez la case, là où elle ne peut être qu’une civilité. Le fichier `.xlsx` d’un tableur n’est pas lu : enregistrez-le d’abord en CSV.

## Les doublons {#duplicates}

**Doublons**, au-dessus de la liste, cherche deux choses, et ne change rien avant votre clic :

- **Un numéro ou une adresse électronique écrits deux fois sur une même fiche.** « 04 65 71 12 34 » et « +33 4 65 71 12 34 » sont un seul numéro : les espaces, les points et l’indicatif du pays n’y changent rien. Chaque fiche concernée est listée avec ce qui partirait, coché ; **Retirer les doublons** garde un exemplaire de chaque, celui qui est écrit avec son pays, avec ce que les autres en disaient (mobile, travail).
- **Deux fiches qui pourraient être une seule personne** : le même nom (dans n’importe quel ordre, sans tenir compte des majuscules ni des accents), le même numéro, ou la même adresse électronique. Elles viennent une paire à la fois, côte à côte, avec ce qu’elles ont en commun. Choisissez le nom gardé, puis **Fusionner** : une fiche garde tout des deux (numéros, adresses électroniques et postales, sites web, catégories, notes, et la photo, l’organisation et l’anniversaire du nom gardé quand elle en a), et l’autre est supprimée, ici et sur le serveur. **Pas la même personne** les garde à part et ne le redemande jamais sur cet appareil ; **Plus tard** montre la paire suivante.

Quand la fiche gardée est dans un carnet dont les groupes décident qui peut vous joindre et l’autre dans un carnet dont les groupes ne décident pas (quelqu’un d’autre peut le modifier), les groupes que seule l’autre porte sont laissés de côté : la fusion les nomme, avec une case, décochée, pour les prendre aussi, et dit ce que les prendre changerait.

**Fait récemment** liste ce qui a été nettoyé et fusionné, chacun avec **Annuler**, pendant trente jours : les fiches reviennent comme elles étaient, ici et sur le serveur. Les contacts gardés sur cet appareil seulement sont traités de la même façon.

!!! note "Les numéros écrits sans leur pays"
    « 04 65 71 12 34 » n’a pas de pays : Sioul le lit comme un numéro du pays choisi dans les réglages des contacts, par défaut celui de votre système (la France en français). Cela ne sert qu’à comparer les numéros : vos fiches les gardent tels qu’ils sont écrits.

## La carte {#the-map}

**Carte**, au-dessus de la liste, montre sur une seule carte toutes les personnes qui ont une adresse postale, une épingle chacune ; une épingle ouvre la fiche, et **Liste** ramène la liste.

Pour être placée, une adresse doit être transformée en un point sur la carte. Sioul ne le fait pas de lui-même : la première fois, il demande.

!!! note "Placer vos contacts"
    **Les placer** envoie les adresses postales de vos contacts au géocodeur d’OpenStreetMap (Nominatim), une fois chacune, une par seconde. Une adresse qu’il ne sait pas lire (le nom d’un lieu avant sa rue) est demandée une fois à la Base Adresse Nationale quand votre région est la France, puis à Photon, qui tolèrent les mots qu’ils ne connaissent pas. Rien d’autre ne part avec. Les lieux trouvés sont gardés sur cet appareil, et chaque adresse n’est demandée qu’une fois.

Les images de la carte viennent d’OpenStreetMap, demandées avec mesure et gardées. Vous pouvez donner une autre source dans les réglages.

## Sur Android {#on-a-phone}

Sous Android, Sioul synchronise lui-même vos carnets d’adresses et en garde sa propre copie, à part des contacts de l’appareil. Avec votre accord (Paramètres ▸ Appareils, la carte de ce téléphone, **Ce qu’Android doit permettre** ▸ **Contacts**), il lit les contacts de l’appareil, et seulement pour trois choses : nommer un appelant que vos carnets d’adresses ne connaissent pas, reconnaître qui a écrit dans la notification d’une autre application, et voir lesquelles des personnes qui passent toujours sont en favori pour le mode « Ne pas déranger » d’Android. Il n’écrit jamais un contact, un nom ni un numéro : pour une personne qui devrait être en favori, il ouvre son contact dans votre application Contacts, ou le propre formulaire de cette application, rempli, pour que vous l’enregistriez. Il ne met lui-même un favori, et rien d’autre, qu’une fois que vous activez **Les contacts favoris passent toujours, dans les deux sens**, sous votre liste Passent toujours (désactivé tant que vous ne l’activez pas ; Android demande alors si Sioul peut modifier vos contacts) : un contact que vous mettez en favori sur le téléphone rejoint la liste sur chaque appareil, et chaque personne de la liste est mise en favori là où se trouve son contact. Un favori sur un serveur qui garde les favoris (un Nextcloud) compte de la même façon ([Passent toujours](notifications.md#always-through)). Quand le téléphone filtre vos appels ([Les appels](calls.md)), les appels d’une personne bloquée sont refusés avant que le téléphone sonne.

## Les réglages des contacts {#the-contacts-settings}

Le ⚙ au-dessus de la liste, après ses boutons, a trois parties, **Carnets d’adresses**, **Changements de nombreux contacts, ces trente derniers jours** et **Carte et numéros** :

- **Carnets d’adresses** : chacun avec l’endroit où il est gardé, combien de contacts, à qui il est et s’il est modifiable ; créés, renommés, fusionnés, supprimés et partagés là ([Les carnets d’adresses](#address-books)).
- **Changements de nombreux contacts** : ceux des trente derniers jours, le plus récent d’abord, chacun avec ce qu’il a fait et quand, et **Annuler…** ou la raison pour laquelle il ne peut plus l’être ([Plusieurs contacts à la fois](#many-at-once)).
- **Placer les contacts sur la carte** : activé ou désactivé.
- **Le service de recherche de la carte** : là où Sioul demande où se trouve une adresse (la carte des contacts, celle d’un événement) et où est un bureau et ses horaires (**Les chercher…**), seulement quand vous le demandez : par défaut celui d’OpenStreetMap (Nominatim) ; une autre adresse (`https://…`), ou **Aucun**, qui arrête les deux. Les règles d’OpenStreetMap demandent qu’une application puisse quitter son service, ou s’arrêter, sans mise à jour : Sioul n’est jamais commandé à distance, par principe, et ce réglage est donc cet interrupteur ; il suit vos réglages sur vos autres appareils. Quand le service demande à Sioul d’attendre (il répond 403 ou 429), Sioul le dit une fois et ne l’interroge plus de la journée, pour les deux usages.
- **Tuiles de carte** : d’où viennent les images de la carte, en `https://…/{z}/{x}/{y}.png`. Vide : celles d’OpenStreetMap.
- **Pays des numéros de téléphone écrits sans indicatif** : le pays auquel appartient « 04 65 71 12 34 », pour retrouver le même numéro écrit « +33 4 65 71 12 34 ». Par défaut, celui de votre système, quand c’est l’un des 34 pays et territoires dont Sioul connaît la numérotation ; sinon ces numéros sont comparés tels qu’ils sont écrits, jusqu’à ce que vous choisissiez un pays.

## Où vivent les contacts {#where-contacts-live}

Sur votre serveur de contacts, en fiches standard : votre téléphone et les autres programmes les voient. Modifier un contact dans Sioul ne change que ce que vous avez changé : une photo ou les champs d’un autre programme restent tels qu’ils étaient. Quand une fiche a été modifiée ici et sur le serveur entre deux synchronisations, la version du serveur est gardée, et la vôtre est mise de côté parmi les versions précédentes (**Paramètres ▸ Partage ▸ Voir les versions précédentes**, sous **Vos agendas et contacts**), sur Android aussi, à remettre si vous la préférez ; la ligne d’état le dit.

Les contacts Google marchent de la même façon, en gardant moins : Sioul les écrit comme Google les lit, et dit ce qui serait perdu avant d’y déplacer un contact.

## Pas encore là {#not-there-yet}

Changer la photo d’un contact dans Sioul est prévu. Sioul ne fait pas d’agenda des anniversaires de vos contacts, et ne garde pas de corbeille pour les contacts supprimés au-delà d’**Annuler** : dix secondes dans la ligne d’état, trente jours pour un changement de plusieurs contacts à la fois (Contacts ⚙ ▸ Changements de nombreux contacts).

## Pour aller plus loin {#going-further}

- **Quand deux fiches sont prises pour une seule personne** : un numéro, une adresse électronique ou un nom que partagent plus de quatre fiches (un standard, un bureau, un nom courant) ne dit rien d’une personne, et ne forme donc aucune paire. Les paires qui ont le plus en commun viennent d’abord. Les fiches que vous ne pouvez que lire ne sont jamais nettoyées ni fusionnées.
- **Ce que garde une fusion** : la fiche du nom gardé reste telle qu’elle est, et prend à l’autre ce qui lui manque : numéros, adresses électroniques et postales, sites web, adresses de messagerie instantanée, catégories, surnoms, clés publiques ; les notes des deux, mises bout à bout ; la photo, l’organisation et l’anniversaire de l’autre seulement quand la fiche gardée n’en a pas ; et les champs d’autres programmes qu’elle porte, sauf si la fiche gardée a les mêmes.
- **Annuler après une synchronisation** : quand une fiche a changé après son nettoyage ou sa fusion (une synchronisation, une modification), sa nouvelle version est mise de côté dans un fichier avant que l’ancienne soit réécrite : rien n’est perdu, dans un sens comme dans l’autre.
- **Qui décide de la liste d’une personne**, le plus précis d’abord : son adresse ou son numéro sur une liste ; sa fiche ; ses catégories ; un domaine ou le début d’un numéro (`*@example.org`, les premiers chiffres d’un standard) ; puis Vos contacts, quand elle est dans un carnet d’adresses ou que vous l’avez laissée entrer. À niveau égal, bloqué passe avant restreint, vos contacts et sûr.
- **La photo d’une fiche** est lue dans la fiche elle-même. Une fiche dont la photo est une adresse web (`https` seulement) ne la fait jamais venir d’elle-même : la fiche propose **Charger la photo**, en nommant le serveur. Si vous appuyez, Sioul le demande une fois à ce serveur (vingt secondes et un mégaoctet au plus, une image seulement) et en garde la copie ici, que la fiche montre ensuite.

## Pourquoi cela fonctionne ainsi {#why-it-works-this-way}

**Les carnets pour garder et partager, les groupes pour décider.** Un carnet d’adresses dit où une fiche est gardée et avec qui elle est partagée ; un groupe dit ce que la personne est pour vous (un client, un fournisseur, la famille). Seuls les groupes, avec vos propres choix, décident de qui vous joint, jamais un carnet (une exception, de confiance : les groupes d’un carnet que quelqu’un d’autre peut modifier ne comptent qu’une fois que vous le dites pour ce carnet, puisqu’il pourrait sinon y ajouter une fiche dans un groupe qu’une liste laisse passer) : ranger des contacts entre des carnets dont les groupes comptent ne change jamais qui peut vous joindre, et une fiche déplacée vers un autre carnet garde son identifiant, ses groupes et sa liste. Un déplacement, une fusion, un groupe donné ou un import qui entre dans un carnet dont les groupes sont tenus à l’écart, ou qui en sort, dit avant de commencer, chiffres à l’appui, ce qu’il change. Les groupes voyagent dans chaque fiche vers toutes les applications, et vers Google, qui ne garde qu’un carnet d’adresses ; la même règle vaut donc partout où sont vos contacts.

Une personne n’est pas une fiche. Une fiche sert à joindre quelqu’un, et toutes les applications de votre téléphone la lisent, comme le serveur qui la garde ; un bébé n’a pas de fiche, et une personne peut en avoir deux. Une personne est donc une fiche de Sioul, gardée sur vos appareils, liée aux contacts qui sont elle.

Rien n’est écrit dans vos fiches. Le format vCard a une ligne pour les liens entre personnes (RFC 6350), mais aucun carnet d’adresses ne la garde comme un lien que toutes les applications suivent : Google garde un nom et un type, Apple un nom étiqueté, Nextcloud un texte typé, et le serveur de contacts de Google n’a aucune place pour elle. Écrit dans une fiche, « Léa : enfant » serait lu par toutes les applications qui lisent vos contacts, et ce sont les plus proches qui peuvent mal user de tels détails : des partenaires violents font des fonctions familiales ordinaires un moyen de surveiller (Freed et al. 2018 ; Levy et Schneier 2020).

Chaque personne est fermée aux agents d’IA tant que vous ne l’ouvrez pas. Garder les papiers de votre famille sur vos propres appareils est une affaire domestique, que le droit des données vous laisse (RGPD, art. 2, § 2, c) ; ce qu’un agent lit part chez l’entreprise qui fait tourner son modèle, hors de votre foyer, et les données des enfants méritent une protection particulière (RGPD, considérant 38). Un agent ne voit donc les papiers d’une personne qu’une fois cette personne ouverte, et jamais les papiers de santé de personne d’autre que vous.

Sioul ne surveille personne : pas de position, pas de « vu à », pas de lecture des messages de quiconque, pas de compte de ce qui a été fait pour qui. Les enfants ont mis une étoile aux applications de contrôle parental dans trois avis sur quatre, et plus de contrôle allait avec plus de risques, non moins (Ghosh et al. 2018) ; des enfants adultes voulaient des capteurs et du suivi plus que leurs mères (Berridge et Wetle 2020). Le soin s’achève dans la réponse de qui le reçoit (Tronto 1993) : Sioul ne garde rien sur quelqu’un que vous ne pourriez lui montrer.

Plus de détails dans [ce que dit la recherche (en anglais)](https://sioul.aurelienpierre.com/dev/research.html), constats 55 à 60.

## Comparé à d’autres applications {#compared-with-other-apps}

**Trouver les doublons n’est pas propre à Sioul.** Google Contacts propose des fusions sous « Fusionner et corriger ». Apple Contacts cherche les doublons sur le Mac, et sur iPhone et iPad trouve les fiches qui ont le même prénom et le même nom. KAddressBook trouve les fiches qui ont le même nom ou le même surnom, ou une adresse électronique en commun, et Proton Contacts propose de fusionner les fiches qui portent le même nom. Ce que Sioul fait à sa façon :

- **Il fusionne les fiches elles-mêmes, sur le serveur qui les garde**, le vôtre ou celui de Google, pour que votre téléphone et toutes les autres applications ne voient ensuite qu’une fiche. KAddressBook le fait aussi ; GNOME Contacts et Fossify ne font que montrer deux fiches comme une seule, dans leur propre fenêtre.
- **Il compare les numéros selon le plan de numérotation de chaque pays**, pour que « 04 65 71 12 34 » et « +33 4 65 71 12 34 » soient un seul numéro, là où GNOME Contacts compare leurs sept derniers chiffres.
- **Il laisse de côté ce que beaucoup de fiches partagent** : un numéro, une adresse électronique ou un nom présents sur plus de quatre fiches ne forment aucune paire.
- **Il annule chaque fusion à part, pendant trente jours**, ici et sur le serveur, là où Google, et Apple sur iCloud.com, ne peuvent que remettre tout le carnet d’adresses à un moment antérieur, et où KAddressBook, Proton et Nextcloud ne décrivent aucune annulation.

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

=== "Au quotidien"

    | | Sioul | Google | Apple | Outlook | Thunderbird | DAVx⁵ + Fossify |
    |---|---|---|---|---|---|---|
    | Trouve les doublons et les fusionne quand vous le dites | ✓ | ✓ | ✓ | en partie¹ | en partie² | en partie³ |
    | Prend « 04 65 71 12 34 » et « +33 4 65 71 12 34 » pour un seul numéro | ✓ | ?⁴ | ? | ? | en partie⁵ | en partie⁶ |
    | Annule une fusion plus tard | ✓⁷ | en partie⁸ | en partie⁹ | ?¹⁰ | ✗ | —¹¹ |
    | Montre vos contacts sur une carte | ✓ | en partie¹² | en partie¹² | ? | ✗¹³ | en partie¹⁴ |
    | Des groupes qui vous suivent sur vos autres appareils | ✓ | en partie¹⁵ | ✓ | ✓¹⁶ | en partie¹⁷ | ✓ |
    | Bloque quelqu’un sur tous les canaux à la fois | ✓¹⁸ | en partie¹⁹ | ✓²⁰ | en partie²¹ | en partie²¹ | en partie²² |
    | Laisse passer les personnes choisies aux moments choisis | ✓²³ | en partie²⁴ | ✓ | en partie²⁵ | ✗ | en partie²⁶ |
    | Liste les tâches, les événements et les notes liés à la personne | ✓ | en partie²⁷ | ? | en partie²⁸ | ✗²⁹ | ✗ |
    | Liste les courriels échangés avec elle | en partie³⁰ | en partie²⁷ | ? | ✓ | en partie²⁹ | ✗ |
    | Retrouve un contact supprimé | en partie³¹ | ✓ | en partie⁹ | ✓ | ✗ | ✗ |
    | Importe et exporte des fichiers vCard | ✓³² | ✓ | ✓ | en partie³³ | ✓ | ✓ |
    | Met les anniversaires de vos contacts dans votre agenda | ✗³⁴ | ✓ | ✓ | ✓ | ✗ | ✓³⁵ |
    | Pour d’autres personnes : celles pour qui vous faites des démarches | ✓³⁶ | en partie³⁷ | en partie³⁸ | ? | ? | ? |

    1. Le nouvel Outlook et Outlook sur le web cachent une fiche qui est la copie exacte d’une autre, ou une partie d’une autre, sans demander et sans fusionner ; Outlook classique ne demande que quand vous enregistrez ou importez un doublon.
    2. Seulement avec le module complémentaire Duplicate Contacts Manager, fait par quelqu’un d’autre : vous modifiez ou supprimez une fiche de chaque paire.
    3. Fossify montre les fiches qui portent le même nom comme une seule entrée, sans demander ; les fiches enregistrées ne sont pas fusionnées.
    4. Google garde la forme internationale de chaque numéro, mais ne dit pas s’il s’en sert pour trouver les doublons.
    5. Avec le module de la note 2, une fois qu’on lui a donné un indicatif de pays ; les numéros sont comparés, jamais réécrits.
    6. Sur l’écran d’un contact, les numéros qui finissent par les mêmes neuf chiffres ne sont montrés qu’une fois.
    7. Chaque nettoyage et chaque fusion à part, pendant trente jours.
    8. « Annuler les modifications » remet tout le carnet d’adresses à un moment des 30 derniers jours, en annulant chaque changement fait depuis.
    9. Aucune annulation de fusion ni aucune corbeille ne sont décrites ; iCloud.com peut remettre une copie antérieure de tous vos contacts.
    10. Le nouvel Outlook ne fusionne rien ; Outlook classique ne décrit aucune annulation de sa mise à jour d’un contact existant.
    11. Rien n’est fusionné dans les fiches enregistrées : désactiver le réglage montre de nouveau les fiches séparées.
    12. L’adresse d’un contact s’ouvre dans Google Maps, ou dans Plans d’Apple ; une carte de tous vos contacts n’est pas décrite.
    13. « Get Map » a disparu avec le nouveau carnet d’adresses de Thunderbird 102 ; la demande pour le faire revenir est ouverte.
    14. Une adresse postale s’ouvre dans l’application de cartes que vous avez installée.
    15. Les libellés s’affichent dans Gmail et dans l’application Android de Google ; le CardDAV de Google ne les transmet pas aux autres applications.
    16. Entre les applications d’Outlook, sur le même compte Microsoft.
    17. Des listes de diffusion, dans les carnets gardés sur l’ordinateur seulement ; les groupes d’un serveur ne s’affichent pas encore.
    18. Leurs courriels sont mis de côté, leurs messages dans les autres applications ne viennent jamais, et leurs appels sont refusés sur un téléphone Android qui filtre les appels.
    19. L’application Contacts d’Android bloque les appels et les SMS d’un contact ; Gmail bloque les courriels à part, et l’un ne s’applique pas à l’autre.
    20. Un seul blocage couvre Téléphone, FaceTime, Messages et Mail, sur tous les appareils du même compte Apple.
    21. Les courriels seulement, par une liste de blocage ou un filtre ; il ne gère pas les appels.
    22. Les appels et les SMS, par les numéros bloqués d’Android ; pas les courriels.
    23. Les courriels sur chaque appareil ; les appels et les messages des autres applications sur un téléphone Android où Sioul tourne.
    24. Les modes d’Android laissent passer les appels et les messages de personnes choisies ; les courriels sont à part.
    25. Son application pour téléphone coupe ses notifications selon un horaire ; laisser passer des personnes choisies n’est pas décrit.
    26. Le mode « Ne pas déranger » d’Android peut laisser passer les contacts en favori.
    27. Pour les comptes professionnels et scolaires seulement : les conversations et les réunions avec les personnes de votre organisation.
    28. La fiche de profil, dans l’application d’Outlook pour téléphone, montre les événements que vous partagez.
    29. Un bouton cherche les courriels du contact, un autre crée un événement avec lui ; la fiche ne liste rien.
    30. Les courriels qu’elle vous a envoyés et dont Sioul a prouvé qu’ils viennent d’elle, et ceux que vous lui avez liés ; ce que vous lui avez envoyé, par la recherche des courriels.
    31. **Annuler** pendant dix secondes après **Supprimer** ; les contacts supprimés depuis une sélection reviennent pendant trente jours (Contacts ⚙ ▸ Changements de nombreux contacts) ; pas de corbeille.
    32. Les fichiers vCard et CSV s’importent, ceux déjà là trouvés par leur identifiant ou leur nom et laissés tels quels sauf si vous demandez de les compléter ; un carnet ou une sélection s’exporte en un fichier vCard, sur un ordinateur.
    33. Outlook classique ouvre et enregistre des vCard une par une ; le nouvel Outlook et le web importent et exportent du CSV seulement.
    34. Sioul ne fait pas d’agenda des anniversaires ; celui que garde votre serveur (Nextcloud en garde un) s’affiche comme tout agenda que vous ne pouvez que lire.
    35. Dans Fossify Calendar ; Etar n’en a pas.
    36. Des personnes gardées par vous, sur vos appareils, liées à leurs papiers et à leurs fiches, chacune fermée aux agents d’IA tant qu’elle n’est pas ouverte ([Vos proches](#household)).
    37. La « relation » d’un contact garde le nom d’une autre personne et un type, en texte ; Family Link fait du compte d’un enfant un membre d’un groupe familial, dont le parent voit la position.
    38. Les « noms associés » sont des noms étiquetés, liés à aucune fiche ; l’organisateur du Partage familial voit le Temps d’écran des membres, et les membres partagent leur position à moins de l’éteindre.

=== "Technique"

    | | Sioul | Google | Apple | Outlook | Thunderbird | DAVx⁵ + Fossify |
    |---|---|---|---|---|---|---|
    | Tout serveur CardDAV, en lecture et en écriture | ✓ | en partie¹ | ✓ | ✗² | ✓ | ✓ |
    | Les contacts Google, en lecture et en écriture | ✓ | — | en partie³ | en partie⁴ | ✓⁵ | ✓⁶ |
    | Garde les groupes dans chaque fiche, où les autres applications les trouvent | ✓ | ✗⁷ | ✗⁷ | en partie⁸ | ✗⁹ | ✓¹⁰ |
    | Contacts chiffrés de bout en bout | ✗¹¹ | ✗ | ✗¹² | ✗¹³ | ✗ | ✗ |
    | Logiciel libre | ✓ GPL-3.0+ | ✗ | ✗ | ✗ | ✓ MPL-2.0 | ✓ GPL-3.0 |

    1. Les autres applications joignent les contacts de Google par CardDAV ; Google Contacts ne peut pas ajouter le carnet d’adresses d’un autre serveur.
    2. Microsoft ne cite pas CardDAV parmi les comptes qu’ajoute Outlook, et dit qu’Outlook pour Mac ne le prend pas en charge.
    3. En lecture et en écriture sur iPhone et iPad ; en lecture seule sur le Mac.
    4. Le nouvel Outlook et l’application d’Outlook pour téléphone copient les contacts d’un compte Gmail dans le nuage de Microsoft ; Outlook classique importe un export CSV.
    5. Par le CardDAV de Google et la connexion de Google ; les libellés ne passent pas.
    6. Par le propre compte Google d’Android, ou par DAVx⁵, qui le dit « pas officiellement pris en charge » ; les libellés ne passent pas.
    7. Les groupes sont des objets à part (les libellés de Google, les groupes d’Apple), pas des mots dans chaque fiche.
    8. Les catégories sont gardées sur chaque contact, dans le format propre à Microsoft.
    9. Les groupes et les catégories d’un serveur ne s’affichent pas encore.
    10. DAVx⁵ garde les groupes en catégories dans chaque fiche, ou en fiches à part, comme vous le choisissez pour chaque compte.
    11. Les contacts d’un serveur peuvent être lus par ce serveur, comme avec toute application CardDAV. Ce que Sioul partage entre vos appareils (qui est sur quelle liste, les appels filtrés, les carnets d’adresses gardés sur un appareil) est scellé.
    12. Pas non plus avec la protection avancée des données : Apple dit que CardDAV n’a pas de chiffrement de bout en bout intégré.
    13. Chiffrés en chemin et sur les disques de Microsoft, avec des clés que Microsoft détient.

**Les autres carnets d’adresses.** GNOME Contacts propose de lier deux fiches qui ont un numéro, une adresse électronique ou un nom proche en commun, se souvient d’un « non », et vous laisse les délier à tout moment ; il laisse les deux fiches telles qu’elles sont dans leurs carnets. Nextcloud Contacts fusionne deux fiches que vous choisissez, côte à côte, champ par champ, et garde les groupes dans chaque fiche, comme Sioul les lit ; il ne cherche pas les doublons de lui-même. Proton Contacts chiffre de bout en bout chaque champ d’une fiche, sauf le nom et les adresses électroniques, et n’a pas de CardDAV.

??? info "Sources (en anglais)"
    - Google Contacts : fusionner les doublons (« Fusionner et corriger »), <https://support.google.com/contacts/answer/7078226>, lu le 8 octobre 2026.
    - Google Contacts : supprimer, restaurer et bloquer des contacts, et annuler des modifications, <https://support.google.com/contacts/answer/7280886>, lu le 8 octobre 2026.
    - Google Contacts : les contacts sous Android, regroupés et libellés, <https://support.google.com/contacts/answer/30970>, lu le 8 octobre 2026.
    - Google Maps : l’adresse d’un contact sur la carte, <https://support.google.com/maps/answer/3131570>, lu le 8 octobre 2026.
    - Compte Google : les utilisateurs bloqués, et ce que couvre un blocage, <https://support.google.com/accounts/answer/6388749>, lu le 8 octobre 2026.
    - Gmail : bloquer un expéditeur, <https://support.google.com/mail/answer/8151>, lu le 8 octobre 2026.
    - Android : les modes et « Ne pas déranger », <https://support.google.com/android/answer/9069335>, lu le 8 octobre 2026.
    - Téléphone de Google : le filtrage des appels, <https://support.google.com/phoneapp/answer/9118387>, lu le 8 octobre 2026.
    - Google Workspace Updates : la page d’un contact pour les comptes professionnels (juin 2021), <https://workspaceupdates.googleblog.com/2021/06/update-google-contacts-experience.html>, lu le 8 octobre 2026.
    - Google Agenda : les anniversaires, <https://support.google.com/calendar/answer/6084659>, lu le 8 octobre 2026.
    - API Google People : CardDAV, <https://developers.google.com/people/carddav>, lu le 8 octobre 2026.
    - API Google People : la ressource « person » (la forme internationale des numéros), <https://developers.google.com/people/api/rest/v1/people>, lu le 8 octobre 2026.
    - API Google People : les groupes de contacts, <https://developers.google.com/people/api/rest/v1/contactGroups>, lu le 8 octobre 2026.
    - Contacts pour Mac : fusionner des fiches, <https://support.apple.com/guide/contacts/merge-contact-cards-adrbk1456/mac>, lu le 8 octobre 2026.
    - Guide de l’iPhone : fusionner ou masquer des contacts en double, <https://support.apple.com/guide/iphone/merge-or-hide-duplicate-contacts-iph2ab28320d/ios>, lu le 8 octobre 2026.
    - Guide d’iCloud : restaurer des contacts, <https://support.apple.com/guide/icloud/restore-contacts-mm1d9cfdb498/icloud>, lu le 8 octobre 2026.
    - Contacts pour Mac : l’adresse d’un contact sur une carte, <https://support.apple.com/guide/contacts/show-a-contacts-address-on-a-map-adrb183385bb/mac>, lu le 8 octobre 2026.
    - Guide de sécurité personnelle d’Apple : bloquer les appels et les messages, <https://support.apple.com/guide/personal-safety/block-calls-and-messages-ipsac1e87c54/web>, lu le 8 octobre 2026.
    - Guide de l’iPhone : laisser passer des personnes avec un mode Concentration, <https://support.apple.com/guide/iphone/allow-or-silence-notifications-for-a-focus-iph21d43af5b/ios>, lu le 8 octobre 2026.
    - Guide de l’iPhone : filtrer et bloquer les appels, <https://support.apple.com/guide/iphone/screen-and-block-calls-iphe4b3f7823/ios>, lu le 8 octobre 2026.
    - Calendrier pour Mac : les jours fériés et les anniversaires, <https://support.apple.com/guide/calendar/show-holidays-and-birthdays-iclead4e0ec3/mac>, lu le 8 octobre 2026.
    - Contacts pour Mac : les comptes et ce qu’ils laissent modifier, <https://support.apple.com/guide/contacts/update-contact-information-adrbk1515/mac>, lu le 8 octobre 2026.
    - Apple Developer : CNGroup, <https://developer.apple.com/documentation/contacts/cngroup>, lu le 8 octobre 2026.
    - Apple : vue d’ensemble de la sécurité des données iCloud, <https://support.apple.com/en-us/102651>, lu le 8 octobre 2026.
    - Outlook : gérer les contacts en double, <https://support.microsoft.com/en-us/Outlook/people/manage-duplicate-contacts-in-outlook>, lu le 8 octobre 2026.
    - Outlook sur le web : contacts et listes, <https://support.microsoft.com/en-us/outlook/using-contacts-people-in-outlook-on-the-web>, lu le 8 octobre 2026.
    - Outlook : bloquer des expéditeurs, <https://support.microsoft.com/en-us/Outlook/mail/block-or-unblock-senders-in-outlook>, lu le 8 octobre 2026.
    - Outlook : les notifications sur un téléphone, <https://support.microsoft.com/en-us/outlook/how-can-i-turn-push-notifications-and-sounds-on-or-off>, lu le 8 octobre 2026.
    - Microsoft 365 : les fiches de profil, <https://support.microsoft.com/en-us/Outlook/profile-cards-in-microsoft-365>, lu le 8 octobre 2026.
    - Outlook.com : contacts et listes de contacts, <https://support.microsoft.com/en-us/outlook/create-view-and-edit-contacts-and-contact-lists-in-outlook-com>, lu le 8 octobre 2026.
    - Outlook : restaurer un contact supprimé, <https://support.microsoft.com/en-us/Outlook/people/restore-or-recover-a-deleted-contact-in-outlook>, lu le 8 octobre 2026.
    - Outlook : importer des vCard, <https://support.microsoft.com/en-us/outlook/import-vcards-to-outlook-contacts>, lu le 8 octobre 2026.
    - Outlook pour Mac : les agendas iCloud, et ni CalDAV ni CardDAV, <https://support.microsoft.com/en-us/outlook/sync-your-icloud-calendar-with-outlook-for-mac>, lu le 8 octobre 2026.
    - Outlook : les comptes synchronisés par le nuage de Microsoft, <https://support.microsoft.com/en-us/Outlook/getstarted/sync-your-account-in-outlook-to-the-microsoft-cloud>, lu le 8 octobre 2026.
    - Microsoft Purview : le chiffrement dans Microsoft 365, <https://learn.microsoft.com/en-us/purview/encryption>, lu le 8 octobre 2026.
    - Outlook : un agenda des anniversaires, <https://support.microsoft.com/en-us/outlook/calendar/add-a-birthday-calendar-and-reminder-in-outlook>, lu le 8 octobre 2026.
    - Thunderbird : bogue 259531, fusionner les contacts en double (ouvert depuis 2004), <https://bugzilla.mozilla.org/show_bug.cgi?id=259531>, lu le 8 octobre 2026.
    - Modules de Thunderbird : Duplicate Contacts Manager, <https://addons.thunderbird.net/en-US/thunderbird/addon/duplicate-contacts-manager/>, lu le 8 octobre 2026.
    - Thunderbird : bogue 94407, annuler la suppression d’une fiche, <https://bugzilla.mozilla.org/show_bug.cgi?id=94407>, lu le 8 octobre 2026.
    - Thunderbird : bogue 1781076, « Get Map » dans le nouveau carnet d’adresses, <https://bugzilla.mozilla.org/show_bug.cgi?id=1781076>, lu le 8 octobre 2026.
    - Thunderbird : bogue 1764184, les groupes d’un serveur CardDAV, <https://bugzilla.mozilla.org/show_bug.cgi?id=1764184>, lu le 8 octobre 2026.
    - Assistance de Thunderbird : bloquer un expéditeur, <https://support.mozilla.org/en-US/kb/blocking-sender>, lu le 8 octobre 2026.
    - Assistance de Thunderbird : les indésirables et vos carnets d’adresses, <https://support.mozilla.org/en-US/kb/thunderbird-and-junk-spam-messages>, lu le 8 octobre 2026.
    - Blog de Thunderbird : le carnet d’adresses de Thunderbird 102, <https://blog.thunderbird.net/2022/05/7-great-new-features-coming-to-thunderbird-102/>, lu le 8 octobre 2026.
    - Thunderbird : bogue 151994, un agenda des anniversaires tiré du carnet d’adresses, <https://bugzilla.mozilla.org/show_bug.cgi?id=151994>, lu le 8 octobre 2026.
    - Fossify Commons : ContactsHelper.kt, les fiches de même nom montrées comme une seule, <https://github.com/FossifyOrg/Commons/blob/main/commons/src/main/kotlin/org/fossify/commons/helpers/ContactsHelper.kt>, lu le 8 octobre 2026.
    - Fossify Contacts : ticket 607, la fusion par le nom seul, <https://github.com/FossifyOrg/Contacts/issues/607>, lu le 8 octobre 2026.
    - Fossify Contacts : ViewContactActivity.kt, les numéros comparés sur neuf chiffres, <https://github.com/FossifyOrg/Contacts/blob/main/app/src/main/kotlin/org/fossify/contacts/activities/ViewContactActivity.kt>, lu le 8 octobre 2026.
    - Android Developers : BlockedNumberContract, <https://developer.android.com/reference/android/provider/BlockedNumberContract>, lu le 8 octobre 2026.
    - Android Developers : NotificationManager.Policy, <https://developer.android.com/reference/android/app/NotificationManager.Policy>, lu le 8 octobre 2026.
    - Fossify Calendar : ses chaînes (« Add contact birthdays »), <https://github.com/FossifyOrg/Calendar/blob/main/app/src/main/res/values/strings.xml>, lu le 8 octobre 2026.
    - Etar : ticket 234, les anniversaires, <https://github.com/Etar-Group/Etar-Calendar/issues/234>, lu le 8 octobre 2026.
    - DAVx⁵ : essayé avec Google, <https://www.davx5.com/tested-with/google>, lu le 8 octobre 2026.
    - Manuel de DAVx⁵ : informations techniques (les groupes), <https://manual.davx5.com/technical_information.html>, lu le 8 octobre 2026.
    - KAddressBook : sa page, <https://apps.kde.org/kaddressbook/>, lu le 8 octobre 2026.
    - KAddressBook : le code de la recherche des doublons, <https://invent.kde.org/pim/kdepim-addons/-/blob/master/kaddressbook/plugins/mergelib/job/searchpotentialduplicatecontactjob.cpp>, lu le 8 octobre 2026.
    - Manuel de KAddressBook : les commandes, <https://docs.kde.org/stable_kf6/en/kaddressbook/kaddressbook/commands.html>, lu le 8 octobre 2026.
    - Proton : fusionner des contacts, <https://proton.me/support/merge-contacts>, lu le 8 octobre 2026.
    - Proton : Proton Contacts et son chiffrement, <https://proton.me/support/proton-contacts>, lu le 8 octobre 2026.
    - Proton : ajouter et importer des contacts, <https://proton.me/support/adding-contacts>, lu le 8 octobre 2026.
    - Aide de GNOME : lier et délier des contacts, <https://help.gnome.org/gnome-help/contacts-link-unlink.html>, lu le 8 octobre 2026.
    - GNOME Contacts : contacts-store.vala, <https://gitlab.gnome.org/GNOME/gnome-contacts/-/blob/main/src/contacts-store.vala>, lu le 8 octobre 2026.
    - GNOME folks : phone-details.vala, les numéros comparés sur sept chiffres, <https://gitlab.gnome.org/GNOME/folks/-/blob/main/folks/phone-details.vala>, lu le 8 octobre 2026.
    - GNOME Contacts : ticket 145, lier deux fiches d’un même carnet Nextcloud, <https://gitlab.gnome.org/GNOME/gnome-contacts/-/issues/145>, lu le 8 octobre 2026.
    - Manuel de Nextcloud : Contacts, <https://docs.nextcloud.com/server/latest/user_manual/en/groupware/contacts.html>, lu le 8 octobre 2026.
    - Nextcloud Contacts : ticket 5619, trouver les doublons, <https://github.com/nextcloud/contacts/issues/5619>, lu le 8 octobre 2026.

## Côté technique {#for-technical-readers}

- **vCard** : Sioul lit vCard 4.0 (RFC 6350) et 3.0 (RFC 2426), avec calcard, et écrit les nouvelles fiches en 3.0. Une modification ne réécrit que les lignes qui ont changé : une fiche inchangée revient octet pour octet, et les libellés groupés d’Apple (`item1.X-ABLabel`), les photos et les champs des autres programmes sont gardés. Les catégories sont les lignes `CATEGORIES` de la fiche, comme Nextcloud Contacts écrit ses groupes. Les groupes gardés comme des fiches à part (le `KIND:group` de vCard 4, le `X-ADDRESSBOOKSERVER-KIND` d’Apple) s’affichent comme des fiches, pas comme des catégories ([Fonctionne avec](compatibility.md#calendars-tasks-and-contacts)).
- **CardDAV** (RFC 6352, sur WebDAV, RFC 4918) : découverte par `/.well-known/carddav` (RFC 6764) et le principal de l’utilisateur (RFC 5397) ; changements relevés avec le jeton de synchronisation (RFC 6578), sinon par les ETag, par lots `addressbook-multiget` de 50 ; écritures avec `If-Match`, pour que rien ne soit écrasé en silence, et `If-None-Match: *` pour une nouvelle fiche. Quand les deux côtés ont changé une fiche, la version du serveur l’emporte et la vôtre est gardée parmi les versions précédentes de cet appareil, à remettre depuis la fenêtre. Une synchronisation interrompue reprend.
- **Le transport** : HTTPS seulement (une adresse `http://` est refusée), TLS par rustls avec les certificats racine du système. Pendant la découverte, une redirection vers un autre domaine n’est pas suivie : votre mot de passe ne part jamais vers un hôte vers lequel votre domaine ne fait que pointer. Authentification Basic, par TLS seulement.
- **Les contacts Google** passent par le propre CardDAV de Google, avec une connexion OAuth 2.0 pour les applications natives (RFC 8252, une redirection en boucle locale) et PKCE (RFC 7636) : le jeton de renouvellement dans le trousseau, le jeton d’accès en mémoire seulement. Les fiches sont écrites en vCard 3.0 pour Google : les libellés, les anniversaires sans année et les champs plus récents s’y perdent, ce que Sioul dit avant un déplacement.
- **Les secrets** : les mots de passe dans le Secret Service (KWallet, GNOME Keyring), le trousseau de macOS, le gestionnaire d’identifiants de Windows, ou le KeyStore d’Android. Les mots de passe ne voyagent jamais entre vos appareils : un compte partagé depuis un autre appareil demande son mot de passe, que le serveur vérifie avant que le trousseau de cet appareil le garde.
- **Les numéros de téléphone** sont comparés par une clé E.164, faite à partir d’une table des plans de numérotation de 34 pays et territoires (le préfixe national, le préfixe international, la longueur des numéros, les départements d’outre-mer), pas par libphonenumber. Les numéros courts (« 112 », « 3631 ») et ceux qui contiennent des lettres sont comparés tels qu’ils sont écrits. Les valeurs ne sont jamais réécrites.
- **Les doublons** : les noms sont comparés sans tenir compte des majuscules, des accents, des ligatures (ß, æ, œ) ni de la ponctuation, leurs mots triés ; un nom, un numéro ou une adresse électronique que partagent de deux à quatre fiches forment des paires, classées selon le nombre de sortes d’indices qu’elles partagent. « Pas la même personne » est gardé par UID dans `$XDG_STATE_HOME/sioul/contacts-not-the-same.toml`. Une fusion garde les propriétés à valeur unique de la fiche gardée (`ORG`, `TITLE`, `BDAY`, `PHOTO`, `TZ`, `GEO`…) et ajoute les propriétés à plusieurs valeurs qui lui manquent (`ADR`, `URL`, `IMPP`, `RELATED`, `NICKNAME`, `KEY`, `CALURI`…) ; une ligne vCard 4.0 qui entre dans une fiche 3.0 est convertie. Chaque changement est d’abord noté, avec les fiches avant et après, dans `$XDG_STATE_HOME/sioul/contacts-undo`, pendant trente jours ; une écriture qui échoue défait celles qui la précèdent.
- **Qui est une personne** : son adresse ou son numéro sur une liste, puis sa fiche (`contact:<UID>`), puis ses catégories, puis le motif le plus précis (`*@example.org`, `tel:+3319900*`), puis Vos contacts (`neutral` dans les fichiers) quand elle est dans un carnet d’adresses ou a été laissée entrer, sinon inconnue ; à précision égale, bloqué avant restreint, vos contacts et sûr. Les listes sont de simples fichiers texte ; les numéros sont reconnus par leur clé E.164.
- **La preuve avant les listes** : un courriel dont SPF et DKIM échouent tous les deux, sans DMARC réussi, est jugé comme celui d’un inconnu quelle que soit la liste de son adresse, sauf si un sceau ARC (RFC 8617) de votre propre fournisseur ou de votre domaine s’en porte garant une fois le courriel transféré. Un échec DMARC sous une politique `quarantine` ou `reject` est jugé falsifié et mis de côté avant cela. Bloqué est vérifié d’abord et vaut toujours. Un nom lu dans la notification d’une autre application ne peut que faire baisser le rang de quelqu’un, jamais l’élever, puisque n’importe qui peut prendre n’importe quel nom dans une discussion.
- **Le géocodage** suit la politique d’utilisation de Nominatim : un User-Agent qui nomme Sioul, une demande toutes les 1,1 seconde, par HTTPS seulement (redirections comprises), l’adresse seule (ses lignes mises bout à bout), à l’adresse du service de recherche de la carte (`[map] search`, réglages des Contacts ; `none` l’arrête ; après un 403 ou un 429, plus rien de la journée locale, gardé dans `$XDG_STATE_HOME/sioul/map-search-wait`) ; une adresse pour laquelle il ne trouve rien est demandée une fois à la Base Adresse Nationale (région France), puis à Photon, leur première réponse gardée (`sioul_sync::geocode::locate`) ; les réponses, trouvées ou non, sont gardées dans `$XDG_CACHE_HOME/sioul/places.toml`. La carte est le greffon OpenStreetMap de Qt Location, avec la recherche de fournisseurs en ligne de Qt désactivée ; la grande carte ne charge des tuiles qu’une fois au moins une adresse placée. Le serveur de tuiles, comme pour toute carte en ligne, voit quelles zones vous regardez.
- **L’affichage d’une fiche** : une fiche vient d’un serveur ou du vCard de n’importe qui, elle est donc montrée sans balisage : les notes en texte brut, les sites web en liens seulement quand ils sont en `http` ou `https`, leur texte échappé. Une photo intégrée est gardée dans le cache, nommée d’après son contenu.
- **Le stockage** : un fichier `.vcf` par fiche dans des dossiers vdir (`~/.local/share/sioul/contacts`), que khard et vdirsyncer lisent aussi ; l’état de la synchronisation est gardé à part. Sioul ne les chiffre pas sur le disque : le chiffrement de votre disque s’en charge. Les dossiers de données, d’état et de cache de Sioul sont créés, ou restreints, au mode 0700 à chaque démarrage sous Linux et macOS, pour que les autres comptes de l’ordinateur ne puissent pas les lire ; sous Android, ils sont dans le stockage privé de l’application.
- **Carnets d’adresses** : créés par un MKCOL étendu (RFC 5689) à la prochaine synchronisation, renommés par PROPPATCH, supprimés par DELETE (un carnet vidé par une fusion seulement quand son serveur n’en garde plus aucune fiche) ; une fiche déplacée entre deux carnets d’un même serveur par le MOVE de WebDAV (RFC 4918, § 9.9) avec `If-Match` et `Overwrite: F`, sinon envoyée à nouveau avec `If-None-Match: *` et son ancienne copie supprimée une fois la nouvelle dotée de son ETag. Partagés par le partage de Nextcloud (`oc:share`, `oc:invite`), comme les agendas. Tout ce qui écrit une fiche passe par une seule porte, qui refuse un carnet que son serveur met en lecture seule.
- **Plusieurs à la fois** : un lot garde tout son plan, chaque fiche telle qu’elle était, dans `$XDG_STATE_HOME/sioul/contacts-batches/` avant de commencer (trente jours), puis change une fiche après l’autre ; une note à côté dit chaque pas fait. Son Annuler reprend chaque pas fait tant que sa fiche est telle que le pas l’a laissée, et laisse les autres. Les filtres sont gardés dans `$XDG_STATE_HOME/sioul/contacts-view.toml` (jamais la recherche), les colonnes gardées de l’import dans `contacts-import.toml` à côté. Les civilités sont des données (`crates/sioul-core/data/titles.toml`), et vont au préfixe honorifique de `N` (RFC 6350, § 6.2.2) ; `TITLE`, la fonction, n’est pas touché.
- **Import** : vCard 2.1, 3.0 et 4.0 lus par calcard (lignes repliées, quoted-printable, jeux de caractères) ; une fiche 2.1 est écrite en 3.0, les autres telles qu’elles viennent, chacune gardant son UID ou en recevant un. CSV : marque d’ordre des octets retirée, UTF-16 quand elle le dit, UTF-8 sinon Windows-1252, séparateur trouvé parmi point-virgule, virgule et tabulation (ou choisi), cellules entre guillemets avec retours à la ligne ; la table des titres de colonnes est une donnée (`contact-columns.toml`). 32 Mo au plus, 20 000 fiches ou lignes, 200 colonnes, 2 Mo par fiche, 64 Ko par cellule. Une fiche est celle d’ici qui a son UID, ou son nom et une adresse ou un numéro.
- **Vos proches** : `persons.toml` dans le dossier de données de Sioul (`~/.local/share/sioul/`), à côté des liens, jamais dans le dossier de notes, une fiche ni un serveur de contacts ([la note de conception](https://sioul.aurelienpierre.com/dev/persons.html), en anglais).
- **Le partage scellé** : chaque enregistrement est chiffré avec XChaCha20-Poly1305, sous une clé faite à partir de votre phrase de passe par Argon2id et gardée dans le trousseau de chaque appareil ; le serveur du dossier partagé voit quel appareil a écrit, quand et combien, jamais quoi. La partie **Expéditeurs** porte les quatre listes, les expéditeurs laissés entrer, les personnes qui passent toujours et les clés publiques des autres ; **Appels** porte les appels filtrés, jamais ceux d’un numéro bloqué ; **Listes gardées ici** porte les carnets d’adresses gardés sur un seul appareil ; **Réglages et comptes** porte vos proches, chaque champ de chaque personne à part, pour qu’un nom changé sur un appareil et une date de naissance ajoutée sur un autre restent tous les deux. Les contacts d’un serveur ne sont pas dans le partage : ils voyagent par leur serveur ([Le partage](sharing.md#what-it-protects-and-what-it-cannot-hide)).
- **Android** : Sioul synchronise les carnets d’adresses dans son stockage privé, pas dans les contacts d’Android. Il demande `READ_CONTACTS`, et `WRITE_CONTACTS` seulement une fois que vous activez les favoris dans les deux sens (`[dnd] stars`, désactivé tant que vous ne l’activez pas) : il écrit alors une seule colonne, `Contacts.STARRED`, d’un contact qui existe, et ne crée, ne modifie ni ne supprime jamais un contact. Sur un serveur qui garde un favori par fiche (la propriété `favorite` de Nextcloud), le même interrupteur écrit cette propriété sur les fiches de vos propres carnets, et rien d’autre, une fois que le serveur en a donné une comme la sienne. Les appels sont filtrés en tant qu’« appli numéro de l’appelant et spam » d’Android (`CallScreeningService`), d’après une table que Rust écrit à l’avance et que Java lit en quelques millisecondes, sans le réseau ; un appel qui ne peut pas être décidé sonne.
