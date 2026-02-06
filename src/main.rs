use clap::{Parser, Subcommand};
use std::fs;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

mod config;
mod hyprctl;
mod scheduler;
mod state;
mod transition;

#[derive(Parser, Debug)]
#[command(name = "rustysunset")]
#[command(author = "rustysunset developers")]
#[command(version = "0.1.0")]
#[command(about = "Smooth color temperature transitions for hyprsunset", long_about = None)]
#[allow(clippy::struct_excessive_bools)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, global = true)]
    config: Option<String>,

    #[arg(short, long, global = true)]
    verbose: bool,

    #[arg(long, global = true)]
    json: bool,

    #[arg(short, long, global = true)]
    quiet: bool,

    #[arg(long, global = true)]
    dry_run: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Daemon,
    Now,
    Status,
    Set { temperature: u16 },
    Pause,
    Resume,
    Config,
}

fn read_status_file(path: &str) -> (u16, String, u16, f64) {
    let content = fs::read_to_string(path).unwrap_or_default();
    let mut temp = 0;
    let mut phase = "unknown".to_string();
    let mut target = 0;
    let mut progress = 0.0;

    for line in content.lines() {
        if let Some(val) = line.strip_prefix("temp=") {
            if let Ok(t) = val.parse() {
                temp = t;
            }
        } else if let Some(val) = line.strip_prefix("phase=") {
            phase = val.to_string();
        } else if let Some(val) = line.strip_prefix("target=") {
            if let Ok(t) = val.parse() {
                target = t;
            }
        } else if let Some(val) = line.strip_prefix("progress=") {
            if let Ok(p) = val.parse() {
                progress = p;
            }
        }
    }

    (temp, phase, target, progress)
}

