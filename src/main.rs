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
mod transition;

#[derive(Parser, Debug)]
#[command(name = "rustysunset")]
#[command(author = "rustysunset developers")]
#[command(version = "0.1.0")]
#[command(about = "Smooth color temperature transitions for hyprsunset", long_about = None)]
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
            if let Err(e) = run_daemon(config.clone(), args.dry_run, args.quiet) {
                eprintln!("Daemon error: {e}");
                process::exit(1);
            }
        }
        Some(Commands::Now) => {
            let (temp, _, _, _) = read_status_file(&config.daemon.status_file);
            if args.json {
                println!(r#"{{"temp":{}}}"#, temp);
            } else {
                println!("{}K", temp);
            }
        }
        Some(Commands::Status) => {
            let (temp, phase, target, progress) = read_status_file(&config.daemon.status_file);

            if args.json {
                println!(
                    r#"{{"temp":{},"phase":"{}","target":{},"progress":{:.2}}}"#,
                    temp, phase, target, progress
                );
            } else {
                println!("temp={}", temp);
                println!("phase={}", phase);
                println!("target={}", target);
                println!("progress={:.2}", progress);
            }
        }
        Some(Commands::Set { temperature }) => {
            if !args.quiet {
                println!("Setting temperature to {}K", temperature);
            }
            if !args.dry_run {
                if let Err(e) = hyprctl::set_temperature(temperature) {
                    eprintln!("Failed to set temperature: {e}");
                    process::exit(1);
                }
                // Also update status file
                let status = format!(
                    "temp={}\nphase=manual\ntarget={}\nprogress=1.00\n",
                    temperature, temperature
                );
                let _ = fs::write(&config.daemon.status_file, status);
            }
        }
        Some(Commands::Pause) => {
            // Write pause command to control file
            let control_file = config.daemon.status_file.replace("status", "control");
            let _ = fs::write(&control_file, "pause\n");
            if !args.quiet {
                println!("Paused");
            }
        }
        Some(Commands::Resume) => {
            // Write resume command to control file
            let control_file = config.daemon.status_file.replace("status", "control");
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

fn run_daemon(
    config: config::Config,
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

    let paused = Arc::new(AtomicBool::new(false));
    let paused_clone = paused.clone();

    ctrlc::set_handler(move || {
        paused_clone.store(true, Ordering::SeqCst);
    })?;

    // Check for control file commands
    let control_file = config.daemon.status_file.replace("status", "control");
    let status_file = config.daemon.status_file.clone();

    let scheduler = scheduler::Schedule::new(config.clone());
    let mut transition = transition::Transition::new(config.clone());
    let tick_interval = Duration::from_secs(config.daemon.tick_interval_seconds);

    // For tracking when to update status file
    let mut tick_count = 0;
    let status_update_interval = if config.daemon.status_update_interval == 0 {
        1
    } else {
        config.daemon.status_update_interval
    };

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

        if paused.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(100));
            continue;
        }

        let phase = scheduler.current_phase();
        let target_temp = scheduler.target_temperature();
        transition.update(target_temp);

        let temp = transition.current_temperature();
        let target = transition.target_temperature();
        let progress = transition.progress();

        if !quiet {
            log::debug!(
                "Phase: {:?}, Temp: {}, Target: {}, Progress: {:.2}",
                phase,
                temp,
                target,
                progress
            );
        }

        // Only call hyprctl if temperature changed (optimization)
        if !dry_run {
            if let Err(e) = hyprctl::set_temperature(temp) {
                eprintln!("Error setting temperature: {}", e);
            } else {
                log::debug!("Set temperature to {}", temp);
            }

            // Only update status file at configured interval
            tick_count += 1;
            if tick_count >= status_update_interval {
                tick_count = 0;
                let status = format!(
                    "temp={}\nphase={}\ntarget={}\nprogress={:.2}\n",
                    temp,
                    phase.as_str(),
                    target,
                    progress
                );
                let _ = fs::write(&status_file, status);
            }
        }

        thread::sleep(tick_interval);
    }
}
