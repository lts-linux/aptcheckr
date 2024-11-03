use std::process::exit;

use clap::{arg, command};
use libapt::{Distro, Key};
use aptcheckr::check_repo;

fn main() {
    let matches = command!()
        .arg(arg!([url] "URL of the APT repository. Defaults to Ubuntu apt repo.").required(false))
        .arg(arg!(-d --distro <DISTRO> "Name of the distribution. Defaults to jammy.").required(false))
        .arg(arg!(-p --path <PATH> "Path for flat repos. Use './' for root folder.").required(false))
        .arg(arg!(-k --key <KEY> "Signing key of the InRelease file.").required(false))
        .arg(arg!(-r --rawkey "Key is a binary key, i.e. not armored.").required(false))
        .arg(arg!(-c --component <COMPONENT> ... "Component to check.").required(false))
        .arg(arg!(-a --arch <ARCHITECTURE> ... "Architecture to check.").required(false))
        .get_matches();

    let url = match matches.get_one::<String>("url"){
        Some(name) => name.to_string(),
        None => "http://archive.ubuntu.com/ubuntu".to_string(),
    };

    let distro = match matches.get_one::<String>("distro"){
        Some(name) => Some(name.to_string()),
        None => None,
    };

    let path = match matches.get_one::<String>("path"){
        Some(name) => Some(name.to_string()),
        None => None,
    };

    let key = match matches.get_one::<String>("key"){
        Some(name) => Some(name.to_string()),
        None => None,
    };

    let distro = if distro == None && path == None {
        Some("jammy".to_string())
    } else {
        distro
    };

    let key = match key {
        Some(key) => if matches.get_flag("raw-key") {
            Key::key(&key)
        } else {
            Key::armored_key(&key)
        },
        None => Key::NoSignatureCheck,
    };

    let components: Vec<String> = match matches.get_many("component") {
        Some(comps) => {
            comps.map(|c: &String| c.to_string()).collect()
        },
        None => Vec::new(),
    };

    let architectures: Vec<String> = match matches.get_many("arch") {
        Some(archs) => {
            archs.map(|c: &String| c.to_string()).collect()
        },
        None => Vec::new(),
    };

    let d = Distro {
        url: url,
        name: distro,
        path: path,
        key: key,
    };

    match check_repo(&d, components, architectures) {
        Ok(success) => {
            if success {
                println!("Repo is OK.");
                exit(0);
            } else {
                println!("Issues were found during check, see logs.");
                exit(1);
            }
        }
        Err(e) => {
            println!("Repo check failed with error: {e}!");
            exit(2);
        }
    }
}
