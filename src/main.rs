use clap::Parser;
use colored::*;
use regex::Regex;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::time::Instant;
use chrono::Local;

// Structure pour le fichier de configuration YAML
#[derive(Deserialize, Debug)]
struct Config {
    mask_ips: bool,
    threshold: u32,
    output_file: Option<String>,
}

// Arguments de la ligne de commande
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Chemin vers le fichier de configuration YAML
    #[arg(short, long, default_value = "config.yaml")]
    config: String,

    /// Fichier de log à analyser (si vide, utilise l'entrée standard)
    #[arg(short, long)]
    file: Option<String>,
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

    // 1. Chargement de la configuration
    let config_str = fs::read_to_string(&args.config)
        .expect("❌ Impossible de lire le fichier de configuration");
    let config: Config = serde_yaml::from_str(&config_str)
        .expect("❌ Erreur de format dans le fichier YAML");

    let ip_regex = Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}").unwrap();

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
        Some(path) => {
            let f = File::open(path).expect("❌ Impossible d'ouvrir le fichier source");
            Box::new(BufReader::new(f))
        }
        None => Box::new(BufReader::new(io::stdin())),
    };

    // 4. Traitement
    process_logs(input, &config, &ip_regex, &mut stats, &mut output, is_stdout_mode);

    // On affiche le rapport final seulement si on n'est PAS en mode stdout
    if !is_stdout_mode {
        display_final_report(&stats);
    }
}

fn process_logs(reader: Box<dyn BufRead>, config: &Config, ip_regex: &Regex, stats: &mut Stats, output: &mut Box<dyn Write>, silent_mode: bool) {
    let mut last_msg = String::new();
    let mut count = 0;

    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() { continue; }
        stats.total += 1;

        let mut processed = clean_timestamp(&line);
        if config.mask_ips {
            processed = ip_regex.replace_all(&processed, "[MASKED_IP]").to_string();
        }

        if processed == last_msg {
            count += 1;
        } else {
            if !last_msg.is_empty() && count >= config.threshold {
                print_log(count, &last_msg, output, silent_mode);
                stats.sent += 1;
            }
            last_msg = processed;
            count = 1;
        }
    }

    if !last_msg.is_empty() && count >= config.threshold {
        print_log(count, &last_msg, output, silent_mode);
        stats.sent += 1;
    }
}

fn clean_timestamp(line: &str) -> String {
    if let Some(pos) = line.find(']') {
        line[pos + 1..].trim().to_string()
    } else {
        line.trim().to_string()
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
