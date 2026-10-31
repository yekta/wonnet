//! Makes the system open `wonnet://` links (the end of a Google sign-in) with this app.

/// Registers the scheme for this copy of the app, off the window's thread.
pub fn register() {
    std::thread::spawn(|| {
        if let Err(error) = register_now() {
            eprintln!("The wonnet:// links couldn't be registered: {error}");
        }
    });
}

#[cfg(target_os = "windows")]
fn register_now() -> std::io::Result<()> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let exe = std::env::current_exe()?;
    let exe = exe.display();
    let (classes, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey("Software\\Classes\\wonnet")?;
    classes.set_value("", &"URL:Wonnet")?;
    classes.set_value("URL Protocol", &"")?;
    let (icon, _) = classes.create_subkey("DefaultIcon")?;
    icon.set_value("", &format!("\"{exe}\",0"))?;
    let (command, _) = classes.create_subkey("shell\\open\\command")?;
    command.set_value("", &format!("\"{exe}\" \"%1\""))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn register_now() -> std::io::Result<()> {
    let exe = match std::env::var_os("APPIMAGE") {
        Some(image) => std::path::PathBuf::from(image),
        None => std::env::current_exe()?,
    };
    let applications = dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("applications");
    std::fs::create_dir_all(&applications)?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Wonnet\nExec=\"{}\" %u\nIcon=wonnet\nNoDisplay=true\nTerminal=false\nMimeType=x-scheme-handler/wonnet;\n",
        exe.display()
    );
    std::fs::write(applications.join("wonnet-links.desktop"), entry)?;
    let _ = std::process::Command::new("xdg-mime")
        .args(["default", "wonnet-links.desktop", "x-scheme-handler/wonnet"])
        .status();
    let _ = std::process::Command::new("update-desktop-database").arg(&applications).status();
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn register_now() -> std::io::Result<()> {
    Ok(())
}
