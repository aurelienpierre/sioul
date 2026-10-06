---
description: La page Comptes de Sioul – vos adresses et leurs services, ajouter un compte, qui peut vous écrire et quand, et vos clés de chiffrement.
---

# Les comptes {#accounts}

**Comptes** est l’icône de personne en bas de la colonne de gauche. La page a quatre onglets : **Vos comptes**, **Ajouter un compte**, **Expéditeurs**, **Chiffrement**.

Les sites (messageries sécurisées, discussions) ne sont pas des comptes : ils se créent et se modifient sur la page [Sites](sites.md).

## Vos comptes {#your-accounts}

<figure markdown="span">
  [![L’onglet « Vos comptes » : en haut, « Jusqu’où remontent le courrier et l’agenda », réglé sur 2 semaines ; en dessous, la fiche d’une adresse : son service de courrier avec son serveur, un interrupteur et Retirer, puis à quoi sert cette adresse (Travail coché, Vos démarches, Loisirs), jusqu’où remonter, à quel rythme elle est relevée, la protection contre le harcèlement, sa priorité, « Nom et signature… », et « Serveur et dossiers », replié.](../assets/screens/fr/accounts.png){ loading=lazy }](../assets/screens/fr/accounts.png "Ouvrir l’image en grand")
  <figcaption>Une fiche par adresse, chacun de ses services avec un interrupteur.</figcaption>
</figure>

En haut, **Jusqu’où remontent le courrier et l’agenda** : d’une semaine à un an, ou tout. Le courrier plus ancien arrive au prochain relevé ; « Tout » peut prendre du temps, et de la place sur le disque.

Puis une fiche par adresse, avec chacun de ses services : **Courrier** ; **Agendas, tâches et contacts** ; **Google : agendas, contacts et tâches**. Chacun a :

- **un interrupteur** : éteint, le service garde ses réglages et n’est ni synchronisé ni montré ;
- son serveur, et ce qu’a dit sa dernière relève, en une phrase ;
- **Retirer**.

**Ce que ce serveur offre**, en haut de la fiche, demande au serveur ce qu’il a d’autre : des agendas et des contacts à côté du courrier, et pour un Nextcloud, sa version et ses applications. Ce dont Sioul peut se servir s’ajoute en un clic.

### Une adresse de courrier {#a-mail-address}

Sur sa fiche, **Réglages de cette adresse**, replié tant que vous ne l’ouvrez pas :

