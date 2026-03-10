# Telemetry Shrinker Agent

**Telemetry Shrinker** est un agent de telemetrie ultra-leger ecrit en **Rust**, concu pour reduire drastiquement les couts de stockage et de transfert de logs dans le Cloud (AWS, Azure, Google Cloud).

Il est particulierement adapte pour tourner sur des infrastructures a ressources limitees comme le **Raspberry Pi** ou dans des environnements **Kubernetes**.

## Pourquoi utiliser Shrinker ?

Dans une infrastructure moderne, 70% des logs sont du "bruit" (repetitions, messages de succes inutiles). Les fournisseurs Cloud facturent au volume.
Shrinker permet de :
- **Reduire le volume de logs** de 60% a 90% via un dedoublonnage intelligent.
- **Securiser les donnees** en masquant automatiquement les adresses IP (IPv4 et IPv6).
- **Parser les logs JSON** (Kubernetes, Docker) automatiquement.
- **Alerter en temps reel** via Webhook (Discord/Slack) en cas d'erreur critique.
- **Economiser de l'argent** en ne transmettant que les informations critiques.

## Fonctionnalites

| Fonctionnalite | Description |
|----------------|-------------|
| Deduplication intelligente | Regroupe les messages repetes avec un compteur `[x6]` |
| Masquage IPv4/IPv6 | Remplace les IPs par `[MASKED_IPv4]` / `[MASKED_IPv6]` |
| Parsing JSON | Extrait `msg`/`message` + `level` des logs JSON (Kubernetes, Docker) |
| Alertes Webhook | Envoie une notification Discord/Slack si un message depasse un seuil |
| Rate limiting alertes | `cooldown_minutes` : evite le spam (max 1 alerte par X min pour le meme message) |
| Mode verbose | Affiche chaque ligne traitee et les raisons de filtrage |
| Mode quiet | Aucune sortie sauf les logs traites |
| Dry-run | Simule le traitement sans ecrire |
| Patterns d'exclusion | Ignore les lignes contenant certains mots-cles (health check, heartbeat...) |
| Patterns d'inclusion | Ne conserve que les lignes contenant ces mots (error, critical, fatal...) |
| Sortie JSON | `--output-format json` pour chaîner avec jq, Elasticsearch, Loki |
| Webhook via env | `webhook_url: "$DISCORD_WEBHOOK"` pour eviter les secrets en clair |
| `shrinker init` | Genere un `config.yaml` commente et pret a l'emploi |
| Mode watch | `--watch` : surveille un fichier en continu (comme tail -f) |

## Installation

### Pre-requis
- Rust & Cargo installes (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)

### Construction
```bash
git clone https://github.com/KarimHaddadi20/Shrinker.git
cd Shrinker
cargo build --release
```

## Demarrage rapide

```bash
# Generer la configuration
shrinker init

# Analyser un fichier de logs
shrinker --file production.log

# Mode temps reel (pipe Unix)
tail -f /var/log/syslog | shrinker > clean.log

# Mode watch : surveille un fichier en continu (comme tail -f)
shrinker --file app.log --watch

# Mode watch : ignorer le contenu existant, ne traiter que les nouvelles lignes
shrinker --file app.log --watch --skip-initial

# Mode verbose (debug)
shrinker --file app.log --verbose

# Mode silencieux (production)
shrinker --file app.log --quiet

# Surcharger le seuil
shrinker --file app.log --threshold 10

# Desactiver le masquage IP
shrinker --file app.log --no-mask-ips

# Simulation (rien n'est ecrit)
shrinker --file app.log --dry-run

# Sortie JSON (compatible jq, Elasticsearch, Loki)
shrinker --file app.log --output-format json
tail -f syslog | shrinker --output-format json | jq .
```

Format JSON (une ligne par entree, JSONL) : `{"count": 5, "message": "...", "timestamp": "2026-03-07T10:00:00.000Z"}`

## Configuration (`config.yaml`)

```yaml
# Masquer les adresses IP (IPv4 et IPv6)
mask_ips: true

# Seuil de deduplication : conserver uniquement si le message se repete N fois
threshold: 5

# Fichier de sortie (null = stdout, ideal pour les pipes Unix)
output_file: null

# Patterns d'exclusion (case-insensitive)
exclude_patterns:
  - "health check"
  - "heartbeat"
  # - "DEBUG"
  # - "keep-alive"

# Patterns d'inclusion : si defini, seules les lignes contenant ces mots sont conservees
# Laissez vide ou supprimez pour tout conserver
# include_patterns:
#   - "error"
#   - "critical"
#   - "fatal"

# Alertes Webhook (optionnel)
# webhook_url accepte une URL ou une variable d'environnement : $DISCORD_WEBHOOK
alert:
  webhook_url: "$DISCORD_WEBHOOK"   # ou URL en clair
  threshold: 50
  cooldown_minutes: 15   # Max 1 alerte par 15 min pour le meme message (evite le spam)
```

Generez un fichier de configuration avec `shrinker init`.

## Options CLI

```
Usage: shrinker [OPTIONS] [COMMAND]

Commands:
  init   Genere un fichier config.yaml par defaut

Options:
  -f, --file <FILE>           Fichier de log a analyser (stdin si omis)
  -c, --config <CONFIG>       Fichier de configuration YAML [default: config.yaml]
  -t, --threshold <THRESHOLD> Surcharge le seuil du config.yaml
      --no-mask-ips           Desactiver le masquage IP
      --dry-run               Simulation sans ecriture
  -v, --verbose               Affiche chaque ligne traitee
  -q, --quiet                 N'affiche que les erreurs critiques
      --output-format <FMT>   Format de sortie : text (defaut) ou json
  -w, --watch                 Surveille le fichier en continu (requiert --file)
      --skip-initial          En mode watch : ignorer le contenu existant
  -h, --help                  Aide
  -V, --version               Version
```

## Deploiement Ansible

Shrinker est installable automatiquement via un **role Ansible universel** (Debian/Ubuntu, RHEL/CentOS/Fedora, Arch Linux).

Le role est disponible dans un depot separe : **[shrinker_role_ansible](https://github.com/KarimHaddadi20/shrinker_role_ansible)**

```bash
git clone https://github.com/KarimHaddadi20/shrinker_role_ansible.git
ansible-playbook -i inventory.ini playbook.yml
```

Consultez le [README du role](https://github.com/KarimHaddadi20/shrinker_role_ansible#readme) pour la documentation complete.

## RoadMap
- [x] Deduplication intelligente des logs.
- [x] Masquage IPv4 et IPv6.
- [x] Alertes Webhook (Discord/Slack).
- [x] Parsing JSON intelligent (Kubernetes, Docker).
- [x] Deploiement automatise via Ansible.
- [x] CLI documentee avec `--help`, `--verbose`, `--quiet`, `--dry-run`.
- [x] Commande `shrinker init` pour generer la configuration.
- [x] Gestion d'erreurs conviviale (messages clairs, pas de panic).
- [x] Patterns d'exclusion configurables (health check, heartbeat, etc.).
- [x] Sortie JSON (`--output-format json`) pour interoperabilite.
- [x] Webhook via variable d'environnement (securite des secrets).
- [x] Patterns d'inclusion configurables (ne garder que error, critical, etc.).
- [x] Rate limiting des alertes (`cooldown_minutes` pour eviter le spam).
- [x] Mode watch (`--watch`) : surveillance d'un fichier en continu.

---
Projet cree dans le cadre d'un apprentissage Rust oriente **DevOps & Infrastructure**.
