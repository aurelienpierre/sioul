---
description: "Le Porche, où le nouveau courrier attend dans Sioul : vérifié, réparti en files, et montré à vos heures ; les codes et les liens que vous venez de demander, tout de suite."
---

# Le Porche {#the-porch}

Le Porche est l’endroit où le nouveau courrier attend que vous regardiez : venu de chaque adresse, vérifié (authentique ou falsifié) et réparti en files. Il remplace la boîte de réception qui crie.

<figure markdown="span">
  [![Le Porche aux heures de travail. En haut, « Ouvert jusqu’à 17:00. » et la case Temps réel ; les paiements de la semaine en une ligne ; deux cartes, pour un lien de confirmation et un code venus d’expéditeurs vérifiés, le code avec un bouton Copier et sa durée de validité ; les nouvelles d’une discussion ; puis deux files, celle d’un projet et « De personnes que vous connaissez », chacune avec ses messages.](../assets/screens/fr/porch.png){ loading=lazy }](../assets/screens/fr/porch.png "Ouvrir l’image en grand")
  <figcaption>Les codes et les liens en haut, les nouvelles des sites, puis les files.</figcaption>
</figure>

## Quand il ouvre {#when-it-opens}

- **À vos heures**, le Porche dit jusqu’à quand il est ouvert (« Ouvert jusqu’à 12:00. »), puis montre ce qui est arrivé, réparti en files.
- **En dehors de vos heures**, le Porche dit quand il ouvre la prochaine fois, et rien d’autre de votre courrier : aucun nombre, aucun nom. Vos prises du jour s’affichent quand même (plus bas). **L’ouvrir quand même** reste possible, discrètement. D’ici là, ce qui arrive est vérifié et trié.
- **Sans aucune heure réglée**, le Porche est toujours ouvert.

