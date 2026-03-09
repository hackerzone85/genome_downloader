use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;

const REFSEQ_URL: &str =
    "https://ftp.ncbi.nlm.nih.gov/genomes/ASSEMBLY_REPORTS/assembly_summary_refseq.txt";
const GENBANK_URL: &str =
    "https://ftp.ncbi.nlm.nih.gov/genomes/ASSEMBLY_REPORTS/assembly_summary_genbank.txt";

// ── FIX 1 (E0774 + E0252): removed the misplaced `#[derive(Parser)]` attribute
// and the duplicate `use clap::{Parser}` that appeared between the constants
// and the struct definition. `use clap::Parser` is already imported above, and
// `#[derive(Parser, Debug)]` is correctly placed on `struct Args` below. ──────

#[derive(Parser, Debug)]
#[command(
    name = "genome_downloader",
    version = "0.1.0",
    about = "General-purpose Bacterial Genome Downloader",
    long_about = "A high-performance tool to fetch and filter bacterial genomes \
                  from NCBI (RefSeq/GenBank) based on assembly quality and status."
)]
struct Args {
    /// Organism name [Genus species] — REQUIRED.
    /// Example: -o "Klebsiella pneumoniae"  or  -o "Escherichia coli"
    /// Matches the `organism_name` column prefix in the NCBI assembly summary.
    #[arg(short, long)]
    organism: String,

    /// Genome version status. 'latest' fetches the most recent assembly;
    /// 'all' includes suppressed/older versions.
    #[arg(short, long, default_value = "latest")]
    version_status: String,

    /// Level of assembly. Multiple values allowed: -a "Chromosome,Complete Genome" [without space after comma].
    /// Options: "Chromosome", "Complete Genome", "Scaffold", "Contig".
    #[arg(
        short,
        long,
        value_delimiter = ',',
        default_values_t = vec![String::from("Chromosome")]
    )]
    assembly_level: Vec<String>,

    /// Genome representation. 'Full' means the entire genome is sequenced;
    /// 'Partial' may be incomplete.
    #[arg(short, long, default_value = "Full")]
    genome_rep: String,

    /// Output directory for downloaded .fna files.
    #[arg(short = 'O', long, default_value = "./genome_data")]
    outdir: String,

    /// Parallel download slots. -t / --threads: recommended maximum is 8 for NCBI FTP.
    /// Values above 10 trigger HTTP 503 throttling.
    #[arg(short, long, default_value_t = 8)]
    threads: usize,

    /// Directory where NCBI assembly summary files are cached between runs.
    /// If not specified, defaults to <outdir>/cache at runtime.
    /// The files (~900 MB combined) are re-used until they exceed --cache-days
    /// old, avoiding redundant downloads.
    #[arg(long, default_value = "")]
    cache_dir: String,

    /// Maximum age (days) of a cached assembly summary before re-downloading.
    #[arg(long, default_value_t = 7)]
    cache_days: u64,

    /// Force re-download of assembly summary files even if the cache is fresh.
    #[arg(long, default_value_t = false)]
    refresh_cache: bool,

    /// Restrict to exactly one subspecies epithet (case-insensitive substring
    /// match against the part of organism_name that follows the species binomial).
    ///
    /// Example: --only-subsp "subsp. pneumoniae"
    ///   accepts  : "Klebsiella pneumoniae subsp. pneumoniae"
    ///   rejects  : "Klebsiella pneumoniae subsp. ozaenae"
    ///   rejects  : "Klebsiella pneumoniae subsp. rhinoscleromatis"
    ///
    /// Leave empty (default) to accept all subspecies.
    #[arg(long, default_value = "")]
    only_subsp: String,

    /// Exclude one or more subspecies epithets (comma-separated, case-insensitive).
    /// Applied after --only-subsp, so exclusions always win.
    ///
    /// Example: --exclude-subsp "ozaenae,rhinoscleromatis"
    #[arg(long, value_delimiter = ',', default_values_t = Vec::<String>::new())]
    exclude_subsp: Vec<String>,

    /// Minimum genome size in base pairs. Assemblies below this threshold are
    /// excluded — this removes plasmid-only assemblies that NCBI annotates as
    /// "Complete Genome" but contain no chromosomal sequence.
    /// For Klebsiella pneumoniae the chromosome is 5–6 Mb; plasmids are <300 Kb.
    /// Default: 1,000,000 bp (1 Mb) — safe cutoff with no ambiguous cases.
    /// Set to 0 to disable size filtering.
    #[arg(long, default_value_t = 1_000_000)]
    min_genome_size: u64,
}

