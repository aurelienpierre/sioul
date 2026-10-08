---
description: "Vie privée et sécurité dans Sioul : ce qui est protégé par défaut, sans rien régler ; comment chaque message et chaque pièce jointe sont vérifiés ; comment ce qui voyage entre vos appareils est scellé ; les clés de sécurité et Bitwarden ; ce qui sort de votre appareil et quand ; ce que Sioul ne peut pas protéger ; et la comparaison avec d’autres applications."
---

# Vie privée et sécurité {#privacy-and-security}

## En bref {#in-short}

Le courrier administratif est la cible des escroqueries, et vous ne devriez pas avoir à rester sur vos gardes chaque fois que vous l’ouvrez. Sioul vous protège donc par défaut, sans vous demander de rien régler. Il tourne sur vos appareils, garde vos données dans de simples fichiers de vos propres dossiers, et n’a aucun serveur à lui : rien ne parvient au développeur. Chaque message est vérifié avant que vous le voyiez, rien ne se charge depuis le réseau pendant que vous lisez, et une pièce jointe passe par votre antivirus avant de s’ouvrir. Ce qui voyage entre vos appareils est scellé avec une clé que vous seul détenez, les mots de passe restent dans le trousseau de votre système, et une clé de sécurité peut garder votre courrier chiffré, les sites que vous gardez et votre coffre Bitwarden.

Le texte formel, entre autres pour la connexion avec Google : [Politique de confidentialité](../privacy.md).

## Ce qui est protégé par défaut {#what-is-protected}

### Vos données restent à vous {#what-stays-on-your-computer}

Sioul tourne sur votre appareil. Il n’y a pas de serveur Sioul ni de compte chez nous ; Sioul n’envoie ni statistiques, ni rapports de plantage, ni vérification de mise à jour, et ne contacte jamais le développeur. Votre courrier, vos agendas et vos contacts ne voyagent qu’entre votre appareil et vos propres fournisseurs.

Tout ce que Sioul garde est dans des fichiers simples, dans vos propres dossiers, lisibles par d’autres programmes, et à vous de les copier, de les sauvegarder ou de les effacer :

| Quoi | Où, sous Linux |
|---|---|
| Les réglages, les expéditeurs que vous laissez entrer, sûrs, neutres, restreints ou bloqués | `~/.config/sioul/` |
| Le courrier (un Maildir par adresse), les agendas et les contacts (un fichier par élément), les brouillons, le temps passé, les factures, les médicaments, les SMS de votre téléphone et leurs médias (tous, scellés) et ceux écrits pour qu’il les envoie | `~/.local/share/sioul/` |
| Où chaque relève s’est arrêtée, où le Porche a été fermé, les prises que vous avez notées et le relevé de chaque prise, les nouvelles des sites, le journal des appels d’un agent d’IA, ce que l’IA a dit du courrier d’une adresse protégée, les appels filtrés par vos téléphones (un mois d’entre eux), les messages qu’ont apportés les notifications de votre téléphone pour les applications que vous envoyez à vos ordinateurs (une semaine d’entre eux) | `~/.local/state/sioul/` |
| Votre propre filtre à indésirables : ce que son apprentissage a lu de votre courrier, le modèle de langue qu’il a appris, sa table ; ce que vous avez dit indésirable ou non | `~/.local/share/sioul/spam/`, `~/.local/state/sioul/spam/` |
| Notes, projets, budgets, papiers, lettres scannées | votre dossier de notes, là où vous l’avez choisi |

Sous Windows, les dossiers de Sioul sont dans `%APPDATA%\Sioul` ; sous macOS, dans `~/Library/Application Support/Sioul`. Sous Linux et macOS, Sioul rend ses propres dossiers accessibles à vous seul chaque fois qu’il démarre : les autres comptes du même ordinateur ne peuvent pas les ouvrir. Votre dossier de notes garde les droits que vous lui avez donnés.

**Les mots de passe**, l’accès à Google, les clés et les jetons des services que vous activez, et les phrases de passe vont dans le trousseau de votre système : GNOME Keyring ou KWallet sous Linux, le Gestionnaire d’identification sous Windows, le Trousseau d’accès sous macOS ; sur un téléphone, chiffrés avec une clé que garde le KeyStore d’Android. Jamais dans un fichier en clair.

Sioul ne chiffre pas lui-même les fichiers de votre appareil : votre système le fait mieux, pour tous les programmes à la fois. Si son chiffrement du disque n’est pas encore activé, activez-le : BitLocker ou le chiffrement de l’appareil sous Windows, FileVault sous macOS, le chiffrement que propose l’installateur de votre Linux. Les téléphones Android chiffrent déjà leur stockage.

### Chaque connexion chiffrée {#every-connection-encrypted}

Votre courrier, vos agendas, vos contacts, votre coffre Bitwarden et les recherches de clés de chiffrement voyagent toujours chiffrés, et le certificat de chaque serveur est vérifié avec ceux de votre système. Aucun réglage ne permet de les envoyer en clair. La connexion à Google se fait sur la page de Google, dans votre navigateur habituel, avec ce que Google y demande (votre mot de passe, votre clé de sécurité, une clé d’accès) : Sioul ne voit jamais votre mot de passe Google, et reçoit la réponse de Google sur votre seul appareil.

### Chaque message vérifié {#every-message-checked}

Quand un message arrive, Sioul le vérifie lui-même, par le DNS de votre système :

- **SPF**, sur le serveur qui a remis le message à votre fournisseur ;
- **DKIM**, chaque signature contre la clé que publie son domaine, vérifiée à l’arrivée, car les clés changent ;
- **DMARC**, avec la politique du domaine lui-même ;
- **ARC**, pour le courrier transféré, et le **DNS inverse** du serveur d’envoi.