Quel courrier vient quand dépend de qui écrit, aux moments que vous cochez pour chacun des cinq états (sûrs, neutres, restreints, inconnus ; les bloqués jamais), et de ce à quoi sert chaque adresse : travail, vos démarches, loisirs. Voir [Qui peut vous joindre](accounts.md#senders) et [Les heures](hours.md).

Pendant tout ce temps, le courrier continue d’arriver en arrière-plan. Le Porche décide seulement quand il est montré, et quand il est dit ([plus bas](#new-mail-told-at-its-times)).

## Les codes et les liens arrivent tout de suite {#codes-and-links-come-at-once}

Les codes à usage unique, les mots de passe temporaires, les réinitialisations de mot de passe, les liens de connexion et les liens pour confirmer une adresse viennent de quelque chose que vous venez de demander à un site, et ils expirent. Sioul les montre donc tout de suite, à n’importe quelle heure, même quand ils viennent d’une adresse automatique de site (no-reply…) :

- une seule notification de bureau, sans son, avec le code, un bouton pour le copier (sous Linux), et sa durée de validité ; aucune pendant le sommeil : la carte attend ici ;
- la même carte en haut du Porche, avec **Copier**.

Rien d’autre ne s’ouvre avec. Une fois expiré, le code est masqué, et le message va dans sa file. Il expire quand le message le dit, sinon quand son type expire d’habitude : un code après 30 minutes, un lien de connexion après une heure, une réinitialisation de mot de passe après deux heures, un lien pour confirmer une adresse après un jour, un mot de passe temporaire après une semaine.

Beaucoup de sites envoient leurs codes et leurs liens par les mêmes services que leurs lettres d’information, avec les mêmes en-têtes. Ils arrivent quand même tout de suite. Sioul cherche le code dans leur objet et en haut de leur texte, là où un site met ce que vous avez demandé : l’article d’une lettre d’information sur les mots de passe reste une lettre d’information.

Les faux messages « votre code » sont une ruse d’hameçonnage courante. Un message falsifié est mis de côté, même quand il porte les en-têtes d’une lettre d’information. Celui d’un expéditeur seulement *non vérifié* arrive quand même, avec un avertissement : ne vous en servez que si vous venez de le demander à ce site.

## Le nouveau courrier, dit à ses heures {#new-mail-told-at-its-times}

Quand arrive du courrier que vos listes laissent passer maintenant, une seule notification discrète le dit, pour tout le lot : « Deux lettres », et les premiers expéditeurs avec leur objet : « Murena, Votre facture · Alice, Dîner vendredi ». **Ouvrir** montre le Porche.

- **Ce qui attendait** (le courrier arrivé hors des heures de sa liste, pendant votre sommeil ou une pause) est dit une fois, quand son heure vient : « Le Porche ouvre : trois lettres vous attendent. »
- **Jamais** pour les codes (ils ont la leur, plus haut), le courrier mis de côté, les expéditeurs bloqués, vos adresses moins importantes, ce que vous vous envoyez, les lettres d’information sauf si vous les incluez, ni le courrier déjà lu ailleurs. Rien pendant le sommeil ni une pause.
- **Sans son.** Avec un téléphone et un ordinateur, seul celui dont vous vous êtes servi en dernier le dit.

Paramètres ▸ Rappels et notifications ▸ **Nouveau courrier : notifier aux heures où il peut venir**, et **Avec les lettres d’information** ([Paramètres](settings.md#reminders-and-notifications)).

## Les files {#the-lanes}

Chaque message va dans la première file qui le prend, dans cet ordre :

| File | Ce qu’elle contient |
|---|---|
| **Mis de côté** | Le courrier falsifié, celui qui emprunte un nom, les indésirables, et les expéditeurs que vous avez bloqués. Montrée en bas, chaque message avec sa raison. Rien n’est supprimé. |
| **Hostile, écarté** | Seulement pour une adresse que vous protégez contre le harcèlement (plus bas) : insultes, harcèlement, menaces. Leurs mots restent cachés. |
| **Tout de suite** | Les codes et les liens que vous venez de demander à un site, vus plus haut. |
| **Une file par projet** | Le courrier qui correspond aux règles du projet, ou qui appartient à une conversation du projet. Voir [Les projets](projects.md). |
| **Votre adresse publique** | Le courrier envoyé à une adresse que vous protégez, par des personnes que vous n’avez pas laissées entrer, lu d’abord et trié par sujet : le travail en premier. |
| **Comptes moins importants** | Le courrier des adresses que vous avez classées sous les autres : réseaux sociaux, notifications que vous lisez de temps en temps. Repliée en bas. |
| **Rangé : lettres d’information et notifications** | Listes de diffusion, lettres d’information et expéditeurs automatiques (no-reply…). Repliée. |
| **Nouveaux expéditeurs, en attente de votre accord** | Un inconnu : dans aucun de vos carnets d’adresses, sur aucune liste. Vous le laissez entrer, ou non. |
| **De personnes que vous connaissez** | Les personnes de vos carnets d’adresses, celles que vous avez laissées entrer, et celles qui sont sur une liste par leur propre adresse. |

Le courrier que vous vous envoyez vous-même, d’une de vos adresses à une autre (un fichier envoyé depuis votre téléphone), vient avec les personnes que vous connaissez, à toute heure, sans jamais passer par le filtre d’accueil, quand il est vérifié : falsifier votre propre adresse est une ruse classique.

Sous le titre de chaque file, une ligne dit ce qu’elle contient. Son **?** (Comment le courrier arrive ici) dit, phrase par phrase, comment le courrier y arrive, et où le changer.

### Laisser entrer quelqu’un {#letting-someone-in}

Un message en attente de votre accord a **Accepter cette adresse** : les messages suivants de cette personne vont dans « De personnes que vous connaissez ».

Chaque message a aussi **Son courrier**, dans son menu (⋮). D’abord, une ligne dit ce qui décide pour cet expéditeur maintenant : « Sûr, comme le dit la catégorie Amis. » Puis les choix : **Comme le disent ses catégories** (son entrée propre retirée des listes : les catégories de sa fiche décident, sinon le domaine de son adresse), ou l’une des quatre listes, chacune avec les moments de son courrier : **Sûr**, **Neutre**, **Restreint**, **Bloqué** (mis de côté pour de bon, jamais montré). Un expéditeur qui n’est dans aucun de vos carnets d’adresses ni sur aucune liste est un inconnu, avec ses propres moments. Les mêmes listes, avec des motifs comme `*@example.org` et des numéros, et le moment où vient chacun, sont dans [Comptes ▸ Qui peut vous joindre](accounts.md#senders).

Le courrier falsifié est jugé à part : un message falsifié est mis de côté quoi que disent les listes, même s’il prétend venir de quelqu’un que vous avez marqué sûr, et pesé comme celui d’un inconnu.

## Lire un message {#reading-a-message}

Un message s’ouvre à droite. Avant son texte :

- **qui l’a envoyé**, et à quel point c’est vérifié : *vérifié*, *non vérifié* ou *falsifié*. Survoler le bouclier montre chaque vérification (SPF, DKIM, DMARC, ARC, DNS inverse), son résultat, et qui l’a faite ;
- **à qui**, en copie à qui, quand, le sujet ;
- **pourquoi il est ici**, replié jusqu’à ce que vous le demandiez.

Puis le texte, rendu sûr :

- Le courrier HTML garde ses paragraphes, ses listes, son texte en gras et ses liens. Rien d’autre n’est montré : pas d’images, pas de styles, pas de scripts. Rien ne se charge depuis le réseau, rien ne s’exécute.
- Les messages précédents qu’une réponse cite sont repliés sous **Afficher les messages précédents**. Une signature est atténuée.
- Chaque lien montre son adresse complète sous le message tant que le pointeur est dessus, avant que vous cliquiez.
- Les **Pièces jointes** sont repliées, une ligne chacune, avec leur type, leur nom et leur taille. En ouvrir ou en enregistrer une lance d’abord l’antivirus ; si l’appareil n’en a pas, Sioul le dit et demande avant d’ouvrir. Voir [Vie privée et sécurité](privacy-security.md#attachments-and-the-antivirus).

Depuis le message : répondre, transférer, archiver, supprimer, et le reste, comme sur la page [Courrier](mail.md). Une lettre d’information a aussi **Se désabonner** ([Courrier](mail.md#unsubscribing)).

## Terminé pour l’instant {#done-for-now}

**Terminé pour l’instant** ferme le Porche sur ce qu’il a montré. Tout ce qui est plus récent attend sa prochaine ouverture, même le courrier qui a atteint le serveur plus tôt mais n’est arrivé ici qu’ensuite.

Fermer le Porche ne dit rien au serveur : aucun message n’est marqué comme lu par là. Seule l’ouverture d’un message le marque comme lu, comme dans tout logiciel de courrier.

Une adresse sur laquelle le Porche n’a jamais été fermé ne montre que ses deux dernières semaines. Le courrier plus ancien reste dans [Courrier](mail.md), à sa place.

## Temps réel {#real-time}

La case **Temps réel**, en haut du Porche, relève chaque dossier de chaque adresse toutes les minutes, et garde le Porche ouvert : pour un code ou un mot de passe que vous attendez. Décochez-la pour revenir au rythme habituel.

Pendant le calme, **Travailler maintenant** apparaît à côté, pour montrer le travail quelles que soient les heures. Voir [Les heures](hours.md#work-now).

## Au-dessus des files {#above-the-lanes}

Quand il y a quelque chose, quelques lignes viennent avant les files :

- **Quand vos heures ne sont pas réglées**, une carte les demande, avec **Régler mes heures** (qui ouvre les Paramètres à cet endroit) et **Laisser ainsi** (qui arrête de demander).
- **Quand votre nuit n’est pas réglée**, une carte dit que rien n’éloigne les notifications pendant que vous dormez, avec **Régler ma nuit** (qui ouvre la page Santé là où se règlent les repas et la nuit) et **Laisser ainsi**. Voir [Les heures](hours.md#sleep).
- **Où vous en étiez** : la ligne que vous avez laissée quand quelque chose est venu vous interrompre, avec sa tâche, jusqu’à ce que vous appuyiez sur **C’est fait**. Voir [Les tâches](tasks.md#starting-and-stopping).
- **Deux événements en même temps aujourd’hui**, le temps d’y aller et d’en revenir compté, avec **Ouvrir « … »** pour chacun et **Ne plus en parler**. Voir [L’agenda](agenda.md#two-events-at-once).
- **Les prises du jour pas encore marquées**, à partir de leur heure, qu’une notification vous les ait rappelées ou non : chacune avec son heure, son nom et **Pris** (plus d’une demi-heure en retard, **Pris…** demande quand vous l’avez prise). Quelles que soient vos heures : une prise n’est pas du courrier. Chacune reste jusqu’à ce que vous la marquiez, que la journée finisse ou que douze heures soient passées ; pendant votre sommeil, si les prises restent silencieuses, elles attendent votre réveil. Quand un autre appareil peut en savoir plus, le doute est dit sous la prise : vérifiez avant de la prendre. Voir [La santé](health.md#reminders).
- **Les prises prévues pendant que Sioul était fermé**, ni marquées ni rappelées nulle part : **Pris…** (quand vous l’avez prise) ou **Pas pris**. Quand un autre appareil peut en savoir plus, le doute est dit sous la prise. Voir [La santé](health.md#reminders).
- **Les appels refusés par Sioul**, sur un téléphone qui filtre les appels, chacun à un moment où son auteur peut vous joindre : « Pendant votre sommeil : un numéro absent de vos contacts a appelé à 09:30. », avec **Répondre par SMS**, **Rappeler**, **Écouter** quand Free a envoyé le message vocal par courriel. Voir [Les appels](calls.md#afterwards-on-the-porch).
- **« Du nouveau sur &lt;site&gt; »** : ce que les sites que vous gardez dans Sioul ont notifié, et qui vous attend. Ouvrir le site efface ses nouvelles. Voir [Les sites](sites.md).
- **Le courrier papier** que vous avez scanné, chaque lettre en carte : qui, quoi, combien, pour quand. Voir [Les papiers et les lettres](papers.md#paper-letters).
- **Les paiements de la semaine**, en une ligne : « Cette semaine : Électricité 62 € (lun.). Le compte les tient. » Quand quelque chose demande un coup d’œil côté argent, la ligne le dit, sans nombre. Voir [Les budgets](budgets.md#the-bank-watch).

<figure markdown="span">
  [![Une carte en haut du Porche : « Vos heures ne sont pas réglées : travail et démarches arrivent à toute heure. », une phrase sur ce qu’apporte chaque sorte d’heures, et deux boutons, « Régler mes heures » et « Laisser ainsi ».](../assets/screens/fr/porch-hours.png){ loading=lazy }](../assets/screens/fr/porch-hours.png "Ouvrir l’image en grand")
  <figcaption>Tant que vos heures ne sont pas réglées, le Porche demande une fois.</figcaption>
</figure>

## Une adresse publique, protégée {#a-public-address-protected}

Une adresse que vous publiez apporte du travail, et parfois des insultes. Sur sa fiche dans [Comptes](accounts.md#a-mail-address), **Protégée contre le harcèlement** fait lire son courrier avant que vous le voyiez :

- **son ton** : calme, grossier, ou hostile (insultes qui vous visent, harcèlement, menaces), à partir de listes de mots en français et en anglais ;
- **son sujet** : travail, une question, la presse, des remerciements, un don, autre chose.

Le courrier hostile va dans sa propre file, repliée, qui ne montre ni le nom de l’expéditeur ni le sujet. En ouvrir un demande d’abord : il peut attendre, aller à quelqu’un de confiance (**Transférer à quelqu’un de confiance**), ou partir (bloqué, supprimé). Le lire quand même est votre choix, au moment qui vous convient. Le reste du courrier de l’adresse a sa propre file, le travail en premier, avec son sujet affiché, et une marque quand il est grossier.

**Laisser l’IA le lire d’abord**, en dessous, reste désactivé tant que vous ne l’activez pas : chaque nouveau message à cette adresse est alors envoyé une fois à Claude, d’Anthropic, avec son sujet, pour en dire le ton et le sujet plus finement que des listes de mots. Le texte quitte votre appareil pour cela. La clé du service d’Anthropic se tape une fois dans Comptes et reste dans votre trousseau.

## Les réglages du Porche {#the-porchs-settings}

Le ⚙ en haut du Porche contient ce qui n’appartient qu’au Porche, et la lecture d’un message ouvert ici :

- **Projets montrés ici** : quels projets ont une file sur le Porche. Le courrier des autres reste sur leur page dans Projets.
- **Courrier papier ▸ Où arrivent les scans** : le dossier où arrivent vos scans.
- **Lecture du texte** : la police, sa taille et l’interligne d’un message ouvert ici, les mêmes que dans le ⚙ de la page Courrier.
- **Comment le courrier est trié** : chaque file, dans l’ordre du tri, avec ses règles ; les expéditeurs que vous connaissez ; les mots qui rendent un expéditeur automatique (no-reply…).

Ce qui appartient à autre chose se règle là où se trouve cette chose : le rang et la protection d’une adresse sur sa fiche dans [Comptes](accounts.md), les règles d’un projet sur sa page dans [Projets](projects.md), qui peut vous écrire et quand dans [Comptes ▸ Expéditeurs](accounts.md#senders), vos heures dans [Paramètres](settings.md#hours).

### Certaines adresses d’abord, d’autres en dernier {#some-addresses-first-others-last}

Sur la fiche de chaque adresse dans Comptes, **Priorité** : *Plus important*, *Normal* ou *Moins important*. Le courrier des adresses plus importantes vient en premier dans chaque file, avec une petite marque. Celui des moins importantes ne passe pas par le filtre d’accueil et attend, replié, en bas. Leurs codes arrivent quand même tout de suite.

## Pourquoi il fonctionne ainsi {#why-it-works-this-way}

- Relever son courrier trois fois par jour a fait baisser le stress quotidien dans un essai randomisé (Kushlev & Dunn 2015). Regrouper les notifications a aidé l’attention et l’humeur, alors que n’en recevoir aucune rendait les personnes plus anxieuses (Fitz et al. 2019) : ce qui aide, c’est la prévisibilité, pas le silence. Le Porche ouvre donc aux heures que vous choisissez, et dit quand.
- Les personnes interrompues travaillent plus vite, avec plus de stress et de frustration (Mark, Gudith & Klocke 2008), et une notification laissée sans réponse coûte quand même de l’attention (Stothart, Mitchum & Yehnert 2015). Rien ne surgit donc hors des heures que vous choisissez, une seule notification dit tout un lot, et rien ne bouge sous vos yeux quand le courrier arrive.
- On évite les informations dont on attend qu’elles fassent mal (Sweeny et al. 2010). Ce qu’est un message, qui l’a envoyé et à quel point c’est vérifié viennent donc avant son texte.

Plus de détails dans [ce que dit la recherche (en anglais)](https://aurelienpierre.github.io/sioul/dev/research.html).