// Dynamic record type — column names are read from the file header at runtime.
// This ensures the tool remains correct if NCBI adds, removes, or renames columns.
type AssemblyRecord = HashMap<String, String>;

// Required columns for filter logic — validated at parse time.
// If NCBI renames any of these, the tool exits with a clear error message.
const REQUIRED_COLS: &[&str] = &[
    "assembly_accession",
    "biosample",
    "organism_name",
    "version_status",
    "assembly_level",
    "genome_rep",
    "genome_size",
    "group",
    "species_taxid",
    "ftp_path",
];

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    fs::create_dir_all(&args.outdir)?;

    // Resolve cache directory: explicit flag wins, otherwise <outdir>/cache
    let cache_dir: PathBuf = if args.cache_dir.is_empty() {
        PathBuf::from(&args.outdir).join("cache")
    } else {
        PathBuf::from(&args.cache_dir)
    };
    fs::create_dir_all(&cache_dir)?;

    // ── Print full parameter summary before any network activity ────────────
    let sep = "─".repeat(49);
    println!("{}", sep);
    println!("  genome_downloader  v{}", env!("CARGO_PKG_VERSION"));
    println!("{}", sep);
    println!("  Organism          : {}", args.organism);
    println!("  Assembly level    : {}", args.assembly_level.join(", "));
    println!("  Version status    : {}", args.version_status);
    println!("  Genome rep.       : {}", args.genome_rep);
    if args.min_genome_size > 0 {
        println!("  Min genome size   : {} bp", args.min_genome_size);
    } else {
        println!("  Min genome size   : disabled");
    }
    if !args.only_subsp.is_empty() {
        println!("  Subspecies filter : ONLY → \"{}\"", args.only_subsp);
    } else {
        println!("  Subspecies filter : all accepted (use --only-subsp to restrict)");
    }
    if !args.exclude_subsp.is_empty() {
        println!("  Excluded subsp.   : {}", args.exclude_subsp.join(", "));
    } else {
        println!("  Excluded subsp.   : none");
    }
    println!("  Output directory  : {}", args.outdir);
    const NCBI_MAX_THREADS: usize = 8;
    let effective_threads = args.threads.min(NCBI_MAX_THREADS);
    if args.threads > NCBI_MAX_THREADS {
        println!(
            "  Threads           : {}  (requested {} — capped at {}, NCBI FTP limit)",
            effective_threads, args.threads, NCBI_MAX_THREADS
        );
    } else {
        println!("  Threads           : {}", effective_threads);
    }
    println!("{}", sep);

    // 1. Fetch and filter metadata from both NCBI sources (with disk cache)
    let mut all_records: HashMap<String, AssemblyRecord> = HashMap::new();
    process_summary(
        REFSEQ_URL,
        "assembly_summary_refseq.txt",
        &cache_dir,
        &args,
        &mut all_records,
    )
    .await?;
    process_summary(
        GENBANK_URL,
        "assembly_summary_genbank.txt",
        &cache_dir,
        &args,
        &mut all_records,
    )
    .await?;

    // ── Dataset summary — printed before download starts ────────────────────
    let total = all_records.len();
    let n_complete = all_records
        .values()
        .filter(|r| {
            r.get("assembly_level")
                .map(|s| s == "Complete Genome")
                .unwrap_or(false)
        })
        .count();
    let n_chromosome = all_records
        .values()
        .filter(|r| {
            r.get("assembly_level")
                .map(|s| s == "Chromosome")
                .unwrap_or(false)
        })
        .count();
    let n_bacteria = all_records
        .values()
        .filter(|r| r.get("group").map(|s| s == "bacteria").unwrap_or(false))
        .count();
    let n_latest = all_records
        .values()
        .filter(|r| {
            r.get("version_status")
                .map(|s| s == "latest")
                .unwrap_or(false)
        })
        .count();
    let n_full = all_records
        .values()
        .filter(|r| r.get("genome_rep").map(|s| s == "Full").unwrap_or(false))
        .count();
    // Collect unique species_taxid values — expect exactly one for a clean run.
    // A single taxid across all records confirms no cross-species contamination.
    let mut taxid_counts: HashMap<String, usize> = HashMap::new();
    for rec in all_records.values() {
        let tid = rec
            .get("species_taxid")
            .map(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        if !tid.is_empty() {
            *taxid_counts.entry(tid).or_insert(0) += 1;
        }
    }
    let n_taxid_clean = taxid_counts.len() <= 1;

    println!("{}", sep);
    println!("  Dataset Summary");
    println!("{}", sep);
    println!("  Total genomes         : {:>9}", fmt_num(total));
    println!("  Complete Genome       : {:>9}", fmt_num(n_complete));
    println!("  Chromosome            : {:>9}", fmt_num(n_chromosome));
    println!(
        "  Group = bacteria      : {:>9} / {:>9}  {}",
        fmt_num(n_bacteria),
        fmt_num(total),
        if n_bacteria == total {
            "✔"
        } else {
            "⚠  contamination detected!"
        }
    );
    println!(
        "  Species taxid         : {:>9}  {}",
        if taxid_counts.is_empty() {
            "n/a".to_string()
        } else {
            taxid_counts.keys().cloned().collect::<Vec<_>>().join(", ")
        },
        if n_taxid_clean {
            "✔"
        } else {
            "⚠  multiple taxids — possible contamination!"
        }
    );
    println!(
        "  Version = latest      : {:>9} / {:>9}  {}",
        fmt_num(n_latest),
        fmt_num(total),
        if n_latest == total { "✔" } else { "⚠" }
    );
    println!(
        "  Genome rep = Full     : {:>9} / {:>9}  {}",
        fmt_num(n_full),
        fmt_num(total),
        if n_full == total { "✔" } else { "⚠" }
    );
    // ── Organism name pattern summary (taxon collapsing) ─────────────────────
    // Mirrors extract_taxon() from check_unique_values.py:
    //   "Genus species subsp. epithet"  → take 4 tokens
    //   "Genus species complex sp."     → take 4 tokens
    //   "Genus species phage"           → take 3 tokens
    //   anything else                   → take 2 tokens (Genus species)
    let mut taxon_counts: HashMap<String, usize> = HashMap::new();
    for rec in all_records.values() {
        let name = rec.get("organism_name").map(|s| s.as_str()).unwrap_or("");
        let tokens: Vec<&str> = name.split_whitespace().collect();
        let taxon = if tokens.len() >= 4 && (tokens[2] == "subsp." || tokens[2] == "complex") {
            tokens[..4].join(" ")
        } else if tokens.len() >= 3
            && (tokens[2] == "phage" || tokens[2] == "virus" || tokens[2] == "sp.")
        {
            tokens[..3].join(" ")
        } else {
            tokens[..2.min(tokens.len())].join(" ")
        };
        *taxon_counts.entry(taxon).or_insert(0) += 1;
    }
    let mut taxon_vec: Vec<(String, usize)> = taxon_counts.into_iter().collect();
    taxon_vec.sort_by(|a, b| b.1.cmp(&a.1)); // sort by count descending

    println!("  Organism name patterns (strain name collapsed)");
    println!("{}", sep);
    for (taxon, count) in &taxon_vec {
        println!("  {:>9}  →  {}", fmt_num(*count), taxon);
    }
    println!("{}", sep);

    // 2. Write filtered TSV summary and FTP URL list
    save_outputs(&all_records, Path::new(&args.outdir))?;

    // 3. Parallel download and decompress
    download_all_genomes(all_records, &args, effective_threads).await?;

    Ok(())
}

