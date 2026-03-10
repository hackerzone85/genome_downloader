# Changelog

All notable changes to this project will be documented in this file.

## [0.1.2] - 2024-03-11

### Added
- New `--summary-files-dir` flag for explicit shared cache across organisms
- Multi-organism workflow section in README with time and bandwidth savings guidance
- Enhanced help text explaining cache reuse benefits for batch processing

### Changed
- `--cache-dir` flag now marked as deprecated (backward compatible, emits warning)
- Default cache location logic now prefers `--summary-files-dir` when specified
- Improved parameter summary output to clarify cache directory being used

### Fixed
- Compiler warning for unused variable (prefixed with underscore in filter logic)
- Clarified deprecation messaging for users transitioning from `--cache-dir`

### Performance
- Users processing 10+ organisms can save 30–40 minutes by sharing summary cache across runs
- First organism run: approximately 3–4 minutes. Subsequent organisms: approximately 1 second per organism
- Summary files (~900 MB combined) download once and reuse across all organism queries

## [0.1.1] - 2024-03-10
### Fixed
- Fixed summary reporting logic to display all requested assembly levels
- Previously only showed hardcoded "Complete Genome" and "Chromosome"
- Now dynamically displays all levels requested via `--assembly-level` flag
- Resolves issue where Scaffold and Contig counts were not shown in Dataset Summary

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
