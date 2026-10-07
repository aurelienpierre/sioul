---
description: Les contacts dans Sioul – les noms d’abord, les détails repliés, Écrire et Appeler à côté de chaque adresse, les catégories comme les groupes de Nextcloud, les doublons trouvés et fusionnés sur votre clic, et la carte de tout le monde, placée seulement si vous le permettez.
---

# Les contacts et la carte {#contacts-and-the-map}

Les contacts, ce sont d’abord des noms. Les détails restent repliés jusqu’à ce que vous les ouvriez.

<figure markdown="span">
  [![La page Contacts : un champ de recherche, un bouton pour un nouveau contact et un pour la carte au-dessus d’une liste de noms, chacun avec une adresse ou un numéro dessous ; un contact ouvert à droite, avec sa fonction et son organisation, une adresse électronique avec Écrire, un numéro de téléphone avec Appeler, « Son courrier » réglé sur Neutre, une petite carte avec une épingle à son adresse, et ce qui lui est lié : deux tâches et deux événements.](../assets/screens/fr/contacts.png){ loading=lazy }](../assets/screens/fr/contacts.png "Ouvrir l’image en grand")
  <figcaption>Un contact : comment le joindre, où il est, et ce qui le concerne. <small>Carte © <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> et ses contributeurs.</small></figcaption>
</figure>

## La liste {#the-list}

Des noms, avec une ligne sous chacun, et un champ de recherche en haut : quelques lettres d’un nom, d’une organisation, d’une adresse ou d’un numéro suffisent à les trouver.

Quand vos fiches ont des catégories, un choix sous le champ de recherche montre les contacts d’une seule catégorie, ou **Toutes les catégories**.

## Un contact {#a-contact}

Un contact s’ouvre à droite :

