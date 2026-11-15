//! Compare one completed old/new cohort using original versioned measurements.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
};
fn read(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn number(value: &Value, key: &str) -> Result<f64> {
    value[key]
        .as_f64()
        .with_context(|| format!("missing {key}"))
}
fn single<'a>(run: &'a Value, backend: &str) -> Result<&'a Value> {
    ensure!(
        run["failures"].as_array().context("failures")?.is_empty(),
        "suite failed"
    );
    let rows = run["cohorts"]
        .as_array()
        .context("cohorts")?
        .iter()
        .filter(|c| c["backend"] == backend)
        .collect::<Vec<_>>();
    ensure!(
        rows.len() == 1,
        "comparison requires one cohort per backend"
    );
    let row = rows[0];
    let ops = row["operations"].as_array().context("operations")?;
    let count = run["worktreesPerCohort"].as_u64().context("count")?;
    ensure!(
        count > 0
            && ops.len() as u64 == count
            && row["succeeded"] == count
            && row["failed"] == 0
            && row["sourceUnchanged"] == true,
        "incomplete or incorrect cohort"
    );
    ensure!(
        ops.iter().all(|o| o["verified"] == true
            && o["exitCode"] == 0
            && o["error"].is_null()
            && (backend != "twig" || o["nativeCow"] == true)),
        "invalid operation"
    );
    ensure!(
        row["verifiedFiles"].as_u64()
            == Some(count * row["trackedFiles"].as_u64().context("trackedFiles")?),
        "incomplete content verification"
    );
    if run["concurrency"] == 1 {
        for pair in ops.windows(2) {
            ensure!(
                number(&pair[0], "createFinishedAtMs")? <= number(&pair[1], "createStartedAtMs")?,
                "overlapping sequential creates"
            );
        }
    }
    Ok(row)
}
fn bounds(snapshot: &Value) -> Result<(f64, f64)> {
    let point = number(snapshot, "allocatedBytes")?;
    let observations = snapshot["observedAllocatedBytes"]
        .as_array()
        .context("allocator observations")?;
    observations.iter().try_fold((point, point), |(lo, hi), v| {
        let n = v.as_f64().context("allocator observation")?;
        Ok((lo.min(n), hi.max(n)))
    })
}
fn allocation(row: &Value) -> Result<(f64, f64)> {
    let (before_lo, before_hi) = bounds(&row["before"])?;
    let (after_lo, after_hi) = bounds(&row["afterStatus"])?;
    Ok((
        (after_lo - before_hi) / 1048576.0,
        (after_hi - before_lo) / 1048576.0,
    ))
}
fn filename(path: &Path) -> Result<&str> {
    path.file_name()
        .context("filename")?
        .to_str()
        .context("UTF-8 filename")
}
fn range(lo: f64, hi: f64) -> String {
    if hi - lo < 0.05 {
        format!("{lo:.1}")
    } else {
        format!("{lo:.1}–{hi:.1}")
    }
}
fn main() -> Result<()> {
    let args = env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    ensure!(
        args.len() == 3,
        "usage: twig-real-compare old.json new.json output.json"
    );
    let old = read(&args[0])?;
    let new = read(&args[1])?;
    for key in [
        "schemaVersion",
        "filesystem",
        "imageSizeGiB",
        "warmup",
        "concurrency",
        "worktreesPerCohort",
        "repeats",
        "platform",
        "cpu",
        "gitVersion",
    ] {
        ensure!(old[key] == new[key], "different {key}");
    }
    ensure!(
        old["dirty"] == false && new["dirty"] == false,
        "dirty suite provenance"
    );
    let git = single(&new, "git")?;
    let before = single(&old, "twig")?;
    let after = single(&new, "twig")?;
    for row in [before, after] {
        for key in [
            "sourceCommit",
            "sourceTree",
            "trackedFiles",
            "logicalBytesPerWorktree",
        ] {
            ensure!(row[key] == git[key], "different {key}");
        }
    }
    let mut rows = Vec::new();
    for (label, row, run) in [
        ("Git", git, &new),
        ("Twig per-file", before, &old),
        ("Twig directory", after, &new),
    ] {
        let (lo, hi) = allocation(row)?;
        rows.push(json!({"label":label,"candidateSha256":run["candidateSha256"],"createWallMs":number(row,"createWallMs")?,"allocationLowMiB":lo,"allocationHighMiB":hi,"verifiedFiles":row["verifiedFiles"]}));
    }
    let result = json!({"schemaVersion":1,"sources":[filename(&args[0])?,filename(&args[1])?],"configuration":{"worktreesPerCohort":new["worktreesPerCohort"],"concurrency":new["concurrency"],"filesystem":new["filesystem"],"repeats":new["repeats"]},"notes":["One uninstrumented local round per variant on a shared Mac; sequential order and host activity can affect timings. Not a stable improvement estimate.","Creation excludes first status. Space is additional isolated filesystem allocation after first status, including indexes/staging/metadata; bounds retain observed allocator variation."],"rows":rows});
    fs::write(&args[2], serde_json::to_vec_pretty(&result)?)?;
    let max_time = rows
        .iter()
        .map(|r| r["createWallMs"].as_f64().unwrap())
        .fold(1.0, f64::max)
        * 1.1;
    let min_space = rows
        .iter()
        .map(|r| r["allocationLowMiB"].as_f64().unwrap())
        .fold(0.0, f64::min);
    let max_space = rows
        .iter()
        .map(|r| r["allocationHighMiB"].as_f64().unwrap())
        .fold(1.0, f64::max)
        * 1.1;
    let zero = 650.0 - min_space / (max_space - min_space) * 300.0;
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1150\" height=\"350\"><rect width=\"100%\" height=\"100%\" fill=\"#111827\"/><g fill=\"#e5e7eb\" font-family=\"system-ui,sans-serif\"><text x=\"20\" y=\"30\" font-size=\"19\">Linux kernel · ten sequential creates · optimized releases</text><text x=\"20\" y=\"55\" font-size=\"12\">One local round; timings exclude first status; space is additional APFS allocation after status.</text><text x=\"200\" y=\"85\">creation wall time (seconds)</text><text x=\"650\" y=\"85\">allocated growth (MiB)</text>",
    );
    let mut table = String::from(
        "<!-- Generated by twig-real-compare from versioned raw runs; do not edit numbers. -->\n\nTen sequential creates per variant, maximum one in flight; one local round, not a stable estimate. Creation excludes first status; additional allocation is measured after first status.\n\n| implementation | creation s | additional allocation MiB |\n|---|---:|---:|\n",
    );
    for (index, row) in rows.iter().enumerate() {
        let y = 110 + index * 70;
        let color = ["#94a3b8", "#60a5fa", "#34d399"][index];
        let time = number(row, "createWallMs")?;
        let lo = number(row, "allocationLowMiB")?;
        let hi = number(row, "allocationHighMiB")?;
        let end = 650.0 + (hi - min_space) / (max_space - min_space) * 300.0;
        svg.push_str(&format!("<text x=\"20\" y=\"{}\">{}</text><rect x=\"200\" y=\"{y}\" width=\"{:.2}\" height=\"28\" fill=\"{color}\"/><text x=\"515\" y=\"{}\">{:.3} s</text><rect x=\"{:.2}\" y=\"{y}\" width=\"{:.2}\" height=\"28\" fill=\"{color}\"/><text x=\"960\" y=\"{}\">{} MiB</text>",y+20,row["label"].as_str().unwrap(),time/max_time*300.0,y+20,time/1000.0,zero.min(end),(end-zero).abs(),y+20,range(lo,hi)));
        table.push_str(&format!(
            "| {} | {:.3} | {} |\n",
            row["label"].as_str().unwrap(),
            time / 1000.0,
            range(lo, hi)
        ));
    }
    svg.push_str("</g></svg>\n");
    fs::write(args[2].with_extension("svg"), svg)?;
    fs::write(args[2].with_extension("summary.md"), table)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_negative_allocator_deltas() {
        let row = json!({"before":{"allocatedBytes":100,"observedAllocatedBytes":[100,110]},"afterStatus":{"allocatedBytes":90,"observedAllocatedBytes":[80,90]}});
        let (lo, hi) = allocation(&row).unwrap();
        assert_eq!(lo, -30.0 / 1048576.0);
        assert_eq!(hi, -10.0 / 1048576.0);
    }
    #[test]
    fn rejects_incomplete_suites() {
        assert!(single(&json!({"failures":[],"cohorts":[]}), "twig").is_err());
    }
}
