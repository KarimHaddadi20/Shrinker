use clap::Parser;
use colored::*;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::time::Instant;
use chrono::Local;
use std::process::Command;

// Structure pour le fichier de configuration YAML
#[derive(Deserialize, Debug)]
struct Config {
    mask_ips: bool,
    threshold: u32,
    output_file: Option<String>,
    alert: Option<AlertConfig>,
}

#[derive(Deserialize, Debug)]
struct AlertConfig {
    webhook_url: String,
    threshold: u32,
}

#[derive(Parser, Debug)]
#[command(
    name = "shrinker",
    version,
    about = "Agent de telemetrie ultra-leger ecrit en Rust",
    long_about = "Shrinker reduit le volume de logs via deduplication intelligente et masquage IP (IPv4/IPv6).\n\
                  Il peut envoyer des alertes Webhook (Discord/Slack) en cas d'erreur critique repetee.\n\n\
                  EXEMPLES:\n\
                  \n  Analyser un fichier de logs:\n    shrinker --file production.log\
                  \n\n  Mode temps reel (pipe Unix):\n    tail -f /var/log/syslog | shrinker > clean.log\
                  \n\n  Surcharger le seuil de deduplication:\n    shrinker --file app.log --threshold 10\
                  \n\n  Desactiver le masquage IP:\n    shrinker --file app.log --no-mask-ips",
)]
struct Args {
    /// Fichier de log a analyser (stdin si omis)
    #[arg(short, long)]
    file: Option<String>,

    /// Fichier de configuration YAML
    #[arg(short, long, default_value = "config.yaml")]
    config: String,

    /// Seuil de deduplication (surcharge la valeur du config.yaml)
    #[arg(short, long)]
    threshold: Option<u32>,

    /// Desactiver le masquage des adresses IP
    #[arg(long)]
    no_mask_ips: bool,

    /// Mode simulation : affiche ce qui serait fait sans rien ecrire
    #[arg(long)]
    dry_run: bool,
}

struct Stats {
    total: u64,
    sent: u64,
    start_time: Instant,
}

fn main() {
    let args = Args::parse();
    let mut stats = Stats {
        total: 0,
        sent: 0,
        start_time: Instant::now(),
    };

    // 1. Chargement de la configuration (YAML + surcharges CLI)
    let config_str = fs::read_to_string(&args.config)
        .expect("❌ Impossible de lire le fichier de configuration");
    let mut config: Config = serde_yaml::from_str(&config_str)
        .expect("❌ Erreur de format dans le fichier YAML");

    if let Some(t) = args.threshold {
        config.threshold = t;
    }
    if args.no_mask_ips {
        config.mask_ips = false;
    }

    let ipv4_regex = Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}").unwrap();
    let ipv6_regex = Regex::new(
        r"(?i)[0-9a-f]{1,4}(:[0-9a-f]{1,4}){7}|([0-9a-f]{1,4}:)+:([0-9a-f]{1,4}:)*[0-9a-f]{1,4}|([0-9a-f]{1,4}:)+:|::[0-9a-f]{1,4}(:[0-9a-f]{1,4})*|::"
    ).unwrap();

    // Détermine si on est en mode fichier ou stdout
    let is_stdout_mode = match &config.output_file {
        Some(f) => f.is_empty(),
        None => true,
    };

    // Si on écrit dans un fichier, on peut afficher les infos dans le terminal
    if !is_stdout_mode {
        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
        eprintln!("{} {}", "🚀 AGENT SHRINKER V1.2 - CONFIG CHARGÉE".bright_cyan(), Local::now().format("%H:%M:%S").to_string().yellow());
        
        let output_target = config.output_file.as_ref().unwrap();
        eprintln!("📂 Sortie : {}", output_target.bright_magenta());
        eprintln!("🛡️  Sécurité IP : {}", if config.mask_ips { "ACTIVE".green() } else { "INACTIVE".red() });
        
        if let Some(alert) = &config.alert {
            eprintln!("🚨 Alertes : ACTIVE (Seuil: {})", alert.threshold.to_string().red().bold());
        }

        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
    }

    // 2. Préparation de la sortie
    let mut output: Box<dyn Write> = if is_stdout_mode {
        Box::new(io::stdout())
    } else {
        let path = config.output_file.as_ref().unwrap();
        let f = File::create(path).expect("❌ Impossible de créer le fichier de sortie");
        Box::new(f)
    };

    // 3. Sélection de la source
    let input: Box<dyn BufRead> = match args.file {
        Some(ref path) => {
            let f = File::open(path).expect("❌ Impossible d'ouvrir le fichier source");
            Box::new(BufReader::new(f))
        }
        None => Box::new(BufReader::new(io::stdin())),
    };

    // 4. Traitement
    if args.dry_run {
        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
        eprintln!("{}", "🧪 MODE DRY-RUN (Simulation, aucune ecriture)".bright_yellow().bold());
        eprintln!("📂 Source : {}", args.file.as_deref().unwrap_or("stdin"));
        eprintln!("🛡️  Masquage IP : {}", if config.mask_ips { "ACTIVE" } else { "INACTIVE" });
        eprintln!("📊 Seuil : {}", config.threshold);
        if let Some(alert) = &config.alert {
            eprintln!("🚨 Alertes : Seuil {}", alert.threshold);
        }
        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
    }

    process_logs(input, &config, &ipv4_regex, &ipv6_regex, &mut stats, &mut output, is_stdout_mode, args.dry_run);

    if !is_stdout_mode || args.dry_run {
        display_final_report(&stats);
    }
}

