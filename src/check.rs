use std::collections::HashMap;

use libapt::{Architecture, Error, PackageIndex, PackageVersion, Release, Result, SourceIndex, VersionRelation, get_etag};
use log::{debug, error, info, warn};

pub struct AptCheck {
    components: Vec<String>,
    architectures: Vec<Architecture>,
    binary_indices: Vec<PackageIndex>,
    source_indices: HashMap<String, SourceIndex>,
    issues: Vec<(String, Architecture, Error)>,
    // (Component, Package, Dependency)
    missing_packages: Vec<(String, String, PackageVersion)>,
    // (Component, Package, Source)
    missing_sources: Vec<(String, String, String)>,
    release: Release,
}

impl AptCheck {
    pub fn new(release: Release, components: Vec<String>, architectures: Vec<String>) -> Result<AptCheck> {
        let components = if components.is_empty() {
            release.components.clone()
        } else {
            components
        };
    
        let architectures = if architectures.is_empty() {
            release.architectures.clone()
        } else {
            let mut result: Vec<Architecture> = Vec::new();
            for arch in architectures {
                result.push(Architecture::from_str(&arch)?);
            }
            result
        };

        Ok(AptCheck {
            components: components,
            architectures: architectures,
            binary_indices: Vec::new(),
            source_indices: HashMap::new(),
            issues: Vec::new(),
            missing_packages: Vec::new(),
            missing_sources: Vec::new(),
            release: release,
        })
    }

    pub fn check_repo(&mut self) -> Result<bool> {

        info!("Checking compliance of InRelease file...");
        match self.release.check_compliance() {
            Ok(_) => info!("InRelease complies to Debian policy."),
            Err(e) => warn!("InRelease does not comply to Debian policy: {e}"),
        }
    
        info!("Checking single components...");
        self.check()?;

        info!("Checking cross components...");
        self.cross_check()?;
        
        info!("Found {} issues.", self.issues.len());
        for (component, architecture, issue) in &self.issues {
            error!("Found issue in component {component} for architecture {architecture}: {issue}");
        }

        info!("Found {} missing binary dependencies.", self.missing_packages.len());
        for (component, package, dependency) in &self.missing_packages {
            warn!("Component {component}: Dependency {:?} of package {} is missing.", dependency, package);
        }

        info!("Found {} missing sources.", self.missing_sources.len());
        for (component, package, source) in &self.missing_sources {
            warn!("Component {component}: Source {} of package {} is missing.", source, package);
        }
    
        Ok(self.issues.is_empty() && self.missing_packages.is_empty() && self.missing_sources.is_empty())
    }
    
    fn cross_check(&mut self) -> Result<()> {
        // TODO: search for missing sources and packages in other components.
        Ok(())
    }

    fn check(&mut self) -> Result<()> {
        for component in &self.components.clone() {
            match self.check_source_component(component) {
                Ok(_) => {},
                Err(e) => {
                    let message = format!("Checking sources of component {component} failed: {e}");
                    error!("{}", message);
                    self.issues.push((component.clone(), Architecture::Source, Error::new(&message, libapt::ErrorType::DownloadFailure)));
                }
            }
        }

        for component in &self.components.clone() {
            for architecture in &self.architectures.clone() {
                if architecture == &Architecture::Source {
                    continue;
                }

                match self.check_binary_component(component, architecture) {
                    Ok(_) => {},
                    Err(e) => {
                        let message = format!("Checking component {component} for architecture {architecture} failed: {e}");
                        error!("{}", message);
                        self.issues.push((component.clone(), architecture.clone(), Error::new(&message, libapt::ErrorType::DownloadFailure)));
                    }
                }
            }
        }
    
        Ok(())
    }
    
    fn check_binary_component(&mut self, component: &str, architecture: &Architecture) -> Result<()> {
        info!("Checking binary index of component {component} for architecture {architecture}...");
        let index = PackageIndex::new(&self.release, component, architecture)?;

        for package in index.packages() {
            debug!("Checking binary package {package}...");
            // TODO: support for multiple source versions!
            let package = match index.get(&package, None) {
                Some(package) => package,
                None => {
                    let message = format!("Package {} of component {} and architecture {} is missing.", package, component, architecture);
                    error!("{}", message);
                    self.issues.push((component.to_string(), architecture.clone(), Error::new(&message, libapt::ErrorType::DownloadFailure)));

                    continue;
                }
            };

            debug!("Checking file of binary package {}...", package.package);
            // Check existence of linked deb file.
            match get_etag(&package.link.url) {
                Ok(_) => {} // pass!
                Err(e) => {
                    let message = format!("File {} of source {} is broken: {e}", &package.link.url, package.package);
                    error!("{}", message);
                    self.issues.push((component.to_string(), Architecture::Source, Error::new(&message, libapt::ErrorType::DownloadFailure)));
                }
            }

            debug!("Checking dependencies of binary package {}...", package.package);
            // Check for dependent packages.
            for dependency in &package.depends {
                let name = &dependency.name;
                debug!("Checking dependency {name} of binary package {}...", package.package);
                let version = Some(dependency.clone());
                match index.get(name, version) {
                    Some(_) => {} // OK
                    None => {
                        // Missing dependency
                        self.missing_packages.push((component.to_string(), package.package.clone(), dependency.clone()));
                    }
                }
            }

            debug!("Checking source of binary package {}...", package.package); 
            // Check for source package.
            if let Some(source) = &package.source {
                if let Some(source_index) = self.source_indices.get(component) {
                    let vd = PackageVersion {
                        name: source.clone(),
                        architecture: package.architecture.clone(),
                        relation: Some(VersionRelation::Exact),
                        version: Some(package.version.clone()),
                    };
                    match source_index.get(source, Some(vd)) {
                        Some(_) => {}, // Ok.
                        None => {
                            // Missing source package
                            self.missing_sources.push((component.to_string(), package.package.clone(), source.clone()));
                        }
                    }
                } else {
                    warn!("No source index for component {component} found!");
                }
            } else {
                warn!("No source for package {} of component {component} found!", package.package);
            }
        }
    
        Ok(())
    }
    
    fn check_source_component(&mut self, component: &str) -> Result<()> {
        info!("Checking sources of component {component}...");

        let index = SourceIndex::new(&self.release, component)?;

        info!("Checking sources packages of component {component}...");
        for source in index.packages() {
            debug!("Checking source {source}...");
            // TODO: support for multiple source versions!
            let package = match index.get(&source, None) {
                Some(package) => package,
                None => {
                    let message = format!("Source {} of component {} is missing.", source, component);
                    error!("{}", message);
                    self.issues.push((component.to_string(), Architecture::Source, Error::new(&message, libapt::ErrorType::DownloadFailure)));

                    continue;
                }
            };

            debug!("Checking links of source {source}...");
            for (_key, link) in package.links {
                match get_etag(&link.url) {
                    Ok(_) => {} // pass!
                    Err(e) => {
                        let message = format!("File {} of source {} is broken: {e}", link.url, package.package);
                        error!("{}", message);
                        self.issues.push((component.to_string(), Architecture::Source, Error::new(&message, libapt::ErrorType::DownloadFailure)));
                    }
                }
            }
        }

        self.source_indices.insert(component.to_string(), index);

        Ok(())
    }
}