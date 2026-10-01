---
type: review
slug: codebase
title: Revue de la base de code avant la 0.3.0
created: 2026-10-02
scope: tout src/ (Rust et widget.js), Cargo.toml
angles: structure, simplicity, bugs
readers: Claude Code, review:review-reader, Opus 5.5, niveau de la session
dropped: 2
---

# Revue de la base de code avant la 0.3.0

Le découpage en modules suit de vraies responsabilités : proxy, live, widget,
share, state, tunnel, mcp et tui, sans dépendance inutile. Le désordre se
concentre en trois endroits : le dashboard, qui est un objet-dieu, le module
widget, qui fait cinq métiers, et des règles copiées entre le dashboard et le
serveur MCP. Six bugs mineurs à corriger avant la sortie.

## Points

- [x] R01 [major] `src/mcp/host.rs:210` Deux arrêts simultanés du serveur MCP peuvent sortir avant que les partages soient fermés, et laisser cloudflared orphelin. Preuve : `close_all` vide la liste avec `mem::take` ; le second appelant (fin de stdin ou signal) la trouve vide, rend la main tout de suite, et le processus fait `exit(0)` pendant que le premier attend encore `halt()`. Un client qui ferme stdin puis envoie SIGTERM aussitôt déclenche le cas. -> ship
- [x] R02 [major] `src/widget.rs:161` Avec le lien, n'importe qui peut noyer le journal d'événements de l'agent. Preuve : le `sid` des pings n'est jamais vérifié (seul celui de la WebSocket l'est, dans `live::sid`). Chaque ping avec un nouveau sid crée une entrée dans `Seen::presence` (`src/mcp/host.rs:578`) et un événement « arrived ». Le journal de 1000 événements chasse alors les vraies notes, et les visiteurs partis ne sont jamais retirés de la table. -> ship
- [x] R03 [major] `src/proxy.rs:267` Une réponse en flux de type texte ou JSON n'arrive au visiteur qu'une fois terminée. Preuve : pour tout type qui contient `json`, `javascript`, `html`, `css` ou `text/plain`, `adapt` lit le corps en entier (`body.collect()`) pour réécrire les URL localhost. Un flux `text/plain` ou `application/x-ndjson`, comme les jetons d'un LLM, reste figé jusqu'à la fin ; un flux sans fin bloque la page et grossit en mémoire. Probable pour l'impact réel, certain pour le comportement. -> issue [#7](https://github.com/mirkobozzetto/bunflared/issues/7)
- [ ] R04 [major] `src/tui/mod.rs:199` `App` est un objet-dieu : environ 70 champs et un seul `impl` de 950 lignes. Preuve : la même structure porte les statistiques de trafic, les visiteurs et le chat, la machine à états des phases, la saisie, les animations (particules, feu, sprites, shaders) et le rendu. `dashboard.rs` la modifie pendant le dessin (`app.keep_clear` ligne 133, `app.details_scroll` ligne 978). -> propose
- [ ] R05 [major] `src/tui/mod.rs:740` `on_key` fait 186 lignes et mélange cinq métiers. Preuve : quitter, les codes secrets (Konami, « yum »), la fenêtre de détail d'une requête, l'envoi de commandes aux pages, le presse-papier et le navigateur, les feux d'artifice. À traiter avec R04. -> propose
- [ ] R06 [major] `src/widget.rs:140` `widget.rs` fait cinq métiers : les routes HTTP du widget, la lecture des requêtes, l'écriture des notes et des captures sur disque, la mise en Markdown, la détection d'appareil, plus le modèle `Presence` et ses règles de statut, que le dashboard et le serveur MCP importent de ce module HTTP. Preuve : `handle` (140-210) route quatre points d'entrée, émet les événements, prévient le hub et écrit les fichiers via `save_note` et `save_shot` ; `widget.rs` et `live.rs` s'importent l'un l'autre. -> propose
- [ ] R07 [major] `src/share.rs:54` L'énum `Event` sert de bus unique à tout le programme, ce qui crée un cycle d'imports. Preuve : il mêle la progression du tunnel, le trafic, la présence et le chat, le rejeu du dashboard et la fin du partage. `share.rs` importe `live`, `widget` et `proxy`, qui importent tous `share` en retour ; `Hit` vit dans `share.rs` mais porte `proxy::Exchange`. Chaque consommateur filtre un sous-ensemble et finit par `_ => {}`. -> propose
- [x] R08 [minor] `src/mcp/host.rs:599` Un onglet en arrière-plan alterne « left » et « arrived » chaque minute dans les événements de l'agent. Preuve : un visiteur est marqué parti après 20 s sans ping, le widget pinge toutes les 5 s, et un navigateur ralentit les minuteries d'un onglet caché, jusqu'à une par minute après quelques minutes. Le ping suivant relance « arrived ». Probable : dépend du navigateur. -> ship
- [x] R09 [minor] `src/widget.js:438` La réaction ne s'anime jamais sur la page de celui qui clique. Preuve : `say()` renvoie le résultat de `WebSocket.send`, toujours `undefined`, donc `!say(...)` est vrai et la fonction sort avant `float(emoji, false)`. -> ship
- [x] R10 [minor] `src/mcp/mod.rs:51` Un seul octet non UTF-8 sur stdin termine la session MCP et ferme tous les partages. Preuve : `lines()` renvoie une erreur `InvalidData`, et la boucle fait `break`, alors qu'un JSON invalide reçoit une simple erreur -32700. -> ship
- [x] R11 [minor] `src/proxy.rs:211` La boucle d'acceptation tourne à 100 % du processeur quand `accept()` échoue en boucle. Preuve : `continue` sans pause ; quand les descripteurs de fichiers sont épuisés, `accept` échoue immédiatement à chaque tour. -> ship
- [x] R12 [minor] `src/cloudflared.rs:60` Deux premiers partages publics lancés en parallèle téléchargent cloudflared dans le même fichier. Preuve : le chemin `{asset}.part` est fixe ; deux appels `open` simultanés de l'agent, sans cloudflared installé, écrivent et renomment le même fichier, qui peut finir tronqué. Probable : premier lancement seulement. -> issue [#8](https://github.com/mirkobozzetto/bunflared/issues/8)
- [ ] R13 [minor] `src/mcp/host.rs:32` `host.rs` porte le cycle de vie des partages, le journal d'événements, le modèle des visiteurs et la mise en JSON pour l'agent, en 665 lignes. Preuve : `Host::start` (runtime et thread), `events` et `log` (journal avec curseur), `Seen` (visiteurs), `Share::request` (vues JSON). Supportable aujourd'hui, à découper avec R04 et R06. -> propose
- [ ] R14 [minor] `src/state.rs:91` `state.rs` mélange le stockage des fiches de partage et trois commandes : `ls` (affichage), `down` (arrêt de processus) et `--detach` (lancement d'un enfant). Preuve : `list` ligne 91, `down` ligne 129, `detach` ligne 179, à côté de `Record` et `Saved`. -> propose
- [x] R15 [minor] `src/live.rs:371` La même petite fonction `status()` existe dans `live.rs` et `widget.rs`, et les constructeurs de réponses HTTP sont éparpillés. Preuve : `live.rs:371` et `widget.rs:361` sont identiques ; `proxy.rs` a déjà `full`, `plain` et `html`. -> ship

## Ce qui n'est pas logique

- [x] R16 [major] Le dashboard et le serveur MCP appliquent les mêmes règles avec deux copies, qui divergent déjà dans leur formulation. « Une requête est-elle rejouable » : `src/tui/mod.rs:1064` et `src/mcp/host.rs:628`. « Préfixer le chemin par / » : `src/tui/mod.rs:1001` et `src/mcp/host.rs:459`. `clock()` : `src/tui/mod.rs:1292` et `src/mcp/host.rs:663`. Ces règles appartiennent à `Hit` et au `Hub`, pas aux deux interfaces. -> ship

## Vérification

Trois relecteurs Opus 5.5 en lecture seule : structure, simplicité, bugs.
Chaque point a été rouvert à la ligne citée par le lecteur principal.
Confirmés : 13. Probables : 3 (R03 pour l'impact, R08, R12), qui dépendent
d'un navigateur ou d'un premier lancement. Écartés : 2. Le premier : sept
délais de 3 s qui ne mesurent pas la même chose. Le second : « main.rs câble
l'application », ce qui est justement son rôle. GitNexus : index jamais
construit, recherche texte à la place. `node .gitnexus/run.cjs analyze`
l'activerait pour la prochaine revue.
