use clap::{Parser, Subcommand};
use colored::*;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::time::{Duration, Instant};
use chrono::{Local, Utc};
use std::process::{self, Command};
use std::env;

#[derive(Deserialize, Debug)]
struct Config {
    mask_ips: bool,
    threshold: u32,
    output_file: Option<String>,
    alert: Option<AlertConfig>,
    #[serde(default)]
    exclude_patterns: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct AlertConfig {
    webhook_url: String,
    threshold: u32,
    /// Délai minimum (en minutes) entre deux alertes pour le même message
    #[serde(default)]
    cooldown_minutes: Option<u64>,
}

#[derive(Parser, Debug)]
#[command(
    name = "shrinker",
    version,
    about = "Agent de telemetrie ultra-leger ecrit en Rust",
    long_about = "Shrinker reduit le volume de logs via deduplication intelligente et masquage IP (IPv4/IPv6).\n\
                  Il peut envoyer des alertes Webhook (Discord/Slack) en cas d'erreur critique repetee.\n\n\
                  EXEMPLES:\n\
                  \n  Generer un fichier de configuration:\n    shrinker init\
                  \n\n  Analyser un fichier de logs:\n    shrinker run --file production.log\
                  \n\n  Mode temps reel (pipe Unix):\n    tail -f /var/log/syslog | shrinker run > clean.log\
                  \n\n  Mode verbose:\n    shrinker run --file app.log --verbose\
                  \n\n  Mode silencieux:\n    shrinker run --file app.log --quiet",
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

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

    /// Mode verbose : affiche chaque ligne traitee
    #[arg(short, long)]
    verbose: bool,

    /// Mode silencieux : n'affiche que les erreurs critiques
    #[arg(short, long)]
    quiet: bool,

    /// Format de sortie : text (defaut) ou json (une ligne JSON par entree, compatible jq/Elasticsearch)
    #[arg(long, default_value = "text")]
    output_format: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Genere un fichier config.yaml par defaut dans le repertoire courant
    Init {
        /// Chemin du fichier de configuration a generer
        #[arg(short, long, default_value = "config.yaml")]
        output: String,
    },
}

#[derive(Clone, Copy, PartialEq)]
enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

struct Stats {
    total: u64,
    sent: u64,
    skipped: u64,
    excluded: u64,
    start_time: Instant,
}

fn exit_error(msg: &str) -> ! {
    eprintln!("{} {}", "erreur:".red().bold(), msg);
    process::exit(1);
}

fn generate_default_config(path: &str) {
    if fs::metadata(path).is_ok() {
        exit_error(&format!("le fichier '{}' existe deja (utilisez un autre nom avec --output)", path));
    }

    let default_config = r#"# Configuration Shrinker
# Documentation : https://github.com/KarimHaddadi20/Shrinker

# Masquer les adresses IP (IPv4 et IPv6) dans les logs
mask_ips: true

# Seuil de deduplication : un message doit se repeter N fois pour etre conserve
threshold: 5

# Fichier de sortie (null = stdout, ideal pour les pipes Unix)
output_file: null

# Patterns d'exclusion : les lignes contenant ces mots seront ignorees (case-insensitive)
exclude_patterns:
  - "health check"
  - "heartbeat"
  # - "DEBUG"
  # - "keep-alive"

# Alertes Webhook (optionnel, decommentez pour activer)
# webhook_url accepte une URL ou une variable d'environnement : $DISCORD_WEBHOOK ou ${DISCORD_WEBHOOK}
# alert:
#   webhook_url: "$DISCORD_WEBHOOK"
#   threshold: 50
#   cooldown_minutes: 15   # Max 1 alerte par 15 min pour le meme message (evite le spam)
"#;

    if let Err(e) = fs::write(path, default_config) {
        exit_error(&format!("impossible de creer '{}': {}", path, e));
    }

    eprintln!("{} Configuration generee dans '{}'", "ok:".green().bold(), path);
    eprintln!("   Editez ce fichier puis lancez : shrinker --file vos_logs.txt");
}

fn resolve_env_var(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with("${") {
        if let Some(end) = s.find('}') {
            let var_name = &s[2..end];
            return env::var(var_name).ok();
        }
    }
    if s.starts_with('$') {
        let var_name = s[1..].split_whitespace().next().unwrap_or(&s[1..]);
        return env::var(var_name).ok();
    }
    None
}

