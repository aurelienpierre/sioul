---
description: Les contacts dans Sioul – les noms d’abord, les détails repliés, Écrire et Appeler à côté de chaque adresse, et la carte de tout le monde, placée seulement si vous le permettez.
---

# Les contacts et la carte {#contacts-and-the-map}

Les contacts, ce sont d’abord des noms. Les détails restent repliés jusqu’à ce que vous les ouvriez.

<figure markdown="span">
  [![La page Contacts : un champ de recherche, un bouton pour un nouveau contact et un pour la carte au-dessus d’une liste de noms, chacun avec une adresse ou un numéro dessous ; un contact ouvert à droite, avec sa fonction et son organisation, une adresse électronique avec Écrire, un numéro de téléphone avec Appeler, « Son courrier » réglé sur Neutre, une petite carte avec une épingle à son adresse, et ce qui lui est lié : deux tâches et deux événements.](../assets/screens/fr/contacts.png){ loading=lazy }](../assets/screens/fr/contacts.png "Ouvrir l’image en grand")
  <figcaption>Un contact : comment le joindre, où il est, et ce qui le concerne. <small>Carte © <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> et ses contributeurs.</small></figcaption>
</figure>

## La liste {#the-list}

Des noms, avec une ligne sous chacun, et un champ de recherche en haut : quelques lettres d’un nom, d’une organisation, d’une adresse ou d’un numéro suffisent à les trouver.

## Un contact {#a-contact}

Un contact s’ouvre à droite :

- ses **Adresses électroniques**, chacune avec **Écrire** ;
- ses **Numéros de téléphone**, chacun avec **Appeler** ;
- replié sous **Plus** : adresses postales, organisation et fonction, anniversaire, notes, sites web ;
- **Son courrier** : sûr, neutre ou bloqué, pour chaque adresse de la fiche (voir [le Porche](porch.md#letting-someone-in)) ;
- une petite carte avec une épingle à son adresse, une fois celle-ci placée (plus bas) ;
- **Lié à cela** : le courrier échangé, les tâches, les événements, les notes, les projets.

**Pour le changer**, **Modifier**, puis **Enregistrer** : il est modifié sur place. Sur un téléphone, **Retour** quitte le formulaire sans enregistrer. **Pour en ajouter un**, le **+** au-dessus de la liste, ou **Nouveau ▾ ▸ Un contact**. Depuis un message, **Ajouter aux contacts** fait de l’expéditeur un contact en un clic.

**Pour en supprimer un**, **Supprimer** : la suppression attend dix secondes, avec **Annuler**.

**Pour en déplacer un** vers un autre carnet d’adresses, y compris d’un compte à l’autre, choisissez ce carnet sous **Carnet d’adresses**, sur sa fiche. Quand l’autre endroit ne garderait pas quelque chose (Google garde moins qu’un serveur ouvert), Sioul dit quoi, et demande avant de le déplacer.

Quand vous écrivez un message, les adresses se complètent depuis vos contacts.

## La carte {#the-map}

Le bouton de la carte, au-dessus de la liste, montre sur une seule carte toutes les personnes qui ont une adresse postale, une épingle chacune ; une épingle ouvre la fiche.

Pour être placée, une adresse doit être transformée en un point sur la carte. Sioul ne le fait pas de lui-même : la première fois, il demande.

!!! note "Placer vos contacts"
    **Les placer** envoie les adresses postales de vos contacts au géocodeur d’OpenStreetMap (Nominatim), une fois chacune, une par seconde. Rien d’autre ne part avec. Les lieux trouvés sont gardés sur cet ordinateur, et chaque adresse n’est demandée qu’une fois.

Les images de la carte viennent d’OpenStreetMap, demandées avec mesure et gardées. Vous pouvez donner une autre source dans les réglages.

## Les réglages des contacts {#the-contacts-settings}

Le ⚙ en haut de la page :

- **Carnets d’adresses** : renommés ici, et sur le serveur à la prochaine synchronisation. Un carnet vide peut être supprimé ; un carnet qui contient des contacts reste.
- **Placer les contacts sur la carte** : activé ou désactivé.
- **Tuiles de carte** : d’où viennent les images de la carte, en `https://…/{z}/{x}/{y}.png`. Vide : celles d’OpenStreetMap.

## Où vivent les contacts {#where-contacts-live}

Sur votre serveur de contacts, en fiches CardDAV standard : votre téléphone et les autres programmes les voient. Modifier un contact dans Sioul ne change que ce que vous avez changé : une photo ou les champs d’un autre programme reviennent tels qu’ils étaient. Quand une fiche a été modifiée ici et sur le serveur entre deux synchronisations, la version du serveur est gardée, la vôtre est mise de côté dans un fichier, et la ligne d’état dit où.

Les contacts Google marchent de la même façon, en gardant moins : Sioul les écrit comme Google les lit, et dit ce qui serait perdu avant d’y déplacer un contact.

## Pas encore là {#not-there-yet}

Les groupes de contacts, les photos, ainsi que la recherche et la fusion des doublons sont prévus.
