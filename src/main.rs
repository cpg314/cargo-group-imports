use std::collections::HashSet;
use std::path::PathBuf;

use clap::Parser;
use log::*;
use rayon::prelude::*;

use cargo_group_imports::*;

fn main_impl() -> anyhow::Result<()> {
    let MainFlags::GroupImports(args) = MainFlags::parse();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .write_style(args.write_style())
        .init();
    let start = std::time::Instant::now();

    // Retrieve workspace packages
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path("Cargo.toml")
        .current_dir(&args.workspace)
        .exec()?;
    let workspace_packages: HashSet<String> = metadata
        .workspace_packages()
        .into_iter()
        .map(|p| p.name.replace('-', "_"))
        .collect();
    debug!("Workspace packages: {:?}", workspace_packages);

    let output = metadata
        .workspace_packages()
        .into_par_iter()
        // Process each package
        .flat_map(|package| {
            info!("Processing {}", package.name);
            let root = package.manifest_path.parent().unwrap();

            // Process each file in the package
            // Only look in src so that the root crate does not pick workspace member sources.
            let mut files: Vec<PathBuf> = walkdir::WalkDir::new(root.join("src"))
                .min_depth(1)
                .into_iter()
                // Exclude target/ for the root package
                .filter_entry(|e| !e.path().join("CACHEDIR.TAG").exists())
                .filter_map(|e| e.ok())
                // Rust source files
                .filter(|f| {
                    f.file_type().is_file() && f.path().extension().map_or(false, |e| e == "rs")
                })
                .map(|f| f.path().to_owned())
                .collect();
            // build.rs
            let build = root.join("build.rs");
            if build.is_file() {
                files.push(build.into());
            }
            files
                .into_par_iter()
                .map(|f| process_file(&f, &package.name, &workspace_packages, &args))
        })
        .collect::<Result<Vec<_>, _>>()?;
    info!(
        "Processed {} files in {:?}, {} changed",
        output.len(),
        start.elapsed(),
        output.iter().filter(|x| **x).count()
    );
    if !args.fix && output.iter().any(|x| *x) {
        warn!("Not all files are formatted. Rerun with --fix to attempt to fix the issues");
        std::process::exit(1);
    }

    Ok(())
}

fn main() {
    if let Err(e) = main_impl() {
        error!("{}", e);
        std::process::exit(2);
    }
}
