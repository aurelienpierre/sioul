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

Tout est en vue sur sa fiche :

- **À quoi sert cette adresse** : travail, vos démarches, loisirs, plusieurs à la fois. Son courrier vient aux heures de ce à quoi elle sert. Sans rien de coché, elle compte comme du travail, pour que son courrier n’atteigne jamais vos soirées. Voir [Les heures](hours.md).
- **Jusqu’où remonter** : combien de semaines du courrier de cette adresse ses dossiers montrent, ou comme les autres adresses.
- **Relever toutes les** : à quel rythme ses dossiers autres que la boîte de réception sont relevés ; 0 suit les autres adresses. Une adresse publique peut être relevée deux fois par jour (720 minutes).
- **Protégée contre le harcèlement**, et une fois cette protection activée, **Laisser l’IA le lire d’abord**. Voir [le Porche](porch.md#a-public-address-protected).
- **Priorité** : *Plus important*, *Normal* ou *Moins important*. Voir [le Porche](porch.md#some-addresses-first-others-last).
- **Nom et signature…** : votre nom, tel que les destinataires le voient, et votre signature, en Markdown.
- **Serveur et dossiers**, replié : le serveur, et l’endroit où son courrier est gardé sur cet ordinateur.

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

Pas à pas : [Premiers pas](first-steps.md#add-your-mail). Les mots de passe vont dans le trousseau de votre système, nulle part ailleurs.

## Expéditeurs {#senders}

<figure markdown="span">
  [![L’onglet Expéditeurs : trois listes, « Sûr : à toute heure », « Neutre : aux heures de travail » et « Bloqué : jamais », chacune avec des adresses et des motifs comme *@example.org, et un champ pour en ajouter un.](../assets/screens/fr/accounts-senders.png){ loading=lazy }](../assets/screens/fr/accounts-senders.png "Ouvrir l’image en grand")
  <figcaption>Qui peut vous écrire, et quand.</figcaption>
</figure>

Qui peut vous écrire, et quand, quelle que soit l’adresse où la personne écrit. Trois listes :

- **Sûr : à toute heure** : les amis, les collègues et la famille que vous choisissez. Leur courrier vous parvient aussi dans le calme, et ne passe pas par le filtre d’accueil. C’est toujours vous qui y mettez quelqu’un.
- **Neutre : aux heures de travail** : toute personne nommée nulle part ailleurs, y compris les personnes inconnues. Nommez quelqu’un ici pour le garder neutre dans un domaine marqué sûr.
- **Bloqué : jamais** : les indésirables et le harcèlement, écartés pour de bon, jamais montrés, jamais comptés. Rien n’est supprimé.

Chaque ligne est une adresse, ou un motif avec `*` : `*@example.org` pour tout le monde là-bas, `*@*.example.org` pour ses sous-domaines. L’entrée la plus précise l’emporte : une adresse marquée sûre reste sûre dans un domaine bloqué ici. Partager un serveur ou un domaine avec quelqu’un d’autre ne fait jamais bloquer personne.

Le courrier falsifié est jugé à part, avant les listes : un message falsifié est écarté même quand il se réclame de l’adresse d’un expéditeur sûr.

Depuis un message ou un contact, **Son courrier** met quelqu’un dans l’une des trois listes. Les listes elles-mêmes, avec leurs motifs, ne se modifient qu’ici.

## Chiffrement {#encryption}

Vos clés OpenPGP, pour signer et chiffrer vos messages ([Le courrier](mail.md#signing-and-encrypting)) :

- **Créer une clé pour** une adresse : une nouvelle clé, valable trois ans ; sa phrase secrète est faite au hasard et gardée dans votre trousseau, si bien que rien n’est demandé à chaque message.
- **Importer une clé…** : une clé exportée de GnuPG, avec sa phrase secrète, demandée une fois.
- **Enregistrer la clé publique** : dans vos téléchargements, pour la donner aux autres.
- **Clés des autres** : celles venues avec leurs messages, ou d’un fichier, ou trouvées par **Chercher leurs clés** dans la fenêtre d’écriture.

Vos propres clés restent sur cet ordinateur. Elles ne sont pas partagées avec vos autres ordinateurs : copiez-les à la main si vous en avez besoin là-bas.
