# Changelog

All notable changes to this project will be documented in this file.

## [0.1.1] - 2024-03-10
### Fixed
- Fixed summary reporting logic to display all requested assembly levels
- Previously only showed hardcoded "Complete Genome" and "Chromosome"
- Now dynamically displays all levels requested via `--assembly-level` flag
- Resolves issue where Scaffold and Contig counts were not shown in Dataset Summary

## [0.1.0] - 2024-03-09
### Initial Release
- Initial release of genome_downloader
- Support for filtering genomes by organism, assembly level, and genome representation
- Parallel download support with resume capability
- Caching of NCBI assembly summary files
