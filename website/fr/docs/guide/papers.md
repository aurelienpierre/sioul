---
description: Les papiers et le courrier papier dans Sioul – les papiers demandés encore et encore, où en est chacun et un rappel avant de le renouveler ; le courrier scanné, lu sur votre ordinateur et montré sous forme de carte calme.
---

# Les papiers et les lettres {#papers-and-letters}

## Les papiers {#papers}

Les mêmes papiers sont demandés encore et encore : une carte d’identité, le dernier avis d’impôt, un RIB, des quittances de loyer de moins de trois mois, une attestation de droits à l’Assurance Maladie. Un papier périmé bloque ce qui en dépend : un bail, une couverture, un voyage. Se souvenir qu’un passeport finit en mars, c’est ce type de mémoire qui fait le plus souvent défaut (Landsiedel, Williams & Abbot-Smith 2017). Sioul garde donc vos papiers, dit où en est chacun, et vous rappelle quand le renouvellement doit commencer.

<figure markdown="span">
  [![La page Papiers, avec Ajouter un papier : les papiers groupés par famille (Identité, Santé, Logement, Argent, Garanties), chacun avec son nom et où il en est, en mots, comme « valable jusqu’au vendredi 18 décembre : le moment de le renouveler » ; Prévoir le renouvellement à côté de ceux à renouveler, et Ouvrir à côté de chacun.](../assets/screens/fr/papers.png){ loading=lazy }](../assets/screens/fr/papers.png "Ouvrir l’image en grand")
  <figcaption>Les papiers, par famille, chacun avec où il en est.</figcaption>
</figure>

### Où en est chacun {#where-each-stands}

En mots, jamais en rouge :

- « valable jusqu’au 20 décembre » ;
- « valable jusqu’au 20 décembre : le moment de le renouveler » ;
- « fini le 3 mai » ;
- « du 30 juin, plus vieux que ce qui est demandé d’habitude », pour une quittance de loyer ou un bulletin de paie.

### Ajouter un papier {#adding-a-paper}

**Ajouter un papier** (ou **Nouveau ▾ ▸ Un papier**), puis :

- **Quoi** : une carte d’identité, un passeport, un titre de séjour, un permis de conduire, une carte Vitale ou une attestation de droits, une complémentaire santé, une attestation d’assurance, un avis d’impôt, une quittance de loyer, un RIB, un bulletin de paie, une attestation ou un certificat, une garantie, ou un autre papier ;
- **Nom** : « Passeport », « Quittance septembre » ;
- **Fichier** : un scan ou une photo, copié dans vos papiers ;
- **Délivré le** et **Valable jusqu’au**. Pour un passeport, une carte d’identité ou une garantie, une fin est proposée d’après la date de délivrance quand vous n’en donnez pas ;
- **À qui**, dans un foyer ;
- **Notes**.

Depuis un message, **Garder dans les papiers**, à côté d’une pièce jointe, fait de même : l’antivirus vérifie le fichier, il est gardé, et le formulaire s’ouvre avec un nom et un type devinés d’après le nom du fichier et l’objet du message. Vous ne dites que ce qui ne se devine pas.

Pour en envoyer un, la fenêtre d’écriture a **Un papier**, à côté de **Joindre**. Un papier fini, ou plus vieux que ce qui est demandé d’habitude, le dit dans la liste.

### Renouveler à temps {#renewing-in-time}

| Type | Le renouvellement commence |
|---|---|
| Carte d’identité, passeport | 3 mois avant la fin : un rendez-vous, puis la fabrication |
| Titre de séjour | 4 mois avant |
| Permis de conduire | 2 mois avant |
| Complémentaire santé | 3 mois avant : certaines ne se renouvellent pas d’elles-mêmes |
| Attestation d’assurance | 1 mois avant |
| Garantie, preuve d’achat | 1 mois avant |

Quittances de loyer, bulletins de paie et attestations n’ont pas de fin : passé trois mois, ils sont dits plus vieux que ce qui est demandé d’habitude. Pour un avis d’impôt, c’est d’habitude le dernier qui est demandé.

