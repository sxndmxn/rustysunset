use std::process::Command;

pub fn set_temperature(kelvin: u16) -> Result<(), Box<dyn std::error::Error>> {
    let args = ["hyprsunset", "temperature", &kelvin.to_string()];
    let output = Command::new("hyprctl")
        .args(&args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().map_or("unknown".to_string(), |c| c.to_string());
        return Err(format!(
            "hyprctl {} failed (exit code {}): {}",
            args.join(" "),
            exit_code,
            stderr.trim()
        ).into());
    }

    Ok(())
}

pub fn is_hyprsunset_running() -> bool {
    Command::new("pidof")
        .arg("hyprsunset")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn ensure_hyprsunset_running() -> Result<(), Box<dyn std::error::Error>> {
    if !is_hyprsunset_running() {
        log::info!("Starting hyprsunset...");
        Command::new("hyprsunset").spawn()?;
    }
    Ok(())
}
