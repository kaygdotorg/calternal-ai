//! File publication and rename guarantees on the host filesystem, including case aliases.
use super::*;
use std::collections::HashSet;

/// Adds a process-local sequence so concurrent fixtures never share a coarse clock tick.
pub(super) fn tmp() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "calternal-fsio-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn durable_directory_creation_syncs_each_new_parent_in_order() {
    let root = tmp();
    let target = root.join("one/two/three");
    let mut synced = Vec::new();

    create_dir_all_durable_with(&target, &mut |parent| {
        synced.push(parent.to_path_buf());
        Ok(())
    })
    .unwrap();

    assert_eq!(
        synced,
        vec![root.clone(), root.join("one"), root.join("one/two")]
    );
    assert!(target.is_dir());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn atomic_write_replaces_and_leaves_no_temp_files() {
    let dir = tmp();
    let f = dir.join("Note.md");
    write_atomic(&f, b"one").unwrap();
    write_atomic(&f, b"two").unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), b"two");
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec!["Note.md"]);
    std::fs::remove_dir_all(dir).unwrap();
}

/// A private atomic write must leave the published key accessible only to its owner.
#[cfg(unix)]
#[test]
fn atomic_write_mode_keeps_private_files_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tmp();
    let path = dir.join("token.key");
    write_atomic_mode(&path, b"secret", 0o600).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"secret");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_write_keeps_the_old_file() {
    let dir = tmp();
    let f = dir.join("Note.md");
    write_atomic(&f, b"old").unwrap();
    // Writing into a missing folder fails before touching the target.
    assert!(write_atomic(&dir.join("missing/Note.md"), b"new").is_err());
    assert_eq!(std::fs::read(&f).unwrap(), b"old");
    std::fs::remove_dir_all(dir).unwrap();
}

/// #298: a big write goes out in steps and arrives whole, from bytes or
/// from a stream, with an odd-sized last step.
#[test]
fn big_writes_arrive_whole() {
    let dir = tmp();
    let len = paced::STEP * 3 + 12345;
    let bytes: Vec<u8> = (0..len).map(|i| (i * 31 % 251) as u8).collect();
    let a = dir.join("a.bin");
    write_atomic(&a, &bytes).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), bytes);
    let b = dir.join("b.bin");
    let n = write_atomic_from(&b, &mut &bytes[..]).unwrap();
    assert_eq!(n, len as u64);
    assert_eq!(std::fs::read(&b).unwrap(), bytes);
    let empty = dir.join("empty.bin");
    assert_eq!(write_atomic_from(&empty, &mut &[][..]).unwrap(), 0);
    assert_eq!(std::fs::read(&empty).unwrap(), Vec::<u8>::new());
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
    assert_eq!(left.len(), 3, "no temporary files left");
}

