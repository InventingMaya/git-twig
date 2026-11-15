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
    allocation: String,
}
fn main() -> Result<()> {
    let check = env::args().skip(1).any(|arg| arg == "--check");
    let manifest: Manifest = serde_json::from_slice(&fs::read("bench/results/readme.json")?)?;
    let mut block = String::from(
        "\n\n<!-- Generated from bench/results/readme.json by twig-readme-table. Do not edit numbers. -->\n\n| repository | tracked files | parallel creates | timing filesystem | Git create | Twig create | Git allocation | Twig allocation | paired timing rounds | Git / Twig failed attempts |\n|---|---:|---:|---|---:|---:|---:|---:|---:|---:|\n",
    );
    for entry in &manifest.entries {
        let timing = load(&entry.timing)?;
        let allocation = load(&entry.allocation)?;
        ensure!(
            timing["candidateSha256"] == allocation["candidateSha256"],
            "different candidate binaries for {}",
            entry.name
        );
        ensure!(
            timing["concurrency"] == allocation["concurrency"],
            "different concurrency for {}",
            entry.name
        );
        let gt = records(&timing, &entry.repository, "git")?;
        let tt = records(&timing, &entry.repository, "twig")?;
        let ga = records(&allocation, &entry.repository, "git")?;
        let ta = records(&allocation, &entry.repository, "twig")?;
        let reference = gt[0];
        for point in gt.iter().chain(&tt).chain(&ga).chain(&ta) {
            ensure!(
                point["sourceTree"] == reference["sourceTree"]
                    && point["sourceCommit"] == reference["sourceCommit"]
                    && point["trackedFiles"] == reference["trackedFiles"],
                "different input trees for {}",
                entry.name
            );
            ensure!(
                point["sourceUnchanged"] == true,
                "source changed in {}",
                entry.name
            );
            for op in point["operations"].as_array().context("operations")? {
                ensure!(
                    op["exitCode"] != 0 || (op["verified"] == true && op["error"].is_null()),
                    "acknowledged checkout failed verification in {}",
                    entry.name
                );
                if point["backend"] == "twig" && op["exitCode"] == 0 {
                    ensure!(
                        op["nativeCow"] == true,
                        "Twig delegated instead of cloning in {}",
                        entry.name
                    );
                }
            }
        }
        ensure!(
            gt.len() == tt.len() && ga.len() == ta.len(),
            "unpaired timing/allocation rounds for {}",
            entry.name
        );
        let filesystem = match timing["filesystem"].as_str().unwrap_or("isolated-image") {
            "host" => "host APFS",
            "case-sensitive-image" => "case-sensitive APFS image",
            _ => "APFS image",
        };
        block.push_str(&format!(
            "| [{}]({}) | {} | {} | {} | {:.2} s | {:.2} s | {} | {} | {} | {} / {} |\n",
            entry.name,
            entry.timing,
            reference["trackedFiles"],
            timing["concurrency"],
            filesystem,
            wall(&gt)?,
            wall(&tt)?,
            allocation_mib(&ga)?,
            allocation_mib(&ta)?,
            gt.len(),
            failed(&gt)?,
            failed(&tt)?
        ));
    }
    block.push_str("\nCreation is wall time for the entire concurrent cohort, excluding first status. Values are medians of the recorded timing rounds; one round is an observation, not a stable estimate. Allocation is additional **filesystem-allocated MiB after first status**, including indexes, retained staging and APFS metadata, measured separately on isolated images. Ranges retain observed allocator variation; they are not confidence intervals. Never infer exclusive APFS allocation from logical size or `du`.\n\n");
    for note in &manifest.notes {
        block.push_str(&format!("- {note}\n"));
    }
    block.push_str("\n[Methodology](bench/real-repositories.md) · [raw run index](bench/results/readme.json). First-status latency, p95/p99, errors, source revisions, candidate SHA-256, platform and full samples are in the linked raw runs.\n\n");
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
fn load(path: &str) -> Result<Value> {
    ensure!(
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "dataset path must be repository-relative"
    );
    let run: Value = serde_json::from_slice(&fs::read(path)?)?;
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
    let rows: Vec<_> = run["cohorts"]
        .as_array()
        .context("cohorts")?
        .iter()
        .filter(|c| c["repository"] == repository && c["backend"] == backend)
        .collect();
    ensure!(
        !rows.is_empty(),
        "missing completed cohort: {repository}/{backend}"
    );
    Ok(rows)
}
fn wall(rows: &[&Value]) -> Result<f64> {
    Ok(median(
        rows.iter()
            .map(|r| r["createWallMs"].as_f64().context("create wall time"))
            .collect::<Result<_>>()?,
    ) / 1000.0)
}
fn failed(rows: &[&Value]) -> Result<u64> {
    rows.iter()
        .map(|r| r["failed"].as_u64().context("failure count"))
        .sum()
}
fn allocation_mib(rows: &[&Value]) -> Result<String> {
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
}