fn process_logs(reader: Box<dyn BufRead>, config: &Config, ipv4_regex: &Regex, ipv6_regex: &Regex, stats: &mut Stats, output: &mut Box<dyn Write>, silent_mode: bool, dry_run: bool) {
    let mut last_msg = String::new();
    let mut count = 0;

    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() { continue; }
        stats.total += 1;

        let mut processed = extract_message(&line);
        if config.mask_ips {
            processed = ipv4_regex.replace_all(&processed, "[MASKED_IPv4]").to_string();
            processed = ipv6_regex.replace_all(&processed, "[MASKED_IPv6]").to_string();
        }

        if processed == last_msg {
            count += 1;
        } else {
            if !last_msg.is_empty() {
                if !dry_run {
                    check_alert(&last_msg, count, config, silent_mode);
                }
                if count >= config.threshold {
                    if !dry_run {
                        print_log(count, &last_msg, output, silent_mode);
                    } else {
                        eprintln!("  {} [x{}] {}", ">>".bright_yellow(), count, last_msg);
                    }
                    stats.sent += 1;
                }
            }
            last_msg = processed;
            count = 1;
        }
    }

    if !last_msg.is_empty() {
        if !dry_run {
            check_alert(&last_msg, count, config, silent_mode);
        }
        if count >= config.threshold {
            if !dry_run {
                print_log(count, &last_msg, output, silent_mode);
            } else {
                eprintln!("  {} [x{}] {}", ">>".bright_yellow(), count, last_msg);
            }
            stats.sent += 1;
        }
    }
}

fn check_alert(msg: &str, count: u32, config: &Config, silent_mode: bool) {
    if let Some(alert) = &config.alert {
        if count >= alert.threshold {
            if !silent_mode {
                eprintln!("{} {} (x{})", "🚨 ENVOI ALERTE :".red().bold(), msg, count);
            }
            
            // Construction manuelle du JSON
            // On échappe les caractères spéciaux basiques
            let safe_msg = msg.replace("\"", "\\\"").replace("\n", "\\n").replace("\r", "");
            let json_body = format!(r#"{{"content": "🚨 **ALERTE CRITIQUE**\nMessage répété **{} fois**\n`{}`"}}"#, count, safe_msg);

            // On lance curl en arrière-plan (spawn) pour ne pas bloquer le traitement des logs
            let _ = Command::new("curl")
                .arg("-X").arg("POST")
                .arg("-H").arg("Content-Type: application/json")
                .arg("-d").arg(&json_body)
                .arg(&alert.webhook_url)
                .stdout(std::process::Stdio::null()) // Mode silencieux
                .stderr(std::process::Stdio::null())
                .spawn();
        }
    }
}

fn extract_message(line: &str) -> String {
    let trimmed = line.trim();

    // Si la ligne commence par '{', on tente un parsing JSON
    if trimmed.starts_with('{') {
        if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
            // On cherche le champ "message" ou "msg" (les deux standards les plus courants)
            let msg = json.get("message")
                .or_else(|| json.get("msg"))
                .and_then(|v| v.as_str());

            let level = json.get("level")
                .and_then(|v| v.as_str());

            if let Some(m) = msg {
                return match level {
                    Some(l) => format!("[{}] {}", l.to_uppercase(), m),
                    None => m.to_string(),
                };
            }
        }
    }

    // Sinon, on utilise le nettoyage classique (suppression du timestamp entre crochets)
    if let Some(pos) = trimmed.find(']') {
        trimmed[pos + 1..].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn print_log(count: u32, msg: &str, output: &mut Box<dyn Write>, silent_mode: bool) {
    // En mode fichier, on affiche un feedback visuel dans le terminal
    if !silent_mode {
        if count > 1 {
            eprintln!("  {} {}", format!("[x{}]", count).bright_yellow().bold(), msg);
        } else {
            eprintln!("  {} {}", "[+]".bright_green(), msg);
        }
    }

    // Écriture réelle (Fichier ou Stdout)
    // En mode stdout, on n'ajoute pas de couleurs ANSI dans le flux de données
    let log_line = if count > 1 {
        format!("[x{}] {}\n", count, msg)
    } else {
        format!("[+] {}\n", msg)
    };
    
    output.write_all(log_line.as_bytes()).expect("❌ Erreur d'écriture");
}

fn display_final_report(stats: &Stats) {
    if stats.total == 0 { return; }
    let duration = stats.start_time.elapsed();
    let reduction = 100 - (stats.sent * 100 / stats.total);
    
    eprintln!("{}", "\n━━━━━━━━━━━━━━━━━━━━ STATS ━━━━━━━━━━━━━━━━━━━━".bright_cyan());
    eprintln!("⏱️  Temps : {:?}", duration);
    eprintln!("📄 Total : {}", stats.total);
    eprintln!("📤 Envoyé : {}", stats.sent);
    eprintln!("💰 ÉCO : {}", format!("{}%", reduction).bright_green().bold());
    eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
}
