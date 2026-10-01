---
type: brief
slug: mcp-server
title: L'agent dans la session live, bunflared en serveur MCP avec un mode local
status: ready
created: 2026-10-02
next_action: Ajouter `bunflared mcp`, un serveur MCP sur stdio qui ouvre ses propres partages, parle aux pages, lit les notes et rejoue les requêtes, et un mode `--local` sans Cloudflare
resume_cmd: /ship docs/brief/mcp-server
base: main
branch: feat/mcp-server
issues: "#3"
---

# L'agent dans la session live, bunflared en serveur MCP avec un mode local

## Problème

Un agent de code sait ouvrir un partage avec `--detach` et lire les notes de
`bunflared-feedback/`, mais il reste hors de la session live. Il ne peut pas
répondre au visiteur, l'envoyer sur la page qu'il vient de corriger,
recharger cette page, ni regarder et rejouer les requêtes passées par le
lien. Tout ça n'existe que dans le dashboard, donc il faut un humain devant
le terminal.

Et le widget n'existe que derrière un lien Cloudflare. Un développeur seul
sur sa machine ne peut pas s'en servir comme canal entre son onglet et son
agent : pointer un élément de sa propre page, laisser une note, et voir
l'agent corriger et recharger. Chaque essai coûte un lien public, et
Cloudflare bloque après une vingtaine de liens rapprochés.

## Utilisateurs

- Le développeur qui travaille avec un agent de code (Claude Code d'abord)
  sur son app locale.
- L'agent, qui pilote le partage et la conversation par des outils MCP.
- Le client ou le coéquipier sur le lien public, qui parle à l'agent comme
  il parle déjà au dashboard.

## Objectifs

- L'agent fait tout ce que fait le dashboard : ouvrir, lister et fermer un
  partage, parler aux visiteurs, les guider, recharger, lire les notes,
  inspecter et rejouer les requêtes.
- Un mode local, sans tunnel ni compte, pour boucler entre l'onglet du
  développeur et son agent.
- Ce qu'écrit le visiteur arrive à l'agent sans que l'agent tourne en
  boucle pour le chercher.

## Acceptance criteria

### Mode local

- AC1 `bunflared 5173 --local` sert l'app avec le widget sur
  `http://127.0.0.1:<port>` en moins d'une seconde. Le dashboard, `--json`
  et `--detach` marchent comme avec un lien, et le champ `url` de la ligne
  JSON est cette adresse locale. Aucune requête ne part vers Cloudflare :
  cloudflared n'est ni lancé, ni téléchargé.
- AC2 L'adresse locale ne répond que depuis la machine, jamais depuis le
  réseau.

### Serveur MCP

- AC3 `bunflared mcp` parle MCP sur stdio. Après
  `claude mcp add bunflared -- bunflared mcp`, ses outils apparaissent dans
  Claude Code. Rien d'autre que des messages MCP ne sort sur stdout.
- AC4 L'agent ouvre un partage (ports, local ou public) et reçoit son id et
  son adresse. Il peut en ouvrir plusieurs, les lister, en fermer un. Un
  échec (port muet, `~/.cloudflared/config.yaml`, cloudflared introuvable)
  revient comme une erreur d'outil lisible, et le serveur reste debout.
- AC5 Chaque partage ouvert par l'agent apparaît dans `bunflared ls`, et
  `bunflared down <id>` ferme celui-là seulement : les autres partages de
  l'agent continuent.
- AC6 Quand la session de l'agent se termine (stdin fermé ou signal), tous
  ses partages se ferment et aucun cloudflared ne reste en vie.
- AC7 Les notes et le journal `session_<date>.md` arrivent dans
  `bunflared-feedback/` du projet sur lequel l'agent travaille.

### Parler aux pages

- AC8 L'agent voit qui est sur un partage : chaque visiteur, son appareil,
  sa page.
- AC9 L'agent envoie un message à un visiteur ou à tous. Il s'affiche sur la
  page en bulle, comme un message du dashboard, et entre dans le journal de
  session.
- AC10 L'agent envoie un visiteur, ou tout le monde, sur un chemin, et
  recharge une page ou toutes.
- AC11 L'agent lit ce qui s'est passé depuis son dernier regard : messages,
  notes (texte, page, élément pointé, chemin du fichier Markdown et de la
  capture), réactions, arrivées et départs. Il peut attendre jusqu'à N
  secondes le prochain événement, pour dire « laisse ta note, je regarde »
  sans tourner en boucle.
- AC12 Quand la session Claude Code active les channels, chaque message et
  chaque note d'un visiteur arrive tout seul dans la session de l'agent.
  Sans channels, rien ne change et AC11 suffit.

### Requêtes

- AC13 L'agent liste les requêtes récentes (méthode, chemin, statut, durée,
  visiteur), en lit une en détail (en-têtes, corps tronqués à la même
  limite que le dashboard) et en rejoue une, avec le nouveau statut en
  retour. Une requête au-delà de la limite le dit et n'est pas rejouée.

