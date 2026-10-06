---
title: Politique de confidentialité
description: "La politique de confidentialité de Sioul : Sioul fonctionne sur votre appareil et n’a pas de serveur ; ce qu’il lit et écrit dans votre compte Google, où c’est gardé, ce qui est partagé, et comment retirer l’accès."
---

# Politique de confidentialité {#privacy-policy}

Cette page traduit la politique de confidentialité de Sioul ; en cas de doute, la [version anglaise](https://aurelienpierre.github.io/sioul/privacy.html) fait foi.

*Dernière mise à jour le 5 octobre 2026 (2026-10-05).*

Sioul est une application de bureau libre et à code source ouvert, publiée par Aurélien Pierre sous la licence GPL-3.0-or-later. Cette politique dit ce que Sioul fait de vos données, et en particulier des données qu’il reçoit de Google quand vous vous connectez avec un compte Google.

## En bref {#in-short}

- **Sioul fonctionne sur votre appareil. Il n’y a pas de serveur Sioul.**
- Vos données restent sur votre appareil et dans vos propres comptes : votre fournisseur de courrier, votre serveur d’agendas, votre compte Google.
- Rien n’est envoyé au développeur de Sioul. Rien n’est vendu, rien ne sert à la publicité, rien ne sert à constituer des profils.
- Les données ne quittent votre appareil que vers les services que vous ajoutez, et vers les quelques autres décrits plus bas, chacun pour une fonction que vous choisissez.

Tout le monde peut lire le code source de Sioul, pour vérifier ce que dit cette politique : [github.com/aurelienpierre/sioul](https://github.com/aurelienpierre/sioul).

## Qui est responsable {#who-is-responsible}

Sioul est fait et publié par Aurélien Pierre. Comme Sioul n’a pas de serveur, son développeur ne reçoit, ne garde ni ne voit jamais vos données. Pour toute question sur cette politique : voir [Contact](#contact).

## Comment Sioul fonctionne {#how-sioul-works}

Sioul se connecte, depuis votre appareil, aux services que vous lui ajoutez : votre fournisseur de courrier (IMAP et SMTP), votre serveur d’agendas et de contacts (CalDAV et CardDAV), Google. Il garde une copie de ce qu’il récupère dans des fichiers de vos propres dossiers d’utilisateur, pour pouvoir fonctionner sans attendre le réseau. Les mots de passe et les jetons d’accès vont dans le trousseau de votre système (le Secret Service sur Linux, le Gestionnaire d’identification sur Windows, le Trousseau d’accès sur macOS), jamais dans un fichier.

## Agendas, contacts et tâches Google {#google-calendars-contacts-and-tasks}

Quand vous ajoutez un compte Google dans Sioul (Comptes ▸ Ajouter un compte ▸ Agendas, contacts et tâches Google), vous vous connectez sur la page de Google elle-même, dans votre navigateur. Google vous demande alors votre autorisation pour ces champs d’application :

| Champ d’application | Ce que Sioul en fait |
|---|---|
| `openid`, `email` | Lit l’adresse du compte Google utilisé pour vous connecter, pour vérifier que c’est bien celle que vous avez donnée. Rien d’autre. |
| `https://www.googleapis.com/auth/calendar` | Lit vos agendas et leurs événements, par l’interface CalDAV de Google, pour les montrer dans l’agenda de Sioul. Écrit les événements que vous créez, modifiez ou supprimez dans Sioul. |
| `https://www.googleapis.com/auth/carddav` | Lit vos contacts, par l’interface CardDAV de Google, pour les montrer dans les contacts de Sioul. Écrit les contacts que vous créez, modifiez, déplacez ou supprimez dans Sioul. |
| `https://www.googleapis.com/auth/tasks` | Lit les listes de tâches et les tâches du compte utilisé pour vous connecter, par l’API Google Tasks, pour les montrer dans les tâches et le plan de Sioul. Écrit les tâches et les listes de tâches que vous créez, modifiez, terminez, déplacez ou supprimez dans Sioul. |

Si vous décochez les tâches sur la page de Google, Sioul laisse les tâches de côté, et le reste fonctionne.

Se connecter avec Google ne donne à Sioul aucun accès à votre Gmail. Si vous lisez Gmail dans Sioul, il est ajouté comme un compte de courrier ordinaire, avec un mot de passe d’application que vous créez vous-même, comme avec n’importe quel logiciel de courrier.

### Comment les données Google sont utilisées {#how-google-data-is-used}

Seulement pour montrer vos agendas, vos contacts et vos tâches dans Sioul, sur votre appareil, et pour réécrire dans votre compte Google les changements que vous faites dans Sioul, au moment où vous les faites. Dans Sioul, ces données servent aux fonctions que vous voyez : l’agenda, les contacts, les tâches et le plan de vos journées, les rappels avant les événements et les dates, et les liens que vous faites entre vos affaires.

Les données Google ne servent pas à la publicité, ne sont pas vendues, ne servent pas à constituer des profils, et ne servent pas à développer, améliorer ou entraîner des modèles d’intelligence artificielle ou d’apprentissage automatique. Aucune personne travaillant sur Sioul ne peut les lire : elles ne lui parviennent jamais.

### Où les données Google sont gardées {#where-google-data-is-kept}

- **Sur votre appareil** : un fichier par événement, tâche et contact, dans le dossier de données de Sioul (sur Linux, `~/.local/share/sioul/calendars/` et `~/.local/share/sioul/contacts/`, dans un dossier au nom du compte).
- **Dans votre propre compte Google**, où elles sont déjà.
- **La connexion** : le jeton d’actualisation que donne Google, avec l’identifiant du client, en une seule entrée du trousseau de votre système. Le jeton d’accès reste en mémoire seulement, une heure au plus.

Rien n’est gardé sur un serveur du développeur : il n’y en a aucun.

### Partage des données Google {#sharing-of-google-data}

Sioul n’envoie les données reçues des API Google ni à son développeur, ni à aucun tiers. Ces données ne quittent votre appareil que dans ces cas, chacun choisi par vous :

1. **En retour vers Google**, quand vous changez quelque chose dans Sioul.
2. **Quand vous envoyez quelque chose vous-même** : un courriel ne part que lorsque vous appuyez sur Envoyer. Quand vous écrivez aux invités d’un événement, leurs adresses viennent de l’événement.
3. **Au géocodeur d’OpenStreetMap**, seulement une fois que vous l’avez permis (**Les placer**, sur la carte de la page Contacts, ou **Placer les contacts sur la carte** dans ses réglages) : les adresses postales de vos contacts, contacts Google compris, sont envoyées une fois chacune au service Nominatim d’OpenStreetMap, pour trouver où elles sont sur la carte. Rien d’autre n’est envoyé avec elles.
4. **À vos autres appareils**, seulement si vous activez [le partage entre vos appareils](guide/sharing.md) : ce que Sioul garde sur cet appareil à propos de vos affaires (par exemple, quelle note est liée à quelle tâche, ou le temps noté sur une tâche) passe par un dossier de votre propre service de synchronisation, chiffré sur votre appareil avec votre phrase de passe ; ce service ne peut pas le lire. Vos événements, contacts et tâches Google eux-mêmes ne passent que par Google.
5. **À un agent d’IA que vous connectez vous-même**, seulement si vous le faites (voir la page [Avec un agent d’IA](guide/ai-agent.md)) : un agent comme Claude Code ou Claude Desktop, connecté à Sioul sur votre appareil, peut lire ce que Sioul garde, agendas, contacts et tâches Google compris, et l’entreprise derrière son modèle reçoit ce qu’il lit, selon votre propre accord avec cette entreprise. Sioul ne connecte jamais un agent de lui-même.

> Sioul's use and transfer of information received from Google APIs adheres to the [Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy), including the Limited Use requirements.
> {: lang="en" }

En français : l’utilisation et le transfert, par Sioul, des informations reçues des API Google respectent les [règles de Google sur les données des utilisateurs de ses services d’API (en anglais)](https://developers.google.com/terms/api-services-user-data-policy), y compris les exigences d’utilisation limitée.

### Retirer l’accès {#taking-the-access-back}

- **Dans Sioul** : Comptes ▸ Vos comptes, sur la fiche du compte Google, **Retirer**. Sioul rend l’accès à Google (il révoque le jeton) et vide son entrée dans le trousseau.
- **Chez Google** : [myaccount.google.com/permissions](https://myaccount.google.com/permissions), où vous pouvez retirer l’accès de Sioul à tout moment.

Retirer le compte laisse votre compte Google lui-même tel qu’il est. Les copies déjà sur votre appareil restent dans le dossier de données de Sioul jusqu’à ce que vous les supprimiez : sur Linux, les dossiers au nom du compte dans `~/.local/share/sioul/calendars/` et `~/.local/share/sioul/contacts/`.

Si vous utilisez une clé Google à vous (votre propre projet Google Cloud, comme Sioul le permet), tout ce qui précède vaut de même : les données ne circulent qu’entre votre appareil et Google.

## Autres données {#other-data}

Sioul lit et garde votre courrier, vos agendas, contacts, tâches, notes, budgets, papiers et les autres choses que vous lui confiez, sur votre appareil, comme décrit plus haut. En dehors de vos propres fournisseurs, il ne contacte d’autres services que pour une fonction que vous choisissez :

- **La météo**, si vous choisissez un lieu : Open-Meteo reçoit les coordonnées de ce lieu, arrondies à deux décimales, au plus une fois par demi-heure, ou le nom d’une ville que vous cherchez.
- **La carte de vos contacts**, si vous le permettez : le géocodeur d’OpenStreetMap reçoit leurs adresses postales, une fois chacune ; les images de la carte viennent d’OpenStreetMap, ou de la source que vous avez choisie.
- **Trouver la clé de chiffrement de quelqu’un**, quand vous le demandez : le Web Key Directory de son domaine, puis keys.openpgp.org, reçoivent l’adresse à laquelle vous écrivez.
- **Le bouclier par IA**, seulement pour une adresse que vous protégez contre le harcèlement et seulement si vous l’activez : chaque nouveau message à cette adresse est envoyé une fois à Anthropic, avec votre propre clé, pour en dire le ton et le sujet. La réponse reste sur votre appareil. Il ne lit que le courrier, jamais les données Google.
- **GitHub**, si vous l’activez : vos tickets et vos demandes de fusion (pull requests) sont lus avec votre propre jeton. Rien n’est écrit sur GitHub.
- **Bitwarden**, quand vous remplissez un identifiant depuis votre coffre : votre serveur Bitwarden, avec votre compte.
- **Les sites que vous épinglez**, comme dans tout navigateur : chacun reçoit aussi une demande pour sa propre icône.
- **Les signatures antivirus**, une fois par jour, seulement quand votre système n’en tient aucune à jour lui-même.

Sur un téléphone, si vous donnez à Sioul l’accès aux notifications d’Android, il lit les notifications des applications que vous le laissez voir, sur le téléphone, pour les retenir jusqu’à leur heure ; il ne garde aucun de leurs mots et n’en envoie rien nulle part, vos autres appareils compris ([Paramètres](guide/settings.md#other-apps)).

Sur un téléphone, si vous faites de Sioul l’appli numéro de l’appelant et spam d’Android, Android lui montre le numéro de chaque appel avant que le téléphone sonne ; Sioul décide là, sur le téléphone, y garde la liste des appels qu’il a refusés, et n’envoie aucun numéro nulle part : aucun serveur, aucune recherche, vos autres appareils compris. Il ne décroche jamais, n’enregistre jamais un appel et n’écoute jamais ([Les appels](guide/calls.md)).

La liste complète, avec le moment où chaque chose se produit : [Vie privée et sécurité](guide/privacy-security.md#what-leaves-your-computer-and-when).

Sioul n’a ni mesure d’audience, ni publicité, ni pistage, ni rapports de plantage.

## Sécurité {#security}

Les connexions à Google passent par HTTPS. La connexion au compte suit les recommandations de Google pour les applications installées : la page de Google s’ouvre dans votre navigateur, et Sioul reçoit la réponse sur votre propre appareil seulement (une adresse de bouclage), vérifiée avec PKCE et une valeur d’état. Les mots de passe et les jetons sont gardés dans le trousseau de votre système. Ce que vous partagez entre vos appareils est chiffré sur votre appareil avant de le quitter.

## Ce site {#this-website}

Ce site n’a ni mesure d’audience, ni publicité, ni cookies, et ne charge rien depuis d’autres sites. Il est hébergé par GitHub Pages, qui garde ses propres journaux de serveur selon la [déclaration de confidentialité de GitHub (en anglais)](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

## Modifications de cette politique {#changes-to-this-policy}

Toute modification est publiée sur cette page, avec une nouvelle date. L’historique de cette page est public, dans le dépôt de Sioul.

## Contact {#contact}

Pour toute question sur cette politique, ou sur vos données : [tickets GitHub](https://github.com/aurelienpierre/sioul/issues).

Le développeur ne détient aucune de vos données : il n’y a donc rien de vous à lui demander, à faire corriger ou à faire supprimer. Tout ce que Sioul garde est sur votre appareil et dans vos propres comptes, où vous pouvez le lire, le modifier ou le supprimer.
