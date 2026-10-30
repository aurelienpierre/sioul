---
description: L’agenda dans Sioul – ce qui vient à partir d’aujourd’hui, le jour, la semaine et le mois ; le temps d’y aller et d’en revenir autour de chaque événement, gardé libre par votre plan ; deux événements en même temps signalés ; un seul rappel discret ; les invitations répondues, rien d’ajouté avant votre réponse ; comparé aux autres agendas.
---

# L’agenda {#agenda}

## En bref {#in-short}

L’agenda montre ce qui vient, à partir d’aujourd’hui, en une liste calme de jours, avec le jour, la semaine et le mois à un clic. Chaque événement peut garder du temps avant lui, pour se préparer et y aller, et après lui, pour en revenir : votre plan garde ce temps libre, place vos tâches autour de vos événements, et décale un repas qui tombe pendant l’un d’eux. Sioul vous dit quand deux événements se chevauchent, et vous fait un seul rappel, discret, avant qu’il faille partir. Vos événements restent sur votre propre serveur d’agenda, chez Google ou sur cet appareil, et rien d’une invitation n’entre dans votre agenda avant que vous y répondiez.

<figure markdown="span">
  [![L’Agenda sur « Semaine » : en haut, Aujourd’hui, deux flèches et « Semaine du 5 octobre », le choix de la vue, et Nouvel événement ; en dessous, sept colonnes de jours à partir du lundi, les événements placés à leur heure, et une fine ligne qui marque l’heure actuelle.](../assets/screens/fr/agenda-week.png){ loading=lazy }](../assets/screens/fr/agenda-week.png "Ouvrir l’image en grand")
  <figcaption>La semaine, à un clic de la liste.</figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

- Vos événements restent sur votre serveur d’agenda, chez Google, ou sur cet appareil. Sioul n’a pas de serveur à lui : votre plan, vos rappels et la recherche de deux événements en même temps se calculent sur votre appareil.
- Sioul ne joint vos agendas que par une connexion chiffrée, et votre mot de passe reste dans le trousseau de votre système.
- Rien d’une invitation n’entre dans votre agenda avant que vous y répondiez. Un courriel franchement falsifié est mis de côté, invitation comprise, et un courriel qui échoue aux vérifications de l’adresse qu’il montre compte comme celui d’un inconnu, quel que soit le nom qu’il affiche. Certains serveurs ajoutent eux-mêmes les invitations à votre agenda ; Sioul montre alors ce que le serveur contient.
- Votre réponse à une invitation part de votre propre compte de courriel vers la personne qui organise, et ne porte que ce dont celle-ci a besoin : l’événement, son heure et votre réponse, jamais les autres personnes invitées.
- Les notes d’un événement s’affichent à partir de leur Markdown, sans risque, qui que soit leur auteur : le HTML qu’elles contiennent s’affiche en mots et aucune image n’est dessinée, donc rien ne se charge depuis le réseau, et rien ne s’exécute. Un lien s’ouvre dans votre navigateur, jamais dans Sioul, et dit d’abord où il mène.

## Quatre façons de regarder {#four-ways-to-look}

Un choix en haut de la page :

- **Ce qui vient** : aujourd’hui en entier, puis les deux prochaines semaines, en liste. Aujourd’hui montre ce qui reste de ses rendez-vous et de ses tâches fixées, ses repas et sa sieste, ses dates, la nuit de ce soir en dernière ligne, et la prochaine étape du plan avec la suivante (« Le plan : Appeler la CAF, puis Trier les reçus »). Ses prises sont listées comme le Porche les liste, une ligne par minute (« 20:00 · Deux prises »), jamais par leur nom et sans rien à presser : un toucher les ouvre dans Santé. Chaque autre jour ne montre que ce qui diffère de vos jours habituels : ses rendez-vous, ses dates, un repas déplacé ou ajouté ce jour-là, une nuit sans son réveil, un médicament pris tous les quelques jours, et la première étape du plan. Un toucher sur la ligne du plan ouvre ce jour. C’est là que la page s’ouvre.
- **Jour**, **Semaine**.
- **Mois** : six semaines de jours. Sur un ordinateur, chaque jour montre d’abord ses dates, puis ses rendez-vous, trois lignes au plus, puis « … » ; un toucher sur une ligne l’ouvre, un toucher sur le numéro du jour ouvre ce jour. Les jours de congé sont grisés. Sur un téléphone, chaque jour montre son numéro et deux marques au plus, un losange pour une date demandée et un point pour les rendez-vous, et sous le mois « Dates et rendez-vous de ce mois » les liste jour par jour à partir d’aujourd’hui ; un toucher sur un jour l’amène en haut de cette liste. Le Mois ne montre ni étape, ni repas, ni prise.

Les flèches ◂ et ▸ vont plus tôt et plus tard ; **Aujourd’hui** revient à aujourd’hui. Rien n’est en retard ni dépassé : le passé ne s’affiche que si vous y retournez.

La page suit l’horloge : la ligne de l’heure actuelle avance avec elle, et aujourd’hui devient demain à minuit. La page relit vos agendas toutes les cinq minutes et chaque fois que vous revenez dans Sioul ; votre serveur d’agenda est interrogé tous les quarts d’heure, et aussitôt après un changement que vous faites.

## Les événements {#events}

**Nouvel événement**, en haut de la page (ou **Nouveau ▾ ▸ Un événement**) : un titre, quand il commence et finit, où. **Toute la journée** se coche d’un clic. Repliés sous **Plus** : ses notes ; **Se répète**, tous les jours, toutes les semaines, tous les mois ou tous les ans ; **Agenda**, celui où il va, quand vous en avez plusieurs ; et, si vous le voulez :

