use libapt::{Distro, Key, Release, Result};
use log::{debug, info};
use env_logger::Env;

mod check;

use crate::check::AptCheck;

/// Setup env_logger.
fn init_logging() {
    let env = Env::default()
        .filter_or("APTCHECKR_LOG_LEVEL", "info")
        .write_style_or("APTCHECKR_LOG_STYLE", "always");

    env_logger::init_from_env(env);
}

/// Log user-provided distro information.
fn log_distro(distro: &Distro) {
    let name = if let Some(name) = &distro.name {
        format!("Name: {}", name)
    } else if let Some(path) = &distro.path {
        format!("Path: {}", path)
    } else {
        "No name or path!".to_string()
    };

    let key = match &distro.key {
        Key::ArmoredKey(url) => format!("Armored key from {url}"),
        Key::Key(url) => format!("Binary key from {url}"),
        Key::NoSignatureCheck => "No key. InRelease signature will not get verified!".to_string(),
    };

    info!("Distro-Info:\nURL: {}\n{}\nKey: {}", distro.url, name, key);
}

/// Lib entry point for apt repo checking.
pub async fn check_repo(distro: &Distro, components: Vec<String>, architectures: Vec<String>) -> Result<bool> {
    init_logging();
    log_distro(distro);

    debug!("Parsing InRelease file...");
    let release = Release::from_distro(distro).await?;

    debug!("Checking indices for components {:?} and architectures {:?}...", components, architectures);
    let mut check = AptCheck::new(release, components, architectures)?;

    let result = check.check_repo().await?;

    Ok(result)
}