### Docs

- AC14 Le README, `--help`, `skills/bunflared/SKILL.md` et la note de
  `bunflared agents` décrivent `--local`, `bunflared mcp`, la commande pour
  l'ajouter à Claude Code et le flag des channels. Les sorties JSON et
  `--detach` des commandes existantes ne changent pas.

## Success metrics

- Le scénario de l'issue #3 passe de bout en bout depuis Claude Code, en
  mode local : le développeur pointe un élément et laisse une note, l'agent
  la lit, corrige, recharge la page, et le développeur voit le changement
  sans toucher au navigateur.
- En mode local, aucun processus cloudflared et aucune connexion vers
  `trycloudflare.com` pendant tout le scénario.
- Le même scénario se rejoue dix fois de suite sans aucune limite de débit.
- Avec les channels, un message tapé dans le widget arrive dans la session
  de l'agent en moins de deux secondes.

## Out-of-scope

- Piloter depuis MCP un partage lancé par le dashboard ou par `--detach` :
  chaque processus garde ses partages.
- Le transport MCP HTTP. Stdio seulement.
- Inscrire automatiquement le serveur MCP dans chaque agent :
  `bunflared agents` documente la commande, il ne l'exécute pas.
- Faire entrer bunflared dans la liste approuvée des channels d'Anthropic.
- Suivre le pointeur d'un visiteur, envoyer des réactions ou modifier une
  requête avant de la rejouer depuis l'agent.
- Un port local stable d'un partage à l'autre, et un mot de passe sur
  l'adresse locale.

## Contraintes et hypothèses

- Décision prise : le serveur MCP fait tourner lui-même ses partages. Pas
  de canal entre processus vers les partages `--detach`.
- Décision prise : aucune nouvelle dépendance. MCP sur stdio, c'est du
  JSON-RPC, un message par ligne, et rien d'autre sur stdout ; les logs vont
  sur stderr
  ([spec stdio](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio)).
  `serde_json` et tokio sont déjà là.
- La spec 2026-07-28 est sans état : chaque requête porte sa version dans
  `_meta`, les clients récents sondent avec `server/discover`, les anciens
  passent par `initialize`, et le serveur ne doit pas traiter la connexion
  comme une session
  ([spec](https://modelcontextprotocol.io/specification/2026-07-28/basic/index)).
  « Depuis son dernier regard » (AC11) passe donc par un curseur que l'agent
  renvoie, pas par une mémoire par connexion. La génération que parle Claude
  Code est à constater au premier test.
- Channels : research preview, Claude Code 2.1.80 ou plus, connexion
  claude.ai ou Console. Le serveur déclare `experimental['claude/channel']`
  et émet `notifications/claude/channel` (`content`, `meta`). Un serveur
  hors liste approuvée se lance avec
  `claude --dangerously-load-development-channels server:bunflared`, et les
  événements sont perdus en silence quand les channels sont coupés
  ([channels reference](https://code.claude.com/docs/en/channels-reference)).
- Hypothèse à vérifier : Claude Code lance un serveur stdio depuis le
  dossier du projet, ce qui place `bunflared-feedback/` au bon endroit.
- `bunflared down` envoie aujourd'hui un signal au processus du partage.
  Pour un partage de l'agent, ce processus est le serveur MCP, qui doit
  rester debout pour ses autres partages (AC5).
- Les ids de partage valent aujourd'hui le pid du processus. Un serveur MCP
  qui tient plusieurs partages a besoin d'un id par partage.
- Le proxy écoute déjà sur `127.0.0.1` seulement (`src/proxy.rs`).
- Le widget charge sa bibliothèque de capture depuis `cdn.jsdelivr.net` :
  le mode local se passe de Cloudflare, pas du réseau, pour les captures.
- Cloudflare bloque après une vingtaine de liens rapprochés : les tests de
  bout en bout tournent en mode local, avec un seul lien public par série
  pour vérifier AC4 à AC6 en public.

## Boundary

Owns:
- `src/` sauf `src/widget.js`
- `README.md`, `skills/bunflared/`

Must not touch:
- `src/widget.js`
- `.github/`, `dist-workspace.toml`, `docs/demo/`
- `Cargo.toml` dependencies