- **À quoi sert cette adresse** : travail, vos démarches, loisirs, plusieurs à la fois. Son courrier vient aux heures de ce à quoi elle sert. Sans rien de coché, elle compte comme du travail, pour que son courrier n’atteigne jamais vos soirées. Voir [Les heures](hours.md).
- **Jusqu’où remonter** : combien de semaines du courrier de cette adresse ses dossiers montrent, ou comme les autres adresses.
- **Relever toutes les** : à quel rythme ses dossiers autres que la boîte de réception sont relevés ; 0 suit les autres adresses. Une adresse publique peut être relevée deux fois par jour (720 minutes).
- **Protégée contre le harcèlement**, et une fois cette protection activée, **Laisser l’IA le lire d’abord**. Voir [le Porche](porch.md#a-public-address-protected).

Toujours en vue en dessous :

- **Priorité** : *Plus important*, *Normal* ou *Moins important*. Voir [le Porche](porch.md#some-addresses-first-others-last).
- **Nom et signature…** : votre nom, tel que les destinataires le voient, et votre signature, en Markdown.

Puis **Serveur et dossiers**, replié : le serveur, et l’endroit où son courrier est gardé sur cet appareil.

Sous les fiches, **Le bouclier IA** garde la clé du service d’Anthropic, utilisée seulement par les adresses qui laissent l’IA les lire d’abord. Elle est gardée dans le trousseau de votre système, jamais dans un fichier ; **Oublier la clé** la retire.

### Retirer un compte {#removing-one}

**Retirer** demande d’abord. Le mot de passe d’un compte de courrier quitte le trousseau ; son courrier reste sur votre disque et sur le serveur. Pour Google, l’accès est rendu à Google.

### Un compte venu de votre autre appareil {#an-account-from-your-other-device}

[Le partage entre vos appareils](sharing.md) apporte vos comptes, jamais leurs mots de passe. Un tel compte dit qu’il n’a pas encore de mot de passe ici, avec **Mot de passe…** sur sa fiche : tapez-le, ou choisissez **Depuis Bitwarden…**. Votre coffre s’ouvre (son mot de passe principal, puis sa deuxième étape : le code d’une application, d’un courriel, d’une YubiKey), et les identifiants qu’il garde pour cette adresse sont listés ; choisissez-en un. Le mot de passe est essayé auprès du serveur, puis gardé dans le trousseau de cet appareil. **Mot de passe…** revient si le serveur refuse un jour celui qui est gardé.

Sur un téléphone, une clé de sécurité ne peut pas encore ouvrir le coffre : prenez une autre deuxième étape de votre compte Bitwarden.

## Ajouter un compte {#add-an-account}

Trois formulaires, l’un après l’autre :

- **Ajouter un compte de courrier** : votre adresse, **Trouver le serveur**, votre mot de passe, **Se connecter et ajouter**.
- **Ajouter des contacts et agendas** : depuis un serveur CalDAV et CardDAV, comme Nextcloud, Fastmail, iCloud ou votre hébergeur.
- **Agendas, contacts et tâches Google** : **Se connecter avec Google**, sur la page de Google elle-même.

Sur un téléphone, **Depuis les comptes du téléphone…**, au-dessus d’eux, ouvre la liste d’Android des comptes que le téléphone connaît. Une adresse Google remplit le formulaire de Google ; toute autre remplit le formulaire de courrier, dont Sioul cherche alors le serveur, et celui des contacts et agendas. Android ne prête aucun mot de passe : Sioul le demande une fois.

Pas à pas : [Premiers pas](first-steps.md#add-your-mail). Les mots de passe vont dans le trousseau de votre système, nulle part ailleurs.

## Expéditeurs {#senders}

<figure markdown="span">
  [![L’onglet Expéditeurs : une grille de cases, les listes Sûrs, Neutres et Restreints en lignes et les moments Travail, Démarches, Loisirs, Repas et Sommeil en colonnes ; puis quatre listes, « Sûrs : à tout moment », « Neutres : travail, démarches », « Restreints : travail » et « Bloqués : jamais », chacune avec des adresses et des motifs comme *@example.org, et un champ pour en ajouter un ; puis les catégories de vos contacts, chacune avec une liste ou aucune.](../assets/screens/fr/accounts-senders.png){ loading=lazy }](../assets/screens/fr/accounts-senders.png "Ouvrir l’image en grand")
  <figcaption>Qui peut vous écrire, et quand.</figcaption>
</figure>

Qui peut vous écrire, et quand, quelle que soit l’adresse où la personne écrit.

**Quand vient le courrier de chaque liste** : une grille de cases, les listes en lignes (Sûrs, Neutres, Restreints), les cinq moments en colonnes (Travail, Démarches, Loisirs, Repas, Sommeil : voir [Les heures](hours.md)). Cochez-en autant que vous voulez sur chaque ligne. Coché, leur courrier vient alors ; décoché, il attend, jamais perdu, le prochain moment coché. Sur un écran étroit, la grille défile de côté. Au départ :

| | Travail | Démarches | Loisirs | Repas | Sommeil |
|---|---|---|---|---|---|
| Sûrs | ✓ | ✓ | ✓ | ✓ | ✓ |
| Neutres | ✓ | ✓ | | | |
| Restreints | ✓ | | | | |

Les codes et les liens que vous venez de demander à un site, et ce que vous vous envoyez, arrivent tout de suite, quoi que dise la grille. Pendant le sommeil, rien ne notifie : le courrier coché pour le sommeil se montre au Porche si vous ouvrez Sioul, sans son ni notification.

**Quatre listes**, chacune disant ses moments :

- **Sûrs** : les amis, les collègues et la famille que vous choisissez. Leur courrier ne passe pas par le filtre d’accueil, et vient à n’importe laquelle de vos adresses. C’est toujours vous qui y mettez quelqu’un.
- **Neutres** : toute personne qu’aucune liste ne nomme, y compris les personnes inconnues. Nommez quelqu’un ici pour le garder neutre dans un domaine ou une catégorie mis sur une autre liste.
- **Restreints** : ceux dont vous préférez n’avoir des nouvelles qu’à des moments choisis : un client exigeant, quelqu’un dont le courrier pèse.
- **Bloqués** : les indésirables et le harcèlement, écartés pour de bon, jamais montrés, jamais comptés, jamais notifiés. Rien n’est supprimé.

Chaque ligne est une adresse, ou un motif avec `*` : `*@example.org` pour tout le monde là-bas, `*@*.example.org` pour ses sous-domaines. Partager un serveur ou un domaine avec quelqu’un d’autre ne fait jamais bloquer personne.

Le courrier d’un expéditeur neutre ou restreint ne vient qu’à une adresse faite pour le moment présent (l’adresse du travail aux heures de travail, une adresse personnelle aux heures de démarches et aux loisirs) ; quand les deux ne se rencontrent jamais, les moments de sa liste décident seuls, pour que rien n’attende pour toujours.

**Les catégories de vos contacts** : une ligne pour chaque catégorie qu’ont vos contacts (Amis, Famille, Clients…), avec un choix : **Aucune liste**, **Sûrs**, **Neutres**, **Restreints** ou **Bloqués**. Toute personne dont la fiche est dans la catégorie la suit, à chacune de ses adresses. Rien ne va sur une liste tout seul, famille et amis compris.

**De la personne au groupe** : le choix propre à une personne passe d’abord, puis les catégories de sa fiche, puis le domaine de son adresse ; toutes les autres sont neutres. Un ami peut donc être neutre alors que la catégorie Amis est sûre, et un collègue dans Amis reste sûr dans un domaine marqué restreint. Quand un même niveau donne deux réponses (une personne dans deux catégories mises sur deux listes), bloqué l’emporte, puis restreint, puis neutre, puis sûr.

Le courrier falsifié est jugé à part, avant les listes : un message falsifié est écarté même quand il se réclame de l’adresse d’un expéditeur sûr, et pesé comme celui d’un inconnu.

Depuis un message ou un contact, **Son courrier** met quelqu’un dans l’une des quatre listes, ou le rend à **Comme le disent ses catégories**. Les listes elles-mêmes, avec leurs motifs, et les catégories ne se modifient qu’ici.

## Chiffrement {#encryption}

Vos clés OpenPGP, pour signer et chiffrer vos messages ([Le courrier](mail.md#signing-and-encrypting)) :

- **Créer une clé pour** une adresse : une nouvelle clé, valable trois ans ; sa phrase secrète est faite au hasard et gardée dans votre trousseau, si bien que rien n’est demandé à chaque message.
- **Importer une clé…** : une clé exportée de GnuPG, avec sa phrase secrète, demandée une fois.
- **Enregistrer la clé publique** : dans vos téléchargements, pour la donner aux autres.
- **Clés des autres** : celles venues avec leurs messages, ou d’un fichier, ou trouvées par **Chercher leurs clés** dans la fenêtre d’écriture.

Vos propres clés restent sur cet appareil. Elles ne sont pas partagées avec vos autres appareils : copiez-les à la main si vous en avez besoin là-bas.
