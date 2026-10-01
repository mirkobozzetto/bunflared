---
type: tasks
slug: mcp-server
source_brief: docs/brief/mcp-server/brief.md
---

# Tasks: L'agent dans la session live, bunflared en serveur MCP avec un mode local

## Relevant Files

- `src/share.rs` - ouvre un partage, le mode local y saute le tunnel
- `src/state.rs` - registre des partages, ids, `ls` et `down`
- `src/main.rs` - flags et sous-commandes, `--local` et `mcp`
- `src/live.rs` - le hub : messages, guidage, rechargement vers les pages
- `src/proxy.rs` - proxy local, capture des requêtes, rejeu
- `src/widget.rs` - présence et notes, ce que l'agent lit
- `src/agents.rs` - la note apprise aux agents
- `src/tui/mod.rs` - le dashboard, modèle de ce que l'agent doit pouvoir faire
- `skills/bunflared/SKILL.md`, `README.md` - docs

## Tasks

Ordered. Each task closes the acceptance criteria it names. Les tests de
bout en bout tournent en mode local ; un seul lien public par série.

## T01 - Mode local

Closes: AC1, AC2

- [x] `--local` sert l'app et le widget sur `127.0.0.1` en moins d'une seconde
- [x] dashboard, `--json` et `--detach` marchent en local, `url` locale
- [x] aucun cloudflared lancé ni téléchargé, aucune requête vers Cloudflare
- [x] l'adresse ne répond pas depuis le réseau

## T02 - Serveur MCP et cycle de vie des partages

Closes: AC3, AC4, AC5, AC6, AC7

- [x] `bunflared mcp` répond au client que lance Claude Code, stdout propre
- [x] l'agent ouvre, liste et ferme des partages locaux et publics
- [x] un échec revient en erreur d'outil lisible, le serveur reste debout
- [x] un id par partage, visible dans `bunflared ls`
- [x] `bunflared down <id>` ferme ce partage seul
- [x] fin de session : tous les partages fermés, aucun cloudflared orphelin
- [x] notes et journal dans `bunflared-feedback/` du projet

## T03 - Parler aux pages

Closes: AC8, AC9, AC10, AC11

- [x] l'agent voit les visiteurs, leur appareil et leur page
- [x] message à un visiteur ou à tous, bulle sur la page, ligne au journal
- [x] envoyer sur un chemin, recharger une page ou toutes
- [x] lire les événements depuis un curseur, notes avec élément et fichiers
- [x] attendre jusqu'à N secondes le prochain événement

## T04 - Channels

Closes: AC12

- [x] avec les channels, messages et notes arrivent seuls dans la session
- [x] sans channels, aucune erreur ni changement

## T05 - Requêtes

Closes: AC13

- [x] lister les requêtes récentes
- [x] lire une requête, corps tronqués à la limite du dashboard
- [x] rejouer une requête et rendre le nouveau statut
- [x] une requête hors limite le dit et n'est pas rejouée

## T06 - Docs et scénario de bout en bout

Closes: AC14

- [x] README, `--help`, `SKILL.md` et note des agents à jour
- [x] sorties JSON et `--detach` existantes inchangées
- [x] scénario de l'issue #3 joué depuis Claude Code en mode local, dix fois
