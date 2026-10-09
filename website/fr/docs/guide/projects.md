---
description: Les projets dans Sioul – toute affaire que vous suivez, pour un client ou pour vous, qui rassemble ses tâches, ses événements, son courrier, ses notes, son temps et ses factures sur une ligne de temps ; son courrier rangé tout seul à son arrivée, une fois que Sioul a vérifié qui l’envoie.
---

# Les projets {#projects}

## En bref {#in-short}

Un projet, c’est toute affaire que vous suivez : un travail pour un client, dont le temps se facture, ou une affaire à vous, comme le logement, la santé, une déclaration d’impôts ou un voyage. Sa page rassemble ce qui lui appartient : ses tâches, ses événements, son courrier, ses notes, le temps que vous y avez passé et ses factures, sur une ligne de temps, avec d’abord ce qui vient. À l’arrivée du courrier, Sioul le range là selon les règles que vous donnez au projet, après avoir vérifié qui l’envoie. La liste des projets est un seul fichier lisible, dans votre propre dossier de notes.

<figure markdown="span">
  [![La page Projets : trois projets listés à gauche, avec Nouveau projet ; l’un ouvert à droite : « Pour un client », son taux, Tableau, Liste, Calendrier, Noter du temps et Faire la facture ; ses tâches ouvertes et faites, le temps noté et à facturer ; « Le courrier qui arrive ici tout seul », replié ; ses factures, l’une marquée Payée ; puis « Sur une ligne de temps », ce qui vient et ce qui s’est passé avant.](../assets/screens/fr/projects.png){ loading=lazy }](../assets/screens/fr/projects.png "Ouvrir l’image en grand")
  <figcaption>Un projet : ce qui vient, puis ce qui s’est passé.</figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

- **Le courrier est vérifié avant d’être rangé.** À son arrivée, un message falsifié (son domaine dit ne pas l’avoir envoyé), ou un message qui emprunte un nom connu, est mis de côté d’abord, et jamais rangé dans un projet, même quand son adresse ressemble à celle de votre client. La page du projet vérifie son courrier de la même façon chaque fois qu’elle le montre, comme ce qu’un agent IA lit du projet : un tel message n’apparaît jamais sur sa ligne du temps, pas plus qu’une conversation que seul un tel message y rattache.
- **Un message qui échoue aux vérifications de l’expéditeur** (les deux signatures par lesquelles les serveurs se portent garants d’un expéditeur ont échoué) n’est jamais rangé par une règle sur l’adresse ou le domaine de votre client. Une règle sur des mots de l’objet, du texte ou du nom d’une pièce jointe peut encore le ranger, puisque ceux-ci ne reposent pas sur qui l’a envoyé ; sur la page du projet, il dit alors « dont l’adresse n’a pas pu être vérifiée ».
- **Chaque message rangé dit pourquoi il est venu** : « domaine de l’expéditeur : example.org ».
- **Vos projets sont un seul fichier lisible** dans votre propre dossier de notes, et le courrier est rangé sur votre appareil. Entre vos appareils, ce fichier voyage avec votre dossier de notes, ou scellé par le partage de Sioul une fois **Projets et argent** activé : le dossier et son serveur voient que quelque chose a changé, jamais quoi ([Le partage](sharing.md)).
- Il n’y a pas de serveur de Sioul, et rien de vos projets n’y va.

## Créer un projet {#making-a-project}

**Nouveau projet** (ou **Nouveau ▾ ▸ Un projet**) :

