# Changelog

All notable changes to this project are documented in this file.  
The format is based on https://keepachangelog.com/en/1.1.0/

## [Unreleased]

## [0.4.2] - 2026-03-01
### Added
- Added FTUE changes for account setup in https://github.com/tilesprivacy/tiles/pull/88
- Added OTA updater in https://github.com/tilesprivacy/tiles/pull/89
  - Supports auto update checking and installing
  - Use `tiles update` for updating Tiles CLI manually

### Changed
- Integrated Harmony renderer for gpt-oss model in https://github.com/tilesprivacy/tiles/pull/92

### Fixed
- fix: Added path unavailability warning during installation in https://github.com/tilesprivacy/tiles/pull/90
- coverage patch-1 in @https://github.com/tilesprivacy/tiles/pull/91

## [0.4.1] - 2026-02-22
### Added
- Identity system for Tiles:
  - `tiles account` to show account details
  - `tiles account create <nickname>` to create root identity and optional nickname
  - `tiles account set-nickname` to set a nickname for root identity
- Updated CLI to include default `tiles` command

## [0.4.0] - 2026-02-04
### Added
- Portable Python runtime in the installer (no system Python required)
- Bundled default Modelfiles and direct reading of system prompt from Modelfile
- Support for `gpt-oss-20b` in interactive chat
- Basic support for the Open Responses API (`/v1/responses`) and REST endpoints
- Token metrics for model responses in the REPL
- `-m` flag for `tiles run` to execute Tiles in memory mode (experimental)
- Tilekit 0.2.0: `optimize` subcommand for automatic system-prompt optimization via DSRs

## [0.3.1] - 2026-01-09
### Added
- `--relay-count` / `-r` option for `tiles run` (helps if model gets stuck)
- CLI shows progress status while downloading models
- Slash commands and placeholder hint in the REPL
- Ability to set custom memory location via `tiles memory set-path <PATH>`

### Changed
- Minor internal refactoring

## [0.3.0] - 2026-01-06
### Fixed
- Tiles binary startup issue when run from outside a project directory
- Model not unloading after exiting the REPL
- Updated Python version to 3.13 for development
- Enabled basic Linux compatibility

### Changed
- Basic refactoring to support multiple inference runtimes

## [0.2.0] - 2025-12-20
### Added
- Server commands
- Streaming support with “thinking tokens” in the CLI
- Auto-downloading of model specified in Modelfile