/// Fetch (or load from disk cache) an NCBI assembly summary file, then
/// filter it and insert matching records into `map`.
///
/// Cache behaviour:
///   - On the first run (or if the cache file is absent) the summary is
///     streamed from NCBI and written to `<cache_dir>/<filename>`.
///   - On subsequent runs the cache file is used directly if its age is
///     below `args.cache_days`. No network request is made.
///   - Pass `--refresh-cache` to force a re-download regardless of age.
async fn process_summary(
    url: &str,
    filename: &str,
    cache_dir: &Path,
    args: &Args,
    map: &mut HashMap<String, AssemblyRecord>,
) -> Result<()> {
    use std::time::{Duration, SystemTime};

    let cache_path = cache_dir.join(filename);
    let tmp_path = cache_dir.join(format!("{}.tmp", filename));
    let max_age = Duration::from_secs(args.cache_days * 86_400);

    // ── Decide whether to (re-)download ──────────────────────────────────────
    // A .tmp file means a prior run was killed mid-download — always resume it.
    let has_tmp = tmp_path.exists();

    // ── Cache staleness check ────────────────────────────────────────────────
    // `duration_since` returns Err if mtime > now (can happen on Windows-backed
    // WSL2 drives due to clock skew between the Windows and Linux clocks).
    // Using checked_duration_since / saturating logic avoids the unwrap_or(max_age)
    // fallback that was incorrectly treating fresh files as stale.
    let is_stale = cache_path
        .metadata()
        .and_then(|m| m.modified())
        .map(|mtime| {
            SystemTime::now()
                .duration_since(mtime)
                .unwrap_or(Duration::ZERO)   // clock skew → treat as age 0 (fresh)
                >= max_age
        })
        .unwrap_or(true); // can't read metadata → treat as missing

    let need_download = has_tmp || args.refresh_cache || !cache_path.exists() || is_stale;

    if need_download {
        // ── Resume support for assembly summary files ─────────────────────────
        // Stream to <filename>.tmp first. On completion, atomically rename to
        // the final path. A killed run leaves the .tmp file intact; the next
        // invocation detects it (has_tmp = true) and resumes via Range header.
        let existing_bytes = tmp_path.metadata().map(|m| m.len()).unwrap_or(0);

        if existing_bytes > 0 {
            println!(
                "↷ Resuming assembly summary download ({:.1} MB already present) → {}",
                existing_bytes as f64 / 1_048_576.0,
                cache_path.display()
            );
        } else {
            println!("↷ Downloading assembly summary → {}", cache_path.display());
        }

        let client = reqwest::Client::new();
        let mut req = client.get(url);
        if existing_bytes > 0 {
            req = req.header("Range", format!("bytes={}-", existing_bytes));
        }
        let response = req.send().await?;
        let status = response.status();
        let total_size = response.content_length().unwrap_or(0)
            + if status == reqwest::StatusCode::PARTIAL_CONTENT {
                existing_bytes
            } else {
                0
            };

        let pb = ProgressBar::new(total_size);
        pb.set_position(if status == reqwest::StatusCode::PARTIAL_CONTENT {
            existing_bytes
        } else {
            0
        });
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
                )?
                .progress_chars("#>-"),
        );

        // Open .tmp for append (resume) or create fresh
        let file_handle = if status == reqwest::StatusCode::PARTIAL_CONTENT {
            std::fs::OpenOptions::new().append(true).open(&tmp_path)?
        } else {
            File::create(&tmp_path)?
        };
        let mut file = BufWriter::new(file_handle);
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item.context("Error while downloading metadata chunk")?;
            file.write_all(&chunk)?;
            pb.inc(chunk.len() as u64);
        }
        file.flush()?;
        pb.finish_with_message("Download complete.");

        // ── Atomic rename: only now does the final cache file appear ──────────
        fs::rename(&tmp_path, &cache_path)?;
        println!("✔ Saved: {}", cache_path.display());
    } else {
        let age_secs = cache_path
            .metadata()
            .and_then(|m| m.modified())
            .map(|mtime| {
                SystemTime::now()
                    .duration_since(mtime)
                    .unwrap_or_default()
                    .as_secs()
            })
            .unwrap_or(0);
        println!(
            "✔ Using cached assembly summary ({:.1} days old): {}",
            age_secs as f64 / 86_400.0,
            cache_path.display()
        );
    }

    // ── Parse from disk cache — header extracted from file, not hardcoded ──────
    println!("  Filtering records for {}...", args.organism);

    let file = File::open(&cache_path)?;
    let mut reader = std::io::BufReader::new(file);
    let mut header: Vec<String> = Vec::new();

    // Use read_line on the BufReader directly so the reader's cursor advances
    // past the header lines. The same BufReader is then handed to the CSV reader
    // for data rows — no need to reopen the file or use a Lines iterator.
    //
    // Line 1: "##  See ftp://..." — skip (description comment)
    // Line 2: "#assembly_accession\tbioproject\t..." — parse as header
    // Line 3+: data rows — consumed by CSV reader below
    loop {
        let mut line = String::new();
        let n = std::io::BufRead::read_line(&mut reader, &mut line)?;
        if n == 0 {
            break;
        } // EOF before header found
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed.starts_with("##") {
            continue; // description line — skip
        } else if trimmed.starts_with('#') {
            // header line — strip leading # and split on tab
            header = trimmed
                .trim_start_matches('#')
                .split('\t')
                .map(|s| s.trim().to_string())
                .collect();

            // ── Validate required columns exist ───────────────────────────────
            for required in REQUIRED_COLS {
                if !header.contains(&required.to_string()) {
                    anyhow::bail!(
                        "Required column '{}' not found in {}\nAvailable columns: {}",
                        required,
                        cache_path.display(),
                        header.join(", ")
                    );
                }
            }
            println!("  Detected {} columns from file header.", header.len());
            break;
        }
    }

    if header.is_empty() {
        anyhow::bail!("No header line found in {}", cache_path.display());
    }

    // Build column name → index lookup for O(1) field access
    let col: HashMap<&str, usize> = header
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), i))
        .collect();

    // Helper function: get field value by column name.
    // Defined as a named function (not closure) so lifetimes can be annotated
    // explicitly — closures cannot carry explicit lifetime parameters in Rust.
    // The returned &str borrows from `row`, not from `name`.
    fn get<'a>(col: &HashMap<&str, usize>, row: &'a [String], name: &str) -> &'a str {
        col.get(name)
            .and_then(|&i| row.get(i))
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    // Parse data rows — BufReader cursor already past header lines
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(reader);

    for result in rdr.records() {
        let row: csv::StringRecord = result?;
        let row: Vec<String> = row.iter().map(|s| s.to_string()).collect();

        let organism_name = get(&col, &row, "organism_name");
        let version_status = get(&col, &row, "version_status");
        let assembly_level = get(&col, &row, "assembly_level");
        let genome_rep = get(&col, &row, "genome_rep");
        let ftp_path = get(&col, &row, "ftp_path");
        let biosample = get(&col, &row, "biosample");
        let assembly_accession = get(&col, &row, "assembly_accession");

        // ── Subspecies suffix ─────────────────────────────────────────────────
        let name_lower = organism_name.to_lowercase();
        let subsp_suffix = name_lower
            .strip_prefix(&args.organism.to_lowercase())
            .unwrap_or("")
            .trim()
            .to_string();

        let passes_only =
            args.only_subsp.is_empty() || subsp_suffix.contains(&args.only_subsp.to_lowercase());

        let passes_exclude = args
            .exclude_subsp
            .iter()
            .all(|ex| !subsp_suffix.contains(&ex.to_lowercase()));

        // genome_size filter — rejects plasmid-only assemblies
        let genome_size: u64 = get(&col, &row, "genome_size").parse().unwrap_or(0);
        let passes_size = args.min_genome_size == 0 || genome_size >= args.min_genome_size;

        // group filter — rejects viral, fungal, eukaryotic contamination
        // "bacteria" is the only accepted value for bacterial genome downloads.
        // Empty group field (some older records) is allowed through.
        let group = get(&col, &row, "group");
        let passes_group = group.is_empty() || group == "bacteria";

        // species_taxid — stored in record for post-filter contamination check.
        // Not used as a hard filter: the value varies per organism and is
        // unknown at parse time. Uniqueness is validated in the summary block.
        let _species_taxid = get(&col, &row, "species_taxid");

        if organism_name.starts_with(&args.organism)
            && passes_only
            && passes_exclude
            && passes_size
            && passes_group
            && version_status == args.version_status
            && args.assembly_level.iter().any(|a| a == assembly_level)
            && genome_rep == args.genome_rep
            && ftp_path.starts_with("https")
        {
            // Build dynamic record: column name → value
            let record: AssemblyRecord = header
                .iter()
                .zip(row.iter())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            let existing = map.get(biosample);
            let is_new = existing.is_none();
            let is_better = !is_new
                && assembly_accession.starts_with("GCF")
                && !existing
                    .unwrap()
                    .get("assembly_accession")
                    .map(|s| s.starts_with("GCF"))
                    .unwrap_or(false);

            if is_new || is_better {
                map.insert(biosample.to_string(), record);
            }
        }
    }

    println!("  Found {} matching genomes so far.", map.len());
    Ok(())
}

