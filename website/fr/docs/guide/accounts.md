---
description: La page Comptes de Sioul – une fiche par adresse, un interrupteur pour chacun de ses services ; Google connecté sur la page de Google ; les mots de passe dans le trousseau de votre système, ou venus de Bitwarden ; vos clés OpenPGP, sur cet appareil ou sur une clé de sécurité ; comparée aux autres logiciels de courriel.
---

# Les comptes {#accounts}

## En bref {#in-short}

**Comptes** est l’icône de personne en bas de la colonne de gauche. La page tient une fiche par adresse, avec un interrupteur pour chacun de ses services : les courriels, les agendas et les contacts, ceux de Google. Les mots de passe vont dans le trousseau de votre système et ne voyagent jamais entre vos appareils ; Google se connecte sur sa propre page ; Bitwarden peut donner son mot de passe à un compte. L’onglet **Chiffrement** tient vos clés OpenPGP, gardées sur cet appareil ou sur une clé de sécurité comme une YubiKey.

Qui peut vous joindre, et quand, est dans Paramètres ▸ [Ce qui vous joint](notifications.md). Les sites (messageries sécurisées, discussions) ne sont pas des comptes : ils se créent et se modifient sur la page [Sites](sites.md).

## Protégé par défaut {#what-is-protected}

- Chaque mot de passe va dans le trousseau de votre système, jamais dans un fichier. Les mots de passe ne voyagent jamais entre vos appareils : chaque appareil le demande une fois.
- Sioul ne parle à vos serveurs de courriel, d’agendas et de contacts que par des connexions chiffrées : aucun réglage ne permet de s’en passer.
- Google : vous vous connectez sur la page de Google elle-même, dans votre navigateur habituel, avec ce que Google y demande (votre mot de passe, votre clé de sécurité, une clé d’accès). Sioul ne voit jamais votre mot de passe Google, et retirer le compte rend l’accès à Google.
- Bitwarden peut donner son mot de passe à un compte : Sioul lit votre coffre lui-même et n’y écrit jamais.
- Vos clés OpenPGP restent sur cet appareil, ou sur votre clé de sécurité ; les personnes à qui vous écrivez reçoivent votre clé publique avec vos courriels, pour pouvoir vous répondre chiffré.
- Sur une clé de sécurité, vos clés ne la quittent jamais. Son code PIN est gardé en mémoire un quart d’heure sans usage sur un ordinateur, tapé à chaque usage sur Android, et jamais écrit nulle part.
- Chercher la clé de quelqu’un dit à un serveur quelle clé vous cherchez : Sioul ne le fait que quand vous le demandez.

## Vos comptes {#your-accounts}

<figure markdown="span">
  [![L’onglet « Vos comptes » : en haut, la fiche « Gestionnaire de mots de passe », avec une ligne, les champs de l’adresse et du serveur et « Ajouter votre compte Bitwarden » ; puis « Jusqu’où remontent les courriels et l’agenda », réglé sur 2 semaines ; en dessous, la fiche d’une adresse : « Ce que ce serveur offre » en haut, son service de courriel avec son serveur, un interrupteur et Retirer, « Réglages de cette adresse », replié, sa priorité, « Nom et signature… », sa couleur, et « Serveur et dossiers », replié.](../assets/screens/fr/accounts.png){ loading=lazy }](../assets/screens/fr/accounts.png "Ouvrir l’image en grand")
  <figcaption>Une fiche par adresse, chacun de ses services avec un interrupteur.</figcaption>
</figure>

