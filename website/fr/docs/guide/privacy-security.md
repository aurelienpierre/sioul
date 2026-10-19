---
description: "Vie privée et sécurité dans Sioul : pourquoi Sioul chiffre les courriels, quand les banques et les organismes gardent leurs courriers derrière des messageries sécurisées ; ce qui est protégé par défaut, sans rien régler ; comment chaque courriel et chaque pièce jointe sont vérifiés ; comment ce qui voyage entre vos appareils est scellé ; les clés de sécurité et Bitwarden ; ce qui sort de votre appareil et quand ; ce que Sioul ne peut pas protéger ; et la comparaison avec d’autres applications."
---

# Vie privée et sécurité {#privacy-and-security}

## En bref {#in-short}

Les courriels administratifs sont la cible des escroqueries, et vous ne devriez pas avoir à rester sur vos gardes chaque fois que vous les ouvrez. Sioul vous protège donc par défaut, sans vous demander de rien régler. Il tourne sur vos appareils, garde vos données dans de simples fichiers de vos propres dossiers, et n’a aucun serveur à lui : rien ne parvient au développeur. Chaque courriel est vérifié avant que vous le voyiez, rien ne se charge depuis le réseau pendant que vous lisez, et une pièce jointe passe par votre antivirus avant de s’ouvrir. Ce qui voyage entre vos appareils est scellé avec une clé que vous seul détenez, les mots de passe restent dans le trousseau de votre système, et une clé de sécurité peut garder vos courriels chiffrés, les sites que vous gardez et votre coffre Bitwarden.

Le texte formel, entre autres pour la connexion avec Google : [Politique de confidentialité](../privacy.md).

## Pourquoi Sioul chiffre les courriels {#why-sioul-encrypts-mail}

Sioul ne prend pas en charge le chiffrement pour le principe. Il est là pour que les courriers des banques, des assurances et des administrations puissent revenir au courriel, en sécurité, et pour que vous n’ayez plus à surveiller un portail par organisme.

**Le vrai problème.** Le courriel n’a pas été conçu pour être confidentiel. Les serveurs ne le chiffrent entre eux que si tous deux sont d’accord, et un attaquant qui peut effacer une ligne de leur échange peut les amener à l’envoyer en clair (RFC 3207, RFC 8461). Il attend ensuite, lisible, dans chaque boîte où il arrive. Le RGPD demande à qui envoie des données personnelles de les protéger, et range le chiffrement parmi ses premières mesures (article 32) ; la CNIL rappelle que, sans mesure complémentaire, les canaux grand public comme le courriel « constituent rarement un moyen de communication sûr pour transmettre des données personnelles », et que les entités qui ont accès aux serveurs traversés « peuvent avoir accès à leur contenu » (2024).

**La réponse choisie par les organismes.** Chacun a bâti sa « messagerie sécurisée », derrière son identifiant et son code, et votre propre boîte reçoit tout au plus un avis. Le guide des impôts le dit sans détour : « un courriel vous informera de son traitement. Vous devrez vous connecter à votre espace particulier pour lire la réponse. » Le Défenseur des droits a décrit ce que cela a reporté sur les personnes : s’informer, remplir seul des formulaires, « suivre ses courriels sur une ou plusieurs messageries » ; ouvrir un espace personnel vaut souvent adhésion à des échanges uniquement en ligne, si bien que la personne qui ne consulte pas son espace à temps risque une « rupture des droits » (2022). En 2026, le Défenseur des droits ajoute que la dématérialisation s’est construite sur « un modèle d’usager idéalisé, à la fois disponible, autonome et connecté », que les systèmes d’authentification « changent sans cesse », et que, « sans la possibilité d’être aidée par des proches, près d’une personne sur quatre, à force de temps perdu, de découragement, voire d’angoisse, finit ainsi par renoncer à des droits auxquels elle peut pourtant prétendre ».

Chaque portail demande de retenir, de prévoir et de suivre : en moyenne, ce que le TDAH, la dépression, l’épuisement professionnel et le TSPT rendent plus difficile (méta-analyses : Willcutt et al. 2005 ; Snyder 2013 ; Scott et al. 2015 ; une revue pour l’épuisement : Deligkaris et al. 2014). Aucune étude n’a mesuré les portails dans ces groupes : qu’ils y pèsent le plus est une déduction tirée de ces résultats, pas un résultat. Le W3C, qui écrit les règles d’accessibilité du web, nomme les mots de passe à retenir et les codes à recopier comme des obstacles pour les personnes dont la mémoire ou les fonctions exécutives sont atteintes, et les gestionnaires de mots de passe comme un remède (WCAG 2.2, critère 3.3.8).

**Les réserves derrière les portails n’étaient pas sûres.** Ces réserves réunissent les données de millions de personnes en un seul endroit, joignable par les comptes d’agents, de partenaires et de prestataires. Depuis 2020 :

