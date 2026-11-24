---
description: Les projets dans Sioul – toute affaire que vous suivez, pour un client ou pour vous, qui rassemble ses tâches, ses événements, ses courriels, ses notes, son temps et ses factures sur une ligne de temps ; ses courriels rangés tout seuls à leur arrivée, une fois que Sioul a vérifié qui les envoie.
---

# Les projets {#projects}

## En bref {#in-short}

Un projet, c’est toute affaire que vous suivez : un travail pour un client, dont le temps se facture, ou une affaire à vous, comme le logement, la santé, une déclaration d’impôts ou un voyage. Sa page rassemble ce qui lui appartient : ses tâches, ses événements, ses courriels, ses notes, le temps que vous y avez passé et ses factures, sur une ligne de temps, avec d’abord ce qui vient. À l’arrivée des courriels, Sioul les range là selon les règles que vous donnez au projet, après avoir vérifié qui les envoie. La liste des projets est un seul fichier lisible sur votre appareil, parmi les propres listes de Sioul.

<figure markdown="span">
  [![La page Projets : trois projets listés à gauche, avec Nouveau projet ; l’un ouvert à droite : « Pour un client », son taux, Tableau, Liste, Gantt, Noter du temps et Faire la facture ; ses tâches ouvertes et faites, le temps noté et à facturer ; « Les courriels qui arrivent ici tout seuls », replié ; ses factures, l’une marquée Payée ; puis « Sur une ligne de temps », ce qui vient et ce qui s’est passé avant.](../assets/screens/fr/projects.png){ loading=lazy }](../assets/screens/fr/projects.png "Ouvrir l’image en grand")
  <figcaption>Un projet : ce qui vient, puis ce qui s’est passé.</figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

