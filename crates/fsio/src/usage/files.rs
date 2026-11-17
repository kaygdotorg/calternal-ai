//! Exact file sizes, grouped by parent directory to share path prefixes.
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

type Entries = HashMap<Box<OsStr>, u64>;

#[derive(Default)]
pub(super) struct Files {
    folders: HashMap<Box<Path>, Entries>,
}

impl Files {
    pub(super) fn insert_in(&mut self, parent: &Path, name: &OsStr, bytes: u64) -> Option<u64> {
        if !self.folders.contains_key(parent) {
            self.folders.insert(parent.into(), Entries::new());
        }
        self.folders
            .get_mut(parent)
            .unwrap()
            .insert(name.into(), bytes)
    }

    pub(super) fn insert(&mut self, path: &Path, bytes: u64) -> Option<u64> {
        self.insert_in(path.parent()?, path.file_name()?, bytes)
    }

    pub(super) fn get(&self, path: &Path) -> Option<u64> {
        self.folders
            .get(path.parent()?)?
            .get(path.file_name()?)
            .copied()
    }

    pub(super) fn remove(&mut self, path: &Path) -> Option<u64> {
        let parent = path.parent()?;
        let entries = self.folders.get_mut(parent)?;
        let old = entries.remove(path.file_name()?);
        if entries.is_empty() {
            self.folders.remove(parent);
        }
        old
    }

    pub(super) fn totals(&self, person: impl Fn(&Path) -> Option<String>) -> HashMap<String, u64> {
        let mut totals = HashMap::new();
        for (parent, entries) in &self.folders {
            if let Some(owner) = person(parent) {
                *totals.entry(owner).or_default() += entries.values().sum::<u64>();
            } else {
                // Preserve accounting for files directly under the users root.
                for (name, bytes) in entries {
                    if let Some(owner) = person(&parent.join(Path::new(name))) {
                        *totals.entry(owner).or_default() += bytes;
                    }
                }
            }
        }
        totals
    }

    /// Full paths are needed only when moving a directory's children.
    pub(super) fn under(&self, prefix: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for (parent, entries) in &self.folders {
            if parent.starts_with(prefix) {
                paths.extend(entries.keys().map(|name| parent.join(Path::new(name))));
            } else if prefix.parent() == Some(parent.as_ref())
                && prefix
                    .file_name()
                    .is_some_and(|name| entries.contains_key(name))
            {
                paths.push(prefix.to_owned());
            }
        }
        paths
    }

    #[cfg(test)]
    fn owned_bytes(&self) -> usize {
        // Conservative table estimate, plus exact boxed key payloads.
        fn table<K, V>(map: &HashMap<K, V>) -> usize {
            if map.capacity() == 0 {
                return 0;
            }
            map.capacity().next_power_of_two() * (std::mem::size_of::<(K, V)>() + 1)
        }
        table(&self.folders)
            + self
                .folders
                .iter()
                .map(|(parent, entries)| {
                    parent.as_os_str().len()
                        + table(entries)
                        + entries.keys().map(|n| n.len()).sum::<usize>()
                })
                .sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_keys_fit_a_small_per_file_budget() {
        let mut files = Files::default();
        let parent = PathBuf::from(format!("/{}users/person/Notes", "long-root/".repeat(20)));
        for n in 0..10_000 {
            files.insert_in(&parent, OsStr::new(&format!("note-{n:05}.md")), n);
        }
        assert!(
            files.owned_bytes() < 100 * 10_000,
            "snapshot retains {} bytes for 10000 short filenames",
            files.owned_bytes()
        );
    }

    #[test]
    fn exact_paths_and_sizes_match_the_flat_map() {
        let mut compact = Files::default();
        let mut flat = HashMap::new();
        let mut rng = 17u64;
        for step in 0..2000 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let person = if rng & 8 == 0 { "a" } else { "ab" };
            let path = PathBuf::from(format!(
                "/homes/{person}/Notes/f{}/n{}.md",
                rng % 7,
                rng % 89
            ));
            match rng % 3 {
                0 => assert_eq!(compact.remove(&path), flat.remove(&path)),
                _ => assert_eq!(compact.insert(&path, rng), flat.insert(path.clone(), rng)),
            }
            assert_eq!(compact.get(&path), flat.get(&path).copied());
            if step % 50 == 0 {
                for prefix in ["/homes/a", "/homes/ab/Notes", "/homes"] {
                    let mut got = compact.under(Path::new(prefix));
                    let mut want = flat
                        .keys()
                        .filter(|p| p.starts_with(prefix))
                        .cloned()
                        .collect::<Vec<_>>();
                    got.sort();
                    want.sort();
                    assert_eq!(got, want);
                }
            }
        }
    }

    #[test]
    fn identical_names_in_different_folders_keep_separate_totals() {
        let mut files = Files::default();
        files.insert(Path::new("/homes/a/Notes/n.md"), 3);
        files.insert(Path::new("/homes/a/Trash/n.md"), 5);
        files.insert(Path::new("/homes/b/Notes/n.md"), 7);
        let totals = files.totals(|p| {
            Some(
                p.strip_prefix("/homes")
                    .ok()?
                    .components()
                    .next()?
                    .as_os_str()
                    .to_str()?
                    .to_owned(),
            )
        });
        assert_eq!(totals.get("a"), Some(&8));
        assert_eq!(totals.get("b"), Some(&7));
        assert_eq!(files.remove(Path::new("/homes/a/Trash/n.md")), Some(5));
        assert_eq!(files.get(Path::new("/homes/a/Notes/n.md")), Some(3));
        assert!(files.under(Path::new("/homes/a/Trash")).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn filenames_keep_their_exact_os_bytes() {
        use std::os::unix::ffi::OsStrExt;
        let parent = Path::new("/homes/a/Notes");
        let name = OsStr::from_bytes(b"note-\xff.md");
        let mut files = Files::default();
        files.insert_in(parent, name, 9);
        assert_eq!(files.get(&parent.join(Path::new(name))), Some(9));
        assert_eq!(files.under(parent), vec![parent.join(Path::new(name))]);
    }
}