- 2021 : l’AP-HP s’est fait dérober des fichiers sur un « service sécurisé de partage de fichiers » qu’elle hébergeait : environ 1,4 million de personnes testées pour la Covid-19 en 2020 ([AP-HP, septembre 2021](https://www.aphp.fr/actualites/lap-hp-porte-plainte-suite-une-attaque-informatique-sur-son-service-securise-de-partage)).
- 2022 : dix-neuf comptes de professionnels de santé sur amelipro, le portail des professionnels de l’Assurance Maladie, dont les adresses électroniques avaient été compromises, ont livré les données de 510 000 assurés ([Assurance Maladie, 17 mars 2022](https://www.assurance-maladie.ameli.fr/presse/2022-03-17-infopresse-connexions-non-autorisees-comptes-amel)).
- 2023 : un prestataire de Pôle emploi (aujourd’hui France Travail) a été piraté : les noms et numéros de sécurité sociale des personnes inscrites en février 2022, et d’anciens usagers, sont potentiellement concernés ([Pôle emploi, 23 août 2023](https://francetravail.org/files/live/sites/peorg/files/documents/Communiqu%C3%A9%20de%20presse/20230823%20-%20POLE%20EMPLOI%20-%20CP%20-%20Un%20prestataire%20de%20P%C3%B4le%20emploi%20victime%20d%E2%80%99un%20acte%20de%20cyber-malveillance-1.pdf)).
- 2024 : Viamedis et Almerys, opérateurs de tiers payant des complémentaires santé : plus de 33 millions de personnes ([CNIL, 7 février 2024](https://cnil.fr/fr/violation-de-donnees-de-deux-operateurs-de-tiers-payant-la-cnil-ouvre-une-enquete-et-rappelle-aux)).
- 2024 : France Travail et Cap emploi : des attaquants, avec des comptes usurpés de conseillers de Cap emploi (les structures qui accompagnent vers l’emploi les personnes en situation de handicap), ont atteint les données de toutes les personnes inscrites, ou qui l’ont été au cours des vingt dernières années, potentiellement 43 millions selon les premiers chiffres de la CNIL ; en janvier 2026, la CNIL a infligé 5 millions d’euros d’amende à France Travail, jugeant l’authentification des conseillers insuffisamment robuste ([CNIL, 13 mars 2024](https://www.cnil.fr/fr/france-travail-la-cnil-enquete-sur-la-fuite-de-donnees-et-donne-des-conseils-pour-se-proteger) ; [29 janvier 2026](https://cnil.fr/fr/violation-de-donnees-sanction-5millions-france-travail)).
- 2026 : FICOBA, le fichier de tous les comptes bancaires ouverts en France : une partie en a été consultée de fin janvier au 13 février avec les identifiants usurpés d’un fonctionnaire d’une autre administration ([réponse du ministère à l’Assemblée nationale, 28 avril 2026](https://www.assemblee-nationale.fr/dyn/17/questions/QANR5L17QE13264)).
- 2026 : France Titres (ANTS), le portail des cartes d’identité, des passeports, des permis de conduire et des cartes grises : un incident pouvant impliquer une divulgation de données de 11,7 millions de ses comptes ([info.gouv.fr, 22 avril 2026](https://www.info.gouv.fr/actualite/france-titres-le-point-sur-l-incident-de-securite)).
- 2026 : Vacaf, le service d’aide aux vacances des Caf, hébergé par un prestataire : une fuite des données des bénéficiaires a été signalée ([Cnaf, 10 octobre 2026](https://www.caf.fr/sites/default/files/medias/cnaf/Nous_connaitre/Presse/2026/261010%20CPIncident%20de%20cybers%C3%A9curit%C3%A9%20concernant%20Vacaf_0.pdf)).
- Hors de France : une seule faille d’un seul outil de transfert de fichiers, MOVEit Transfer, a servi dès le 27 mai 2023 à voler des données chez les organisations dont les serveurs le faisaient tourner ([CISA et FBI, 7 juin 2023, en anglais](https://www.cisa.gov/news-events/cybersecurity-advisories/aa23-158a)) ; chez une seule entreprise américaine, Change Healthcare, environ 192,7 millions de personnes ont été touchées ([HHS, 2025, en anglais](https://www.hhs.gov/hipaa/for-professionals/special-topics/change-healthcare-cybersecurity-incident-frequently-asked-questions/index.html)).

La CNIL le résume : en 2024, « le nombre de violations touchant plus d’un million de personnes a doublé en un an » ; elle cite parmi les voies d’entrée des identifiants compromis, l’hameçonnage et les sous-traitants, et certaines intrusions n’ont été découvertes qu’à la mise en vente des données ([CNIL, 28 janvier 2025](https://www.cnil.fr/fr/violations-massives-de-donnees-en-2024-quels-sont-les-principaux-enseignements-mesures-a-prendre)).

**La réponse de Sioul.** Un courriel chiffré avec OpenPGP pour la personne à qui il est destiné, et ouvert par une clé qu’elle détient, peut rester sur n’importe quel serveur et traverser n’importe quel réseau sans être lisible : une violation chez un fournisseur de courriel en chemin, ou de votre propre boîte, ne livre alors rien de ce que dit le courrier, et aucun portail ne garde de réserve des messages de tout le monde à dérober. (L’organisme garde ses propres dossiers : le chiffrement protège le courrier, pas ses fichiers.) Le RGPD le dit à sa façon : la violation de données rendues « incompréhensibles pour toute personne qui n’est pas autorisée à y avoir accès » n’oblige même pas à prévenir les personnes concernées (article 34). De votre côté, tout reste dans une fenêtre : votre clé, sur une clé de sécurité si vous le souhaitez, qui ne la livre jamais ; vos identifiants, dans votre gestionnaire de mots de passe ; et une clé de sécurité qu’une fausse page ne peut pas tromper. C’est ce que demandent les recommandations officielles : un gestionnaire de mots de passe (l’ANSSI, la CNIL, le NIST américain), un mot de passe différent pour chaque service, et une deuxième étape, de préférence un objet que vous détenez (l’ANSSI). En attendant que les organismes écrivent ainsi, la page [Sites](sites.md) garde leurs portails en un seul endroit : sur un ordinateur, chacun connecté une fois, ses nouvelles gardées pour vos heures et dites en une ligne sous le Porche ; sur un téléphone, ouverts dans votre navigateur.

**Ce qu’il ne peut pas faire seul.** Un organisme doit envoyer des courriels chiffrés pour qu’ils arrivent chiffrés. Certains en reçoivent ou en envoient déjà : le CERT-FR, l’équipe nationale de réponse aux incidents informatiques, publie sa clé OpenPGP ; en 2019, une caisse d’épargne allemande laissait ses clients choisir entre son portail et des courriels S/MIME ou OpenPGP, lus directement dans leur propre boîte (avec un logiciel de courriel, pas avec une messagerie web) ; en 2019 aussi, l’administration judiciaire de Basse-Saxe expliquait comment elle échange des courriels S/MIME et PGP pour les affaires administratives. Le chiffrement cache ce que dit un courrier, pas qui a écrit à qui ni quand, ni, dans Sioul comme dans la plupart des logiciels de courriel, son objet. Une clé perdue sans sauvegarde emporte avec elle de quoi ouvrir les anciens messages. Et votre clé OpenPGP sur une clé de sécurité marche sur un ordinateur, pas encore sur un téléphone.

??? info "Sources"
    Lues le 18 octobre 2026. Les pages de l’AP-HP, du gouvernement sur France Titres, de HHS et de La Banque Postale refusent la lecture automatique : elles ont été lues dans les copies de l’Internet Archive.

    - Le courriel entre serveurs, chiffré seulement quand les deux le veulent : RFC 3207, SMTP sur TLS (2002), section 6, <https://www.rfc-editor.org/rfc/rfc3207> ; RFC 8461, MTA-STS (2018), section 1, <https://www.rfc-editor.org/rfc/rfc8461> (en anglais).
    - Le RGPD, règlement (UE) 2016/679, articles 32 et 34 : <https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=CELEX:32016R0679>.
    - CNIL, « Sécurité : Sécuriser les échanges avec l’extérieur », 14 mars 2024 : <https://www.cnil.fr/fr/securite-securiser-les-echanges-avec-lexterieur>.
    - Les messageries sécurisées de deux banques, dans leurs propres mots : Société Générale (« de façon renforcée par rapport à une boîte mail classique »), <https://particuliers.sg.fr/services-en-ligne/messagerie-securisee-conseiller> ; La Banque Postale, <https://www.labanquepostale.fr/particulier/comptes-et-cartes/espaces-clients/messagerie-securisee.html>.
    - Un code par portail : une deuxième étape pour impots.gouv.fr depuis juin 2025, dans la réponse du ministère à la question écrite 13264, 28 avril 2026, <https://www.assemblee-nationale.fr/dyn/17/questions/QANR5L17QE13264> ; l’authentification forte dans chaque banque, directive (UE) 2015/2366, article 97, <https://eur-lex.europa.eu/legal-content/FR/TXT/HTML/?uri=CELEX:32015L2366>.
    - DGFiP, « J’utilise ma messagerie sécurisée », fiche 09, janvier 2026 : <https://www.impots.gouv.fr/sites/default/files/media/srp/plsu/fiches/09_impots_gouv_fr_utiliser_messagerie_securisee.pdf>.
    - Défenseur des droits, « Dématérialisation des services publics : trois ans après, où en est-on ? », février 2022, p. 23 et 58–59 : <https://www.vie-publique.fr/files/rapport/pdf/283832.pdf>.
    - Défenseur des droits, « Rapport annuel d’activité 2025 », avril 2026 : <https://www.defenseurdesdroits.fr/sites/default/files/2026-04/ddd_rapport-annuel-2025_20260407.pdf>.
    - Les fonctions exécutives, en moyenne (en anglais) : Willcutt, Doyle, Nigg, Faraone & Pennington (2005), TDAH, une méta-analyse de 83 études, *Biological Psychiatry* 57(11), <https://doi.org/10.1016/j.biopsych.2005.02.006> ; Snyder (2013), dépression, 113 études, *Psychological Bulletin* 139(1), <https://doi.org/10.1037/a0028727> ; Scott et al. (2015), TSPT, 60 études, *Psychological Bulletin* 141(1), <https://doi.org/10.1037/a0038039> ; Deligkaris, Panagopoulou, Montgomery & Masoura (2014), épuisement professionnel, une revue systématique de 15 études, *Work & Stress* 28(2), <https://doi.org/10.1080/02678373.2014.909545>.
    - W3C, WCAG 2.2, critère de succès 3.3.8, « Accessible Authentication (Minimum) », <https://www.w3.org/TR/WCAG22/#accessible-authentication-minimum>, et son explication, <https://www.w3.org/WAI/WCAG22/Understanding/accessible-authentication-minimum.html> ; W3C, « Making Content Usable for People with Cognitive and Learning Disabilities » (2021), <https://www.w3.org/TR/coga-usable/> (en anglais).
    - Les violations de données : l’avis de chaque organisme, ou celui du régulateur, en lien plus haut ; chaque organisme est nommé comme cet avis le nomme.
    - Les mots de passe et la deuxième étape : ANSSI, « Recommandations relatives à l’authentification multifacteur et aux mots de passe » (2021), R33, R34 et son résumé pour les utilisateurs, <https://messervices.cyber.gouv.fr/documents-guides/anssi-guide-authentification_multifacteur_et_mots_de_passe.pdf> ; CNIL, délibération 2022-100 sur les mots de passe, § 32, <https://www.cnil.fr/sites/cnil/files/atoms/files/deliberation-2022-100-du-21-juillet-2022_recommandation-aux-mots-de-passe.pdf> ; NIST SP 800-63B (2025), <https://pages.nist.gov/800-63-4/sp800-63b.html> (en anglais).
    - CERT-FR, sa page de contact et sa clé OpenPGP : <https://cert.cyber.gouv.fr/contact/>.
    - Sparkasse Rhein-Nahe, « Secure E-Mail - Kundeninformation », septembre 2019 : <https://www.sparkasse-rhein-nahe.de/content/dam/myif/spk-rhein-nahe/work/dokumente/pdf/allgemein/Secure%20E-Mail%20Kundeninformation.pdf> (en allemand).
    - Niedersächsische Justiz, « Informationen zur sicheren E-Mail-Kommunikation mit der Nds. Justiz », 25 mars 2019 : <https://cert.justiz.niedersachsen.de/Informationen_externe_Partner.pdf> (en allemand).
    - Ce que le chiffrement laisse lisible : RFC 9580, OpenPGP (2024), section 13.11, <https://www.rfc-editor.org/rfc/rfc9580> (en anglais).
    - Tout le dossier, chaque source citée, et ce que les sources ne disent pas : [Secure mailboxes and their record (en anglais)](https://aurelienpierre.github.io/sioul/dev/research/secure-mailboxes.html).

## Ce qui est protégé par défaut {#what-is-protected}

### Vos données restent à vous {#what-stays-on-your-computer}

Sioul tourne sur votre appareil. Il n’y a pas de serveur Sioul ni de compte chez nous ; Sioul n’envoie ni statistiques, ni rapports de plantage, ni vérification de mise à jour, et ne contacte jamais le développeur. Vos courriels, vos agendas et vos contacts ne voyagent qu’entre votre appareil et vos propres fournisseurs.

Tout ce que Sioul garde est dans des fichiers simples, dans vos propres dossiers, lisibles par d’autres programmes, et à vous de les copier, de les sauvegarder ou de les effacer :

| Quoi | Où, sous Linux |
|---|---|
| Les réglages, les expéditeurs que vous laissez entrer, sûrs, neutres, restreints ou bloqués | `~/.config/sioul/` |
| Les courriels (un Maildir par adresse), les agendas et les contacts (un fichier par élément), les brouillons, le temps passé, les factures, les médicaments, les SMS de votre téléphone et leurs médias (tous, scellés) et ceux écrits pour qu’il les envoie | `~/.local/share/sioul/` |
| Où chaque relève s’est arrêtée, où le Porche a été fermé, les prises que vous avez notées et le relevé de chaque prise, les nouvelles des sites, le journal des appels d’un agent d’IA, ce que l’IA a dit des courriels d’une adresse protégée, les appels filtrés par vos téléphones (un mois d’entre eux), les messages qu’ont apportés les notifications de votre téléphone pour les applications que vous envoyez à vos ordinateurs (une semaine d’entre eux) et les réponses que vous leur avez écrites depuis un ordinateur (une semaine) | `~/.local/state/sioul/` |
| Votre propre filtre à indésirables : ce que son apprentissage a lu de vos courriels, le modèle de langue qu’il a appris, sa table ; ce que vous avez dit indésirable ou non | `~/.local/share/sioul/spam/`, `~/.local/state/sioul/spam/` |
| Notes, projets, budgets, papiers, lettres scannées | votre dossier de notes, là où vous l’avez choisi |

Sous Windows, les dossiers de Sioul sont dans `%APPDATA%\Sioul` ; sous macOS, dans `~/Library/Application Support/Sioul`. Sous Linux et macOS, Sioul rend ses propres dossiers accessibles à vous seul chaque fois qu’il démarre : les autres comptes du même ordinateur ne peuvent pas les ouvrir. Votre dossier de notes garde les droits que vous lui avez donnés.

**Les mots de passe**, l’accès à Google, les clés et les jetons des services que vous activez, et les phrases de passe vont dans le trousseau de votre système : GNOME Keyring ou KWallet sous Linux, le Gestionnaire d’identification sous Windows, le Trousseau d’accès sous macOS ; sur un téléphone, chiffrés avec une clé que garde le KeyStore d’Android. Jamais dans un fichier en clair.

Sioul ne chiffre pas lui-même les fichiers de votre appareil : votre système le fait mieux, pour tous les programmes à la fois. Si son chiffrement du disque n’est pas encore activé, activez-le : BitLocker ou le chiffrement de l’appareil sous Windows, FileVault sous macOS, le chiffrement que propose l’installateur de votre Linux. Les téléphones Android chiffrent déjà leur stockage.

### Chaque connexion chiffrée {#every-connection-encrypted}

Vos courriels, vos agendas, vos contacts, votre coffre Bitwarden et les recherches de clés de chiffrement voyagent toujours chiffrés, et le certificat de chaque serveur est vérifié avec ceux de votre système. Aucun réglage ne permet de les envoyer en clair. Le mot de passe de vos agendas et de vos contacts ne va qu’aux serveurs de votre fournisseur : une adresse sur un autre serveur, donnée dans leurs réponses, n’est jamais demandée. La connexion à Google se fait sur la page de Google, dans votre navigateur habituel, avec ce que Google y demande (votre mot de passe, votre clé de sécurité, une clé d’accès) : Sioul ne voit jamais votre mot de passe Google, et reçoit la réponse de Google sur votre seul appareil.

### Chaque courriel vérifié {#every-message-checked}

Quand un courriel arrive, Sioul le vérifie lui-même, par le DNS de votre système :

- **SPF**, sur le serveur qui a remis le courriel à votre fournisseur ;
- **DKIM**, chaque signature contre la clé que publie son domaine, vérifiée à l’arrivée, car les clés changent ;
- **DMARC**, avec la politique du domaine lui-même ;
- **ARC**, pour les courriels transférés, et le **DNS inverse** du serveur d’envoi.

Les résultats sont gardés avec le courriel, sous un nom que seule votre copie de Sioul utilise, pour qu’aucun expéditeur ne puisse écrire des résultats qui passeraient pour ceux de Sioul. Un courriel est alors **vérifié**, **non vérifié** (avec la raison), ou **falsifié**. Une signature se porte garante de l’expéditeur quand elle vient de sa propre organisation : `mail.example.org` signe pour `shop.example.org`, tandis que deux noms sous un suffixe partagé par de nombreux propriétaires, comme `github.io`, restent étrangers l’un à l’autre (la liste publique des suffixes, la Public Suffix List, dit lesquels). Les courriels falsifiés sont mis de côté, avec la raison ; rien n’est supprimé. Les verdicts de votre fournisseur sur les indésirables comptent aussi, pour les courriels d’inconnus seulement ([plus bas](#your-own-spam-filter)).

**Une adresse copiée ne suffit pas.** Un courriel que rien n’authentifie (il échoue à SPF et à DKIM, et rien ne s’en porte garant) est traité comme celui d’un inconnu, quelle que soit l’adresse qu’il affiche. Quelqu’un qui écrit depuis l’adresse de votre banque, ou celle d’un ami, n’obtient ni leur place sur le Porche, ni leurs heures, ni leur protection contre le filtre à indésirables.

**Les noms empruntés** sont repérés : un expéditeur qui prend le nom d’une banque, d’un service public ou de votre propre domaine, depuis une adresse qui ne leur appartient pas, est mis de côté. Une marque dont le nom est aussi un mot courant ou un lieu (Apple, Amazon, Orange, La Poste…) ne compte que seule ou à côté des mots d’un service, comme « Apple Support », « Amazon.com » ou « Service client Orange », si bien que « Orange County Library » ou « Café de la Poste » n’empruntent rien ; toute autre marque compte où qu’elle soit dans le nom. Les lettres qui se ressemblent (un l minuscule pour un I majuscule, un 0 pour un O, des lettres cyrilliques) ne cachent pas le nom. Les *domaines* qui en imitent d’autres, comme le nom d’une banque avec une lettre changée, ne sont pas encore repérés.

**Les codes que vous demandez** arrivent tout de suite, à toute heure. Si leur expéditeur n’est pas vérifié, Sioul dit de ne s’en servir que si vous venez d’en demander un à ce site, puisque les faux codes sont une ruse d’hameçonnage ; un code falsifié est mis de côté.

### Lire sans risque {#reading-safely}

**Les courriels HTML** sont montrés avec leurs paragraphes, leurs listes, leur texte en gras et leurs liens seulement : pas d’images, pas de styles, pas de scripts, pas de formulaires. Rien ne se charge depuis le réseau, jamais, et aucun réglage ne le permet : aucune image de pistage n’apprend que vous avez ouvert un courriel. Sioul n’envoie jamais d’accusé de lecture non plus.

**Les noms et les titres** qui viennent avec vos courriels et vos agendas (le nom d’un expéditeur, un objet, le titre d’un événement d’un agenda partagé ou d’une invitation, le nom d’un contact ou d’un dossier) sont montrés comme de simples mots, partout où ils paraissent : la balise d’une image cachée dans l’un d’eux s’affiche telle qu’elle est écrite, et ne charge rien. Un courriel, si grand ou si étrangement construit soit-il, s’ouvre à part, la fenêtre le disant en attendant, et ne la bloque jamais.

**Les liens** montrent leur adresse complète avant de s’ouvrir : sous le pointeur, ou après un premier toucher sur un écran tactile. Seuls les liens web et de courriel s’ouvrent. Un lien de courriel commence un courriel vers sa seule adresse, sans l’objet ni le texte qu’il remplirait pour vous.

### Les pièces jointes et l’antivirus {#attachments-and-the-antivirus}

Une pièce jointe est vérifiée avant de s’ouvrir ou d’être enregistrée, par l’antivirus de votre système :

- **Sous Linux et macOS** : ClamAV, quand il est installé ([Installer](install.md#the-packages)).
- **Sous Windows** : Microsoft Defender, par l’Antimalware Scan Interface.

Quand une menace est trouvée, rien ne s’ouvre, et la copie est effacée. Un programme, un script, un raccourci ou un installateur n’est jamais lancé depuis un courriel, même vérifié : enregistrez-le si vous lui faites confiance. Sous Windows et macOS, un fichier que vous ouvrez ou enregistrez depuis un courriel porte la marque du système qui dit qu’il vient d’Internet, pour qu’Office l’ouvre en mode protégé et que macOS le vérifie avant qu’il se lance. Les pièces jointes des courriels mis de côté ne s’ouvrent pas du tout.

Sans antivirus, Sioul ne refuse pas : il dit que le fichier ne sera pas vérifié, demande avant de l’ouvrir (**L’ouvrir sans vérification**), et dit comment en avoir un. Un fichier plus gros que ce que l’antivirus examine (ClamAV s’arrête à ses limites sur la taille d’un fichier et sur ce qu’une archive contient) n’est jamais dit sain : Sioul dit qu’il n’a pas été examiné parce qu’il est trop gros, et demande de la même façon. Un téléphone n’a pas d’antivirus à qui Sioul puisse demander : les pièces jointes n’y sont pas vérifiées, et Sioul le dit plutôt que de demander chaque fois. L’application que vous choisissez lit ce seul fichier et rien d’autre de Sioul, et les installateurs d’Android ne s’ouvrent jamais.

### Votre filtre à indésirables {#your-own-spam-filter}

Le propre filtre à indésirables de Sioul apprend sur votre ordinateur, quand vous appuyez sur **Entraîner maintenant** et de lui-même une fois par semaine quand cet ordinateur est branché et inactif, de vos propres courriels ([Courriels, Votre filtre à indésirables](mail.md#your-own-spam-filter)) :

- **Ce que l’apprentissage lit** : chaque dossier de chaque adresse (une adresse Microsoft seulement une fois nommée sous **Entraîner maintenant** lors de votre dernier appui, une adresse professionnelle ou scolaire jamais), sauf la corbeille, les brouillons et les envoyés (dans « Tous les messages » de Gmail, seulement les courriels archivés qu’aucun autre dossier ne contient, jamais les vôtres), sans rien changer sur le serveur. De chaque courriel, il garde les en-têtes, le nom et le type de ses parties et le début de son texte, jamais les pièces jointes elles-mêmes ; pour vérifier son expéditeur comme Sioul vérifie les courriels qu’il garde, il lit chaque courriel de 2 Mo au plus en entier, en mémoire seulement, n’en garde que les résultats, et interroge le DNS de votre système sur le domaine de l’expéditeur. Ce qu’il garde reste sur cet ordinateur (`~/.local/share/sioul/spam/corpus/`), pour que les indésirables restent connus après que votre fournisseur a vidé son dossier des indésirables.
- **Ce qu’il apprend** : un modèle de langue des mots de vos courriels, gardé à côté (`language.bin`). Il contient les mots de vos courriels en clair : il ne quitte jamais cet ordinateur.
- **Ce qui voyage** : sa table, le résultat, vers vos autres appareils par votre dossier, scellée comme tout ce qui s’y trouve : des nombres, les mots en empreintes, jamais en clair ; et ce que vous avez dit, plus bas. Un téléphone n’apprend jamais : il lit cette table.
- **Ce que vous avez dit** : **Indésirable**, **Pas indésirable** et bloquer un expéditeur depuis un courriel ajoutent une ligne au relevé de cet appareil (`~/.local/state/sioul/spam/labels/`), chaque courriel que le filtre déplace dans un dossier Indésirables une autre (`~/.local/state/sioul/spam/moved/`), et chaque courriel qu’il signale là où il est une autre encore (`~/.local/state/sioul/spam/flagged/`) : quand, où, quel courriel par son numéro et son Message-ID, indésirable ou non ; jamais un mot de lui. Chaque appareil garde le sien ; ils voyagent scellés vers vos autres appareils, pour qu’un courriel dont vous avez dit qu’il n’était pas indésirable sur l’un ne soit plus jamais signalé sur un autre.
- **L’apport extérieur**, si vous en importez (`sioul spam import`) : les objets et les textes de courriels étiquetés ailleurs, gardés sur cet ordinateur seulement, à part de vos courriels (`~/.local/share/sioul/spam/external/`), à vous seul, jamais partagés ; `sioul spam import --remove` les retire.
- **Ce qu’il demande à votre ordinateur**, pour se réentraîner de lui-même : s’il est branché, en économie d’énergie, inactif ou verrouillé, et si votre connexion est limitée, aux services du système ; demandé sur cet ordinateur, gardé nulle part, envoyé nulle part.
- **Rien à personne d’autre** : aucun serveur, aucune IA de Sioul. Ses chiffres ne sont que des totaux ; les mots qui ont pesé dans un courriel sont pour vous, dans votre propre terminal (`sioul spam why`), jamais proposés à un agent d’IA. Un agent d’IA que vous connectez vous-même peut se servir des outils du filtre, qui listent ses pires erreurs par expéditeur et par objet, les codes et les numéros de compte masqués, sauf si vous gardez ces outils loin de lui ([Avec un agent d’IA](ai-agent.md#the-spam-filters-tools)).

Pour tout oublier, supprimez `~/.local/share/sioul/spam/` sur cet ordinateur : sa table, partagée, quitte alors aussi vos autres appareils.

### Les sites, Bitwarden et les clés de sécurité {#sites}

Une clé de sécurité ne se laisse pas tromper par une fausse page : elle ne répond qu’au site pour lequel elle a été faite. C’est pourquoi Sioul vous permet d’en utiliser une partout où vous vous connectez.

Les sites que vous épinglez vivent dans un profil de navigateur propre à Sioul, à part de votre navigateur habituel. Leurs notifications sont gardées par Sioul. Le micro, la caméra et le partage de l’écran sont permis site par site, par vous ; l’endroit où vous êtes n’est jamais donné, ni la liste des polices de votre ordinateur, dont les sites se servent pour distinguer les ordinateurs. Les téléchargements vont dans votre dossier de téléchargements.

Un site qui demande une clé de sécurité l’obtient (FIDO2 et WebAuthn : une YubiKey, une Nitrokey), de même que les clés d’accès qu’elle garde ; le code PIN de la clé est demandé dans la boîte de dialogue propre à Sioul. Les clés d’accès gardées dans un téléphone ou dans le système ne fonctionnent pas dans les sites de Sioul sous Linux et macOS.

Les identifiants viennent de votre coffre Bitwarden, ouvert par Sioul lui-même, en lecture seule : rien n’y est jamais écrit, votre mot de passe principal et vos clés ne sont jamais gardés, et les identifiants restent en mémoire jusqu’à la fermeture de Sioul. Depuis la page Sites, le coffre peut s’ouvrir avec votre clé de sécurité, comme deuxième étape, ou seule quand Bitwarden la connaît comme clé d’accès. Un identifiant fait pour un autre domaine le dit, pour qu’un site qui en imite un autre se remarque. Voir [Les sites](sites.md).

Sur un téléphone, les sites s’ouvrent dans votre navigateur, avec ses propres protections, et Sioul n’y garde aucun identifiant.

### Les courriels chiffrés {#encrypted-mail}

Avec OpenPGP, Sioul signe et chiffre vos courriels quand vous les envoyez, déchiffre et vérifie ceux que vous recevez, et donne votre clé publique aux personnes à qui vous écrivez (Autocrypt). Il ne garde la clé d’un expéditeur que depuis un courriel qu’il a vérifié, pour qu’un courriel falsifié ne puisse pas glisser une clé pour quelqu’un à qui vous écrivez. Il ne cherche la clé de quelqu’un sur le réseau que quand vous le demandez (**Chercher leurs clés**), puisque cela dit à un serveur à qui vous écrivez.

Vos clés secrètes restent sur l’appareil où elles ont été créées. Une clé gardée sur une [clé de sécurité](accounts.md#security-key) ne la quitte jamais : la clé signe et ouvre elle-même, après son code PIN, que Sioul garde en mémoire quinze minutes sans usage et n’écrit ni ne journalise jamais nulle part. Sioul se contente d’utiliser la clé : il ne demande jamais son code PIN d’administration et ne change jamais ce qu’elle contient. Voir [Les courriels](mail.md#signing-and-encrypting).

Quand vous répondez à un courriel chiffré ou le transférez, la copie que Sioul garde avec votre brouillon reste chiffrée, telle qu’elle est arrivée. Elle ne s’ouvre qu’au moment où vous appuyez sur **Envoyer**, en mémoire, avec le même code PIN que la signature, et sa citation part chiffrée. Envoyée sans chiffrement, votre réponse part sans la citation, et un transfert ne part pas : la fenêtre le dit dès que vous décochez **Chiffrer**. Sur un appareil où aucune de vos clés ne l’ouvre, comme un téléphone pour l’instant, le brouillon le dit, la copie reste chiffrée, et une réponse envoyée de là part sans la citation.

L’objet d’un courriel chiffré reste lisible, comme dans la plupart des logiciels de courriel aujourd’hui.

### Entre vos appareils {#between-your-devices}

Chaque appareil garde ses propres données. Ce qui doit voyager entre vos ordinateurs et votre téléphone passe par un dossier que votre propre application de synchronisation transporte (Nextcloud, Dropbox, Syncthing, Google Drive, OneDrive…), scellé sur votre appareil avant d’y être écrit, avec une clé tirée de votre phrase de passe. Il n’y a aucun serveur à nous entre les deux. Le serveur du dossier voit quel appareil a écrit, quand et combien, et la taille de chaque note ou papier scellé ; jamais quoi. Rien de ce qui arrive dans ce dossier, un fichier abîmé ou effacé, ne peut retirer des données à vos appareils. Les mots d’une notification ne voyagent que si vous activez **Messages de votre téléphone**, pour les applications que vous choisissez, une semaine au plus ([Sur vos ordinateurs](notifications.md#messages-on-your-computers)). Comment cela marche, ce que cela protège et ce que cela ne peut pas cacher : [Partager entre vos appareils](sharing.md).

### Les images envoyées par SMS {#pictures-sent-by-text}

Une image que vous joignez à un SMS sur un ordinateur y est réécrite, depuis ses seuls pixels, avant que rien n’en parte ([SMS](texts.md#pictures-and-files)).

- **Elle garde** ce qu’elle montre : tournée dans le sens où l’appareil était tenu, en couleurs standard (sRGB), de 1 600 pixels au plus sur son grand côté, en JPEG.
- **Elle perd** tout ce qu’un appareil photo, un téléphone ou un logiciel de retouche écrit d’autre dans le fichier d’une image : la position GPS de la prise de vue, l’appareil ou le téléphone qui l’a prise, l’heure, les réglages, les commentaires, légendes et mots-clés (EXIF, XMP, IPTC, les textes d’un PNG), et son profil de couleur. Le nom de son fichier ne part jamais non plus : le message l’appelle `image1.jpg`.
- **Un GIF animé** ne garde que ses images : ses commentaires et les blocs d’autres logiciels (XMP, un profil de couleur) s’en vont.
- **Une vidéo, un son ou une fiche de contact** part tel quel, avec tout ce que son fichier contient (une vidéo peut contenir le lieu où elle a été filmée, un son ses étiquettes) : n’en joignez qu’à qui vous l’enverriez par courriel.

Elle est ensuite scellée comme tous les médias de vos SMS, passe par votre propre partage jusqu’à votre téléphone, et votre téléphone l’envoie par votre opérateur au téléphone de l’autre personne : votre opérateur et le sien la transportent, comme tout message multimédia.

### L’IA, seulement si vous la demandez {#ai-only-if-you-ask}

Sioul ne connecte aucune IA de lui-même.

Un agent d’IA que vous connectez, comme Claude Code, peut lire ce que vous lui ouvrez sur cet appareil et préparer des tâches, des événements, des notes et des brouillons : chaque projet est fermé aux agents jusqu’à ce que vous l’ouvriez, et ce qui n’est dans aucun projet peut être fermé aussi ([Choisir ce qu’un agent voit](ai-agent.md#choosing-what-an-agent-sees)). Il ne peut ni envoyer, ni supprimer, ni déplacer d’argent, ni lire un mot de passe, et les codes à usage unique, les numéros de carte et de compte bancaire lui sont cachés ; l’entreprise derrière lui reçoit ce qu’il lit. Il peut aussi se servir des outils de votre propre filtre à indésirables, qui lisent vos serveurs de courriel sans rien y changer ; vous pouvez garder ces outils loin des agents. Voir [Avec un agent d’IA](ai-agent.md).

Pour une adresse que vous protégez contre le harcèlement, **Laisser l’IA le lire d’abord** reste désactivé tant que vous ne l’activez pas. Sioul envoie alors à Anthropic, avec votre propre clé, chaque courriel de la boîte de réception de cette adresse, de n’importe qui, y compris ceux qui s’y trouvent déjà : son objet et les 4 000 premiers caractères de son texte, **sans rien masquer**. Contrairement à ce que reçoit un agent, rien n’y est caché : un code ou un numéro de compte dans un tel courriel parvient aussi à Anthropic. Voir [le Porche](porch.md#a-public-address-protected).

### Une adresse publique {#a-public-address}

Une adresse que vous publiez peut être protégée contre le harcèlement : ses courriels sont lus sur votre appareil avant que vous les voyiez, et les courriels hostiles sont mis de côté sans que leurs mots soient montrés. Voir [le Porche](porch.md#a-public-address-protected).

### Votre serveur de courriel {#your-mail-server}

Relever ne change rien sur le serveur : aucun courriel n’est marqué lu parce qu’il a été relevé. Sioul n’y écrit que quand vous agissez : ouvrir un courriel le marque lu, comme le fait tout logiciel de courriel ; archiver, supprimer, mettre en indésirable et déplacer se font dix secondes après votre demande, pour qu’**Annuler** puisse les arrêter. Les courriels ne partent que quand vous les envoyez, ou quand vous demandez à une liste de diffusion de vous laisser partir. Retirer un compte retire son mot de passe, pas ses courriels.

## Ce qui sort de votre appareil, et quand {#what-leaves-your-computer-and-when}

| Vers | Quoi | Quand |
|---|---|---|
| Votre fournisseur de courriel | vos courriels : relevés, rangés comme vous le demandez, envoyés quand vous envoyez | toujours : ce sont vos courriels |
| Votre serveur d’agendas et de contacts, ou Google | vos agendas, tâches et contacts, dans les deux sens | toujours |
| Votre opérateur mobile, par votre téléphone | un SMS ou un message multimédia que vous envoyez depuis un ordinateur, ses images réécrites sans leurs détails | quand vous en envoyez un |
| Les sites que vous épinglez | ce que tout navigateur leur envoie | quand ils sont ouverts |
| Les réglages publiés par votre fournisseur, et la liste de fournisseurs de Thunderbird | le domaine de votre adresse, pour trouver le serveur | quand vous ajoutez un compte |
| Le résolveur DNS de votre système | des requêtes sur les domaines des expéditeurs, pour les vérifier | quand des courriels arrivent, et quand l’apprentissage de votre filtre à indésirables télécharge vos courriels |
| L’adresse de désabonnement d’une liste de diffusion | une demande « en un clic », sans rien de vous que ce que la liste y a écrit ; ou un bref courriel à cette adresse, depuis la vôtre ; ou sa page, dans votre navigateur | seulement quand vous choisissez **Se désabonner** |
| Open-Meteo | les coordonnées du lieu choisi, arrondies à deux décimales, ou le nom d’une ville que vous cherchez | seulement si vous choisissez un lieu pour la météo ; toutes les demi-heures au plus |
| Le géocodeur d’OpenStreetMap (Nominatim) | les adresses postales de vos contacts, une fois chacune, rien d’autre | seulement après que vous avez choisi **Les placer** |
| Les images de carte d’OpenStreetMap, ou celles que vous avez choisies | quelle partie de la carte est montrée | tant que la carte est ouverte |
| Le Web Key Directory du domaine des destinataires, puis keys.openpgp.org | les adresses auxquelles vous écrivez | seulement quand vous choisissez **Chercher leurs clés** |
| L’adresse écrite sur votre clé de sécurité, l’annuaire de clés de votre domaine, puis keys.openpgp.org | l’empreinte de votre clé de sécurité, et le domaine de vos adresses | seulement quand vous choisissez **La chercher** ou **Chercher une version plus récente** |
| Chaque site que vous épinglez | une demande de sa propre icône | environ une fois par semaine |
| Anthropic | l’objet et les 4 000 premiers caractères du texte de chaque courriel d’une adresse que vous protégez, sans rien masquer | seulement avec **Laisser l’IA le lire d’abord**, et votre propre clé |
| L’agent d’IA que vous connectez, et son fournisseur | ce qu’il lit | seulement si vous en connectez un ([Avec un agent d’IA](ai-agent.md)) |
| GitHub | votre jeton ; des recherches de vos tickets et pull requests | seulement si vous activez GitHub |
| Votre serveur Bitwarden, et sa propre page de clé de sécurité quand votre clé est votre deuxième étape | votre connexion, pour ouvrir votre coffre | quand vous remplissez un identifiant depuis lui |
| Votre Nextcloud (celui de Murena compris) | les fichiers scellés du dossier de partage | seulement si Sioul tient ce dossier à jour lui-même, ou va l’y chercher en secours |
| Les serveurs de mise à jour de ClamAV | une demande de signatures de virus | une fois par jour, seulement quand votre système n’en garde aucune lui-même |

Sioul n’a ni mesure d’audience, ni publicité, ni rapports de plantage, ni vérification de mise à jour.

## Ce que Sioul ne peut pas protéger {#what-sioul-cannot-protect}

Dire où la protection s’arrête en fait partie :

- **Votre boîte de courriel reste chez votre fournisseur.** Sioul est un logiciel de courriel : vos courriels, vos agendas et vos contacts restent sur les serveurs de vos fournisseurs, et ce que ces serveurs peuvent lire dépend d’eux. Seuls les courriels que vous chiffrez avec OpenPGP leur sont illisibles, leur objet mis à part. Choisissez un fournisseur en qui vous avez confiance.
- **Votre appareil est protégé par votre système.** Les fichiers de Sioul sont de simples fichiers, pour que vous et d’autres programmes puissiez les lire ; le chiffrement du disque de votre système et le mot de passe de votre session gardent les autres dehors.
- **Ce qu’un disque garde d’un fichier remplacé.** Jusqu’au 19 octobre 2026, répondre à un courriel chiffré gardait son texte déchiffré avec le brouillon, sur chaque appareil où le brouillon arrivait. Sioul chiffre maintenant ces copies à nouveau et écrase les anciennes, mais les SSD, les téléphones et certains systèmes de fichiers (Btrfs, APFS) gardent les anciens blocs jusqu’à leur réemploi, et les sauvegardes gardent ce qu’elles ont pris : c’est le chiffrement du disque de votre système qui les rend illisibles ([Vos données restent à vous](#what-stays-on-your-computer)).
- **Le dossier entre vos appareils en dit un peu.** Son serveur voit quel appareil a écrit, quand et combien ([le détail](sharing.md#what-it-protects-and-what-it-cannot-hide)).
- **Un téléphone a moins de protections.** Sur Android, il n’y a ni antivirus pour vérifier les pièces jointes ni clé de sécurité, et les sites s’ouvrent dans votre navigateur.
- **Les domaines qui en imitent d’autres ne sont pas encore repérés**, seulement les noms empruntés, et la liste des marques est fixe : surtout des services français et de grandes plateformes.
- **Un agent d’IA que vous connectez lit ce que vous lui ouvrez** sur cet appareil (les projets que vous ouvrez, et ce qui n’est dans aucun projet sauf si vous le fermez), jamais les mots de passe, les codes, les numéros de compte, les médicaments et les prises, et l’entreprise derrière lui reçoit ce qu’il lit.
- **La lecture par l’IA d’une adresse protégée envoie le début de chaque courriel tel quel** : son objet et ses 4 000 premiers caractères, codes et numéros de compte compris.
- **Certaines protections n’ont pas encore été essayées sur du vrai matériel** : l’antivirus sous Windows ; les clés de sécurité dans les sites et pour votre coffre Bitwarden ; votre clé OpenPGP sur une clé de sécurité sous Windows et macOS (elle a été essayée avec une vraie clé sous Linux) ([Côté technique](#for-technical-readers)).
- **Les paquets pour Windows et macOS ne sont pas encore signés** : votre système avertit la première fois ([Installer](install.md)).

## Ce site web {#this-website}

Ce site web n’a ni mesure d’audience, ni publicité, ni cookies, et ne charge rien depuis d’autres sites. Il est hébergé par GitHub Pages, qui garde ses propres journaux de serveur, selon [la déclaration de confidentialité de GitHub (en anglais)](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

## Signaler un problème {#reporting-a-problem}

Un problème de sécurité, ou tout ce qui vous inquiète : [les tickets GitHub](https://github.com/aurelienpierre/sioul/issues).

## Comparé à d’autres applications {#compared-with-other-apps}

En octobre 2026, d’après la documentation de chaque application.

✓ documenté · en partie, avec une note · ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) · ? pas confirmé · — sans objet.

Chaque colonne est tout un ensemble d’outils : Thunderbird avec Firefox et son gestionnaire de mots de passe ; Google, Gmail et Workspace avec Chrome et le Gestionnaire de mots de passe de Google ; Microsoft, Outlook et Microsoft 365 avec Edge et Defender pour Office 365 ; Proton, ses Mail, Calendar, Drive et Pass avec Bridge ; Nextcloud, Nextcloud Hub avec Mail et Calendar. Les logiciels de courriel seuls sont comparés sur [la page Courriels](mail.md#compared-with-other-apps).

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
    6. Sioul vérifie lui-même SPF, DKIM, DMARC, ARC et le DNS inverse, à l’arrivée de chaque courriel, et dit sur chaque courriel s’il est vérifié, non vérifié ou falsifié.
    7. Thunderbird : seulement par une extension.
    8. Gmail : un point d’interrogation sur les courriels qui ne sont pas authentifiés ; « Envoyé par » et « Signé par » dans les détails d’un courriel, sur un ordinateur et sur Android.
    9. Outlook : un « ? » quand il ne peut pas vérifier l’expéditeur, et « via » quand l’expéditeur réel est un autre.
    10. Proton Mail : un avertissement sur les courriels qui échouent aux vérifications de leur domaine ; rien n’est dit quand ils les passent.
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
    22. Thunderbird laisse l’antivirus de votre système mettre les nouveaux courriels en quarantaine ; il n’a pas d’analyseur à lui.
    23. Les fichiers exécutables bloqués et les pièces jointes analysées ; un bac à sable à partir de Business Standard.
    24. Activé par défaut pour chaque boîte ; l’exécution dans un bac à sable avec Defender pour Office 365.
    25. Proton Mail : sur ses serveurs, pour les courriels qui ne sont pas chiffrés de bout en bout ; Proton Drive compare les fichiers à une liste de fichiers malveillants connus.
    26. Les fichiers déposés sur Nextcloud, avec son application antivirus ; pas les pièces jointes des courriels.
    27. Ce que Sioul transporte entre vos appareils, sans serveur de Sioul ; vos courriels, vos agendas et vos contacts passent par vos propres fournisseurs, comme avec n’importe quel logiciel de courriel.
    28. Firefox Sync est de bout en bout ; Thunderbird n’a pas de synchronisation à lui.
    29. Ni Gmail ni Drive ; le chiffrement côté client dans Enterprise Plus, Education Standard et Plus, Frontline Plus ; les mots de passe une fois le chiffrement sur l’appareil mis en place.
    30. Microsoft détient les clés, sauf avec le Double Key Encryption (Microsoft 365 E5) ; Edge chiffre les mots de passe avant de les synchroniser.
    31. Mail, Calendar, Drive et Pass ; les objets et les adresses ne sont pas de bout en bout.
    32. Des dossiers vides choisis, depuis les applications de bureau et mobiles ; ni les agendas, ni les contacts, ni les courriels.
    33. Dans de simples fichiers ; vos courriels aussi chez votre fournisseur ; le dossier de partage scellé, dans le cloud que vous choisissez.
    34. Le profil de Thunderbird ; vos courriels chez votre fournisseur ; les données de Firefox Sync chez Mozilla, chiffrées.
    35. La frontière des données de l’UE pour les organisations inscrites dans l’UE ou l’AELE.
    36. Proton AG, en Suisse.
    37. Ou celui de votre hébergeur ; vos courriels chez votre fournisseur de courriel.
    38. Chromium, sur lequel Chrome est construit, est ouvert.
    39. Ses applications, auditées par des tiers ; pas ses serveurs.

=== "Technique"

    | | Sioul | Thunderbird | Google | Microsoft | Proton | Nextcloud |
    |---|---|---|---|---|---|---|
    | Votre clé de courriel sur une clé de sécurité ou une carte à puce | en partie¹ | en partie² | en partie³ | ?⁴ | ✗⁵ | ✗ |
    | OpenPGP intégré | ✓⁶ | ✓⁷ | ✗ | ✗ | ✓ | en partie⁸ |
    | Clés de sécurité et clés d’accès pour se connecter | en partie⁹ | ✓¹⁰ | ✓¹¹ | ✓ | ✓¹² | ✓¹³ |
    | Gestionnaire de mots de passe | ✓¹⁴ | ✓ | ✓ | ✓ | ✓¹⁵ | en partie¹⁶ |
    | S/MIME | ✗ | ✓ | en partie¹⁷ | ✓ | ✗ | ✓ |
    | Courriels stockés illisibles pour le fournisseur | en partie¹⁸ | en partie¹⁸ | en partie¹⁹ | en partie²⁰ | ✓²¹ | en partie¹⁸ |

    1. Sioul : une carte OpenPGP (YubiKey, Nitrokey) signe et ouvre, son code PIN gardé en mémoire seulement, jamais écrit ni journalisé ; sur les ordinateurs ; essayée avec une vraie clé, une YubiKey 5, sous Linux (8 octobre 2026).
    2. Thunderbird : par GnuPG, que vous installez et réglez vous-même.
    3. Google : une carte à puce PIV garde la clé S/MIME, avec le chiffrement côté client, sous Windows, dans les éditions les plus hautes de Workspace avec Assured Controls.
    4. Microsoft : une carte à puce pour le S/MIME d’Outlook n’est pas confirmée dans les pages lues.
    5. Proton : sa documentation sur les clés ne mentionne aucune carte à puce ni clé de sécurité.
    6. Sioul : avec Autocrypt (clés prises seulement dans les courriels vérifiés) et le Web Key Directory.
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
    18. Seulement les courriels que vous chiffrez (OpenPGP, ou S/MIME là où il est proposé) ; le reste demeure comme votre fournisseur le garde.
    19. Google : le chiffrement côté client dans certaines éditions de Workspace ; pas Gmail sinon.
    20. Microsoft : S/MIME, et le Double Key Encryption dans Microsoft 365 E5.
    21. Proton : le chiffrement à accès zéro de chaque courriel stocké.

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

- **La vérification des expéditeurs** : `mail-auth`, de Stalwart, à l’arrivée, par le résolveur du système : SPF (RFC 7208) sur le saut où le courriel est entré chez votre fournisseur, DKIM (RFC 6376), DMARC (RFC 7489), ARC (RFC 8617), iprev. Les résultats sont ajoutés en tête comme `Authentication-Results` (RFC 8601) sous un authserv-id propre à l’installation (`sioul-….invalid`) ; les résultats d’un fournisseur ne comptent que sous son identifiant, appris dans ses courriels. Vérifié : DMARC passe, sinon une signature DKIM valide du domaine de De. Falsifié : DMARC échoue sous `p=quarantine` ou `p=reject`. Non authentifié : SPF et DKIM échouent, DMARC ne passe pas, aucun ARC scellé par votre fournisseur ou vos domaines ; un tel expéditeur est un inconnu pour chaque règle du Porche. Plus de détails : [Courriels, comment un expéditeur est vérifié](mail.md#how-a-sender-is-checked).
- **Les noms empruntés** : les squelettes de confusables d’Unicode (UTS #39), comparés à une cinquantaine de marques et de services publics et à vos propres domaines.
- **Le HTML** : une liste d’ammonia des balises de texte permises ; `href` seul attribut ; http, https et mailto seuls schémas ; les liens relatifs refusés ; pas de `<img>`, de style ni de script. Le site d’un lien est lu après tout `nom@` placé devant pour tromper.
- **Les pièces jointes** : ClamAV (`clamdscan --fdpass`, sinon `clamscan` ; les signatures rafraîchies chaque jour avec `freshclam` quand le système n’en garde aucune) ou AMSI ; les programmes seulement enregistrés ; les noms débarrassés de tout dossier, et sous Windows des noms de périphériques et des flux ; la Mark of the Web (`Zone.Identifier`, ZoneId=3) et `com.apple.quarantine` sur ce qui est ouvert ou enregistré.
- **Le transport** : rustls avec *ring*, TLS 1.2 et 1.3, les certificats racines du système (ceux de Mozilla quand un système n’en a pas) ; IMAP et SMTP en TLS dès le premier octet (RFC 8314) ou en STARTTLS, sans option en clair, et un serveur sans STARTTLS refusé ; CalDAV, CardDAV, Bitwarden et les recherches de clés en HTTPS seulement. SMTP se présente comme `[127.0.0.1]`, pour que le nom de votre ordinateur reste hors des lignes `Received`. Google : OAuth 2.0 pour les applications natives (RFC 8252) sur un port local tiré au hasard, PKCE S256 (RFC 7636), l’état comparé en temps constant ; le jeton de renouvellement dans le trousseau, le jeton d’accès en mémoire.
- **Le partage** : XChaCha20-Poly1305 avec un nonce aléatoire de 192 bits par enregistrement, chacun lié à son appareil, son tour, sa place et son horloge comme données associées. La clé vient d’Argon2id (64 Mio, 3 passes, un sel de 16 octets), et le trousseau de chaque appareil garde la clé, pas la phrase de passe. Les notes et les papiers sont compressés en gzip et scellés en morceaux de 1 Mio sous une sous-clé HKDF-SHA-256, nommés par un HMAC-SHA-256 de leur contenu. Le détail : [Le partage, côté technique](sharing.md#for-technical-readers).
- **OpenPGP** : Sequoia, cryptographie en pur Rust, politique standard ; RFC 9580, PGP/MIME (RFC 3156), le PGP en ligne lu. Les nouvelles clés sont en version 4 sur Curve25519 (EdDSA, ECDH), valables trois ans. Les courriels chiffrés sont signés à l’intérieur, pour chaque destinataire et pour vous. Autocrypt envoie votre clé et ne prend de clés que dans les courriels vérifiés. « Chercher leurs clés » interroge le Web Key Directory, puis keys.openpgp.org, en HTTPS. Essayé dans les deux sens avec GnuPG 2.4.
- **OpenPGP sur une clé de sécurité** : carte OpenPGP 3.4 (YubiKey, Nitrokey) par PC/SC, en mode partagé, une transaction par opération, la carte remise à zéro ensuite. Chaque signature est vérifiée avec le certificat avant usage, et un certificat n’est accepté que s’il contient les propres clés publiques de la carte. Le code PIN est gardé en mémoire seulement (le `Password` de Sequoia), oublié après 15 minutes sans usage, quand la clé est retirée, ou à la fermeture. Aucun journal n’est installé, et la journalisation est plafonnée au démarrage de chaque programme, pour que les traces de la bibliothèque de la carte, qui contiendraient le code PIN, ne soient jamais écrites. Jamais le code PIN d’administration ; rien n’est écrit sur la carte.
- **Les sites** : le WebAuthn de Chromium dans Qt WebEngine (FIDO2, CTAP2 par USB, clés d’accès sur la clé), avec la boîte de dialogue de Sioul pour le compte et le code PIN ; un profil à lui ; la position, les polices locales et le verrouillage du pointeur refusés ; aucune erreur de certificat jamais acceptée par Sioul.
- **Bitwarden** : le client propre à Sioul, en lecture seule. PBKDF2-SHA256 ou Argon2id selon ce que dit votre compte, tenus dans les limites de Bitwarden lui-même pour qu’un serveur ne puisse pas rendre votre mot de passe moins coûteux à deviner ; la connexion envoie une empreinte, jamais le mot de passe ; étirement par HKDF, AES-256-CBC avec HMAC-SHA256 (le MAC vérifié d’abord, en temps constant) ou COSE, les clés d’organisation par RSA-OAEP. Les clés vivent dans une mémoire effacée après usage, et le trousseau ne garde que le jeton de l’appareil. La deuxième étape peut être une clé FIDO2, par la page de clé de sécurité de Bitwarden lui-même ; la connexion par clé d’accès utilise l’extension PRF de WebAuthn ; les codes à usage unique suivent la RFC 6238.
- **Le trousseau** : le Secret Service, le Gestionnaire d’identification de Windows, le Trousseau d’accès de macOS, le KeyStore d’Android (AES-GCM, la clé ne quittant jamais le KeyStore).
- **Les fichiers** : les dossiers de Sioul en 0700, créés ou resserrés ainsi au démarrage de Sioul, et ses fichiers sensibles en 0600, sous Linux et macOS.
- **Les agents d’IA** : MCP par l’entrée et la sortie standard seulement, donc sans port réseau ouvert. Les codes, les liens de connexion, les IBAN (mod 97), les numéros de carte (Luhn) et les numéros de sécurité sociale français sont masqués ; les courriels et les notes arrivent encadrés comme des données ; chaque appel est noté par ses adresses et sa longueur, jamais par ses mots. Plus de détails : [Avec un agent d’IA](ai-agent.md#for-technical-readers).
- **L’IA du bouclier** : Claude Haiku 4.5 par l’API d’Anthropic, avec votre clé tirée du trousseau ; l’objet et les 4 000 premiers caractères de chaque courriel reçu à cette adresse, sans rien masquer ; aucune redirection suivie, pour que la clé n’aille qu’à Anthropic.
- **Aucune télémétrie** : aucun code de mesure d’audience, de rapport de plantage ni de vérification de mise à jour n’existe. Les seules destinations réseau du code sont celles du tableau plus haut.
- **Ce qui a été essayé** : l’antivirus sous Windows (AMSI) suit la séquence documentée par Microsoft et est construit par les compilations automatiques du projet, mais n’a pas encore tourné sous Windows ; la clé de sécurité OpenPGP est essayée avec une carte logicielle, et a été essayée avec une vraie clé, une YubiKey 5, sous Linux le 8 octobre 2026 (mise en place, renouvelée par GnuPG, signature et chiffrement à l’envoi, ouverture de courriels chiffrés) ; les clés de sécurité dans les sites n’ont pas encore été essayées avec une vraie clé ; la connexion à Google n’a été essayée qu’avec des doublures de Google.
- **Le code** est un logiciel libre, sous licence GPL-3.0-or-later, pour que tout cela puisse être vérifié : [github.com/aurelienpierre/sioul](https://github.com/aurelienpierre/sioul). Le paquet Android est signé avec la propre clé de Sioul ; l’installateur Windows n’est pas encore signé, et l’image macOS pas encore notariée.
