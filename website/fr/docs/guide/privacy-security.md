---
description: Vie privée et sécurité dans Sioul – ce qui reste sur votre appareil, ce qui en sort et quand, comment chaque message et chaque pièce jointe sont vérifiés, où vont les mots de passe.
---

# Vie privée et sécurité {#privacy-and-security}

Sioul tourne sur votre appareil. Il n’y a pas de serveur Sioul, pas de compte chez nous, et rien ne va au développeur. Votre courrier, vos agendas et vos contacts ne voyagent qu’entre votre appareil et vos propres fournisseurs.

Le texte formel, entre autres pour la connexion avec Google : [Politique de confidentialité](../privacy.md).

## Ce qui reste sur votre appareil {#what-stays-on-your-computer}

Tout ce que Sioul garde est dans des fichiers simples, dans vos propres dossiers, lisibles par d’autres programmes :

| Quoi | Où, sous Linux |
|---|---|
| Les réglages, les expéditeurs que vous laissez entrer, sûrs, neutres, restreints ou bloqués | `~/.config/sioul/` |
| Le courrier (un Maildir par adresse), les agendas et les contacts (un fichier par élément), les brouillons, le temps passé, les factures, les médicaments, les journées de votre montre | `~/.local/share/sioul/` |
| Où chaque relève s’est arrêtée, où le Porche a été fermé, les nouvelles des sites, le journal des appels d’un agent d’IA | `~/.local/state/sioul/` |
| Notes, projets, budgets, papiers, lettres scannées | votre dossier de notes, là où vous l’avez choisi |

Sous Windows, les dossiers de Sioul sont dans `%APPDATA%\Sioul` ; sous macOS, dans `~/Library/Application Support/Sioul`.

**Les mots de passe**, l’accès à Google, les clés et les jetons des services que vous activez, et les phrases de passe vont dans le trousseau de votre système (GNOME Keyring ou KWallet sous Linux, le Gestionnaire d’identification sous Windows, le Trousseau d’accès sous macOS). Jamais dans un fichier.

## Ce qui sort de votre appareil, et quand {#what-leaves-your-computer-and-when}

| Vers | Quoi | Quand |
|---|---|---|
| Votre fournisseur de courrier | votre courrier : relevé, rangé comme vous le demandez, envoyé quand vous envoyez | toujours : c’est votre courrier |
| Votre serveur d’agendas et de contacts, ou Google | vos agendas, tâches et contacts, dans les deux sens | toujours |
| Les sites que vous épinglez | ce que tout navigateur leur envoie | quand ils sont ouverts |
| Les réglages publiés par votre fournisseur, et la liste de fournisseurs de Thunderbird | le domaine de votre adresse, pour trouver le serveur | quand vous ajoutez un compte |
| Le résolveur DNS de votre système | des requêtes sur les domaines des expéditeurs, pour les vérifier | quand du courrier arrive |
| Open-Meteo | les coordonnées du lieu choisi, arrondies à deux décimales, ou le nom d’une ville que vous cherchez | seulement si vous choisissez un lieu pour la météo ; toutes les demi-heures au plus |
| Le géocodeur d’OpenStreetMap (Nominatim) | les adresses postales de vos contacts, une fois chacune, rien d’autre | seulement après que vous avez choisi **Les placer** |
| Les images de carte d’OpenStreetMap, ou celles que vous avez choisies | quelle partie de la carte est montrée | tant que la carte est ouverte |
| Le Web Key Directory du domaine des destinataires, puis keys.openpgp.org | les adresses auxquelles vous écrivez | seulement quand vous choisissez **Chercher leurs clés** |
| Chaque site que vous épinglez | une demande de sa propre icône | environ une fois par semaine |
| Anthropic | chaque nouveau message vers une adresse que vous protégez | seulement avec **Laisser l’IA le lire d’abord**, et votre propre clé |
| L’agent d’IA que vous connectez, et son fournisseur | ce qu’il lit | seulement si vous en connectez un ([Avec un agent d’IA](ai-agent.md)) |
| GitHub | votre jeton ; des recherches de vos tickets et pull requests | seulement si vous activez GitHub |
| Votre serveur Bitwarden | votre connexion, pour ouvrir votre coffre | quand vous remplissez un identifiant depuis lui |
| Les serveurs de mise à jour de ClamAV | une demande de signatures de virus | une fois par jour, seulement quand votre système n’en garde aucune lui-même |

Sioul n’a ni mesure d’audience, ni publicité, ni rapports de plantage.

## Votre serveur de courrier {#your-mail-server}

Relever ne change rien sur le serveur : aucun message n’est marqué lu parce qu’il a été relevé. Sioul n’y écrit que quand vous agissez : ouvrir un message le marque lu, comme le fait tout programme de courrier ; archiver, supprimer, mettre en indésirable et déplacer se font dix secondes après votre demande, pour qu’**Annuler** puisse les arrêter. Retirer un compte retire son mot de passe, pas son courrier.

## Chaque message vérifié {#every-message-checked}

Quand un message arrive, Sioul le vérifie lui-même, par le DNS de votre système :