fn load_config(path: &str) -> Config {
    let config_str = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                exit_error(&format!(
                    "fichier '{}' introuvable\n   Lancez 'shrinker init' pour en generer un",
                    path
                ));
            }
            exit_error(&format!("impossible de lire '{}': {}", path, e));
        }
    };

    match serde_yaml::from_str::<Config>(&config_str) {
        Ok(c) => c,
        Err(e) => {
            let hint = if e.to_string().contains("missing field") {
                let field = e.to_string();
                format!("\n   Verifiez que le champ manquant est bien present dans '{}'", path)
                    + &format!("\n   Detail : {}", field)
            } else {
                format!("\n   Detail : {}", e)
            };
            exit_error(&format!("format YAML invalide dans '{}'{}", path, hint));
        }
    }
}

fn main() {
    let cli = Cli::parse();

    if let Some(Commands::Init { output }) = &cli.command {
        generate_default_config(output);
        return;
    }

    if cli.verbose && cli.quiet {
        exit_error("--verbose et --quiet ne peuvent pas etre utilises ensemble");
    }

    let output_format_json = matches!(cli.output_format.to_lowercase().as_str(), "json" | "j");

    let verbosity = if cli.quiet {
        Verbosity::Quiet
    } else if cli.verbose {
        Verbosity::Verbose
    } else {
        Verbosity::Normal
    };

    let mut stats = Stats {
        total: 0,
        sent: 0,
        skipped: 0,
        excluded: 0,
        start_time: Instant::now(),
    };

    let mut config = load_config(&cli.config);

    if let Some(ref mut alert) = config.alert {
        if let Some(resolved) = resolve_env_var(&alert.webhook_url) {
            alert.webhook_url = resolved;
        } else if alert.webhook_url.starts_with('$') {
            let var_hint = if alert.webhook_url.starts_with("${") {
                alert.webhook_url.split('}').next().map(|s| &s[2..]).unwrap_or("?")
            } else {
                &alert.webhook_url[1..]
            };
            exit_error(&format!(
                "variable d'environnement '{}' non definie (webhook_url dans config)",
                var_hint
            ));
        }
    }

    if let Some(t) = cli.threshold {
        config.threshold = t;
    }
    if cli.no_mask_ips {
        config.mask_ips = false;
    }

    let ipv4_regex = Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}").unwrap();
    let ipv6_regex = Regex::new(
        r"(?i)[0-9a-f]{1,4}(:[0-9a-f]{1,4}){7}|([0-9a-f]{1,4}:)+:([0-9a-f]{1,4}:)*[0-9a-f]{1,4}|([0-9a-f]{1,4}:)+:|::[0-9a-f]{1,4}(:[0-9a-f]{1,4})*|::"
    ).unwrap();

    let is_stdout_mode = match &config.output_file {
        Some(f) => f.is_empty(),
        None => true,
    };

    if !is_stdout_mode && verbosity != Verbosity::Quiet {
        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
        eprintln!("{} {}", "AGENT SHRINKER V1.3 - CONFIG CHARGEE".bright_cyan(), Local::now().format("%H:%M:%S").to_string().yellow());

        let output_target = config.output_file.as_ref().unwrap();
        eprintln!("   Sortie : {}", output_target.bright_magenta());
        eprintln!("   Securite IP : {}", if config.mask_ips { "ACTIVE".green() } else { "INACTIVE".red() });
        eprintln!("   Seuil : {}", config.threshold.to_string().yellow());

        if !config.exclude_patterns.is_empty() {
            eprintln!("   Exclusions : {} pattern(s)", config.exclude_patterns.len().to_string().yellow());
        }

        if let Some(alert) = &config.alert {
            let cooldown_info = alert
                .cooldown_minutes
                .map(|m| format!(", Cooldown: {} min", m))
                .unwrap_or_default();
            eprintln!(
                "   Alertes : ACTIVE (Seuil: {}{})",
                alert.threshold.to_string().red().bold(),
                cooldown_info
            );
        }

        if verbosity == Verbosity::Verbose {
            eprintln!("   Mode : {}", "VERBOSE".bright_yellow().bold());
        }

        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
    }

    let mut output: Box<dyn Write> = if is_stdout_mode {
        Box::new(io::stdout())
    } else {
        let path = config.output_file.as_ref().unwrap();
        match File::create(path) {
            Ok(f) => Box::new(f),
            Err(e) => exit_error(&format!("impossible de creer le fichier de sortie '{}': {}", path, e)),
        }
    };

    let input: Box<dyn BufRead> = match cli.file {
        Some(ref path) => {
            match File::open(path) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => {
                    if e.kind() == io::ErrorKind::NotFound {
                        exit_error(&format!("fichier '{}' introuvable", path));
                    }
                    exit_error(&format!("impossible d'ouvrir '{}': {}", path, e));
                }
            }
        }
        None => Box::new(BufReader::new(io::stdin())),
    };

    if cli.dry_run && verbosity != Verbosity::Quiet {
        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
        eprintln!("{}", "MODE DRY-RUN (Simulation, aucune ecriture)".bright_yellow().bold());
        eprintln!("   Source : {}", cli.file.as_deref().unwrap_or("stdin"));
        eprintln!("   Masquage IP : {}", if config.mask_ips { "ACTIVE" } else { "INACTIVE" });
        eprintln!("   Seuil : {}", config.threshold);
        if !config.exclude_patterns.is_empty() {
            eprintln!("   Exclusions : {:?}", config.exclude_patterns);
        }
        if let Some(alert) = &config.alert {
            let cooldown_info = alert
                .cooldown_minutes
                .map(|m| format!(", Cooldown {} min", m))
                .unwrap_or_default();
            eprintln!("   Alertes : Seuil {}{}", alert.threshold, cooldown_info);
        }
        eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
    }

    let mut alert_cooldown: HashMap<String, Instant> = HashMap::new();
    process_logs(input, &config, &ipv4_regex, &ipv6_regex, &mut stats, &mut output, &mut alert_cooldown, is_stdout_mode, cli.dry_run, verbosity, output_format_json);

    if (!is_stdout_mode || cli.dry_run) && verbosity != Verbosity::Quiet {
        display_final_report(&stats);
    }
}

