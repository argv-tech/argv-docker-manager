# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### 🚀 Features

- Add persistent per-project auto-restart selection and a dynamically generated systemd boot unit

## [0.2.0] - 2026-02-24

### 🐛 Bug Fixes

- Stop docker events and child processes on app exit
- I fixed ai code because it's the worse

## [0.1.1] - 2026-02-19

### 🚀 Features

- Sorting
- Improve daemon ui

### 🐛 Bug Fixes

- Resolve status update delays in service start and stop
- Logs
- Services Statutes

### 🚜 Refactor

- Better code base
- *(keybinds)* Better vim style keybinds
- Remove unused re-exports from src/docker/mod.rs
- *(ui)* Improve ui

### ⚙️ Miscellaneous Tasks

- Code formatting

## [0.1.0] - 2025-12-18

### 🚀 Features

- *(postgres)* Volume
- Mongodb
- Mongodb
- Moodle
- Odoo-14
- Formatting
- Add adminer
- Add wordpress
- Mysql
- Init.tmux
- Add run.sh
- Phpmyadmin
- *(phpmyadmin)* Adjust upload limits
- Redis
- Tmux support
- Multi container support in tmux
- Select window
- Check if container aleardy running
- Tui app
- Add daemon start button, container logs panel, and UI enhancements
- Replace output panel with notification toast
- Better container logs
- Improve layout
- *(logs)* Better simpler logs
- Dynamic keybinds
- *(ui)* Improved layout
- Handle commands when docker.service is not running
- Live logs

### 🐛 Bug Fixes

- *(mysql)* Volume
- *(run.sh)* Auto git pull
- Init.tmux
- Auto close
- Pgsql volumes
- Auto update
- Correct status display and docker-compose paths
- Scroll
- Services state
- States
- Live logs

### 💼 Other

- Ports

### 🚜 Refactor

- Rm "version"
- Reorganize project directory structure
- Simplify if-let expressions using && let pattern
- Remove states.json persistence and serde dependencies
- *(core)* Seperate logic
- *(logs)* Better real-time logs
- *(logs)* Initial logs

### 📚 Documentation

- Init
- LICENSE
- README.md
- CODE_OF_CONDUCT.md
- Update readme.md
- Focus README on Docker Manager and add run.sh alternative note

### ⚙️ Miscellaneous Tasks

- Update
- *(agents)* Running rules
- *(ai)* Update AGENTS
- Name and version
- Fix jobs