/// Format a number with thousand-separator commas (e.g. 5067 → "5,067").
/// Rust's std fmt does not support {:,} so this helper is required.
fn fmt_num(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn save_outputs(map: &HashMap<String, AssemblyRecord>, outdir: &Path) -> Result<()> {
    let summary_path = outdir.join("filtered_assemblies.txt");
    let url_path = outdir.join("ftp_urls.txt");

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(&summary_path)?;

    // ── Derive column order from the first record in the map ─────────────────
    // Column names come from the file header parsed at runtime — no hardcoding.
    // All records in the map have identical keys (same source file schema),
    // so any record can be used to recover the column list.
    let Some(first) = map.values().next() else {
        println!("  No records to write.");
        return Ok(());
    };

    // Collect and sort keys to produce a stable, consistent column order.
    // Sorting alphabetically is reproducible; original file order is not
    // recoverable from a HashMap without storing the header separately.
    let mut col_names: Vec<&String> = first.keys().collect();
    col_names.sort();

    // Write header row — derived from actual file columns, not hardcoded
    wtr.write_record(col_names.iter().map(|s| s.as_str()))?;

    let mut url_file = BufWriter::new(File::create(&url_path)?);

    for rec in map.values() {
        // Write values in the same sorted column order as the header
        let row: Vec<&str> = col_names
            .iter()
            .map(|k| rec.get(*k).map(|s| s.as_str()).unwrap_or(""))
            .collect();
        wtr.write_record(&row)?;

        let ftp_path = rec
            .get("ftp_path")
            .map(|s| s.as_str())
            .unwrap_or("")
            .trim_end_matches('/');
        let asm_id = ftp_path.split('/').next_back().unwrap_or_default();
        writeln!(url_file, "{}/{}_genomic.fna.gz", ftp_path, asm_id)?;
    }

    println!("Saved filtered summary to: {}", summary_path.display());
    println!("Saved URL list to:         {}", url_path.display());
    Ok(())
}

async fn download_all_genomes(
    records: HashMap<String, AssemblyRecord>,
    args: &Args,
    effective_threads: usize,
) -> Result<()> {
    let semaphore = Arc::new(Semaphore::new(effective_threads));
    let outdir: PathBuf = PathBuf::from(&args.outdir);
    let total = records.len() as u64;

    // ── Pre-flight: tally already-complete genomes so the progress bar
    //    starts at the correct position rather than zero. ──────────────
    // Guard: skip records where ftp_path is empty or "na" — these produce
    // asm_id = "" and create ghost files (_genomic.fna) that falsely satisfy
    // the exists() check and cause all 5000+ records to be counted as done.
    let already_done = records
        .values()
        .filter(|rec| {
            let ftp = rec
                .get("ftp_path")
                .map(|s| s.as_str())
                .unwrap_or("")
                .trim_end_matches('/');
            if ftp.is_empty() || ftp == "na" {
                return false;
            }
            let asm_id = ftp.split('/').next_back().unwrap_or_default();
            if asm_id.is_empty() {
                return false;
            }
            let fna_path = outdir.join(format!("{}_genomic.fna", asm_id));
            let sentinel_path = outdir.join(format!("{}.done", asm_id));
            // A .done sentinel is written only after successful decompression —
            // it is a reliable completion marker regardless of genome file size.
            // Falling back to .fna existence handles genomes downloaded before
            // sentinel logic was introduced.
            sentinel_path.exists() || fna_path.exists()
        })
        .count() as u64;

    let remaining = total.saturating_sub(already_done);

    println!(
        "✔ Resume check — total: {}  already done: {}  remaining: {}",
        fmt_num(total as usize),
        fmt_num(already_done as usize),
        fmt_num(remaining as usize)
    );

    if remaining == 0 {
        println!("✔ All genomes already present. Nothing to download.");
        return Ok(());
    }

    let pb = ProgressBar::new(total);
    pb.set_position(already_done);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{desc}: [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
            .progress_chars("#>-"),
    );
    pb.set_message("Downloading");

    let mut tasks = Vec::new();

    for rec in records.into_values() {
        let sem = Arc::clone(&semaphore);
        let pb_clone = pb.clone();
        let outdir = outdir.clone();

        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            let ftp_path = rec
                .get("ftp_path")
                .map(|s| s.trim_end_matches('/').to_string())
                .unwrap_or_default();

            // Guard: skip records with missing or placeholder ftp_path —
            // these produce empty asm_id and create ghost _genomic.fna files.
            if ftp_path.is_empty() || ftp_path == "na" {
                pb_clone.inc(1);
                return;
            }

            let asm_id = ftp_path
                .split('/')
                .next_back()
                .unwrap_or_default()
                .to_string();
            if asm_id.is_empty() {
                pb_clone.inc(1);
                return;
            }

            let url = format!("{}/{}_genomic.fna.gz", ftp_path, asm_id);
            let gz_path = outdir.join(format!("{}_genomic.fna.gz", asm_id));
            let fna_path = outdir.join(format!("{}_genomic.fna", asm_id));

            let sentinel_path = outdir.join(format!("{}.done", asm_id));

            // Case 1 — sentinel exists (reliable completion marker) or
            //           .fna already present → nothing to do
            if sentinel_path.exists() || fna_path.exists() {
                pb_clone.inc(1);
                return;
            }

            // Case 2 — .fna.gz present (prior run downloaded but did not
            //           decompress) → decompress now, write sentinel, skip download
            if gz_path.exists() {
                if decompress_and_cleanup(&gz_path, &fna_path).is_ok() {
                    let _ = File::create(&sentinel_path);
                }
                pb_clone.inc(1);
                return;
            }

            // Case 3 — partial .fna.gz exists (interrupted transfer) → resume
            //           via HTTP Range request, then decompress
            // Case 4 — no file at all → fresh download
            match download_file_resume(&url, &gz_path).await {
                Ok(()) => {
                    if decompress_and_cleanup(&gz_path, &fna_path).is_ok() {
                        // Write sentinel file — marks this genome as fully complete.
                        // Used by resume logic instead of file size threshold.
                        let _ = File::create(&sentinel_path);
                    }
                }
                Err(e) => {
                    // Log failed URL to failed_downloads.txt for inspection/retry
                    let fail_path = outdir.join("failed_downloads.txt");
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&fail_path)
                    {
                        let _ = writeln!(f, "{}	{}", url, e);
                    }
                }
            }

            pb_clone.inc(1);
        }));
    }

    futures_util::future::join_all(tasks).await;
    pb.finish_with_message("Workflow complete.");
    Ok(())
}

