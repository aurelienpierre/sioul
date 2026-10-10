---
description: Les contacts dans Sioul – les noms d’abord, les détails repliés ; les doublons trouvés et fusionnés quand vous le dites, chaque fusion annulable pendant trente jours ; la carte de tout le monde, placée seulement si vous le permettez ; qui vous joint, et quand ; comparés aux autres carnets d’adresses.
---

# Les contacts et la carte {#contacts-and-the-map}

## En bref {#in-short}

Les contacts réunissent les personnes à qui vous avez affaire : leur nom d’abord, leurs détails repliés jusqu’à ce que vous les ouvriez. Sioul trouve la même personne écrite deux fois et fusionne les fiches quand vous le dites, avec trente jours pour annuler chaque fusion. Il montre sur une seule carte toutes les personnes qui ont une adresse postale, une fois que vous l’avez permis. Chaque personne est sur une liste, sûre, neutre, restreinte ou bloquée, ou bien c’est une inconnue, et cette liste décide quand son courrier, ses appels et ses messages vous joignent ; un message falsifié ne peut pas prendre sa place. Vos contacts restent sur votre propre serveur, chez Google ou sur cet appareil, et Sioul ne les joint que par une connexion chiffrée.

<figure markdown="span">
  [![La page Contacts : un champ de recherche, un bouton pour un nouveau contact et un pour la carte au-dessus d’une liste de noms, chacun avec une adresse ou un numéro dessous ; un contact ouvert à droite, avec sa fonction et son organisation, une adresse électronique avec Écrire, un numéro de téléphone avec Appeler, «   Son courrier   » réglé sur Neutre, une petite carte avec une épingle à son adresse, et ce qui lui est lié : deux tâches et deux événements.](../assets/screens/fr/contacts.png){ loading=lazy }](../assets/screens/fr/contacts.png "Ouvrir l’image en grand")
  <figcaption>Un contact : comment le joindre, où il est, et ce qui le concerne. <small>Carte © <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> et ses contributeurs.</small></figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

- Vos contacts restent là où vous les gardez : sur votre propre serveur de contacts (un Nextcloud, par exemple), chez Google, ou sur cet appareil. Sioul n’a pas de serveur à lui, et n’envoie vos contacts nulle part ailleurs.
- Sioul ne joint votre serveur que par une connexion chiffrée, et refuse celle qui ne l’est pas. Votre mot de passe reste dans le trousseau de votre système, jamais dans un fichier.
- La carte n’envoie rien tant que vous ne l’avez pas permis. Ensuite, elle n’envoie que des adresses postales, une à la fois, à OpenStreetMap : jamais un nom, un numéro ni une adresse électronique.
- Personne ne peut emprunter une place sur vos listes. Un courrier qui montre l’adresse d’un contact sûr mais échoue aux vérifications de cette adresse compte comme celui d’un inconnu, et un courrier franchement falsifié est mis de côté. Quelqu’un que vous avez bloqué reste bloqué, quoi que prouve son message.
- Qui est sur quelle liste, et les appels que vos téléphones ont filtrés, voyagent entre vos appareils scellés par votre phrase de passe : le serveur du dossier partagé ne les lit jamais.

## La liste {#the-list}

Les noms viennent d’abord, avec une ligne sous chacun, et un champ de recherche est en haut : quelques lettres d’un nom, d’une organisation, d’une adresse électronique ou d’un numéro de téléphone suffisent à trouver un contact. La recherche ne regarde pas les adresses postales.

Quand vos fiches ont des catégories, un choix sous le champ de recherche montre les contacts d’une seule catégorie, ou **Toutes les catégories**.

## Un contact {#a-contact}

Un contact s’ouvre à droite :

