use std::process::Command;

pub fn set_temperature(kelvin: u16) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("hyprctl")
        .args(["hyprsunset", "temperature", &kelvin.to_string()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("hyprctl failed: {stderr}").into());
    }

    Ok(())
}

fn is_hyprsunset_running() -> bool {
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
