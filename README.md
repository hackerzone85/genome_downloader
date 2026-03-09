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
| `--cache-dir` | `<outdir>/cache` | Directory for cached NCBI assembly summary files |
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

### Reuse an existing cache across multiple species

```bash
genome_downloader -o "Klebsiella variicola" -a "Complete Genome,Chromosome" \
    -O kv_genomes --cache-dir kp_genomes/cache/
```

### Escherichia coli — Complete Genome only, subspecies coli

```bash
genome_downloader \
    -o "Escherichia coli" \
    -a "Complete Genome" \
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
| `<outdir>/cache/` | Cached NCBI assembly summary files |

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

---

## License

MIT License. See [LICENSE](LICENSE).

---

## Citation

If you use `genome_downloader` in your research, please cite this repository:

```
Mahendra Gaur. genome_downloader (2026). https://github.com/hackerzone85/genome_downloader
```