// ── Idiomatic Rust API Guidelines: prefer `&Path` over `&PathBuf` for fn args ─

/// Download a file with HTTP Range-based resume support and exponential backoff.
///
/// Retries up to MAX_RETRIES times on HTTP 503 (NCBI rate-limit) with
/// exponential backoff: 2s, 4s, 8s, 16s. Other non-success statuses fail
/// immediately. On a fresh download the first chunk is validated against the
/// gzip magic bytes (0x1f 0x8b) — an HTML/XML error page is rejected and
/// the partial file deleted so the caller can log and skip cleanly.
async fn download_file_resume(url: &str, path: &Path) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    const MAX_RETRIES: u32 = 4;

    let existing_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
    let client = reqwest::Client::new();

    let mut attempt = 0u32;
    let response = loop {
        let request = {
            let mut r = client.get(url);
            if existing_bytes > 0 {
                r = r.header("Range", format!("bytes={}-", existing_bytes));
            }
            r
        };

        let resp = request.send().await?;
        let status = resp.status();

        if status == reqwest::StatusCode::SERVICE_UNAVAILABLE && attempt < MAX_RETRIES {
            // NCBI rate-limiting — back off exponentially before retrying
            let wait = std::time::Duration::from_secs(2u64.pow(attempt + 1));
            tokio::time::sleep(wait).await;
            attempt += 1;
            continue;
        }

        if !status.is_success() {
            anyhow::bail!("HTTP {} for {}", status, url);
        }

        break resp;
    };

    let status = response.status();

    let mut file = if status == reqwest::StatusCode::PARTIAL_CONTENT {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .await?
    } else {
        tokio::fs::File::create(path).await?
    };

    let mut stream = response.bytes_stream();
    let mut first = true;

    while let Some(item) = stream.next().await {
        let chunk = item.context("Error reading download chunk")?;

        // ── Gzip magic byte check on the very first chunk ─────────────────
        // NCBI serves XML/HTML error pages with HTTP 200 for withdrawn or
        // missing accessions. A real .fna.gz always starts with 0x1f 0x8b.
        // Check applies to fresh downloads only (existing_bytes == 0).
        // For resume responses (HTTP 206) the first chunk is mid-file so the
        // magic bytes are not present — skip the check in that case.
        if first {
            first = false;
            if existing_bytes == 0 && chunk.len() >= 2 && !(chunk[0] == 0x1f && chunk[1] == 0x8b) {
                // Delete the corrupt file and bail — caller logs and skips
                drop(file);
                let _ = std::fs::remove_file(path);
                anyhow::bail!(
                    "Not a gzip (0x{:02x}{:02x}) — NCBI error page for: {}",
                    chunk[0],
                    chunk[1],
                    url
                );
            }
        }

        file.write_all(&chunk).await?;
    }

    file.flush().await?;
    Ok(())
}

fn decompress_and_cleanup(src: &Path, dst: &Path) -> Result<()> {
    let gz_file = File::open(src)?;
    let mut decoder = flate2::read::GzDecoder::new(gz_file);
    let mut out_file = File::create(dst)?;
    std::io::copy(&mut decoder, &mut out_file)?;
    fs::remove_file(src)?;
    Ok(())
}