fn process_logs(
    reader: Box<dyn BufRead>,
    config: &Config,
    ipv4_regex: &Regex,
    ipv6_regex: &Regex,
    stats: &mut Stats,
    output: &mut Box<dyn Write>,
    alert_cooldown: &mut HashMap<String, Instant>,
    silent_mode: bool,
    dry_run: bool,
    verbosity: Verbosity,
    output_format_json: bool,
) {
    let mut last_msg = String::new();
    let mut count: u32 = 0;

    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() { continue; }
        stats.total += 1;

        if verbosity == Verbosity::Verbose && !silent_mode {
            eprintln!("  {} {}", format!("#{}", stats.total).dimmed(), line.dimmed());
        }

        let mut processed = extract_message(&line);
        if config.mask_ips {
            processed = ipv4_regex.replace_all(&processed, "[MASKED_IPv4]").to_string();
            processed = ipv6_regex.replace_all(&processed, "[MASKED_IPv6]").to_string();
        }

        let lower = processed.to_lowercase();
        if config.exclude_patterns.iter().any(|p| lower.contains(&p.to_lowercase())) {
            stats.excluded += 1;
            if verbosity == Verbosity::Verbose && !silent_mode {
                eprintln!("  {} {}", "exclu".dimmed(), processed.dimmed());
            }
            continue;
        }

        if processed == last_msg {
            count += 1;
        } else {
            if !last_msg.is_empty() {
                if !dry_run {
                    check_alert(&last_msg, count, config, silent_mode, alert_cooldown);
                }
                if count >= config.threshold {
                    if !dry_run {
                        print_log(count, &last_msg, output, silent_mode, output_format_json);
                    } else if verbosity != Verbosity::Quiet {
                        eprintln!("  {} [x{}] {}", ">>".bright_yellow(), count, last_msg);
                    }
                    stats.sent += 1;
                } else {
                    stats.skipped += 1;
                    if verbosity == Verbosity::Verbose && !silent_mode {
                        eprintln!("  {} [x{}] {} {}", "skip".dimmed(), count, last_msg.dimmed(), format!("(< seuil {})", config.threshold).dimmed());
                    }
                }
            }
            last_msg = processed;
            count = 1;
        }
    }

    if !last_msg.is_empty() {
        if !dry_run {
            check_alert(&last_msg, count, config, silent_mode, alert_cooldown);
        }
        if count >= config.threshold {
            if !dry_run {
                print_log(count, &last_msg, output, silent_mode, output_format_json);
            } else if verbosity != Verbosity::Quiet {
                eprintln!("  {} [x{}] {}", ">>".bright_yellow(), count, last_msg);
            }
            stats.sent += 1;
        } else {
            stats.skipped += 1;
            if verbosity == Verbosity::Verbose && !silent_mode {
                eprintln!("  {} [x{}] {} {}", "skip".dimmed(), count, last_msg.dimmed(), format!("(< seuil {})", config.threshold).dimmed());
            }
        }
    }
}

