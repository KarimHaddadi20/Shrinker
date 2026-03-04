# 🚀 Telemetry Shrinker Agent

**Telemetry Shrinker** est un agent de télémétrie ultra-léger écrit en **Rust**, conçu pour réduire drastiquement les coûts de stockage et de transfert de logs dans le Cloud (AWS, Azure, Google Cloud).

Il est particulièrement adapté pour tourner sur des infrastructures à ressources limitées comme le **Raspberry Pi** ou dans des environnements **Kubernetes**.

## 💡 Pourquoi utiliser Shrinker ?

Dans une infrastructure moderne, 70% des logs sont du "bruit" (répétitions, messages de succès inutiles). Les fournisseurs Cloud facturent au volume.
Shrinker permet de :
- **Réduire le volume de logs** de 60% à 90% via un dédoublonnage intelligent.
- **Sécuriser les données** en masquant automatiquement les adresses IP (Anonymisation).
- **Économiser de l'argent** en ne transmettant que les informations critiques.

## ✨ Fonctionnalités

- 🦀 **Performance Rust** : Consommation CPU/RAM proche de zéro.
- 🛡️ **Security First** : Masquage automatique des adresses IPv4 et IPv6.
- 🔍 **JSON Intelligent** : Parsing automatique des logs JSON (Kubernetes, Docker, etc.).
- ⚙️ **Configurable** : Pilotage via un fichier `config.yaml`.
- 📊 **Rapport ROI** : Calcule en temps réel l'économie réalisée.
- 📂 **Multi-Source** : Lit depuis un fichier ou en direct via `stdin`.
- 🚨 **Alertes Webhook** : Notifications Discord/Slack en cas d'erreur critique.

## 🚀 Installation Rapide

### Pré-requis
- Rust & Cargo installés (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)

### Construction
```bash
git clone https://github.com/votre-utilisateur/shrinker-rust.git
cd shrinker-rust
cargo build --release
```

## 🛠️ Utilisation

### Mode Agent (Temps réel)
```bash
# Redirige la sortie vers un autre fichier ou un pipe
tail -f /var/log/syslog | ./target/release/shrinker > logs_propres.log
```

### Analyse de fichier
```bash
./target/release/shrinker --file production.log
```

## ⚙️ Configuration (`config.yaml`)

```yaml
mask_ips: true      # Masquer les adresses IP pour la sécurité
threshold: 5        # Ne logger que si le message se répète 5 fois
output_file: null   # null = Sortie Standard (stdout), ou mettre "out.log" pour un fichier

# Section Alertes (Optionnel)
alert:
  webhook_url: "https://discord.com/api/webhooks/..." # URL de votre Webhook
  threshold: 50 # Déclenche une alerte si le message se répète 50 fois
```

## 🤖 Déploiement Ansible

Shrinker est installable automatiquement via un **rôle Ansible universel** (Debian/Ubuntu, RHEL/CentOS/Fedora, Arch Linux).

Le rôle est disponible dans un dépôt séparé :

**[shrinker_role_ansible](https://github.com/KarimHaddadi20/shrinker_role_ansible)**

### Installation rapide

```bash
git clone https://github.com/KarimHaddadi20/shrinker_role_ansible.git
```

```yaml
# playbook.yml
- name: Deployer Shrinker
  hosts: all
  become: yes
  roles:
    - shrinker_role_ansible
```

```bash
ansible-playbook -i inventory.ini playbook.yml
```

Consultez le [README du rôle](https://github.com/KarimHaddadi20/shrinker_role_ansible#readme) pour la documentation complète (variables, exemples, distributions supportées).

## 📈 RoadMap
- [x] Support du masquage IPv4.
- [x] Envoi direct vers Discord/Slack via Webhooks.
- [x] Support du masquage IPv6.
- [x] Parsing JSON intelligent pour Kubernetes.
- [x] Déploiement automatisé via Ansible.

---
Projet créé dans le cadre d'un apprentissage Rust orienté **DevOps & Infrastructure**.

