# rustysunset

Smooth color temperature transitions for hyprsunset.

## Features

- Automatic sunrise/sunset calculation based on location
- Fixed schedule mode for explicit wakeup/bedtime times
- Smooth easing transitions (linear, ease_in, ease_out, ease_in_out)
- Simple TOML configuration
- Environment variable overrides
- UNIX philosophy: simple, composable, efficient
- Optimized updates (only calls hyprctl when temperature changes)

## Installation

### From Source

```bash
git clone https://github.com/rustysunset/rustysunset
cd rustysunset
cargo build --release
cargo install --path .
```

### From Crates.io

```bash
cargo install rustysunset
```

## Configuration

Create a config file at `~/.config/rustysunset/config.toml`:

```toml
mode = "auto"

[location]
latitude = 48.516
longitude = 9.12

[schedule]
wakeup = "07:00"
bedtime = "22:00"

[transition]
duration_minutes = 60
easing = "linear"

[temperature]
day = 6500
night = 1500

[daemon]
tick_interval_seconds = 5
optimize_updates = true
status_update_interval = 1
```

### Configuration Options

| Option | Description | Default |
|--------|-------------|---------|
| `mode` | `auto` or `fixed` | `auto` |
| `location.latitude` | Latitude for sunrise/sunset | `0.0` |
| `location.longitude` | Longitude for sunrise/sunset | `0.0` |
| `schedule.wakeup` | Wake time (HH:MM) | `07:00` |
| `schedule.bedtime` | Bed time (HH:MM) | `22:00` |
| `transition.duration_minutes` | Transition duration | `60` |
| `transition.easing` | Easing function | `linear` |
| `temperature.day` | Day temperature (K) | `6500` |
| `temperature.night` | Night temperature (K) | `1500` |
| `daemon.tick_interval_seconds` | Update interval | `5` |
| `daemon.optimize_updates` | Only call hyprctl when temp changes | `true` |
| `daemon.status_update_interval` | Status file update frequency (0=every tick) | `1` |

### Environment Variables

All config options can be overridden via environment variables:

```bash
RUSTYSUNSET_MODE=auto
RUSTYSUNSET_LATITUDE=48.516
RUSTYSUNSET_LONGITUDE=9.12
RUSTYSUNSET_DAY_TEMP=6500
RUSTYSUNSET_NIGHT_TEMP=1500
RUSTYSUNSET_TRANSITION_DURATION=60
RUSTYSUNSET_EASING=linear
RUSTYSUNSET_TICK_INTERVAL=5
RUSTYSUNSET_OPTIMIZE_UPDATES=true
RUSTYSUNSET_STATUS_UPDATE_INTERVAL=1
```

## Usage

```bash
rustysunset daemon    # Run the daemon (default)
rustysunset now       # Show current temperature
rustysunset status    # Show status (temp, phase, target, progress)
rustysunset set 3000  # Set temperature immediately
rustysunset pause     # Pause transition
rustysunset resume    # Resume transition
rustysunset config    # Print current config
```

### Status File

The daemon writes status to `/tmp/rustysunset.status`:

```
temp=5432
phase=night
target=1500
progress=0.75
```

Use this for waybar integration:

```json
"custom/rustysunset": {
    "exec": "rustysunset status",
    "interval": 5,
    "format": "{}K"
}
```

Or simply:

```bash
watch -n 5 rustysunset status
```

## Hyprland Integration

Add to your `~/.config/hypr/hyprland.conf`:

```hypr
exec-once = rustysunset daemon
```

Or as a systemd user service:

```ini
[Unit]
Description=rustysunset daemon
After=graphical-session.target

[Service]
ExecStart=%h/.cargo/bin/rustysunset daemon
Restart=on-failure

[Install]
WantedBy=default.target
```

Then enable:

```bash
systemctl --user daemon-reload
systemctl --user enable --now rustysunset
```

## Optimization

rustysunset follows UNIX philosophy:
- **Optimized updates**: By default, hyprctl is only called when the temperature actually changes
- **Configurable status updates**: Control how often the status file is updated (0 = every tick, N = every N ticks)
- **Simple status file**: Easy to parse with shell tools, suitable for waybar modules

## License

MIT