fn check_alert(
    msg: &str,
    count: u32,
    config: &Config,
    silent_mode: bool,
    alert_cooldown: &mut HashMap<String, Instant>,
) {
    if let Some(alert) = &config.alert {
        if count >= alert.threshold {
            if let Some(cooldown_mins) = alert.cooldown_minutes {
                if let Some(&last_sent) = alert_cooldown.get(msg) {
                    let cooldown = Duration::from_secs(cooldown_mins * 60);
                    if last_sent.elapsed() < cooldown {
                        if !silent_mode {
                            eprintln!(
                                "{} {} (x{}) {}",
                                "ALERTE IGNOREE (cooldown):".dimmed(),
                                msg.dimmed(),
                                count,
                                format!("(prochaine dans {} min)", cooldown_mins).dimmed()
                            );
                        }
                        return;
                    }
                }
            }

            if !silent_mode {
                eprintln!("{} {} (x{})", "ENVOI ALERTE :".red().bold(), msg, count);
            }

            let content = format!(
                "**ALERTE CRITIQUE**\nMessage repete **{} fois**\n`{}`",
                count, msg
            );
            let payload = serde_json::json!({ "content": content });
            let json_body = payload.to_string();

            let _ = Command::new("curl")
                .arg("-X").arg("POST")
                .arg("-H").arg("Content-Type: application/json")
                .arg("-d").arg(&json_body)
                .arg(&alert.webhook_url)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();

            if alert.cooldown_minutes.is_some() {
                alert_cooldown.insert(msg.to_string(), Instant::now());
            }
        }
    }
}

fn extract_message(line: &str) -> String {
    let trimmed = line.trim();

    if trimmed.starts_with('{') {
        if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
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

    if let Some(pos) = trimmed.find(']') {
        trimmed[pos + 1..].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn print_log(count: u32, msg: &str, output: &mut Box<dyn Write>, silent_mode: bool, output_format_json: bool) {
    if !silent_mode {
        if count > 1 {
            eprintln!("  {} {}", format!("[x{}]", count).bright_yellow().bold(), msg);
        } else {
            eprintln!("  {} {}", "[+]".bright_green(), msg);
        }
    }

    let log_line = if output_format_json {
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let payload = serde_json::json!({
            "count": count,
            "message": msg,
            "timestamp": timestamp
        });
        format!("{}\n", payload.to_string())
    } else if count > 1 {
        format!("[x{}] {}\n", count, msg)
    } else {
        format!("[+] {}\n", msg)
    };

    output.write_all(log_line.as_bytes()).unwrap_or_else(|e| {
        exit_error(&format!("erreur d'ecriture: {}", e));
    });
}

fn display_final_report(stats: &Stats) {
    if stats.total == 0 {
        eprintln!("{}", "\nAucune ligne traitee.".dimmed());
        return;
    }
    let duration = stats.start_time.elapsed();
    let reduction = 100 - (stats.sent * 100 / stats.total);

    eprintln!("{}", "\n━━━━━━━━━━━━━━━━━━━━ STATS ━━━━━━━━━━━━━━━━━━━━".bright_cyan());
    eprintln!("   Temps    : {:?}", duration);
    eprintln!("   Total    : {}", stats.total);
    eprintln!("   Conserve : {}", stats.sent);
    eprintln!("   Filtre   : {}", stats.skipped);
    if stats.excluded > 0 {
        eprintln!("   Exclu    : {}", stats.excluded.to_string().yellow());
    }
    eprintln!("   Economie : {}", format!("{}%", reduction).bright_green().bold());
    eprintln!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
}
