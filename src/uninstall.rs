use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};

use crate::config::Config;

pub fn run(purge_config: bool, yes: bool) -> Result<()> {
    let current_exe = env::current_exe().context("failed to locate current executable")?;
    let manifest = install_manifest_path();
    let manifest_binary = read_manifest_binary(&manifest)?;
    let binary = manifest_binary.unwrap_or(current_exe);

    validate_binary_path(&binary)?;

    println!("Cinto uninstall");
    println!("binary: {}", binary.display());
    if purge_config {
        if let Some(config_dir) = config_dir() {
            println!("config: {}", config_dir.display());
        }
    } else {
        println!("config: preserved (pass --purge-config to remove it)");
    }

    if !yes && !confirm("Remove these files? [y/N] ")? {
        println!("uninstall cancelled");
        return Ok(());
    }

    if binary.exists() {
        fs::remove_file(&binary)
            .with_context(|| format!("failed to remove {}", binary.display()))?;
        println!("removed {}", binary.display());
    } else {
        println!("binary was already absent: {}", binary.display());
    }

    if manifest.exists() {
        fs::remove_file(&manifest)
            .with_context(|| format!("failed to remove {}", manifest.display()))?;
    }

    if purge_config
        && let Some(config_dir) = config_dir()
        && config_dir.exists()
    {
        fs::remove_dir_all(&config_dir)
            .with_context(|| format!("failed to remove {}", config_dir.display()))?;
        println!("removed {}", config_dir.display());
    }

    println!("cinto uninstalled");
    Ok(())
}

fn validate_binary_path(path: &Path) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("could not determine binary file name"))?;
    if matches!(name, "cinto" | "cinto.exe") {
        Ok(())
    } else {
        Err(anyhow!(
            "refusing to remove a binary not named cinto or cinto.exe: {}",
            path.display()
        ))
    }
}

fn read_manifest_binary(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }

    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    for line in contents.lines() {
        let Some(value) = line.strip_prefix("binary=") else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        return Ok(Some(PathBuf::from(value)));
    }

    Ok(None)
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt}");
    io::stdout().flush().context("failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read confirmation")?;
    Ok(matches!(input.trim(), "y" | "Y" | "yes" | "YES"))
}

fn install_manifest_path() -> PathBuf {
    data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("install.toml")
}

fn config_dir() -> Option<PathBuf> {
    Config::default_path().and_then(|path| path.parent().map(Path::to_path_buf))
}

fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("cinto"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_only_cinto_binary_names() {
        assert!(validate_binary_path(Path::new("/tmp/cinto")).is_ok());
        assert!(validate_binary_path(Path::new("/tmp/cinto.exe")).is_ok());
        assert!(validate_binary_path(Path::new("/tmp/other")).is_err());
    }
}
