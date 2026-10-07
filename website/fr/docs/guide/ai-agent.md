---
description: Laisser un agent d’IA comme Claude Code ou Claude Desktop lire ce que Sioul garde et préparer tâches, événements, notes et brouillons, par MCP ; il n’envoie rien, ne supprime rien, ne paie rien et ne lit aucun mot de passe.
---

# Avec un agent d’IA {#using-an-ai-agent}

## En bref {#in-short}

Un agent d’IA dont vous vous servez déjà, comme Claude Code ou Claude Desktop, peut lire ce que Sioul garde sur cet appareil et vous préparer du travail : tâches, événements, notes, liens entre les choses, brouillons. Il explique et prépare ; vous relisez, et c’est vous qui envoyez. Il n’envoie jamais rien, ne supprime jamais rien, ne déplace jamais d’argent et ne lit jamais un mot de passe, et les codes à usage unique et les numéros de compte lui sont cachés. Sioul n’en connecte aucun de lui-même : cette page est pour le jour où vous le voulez.

## Avant d’en connecter un {#before-you-connect-one}

!!! warning "Ce qu’un agent voit"
    Une fois connecté, un agent peut lire tout ce que Sioul garde sur cet appareil : votre courrier, vos tâches, votre agenda, vos contacts, vos budgets, vos notes et vos projets, qu’ils servent au travail, à vos démarches ou aux loisirs. Ce qu’il lit part vers le modèle derrière lui (celui d’Anthropic, pour Claude), selon votre propre contrat avec cette entreprise. Ouvrir certains projets à un agent et en garder d’autres fermés n’existe pas encore.