/// #222: a write never replaces a version someone else wrote since the
/// caller last looked; that version stays and comes back to the caller.
#[test]
fn write_unless_changed_keeps_someone_elses_version() {
    let dir = tmp();
    let f = dir.join("Note.md");
    // No file yet: written.
    assert_eq!(
        write_unless_changed(&f, b"ours 1", |_| false).unwrap(),
        None
    );
    assert_eq!(std::fs::read(&f).unwrap(), b"ours 1");
    // Still what we last wrote: written.
    assert_eq!(
        write_unless_changed(&f, b"ours 2", |old| old == b"ours 1").unwrap(),
        None
    );
    assert_eq!(std::fs::read(&f).unwrap(), b"ours 2");
    // Another app wrote it meanwhile: kept, and handed back.
    write_atomic(&f, b"theirs").unwrap();
    let found = write_unless_changed(&f, b"ours 3", |old| old == b"ours 2").unwrap();
    assert_eq!(found.as_deref(), Some(&b"theirs"[..]));
    assert_eq!(std::fs::read(&f).unwrap(), b"theirs");
    // Another app renames its version in right after the check: swapped
    // straight back, and handed back.
    let once = std::cell::Cell::new(true);
    let found = write_unless_changed(&f, b"ours 4", |old| {
        if once.replace(false) {
            write_atomic(&f, b"theirs 2").unwrap();
        }
        old == b"theirs"
    })
    .unwrap();
    if cfg!(any(target_os = "linux", target_vendor = "apple")) {
        assert_eq!(found.as_deref(), Some(&b"theirs 2"[..]));
        assert_eq!(std::fs::read(&f).unwrap(), b"theirs 2");
    }
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec!["Note.md"], "no temporary files left");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn only_one_process_owns_a_data_folder() {
    let dir = tmp();
    let first = DataLock::acquire(&dir).unwrap();
    // A second handle in the same process is refused too (flock is per
    // open file description).
    let second = DataLock::acquire(&dir);
    assert!(second.is_err(), "second lock must fail");
    drop(first);
    assert!(DataLock::acquire(&dir).is_ok(), "lock frees on drop");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn case_only_rename_works_and_other_clashes_are_refused() {
    let dir = tmp();
    write_atomic(&dir.join("note.md"), b"a").unwrap();
    rename_safely(&dir.join("note.md"), &dir.join("Note.md")).unwrap();
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, vec!["Note.md"]);
    write_atomic(&dir.join("Other.md"), b"b").unwrap();
    assert!(rename_safely(&dir.join("Other.md"), &dir.join("Note.md")).is_err());
    assert_eq!(std::fs::read(dir.join("Note.md")).unwrap(), b"a");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn names_fold_case_and_unicode_form() {
    let nfd = "Cafe\u{301}"; // e + combining acute
    let nfc = "Caf\u{e9}";
    assert_ne!(nfd, nfc);
    assert_eq!(names::nfc(nfd), nfc);
    assert_eq!(names::fold(nfd), names::fold("CAF\u{c9}"));
    assert_eq!(names::stem_from_title("  ../a/b:c\u{7}  "), "-a-b-c");
    assert_eq!(names::stem_from_title("   "), "Untitled");
    assert_eq!(names::stem_from_title(nfd), nfc);
}

#[test]
fn unique_names_skip_case_and_form_clashes() {
    let taken: HashSet<String> = ["note.md", "note 2.md"]
        .iter()
        .map(|s| names::fold(s))
        .collect();
    let name = names::unique_md_name("Note", |k| taken.contains(k));
    assert_eq!(name, "Note 3.md");
    assert_eq!(
        names::unique_md_name("Fresh", |k| taken.contains(k)),
        "Fresh.md"
    );
}

/// #298: a prepared write that is never placed leaves the file alone and
/// no temporary file behind; placed, it lands like a whole write.
#[test]
fn prepared_writes_land_only_when_placed() {
    let dir = tmp();
    let f = dir.join("Note.md");
    write_atomic(&f, b"old").unwrap();
    drop(Prepared::new(&f, b"never").unwrap());
    assert_eq!(std::fs::read(&f).unwrap(), b"old");
    let p = Prepared::new(&f, b"new").unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), b"old", "not before place");
    assert_eq!(p.place(|old| old == b"old").unwrap(), None);
    assert_eq!(std::fs::read(&f).unwrap(), b"new");
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
    assert_eq!(left.len(), 1, "no temporary files left");
}

