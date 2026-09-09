> NOTICE: containers are not production

# Table of Contents
- [ARGV Podman Manager](#argv-podman-manager)
- [Available Containers](#available-containers)
- [Contributing](#contributing)
- [init.tmux - Automated Setup](#inittmux---automated-setup)
- [run.sh Usage](#runsh-usage)

# ARGV Podman Manager

This repository provides a collection of Compose configurations for development purposes and a Rust-based terminal UI application for interactively managing Podman Compose services.

## ARGV Podman Manager

ARGV Podman Manager (`argv-podman-manager`) is a Rust terminal interface for managing the Podman Compose projects in this repository.

### Features
- Interactive selection and management of Podman Compose services
- Start, stop, and toggle service states
- View live container logs and Podman events
- Search/filter services
- Podman API socket control (start/stop/restart)
- Toast notifications for actions
- Configurable keybinds
- Per-project auto-restart selection at boot

### Dependencies
- Rust 2024 and Cargo
- Podman and `podman-compose`
- ratatui, tokio, crossterm

### Building and Running
1. Ensure Rust is installed: https://rustup.rs/
2. Build the application:
   ```bash
   cargo build --release
   ```
3. Run the manager:
   ```bash
   ./target/release/argv-podman-manager
   ```

**Alternative:** If Rust or Cargo is not available, use the `run.sh` script for interactive container management.

### Usage
The top status rail shows the app version, Podman health, running-project count, and auto-start count.

**Navigation:**
- `Tab` / `Shift+Tab`: Select the next / previous service
- `h` / `l`: Focus Services / Logs

**Services Pane:**
- `j` / `k`: Scroll through services
- `s`: Toggle start/stop selected service
- `a`: Toggle auto-restart at boot for the selected service (`↻` marks enabled services)
- `/`: Search services (type to filter, Esc to exit)

**Logs Pane:**
- `j` / `k` / `Page Up` / `Page Down`: Scroll logs
- `Space`: Toggle auto-scroll
- `t`: Switch to Events tab
- `T`: Switch to Live Logs tab

**General:**
- `r`: Refresh services status
- `d`: Open Podman API socket control menu
- `q`: Quit

Keybinds are configurable in `keybinds.toml`.

Services are loaded from the `containers/` directory.

### Auto-restart selected services after reboot

Auto-restart is opt-in per Compose project. In the Services pane, select a project and press `a` to toggle it. The selected names are stored in the git-ignored `.argv-podman-manager-autorestart.toml` file in this clone. Existing Docker Manager auto-restart files are still read for migration.

After choosing services, install the boot unit once:

```bash
./target/release/argv-podman-manager --install-auto-restart
```

The installer asks for sudo access, generates a systemd unit with absolute paths to the current release binary and repository clone, enables it for `multi-user.target`, and does not start any unselected projects. This makes the unit work when the repository is cloned at a different path on another machine: build there and run the installer from that clone.

Re-run the installer after moving the clone or changing the binary location. Changing the selected services does not require reinstalling the unit.

To inspect the generated unit after installation, use the unit name printed by the installer:

```bash
systemctl status argv-podman-manager-autorestart.service
journalctl -u argv-podman-manager-autorestart.service
```

## Available Containers

Each directory contains a `compose.yml` for its service:

- **mysql** - MySQL database with integrated phpMyAdmin
- **postgres** - PostgreSQL database with integrated Adminer
- **redis** - Redis cache
- **phpmyadmin** - phpMyAdmin standalone (for external MySQL databases)
- **adminer** - Adminer standalone (for external databases)

## Contributing

### Adding New Containers

To add a new container configuration:

1. Create a new directory under `containers/` (e.g., `containers/myapp/`)

2. Add a `compose.yml` file with your service configuration. Follow these guidelines:
   - Use relative paths for volumes if needed
   - Expose ports appropriately for development
   - Include health checks where possible
   - Add comments for complex configurations

3. Test your configuration:
   ```bash
   cd containers/myapp
   podman-compose up
   podman-compose down
   ```

4. Update this README:
   - Add your container to the "Available Containers" list above
   - Describe what the container provides

5. Submit a pull request with your changes

### Guidelines
- Ensure containers are suitable for development environments
- Include necessary environment variables or configuration files
- Document any special setup requirements
- Follow Compose and Podman best practices

## init.tmux - Automated Setup

The `init.tmux` file provides automated tmux session setup:

**What it does:**
- Automatically runs `run.sh` in tmux window 1
- Sets a session-closed hook that kills all running Podman containers when you exit tmux

**Usage:**

Start tmux and source the init file in one command:
```bash
tmux new -s containers \; source-file ./init.tmux
```

Or source it in an existing tmux session:
```bash
tmux source-file ./init.tmux
```

**Note:** The cleanup hook will stop all Podman containers (not just this project's) when the tmux session closes. If you have other containers running, use the manual method instead.

## run.sh Usage

The `run.sh` script provides an interactive way to start multiple containers.

### What it does
1. Runs `git pull` to update the repository
2. Starts the Podman Manager

### Dependencies
- **podman** and **podman-compose**
- **fd** - fast file finder
- **fzf** - fuzzy finder for interactive selection
- **tmux** - terminal multiplexer (required for multi-window experience)
- **systemd** - optional, for managing the Podman API socket and auto-restart unit

### How to use
1. Make the script executable:
   ```bash
   chmod +x ./run.sh
   ```

2. Start a tmux session (recommended):
   ```bash
   tmux new -s containers
   ```

3. Run the script:
   ```bash
   ./run.sh
   ```

4. Select containers:
   - Use arrow keys to navigate
   - Press `Tab` to select multiple containers
   - Press `Enter` to start selected containers

5. Each container will open in its own tmux window

### Stopping containers
- In tmux: Press `Ctrl+C` in the window running the container
- From another terminal: `podman-compose -f <dir>/compose.yml down`