Certaines choses lui restent cachées quoi qu’il arrive ([plus bas](#what-does-not-move)) : mots de passe et clés, codes à usage unique et liens de connexion, numéros de compte bancaire, de carte et de sécurité sociale, vos médicaments et vos prises.

## Le connecter {#connecting}

Sioul sert les agents par le Model Context Protocol (MCP), avec la commande `sioul mcp`. Il faut pour cela la ligne de commande `sioul` installée ([Installer](install.md#into-your-application-menu)).

=== "Claude Code"

    ```
    claude mcp add sioul -- sioul mcp
    ```

    Avec `--scope user` après `add`, Sioul est là dans tous les projets, pas seulement dans celui en cours. `claude mcp add sioul -- sioul --language fr mcp` met en français les phrases de Sioul lui-même.

=== "Claude Desktop"

    Dans le fichier `claude_desktop_config.json` de Claude Desktop (sous macOS dans `~/Library/Application Support/Claude/`, sous Windows dans `%APPDATA%\Claude\`), puis redémarrez Claude Desktop :

    ```json
    {
      "mcpServers": {
        "sioul": {
          "command": "/full/path/to/sioul",
          "args": ["mcp"]
        }
      }
    }
    ```

    Claude Desktop a besoin du chemin complet de `sioul` : `command -v sioul`, dans un terminal, l’affiche. Pour les phrases de Sioul en français : `"args": ["--language", "fr", "mcp"]`.

`sioul mcp` a été testé seul ; il n’a pas encore été essayé dans Claude Code ou Claude Desktop eux-mêmes. Si quelque chose ne marche pas, un mot dans [les tickets GitHub](https://github.com/aurelienpierre/sioul/issues) aide.

## Ce qu’un agent peut lire {#what-an-agent-can-read}

- **Le Porche**, file par file, seulement dans vos heures, sauf si vous lui demandez de l’ouvrir. On lui dit qu’un code est arrivé, jamais le code.
- **Votre courrier** : des messages trouvés par leur objet, leur expéditeur ou leur texte ; un message entier, avec qui l’a écrit, s’il est authentique ou falsifié et pourquoi, et le nom de ses pièces jointes.
- **Vos tâches** : la prochaine étape et pourquoi, le déroulé de la journée, chaque tâche ouverte dans l’ordre du plan.
- **Votre agenda**, vos **contacts**, vos **budgets** (chaque verdict, les réserves, ce que la veille sur l’argent a remarqué), vos **notes**, vos **projets**.
- **Ce à quoi chaque chose est liée**, dans les deux sens, et toute chose trouvée par quelques mots de son titre.

## Ce qu’un agent peut préparer {#what-an-agent-can-prepare}

Sur cet appareil seulement :

- **une tâche**, ou **une tâche marquée faite** : une tâche part vers votre serveur d’agenda à la prochaine synchronisation, comme une tâche créée dans la fenêtre ; une tâche marquée faite peut être rouverte depuis la fenêtre ;
- **un événement** : personne n’est invité ;
- **une note**, jamais écrite par-dessus une autre ;
- **une réponse**, ou **un nouveau message** : enregistré dans Brouillons, jamais envoyé. Vous le lisez, puis l’envoyez depuis la fenêtre, ou le supprimez ;
- **un lien** entre deux choses.

## Les outils de votre filtre à indésirables {#the-spam-filters-tools}

Un agent peut aussi s’occuper de votre propre filtre à indésirables, comme vous le feriez depuis le terminal : dire où il en est, le tester et lister ses pires erreurs par expéditeur et par objet (les codes et les numéros de compte masqués, comme partout), dire ce qu’il ferait maintenant de vos boîtes de réception, dire qu’un message est indésirable ou non quand vous le lui demandez (une ligne que chaque appareil lit), télécharger le début de votre courrier depuis vos serveurs pour en apprendre, et l’entraîner. Le téléchargement ne fait que lire : rien ne change sur vos serveurs. Un apprentissage est un essai sauf si vous demandez plus, et il ne remplace le filtre de tous vos appareils que si le nouveau ne prend pas plus de bon courrier pour indésirable que l’ancien. Ces outils sont actifs tant que vous ne les désactivez pas : pour les garder loin des agents, écrivez `spam = false` sous `[mcp]` dans la configuration de Sioul (`~/.config/sioul/config.toml`).

## Ce qui ne bouge pas {#what-does-not-move}

- **Rien n’est envoyé, supprimé ni payé.** Aucun outil n’envoie de courrier, ne supprime ni ne déplace un message, ni ne déplace d’argent. Les outils du filtre à indésirables sont les seuls à joindre vos serveurs de courrier, et seulement pour lire ([plus haut](#the-spam-filters-tools)).
- **Rien de secret.** Aucun mot de passe, aucune clé, aucun jeton n’est lu. Les codes à usage unique sont cachés partout où ils sont écrits, de même que les liens de connexion, de réinitialisation et de confirmation ; un message qui donne un mot de passe est retenu en entier. Les numéros de compte bancaire (IBAN), les numéros de carte et les numéros de sécurité sociale français sont masqués dans le courrier, les notes et les budgets : un IBAN ou un numéro de carte garde ses quatre derniers caractères, pour distinguer deux comptes ; un numéro de sécurité sociale est caché en entier. Vos médicaments et vos prises ne sont pas donnés à un agent.
- **Le courrier, ce sont des données.** Le texte d’un message arrive marqué comme les mots de son expéditeur, pas comme des instructions, et l’agent en est averti quand il se connecte. Le courrier falsifié est dit falsifié. Le courrier hostile envoyé à une adresse protégée, et le courrier chiffré, ne sont pas donnés.
- **Écrire ne fait qu’ajouter.** Une note n’en remplace jamais une autre ; un lien ne se fait qu’une fois.
- **Vos heures tiennent.** On demande à l’agent de les respecter, sauf si vous demandez autre chose.
- **Un journal.** Chaque appel est noté sur cet appareil, dans un fichier que vous seul pouvez lire, un par mois dans les dossiers de Sioul : quel agent a demandé, pour quoi, et les adresses de ce qui lui a été donné et sa longueur, jamais ses mots. Le journal reste ici ; le modèle de l’agent, lui, a reçu le texte.

## La ligne de commande {#the-command-line}

Les agents et les scripts peuvent aussi se servir de la ligne de commande, comme vous : `sioul tasks now`, `sioul tasks list`, `sioul links`, `sioul notes`, et d’autres (`sioul --help`).

`sioul tasks import plan.toml` prend un plan écrit dans un fichier : des tâches avec leurs étapes et ce qu’elles attendent, les contacts des organismes concernés, et des brouillons, qui restent dans Sioul et ne sont jamais envoyés. Importer à nouveau met à jour ce que le fichier a créé, au lieu de l’ajouter deux fois ; `--dry-run` dit ce qui changerait. Le format du fichier : [notes sur les tâches, « Importing » (en anglais)](https://aurelienpierre.github.io/sioul/dev/tasks.html#importing).

## L’autre IA dans Sioul {#the-other-ai-in-sioul}

Une seule autre fonction se sert d’une IA, et seulement pour une adresse que vous protégez contre le harcèlement, quand vous l’autorisez : **Laisser l’IA le lire d’abord** envoie à Claude, d’Anthropic, avec votre propre clé, chaque message de la boîte de réception de cette adresse, pour en dire le ton et le sujet. Il reçoit l’objet de chaque message et les 4 000 premiers caractères de son texte, **sans rien masquer** : contrairement à ce que reçoit un agent, rien n’y est caché, si bien qu’un code, un mot de passe ou un numéro de compte dans un tel message parvient aussi à Anthropic. Une fois la fonction activée, les messages qui se trouvent déjà dans cette boîte partent aussi, de n’importe qui. Voir [le Porche](porch.md#a-public-address-protected).

## Pas encore là {#not-there-yet}

- Ouvrir certains projets à un agent et en garder d’autres fermés.
- ChatGPT, et les agents qui tournent ailleurs que sur cet appareil.
- Un compagnon dans Sioul qui explique une lettre en mots simples, ligne par ligne, et vous tient compagnie pendant un moment de démarches.

## Côté technique {#for-technical-readers}

- **Le protocole** : le Model Context Protocol, révision 2025-06-18 (2025-03-26 et 2024-11-05 acceptées aussi), JSON-RPC 2.0, par l’entrée et la sortie standard seulement : l’agent tourne sur cet appareil, et aucun port réseau n’est ouvert.
- **Les indications des outils** : chaque outil dit s’il écrit, et aucun n’est destructeur (`readOnlyHint`, `destructiveHint: false`, `idempotentHint` de MCP ; `openWorldHint` seulement pour le téléchargement du filtre à indésirables, qui lit vos serveurs de courrier).
- **Les masques** : chaque numéro n’est caché que si sa propre vérification tient, pour que les numéros de commande et de téléphone restent : les IBAN par le mod 97 de l’ISO 13616, les numéros de carte de 16 ou 19 chiffres par la clé de Luhn (quatorze chiffres sont laissés tels quels : un SIRET a aussi une clé de Luhn, et n’est pas un secret), les numéros de sécurité sociale français par leur clé. Les codes sont cachés partout où ils sont écrits, espacés ou non ; les liens de réinitialisation, de connexion et de confirmation sont cachés ; un message qui donne un mot de passe est retenu en entier.
- **L’encadrement** : le texte d’un message ou d’une note arrive entre deux lignes portant une marque faite pour cette réponse, et les instructions du serveur disent qu’un tel texte est une donnée, jamais une instruction.
- **Jamais donnés** : le courrier hostile envoyé à une adresse protégée, le courrier chiffré, les médicaments et les prises, les journées de votre montre. La journée organisée (`list_tasks`, aujourd’hui) montre les heures des repas et des siestes, comme la journée dans les Tâches.
- **Le journal** : `$XDG_STATE_HOME/sioul/mcp/AAAA-MM.jsonl`, une ligne JSON par appel, écrite en 0600 dans un dossier en 0700 sous Linux et macOS. Il contient l’heure, l’agent, l’outil, ses arguments (200 caractères chacun, un corps ou des notes par leur seule longueur, les numéros de compte et de carte masqués), et les adresses (`mid:…`, `sioul:…`) et la longueur de ce qui a été donné.
- **Désactiver des outils** : `[mcp] spam = false` garde les outils du filtre à indésirables loin des agents, ni listés ni acceptés.
- **L’IA du bouclier**, à part des agents : Claude Haiku 4.5 par l’API d’Anthropic, avec votre clé tirée du trousseau ; l’objet et les 4 000 premiers caractères de chaque message de la boîte de réception d’une adresse protégée, sans rien masquer ; aucune redirection suivie, pour que la clé n’aille qu’à Anthropic.

La conception derrière tout cela : [fournisseurs d’IA et agents (en anglais)](https://aurelienpierre.github.io/sioul/dev/ai.html) et [le serveur MCP (en anglais)](https://aurelienpierre.github.io/sioul/dev/mcp.html).