**Un rappel** vient une fois, quand le renouvellement commence, aux prochaines heures de travail : « Passeport : valable jusqu’au dimanche 20 décembre », avec ce que demande le renouvellement. Pas pour un papier ajouté une fois le renouvellement commencé (vous venez de le voir), ni une fois son renouvellement prévu.

**Prévoir le renouvellement** crée une tâche dans votre liste habituelle, pour que votre téléphone l’ait, du début du renouvellement au jour où le papier finit, liée au papier, avec les étapes connues pour son type.

### Où vivent les papiers {#where-papers-live}

`sioul-papers.toml` à la racine de votre dossier de notes, et les fichiers dans son dossier `papers` : ils voyagent avec vos notes. **Retirer** enlève un papier du portefeuille ; son fichier reste où il est.

## Le courrier papier {#paper-letters}

L’enveloppe reste dehors. Un scan, une photo prise au téléphone ou un PDF déposé dans un dossier, par vous, par un scanner, ou par une personne qui ouvre le courrier pour vous, est lu sur cet ordinateur et attend vos heures sous forme de carte, comme un courriel. Ouvrir le courrier fait partie de l’angoisse administrative (Money and Mental Health 2018), et un délai écrit « dans un délai de deux mois » n’est retenu par personne : la carte le dit sous forme de date.

### Mise en place {#setting-it-up}

1. Dans le ⚙ du Porche, **Courrier papier ▸ Où arrivent les scans** : le dossier où arrivent vos scans. S’il n’est pas réglé, c’est `letters/inbox` dans votre dossier de notes. L’appli de scan d’un téléphone qui s’y synchronise convient aussi.
2. Pour lire les scans, Sioul se sert de deux programmes de votre système : Poppler, pour le texte qu’un PDF contient déjà, et Tesseract, pour lire les images. Sans eux, les scans attendent, non lus, et le Porche dit comment les installer ([Installer](install.md#the-packages)).

Les fichiers PDF, PNG, JPEG, TIFF et WebP sont lus. Un fichier encore en cours d’écriture est lu la minute suivante. Rien ne quitte votre ordinateur.

### La carte {#the-card}

Sur le Porche, chaque lettre est une carte : qui l’a envoyée et ce que c’est, puis une phrase pour chaque chose trouvée :

- « Demande 86,40 €. »
- « Au plus tard le dimanche 15 novembre », avec les mots qui fixent cette date, tels que la lettre les écrit.
- « Un rendez-vous : jeudi 12 novembre à 14:15. »
- « Envoyée en recommandé avec accusé de réception. »
- « Datée du 28 septembre. » « Votre numéro : … »

Puis :

- **Voir le scan** ;
- **Une tâche pour cette date** : une tâche dans votre liste habituelle, à faire pour ce jour-là, liée au scan, comme « Payer la facture d’eau 86,40 € » ;
- **Dans l’agenda**, pour un rendez-vous ;
- le projet auquel elle appartient ;
- **Fini, classé** : le scan va dans `letters/<année>/`, nommé d’après son jour, son expéditeur et son type ; son texte est gardé.

Les règles lisent d’abord le français, et l’anglais aussi. Elles connaissent les organismes français qui écrivent à tout le monde (les Finances publiques, l’Assurance Maladie, la CAF, l’Urssaf, France Travail, un tribunal, un hôpital, une mairie, une banque, un fournisseur d’énergie), et les types de lettres, les plus graves d’abord : une mise en demeure, un avis d’impôt, une décision avec ses voies de recours, une relance, un rendez-vous, une facture. Les autres expéditeurs sont nommés d’après la première ligne de leur en-tête.

### Pas encore là {#not-there-yet}

Sont prévus : une IA qui lit une lettre ligne à ligne, la vérification qu’une lettre est authentique (le domaine de l’organisme, ses coordonnées bancaires), et une adresse où une personne qui vous aide pourrait envoyer les scans par courriel.
