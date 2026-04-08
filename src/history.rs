use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::loc::{FileLoc, LocSummary};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotFile {
    pub path: String,
    pub code: usize,
    pub comments: usize,
    pub blanks: usize,
    pub total: usize,
    pub max_complexity: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotData {
    pub version: u32,
    pub created_at: String,
    pub root_path: Option<String>,
    pub summary: LocSummary,
    pub files: Vec<SnapshotFile>,
}

#[derive(Clone, Debug)]
pub struct SnapshotComparison {
    pub added_files: usize,
    pub removed_files: usize,
    pub changed_files: usize,
    pub unchanged_files: usize,
    pub code_delta: isize,
    pub comments_delta: isize,
    pub blanks_delta: isize,
    pub total_delta: isize,
}

pub fn create_snapshot(
    root: Option<&Path>,
    summary: &LocSummary,
    files: &[FileLoc],
) -> SnapshotData {
    let snapshot_files = files
        .iter()
        .map(|file| SnapshotFile {
            path: normalized_key(&file.path, root),
            code: file.code,
            comments: file.comments,
            blanks: file.blanks,
            total: file.total(),
            max_complexity: file.complexity.as_ref().map(|c| c.max_cyclomatic),
        })
        .collect();

    SnapshotData {
        version: 1,
        created_at: chrono::Local::now().to_rfc3339(),
        root_path: root.map(|p| p.to_string_lossy().to_string()),
        summary: summary.clone(),
        files: snapshot_files,
    }
}

pub fn save_snapshot(path: &Path, snapshot: &SnapshotData) -> Result<()> {
    let content = serde_json::to_string_pretty(snapshot)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn load_snapshot(path: &Path) -> Result<SnapshotData> {
    let content = fs::read_to_string(path)?;
    let snapshot: SnapshotData = serde_json::from_str(&content)?;
    Ok(snapshot)
}

pub fn compare_with_snapshot(
    current_root: Option<&Path>,
    current_summary: &LocSummary,
    current_files: &[FileLoc],
    baseline: &SnapshotData,
) -> SnapshotComparison {
    let baseline_map: BTreeMap<String, &SnapshotFile> =
        baseline.files.iter().map(|f| (f.path.clone(), f)).collect();

    let current_map: BTreeMap<String, SnapshotFile> = current_files
        .iter()
        .map(|f| {
            let key = normalized_key(&f.path, current_root);
            (
                key.clone(),
                SnapshotFile {
                    path: key,
                    code: f.code,
                    comments: f.comments,
                    blanks: f.blanks,
                    total: f.total(),
                    max_complexity: f.complexity.as_ref().map(|c| c.max_cyclomatic),
                },
            )
        })
        .collect();

    let mut added = 0usize;
    let mut removed = 0usize;
    let mut changed = 0usize;
    let mut unchanged = 0usize;

    for (path, current) in &current_map {
        if let Some(old) = baseline_map.get(path) {
            if old.code == current.code
                && old.comments == current.comments
                && old.blanks == current.blanks
                && old.max_complexity == current.max_complexity
            {
                unchanged += 1;
            } else {
                changed += 1;
            }
        } else {
            added += 1;
        }
    }

    for path in baseline_map.keys() {
        if !current_map.contains_key(path) {
            removed += 1;
        }
    }

    SnapshotComparison {
        added_files: added,
        removed_files: removed,
        changed_files: changed,
        unchanged_files: unchanged,
        code_delta: current_summary.code as isize - baseline.summary.code as isize,
        comments_delta: current_summary.comments as isize - baseline.summary.comments as isize,
        blanks_delta: current_summary.blanks as isize - baseline.summary.blanks as isize,
        total_delta: current_summary.total() as isize - baseline.summary.total() as isize,
    }
}

fn normalized_key(path: &Path, root: Option<&Path>) -> String {
    if let Some(root) = root {
        let root_canon = root.canonicalize().ok();
        let path_canon = path.canonicalize().ok();
        if let (Some(root_canon), Some(path_canon)) = (root_canon, path_canon)
            && let Ok(relative) = path_canon.strip_prefix(&root_canon)
        {
            return relative.to_string_lossy().replace('\\', "/");
        }
    }

    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loc::FileLoc;
    use std::path::PathBuf;

    #[test]
    fn compare_detects_added_removed_and_changed_files() {
        let old_snapshot = SnapshotData {
            version: 1,
            created_at: "2026-01-01T00:00:00+08:00".to_string(),
            root_path: None,
            summary: LocSummary {
                files: 2,
                code: 10,
                comments: 1,
                blanks: 1,
                complexity: None,
            },
            files: vec![
                SnapshotFile {
                    path: "a.cpp".to_string(),
                    code: 5,
                    comments: 1,
                    blanks: 1,
                    total: 7,
                    max_complexity: Some(3),
                },
                SnapshotFile {
                    path: "b.cpp".to_string(),
                    code: 5,
                    comments: 0,
                    blanks: 0,
                    total: 5,
                    max_complexity: Some(2),
                },
            ],
        };

        let current_files = vec![
            FileLoc {
                path: PathBuf::from("a.cpp"),
                code: 6,
                comments: 1,
                blanks: 1,
                complexity: None,
            },
            FileLoc {
                path: PathBuf::from("c.cpp"),
                code: 2,
                comments: 0,
                blanks: 0,
                complexity: None,
            },
        ];

        let current_summary = LocSummary::from_files(&current_files);
        let comparison =
            compare_with_snapshot(None, &current_summary, &current_files, &old_snapshot);

        assert_eq!(comparison.added_files, 1);
        assert_eq!(comparison.removed_files, 1);
        assert_eq!(comparison.changed_files, 1);
        assert_eq!(comparison.unchanged_files, 0);
        assert_eq!(comparison.code_delta, -2);
    }
}
