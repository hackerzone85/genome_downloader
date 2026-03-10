# genome_downloader

A high-performance, general-purpose bacterial genome downloader from NCBI RefSeq and GenBank.

Built in Rust for production use in large-scale comparative genomics pipelines.

---

## Features

- Downloads complete and chromosome-level bacterial genomes from both NCBI RefSeq and GenBank
- Deduplicates by BioSample, preferring RefSeq (GCF) over GenBank (GCA)
- Disk cache for NCBI assembly summary files (avoids re-downloading ~900 MB on every run)
- Resume logic — interrupted downloads restart from where they left off
- Sentinel-based completion tracking (reliable regardless of genome file size)
- Plasmid-only assembly filter via `--min-genome-size`
- Contamination guards: `group = bacteria` filter + dynamic species_taxid validation
- Subspecies include/exclude filtering
- Parallel downloads with automatic NCBI FTP thread cap (8 threads)
- HTTP 503 exponential backoff retry (4 attempts)
- Gzip magic byte validation — rejects XML error pages saved as `.fna.gz`
- Rich console output with dataset summary and organism name pattern report

---

## Installation

### Pre-built binaries (recommended)

Download the latest release binary for your platform from the [Releases](../../releases) page.

```bash
# Linux x86_64
chmod +x genome_downloader-linux-x86_64
./genome_downloader-linux-x86_64 --help
```

### Build from source

```bash
git clone https://github.com/hackerzone85/genome_downloader
cd genome_downloader
cargo build --release
./target/release/genome_downloader --help
```

Requires Rust 1.75+ (`rustup` recommended).

---

## Usage

```bash
genome_downloader -o "Organism name" [OPTIONS]
```

### Required

| Flag | Description |
|------|-------------|
| `-o`, `--organism` | Organism name, e.g. `"Klebsiella pneumoniae"` |

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `-a`, `--assembly-level` | `Chromosome` | Assembly level(s), comma-separated. Options: `Complete Genome`, `Chromosome`, `Scaffold`, `Contig` |
| `-v`, `--version-status` | `latest` | Version status. Use `latest` for current assemblies |
| `-g`, `--genome-rep` | `Full` | Genome representation |
| `-O`, `--outdir` | `./genome_data` | Output directory for downloaded `.fna` files |
| `-t`, `--threads` | `8` | Parallel download slots (capped at 8 for NCBI FTP) |
| `--min-genome-size` | `1000000` | Minimum genome size in bp — filters plasmid-only assemblies. Set to `0` to disable |
|`--summary-files-dir` | `<outdir>/cache` | **NEW** Directory for cached NCBI assembly summary files. Share this across organisms to avoid re-downloading 1.65 GB per organism |
| `--cache-dir` | *(deprecated)* | Deprecated alias for `--summary-files-dir` |
| `--cache-days` | `7` | Maximum age of cached summary before re-download |
| `--refresh-cache` | `false` | Force re-download of assembly summary files |
| `--only-subsp` | _(none)_ | Restrict to one subspecies epithet (case-insensitive substring match) |
| `--exclude-subsp` | _(none)_ | Exclude subspecies epithets, comma-separated |

---

## Examples

### Klebsiella pneumoniae — Complete Genome and Chromosome, excluding known non-pneumoniae subspecies

```bash
genome_downloader \
    -o "Klebsiella pneumoniae" \
    -a "Complete Genome,Chromosome" \
    -t 8 \
    -O kp_genomes \
    --exclude-subsp "ozaenae,rhinoscleromatis,complex,phage" \
    --min-genome-size 1000000
```

### Klebsiella oxytoca — Complete Genome only

```bash
genome_downloader \
    -o "Klebsiella oxytoca" \
    -a "Complete Genome" \
    -O ko_genomes
```

### Reuse an existing cache across multiple species (recommended)
When processing multiple organisms, reuse a single summary cache to save 4+ minutes and 1.65 GB per organism:

```bash
# Set up once
export SUMMARY_DIR=~/.ncbi_assembly_summaries
mkdir -p "$SUMMARY_DIR"

# E. coli — downloads summaries (~4 min)
genome_downloader \
    -o "Escherichia coli" \
    -a "Complete Genome,Chromosome" \
    --summary-files-dir "$SUMMARY_DIR" \
    -O ecoli_genomes

# Klebsiella — reuses summaries from cache (~1 sec) ⚡
genome_downloader \
    -o "Klebsiella pneumoniae" \
    -a "Complete Genome,Chromosome" \
    --summary-files-dir "$SUMMARY_DIR" \
    -O kp_genomes

# Staphylococcus — reuses summaries from cache (~1 sec) ⚡
genome_downloader \
    -o "Staphylococcus aureus" \
    -a "Complete Genome,Chromosome" \
    --summary-files-dir "$SUMMARY_DIR" \
    -O staph_genomes
```

### Escherichia coli — Complete Genome only, subspecies coli

```bash
genome_downloader \
    -o "Escherichia coli" \
    -a "Complete Genome,Chromosome" \
    --summary-files-dir "$SUMMARY_DIR" \
    --only-subsp "subsp. coli" \
    -O ecoli_genomes
```
---

