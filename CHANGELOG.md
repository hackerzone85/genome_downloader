# Changelog

All notable changes to `genome_downloader` are documented here.

---

## [0.1.0] — 2026-03-09

### Initial release

**Core functionality**
- Download bacterial genomes from NCBI RefSeq and GenBank
- Filter by organism name, assembly level, version status, genome representation
- Deduplicate by BioSample — prefer RefSeq (GCF) over GenBank (GCA)
- Parallel downloads via Tokio async runtime

**Reliability**
- Resume interrupted downloads via HTTP Range requests
- Sentinel `.done` file completion tracking — reliable for any genome size
- Gzip magic byte (`0x1f 0x8b`) validation — rejects XML error pages
- HTTP 503 exponential backoff retry (4 attempts: 2s / 4s / 8s / 16s)
- Thread cap at 8 (empirically determined NCBI FTP limit)
- Failed download logging to `failed_downloads.txt`

**Filtering**
- `--min-genome-size` — removes plasmid-only assemblies (default: 1,000,000 bp)
- `group = bacteria` hard filter — rejects viral/fungal/eukaryotic contamination
- Dynamic `species_taxid` uniqueness validation — warns on multi-taxid datasets
- `--only-subsp` / `--exclude-subsp` — subspecies-level include/exclude filtering

**Cache**
- Disk cache for NCBI assembly summary files (~900 MB combined)
- Configurable cache age via `--cache-days` (default: 7 days)
- `--refresh-cache` flag for forced re-download
- `--cache-dir` allows sharing a single cache across multiple species runs

**Console output**
- Full parameter summary before any network activity
- Dataset summary with contamination checks and ✔/⚠ markers
- Organism name pattern report (strain name collapsed)
- Comma-formatted numbers throughout
- Resume check showing total / already done / remaining

**Tested species**
- *Klebsiella pneumoniae* (taxid 573) — 5,067 genomes
- *Klebsiella oxytoca* (taxid 571) — 90 genomes
- *Klebsiella variicola* (taxid 244366) — 211 genomes