- ses **catégories**, de petites étiquettes sous son nom : un clic sur l’une d’elles montre la liste de cette catégorie ;
- ses **adresses électroniques**, chacune avec **Écrire**, et ses **numéros de téléphone**, chacun avec **Appeler**, qui donne le numéro à votre téléphone, ou sur un ordinateur au programme qui répond aux liens de téléphone ;
- **Comment cette personne vous joint** : qui elle est pour vous, en une phrase («   Neutre : dans votre carnet d’adresses, sur aucune liste.   »), et sa fiche ([plus bas](#how-they-reach-you)) ;
- une petite carte avec une épingle à son adresse, une fois celle-ci placée ([La carte](#the-map)) ;
- replié sous **Plus** : ses adresses postales, son organisation et sa fonction, son anniversaire, ses notes, ses sites web, et le carnet d’adresses où se trouve la fiche ;
- **Lié à cela** : les tâches qui la nomment, les événements où elle est invitée, les notes qui la mentionnent, et ce que vous lui avez lié à la main. Un message s’y montre quand vous le lui avez lié ; la [recherche du courrier](mail.md#searching) trouve le reste de son courrier.

**Pour le changer**, **Modifier**, puis **Enregistrer** : il est modifié sur place. Sur un téléphone, **Retour** quitte le formulaire sans enregistrer. **Pour en ajouter un**, le **+** au-dessus de la liste, ou **Nouveau ▾ ▸ Un contact**. Depuis un message, **Ajouter aux contacts** fait de l’expéditeur un contact en un clic.

**Pour en supprimer un**, **Supprimer** : la suppression attend dix secondes, avec **Annuler**.

**Pour en déplacer un** vers un autre carnet d’adresses, y compris d’un compte à l’autre, choisissez ce carnet sous **Carnet d’adresses**, sur sa fiche. Quand l’autre endroit ne garderait pas quelque chose (Google garde moins qu’un serveur ouvert), Sioul dit quoi, et demande avant de le déplacer.

Quand vous écrivez un message, les adresses se complètent depuis vos contacts.

## Comment on vous joint {#how-they-reach-you}

Chaque personne est sur une liste, et la même liste vaut pour son courrier, ses appels et ses messages dans les autres applications :

- **Sûrs** : la famille, les amis, les collègues que vous choisissez. Vous seul y mettez quelqu’un.
- **Neutres** : toute personne de vos carnets d’adresses, tant que vous n’en décidez pas autrement.
- **Restreints** : les personnes dont vous préférez n’avoir des nouvelles qu’aux moments que vous choisissez.
- **Bloqués** : jamais, sur aucun canal. Leur courrier est mis de côté pour de bon, et leurs appels sont refusés.
- **Inconnus** : toutes les personnes qui ne sont dans aucun de vos carnets d’adresses ni sur aucune liste. Leur premier message attend sur [le Porche](porch.md#the-lanes), dans le filtre des nouveaux expéditeurs, jusqu’à ce que vous les laissiez entrer.

Votre choix pour la personne passe d’abord ; puis ses catégories, quand une liste en nomme une («   Amis   » sont sûrs) ; puis son carnet d’adresses. **Comment … vous joint** ouvre sa fiche : sa liste et pourquoi, **Passe toujours** pour les quelques personnes qui doivent toujours passer, ce qui vous joint de sa part à chaque moment de votre journée, et ses appels du mois, sonnés ou refusés. Quand chaque liste vous joint, c’est à vous de le changer, dans Paramètres ▸ [Ce qui vous joint](notifications.md#by-person) : par défaut, par exemple, le courrier d’une personne neutre vient pendant vos heures de travail et de démarches, et l’appel d’un inconnu part sur votre messagerie dès qu’un de vos téléphones filtre les appels.

## Les catégories {#categories}

Les catégories sont les groupes que montre Nextcloud Contacts : «   Famille   », «   Amis   », «   Voisins   » ; une fiche peut en avoir plusieurs. Dans le formulaire, sous **Catégories**, × en retire une, et le champ qui les suit en ajoute une, choisie parmi celles qu’ont déjà vos fiches ou tapée. «   amis   » et «   Amis   » sont une seule catégorie, écrite comme vos fiches l’ont écrite d’abord.

Elles sont enregistrées dans la fiche elle-même : Nextcloud, votre téléphone et les autres programmes les voient, et une fiche enregistrée dans Sioul garde celles qu’elle avait. Une liste peut nommer une catégorie (Paramètres ▸ [Ce qui vous joint](notifications.md#who-is-on-which-list), «   Les catégories de vos contacts   ») : ses personnes vous joignent alors comme le dit cette liste, par courrier, par téléphone et par les messages des autres applications, sauf si vous avez choisi autre chose pour elles sur leur fiche. Rien ne va sur une liste tout seul, famille et amis compris.

## Les doublons {#duplicates}

**Doublons**, au-dessus de la liste, cherche deux choses, et ne change rien avant votre clic :

- **Un numéro ou une adresse électronique écrits deux fois sur une même fiche.** «   04 65 71 12 34   » et «   +33 4 65 71 12 34   » sont un seul numéro : les espaces, les points et l’indicatif du pays n’y changent rien. Chaque fiche concernée est listée avec ce qui partirait, coché ; **Retirer les doublons** garde un exemplaire de chaque, celui qui est écrit avec son pays, avec ce que les autres en disaient (mobile, travail).
- **Deux fiches qui pourraient être une seule personne** : le même nom (dans n’importe quel ordre, sans tenir compte des majuscules ni des accents), le même numéro, ou la même adresse électronique. Elles viennent une paire à la fois, côte à côte, avec ce qu’elles ont en commun. Choisissez le nom gardé, puis **Fusionner** : une fiche garde tout des deux (numéros, adresses électroniques et postales, sites web, catégories, notes, et la photo, l’organisation et l’anniversaire du nom gardé quand elle en a), et l’autre est supprimée, ici et sur le serveur. **Pas la même personne** les garde à part et ne le redemande jamais sur cet appareil ; **Plus tard** montre la paire suivante.

**Fait récemment** liste ce qui a été nettoyé et fusionné, chacun avec **Annuler**, pendant trente jours : les fiches reviennent comme elles étaient, ici et sur le serveur. Les contacts gardés sur cet appareil seulement sont traités de la même façon.

!!! note "Les numéros écrits sans leur pays"
    «   04 65 71 12 34   » n’a pas de pays : Sioul le lit comme un numéro du pays choisi dans les réglages des contacts, par défaut celui de votre système (la France en français). Cela ne sert qu’à comparer les numéros : vos fiches les gardent tels qu’ils sont écrits.

## La carte {#the-map}

Le bouton de la carte, au-dessus de la liste, montre sur une seule carte toutes les personnes qui ont une adresse postale, une épingle chacune ; une épingle ouvre la fiche.

Pour être placée, une adresse doit être transformée en un point sur la carte. Sioul ne le fait pas de lui-même : la première fois, il demande.

!!! note "Placer vos contacts"
    **Les placer** envoie les adresses postales de vos contacts au géocodeur d’OpenStreetMap (Nominatim), une fois chacune, une par seconde. Rien d’autre ne part avec. Les lieux trouvés sont gardés sur cet appareil, et chaque adresse n’est demandée qu’une fois.

Les images de la carte viennent d’OpenStreetMap, demandées avec mesure et gardées. Vous pouvez donner une autre source dans les réglages.

## Sur un téléphone {#on-a-phone}

Sous Android, Sioul synchronise lui-même vos carnets d’adresses et en garde sa propre copie, à part des contacts du téléphone. Avec votre accord, il lit les contacts du téléphone, et seulement pour trois choses : nommer un appelant que vos carnets d’adresses ne connaissent pas, reconnaître qui a écrit dans la notification d’une autre application, et voir lesquelles des personnes qui passent toujours sont en favori pour le mode «   Ne pas déranger   » d’Android. Il n’écrit jamais un contact ni un favori : pour une personne qui devrait être en favori, il ouvre son contact dans votre application Contacts, ou le propre formulaire de cette application, rempli, pour que vous l’enregistriez. Quand le téléphone filtre vos appels ([Les appels](calls.md)), les appels d’une personne bloquée sont refusés avant que le téléphone sonne.

## Les réglages des contacts {#the-contacts-settings}

Le ⚙ en haut de la page :

- **Carnets d’adresses** : renommés ici, et sur le serveur à la prochaine synchronisation. Un carnet vide peut être supprimé ; un carnet qui contient des contacts reste.
- **Placer les contacts sur la carte** : activé ou désactivé.
- **Tuiles de carte** : d’où viennent les images de la carte, en `https://…/{z}/{x}/{y}.png`. Vide : celles d’OpenStreetMap.
- **Pays des numéros de téléphone écrits sans indicatif** : le pays auquel appartient «   04 65 71 12 34   », pour retrouver le même numéro écrit «   +33 4 65 71 12 34   ». Par défaut, celui de votre système.

## Où vivent les contacts {#where-contacts-live}

Sur votre serveur de contacts, en fiches standard : votre téléphone et les autres programmes les voient. Modifier un contact dans Sioul ne change que ce que vous avez changé : une photo ou les champs d’un autre programme restent tels qu’ils étaient. Quand une fiche a été modifiée ici et sur le serveur entre deux synchronisations, la version du serveur est gardée, et la vôtre est mise de côté parmi les versions précédentes (**Paramètres ▸ Votre dossier et le partage ▸ Voir les versions précédentes**, sous **Vos agendas et contacts**), sur un téléphone aussi, à remettre si vous la préférez ; la ligne d’état le dit.

Les contacts Google marchent de la même façon, en gardant moins : Sioul les écrit comme Google les lit, et dit ce qui serait perdu avant d’y déplacer un contact.

## Pas encore là {#not-there-yet}

Changer la photo d’un contact dans Sioul est prévu. Les fichiers vCard ne s’importent ni ne s’exportent encore d’un bouton : vos fiches sont de simples fichiers `.vcf` dans le dossier de données de Sioul, que les autres programmes lisent. Sioul ne fait pas d’agenda des anniversaires de vos contacts, et ne garde pas de corbeille pour les contacts supprimés au-delà des dix secondes d’**Annuler**.

## Pour aller plus loin {#going-further}

- **Quand deux fiches sont prises pour une seule personne** : un numéro, une adresse électronique ou un nom que partagent plus de quatre fiches (un standard, un bureau, un nom courant) ne dit rien d’une personne, et ne forme donc aucune paire. Les paires qui ont le plus en commun viennent d’abord. Les fiches que vous ne pouvez que lire ne sont jamais nettoyées ni fusionnées.
- **Ce que garde une fusion** : la fiche du nom gardé reste telle qu’elle est, et prend à l’autre ce qui lui manque : numéros, adresses électroniques et postales, sites web, adresses de messagerie instantanée, catégories, surnoms, clés publiques ; les notes des deux, mises bout à bout ; la photo, l’organisation et l’anniversaire de l’autre seulement quand la fiche gardée n’en a pas.
- **Annuler après une synchronisation** : quand une fiche a changé après son nettoyage ou sa fusion (une synchronisation, une modification), sa nouvelle version est mise de côté dans un fichier avant que l’ancienne soit réécrite : rien n’est perdu, dans un sens comme dans l’autre.
- **Qui décide de la liste d’une personne**, le plus précis d’abord : son adresse ou son numéro sur une liste ; sa fiche ; ses catégories ; un domaine ou le début d’un numéro (`*@example.org`, les premiers chiffres d’un standard) ; puis neutre, quand elle est dans un carnet d’adresses ou que vous l’avez laissée entrer. À niveau égal, bloqué passe avant restreint, neutre et sûr.
- **La photo d’une fiche** est lue dans la fiche elle-même. Une fiche dont la photo est une adresse web (`https` seulement) la fait venir de là quand la fiche s’affiche : c’est la seule demande qu’une fiche puisse faire d’elle-même.

## Comparé à d’autres applications {#compared-with-other-apps}

**Trouver les doublons n’est pas propre à Sioul.** Google Contacts propose des fusions sous «   Fusionner et corriger   ». Apple Contacts cherche les doublons sur le Mac, et sur iPhone et iPad trouve les fiches qui ont le même prénom et le même nom. KAddressBook trouve les fiches qui ont le même nom ou le même surnom, ou une adresse électronique en commun, et Proton Contacts propose de fusionner les fiches qui portent le même nom. Ce que Sioul fait à sa façon :

- **Il fusionne les fiches elles-mêmes, sur le serveur qui les garde**, le vôtre ou celui de Google, pour que votre téléphone et toutes les autres applications ne voient ensuite qu’une fiche. KAddressBook le fait aussi ; GNOME Contacts et Fossify ne font que montrer deux fiches comme une seule, dans leur propre fenêtre.
- **Il compare les numéros selon le plan de numérotation de chaque pays**, pour que «   04 65 71 12 34   » et «   +33 4 65 71 12 34   » soient un seul numéro, là où GNOME Contacts compare leurs sept derniers chiffres.
- **Il laisse de côté ce que beaucoup de fiches partagent** : un numéro, une adresse électronique ou un nom présents sur plus de quatre fiches ne forment aucune paire.
- **Il annule chaque fusion à part, pendant trente jours**, ici et sur le serveur, là où Google, et Apple sur iCloud.com, ne peuvent que remettre tout le carnet d’adresses à un moment antérieur, et où KAddressBook, Proton et Nextcloud ne décrivent aucune annulation.

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

=== "Au quotidien"

    | | Sioul | Google | Apple | Outlook | Thunderbird | DAVx⁵ + Fossify |
    |---|---|---|---|---|---|---|
    | Trouve les doublons et les fusionne quand vous le dites | ✓ | ✓ | ✓ | en partie¹ | en partie² | en partie³ |
    | Prend «   04 65 71 12 34   » et «   +33 4 65 71 12 34   » pour un seul numéro | ✓ | ?⁴ | ? | ? | en partie⁵ | en partie⁶ |
    | Annule une fusion plus tard | ✓⁷ | en partie⁸ | en partie⁹ | ?¹⁰ | ✗ | —¹¹ |
    | Montre vos contacts sur une carte | ✓ | en partie¹² | en partie¹² | ? | ✗¹³ | en partie¹⁴ |
    | Des groupes qui vous suivent sur vos autres appareils | ✓ | en partie¹⁵ | ✓ | ✓¹⁶ | en partie¹⁷ | ✓ |
    | Bloque quelqu’un sur tous les canaux à la fois | ✓¹⁸ | en partie¹⁹ | ✓²⁰ | en partie²¹ | en partie²¹ | en partie²² |
    | Laisse passer les personnes choisies aux moments choisis | ✓²³ | en partie²⁴ | ✓ | en partie²⁵ | ✗ | en partie²⁶ |
    | Liste les tâches, les événements et les notes liés à la personne | ✓ | en partie²⁷ | ? | en partie²⁸ | ✗²⁹ | ✗ |
    | Liste le courrier échangé avec elle | ✗³⁰ | en partie²⁷ | ? | ✓ | en partie²⁹ | ✗ |
    | Retrouve un contact supprimé | en partie³¹ | ✓ | en partie⁹ | ✓ | ✗ | ✗ |
    | Importe et exporte des fichiers vCard | ✗³² | ✓ | ✓ | en partie³³ | ✓ | ✓ |
    | Met les anniversaires de vos contacts dans votre agenda | ✗³⁴ | ✓ | ✓ | ✓ | ✗ | ✓³⁵ |

    1. Le nouvel Outlook et Outlook sur le web cachent une fiche qui est la copie exacte d’une autre, ou une partie d’une autre, sans demander et sans fusionner ; Outlook classique ne demande que quand vous enregistrez ou importez un doublon.
    2. Seulement avec le module complémentaire Duplicate Contacts Manager, fait par quelqu’un d’autre : vous modifiez ou supprimez une fiche de chaque paire.
    3. Fossify montre les fiches qui portent le même nom comme une seule entrée, sans demander ; les fiches enregistrées ne sont pas fusionnées.
    4. Google garde la forme internationale de chaque numéro, mais ne dit pas s’il s’en sert pour trouver les doublons.
    5. Avec le module de la note 2, une fois qu’on lui a donné un indicatif de pays ; les numéros sont comparés, jamais réécrits.
    6. Sur l’écran d’un contact, les numéros qui finissent par les mêmes neuf chiffres ne sont montrés qu’une fois.
    7. Chaque nettoyage et chaque fusion à part, pendant trente jours.
    8. «   Annuler les modifications   » remet tout le carnet d’adresses à un moment des 30 derniers jours, en annulant chaque changement fait depuis.
    9. Aucune annulation de fusion ni aucune corbeille ne sont décrites ; iCloud.com peut remettre une copie antérieure de tous vos contacts.
    10. Le nouvel Outlook ne fusionne rien ; Outlook classique ne décrit aucune annulation de sa mise à jour d’un contact existant.
    11. Rien n’est fusionné dans les fiches enregistrées : désactiver le réglage montre de nouveau les fiches séparées.
    12. L’adresse d’un contact s’ouvre dans Google Maps, ou dans Plans d’Apple ; une carte de tous vos contacts n’est pas décrite.
    13. «   Get Map   » a disparu avec le nouveau carnet d’adresses de Thunderbird 102 ; la demande pour le faire revenir est ouverte.
    14. Une adresse postale s’ouvre dans l’application de cartes que vous avez installée.
    15. Les libellés s’affichent dans Gmail et dans l’application Android de Google ; le CardDAV de Google ne les transmet pas aux autres applications.
    16. Entre les applications d’Outlook, sur le même compte Microsoft.
    17. Des listes de diffusion, dans les carnets gardés sur l’ordinateur seulement ; les groupes d’un serveur ne s’affichent pas encore.
    18. Leur courrier est mis de côté, leurs messages dans les autres applications ne viennent jamais, et leurs appels sont refusés sur un téléphone Android qui filtre les appels.
    19. L’application Contacts d’Android bloque les appels et les SMS d’un contact ; Gmail bloque le courrier à part, et l’un ne s’applique pas à l’autre.
    20. Un seul blocage couvre Téléphone, FaceTime, Messages et Mail, sur tous les appareils du même compte Apple.
    21. Le courrier seulement, par une liste de blocage ou un filtre ; il ne gère pas les appels.
    22. Les appels et les SMS, par les numéros bloqués d’Android ; pas le courrier.
    23. Le courrier sur chaque appareil ; les appels et les messages des autres applications sur un téléphone Android où Sioul tourne.
    24. Les modes d’Android laissent passer les appels et les messages de personnes choisies ; le courrier est à part.
    25. Son application pour téléphone coupe ses notifications selon un horaire ; laisser passer des personnes choisies n’est pas décrit.
    26. Le mode «   Ne pas déranger   » d’Android peut laisser passer les contacts en favori.
    27. Pour les comptes professionnels et scolaires seulement : les conversations et les réunions avec les personnes de votre organisation.
    28. La fiche de profil, dans l’application d’Outlook pour téléphone, montre les événements que vous partagez.
    29. Un bouton cherche le courrier du contact, un autre crée un événement avec lui ; la fiche ne liste rien.
    30. Seulement un message que vous lui avez lié ; la recherche du courrier trouve le reste.
    31. **Annuler** pendant dix secondes après **Supprimer** ; pas de corbeille ensuite.
    32. Pas encore de bouton ; vos fiches sont de simples fichiers `.vcf` que les autres programmes lisent.
    33. Outlook classique ouvre et enregistre des vCard une par une ; le nouvel Outlook et le web importent et exportent du CSV seulement.
    34. Sioul ne fait pas d’agenda des anniversaires ; celui que garde votre serveur (Nextcloud en garde un) s’affiche comme tout agenda que vous ne pouvez que lire.
    35. Dans Fossify Calendar ; Etar n’en a pas.

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
    6. Par le propre compte Google d’Android, ou par DAVx⁵, qui le dit «   pas officiellement pris en charge   » ; les libellés ne passent pas.
    7. Les groupes sont des objets à part (les libellés de Google, les groupes d’Apple), pas des mots dans chaque fiche.
    8. Les catégories sont gardées sur chaque contact, dans le format propre à Microsoft.
    9. Les groupes et les catégories d’un serveur ne s’affichent pas encore.
    10. DAVx⁵ garde les groupes en catégories dans chaque fiche, ou en fiches à part, comme vous le choisissez pour chaque compte.
    11. Les contacts d’un serveur peuvent être lus par ce serveur, comme avec toute application CardDAV. Ce que Sioul partage entre vos appareils (qui est sur quelle liste, les appels filtrés, les carnets d’adresses gardés sur un appareil) est scellé.
    12. Pas non plus avec la protection avancée des données : Apple dit que CardDAV n’a pas de chiffrement de bout en bout intégré.
    13. Chiffrés en chemin et sur les disques de Microsoft, avec des clés que Microsoft détient.

**Les autres carnets d’adresses.** GNOME Contacts propose de lier deux fiches qui ont un numéro, une adresse électronique ou un nom proche en commun, se souvient d’un «   non   », et vous laisse les délier à tout moment ; il laisse les deux fiches telles qu’elles sont dans leurs carnets. Nextcloud Contacts fusionne deux fiches que vous choisissez, côte à côte, champ par champ, et garde les groupes dans chaque fiche, comme Sioul les lit ; il ne cherche pas les doublons de lui-même. Proton Contacts chiffre de bout en bout chaque champ d’une fiche, sauf le nom et les adresses électroniques, et n’a pas de CardDAV.

??? info "Sources (en anglais)"
    - Google Contacts : fusionner les doublons («   Fusionner et corriger   »), <https://support.google.com/contacts/answer/7078226>, lu le 8 octobre 2026.
    - Google Contacts : supprimer, restaurer et bloquer des contacts, et annuler des modifications, <https://support.google.com/contacts/answer/7280886>, lu le 8 octobre 2026.
    - Google Contacts : les contacts sous Android, regroupés et libellés, <https://support.google.com/contacts/answer/30970>, lu le 8 octobre 2026.
    - Google Maps : l’adresse d’un contact sur la carte, <https://support.google.com/maps/answer/3131570>, lu le 8 octobre 2026.
    - Compte Google : les utilisateurs bloqués, et ce que couvre un blocage, <https://support.google.com/accounts/answer/6388749>, lu le 8 octobre 2026.
    - Gmail : bloquer un expéditeur, <https://support.google.com/mail/answer/8151>, lu le 8 octobre 2026.
    - Android : les modes et «   Ne pas déranger   », <https://support.google.com/android/answer/9069335>, lu le 8 octobre 2026.
    - Téléphone de Google : le filtrage des appels, <https://support.google.com/phoneapp/answer/9118387>, lu le 8 octobre 2026.
    - Google Workspace Updates : la page d’un contact pour les comptes professionnels (juin 2021), <https://workspaceupdates.googleblog.com/2021/06/update-google-contacts-experience.html>, lu le 8 octobre 2026.
    - Google Agenda : les anniversaires, <https://support.google.com/calendar/answer/6084659>, lu le 8 octobre 2026.
    - API Google People : CardDAV, <https://developers.google.com/people/carddav>, lu le 8 octobre 2026.
    - API Google People : la ressource «   person   » (la forme internationale des numéros), <https://developers.google.com/people/api/rest/v1/people>, lu le 8 octobre 2026.
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
    - Thunderbird : bogue 1781076, «   Get Map   » dans le nouveau carnet d’adresses, <https://bugzilla.mozilla.org/show_bug.cgi?id=1781076>, lu le 8 octobre 2026.
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
    - Fossify Calendar : ses chaînes («   Add contact birthdays   »), <https://github.com/FossifyOrg/Calendar/blob/main/app/src/main/res/values/strings.xml>, lu le 8 octobre 2026.
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
- **Les numéros de téléphone** sont comparés par une clé E.164, faite à partir d’une table des plans de numérotation de 34 pays et territoires (le préfixe national, le préfixe international, la longueur des numéros, les départements d’outre-mer), pas par libphonenumber. Les numéros courts («   112   », «   3631   ») et ceux qui contiennent des lettres sont comparés tels qu’ils sont écrits. Les valeurs ne sont jamais réécrites.
- **Les doublons** : les noms sont comparés sans tenir compte des majuscules, des accents, des ligatures (ß, æ, œ) ni de la ponctuation, leurs mots triés ; un nom, un numéro ou une adresse électronique que partagent de deux à quatre fiches forment des paires, classées selon le nombre de sortes d’indices qu’elles partagent. «   Pas la même personne   » est gardé par UID dans `$XDG_STATE_HOME/sioul/contacts-not-the-same.toml`. Une fusion garde les propriétés à valeur unique de la fiche gardée (`ORG`, `TITLE`, `BDAY`, `PHOTO`, `TZ`, `GEO`…) et ajoute les propriétés à plusieurs valeurs qui lui manquent (`ADR`, `URL`, `IMPP`, `RELATED`, `NICKNAME`, `KEY`, `CALURI`…) ; une ligne vCard 4.0 qui entre dans une fiche 3.0 est convertie. Chaque changement est d’abord noté, avec les fiches avant et après, dans `$XDG_STATE_HOME/sioul/contacts-undo`, pendant trente jours ; une écriture qui échoue défait celles qui la précèdent.
- **Qui est une personne** : son adresse ou son numéro sur une liste, puis sa fiche (`contact:<UID>`), puis ses catégories, puis le motif le plus précis (`*@example.org`, `tel:+3319900*`), puis neutre quand elle est dans un carnet d’adresses ou a été laissée entrer, sinon inconnue ; à précision égale, bloqué avant restreint, neutre et sûr. Les listes sont de simples fichiers texte ; les numéros sont reconnus par leur clé E.164.
- **La preuve avant les listes** : un message dont SPF et DKIM échouent tous les deux, sans DMARC réussi, est jugé comme celui d’un inconnu quelle que soit la liste de son adresse, sauf si un sceau ARC (RFC 8617) de votre propre fournisseur ou de votre domaine s’en porte garant une fois le message transféré. Un échec DMARC sous une politique `quarantine` ou `reject` est jugé falsifié et mis de côté avant cela. Bloqué est vérifié d’abord et vaut toujours. Un nom lu dans la notification d’une autre application ne peut que faire baisser le rang de quelqu’un, jamais l’élever, puisque n’importe qui peut prendre n’importe quel nom dans une discussion.
- **Le géocodage** suit la politique d’utilisation de Nominatim : un User-Agent qui nomme Sioul, une demande toutes les 1,1 seconde, par HTTPS, l’adresse seule (ses lignes mises bout à bout) ; les réponses, trouvées ou non, sont gardées dans `$XDG_CACHE_HOME/sioul/places.toml`. La carte est le greffon OpenStreetMap de Qt Location, avec la recherche de fournisseurs en ligne de Qt désactivée ; la grande carte ne charge des tuiles qu’une fois au moins une adresse placée. Le serveur de tuiles, comme pour toute carte en ligne, voit quelles zones vous regardez.
- **L’affichage d’une fiche** : une fiche vient d’un serveur ou du vCard de n’importe qui, elle est donc montrée sans balisage : les notes en texte brut, les sites web en liens seulement quand ils sont en `http` ou `https`, leur texte échappé. Une photo intégrée est gardée dans le cache, nommée d’après son contenu.
- **Le stockage** : un fichier `.vcf` par fiche dans des dossiers vdir (`~/.local/share/sioul/contacts`), que khard et vdirsyncer lisent aussi ; l’état de la synchronisation est gardé à part. Sioul ne les chiffre pas sur le disque : le chiffrement de votre disque s’en charge. Les dossiers de données, d’état et de cache de Sioul sont créés, ou restreints, au mode 0700 à chaque démarrage sous Linux et macOS, pour que les autres comptes de l’ordinateur ne puissent pas les lire ; sous Android, ils sont dans le stockage privé de l’application.
- **Le partage scellé** : chaque enregistrement est chiffré avec XChaCha20-Poly1305, sous une clé faite à partir de votre phrase de passe par Argon2id et gardée dans le trousseau de chaque appareil ; le serveur du dossier partagé voit quel appareil a écrit, quand et combien, jamais quoi. La partie **Expéditeurs** porte les quatre listes, les expéditeurs laissés entrer, les personnes qui passent toujours et les clés publiques des autres ; **Appels** porte les appels filtrés, jamais ceux d’un numéro bloqué ; **Listes gardées ici** porte les carnets d’adresses gardés sur un seul appareil. Les contacts d’un serveur ne sont pas dans le partage : ils voyagent par leur serveur ([Le partage](sharing.md#what-it-protects-and-what-it-cannot-hide)).
- **Android** : Sioul synchronise les carnets d’adresses dans son stockage privé, pas dans les contacts d’Android. Il ne demande que `READ_CONTACTS`, jamais `WRITE_CONTACTS`. Les appels sont filtrés en tant qu’«   appli numéro de l’appelant et spam   » d’Android (`CallScreeningService`), d’après une table que Rust écrit à l’avance et que Java lit en quelques millisecondes, sans le réseau ; un appel qui ne peut pas être décidé sonne.
