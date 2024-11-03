use libapt::{Distro, Key, Release, Result};
use log::{debug, info, LevelFilter};
use env_logger::{Builder, WriteStyle};

mod check;

use crate::check::AptCheck;

fn init_logging() {
    Builder::new()
        .filter(None, LevelFilter::Debug)
        .write_style(WriteStyle::Always)
        .init();
}

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

pub fn check_repo(distro: &Distro, components: Vec<String>, architectures: Vec<String>) -> Result<bool> {
    init_logging();
    log_distro(distro);

    debug!("Parsing InRelease file...");
    let release = Release::from_distro(distro)?;

    debug!("Checking indices for components {:?} and architectures {:?}...", components, architectures);
    let mut check = AptCheck::new(release, components, architectures)?;

    let result = check.check_repo()?;

    Ok(result)
}
