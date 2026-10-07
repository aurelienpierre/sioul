---
description: La page Comptes de Sioul – vos adresses et leurs services, ajouter un compte, et vos clés de chiffrement ; qui peut vous joindre et quand est dans les Paramètres, Ce qui vous joint.
---

# Les comptes {#accounts}

**Comptes** est l’icône de personne en bas de la colonne de gauche. La page a trois onglets : **Vos comptes**, **Ajouter un compte**, **Chiffrement**. Qui peut vous joindre, et quand, est dans Paramètres ▸ [Ce qui vous joint](notifications.md).

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

Qui peut vous joindre, et quand, par courrier, par téléphone et par les messages des autres applications, et qui est sur quelle liste (sûrs, neutres, restreints, bloqués, avec des adresses, des numéros, des motifs comme `*@example.org`, les personnes mises sur une liste et les catégories de vos contacts) : Paramètres ▸ **Ce qui vous joint** ▸ **Par personne** ([Ce qui vous joint, et quand](notifications.md#by-person)). Pour une personne, sa fiche, depuis sa fiche de contact ou un message : **Comment … vous joint** ([La fiche d’une personne](notifications.md#a-persons-sheet)). Ce à quoi sert chaque adresse reste sur sa carte, plus haut : le courrier d’une adresse pour un autre moment attend ce moment.

## Chiffrement {#encryption}

Vos clés OpenPGP, pour signer et chiffrer vos messages ([Le courrier](mail.md#signing-and-encrypting)) :

- **Créer une clé pour** une adresse : une nouvelle clé, valable trois ans ; sa phrase secrète est faite au hasard et gardée dans votre trousseau, si bien que rien n’est demandé à chaque message.
- **Importer une clé…** : une clé exportée de GnuPG, avec sa phrase secrète, demandée une fois.
- **Enregistrer la clé publique** : dans vos téléchargements, pour la donner aux autres.
- **Clés des autres** : celles venues avec leurs messages, ou d’un fichier, ou trouvées par **Chercher leurs clés** dans la fenêtre d’écriture.

Vos propres clés restent sur cet appareil. Elles ne sont pas partagées avec vos autres appareils : copiez-les à la main si vous en avez besoin là-bas.