- **Les courriels sont vérifiés avant d’être rangés.** À leur arrivée, un courriel falsifié (son domaine dit ne pas l’avoir envoyé), ou un courriel qui emprunte un nom connu, est mis de côté d’abord, et jamais rangé dans un projet, même quand son adresse ressemble à celle de votre client. La page du projet vérifie ses courriels de la même façon chaque fois qu’elle les montre, comme ce qu’un agent IA lit du projet : un tel courriel n’apparaît jamais sur sa ligne du temps, pas plus qu’une conversation que seul un tel courriel y rattache.
- **Un courriel qui échoue aux vérifications de l’expéditeur** (les deux signatures par lesquelles les serveurs se portent garants d’un expéditeur ont échoué) n’est jamais rangé par une règle sur l’adresse ou le domaine de votre client. Une règle sur des mots de l’objet, du texte ou du nom d’une pièce jointe peut encore le ranger, puisque ceux-ci ne reposent pas sur qui l’a envoyé ; sur la page du projet, il dit alors « dont l’adresse n’a pas pu être vérifiée ».
- **Chaque courriel rangé dit pourquoi il est venu** : « domaine de l’expéditeur : example.org ».
- **Vos projets sont un seul fichier lisible** sur votre appareil, parmi les propres listes de Sioul (sur un appareil qui les gardait déjà dans votre dossier de notes, elles y restent tant que vous ne les avez pas déplacées : [Où Sioul garde ses listes](sharing.md#where-sioul-keeps-its-lists)), et les courriels sont rangés sur votre appareil. Entre vos appareils, ce fichier voyage scellé par le partage de Sioul, dans **Projets et argent** : le dossier et son serveur voient que quelque chose a changé, jamais quoi ([Le partage](sharing.md)). Avant le déplacement, la synchronisation de votre dossier de notes peut le transporter à sa place.
- Il n’y a pas de serveur de Sioul, et rien de vos projets n’y va.

## Créer un projet {#making-a-project}

**Nouveau projet** (ou **Nouveau ▾ ▸ Un projet**) :

- **Nom**.
- **Pour un client : son temps se facture à un taux horaire**, cochée quand le formulaire s’ouvre sur un nouveau projet. Puis : **Pour** (pour qui), **Une heure coûte** (sinon le taux de [Paramètres ▸ Factures](settings.md#invoices)), et **Factures attendues dans** : le budget où l’argent est attendu jusqu’au paiement. Quand **Pour** nomme l’un de vos [contacts](contacts.md) (son nom, ou son organisation), les factures prennent sa fiche de client quand il en a une ([Vos clients](time.md#your-clients)), sinon son adresse postale.
- Sans cela, le projet est à vous.
- **À quoi il sert** : travail, vos démarches, loisirs, plusieurs ensemble, les trois mêmes interrupteurs que partout ([Les heures](hours.md)). Les pages Projets et Temps le montrent à ces heures-là, et ses tâches y viennent sauf si elles disent les leurs. Rien de coché, le projet d’un client compte comme travail ; un autre suit ses tâches ouvertes, et s’affiche à toute heure s’il n’en a aucune.
- **Où il en est** : ouvert, en attente ou fermé.
- **Ouvert aux agents d’IA** : éteint tant que vous ne l’allumez pas ([Choisir ce qu’un agent voit](ai-agent.md#choosing-what-an-agent-sees)).

**Modifier**, en haut de la page d’un projet, rouvre ce formulaire. **Retirer de la liste**, au bas de ce formulaire, enlève le projet une fois que vous avez confirmé. Ses tâches, notes, courriels et temps restent où ils sont ; ils ne sont plus rassemblés sous lui.

## La page d’un projet {#a-projects-page}

À gauche, tous les projets, avec leurs tâches ouvertes et le temps qui reste à facturer ; les projets fermés viennent en dernier, en gris. À droite, le projet ouvert :

- **Sur une ligne de temps** : d’abord ce qui est **À venir** (les dates demandées de ses tâches ouvertes, les événements), puis ce qui s’est passé **Avant**, du plus récent au plus ancien : tâches faites, courriels, notes modifiées, temps noté, factures.
- **Ses tâches**, en **Tableau**, en **Liste** ou en **Gantt** : chacun ouvre la page [Tâches](tasks.md), limitée à ce projet. Le **Gantt** est celui de la page Tâches ([Liste, Tableau, Gantt](tasks.md#list-board-timeline)) : chaque tâche sur ses jours, la date demandée en petit losange.
- **Noter du temps** : une réunion, un appel, un travail fait loin du minuteur.
- **Faire la facture**, quand du temps facturable attend d’être facturé : un brouillon de ce temps s’ouvre dans l’éditeur, dans Budgets ▸ Factures, où vous le vérifiez et l’émettez. Un projet qui a déjà un brouillon retrouve ce brouillon, avec le temps noté depuis ajouté ([L’éditeur](time.md#the-editor)).
- **La fiche de son client**, sur un projet pour un client : une ligne, comme « Fiche client : Une entreprise · France · 904 123 452 », ou « Pas encore de fiche client : les factures le facturent comme une entreprise, d’après sa fiche de contact. », avec **Fiche client…** (**Faire sa fiche client…** tant qu’il n’en a pas) ([Vos clients](time.md#your-clients)).
- **Ses factures** : chacune ouvre son volet dans Budgets ▸ Factures, avec **Payée…**, **Écrire le courriel**, **Imprimer à nouveau** et **Avoir…** ([Le volet d’une facture émise](time.md#an-issued-invoice)). La case **Payée** à côté de chacune dit qu’elle est payée, par virement, aujourd’hui ; sur un avoir qui a de l’argent à rendre, la case dit **Remboursé**. **Imprimer à nouveau**, à côté de la case, copie le fichier émis là où vont vos factures, et l’ouvre.
- **Ouvert aux agents d’IA** : un interrupteur, éteint tant que vous ne l’allumez pas. Un agent d’IA que vous connectez voit alors le projet et ce qui lui est lié, et peut y ajouter des tâches, des notes, des liens et des brouillons ([Choisir ce qu’un agent voit](ai-agent.md#choosing-what-an-agent-sees)).
- **Ajouter ▾** et **Lier à…**, au-dessus de **Les courriels qui arrivent ici tout seuls** : quelque chose de nouveau lié au projet, ou un lien vers ce qui existe, comme le contact du client ; **Liens**, tout ce que le projet tient et ce à quoi c’est lié ([Les liens](ties.md)).

Sur un téléphone, la liste des projets remplit l’écran, puis le projet que vous ouvrez ; **Retour** revient à la liste ([Sur un téléphone ou une tablette](first-steps.md#on-a-phone)).

## Les courriels qui arrivent ici tout seuls {#mail-that-comes-here-by-itself}

**Les courriels qui arrivent ici tout seuls**, sur la page du projet, déplie les règles du projet ; **Ajouter une règle** en commence une, et × en retire une. Un courriel va au projet quand une règle correspond ; chaque ligne remplie dans une règle doit correspondre :

- **Des domaines** : le domaine de l’expéditeur, ou l’un de ses sous-domaines ;
- **Des adresses** ;
- **Mots dans le sujet** ;
- **Mots dans le texte** ;
- **Mots dans le nom d’une pièce jointe** (« devis » prend `Devis_2026.pdf`).

Plusieurs mots ou domaines sur une ligne, séparés par des virgules : n’importe lequel d’entre eux. Les mots se comparent entiers, sans tenir compte des majuscules ni des accents ; dans le nom d’une pièce jointe, ils se trouvent n’importe où dans le nom.

Une réponse dans une conversation du projet va avec lui, comme un courriel que vous y liez à la main (**Lier à…**) : ce que vous liez reste tel que vous l’avez lié, même un courriel que les vérifications mettraient de côté. Chaque courriel rangé garde sa raison, montrée sur demande sous **Pourquoi il est ici** quand vous ouvrez le courriel depuis le Porche : « domaine de l’expéditeur : example.org ».

À leur arrivée, et chaque fois que la page du projet les montre, les courriels sont vérifiés avant qu’une règle les range ([plus haut](#what-is-protected)) : les courriels falsifiés et ceux qui empruntent un nom connu sont mis de côté d’abord, et une règle sur l’adresse ou le domaine de votre client ne range pas un courriel qui a échoué aux vérifications de l’expéditeur. Dans l’autre sens, une règle de projet l’emporte sur une marque d’indésirable : les vrais courriels de votre client ne se perdent pas parmi les indésirables, et le propre filtre à indésirables de Sioul laisse en paix les conversations d’un projet.

### Une tâche de chaque courriel {#a-task-of-each-message}

Une règle peut faire une tâche de chaque courriel qu’elle prend : l’avis de vente d’une boutique, « [New sale] Gift card 50 € », devient la tâche « Gift card 50 € » dans le projet et dans votre plan, le courriel à une pression dans ses liens, avec tout ce qu’il faut imprimer. Rien du courriel n’est copié dans la tâche.

- **Faire une tâche de chaque courriel**, sous une règle, ouvre son formulaire ; depuis un courriel, **⋮ ▸ Faire une tâche des courriels comme celui-ci…** ouvre le même formulaire, rempli de son expéditeur et des mots entre crochets de son sujet (« New sale »), dans son projet, ou un nouveau projet que vous y nommez.
- **La tâche** : son nom (« Envoyer une carte cadeau »), des mots devant son titre (« Envoyer : »), la liste où elle va (votre liste habituelle, sauf si vous en choisissez une), **la date demandée**, tant de jours travaillés après l’arrivée du courriel (3 au départ : une vente du vendredi est demandée pour le mercredi), sa première estimation, et les stocks qu’utilisera chaque tâche ([Stocks](supplies.md)), comptés comme prévus aussitôt.
- **Les jours travaillés** sont les jours qui ont des heures de travail dans votre semaine (du lundi au vendredi si vous n’en dites aucune), moins vos jours de congé : les périodes que vous avez saisies, et les jours fériés dès que leur calendrier est abonné ([Vacances et jours fériés](hours.md)). Avant cet abonnement, un jour férié compte comme un jour travaillé.
- **Une fois par courriel, sur chaque appareil** : deux de vos appareils qui reçoivent le même courriel font une seule tâche. Une tâche que vous avez supprimée n’est jamais refaite.
- **Seul le courrier que le Porche laisse entrer** : un faux « [New sale] » ne fait aucune tâche et ne prévoit aucun stock ; par l’expéditeur, seulement le courrier dont l’expéditeur est prouvé ([plus haut](#what-is-protected)). Le courrier que vous envoyez vous-même n’en fait aucune.
- **Les courriels déjà là ne font rien d’eux-mêmes** : une fois la règle enregistrée, **Aussi pour les 12 courriels déjà là** fait les leurs, une fois, quand vous le demandez.
- Chaque tâche faite est dite une fois dans la ligne d’état : « « Gift card 50 € » est dans Travail, faite d’un courriel, demandée pour mercredi 25 novembre. » Sous la règle, sa ligne dit ce qu’est chaque tâche, avec **Changer…** ; décochez la case, et la règle ne fait plus que ranger le courrier.

## Où vivent les projets {#where-projects-live}

Dans un seul fichier, `sioul-projects.toml`, parmi les propres listes de Sioul, dans `records/` du dossier de données de Sioul (`~/.local/share/sioul/records/` sous Linux) : lisible, et modifiable à la main si vous voulez. Sur un appareil qui gardait déjà les listes de Sioul dans votre dossier de notes, il reste à la racine de ce dossier tant que vous ne les avez pas déplacées ([Où Sioul garde ses listes](sharing.md#where-sioul-keeps-its-lists)). Il voyage scellé par le partage de Sioul, dans **Projets et argent** ; avant le déplacement, la synchronisation de votre dossier de notes peut le transporter à sa place ([Le partage](sharing.md)). Les notes d’un projet restent dans votre dossier de notes, et le projet les nomme par leur chemin dans ce dossier. Les tâches et les événements d’un projet portent son identifiant dans un champ standard, pour que les autres programmes d’agenda gardent le regroupement.

Si les listes de Sioul contiennent encore `sioul-cases.toml`, le nom du fichier d’avant le nom unique des projets, rien ne change : Sioul le lit et l’écrit tel quel, pour qu’un appareil pas encore mis à jour le lise aussi. Quand chacun de vos appareils a cette version, **Renommer en sioul-projects.toml**, dans les réglages de la page Notes (⚙), sous le dossier des notes, le renomme, en gardant une copie de l’ancien fichier à côté. Les liens vers un projet écrits avant l’ouvrent toujours.

## Selon vos heures {#in-quiet-time}

Le bouton au-dessus de la liste dit ce qu’elle montre (« Loisirs ▾ ») et ouvre trois cases, **Travail**, **Démarches** et **Loisirs**, cochées comme le disent vos heures sur cet appareil : les projets du travail pendant vos heures de travail, ceux de vos démarches pendant vos heures de démarches, ceux des loisirs le soir, aux repas et la nuit. Cochez-en une autre pour voir ses projets aussi : cela tient jusqu’au changement de vos heures, un jour au plus ; **Selon vos heures** rend aussitôt les leurs. Sur un appareil dont vous avez dit pour quoi il est fait, les cases suivent cela aussi, et le **?** à côté d’elles dit ce qui attend sur vos autres appareils ([À quoi sert chaque appareil](hours.md#what-each-device-is-for)). Le projet d’un client sans rien de coché compte comme travail ; un autre sans rien de coché s’affiche pour ce à quoi servent ses tâches ouvertes, et à toute heure s’il n’en a aucune. Quand les cases ne montrent aucun projet, une phrase le dit : « Aucun projet pour ce qui est coché. » Rien ne couvre la page, à aucune heure, et un projet ouvert d’ailleurs s’ouvre quelles que soient les cases. La page [Temps](time.md#in-quiet-time) partage ces cases. Voir [Les heures](hours.md).

## Pour aller plus loin {#going-further}

- **Une file sur le Porche** : **Projets montrés ici**, dans le ⚙ du Porche, donne à un projet sa propre file ([Le Porche](porch.md)).
- **Sur un appareil pas encore mis à jour** : **Démarches** et **Loisirs** ensemble, et les trois sur le projet d’un client, s’écrivent comme l’ancienne case **À vous, hors travail**, qu’un Sioul plus ancien lit, et cette case se lit ici comme démarches et loisirs (à côté d’un client, les trois, comme le Sioul plus ancien la lisait). Les autres choix sont nouveaux pour un Sioul plus ancien : il les lit comme si rien n’était dit, et enregistrer le formulaire du projet là-bas les retire. Mettez chaque appareil à jour avant de vous en servir.
- **À la main, si vous voulez** : la liste des projets est un fichier lisible, `sioul-projects.toml`, que vous pouvez modifier ; sa forme est [plus bas](#for-technical-readers).
- **Un agent d’IA que vous connectez** ([Avec un agent d’IA](ai-agent.md)) ne voit un projet qu’une fois que vous l’avez ouvert : **Ouvert aux agents d’IA**, sur sa page ou dans son formulaire. Il lit alors ce que la page du projet rassemble, les codes à usage unique, les liens de connexion, les numéros de compte et de carte masqués dans ses courriels, et peut y ajouter des tâches, des notes, des liens et des brouillons. D’un projet laissé fermé, il ne voit rien ([Choisir ce qu’un agent voit](ai-agent.md#choosing-what-an-agent-sees)).

## Pourquoi cela fonctionne ainsi {#why-it-works-this-way}

Classer ses courriels à la main coûte et échoue : dans le courrier de 345 personnes, retrouver un courriel par son dossier prenait 59 secondes, par une recherche 17, et celles qui classaient le plus n’en retrouvaient pas davantage (Whittaker et al. 2011). Les courriels d’un projet y viennent donc par les règles que vous lui donnez une fois, après que Sioul a vérifié qui les envoie, et vous ne classez rien. Les supports visuels aident beaucoup de personnes autistes (Steinbrenner et al. 2020) : les tâches d’un projet peuvent se dessiner sur un Gantt quand cela aide, sans jamais y être obligées.

La liste suit vos heures comme les Courriels, puisque les rappels du travail le soir défont la récupération, et tout est à une case près : une liste sert aussi à gérer. Plus de détails dans [ce que dit la recherche (en anglais)](https://sioul.aurelienpierre.com/dev/research.html), constats 13, 21 et 61.

## Comparé à d’autres applications {#compared-with-other-apps}

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

| | Sioul | Trello | Asana | Notion | Nextcloud Deck | Vikunja |
|---|---|---|---|---|---|---|
| Les courriels rangés tout seuls dans le projet, selon vos règles | ✓ | en partie¹ | en partie² | ?³ | ✗⁴ | ?⁵ |
| Tableau | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Liste | ✓ | en partie⁶ | ✓ | ✓ | ✗⁷ | ✓ |
| Calendrier ou frise chronologique (Gantt) | ✓⁸ | en partie⁶ | ✓ | ✓ | en partie⁹ | ✓ |
| Des tâches qui en attendent d’autres | ✓ | ✗¹⁰ | ✓¹¹ | ✓ | ✗⁷ | ✓ |
| Le temps passé, noté sur le projet | ✓ | ✗¹⁰ | ✓¹² | ✗¹⁰ | ✗⁷ | en partie¹³ |
| Des factures faites du temps du projet | ✓ | ✗¹⁰ | ✗¹⁰ | ✗¹⁰ | ✗⁷ | ✗¹⁰ |
| Des liens dans les deux sens vers les notes, les personnes et les événements | ✓ | ? | ? | en partie¹⁴ | ? | en partie¹⁵ |
| Partagé avec d’autres personnes | ✗¹⁶ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Des tâches que d’autres applications lisent (CalDAV) | ✓ | ✗¹⁰ | ✗¹⁰ | ✗¹⁰ | en partie⁹ | ✓¹⁷ |
| Logiciel libre, sur votre appareil ou votre serveur | ✓ | ✗¹⁸ | ✗¹⁸ | ✗¹⁸ | ✓ | ✓ |

1. Vous envoyez ou transférez un courriel à l’adresse propre au tableau ; il devient une carte.
2. Depuis Gmail ou Outlook, à la main, avec le module d’Asana. Une adresse électronique propre à un projet n’a pas pu être confirmée : le centre d’aide d’Asana n’a pas pu être lu.
3. Les pages de Notion Mail n’ont pas pu être lues.
4. Absent de la documentation de Deck. Nextcloud Mail peut faire une tâche d’agenda à partir d’un courriel, à la main, et ses filtres déplacent les courriels dans des dossiers, pas dans des tableaux.
5. Absent de la page des fonctions de Vikunja.
6. Les vues Tableau (Table), Calendrier et Frise sont dans les offres Premium et Enterprise ; le Planner, sur un calendrier, dans les offres payantes.
7. Absent de la documentation d’utilisation de Deck, qui décrit des tableaux faits de piles de cartes.
8. Le bouton **Gantt** du projet ouvre le Gantt de la page Tâches, limité au projet.
9. Chaque tableau est offert en CalDAV comme une liste de tâches, en lecture seule : les cartes datées s’affichent dans l’agenda de Nextcloud (d’après le code de Deck).
10. Absent de sa documentation et de la liste de ses offres.
11. À partir de l’offre Starter.
12. À partir de l’offre Advanced, le temps estimé face au temps réel.
13. Une fonction Pro pour les installations auto-hébergées, « in private beta », absente du nuage de Vikunja.
14. Des relations dans les deux sens entre bases de données.
15. Des relations entre tâches, aussi d’un projet à l’autre (« a subtask or blocking task »).
16. Vos projets sont à vous : ils ne voyagent qu’entre vos propres appareils. Sioul n’a pas de fonctions d’équipe.
17. Vikunja décrit son CalDAV comme « in an early alpha stage » ; en CalDAV, il n’écrit que les relations parent et enfant, pas ses relations de blocage.
18. Un service hébergé, selon ses propres conditions ; aucune licence libre n’est publiée pour lui.

D’autres en font plus pour le travail à plusieurs : chacune des autres applications ici partage un projet avec d’autres personnes, et les nombreuses vues et automatisations de Notion et d’Asana, comme les intégrations de Trello et d’Asana, vont bien au-delà de ce que fait Sioul. Ce que Sioul garde en un seul endroit, ce sont les courriels du client rangés tout seuls, le temps noté et les factures faites de ce temps, avec ses tâches gardées comme des tâches CalDAV standard.

??? info "Sources"
    Toutes lues le 8 octobre 2026.

    - **Trello**, la page d’Atlassian sur la création de cartes par courriel, et ses prix et offres : <https://support.atlassian.com/trello/docs/creating-cards-by-email/>, <https://trello.com/pricing>
    - **Asana**, ses prix et offres, et sa page sur les intégrations de courriel : <https://asana.com/pricing>, <https://asana.com/apps/email>
    - **Notion**, ses pages d’aide sur les vues, les tâches et dépendances, les relations et les agrégations, et ses prix : <https://www.notion.com/help/views-filters-and-sorts>, <https://www.notion.com/help/tasks-and-dependencies>, <https://www.notion.com/help/relations-and-rollups>, <https://www.notion.com/pricing>
    - **Nextcloud Deck**, son README, sa documentation d’utilisation et son code CalDAV (`lib/DAV/Calendar.php`, `CalendarObject.php`), et le manuel d’utilisation de Nextcloud Mail : <https://github.com/nextcloud/deck>, <https://deck.readthedocs.io/en/latest/User_documentation_en/>, <https://github.com/nextcloud/deck/blob/main/lib/DAV/Calendar.php>, <https://docs.nextcloud.com/server/latest/user_manual/en/groupware/mail.html>
    - **Vikunja**, sa page des fonctions et ses pages sur CalDAV et les relations entre tâches : <https://vikunja.io/features/>, <https://vikunja.io/help/caldav/>, <https://vikunja.io/help/task-relations/>

## Côté technique {#for-technical-readers}

### Le fichier {#the-file}

`sioul-projects.toml`, dans `records/` du dossier de données de Sioul (à la racine de votre dossier de notes tant que les listes de Sioul n’ont pas été déplacées) : du TOML, un `[[project]]` par projet (`id`, `title`, `kind = "project"` pour celui d’un client, `client`, `rate`, `budget`, `status`, `area` pour ce à quoi il sert, `files` pour ses notes, `ai`), chacun avec ses lignes `[[project.route]]`, et ses modèles de tâche, `[[project.model]]` (`id`, `name`, `list`, `prefix`, `business_days`, `estimate`, `supplies`), qu’une règle nomme dans `makes` ([Une tâche de chaque courriel](#a-task-of-each-message)). Une tâche qu’une règle a faite porte `X-SIOUL-ORIGIN:mid:<Message-ID>`, son UID fait du projet et du courriel, et elle est liée à son courriel dans `links.toml` (`how = "made"`). Le premier nom du fichier, `sioul-cases.toml`, avec `[[case]]` et `[[case.route]]`, se lit de même et garde ses noms quand Sioul l’écrit ; le renommer vérifie que rien ne change avant d’écrire, et garde `sioul-cases.toml.before-rename`. Les tâches et les événements d’un projet portent son `id` dans `X-SIOUL-REFID` (le `REFID` de la RFC 9253 §8.3 sous le nom propre à Sioul, que les gestionnaires de comptes d’Android peuvent renvoyer ; `REFID` est lu aussi), pour que les autres programmes CalDAV gardent le regroupement.

### Comment les règles correspondent {#how-routes-match}

- `from_domains` : le domaine de l’expéditeur ou l’un de ses sous-domaines (« example.org » prend « mail.example.org », jamais « notexample.org ») ; `from_addresses` : exactes, sans tenir compte de la casse ; `subject_contains`, `text_contains` : des mots entiers, sans tenir compte de la casse ni des accents ; `attachment_contains` : n’importe où dans le nom du fichier, sans tenir compte de la casse ni des accents. Chaque liste remplie doit correspondre ; n’importe quel élément d’une liste suffit.
- Le texte est lu dans ses 6 000 premiers caractères (la partie HTML, lue comme du texte, quand la partie texte n’est qu’une ébauche).
- Les conversations suivent `In-Reply-To` et `References`, à travers les seuls courriels que les vérifications laissent passer : une réponse falsifiée ne se montre pas et ne joint pas deux conversations. Un courriel rangé à son arrivée est lié à son projet (`links.toml` : `mid:` ↔ `sioul:project/<id>` ; `sioul:case/<id>`, écrit avant, se lit de même), et y reste quand les règles changent. Changer les règles d’un projet lie les courriels déjà là que ses règles sur le texte ou les pièces jointes prennent, après les mêmes vérifications.

### La sécurité, précisément {#security-precisely}

- À l’arrivée des courriels, sur le Porche, sur la page d’un projet et dans ce qu’un agent lit d’un projet, une seule fonction décide (`porch::admission`), dans l’ordre de la protection : un expéditeur bloqué reste bloqué ; les courriels falsifiés (DMARC en échec sous une politique `quarantine` ou `reject`) et ceux dont le nom affiché emprunte un nom connu sont mis de côté ; les codes à usage unique gardent leur file ; puis viennent les règles des projets. Les résultats d’authentification viennent du `Authentication-Results` de votre propre fournisseur (RFC 8601, seulement d’un `authserv-id` auquel vous faites confiance) et de la vérification que fait Sioul de chaque courriel à sa réception ([Chaque courriel vérifié](privacy-security.md#every-message-checked)).
- Les courriels non authentifiés (SPF et DKIM tous deux en échec, aucun succès DMARC, aucun sceau ARC d’un relais de confiance) sont rangés sans leur expéditeur : `from_domains` et `from_addresses` ne peuvent pas les prendre, tandis que les règles sur l’objet, le texte et les pièces jointes le peuvent encore. Un courriel sans résultats, ou dont un seul des deux échoue, garde son expéditeur : une règle sur l’expéditeur se fie alors à l’adresse telle qu’elle est écrite.
- Une règle qui correspond l’emporte sur la marque d’indésirable du fournisseur, et le propre filtre à indésirables de Sioul laisse en paix les conversations d’un projet.
- La partie **Projets et argent** du partage scelle le fichier des projets (`sioul-projects.toml`, ou `sioul-cases.toml`) avec les fichiers des fiches des clients, des budgets, de la banque, des mouvements et des contrats et, une fois les listes de Sioul déplacées, les listes des papiers et des courriers : chaque enregistrement avec XChaCha20-Poly1305, sous une clé tirée de votre phrase de passe par Argon2id (64 Mio, 3 passes) et gardée dans le trousseau de chaque appareil ([Le partage](sharing.md#what-it-protects-and-what-it-cannot-hide)).
