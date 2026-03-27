use std::{fs, path::PathBuf};

use anyhow::Result;
use mosaic_config::redact_mosaic_config;
use mosaic_inspect::RunTrace;

pub(crate) fn inspect_cmd(file: PathBuf, verbose: bool, json: bool) -> Result<()> {
    let content = fs::read_to_string(&file)?;
    let trace: RunTrace = serde_json::from_str(&content)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&trace)?);
        return Ok(());
    }

    let workspace = crate::load_config()
        .ok()
        .map(|loaded| redact_mosaic_config(&loaded.config));
    println!(
        "{}",
        crate::output::render_inspect_report(&trace, workspace.as_ref(), verbose)?
    );

    if !verbose {
        let mut next_steps = vec![format!("mosaic inspect {} --verbose", file.display())];
        if trace.status() == "failed" {
            next_steps.push(format!("mosaic gateway incident {}", trace.run_id));
        }
        crate::print_next_steps(next_steps);
    }

    Ok(())
}
