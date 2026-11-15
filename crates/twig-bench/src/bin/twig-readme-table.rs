//! Generate the README benchmark block exclusively from committed raw runs.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::{
    env, fs,
    path::{Component, Path},
};

const START: &str = "<!-- twig-bench:start -->";
const END: &str = "<!-- twig-bench:end -->";
const METRICS: &[&str] = &[
    "tracked files",
    "worktrees per run",
    "simultaneous creates",
    "timing filesystem",
    "Git create",
    "Twig create",
    "Git first status (median)",
    "Twig first status (median)",
    "Git allocation",
    "Twig allocation",
    "paired timing rounds",
    "Git / Twig failed attempts",
    "status",
];
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    entries: Vec<Entry>,
    notes: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    name: String,
    repository: String,
    timing: String,
    allocation: Option<String>,
    input: Option<String>,
    #[serde(default)]
    incomplete: bool,
}
struct Column {
    heading: String,
    cells: Vec<String>,
}

fn main() -> Result<()> {
    let check = env::args().skip(1).any(|arg| arg == "--check");
    let manifest: Manifest = serde_json::from_slice(&fs::read("bench/results/readme.json")?)?;
    let columns = manifest
        .entries
        .iter()
        .map(column)
        .collect::<Result<Vec<_>>>()?;
    let mut block = String::from(
        "\n\n<!-- Generated from bench/results/readme.json by twig-readme-table. Do not edit numbers. -->\n\n",
    );
    let summary = [
        ("Git time", "Git create"),
        ("Twig time", "Twig create"),
        ("Git space", "Git allocation"),
        ("Twig space", "Twig allocation"),
    ];
    let summary_columns = columns
        .iter()
        .map(|column| Column {
            heading: column.heading.clone(),
            cells: summary
                .iter()
                .map(|(_, source)| {
                    let index = METRICS
                        .iter()
                        .position(|metric| metric == source)
                        .expect("summary metric exists");
                    column.cells[index].clone()
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let labels = summary.iter().map(|(label, _)| *label).collect::<Vec<_>>();
    block.push_str(&render_table(&labels, &summary_columns)?);
    block.push_str("\nTime is batch creation; space is additional APFS allocation after first status. Batch sizes differ by repository; see setup below.\n\n<details>\n<summary>Setup, verification and detailed measurements</summary>\n\n");
    block.push_str(&render_table(METRICS, &columns)?);
    block.push_str("\nCreation is wall time for the entire batch at the recorded concurrency, excluding first status. Values are medians of the recorded timing rounds; one round is an observation, not a stable estimate. Allocation is additional **filesystem-allocated MiB after first status**, including indexes, retained staging and APFS metadata, measured separately on isolated images. Ranges retain observed allocator variation; they are not confidence intervals. Missing measurements are not zeros. Never infer exclusive APFS allocation from logical size or `du`.\n\n");
    for note in &manifest.notes {
        block.push_str(&format!("- {note}\n"));
    }
    block.push_str("\n[Methodology](bench/real-repositories.md) · [raw run index](bench/results/readme.json). First-status latency, p95/p99, errors, source revisions, candidate SHA-256, platform and full samples are in the linked raw runs.\n\n");
    block.push_str("</details>\n\n");
    let readme = fs::read_to_string("README.md")?;
    ensure!(
        readme.matches(START).count() == 1 && readme.matches(END).count() == 1,
        "README needs unique benchmark markers"
    );
    let start = readme.find(START).context("start marker")? + START.len();
    let end = readme.find(END).context("end marker")?;
    ensure!(start <= end, "reversed benchmark markers");
    let generated = format!("{}{}{}", &readme[..start], block, &readme[end..]);
    if check {
        ensure!(
            generated == readme,
            "README benchmark table is stale; run cargo run -p twig-bench --bin twig-readme-table"
        );
    } else {
        fs::write("README.md", generated)?;
    }
    Ok(())
}

fn column(entry: &Entry) -> Result<Column> {
    let timing = load(&entry.timing)?;
    let allocation = entry.allocation.as_deref().map(load).transpose()?;
    if let Some(allocation) = &allocation {
        ensure!(
            timing["candidateSha256"] == allocation["candidateSha256"],
            "different candidate binaries for {}",
            entry.name
        );
        ensure!(
            worktree_count(&timing)? == worktree_count(allocation)?,
            "different worktree counts for {}",
            entry.name
        );
        ensure!(
            timing["concurrency"] == allocation["concurrency"],
            "different concurrency for {}",
            entry.name
        );
    }
    let gt = records(&timing, &entry.repository, "git")?;
    let tt = records(&timing, &entry.repository, "twig")?;
    let ga = allocation
        .as_ref()
        .map(|run| records(run, &entry.repository, "git"))
        .transpose()?
        .unwrap_or_default();
    let ta = allocation
        .as_ref()
        .map(|run| records(run, &entry.repository, "twig"))
        .transpose()?
        .unwrap_or_default();
    let rows: Vec<_> = gt
        .iter()
        .chain(&tt)
        .chain(&ga)
        .chain(&ta)
        .copied()
        .collect();
    if !entry.incomplete {
        ensure!(
            !gt.is_empty() && gt.len() == tt.len(),
            "missing/unpaired completed timing cohorts for {}",
            entry.name
        );
        if allocation.is_some() {
            ensure!(
                !ga.is_empty() && ga.len() == ta.len(),
                "missing/unpaired allocation cohorts for {}",
                entry.name
            );
        }
    }
    validate_rows(&rows, &entry.name)?;
    let expected_count = worktree_count(&timing)?;
    for row in &rows {
        let operations = row["operations"].as_array().context("operations")?;
        ensure!(
            operations.len() as u64 == expected_count,
            "incomplete operation list for {}",
            entry.name
        );
        if timing["concurrency"] == 1 {
            for pair in operations.windows(2) {
                ensure!(
                    pair[0]["createFinishedAtMs"]
                        .as_f64()
                        .context("sequential finish timestamp")?
                        <= pair[1]["createStartedAtMs"]
                            .as_f64()
                            .context("sequential start timestamp")?,
                    "sequential operations overlap"
                );
            }
        }
    }
    let tracked = if let Some(reference) = rows.first() {
        reference["trackedFiles"].to_string()
    } else if let Some(path) = &entry.input {
        read_json(path)?["trackedFiles"]
            .as_u64()
            .context("input file count")?
            .to_string()
    } else {
        "not measured".into()
    };
    let filesystem = match timing["filesystem"].as_str().unwrap_or("isolated-image") {
        "host" => "host APFS",
        "case-sensitive-image" => "case-sensitive APFS image",
        _ => "APFS image",
    };
    let missing = if entry.incomplete {
        "budget stopped"
    } else {
        "not measured"
    };
    let rounds = if entry.incomplete {
        format!("{} (incomplete)", gt.len().min(tt.len()))
    } else {
        gt.len().to_string()
    };
    Ok(Column {
        heading: format!("[{}]({})", entry.name, entry.timing),
        cells: vec![
            tracked,
            expected_count.to_string(),
            timing["concurrency"].to_string(),
            filesystem.into(),
            wall_cell(&gt, missing)?,
            wall_cell(&tt, missing)?,
            first_status_cell(&gt)?,
            first_status_cell(&tt)?,
            allocation_cell(
                &ga,
                if allocation.is_some() {
                    missing
                } else {
                    "not measured"
                },
            )?,
            allocation_cell(
                &ta,
                if allocation.is_some() {
                    missing
                } else {
                    "not measured"
                },
            )?,
            rounds,
            format!("{} / {}", failure_cell(&gt)?, failure_cell(&tt)?),
            if entry.incomplete {
                "budget stopped; unpaired".into()
            } else {
                "completed".into()
            },
        ],
    })
}

fn worktree_count(run: &Value) -> Result<u64> {
    run.get("worktreesPerCohort")
        .unwrap_or(&run["concurrency"])
        .as_u64()
        .context("worktree count")
}

fn validate_rows(rows: &[&Value], name: &str) -> Result<()> {
    let Some(reference) = rows.first() else {
        return Ok(());
    };
    for point in rows {
        ensure!(
            point["sourceTree"] == reference["sourceTree"]
                && point["sourceCommit"] == reference["sourceCommit"]
                && point["trackedFiles"] == reference["trackedFiles"],
            "different input trees for {name}"
        );
        ensure!(point["sourceUnchanged"] == true, "source changed in {name}");
        for op in point["operations"].as_array().context("operations")? {
            ensure!(
                op["exitCode"] != 0 || (op["verified"] == true && op["error"].is_null()),
                "acknowledged checkout failed verification in {name}"
            );
            if point["backend"] == "twig" && op["exitCode"] == 0 {
                ensure!(
                    op["nativeCow"] == true,
                    "Twig delegated instead of cloning in {name}"
                );
            }
        }
    }
    Ok(())
}

fn render_table(metrics: &[&str], columns: &[Column]) -> Result<String> {
    ensure!(
        !columns.is_empty()
            && columns
                .iter()
                .all(|column| column.cells.len() == metrics.len()),
        "invalid table dimensions"
    );
    let mut table = String::from("| metric |");
    for column in columns {
        table.push_str(&format!(" {} |", column.heading));
    }
    table.push_str("\n|---|");
    for _ in columns {
        table.push_str("---|");
    }
    table.push('\n');
    for (index, metric) in metrics.iter().enumerate() {
        table.push_str(&format!("| {metric} |"));
        for column in columns {
            table.push_str(&format!(" {} |", column.cells[index]));
        }
        table.push('\n');
    }
    Ok(table)
}
fn read_json(path: &str) -> Result<Value> {
    ensure!(
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "dataset path must be repository-relative"
    );
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn load(path: &str) -> Result<Value> {
    let run = read_json(path)?;
    ensure!(
        run["suite"] == "real-apfs-cohort"
            && (run["schemaVersion"] == 1 || run["schemaVersion"] == 2),
        "unsupported raw run"
    );
    ensure!(
        run["dirty"] == false && run["failures"].as_array().context("failures")?.is_empty(),
        "dirty or setup-failed run is not publishable"
    );
    Ok(run)
}
fn records<'a>(run: &'a Value, repository: &str, backend: &str) -> Result<Vec<&'a Value>> {
    Ok(run["cohorts"]
        .as_array()
        .context("cohorts")?
        .iter()
        .filter(|c| c["repository"] == repository && c["backend"] == backend)
        .collect())
}
fn wall_cell(rows: &[&Value], missing: &str) -> Result<String> {
    if rows.is_empty() {
        return Ok(missing.into());
    }
    let values = rows
        .iter()
        .map(|row| row["createWallMs"].as_f64().context("create wall time"))
        .collect::<Result<_>>()?;
    Ok(format!("{:.2} s", median(values) / 1000.0))
}
fn first_status_cell(rows: &[&Value]) -> Result<String> {
    let mut values = Vec::new();
    for row in rows {
        values.extend(
            row["operations"]
                .as_array()
                .context("operations")?
                .iter()
                .filter_map(|operation| operation["firstStatusMs"].as_f64()),
        );
    }
    if values.is_empty() {
        return Ok("not measured".into());
    }
    Ok(format!("{:.2} s", median(values) / 1000.0))
}

fn failure_cell(rows: &[&Value]) -> Result<String> {
    if rows.is_empty() {
        return Ok("not measured".into());
    }
    let count: u64 = rows
        .iter()
        .map(|row| row["failed"].as_u64().context("failure count"))
        .sum::<Result<_>>()?;
    Ok(count.to_string())
}
fn allocation_cell(rows: &[&Value], missing: &str) -> Result<String> {
    if rows.is_empty() {
        return Ok(missing.into());
    }
    let mut lows = vec![];
    let mut highs = vec![];
    for row in rows {
        let (before_low, before_high) = bounds(&row["before"])?;
        let (after_low, after_high) = bounds(&row["afterStatus"])?;
        lows.push((after_low - before_high) / 1048576.0);
        highs.push((after_high - before_low) / 1048576.0);
    }
    let low = median(lows);
    let high = median(highs);
    Ok(if high - low < 0.05 {
        format!("{low:.1} MiB")
    } else {
        format!("{low:.1}–{high:.1} MiB")
    })
}
fn bounds(snapshot: &Value) -> Result<(f64, f64)> {
    let point = snapshot["allocatedBytes"]
        .as_f64()
        .context("no isolated allocation snapshot")?;
    Ok(snapshot["observedAllocatedBytes"]
        .as_array()
        .context("allocator observations")?
        .iter()
        .filter_map(Value::as_f64)
        .fold((point, point), |(low, high), value| {
            (low.min(value), high.max(value))
        }))
}
fn median(mut values: Vec<f64>) -> f64 {
    assert!(!values.is_empty(), "median requires samples");
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n.is_multiple_of(2) {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    } else {
        values[n / 2]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn median_uses_both_middle_values_for_even_samples() {
        assert_eq!(median(vec![4.0, 1.0, 3.0, 2.0]), 2.5);
        assert_eq!(median(vec![4.0, 1.0, 3.0]), 3.0);
    }
    #[test]
    fn repositories_are_columns_and_metrics_are_rows() {
        let columns = vec![
            Column {
                heading: "one".into(),
                cells: vec!["1".into(), "2".into()],
            },
            Column {
                heading: "two".into(),
                cells: vec!["3".into(), "4".into()],
            },
        ];
        assert_eq!(
            render_table(&["files", "creates"], &columns).unwrap(),
            "| metric | one | two |\n|---|---|---|\n| files | 1 | 3 |\n| creates | 2 | 4 |\n"
        );
    }
    #[test]
    fn missing_measurements_are_not_zeros() {
        assert_eq!(wall_cell(&[], "budget stopped").unwrap(), "budget stopped");
        assert_eq!(
            allocation_cell(&[], "not measured").unwrap(),
            "not measured"
        );
        assert_eq!(failure_cell(&[]).unwrap(), "not measured");
        assert_eq!(first_status_cell(&[]).unwrap(), "not measured");
    }
}