**Vos comptes** est l’onglet sur lequel la page s’ouvre. En haut, **Gestionnaire de mots de passe** : votre compte Bitwarden, un seul pour toute l’application ([plus bas](#your-password-manager)).

Puis **Jusqu’où remontent les courriels et l’agenda** : d’une semaine à un an, ou tout. Les courriels plus anciens arrivent au prochain relevé ; « Tout » peut prendre du temps, et de la place sur le disque.

Puis une fiche par adresse, avec chacun de ses services : **Courriels** ; **Agendas, tâches et contacts** ; **Google : agendas, contacts et tâches**. Chacun a :

- **un interrupteur** : éteint, le service garde ses réglages et n’est ni synchronisé ni montré ;
- son serveur, et ce qu’a dit sa dernière relève, en une phrase ;
- **Retirer**.

**Ce que ce serveur offre**, en haut de la fiche, demande au serveur ce qu’il a d’autre : des agendas et des contacts à côté des courriels, et pour un Nextcloud, sa version et ses applications, chacune dite dans Sioul ou pas encore (elles sont listées une fois ses agendas ici : c’est leur mot de passe qui les demande). Ce dont Sioul peut se servir s’ajoute de là : les courriels en un clic, avec le mot de passe de ses agendas, ou avec un autre mot de passe ; les agendas et les contacts par **Les ajouter**, qui ouvre leur formulaire dans [Ajouter un compte](#add-an-account), l’adresse remplie, pour leur mot de passe.

### Votre gestionnaire de mots de passe {#your-password-manager}

Sioul remplit les identifiants des sites et les mots de passe des comptes depuis votre coffre Bitwarden, lu par Sioul lui-même : rien d’autre à installer. Sa fiche, en haut de **Vos comptes**, tient :

- **Compte Bitwarden** : son adresse ; **Serveur Bitwarden** : vide pour bitwarden.com, ou bitwarden.eu, ou l’adresse de votre propre serveur (Vaultwarden aussi).
- **Bitwarden sur cet appareil** : « Bitwarden est ouvert. » avec **Verrouiller**, ou « Bitwarden est verrouillé. » avec **Déverrouiller**. Déverrouiller ouvre le coffre pour la session : avec votre seule clé de sécurité, ou avec votre mot de passe principal et la deuxième étape que demande votre compte ([Sites](sites.md#logins-from-bitwarden) dit chacune). Sur un téléphone, par le mot de passe principal et un code : celui d’une application d’authentification, d’un courriel, d’une YubiKey, un code de récupération.
- **Verrouiller Bitwarden** : après un temps sans usage, de cinq minutes à quatre heures, comme les applications de Bitwarden ; tant que vous n’avez pas choisi, il reste ouvert jusqu’à la fermeture de Sioul.

Sans compte, la fiche le demande elle-même : une ligne, l’adresse de votre compte Bitwarden, son serveur quand ce n’est pas bitwarden.com, et **Ajouter votre compte Bitwarden**, qui ouvre aussitôt le coffre. Il se règle et se change sur cette seule fiche. Le compte suit vos réglages sur vos autres appareils, quand vous les [partagez](sharing.md) ; le coffre lui-même ne quitte jamais l’appareil qui l’a ouvert. Sur un ordinateur, la page Sites remplit ses identifiants depuis le même coffre (un identifiant demandé avant qu’un compte soit réglé le dit, avec un bouton vers cette fiche) ; sur Android, où les sites s’ouvrent dans le navigateur, le coffre ne sert qu’aux mots de passe des comptes.

### Une adresse électronique {#a-mail-address}

Sur sa fiche, **Réglages de cette adresse**, replié tant que vous ne l’ouvrez pas :

- **À quoi sert cette adresse** : travail, vos démarches, loisirs, plusieurs à la fois. Ses courriels sont notifiés aux heures de ce à quoi elle sert ; ceux de vos expéditeurs sûrs s’affichent sur le Porche à leurs moments et sont notifiés quand ces heures commencent, et ceux des personnes qui passent toujours viennent tout de suite. Sans rien de coché, elle compte comme du travail, pour que les courriels des autres n’atteignent jamais vos soirées ; ceux de vos expéditeurs sûrs sont toujours notifiés à leurs moments. Voir [Les heures](hours.md).
- **Jusqu’où remonter** : combien de semaines des courriels de cette adresse ses dossiers montrent, ou comme les autres adresses.
- **Relever toutes les** : à quel rythme ses dossiers autres que la boîte de réception sont relevés ; 0 suit les autres adresses, dont le rythme est **Relever toutes les** dans le ⚙ de la page Courriels. Une adresse publique peut être relevée deux fois par jour (720 minutes).
- **Protégée contre le harcèlement**, et une fois cette protection activée, **Laisser l’IA le lire d’abord** (pas proposé pour une adresse Microsoft professionnelle ou scolaire : sa fiche dit pourquoi). Voir [le Porche](porch.md#a-public-address-protected).

Toujours en vue en dessous :

- **Priorité** : *Plus important*, *Normal* ou *Moins important*. Voir [le Porche](porch.md#some-addresses-first-others-last).
- **Nom et signature…** : votre nom, tel que les destinataires le voient, et votre signature, en Markdown.
- **Sa couleur**, dès que vous avez plus d’une adresse : l’une de huit pour le carré qui marque ses courriels dans [Toutes les adresses](mail.md#all-addresses), ou **Selon son rang**. Vos autres appareils dessinent la même.

Puis **Serveur et dossiers**, replié : le serveur, le nom sous lequel votre fournisseur consigne ses vérifications des expéditeurs (**Vérification des expéditeurs**, appris de vos courriels), et l’endroit où ses courriels sont gardés sur cet appareil.

Sous les fiches, dès qu’une adresse laisse l’IA la lire d’abord, **Clé de l’API d’Anthropic** prend la clé du service d’Anthropic, utilisée seulement par ces adresses : collez-la et choisissez **Garder**. Elle est gardée dans le trousseau de votre système, jamais dans un fichier ; **Oublier la clé** la retire.

### Retirer un compte {#removing-one}

**Retirer** demande d’abord. Le mot de passe d’un compte de courriel quitte le trousseau ; ses courriels restent sur votre disque et sur le serveur. Pour Google, l’accès est rendu à Google : celui des courriels et celui des agendas à part, pour que retirer l’un laisse son accès à l’autre. Pour Microsoft, sa connexion est oubliée sur cet appareil, et **Retirer** demande aussi s’il faut supprimer les courriels que Sioul garde sur cet appareil pour ce compte : oui, sauf si vous décochez. Microsoft n’offre aux applications aucun moyen de rendre un accès : la ligne que Sioul montre ensuite nomme la page de Microsoft où vous le retirez.

### Un compte venu de votre autre appareil {#an-account-from-your-other-device}

[Le partage entre vos appareils](sharing.md) apporte vos comptes, jamais leurs mots de passe. Un tel compte dit qu’il n’a pas encore de mot de passe ici, avec **Mot de passe…** sur sa fiche : tapez-le, ou choisissez **Depuis Bitwarden…** (montré une fois votre [gestionnaire de mots de passe](#your-password-manager) réglé). Les identifiants dont le nom d’utilisateur est cette adresse sont listés, puis ceux faits pour son domaine sous un autre nom d’utilisateur (un numéro de client, une autre adresse), ceux du serveur en premier ; choisissez-en un, ou tapez un site pour les restreindre ([Sites](sites.md#logins-from-bitwarden)). Le mot de passe est essayé auprès du serveur, puis gardé dans le trousseau de cet appareil. **Mot de passe…** revient si le serveur refuse un jour celui qui est gardé.

Depuis cette page, votre coffre Bitwarden s’ouvre comme depuis la page Sites : avec votre clé de sécurité, seule ou en deuxième étape, ou avec votre mot de passe principal et le code d’une application, celui d’un courriel, celui d’une YubiKey, un code de récupération. Il reste ensuite ouvert pour toutes les pages jusqu’à ce que vous le verrouilliez, jusqu’à la fin du temps sans usage choisi sous **Verrouiller Bitwarden**, ou jusqu’à la fermeture de Sioul. Sur Android, une clé de sécurité ne peut pas encore ouvrir le coffre : prenez une autre deuxième étape de votre compte Bitwarden.

### Les courriels de Google {#googles-mail}

Le compte de courriel Gmail ou Google Workspace connecté avec Google dit quand Google met fin à l’accès (votre mot de passe Google changé, votre propre application Google supprimée, un projet Google laissé en test). Sur sa fiche, **Se reconnecter** ouvre le formulaire des courriels dans [Ajouter un compte](#add-an-account), son adresse remplie : **Se connecter avec Google** y rouvre la page de Google, dans votre navigateur ; **Mot de passe d’application…** lui donne un mot de passe d’application à la place. Les agendas, contacts et tâches de Google ont aussi **Se reconnecter** quand Google a mis fin à leur accès : il ouvre la page de Google tout de suite. Sioul ne demande jamais le mot de passe de votre compte Google. Voir [Premiers pas](first-steps.md#gmail-and-google-workspace).

### Les courriels de Microsoft {#microsofts-mail}

Le compte de courriel Outlook.com, Hotmail, Live, MSN ou Microsoft 365 connecté avec Microsoft dit quand Microsoft met fin à l’accès (votre mot de passe changé, quatre-vingt-dix jours sans usage, les règles de votre organisation), ou quand la boîte reste fermée aux logiciels de courriel, avec ce qu’il faut faire. Sur sa fiche, **Se reconnecter** ouvre le formulaire des courriels dans [Ajouter un compte](#add-an-account), son adresse remplie : **Se connecter avec Microsoft** y rouvre la page de Microsoft, dans votre navigateur. Une adresse ajoutée avant avec un mot de passe, sur le serveur de Microsoft, montre **Se connecter avec Microsoft** au lieu de **Mot de passe…**, qui mène au même formulaire : Microsoft ne prend plus aucun mot de passe, mots de passe d’application compris, et se connecter oublie celui qui était gardé. Sioul ne demande jamais votre mot de passe Microsoft. Sous **Serveur et dossiers**, sa fiche dit **Connexion** : avec Microsoft, comme compte personnel ou comme compte professionnel ou scolaire. Voir [Premiers pas](first-steps.md#outlook-and-microsoft-365).

## Ajouter un compte {#add-an-account}

L’onglet **Ajouter un compte** tient trois formulaires, l’un après l’autre (une fois un compte ajouté, la page revient à **Vos comptes**) :

- **Ajouter un compte de courriel** : votre adresse, **Trouver le serveur**, votre mot de passe, **Se connecter et ajouter**. Pour Gmail et Google Workspace, **Utiliser un mot de passe d’application** ou **Se connecter avec Google** au lieu d’un mot de passe (pour Workspace, la connexion est la façon fiable : Google peut ne pas lui proposer de mot de passe d’application) ; pour Yahoo Mail, un mot de passe d’application comme mot de passe (**Créer un mot de passe d’application** ouvre la page de sécurité de votre compte Yahoo) ; pour Outlook.com, Hotmail, Live, MSN et Microsoft 365, **Se connecter avec Microsoft**. Quand rien n’est trouvé pour une adresse professionnelle ou scolaire gardée chez Microsoft 365, **Cette adresse est chez Microsoft**, sous le formulaire, amène la connexion de Microsoft. Pour un domaine à vous gardé chez un hébergeur, Sioul cherche le serveur de l’hébergeur ; s’il n’en trouve pas, le formulaire s’ouvre vide pour le nom de serveur que donne votre hébergeur, et **Serveur d’envoi, s’il ne se trouve pas** prend un autre serveur pour l’envoi ([Votre propre domaine chez un hébergeur](first-steps.md#your-own-domain-at-a-host)).
- **Ajouter des contacts et agendas** : depuis un serveur CalDAV et CardDAV, comme Nextcloud, Fastmail, iCloud ou votre hébergeur : votre adresse, son mot de passe, **Se connecter et ajouter** ; **Adresse du serveur, s’il ne se trouve pas**, replié, prend l’adresse du serveur et son identifiant. Ses carnets d’adresses et ses agendas se gèrent ensuite là où ils servent : Contacts ⚙ ▸ **Carnets d’adresses** ([Les contacts](contacts.md#address-books)) et Agenda ⚙ ▸ **Vos agendas** ([L’agenda](agenda.md#agendas-on-their-servers)). Un agenda lu par son adresse web (l’emploi du temps d’un établissement scolaire, les jours fériés) n’est pas un compte : on s’y abonne là, avec **S’abonner à un agenda…** ([Les agendas par leur adresse](agenda.md#calendars-by-their-address)).
- **Agendas, contacts et tâches Google** : **Se connecter avec Google**, sur la page de Google elle-même.

Chaque champ de mot de passe de ces formulaires a **Depuis Bitwarden…** en dessous, une fois votre gestionnaire de mots de passe réglé : le coffre s’ouvre s’il est verrouillé, puis ses identifiants sont listés par le nom avec lequel le compte se connecte, et celui que vous choisissez donne son mot de passe ; le bouton dit alors de quel identifiant il vient. Ce mot de passe ne reste dans le champ que tant que le coffre est ouvert et le formulaire à l’écran : verrouiller le coffre (ou son temps sans usage), quitter Ajouter un compte, ou ajouter le compte vide le champ, et Annuler comme Rétablir n’en ramènent rien, même après que vous l’avez vidé à la main. Si vous avez tapé votre propre mot de passe par-dessus, le vôtre reste, sans celui du coffre derrière lui. Un champ que le coffre n’a jamais rempli garde ce que vous y avez tapé.

Sur Android, **Depuis les comptes du téléphone…** (de la tablette, sur une tablette), au-dessus d’eux, ouvre la liste d’Android des comptes que l’appareil connaît. Une adresse Google remplit le formulaire de Google et celui des courriels, avec les deux façons de Google ; toute autre remplit le formulaire des courriels, dont Sioul cherche alors le serveur, et celui des contacts et agendas. Android ne prête aucun mot de passe : Sioul le demande une fois.

Pas à pas : [Premiers pas](first-steps.md#add-your-mail). Les mots de passe vont dans le trousseau de votre système, nulle part ailleurs, et Sioul ne parle à vos serveurs que par des connexions chiffrées.

Les adresses Outlook.com, Hotmail, Live, MSN et Microsoft 365 se connectent sur la page de Microsoft elle-même, dans votre navigateur : Microsoft ne prend aucun mot de passe des autres logiciels de courriel. Une adresse professionnelle ou scolaire peut demander l’approbation de son organisation, une fois ([Premiers pas](first-steps.md#outlook-and-microsoft-365) ; [Fonctionne avec](compatibility.md#mail)).

## Qui peut vous joindre {#senders}

Qui peut vous joindre, et quand, par courriel et par les messages des autres applications, et qui est dans quel cercle (sûrs, restreints, bloqués, avec des adresses, des numéros, des motifs comme `*@example.org`, les personnes mises sur une liste et vos groupes ; vos contacts sont toute personne de vos carnets d’adresses) : Paramètres ▸ **Ce qui vous joint**, sous **Personnes** et **Qui est dans quel cercle** ([Ce qui vous joint, et quand](notifications.md#by-person)). Les groupes d’un carnet d’adresses que quelqu’un d’autre peut modifier n’y comptent qu’une fois que vous le dites, dans Contacts ⚙ ▸ **Carnets d’adresses** ([Les contacts](contacts.md#address-books)). Pour une personne, sa fiche, depuis sa fiche de contact ou un message : **Comment … vous joint** ([La fiche d’une personne](notifications.md#a-persons-sheet)). Ce à quoi sert chaque adresse reste sur sa fiche, plus haut : les courriels d’une adresse pour un autre moment attendent ce moment.

## Chiffrement {#encryption}

Avec une clé à vous, n’importe qui, un organisme compris, peut vous écrire de façon que vous seul puissiez le lire, dans votre propre boîte plutôt que sur son portail ([pourquoi](privacy-security.md#why-sioul-encrypts-mail)).

Vos clés OpenPGP, pour signer et chiffrer vos courriels ([Les courriels](mail.md#signing-and-encrypting)) :

- **Créer une clé pour** une adresse : une nouvelle clé, valable trois ans ; sa phrase secrète est faite au hasard et gardée dans votre trousseau, si bien que rien n’est demandé à chaque courriel.
- **Importer une clé…** : une clé exportée de GnuPG, avec sa phrase secrète, demandée une fois.
- **Enregistrer la clé publique** : dans vos téléchargements, pour la donner aux autres.
- **Enregistrer le certificat de révocation**, pour une clé créée ici : une copie dans vos téléchargements, à garder en lieu sûr, ailleurs que sur cet appareil. C’est lui qui déclare la clé hors d’usage, si elle est un jour perdue ou volée ; qui le détient peut le faire. Sioul le garde à côté de la clé, lisible par vous seulement ([Les courriels, OpenPGP](mail.md)), et ne sait pas encore révoquer une clé.
- **Clés des autres**, replié : celles venues avec leurs courriels, ou d’un fichier, ou trouvées par **Chercher leurs clés** dans la fenêtre d’écriture. Chacune ne sert que l’adresse pour laquelle elle est venue, ou toutes les adresses d’un fichier que vous importez. Une clé que Sioul gardait avant le 19 octobre 2026 et qui nomme plus d’une adresse, dans l’un quelconque de ses identifiants, n’en sert aucune jusqu’à ce qu’elle revienne : le prochain courriel de son propriétaire, **Chercher leurs clés**, ou le fichier importé de nouveau. Une clé que son propriétaire a révoquée n’en sert aucune non plus. **Retirer**, à côté de chacune, l’enlève.

Vos propres clés restent sur cet appareil. Elles ne sont pas partagées avec vos autres appareils : copiez-les à la main si vous en avez besoin là-bas. La partie publique d’une clé de sécurité fait exception : elle voyage (ci-dessous).

### Clé de sécurité {#security-key}

Vos clés OpenPGP peuvent rester sur une clé de sécurité, comme une YubiKey ou une Nitrokey : elle signe et ouvre vos courriels elle-même, après son code PIN, et ses clés privées ne la quittent jamais. Sioul n’en garde que la partie publique. Cela marche sur un ordinateur, branchée, et sur un téléphone ou une tablette, tenue contre son dos (NFC) ou branchée sur son port USB-C. Sur Android, c’est construit et testé avec une carte de remplacement, pas encore essayé avec une vraie clé.

Vos autres appareils apprennent cette partie publique de l’ordinateur qui a lu la clé, scellée avec le reste de ce qu’ils partagent, jamais une clé privée ni le code PIN. Un téléphone ou une tablette chiffre alors aussi pour vous ce que vous envoyez, pour que vous puissiez relire vos courriels envoyés sur chacun de vos appareils, et donne votre clé à ceux à qui vous écrivez. Lue une fois sur le téléphone ou la tablette (**Utiliser une clé de sécurité**, la clé tenue contre son dos ou branchée), elle y signe aussi ; d’ici là, **Signer** y reste désactivé, avec la raison. Un téléphone ou une tablette sans NFC ni port USB-C qui prenne une clé dit « Chiffré pour votre clé de sécurité : ouvrez-le sur un ordinateur pour l’instant » pour un courriel qui attend la clé. Un autre de vos ordinateurs qui a appris la clé ainsi chiffre lui aussi pour vous et donne votre clé, mais ne signe avec elle qu’une fois que vous l’y branchez et l’y lisez : d’ici là, il dit « Lue sur un autre de vos ordinateurs », et **Signer** reste désactivé. Sous vos clés, il ne propose jamais une seconde clé pour cette adresse, mais **Utiliser une clé de sécurité**, pour la lire là.

1. Branchez la clé, puis choisissez **Utiliser une clé de sécurité**. Sioul la lit sans son code PIN : son nom et son numéro de série, et ce avec quoi elle signe et déchiffre.
2. Sioul a besoin de la partie publique de ces clés, que la clé ne contient pas. **La chercher** interroge l’adresse écrite sur la clé, l’annuaire de clés de votre domaine, puis keys.openpgp.org ; cela dit à ces serveurs que quelqu’un cherche votre clé. **Importer depuis GnuPG** la prend à GnuPG sur cet ordinateur, s’il l’a. Ou **Importer un fichier…** ; sous **À la main**, la commande GnuPG qui l’écrit dans un fichier, prête à copier.
3. Sioul ne la garde que si c’est bien celle de la clé : les mêmes clés, les mêmes parties publiques. Votre clé de sécurité signe alors, seule, pour les adresses qu’elle nomme parmi les vôtres (une clé créée ou importée ici pour la même adresse n’ouvre plus que les anciens courriels), et ouvre ce qui lui est chiffré.

Quand vous envoyez un courriel signé ou ouvrez un courriel chiffré, Sioul demande la clé là où vous êtes : branchez-la, tapez son code PIN, et touchez-la quand Sioul le dit. Gardez le doigt sur la clé une seconde ou deux : un toucher bref n’est souvent pas pris, sa lumière en dit peu, et elle attend environ quinze secondes. Si la clé s’en va pendant qu’elle signe, rien n’est envoyé et le courriel reste dans les Brouillons ([Les courriels](mail.md#signing-and-encrypting)). S’il ne reste à la clé qu’un essai de code PIN (les autres dépensés ailleurs), Sioul n’envoie rien d’abord et le dit : retapez le code PIN seulement si vous en êtes sûr, car un code faux verrouillerait la clé.

Sur Android, Sioul demande d’abord le code PIN, puis la clé : tenez-la à plat contre le dos du téléphone ou de la tablette, ou branchez-la sur son port USB-C, et gardez-la là jusqu’à ce que Sioul dise que c’est fini. Tenue, le contact vaut toucher, quel que soit le réglage de toucher de la clé ; branchée, touchez-la quand Sioul le dit, comme sur un ordinateur. Si la clé s’éloigne ou est débranchée avant la fin, Sioul le dit, et rien n’a été envoyé. Le NFC désactivé propose **Ouvrir les réglages NFC** ; une clé branchée est prise quand même. Android demande une fois, à chaque branchement, si Sioul peut utiliser la clé : refusé, Sioul dit de la débrancher, de la rebrancher et de choisir Autoriser. Si une autre application tient la clé (Yubico Authenticator, OpenKeychain), Sioul le dit. Sioul ne lit les clés que pendant qu’il en demande une, et seulement la partie carte à puce de la clé : les autres applications et les codes à usage unique de la clé (Yubico OTP) marchent comme avant. Un code que la clé tape quand on la touche, pendant que le champ du code PIN attend, n’est jamais donné à la clé comme son code PIN. Le code PIN n’est jamais gardé sur l’appareil : vous le tapez à chaque usage, l’écran caché des captures pendant ce temps. Si aucune clé ne vient en une minute, Sioul oublie le code PIN tapé et le redemande.

Chaque clé montre pour quoi elle signe, si elle demande un toucher, d’où vient sa partie publique et quand elle expire. Sioul dit un mois avant qu’elle va expirer ; une fois expirée, les courriels ne peuvent plus être signés avec elle avant qu’elle soit renouvelée, et les anciens courriels s’ouvrent toujours. **Renouveler pour deux ans** la renouvelle avec GnuPG sur cet ordinateur, quand GnuPG connaît la clé : GnuPG demande le code PIN de la clé dans une fenêtre à lui, puis Sioul dit quand toucher la clé, le doigt gardé dessus une seconde ou deux, pendant que GnuPG renouvelle la clé puis ses sous-clés ; Sioul garde ensuite la partie publique renouvelée. Si la clé a attendu un toucher en vain, Sioul le dit, avec **Réessayer**. Là où GnuPG ne le peut pas (un Flatpak, Android, un ordinateur sans GnuPG), les deux commandes qui la renouvellent sont sous **À la main**, prêtes à copier. **Chercher une version plus récente** rapporte la partie publique renouvelée depuis les serveurs, **Importer depuis GnuPG** depuis GnuPG, ou **Importer un fichier…** depuis un nouvel export. **L’envoyer à keys.openpgp.org** y met votre clé publique, après une phrase qui dit ce qui devient public : chacun peut alors la trouver par son empreinte, et keys.openpgp.org envoie à chacune de ses adresses un lien qui, une fois ouvert, rend cette adresse trouvable ; Sioul ne l’envoie que quand vous choisissez **L’envoyer**. **Oublier le code PIN maintenant** oublie le code PIN que Sioul garde. **Ne plus utiliser cette clé de sécurité** la met de côté sur cet appareil ; vos autres ordinateurs qui l’ont lue continuent de l’utiliser jusqu’à ce que vous cessiez de l’utiliser là-bas aussi. Sur un ordinateur, lire la clé de nouveau la reprend. Un téléphone ou une tablette la relit de la même façon, tenue contre lui ou branchée. Un téléphone ou une tablette qui ne peut atteindre aucune clé ne le propose pas pour une clé lue sur un ordinateur, puisqu’il ne pourrait pas la reprendre : cessez de l’utiliser sur un ordinateur qui l’a lue, et elle quitte aussi le téléphone.

Chaque commande que Sioul montre, ici et ailleurs (pour démarrer le service des cartes à puce, installer un antivirus ou le lecteur des numérisations), est dans un champ que vous pouvez sélectionner, avec **Copier** à côté.

Si un autre programme garde la clé (GnuPG, le plus souvent), Sioul le dit et propose **Laisser GnuPG la libérer** ([Côté technique](#for-technical-readers)) ; après ses propres étapes GnuPG, Sioul libère la clé lui-même. Pour que GnuPG et Sioul partagent la clé durablement, ajoutez la ligne `pcsc-shared` à `scdaemon.conf` dans le dossier de GnuPG (`~/.gnupg`).

Quand une partie de la clé a expiré, Sioul dit laquelle et depuis quand (« Sa sous-clé de chiffrement (B81A…BF8C) a expiré le 8 avril 2025 : personne ne peut plus vous chiffrer avec elle. »), et propose **Renouveler pour deux ans**, qui renouvelle la clé principale et chaque sous-clé, celles déjà expirées aussi, puis dit la nouvelle date de chaque partie.

Sioul ne fait qu’utiliser la clé : il ne demande jamais son code PIN d’administration, ne change ni ne débloque jamais un code PIN, ne charge ni ne crée jamais de clé dessus. Ce qui a été essayé jusqu’ici : [Fonctionne avec](compatibility.md#logins-and-keys).

## Pour aller plus loin {#going-further}

- **Une clé Google à vous** : Google ne donne l’accès aux courriels de Gmail qu’aux applications qu’il a examinées. Tant que celle de Sioul ne l’est pas, **Se connecter avec Google** prend, pour les courriels, l’identifiant client et le secret d’une application Google que vous créez une fois dans Google Cloud (environ un quart d’heure ; **Faire votre clé Google** déplie les étapes). Une copie de Sioul construite sans clé Google à elle demande la vôtre pour les agendas aussi ; les copies publiées jusqu’ici sont construites sans. Une copie construite avec une clé montre **Utiliser une clé Google à moi**, à cocher d’abord. Cette « clé » est le nom et le secret d’une application, pas une clé de sécurité ([Premiers pas](first-steps.md#gmail-and-google-workspace)).
- **Un mot de passe d’application** pour Gmail marche aussi, sans la connexion de Google. Il demande la validation en deux étapes ; l’aide de Google dit que le réglage peut ne pas être disponible avec des clés de sécurité pour seule seconde étape, pour le compte d’un travail, d’un établissement scolaire ou d’une autre organisation (une adresse Google Workspace), ou avec la Protection Avancée. Changer le mot de passe de votre compte Google révoque tous les mots de passe d’application.
- **La clé du bouclier IA**, pour les adresses qui laissent l’IA les lire d’abord, est gardée dans le trousseau comme un mot de passe ; **Oublier la clé** la retire ([le Porche](porch.md#a-public-address-protected)).
- **Partager la clé de sécurité avec GnuPG** : ajoutez la ligne `pcsc-shared` à `~/.gnupg/scdaemon.conf`, et les deux se servent de la clé à tour de rôle.

## Pourquoi cela fonctionne ainsi {#why-it-works-this-way}

Se souvenir de mots de passe et recopier des codes est « une charge très lourde, voire impossible » pour certaines personnes ayant un handicap cognitif, et un gestionnaire de mots de passe est un remède nommé (WCAG 2.2, critère 3.3.8 ; W3C 2021). Chaque appareil demande donc un mot de passe une fois et le trousseau de votre système le garde, ou Bitwarden le donne. Votre propre clé est ce qui permet aux organismes de vous écrire chiffré, dans votre boîte plutôt que derrière leurs portails ([pourquoi](privacy-security.md#why-sioul-encrypts-mail)).

Plus de détails dans [ce que dit la recherche (en anglais)](https://sioul.aurelienpierre.com/dev/research.html), constat 37.

## Comparé à d’autres applications {#compared-with-other-apps}

**Vos mots de passe et vos clés, gardés par votre système et votre clé de sécurité.** Sioul garde les mots de passe des comptes dans le trousseau de votre système, là où Thunderbird garde sa propre réserve. Il laisse Bitwarden donner son mot de passe à un compte, ce qu’aucun autre logiciel de courriel ici ne décrit. Il se sert des clés OpenPGP directement depuis une clé de sécurité, sans GnuPG, là où Thunderbird a besoin de GnuPG et d’un réglage caché, et qualifie cela d’expérimental. D’autres font plus ailleurs : Sioul n’a pas S/MIME, a besoin d’une application Google à vous, ou d’un mot de passe d’application, pour les courriels de Gmail, et n’a pas encore essayé sa connexion Microsoft avec un vrai compte Microsoft ; sa clé de sécurité OpenPGP n’a été essayée avec une vraie clé que sous Linux, et sur Android qu’avec une carte de remplacement.

En octobre 2026, d’après la documentation de chaque application.

✓ documenté ; **en partie**, avec une note ; ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) ; ? pas confirmé ; — sans objet. La colonne de Sioul a été vérifiée dans son code.

=== "Au quotidien"

    | | Sioul | Thunderbird | Gmail | Outlook | Apple Mail | Evolution |
    |---|---|---|---|---|---|---|
    | Google connecté sur la page de Google elle-même | en partie¹ | ✓ | — | ✓ | ? | ✓ |
    | Les adresses Outlook.com et Microsoft 365 | en partie² | ✓ | ✓³ | ✓ | ? | ? |
    | Un gestionnaire de mots de passe donne son mot de passe à un compte | ✓⁴ | ✗ | ✗ | ✗ | ? | ✗ |
    | Un service d’un compte éteint, ses réglages gardés | ✓ | en partie⁵ | — | ? | ✓ | ✓ |

    1. La page de Google s’ouvre dans votre navigateur. Les courriels de Gmail demandent une application Google à vous (un projet Google Cloud gratuit, environ un quart d’heure, une fois) ou un mot de passe d’application ; les agendas demandent aussi une application Google à vous, dans une copie de Sioul construite sans clé à elle.
    2. La page de Microsoft s’ouvre dans votre navigateur. Pas encore essayé avec un vrai compte Microsoft. Dans une copie de Sioul construite sans sa propre application Microsoft, une application à vous, inscrite une fois (les étapes sont dans le formulaire). Une adresse professionnelle ou scolaire peut demander l’approbation de son organisation, une fois ([Fonctionne avec](compatibility.md#mail)).
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
    6. Autocrypt en partie, ce que Thunderbird appelle une « compatibilité limitée ».
    7. Par GnuPG, qui crée les clés ; Autocrypt et l’annuaire de clés Web Key Directory depuis 2022–2023.
    8. Sur un ordinateur, par le service des cartes à puce du système, essayé avec une YubiKey 5 le 8 octobre 2026 ; sur un téléphone ou une tablette, tenue contre lui (NFC) ou branchée (USB-C), testé avec une carte de remplacement seulement ([Fonctionne avec](compatibility.md#logins-and-keys)).
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
    - Aide de Gmail : Gmail dans d’autres logiciels de courriel, <https://support.google.com/mail/answer/6078445>, lu le 8 octobre 2026.
    - Aide de Gmail : ajouter d’autres comptes dans l’application Gmail, <https://support.google.com/mail/answer/21289>, lu le 8 octobre 2026.
    - Aide du compte Google : les mots de passe d’application, <https://support.google.com/accounts/answer/185833>, lu le 17 novembre 2026.
    - Aide des administrateurs Google Workspace : contrôler l’accès des applications moins sécurisées, <https://knowledge.workspace.google.com/admin/apps/control-access-to-less-secure-apps>, lu le 17 novembre 2026.
    - Aide du compte Google : les clés de sécurité pour la validation en deux étapes, <https://support.google.com/accounts/answer/6103523>, lu le 8 octobre 2026.
    - Aide du compte Google : les clés d’accès, <https://support.google.com/accounts/answer/13548313>, lu le 8 octobre 2026.
    - Administration de Google Workspace : S/MIME hébergé, <https://knowledge.workspace.google.com/admin/gmail/advanced/turn-on-hosted-s-mime-for-message-encryption>, lu le 8 octobre 2026.
    - Administration de Google Workspace : le chiffrement côté client, <https://knowledge.workspace.google.com/admin/security/about-client-side-encryption>, lu le 8 octobre 2026.
    - Administration de Google Workspace : le chiffrement par clé matérielle pour Gmail, <https://knowledge.workspace.google.com/admin/security/gmail-only-set-up-and-manage-hardware-key-encryption>, lu le 8 octobre 2026.
    - Outlook : ajouter un compte Gmail à Outlook pour Windows, <https://support.microsoft.com/en-us/outlook/getstarted/add-a-gmail-account-to-outlook-for-windows>, lu le 8 octobre 2026.
    - Outlook : les comptes synchronisés par le nuage de Microsoft, <https://support.microsoft.com/en-us/Outlook/getstarted/sync-your-account-in-outlook-to-the-microsoft-cloud>, lu le 8 octobre 2026.
    - Microsoft Purview : le chiffrement des courriels, <https://learn.microsoft.com/en-us/purview/email-encryption>, lu le 8 octobre 2026.
    - Assistance Microsoft : chiffrer des courriels, <https://support.microsoft.com/en-us/office/encrypt-email-messages-373339cb-bf1a-4509-b296-802a39d801dc>, lu le 8 octobre 2026.
    - Outlook sur le web : S/MIME, <https://support.microsoft.com/en-us/office/encrypt-messages-by-using-s-mime-in-outlook-on-the-web-878c79fc-7088-4b39-966f-14512658f480>, lu le 8 octobre 2026.
    - Compte Microsoft : Windows Hello et les clés de sécurité (redirigé aujourd’hui vers « Configurer Windows Hello »), <https://support.microsoft.com/en-us/account-billing/sign-in-to-your-microsoft-account-with-windows-hello-or-a-security-key-800a8c01-6b61-49f5-0660-c2159bea4d84>, lu le 8 octobre 2026.
    - Microsoft Learn : présentation du nouvel Outlook pour Windows, <https://learn.microsoft.com/en-us/microsoft-365-apps/outlook/overview-new-outlook-windows>, lu le 8 octobre 2026.
    - Mail pour Mac : ajouter et gérer des comptes, <https://support.apple.com/guide/mail/add-and-manage-email-accounts-mail35803/mac>, lu le 8 octobre 2026.
    - Guide de l’iPhone : les comptes de courriel, de contacts et de calendrier, <https://support.apple.com/guide/iphone/add-mail-contacts-and-calendar-accounts-ipha0d932e96/ios>, lu le 8 octobre 2026.
    - Trousseaux d’accès : ce que c’est, <https://support.apple.com/guide/keychain-access/what-is-keychain-access-kyca1083/mac>, lu le 8 octobre 2026.
    - Mail pour Mac : les certificats personnels, <https://support.apple.com/guide/mail/use-personal-certificates-mlhlp1179/mac>, lu le 8 octobre 2026.
    - Guide de l’iPhone : ajouter et supprimer des comptes de courriel, <https://support.apple.com/guide/iphone/add-and-remove-email-accounts-iph44d1ae58a/ios>, lu le 8 octobre 2026.
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

- **Les connexions** : les courriels en TLS dès le premier octet (préféré, RFC 8314) ou en STARTTLS, sans réglage non chiffré ; un STARTTLS qui échoue met fin à la connexion au lieu de continuer en clair. Les certificats sont vérifiés d’après ceux du système (la liste de Mozilla quand le système n’en a pas), avec rustls. CalDAV et CardDAV en HTTPS seulement.
- **Trouver le serveur** : `autoconfig.<domaine>`, puis `<domaine>/.well-known/autoconfig`, puis la liste des fournisseurs de Thunderbird d’après votre domaine ; puis, comme le fait Thunderbird, la même liste d’après les noms des serveurs qui reçoivent les courriels de votre domaine (ses enregistrements MX ; on dit à la liste le domaine, puis ces noms, jamais l’adresse) ; puis `imap.<domaine>` et `mail.<domaine>` ; puis les enregistrements SRV de votre domaine (RFC 6186), que rien ne signe. Ce que la liste donne d’après ces noms, une supposition et un enregistrement ne sont proposés que si leur serveur présente un certificat valable pour son propre nom (l’alias du serveur d’un hébergeur ne le présente pas), et sont dits comme à vérifier. HTTPS seulement, car un fichier de réglages venu en clair pourrait envoyer votre mot de passe ailleurs. Les domaines Google Workspace et Microsoft 365 sont reconnus par leurs enregistrements MX. 28 secondes au plus en tout ; une recherche que le réseau a ralentie le dit. Le serveur d’envoi se trouve au premier courriel envoyé : les réglages HTTPS du domaine, puis le serveur d’où viennent vos courriels (465, puis 587), puis l’entrée de l’hébergeur, les enregistrements (RFC 6186, RFC 8314) et `smtp.<domaine>`, chacun vérifié, et ce que nomme le DNS seulement chez le même fournisseur que le serveur de réception. Il est gardé une fois qu’il a pris un courriel, jamais avant.
- **Google** : OAuth 2.0 pour les applications natives (RFC 8252) : votre navigateur s’ouvre sur la page de Google, la réponse revient à une adresse en boucle locale sur un port tiré au hasard, et PKCE (S256, RFC 7636) avec un `state` tiré au hasard prouve que c’est le même programme. Les courriels (IMAP et SMTP, SASL XOAUTH2) ont leur propre autorisation, à part des agendas, des contacts et des tâches, pour que l’un puisse être retiré seul. Le jeton de renouvellement est gardé dans le trousseau, le jeton d’accès en mémoire seulement ; retirer un compte révoque son jeton chez Google.
- **Le trousseau** : le Secret Service sous Linux (KWallet, GNOME Keyring), le trousseau de macOS, le gestionnaire d’identifiants de Windows, le KeyStore d’Android sur un téléphone ou une tablette. Chaque entrée est rangée sous `sioul`, en « *identifiant* on *serveur* », facile à retrouver, vérifier ou retirer là.
- **OpenPGP** : RFC 9580, avec Sequoia-PGP et sa cryptographie en Rust pur. Une clé créée ici est Ed25519 pour signer et Curve25519 pour chiffrer, valable trois ans, ses parties secrètes chiffrées par une phrase secrète tirée au hasard et gardée dans le trousseau. Autocrypt niveau 1 : vos courriels portent votre clé ; une clé trouvée dans l’en-tête d’un expéditeur n’est gardée que si elle nomme cet expéditeur, parties publiques seulement. Les clés des autres ne sont cherchées que quand vous le demandez : dans le Web Key Directory de leur domaine, puis sur keys.openpgp.org, en HTTPS seulement. Pas de S/MIME.
- **La clé de sécurité** : la spécification de carte OpenPGP 3.4, par le service des cartes à puce du système, PC/SC (pcscd sous Linux, que la plupart des systèmes démarrent quand un programme le demande ; le service Carte à puce sous Windows ; CryptoTokenKit sous macOS). La clé est ouverte pour une seule opération, puis remise à zéro, pour que son code PIN ne reste pas donné à un autre programme ; les lecteurs sont joints en mode partagé. Le code PIN est gardé en mémoire, chiffré, pendant quinze minutes sans usage, jamais écrit. Une signature que fait la clé est vérifiée d’après sa partie publique avant que le courriel parte. Les journaux des bibliothèques sont bridés, pour qu’aucun code PIN ni aucune clé de session ne puisse atteindre un journal.
- **GnuPG** : son service de cartes à puce garde la clé pour lui après s’en être servi, sauf si `scdaemon.conf` dit `pcsc-shared`. **Laisser GnuPG la libérer** lance `gpgconf --kill scdaemon`, à ce clic seulement, et Sioul aussi à la fin de son propre renouvellement par GnuPG, pour nettoyer derrière lui ; gpg le redémarre quand il a besoin de la clé, et redemande alors son code PIN. Sioul ne change jamais les réglages de GnuPG.
- **Bitwarden depuis cette page** : le coffre s’ouvre comme depuis la page Sites. Sur un ordinateur, la fenêtre tient le profil web des sites et la page de Bitwarden elle-même qui demande une clé de sécurité, invisible, faits la première fois qu’il en faut un, pour les boîtes de dialogue des deux pages ; sur Android, qui n’a pas de moteur web pour cela, le mot de passe principal et une deuxième étape autre qu’une clé. Comment le coffre est lu : [Sites, côté technique](sites.md#for-technical-readers).