- **Avant** et **Après** : le temps de se préparer et d’y aller, et celui d’en revenir ([plus bas](#time-around-an-event)).
- **Rappel** : quand Sioul vous le rappelle : comme d’habitude (un quart d’heure avant lui et le temps d’y aller, sauf si vous le changez dans les [Paramètres](settings.md#reminders)), pas pour celui-ci, ou de 5 minutes à 2 heures avant. Voir [Les rappels](#reminders).
- Ce que l’événement vous demande, et ce qu’il apporte : les mêmes tuiles que pour [une tâche](tasks.md#what-it-costs-and-what-it-gives-back), **Réflexion**, **Émotions**, **Inquiétude**, **Corps et sens** et **Ce que ça apporte**, chacune de 0 à 10, avec un mot à côté du nombre et une question en petit. Un appui sur une case donne sa valeur, le même appui l’efface ; chaque tuile dit **non noté** tant que vous ne l’avez pas touchée. Votre plan en tient compte ([plus bas](#your-day-around-your-events)).

Un événement s’ouvre dans sa carte : à côté de la page sur un ordinateur (sur sa partie droite quand la page a besoin de sa place), toute la page sur un téléphone. Elle dit son jour, son heure, son lieu, son agenda, et s’il se répète ; **Modifier** ouvre son formulaire, et là où il ne peut pas changer (un agenda que vous pouvez seulement lire, une réunion avec d’autres), la carte dit pourquoi à la place. Ses notes, qui l’organise et les personnes invitées sont repliés sous **Plus**. **Ajouter des étapes et une liste** fait une tâche qui le prépare ([Un rendez-vous et ses étapes](tasks.md#the-plan)) ; une fois là, les détails la nomment, avec son étape suivante. Une liste à cocher dans ses notes (« À emporter : - [ ] Carte Vitale ») s’y coche, sauf pour une réunion avec d’autres : sa liste est alors celle de son organisateur ([Une liste à cocher](tasks.md#a-list-to-tick)). Il s’ouvre de même depuis un lien (le lien d’une tâche, la journée de la page Tâches), ses jours montrés ; son menu (clic droit, ou appui long sur un écran tactile) propose d’abord **Détails**, puis **Modifier**.

- **Pour le changer** : **Modifier**.
- **Pour le supprimer** : **Supprimer**. Il attend dix secondes, avec **Annuler**. Un événement qui se répète demande d’abord : **Cette fois seulement**, ou **Toutes les fois**.
- **Une réunion avec d’autres**, un événement avec des personnes invitées (vous les avez invitées, ou elles vous ont invité) : son heure, son titre, son lieu, ses notes et son agenda sont ceux de son organisateur, car son serveur préviendrait chaque invité d’un changement. Sioul ne la fait ni glisser ni supprimer, et son formulaire ne change que ce qui est à vous : le temps gardé avant et après, ses coûts et ce qu’elle vous rend, votre rappel. Un événement qui ne nomme qu’un organisateur, comme Google l’écrit dans vos propres événements, reste à vous de le changer.

Un événement peut aussi naître d’un courriel, d’une tâche, ou du rendez-vous d’une lettre papier, et il lui reste lié. Depuis un événement, vous pouvez faire une note (datée, avec la liste des personnes invitées, et **Envoyer aux personnes invitées** une fois écrite) ou une tâche pour le préparer.

## Le temps de se préparer, d’y aller et d’en revenir {#time-around-an-event}

**Avant**, c’est le temps de se préparer et d’y aller ; **Après**, celui d’en revenir ; de 5 minutes à 2 heures chacun. Votre plan garde ce temps libre et ne le compte jamais comme une pause, et la journée le montre autour de l’événement (« Autour : … »). Le rappel de Sioul vient avant lui, et deux événements dont les trajets se chevauchent sont dits se chevaucher.

Avec un temps gardé avant lui, un rendez-vous dit quand partir, sous son titre dans la journée et dans sa carte : « Partir à 13:05 · Ensuite : vérifier l’horaire du bus ». Quand des étapes le préparent ([Un rendez-vous et ses étapes](tasks.md#the-plan)), sa carte les dit sous **Avant** (la suivante, puis celle d’après, les autres repliées) et liste ce qu’il faut emporter sous **À emporter**, que vous pouvez cocher là. Le jour du rendez-vous, ses étapes sont posées l’une après l’autre pour que la dernière finisse quand vous partez, quand rien d’autre n’est gardé à ce moment. Rien ne vous demande de cocher quoi que ce soit, et une étape laissée ouverte ne coûte rien.

**Un trajet** autour de lui (un bus, la marche depuis l’arrêt) : **Ajouter un trajet…** dans sa carte ouvre le formulaire d’événement pour lui, avant ou après le rendez-vous, avec **D’après un horaire** quand son heure vient d’un horaire. C’est un événement à part entière, dans l’agenda du rendez-vous, et il se déplace avec le rendez-vous : déplacez le rendez-vous, ou changez son heure, et ses trajets bougent d’autant, sous le même **Annuler** ; un bus déplacé ainsi est dit « horaire non vérifié » jusqu’à ce que vous fixiez son heure vous-même. Déplacez le trajet lui-même et il reste là où vous le posez. Supprimer le rendez-vous supprime ses trajets à venir, sous le même **Annuler**. « Partir à » est alors le début du premier trajet, le rappel avant lui vient une fois pour toute la sortie, compté depuis lui, et la carte dit **Y aller** (les trajets avant lui) et **Au retour** (les trajets après lui, puis les étapes qui viennent après lui). Dans la journée et la semaine, un trajet est une ligne discrète dans l’avant du rendez-vous ; **Afficher ▸ Chaque trajet comme un événement à part** dessine chacun comme un événement. Vos autres applications d’agenda le montrent comme l’événement ordinaire qu’il est ; déplacé là-bas, il est remis en place la prochaine fois que Sioul le lit. × dans la carte d’un trajet le détache de son rendez-vous. Un trajet peut aussi être l’heure d’une étape (**Son étape** dans son formulaire : « Prendre le bus », une étape de la tâche qui prépare le rendez-vous), une fois que chacun de vos appareils a un Sioul qui garde un tel trajet ; d’ici là, le formulaire le dit. Un rendez-vous qui se répète, ou qui dure toute la journée, ne prend pas de trajet.

**Une réunion avec d’autres** (quelqu’un d’autre l’organise, ou quelqu’un d’autre est invité) garde ses propres lignes telles que son organisateur les a mises : Sioul ne change jamais son heure, son titre, son lieu, ses notes ni ses invités, son serveur dirait tout changement à chaque invité. Le **Modifier** de sa carte ouvre **Votre temps autour** : le temps gardé avant et après, ce qu’elle coûte et ce qu’elle rend, votre rappel. Ils sont gardés dans un fichier à vous, à côté de vos autres données, et vont vers vos autres appareils avec votre temps, dès que chacun a cette version de Sioul (Paramètres ▸ Votre dossier et le partage nomme un appareil qui les garde ici ; un appareil dont vous ne vous servez plus peut y être oublié) ; votre plan, vos rappels et « Partir à » les lisent là. Tant que son invitation est encore dans vos courriels, sa carte propose **Répondre…** : oui, peut-être ou non, envoyé à son organisateur depuis le compte où l’invitation est arrivée.

## Les dates qui vous sont demandées {#dates}

Les dates où quelque chose vous est demandé s’affichent à leur jour, à partir d’aujourd’hui : la date demandée d’une tâche (à son heure quand elle en a une), la date demandée d’un courrier et son rendez-vous, le dernier jour pour résilier un contrat qui se renouvelle chaque année, le dernier jour de validité d’un papier, un paiement prévu, une facture que vous avez émise et qui n’est pas payée ; et, sur votre écran seulement, le jour où une ordonnance prend fin et celui où un médicament s’épuise, nommés par ce qu’ils sont. Chacune est un petit losange en tête de son jour (avec ses mots dans le Jour : « dossier CAF · Date demandée · encore quatre jours »), plein à sept jours ou moins, vide au-delà ; une facture, de l’argent qu’on vous doit, n’est jamais pleine. Rien n’est rouge, rien n’est compté, et une fois son jour passé une date quitte l’agenda : sa carte et sa page la gardent. Une date ne s’affiche qu’une fois : celle qu’a écrite le rendez-vous d’une tâche laisse la place au rendez-vous, celle d’un courrier à sa tâche ou à l’événement qui tient son rendez-vous, celle d’un papier à sa tâche de renouvellement. En temps de travail et d’administratif, la ligne au-dessus d’aujourd’hui nomme la date la plus proche de la semaine : « Date demandée : dossier CAF, vendredi 30 octobre : encore quatre jours. » Le soir, elle parle de demain.

Un toucher sur une date ouvre sa carte là où vous êtes, avec les mots de sa page et ses propres actions : pour un courrier **Voir le scan**, **Une tâche pour cette date**, **Dans l’agenda**, **Fini, classé** ; pour un papier **Prévoir le renouvellement** ; pour une facture **Payée** ; pour une ordonnance **Retiré aujourd’hui**. **Modifier** ouvre le formulaire propre à cette chose. Les dates d’un courrier sont lues sur son scan : tant qu’il n’a ni sa tâche ni son événement, son **Modifier** ouvre un petit formulaire où vous les corrigez. « Dans l’agenda » écrit l’adresse du courrier dans l’événement qu’il crée, pour que cet événement tienne le rendez-vous du courrier sur chaque appareil ; un rendez-vous à vous ce jour-là, à moins d’une heure de celui du courrier, compte aussi, et la carte du courrier dit lequel (« Dans l’agenda : Médecin, 14:00 »).

## Jour et Semaine : chaque chose à son heure {#day-and-week}

**Jour** et **Semaine** montrent tout ce qui a une heure, sur une seule ligne du temps : vos rendez-vous, à la couleur de leur agenda ; les heures de vos tâches ; les étapes que pose votre plan, entourées d’un trait, la suivante remplie ; ce que vous avez fini, coché et estompé ; vos repas, siestes et nuit tels que [Santé](health.md#meals-rest-and-sleep) les a (nommés dans le jour, en bandes dans la semaine) ; et vos prises. Le temps gardé autour d’une chose (se préparer, y aller, en revenir, se préparer à dormir) s’affiche plus clair autour d’elle. Les jours à venir, pendant vos heures calmes, les étapes que la page Tâches met de côté s’affichent en une bande par plage d’heures ; un toucher sur une bande, ou **Afficher ▸ Les étapes mises de côté pour l’instant**, les dessine.

Les heures affichées commencent une demi-heure avant votre réveil et finissent une demi-heure après votre coucher, les mêmes pour chaque jour, pour que les lignes des heures restent en place. Un rendez-vous en dehors les élargit. Le reste de ce qui tombe dans la nuit, une prise à 05:00 par exemple, se place au bord du haut ou du bas avec son heure.

Tant qu’aujourd’hui s’affiche, une ligne sous l’en-tête dit à quoi sert ce moment et ce qui vient ensuite (« Travail jusqu’à 17:00. Ensuite : 19:00 Dîner. »), avec l’heure de partir pour un rendez-vous.

**Les prises** s’affichent en petites marques à côté de chaque jour, jamais avec le nom du médicament : un cercle vide avant son heure, un cercle une fois l’heure venue, plein une fois prise, en pointillés pendant que Sioul lit ce que savent vos appareils, un cercle de couleur chaude quand il vaut mieux vérifier avant de la prendre. Sioul n’affiche jamais une prise comme « non prise » : ce qu’un appareil ne peut pas savoir, il le montre comme un doute. Les jours à venir montrent de petits traits, et les jours passés n’en montrent aucun. Touchez une marque pour ouvrir ce jour dans [Santé](health.md), où le médicament est nommé.

Un toucher ouvre ce que vous avez touché dans sa carte, là où vous êtes : celle d’un rendez-vous ; pour une étape, le temps d’une tâche ou une tâche faite, celle de sa tâche, avec l’endroit où le plan la place ou l’heure où elle est fixée ; celle d’un repas, d’une sieste ou de la nuit, avec ce qui le change ce jour-là, comme dans [Santé](health.md). Une prise ouvre son jour dans Santé, où son médicament est nommé. Depuis une carte, un rendez-vous ou une tâche qui lui est lié s’ouvre dans sa propre carte, et **Retour** (ou Échap) revient à la carte d’où vous venez, puis la ferme ; un message, une note ou un contact s’ouvre là où il vit (sur un téléphone, la carte se ferme d’abord). Un formulaire fermé sans enregistrer garde ce que vous avez tapé jusqu’à ce que vous le rouvriez, et le dit ; ce que vous n’avez pas tapé suit le rendez-vous tel qu’il est alors (une heure déplacée depuis reste déplacée). **Annuler** l’oublie. **Afficher**, en haut, choisit ce que chaque vue montre sur cet appareil (le jour, la semaine, Ce qui vient et le mois) : les étapes du plan, les repas, siestes et nuit, les prises, chaque agenda (deux agendas du même nom restent distincts).

Au clavier, une fois la ligne du temps cliquée : Haut et Bas vont d’une chose à l’autre, Entrée l’ouvre, Ctrl+E ouvre son formulaire, et Page précédente et Page suivante vont au jour ou à la semaine d’avant et d’après.

## Déplacer un événement à la main {#moving-an-event-by-hand}

Dans **Jour** et **Semaine**, faites glisser un événement vers une autre heure : il garde sa durée. Dans **Semaine**, vers un autre jour aussi. Faites glisser son bord du bas pour changer l’heure à laquelle il finit. Pendant que vous le faites glisser, sa nouvelle place s’affiche avec ses heures (« mar. 6, 11:15–12:15 »), de cinq en cinq minutes.

Lâchez-le : il est enregistré et envoyé à votre serveur d’agenda ; **Annuler** attend dix secondes dans la ligne d’état. Lâché là où il était, rien ne change.

- **Un événement qui se répète** demande d’abord : **Cette fois seulement**, ou **Toutes les fois**. Toutes les fois déplace tous ses jours d’autant ; un événement de chaque semaine glissé vers un autre jour vient ce jour-là chaque semaine. Un événement qui se répète à des jours fixés du mois (« le premier lundi ») ne se déplace que cette fois : changez sa répétition dans son formulaire.
- **Une étape de votre plan**, glissée, est fixée à sa nouvelle heure, comme [Tâches](tasks.md#pinned-to-a-time) la fixe ; jamais avant maintenant.
- **Un repas, une sieste ou la nuit**, glissés, changent pour ce jour-là seulement, comme sur la page [Santé](health.md#meals-rest-and-sleep) : la nuit par son bord du haut (votre coucher, avec le temps pour se préparer à dormir) ou son bord du bas (votre réveil).
- **Un agenda que vous ne pouvez que lire**, une réunion avec d’autres (le serveur de son organisateur préviendrait chaque invité), les événements de toute la journée et les prises ne bougent pas.
- **À la souris** : appuyez et faites glisser ; la molette fait défiler. **Sur un écran tactile** : gardez le doigt sur l’événement jusqu’à ce qu’il se soulève, puis faites-le glisser ; un simple balayage fait défiler. Gardé puis lâché sans bouger, il ouvre son menu.
- Seulement dans les heures affichées ; au-delà, **Modifier**.
- S’il en chevauche un autre, Sioul le dit au-dessus de l’agenda, comme pour deux événements en même temps. Votre plan contourne aussitôt sa nouvelle heure.

## Les tâches fixées à une heure {#tasks-pinned-to-a-time}

Une tâche que vous fixez à une heure ([Tâches](tasks.md#pinned-to-a-time)) a son créneau dans votre agenda, « Tâches planifiées » sauf si vous en avez choisi un autre : un événement au titre de la tâche, qui suit le sien. Déplacez-le comme tout événement, ici ou dans un autre programme, et la tâche se déplace avec lui ; supprimez-le, et le plan replace la tâche. Votre plan le compte une fois, comme la tâche.

Dans l’agenda, il se montre comme la tâche qu’il est, jamais comme un événement : la marque des tâches là où un événement a la couleur de son agenda, et « Tâche · » avec le nom de sa liste là où un événement dit son lieu ; ses détails proposent **Ouvrir la tâche**. Qu’une chose soit une tâche ou un rendez-vous, c’est à vous de le dire : une tâche peut avoir une heure et rester une tâche.

- **En faire un rendez-vous**, dans les détails d’une tâche ou son menu : le formulaire d’événement s’ouvre avec le titre, le lieu et les notes de la tâche, à son heure, dans l’agenda des nouveaux événements (ou un autre que vous choisissez). Enregistré, l’événement remplace la tâche et son créneau : jamais les deux, jamais aucun, même si Sioul s’arrête entre-temps. Une tâche qui a ses propres étapes reste une tâche.
- **En faire une tâche**, sur un événement à vous : une tâche dans votre liste habituelle, à son jour, sa durée pour estimation, son heure gardée comme celle de la tâche quand elle est encore à venir. Une invitation reste un événement : d’autres vous y attendent.

## Deux événements en même temps {#two-events-at-once}

Quand deux événements se chevauchent, Sioul le dit, et quand ils se suivent de plus près qu’un temps que vous avez fixé, il dit qu’ils sont trop proches : ceux d’aujourd’hui sur le [Porche](porch.md), avec un bouton pour ouvrir chacun ; ceux d’aujourd’hui et des deux semaines suivantes au-dessus de l’agenda, quelle que soit la vue. **Ne plus en parler** les met de côté pour de bon, sur cet appareil. Déplacez l’un des deux, et c’est une nouvelle question. Une fois les deux finis, plus rien n’est dit. Deux événements se chevauchent seulement quand leurs heures se chevauchent. Aucun temps n’est jamais mis entre eux sans que vous l’ayez fixé : l’**Avant** et l’**Après** propres à un événement (dans son formulaire, et dits dans ses détails), ou **Temps gardé entre deux événements** dans les paramètres de l’agenda, 0 au début ; le plus long compte, jamais ajoutés. Trop proches, la ligne dit les minutes qui les séparent, le temps que vous avez fixé et où vous l’avez fixé.

## Votre journée autour de vos événements {#your-day-around-your-events}

Votre plan ([Tâches](tasks.md#the-plan)) contourne vos événements : chacun tient son heure, son Avant et son Après, et cinq minutes avant et après lui ; un événement de moins d’un quart d’heure tient un quart d’heure. Vos tâches vont dans ce qui reste de vos heures. Un repas qui tombe pendant un événement passe après lui, pour ce jour-là seulement (voir [Santé](health.md#meals-rest-and-sleep)).

Ce qu’un événement vous demande compte aussi : les événements d’une journée prennent leur part de ce que cette journée peut tenir, et les jours avant et après un événement lourd (un coût de 7 ou plus) en tiennent moins ([Tâches](tasks.md#what-it-costs-and-what-it-gives-back)).

## Les invitations {#invitations}

Une invitation qui arrive par courriel s’affiche dans le courriel : **Accepter**, **Peut-être**, **Décliner**. Acceptée, ou peut-être, l’événement va dans votre agenda, et votre réponse part vers la personne qui organise, depuis l’adresse où l’invitation est arrivée ; déclinée, seule votre réponse part. Rien n’entre dans votre agenda avant votre réponse. Un événement envoyé pour information montre **Ajouter à mon agenda** ; un événement annulé, **Le retirer de mon agenda**. Inviter des personnes depuis Sioul n’est pas encore possible ([Pas encore là](#not-there-yet)). Voir [Courriels](mail.md#invitations).

## Les rappels {#reminders}

Un rappel discret avant chaque événement, un quart d’heure avant lui et le temps de se préparer et d’y aller : « 14:00 · Mairie, dans 45 minutes », puis « Se préparer, y aller : dès 13:30. » et le lieu, avec **Ouvrir**. Combien de temps avant, c’est vous qui le dites (Paramètres ▸ Rappels ▸ **Avant un événement**), et chaque événement peut dire le sien : **Rappel**, dans son formulaire et dans ses détails, qui disent aussi quand il est rappelé ; pour chaque fois que vient un événement qui se répète. Une sortie, un rendez-vous avec un trajet avant lui ou une tâche qui le prépare avec des choses à emporter, est rappelée une fois, à la place de ce rappel, comme un **départ** : « Partir dans 15 minutes pour Médecin (14:00) », puis « Bus 24 à 13:10. » et « À emporter : Carte Vitale, Lunettes, Clés, et 1 de plus. », les choses à emporter non cochées de sa liste (sa liste « À emporter », ou une liste sans groupe ; jamais ce qui est à vérifier), trois au plus ; il ouvre la carte du rendez-vous. Sur un téléphone dont l’écran verrouillé cache le contenu privé, il dit seulement « Sioul a un rappel pour vous. » Pas pour les journées entières, ni pour un événement annulé ou décliné. Les agendas que vous ne pouvez que lire (jours fériés, abonnements) n’ont pas de rappel de Sioul, sauf si l’un de leurs événements en demande un ; leurs propres alarmes viennent toujours.

Un autre, une demi-heure avant la fin du travail, le dernier jour travaillé avant un événement : « lun. 12 oct. 09:00 · Mairie », et le lieu. Les alarmes que porte un événement (réglées par vous, par votre téléphone, ou par la personne à l’origine de l’invitation) viennent aussi à leur heure ; celle qui tombe à moins de cinq minutes du rappel de Sioul n’est dite qu’une fois. Pendant le sommeil, un rappel attend votre réveil, sauf si l’événement lui-même est dans la nuit : c’est vous qui l’avez choisi. Sans son, jamais répété ; sur un téléphone aussi, Sioul ouvert ou non. Voir [Paramètres](settings.md#reminders).

## Sur un téléphone {#on-a-phone}

**Jour** montre un jour aux heures plus hautes ; balayez-le de côté pour aller au jour suivant ou à celui d’avant. **Semaine** est une liste de jours : chaque chose sur sa ligne dans l’ordre des heures, les prises d’aujourd’hui sur une ligne.

Sous Android, Sioul synchronise lui-même vos agendas et en garde sa propre copie : l’application d’agenda de votre téléphone ne montre les mêmes événements que par sa propre synchronisation (DAVx⁵, par exemple). Les rappels viennent même quand Sioul est fermé : Sioul confie à l’avance les prochains à Android, qui le réveille à l’heure de chacun, pour qu’il vérifie que l’événement tient toujours. La carte d’agenda de Sioul sur l’écran d’accueil (« Sioul — Agenda ») liste vos prochains événements, et les dates qu’on vous demande en tête de leur jour (un formulaire demandé, un dernier jour pour résilier, le dernier jour de validité d’un papier, un paiement), avec le temps qui reste ; avec **Détails sur l’écran d’accueil** désactivé, elle montre l’heure des événements sans leur titre, et chaque date par son seul type (« Une date demandée ») ([Paramètres](settings.md#display)). Si l’application d’agenda de votre téléphone fait aussi sonner les alarmes d’un événement, désactivez l’une des deux, pour ne les entendre qu’une fois ; le rappel de Sioul avant un événement ne sonne jamais là-bas.

## Les réglages de l’agenda {#the-agenda-settings}

Le ⚙ en haut de la page :

- **Agendas** : renommés ici, et sur le serveur à la prochaine synchronisation. Un agenda vide peut être supprimé ; un agenda qui contient des événements reste. Celui que vous pouvez seulement lire le dit, et n’est jamais proposé pour un nouvel événement.
- **Les nouveaux événements vont dans** : l’agenda où va un nouvel événement ; tant que vous ne choisissez pas, le premier qui prend des événements.
- **Temps gardé entre deux événements** : 0 au début. Plus proches que cela, deux événements sont dits trop proches, jamais en même temps.
- **Ce que prend un agenda** : des événements, des tâches ou les deux, comme le dit votre serveur. L’agenda montre ceux qui prennent des événements, la page Tâches les listes qui prennent des tâches ; un agenda qui prend les deux est dans les deux, et dit « événements et tâches » partout où il est proposé.
- **La journée commence à** : l’heure à laquelle commencent les vues du jour et de la semaine quand ni vos nuits (réglées dans [Santé](health.md)) ni vos heures ne disent autre chose.

**L’agenda d’un événement** : son éditeur dit dans quel agenda il est, et **Agenda** le déplace vers un autre où vous pouvez écrire, de n’importe quel compte, sur un téléphone aussi : écrit d’abord là-bas, puis retiré du premier, jamais en double et jamais perdu, même si Sioul s’arrête entre les deux ; votre serveur reçoit les deux à la synchronisation suivante. Un événement rangé par erreur dans un agenda propre à cet appareil rejoint vos autres appareils une fois déplacé dans celui de votre serveur. Un agenda propre à cet appareil que votre partage ne transporte pas reste ici : l’éditeur de ses événements dit « Seulement sur cet ordinateur » (ou téléphone), et c’est pourquoi un tel événement s’affiche sur un appareil et pas sur l’autre. Un agenda d’un serveur est sur chaque appareil qui a son compte.

## Où vivent les événements {#where-events-live}

Sur votre serveur d’agenda, en événements CalDAV standard : votre téléphone et les autres programmes les voient. Les heures gardent leur fuseau horaire et s’affichent dans le vôtre. Modifier dans Sioul ne change que ce que vous avez changé : la liste des personnes invitées, une alarme ou les champs d’un autre programme restent tels qu’ils étaient. Ce que vous réglez dans Sioul (Avant et Après, **Rappel**, ce que l’événement vous demande) est écrit dans l’événement lui-même : il le suit sur vos autres appareils, et votre serveur et vos autres applications peuvent le lire, comme ils lisent l’événement.

Les agendas Google se créent, se renomment et se suppriment sur les pages de Google ; Sioul grise ces réglages, et dit pourquoi.

**Jusqu’où remontent les courriels et l’agenda** se règle en haut de [Comptes](accounts.md#your-accounts).

Le jour et la semaine de Santé dessinés dans le temps, et **La journée** de Tâches, sont cet agenda désormais : Santé garde sa journée en liste, et son bouton **La semaine, dans l’agenda** ouvre la Semaine de l’agenda ; **La journée** de Tâches ouvre le Jour de l’agenda, et son **Maintenant** y mène par un lien.

## Pas encore là {#not-there-yet}

Changer le titre ou le lieu d’une seule occurrence (son heure se déplace à la main), et choisir ici la couleur d’un agenda, sont prévus. Pas encore là non plus : inviter des personnes depuis Sioul ; importer ou exporter des fichiers `.ics`, et s’abonner à un agenda par son adresse ; plus d’un rappel de Sioul pour un événement ; choisir le fuseau horaire d’un événement. La copie d’une invitation gardée dans votre agenda ne garde pas votre réponse : c’est la personne qui organise qui l’a.

## Pour aller plus loin {#going-further}

- **Les événements répétés faits ailleurs** gardent leur règle, si riche soit-elle (« un lundi et un mercredi sur deux », « le premier lundi du mois ») : changer leur titre dans Sioul laisse la règle entière. Le formulaire de Sioul fait tous les jours, toutes les semaines, tous les mois ou tous les ans.
- **Les fuseaux horaires** : un événement écrit dans un autre fuseau le garde et s’affiche dans le vôtre ; Sioul écrit les nouveaux événements dans votre fuseau.
- **Un événement créé après l’heure de son rappel** n’est pas rappelé : vous venez de le voir.
- **Le jour travaillé d’avant** se lit dans vos heures de travail ([Paramètres](settings.md#hours)).
- **Un fichier d’agenda dans un courriel** : un `.ics` que porte un courriel va dans votre agenda avec **Ajouter à mon agenda**.
- **Deux agendas, un seul événement** : le même événement dans deux de vos agendas (le même UID au même début) n’est pas pris pour deux événements en même temps.

## Comparé à d’autres applications {#compared-with-other-apps}

**Du temps autour des événements, et un plan autour d’eux.** Sioul garde du temps avant et après chaque événement, réglé par vous, et le compte partout à la fois : dans votre plan, dans l’avertissement pour deux événements en même temps, et dans son rappel. Calendrier d’Apple et Fantastical gardent un temps de trajet avant un événement seulement, et disent quand partir d’après la circulation du moment, ce que Sioul ne fait pas. Morgen calcule le temps de trajet dans les deux sens avec Google Maps, et Reclaim ajoute un temps de trajet fixe dans les deux sens, chacun pour les agendas Google et Outlook (Morgen aussi pour iCloud et Fastmail), avec un compte chez eux. Le planificateur IA de Morgen et les habitudes de Reclaim placent aussi le travail et les routines, comme le déjeuner, autour de vos événements ; Sioul le fait sur votre appareil, d’après vos propres heures, vos repas et vos nuits, avec n’importe quel serveur CalDAV et sans compte ailleurs. Sioul ne sait pas envoyer d’invitations, importer ou exporter des fichiers `.ics`, ni montrer de lui-même les anniversaires, là où la plupart des autres le savent ; et parmi ces applications, seul Proton chiffre de bout en bout ce que dit un événement, ses heures mises à part.

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

=== "Au quotidien"

    | | Sioul | Google | Apple | Outlook | Thunderbird | Morgen |
    |---|---|---|---|---|---|---|
    | Garde le temps d’y aller avant un événement, et d’en revenir après | ✓¹ | ✗² | en partie³ | en partie⁴ | ✗⁵ | ✓⁶ |
    | Dit quand partir, d’après la circulation | ✗⁷ | ✗⁸ | ✓⁹ | ✗¹⁰ | ✗ | en partie¹¹ |
    | Avertit quand deux événements se chevauchent | ✓¹² | ? | ? | en partie¹³ | ✗ | en partie¹⁴ |
    | Place vos tâches autour de vos événements, repas et repos gardés | ✓¹⁵ | ?¹⁶ | ✗ | en partie¹⁷ | ✗ | ✓¹⁸ |
    | Plusieurs rappels pour un événement | en partie¹⁹ | ✓ | ✓ | ?²⁰ | ✓²¹ | ✓ |
    | Se répète selon des règles comme « le premier lundi » | en partie²² | ✓ | ✓ | ✓ | ✓ | ✓ |
    | Répond aux invitations qui arrivent par courriel | ✓ | ✓ | ✓ | ✓ | ✓ | en partie²³ |
    | Envoie des invitations | ✗ | ✓ | ✓²⁴ | ✓ | ✓ | ✓ |
    | Déplace un événement en le faisant glisser, avec annulation | ✓ | ✓ | en partie²⁵ | ?²⁶ | ✓ | en partie²⁷ |
    | Déplace un événement vers un autre agenda | ✓ | ✓ | ✓ | ✓ | ✓ | ? |
    | Montre les anniversaires de vos contacts | ✗²⁸ | ✓ | ✓ | ✓ | ✗ | ? |

    1. **Avant** et **Après**, de 5 minutes à 2 heures chacun, gardés libres par votre plan et comptés par la recherche de chevauchements et par le rappel.
    2. Ni le formulaire d’un événement ni l’ensemble des réglages sous Android n’ont de temps de trajet ou de préparation.
    3. Un temps de trajet avant l’événement, ajouté à sa durée ; rien après lui.
    4. Pas de temps par événement ; un seul réglage raccourcit toutes vos réunions (« finir plus tôt » ou « commencer plus tard ») pour laisser des intervalles.
    5. Demandé en 2009, et refusé.
    6. Morgen Assist calcule le temps de trajet avec Google Maps, avant les événements qui ont une adresse et, si vous le voulez, après eux ; pour les agendas Google, Outlook, iCloud et Fastmail ; toutes les formules sont payantes.
    7. Le rappel de Sioul vient avant votre **Avant**, au moment que vous choisissez ; il ne lit pas la circulation.
    8. Pas dans Google Agenda ; l’application Waze de Google donne de telles alertes d’après l’agenda du téléphone.
    9. D’après le lieu de l’événement, l’endroit où vous êtes probablement et la circulation ou les transports, pour des trajets de moins de trois heures.
    10. Pas sur un ordinateur ni sur le web ; le « Time to Leave » de l’application pour téléphone venait de Cortana, arrêtée en 2023.
    11. Le créneau de trajet montre quand partir ; une alerte pour cela n’est pas décrite.
    12. Sur le Porche pour aujourd’hui, et au-dessus de l’agenda pour deux semaines, avec le temps d’y aller et d’en revenir de chaque événement.
    13. Pour les invitations : un avis de conflit, une vue de l’endroit où tombe l’invitation, et un choix pour décliner les invitations en conflit.
    14. Le planificateur IA avertit quand une réunion tombe sur une tâche prévue ; deux événements qui se chevauchent ne sont pas décrits.
    15. Votre plan place vos tâches dans vos heures autour de vos événements, de leur Avant et de leur Après et de cinq minutes autour de chacun ; un repas qui tombe pendant un événement passe après lui ; les jours autour d’un événement lourd en tiennent moins.
    16. Non décrit dans l’aide de Google ; les « créneaux suggérés » de Gemini, dans les formules payantes de Workspace, proposent des horaires de réunion.
    17. Les comptes professionnels avec Viva Insights réservent du temps de concentration, des pauses et un déjeuner ; les tâches de To Do se placent en les faisant glisser.
    18. Le planificateur IA propose jusqu’à 8 jours de tâches, pauses réglables, écrites une fois que vous les approuvez ; les « Routines » tiennent des créneaux fixes comme le déjeuner.
    19. Un seul rappel de Sioul avant chaque événement (son moment choisi événement par événement, ou aucun), et les alarmes que porte l’événement.
    20. Microsoft décrit un seul choix de rappel par événement, et une valeur par défaut par compte.
    21. Plusieurs par événement ; une seule valeur par défaut pour tous les agendas ; les rappels ne viennent que pendant que Thunderbird tourne.
    22. Sioul garde toute règle faite ailleurs, et fait tous les jours, toutes les semaines, tous les mois ou tous les ans.
    23. Morgen répond sur les événements qui ont des personnes invitées, par le fournisseur de l’agenda ; il ne lit aucun courriel.
    24. Avec iCloud, Exchange et certains serveurs CalDAV.
    25. Glisser pour déplacer sur le Mac et l’iPhone ; une annulation n’est pas décrite dans les guides d’Apple.
    26. Faire glisser un événement vers un autre agenda est décrit ; le déplacer dans la grille, et l’annuler, ne le sont pas dans les pages lues.
    27. Le glisser-déposer est décrit ; une annulation ne l’est pas.
    28. Sioul ne fait pas d’agenda des anniversaires ; celui que garde votre serveur (Nextcloud en garde un) s’affiche comme tout agenda que vous ne pouvez que lire.

=== "Technique"

    | | Sioul | Google | Apple | Thunderbird | DAVx⁵ + Fossify ou Etar | Proton |
    |---|---|---|---|---|---|---|
    | Tout serveur CalDAV, en lecture et en écriture | ✓ | en partie¹ | ✓ | ✓ | ✓ | ✗² |
    | Les agendas Google, en lecture et en écriture | ✓ | — | ✓ | ✓ | ✓ | en partie³ |
    | Répond par courriel aux invitations de tout système (iMIP) | ✓ | ?⁴ | ✓ | ✓ | ✗⁵ | ✓⁶ |
    | Importe et exporte des fichiers `.ics`, s’abonne par adresse | ✗⁷ | ✓⁸ | ✓ | ✓ | en partie⁹ | ✓ |
    | Choisit le fuseau horaire d’un événement | en partie¹⁰ | ✓ | ✓ | ✓ | ✓ | ✓ |
    | Événements chiffrés de bout en bout | ✗¹¹ | ✗¹² | ✗¹³ | ✗ | ✗ | en partie¹⁴ |
    | Logiciel libre | ✓ GPL-3.0+ | ✗ | ✗ | ✓ MPL-2.0 | ✓ GPL-3.0 | ✓ GPL-3.0¹⁵ |

    1. Les autres applications joignent Google Agenda par CalDAV ; ses applications pour téléphone montrent les agendas des comptes que le téléphone synchronise déjà, et son application web n’ajoute que des agendas Google et des liens publics.
    2. « Proton Calendar doesn’t support CalDAV » : pas de synchronisation directe dans les deux sens avec un agenda extérieur.
    3. Une importation unique, ou un abonnement en lecture seule à l’adresse iCal secrète de Google.
    4. Vous répondez dans Agenda ou dans Gmail ; les invitations venues d’autres systèmes ne sont pas décrites.
    5. Les applications règlent votre réponse sur les événements déjà présents dans un agenda synchronisé ; une invitation dans un courriel ne peut pas y être répondue.
    6. Depuis Proton Mail ; les invitations d’autres fournisseurs sont ajoutées à votre agenda dès leur arrivée, là où Sioul n’ajoute rien avant votre réponse.
    7. Un `.ics` que porte un courriel entre avec **Ajouter à mon agenda** ; pas d’import depuis un fichier, pas d’export, pas d’abonnement par adresse.
    8. Sur un ordinateur seulement.
    9. Les deux applications importent et exportent des fichiers `.ics` ; s’abonner par adresse demande ICSx⁵.
    10. Un événement fait ailleurs garde son fuseau horaire et s’affiche dans le vôtre ; Sioul écrit les nouveaux événements dans votre fuseau.
    11. Les événements d’un serveur peuvent être lus par ce serveur, comme avec toute application CalDAV ; les agendas gardés sur un seul appareil voyagent scellés entre vos appareils.
    12. Un chiffrement côté client dans certaines formules payantes de Workspace seulement, pour la description, les pièces jointes et les données de Meet, pas pour le titre, l’heure ni les personnes invitées.
    13. Pas non plus avec la protection avancée des données : Apple dit que CalDAV n’a pas de chiffrement de bout en bout intégré.
    14. Le titre, la description, le lieu et les personnes invitées sont chiffrés sur votre appareil ; le début et la fin, le fuseau horaire, la répétition et les alarmes sont seulement signés, et les serveurs de Proton les lisent.
    15. Le code du client web est sous GPL-3.0 ; Proton dit que toutes ses applications sont libres.

**Les autres agendas.** Fantastical (Mac, iPhone, iPad, Apple Watch, Vision Pro, Windows) ajoute, avec sa formule Premium, un temps de trajet avant un événement, depuis le domicile, le travail ou l’événement précédent, et des alertes « temps de partir » d’après l’endroit où vous êtes et la circulation ; il lit tout compte CalDAV, ne place aucune tâche de lui-même, et ne décrit pas d’avertissement pour les événements qui se chevauchent. Reclaim.ai, une application web au-dessus de Google Agenda ou d’Outlook seulement, ajoute un temps de trajet avant et après les événements qui ont une adresse, et du temps pour se préparer ou décompresser autour des réunions ; il place des habitudes comme le déjeuner dans une plage et les déplace quand une réunion tombe dessus, et garde les événements de votre agenda principal dans ses propres bases de données. Sous Android, Fossify Calendar ajoute les anniversaires de vos contacts et déplace les événements en les faisant glisser, par heures entières ; Etar ne fait ni l’un ni l’autre.

??? info "Sources (en anglais)"
    - Google Agenda : les réglages sous Android, <https://support.google.com/calendar/answer/6084644?co=GENIE.Platform%3DAndroid>, lu le 8 octobre 2026.
    - Waze : les événements de l’agenda et quand partir, <https://support.google.com/waze/answer/6341187>, lu le 8 octobre 2026.
    - Google Workspace Updates : les suggestions de réunion avec Gemini dans Agenda (janvier 2026), <https://workspaceupdates.googleblog.com/2026/01/improved-meeting-suggestions-gemini-calendar.html>, lu le 8 octobre 2026.
    - Google Agenda : notifications et rappels, <https://support.google.com/calendar/answer/37161>, lu le 8 octobre 2026.
    - Google Agenda : répondre aux invitations, <https://support.google.com/calendar/answer/37135>, lu le 8 octobre 2026.
    - Google Agenda : importer et exporter des événements, <https://support.google.com/calendar/answer/37118>, lu le 8 octobre 2026.
    - Google Agenda : les autres agendas sous Android, <https://support.google.com/calendar/answer/13322290?co=GENIE.Platform%3DAndroid>, lu le 8 octobre 2026.
    - Google Agenda : les anniversaires, <https://support.google.com/calendar/answer/6084659>, lu le 8 octobre 2026.
    - Google for Developers : le guide de l’API CalDAV, <https://developers.google.com/workspace/calendar/caldav/v2/guide>, lu le 8 octobre 2026.
    - Google Workspace Updates : le chiffrement côté client dans Agenda (février 2023), <https://workspaceupdates.googleblog.com/2023/02/client-side-encryption-google-calendar.html>, lu le 8 octobre 2026.
    - Calendrier pour Mac : événements, temps de trajet et agendas, <https://support.apple.com/guide/calendar/icalwr13-events/27.0/mac/27>, lu le 8 octobre 2026.
    - Calendrier pour Mac : l’heure de départ, <https://support.apple.com/guide/calendar/icl40727/27.0/mac/27>, lu le 8 octobre 2026.
    - Calendrier pour Mac : les alertes, <https://support.apple.com/guide/calendar/icl624fd1912/27.0/mac/27>, lu le 8 octobre 2026.
    - Calendrier pour Mac : jours fériés et anniversaires, <https://support.apple.com/guide/calendar/show-holidays-and-birthdays-iclead4e0ec3/mac>, lu le 8 octobre 2026.
    - Guide de l’iPhone : inviter d’autres personnes dans Calendrier, <https://support.apple.com/guide/iphone/iph82c5721ca/ios>, lu le 8 octobre 2026.
    - Apple : vue d’ensemble de la sécurité des données iCloud, <https://support.apple.com/en-us/102651>, lu le 8 octobre 2026.
    - Outlook : finir les réunions plus tôt ou les commencer plus tard, <https://support.microsoft.com/en-us/outlook/calendar/end-meetings-early-or-start-late>, lu le 8 octobre 2026.
    - Microsoft : la fin de Cortana, <https://support.microsoft.com/en-us/cortana/end-of-support-for-cortana>, lu le 8 octobre 2026.
    - Outlook : présentation du calendrier, <https://support.microsoft.com/en-us/outlook/calendar/introduction-to-the-outlook-calendar>, lu le 8 octobre 2026.
    - Viva Insights : la gestion du temps, <https://support.microsoft.com/en-us/viva/insights/time-management-features-in-viva-insights>, lu le 8 octobre 2026.
    - Outlook : notifications et rappels, <https://support.microsoft.com/en-us/outlook/calendar/add-or-delete-notifications-or-reminders-in-outlook>, lu le 8 octobre 2026.
    - Outlook : un agenda des anniversaires, <https://support.microsoft.com/en-us/outlook/calendar/add-a-birthday-calendar-and-reminder-in-outlook>, lu le 8 octobre 2026.
    - Thunderbird : bogue 478376, le temps de trajet (refusé), <https://bugzilla.mozilla.org/show_bug.cgi?id=478376>, lu le 8 octobre 2026.
    - Thunderbird : les réglages de l’agenda, leurs chaînes, <https://github.com/thunderbird/thunderbird-desktop/blob/main/calendar/locales/en-US/calendar/preferences.ftl>, lu le 8 octobre 2026.
    - Thunderbird : bogue 285200, un avertissement pour les événements qui se chevauchent (ouvert), <https://bugzilla.mozilla.org/show_bug.cgi?id=285200>, lu le 8 octobre 2026.
    - Thunderbird : bogue 353492, plusieurs rappels pour un événement, <https://bugzilla.mozilla.org/show_bug.cgi?id=353492>, lu le 8 octobre 2026.
    - Thunderbird : bogue 134969, les rappels quand Thunderbird est fermé (ouvert), <https://bugzilla.mozilla.org/show_bug.cgi?id=134969>, lu le 8 octobre 2026.
    - Thunderbird : bogue 151994, un agenda des anniversaires tiré du carnet d’adresses, <https://bugzilla.mozilla.org/show_bug.cgi?id=151994>, lu le 8 octobre 2026.
    - Morgen : le temps de trajet, <https://www.morgen.so/guides/auto-schedule-travel-time>, lu le 8 octobre 2026.
    - Morgen : questions et réponses, <https://www.morgen.so/faq?84acd400_page=3>, lu le 8 octobre 2026.
    - Morgen : planifier sa journée avec le planificateur IA, <https://www.morgen.so/guides/plan-your-day-using-the-ai-planner>, lu le 8 octobre 2026.
    - Morgen : les Routines, <https://www.morgen.so/guides/morgen-routines-guide>, lu le 8 octobre 2026.
    - Morgen : plusieurs alertes pour un événement, <https://www.morgen.so/guides/how-to-add-multiple-alerts-to-calendar-events>, lu le 8 octobre 2026.
    - Morgen : les prix, <https://www.morgen.so/pricing>, lu le 8 octobre 2026.
    - Proton : les agendas extérieurs, et pas de CalDAV, <https://proton.me/support/subscribe-to-external-calendar>, lu le 8 octobre 2026.
    - Proton : venir de Google avec ses agendas, <https://proton.me/support/easy-switch-calendars>, lu le 8 octobre 2026.
    - Proton : ajouter un événement depuis la boîte de réception, <https://proton.me/support/add-event-from-inbox>, lu le 8 octobre 2026.
    - Proton : le modèle de sécurité de Proton Calendar, <https://proton.me/blog/protoncalendar-security-model>, lu le 8 octobre 2026.
    - Proton : la licence des clients web, <https://github.com/ProtonMail/WebClients/blob/main/LICENSE>, lu le 8 octobre 2026.
    - Proton : le logiciel libre chez Proton, <https://proton.me/community/open-source>, lu le 8 octobre 2026.
    - Manuel de DAVx⁵ : informations techniques, <https://manual.davx5.com/technical_information.html>, lu le 8 octobre 2026.
    - Manuel de DAVx⁵ : comptes et collections, <https://manual.davx5.com/accounts_collections.html>, lu le 8 octobre 2026.
    - Fossify Calendar : son dépôt, <https://github.com/FossifyOrg/Calendar>, lu le 8 octobre 2026.
    - Fossify Calendar : ticket 207, répondre à une invitation depuis un courriel, <https://github.com/FossifyOrg/Calendar/issues/207>, lu le 8 octobre 2026.
    - Etar : ticket 97, supprimer une seule occurrence, <https://github.com/Etar-Group/Etar-Calendar/issues/97>, lu le 8 octobre 2026.
    - Etar : ticket 234, les anniversaires, <https://github.com/Etar-Group/Etar-Calendar/issues/234>, lu le 8 octobre 2026.
    - Fantastical : le temps de trajet, <https://flexibits.com/fantastical/help/travel-time>, lu le 8 octobre 2026.
    - Fantastical : premiers pas, <https://flexibits.com/fantastical/help/getting-started>, lu le 8 octobre 2026.
    - Flexibits : les prix, <https://flexibits.com/pricing>, lu le 8 octobre 2026.
    - Reclaim.ai : le temps tampon, de trajet et de décompression, <https://help.reclaim.ai/en/articles/4281992-buffer-time-overview-travel-decompression-and-tasks-habit-breaks>, lu le 8 octobre 2026.
    - Reclaim.ai : les habitudes, <https://help.reclaim.ai/en/articles/17124978-habits-agents-overview-automate-your-recurring-routines>, lu le 8 octobre 2026.
    - Reclaim.ai : la sécurité, <https://reclaim.ai/security>, lu le 8 octobre 2026.
    - Reclaim.ai : les prix, <https://reclaim.ai/pricing>, lu le 8 octobre 2026.

## Côté technique {#for-technical-readers}

- **iCalendar** (RFC 5545) : un événement et ses exceptions par fichier, comme CalDAV les garde (RFC 4791, section 4.1). Les événements répétés sont déployés avec calcard (`RRULE`, `RDATE`, `EXDATE`, les remplacements `RECURRENCE-ID`), chacun dans son fuseau horaire, affiché dans le vôtre ; une occurrence modifiée est reconnue par son seul `RECURRENCE-ID`, comme le dit RFC 5545, même quand le `SEQUENCE` de la série a avancé. Les événements écrits depuis le formulaire, uniques ou répétés, sont à l’heure locale avec votre fuseau (`DTSTART;TZID=Europe/Paris:…`), avec un `VTIMEZONE` que Sioul écrit, pour que 9 h reste 9 h en hiver comme en été ; un événement déplacé ou auquel vous répondez garde la forme dans laquelle il était écrit, UTC compris, et aucun fichier n’est réécrit pour cela. Une modification ne réécrit que les lignes qui ont changé et augmente `SEQUENCE` ; les alarmes, les personnes invitées et les propriétés des autres programmes sont gardées.
- **Les propriétés propres à Sioul** voyagent dans l’événement : `X-SIOUL-BEFORE` et `X-SIOUL-AFTER` (en minutes, un jour au plus), `X-SIOUL-COST`, `X-SIOUL-GAIN`, `X-SIOUL-REMIND` et `X-SIOUL-TASK` pour le créneau d’une tâche. `X-SIOUL-REMIND` n’est volontairement pas un `VALARM`, pour que vos autres applications d’agenda ne le fassent pas sonner une deuxième fois, et pour que « pas pour celui-ci » puisse se dire. Les liens sont les liens et les relations de la RFC 9253, écrits sous les noms propres à Sioul (`X-SIOUL-LINK`, `X-SIOUL-RELATED-TO` pour une attente ; le `RELATED-TO` d’une étape tel quel), qu’un gestionnaire de comptes de téléphone peut renvoyer ; les lignes du standard sont lues aussi. Le temps de trajet d’Apple (`X-APPLE-TRAVEL-DURATION`) n’est pas lu.
- **CalDAV** (RFC 4791, sur WebDAV) : le même client que pour les contacts. Découverte par RFC 6764 et le principal de l’utilisateur (RFC 5397), jetons de synchronisation (RFC 6578), `calendar-multiget`, écritures avec `If-Match` et l’ETag de l’élément ; quand les deux côtés ont changé un événement, la version du serveur l’emporte et la vôtre est gardée parmi les versions précédentes de cet appareil (Paramètres ▸ Votre dossier et le partage ▸ Voir les versions précédentes), à remettre depuis la fenêtre, et la ligne d’état le dit. HTTPS seulement, TLS par rustls avec les certificats racine du système. Chaque compte se synchronise tous les quarts d’heure et aussitôt après un changement fait ici ; la page relit ses fichiers toutes les cinq minutes, quand elle revient en vue, et à minuit.
- **Les agendas Google** passent par le CalDAV de Google, avec OAuth 2.0 pour les applications natives (RFC 8252) et PKCE (RFC 7636) ; le jeton de renouvellement dans le trousseau, le jeton d’accès en mémoire seulement. Les agendas ne peuvent pas y être créés, renommés ni supprimés depuis Sioul, et Google ajoute vos rappels par défaut à chaque événement ([Fonctionne avec](compatibility.md#google)).
- **Les invitations** : la partie `text/calendar` d’un courriel (iMIP, RFC 6047) est lue comme iTIP (RFC 5546) : `REQUEST`, `CANCEL`, `PUBLISH`, analysée par Sioul lui-même ; aucun autre programme ne l’ouvre. La réponse est un `REPLY` iTIP avec votre `PARTSTAT` (`ACCEPTED`, `TENTATIVE`, `DECLINED`), envoyé dans un courriel en deux parties depuis le compte où l’invitation est arrivée, par le serveur SMTP de ce compte. Il porte `UID`, `SEQUENCE`, `DTSTART`, `DTEND` ou `DURATION`, `SUMMARY`, `ORGANIZER`, `RECURRENCE-ID`, les fuseaux horaires qu’ils nomment, `DTSTAMP` et votre propre ligne `ATTENDEE` : aucune autre personne invitée, aucune description, aucun lieu. L’événement est gardé tel que l’a envoyé la personne qui organise, sans `METHOD` (RFC 4791, section 4.1). Une invitation n’entre dans un agenda que par **Accepter**, **Peut-être** ou **Ajouter à mon agenda**.
- **Les courriels falsifiés** : un courriel dont DMARC échoue sous une politique `quarantine` ou `reject` (hors listes de diffusion) est jugé falsifié et mis de côté, jamais supprimé ; son invitation n’est pas parmi vos courriels, sauf si vous ouvrez ce qui a été mis de côté.
- **Les chevauchements** : les événements à heure fixe, de durée positive et non annulés, chacun élargi de son Avant et de son Après ; le même UID au même début dans deux agendas n’est pas une double réservation. Les paires que vous mettez de côté sont gardées dans `$XDG_STATE_HOME/sioul/overlaps-set-aside.toml`, sur cet appareil.
- **Les rappels sous Android** : les rappels des huit prochains jours, cent au plus, sont confiés à l’avance au gestionnaire d’alarmes d’Android (`setExactAndAllowWhileIdle` quand « Alarmes et rappels » est autorisé, sinon avec jusqu’à une heure de retard) ; à l’heure de chacun, un récepteur demande à Rust si l’événement tient toujours. Les rappels de Sioul passent par un canal « Événements » discret ; les alarmes propres à un événement par un autre. Sioul synchronise lui-même les agendas et ne passe pas par le fournisseur d’agendas d’Android. **Sur un ordinateur**, les rappels viennent de la fenêtre, ou, fenêtre fermée, de `sioul remind --watch`, lancé avec votre session quand vous le demandez.
- **L’affichage d’un événement** : ses notes sont du Markdown (CommonMark, avec les tableaux, le texte barré et les listes de tâches de GFM), dessiné en HTML fait en Rust (`description::description_html`). Le HTML qui y est écrit s’affiche en texte ; aucune image n’est dessinée (une image du web devient un lien vers elle) ; seuls les liens `http`, `https`, `mailto` et `tel` sont gardés, et les adresses web écrites telles quelles deviennent des liens. Un lien s’ouvre dans le navigateur, ou dans un nouveau message pour une adresse, une fois son adresse montrée (sous le pointeur, ou après un premier appui sur un écran tactile) ; Sioul ne le charge jamais. Le titre d’une invitation s’affiche en texte brut (`Text.PlainText`) ; les descriptions HTML (`X-ALT-DESC`) ne sont pas lues.
- **Le stockage** : un fichier `.ics` par événement dans des dossiers vdir (`~/.local/share/sioul/calendars`), que khal et vdirsyncer lisent aussi, dans des dossiers au mode 0700 sous Linux et macOS, ou dans le stockage privé de l’application sous Android. Sioul ne les chiffre pas sur le disque : le chiffrement de votre disque s’en charge. Les événements d’un serveur CalDAV peuvent être lus par ce serveur. Les agendas gardés sur un seul appareil voyagent entre vos appareils scellés, dans la partie **Listes gardées ici** (XChaCha20-Poly1305, sous une clé faite à partir de votre phrase de passe par Argon2id).
