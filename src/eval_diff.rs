use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct BatchOutputs {
    parsed_successfully: bool,
    required_slots_present: bool,
    filepaths_valid: bool,
    code_compiles: Option<bool>,
    tests_pass: Option<bool>,
    evaluator_score: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BatchMetrics {
    duration_ms: u128,
}

#[derive(Debug, Deserialize)]
struct BatchResult {
    task_id: String,
    outputs: BatchOutputs,
    metrics: BatchMetrics,
}

pub fn run(base_path: PathBuf, compare_path: PathBuf) -> Result<()> {
    let base_results = load_results(&base_path)?;
    let compare_results = load_results(&compare_path)?;

    let mut regressions = Vec::new();
    let mut improvements = Vec::new();
    let mut unchanged_pass = 0;
    let mut unchanged_fail = 0;

    let mut total_duration_base = 0;
    let mut total_duration_compare = 0;
    let mut common_tasks = 0;

    for (task_id, base) in &base_results {
        if let Some(compare) = compare_results.get(task_id) {
            common_tasks += 1;
            total_duration_base += base.metrics.duration_ms;
            total_duration_compare += compare.metrics.duration_ms;

            let base_pass = is_success(base);
            let compare_pass = is_success(compare);

            if base_pass && !compare_pass {
                regressions.push(task_id.clone());
            } else if !base_pass && compare_pass {
                improvements.push(task_id.clone());
            } else if base_pass && compare_pass {
                unchanged_pass += 1;
            } else {
                unchanged_fail += 1;
            }
        }
    }

    println!("========================================");
    println!("          EVALUATION DIFF RUN           ");
    println!("========================================");
    println!("Base run: {}", base_path.display());
    println!("Compare run: {}", compare_path.display());
    println!("Common tasks evaluated: {}", common_tasks);
    println!("----------------------------------------");

    println!("SUMMARY:");
    println!("  Regressions (Pass -> Fail): {}", regressions.len());
    println!("  Improvements (Fail -> Pass): {}", improvements.len());
    println!("  Unchanged (Pass): {}", unchanged_pass);
    println!("  Unchanged (Fail): {}", unchanged_fail);
    println!("----------------------------------------");

    if !regressions.is_empty() {
        println!("REGRESSIONS:");
        for task_id in &regressions {
            println!("  - {}", task_id);
        }
        println!("----------------------------------------");
    }

    if !improvements.is_empty() {
        println!("IMPROVEMENTS:");
        for task_id in &improvements {
            println!("  - {}", task_id);
        }
        println!("----------------------------------------");
    }

    if common_tasks > 0 {
        let avg_base = total_duration_base / common_tasks as u128;
        let avg_compare = total_duration_compare / common_tasks as u128;
        let diff = avg_compare as f64 - avg_base as f64;
        let diff_percent = (diff / avg_base as f64) * 100.0;
        
        println!("METRICS (Averages for common tasks):");
        println!("  Duration Base: {} ms", avg_base);
        println!("  Duration Compare: {} ms", avg_compare);
        println!("  Diff: {:.2} ms ({:+.2}%)", diff, diff_percent);
    }

    Ok(())
}

fn load_results(path: &PathBuf) -> Result<HashMap<String, BatchResult>> {
    let file = File::open(path).with_context(|| format!("failed to open file {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut map = HashMap::new();

    for (idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let result: BatchResult = serde_json::from_str(&line)
            .with_context(|| format!("failed to parse JSON at {}:{}", path.display(), idx + 1))?;
        map.insert(result.task_id.clone(), result);
    }
    Ok(map)
}

fn is_success(res: &BatchResult) -> bool {
    // A task is considered successful if:
    // 1. It parsed successfully
    // 2. All required slots are present
    // 3. Filepaths are valid
    // 4. (If checked) Code compiles
    // 5. (If checked) Tests pass
    // 6. (If checked) Evaluator score contains PASS
    if !res.outputs.parsed_successfully
        || !res.outputs.required_slots_present
        || !res.outputs.filepaths_valid
    {
        return false;
    }

    if let Some(compiles) = res.outputs.code_compiles {
        if !compiles {
            return false;
        }
    }

    if let Some(tests) = res.outputs.tests_pass {
        if !tests {
            return false;
        }
    }

    if let Some(score) = &res.outputs.evaluator_score {
        if !score.contains("PASS") {
            return false;
        }
    }

    true
}
