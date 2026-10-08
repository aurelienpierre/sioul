---
description: Les SMS sur votre ordinateur avec Sioul - tous les SMS de votre téléphone et leurs images lus sur votre ordinateur, gardés scellés par votre propre partage, et les SMS écrits là-bas que votre téléphone envoie, chacun une fois, avec ce qu’il en est advenu dit simplement.
---

# Les SMS : lus et envoyés depuis votre ordinateur {#texts-read-and-sent-from-your-computer}

Les SMS de votre téléphone peuvent se lire sur votre ordinateur, et un SMS écrit là-bas peut être envoyé par votre téléphone. Sioul ne devient pas l’application de SMS de votre téléphone : votre application de messages reste le registre de vos SMS, et continue de fonctionner comme avant. Vos ordinateurs les gardent tous, scellés, comme une archive qui ne fait que grandir.

## La mise en place {#setting-it-up}

1. **Sur le téléphone**, Réglages ▸ Ce téléphone ▸ **SMS** dit à quoi sert chaque autorisation : lire vos SMS, être averti d’un nouveau, envoyer les SMS que vous écrivez sur un ordinateur, et le nom de vos cartes SIM. **Autoriser dans Android…** les demande.
2. **Activez la partie SMS**, sur le téléphone et sur chaque ordinateur qui doit lire ou écrire des SMS (Réglages ▸ Votre dossier et le partage ▸ Ce qui part de cet appareil). Elle reste désactivée tant que vous ne l’activez pas.
3. **L’historique suit ensuite de lui-même**, 500 SMS à la fois, les images et autres médias après leurs SMS, pour que vos prises, le mode ne pas déranger et les appels n’attendent jamais derrière lui. Sur le téléphone, Réglages ▸ Ce téléphone ▸ SMS dit combien de SMS et de médias il a, puis combien sont partis. Sur un ordinateur, la ligne de la partie dans le panneau du partage dit combien il en garde.

## Ce que vous voyez {#what-you-see}

**SMS** est une page à part entière, sous Courrier parmi les lieux à gauche, disposée comme Courrier.

- **À gauche, les conversations**, les plus récentes en premier, les personnes sous le nom de vos carnets d’adresses, leurs derniers mots, et quand ; « Un brouillon d’un agent d’IA attend ici. » là où il y en a un. La recherche au-dessus parcourt les mots de chaque SMS, ainsi que les noms et les numéros.
- **À droite, la conversation que vous ouvrez**, en fil : tout l’historique dans l’ordre du temps, chaque jour dit une fois, vos SMS à droite et les leurs à gauche.
- **Les messages multimédias** : une image s’affiche dans son message ; un son, une vidéo, une fiche de contact ou un autre fichier dit sa nature et sa taille, avec **Ouvrir** et **Enregistrer…**. Un fichier plus gros que la taille choisie sur le téléphone (10 Mo sauf si vous en choisissez une autre) y reste, et la page le dit ; les opérateurs gardent les messages multimédias petits, si bien que presque tous viennent.
- **Un SMS effacé sur votre téléphone** reste ici, dit discrètement.
- **En écrivant**, sous le fil : d’abord les brouillons d’un agent d’IA, s’il en a laissé (**L’utiliser** met leurs mots dans la zone ; l’envoi reste le vôtre), puis la zone. La page dit combien de SMS le vôtre prendra. Les accents et les signes hors de l’alphabet propre aux SMS laissent moins de caractères par SMS.
- **Au clavier** : Haut et Bas parcourent la liste et Entrée ouvre une conversation ; Échap la ferme.
- **Dans une fenêtre étroite**, comme dans Courrier : la liste, puis la conversation, et Retour.
- **Ce qu’il est advenu d’un SMS que vous avez envoyé**, en mots : en attente de votre téléphone (et quand il a partagé pour la dernière fois), en cours d’envoi, envoyé à, remis à (seulement quand votre opérateur le dit), pas envoyé et pourquoi, expiré, ou « Votre téléphone ne l’a peut-être pas envoyé », quand Sioul ne peut pas le savoir. **Renvoyer** fait un nouveau SMS ; rien n’est jamais renvoyé de lui-même.
- **Pas affichés** : les brouillons et les SMS qui ont échoué ou attendent de partir. Android ne les montre qu’à votre application de messages. Un message multimédia que votre téléphone n’a pas téléchargé le dit.

## Chaque SMS une fois {#each-text-once}

Un SMS écrit sur un ordinateur attend dans votre partage que votre téléphone le prenne. Le téléphone l’inscrit dans un registre privé avant de le confier à Android : il n’est jamais envoyé deux fois, quoi qu’il arrive au partage. Un SMS qui a attendu le téléphone plus de 15 minutes n’est pas envoyé : la page le dit, et vous choisissez de le renvoyer ou non. Android réessaie de lui-même quand le réseau fait défaut ; Sioul, jamais.

**Pas envoyés depuis un ordinateur** : les numéros courts, qui peuvent coûter de l’argent ; un SMS à plusieurs personnes à la fois, qui est un message multimédia ; les images. Envoyez-les depuis votre téléphone.

## Privé {#private}

Vos SMS et leurs médias passent par votre propre dossier de partage, scellés avec votre phrase de passe, et sont scellés à nouveau sur chaque ordinateur qui les garde, avec une clé que seuls vos appareils détiennent. Le serveur du dossier voit quel appareil a écrit et quand, jamais quoi, ni la longueur de chaque SMS. Pour afficher une image ou ouvrir un fichier, la page en écrit une copie pour vous seul, et l’efface quand elle se ferme. Désactivez la partie sur un ordinateur, et plus rien ne lui arrive.

## Une archive {#an-archive}

Vos ordinateurs gardent chaque SMS et tout ce dont aurait besoin une restauration future sur un nouveau téléphone : chaque SMS tel que les messages de votre téléphone le gardent, et chaque média. Cette restauration n’est pas encore construite ; pour l’instant l’archive sert à lire et écrire sur vos ordinateurs.

## Où la trouver {#where-to-find-it}

- **Parmi les lieux à gauche**, sous Courrier.
- **Sur le Porche**, **Répondre par SMS** sur un SMS ouvre sa conversation ici.
- **Sur la fiche d’une personne**, **SMS avec cette personne…**.

## Pas encore là {#not-there-yet}

- Répondre depuis l’ordinateur au message d’une application de discussion.

## Pour les lecteurs techniques {#for-technical-readers}

Comment c’est construit, avec chaque règle qui empêche un SMS de partir deux fois : [les notes sur les SMS (en anglais)](https://aurelienpierre.github.io/sioul/dev/texts.html). La recherche derrière : [Texts and notifications on the computer (en anglais)](https://aurelienpierre.github.io/sioul/dev/research/sms.html).