## Output

| File | Description |
|------|-------------|
| `<outdir>/*.fna` | Decompressed FASTA genome files |
| `<outdir>/<accession>.done` | Sentinel files marking completed downloads |
| `<outdir>/filtered_assemblies.txt` | Filtered assembly metadata (TSV) |
| `<outdir>/ftp_urls.txt` | FTP URLs of all downloaded genomes |
| `<outdir>/failed_downloads.txt` | URLs that failed after all retries (if any) |
| `~/.ncbi_assembly_summaries` | Cached NCBI assembly summary files (Default Dir.) |

---

## Console Output

```
─────────────────────────────────────────────────
  genome_downloader  v0.1.0
─────────────────────────────────────────────────
  Organism          : Klebsiella pneumoniae
  Assembly level    : Complete Genome, Chromosome
  Version status    : latest
  Genome rep.       : Full
  Min genome size   : 1000000 bp
  Subspecies filter : all accepted
  Excluded subsp.   : ozaenae, rhinoscleromatis, complex, phage
  Output directory  : kp_genomes
  Threads           : 8
─────────────────────────────────────────────────
  Dataset Summary
─────────────────────────────────────────────────
  Total genomes         :     5,067
  Complete Genome       :     4,699
  Chromosome            :       368
  Group = bacteria      :     5,067 /     5,067  ✔
  Species taxid         :       573  ✔
  Version = latest      :     5,067 /     5,067  ✔
  Genome rep = Full     :     5,067 /     5,067  ✔
  Organism name patterns (strain name collapsed)
─────────────────────────────────────────────────
      4,845  →  Klebsiella pneumoniae
        222  →  Klebsiella pneumoniae subsp. pneumoniae
─────────────────────────────────────────────────
✔ Resume check — total: 5,067  already done: 5,067  remaining: 0
✔ All genomes already present. Nothing to download.
```

---

## Design Notes

**Why is the thread limit capped at 8?**
Empirical testing showed no download speed improvement beyond 8 concurrent connections to NCBI FTP — the bottleneck is network/NCBI server throughput. Requests above 10 threads reliably trigger HTTP 503 throttling responses.

**Why sentinel files instead of file size?**
Early versions used a 1 MB size threshold to detect complete downloads. This was found to be incorrect — legitimate plasmid-only assemblies annotated as "Complete Genome" by NCBI can be as small as 47 KB. Sentinel `.done` files written after successful decompression are a reliable completion marker independent of genome size.

**Why dynamic species_taxid validation instead of a hardcoded value?**
The tool is designed to be organism-agnostic. Hardcoding taxid 573 would silently reject all records for any non-*K. pneumoniae* species. Instead, the tool collects all unique taxids found after filtering and warns if more than one is present — indicating possible cross-species contamination.

---

## Tested Species

| Species | Taxid | Genomes |
|---------|-------|---------|
| *Klebsiella pneumoniae* | 573 | 5,067 |
| *Klebsiella oxytoca* | 571 | 90 |
| *Klebsiella variicola* | 244366 | 211 |

## Why genome_downloader?

Large-scale bacterial genomics studies — particularly those involving thousands of genomes for comparative analysis, pangenome construction, or graph-based clonal complex discovery — impose demands that existing NCBI download tools were not designed to meet. This section provides an honest comparison against the four most commonly used alternatives, followed by a summary of where `genome_downloader` fills the gap.

---

### 1. NCBI Datasets CLI (Official Tool)

The official NCBI Datasets CLI (`datasets download genome taxon`) is well-maintained and broadly supported. For downloads under 1,000 genomes it works reliably. However, for large-scale bacterial genomics it has several documented limitations:

- **Large downloads require a multi-step dehydrate/rehydrate workflow.** NCBI's own documentation explicitly recommends that downloads exceeding 1,000 genomes or 15 GB use a dehydrated zip archive approach — download metadata first, unzip, then rehydrate. This adds manual steps and intermediate files that complicate pipeline integration.
- **No resume on interrupted downloads.** The rehydration step has no HTTP Range-based resume. If a multi-hour download for thousands of genomes is interrupted, the entire rehydration must restart. A GitHub issue filed by users directly reports the absence of a resume option for large bacterial downloads.
- **Intermittent zip archive corruption at scale.** Multiple users have reported `invalid zip archive` errors when downloading hundreds to thousands of genomes, a bug that NCBI acknowledged and confirmed they could reproduce.
- **Downloads bundled in zip archives.** Genomes arrive packaged inside a zip file with NCBI's directory structure (`ncbi_dataset/data/GCF_.../`) rather than as flat `.fna` files ready for downstream tools such as Mash or Prokka.
- **No plasmid-only assembly filter.** The CLI provides no mechanism to exclude assemblies that are annotated as "Complete Genome" but contain only plasmid sequence — a common source of contamination in bacterial datasets.
- **No cross-species contamination validation.** There is no built-in check to confirm that all downloaded assemblies belong to a single species taxid.

---

### 2. `ncbi-genome-download` (Community Python Tool)

