---
description: La page Comptes de Sioul – une fiche par adresse, un interrupteur pour chacun de ses services ; Google connecté sur la page de Google ; les mots de passe dans le trousseau de votre système, ou venus de Bitwarden ; vos clés OpenPGP, sur cet appareil ou sur une clé de sécurité ; comparée aux autres logiciels de courrier.
---

# Les comptes {#accounts}

## En bref {#in-short}

**Comptes** est l’icône de personne en bas de la colonne de gauche. La page tient une fiche par adresse, avec un interrupteur pour chacun de ses services : le courrier, les agendas et les contacts, ceux de Google. Les mots de passe vont dans le trousseau de votre système et ne voyagent jamais entre vos appareils ; Google se connecte sur sa propre page ; Bitwarden peut donner son mot de passe à un compte. L’onglet **Chiffrement** tient vos clés OpenPGP, gardées sur cet appareil ou sur une clé de sécurité comme une YubiKey.

Qui peut vous joindre, et quand, est dans Paramètres ▸ [Ce qui vous joint](notifications.md). Les sites (messageries sécurisées, discussions) ne sont pas des comptes : ils se créent et se modifient sur la page [Sites](sites.md).

## Protégé par défaut {#what-is-protected}

- Chaque mot de passe va dans le trousseau de votre système, jamais dans un fichier. Les mots de passe ne voyagent jamais entre vos appareils : chaque appareil le demande une fois.
- Sioul ne parle à vos serveurs de courrier, d’agendas et de contacts que par des connexions chiffrées : aucun réglage ne permet de s’en passer.
- Google : vous vous connectez sur la page de Google elle-même, dans votre navigateur habituel, avec ce que Google y demande (votre mot de passe, votre clé de sécurité, une clé d’accès). Sioul ne voit jamais votre mot de passe Google, et retirer le compte rend l’accès à Google.
- Bitwarden peut donner son mot de passe à un compte : Sioul lit votre coffre lui-même et n’y écrit jamais.
- Vos clés OpenPGP restent sur cet appareil, ou sur votre clé de sécurité ; les personnes à qui vous écrivez reçoivent votre clé publique avec vos messages, pour pouvoir vous répondre chiffré.
- Sur une clé de sécurité, vos clés ne la quittent jamais. Son code PIN est gardé en mémoire un quart d’heure sans usage, et jamais écrit nulle part.
- Chercher la clé de quelqu’un dit à un serveur quelle clé vous cherchez : Sioul ne le fait que quand vous le demandez.

## Vos comptes {#your-accounts}

<figure markdown="span">
  [![L’onglet «  Vos comptes  » : en haut, «  Jusqu’où remontent le courrier et l’agenda  », réglé sur 2 semaines ; en dessous, la fiche d’une adresse : son service de courrier avec son serveur, un interrupteur et Retirer, puis à quoi sert cette adresse (Travail coché, Vos démarches, Loisirs), jusqu’où remonter, à quel rythme elle est relevée, la protection contre le harcèlement, sa priorité, «  Nom et signature…  », et «  Serveur et dossiers  », replié.](../assets/screens/fr/accounts.png){ loading=lazy }](../assets/screens/fr/accounts.png "Ouvrir l’image en grand")
  <figcaption>Une fiche par adresse, chacun de ses services avec un interrupteur.</figcaption>
</figure>

En haut, **Jusqu’où remontent le courrier et l’agenda** : d’une semaine à un an, ou tout. Le courrier plus ancien arrive au prochain relevé ; «  Tout  » peut prendre du temps, et de la place sur le disque.

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

