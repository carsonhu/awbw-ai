//! Runs recorded AWBW games through the engine and reports where they diverge.
//!
//! Usage: verify-replays <prepared.json | directory> [--verbose] [--limit N]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use awbw_replay::schema::Replay;
use awbw_replay::{has_only_plain_cos, uses_powers, Report, Verifier};

fn collect(path: &Path, limit: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        entries.sort();
        files.extend(entries);
    } else {
        files.push(path.to_path_buf());
    }
    if limit > 0 {
        files.truncate(limit);
    }
    files
}

enum Outcome {
    Verified(Report, bool),
    Filtered(&'static str),
}

fn verify_one(
    path: &Path,
    vanilla_only: bool,
    no_powers: bool,
    no_fog: bool,
) -> Result<Outcome, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let replay: Replay = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let powered = uses_powers(&replay);
    if no_powers && powered {
        return Ok(Outcome::Filtered("co-power-used"));
    }
    // Fog snapshots only list what the recording player could see, so a unit the
    // engine correctly tracks can read as a phantom. Until vision is modelled,
    // these games cannot be held to a strict diff.
    if no_fog && replay.fog {
        return Ok(Outcome::Filtered("fog"));
    }
    if vanilla_only {
        if !has_only_plain_cos(&replay) {
            return Ok(Outcome::Filtered("co-abilities"));
        }
        if powered {
            return Ok(Outcome::Filtered("co-power-used"));
        }
        if replay.fog {
            return Ok(Outcome::Filtered("fog"));
        }
    }
    let verifier = Verifier::new(&replay)?;
    Ok(Outcome::Verified(verifier.verify(), powered))
}

/// Running totals, kept once for games that used a CO power and once for those
/// that did not. Powers are unmodelled by design, so splitting the two is the
/// only way to see what the engine itself still gets wrong.
#[derive(Default)]
struct Totals {
    games: usize,
    clean: usize,
    checks: usize,
    turns: usize,
    divergences: usize,
}

impl Totals {
    fn add(&mut self, report: &Report) {
        self.games += 1;
        self.checks += report.checks;
        self.turns += report.turns_checked;
        self.divergences += report.divergences.len();
        if report.is_clean() {
            self.clean += 1;
        }
    }

    fn print(&self, label: &str) {
        if self.games == 0 {
            return;
        }
        let agreement = if self.checks > 0 {
            100.0 * (1.0 - self.divergences as f64 / self.checks as f64)
        } else {
            100.0
        };
        println!(
            "{label:<22} {:>4} games ({:>3} clean, {:>5.1}%)  {:>7} turns  {:>8} checks  \
{:>6} divergences  {agreement:.3}% agreement",
            self.games,
            self.clean,
            100.0 * self.clean as f64 / self.games as f64,
            self.turns,
            self.checks,
            self.divergences
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
    // Restrict to games the engine can actually be held to: no CO abilities, no
    // powers, no fog. Anything else is measuring unimplemented features.
    let vanilla_only = args.iter().any(|a| a == "--vanilla");
    let no_powers = args.iter().any(|a| a == "--no-powers");
    let no_fog = args.iter().any(|a| a == "--no-fog");
    let limit = args
        .iter()
        .position(|a| a == "--limit")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0usize);
    let target = args
        .iter()
        .find(|a| !a.starts_with('-') && a.parse::<usize>().is_err())
        .cloned()
        .unwrap_or_else(|| "data/prepared".to_string());

    let files = collect(Path::new(&target), limit);
    if files.is_empty() {
        eprintln!("no prepared replays found at {target}");
        std::process::exit(1);
    }

    let mut games = 0usize;
    let mut clean = 0usize;
    let mut total_checks = 0usize;
    let mut total_turns = 0usize;
    let mut total_luck = 0usize;
    let mut by_kind: HashMap<&'static str, usize> = HashMap::new();
    let mut unsupported: HashMap<String, usize> = HashMap::new();
    let mut samples: HashMap<&'static str, String> = HashMap::new();
    let mut filtered: HashMap<&'static str, usize> = HashMap::new();
    let mut with_powers = Totals::default();
    let mut without_powers = Totals::default();
    let mut kinds_without_powers: HashMap<&'static str, usize> = HashMap::new();

    for file in &files {
        match verify_one(file, vanilla_only, no_powers, no_fog) {
            Ok(Outcome::Filtered(reason)) => {
                *filtered.entry(reason).or_insert(0) += 1;
            }
            Ok(Outcome::Verified(report, powered)) => {
                if powered {
                    with_powers.add(&report);
                } else {
                    without_powers.add(&report);
                    for (kind, n) in report.counts_by_kind() {
                        *kinds_without_powers.entry(kind).or_insert(0) += n;
                    }
                }
                games += 1;
                total_checks += report.checks;
                total_turns += report.turns_checked;
                total_luck += report.luck_slack;
                if report.is_clean() {
                    clean += 1;
                }
                for (kind, n) in report.counts_by_kind() {
                    *by_kind.entry(kind).or_insert(0) += n;
                }
                for (kind, n) in &report.actions_unsupported {
                    *unsupported.entry(kind.clone()).or_insert(0) += n;
                }
                for d in &report.divergences {
                    samples.entry(d.kind).or_insert_with(|| {
                        format!("game {} turn {} day {}: {}", report.game_id, d.turn_index, d.day, d.detail)
                    });
                }
                if verbose {
                    println!(
                        "{}: {} turns, {} checks, {} divergences, {} luck-slack",
                        file.file_name().unwrap().to_string_lossy(),
                        report.turns_checked,
                        report.checks,
                        report.divergences.len(),
                        report.luck_slack
                    );
                    for d in report.divergences.iter().take(12) {
                        println!("    [{}] turn {} day {}: {}", d.kind, d.turn_index, d.day, d.detail);
                    }
                }
            }
            Err(e) => eprintln!("{}: {e}", file.display()),
        }
    }

    let total_divergences: usize = by_kind.values().sum();
    println!("\n=== summary ===");
    println!("games verified:  {games} ({clean} fully clean)");
    println!("turns checked:   {total_turns}");
    println!("assertions:      {total_checks}");
    println!("divergences:     {total_divergences}");
    if total_checks > 0 {
        println!(
            "agreement:       {:.3}%",
            100.0 * (1.0 - total_divergences as f64 / total_checks as f64)
        );
    }
    println!("luck slack:      {total_luck} (HP within one displayed point)");

    println!("\nsplit by whether the game used a CO power (unmodelled by design):");
    with_powers.print("  powers used");
    without_powers.print("  no powers");

    if !kinds_without_powers.is_empty() {
        println!("\ndivergences in power-free games (i.e. real engine gaps):");
        let mut kinds: Vec<_> = kinds_without_powers.into_iter().collect();
        kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        for (kind, n) in kinds.iter().take(12) {
            println!("  {kind:22} {n}");
        }
    }

    if !by_kind.is_empty() {
        println!("\ndivergences by kind:");
        let mut kinds: Vec<_> = by_kind.into_iter().collect();
        kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        for (kind, n) in kinds {
            println!("  {kind:22} {n}");
            if let Some(sample) = samples.get(kind) {
                println!("      e.g. {sample}");
            }
        }
    }
    if !unsupported.is_empty() {
        println!("\nunsupported actions:");
        let mut kinds: Vec<_> = unsupported.into_iter().collect();
        kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        for (kind, n) in kinds.iter().take(15) {
            println!("  {kind:22} {n}");
        }
    }
}
