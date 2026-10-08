---
description: "Le Porche, où le nouveau courrier attend dans Sioul : vérifié, réparti en files selon qui écrit et pourquoi, et montré à vos heures ; les codes et les liens que vous venez de demander, tout de suite."
---

# Le Porche {#the-porch}

## En bref {#in-short}

Le Porche est l’endroit où le nouveau courrier de toutes vos adresses attend que vous regardiez. Avant que rien ne s’affiche, Sioul vérifie chaque message, authentique ou falsifié, et le range dans une file selon qui l’a écrit et à quoi il sert : les personnes que vous connaissez, quelqu’un de nouveau en attente de votre accord, un projet, les lettres d’information, ce qui est mis de côté. Il ouvre aux heures que vous choisissez et reste silencieux en dehors, tandis que les codes et les liens que vous venez de demander à un site arrivent tout de suite. Rien n’est compté devant vous, et rien n’est supprimé.

<figure markdown="span">
  [![Le Porche aux heures de travail. En haut, « Ouvert jusqu’à 17:00. » et la case Temps réel ; les paiements de la semaine en une ligne ; deux cartes, pour un lien de confirmation et un code venus d’expéditeurs vérifiés, le code avec un bouton Copier et sa durée de validité ; les nouvelles d’une discussion ; puis deux files, celle d’un projet et « De personnes que vous connaissez », chacune avec ses messages.](../assets/screens/fr/porch.png){ loading=lazy }](../assets/screens/fr/porch.png "Ouvrir l’image en grand")
  <figcaption>Les codes et les liens en haut, les nouvelles des sites, puis les files.</figcaption>
</figure>

## Protégé par défaut {#what-is-protected}

Sans rien régler :

