use libapt::{Distro, Key, Release, Result, Error};
use log::{debug, info, error};
use env_logger::Env;
use serde_json;
use std::fs::File;
use std::io::prelude::*;

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

fn save_as_json(check: AptCheck, filename: &str) ->Result<()> {
    let mut file = match File::create(filename) {
        Ok(file) => file,
        Err(e) => {
            let message = format!("Saving result failed! {e}");
            error!("{}", message);
            // TODO: extendable errors
            return Err(Error::new(&message, libapt::ErrorType::ApiUsage))
        }
    }; 

    let data = match serde_json::to_string_pretty(&check) {
        Ok(data) => data,
        Err(e) => {
            let message = format!("Json serializing failed! {e}");
            error!("{}", message);
            // TODO: extendable errors
            return Err(Error::new(&message, libapt::ErrorType::ApiUsage))
        }
    };

    match file.write(&data.as_bytes()) {
        Ok(_) => {},
        Err(e) => {
            let message = format!("Writing json failed! {e}");
            error!("{}", message);
            // TODO: extendable errors
            return Err(Error::new(&message, libapt::ErrorType::ApiUsage))
        }
    }

    Ok(())
}

/// Lib entry point for apt repo checking.
pub async fn check_repo(distro: &Distro, components: Vec<String>, architectures: Vec<String>, check_files: bool) -> Result<bool> {
    init_logging();
    log_distro(distro);

    debug!("Parsing InRelease file...");
    let release = Release::from_distro(distro).await?;

    debug!("Checking indices for components {:?} and architectures {:?}...", components, architectures);
    let mut check = AptCheck::new(release, components, architectures, check_files)?;

    let result = check.check_repo().await?;

    save_as_json(check, "result.json")?;

    Ok(result)
}
