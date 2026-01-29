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
    output_file: String,
}

// Arguments de la ligne de commande (on garde juste l'option config)
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

    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
    println!("{} {}", "🚀 AGENT SHRINKER V1.2 - CONFIG CHARGÉE".bright_cyan(), Local::now().format("%H:%M:%S").to_string().yellow());
    println!("📂 Sortie : {}", config.output_file.bright_magenta());
    println!("🛡️  Sécurité IP : {}", if config.mask_ips { "ACTIVE".green() } else { "INACTIVE".red() });
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());

    // 2. Préparation du fichier de sortie
    let mut f_output = File::create(&config.output_file)
        .expect("❌ Impossible de créer le fichier de sortie");

    // 3. Sélection de la source
    let input: Box<dyn BufRead> = match args.file {
        Some(path) => {
            let f = File::open(path).expect("❌ Impossible d'ouvrir le fichier source");
            Box::new(BufReader::new(f))
        }
        None => Box::new(BufReader::new(io::stdin())),
    };

    // 4. Traitement
    process_logs(input, &config, &ip_regex, &mut stats, &mut f_output);

    display_final_report(&stats);
}

fn process_logs(reader: Box<dyn BufRead>, config: &Config, ip_regex: &Regex, stats: &mut Stats, output: &mut File) {
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
                print_log(count, &last_msg, output);
                stats.sent += 1;
            }
            last_msg = processed;
            count = 1;
        }
    }

    if !last_msg.is_empty() && count >= config.threshold {
        print_log(count, &last_msg, output);
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

fn print_log(count: u32, msg: &str, output: &mut File) {
    if count > 1 {
        println!("  {} {}", format!("[x{}]", count).bright_yellow().bold(), msg);
    } else {
        println!("  {} {}", "[+]".bright_green(), msg);
    }

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
    
    println!("{}", "\n━━━━━━━━━━━━━━━━━━━━ STATS ━━━━━━━━━━━━━━━━━━━━".bright_cyan());
    println!("⏱️  Temps : {:?}", duration);
    println!("📄 Total : {}", stats.total);
    println!("📤 Envoyé : {}", stats.sent);
    println!("💰 ÉCO : {}%", format!("{}%", reduction).bright_green().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
}
