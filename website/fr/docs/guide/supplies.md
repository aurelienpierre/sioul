---
description: Les stocks dans Sioul – ce que vous utilisez ou vendez (timbres, enveloppes, cartes, nourriture du chat) et combien il en reste, compté à partir de ce qui est arrivé et de ce que vos tâches ont utilisé ; une tâche pour réapprovisionner créée une fois quand l’un baisse, jamais un chiffre en rouge.
---

# Les stocks {#supplies}

## En bref {#in-short}

**Stocks** garde ce que vous utilisez ou vendez : timbres, enveloppes, cartes imprimées à la main, nourriture du chat. Chaque article dit combien il en reste, compté à partir de ce qui est arrivé et de ce que vos tâches ont utilisé. Une tâche peut dire ce qu’elle utilisera (« 1 timbre, 1 enveloppe, 1 carte ») ; la cocher faite le retire du stock, sans question, et une pression le change quand une deuxième enveloppe y est passée. Quand ce qui reste baisse, une tâche pour réapprovisionner est créée, une fois, dans la liste que vous choisissez. Rien ne devient rouge, rien ne compte ce qui est bas.

## Protégé par défaut {#what-is-protected}

- Vos stocks sont un fichier lisible sur votre appareil, parmi les propres listes de Sioul, et ce qui est arrivé et parti, un fichier par appareil à côté (tant que vous n’avez pas déplacé les listes de Sioul, les deux sont dans votre dossier de notes : [Où Sioul garde ses listes](sharing.md#where-sioul-keeps-its-lists)). Il n’y a pas de serveur de Sioul.
- Entre vos appareils, ils voyagent scellés par le partage de Sioul, dans **Projets et argent** : le dossier et son serveur voient que quelque chose a changé, jamais quoi ([Le partage](sharing.md)). Chaque appareil n’écrit que son propre fichier de mouvements : deux appareils n’écrivent jamais l’un par-dessus l’autre.
- Ce qu’une tâche prévoit d’utiliser et ce qu’elle a utilisé sont écrits dans la tâche elle-même, et voyagent donc avec elle jusqu’à votre serveur d’agendas : les autres programmes gardent ces lignes, Google Tasks non.
- Les tâches pour réapprovisionner vont dans la liste que vous choisissez : si cette liste est sur votre serveur d’agendas, leurs titres (« Réapprovisionner : timbres ») y sont aussi.

## La page {#the-page}

**Stocks** est parmi les lieux, après Projets.

- **Pour maintenant**, d’abord : les articles de ce à quoi sert ce moment (le travail aux heures de travail, vos démarches à leurs heures, les loisirs le reste du temps : [Les heures](hours.md)), par catégorie, une ligne chacun : son nom, combien il en reste une fois que les tâches ouvertes ont ce qu’elles prévoient (« reste 9 »), « prévu : 4 » quand il y en a, et un discret « stock bas » ou « épuisé ». Combien sont gardés pour d’autres moments tient en une ligne dessous.
- **Tous les articles**, à un onglet : chaque article quelle que soit l’heure, avec ce qui est en stock, son seuil et ses prix ; les archivés repliés à la fin.
- **⚙** : **Les tâches pour réapprovisionner vont dans** : une liste de tâches, sinon là où vont les démarches de Santé.

## Un article {#a-supply}

**Nouvel article**, puis :

- **Nom** : « Timbres », « Cartes cadeaux ».
- **Catégorie** : un mot à vous (« Poste », « Boutique ») ; celles en usage sont proposées sous le champ.
- **À quoi il sert** : le travail, vos démarches, les loisirs, comme pour une tâche ; rien de coché, il se montre à toute heure.
- **Unité** : rien pour une pièce, « g », « m », « feuilles ».
- **Prix d’achat** et **Prix de revente**, à l’unité, comme vous les écrivez.
- **Seuil** : en dessous de combien il est bas, de ce qui reste une fois que les tâches prévues ont le leur ; vide, jamais bas.
- **Acheté d’habitude par** : combien arrivent par commande : ce qu’apporte une tâche pour réapprovisionner une fois faite.
- **Ses tâches pour réapprovisionner vont dans le projet** : facultatif.
- **Combien il y en a maintenant** : le premier compte.

Une pression sur un article ouvre sa fiche :

- **Ce que c’est** : ses champs, chacun enregistré quand vous le changez.
- **En stock** : le nombre, avec **Reçu…** (tant sont arrivés) et **Recompter…** (il y en a tant maintenant). Recompter est la façon de remettre un nombre juste, à tout moment ; rien ne vous demande jamais d’en vérifier un. Un stock sous zéro est montré tel quel, avec ce mot.
- **Prévu** : les tâches ouvertes qui l’utiliseront, chacune ouvrant sa tâche.
- **Historique** : ce qui est arrivé et parti, le plus récent d’abord, chacun disant pourquoi et ce qui restait après (« Utilisé : 1 → 11 », avec sa tâche) ; **Tout l’historique** montre au-delà des dix derniers.
- **Archiver** : un article qui a un historique n’est jamais supprimé ; archivé, il quitte les listes, et **Le reprendre** le ramène.

## Les tâches qui utilisent des stocks {#tasks-that-use-supplies}

- **Dans le formulaire de la tâche**, la section **Stocks** (dès que vous en gardez) : un article et combien, une ligne chacun, **Ajouter un article**. C’est ce qu’elle prévoit d’utiliser : la page le compte aussitôt comme prévu, si bien que dix ventes arrivant avec huit cartes en stock rendent les cartes « épuisé » à leur arrivée, pas quand la huitième carte part.
- **Cochée faite**, où que vous la cochiez (une liste, Maintenant, sa fiche, la fenêtre de concentration, un agent, la ligne de commande, votre téléphone, un autre programme), ce qu’elle prévoyait est retiré du stock, sans question. La ligne d’état le dit : « C’est fait. Utilisé : 1 Timbres, 1 Enveloppes, 1 Cartes cadeaux. », avec **Changer…** à côté de **Comment c’était ?**, pour une deuxième enveloppe ou un article que personne n’avait prévu ; pour une tâche qui ne prévoyait rien, **Articles utilisés…**.
- **Sur une tâche faite**, sa fiche garde la section **Articles utilisés**, chaque ligne modifiable, **Ajouter un article** : à remettre juste des jours plus tard si vous voulez.
- **Rouverte**, ses stocks reviennent.
- **Une tâche qui revient** (chaque semaine) prend ce qu’elle prévoit à chaque tour fait ; **Changer…** remet ce tour juste.

## Quand l’un baisse {#when-one-runs-low}

Quand ce qui reste d’un article est à son **Seuil** ou dessous, et qu’aucune tâche pour le réapprovisionner n’est ouverte, une est créée, une fois : « Réapprovisionner : timbres », avec l’étiquette `supplies`, pour ce à quoi sert l’article, dans son projet quand il en a un, dans la liste de **Les tâches pour réapprovisionner vont dans**. La ligne d’état dit où elle est allée. Le plan la place dans les heures de ce à quoi elle sert, comme toute tâche. Supprimée, elle n’est pas recréée avant le prochain compte ou la prochaine arrivée.

Cochée faite ici, elle demande **Combien en sont arrivés ?**, rempli avec **Acheté d’habitude par**. Faite dans un autre programme, ce qu’il est acheté d’habitude par arrive tout seul ; quand il n’en dit rien, la fiche de l’article le demande une fois.

## Pour aller plus loin {#going-further}

- **À la main, si vous voulez** : la liste est un fichier lisible, `sioul-supplies.toml` ; sa forme est [plus bas](#for-technical-readers).
- **Un agent IA que vous connectez** ([Utiliser un agent IA](ai-agent.md)) lit ce qu’une tâche prévoit d’utiliser et ce qu’elle a utilisé, là où il lit la tâche ; y marquer une tâche faite retire ses stocks comme ici. Il n’a pas d’outil à lui pour les stocks.
- **Sur un appareil pas encore mis à jour** : les stocks ne voyagent qu’une fois que chacun de vos appareils a un Sioul qui les connaît ; d’ici là, chaque appareil garde les siens.

## Pourquoi ça marche ainsi {#why-it-works-this-way}

Un nombre tenu à la main se trompe en silence, et le vérifier devient une corvée de plus. Sioul garde donc ce qui s’est passé (ce qui est arrivé, ce que chaque tâche a utilisé, chaque compte) et en tire le nombre : chaque nombre s’explique par son historique, une tâche rouverte rend ses stocks, et deux appareils qui en utilisent chacun n’écrivent jamais l’un par-dessus l’autre. « Stock bas » se dit de ce qui reste une fois que les tâches prévues ont le leur, pour que l’avertissement vienne quand les commandes arrivent, pas quand la dernière carte est partie. Et il se dit d’un mot, jamais d’un chiffre rouge à combattre : la tâche pour réapprovisionner entre dans le plan, où elle se fait en son temps.

Aucune autre application n’a été comparée pour cette page.

## Pour les lecteurs techniques {#for-technical-readers}

`sioul-supplies.toml`, à côté de `sioul-projects.toml` :

```toml
[[supply]]
id = "stamps"          # fait une fois à partir de son nom ; jamais changé
name = "Timbres"
category = "Poste"
area = "work"          # work, admin, leisure, joints par "+" ; rien : à toute heure
unit = ""              # rien : une pièce
buy = "1.39"
sell = ""
low_at = 10            # rien : jamais bas
bought_by = 50
project = ""
archived = false
```

Ce qui est arrivé et parti est dans `sioul-supplies-moves/`, un fichier par appareil, chacun écrit par son seul appareil ; les stocks d’une tâche sont des lignes de la tâche (`X-SIOUL-SUPPLY`, `X-SIOUL-SUPPLY-USED`). Les règles, la forme des fichiers et comment ils voyagent : [les stocks, pour les développeurs (en anglais)](https://sioul.aurelienpierre.com/dev/supplies.html).