- **Nom**.
- **Pour un client : son temps se facture à un taux horaire**. Puis : **Pour** (pour qui), **Une heure coûte** (sinon le taux de [Paramètres ▸ Factures](settings.md#invoices)), et **Factures attendues dans** : le budget où l’argent est attendu jusqu’au paiement. Quand **Pour** nomme l’un de vos [contacts](contacts.md) (son nom, ou son organisation), les factures prennent son adresse postale.
- Sans cela, le projet est à vous. **À vous, hors travail** garde ses tâches en vue pendant le calme, avec la famille et les amis.
- **Où il en est** : ouvert, en attente ou fermé.

**Retirer de la liste** enlève un projet. Ses tâches, notes, courriels et temps restent où ils sont ; ils ne sont plus rassemblés sous lui.

## La page d’un projet {#a-projects-page}

À gauche, tous les projets, avec leurs tâches ouvertes et le temps qui reste à facturer. À droite, le projet ouvert :

- **Sur une ligne de temps** : d’abord ce qui est **À venir** (les dates demandées de ses tâches ouvertes, les événements), puis ce qui s’est passé **Avant**, du plus récent au plus ancien : tâches faites, courrier, notes modifiées, temps noté, factures.
- **Ses tâches**, en **Tableau**, en **Liste** ou en **Calendrier** : chacun ouvre la page [Tâches](tasks.md), limitée à ce projet. Le **Calendrier** est celui de la page Tâches ([Liste, Tableau, Calendrier](tasks.md#list-board-timeline)) : chaque tâche sur ses jours, la date demandée en petit losange.
- **Noter du temps** : une réunion, un appel, un travail fait loin du minuteur.
- **Faire la facture**, quand du temps facturable attend d’être facturé. Voir [Le temps et les factures](time.md).
- **Ajouter ▾** et **Lier à…**, au bas : quelque chose de nouveau lié au projet, ou un lien vers ce qui existe, comme le contact du client.

## Le courrier qui arrive ici tout seul {#mail-that-comes-here-by-itself}

**Le courrier qui arrive ici tout seul**, sur la page du projet, donne au projet ses règles. Un message va au projet quand une règle correspond ; chaque ligne remplie dans une règle doit correspondre :

- **Des domaines** : le domaine de l’expéditeur, ou l’un de ses sous-domaines ;
- **Des adresses** ;
- **Mots dans le sujet** ;
- **Mots dans le texte** ;
- **Mots dans le nom d’une pièce jointe** (« devis » prend `Devis_2026.pdf`).

Plusieurs mots ou domaines sur une ligne : n’importe lequel d’entre eux. Les mots se comparent entiers, sans tenir compte des majuscules ni des accents.

Une réponse dans une conversation du projet va avec lui, comme un message que vous y liez à la main (**Lier à…**) : ce que vous liez reste tel que vous l’avez lié, même un message que les vérifications mettraient de côté. Chaque message rangé garde sa raison, montrée sur demande : « domaine de l’expéditeur : example.org ».

À son arrivée, et chaque fois que la page du projet le montre, le courrier est vérifié avant qu’une règle le range ([plus haut](#what-is-protected)) : le courrier falsifié et celui qui emprunte un nom connu sont mis de côté d’abord, et une règle sur l’adresse ou le domaine de votre client ne range pas un message qui a échoué aux vérifications de l’expéditeur. Dans l’autre sens, une règle de projet l’emporte sur une marque d’indésirable : le vrai courrier de votre client ne se perd pas parmi les indésirables, et le propre filtre à indésirables de Sioul laisse en paix les conversations d’un projet.

## Où vivent les projets {#where-projects-live}

Dans un seul fichier, `sioul-projects.toml`, à la racine de votre dossier de notes : lisible, et modifiable à la main si vous voulez. Il voyage avec votre dossier de notes, ou par le partage de Sioul une fois **Projets et argent** activé dans le partage ([Le partage](sharing.md)). Les tâches et les événements d’un projet portent son identifiant dans un champ standard, pour que les autres programmes d’agenda gardent le regroupement.

Si votre dossier de notes contient encore `sioul-cases.toml`, le nom du fichier d’avant le nom unique des projets, rien ne change : Sioul le lit et l’écrit tel quel, pour qu’un appareil pas encore mis à jour le lise aussi. Quand chacun de vos appareils a cette version, **Renommer en sioul-projects.toml**, dans Paramètres ▸ Votre dossier et le partage, le renomme, en gardant une copie de l’ancien fichier à côté. Les liens vers un projet écrits avant l’ouvrent toujours.

## Pendant le calme {#in-quiet-time}

Les projets pour des clients se reposent en dehors des heures de travail : « Les projets de travail se reposent jusqu’au retour du travail. » **Montrer quand même** les montre malgré tout. Pendant le sommeil, toute la page attend derrière une phrase et **Montrer quand même**. Voir [Les heures](hours.md).

## Pour aller plus loin {#going-further}

- **Une file sur le Porche** : **Projets montrés ici**, dans le ⚙ du Porche, donne à un projet sa propre file ([Le Porche](porch.md)).
- **À vous, hors travail** : un projet à vous, marqué ainsi, reste en vue pendant le calme, quand les projets de travail se reposent.
- **À la main, si vous voulez** : la liste des projets est un fichier lisible, `sioul-projects.toml`, que vous pouvez modifier ; sa forme est [plus bas](#for-technical-readers).
- **Un agent d’IA que vous connectez** ([Avec un agent d’IA](ai-agent.md)) peut lire la page d’un projet telle que la fenêtre la montre, les codes à usage unique, les liens de connexion, les numéros de compte et de carte masqués dans son courrier. Il n’y change rien.

## Comparé à d’autres applications {#compared-with-other-apps}

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

| | Sioul | Trello | Asana | Notion | Nextcloud Deck | Vikunja |
|---|---|---|---|---|---|---|
| Le courrier rangé tout seul dans le projet, selon vos règles | ✓ | en partie¹ | en partie² | ?³ | ✗⁴ | ?⁵ |
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

1. Vous envoyez ou transférez un message à l’adresse propre au tableau ; il devient une carte.
2. Depuis Gmail ou Outlook, à la main, avec le module d’Asana. Une adresse de courriel propre à un projet n’a pas pu être confirmée : le centre d’aide d’Asana n’a pas pu être lu.
3. Les pages de Notion Mail n’ont pas pu être lues.
4. Absent de la documentation de Deck. Nextcloud Mail peut faire une tâche d’agenda à partir d’un message, à la main, et ses filtres déplacent le courrier dans des dossiers, pas dans des tableaux.
5. Absent de la page des fonctions de Vikunja.
6. Les vues Tableau (Table), Calendrier et Frise sont dans les offres Premium et Enterprise ; le Planner, sur un calendrier, dans les offres payantes.
7. Absent de la documentation d’utilisation de Deck, qui décrit des tableaux faits de piles de cartes.
8. Le bouton **Calendrier** du projet ouvre le calendrier de la page Tâches.
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

D’autres en font plus pour le travail à plusieurs : chacune des autres applications ici partage un projet avec d’autres personnes, et les nombreuses vues et automatisations de Notion et d’Asana, comme les intégrations de Trello et d’Asana, vont bien au-delà de ce que fait Sioul. Ce que Sioul garde en un seul endroit, c’est le courrier du client rangé tout seul, le temps noté et les factures faites de ce temps, avec ses tâches gardées comme des tâches CalDAV standard.

??? info "Sources"
    Toutes lues le 8 octobre 2026.

    - **Trello**, la page d’Atlassian sur la création de cartes par courriel, et ses prix et offres : <https://support.atlassian.com/trello/docs/creating-cards-by-email/>, <https://trello.com/pricing>
    - **Asana**, ses prix et offres, et sa page sur les intégrations de messagerie : <https://asana.com/pricing>, <https://asana.com/apps/email>
    - **Notion**, ses pages d’aide sur les vues, les tâches et dépendances, les relations et les agrégations, et ses prix : <https://www.notion.com/help/views-filters-and-sorts>, <https://www.notion.com/help/tasks-and-dependencies>, <https://www.notion.com/help/relations-and-rollups>, <https://www.notion.com/pricing>
    - **Nextcloud Deck**, son README, sa documentation d’utilisation et son code CalDAV (`lib/DAV/Calendar.php`, `CalendarObject.php`), et le manuel d’utilisation de Nextcloud Mail : <https://github.com/nextcloud/deck>, <https://deck.readthedocs.io/en/latest/User_documentation_en/>, <https://github.com/nextcloud/deck/blob/main/lib/DAV/Calendar.php>, <https://docs.nextcloud.com/server/latest/user_manual/en/groupware/mail.html>
    - **Vikunja**, sa page des fonctions et ses pages sur CalDAV et les relations entre tâches : <https://vikunja.io/features/>, <https://vikunja.io/help/caldav/>, <https://vikunja.io/help/task-relations/>

## Côté technique {#for-technical-readers}

### Le fichier {#the-file}

`sioul-projects.toml`, à la racine de votre dossier de notes : du TOML, un `[[project]]` par projet (`id`, `title`, `kind = "project"` pour celui d’un client, `client`, `rate`, `budget`, `status`, `ai`), chacun avec ses lignes `[[project.route]]`. Le premier nom du fichier, `sioul-cases.toml`, avec `[[case]]` et `[[case.route]]`, se lit de même et garde ses noms quand Sioul l’écrit ; le renommer vérifie que rien ne change avant d’écrire, et garde `sioul-cases.toml.before-rename`. Les tâches et les événements d’un projet portent son `id` dans `REFID` (RFC 9253 §8.3), pour que les autres programmes CalDAV gardent le regroupement.

### Comment les règles correspondent {#how-routes-match}

- `from_domains` : le domaine de l’expéditeur ou l’un de ses sous-domaines (« example.org » prend « mail.example.org », jamais « notexample.org ») ; `from_addresses` : exactes, sans tenir compte de la casse ; `subject_contains`, `text_contains`, `attachment_contains` : des mots entiers, sans tenir compte de la casse ni des accents. Chaque liste remplie doit correspondre ; n’importe quel élément d’une liste suffit.
- Le texte est lu dans ses 6 000 premiers caractères (la partie HTML, lue comme du texte, quand la partie texte n’est qu’une ébauche).
- Les conversations suivent `In-Reply-To` et `References`, à travers les seuls messages que les vérifications laissent passer : une réponse falsifiée ne se montre pas et ne joint pas deux conversations. Un message rangé à son arrivée est lié à son projet (`links.toml` : `mid:` ↔ `sioul:project/<id>` ; `sioul:case/<id>`, écrit avant, se lit de même), et y reste quand les règles changent. Changer les règles d’un projet lie le courrier déjà là que ses règles sur le texte ou les pièces jointes prennent, après les mêmes vérifications.

### La sécurité, précisément {#security-precisely}

- À l’arrivée du courrier, sur le Porche, sur la page d’un projet et dans ce qu’un agent lit d’un projet, une seule fonction décide (`porch::admission`), dans l’ordre de la protection : un expéditeur bloqué reste bloqué ; le courrier falsifié (DMARC en échec sous une politique `quarantine` ou `reject`) et celui dont le nom affiché emprunte un nom connu sont mis de côté ; les codes à usage unique gardent leur file ; puis viennent les règles des projets. Les résultats d’authentification viennent du `Authentication-Results` de votre propre fournisseur (RFC 8601, seulement d’un `authserv-id` auquel vous faites confiance) et de la vérification que fait Sioul de chaque message à sa réception ([Chaque message vérifié](privacy-security.md#every-message-checked)).
- Le courrier non authentifié (SPF et DKIM tous deux en échec, aucun succès DMARC, aucun sceau ARC d’un relais de confiance) est rangé sans son expéditeur : `from_domains` et `from_addresses` ne peuvent pas le prendre, tandis que les règles sur l’objet, le texte et les pièces jointes le peuvent encore. Un message sans résultats, ou dont un seul des deux échoue, garde son expéditeur : une règle sur l’expéditeur se fie alors à l’adresse telle qu’elle est écrite.
- Une règle qui correspond l’emporte sur la marque d’indésirable du fournisseur, et le propre filtre à indésirables de Sioul laisse en paix les conversations d’un projet.
- La partie **Projets et argent** du partage scelle le fichier des projets (`sioul-projects.toml`, ou `sioul-cases.toml`) avec les fichiers des budgets, de la banque et des contrats : chaque enregistrement avec XChaCha20-Poly1305, sous une clé tirée de votre phrase de passe par Argon2id (64 Mio, 3 passes) et gardée dans le trousseau de chaque appareil ([Le partage](sharing.md#what-it-protects-and-what-it-cannot-hide)).