Les résultats sont gardés avec le message, sous un nom que seule votre copie de Sioul utilise, pour qu’aucun expéditeur ne puisse écrire des résultats qui passeraient pour ceux de Sioul. Un message est alors **vérifié**, **non vérifié** (avec la raison), ou **falsifié**. Le courrier falsifié est mis de côté, avec la raison ; rien n’est supprimé. Les verdicts de votre fournisseur sur les indésirables comptent aussi, pour le courrier d’inconnus seulement ([plus bas](#your-own-spam-filter)).

**Une adresse copiée ne suffit pas.** Un message que rien n’authentifie (il échoue à SPF et à DKIM, et rien ne s’en porte garant) est traité comme celui d’un inconnu, quelle que soit l’adresse qu’il affiche. Quelqu’un qui écrit depuis l’adresse de votre banque, ou celle d’un ami, n’obtient ni leur place sur le Porche, ni leurs heures, ni leur protection contre le filtre à indésirables.

**Les noms empruntés** sont repérés : un expéditeur qui prend le nom d’une banque, d’un service public ou de votre propre domaine, depuis une adresse qui ne leur appartient pas, est mis de côté. Les lettres qui se ressemblent (un l minuscule pour un I majuscule, un 0 pour un O, des lettres cyrilliques) ne cachent pas le nom. Les *domaines* qui en imitent d’autres, comme le nom d’une banque avec une lettre changée, ne sont pas encore repérés.

**Les codes que vous demandez** arrivent tout de suite, à toute heure. Si leur expéditeur n’est pas vérifié, Sioul dit de ne s’en servir que si vous venez d’en demander un à ce site, puisque les faux codes sont une ruse d’hameçonnage ; un code falsifié est mis de côté.

### Lire sans risque {#reading-safely}

**Le courrier HTML** est montré avec ses paragraphes, ses listes, son texte en gras et ses liens seulement : pas d’images, pas de styles, pas de scripts, pas de formulaires. Rien ne se charge depuis le réseau, jamais, et aucun réglage ne le permet : aucune image de pistage n’apprend que vous avez ouvert un message. Sioul n’envoie jamais d’accusé de lecture non plus.

**Les liens** montrent leur adresse complète avant de s’ouvrir : sous le pointeur, ou après un premier toucher sur un écran tactile. Seuls les liens web et de courrier s’ouvrent. Un lien de courrier commence un message vers sa seule adresse, sans l’objet ni le texte qu’il remplirait pour vous.

### Les pièces jointes et l’antivirus {#attachments-and-the-antivirus}

Une pièce jointe est vérifiée avant de s’ouvrir ou d’être enregistrée, par l’antivirus de votre système :

- **Sous Linux et macOS** : ClamAV, quand il est installé ([Installer](install.md#the-packages)).
- **Sous Windows** : Microsoft Defender, par l’Antimalware Scan Interface.

Quand une menace est trouvée, rien ne s’ouvre, et la copie est effacée. Un programme, un script, un raccourci ou un installateur n’est jamais lancé depuis un courriel, même vérifié : enregistrez-le si vous lui faites confiance. Sous Windows et macOS, un fichier que vous ouvrez ou enregistrez depuis un courriel porte la marque du système qui dit qu’il vient d’Internet, pour qu’Office l’ouvre en mode protégé et que macOS le vérifie avant qu’il se lance. Les pièces jointes du courrier mis de côté ne s’ouvrent pas du tout.

Sans antivirus, Sioul ne refuse pas : il dit que le fichier ne sera pas vérifié, demande avant de l’ouvrir (**L’ouvrir sans vérification**), et dit comment en avoir un. Un téléphone n’a pas d’antivirus à qui Sioul puisse demander : les pièces jointes n’y sont pas vérifiées, et Sioul le dit plutôt que de demander chaque fois. L’application que vous choisissez lit ce seul fichier et rien d’autre de Sioul, et les installateurs d’Android ne s’ouvrent jamais.

### Votre filtre à indésirables {#your-own-spam-filter}

Le propre filtre à indésirables de Sioul apprend sur votre ordinateur, quand vous appuyez sur **Entraîner maintenant** et de lui-même une fois par semaine quand cet ordinateur est branché et inactif, de votre propre courrier ([Courrier, Votre filtre à indésirables](mail.md#your-own-spam-filter)) :

- **Ce que l’apprentissage lit** : chaque dossier de chaque adresse, sauf la corbeille, les brouillons et les envoyés (dans « Tous les messages » de Gmail, seulement le courrier archivé qu’aucun autre dossier ne contient, jamais le vôtre), sans rien changer sur le serveur. De chaque message, il garde les en-têtes, le nom et le type de ses parties et le début de son texte, jamais les pièces jointes elles-mêmes ; pour vérifier son expéditeur comme Sioul vérifie le courrier qu’il garde, il lit chaque message de 2 Mo au plus en entier, en mémoire seulement, n’en garde que les résultats, et interroge le DNS de votre système sur le domaine de l’expéditeur. Ce qu’il garde reste sur cet ordinateur (`~/.local/share/sioul/spam/corpus/`), pour que les indésirables restent connus après que votre fournisseur a vidé son dossier des indésirables.
- **Ce qu’il apprend** : un modèle de langue des mots de votre courrier, gardé à côté (`language.bin`). Il contient les mots de votre courrier en clair : il ne quitte jamais cet ordinateur.
- **Ce qui voyage** : sa table, le résultat, vers vos autres appareils par votre dossier, scellée comme tout ce qui s’y trouve : des nombres, les mots en empreintes, jamais en clair ; et ce que vous avez dit, plus bas. Un téléphone n’apprend jamais : il lit cette table.
- **Ce que vous avez dit** : **Indésirable**, **Pas indésirable** et bloquer un expéditeur depuis un message ajoutent une ligne au relevé de cet appareil (`~/.local/state/sioul/spam/labels/`), chaque message que le filtre déplace dans un dossier Indésirables une autre (`~/.local/state/sioul/spam/moved/`), et chaque message qu’il signale là où il est une autre encore (`~/.local/state/sioul/spam/flagged/`) : quand, où, quel message par son numéro et son Message-ID, indésirable ou non ; jamais un mot de lui. Chaque appareil garde le sien ; ils voyagent scellés vers vos autres appareils, pour qu’un message dont vous avez dit qu’il n’était pas indésirable sur l’un ne soit plus jamais signalé sur un autre.
- **L’apport extérieur**, si vous en importez (`sioul spam import`) : les objets et les textes de courrier étiqueté ailleurs, gardés sur cet ordinateur seulement, à part de votre courrier (`~/.local/share/sioul/spam/external/`), à vous seul, jamais partagés ; `sioul spam import --remove` les retire.
- **Ce qu’il demande à votre ordinateur**, pour se réentraîner de lui-même : s’il est branché, en économie d’énergie, inactif ou verrouillé, et si votre connexion est limitée, aux services du système ; demandé sur cet ordinateur, gardé nulle part, envoyé nulle part.
- **Rien à personne d’autre** : aucun serveur, aucune IA de Sioul. Ses chiffres ne sont que des totaux ; les mots qui ont pesé dans un message sont pour vous, dans votre propre terminal (`sioul spam why`), jamais proposés à un agent d’IA. Un agent d’IA que vous connectez vous-même peut se servir des outils du filtre, qui listent ses pires erreurs par expéditeur et par objet, les codes et les numéros de compte masqués, sauf si vous gardez ces outils loin de lui ([Avec un agent d’IA](ai-agent.md#the-spam-filters-tools)).

Pour tout oublier, supprimez `~/.local/share/sioul/spam/` sur cet ordinateur : sa table, partagée, quitte alors aussi vos autres appareils.

### Les sites, Bitwarden et les clés de sécurité {#sites}

Une clé de sécurité ne se laisse pas tromper par une fausse page : elle ne répond qu’au site pour lequel elle a été faite. C’est pourquoi Sioul vous permet d’en utiliser une partout où vous vous connectez.

Les sites que vous épinglez vivent dans un profil de navigateur propre à Sioul, à part de votre navigateur habituel. Leurs notifications sont gardées par Sioul. Le micro, la caméra et le partage de l’écran sont permis site par site, par vous ; l’endroit où vous êtes n’est jamais donné, ni la liste des polices de votre ordinateur, dont les sites se servent pour distinguer les ordinateurs. Les téléchargements vont dans votre dossier de téléchargements.

Un site qui demande une clé de sécurité l’obtient (FIDO2 et WebAuthn : une YubiKey, une Nitrokey), de même que les clés d’accès qu’elle garde ; le code PIN de la clé est demandé dans la boîte de dialogue propre à Sioul. Les clés d’accès gardées dans un téléphone ou dans le système ne fonctionnent pas dans les sites de Sioul sous Linux et macOS.

Les identifiants viennent de votre coffre Bitwarden, ouvert par Sioul lui-même, en lecture seule : rien n’y est jamais écrit, votre mot de passe principal et vos clés ne sont jamais gardés, et les identifiants restent en mémoire jusqu’à la fermeture de Sioul. Depuis la page Sites, le coffre peut s’ouvrir avec votre clé de sécurité, comme deuxième étape, ou seule quand Bitwarden la connaît comme clé d’accès. Un identifiant fait pour un autre domaine le dit, pour qu’un site qui en imite un autre se remarque. Voir [Les sites](sites.md).

Sur un téléphone, les sites s’ouvrent dans votre navigateur, avec ses propres protections, et Sioul n’y garde aucun identifiant.

### Le courrier chiffré {#encrypted-mail}

Avec OpenPGP, Sioul signe et chiffre vos messages quand vous les envoyez, déchiffre et vérifie ceux que vous recevez, et donne votre clé publique aux personnes à qui vous écrivez (Autocrypt). Il ne garde la clé d’un expéditeur que depuis un message qu’il a vérifié, pour qu’un message falsifié ne puisse pas glisser une clé pour quelqu’un à qui vous écrivez. Il ne cherche la clé de quelqu’un sur le réseau que quand vous le demandez (**Chercher leurs clés**), puisque cela dit à un serveur à qui vous écrivez.

Vos clés secrètes restent sur l’appareil où elles ont été créées. Une clé gardée sur une [clé de sécurité](accounts.md#security-key) ne la quitte jamais : la clé signe et ouvre elle-même, après son code PIN, que Sioul garde en mémoire quinze minutes sans usage et n’écrit ni ne journalise jamais nulle part. Sioul se contente d’utiliser la clé : il ne demande jamais son code PIN d’administration et ne change jamais ce qu’elle contient. Voir [Le courrier](mail.md#signing-and-encrypting).

L’objet d’un message chiffré reste lisible, comme dans la plupart des logiciels de courrier aujourd’hui.

### Entre vos appareils {#between-your-devices}

Chaque appareil garde ses propres données. Ce qui doit voyager entre vos ordinateurs et votre téléphone passe par un dossier que votre propre application de synchronisation transporte (Nextcloud, Dropbox, Syncthing, Google Drive, OneDrive…), scellé sur votre appareil avant d’y être écrit, avec une clé tirée de votre phrase de passe. Il n’y a aucun serveur à nous entre les deux. Le serveur du dossier voit quel appareil a écrit, quand et combien, et la taille de chaque note ou papier scellé ; jamais quoi. Rien de ce qui arrive dans ce dossier, un fichier abîmé ou effacé, ne peut retirer des données à vos appareils. Les mots d’une notification ne voyagent que si vous activez **Messages de votre téléphone**, pour les applications que vous choisissez, une semaine au plus ([Sur vos ordinateurs](notifications.md#messages-on-your-computers)). Comment cela marche, ce que cela protège et ce que cela ne peut pas cacher : [Partager entre vos appareils](sharing.md).

### L’IA, seulement si vous la demandez {#ai-only-if-you-ask}

Sioul ne connecte aucune IA de lui-même.

Un agent d’IA que vous connectez, comme Claude Code, peut lire ce que vous lui ouvrez sur cet appareil et préparer des tâches, des événements, des notes et des brouillons : chaque projet est fermé aux agents jusqu’à ce que vous l’ouvriez, et ce qui n’est dans aucun projet peut être fermé aussi ([Choisir ce qu’un agent voit](ai-agent.md#choosing-what-an-agent-sees)). Il ne peut ni envoyer, ni supprimer, ni déplacer d’argent, ni lire un mot de passe, et les codes à usage unique, les numéros de carte et de compte bancaire lui sont cachés ; l’entreprise derrière lui reçoit ce qu’il lit. Il peut aussi se servir des outils de votre propre filtre à indésirables, qui lisent vos serveurs de courrier sans rien y changer ; vous pouvez garder ces outils loin des agents. Voir [Avec un agent d’IA](ai-agent.md).

Pour une adresse que vous protégez contre le harcèlement, **Laisser l’IA le lire d’abord** reste désactivé tant que vous ne l’activez pas. Sioul envoie alors à Anthropic, avec votre propre clé, chaque message de la boîte de réception de cette adresse, de n’importe qui, y compris ceux qui s’y trouvent déjà : son objet et les 4 000 premiers caractères de son texte, **sans rien masquer**. Contrairement à ce que reçoit un agent, rien n’y est caché : un code ou un numéro de compte dans un tel message parvient aussi à Anthropic. Voir [le Porche](porch.md#a-public-address-protected).

### Une adresse publique {#a-public-address}

Une adresse que vous publiez peut être protégée contre le harcèlement : son courrier est lu sur votre appareil avant que vous le voyiez, et les messages hostiles sont mis de côté sans que leurs mots soient montrés. Voir [le Porche](porch.md#a-public-address-protected).

### Votre serveur de courrier {#your-mail-server}

Relever ne change rien sur le serveur : aucun message n’est marqué lu parce qu’il a été relevé. Sioul n’y écrit que quand vous agissez : ouvrir un message le marque lu, comme le fait tout programme de courrier ; archiver, supprimer, mettre en indésirable et déplacer se font dix secondes après votre demande, pour qu’**Annuler** puisse les arrêter. Le courrier ne part que quand vous l’envoyez, ou quand vous demandez à une liste de diffusion de vous laisser partir. Retirer un compte retire son mot de passe, pas son courrier.

## Ce qui sort de votre appareil, et quand {#what-leaves-your-computer-and-when}

| Vers | Quoi | Quand |
|---|---|---|
| Votre fournisseur de courrier | votre courrier : relevé, rangé comme vous le demandez, envoyé quand vous envoyez | toujours : c’est votre courrier |
| Votre serveur d’agendas et de contacts, ou Google | vos agendas, tâches et contacts, dans les deux sens | toujours |
| Les sites que vous épinglez | ce que tout navigateur leur envoie | quand ils sont ouverts |
| Les réglages publiés par votre fournisseur, et la liste de fournisseurs de Thunderbird | le domaine de votre adresse, pour trouver le serveur | quand vous ajoutez un compte |
| Le résolveur DNS de votre système | des requêtes sur les domaines des expéditeurs, pour les vérifier | quand du courrier arrive, et quand l’apprentissage de votre filtre à indésirables télécharge votre courrier |
| L’adresse de désabonnement d’une liste de diffusion | une demande « en un clic », sans rien de vous que ce que la liste y a écrit ; ou un court message à cette adresse, depuis la vôtre ; ou sa page, dans votre navigateur | seulement quand vous choisissez **Se désabonner** |
| Open-Meteo | les coordonnées du lieu choisi, arrondies à deux décimales, ou le nom d’une ville que vous cherchez | seulement si vous choisissez un lieu pour la météo ; toutes les demi-heures au plus |
| Le géocodeur d’OpenStreetMap (Nominatim) | les adresses postales de vos contacts, une fois chacune, rien d’autre | seulement après que vous avez choisi **Les placer** |
| Les images de carte d’OpenStreetMap, ou celles que vous avez choisies | quelle partie de la carte est montrée | tant que la carte est ouverte |
| Le Web Key Directory du domaine des destinataires, puis keys.openpgp.org | les adresses auxquelles vous écrivez | seulement quand vous choisissez **Chercher leurs clés** |
| L’adresse écrite sur votre clé de sécurité, l’annuaire de clés de votre domaine, puis keys.openpgp.org | l’empreinte de votre clé de sécurité, et le domaine de vos adresses | seulement quand vous choisissez **La chercher** ou **Chercher une version plus récente** |
| Chaque site que vous épinglez | une demande de sa propre icône | environ une fois par semaine |
| Anthropic | l’objet et les 4 000 premiers caractères du texte de chaque message d’une adresse que vous protégez, sans rien masquer | seulement avec **Laisser l’IA le lire d’abord**, et votre propre clé |
| L’agent d’IA que vous connectez, et son fournisseur | ce qu’il lit | seulement si vous en connectez un ([Avec un agent d’IA](ai-agent.md)) |
| GitHub | votre jeton ; des recherches de vos tickets et pull requests | seulement si vous activez GitHub |
| Votre serveur Bitwarden, et sa propre page de clé de sécurité quand votre clé est votre deuxième étape | votre connexion, pour ouvrir votre coffre | quand vous remplissez un identifiant depuis lui |
| Votre Nextcloud (celui de Murena compris) | les fichiers scellés du dossier de partage | seulement si Sioul tient ce dossier à jour lui-même, ou va l’y chercher en secours |
| Les serveurs de mise à jour de ClamAV | une demande de signatures de virus | une fois par jour, seulement quand votre système n’en garde aucune lui-même |

Sioul n’a ni mesure d’audience, ni publicité, ni rapports de plantage, ni vérification de mise à jour.

## Ce que Sioul ne peut pas protéger {#what-sioul-cannot-protect}

Dire où la protection s’arrête en fait partie :

- **Votre boîte aux lettres reste chez votre fournisseur.** Sioul est un logiciel de courrier : votre courrier, vos agendas et vos contacts restent sur les serveurs de vos fournisseurs, et ce que ces serveurs peuvent lire dépend d’eux. Seul le courrier que vous chiffrez avec OpenPGP leur est illisible, son objet mis à part. Choisissez un fournisseur en qui vous avez confiance.
- **Votre appareil est protégé par votre système.** Les fichiers de Sioul sont de simples fichiers, pour que vous et d’autres programmes puissiez les lire ; le chiffrement du disque de votre système et le mot de passe de votre session gardent les autres dehors.
- **Le dossier entre vos appareils en dit un peu.** Son serveur voit quel appareil a écrit, quand et combien ([le détail](sharing.md#what-it-protects-and-what-it-cannot-hide)).
- **Un téléphone a moins de protections.** Sur Android, il n’y a ni antivirus pour vérifier les pièces jointes ni clé de sécurité, et les sites s’ouvrent dans votre navigateur.
- **Les domaines qui en imitent d’autres ne sont pas encore repérés**, seulement les noms empruntés, et la liste des marques est fixe : surtout des services français et de grandes plateformes.
- **Un agent d’IA que vous connectez lit ce que vous lui ouvrez** sur cet appareil (les projets que vous ouvrez, et ce qui n’est dans aucun projet sauf si vous le fermez), jamais les mots de passe, les codes, les numéros de compte, les médicaments et les prises, et l’entreprise derrière lui reçoit ce qu’il lit.
- **La lecture par l’IA d’une adresse protégée envoie le début de chaque message tel quel** : son objet et ses 4 000 premiers caractères, codes et numéros de compte compris.
- **Certaines protections n’ont pas encore été essayées sur du vrai matériel** : l’antivirus sous Windows, les clés de sécurité avec une vraie clé, dans les sites et pour votre clé OpenPGP ([Côté technique](#for-technical-readers)).
- **Les paquets pour Windows et macOS ne sont pas encore signés** : votre système avertit la première fois ([Installer](install.md)).

## Ce site web {#this-website}

Ce site web n’a ni mesure d’audience, ni publicité, ni cookies, et ne charge rien depuis d’autres sites. Il est hébergé par GitHub Pages, qui garde ses propres journaux de serveur, selon [la déclaration de confidentialité de GitHub (en anglais)](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

## Signaler un problème {#reporting-a-problem}

Un problème de sécurité, ou tout ce qui vous inquiète : [les tickets GitHub](https://github.com/aurelienpierre/sioul/issues).

## Comparé à d’autres applications {#compared-with-other-apps}

En octobre 2026, d’après la documentation de chaque application.

✓ documenté · en partie, avec une note · ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) · ? pas confirmé · — sans objet.

Chaque colonne est tout un ensemble d’outils : Thunderbird avec Firefox et son gestionnaire de mots de passe ; Google, Gmail et Workspace avec Chrome et le Gestionnaire de mots de passe de Google ; Microsoft, Outlook et Microsoft 365 avec Edge et Defender pour Office 365 ; Proton, ses Mail, Calendar, Drive et Pass avec Bridge ; Nextcloud, Nextcloud Hub avec Mail et Calendar. Les logiciels de courrier seuls sont comparés sur [la page Courrier](mail.md#compared-with-other-apps).

=== "Au quotidien"

    | | Sioul | Thunderbird | Google | Microsoft | Proton | Nextcloud |
    |---|---|---|---|---|---|---|
    | Aucune télémétrie par défaut | ✓ | ✗¹ | ✗² | ✗³ | ✗⁴ | en partie⁵ |
    | L’authentification de chaque expéditeur vérifiée et affichée | ✓⁶ | ✗⁷ | en partie⁸ | en partie⁹ | en partie¹⁰ | ✗ |
    | Noms empruntés et domaines qui en imitent d’autres repérés | en partie¹¹ | ✗¹² | en partie¹³ | en partie¹⁴ | en partie¹⁵ | en partie¹⁶ |
    | Contenu distant bloqué par défaut | ✓¹⁷ | ✓ | en partie¹⁸ | en partie¹⁹ | en partie²⁰ | ✓ |
    | Pièces jointes analysées contre les logiciels malveillants | ✓²¹ | en partie²² | ✓²³ | ✓²⁴ | en partie²⁵ | en partie²⁶ |
    | Ce qui se synchronise entre vos appareils est chiffré de bout en bout | ✓²⁷ | en partie²⁸ | en partie²⁹ | en partie³⁰ | ✓³¹ | en partie³² |
    | Où vivent vos données | vos appareils³³ | votre ordinateur³⁴ | Google | Microsoft³⁵ | Proton³⁶ | votre serveur³⁷ |
    | Code ouvert | ✓ | ✓ | ✗³⁸ | ✗ | en partie³⁹ | ✓ |

    1. Thunderbird et Firefox envoient des données techniques et d’usage tant que vous ne le désactivez pas.
    2. Google recueille l’activité, des informations sur l’appareil et les rapports de plantage, comme le dit sa politique de confidentialité.
    3. Microsoft 365 envoie des données de diagnostic facultatives tant qu’un administrateur ne le change pas, et des données de service obligatoires toujours.
    4. Proton Mail, Drive, Calendar et Bridge partagent des diagnostics d’usage tant que vous ne les désactivez pas.
    5. Un serveur Nextcloud vérifie les mises à jour par défaut, en envoyant sa version ; l’enquête d’usage est une application à part.
    6. Sioul vérifie lui-même SPF, DKIM, DMARC, ARC et le DNS inverse, à l’arrivée de chaque message, et dit sur chaque message s’il est vérifié, non vérifié ou falsifié.
    7. Thunderbird : seulement par une extension.
    8. Gmail : un point d’interrogation sur le courrier qui n’est pas authentifié ; « Envoyé par » et « Signé par » dans les détails d’un message, sur un ordinateur et sur Android.
    9. Outlook : un « ? » quand il ne peut pas vérifier l’expéditeur, et « via » quand l’expéditeur réel est un autre.
    10. Proton Mail : un avertissement sur le courrier qui échoue aux vérifications de son domaine ; rien n’est dit quand il les passe.
    11. Sioul : les noms d’une cinquantaine de marques et de services publics, et vos propres domaines, même écrits avec des lettres qui se ressemblent ; les domaines qui en imitent d’autres ne sont pas encore repérés.
    12. Thunderbird signale les liens trompeurs, pas les noms empruntés.
    13. Dans Workspace, réglé par l’administrateur : les domaines qui ressemblent à celui de l’entreprise, et les noms de ses membres ; dans Gmail, une adresse qui imite celle d’un expéditeur connu est l’une des raisons d’un indésirable.
    14. Dans Defender pour Office 365, pour les personnes et les domaines qu’un administrateur liste.
    15. Proton Mail : PhishGuard signale les adresses peut-être usurpées ; la confirmation des liens avertit des lettres qui se ressemblent dans les liens.
    16. Nextcloud Mail : un expéditeur dont l’adresse diffère de celle de votre carnet d’adresses, un Répondre à qui diffère, un lien qui mène ailleurs que son texte.
    17. Jamais chargé : aucun réglage ne le charge.
    18. Gmail : affiché tout de suite, par les serveurs de Google, qui cachent votre appareil et l’endroit où vous êtes ; un réglage le fait demander d’abord.
    19. Outlook classique bloque les images par défaut ; Outlook.com les charge par le relais de Microsoft.
    20. Proton Mail : les traqueurs retirés et les images chargées par le relais de Proton, affichées tout de suite.
    21. Par l’antivirus de votre système (ClamAV, Microsoft Defender) avant qu’une pièce jointe s’ouvre ; les programmes ne s’ouvrent jamais. Pas sur les téléphones.
    22. Thunderbird laisse l’antivirus de votre système mettre les nouveaux messages en quarantaine ; il n’a pas d’analyseur à lui.
    23. Les fichiers exécutables bloqués et les pièces jointes analysées ; un bac à sable à partir de Business Standard.
    24. Activé par défaut pour chaque boîte ; l’exécution dans un bac à sable avec Defender pour Office 365.
    25. Proton Mail : sur ses serveurs, pour le courrier qui n’est pas chiffré de bout en bout ; Proton Drive compare les fichiers à une liste de fichiers malveillants connus.
    26. Les fichiers déposés sur Nextcloud, avec son application antivirus ; pas les pièces jointes du courrier.
    27. Ce que Sioul transporte entre vos appareils, sans serveur de Sioul ; votre courrier, vos agendas et vos contacts passent par vos propres fournisseurs, comme avec n’importe quel logiciel de courrier.
    28. Firefox Sync est de bout en bout ; Thunderbird n’a pas de synchronisation à lui.
    29. Ni Gmail ni Drive ; le chiffrement côté client dans Enterprise Plus, Education Standard et Plus, Frontline Plus ; les mots de passe une fois le chiffrement sur l’appareil mis en place.
    30. Microsoft détient les clés, sauf avec le Double Key Encryption (Microsoft 365 E5) ; Edge chiffre les mots de passe avant de les synchroniser.
    31. Mail, Calendar, Drive et Pass ; les objets et les adresses ne sont pas de bout en bout.
    32. Des dossiers vides choisis, depuis les applications de bureau et mobiles ; ni les agendas, ni les contacts, ni le courrier.
    33. Dans de simples fichiers ; votre courrier aussi chez votre fournisseur ; le dossier de partage scellé, dans le cloud que vous choisissez.
    34. Le profil de Thunderbird ; votre courrier chez votre fournisseur ; les données de Firefox Sync chez Mozilla, chiffrées.
    35. La frontière des données de l’UE pour les organisations inscrites dans l’UE ou l’AELE.
    36. Proton AG, en Suisse.
    37. Ou celui de votre hébergeur ; votre courrier chez votre fournisseur de courrier.
    38. Chromium, sur lequel Chrome est construit, est ouvert.
    39. Ses applications, auditées par des tiers ; pas ses serveurs.

=== "Technique"

    | | Sioul | Thunderbird | Google | Microsoft | Proton | Nextcloud |
    |---|---|---|---|---|---|---|
    | Votre clé de courrier sur une clé de sécurité ou une carte à puce | en partie¹ | en partie² | en partie³ | ?⁴ | ✗⁵ | ✗ |
    | OpenPGP intégré | ✓⁶ | ✓⁷ | ✗ | ✗ | ✓ | en partie⁸ |
    | Clés de sécurité et clés d’accès pour se connecter | en partie⁹ | ✓¹⁰ | ✓¹¹ | ✓ | ✓¹² | ✓¹³ |
    | Gestionnaire de mots de passe | ✓¹⁴ | ✓ | ✓ | ✓ | ✓¹⁵ | en partie¹⁶ |
    | S/MIME | ✗ | ✓ | en partie¹⁷ | ✓ | ✗ | ✓ |
    | Courrier stocké illisible pour le fournisseur | en partie¹⁸ | en partie¹⁸ | en partie¹⁹ | en partie²⁰ | ✓²¹ | en partie¹⁸ |

    1. Sioul : une carte OpenPGP (YubiKey, Nitrokey) signe et ouvre, son code PIN gardé en mémoire seulement, jamais écrit ni journalisé ; sur les ordinateurs ; essayée jusqu’ici avec une carte logicielle, pas encore avec une vraie clé.
    2. Thunderbird : par GnuPG, que vous installez et réglez vous-même.
    3. Google : une carte à puce PIV garde la clé S/MIME, avec le chiffrement côté client, sous Windows, dans les éditions les plus hautes de Workspace avec Assured Controls.
    4. Microsoft : une carte à puce pour le S/MIME d’Outlook n’est pas confirmée dans les pages lues.
    5. Proton : sa documentation sur les clés ne mentionne aucune carte à puce ni clé de sécurité.
    6. Sioul : avec Autocrypt (clés prises seulement dans le courrier vérifié) et le Web Key Directory.
    7. Thunderbird : clés trouvées par WKD et keys.openpgp.org ; Autocrypt en partie.
    8. Nextcloud : par l’extension de navigateur Mailvelope.
    9. Sioul : les clés de sécurité, et les clés d’accès qu’elles gardent, dans les sites que vous gardez et pour ouvrir votre coffre Bitwarden depuis la page Sites ; sur les ordinateurs ; fondé sur le WebAuthn de Chromium et pas encore essayé avec une vraie clé ; les clés d’accès gardées dans un téléphone ou dans le système ne fonctionnent pas sous Linux et macOS.
    10. Firefox : les clés USB avec leur code PIN, et les clés d’accès ; le compte Mozilla lui-même accepte les codes d’une application d’authentification.
    11. Google : clés d’accès, clés de sécurité, et la Protection avancée.
    12. Proton : les clés de sécurité comme deuxième étape du compte ; les clés d’accès gardées dans Proton Pass.
    13. Nextcloud : la connexion WebAuthn, et les clés FIDO2 comme deuxième étape.
    14. Sioul : lit votre coffre Bitwarden, sans jamais y écrire, et remplit les identifiants et leurs codes.
    15. Proton Pass, avec les clés d’accès.
    16. Nextcloud : une application de la communauté dans son magasin d’applications.
    17. Google : dans Workspace ; les éditions n’étaient pas confirmées sur la page lue.
    18. Seulement le courrier que vous chiffrez (OpenPGP, ou S/MIME là où il est proposé) ; le reste demeure comme votre fournisseur le garde.
    19. Google : le chiffrement côté client dans certaines éditions de Workspace ; pas Gmail sinon.
    20. Microsoft : S/MIME, et le Double Key Encryption dans Microsoft 365 E5.
    21. Proton : le chiffrement à accès zéro de chaque message stocké.

??? info "Sources (en anglais)"
    Les articles d’aide de Mozilla ont été lus dans les copies de l’Internet Archive, support.mozilla.org refusant ce jour-là la lecture automatique.

    - Thunderbird, OpenPGP et cartes à puce : <https://support.mozilla.org/en-US/kb/openpgp-thunderbird-howto-and-faq>, lu le 8 octobre 2026.
    - Thunderbird, contenu distant : <https://support.mozilla.org/en-US/kb/remote-content-in-messages>, lu le 8 octobre 2026.
    - Thunderbird, détection des escroqueries : <https://support.mozilla.org/en-US/kb/thunderbirds-scam-detection>, lu le 8 octobre 2026.
    - Thunderbird, réglages de sécurité (antivirus, mots de passe) : <https://support.mozilla.org/en-US/kb/security-panel-settings-in-thunderbird>, lu le 8 octobre 2026.
    - Firefox, Sync : <https://support.mozilla.org/en-US/kb/sync>, lu le 8 octobre 2026.
    - Firefox, gestionnaire de mots de passe : <https://support.mozilla.org/en-US/kb/password-manager-remember-delete-edit-logins>, lu le 8 octobre 2026.
    - Compte Mozilla, authentification en deux étapes : <https://support.mozilla.org/en-US/kb/secure-mozilla-account-two-step-authentication>, lu le 8 octobre 2026.
    - Thunderbird, avis de confidentialité (télémétrie) : <https://www.mozilla.org/en-US/privacy/thunderbird/>, lu le 8 octobre 2026.
    - Firefox, avis de confidentialité (télémétrie) : <https://www.mozilla.org/en-US/privacy/firefox/>, lu le 8 octobre 2026.
    - Firefox 114, notes de version (FIDO2) : <https://www.firefox.com/en-US/firefox/114.0/releasenotes/>, lu le 8 octobre 2026.
    - Firefox 122, notes de version (clés d’accès) : <https://www.firefox.com/en-US/firefox/122.0/releasenotes/>, lu le 8 octobre 2026.
    - Mozilla, licences : <https://www.mozilla.org/en-US/foundation/licensing/>, lu le 8 octobre 2026.
    - Le serveur de Firefox Sync (licence) : <https://github.com/mozilla-services/syncstorage-rs>, lu le 8 octobre 2026.
    - Thunderbird, téléchargements (plateformes) : <https://www.thunderbird.net/en-US/download/>, lu le 8 octobre 2026.
    - Gmail, TLS et S/MIME : <https://support.google.com/mail/answer/6330403>, lu le 8 octobre 2026.
    - Google Workspace, chiffrement côté client : <https://knowledge.workspace.google.com/admin/security/about-client-side-encryption>, lu le 8 octobre 2026.
    - Google Workspace, chiffrement par clé matérielle : <https://knowledge.workspace.google.com/admin/security/gmail-only-set-up-and-manage-hardware-key-encryption>, lu le 8 octobre 2026.
    - Gestionnaire de mots de passe de Google, chiffrement sur l’appareil : <https://support.google.com/accounts/answer/11350823>, lu le 8 octobre 2026.
    - Google Workspace, régions des données : <https://knowledge.workspace.google.com/admin/compliance/choose-a-geographic-location-for-your-data>, lu le 8 octobre 2026.
    - Gmail, authentification affichée : <https://support.google.com/mail/answer/180707> et <https://support.google.com/mail/answer/1311182>, lu le 8 octobre 2026.
    - Gmail, indésirables et raisons affichées : <https://support.google.com/mail/answer/1366858>, lu le 8 octobre 2026.
    - Google Workspace, protection contre l’usurpation et les logiciels malveillants : <https://knowledge.workspace.google.com/admin/gmail/advanced/advanced-phishing-and-malware-protection>, lu le 8 octobre 2026.
    - Gmail, fichiers bloqués : <https://support.google.com/mail/answer/6590>, lu le 8 octobre 2026.
    - Google Workspace, bac à sable des pièces jointes : <https://knowledge.workspace.google.com/admin/gmail/advanced/set-up-rules-to-detect-harmful-attachments>, lu le 8 octobre 2026.
    - Gmail, images : <https://support.google.com/mail/answer/145919>, lu le 8 octobre 2026.
    - Google, clés d’accès : <https://support.google.com/accounts/answer/13548313>, lu le 8 octobre 2026.
    - Google, Protection avancée : <https://landing.google.com/advancedprotection/>, lu le 8 octobre 2026.
    - Google, règles de confidentialité : <https://policies.google.com/privacy>, lu le 8 octobre 2026.
    - Chrome, rapports de plantage : <https://support.google.com/chrome/answer/96817>, lu le 8 octobre 2026.
    - Chromium : <https://www.chromium.org/chromium-projects/>, lu le 8 octobre 2026.
    - Microsoft, politiques anti-hameçonnage (usurpation) : <https://learn.microsoft.com/en-us/defender-office-365/anti-phishing-policies-about>, lu le 8 octobre 2026.
    - Microsoft, protection contre l’usurpation : <https://learn.microsoft.com/en-us/defender-office-365/anti-phishing-protection-spoofing-about>, lu le 8 octobre 2026.
    - Outlook, hameçonnage et comportements suspects (« ? », « via ») : <https://support.microsoft.com/en-us/outlook/mail/phishing-and-suspicious-behavior-in-outlook>, lu le 8 octobre 2026.
    - Microsoft, protection contre les logiciels malveillants : <https://learn.microsoft.com/en-us/defender-office-365/anti-malware-protection-about>, lu le 8 octobre 2026.
    - Outlook, téléchargement des images : <https://support.microsoft.com/en-us/office/block-or-unblock-automatic-picture-downloads-in-email-messages-15e08854-6808-49b1-9a0a-50b81f2d617a>, lu le 8 octobre 2026.
    - Outlook.com, protection des images externes : <https://support.microsoft.com/en-us/office/external-image-protection-in-outlook-com-43c0c17e-8fd1-41c6-93fe-ffe54638e82b>, lu le 8 octobre 2026.
    - Exchange Online, S/MIME : <https://learn.microsoft.com/en-us/exchange/security-and-compliance/smime-exo/smime-exo>, lu le 8 octobre 2026.
    - Microsoft, Double Key Encryption : <https://learn.microsoft.com/en-us/purview/double-key-encryption>, lu le 8 octobre 2026.
    - Microsoft, frontière des données de l’UE : <https://learn.microsoft.com/en-us/privacy/eudb/eu-data-boundary-learn>, lu le 8 octobre 2026.
    - Microsoft, clés d’accès et FIDO2 : <https://learn.microsoft.com/en-us/entra/identity/authentication/how-to-enable-passkey-fido2> et <https://www.microsoft.com/en-us/security/blog/2025/05/01/pushing-passkeys-forward-microsofts-latest-updates-for-simpler-safer-sign-ins/>, lu le 8 octobre 2026.
    - Edge, sécurité du gestionnaire de mots de passe : <https://learn.microsoft.com/en-us/deployedge/microsoft-edge-security-password-manager-security>, lu le 8 octobre 2026.
    - Applications Microsoft 365, données de diagnostic : <https://learn.microsoft.com/en-us/microsoft-365-apps/privacy/overview-privacy-controls>, lu le 8 octobre 2026.
    - Outlook, identifiants numériques : <https://support.microsoft.com/en-us/office/get-a-digital-id-0eaa0ab9-b8a2-4a7e-828b-9bded6370b7b>, lu le 8 octobre 2026.
    - Proton Mail, chiffrement : <https://proton.me/support/proton-mail-encryption-explained>, lu le 8 octobre 2026.
    - Proton Mail, avertissement d’authentification : <https://proton.me/support/email-has-failed-its-domains-authentication-requirements-warning>, lu le 8 octobre 2026.
    - Proton Mail, fonctions de sécurité (PhishGuard) : <https://proton.me/mail/security>, lu le 8 octobre 2026.
    - Proton Mail, liens homographes : <https://proton.me/support/homograph-attacks>, lu le 8 octobre 2026.
    - Proton Mail, signaler l’hameçonnage : <https://proton.me/support/report-phishing>, lu le 8 octobre 2026.
    - Proton Mail, confirmation des liens : <https://proton.me/support/link-confirmation>, lu le 8 octobre 2026.
    - Proton Mail, politique de confidentialité (analyse antivirus) : <https://proton.me/mail/privacy-policy>, lu le 8 octobre 2026.
    - Proton Drive, protection contre les logiciels malveillants : <https://proton.me/support/proton-drive-malware-protection>, lu le 8 octobre 2026.
    - Proton Mail, protection contre les traqueurs : <https://proton.me/support/email-tracker-protection>, lu le 8 octobre 2026.
    - Proton Mail, images : <https://proton.me/support/protonmail-images>, lu le 8 octobre 2026.
    - Proton Mail, PGP : <https://proton.me/support/how-to-use-pgp>, lu le 8 octobre 2026.
    - Proton, clés de chiffrement : <https://proton.me/support/account/security-privacy/encryption-keys> et <https://proton.me/support/openpgp-keys-security>, lu le 8 octobre 2026.
    - Proton, clés de sécurité : <https://proton.me/support/2fa-security-key>, lu le 8 octobre 2026.
    - Proton Pass, clés d’accès : <https://proton.me/support/pass-use-passkeys>, lu le 8 octobre 2026.
    - Proton, statistiques d’usage : <https://proton.me/support/share-usage-statistics>, lu le 8 octobre 2026.
    - Proton, politique de confidentialité : <https://proton.me/legal/privacy>, lu le 8 octobre 2026.
    - Proton, code ouvert : <https://proton.me/community/open-source>, lu le 8 octobre 2026.
    - Proton Mail, application de bureau : <https://proton.me/support/mail-desktop-app>, lu le 8 octobre 2026.
    - Nextcloud, chiffrement côté serveur : <https://docs.nextcloud.com/server/latest/admin_manual/configuration_files/encryption_configuration.html>, lu le 8 octobre 2026.
    - Nextcloud, chiffrement de bout en bout : <https://docs.nextcloud.com/server/latest/user_manual/en/files/using_e2ee.html>, lu le 8 octobre 2026.
    - Nextcloud Mail, détection de l’hameçonnage : <https://docs.nextcloud.com/server/latest/user_manual/en/groupware/mail.html>, lu le 8 octobre 2026.
    - Nextcloud, antivirus : <https://docs.nextcloud.com/server/latest/admin_manual/configuration_server/antivirus_configuration.html>, lu le 8 octobre 2026.
    - Nextcloud Mail, images bloquées : <https://github.com/nextcloud/mail/blob/main/src/components/BlockedContentWarning.vue>, lu le 8 octobre 2026.
    - Nextcloud Mail, Mailvelope et S/MIME : <https://github.com/nextcloud/mail/blob/main/README.md>, lu le 8 octobre 2026.
    - Nextcloud, authentification à deux facteurs et WebAuthn : <https://docs.nextcloud.com/server/latest/user_manual/en/user_2fa.html>, lu le 8 octobre 2026.
    - Nextcloud, application Passwords : <https://apps.nextcloud.com/apps/passwords>, lu le 8 octobre 2026.
    - Nextcloud, vérification des mises à jour : <https://docs.nextcloud.com/server/latest/admin_manual/configuration_server/config_sample_php_parameters.html>, lu le 8 octobre 2026.
    - Nextcloud, enquête d’usage : <https://github.com/nextcloud/survey_client>, lu le 8 octobre 2026.
    - Nextcloud Mail, licence : <https://github.com/nextcloud/mail/blob/main/appinfo/info.xml>, lu le 8 octobre 2026.

## Côté technique {#for-technical-readers}

- **La vérification des expéditeurs** : `mail-auth`, de Stalwart, à l’arrivée, par le résolveur du système : SPF (RFC 7208) sur le saut où le message est entré chez votre fournisseur, DKIM (RFC 6376), DMARC (RFC 7489), ARC (RFC 8617), iprev. Les résultats sont ajoutés en tête comme `Authentication-Results` (RFC 8601) sous un authserv-id propre à l’installation (`sioul-….invalid`) ; les résultats d’un fournisseur ne comptent que sous son identifiant, appris dans son courrier. Vérifié : DMARC passe, sinon une signature DKIM valide du domaine de De. Falsifié : DMARC échoue sous `p=quarantine` ou `p=reject`. Non authentifié : SPF et DKIM échouent, DMARC ne passe pas, aucun ARC scellé par votre fournisseur ou vos domaines ; un tel expéditeur est un inconnu pour chaque règle du Porche. Plus de détails : [Courrier, comment un expéditeur est vérifié](mail.md#how-a-sender-is-checked).
- **Les noms empruntés** : les squelettes de confusables d’Unicode (UTS #39), comparés à une cinquantaine de marques et de services publics et à vos propres domaines.
- **Le HTML** : une liste d’ammonia des balises de texte permises ; `href` seul attribut ; http, https et mailto seuls schémas ; les liens relatifs refusés ; pas de `<img>`, de style ni de script. Le site d’un lien est lu après tout `nom@` placé devant pour tromper.
- **Les pièces jointes** : ClamAV (`clamdscan --fdpass`, sinon `clamscan` ; les signatures rafraîchies chaque jour avec `freshclam` quand le système n’en garde aucune) ou AMSI ; les programmes seulement enregistrés ; les noms débarrassés de tout dossier, et sous Windows des noms de périphériques et des flux ; la Mark of the Web (`Zone.Identifier`, ZoneId=3) et `com.apple.quarantine` sur ce qui est ouvert ou enregistré.
- **Le transport** : rustls avec *ring*, TLS 1.2 et 1.3, les certificats racines du système (ceux de Mozilla quand un système n’en a pas) ; IMAP et SMTP en TLS dès le premier octet (RFC 8314) ou en STARTTLS, sans option en clair, et un serveur sans STARTTLS refusé ; CalDAV, CardDAV, Bitwarden et les recherches de clés en HTTPS seulement. SMTP se présente comme `[127.0.0.1]`, pour que le nom de votre ordinateur reste hors des lignes `Received`. Google : OAuth 2.0 pour les applications natives (RFC 8252) sur un port local tiré au hasard, PKCE S256 (RFC 7636), l’état comparé en temps constant ; le jeton de renouvellement dans le trousseau, le jeton d’accès en mémoire.
- **Le partage** : XChaCha20-Poly1305 avec un nonce aléatoire de 192 bits par enregistrement, chacun lié à son appareil, son tour, sa place et son horloge comme données associées. La clé vient d’Argon2id (64 Mio, 3 passes, un sel de 16 octets), et le trousseau de chaque appareil garde la clé, pas la phrase de passe. Les notes et les papiers sont compressés en gzip et scellés en morceaux de 1 Mio sous une sous-clé HKDF-SHA-256, nommés par un HMAC-SHA-256 de leur contenu. Le détail : [Le partage, côté technique](sharing.md#for-technical-readers).
- **OpenPGP** : Sequoia, cryptographie en pur Rust, politique standard ; RFC 9580, PGP/MIME (RFC 3156), le PGP en ligne lu. Les nouvelles clés sont en version 4 sur Curve25519 (EdDSA, ECDH), valables trois ans. Le courrier chiffré est signé à l’intérieur, pour chaque destinataire et pour vous. Autocrypt envoie votre clé et ne prend de clés que dans le courrier vérifié. « Chercher leurs clés » interroge le Web Key Directory, puis keys.openpgp.org, en HTTPS. Essayé dans les deux sens avec GnuPG 2.4.
- **OpenPGP sur une clé de sécurité** : carte OpenPGP 3.4 (YubiKey, Nitrokey) par PC/SC, en mode partagé, une transaction par opération, la carte remise à zéro ensuite. Chaque signature est vérifiée avec le certificat avant usage, et un certificat n’est accepté que s’il contient les propres clés publiques de la carte. Le code PIN est gardé en mémoire seulement (le `Password` de Sequoia), oublié après 15 minutes sans usage, quand la clé est retirée, ou à la fermeture. Aucun journal n’est installé, et la journalisation est plafonnée au démarrage de chaque programme, pour que les traces de la bibliothèque de la carte, qui contiendraient le code PIN, ne soient jamais écrites. Jamais le code PIN d’administration ; rien n’est écrit sur la carte.
- **Les sites** : le WebAuthn de Chromium dans Qt WebEngine (FIDO2, CTAP2 par USB, clés d’accès sur la clé), avec la boîte de dialogue de Sioul pour le compte et le code PIN ; un profil à lui ; la position, les polices locales et le verrouillage du pointeur refusés ; aucune erreur de certificat jamais acceptée par Sioul.
- **Bitwarden** : le client propre à Sioul, en lecture seule. PBKDF2-SHA256 ou Argon2id selon ce que dit votre compte, tenus dans les limites de Bitwarden lui-même pour qu’un serveur ne puisse pas rendre votre mot de passe moins coûteux à deviner ; la connexion envoie une empreinte, jamais le mot de passe ; étirement par HKDF, AES-256-CBC avec HMAC-SHA256 (le MAC vérifié d’abord, en temps constant) ou COSE, les clés d’organisation par RSA-OAEP. Les clés vivent dans une mémoire effacée après usage, et le trousseau ne garde que le jeton de l’appareil. La deuxième étape peut être une clé FIDO2, par la page de clé de sécurité de Bitwarden lui-même ; la connexion par clé d’accès utilise l’extension PRF de WebAuthn ; les codes à usage unique suivent la RFC 6238.
- **Le trousseau** : le Secret Service, le Gestionnaire d’identification de Windows, le Trousseau d’accès de macOS, le KeyStore d’Android (AES-GCM, la clé ne quittant jamais le KeyStore).
- **Les fichiers** : les dossiers de Sioul en 0700, créés ou resserrés ainsi au démarrage de Sioul, et ses fichiers sensibles en 0600, sous Linux et macOS.
- **Les agents d’IA** : MCP par l’entrée et la sortie standard seulement, donc sans port réseau ouvert. Les codes, les liens de connexion, les IBAN (mod 97), les numéros de carte (Luhn) et les numéros de sécurité sociale français sont masqués ; le courrier et les notes arrivent encadrés comme des données ; chaque appel est noté par ses adresses et sa longueur, jamais par ses mots. Plus de détails : [Avec un agent d’IA](ai-agent.md#for-technical-readers).
- **L’IA du bouclier** : Claude Haiku 4.5 par l’API d’Anthropic, avec votre clé tirée du trousseau ; l’objet et les 4 000 premiers caractères de chaque message reçu à cette adresse, sans rien masquer ; aucune redirection suivie, pour que la clé n’aille qu’à Anthropic.
- **Aucune télémétrie** : aucun code de mesure d’audience, de rapport de plantage ni de vérification de mise à jour n’existe. Les seules destinations réseau du code sont celles du tableau plus haut.
- **Ce qui a été essayé** : l’antivirus sous Windows (AMSI) suit la séquence documentée par Microsoft et est construit par les compilations automatiques du projet, mais n’a pas encore tourné sous Windows ; la clé de sécurité OpenPGP est essayée avec une carte logicielle, et les clés de sécurité dans les sites n’ont pas encore été essayées avec une vraie clé ; la connexion à Google n’a été essayée qu’avec des doublures de Google.
- **Le code** est un logiciel libre, sous licence GPL-3.0-or-later, pour que tout cela puisse être vérifié : [github.com/aurelienpierre/sioul](https://github.com/aurelienpierre/sioul). Le paquet Android est signé avec la propre clé de Sioul ; l’installateur Windows n’est pas encore signé, et l’image macOS pas encore notariée.
