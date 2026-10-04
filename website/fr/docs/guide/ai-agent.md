---
description: Laisser un agent d’IA comme Claude Code ou Claude Desktop lire ce que Sioul garde et préparer tâches, événements, notes et brouillons, par MCP ; il n’envoie rien, ne supprime rien, ne paie rien et ne lit aucun mot de passe.
---

# Avec un agent d’IA {#using-an-ai-agent}

Un agent d’IA dont vous vous servez déjà, comme Claude Code ou Claude Desktop, peut lire ce que Sioul garde sur cet ordinateur et vous préparer du travail : tâches, événements, notes, liens entre les choses, brouillons. Il explique et prépare ; vous relisez. Il n’envoie jamais rien, ne supprime jamais rien, ne déplace jamais d’argent, et ne lit jamais un mot de passe. C’est toujours vous qui envoyez.

Sioul n’en connecte aucun de lui-même. Cette page est pour le jour où vous le voulez.

## Avant d’en connecter un {#before-you-connect-one}

!!! warning "Ce qu’un agent voit"
    Une fois connecté, un agent peut lire tout ce que Sioul garde sur cet ordinateur : votre courrier, vos tâches, votre agenda, vos contacts, vos budgets, vos notes et vos projets, qu’ils servent au travail, à vos démarches ou aux loisirs. Ce qu’il lit part vers le modèle derrière lui (celui d’Anthropic, pour Claude), selon votre propre contrat avec cette entreprise. Ouvrir certains projets à un agent et en garder d’autres fermés n’existe pas encore.

Certaines choses lui restent cachées quoi qu’il arrive (plus bas) : mots de passe et clés, codes à usage unique, numéros de compte bancaire et de carte.

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

Sur cet ordinateur seulement :

- **une tâche**, ou **une tâche marquée faite** : une tâche part vers votre serveur d’agenda à la prochaine synchronisation, comme une tâche créée dans la fenêtre ; une tâche marquée faite peut être rouverte depuis la fenêtre ;
- **un événement** : personne n’est invité ;
- **une note**, jamais écrite par-dessus une autre ;
- **une réponse**, ou **un nouveau message** : enregistré dans Brouillons, jamais envoyé. Vous le lisez, puis l’envoyez depuis la fenêtre, ou le supprimez ;
- **un lien** entre deux choses.

## Ce qui ne bouge pas {#what-does-not-move}

- **Rien n’est envoyé, supprimé ni payé.** Aucun outil n’envoie de courrier, ne supprime ni ne déplace un message, ne déplace d’argent, ni ne parle à un serveur.
- **Rien de secret.** Aucun mot de passe, aucune clé, aucun jeton n’est lu. Les codes à usage unique sont cachés partout où ils sont écrits. Les numéros de compte bancaire (IBAN), les numéros de carte et les numéros de sécurité sociale français sont masqués dans le courrier, les notes et les budgets ; leurs quatre derniers caractères restent, pour distinguer deux comptes.
- **Le courrier, ce sont des données.** Le texte d’un message arrive marqué comme les mots de son expéditeur, pas comme des instructions, et l’agent en est averti quand il se connecte. Le courrier falsifié est dit falsifié. Le courrier hostile envoyé à une adresse protégée, et le courrier chiffré, ne sont pas donnés.
- **Écrire ne fait qu’ajouter.** Une note n’en remplace jamais une autre ; un lien ne se fait qu’une fois.
- **Vos heures tiennent.** On demande à l’agent de les respecter, sauf si vous demandez autre chose.
- **Un journal.** Chaque appel est noté sur cet ordinateur : quel agent a demandé, pour quoi, et le texte qui lui a été donné, un fichier par mois dans les dossiers de Sioul. Ce journal reste ici ; le modèle de l’agent, lui, a reçu ce texte.

## La ligne de commande {#the-command-line}

Les agents et les scripts peuvent aussi se servir de la ligne de commande, comme vous : `sioul tasks now`, `sioul tasks list`, `sioul links`, `sioul notes`, et d’autres (`sioul --help`).

`sioul tasks import plan.toml` prend un plan écrit dans un fichier : des tâches avec leurs étapes et ce qu’elles attendent, les contacts des organismes concernés, et des brouillons, qui restent dans Sioul et ne sont jamais envoyés. Importer à nouveau met à jour ce que le fichier a créé, au lieu de l’ajouter deux fois ; `--dry-run` dit ce qui changerait. Le format du fichier : [notes sur les tâches, « Importing » (en anglais)](https://aurelienpierre.github.io/sioul/dev/tasks.html#importing).

## L’autre IA dans Sioul {#the-other-ai-in-sioul}

Une seule autre fonction se sert d’une IA, et seulement pour une adresse que vous protégez contre le harcèlement, quand vous l’autorisez : **Laisser l’IA le lire d’abord** envoie chaque nouveau message reçu à cette adresse à Claude, d’Anthropic, pour en dire le ton et le sujet. Voir [le Porche](porch.md#a-public-address-protected).

## Pas encore là {#not-there-yet}

- Ouvrir certains projets à un agent et en garder d’autres fermés.
- ChatGPT, et les agents qui tournent ailleurs que sur cet ordinateur.
- Un compagnon dans Sioul qui explique une lettre en mots simples, ligne par ligne, et vous tient compagnie pendant un moment de démarches.

La conception derrière tout cela : [fournisseurs d’IA et agents (en anglais)](https://aurelienpierre.github.io/sioul/dev/ai.html) et [le serveur MCP (en anglais)](https://aurelienpierre.github.io/sioul/dev/mcp.html).