[Le partage entre vos appareils](sharing.md) apporte vos comptes, jamais leurs mots de passe. Un tel compte dit qu’il n’a pas encore de mot de passe ici, avec **Mot de passe…** sur sa fiche : tapez-le, ou choisissez **Depuis Bitwarden…**. Les identifiants dont le nom d’utilisateur est cette adresse sont listés, ceux du serveur en premier ; choisissez-en un, ou tapez un site pour les restreindre ([Sites](sites.md#logins-from-bitwarden)). Le mot de passe est essayé auprès du serveur, puis gardé dans le trousseau de cet appareil. **Mot de passe…** revient si le serveur refuse un jour celui qui est gardé.

Depuis cette page, votre coffre Bitwarden s’ouvre avec son mot de passe principal et une deuxième étape autre qu’une clé de sécurité : le code d’une application, celui d’un courriel, celui d’une YubiKey, un code de récupération. Pour l’ouvrir avec votre clé de sécurité, ouvrez-le d’abord depuis la page Sites (**Remplir l’identifiant**, ou ⋮ ▸ **Choisir un identifiant…**) : il reste ensuite ouvert jusqu’à la fermeture de Sioul, ici aussi. Sur un téléphone, une clé de sécurité ne peut pas encore ouvrir le coffre : prenez une autre deuxième étape de votre compte Bitwarden.

### Le courrier de Google {#googles-mail}

Le courrier Gmail ou Google Workspace connecté avec Google dit quand Google met fin à l’accès (votre mot de passe Google changé, votre propre application Google supprimée, un projet Google laissé en test). Sur sa fiche, **Se reconnecter** rouvre la page de Google, dans votre navigateur ; **Mot de passe d’application…** lui donne un mot de passe d’application à la place. Sioul ne demande jamais le mot de passe de votre compte Google. Voir [Premiers pas](first-steps.md#gmail-and-google-workspace).

## Ajouter un compte {#add-an-account}

Trois formulaires, l’un après l’autre :

- **Ajouter un compte de courrier** : votre adresse, **Trouver le serveur**, votre mot de passe, **Se connecter et ajouter**. Pour Gmail et Google Workspace, **Utiliser un mot de passe d’application** ou **Se connecter avec Google** au lieu d’un mot de passe.
- **Ajouter des contacts et agendas** : depuis un serveur CalDAV et CardDAV, comme Nextcloud, Fastmail, iCloud ou votre hébergeur.
- **Agendas, contacts et tâches Google** : **Se connecter avec Google**, sur la page de Google elle-même.

Sur un téléphone, **Depuis les comptes du téléphone…**, au-dessus d’eux, ouvre la liste d’Android des comptes que le téléphone connaît. Une adresse Google remplit le formulaire de Google et celui du courrier, avec les deux façons de Google ; toute autre remplit le formulaire de courrier, dont Sioul cherche alors le serveur, et celui des contacts et agendas. Android ne prête aucun mot de passe : Sioul le demande une fois.

Pas à pas : [Premiers pas](first-steps.md#add-your-mail). Les mots de passe vont dans le trousseau de votre système, nulle part ailleurs, et Sioul ne parle à vos serveurs que par des connexions chiffrées.

Les adresses Outlook.com, Hotmail et Microsoft 365 ne peuvent pas encore être ajoutées : Microsoft n’accepte des autres logiciels de courrier que sa propre page de connexion ([Fonctionne avec](compatibility.md#mail)).

## Qui peut vous joindre {#senders}

Qui peut vous joindre, et quand, par courrier, par téléphone et par les messages des autres applications, et qui est sur quelle liste (sûrs, neutres, restreints, bloqués, avec des adresses, des numéros, des motifs comme `*@example.org`, les personnes mises sur une liste et les catégories de vos contacts) : Paramètres ▸ **Ce qui vous joint** ▸ **Par personne** ([Ce qui vous joint, et quand](notifications.md#by-person)). Pour une personne, sa fiche, depuis sa fiche de contact ou un message : **Comment … vous joint** ([La fiche d’une personne](notifications.md#a-persons-sheet)). Ce à quoi sert chaque adresse reste sur sa carte, plus haut : le courrier d’une adresse pour un autre moment attend ce moment.

## Chiffrement {#encryption}

Vos clés OpenPGP, pour signer et chiffrer vos messages ([Le courrier](mail.md#signing-and-encrypting)) :

- **Créer une clé pour** une adresse : une nouvelle clé, valable trois ans ; sa phrase secrète est faite au hasard et gardée dans votre trousseau, si bien que rien n’est demandé à chaque message.
- **Importer une clé…** : une clé exportée de GnuPG, avec sa phrase secrète, demandée une fois.
- **Enregistrer la clé publique** : dans vos téléchargements, pour la donner aux autres.
- **Clés des autres** : celles venues avec leurs messages, ou d’un fichier, ou trouvées par **Chercher leurs clés** dans la fenêtre d’écriture.

Vos propres clés restent sur cet appareil. Elles ne sont pas partagées avec vos autres appareils : copiez-les à la main si vous en avez besoin là-bas.

### Clé de sécurité {#security-key}

Vos clés OpenPGP peuvent rester sur une clé de sécurité, comme une YubiKey ou une Nitrokey : elle signe et ouvre votre courrier elle-même, après son code PIN, et ses clés privées ne la quittent jamais. Sioul n’en garde que la partie publique. Cela marche sur un ordinateur ; sur un téléphone, pas encore.

1. Branchez la clé, puis choisissez **Utiliser une clé de sécurité**. Sioul la lit sans son code PIN : son nom et son numéro de série, et ce avec quoi elle signe et déchiffre.
2. Sioul a besoin de la partie publique de ces clés, que la clé ne contient pas. **La chercher** interroge l’adresse écrite sur la clé, l’annuaire de clés de votre domaine, puis keys.openpgp.org ; cela dit à ces serveurs que quelqu’un cherche votre clé. Ou **Importer un fichier…** : Sioul montre la commande GnuPG qui l’écrit dans un fichier.
3. Sioul ne la garde que si c’est bien celle de la clé : les mêmes clés, les mêmes parties publiques. Votre clé de sécurité signe alors, seule, pour les adresses qu’elle nomme parmi les vôtres (une clé créée ou importée ici pour la même adresse n’ouvre plus que l’ancien courrier), et ouvre ce qui lui est chiffré.

Quand vous envoyez un message signé ou ouvrez un message chiffré, Sioul demande la clé là où vous êtes : branchez-la, tapez son code PIN, touchez-la quand elle clignote. Si la clé s’en va pendant qu’elle signe, rien n’est envoyé et le message reste dans les Brouillons ([Le courrier](mail.md#signing-and-encrypting)).

Chaque clé montre pour quoi elle signe, si elle demande un toucher, d’où vient sa partie publique et quand elle expire. Sioul dit un mois avant qu’elle va expirer, avec les deux commandes GnuPG qui la renouvellent ; une fois expirée, le courrier ne peut plus être signé avec elle avant qu’elle soit renouvelée, et l’ancien courrier s’ouvre toujours. **Chercher une version plus récente** rapporte la partie publique renouvelée, ou **Importer un fichier…** un nouvel export. **Oublier le code PIN maintenant** oublie le code PIN que Sioul garde. **Ne plus utiliser cette clé de sécurité** la met de côté.

Si un autre programme garde la clé (GnuPG, le plus souvent), Sioul le dit et propose **Laisser GnuPG la libérer** ([Côté technique](#for-technical-readers)).

Sioul ne fait qu’utiliser la clé : il ne demande jamais son code PIN d’administration, ne change ni ne débloque jamais un code PIN, ne charge ni ne crée jamais de clé dessus. Ce qui a été essayé jusqu’ici : [Fonctionne avec](compatibility.md#logins-and-keys).

## Pour aller plus loin {#going-further}

- **Une clé Google à vous** : Google ne donne l’accès au courrier de Gmail qu’aux applications qu’il a examinées. Tant que celle de Sioul ne l’est pas, **Utiliser une clé Google à moi** prend l’identifiant client et le secret d’une application Google que vous créez une fois dans Google Cloud (environ un quart d’heure, les étapes montrées). Une copie de Sioul construite sans clé Google à elle demande la vôtre pour les agendas aussi ; les copies publiées jusqu’ici sont construites sans. Cette «  clé  » est le nom et le secret d’une application, pas une clé de sécurité ([Premiers pas](first-steps.md#gmail-and-google-workspace)).
- **Un mot de passe d’application** pour Gmail marche aussi, sans la connexion de Google.
- **La clé du bouclier IA**, pour les adresses qui laissent l’IA les lire d’abord, est gardée dans le trousseau comme un mot de passe ; **Oublier la clé** la retire ([le Porche](porch.md#a-public-address-protected)).
- **Partager la clé de sécurité avec GnuPG** : ajoutez la ligne `pcsc-shared` à `~/.gnupg/scdaemon.conf`, et les deux se servent de la clé à tour de rôle.

## Comparé à d’autres applications {#compared-with-other-apps}

**Vos mots de passe et vos clés, gardés par votre système et votre clé de sécurité.** Sioul garde les mots de passe des comptes dans le trousseau de votre système, là où Thunderbird garde sa propre réserve. Il laisse Bitwarden donner son mot de passe à un compte, ce qu’aucun autre logiciel de courrier ici ne décrit. Il se sert des clés OpenPGP directement depuis une clé de sécurité, sans GnuPG, là où Thunderbird a besoin de GnuPG et d’un réglage caché, et qualifie cela d’expérimental. D’autres font plus ailleurs : Sioul ne peut pas ajouter d’adresses Outlook.com, Hotmail ou Microsoft 365, n’a pas S/MIME, et a besoin d’une application Google à vous, ou d’un mot de passe d’application, pour le courrier de Gmail ; sa clé de sécurité OpenPGP n’a pas encore été essayée avec une vraie clé, et n’existe pas encore sur les téléphones.

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

=== "Au quotidien"

    | | Sioul | Thunderbird | Gmail | Outlook | Apple Mail | Evolution |
    |---|---|---|---|---|---|---|
    | Google connecté sur la page de Google elle-même | en partie¹ | ✓ | — | ✓ | ? | ✓ |
    | Les adresses Outlook.com et Microsoft 365 | ✗² | ✓ | ✓³ | ✓ | ? | ? |
    | Un gestionnaire de mots de passe donne son mot de passe à un compte | ✓⁴ | ✗ | ✗ | ✗ | ? | ✗ |
    | Un service d’un compte éteint, ses réglages gardés | ✓ | en partie⁵ | — | ? | ✓ | ✓ |

    1. La page de Google s’ouvre dans votre navigateur. Le courrier de Gmail demande une application Google à vous (un projet Google Cloud gratuit, environ un quart d’heure, une fois) ou un mot de passe d’application ; les agendas aussi, dans une copie de Sioul construite sans clé à elle.
    2. Microsoft n’accepte des autres logiciels de courrier que sa propre page de connexion, et Sioul n’en a pas encore pour le courrier ([Fonctionne avec](compatibility.md#mail)).
    3. Dans l’application Gmail d’un téléphone.
    4. Bitwarden, lu par Sioul lui-même. Bitwarden lui-même ne décrit le remplissage que dans les navigateurs et sur les téléphones.
    5. Les agendas seulement.

=== "Technique"

    | | Sioul | Thunderbird | Gmail | Outlook | Apple Mail | Evolution |
    |---|---|---|---|---|---|---|
    | Les mots de passe gardés dans le trousseau de votre système | ✓ | ✗¹ | ? | ? | en partie² | ✓³ |
    | Votre clé de sécurité quand vous vous connectez à Google | ✓⁴ | en partie⁵ | ✓ | ? | ? | ? |
    | OpenPGP : créer des clés, trouver les clés des autres | ✓ | ✓⁶ | ✗ | ✗ | ✗ | en partie⁷ |
    | Les clés OpenPGP gardées sur une clé de sécurité | ✓⁸ | en partie⁹ | ✗ | ✗ | ✗ | ? |
    | Certificats S/MIME | ✗ | ✓ | en partie¹⁰ | en partie¹¹ | ✓¹² | ✓ |
    | Logiciel libre | ✓ GPL-3.0+ | ✓ MPL-2.0 | ✗ | ✗ | ✗ | ✓ LGPL-2.1+ |

    1. Sa propre réserve de mots de passe, protégée par un mot de passe principal si vous en mettez un.
    2. Gardés par le trousseau iCloud, comme le dit Apple pour les comptes tenus à jour entre vos appareils.
    3. Sous Linux.
    4. La page de Google s’ouvre dans votre navigateur, qui demande la clé.
    5. Les anciennes clés U2F marchent dans sa fenêtre de connexion ; les clés qui demandent un code PIN, et les clés d’accès, sont des demandes ouvertes.
    6. Autocrypt en partie, ce que Thunderbird appelle une «  compatibilité limitée  ».
    7. Par GnuPG, qui crée les clés ; Autocrypt et l’annuaire de clés Web Key Directory depuis 2022–2023.
    8. Sur un ordinateur, par le service des cartes à puce du système ; pas encore sur un téléphone ; pas encore essayé avec une vraie clé ([Fonctionne avec](compatibility.md#logins-and-keys)).
    9. Seulement avec GnuPG installé et un réglage caché activé ; GnuPG signe et déchiffre alors ; dit expérimental.
    10. Dans les éditions de Google Workspace seulement, activé par un administrateur.
    11. Décrit pour les comptes professionnels ou scolaires.
    12. Sur un Mac.

??? info "Sources (en anglais)"
    - Assistance de Thunderbird : Thunderbird et Gmail, <https://support.mozilla.org/en-US/kb/thunderbird-and-gmail>, lu le 8 octobre 2026 (par sa dernière copie sur web.archive.org).
    - Assistance de Thunderbird : la connexion OAuth de Microsoft, <https://support.mozilla.org/en-US/kb/microsoft-oauth-authentication-and-thunderbird-202>, lu le 8 octobre 2026 (par web.archive.org).
    - Assistance de Thunderbird : le gestionnaire de mots de passe, <https://support.mozilla.org/en-US/kb/password-manager-remember-delete-change-tb>, lu le 8 octobre 2026 (par web.archive.org).
    - Assistance de Thunderbird : le mot de passe principal, <https://support.mozilla.org/en-US/kb/protect-your-thunderbird-passwords-primary-password>, lu le 8 octobre 2026 (par web.archive.org).
    - Thunderbird : bogue 1562324, les mots de passe dans la réserve du système (ouvert), <https://bugzilla.mozilla.org/show_bug.cgi?id=1562324>, lu le 8 octobre 2026.
    - Thunderbird : bogue 1444101, les clés de sécurité dans la fenêtre de connexion (corrigé en 2018), <https://bugzilla.mozilla.org/show_bug.cgi?id=1444101>, lu le 8 octobre 2026.
    - Thunderbird : bogue 1868343, le code PIN d’une clé FIDO2 dans la fenêtre de connexion (ouvert), <https://bugzilla.mozilla.org/show_bug.cgi?id=1868343>, lu le 8 octobre 2026.
    - Thunderbird : bogue 1864917, les clés d’accès (ouvert), <https://bugzilla.mozilla.org/show_bug.cgi?id=1864917>, lu le 8 octobre 2026.
    - Assistance de Thunderbird : OpenPGP, mode d’emploi et questions, <https://support.mozilla.org/en-US/kb/openpgp-thunderbird-howto-and-faq>, lu le 8 octobre 2026 (par web.archive.org).
    - Wiki de Mozilla : les cartes à puce OpenPGP dans Thunderbird (modifié le 23 juillet 2021), <https://wiki.mozilla.org/Thunderbird:OpenPGP:Smartcards>, lu le 8 octobre 2026.
    - Thunderbird : les chaînes de l’agenda, <https://searchfox.org/comm-central/source/calendar/locales/en-US/calendar/calendar.ftl>, lu le 8 octobre 2026.
    - Thunderbird : sa licence, <https://searchfox.org/comm-central/source/LICENSE>, lu le 8 octobre 2026.
    - Assistance de Thunderbird : la configuration automatique des comptes, <https://support.mozilla.org/en-US/kb/automatic-account-configuration>, lu le 8 octobre 2026 (par web.archive.org).
    - Aide de Bitwarden : premiers pas avec l’application de bureau, <https://bitwarden.com/help/getting-started-desktop/>, lu le 8 octobre 2026.
    - Aide de Bitwarden : ce que l’application de bureau prend en charge, <https://bitwarden.com/help/desktop-app-feature-support/>, lu le 8 octobre 2026.
    - Aide de Bitwarden : le remplissage sous Android, <https://bitwarden.com/help/auto-fill-android/>, lu le 8 octobre 2026.
    - Aide de Gmail : Gmail dans d’autres logiciels de courrier, <https://support.google.com/mail/answer/6078445>, lu le 8 octobre 2026.
    - Aide de Gmail : ajouter d’autres comptes dans l’application Gmail, <https://support.google.com/mail/answer/21289>, lu le 8 octobre 2026.
    - Aide du compte Google : les mots de passe d’application, <https://support.google.com/accounts/answer/185833>, lu le 8 octobre 2026.
    - Aide du compte Google : les clés de sécurité pour la validation en deux étapes, <https://support.google.com/accounts/answer/6103523>, lu le 8 octobre 2026.
    - Aide du compte Google : les clés d’accès, <https://support.google.com/accounts/answer/13548313>, lu le 8 octobre 2026.
    - Administration de Google Workspace : S/MIME hébergé, <https://knowledge.workspace.google.com/admin/gmail/advanced/turn-on-hosted-s-mime-for-message-encryption>, lu le 8 octobre 2026.
    - Administration de Google Workspace : le chiffrement côté client, <https://knowledge.workspace.google.com/admin/security/about-client-side-encryption>, lu le 8 octobre 2026.
    - Administration de Google Workspace : le chiffrement par clé matérielle pour Gmail, <https://knowledge.workspace.google.com/admin/security/gmail-only-set-up-and-manage-hardware-key-encryption>, lu le 8 octobre 2026.
    - Outlook : ajouter un compte Gmail à Outlook pour Windows, <https://support.microsoft.com/en-us/outlook/getstarted/add-a-gmail-account-to-outlook-for-windows>, lu le 8 octobre 2026.
    - Outlook : les comptes synchronisés par le nuage de Microsoft, <https://support.microsoft.com/en-us/Outlook/getstarted/sync-your-account-in-outlook-to-the-microsoft-cloud>, lu le 8 octobre 2026.
    - Microsoft Purview : le chiffrement du courrier, <https://learn.microsoft.com/en-us/purview/email-encryption>, lu le 8 octobre 2026.
    - Assistance Microsoft : chiffrer des courriels, <https://support.microsoft.com/en-us/office/encrypt-email-messages-373339cb-bf1a-4509-b296-802a39d801dc>, lu le 8 octobre 2026.
    - Outlook sur le web : S/MIME, <https://support.microsoft.com/en-us/office/encrypt-messages-by-using-s-mime-in-outlook-on-the-web-878c79fc-7088-4b39-966f-14512658f480>, lu le 8 octobre 2026.
    - Compte Microsoft : Windows Hello et les clés de sécurité (redirigé aujourd’hui vers «  Configurer Windows Hello  »), <https://support.microsoft.com/en-us/account-billing/sign-in-to-your-microsoft-account-with-windows-hello-or-a-security-key-800a8c01-6b61-49f5-0660-c2159bea4d84>, lu le 8 octobre 2026.
    - Microsoft Learn : présentation du nouvel Outlook pour Windows, <https://learn.microsoft.com/en-us/microsoft-365-apps/outlook/overview-new-outlook-windows>, lu le 8 octobre 2026.
    - Mail pour Mac : ajouter et gérer des comptes, <https://support.apple.com/guide/mail/add-and-manage-email-accounts-mail35803/mac>, lu le 8 octobre 2026.
    - Guide de l’iPhone : les comptes de courrier, de contacts et de calendrier, <https://support.apple.com/guide/iphone/add-mail-contacts-and-calendar-accounts-ipha0d932e96/ios>, lu le 8 octobre 2026.
    - Trousseaux d’accès : ce que c’est, <https://support.apple.com/guide/keychain-access/what-is-keychain-access-kyca1083/mac>, lu le 8 octobre 2026.
    - Mail pour Mac : les certificats personnels, <https://support.apple.com/guide/mail/use-personal-certificates-mlhlp1179/mac>, lu le 8 octobre 2026.
    - Guide de l’iPhone : ajouter et supprimer des comptes de courrier, <https://support.apple.com/guide/iphone/add-and-remove-email-accounts-iph44d1ae58a/ios>, lu le 8 octobre 2026.
    - Aide d’Evolution : un compte Gmail, <https://help.gnome.org/evolution/mail-access-gmail-imap-account.html>, lu le 8 octobre 2026.
    - Evolution : le code de sa connexion à Google, <https://gitlab.gnome.org/GNOME/evolution-data-server/-/raw/master/src/libedataserver/e-oauth2-service-google.c>, lu le 8 octobre 2026.
    - Evolution : le code de sa réserve de secrets, <https://gitlab.gnome.org/GNOME/evolution-data-server/-/raw/master/src/libedataserver/e-secret-store.c>, lu le 8 octobre 2026.
    - Aide d’Evolution : le premier lancement, <https://help.gnome.org/evolution/intro-first-run.html>, lu le 8 octobre 2026.
    - Aide d’Evolution : le chiffrement, <https://help.gnome.org/evolution/mail-encryption.html>, lu le 8 octobre 2026.
    - Aide d’Evolution : préparer GnuPG, <https://help.gnome.org/evolution/mail-encryption-gpg-set-up.html>, lu le 8 octobre 2026.
    - Evolution : son fichier NEWS, <https://gitlab.gnome.org/GNOME/evolution/-/raw/master/NEWS>, lu le 8 octobre 2026.
    - Aide d’Evolution : les certificats S/MIME, <https://help.gnome.org/evolution/mail-encryption-s-mime-manage.html>, lu le 8 octobre 2026.
    - Aide d’Evolution : les types de comptes, <https://help.gnome.org/evolution/intro-account-types.html>, lu le 8 octobre 2026.
    - Evolution : son code et sa licence, <https://gitlab.gnome.org/GNOME/evolution>, lu le 8 octobre 2026.

## Côté technique {#for-technical-readers}

- **Les connexions** : le courrier en TLS dès le premier octet (préféré, RFC 8314) ou en STARTTLS, sans réglage non chiffré ; un STARTTLS qui échoue met fin à la connexion au lieu de continuer en clair. Les certificats sont vérifiés d’après ceux du système (la liste de Mozilla quand le système n’en a pas), avec rustls. CalDAV et CardDAV en HTTPS seulement.
- **Trouver le serveur** : `autoconfig.<domaine>`, puis `<domaine>/.well-known/autoconfig`, puis la liste des fournisseurs de Thunderbird (à qui l’on dit le domaine, jamais l’adresse), puis `imap.<domaine>` et `mail.<domaine>`, pour que vous vérifiiez ; HTTPS seulement, car un fichier de réglages venu en clair pourrait envoyer votre mot de passe ailleurs. Les domaines Google Workspace sont reconnus par leurs enregistrements MX.
- **Google** : OAuth 2.0 pour les applications natives (RFC 8252) : votre navigateur s’ouvre sur la page de Google, la réponse revient à une adresse en boucle locale sur un port tiré au hasard, et PKCE (S256, RFC 7636) avec un `state` tiré au hasard prouve que c’est le même programme. Le courrier (IMAP et SMTP, SASL XOAUTH2) a sa propre autorisation, à part des agendas, des contacts et des tâches, pour que l’un puisse être retiré seul. Le jeton de renouvellement est gardé dans le trousseau, le jeton d’accès en mémoire seulement ; retirer un compte révoque son jeton chez Google.
- **Le trousseau** : le Secret Service sous Linux (KWallet, GNOME Keyring), le trousseau de macOS, le gestionnaire d’identifiants de Windows, le KeyStore d’Android sur un téléphone. Chaque entrée est rangée sous `sioul`, en «  *identifiant* on *serveur*  », facile à retrouver, vérifier ou retirer là.
- **OpenPGP** : RFC 9580, avec Sequoia-PGP et sa cryptographie en Rust pur. Une clé créée ici est Ed25519 pour signer et Curve25519 pour chiffrer, valable trois ans, ses parties secrètes chiffrées par une phrase secrète tirée au hasard et gardée dans le trousseau. Autocrypt niveau 1 : votre courrier porte votre clé ; une clé trouvée dans l’en-tête d’un expéditeur n’est gardée que si elle nomme cet expéditeur, parties publiques seulement. Les clés des autres ne sont cherchées que quand vous le demandez : dans le Web Key Directory de leur domaine, puis sur keys.openpgp.org, en HTTPS seulement. Pas de S/MIME.
- **La clé de sécurité** : la spécification de carte OpenPGP 3.4, par le service des cartes à puce du système, PC/SC (pcscd sous Linux, que la plupart des systèmes démarrent quand un programme le demande ; le service Carte à puce sous Windows ; CryptoTokenKit sous macOS). La clé est ouverte pour une seule opération, puis remise à zéro, pour que son code PIN ne reste pas donné à un autre programme ; les lecteurs sont joints en mode partagé. Le code PIN est gardé en mémoire, chiffré, pendant quinze minutes sans usage, jamais écrit. Une signature que fait la clé est vérifiée d’après sa partie publique avant que le message parte. Les journaux des bibliothèques sont bridés, pour qu’aucun code PIN ni aucune clé de session ne puisse atteindre un journal.
- **GnuPG** : son service de cartes à puce garde la clé pour lui après s’en être servi, sauf si `scdaemon.conf` dit `pcsc-shared`. **Laisser GnuPG la libérer** lance `gpgconf --kill scdaemon`, à ce clic seulement ; gpg le redémarre quand il a besoin de la clé, et redemande alors son code PIN. Sioul ne change jamais les réglages de GnuPG.
- **Bitwarden depuis cette page** : le coffre s’ouvre avec son mot de passe principal et une deuxième étape que cette boîte de dialogue sait demander : le code d’une application d’authentification, un code par courriel, le code à usage unique d’une YubiKey, un code de récupération. Une clé de sécurité, en deuxième étape ou seule, n’est demandée que par la boîte de dialogue de la page Sites, sur un ordinateur ; le coffre reste alors ouvert pour toutes les pages jusqu’à la fermeture de Sioul. Comment le coffre est lu : [Sites, côté technique](sites.md#for-technical-readers).
