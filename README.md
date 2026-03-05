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
| Mode verbose | Affiche chaque ligne traitee et les raisons de filtrage |
| Mode quiet | Aucune sortie sauf les logs traites |
| Dry-run | Simule le traitement sans ecrire |
| `shrinker init` | Genere un `config.yaml` commente et pret a l'emploi |

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
```

## Configuration (`config.yaml`)

```yaml
# Masquer les adresses IP (IPv4 et IPv6)
mask_ips: true

# Seuil de deduplication : conserver uniquement si le message se repete N fois
threshold: 5

# Fichier de sortie (null = stdout, ideal pour les pipes Unix)
output_file: null

# Alertes Webhook (optionnel)
alert:
  webhook_url: "https://discord.com/api/webhooks/VOTRE_ID/VOTRE_TOKEN"
  threshold: 50
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

---
Projet cree dans le cadre d'un apprentissage Rust oriente **DevOps & Infrastructure**.
