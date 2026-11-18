//! A byte ceiling at the SQLite WAL write boundary for the derived search writer.
//! The platform VFS still owns file access, SQLite locking and all sync behavior.
use rusqlite::{Connection, OpenFlags, ffi};
use std::{
    ffi::{c_char, c_int, c_void},
    mem::{align_of, size_of},
    path::Path,
    ptr::{self, NonNull},
    sync::OnceLock,
};

pub(super) const LIMIT: i64 = 64 * 1024 * 1024;
pub(super) const CHECKPOINT_AT: u64 = 48 * 1024 * 1024;
const NAME: &std::ffi::CStr = c"calternal-search-wal";
static BASE: OnceLock<Base> = OnceLock::new();

struct Base(NonNull<ffi::sqlite3_vfs>);
// SQLite's platform VFS is process-lived. We only read its file size and
// open callback. Registration never changes either field.
unsafe impl Sync for Base {}
unsafe impl Send for Base {}

#[repr(C)]
struct WalMethods {
    capped: ffi::sqlite3_io_methods,
    base: *const ffi::sqlite3_io_methods,
}

pub(super) fn open(path: &Path) -> rusqlite::Result<Connection> {
    static REGISTERED: OnceLock<c_int> = OnceLock::new();
    let result = *REGISTERED.get_or_init(register);
    if result != ffi::SQLITE_OK {
        return Err(rusqlite::Error::SqliteFailure(
            ffi::Error::new(result),
            None,
        ));
    }
    Connection::open_with_flags_and_vfs(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        NAME.to_str().expect("ASCII VFS name"),
    )
}

fn register() -> c_int {
    // SQLite initializes the default VFS on lookup. Allocate one process-lived
    // registration. Leave the default and its application data unchanged.
    unsafe {
        let Some(platform) = NonNull::new(ffi::sqlite3_vfs_find(ptr::null())) else {
            return ffi::SQLITE_ERROR;
        };
        let _ = BASE.set(Base(platform));
        let mut vfs = Box::new(*platform.as_ptr());
        vfs.zName = NAME.as_ptr();
        vfs.pNext = ptr::null_mut();
        vfs.szOsFile = (tail_offset(vfs.szOsFile) + size_of::<WalMethods>()) as c_int;
        vfs.xOpen = Some(open_file);
        let result = ffi::sqlite3_vfs_register(&mut *vfs, 0);
        if result == ffi::SQLITE_OK {
            let _ = Box::leak(vfs);
        }
        result
    }
}

fn tail_offset(size: c_int) -> usize {
    (size as usize).next_multiple_of(align_of::<WalMethods>())
}

unsafe extern "C" fn open_file(
    _: *mut ffi::sqlite3_vfs,
    name: *const c_char,
    file: *mut ffi::sqlite3_file,
    flags: c_int,
    out: *mut c_int,
) -> c_int {
    // SQLite allocates the aligned platform file plus our method-table tail.
    // The platform only initializes its own prefix. It is the sole owner of
    // the file handle, and the method tail lives exactly as long as that file.
    unsafe {
        let base = BASE.get().expect("registered platform VFS").0.as_ptr();
        let result = ((*base).xOpen.expect("platform xOpen"))(base, name, file, flags, out);
        if result != ffi::SQLITE_OK || flags & ffi::SQLITE_OPEN_WAL == 0 {
            return result;
        }
        let original = (*file).pMethods;
        let tail = file
            .cast::<u8>()
            .add(tail_offset((*base).szOsFile))
            .cast::<WalMethods>();
        let mut capped = *original;
        capped.xClose = Some(close);
        capped.xWrite = Some(write);
        capped.xTruncate = Some(truncate);
        capped.xFileControl = Some(control);
        tail.write(WalMethods {
            capped,
            base: original,
        });
        (*file).pMethods = tail.cast::<ffi::sqlite3_io_methods>();
        result
    }
}

unsafe fn original(file: *mut ffi::sqlite3_file) -> *const ffi::sqlite3_io_methods {
    // Only WAL files given our method table can call these trampolines.
    unsafe { (*(*file).pMethods.cast::<WalMethods>()).base }
}

unsafe extern "C" fn close(file: *mut ffi::sqlite3_file) -> c_int {
    unsafe {
        let base = original(file);
        (*file).pMethods = base;
        ((*base).xClose.expect("platform xClose"))(file)
    }
}

unsafe extern "C" fn write(
    file: *mut ffi::sqlite3_file,
    data: *const c_void,
    len: c_int,
    offset: i64,
) -> c_int {
    if offset < 0
        || len < 0
        || offset
            .checked_add(i64::from(len))
            .is_none_or(|end| end > LIMIT)
    {
        return ffi::SQLITE_FULL;
    }
    unsafe { ((*original(file)).xWrite.expect("platform xWrite"))(file, data, len, offset) }
}

unsafe extern "C" fn truncate(file: *mut ffi::sqlite3_file, size: i64) -> c_int {
    if size > LIMIT {
        return ffi::SQLITE_FULL;
    }
    unsafe { ((*original(file)).xTruncate.expect("platform xTruncate"))(file, size) }
}

unsafe extern "C" fn control(file: *mut ffi::sqlite3_file, op: c_int, arg: *mut c_void) -> c_int {
    // Optional preallocation and chunk rounding could otherwise grow a WAL
    // beyond the byte range SQLite supplies to xWrite.
    if matches!(
        op,
        ffi::SQLITE_FCNTL_SIZE_HINT | ffi::SQLITE_FCNTL_CHUNK_SIZE
    ) {
        return ffi::SQLITE_NOTFOUND;
    }
    unsafe {
        ((*original(file))
            .xFileControl
            .expect("platform xFileControl"))(file, op, arg)
    }
}