- ses **catégories**, de petites étiquettes sous son nom : l’une d’elles montre la liste de cette catégorie ;
- ses **Adresses électroniques**, chacune avec **Écrire** ;
- ses **Numéros de téléphone**, chacun avec **Appeler** ;
- replié sous **Plus** : adresses postales, organisation et fonction, anniversaire, notes, sites web ;
- **Comment cette personne vous joint** : qui elle est pour vous, en une phrase (« Neutre : dans votre carnet d’adresses, sur aucune liste. »), et **Comment … vous joint**, sa fiche : sa liste, pour toutes ses adresses et tous ses numéros, sur toute fiche, même avec un numéro seul ; **Passe toujours** ; et ce qui vous joint de sa part à chaque moment ([La fiche d’une personne](notifications.md#a-persons-sheet)). Toute personne de vos carnets d’adresses est neutre tant que vous ne choisissez pas ; ses catégories peuvent en décider, quand une liste en nomme une ; votre choix pour cette personne passe avant : sûr, neutre, restreint ou bloqué, ou de nouveau **Comme le disent ses catégories** ;
- une petite carte avec une épingle à son adresse, une fois celle-ci placée (plus bas) ;
- **Lié à cela** : le courrier échangé, les tâches, les événements, les notes, les projets.

**Pour le changer**, **Modifier**, puis **Enregistrer** : il est modifié sur place. Sur un téléphone, **Retour** quitte le formulaire sans enregistrer. **Pour en ajouter un**, le **+** au-dessus de la liste, ou **Nouveau ▾ ▸ Un contact**. Depuis un message, **Ajouter aux contacts** fait de l’expéditeur un contact en un clic.

**Pour en supprimer un**, **Supprimer** : la suppression attend dix secondes, avec **Annuler**.

**Pour en déplacer un** vers un autre carnet d’adresses, y compris d’un compte à l’autre, choisissez ce carnet sous **Carnet d’adresses**, sur sa fiche. Quand l’autre endroit ne garderait pas quelque chose (Google garde moins qu’un serveur ouvert), Sioul dit quoi, et demande avant de le déplacer.

Quand vous écrivez un message, les adresses se complètent depuis vos contacts.

## Les catégories {#categories}

Les catégories sont les groupes que montre Nextcloud Contacts : « Famille », « Amis », « Voisins » ; une fiche peut en avoir plusieurs. Dans le formulaire, sous **Catégories**, × en retire une, et le champ qui les suit en ajoute une, choisie parmi celles qu’ont déjà vos fiches ou tapée. « amis » et « Amis » sont une seule catégorie, écrite comme vos fiches l’ont écrite d’abord.

Elles sont enregistrées dans la fiche elle-même (le `CATEGORIES` du vCard) : Nextcloud, votre téléphone et les autres programmes les voient, et une fiche enregistrée dans Sioul garde celles qu’elle avait. Une liste peut nommer une catégorie (Paramètres ▸ [Ce qui vous joint](notifications.md#who-is-on-which-list), « Les catégories de vos contacts ») : ces personnes vous joignent alors comme le dit cette liste, par courrier, par téléphone et par les messages des autres applications, sauf si vous avez choisi autre chose pour elles sur leur fiche. Rien ne va sur une liste tout seul, famille et amis compris.

## Les doublons {#duplicates}

**Doublons**, au-dessus de la liste, cherche deux choses, et ne change rien avant votre clic :

- **Un numéro ou une adresse écrits deux fois sur une même fiche.** « 04 65 71 12 34 » et « +33 4 65 71 12 34 » sont un seul numéro : les espaces, les points et l’indicatif du pays n’y changent rien. Chaque fiche concernée est listée avec ce qui partirait, cochée ; **Retirer les doublons** garde un exemplaire de chaque, celui qui est écrit avec son pays, avec ce que les autres en disaient (mobile, travail).
- **Deux fiches qui pourraient être une seule personne** : le même nom (dans n’importe quel ordre, sans tenir compte des majuscules ni des accents), le même numéro, ou la même adresse. Elles viennent une paire à la fois, côte à côte, avec ce qu’elles ont en commun. Choisissez le nom gardé, puis **Fusionner** : une fiche garde tout des deux (numéros, adresses, sites web, catégories, notes, et la photo, l’organisation et l’anniversaire du nom gardé quand elle en a), et l’autre est supprimée, ici et sur le serveur. **Pas la même personne** les garde à part et ne le redemande jamais ; **Plus tard** montre la paire suivante.

**Fait récemment** liste ce qui a été nettoyé et fusionné, chacun avec **Annuler**, pendant trente jours : les fiches reviennent comme elles étaient, ici et sur le serveur. Les contacts gardés sur cet appareil seulement sont traités de la même façon.

!!! note "Les numéros écrits sans leur pays"
    « 04 65 71 12 34 » n’a pas de pays : Sioul le lit comme un numéro du pays choisi dans les réglages des contacts, par défaut celui de votre système (la France en français). Cela ne sert qu’à comparer les numéros : vos fiches les gardent tels qu’ils sont écrits.

## La carte {#the-map}

Le bouton de la carte, au-dessus de la liste, montre sur une seule carte toutes les personnes qui ont une adresse postale, une épingle chacune ; une épingle ouvre la fiche.

Pour être placée, une adresse doit être transformée en un point sur la carte. Sioul ne le fait pas de lui-même : la première fois, il demande.

!!! note "Placer vos contacts"
    **Les placer** envoie les adresses postales de vos contacts au géocodeur d’OpenStreetMap (Nominatim), une fois chacune, une par seconde. Rien d’autre ne part avec. Les lieux trouvés sont gardés sur cet appareil, et chaque adresse n’est demandée qu’une fois.

Les images de la carte viennent d’OpenStreetMap, demandées avec mesure et gardées. Vous pouvez donner une autre source dans les réglages.

## Les réglages des contacts {#the-contacts-settings}

Le ⚙ en haut de la page :

- **Carnets d’adresses** : renommés ici, et sur le serveur à la prochaine synchronisation. Un carnet vide peut être supprimé ; un carnet qui contient des contacts reste.
- **Placer les contacts sur la carte** : activé ou désactivé.
- **Tuiles de carte** : d’où viennent les images de la carte, en `https://…/{z}/{x}/{y}.png`. Vide : celles d’OpenStreetMap.
- **Pays des numéros de téléphone écrits sans indicatif** : le pays auquel appartient « 04 65 71 12 34 », pour retrouver le même numéro écrit « +33 4 65 71 12 34 ». Par défaut, celui de votre système.

## Où vivent les contacts {#where-contacts-live}

Sur votre serveur de contacts, en fiches CardDAV standard : votre téléphone et les autres programmes les voient. Modifier un contact dans Sioul ne change que ce que vous avez changé : une photo ou les champs d’un autre programme reviennent tels qu’ils étaient. Quand une fiche a été modifiée ici et sur le serveur entre deux synchronisations, la version du serveur est gardée, la vôtre est mise de côté dans un fichier, et la ligne d’état dit où.

Les contacts Google marchent de la même façon, en gardant moins : Sioul les écrit comme Google les lit, et dit ce qui serait perdu avant d’y déplacer un contact.

## Pas encore là {#not-there-yet}

Changer la photo d’un contact dans Sioul est prévu.
