use anyhow::Result;

use crate::config::Config;
use crate::crp;

pub fn run(config: &Config) -> Result<()> {
    let workspace = &config.harness.workspace;
    let templates_dir = crp::workspace_template_dir(workspace);
    
    crp::TemplateSet::dump_builtins(&templates_dir)?;
    
    println!("Successfully initialized Cinto templates in {}", templates_dir.display());
    println!("You can now edit the .toml files to customize the CRP prompts for this workspace.");
    
    Ok(())
}
