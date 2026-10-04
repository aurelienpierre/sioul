---
description: La santé dans Sioul – des médicaments rappelés une fois et discrètement, les ordonnances et leurs démarches, ce qu’une montre a mesuré sans aucun score, des pauses pour bouger, et une limite par jour pour les discussions.
---

# La santé {#health}

Des médicaments à prendre, des ordonnances à renouveler, une pause pour bouger, une limite pour les discussions. Tout reste sur cet ordinateur ; rien ne compte ce qui a été manqué, rien ne devient rouge.

<figure markdown="span">
  [![La page Santé : une phrase qui dit que médicaments, ordonnances et prises restent sur cet ordinateur ; les prises du jour, l’une marquée « Pris à 07:41 », les autres chacune avec Pris ; les médicaments, chacun avec le moment où il se prend ; une ordonnance, avec quand passer à la pharmacie et quand la renouveler, « Retiré aujourd’hui », et la liste où vont les démarches ; puis la pause pour bouger pendant la concentration, toutes les 45 minutes.](../assets/screens/fr/health.png){ loading=lazy }](../assets/screens/fr/health.png "Ouvrir l’image en grand")
  <figcaption>Les prises du jour d’abord, puis les médicaments et les ordonnances.</figcaption>
</figure>

La page met d’abord ce qui sert le plus : les prises prévues pendant que Sioul était fermé, puis les prises du jour, puis les médicaments et les ordonnances, puis ce que dit votre montre (une fois qu’elle est réglée), puis les pauses.

## Les médicaments {#medicines}

**Ajouter un médicament** : son **Nom**, sa **Dose** (« un comprimé »), et **Quand** :

- **À heures fixes chaque jour** : « 08:00, 20:00 » ;
- **Tous les quelques jours**, à une heure donnée, à partir d’un jour donné : « un jour sur deux à 08:00 » ;
- **Toutes les quelques heures**, à partir d’une heure donnée : « toutes les 6 heures, à partir de 18:30 ».

Puis **Jusqu’au** : un jour, ou aussi longtemps que le traitement dure ; l’**Ordonnance** d’où il vient ; **En pause pour l’instant**.

### Aujourd’hui {#today}

Chaque prise du jour, avec **Pris**. Une prise faite dit à quelle heure ; un clic l’annule. Une prise non marquée est simplement non marquée.

### Les rappels {#reminders}

Une notification sur le bureau par prise, dans la demi-heure qui suit son heure, sans son, avec **Pris**. Jamais répétée. Les rappels viennent aussi pendant le calme : ils sont à vous.

Si Sioul était fermé à ce moment-là, une prise des douze dernières heures qui n’a été ni marquée ni rappelée fait l’objet d’une seule question : « Avez-vous pris … ? », avec **Pris** et **Pas pris**. C’est une question sur le passé, jamais un rappel d’en prendre une maintenant.

### Sur plusieurs ordinateurs {#on-several-computers}

Seul l’ordinateur où vous êtes vous fait le rappel. Une prise marquée sur l’un passe tout de suite aux autres. Pour cela, [partagez entre vos ordinateurs](sharing.md) ; sans cela, la page dit que les prises ne sont connues que de cet ordinateur.

## Les ordonnances {#prescriptions}

**Ajouter une ordonnance** : ce qu’elle prescrit, qui l’a écrite, jusqu’à quand elle est valable, combien de jours la pharmacie donne à la fois, et la date du dernier retrait.

Sioul crée alors les démarches, une fois chacune :

- deux jours avant que les médicaments ne manquent, une tâche « Pharmacie : … » ;
- deux semaines avant la fin de l’ordonnance, une tâche « Médecin : renouveler l’ordonnance de … ».

Elles vont dans la liste que vous choisissez sous **Les démarches vont dans**, pour que votre téléphone les ait. **Retiré aujourd’hui** compte le prochain passage à partir d’aujourd’hui.

!!! note "Ces démarches sont des tâches"
    Les titres des démarches nomment le médicament, et elles vont dans votre liste de tâches, sur votre serveur d’agenda quand la liste s’y trouve. Choisissez une liste « sur cet ordinateur seulement » si vous préférez les garder ici.

## Votre montre {#your-watch}

Ce qu’une montre Garmin a mesuré, lu dans ses propres fichiers, jamais par un compte Garmin : rien ne part vers un serveur.

**Pour la régler**, dans le ⚙ de la page Santé, **Ses fichiers arrivent dans** : un dossier où arrivent les exports de Gadgetbridge (l’application compagnon libre pour Android) ou les fichiers d’export de Garmin. Ou branchez la montre : quand votre bureau montre son dossier `GARMIN`, Sioul le lit.

La page dit alors, en mots : quand la montre a donné des données pour la dernière fois, les pas du jour, la fréquence cardiaque au repos et sa valeur habituelle, le sommeil de la nuit dernière (sa durée et ses heures), Body Battery, les moyennes de la semaine ; puis les courbes du jour, sobres. Pas d’objectif, pas de série, pas de score, et pas de couleur en guise de note.

**Des propositions douces entre deux tâches**, actives sauf si vous les décochez : une pause après une longue période sans bouger, une courte pause, une marche quand il y a de la place pour elle, ou finir la journée quand la réserve est basse. Seulement à un arrêt naturel (une tâche faite, une séance de concentration finie), jamais pendant le calme, six par jour au plus, et une proposition refusée attend plus longtemps avant de revenir, à chaque fois. Une notification à ce sujet ne dit aucun chiffre.

Le matin, la page Tâches peut dire une ligne, jamais une notification : après une nuit courte, « Des séances plus courtes aujourd’hui, et la tâche la plus dure tôt, ou demain ? » ; quand la fréquence cardiaque au repos est bien au-dessus de l’habitude, « Votre corps lutte peut-être contre quelque chose. Une journée plus légère ? ». Chacune vient avec **Une journée plus légère**, qui règle [la météo du jour](tasks.md#how-is-today) sur brume ou brouillard.

## Bouger {#moving}

**Une pause pour bouger**, toutes les 45 minutes sauf si vous changez ce réglage : une notification discrète dit qu’il est temps de bouger et de s’étirer, même la fenêtre cachée. Pendant une séance de concentration, la séance elle-même se met en pause quelques minutes ; **Y retourner** la relance.

## Les discussions {#chats}

**Une limite par jour pour les discussions**, désactivée sauf si vous l’activez. Une fois les minutes choisies passées (comptées pendant qu’une discussion est devant vous), les discussions de [Sites](sites.md) sont couvertes, muettes et silencieuses, pendant le temps choisi. Puis elles reviennent d’elles-mêmes, quoi qu’il arrive.

## Où tout est gardé {#where-it-is-kept}

Sur cet ordinateur, dans deux fichiers des dossiers propres à Sioul : ce que vous saisissez, et les prises marquées. Ils ne vont nulle part, sauf si vous partagez entre vos ordinateurs : ils voyagent alors scellés, par votre propre dossier synchronisé ([Le partage](sharing.md)).
