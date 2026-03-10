# genome_downloader — Snakemake Workflow

A ready-to-use [Snakemake](https://snakemake.readthedocs.io/) workflow for downloading and quality-checking bacterial genomes from NCBI using [genome_downloader](https://github.com/hackerzone85/genome_downloader).

This workflow is provided as an alternative deployment method for users who prefer Snakemake over direct command-line usage. It integrates seamlessly into larger Snakemake pipelines.

---

## Workflow overview

```
STAGE 1 — download_genomes
    └── genome_downloader binary
        ├── Downloads NCBI assembly summary files (cached, ~1.65 GB, once only)
        ├── Filters by organism / assembly level / subspecies / genome size
        ├── Validates species_taxid uniqueness and group = bacteria
        ├── Downloads + decompresses .fna genome files in parallel
        └── Outputs: filtered_assemblies.txt, ftp_urls.txt, data/genomes/*.fna

STAGE 2 — genome_stats
    └── Per-genome FASTA statistics (size, contigs, GC%, N50)
        └── Output: results/genome_stats.tsv
```

---

## Quick start

### 1. Clone this workflow

```bash
git clone https://github.com/hackerzone85/genome_downloader
cd genome_downloader/snakemake_workflow
```

### 2. Download the genome_downloader binary

```bash
mkdir -p bin/

# Linux x86_64
wget -O bin/genome_downloader-linux-x86_64 \
    https://github.com/hackerzone85/genome_downloader/releases/latest/download/genome_downloader-linux-x86_64
chmod +x bin/genome_downloader-linux-x86_64
```

For other platforms (macOS, ARM, Windows) see the [Releases page](https://github.com/hackerzone85/genome_downloader/releases/latest).

### 3. Edit the config file

```bash
nano config/config.yaml
```

Key settings to review:

```yaml
tools:
  genome_downloader: "bin/genome_downloader-linux-x86_64"  # path to binary

download:
  organism:       "Klebsiella pneumoniae"         # change to your organism
  assembly_level: "Complete Genome,Chromosome"    # assembly levels to include
  exclude_subsp:  "ozaenae,rhinoscleromatis,complex,phage"  # subspecies to exclude
  min_genome_size: 1000000                        # minimum genome size in bp
```

### 4. Run the workflow

```bash
# Preview what will run (dry run)
snakemake --dry-run --configfile config/config.yaml

# Run with 8 cores
snakemake --cores 8 --configfile config/config.yaml

# Download genomes only (skip stats)
snakemake download_genomes --cores 8 --configfile config/config.yaml
```

---

## Directory structure

```
snakemake_workflow/
├── Snakefile                   ← workflow rules
├── config/
│   └── config.yaml             ← all parameters (edit this)
├── bin/
│   └── genome_downloader-*     ← binary (download separately)
├── data/
│   ├── genomes/                ← downloaded .fna files (created at runtime)
│   │   └── .download_complete  ← sentinel: all downloads confirmed done
│   ├── filtered_assemblies.txt ← filtered NCBI metadata TSV
│   └── ftp_urls.txt            ← FTP URL list
├── results/
│   ├── genome_stats.tsv        ← per-genome QC statistics
│   └── .logs/                  ← rule log files
└── README.md
```

---

## Output files

| File | Description |
|------|-------------|
| `data/genomes/*.fna` | Decompressed genome FASTA files, ready for downstream tools |
| `data/filtered_assemblies.txt` | Filtered NCBI assembly metadata (TSV, all 38 columns) |
| `data/ftp_urls.txt` | FTP URLs of all downloaded genomes |
| `data/genomes/.download_complete` | Snakemake sentinel — confirms all downloads are done |
| `results/genome_stats.tsv` | Genome size, contig count, GC%, N50 for every genome |
| `results/.logs/download_genomes.log` | Full genome_downloader console output |

---

## Console output during download

```
─────────────────────────────────────────────────
  genome_downloader  v0.1.2
─────────────────────────────────────────────────
  Organism          : Klebsiella pneumoniae
  Assembly level    : Complete Genome, Chromosome
  Version status    : latest
  Genome rep.       : Full
  Min genome size   : 1000000 bp
  Excluded subsp.   : ozaenae, rhinoscleromatis, complex, phage
  Output directory  : data/genomes
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
```

---

## Resuming interrupted downloads

`genome_downloader` uses HTTP Range requests and sentinel `.done` files. If a run is interrupted simply re-run the same command — completed genomes are skipped automatically:

```
✔ Resume check — total: 5,067  already done: 4,823  remaining: 244
```

---

## Using the shared assembly summary cache

The NCBI assembly summary files (~1.65 GB) are stored in `~/.ncbi_assembly_summaries` by default and shared across all organisms and projects. On the first run they are downloaded once. On all subsequent runs they are reused instantly:

```
✔ Using cached assembly summary (0.2 days old): ~/.ncbi_assembly_summaries/assembly_summary_refseq.txt
✔ Using cached assembly summary (0.2 days old): ~/.ncbi_assembly_summaries/assembly_summary_genbank.txt
```

To use a project-specific cache directory instead, change in `config/config.yaml`:
```yaml
download:
  summary_files_dir: "data/cache"
```

---

## Tested species

| Species | Taxid | Genomes | Download time |
|---------|-------|---------|---------------|
| *Klebsiella pneumoniae* | 573 | 5,067 | ~60 min |
| *Klebsiella oxytoca* | 571 | 90 | ~2 min |
| *Klebsiella variicola* | 244366 | 211 | ~4 min |

---

## Requirements

- Snakemake ≥ 7.0
- Python ≥ 3.8 (for `genome_stats` rule)
- `genome_downloader` binary (no other dependencies — statically compiled)

---

## Extending the workflow

The `download_genomes` rule outputs a sentinel file `data/genomes/.download_complete` that downstream rules can depend on. To extend this workflow with your own analysis rules:

```python
rule my_analysis:
    input:
        sentinel = "data/genomes/.download_complete",
        genomes  = expand("data/genomes/{sample}.fna", sample=SAMPLES)
    output:
        "results/my_output.tsv"
    shell:
        "my_tool {input.genomes} > {output}"
```

---

## License

MIT — see [LICENSE](../LICENSE)

## Citation

```
Mahendra Gaur. genome_downloader (2026). https://github.com/hackerzone85/genome_downloader
```