The community tool `ncbi-genome-download` (kblin/ncbi-genome-download, PyPI) is widely used and does a good job for straightforward downloads. Its limitations at scale are:

- **Python runtime dependency.** The tool requires a Python environment (3.9–3.13) and pip installation. On HPC clusters or minimal Docker containers, this introduces dependency management overhead. `genome_downloader` ships as a single static binary with no runtime dependencies.
- **No HTTP Range resume.** Interrupted downloads of individual genome files restart from zero. For a 5,000-genome run on a shared network connection, partial failures are common and costly.
- **No disk cache for assembly summaries.** Every run re-downloads the full NCBI assembly summary files (~900 MB combined for RefSeq + GenBank), even when filtering parameters have not changed.
- **No plasmid-only assembly filter.** Like the NCBI CLI, it provides no genome size threshold to reject plasmid assemblies that are misleadingly annotated as "Complete Genome".
- **No pre-download dataset validation report.** The tool offers no summary of taxonomic composition, species taxid uniqueness, or contamination indicators before committing to a multi-hour download.
- **Sequential metadata parsing.** The tool downloads assembly summaries sequentially without caching, making repeated runs expensive in both time and bandwidth.

---

### 3. NCBI Web Interface (Manual Download)

The NCBI Datasets web interface at [ncbi.nlm.nih.gov/datasets](https://www.ncbi.nlm.nih.gov/datasets/) is appropriate for exploratory work and small downloads. It is not suitable for reproducible large-scale research:

- **Not scriptable or automatable.** Manual clicks through a web interface cannot be embedded in a Snakemake or Nextflow pipeline.
- **Practical limit of tens to hundreds of genomes.** Downloading thousands of genomes through a browser is not feasible.
- **Not reproducible.** There is no command log or parameter record that can be committed to a repository, making methods sections difficult to reproduce exactly.
- **No filtering for assembly quality or contamination.** The web interface offers limited filtering compared to programmatic access.

---

### 4. NCBI FTP Site (Direct `wget`/`rsync`)

Direct FTP access via `wget` or `rsync` is the lowest-level option — powerful but entirely manual:

- **No organism-level filtering.** The FTP site is organised by accession, not by organism or assembly quality. Selecting genomes for a specific species requires parsing assembly summary files manually.
- **No BioSample deduplication.** Without deduplication logic, the same genome may be downloaded twice — once from RefSeq and once from GenBank — inflating dataset size and corrupting downstream analyses.
- **No contamination checks.** Raw FTP access provides no validation of species taxid, taxonomic group, or genome size.
- **Resume is possible via `rsync` but complex to configure correctly** for thousands of files across nested FTP directories.
- **No dataset summary.** There is no pre-download report showing how many genomes will be retrieved or what their taxonomic composition will be.

---

### Where `genome_downloader` fits

`genome_downloader` was designed specifically for the requirements of large-scale bacterial comparative genomics — the scenario where thousands of genomes must be downloaded reproducibly, reliably, and with rigorous quality control, with results that are ready for immediate input into downstream tools such as Mash, Prokka, or Kleborate.

| Feature | NCBI Datasets CLI | ncbi-genome-download | Web Interface | Direct FTP | **genome_downloader** |
|---------|:-----------------:|:--------------------:|:-------------:|:----------:|:---------------------:|
| No runtime dependencies (single binary) | ✗ | ✗ | — | — | **✔** |
| HTTP Range resume on interruption | ✗ | ✗ | ✗ | partial | **✔** |
| Disk cache for assembly summaries | ✗ | ✗ | — | — | **✔** |
| BioSample deduplication (RefSeq preferred) | ✗ | ✔ | ✗ | ✗ | **✔** |
| Plasmid-only assembly filter | ✗ | ✗ | ✗ | ✗ | **✔** |
| Group = bacteria contamination guard | ✗ | ✗ | ✗ | ✗ | **✔** |
| Dynamic species taxid validation | ✗ | ✗ | ✗ | ✗ | **✔** |
| Pre-download dataset summary report | ✗ | ✗ | ✗ | ✗ | **✔** |
| Subspecies include/exclude filtering | ✗ | partial | ✗ | ✗ | **✔** |
| HTTP 503 exponential backoff retry | ✗ | ✗ | — | ✗ | **✔** |
| Flat `.fna` output (pipeline-ready) | ✗ (zip) | ✔ | ✗ | ✔ | **✔** |
| Scriptable / pipeline-embeddable | ✔ | ✔ | ✗ | partial | **✔** |
| Tested at scale (5,000+ genomes) | partial | ✔ | ✗ | ✔ | **✔** |

`genome_downloader` is not a replacement for the NCBI Datasets CLI in general-purpose use — for downloading a handful of genomes or non-bacterial organisms, the official tool is excellent. However, for large-scale bacterial genomics pipelines where reproducibility, contamination control, and pipeline resilience are requirements, `genome_downloader` provides capabilities that no single existing tool combines.
---

## License

MIT License. See [LICENSE](LICENSE).

---

## Citation

If you use `genome_downloader` in your research, please cite this repository:

```
Mahendra Gaur. genome_downloader (2026). https://github.com/hackerzone85/genome_downloader
```
