---
description: La page Comptes de Sioul – vos adresses et leurs services, ajouter un compte, qui peut vous joindre et quand (courrier, appels, messages des autres applications), et vos clés de chiffrement.
---

# Les comptes {#accounts}

**Comptes** est l’icône de personne en bas de la colonne de gauche. La page a quatre onglets : **Vos comptes**, **Ajouter un compte**, **Qui peut vous joindre**, **Chiffrement**.

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

**Retirer** demande d’abord. Le mot de passe d’un compte de courrier quitte le trousseau ; son courrier reste sur votre disque et sur le serveur. Pour Google, l’accès est rendu à Google : celui du courrier et celui des agendas à part, pour que retirer l’un laisse son accès à l’autre.

### Un compte venu de votre autre appareil {#an-account-from-your-other-device}

[Le partage entre vos appareils](sharing.md) apporte vos comptes, jamais leurs mots de passe. Un tel compte dit qu’il n’a pas encore de mot de passe ici, avec **Mot de passe…** sur sa fiche : tapez-le, ou choisissez **Depuis Bitwarden…**. Votre coffre s’ouvre (son mot de passe principal, puis sa deuxième étape : le code d’une application, d’un courriel, d’une YubiKey), et les identifiants dont le nom d’utilisateur est cette adresse sont listés, ceux du serveur en premier ; choisissez-en un, ou tapez un site pour les restreindre ([Sites](sites.md#logins-from-bitwarden)). Le mot de passe est essayé auprès du serveur, puis gardé dans le trousseau de cet appareil. **Mot de passe…** revient si le serveur refuse un jour celui qui est gardé.

Sur un téléphone, une clé de sécurité ne peut pas encore ouvrir le coffre : prenez une autre deuxième étape de votre compte Bitwarden.

### Le courrier de Google {#googles-mail}

Le courrier Gmail ou Google Workspace connecté avec Google dit quand Google met fin à l’accès (votre mot de passe Google changé, votre clé Google supprimée, un projet Google laissé en test). Sur sa fiche, **Se reconnecter** ouvre le formulaire du courrier sur la page de Google, avec votre clé ; **Mot de passe d’application…** lui donne un mot de passe d’application à la place. Sioul ne demande jamais le mot de passe de votre compte Google. Voir [Premiers pas](first-steps.md#gmail-and-google-workspace).

## Ajouter un compte {#add-an-account}

Trois formulaires, l’un après l’autre :

- **Ajouter un compte de courrier** : votre adresse, **Trouver le serveur**, votre mot de passe, **Se connecter et ajouter**. Pour Gmail et Google Workspace, **Utiliser un mot de passe d’application** ou **Se connecter avec Google** au lieu d’un mot de passe.
- **Ajouter des contacts et agendas** : depuis un serveur CalDAV et CardDAV, comme Nextcloud, Fastmail, iCloud ou votre hébergeur.
- **Agendas, contacts et tâches Google** : **Se connecter avec Google**, sur la page de Google elle-même.

Sur un téléphone, **Depuis les comptes du téléphone…**, au-dessus d’eux, ouvre la liste d’Android des comptes que le téléphone connaît. Une adresse Google remplit le formulaire de Google et celui du courrier, avec les deux façons de Google ; toute autre remplit le formulaire de courrier, dont Sioul cherche alors le serveur, et celui des contacts et agendas. Android ne prête aucun mot de passe : Sioul le demande une fois.

Pas à pas : [Premiers pas](first-steps.md#add-your-mail). Les mots de passe vont dans le trousseau de votre système, nulle part ailleurs.

## Qui peut vous joindre {#senders}

<figure markdown="span">
  [![L’onglet Qui peut vous joindre : trois boutons, Courrier, Appels et Messages, au-dessus d’une grille de cases, les états en lignes (Sûrs, Neutres, Restreints, Inconnus, Numéros masqués pour les appels, et Bloqués, jamais cochés) et les moments en colonnes (Travail, Démarches, Loisirs, Repas, Sommeil, En pause), chaque ligne avec Toujours et Jamais ; puis quatre listes avec des adresses, des numéros et des motifs comme *@example.org ; puis les personnes et les catégories mises sur une liste.](../assets/screens/fr/accounts-senders.png){ loading=lazy }](../assets/screens/fr/accounts-senders.png "Ouvrir l’image en grand")
  <figcaption>Qui peut vous joindre, et quand.</figcaption>
</figure>

Qui peut vous joindre, et quand : par courrier, par téléphone, et par les messages des autres applications, quels que soient l’adresse ou le numéro qu’il utilise.

**Cinq états.** Chacun est dans l’un d’eux :

- **Inconnus** : dans aucun de vos carnets d’adresses, ni sur aucune liste.
- **Sûrs** : les amis, les collègues et la famille que vous choisissez. C’est toujours vous qui y mettez quelqu’un.
- **Neutres** : toute personne de vos carnets d’adresses, tant que vous n’en décidez pas autrement.
- **Restreints** : ceux dont vous préférez n’avoir des nouvelles qu’à des moments choisis : un client exigeant, quelqu’un dont le courrier pèse.
- **Bloqués** : les indésirables et le harcèlement. Jamais, sur aucun canal : leur courrier mis de côté pour de bon, jamais montré, jamais compté, jamais notifié ; leurs appels refusés. Rien n’est supprimé.

Les inconnus comptaient comme neutres. Ils ont maintenant leur propre ligne : leur courrier garde les moments des neutres tant que vous ne la changez pas, et leurs appels sont refusés à tout moment tant que vous ne cochez rien.

**Quand chacun vient** : trois boutons choisissent le canal, **Courrier**, **Appels** ou **Messages** (les SMS et les discussions des autres applications, sur un téléphone) ; la grille au-dessous est celle de ce canal, les états en lignes, les moments en colonnes (Travail, Démarches, Loisirs, Repas, Sommeil : voir [Les heures](hours.md) ; **En pause** : la pause, voir [Les pauses](pauses.md)). Cochez-en autant que vous voulez sur chaque ligne ; **Toujours** et **Jamais**, au bout de la ligne, la cochent ou la décochent tout entière d’un clic. Coché, ils viennent alors. Décoché, le courrier et les messages attendent, jamais perdus, le prochain moment coché, et les appels sont refusés. Les bloqués ont leur ligne aussi, jamais cochée. Sur un téléphone, le nom de chaque ligne se place au-dessus de ses cases. Au départ :

| Courrier | Travail | Démarches | Loisirs | Repas | Sommeil | En pause |
|---|---|---|---|---|---|---|
| Sûrs | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Neutres | ✓ | ✓ | | | | |
| Restreints | ✓ | | | | | |
| Inconnus | ✓ | ✓ | | | | |

| Appels | Travail | Démarches | Loisirs | Repas | Sommeil | En pause |
|---|---|---|---|---|---|---|
| Sûrs | ✓ | ✓ | ✓ | ✓ | | |
| Neutres | ✓ | ✓ | | | | |
| Restreints | ✓ | | | | | |
| Inconnus | | | | | | |
| Numéros masqués | ✓ | ✓ | | | | |

Les messages suivent la grille du courrier, ligne par ligne, tant que vous ne changez pas l’une de leurs lignes. Les numéros masqués sont les appels qui ne montrent aucun numéro : quelqu’un qui cache le sien, et souvent le standard d’un hôpital, d’où leurs sonneries aux heures de travail et de démarches.

Les codes et les liens que vous venez de demander à un site, et ce que vous vous envoyez, arrivent tout de suite, quoi que dise la grille. Pendant le sommeil, rien ne notifie : le courrier coché pour le sommeil se montre au Porche si vous ouvrez Sioul, sans son ni notification. En temps libre, seule la ligne des sûrs compte, à sa case Loisirs ([Les pauses](pauses.md)).

**Quatre listes** : Sûrs, Neutres, Restreints et Bloqués. Chaque ligne est une adresse, un numéro de téléphone (`+33 1 99 00 12 34`, écrit comme vous voulez), ou un motif avec `*` : `*@example.org` pour tout le monde là-bas, `*@*.example.org` pour ses sous-domaines, `+33899*` pour tous les numéros qui commencent ainsi. Partager un serveur ou un domaine avec quelqu’un d’autre ne fait jamais bloquer personne.

Le courrier d’un expéditeur neutre ou restreint, ou d’un inconnu, ne vient qu’à une adresse faite pour le moment présent (l’adresse du travail aux heures de travail, une adresse personnelle aux heures de démarches et aux loisirs) ; quand les deux ne se rencontrent jamais, sa ligne décide seule, pour que rien n’attende pour toujours.

**Les personnes sur une liste** : chacune de celles que vous avez mises sur une liste depuis leur fiche dans [Contacts](contacts.md), avec un choix : **Comme le disent ses catégories**, **Sûr**, **Neutre**, **Restreint** ou **Bloqué**. La fiche décide pour toutes ses adresses et tous ses numéros, ceux ajoutés plus tard aussi.

**Les catégories de vos contacts** : une ligne pour chaque catégorie qu’ont vos contacts (Amis, Famille, Clients…), avec un choix : **Aucune liste**, **Sûr**, **Neutre**, **Restreint** ou **Bloqué**. Toute personne dont la fiche est dans la catégorie la suit, à chacune de ses adresses et chacun de ses numéros. Rien ne va sur une liste tout seul, famille et amis compris.

**De la personne au groupe** : le choix propre à une personne passe d’abord (son adresse ou son numéro, puis sa fiche), puis les catégories de sa fiche, puis un domaine ou le début d’un numéro ; puis, neutres : toute personne de vos carnets d’adresses, et toute personne que vous avez laissée entrer depuis le filtre d’accueil ; toutes les autres sont inconnues. Un ami peut donc être neutre alors que la catégorie Amis est sûre, et un collègue dans Amis reste sûr dans un domaine marqué restreint. Quand un même niveau donne deux réponses (une personne dans deux catégories mises sur deux listes), bloqué l’emporte, puis restreint, puis neutre, puis sûr.

Le courrier falsifié est jugé à part, avant les listes : un message falsifié est mis de côté même quand il se réclame de l’adresse d’un expéditeur sûr, et pesé comme celui d’un inconnu.

Depuis un message, **Son courrier** met l’adresse de l’expéditeur sur l’une des quatre listes, ou la rend à **Comme le disent ses catégories**. Depuis la fiche d’un contact, **Sa liste** le fait pour la personne, toutes ses adresses et tous ses numéros.

## Chiffrement {#encryption}

Vos clés OpenPGP, pour signer et chiffrer vos messages ([Le courrier](mail.md#signing-and-encrypting)) :

- **Créer une clé pour** une adresse : une nouvelle clé, valable trois ans ; sa phrase secrète est faite au hasard et gardée dans votre trousseau, si bien que rien n’est demandé à chaque message.
- **Importer une clé…** : une clé exportée de GnuPG, avec sa phrase secrète, demandée une fois.
- **Enregistrer la clé publique** : dans vos téléchargements, pour la donner aux autres.
- **Clés des autres** : celles venues avec leurs messages, ou d’un fichier, ou trouvées par **Chercher leurs clés** dans la fenêtre d’écriture.

Vos propres clés restent sur cet appareil. Elles ne sont pas partagées avec vos autres appareils : copiez-les à la main si vous en avez besoin là-bas.