/// The review's room-rename race, scheduled at the shared placement helper.
#[test]
fn delta_rename_must_preserve_file_arriving_after_absence_check() {
    let dir = tmp();
    let from = dir.join("Rename source.md");
    let to = dir.join("Delta rename.md");
    std::fs::write(&from, b"app body\n").unwrap();
    let outside = b"outside rename destination must survive\n";
    let result = rename_with_hook(&from, &to, || {
        std::fs::write(&to, outside).unwrap();
    });
    assert_eq!(std::fs::read(&to).unwrap(), outside);
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&from).unwrap(), b"app body\n");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn case_only_rename_preserves_a_different_destination() {
    let dir = tmp();
    let from = dir.join("note.md");
    let to = dir.join("Note.md");
    std::fs::write(&from, b"ours").unwrap();
    if !to.exists() {
        std::fs::write(&to, b"outside").unwrap();
        assert_eq!(
            rename_safely(&from, &to).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(&from).unwrap(), b"ours");
        assert_eq!(std::fs::read(&to).unwrap(), b"outside");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn rename_preserves_a_dangling_destination_symlink() {
    let dir = tmp();
    let from = dir.join("From.md");
    let to = dir.join("To.md");
    std::fs::write(&from, b"ours").unwrap();
    std::os::unix::fs::symlink("missing.md", &to).unwrap();
    assert_eq!(
        rename_safely(&from, &to).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(std::fs::read_link(&to).unwrap(), Path::new("missing.md"));
    assert_eq!(std::fs::read(&from).unwrap(), b"ours");
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
/// A native atomic case rename succeeds; staging fallback must preserve racing outside bytes.
#[test]
fn case_only_rename_is_atomic_or_preserves_staged_collisions() {
    for block_rollback in [false, true] {
        let dir = tmp();
        let from = dir.join("note.md");
        let to = dir.join("Note.md");
        std::fs::write(&from, b"original").unwrap();
        // On a case-sensitive volume, a second hard link gives both spellings
        // the same file identity and exercises the case-only staging path.
        let case_insensitive = to.exists();
        if !case_insensitive {
            std::fs::hard_link(&from, &to).unwrap();
        }
        let staged = std::cell::Cell::new(false);
        let result = rename_with_hooks(
            &from,
            &to,
            || {},
            || {
                staged.set(true);
                if to.symlink_metadata().is_ok() {
                    std::fs::remove_file(&to).unwrap();
                }
                std::fs::write(&to, b"outside destination").unwrap();
                if block_rollback && !case_insensitive {
                    std::fs::write(&from, b"outside source").unwrap();
                }
            },
        );
        match result {
            Ok(()) => {
                // APFS can perform the case-only rename atomically with RENAME_EXCL,
                // so the fallback's race hook must never run on this path.
                assert!(case_insensitive);
                assert!(!staged.get());
                assert_eq!(std::fs::read(&to).unwrap(), b"original");
                assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
            }
            Err(error) => {
                assert!(staged.get());
                let expected_kind = if block_rollback || case_insensitive {
                    io::ErrorKind::Other
                } else {
                    io::ErrorKind::AlreadyExists
                };
                assert_eq!(error.kind(), expected_kind);
                assert_eq!(std::fs::read(&to).unwrap(), b"outside destination");
                if block_rollback || case_insensitive {
                    let outside: &[u8] = if case_insensitive {
                        b"outside destination"
                    } else {
                        b"outside source"
                    };
                    assert_eq!(std::fs::read(&from).unwrap(), outside);
                    let saved = std::fs::read_dir(&dir)
                        .unwrap()
                        .flatten()
                        .find(|e| e.file_name().to_string_lossy().starts_with(TEMP_PREFIX))
                        .unwrap()
                        .path();
                    assert_eq!(std::fs::read(&saved).unwrap(), b"original");
                    assert!(error.to_string().contains(saved.to_str().unwrap()));
                } else {
                    assert_eq!(std::fs::read(&from).unwrap(), b"original");
                    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
                }
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn many_new_files_never_replace_anything() {
    let dir = tmp();
    let taken = dir.join("sub/b.md");
    std::fs::create_dir_all(taken.parent().unwrap()).unwrap();
    std::fs::write(&taken, "someone else").unwrap();
    let files = vec![
        (dir.join("sub/a.md"), b"a".to_vec()),
        (taken.clone(), b"mine".to_vec()),
    ];
    let placed = crate::write_new_many(&files).unwrap();
    assert!(placed[0].is_ok());
    assert_eq!(
        placed[1].as_ref().unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(std::fs::read(dir.join("sub/a.md")).unwrap(), b"a");
    assert_eq!(std::fs::read(&taken).unwrap(), b"someone else");
    let leftovers = std::fs::read_dir(dir.join("sub"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(crate::TEMP_PREFIX)
        })
        .count();
    assert_eq!(leftovers, 0, "no temporary file is left behind");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn many_files_are_replaced_whole() {
    let dir = tmp();
    let a = dir.join("raw/a.vcf");
    crate::replace_many(&[(a.clone(), b"one".to_vec())]).unwrap();
    crate::replace_many(&[(a.clone(), b"two".to_vec())]).unwrap();
    assert_eq!(std::fs::read(&a).unwrap(), b"two");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dropping_data_lock_releases_it_even_with_an_inherited_file_handle() {
    // A concurrent process spawn can inherit the open file description
    // between fork and exec. A duplicate models that interval deterministically.
    let dir = tmp();
    let guard = crate::DataLock::acquire(&dir).unwrap();
    let inherited = guard._file.try_clone().unwrap();
    drop(guard);
    let next = crate::DataLock::acquire_existing(&dir)
        .expect("the guard's lifetime, not an inherited handle, owns the lock");
    assert!(crate::DataLock::acquire_existing(&dir).is_err());
    drop(inherited);
    assert!(crate::DataLock::acquire_existing(&dir).is_err());
    drop(next);
    assert!(crate::DataLock::acquire_existing(&dir).is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn staged_bulk_preserves_occupied_files_and_falls_back_when_staging_is_missing() {
    let dir = tmp();
    let notes = dir.join("Notes");
    std::fs::create_dir_all(&notes).unwrap();
    let occupied = notes.join("Occupied.md");
    std::fs::write(&occupied, b"outside text").unwrap();
    let fresh = notes.join("Fresh.md");
    let files = vec![
        (occupied.clone(), b"replacement".to_vec()),
        (fresh.clone(), b"whole note".to_vec()),
    ];
    let placed = crate::write_new_many_observed(&files, Some(&dir), |_| {}).unwrap();
    assert_eq!(
        placed[0].as_ref().unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert!(placed[1].is_ok());
    assert_eq!(std::fs::read(occupied).unwrap(), b"outside text");
    assert_eq!(std::fs::read(fresh).unwrap(), b"whole note");
    let fallback = notes.join("Fallback.md");
    crate::write_new_many_observed(
        &[(fallback.clone(), b"fallback".to_vec())],
        Some(&dir.join("missing")),
        |_| {},
    )
    .unwrap()
    .remove(0)
    .unwrap();
    assert_eq!(std::fs::read(fallback).unwrap(), b"fallback");
    #[cfg(unix)]
    {
        let link = notes.join("Link.md");
        std::os::unix::fs::symlink("missing-target", &link).unwrap();
        let result = crate::write_new_many_observed(
            &[(link.clone(), b"replacement".to_vec())],
            Some(&dir),
            |_| {},
        )
        .unwrap();
        assert_eq!(
            result[0].as_ref().unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            std::fs::read_link(link).unwrap(),
            std::path::PathBuf::from("missing-target")
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn named_bulk_staging_is_whole_exclusive_and_leaves_no_temporary_name() {
    let dir = tmp();
    let notes = dir.join("Notes");
    std::fs::create_dir_all(&notes).unwrap();
    let mut stage = crate::BulkStage::open(&dir).unwrap();
    stage.anonymous = false; // Exercise the older-filesystem fallback.
    let path = notes.join("Contact.md");
    assert!(stage.place(&path, b"whole contact").unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), b"whole contact");
    assert_eq!(
        stage.place(&path, b"replacement").unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    assert_eq!(std::fs::read(path).unwrap(), b"whole contact");
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        1,
        "only Notes remains"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn staged_bulk_keeps_the_destination_default_acl() {
    use rustix::fs::{XattrFlags, setxattr};
    use std::os::unix::fs::PermissionsExt;
    let dir = tmp();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let acl = |group: u16, other: u16| {
        let mut bytes = 2u32.to_le_bytes().to_vec();
        for (tag, permissions) in [(1u16, 7u16), (4, group), (32, other)] {
            bytes.extend(tag.to_le_bytes());
            bytes.extend(permissions.to_le_bytes());
            bytes.extend(u32::MAX.to_le_bytes());
        }
        bytes
    };
    match setxattr(
        &dir,
        "system.posix_acl_default",
        &acl(7, 7),
        XattrFlags::empty(),
    ) {
        Ok(()) => {}
        Err(rustix::io::Errno::OPNOTSUPP | rustix::io::Errno::NOSYS) => {
            std::fs::remove_dir_all(dir).unwrap();
            return;
        }
        Err(e) => panic!("could not set the fixture ACL: {e}"),
    }
    let notes = dir.join("Notes");
    std::fs::create_dir(&notes).unwrap();
    setxattr(
        &notes,
        "system.posix_acl_default",
        &acl(0, 0),
        XattrFlags::empty(),
    )
    .unwrap();
    let different_policy = vec![(notes.join("NewContacts/Person.md"), b"private".to_vec())];
    assert!(
        crate::prepare_new_directory(different_policy, &dir.join("image-cache"), &"d".repeat(64))
            .unwrap()
            .is_none()
    );
    assert!(!notes.join("NewContacts").exists());
    let path = notes.join("Private.md");
    crate::write_new_many_observed(
        &[(path.clone(), b"private contact".to_vec())],
        Some(&dir),
        |_| {},
    )
    .unwrap()
    .remove(0)
    .unwrap();
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600,
        "staging must not grant permissions excluded by the destination ACL"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn parallel_bulk_preserves_result_slots_and_observes_the_final_sync() {
    let dir = tmp();
    let notes = dir.join("Notes");
    std::fs::create_dir(&notes).unwrap();
    let files: Vec<_> = (0..260)
        .map(|i| {
            (
                notes.join(format!("{i}.md")),
                format!("whole {i}").into_bytes(),
            )
        })
        .collect();
    for i in [0, 64, 129, 259] {
        std::fs::write(&files[i].0, "outside").unwrap();
    }
    let mut observed = Vec::new();
    let placed =
        crate::write_new_many_observed(&files, Some(&dir), |done| observed.push(done)).unwrap();
    assert_eq!(observed, (1..=261).collect::<Vec<_>>());
    for (i, ((path, bytes), result)) in files.iter().zip(placed).enumerate() {
        if [0, 64, 129, 259].contains(&i) {
            assert_eq!(
                result.unwrap_err().kind(),
                std::io::ErrorKind::AlreadyExists
            );
            assert_eq!(std::fs::read(path).unwrap(), b"outside");
        } else {
            result.unwrap();
            assert_eq!(&std::fs::read(path).unwrap(), bytes);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn a_private_directory_is_invisible_until_exclusive_publication() {
    let dir = tmp();
    let notes = dir.join("Notes");
    std::fs::create_dir(&notes).unwrap();
    let cache = dir.join("import-cache");
    let target = notes.join("Contacts");
    let files: Vec<_> = (0..150)
        .map(|i| {
            (
                target.join(format!("{i}.md")),
                format!("whole {i}").into_bytes(),
            )
        })
        .collect();
    let image = crate::prepare_new_directory(files.clone(), &cache, &"a".repeat(64))
        .unwrap()
        .unwrap();
    assert!(image.matches(&files));
    assert!(!target.exists());
    assert_eq!(
        crate::clean_prepared_directories(&cache).unwrap(),
        0,
        "an active image is locked"
    );
    image
        .publish(|| {
            assert_eq!(std::fs::read_dir(&target).unwrap().count(), files.len());
            for (path, bytes) in &files {
                assert_eq!(&std::fs::read(path).unwrap(), bytes);
            }
        })
        .unwrap()
        .synced
        .unwrap();
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 150);
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn private_directory_publication_preserves_a_destination_that_arrives() {
    let dir = tmp();
    let notes = dir.join("Notes");
    std::fs::create_dir(&notes).unwrap();
    let cache = dir.join("import-cache");
    let target = notes.join("Contacts");
    let files = vec![(target.join("New.md"), b"new".to_vec())];
    let image = crate::prepare_new_directory(files, &cache, &"b".repeat(64))
        .unwrap()
        .unwrap();
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("Outside.md"), "outside").unwrap();
    let error = image
        .publish(|| panic!("an occupied destination must not publish"))
        .err()
        .unwrap();
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(
        std::fs::read(target.join("Outside.md")).unwrap(),
        b"outside"
    );
    assert!(!target.join("New.md").exists());
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn a_published_directory_retains_ownership_when_a_parent_flush_fails() {
    let dir = tmp();
    let notes = dir.join("Notes");
    std::fs::create_dir(&notes).unwrap();
    let cache = dir.join("import-cache");
    let target = notes.join("Contacts");
    let files = vec![(target.join("New.md"), b"whole".to_vec())];
    let image = crate::prepare_new_directory(files, &cache, &"c".repeat(64))
        .unwrap()
        .unwrap();
    let own = std::fs::read_dir(&cache)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let published = image
        .publish(|| std::fs::remove_dir_all(&own).unwrap())
        .unwrap();
    assert!(
        published.synced.is_err(),
        "the source parent disappeared before its flush"
    );
    assert_eq!(
        std::fs::read(target.join("New.md")).unwrap(),
        b"whole",
        "the caller still owns this published file and must undo it"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn directory_publication_rechecks_changed_inheritance_policy() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tmp();
    let notes = dir.join("Notes");
    std::fs::create_dir(&notes).unwrap();
    let cache = dir.join("import-cache");
    let target = notes.join("Contacts");
    let files = vec![(target.join("New.md"), b"whole".to_vec())];
    let image = crate::prepare_new_directory(files, &cache, &"d".repeat(64))
        .unwrap()
        .unwrap();
    let mode = std::fs::metadata(&notes).unwrap().permissions().mode();
    std::fs::set_permissions(&notes, std::fs::Permissions::from_mode(mode ^ 0o2000)).unwrap();
    let error = image
        .publish(|| panic!("changed policy must use ordinary writes"))
        .err()
        .unwrap();
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    assert!(!target.exists());
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn conditional_removal_keeps_changed_bytes_and_outside_replacements() {
    let dir = tmp();
    let path = dir.join("Note.md");
    std::fs::write(&path, b"later acknowledged work").unwrap();
    assert!(!remove_unless_changed(&path, |b| b == b"seed").unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), b"later acknowledged work");
    let replaced = path.clone();
    assert!(
        !remove_unless_changed(&path, move |_| {
            // The old inode is already held privately; an outside writer creates
            // a replacement at the public name at the destructive boundary.
            std::fs::write(&replaced, b"outside replacement").unwrap();
            true
        })
        .unwrap()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"outside replacement");
    assert!(remove_unless_changed(&path, |b| b == b"outside replacement").unwrap());
    assert!(!path.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn conditional_removal_recovers_the_captured_file_after_restart() {
    let dir = tmp();
    let path = dir.join("Note.md");
    let held = dir.join(format!("{TEMP_PREFIX}delete-Note.md"));
    std::fs::write(&held, b"outside edit captured before crash").unwrap();
    assert!(!remove_unless_changed(&path, |b| b == b"seed").unwrap());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"outside edit captured before crash"
    );
    assert!(!held.exists());
    std::fs::rename(&path, &held).unwrap();
    assert!(remove_unless_changed(&path, |_| true).unwrap());
    assert!(!path.exists());
    assert!(!held.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
#[test]
fn descriptor_lock_rejects_symlinks_and_keeps_outside_bytes() {
    use rustix::fs::{Mode, OFlags};
    let dir = tmp();
    let outside = dir.join("outside");
    std::fs::write(&outside, b"outside lock target").unwrap();
    std::os::unix::fs::symlink(&outside, dir.join(".lock")).unwrap();
    let fd = rustix::fs::open(&dir, OFlags::RDONLY | OFlags::DIRECTORY, Mode::empty()).unwrap();
    assert!(DataLock::acquire_at(&fd, &dir).is_err());
    assert_eq!(std::fs::read(outside).unwrap(), b"outside lock target");
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
#[test]
fn descriptor_lock_cannot_be_redirected_by_root_path_substitution() {
    use rustix::fs::{Mode, OFlags};
    let dir = tmp();
    let root = dir.join("root");
    let outside = dir.join("outside");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let fd = rustix::fs::open(&root, OFlags::RDONLY | OFlags::DIRECTORY, Mode::empty()).unwrap();
    let moved = dir.join("moved");
    std::fs::rename(&root, &moved).unwrap();
    std::os::unix::fs::symlink(&outside, &root).unwrap();
    let guard = DataLock::acquire_at(&fd, &root).unwrap();
    assert!(!outside.join(".lock").exists());
    assert!(DataLock::acquire_existing(&moved).is_err());
    drop(guard);
    assert!(DataLock::acquire_existing(&moved).is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}