- **SPF**, sur le serveur qui a remis le message à votre fournisseur ;
- **DKIM**, chaque signature contre la clé que publie son domaine, vérifiée à l’arrivée, car les clés changent ;
- **DMARC**, avec la politique du domaine lui-même ;
- **ARC**, pour le courrier transféré, et le **DNS inverse** du serveur d’envoi.

Les résultats sont gardés avec le message, sous un nom que seule votre copie de Sioul utilise, pour qu’aucun expéditeur ne puisse écrire des résultats qui passeraient pour ceux de Sioul. Un message est alors **vérifié**, **non vérifié** (avec la raison), ou **falsifié**. Le courrier falsifié est mis de côté, avec la raison ; rien n’est supprimé. Les verdicts de votre fournisseur sur les indésirables comptent aussi.

**Les noms empruntés** sont repérés : un expéditeur qui prend le nom d’une banque, d’un service public ou de votre propre domaine, depuis une adresse qui ne leur appartient pas, est mis de côté. Les lettres qui se ressemblent (un l minuscule pour un I majuscule, un 0 pour un O, des lettres cyrilliques) ne cachent pas le nom.

**Le courrier HTML** est montré avec ses paragraphes, ses listes, son texte en gras et ses liens seulement : pas d’images, pas de styles, pas de scripts, rien qui se charge depuis le réseau, si bien qu’aucune image de pistage n’apprend que vous avez ouvert un message. Chaque lien montre son adresse complète avant que vous cliquiez dessus.

## Les pièces jointes et l’antivirus {#attachments-and-the-antivirus}

Une pièce jointe est vérifiée avant de s’ouvrir ou d’être enregistrée, par l’antivirus de votre système :

- **Sous Linux et macOS** : ClamAV, quand il est installé ([Installer](install.md#the-packages)).
- **Sous Windows** : Microsoft Defender, par l’Antimalware Scan Interface.

Quand une menace est trouvée, rien ne s’ouvre, et la copie est effacée. Un programme, un script ou un installateur n’est jamais lancé depuis un courriel, même vérifié : enregistrez-le si vous lui faites confiance. Sans antivirus, Sioul ne refuse pas : il dit que le fichier ne sera pas vérifié, demande avant de l’ouvrir (**L’ouvrir sans vérification**), et donne la commande qui en installe un. Les pièces jointes du courrier mis de côté ne s’ouvrent pas du tout.

## Les sites {#sites}

Les sites que vous épinglez vivent dans un profil de navigateur propre à Sioul, à part de votre navigateur habituel. Leurs notifications sont gardées par Sioul. Le micro, la caméra et le partage de l’écran sont permis site par site, par vous ; l’endroit où vous êtes n’est jamais donné. Les téléchargements vont dans votre dossier de téléchargements.

Le code PIN d’une clé de sécurité est demandé dans la boîte de dialogue propre à Sioul. Les identifiants viennent de votre coffre Bitwarden, ouvert par Sioul lui-même, en lecture seule : votre mot de passe principal et vos clés ne sont jamais gardés, et les identifiants restent en mémoire jusqu’à la fermeture de Sioul. Un identifiant fait pour un autre domaine le dit, pour qu’un site qui en imite un autre se remarque. Voir [Les sites](sites.md).

## Entre vos appareils {#between-your-devices}

Chaque appareil garde ses propres données. Ce qui doit voyager entre vos ordinateurs et votre téléphone passe par un dossier que votre propre application de synchronisation transporte (Nextcloud, Dropbox, Syncthing, Google Drive, OneDrive…), scellé sur votre appareil avant d’y être écrit (XChaCha20-Poly1305, avec une clé tirée de votre phrase de passe par Argon2id). Le serveur du dossier voit quel appareil a écrit, quand et combien, et la taille de chaque note ou papier scellé ; jamais quoi. Rien de ce qui arrive dans ce dossier, un fichier abîmé ou effacé, ne peut retirer des données à vos appareils. Comment cela marche, ce que cela protège et ce que cela ne peut pas cacher : [Partager entre vos appareils](sharing.md).

## Le courrier chiffré {#encrypted-mail}

Avec OpenPGP, Sioul signe et chiffre vos messages quand vous les envoyez, déchiffre et vérifie ceux que vous recevez, et donne votre clé publique aux personnes à qui vous écrivez (Autocrypt). Vos clés secrètes restent sur l’appareil où elles ont été créées. Voir [Le courrier](mail.md#signing-and-encrypting).

## Une adresse publique {#a-public-address}

Une adresse que vous publiez peut être protégée contre le harcèlement : son courrier est lu avant que vous le voyiez, et les messages hostiles sont écartés sans que leurs mots soient montrés. Voir [le Porche](porch.md#a-public-address-protected).

## Ce site web {#this-website}

Ce site web n’a ni mesure d’audience, ni publicité, ni cookies, et ne charge rien depuis d’autres sites. Il est hébergé par GitHub Pages, qui garde ses propres journaux de serveur, selon [la déclaration de confidentialité de GitHub (en anglais)](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

## Signaler un problème {#reporting-a-problem}

Un problème de sécurité, ou tout ce qui vous inquiète : [les tickets GitHub](https://github.com/aurelienpierre/sioul/issues).