fn main() {
    env_logger::init();

    let args = Args::parse();

    let config_path = args
        .config
        .clone()
        .or_else(|| config::find_config().map(|p| p.to_string_lossy().into_owned()));

    let config = config::load(config_path.as_deref());

    match args.command {
        Some(Commands::Daemon) | None => {
            if let Err(e) = run_daemon(&config, args.dry_run, args.quiet) {
                eprintln!("Daemon error: {e}");
                process::exit(1);
            }
        }
        Some(Commands::Now) => {
            let (temp, _, _, _) = read_status_file(&config.daemon.status_file);
            if args.json {
                println!(r#"{{"temp":{temp}}}"#);
            } else {
                println!("{temp}K");
            }
        }
        Some(Commands::Status) => {
            let (temp, phase, target, progress) = read_status_file(&config.daemon.status_file);

            if args.json {
                println!(
                    r#"{{"temp":{temp},"phase":"{phase}","target":{target},"progress":{progress:.2}}}"#,
                );
            } else {
                println!("temp={temp}");
                println!("phase={phase}");
                println!("target={target}");
                println!("progress={progress:.2}");
            }
        }
        Some(Commands::Set { temperature }) => {
            if !args.quiet {
                println!("Setting temperature to {temperature}K");
            }
            if !args.dry_run {
                if let Err(e) = hyprctl::set_temperature(temperature) {
                    eprintln!("Failed to set temperature: {e}");
                    process::exit(1);
                }
                // Clear state file - this is an immediate override, not a transition
                let state_file = expand_path(&config.daemon.state_file);
                if let Some(ref p) = state_file {
                    let _ = fs::remove_file(p);
                }
                // Also update status file
                let status = format!(
                    "temp={temperature}\nphase=manual\ntarget={temperature}\nprogress=1.00\n",
                );
                let _ = fs::write(&config.daemon.status_file, status);
            }
        }
        Some(Commands::Pause) => {
            // Write pause command to control file
            let control_file = control_file_from_status(&config.daemon.status_file);
            let _ = fs::write(&control_file, "pause\n");
            if !args.quiet {
                println!("Paused");
            }
        }
        Some(Commands::Resume) => {
            // Write resume command to control file
            let control_file = control_file_from_status(&config.daemon.status_file);
            let _ = fs::write(&control_file, "resume\n");
            if !args.quiet {
                println!("Resumed");
            }
        }
        Some(Commands::Config) => {
            if args.json {
                println!("{}", serde_json::to_string(&config).unwrap());
            } else {
                println!("{}", toml::to_string_pretty(&config).unwrap());
            }
        }
    }
}

fn expand_path(path: &str) -> Option<std::path::PathBuf> {
    if path.starts_with('~') {
        dirs::home_dir().map(|home| home.join(&path[2..]))
    } else {
        Some(std::path::PathBuf::from(path))
    }
}

fn control_file_from_status(status_file: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(status_file).with_extension("control")
}

fn should_set_temperature(optimize_updates: bool, last_sent: Option<u16>, current: u16) -> bool {
    if !optimize_updates {
        return true;
    }

    match last_sent {
        Some(prev) => prev != current,
        None => true,
    }
}

#[allow(clippy::too_many_lines)]
fn run_daemon(
    config: &config::Config,
    dry_run: bool,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !quiet {
        log::info!("Starting rustysunset daemon");
    }

    hyprctl::ensure_hyprsunset_running()?;

    if !quiet {
        log::info!("Mode: {:?}", config.mode);
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    let paused = Arc::new(AtomicBool::new(false));

    // Set up signal handler for graceful shutdown
    let result = ctrlc::set_handler(move || {
        shutdown_clone.store(true, Ordering::SeqCst);
    });

    // Check for control file commands
    let control_file = control_file_from_status(&config.daemon.status_file);
    let status_file = std::path::PathBuf::from(&config.daemon.status_file);
    let state_file = config.daemon.state_file.clone();

    let scheduler = scheduler::Schedule::new(config.clone())
        .map_err(|e| format!("Invalid schedule configuration: {e}"))?;

    // Try to load state or calculate appropriate temperature
    let initial_temp = if config.mode == config::Mode::Auto || config.mode == config::Mode::Fixed {
        let target_temp = scheduler.target_temperature();

        // Check for saved state
        if let Some(saved_state) = state::State::load(&state_file) {
            let max_age = u64::from(config.transition.duration_minutes) * 60 * 2;
            if saved_state.age_seconds() < max_age {
                // Resume from saved state
                log::info!("Resuming transition from saved state");
                state::calculate_temperature_from_state(
                    &saved_state,
                    u64::from(config.transition.duration_minutes) * 60,
                    &config.transition.easing,
                )
            } else {
                // State too old, use calculated target
                log::info!("Saved state too old, calculating fresh");
                target_temp
            }
        } else {
            // No state, use calculated target
            target_temp
        }
    } else {
        config.temperature.day
    };

    let mut transition = transition::Transition::new_with_temp(config.clone(), initial_temp);

    let tick_interval = Duration::from_secs(config.daemon.tick_interval_seconds);

    // For tracking when to update status file
    let mut tick_count = 0;
    let status_update_interval = if config.daemon.status_update_interval == 0 {
        1
    } else {
        config.daemon.status_update_interval
    };

    let mut last_set_temperature: Option<u16> = None;

    loop {
        // Check control file for commands
        if let Ok(content) = fs::read_to_string(&control_file) {
            for line in content.lines() {
                match line.trim() {
                    "pause" => {
                        paused.store(true, Ordering::SeqCst);
                    }
                    "resume" => {
                        paused.store(false, Ordering::SeqCst);
                    }
                    _ => {}
                }
            }
            // Clear control file after reading
            let _ = fs::write(&control_file, "");
        }

        if shutdown.load(Ordering::SeqCst) {
            // Save state before exiting
            if !dry_run {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let start = transition.transition_start_timestamp();
                let elapsed = now.saturating_sub(start);

                let state = state::State {
                    transition_start_temp: transition.transition_start_temp(),
                    transition_start_timestamp: start,
                    elapsed_seconds: elapsed,
                    target_temp: transition.target_temperature(),
                };
                let _ = state.save(&state_file);
            }
            break;
        }

        if paused.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(100));
            continue;
        }

        let now = chrono::Local::now();
        let phase = scheduler.current_phase_at(now);
        let target_temp = match phase {
            scheduler::Phase::Day | scheduler::Phase::TransitioningToDay => config.temperature.day,
            scheduler::Phase::Night | scheduler::Phase::TransitioningToNight => {
                config.temperature.night
            }
        };

        if let Some(window) = scheduler.transition_window_at(now) {
            let elapsed = now.signed_duration_since(window.start);
            let elapsed = elapsed.to_std().unwrap_or_default();
            transition.align_with_schedule(window.start_temp, window.target_temp, elapsed);
        } else {
            transition.update(target_temp);
        }

        let temp = transition.current_temperature();
        let target = transition.target_temperature();
        let progress = transition.progress();

        if !quiet {
            log::info!(
                "Phase: {phase:?}, Temp: {temp}, Target: {target}, Progress: {progress:.2}",
            );
        }

        if !dry_run {
            if should_set_temperature(config.daemon.optimize_updates, last_set_temperature, temp) {
                if let Err(e) = hyprctl::set_temperature(temp) {
                    eprintln!("Error setting temperature: {e}");
                } else {
                    last_set_temperature = Some(temp);
                    log::info!("Set temperature to {temp}");
                }
            }

            // Only update status file at configured interval
            tick_count += 1;
            if tick_count >= status_update_interval {
                tick_count = 0;
                let status = format!(
                    "temp={temp}\nphase={phase}\ntarget={target}\nprogress={progress:.2}\n",
                    phase = phase.as_str(),
                );
                let _ = fs::write(&status_file, status);
            }
        }

        thread::sleep(tick_interval);
    }

    if let Err(e) = result {
        eprintln!("Error setting signal handler: {e}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_set_temperature;

    #[test]
    fn optimize_skips_same_temperature() {
        assert!(!should_set_temperature(true, Some(2000), 2000));
    }

    #[test]
    fn optimize_sets_when_temperature_changes() {
        assert!(should_set_temperature(true, Some(2000), 2100));
    }

    #[test]
    fn always_sets_when_optimization_disabled() {
        assert!(should_set_temperature(false, Some(2000), 2000));
    }
}
