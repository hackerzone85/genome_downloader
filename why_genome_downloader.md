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