- **Chaque message est vérifié avant de s’afficher.** Un message falsifié, ou qui emprunte le nom d’une banque ou d’un service public, est mis de côté d’abord, quoi que disent vos listes, même s’il prétend venir de quelqu’un que vous avez marqué sûr ([Les files](#the-lanes)).
- **Le premier message d’un inconnu attend votre accord** avant de vous joindre. Un message dont rien ne prouve qu’il vient de son adresse est traité comme celui d’un inconnu, quelle que soit l’adresse qu’il affiche ([Laisser entrer quelqu’un](#letting-someone-in)).
- **Un faux message « votre code » est mis de côté**, et le code d’un expéditeur seulement non vérifié arrive avec un avertissement ([Les codes et les liens](#codes-and-links-come-at-once)).
- **Les avis sur les indésirables ne touchent jamais les personnes que vous connaissez**, vos codes ni le courrier de vos projets ([Les indésirables, et votre propre filtre](#spam-and-your-own-filter)).
- **Rien n’est supprimé, et fermer le Porche ne marque rien comme lu** ([Terminé pour l’instant](#done-for-now)). Les expéditeurs que vous avez bloqués n’apparaissent jamais.

## Quand il ouvre {#when-it-opens}

- **À vos heures**, le Porche dit jusqu’à quand il est ouvert (« Ouvert jusqu’à 12:00. »), puis montre ce qui est arrivé, réparti en files.
- **En dehors de vos heures**, le Porche dit quand il ouvre la prochaine fois, et rien d’autre de votre courrier : aucun nombre, aucun nom. Vos prises du jour s’affichent quand même (plus bas). **L’ouvrir quand même** reste possible, discrètement. D’ici là, ce qui arrive est vérifié et trié.
- **Sans aucune heure réglée**, le Porche est toujours ouvert.

Quel courrier vient quand dépend de qui écrit, comme le dit la ligne de chaque liste à chaque moment (sûrs, neutres, restreints, inconnus, les personnes qui passent toujours ; les bloqués jamais), et de ce à quoi sert chaque adresse : travail, vos démarches, loisirs. Voir [Ce qui vous joint, et quand](notifications.md#by-person) et [Les heures](hours.md).

Pendant tout ce temps, le courrier continue d’arriver en arrière-plan. Le Porche décide seulement quand il est montré, et quand il est dit ([plus bas](#new-mail-told-at-its-times)).

## Les codes et les liens arrivent tout de suite {#codes-and-links-come-at-once}

Les codes à usage unique, les mots de passe temporaires, les réinitialisations de mot de passe, les liens de connexion et les liens pour confirmer une adresse viennent de quelque chose que vous venez de demander à un site, et ils expirent. Sioul les montre donc tout de suite, à n’importe quelle heure, même quand ils viennent d’une adresse automatique de site (no-reply…) :

- sur un ordinateur, une seule notification, sans son, avec le code, un bouton pour le copier (sous Linux), et sa durée de validité ; aucune pendant le sommeil : la carte attend ici ;
- la même carte en haut du Porche, avec **Copier** ; sur un téléphone, sur le Porche et sur la carte de Sioul sur votre écran d’accueil, puisqu’un téléphone ne notifie pas encore les codes.

Rien d’autre ne s’ouvre avec. Une fois expiré, le code est masqué, et le message va dans sa file. Il expire quand le message le dit, sinon quand son type expire d’habitude : un code après 30 minutes, un lien de connexion après une heure, une réinitialisation de mot de passe après deux heures, un lien pour confirmer une adresse après un jour, un mot de passe temporaire après une semaine.

Beaucoup de sites envoient leurs codes et leurs liens par les mêmes services que leurs lettres d’information, avec les mêmes en-têtes. Ils arrivent quand même tout de suite. Sioul cherche le code dans leur objet et en haut de leur texte, là où un site met ce que vous avez demandé : l’article d’une lettre d’information sur les mots de passe reste une lettre d’information.

Les faux messages « votre code » sont une ruse d’hameçonnage courante. Un message falsifié est mis de côté, même quand il porte les en-têtes d’une lettre d’information. Celui d’un expéditeur seulement *non vérifié* arrive quand même, avec un avertissement : ne vous en servez que si vous venez de le demander à ce site.

## Le nouveau courrier, dit à ses heures {#new-mail-told-at-its-times}

Quand arrive du courrier que vos listes laissent passer maintenant, une seule notification discrète le dit, pour tout le lot : « Deux lettres », et les premiers expéditeurs avec leur objet : « Murena, Votre facture · Alice, Dîner vendredi ». **Ouvrir** montre le Porche.

- **Ce qui attendait** (le courrier arrivé hors des heures de sa liste, pendant votre sommeil ou une pause) est dit une fois, quand son heure vient : « Le Porche ouvre : trois lettres vous attendent. »
- **Jamais** pour les codes (ils ont la leur, plus haut), le courrier mis de côté, ce que votre propre filtre à indésirables a signalé ou déplacé (cela attend dans « Retenu par votre filtre à indésirables », [plus bas](#spam-and-your-own-filter)), les expéditeurs bloqués, vos adresses moins importantes, ce que vous vous envoyez, les lettres d’information sauf si vous les incluez, ni le courrier déjà lu ailleurs. Rien pendant le sommeil ni une pause.
- **Sans son.** Avec un téléphone et un ordinateur, seul celui dont vous vous êtes servi en dernier le dit. Sur l’écran verrouillé d’un téléphone, la notification dit combien de lettres sont arrivées, pas qui a écrit.

Paramètres ▸ Ce qui vous joint ▸ Par personne ▸ Courrier : **Me dire le nouveau courrier**, et **Avec les lettres d’information** ([Ce qui vous joint, et quand](notifications.md#by-person)).

## Les files {#the-lanes}

Chaque message va dans la première file qui le prend, dans cet ordre. Trois d’entre elles n’apparaissent qu’une fois quelque chose mis en place, comme leur ligne le dit.

| File | Ce qu’elle contient |
|---|---|
| **Mis de côté** | Le courrier falsifié, celui qui emprunte un nom, et le courrier d’un inconnu que votre fournisseur dit indésirable ([plus bas](#spam-and-your-own-filter)). Montrée en bas, chaque message avec sa raison. Rien n’est supprimé. Un indésirable a **Pas indésirable** : de retour dans sa file pour de bon, sur chaque appareil. |
| **Hostile, mis de côté** | Seulement pour une adresse que vous protégez contre le harcèlement ([plus bas](#a-public-address-protected)) : insultes, harcèlement, menaces. Leurs mots restent cachés. |
| **Retenu par votre filtre à indésirables** | Ce que votre propre filtre à indésirables a signalé, ou déplacé dans le dossier Indésirables, comme vous l’avez choisi ([plus bas](#spam-and-your-own-filter)). Repliée, sans compte, jamais de notification. Sioul en apprend tel quel ; si l’un n’est pas indésirable, **Pas indésirable** le remet à sa place et l’apprend au filtre. |
| **Tout de suite** | Les codes et les liens que vous venez de demander à un site, vus plus haut. |
| **Une file par projet** | Le courrier qui correspond aux règles du projet, ou qui appartient à une conversation du projet. Voir [Les projets](projects.md). |
| **Votre adresse publique** | Seulement pour une adresse que vous protégez : le courrier de personnes que vous n’avez pas laissées entrer, lu d’abord et trié par sujet, le travail en premier. |
| **Comptes moins importants** | Seulement une fois une adresse classée sous les autres ([plus bas](#some-addresses-first-others-last)) : son courrier, comme les réseaux sociaux ou les notifications que vous lisez de temps en temps, replié en bas. |
| **Rangé : lettres d’information et notifications** | Listes de diffusion, lettres d’information et expéditeurs automatiques (no-reply…). Repliée. |
| **Nouveaux expéditeurs, en attente de votre accord** | Un inconnu : dans aucun de vos carnets d’adresses et sur aucune liste, ou sur la liste neutre ou restreinte par son seul domaine. Aussi tout message dont rien ne prouve qu’il vient de l’adresse qu’il affiche. Vous le laissez entrer, ou non. |
| **De personnes que vous connaissez** | Les personnes de vos carnets d’adresses, celles que vous avez laissées entrer, celles qui sont sur une liste par leur propre adresse, et celles d’un domaine que vous avez marqué sûr. |

**Les expéditeurs que vous avez bloqués** n’apparaissent jamais sur le Porche : leur courrier est mis de côté pour de bon, jamais montré, jamais compté et jamais dit. Rien n’est supprimé.

Le courrier que vous vous envoyez vous-même, d’une de vos adresses à une autre (un fichier envoyé depuis votre téléphone), vient avec les personnes que vous connaissez, à toute heure, sans jamais passer par le filtre d’accueil, quand il est vérifié : falsifier votre propre adresse est une ruse classique.

Sous le titre de chaque file, une ligne dit ce qu’elle contient. Son **?** (Comment le courrier arrive ici) dit, phrase par phrase, comment le courrier y arrive, et où le changer.

### Laisser entrer quelqu’un {#letting-someone-in}

Un message en attente de votre accord a **Accepter cette adresse** : les messages suivants de cette personne vont dans « De personnes que vous connaissez ».

Chaque message a aussi **Comment cette personne vous joint…**, dans son menu (⋮) : la fiche de l’expéditeur. D’abord, une ligne dit ce qui décide pour cet expéditeur maintenant : « Sûr, comme le dit la catégorie Amis. » Puis sa liste : **Comme le disent ses catégories** (son entrée propre retirée des listes : les catégories de sa fiche décident, sinon le domaine de son adresse), ou l’une des quatre listes : **Sûr**, **Neutre**, **Restreint**, **Bloqué** (mis de côté pour de bon, jamais montré) ; **Passe toujours** ; et ce qui vous joint de sa part à chaque moment. Un expéditeur qui n’est dans aucun de vos carnets d’adresses ni sur aucune liste est un inconnu, avec sa propre ligne. Les mêmes listes, avec des motifs comme `*@example.org` et des numéros, et ce que fait chacune à chaque moment, sont dans Paramètres ▸ [Ce qui vous joint](notifications.md#by-person).

Le courrier falsifié est jugé à part : un message falsifié est mis de côté quoi que disent les listes, même s’il prétend venir de quelqu’un que vous avez marqué sûr, et pesé comme celui d’un inconnu.

## Les indésirables, et votre propre filtre {#spam-and-your-own-filter}

Seul le courrier d’un inconnu est jamais jugé. Qui que ce soit qui le juge, votre fournisseur ou le propre filtre de Sioul, cela ne touche jamais le courrier de quelqu’un que vous connaissez (dans vos carnets d’adresses, sur une liste, accepté), les codes et les liens que vous avez demandés, le courrier d’un projet, ce que vous vous envoyez, ni un message dont vous avez dit qu’il n’était pas indésirable, sur aucun de vos appareils. Le courrier falsifié, les noms empruntés et les expéditeurs bloqués sont mis de côté avant, comme toujours. Un message dont les deux vérifications habituelles ont échoué, si bien que rien ne prouve qu’il vient de l’adresse qu’il affiche, compte comme celui d’un inconnu, quelle que soit cette adresse.

- **Le mot de votre fournisseur** : le message d’un inconnu que votre fournisseur marque indésirable va dans Mis de côté.
- **Votre propre filtre**, une fois qu’il a appris de votre courrier ([Courrier, Votre filtre à indésirables](mail.md#your-own-spam-filter)), trouve le message d’un inconnu **probablement indésirable** (à partir de 95 % sauf si vous le changez), **peut-être indésirable** (à partir de 50 %), ou **probablement pas indésirable**. Ce qu’il fait de chacun, c’est vous qui le choisissez, dans ses réglages :
    - **Déplacer dans les indésirables** : à son arrivée, le message va dans le dossier Indésirables de son adresse, sur le serveur ;
    - **Signaler seulement** : il reste où il est, signalé ;
    - **Ne rien faire** : il va dans sa file, comme tout message.

    Tant que vous n’avez pas choisi, il signale ce qui est probablement ou peut-être indésirable, et ne fait rien du reste.

Ce qu’il signale ou déplace attend dans **Retenu par votre filtre à indésirables**, repliée vers le bas du Porche, sans compte : jamais de notification pour cela, sur aucun appareil, ni sur l’écran d’accueil de votre téléphone. Rien n’y demande votre temps : Sioul apprend de ce que le filtre a retenu tel quel (ce qu’il a déplacé ou trouvé probablement indésirable, comme indésirable ; un peut-être indésirable, pas avant que vous l’ayez dit), et vous n’intervenez que si l’un n’est pas indésirable.

<figure markdown="span">
  [![Le Porche. À gauche, « Retenu par votre filtre à indésirables » ouverte, sans compte, avec sa phrase sous le titre : ce que votre filtre a retenu y attend, sans notification ; Sioul en apprend tel quel, et « Pas indésirable » en remet un à sa place et l’apprend au filtre. Puis des messages d’inconnus, chacun avec son mot discret, « probablement indésirable » ou « peut-être indésirable », et Pas indésirable et Indésirable dessous ; au-dessus d’eux, Pas indésirable pour tous et Indésirable pour tous. Le premier, que le filtre a déplacé dans le dossier Indésirables, est ouvert à droite : « Pourquoi il est ici » dit « votre propre filtre : probablement indésirable » et qu’il a été mis dans votre dossier Indésirables, « Pas indésirable » le ramenant dans la boîte de réception ; sous le message, Indésirable, Pas indésirable et Fermer.](../assets/screens/fr/porch-spam.png){ loading=lazy }](../assets/screens/fr/porch-spam.png "Ouvrir l’image en grand")
  <figcaption>Ce que votre propre filtre a signalé ou déplacé.</figcaption>
</figure>

Chaque message y a son mot, discret (« probablement indésirable », « peut-être indésirable »), et deux boutons :

- **Pas indésirable** : de retour dans sa file, pour de bon, sur chaque appareil, et le filtre apprend qu’il ne l’est pas ; celui que le filtre a déplacé revient dans la boîte de réception ;
- **Indésirable** : dans le dossier Indésirables, ou gardé là, pour de bon.

**Pas indésirable pour tous** et **Indésirable pour tous**, au-dessus, répondent pour tous d’un coup. Chacun s’annule pendant dix secondes ; le filtre apprend ce que vous avez dit à son prochain apprentissage, votre mot avant le sien. Ce que vous dites sur votre téléphone, votre ordinateur le sait, et inversement. Sioul dit seulement que votre propre filtre l’a jugé, jamais pourquoi. **Indésirable**, dans le Courrier, déplace un message dans le dossier des indésirables, comme toujours, et l’apprend aussi au filtre. Rien n’est supprimé.

Un message que le filtre a déplacé reste ici tant qu’il est dans le dossier Indésirables et que vous n’en avez rien dit ; un message qu’il a signalé, deux semaines, même après que vous avez fermé le Porche avec **Terminé pour l’instant**.

## Lire un message {#reading-a-message}

Un message s’ouvre à droite. Avant son texte :

- **qui l’a envoyé**, et à quel point c’est vérifié : *vérifié*, *non vérifié* ou *falsifié*. Survoler le bouclier à côté montre chaque vérification ([comment un expéditeur est vérifié](mail.md#how-a-sender-is-checked)) ;
- **à qui**, en copie à qui, quand, l’objet ;
- **pourquoi il est ici**, replié jusqu’à ce que vous le demandiez.

Puis le texte, rendu sûr :

- Le courrier HTML garde ses paragraphes, ses listes, son texte en gras et ses liens. Rien d’autre n’est montré : pas d’images, pas de styles, pas de scripts. Rien ne se charge depuis le réseau, rien ne s’exécute.
- Les messages précédents qu’une réponse cite sont repliés sous **Afficher les messages précédents**. Une signature est atténuée.
- Chaque lien montre son adresse complète sous le message avant de s’ouvrir : tant que le pointeur est dessus, ou après un premier toucher sur un écran tactile.
- Les **Pièces jointes** sont repliées, une ligne chacune, avec leur type, leur nom et leur taille. En ouvrir ou en enregistrer une lance d’abord l’antivirus ; si l’ordinateur n’en a pas, Sioul le dit et demande avant d’ouvrir. Un téléphone n’en a pas que Sioul puisse appeler : là, Sioul dit que le fichier n’est pas vérifié. Un programme n’est jamais lancé depuis un courriel. Voir [Courrier, Les pièces jointes](mail.md#attachments).

Depuis le message : répondre, transférer, archiver, supprimer, et le reste, comme sur la page [Courrier](mail.md). Une lettre d’information a aussi **Se désabonner** ([Courrier](mail.md#unsubscribing)).

## Terminé pour l’instant {#done-for-now}

**Terminé pour l’instant** ferme le Porche sur ce qu’il a montré. Tout ce qui est plus récent attend sa prochaine ouverture, même le courrier qui a atteint le serveur plus tôt mais n’est arrivé ici qu’ensuite.

Fermer le Porche ne dit rien au serveur : aucun message n’est marqué comme lu par là. Seule l’ouverture d’un message le marque comme lu, comme dans tout logiciel de courrier.

Une adresse sur laquelle le Porche n’a jamais été fermé ne montre que ses deux dernières semaines. Le courrier plus ancien reste dans [Courrier](mail.md), à sa place.

## Au-dessus des files {#above-the-lanes}

Quand il y a quelque chose, quelques lignes viennent avant les files :

- **Quand vos heures ne sont pas réglées**, une carte les demande, avec **Régler mes heures** (qui ouvre les Paramètres à cet endroit) et **Laisser ainsi** (qui arrête de demander).
- **Quand votre nuit n’est pas réglée**, une carte dit que rien n’éloigne les notifications pendant que vous dormez, avec **Régler ma nuit** (qui ouvre la page Santé là où se règlent les repas et la nuit) et **Laisser ainsi**. Voir [Les heures](hours.md#sleep).
- **Où vous en étiez** : la ligne que vous avez laissée quand quelque chose est venu vous interrompre, avec sa tâche, jusqu’à ce que vous appuyiez sur **C’est fait**. Voir [Les tâches](tasks.md#starting-and-stopping).
- **Deux événements en même temps aujourd’hui**, le temps d’y aller et d’en revenir compté, avec **Ouvrir « … »** pour chacun et **Ne plus en parler**. Voir [L’agenda](agenda.md#two-events-at-once).
- **Les prises du jour pas encore marquées**, à partir de leur heure, qu’une notification vous les ait rappelées ou non : chacune avec son heure, son nom et **Pris** (plus d’une demi-heure en retard, **Pris…** demande quand vous l’avez prise). Quelles que soient vos heures : une prise n’est pas du courrier. Chacune reste jusqu’à ce que vous la marquiez, que la journée finisse ou que douze heures soient passées ; pendant votre sommeil, si les prises restent silencieuses, elles attendent votre réveil. Quand un autre appareil peut en savoir plus, le doute est dit sous la prise : vérifiez avant de la prendre. Voir [La santé](health.md#reminders).
- **Les prises prévues pendant que Sioul était fermé**, ni marquées ni rappelées nulle part : **Pris…** (quand vous l’avez prise) ou **Pas pris**. Quand un autre appareil peut en savoir plus, le doute est dit sous la prise. Voir [La santé](health.md#reminders).
- **Les appels refusés par Sioul**, sur votre téléphone et sur vos autres appareils, chacun à un moment où son auteur peut vous joindre : « Pendant votre sommeil, un numéro absent de vos contacts a appelé à 09:30. », avec **Répondre par SMS**, **Rappeler**, **Écouter** quand Free a envoyé le message vocal par courriel. Voir [Les appels](calls.md#afterwards-on-the-porch).
- **De votre téléphone**, sur un ordinateur, quand vous y envoyez les messages de votre téléphone : qui a écrit, quand et quoi, à un moment où la personne peut vous joindre, avec **Répondre par SMS**, **Rappeler** et **Vu**. Voir [Ce qui vous joint](notifications.md#messages-on-your-computers).
- **« Du nouveau sur &lt;site&gt; »** : ce que les sites que vous gardez dans Sioul ont notifié, et qui vous attend. Ouvrir le site efface ses nouvelles. Voir [Les sites](sites.md).
- **Le courrier papier** que vous avez scanné, chaque lettre en carte : qui, quoi, combien, pour quand. Voir [Les papiers et les lettres](papers.md#paper-letters).
- **Les paiements de la semaine**, en une ligne : « Cette semaine : Électricité 62 € (lun.). Le compte les tient. » Quand quelque chose demande un coup d’œil côté argent, la ligne le dit, sans nombre. Voir [Les budgets](budgets.md#the-bank-watch).

<figure markdown="span">
  [![Une carte en haut du Porche : « Vos heures ne sont pas réglées : travail et démarches arrivent à toute heure. », une phrase sur ce qu’apporte chaque type d’heures, et deux boutons, « Régler mes heures » et « Laisser ainsi ».](../assets/screens/fr/porch-hours.png){ loading=lazy }](../assets/screens/fr/porch-hours.png "Ouvrir l’image en grand")
  <figcaption>Tant que vos heures ne sont pas réglées, le Porche demande une fois.</figcaption>
</figure>

## Pourquoi il fonctionne ainsi {#why-it-works-this-way}

- Relever son courrier trois fois par jour a fait baisser le stress quotidien dans un essai randomisé (Kushlev & Dunn 2015). Regrouper les notifications a aidé l’attention et l’humeur, alors que n’en recevoir aucune rendait les personnes plus anxieuses (Fitz et al. 2019) : ce qui aide, c’est la prévisibilité, pas le silence. Le Porche ouvre donc aux heures que vous choisissez, et dit quand.
- Les personnes interrompues travaillent plus vite, avec plus de stress et de frustration (Mark, Gudith & Klocke 2008), et une notification laissée sans réponse coûte quand même de l’attention (Stothart, Mitchum & Yehnert 2015). Rien ne surgit donc hors des heures que vous choisissez, une seule notification dit tout un lot, et rien ne bouge sous vos yeux quand le courrier arrive.
- On évite les informations dont on attend qu’elles fassent mal (Sweeny et al. 2010). Ce qu’est un message, qui l’a envoyé et à quel point c’est vérifié viennent donc avant son texte.

Plus de détails dans [ce que dit la recherche (en anglais)](https://aurelienpierre.github.io/sioul/dev/research.html).

## Pour aller plus loin {#going-further}

### Temps réel {#real-time}

La case **Temps réel**, en haut du Porche, relève chaque dossier de chaque adresse toutes les minutes, et garde le Porche ouvert : pour un code ou un mot de passe que vous attendez. Décochez-la pour revenir au rythme habituel.

Pendant le calme, **Travailler maintenant** apparaît à côté, pour montrer le travail quelles que soient les heures. Voir [Les heures](hours.md#work-now).

### Une adresse publique, protégée {#a-public-address-protected}

Une adresse que vous publiez apporte du travail, et parfois des insultes. Sur sa fiche dans [Comptes](accounts.md#a-mail-address), **Protégée contre le harcèlement** fait lire son courrier sur votre appareil avant que vous le voyiez :

- **son ton** : calme, grossier, ou hostile (insultes qui vous visent, harcèlement, menaces), à partir de listes de mots en français et en anglais ;
- **son sujet** : travail, une question, la presse, des remerciements, un don, autre chose.

Le courrier hostile va dans sa propre file, repliée, qui ne montre ni le nom de l’expéditeur ni l’objet. En ouvrir un demande d’abord : il peut attendre, aller à quelqu’un de confiance (**Transférer à quelqu’un de confiance**), ou partir (bloqué, supprimé). Le lire quand même est votre choix, au moment qui vous convient. Le reste du courrier de l’adresse a sa propre file, le travail en premier, avec son sujet affiché, et une marque quand il est grossier.

**Laisser l’IA le lire d’abord**, en dessous, reste désactivé tant que vous ne l’activez pas. Les messages de la boîte de réception de cette adresse sont alors envoyés à Claude, d’Anthropic, quel que soit leur expéditeur, y compris ceux qui s’y trouvent déjà quand vous l’activez, pour en dire le ton et le sujet plus finement que des listes de mots. Ce qui quitte votre appareil, c’est l’objet de chaque message et les 4 000 premiers caractères de son texte, tels quels : rien n’est masqué, si bien qu’un code ou un numéro de compte dans un tel message parvient aussi à Anthropic. La clé du service d’Anthropic se tape une fois dans Comptes et reste dans votre trousseau.

### Les réglages du Porche {#the-porchs-settings}

Le ⚙ en haut du Porche contient ce qui n’appartient qu’au Porche, et la lecture d’un message ouvert ici :

- **Projets montrés ici** : quels projets ont une file sur le Porche. Le courrier des autres reste sur leur page dans Projets.
- **Courrier papier ▸ Où arrivent les scans** : le dossier où arrivent vos scans.
- **Appels ▸ Appels refusés par vos téléphones** : activé sauf si vous l’éteignez, les appels refusés par vos téléphones attendent aussi sur le Porche de cet appareil ; désactivé, chaque téléphone ne liste que les siens, et vos ordinateurs aucun ([Les appels](calls.md#on-your-computers)).
- **Lecture du texte** : la police, sa taille et l’interligne d’un message ouvert ici, les mêmes que dans le ⚙ de la page Courrier.
- **Comment le courrier est trié** : chaque file, dans l’ordre du tri, avec ses règles ; les expéditeurs que vous connaissez ; les mots qui rendent un expéditeur automatique (no-reply…).

Ce qui appartient à autre chose se règle là où se trouve cette chose : le rang et la protection d’une adresse sur sa fiche dans [Comptes](accounts.md), les règles d’un projet sur sa page dans [Projets](projects.md), qui peut vous écrire et quand dans Paramètres ▸ [Ce qui vous joint](notifications.md#by-person), vos heures dans [Paramètres](settings.md#hours).

#### Certaines adresses d’abord, d’autres en dernier {#some-addresses-first-others-last}

Sur la fiche de chaque adresse dans Comptes, **Priorité** : *Plus important*, *Normal* ou *Moins important*. Le courrier des adresses plus importantes vient en premier dans chaque file, avec une petite marque. Celui des moins importantes ne passe pas par le filtre d’accueil et attend, replié, en bas. Leurs codes arrivent quand même tout de suite.

## Comparé à d’autres applications {#compared-with-other-apps}

En octobre 2026, d’après la documentation de chaque application.

✓ documenté · en partie, avec une note · ✗ introuvable dans la documentation de l’application (pour Sioul : pas fait) · ? pas confirmé · — sans objet.

HEY porte l’idée la plus proche, avec son Screener et son Imbox ; Gmail, Outlook et Apple Mail rangent le courrier en catégories ou en onglets ; Superhuman découpe la boîte de réception. Proton Mail, Thunderbird et FairEmail sont comparés sur [la page Courrier](mail.md#compared-with-other-apps) : aucun ne retient le premier message d’un inconnu ni ne montre le courrier selon des heures.

| | Sioul | HEY | Gmail | Outlook | Apple Mail | Superhuman |
|---|---|---|---|---|---|---|
| Range de lui-même le nouveau courrier en files | ✓¹ | en partie² | ✓³ | en partie⁴ | ✓⁵ | ✓⁶ |
| Des sections à vous dans la boîte de réception (un projet, un client) | ✓⁷ | ✗⁸ | ✓⁹ | ✗¹⁰ | ✗¹¹ | ✓¹² |
| Le premier message d’un inconnu attend que vous le laissiez entrer | ✓ | ✓¹³ | ✗ | en partie¹⁴ | ✗¹⁵ | ✗ |
| Le nouveau courrier montré seulement aux heures que vous choisissez | ✓¹⁶ | ✗ | ✗ | en partie¹⁷ | en partie¹⁸ | ✗ |
| Des notifications selon qui écrit, et quand il peut vous joindre | ✓¹⁹ | en partie²⁰ | en partie²¹ | en partie²² | en partie²³ | en partie²⁴ |
| Le code que vous venez de demander à un site, tout de suite | ✓²⁵ | ✗ | ?²⁶ | ✗ | ✓²⁷ | ✗ |
| Ni compteurs de non-lus ni pastilles | ✓²⁸ | ✓²⁹ | ?³⁰ | ✗³¹ | ✗³² | ✗³³ |
| Courrier falsifié et noms empruntés mis de côté d’abord, avec la raison | ✓ | en partie³⁴ | en partie³⁵ | en partie³⁶ | en partie³⁷ | —³⁸ |
| Ne supprime rien de lui-même | ✓³⁹ | ✗⁴⁰ | ✗⁴¹ | ✗⁴¹ | ✗⁴¹ | —³⁸ |
| Le courrier hostile envoyé à une adresse publique retenu, ses mots cachés | ✓ | ✗⁴² | ✗⁴² | ✗⁴² | ✗⁴² | ✗⁴² |
| Fonctionne avec les adresses que vous avez déjà | en partie⁴³ | ✗⁴⁴ | en partie⁴⁵ | ✓ | ✓ | en partie⁴⁶ |

1. Sioul : les personnes que vous connaissez, quelqu’un de nouveau, un projet, les lettres d’information et les notifications, ce qui est mis de côté.
2. HEY : vous choisissez, pour chaque expéditeur que vous laissez entrer, l’Imbox, The Feed ou la Paper Trail ; « HEY ne déplace rien tant que vous ne le lui dites pas ».
3. Gmail : cinq catégories fixes (Principale, Réseaux sociaux, Promotions, Notifications, Forums).
4. Outlook : deux onglets, Prioritaire et Autres.
5. Apple Mail : quatre catégories fixes (Principal, Transactions, Mises à jour, Promotions), depuis iOS 18.2 et macOS 15.4, pas dans tous les pays.
6. Superhuman : Important et Other, et des découpages tout faits (VIP, Team, News, Calendar).
7. Sioul : une file par projet, selon ses expéditeurs, ses domaines ou des mots.
8. HEY : trois endroits fixes ; à la place, des étiquettes, des regroupements et des enchaînements.
9. Gmail : « Plusieurs boîtes de réception », sur un ordinateur, faites de recherches ou de libellés.
10. Outlook : deux onglets ; à la place, des dossiers et des règles.
11. Apple Mail : des catégories fixes ; sur un Mac, les boîtes aux lettres intelligentes sont des recherches enregistrées à côté de la boîte de réception.
12. Superhuman : des découpages à vous, par expéditeur, destinataire, objet ou libellé.
13. HEY : le Screener ; votre décision reste privée et peut couvrir tout un domaine.
14. Outlook : Sender Screening, pour les comptes Microsoft personnels, désactivé tant que vous ne l’activez pas ; le courrier des nouveaux expéditeurs attend en haut de la boîte de réception que vous les autorisiez ou les bloquiez.
15. Apple Mail n’en a pas ; c’est l’app Messages qui filtre les expéditeurs inconnus.
16. Sioul : hors de vos heures, le Porche dit quand il ouvre et ne montre rien d’autre ; les codes et ce que vous vous envoyez arrivent quand même.
17. Outlook : sur un téléphone, les notifications peuvent se taire selon un horaire ; le courrier lui-même reste affiché.
18. Apple : un mode de concentration, qui est un réglage du système, peut ne montrer que certains comptes à des heures choisies.
19. Sioul : une seule notification discrète par lot, aux heures que la liste de chaque expéditeur permet.
20. HEY : notifications désactivées par défaut, activées pour les personnes ou les fils que vous choisissez ; pas d’horaire dans sa documentation.
21. Gmail : tout le courrier, la priorité haute seulement, ou rien ; par libellé sur Android.
22. Outlook : sur un téléphone, tout le courrier, Prioritaire seulement, ou rien, et des heures de calme.
23. Apple Mail : les VIP et les boîtes choisies ; les heures par le mode de concentration du système.
24. Superhuman : la priorité haute seulement, ou les découpages choisis sur un téléphone ; pas d’horaire dans sa documentation.
25. Sioul : une notification avec **Copier** sur un ordinateur ; sur un téléphone, sur le Porche et la carte de l’écran d’accueil, sans notification pour l’instant.
26. Gmail : pas dans la documentation de Google ; seule la presse rapporte une carte « Copier le code » dans ses applications.
27. Apple : les codes reçus dans Mail se remplissent d’eux-mêmes dans Safari, depuis iOS 17 et macOS Sonoma.
28. Sioul n’en montre aucun ; sur Android, un lanceur peut quand même mettre un point sur l’icône de Sioul pendant qu’une notification de courrier attend.
29. HEY : pas de compteurs, et pas de pastilles, par principe.
30. Gmail : pas confirmé dans la documentation de Google.
31. Outlook : la pastille de l’application compte le courrier non lu, tout ou Prioritaire seulement ; iOS peut la désactiver.
32. Apple Mail : des pastilles par compte, qu’on peut désactiver ; sur un iPhone, le nombre est celui de la catégorie Principal.
33. Superhuman : chaque découpage montre son nombre de conversations ; la pastille sur ordinateur compte le découpage en cours.
34. HEY : un avertissement sur le courrier d’un expéditeur que vous avez laissé entrer quand il échoue aux vérifications de son domaine.
35. Gmail : un point d’interrogation sur le courrier non authentifié ; un indésirable dit pourquoi, une adresse qui imite celle d’un expéditeur connu parmi les raisons.
36. Outlook : un « ? » et « via » ; la détection d’usurpation seulement dans Defender pour Office 365, pour les entreprises.
37. Apple : iCloud Mail applique la politique DMARC du domaine de l’expéditeur sur ses serveurs ; les guides d’Apple ne montrent aucune raison donnée dans Mail.
38. Superhuman lit des boîtes Gmail ou Microsoft 365 : leurs vérifications et leurs suppressions s’appliquent.
39. Sioul ne supprime rien ; votre fournisseur peut vider son propre dossier Indésirables (Gmail au bout de 30 jours).
40. HEY : les indésirables et le courrier refusé à l’accueil supprimés au bout de 90 jours.
41. Les indésirables supprimés au bout de 30 jours (Outlook.com : entre 10 et 30 jours ; Apple : dans iCloud Mail).
42. Ils permettent de bloquer ou de refuser un expéditeur une fois qu’on l’a lu ; aucun ne retient le courrier selon son ton.
43. Sioul : tout serveur IMAP et SMTP ; pas Outlook.com, Hotmail ni Microsoft 365.
44. HEY : les adresses HEY seulement ; l’autre courrier peut y être transféré.
45. Gmail : les autres comptes dans ses applications pour téléphone ; sur le web, jusqu’en janvier 2027.
46. Superhuman : les comptes Gmail et Microsoft 365 seulement.

??? info "Sources (en anglais)"
    Le centre d’aide de Superhuman a été lu par sa propre interface publique, ses pages refusant ce jour-là la lecture automatique.

    - HEY, fonctions (le Screener, l’Imbox, The Feed, Paper Trail, notifications, enchaînements) : <https://www.hey.com/features/>, lu le 8 octobre 2026.
    - HEY, la façon HEY (pas de nombres) : <https://www.hey.com/the-hey-way/>, lu le 8 octobre 2026.
    - HEY, notifications (pas de pastilles) : <https://help.hey.com/article/772-notifications>, lu le 8 octobre 2026.
    - HEY, expéditeurs usurpés : <https://help.hey.com/article/741-how-did-a-spammer-spoof-my-email-address>, lu le 8 octobre 2026.
    - HEY, vider la corbeille, les indésirables et le courrier refusé (90 jours) : <https://help.hey.com/article/1014-empty-trash-spam-or-screened-out>, lu le 8 octobre 2026.
    - HEY, étiquettes : <https://help.hey.com/article/884-labels>, lu le 8 octobre 2026.
    - HEY, questions fréquentes (pas d’IMAP, courrier transféré) : <https://www.hey.com/faqs/>, lu le 8 octobre 2026.
    - HEY, applications : <https://www.hey.com/apps/>, lu le 8 octobre 2026.
    - Gmail, catégories : <https://support.google.com/mail/answer/3094499>, lu le 8 octobre 2026.
    - Gmail, types de boîte de réception et « Plusieurs boîtes de réception » : <https://support.google.com/mail/answer/186531>, lu le 8 octobre 2026.
    - Gmail, notifications : <https://support.google.com/mail/answer/1075549>, lu le 8 octobre 2026.
    - Gmail, authentification : <https://support.google.com/mail/answer/180707>, lu le 8 octobre 2026.
    - Gmail, indésirables et raisons affichées : <https://support.google.com/mail/answer/1366858>, lu le 8 octobre 2026.
    - Gmail, indésirables et corbeille supprimés au bout de 30 jours : <https://support.google.com/mail/answer/7015314>, lu le 8 octobre 2026.
    - Gmail, autres comptes : <https://support.google.com/mail/answer/16604719> et <https://support.google.com/mail/answer/17101213>, lu le 8 octobre 2026.
    - Outlook, boîte de réception Prioritaire : <https://support.microsoft.com/en-us/outlook/mail/focused-inbox-for-outlook>, lu le 8 octobre 2026.
    - Outlook, Sender Screening : <https://support.microsoft.com/en-us/outlook/sender-screening>, lu le 8 octobre 2026.
    - Outlook, notifications et heures de calme : <https://support.microsoft.com/en-us/outlook/how-can-i-turn-push-notifications-and-sounds-on-or-off>, lu le 8 octobre 2026.
    - Outlook, la pastille : <https://support.microsoft.com/en-us/outlook/what-does-the-badge-count-icon-represent-and-how-do-i-change-it>, lu le 8 octobre 2026.
    - Outlook, hameçonnage et comportements suspects : <https://support.microsoft.com/en-us/outlook/mail/phishing-and-suspicious-behavior-in-outlook>, lu le 8 octobre 2026.
    - Outlook, politiques anti-hameçonnage de Defender pour Office 365 : <https://learn.microsoft.com/en-us/defender-office-365/anti-phishing-policies-about>, lu le 8 octobre 2026.
    - Outlook, filtre du courrier indésirable (30 jours) : <https://support.microsoft.com/en-us/outlook/filter-junk-email-and-spam-in-outlook>, lu le 8 octobre 2026.
    - Outlook, courrier mis aux indésirables par erreur (10 à 30 jours) : <https://support.microsoft.com/en-us/office/mail-goes-to-the-junk-folder-by-mistake-f409b58c-2617-47e2-8a97-cab612d98eff>, lu le 8 octobre 2026.
    - Apple Mail, catégories : <https://support.apple.com/guide/iphone/use-categories-iphfe4a36baf/ios>, lu le 8 octobre 2026.
    - Apple, versions qui ont les catégories : <https://support.apple.com/en-us/121161> et <https://support.apple.com/en-us/120283>, lu le 8 octobre 2026.
    - Apple Mail, règles et boîtes aux lettres intelligentes sur un Mac : <https://support.apple.com/guide/mail/automatically-sort-incoming-emails-mlhlp1190/mac>, lu le 8 octobre 2026.
    - Apple Mail, filtres de concentration : <https://support.apple.com/guide/iphone/filter-emails-iph057d5e515/ios>, lu le 8 octobre 2026.
    - Apple Mail, notifications et VIP : <https://support.apple.com/guide/iphone/set-email-notifications-iphc13a970c8/ios>, lu le 8 octobre 2026.
    - Apple Mail, consulter son courrier (la pastille) : <https://support.apple.com/guide/iphone/check-your-email-iph461684497/ios>, lu le 8 octobre 2026.
    - Apple, les codes reçus dans Mail : <https://support.apple.com/en-us/118723>, lu le 8 octobre 2026.
    - Apple, annonce de macOS Sonoma (les codes de Mail dans Safari) : <https://www.apple.com/newsroom/2023/06/macos-sonoma-brings-new-capabilities-for-elevating-productivity-and-creativity/>, lu le 8 octobre 2026.
    - Apple, filtrer les expéditeurs inconnus dans Messages : <https://support.apple.com/guide/iphone/screen-and-filter-texts-iph203ab0be4/ios>, lu le 8 octobre 2026.
    - Apple, vérifications d’iCloud Mail : <https://support.apple.com/en-us/102322>, lu le 8 octobre 2026.
    - Apple, courrier indésirable dans iCloud (30 jours) : <https://support.apple.com/guide/icloud/manage-junk-mail-mm6b1a2ced/icloud>, lu le 8 octobre 2026.
    - Superhuman, Split Inbox par défaut : <https://help.superhuman.com/hc/en-us/articles/46005619081101-Default-Split-Inbox>, lu le 8 octobre 2026.
    - Superhuman, Split Inbox personnalisé : <https://help.superhuman.com/hc/en-us/articles/46005636204941-Custom-Split-Inbox>, lu le 8 octobre 2026.
    - Superhuman, notifications (et pastilles) : <https://help.superhuman.com/hc/en-us/articles/46005802618765-Email-Notifications>, lu le 8 octobre 2026.
    - Superhuman, gérer les comptes : <https://help.superhuman.com/hc/en-us/articles/46005777934733-Managing-Accounts>, lu le 8 octobre 2026.
    - Superhuman, éviter les indésirables (le filtrage est celui du fournisseur) : <https://help.superhuman.com/hc/en-us/articles/46005520093453-Keeping-Your-Emails-Out-of-Spam>, lu le 8 octobre 2026.
    - Superhuman, téléchargement (plateformes) : <https://help.superhuman.com/hc/en-us/articles/46005778798605-Download-Superhuman-Mail>, lu le 8 octobre 2026.

## Côté technique {#for-technical-readers}

### L’ordre des files {#the-order-of-the-lanes}

Les files sont décidées dans cet ordre, ce qui vous protège le plus d’abord : mis de côté (falsifié, un nom emprunté, l’indésirable selon votre fournisseur sur le courrier d’un inconnu) ; hostile, pour une adresse protégée ; ce que votre propre filtre a retenu ; les codes et les liens ; un projet ; le courrier de vous-même, quand il est vérifié ; la file propre d’une adresse protégée ; les adresses moins importantes ; les lettres d’information et les expéditeurs automatiques (`List-Id`, `List-Unsubscribe`, `Precedence: bulk`, les adresses comme no-reply) ; l’attente de votre accord ; les personnes que vous connaissez. Un message d’une conversation qu’un projet tient va aussi dans ce projet, sauf s’il a été mis de côté, si c’est un code ou s’il est hostile. Les expéditeurs bloqués sont écartés avant tout cela.

### Avant qu’aucune liste soit lue {#before-any-list-is-read}

Chaque message est d’abord vérifié ([comment un expéditeur est vérifié](mail.md#how-a-sender-is-checked)). Le courrier falsifié et les noms empruntés sont mis de côté quoi que disent vos listes, même quand ils nomment quelqu’un que vous avez marqué sûr. Le courrier que rien n’authentifie (SPF et DKIM ont tous deux échoué, sans DMARC qui passe ni sceau ARC de votre fournisseur ou de l’un de vos domaines) est lu comme celui d’un inconnu : pas de file des personnes connues, les heures d’un inconnu, aucune protection contre les avis sur les indésirables, et aucune file de projet par son adresse. Les avis sur les indésirables ne s’appliquent qu’aux inconnus, ceux de votre fournisseur (les en-têtes de SpamAssassin ou de rspamd, lus seulement là où votre fournisseur les a écrits) comme ceux de votre propre filtre : jamais à vos carnets d’adresses, à vos listes, à ceux que vous avez laissés entrer, à un code, au courrier d’un projet, à votre propre courrier vérifié, ni à un message dont vous avez dit qu’il n’était pas indésirable, sur n’importe quel appareil ou dans un autre logiciel de courrier (`$NotJunk`).

### Les codes {#codes}

Chaque boîte de réception reste ouverte avec IMAP IDLE (RFC 2177), renouvelé toutes les cinq minutes : un code vous parvient quelques secondes après avoir atteint votre fournisseur. Le détecteur lit le français et l’anglais : les codes de 4 à 8 chiffres, groupés ou préfixés (`123-456`, `G-482913`), lettres et chiffres à côté de leurs mots ; les mots de passe temporaires, les réinitialisations, les liens de connexion et les adresses à confirmer. Les dates, les prix, les numéros de téléphone, les codes postaux et les codes des magasins sont écartés. Avec les en-têtes d’une lettre d’information, seuls l’objet et les 1 000 premiers caractères sont lus, là où un site met ce que vous avez demandé. Un code dure ce que dit son message, une semaine au plus ; sinon 30 minutes, un lien de connexion une heure, une réinitialisation deux heures, une adresse à confirmer un jour, un mot de passe temporaire une semaine. Un code qui échoue à DMARC sous la politique de son domaine est falsifié, en-têtes de liste ou non.

### Ce que le Porche garde, et où {#what-the-porch-keeps-and-where}

La relève utilise `EXAMINE` et `BODY.PEEK` : rien n’est marqué comme lu. **Terminé pour l’instant** est gardé sur votre appareil, comme le message le plus récent montré pour chaque adresse (son UIDVALIDITY et son UID), et rejoint vos autres appareils scellé quand vous partagez entre eux ; le serveur n’en sait jamais rien.

### Les notifications {#notifications}

Chaque appareil tient un registre de ce qu’il a dit, en empreintes (aucune adresse, aucun Message-ID), pendant 30 jours, pour que rien ne soit dit deux fois. Avec un téléphone et un ordinateur, seul l’appareil dont vous vous êtes servi en dernier le dit. Seul un lot qui contient quelqu’un qui passe toujours traverse « Ne pas déranger ».

### La lecture par l’IA d’une adresse protégée {#the-ai-reading-of-a-protected-address}

Désactivée tant que vous ne l’activez pas pour une adresse. Chaque message de la boîte de réception de cette adresse que l’IA n’a pas encore lu, quel que soit son expéditeur, est alors envoyé à l’API d’Anthropic (Claude Haiku 4.5), les plus récents d’abord, vingt à la fois : son objet et les 4 000 premiers caractères de son texte, sans rien masquer. Claude répond par un ton, un sujet et une ligne neutre ; la réponse est gardée sur votre appareil sous le Message-ID du message et une empreinte de qui l’a envoyé et de ce qu’il dit, pour que le message ne soit pas envoyé de nouveau, et qu’un message qui emprunte le Message-ID d’un autre soit lu à nouveau. Une réponse qui n’a pas la forme demandée n’est pas gardée : les listes de mots jugent ce message en attendant, et il est demandé de nouveau plus tard. La clé du service d’Anthropic reste dans votre trousseau, et aucune redirection n’est suivie, pour que la clé n’aille qu’à Anthropic.
