//! Implementation of apt repo check.

use std::collections::HashMap;

use libapt::{Architecture, Error, PackageIndex, PackageVersion, Release, Result, SourceIndex, VersionRelation, get_etag};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};


/// AptCheck groups all metadata and apt repository check results.
#[derive(Debug, Deserialize, Serialize)]
pub struct AptCheck {
    // Release to check.
    release: Release,
    // Components to check.
    components: Vec<String>,
    // Architectures to check.
    architectures: Vec<Architecture>,
    // Parsed binary indices.
    binary_indices: Vec<PackageIndex>,
    // Parsed source indices. (Component, Index)
    source_indices: HashMap<String, SourceIndex>,
    // List of found issues. (Component, Architecture, found Issue)
    issues: Vec<(String, Architecture, Error)>,
    // (Component, Package, Dependency)
    missing_packages: Vec<(String, String, PackageVersion)>,
    // (Component, Package, Source)
    missing_sources: Vec<(String, String, String)>,
    // Check existence of referenced files
    check_files: bool
}

impl AptCheck {
    /// Initialize the AptCheck structure.
    pub fn new(release: Release, components: Vec<String>, architectures: Vec<String>, check_files: bool) -> Result<AptCheck> {
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
            check_files: check_files,
        })
    }

    /// Execute the apt repository check.
    /// 
    /// Returns true if no issues were found, false else.
    /// In case of major issues the error is provided as result.
    pub async fn check_repo(&mut self) -> Result<bool> {

        info!("Checking compliance of InRelease file...");
        match self.release.check_compliance() {
            Ok(_) => info!("InRelease complies to Debian policy."),
            Err(e) => warn!("InRelease does not comply to Debian policy: {e}"),
        }
    
        // Run check focussing on one component.
        info!("Checking single components...");
        self.check().await?;

        // Run checks requiring more components, e.g. availability of dependencies.
        info!("Checking cross components...");
        self.cross_check()?;
        
        // Log results
        // TODO: better report!
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
    
        // TODO: fix check and consider package metadata issues
        Ok(self.issues.is_empty() && self.missing_packages.is_empty() && self.missing_sources.is_empty())
    }
    
    /// Do checks involving multiple components.
    fn cross_check(&mut self) -> Result<()> {
        // TODO: search for missing sources and packages in other components.
        Ok(())
    }

    // Do checks for a single component.
    async fn check(&mut self) -> Result<()> {
        // Check sources for all component.
        // This also initializes the source package index 
        // which is required to check the availability of 
        // source packages.
        for component in &self.components.clone() {
            match self.check_source_component(component).await {
                Ok(_) => {},
                Err(e) => {
                    let message = format!("Checking sources of component {component} failed: {e}");
                    error!("{}", message);
                    self.issues.push((component.clone(), Architecture::Source, Error::new(&message, libapt::ErrorType::Download)));
                }
            }
        }

        // Check the binary indices for all architectures and components.
        for component in &self.components.clone() {
            for architecture in &self.architectures.clone() {
                if architecture == &Architecture::Source {
                    continue;
                }

                match self.check_binary_component(component, architecture).await {
                    Ok(_) => {},
                    Err(e) => {
                        let message = format!("Checking component {component} for architecture {architecture} failed: {e}");
                        error!("{}", message);
                        self.issues.push((component.clone(), architecture.clone(), Error::new(&message, libapt::ErrorType::Download)));
                    }
                }
            }
        }
    
        Ok(())
    }
    
    async fn check_binary_component(&mut self, component: &str, architecture: &Architecture) -> Result<()> {
        info!("Checking binary index of component {component} for architecture {architecture}...");
        let index = PackageIndex::new(&self.release, component, architecture).await?;

        for package in index.packages() {
            debug!("Checking binary package {package}...");
            // TODO: support for multiple source versions!
            let package = match index.get(&package, None) {
                Some(package) => package,
                None => {
                    let message = format!("Package {} of component {} and architecture {} is missing.", package, component, architecture);
                    error!("{}", message);
                    self.issues.push((component.to_string(), architecture.clone(), Error::new(&message, libapt::ErrorType::Download)));

                    continue;
                }
            };

            if self.check_files {
                debug!("Checking file of binary package {}...", package.package);
                // Check existence of linked deb file.
                match get_etag(&package.link.url).await {
                    Ok(_) => {} // pass!
                    Err(e) => {
                        let message = format!("File {} of source {} is broken: {e}", &package.link.url, package.package);
                        error!("{}", message);
                        self.issues.push((component.to_string(), Architecture::Source, Error::new(&message, libapt::ErrorType::Download)));
                    }
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
    
    async fn check_source_component(&mut self, component: &str) -> Result<()> {
        info!("Checking sources of component {component}...");

        let index = SourceIndex::new(&self.release, component).await?;

        info!("Checking sources packages of component {component}...");
        for source in index.packages() {
            debug!("Checking source {source}...");
            // TODO: support for multiple source versions!
            let package = match index.get(&source, None) {
                Some(package) => package,
                None => {
                    let message = format!("Source {} of component {} is missing.", source, component);
                    error!("{}", message);
                    self.issues.push((component.to_string(), Architecture::Source, Error::new(&message, libapt::ErrorType::Download)));

                    continue;
                }
            };

            if self.check_files {
                debug!("Checking links of source {source}...");
                for (_key, link) in package.links {
                    match get_etag(&link.url).await {
                        Ok(_) => {} // pass!
                        Err(e) => {
                            let message = format!("File {} of source {} is broken: {e}", link.url, package.package);
                            error!("{}", message);
                            self.issues.push((component.to_string(), Architecture::Source, Error::new(&message, libapt::ErrorType::Download)));
                        }
                    }
                }
            }
        }

        self.source_indices.insert(component.to_string(), index);

        Ok(())
    }
}