//! Explicit ("runtime") loading of the Python shared library.
//!
//! This module is only compiled when the `dynamic-loading` feature is enabled. It
//! provides the machinery which lets `pyo3-ffi` (and therefore `pyo3`) build
//! **without linking to libpython at build time**, resolving every Python C API
//! symbol from a shared library which is opened by the application at runtime.
//!
//! The design follows <https://github.com/PyO3/pyo3/issues/2668>:
//!
//! * Every Python function declared through `extern_libpython!` becomes a thin
//!   wrapper which looks the symbol up on first use and caches the resulting
//!   function pointer.
//! * Every Python data symbol becomes an accessor function with the same name
//!   (e.g. `ffi::PyDict_Type()` returns `*mut PyTypeObject` and
//!   `ffi::PyExc_ValueError()` returns `*mut PyObject`).
//!
//! On Windows the library is opened with `LoadLibraryExW` + `GetProcAddress`, on
//! other platforms with `dlopen(RTLD_NOW | RTLD_GLOBAL)` + `dlsym`.
//!
//! # Choosing the library
//!
//! The library is discovered lazily the first time a symbol is needed, in this
//! order:
//!
//! 1. An explicit call to [`load`] (best control, e.g. after downloading a
//!    Python runtime).
//! 2. The `PYO3_DYNAMIC_LIBRARY` environment variable, naming the shared library
//!    itself (e.g. `C:\...\python312.dll` or `libpython3.12.so.1.0`).
//! 3. The `PYO3_PYTHON` environment variable, naming a Python interpreter: the
//!    shared library next to it, and then the shared library this crate was
//!    configured with (which the loader's own search path resolves, `/usr/lib`
//!    for instance).
//! 4. `PYO3_DYNAMIC_LIBRARY_NAME`, the library this crate was configured with
//!    (set by the build script).
//! 5. The best `python3*.dll` on `PATH` (Windows) or the usual `libpython3*.so`
//!    names (other platforms) - only for a build which does not pin a version,
//!    which in practice means an `abi3` build.
//!
//! When more than one of them can be loaded, the earlier entry wins: an
//! interpreter named by `PYO3_PYTHON` takes precedence over the library this
//! crate was built against, and both take precedence over the platform's search
//! path.
//!
//! The first two name a specific library, so they never silently fall back to a
//! different one: a path which cannot be loaded is an error, and [`loaded_path`]
//! reports the file which was really used. The one exception is Windows reusing
//! a module whose file name is already loaded in the process - for example the
//! interpreter hosting a Python extension module - where the real path of that
//! module is reported.
//!
//! A candidate is only accepted when it really is the Python library this build
//! can use, so that an unrelated file - or a Python of the wrong version, which
//! the compiler's assumptions about layouts and structures would not match - is
//! reported instead of being loaded:
//!
//! * the module has to export the Python C API (`Py_Initialize`, which every
//!   implementation provides), so an executable or an unrelated library is
//!   rejected - on Windows `LoadLibraryExW` will happily open an `.exe`;
//! * the file name has to encode the same variant (debug, free-threaded) and,
//!   unless the build is an `abi3` build, the same version as the configuration.
//!
//! A library which is already loaded is never replaced: symbols which have
//! already been resolved keep pointing into it. What a candidate list is for is
//! finding the library before anything has been resolved.
//!
//! The loader needs the operating system's API (`LoadLibraryExW`, `dlopen`,
//! `std::sync::Mutex`, `std::fs`), so it only exists where `std` does - which is
//! also why `String`, `Vec` and `format!` come from `std` here.
#![allow(clippy::std_instead_of_alloc)]

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};
use std::ffi::OsStr;
use std::path::Path;
use std::string::{String, ToString};
#[allow(
    clippy::disallowed_types,
    reason = "only compiled where std exists; a blocking mutex is what the loader needs"
)]
use std::sync::Mutex;
use std::vec;
use std::vec::Vec;

/// Failure to open the Python shared library.
#[derive(Debug, Clone)]
pub struct Error {
    message: String,
}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Human readable description of what went wrong.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.message)
    }
}

impl core::error::Error for Error {}

/// The environment variable naming the shared library to load.
pub const LIBRARY_ENV_VAR: &str = "PYO3_DYNAMIC_LIBRARY";
/// The environment variable naming a Python interpreter whose shared library is loaded.
pub const PYTHON_ENV_VAR: &str = "PYO3_PYTHON";

/// A symbol every Python shared library exports, used to check that a module
/// which could be opened really is a Python shared library.
const SENTINEL_SYMBOL: &str = "Py_Initialize";

/// The library this crate was configured against, as named by the build script.
const CONFIGURED_LIBRARY_NAME: Option<&str> = option_env!("PYO3_DYNAMIC_LIBRARY_NAME");

mod sys {
    use super::Error;
    use core::ffi::c_void;
    use std::string::String;

    /// An opened shared library.
    #[derive(Debug)]
    pub struct Handle {
        raw: *mut c_void,
    }

    // SAFETY: a loaded library handle is immutable and process-wide, and it is
    // only ever passed to the thread-safe lookups below.
    unsafe impl Send for Handle {}
    // SAFETY: as above.
    unsafe impl Sync for Handle {}

    impl Handle {
        pub unsafe fn open(path: &str) -> Result<Self, Error> {
            imp::open(path)
        }

        pub unsafe fn symbol(&self, name: &str) -> Option<*mut c_void> {
            imp::symbol(self.raw, name)
        }

        /// Whether `path` names this very module.
        ///
        /// Used to accept a request which names a library the process already
        /// has - the file name of the interpreter hosting a Python extension
        /// module, for instance - without loading anything.
        pub fn is_loaded_module(&self, path: &str) -> bool {
            imp::is_loaded_module(self.raw, path)
        }

        /// The path of the file the library was loaded from.
        ///
        /// The loader may have resolved a file name itself, or reused a module
        /// which was already loaded, so the path reported to the user is the one
        /// which was actually used rather than the one which was requested.
        pub fn resolved_path(&self, fallback: &str) -> String {
            imp::resolved_path(self.raw, fallback)
        }
    }

    #[cfg(unix)]
    mod imp {
        use super::*;
        use core::ffi::c_int;
        use std::string::{String, ToString};

        pub fn is_loaded_module(raw: *mut c_void, path: &str) -> bool {
            let Ok(c_path) = std::ffi::CString::new(path) else {
                return false;
            };
            // `RTLD_NOLOAD` only reports a library which is already in the
            // process; nothing is loaded by this call.
            // SAFETY: `c_path` is a valid NUL terminated string, and the handle,
            // when it is one, is released again below.
            let found =
                unsafe { libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_NOLOAD) };
            if found.is_null() {
                return false;
            }
            if found == raw {
                true
            } else {
                // A different loaded library answers to this name: release the
                // reference `dlopen` took for the question, and let the caller
                // treat it as something else.
                // SAFETY: `found` is a handle returned by `dlopen`.
                unsafe { libc::dlclose(found) };
                false
            }
        }

        pub unsafe fn open(path: &str) -> Result<Handle, Error> {
            let c_path = std::ffi::CString::new(path)
                .map_err(|_| Error::new("library path contains a NUL byte"))?;
            // RTLD_NOW: resolve everything up front so that a broken library is
            // reported here rather than at the first API call.
            // RTLD_GLOBAL: Python's own symbols (and those of extension modules
            // loaded later) must be visible process-wide.
            // SAFETY: `c_path` is a valid NUL terminated string, and `dlopen` is
            // safe to call from any thread.
            let raw = unsafe { libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };
            if raw.is_null() {
                return Err(Error::new(last_error()));
            }
            Ok(Handle { raw })
        }

        pub unsafe fn symbol(raw: *mut c_void, name: &str) -> Option<*mut c_void> {
            let c_name = std::ffi::CString::new(name).ok()?;
            // Clear any stale error first, `dlsym` returning NULL is ambiguous.
            // SAFETY: `dlerror` takes no arguments and is safe to call.
            unsafe { libc::dlerror() };
            // SAFETY: `c_name` is a valid NUL terminated string, and `raw` is a
            // handle returned by `dlopen`.
            let symbol = unsafe { libc::dlsym(raw, c_name.as_ptr()) };
            if symbol.is_null() { None } else { Some(symbol) }
        }

        pub fn resolved_path(raw: *mut c_void, fallback: &str) -> String {
            // `dlopen` resolves a file name itself, so ask the loader which file
            // it ended up with, using a symbol every libpython exports.
            //
            // `dladdr` is not available on AIX; there the requested path is kept.
            #[cfg(not(target_os = "aix"))]
            {
                // SAFETY: `raw` is a handle returned by `dlopen`.
                if let Some(initialize) = unsafe { symbol(raw, "Py_Initialize") } {
                    // SAFETY: `Dl_info` is a plain data structure which
                    // `dladdr` only writes to.
                    let mut info: libc::Dl_info = unsafe { core::mem::zeroed() };
                    // SAFETY: `initialize` is the address of a function in the
                    // loaded library, which is what `dladdr` expects.
                    if unsafe { libc::dladdr(initialize, &mut info) } != 0
                        && !info.dli_fname.is_null()
                    {
                        // SAFETY: `dli_fname` points to a NUL terminated string
                        // owned by the loader.
                        let name = unsafe { core::ffi::CStr::from_ptr(info.dli_fname) };
                        if let Ok(path) = name.to_str() {
                            return path.to_string();
                        }
                    }
                }
            }
            fallback.to_string()
        }

        fn last_error() -> String {
            // SAFETY: `dlerror` returns either null or a NUL terminated string
            // owned by the loader.
            unsafe {
                let error = libc::dlerror();
                if error.is_null() {
                    "dlopen failed without an error message".to_string()
                } else {
                    core::ffi::CStr::from_ptr(error)
                        .to_string_lossy()
                        .into_owned()
                }
            }
        }

        #[allow(dead_code, reason = "only used by the dlopen flags above")]
        const _: c_int = libc::RTLD_LAZY;
    }

    #[cfg(windows)]
    mod imp {
        use super::*;
        use core::ffi::c_char;
        use std::string::{String, ToString};
        use std::vec::Vec;

        const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;

        // SAFETY: these are the documented kernel32 entry points; they are
        // available in every supported Windows process.
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn LoadLibraryExW(file_name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
            fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
            fn GetModuleHandleExW(
                flags: u32,
                module_name: *const u16,
                module: *mut *mut c_void,
            ) -> i32;
            fn GetModuleFileNameW(module: *mut c_void, file_name: *mut u16, size: u32) -> u32;
            fn GetLastError() -> u32;
        }

        fn wide(path: &str) -> Vec<u16> {
            let mut wide: Vec<u16> = path.encode_utf16().collect();
            wide.push(0);
            wide
        }

        /// The file name of a path, which is how a module already loaded in the
        /// process is looked up.
        fn file_name(path: &str) -> &str {
            path.rsplit(['\\', '/']).next().unwrap_or(path)
        }

        pub fn is_loaded_module(raw: *mut c_void, path: &str) -> bool {
            let mut loaded: *mut c_void = core::ptr::null_mut();
            // SAFETY: `file_name(path)` is a NUL terminated string, and the
            // returned handle is only compared.
            let found =
                unsafe { GetModuleHandleExW(0, wide(file_name(path)).as_ptr(), &mut loaded) };
            found != 0 && loaded == raw
        }

        pub unsafe fn open(path: &str) -> Result<Handle, Error> {
            let wide_path = wide(path);
            // `LOAD_WITH_ALTERED_SEARCH_PATH` makes Windows search the loaded
            // library's own directory for its dependencies (the C runtime, and
            // the `vcruntime`/`ucrt` DLLs shipped next to python.exe), which is
            // what an application shipping or downloading its own Python wants.
            // The alternate strategy applies to an absolute path only (the
            // relative case is documented as undefined), and `candidates()`
            // also produces bare file names - the name this crate was configured
            // with, and the platform's usual names - which are found through the
            // normal search order instead. A library whose directory is not part
            // of that search order is therefore only found by a full path: an
            // explicit [`load`], `PYO3_DYNAMIC_LIBRARY`, or the directory of
            // `PYO3_PYTHON`.
            // SAFETY: `wide_path` is a NUL terminated string, and the flags
            // below do not ask for anything unsafe.
            let raw = unsafe {
                LoadLibraryExW(
                    wide_path.as_ptr(),
                    core::ptr::null_mut(),
                    LOAD_WITH_ALTERED_SEARCH_PATH,
                )
            };
            if !raw.is_null() {
                return Ok(Handle { raw });
            }

            // SAFETY: `GetLastError` takes no arguments and is safe to call.
            let error = unsafe { GetLastError() };

            // Loading the requested file failed. Retrying with just the file
            // name would search PATH and silently load whichever Python it finds,
            // which is the opposite of choosing an interpreter at runtime. The
            // only substitution which still is "the library which was asked for"
            // is a module of the same file name which this process has already
            // loaded - for example the interpreter hosting a Python extension
            // module, even when the requested path has since gone stale.
            let mut already_loaded: *mut c_void = core::ptr::null_mut();
            // SAFETY: `name` is a NUL terminated string, and the returned handle
            // is only used to look up symbols.
            if unsafe { GetModuleHandleExW(0, wide(file_name(path)).as_ptr(), &mut already_loaded) }
                != 0
                && !already_loaded.is_null()
            {
                return Ok(Handle {
                    raw: already_loaded,
                });
            }

            Err(Error::new(std::format!(
                "LoadLibraryExW({path:?}) failed with Windows error {error}"
            )))
        }

        pub unsafe fn symbol(raw: *mut c_void, name: &str) -> Option<*mut c_void> {
            let c_name = std::ffi::CString::new(name).ok()?;
            // SAFETY: `c_name` is a NUL terminated string, and `raw` is a module
            // handle.
            let symbol = unsafe { GetProcAddress(raw, c_name.as_ptr()) };
            if symbol.is_null() { None } else { Some(symbol) }
        }

        pub fn resolved_path(raw: *mut c_void, fallback: &str) -> String {
            // `LoadLibraryExW` may have reused a module which was already loaded
            // from somewhere else, so report the file it really came from.
            let mut buffer: Vec<u16> = std::vec![0; 260];
            loop {
                // SAFETY: `raw` is a module handle, and `buffer` is a valid
                // writable buffer of the length which is passed with it.
                let length =
                    unsafe { GetModuleFileNameW(raw, buffer.as_mut_ptr(), buffer.len() as u32) };
                if length == 0 {
                    return fallback.to_string();
                }
                let length = length as usize;
                if length < buffer.len() {
                    return String::from_utf16_lossy(&buffer[..length]);
                }
                // A result filling the buffer means the path was truncated.
                if buffer.len() >= 0x8000 {
                    return fallback.to_string();
                }
                buffer.resize(buffer.len() * 2, 0);
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    mod imp {
        use super::*;

        pub unsafe fn open(_path: &str) -> Result<Handle, Error> {
            Err(Error::new(
                "dynamic-loading is only supported on Windows and Unix-like platforms",
            ))
        }

        pub unsafe fn symbol(_raw: *mut c_void, _name: &str) -> Option<*mut c_void> {
            None
        }

        pub fn is_loaded_module(_raw: *mut c_void, _path: &str) -> bool {
            false
        }

        pub fn resolved_path(_raw: *mut c_void, fallback: &str) -> String {
            fallback.to_string()
        }
    }
}

struct Loaded {
    path: String,
    handle: sys::Handle,
}

// SAFETY-NOTE: `std::sync::Mutex` is disallowed across the crate for the
// `no_std` work on `pyo3`; the loader only exists where `std` does, and a
// blocking mutex is what it needs.
#[allow(clippy::disallowed_types)]
static LIBRARY: Mutex<Option<Loaded>> = Mutex::new(None);

/// Opens `path` explicitly and makes it the library used for Python symbols.
///
/// This is the entry point to call when the application decides which Python
/// runtime to use (for example after downloading one). It must be called before
/// any Python C API is used: a library which is already loaded is never
/// replaced, because the symbols which have been resolved so far keep pointing
/// into it. Calling it again is only accepted when it names the library which is
/// already loaded, either by path or by the file name of a module the process
/// already has (which is how a stale path reaches the interpreter hosting a
/// Python extension module).
///
/// Failing to open `path` is an error rather than a fallback to a different
/// library, so that the caller keeps control over which interpreter runs; the
/// same goes for a path which opens but does not export the Python C API. See
/// [`loaded_path`] for the file which was really opened.
pub fn load(path: &str) -> Result<(), Error> {
    let mut library = LIBRARY.lock().unwrap_or_else(|err| err.into_inner());
    if let Some(loaded) = library.as_ref() {
        if loaded.path == path || loaded.handle.is_loaded_module(path) {
            return Ok(());
        }
        return Err(Error::new(std::format!(
            "the Python shared library {} is already loaded, and `load` cannot replace it \
             ({path:?} was requested); `load` has to be called before any Python C API is used",
            loaded.path
        )));
    }
    // SAFETY: opening a library is what this function is for, and the caller
    // passes a path which it wants to load.
    let handle = unsafe { sys::Handle::open(path) }?;
    ensure_python_library(&handle, path)?;
    // `Loaded.path` is what `loaded_path()` reports, and the loader may have
    // resolved the requested name itself (or reused an already loaded module),
    // so record the file which was really used rather than the request.
    let resolved = handle.resolved_path(path);
    *library = Some(Loaded {
        path: resolved,
        handle,
    });
    Ok(())
}

/// Checks that an opened module is a Python shared library.
///
/// Without this, a file which the platform can open but which is not Python - an
/// executable, which `LoadLibraryExW` accepts on Windows, or an unrelated
/// library - would be accepted here and fail at the first symbol lookup, with a
/// message about that symbol rather than about the library.
fn ensure_python_library(handle: &sys::Handle, path: &str) -> Result<(), Error> {
    // SAFETY: looking a symbol up in an opened library is safe.
    if unsafe { handle.symbol(SENTINEL_SYMBOL) }.is_some() {
        return Ok(());
    }
    Err(Error::new(std::format!(
        "{path:?} does not export `{SENTINEL_SYMBOL}`, so it is not a Python shared library"
    )))
}

/// Whether a Python shared library has been opened.
pub fn is_loaded() -> bool {
    LIBRARY
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .is_some()
}

/// The path of the file the currently loaded Python shared library came from, if
/// any.
pub fn loaded_path() -> Option<String> {
    LIBRARY
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .as_ref()
        .map(|loaded| loaded.path.clone())
}

/// Looks up `name` in the currently loaded library.
///
/// Returns `None` when no library is loaded or the symbol is absent. The
/// returned pointer is only valid while that library stays loaded, which it
/// does for the rest of the process: [`load`] cannot replace it.
pub fn symbol(name: &str) -> Option<*mut c_void> {
    let library = LIBRARY.lock().unwrap_or_else(|err| err.into_inner());
    let loaded = library.as_ref()?;
    // SAFETY: the library is loaded, and looking a symbol up in it is safe.
    unsafe { loaded.handle.symbol(name) }
}

/// Loads the library using the environment variables described in the module
/// documentation, or the best guess for the current platform.
///
/// Returns `Ok(true)` when a library was opened by this call, `Ok(false)` when
/// one was already loaded.
pub fn load_default() -> Result<bool, Error> {
    if is_loaded() {
        return Ok(false);
    }
    if let Some(path) = std::env::var_os(LIBRARY_ENV_VAR) {
        let path = path.to_string_lossy().into_owned();
        load(&path)?;
        return Ok(true);
    }
    let python = std::env::var_os(PYTHON_ENV_VAR);
    let candidates = match python.as_deref() {
        Some(python) => interpreter_candidates(python, CONFIGURED_LIBRARY_NAME),
        None => candidates(CONFIGURED_LIBRARY_NAME),
    };

    let mut attempts = Vec::new();
    for candidate in candidates {
        // Another thread may have loaded a library while this one was trying
        // candidates; never replace it.
        if is_loaded() {
            return Ok(false);
        }
        match load(&candidate) {
            Ok(()) => return Ok(true),
            Err(err) => attempts.push(std::format!("  {candidate}: {err}")),
        }
    }
    let attempts = if attempts.is_empty() {
        String::new()
    } else {
        std::format!(
            "\nTried:\n{}\nSet {LIBRARY_ENV_VAR} to the full path of the Python shared library.",
            attempts.join("\n")
        )
    };
    let wanted = match (python, CONFIGURED_LIBRARY_NAME) {
        (Some(python), Some(name)) => std::format!(
            " for the interpreter named by {PYTHON_ENV_VAR} ({}) or the {name} this crate \
             was configured with",
            Path::new(&python).display()
        ),
        (Some(python), None) => std::format!(
            " for the interpreter named by {PYTHON_ENV_VAR} ({})",
            Path::new(&python).display()
        ),
        (None, Some(name)) => std::format!(" ({name}, as this crate was configured with)"),
        (None, None) => String::new(),
    };
    Err(Error::new(std::format!(
        "could not locate the Python shared library{wanted}{attempts}"
    )))
}

/// Candidate libraries for the interpreter named by `PYO3_PYTHON`, best first.
///
/// The interpreter names a specific library: the shared library next to it. Only
/// libraries of the version and variant this crate was configured with are
/// collected, so pointing `PYO3_PYTHON` at a different Python is reported like
/// any other unusable library instead of loading a Python whose layout the
/// compiled code does not match.
///
/// The library of the configuration follows, because on several platforms the
/// library of an interpreter is not in its own directory but where the loader
/// searches by itself (`/usr/lib` on Linux, for instance).
fn interpreter_candidates(python: &OsStr, built_name: Option<&str>) -> Vec<String> {
    let configured = built_name.map(library_identity);
    let mut candidates = Vec::new();

    if let Some(dir) = Path::new(python).parent() {
        candidates.extend(library_names_in(dir, configured));
    }
    // The interpreter executable itself is deliberately not a candidate: on
    // Windows `LoadLibraryExW` opens an `.exe` happily, which would then fail at
    // the first symbol lookup instead of being reported by `load_default`.
    if let Some(name) = built_name {
        candidates.extend(built_file_names(name));
    }

    unique(candidates)
}

/// Candidate libraries when `PYO3_PYTHON` is not set, best first.
///
/// This is the order the module documentation describes. An explicit [`load`] and
/// `PYO3_DYNAMIC_LIBRARY` are honoured by [`load`] and [`load_default`] before
/// this list is consulted at all.
fn candidates(built_name: Option<&str>) -> Vec<String> {
    let configured = built_name.map(library_identity);
    let mut candidates = Vec::new();

    // 1. The shared library matching the configuration this crate was built
    //    with; it is the one whose layout the compiled code is known to match.
    if let Some(name) = built_name {
        for file in built_file_names(name) {
            #[cfg(windows)]
            if let Some(found) = find_in_path(&file) {
                candidates.push(found);
            }
            candidates.push(file);
        }
    }

    // 2. The platform's usual locations, but only for a build whose own name
    //    does not pin a version (`abi3`, or a configuration without a library
    //    name): for every other build a library found by searching would be of a
    //    different version than the compiled code expects.
    if configured.is_none_or(|configured| configured.minor.is_none()) {
        candidates.extend(platform_candidates(configured));
    }

    unique(candidates)
}

/// Names the platform's loader finds by itself, best first.
fn platform_candidates(configured: Option<LibraryIdentity>) -> Vec<String> {
    let mut candidates = Vec::new();

    #[cfg(windows)]
    {
        if let Some(path) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path) {
                candidates.extend(library_names_in(&dir, configured));
            }
        }
        // `python3.dll` is the stable ABI library, so only a build which does not
        // pin a version can use it; `may_load` decides that below.
        push_if_usable(&mut candidates, configured, "python3.dll");
    }

    #[cfg(target_os = "macos")]
    {
        // Highest supported version first; `dlopen` also searches the system
        // library directories when given a bare file name.
        for minor in (9..=16).rev() {
            let name = std::format!("libpython3.{minor}.dylib");
            push_if_usable(&mut candidates, configured, &name);
        }
        push_if_usable(&mut candidates, configured, "libpython3.dylib");
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Highest supported version first; `dlopen` also searches the system
        // library directories when given a bare file name.
        for minor in (9..=16).rev() {
            for name in [
                std::format!("libpython3.{minor}.so.1.0"),
                std::format!("libpython3.{minor}.so"),
                std::format!("libpython3.{minor}d.so.1.0"),
                std::format!("libpython3.{minor}t.so.1.0"),
            ] {
                push_if_usable(&mut candidates, configured, &name);
            }
        }
        push_if_usable(&mut candidates, configured, "libpython3.so");
    }

    #[cfg(not(any(windows, unix)))]
    let _ = configured;

    unique(candidates)
}

/// Adds `name` when this build may load it.
fn push_if_usable(candidates: &mut Vec<String>, configured: Option<LibraryIdentity>, name: &str) {
    if may_load(configured, name) {
        candidates.push(name.to_string());
    }
}

/// Whether a library with the file name `name` may be used by this build.
///
/// A build which pins a version may only load that version and variant: the
/// compiler's assumptions about object layouts come from the interpreter the
/// crate was configured with. An `abi3` build is free to load any later version,
/// which is what the stable ABI promises, and a name which encodes no version
/// (`python3.dll`, `libpython3.so`) is only usable by a build which does not pin
/// one either.
fn may_load(configured: Option<LibraryIdentity>, name: &str) -> bool {
    let Some(configured) = configured else {
        // Nothing to compare against: the configuration is taken at face value,
        // and a symbol which cannot be resolved is reported when it is used.
        return true;
    };
    let candidate = library_identity(name);
    candidate.debug == configured.debug
        && candidate.free_threaded == configured.free_threaded
        && match (configured.minor, candidate.minor) {
            (Some(configured), Some(candidate)) => {
                if cfg!(Py_LIMITED_API) {
                    candidate >= configured
                } else {
                    candidate == configured
                }
            }
            (None, _) => true,
            (Some(_), None) => false,
        }
}

/// The candidates in order, without duplicates.
fn unique(candidates: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate.clone()))
        .collect()
}

/// The file name(s) of the shared library for `name`, where `name` is the
/// libpython name from the build configuration (e.g. `python314` on Windows,
/// `python3.14` elsewhere).
///
/// The configuration already encodes the debug and free-threaded variants (a
/// debug build is configured against `python314_d`, a free-threaded one against
/// `python314t`, and `libpython3.14d.so.1.0` on Unix), so the other variants are
/// deliberately not offered: they would not be the ABI the crate was compiled
/// against.
fn built_file_names(name: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![std::format!("{name}.dll")]
    }
    #[cfg(target_os = "macos")]
    {
        match name.strip_prefix("python") {
            // `python3.14` -> `libpython3.14.dylib`
            Some(version) => vec![std::format!("libpython{version}.dylib")],
            None => vec![std::format!("lib{name}.dylib")],
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        match name.strip_prefix("python") {
            // `python3.14` -> `libpython3.14.so.1.0`
            Some(version) => vec![
                std::format!("libpython{version}.so.1.0"),
                std::format!("libpython{version}.so"),
            ],
            None => vec![std::format!("lib{name}.so")],
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = name;
        Vec::new()
    }
}

/// Full path of `file_name` on `PATH`, if it is there (Windows only).
#[cfg(windows)]
fn find_in_path(file_name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(file_name))
        .find(|candidate| candidate.is_file())
        .map(|candidate| candidate.to_string_lossy().into_owned())
}

/// Python shared libraries in `dir` which this build may load, newest version
/// first.
fn library_names_in(dir: &Path, configured: Option<LibraryIdentity>) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(i64, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            (is_python_library(&lower) && may_load(configured, &lower)).then(|| {
                (
                    library_version_rank(&lower),
                    entry.path().to_string_lossy().into_owned(),
                )
            })
        })
        .collect();
    // Prefer the highest version.
    found.sort_by_key(|(rank, _)| core::cmp::Reverse(*rank));
    found.into_iter().map(|(_, path)| path).collect()
}

/// Whether `lowercase_name` names a Python shared library.
fn is_python_library(lowercase_name: &str) -> bool {
    #[cfg(windows)]
    {
        // `libpython3.14.dll` is what a MinGW build of CPython ships.
        (lowercase_name.starts_with("python3") || lowercase_name.starts_with("libpython3"))
            && lowercase_name.ends_with(".dll")
    }
    #[cfg(target_os = "macos")]
    {
        lowercase_name.starts_with("libpython3") && lowercase_name.ends_with(".dylib")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        lowercase_name.starts_with("libpython3")
            && (lowercase_name.ends_with(".so") || lowercase_name.contains(".so."))
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = lowercase_name;
        false
    }
}

/// The version and variant a Python shared library file name encodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LibraryIdentity {
    /// The minor version, when the name encodes one. `python3.dll` and
    /// `libpython3.so` are the stable ABI libraries, which do not.
    minor: Option<i32>,
    /// Whether the library is a debug build (`python314_d.dll`).
    debug: bool,
    /// Whether the library is a free-threaded build (`libpython3.14t.so.1.0`).
    free_threaded: bool,
}

/// Reads the version and variant out of a libpython name.
///
/// `python314.dll`, `python3.14`, `libpython3.14.so.1.0`, `python314t_d.dll`,
/// `libpython3.14d.dylib` and the stable ABI names (`python3.dll`,
/// `python3t.dll`, `libpython3.so`) are all understood; a name which encodes
/// neither, such as PyPy's `libpypy3.11-c`, comes out as the version-less
/// identity.
fn library_identity(name: &str) -> LibraryIdentity {
    let lower = name.to_ascii_lowercase();
    let rest = lower.strip_prefix("lib").unwrap_or(&lower);
    let rest = rest.strip_prefix("python").unwrap_or(rest);
    let rest = rest.strip_prefix('3').unwrap_or(rest);
    // What follows is `.14d.so.1.0`, `14t_d.dll`, `.dll` (the stable ABI
    // library) or `t.dll` (the free-threaded stable ABI library).
    let separated = rest.starts_with(['.', '_']);
    let rest = rest.strip_prefix(['.', '_']).unwrap_or(rest);
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if separated && digits.is_empty() {
        // `python3.dll`, `libpython3.so`: the stable ABI library, which encodes
        // neither a version nor a variant. (Without the separator the same
        // shape is the free-threaded stable ABI library, `python3t.dll`.)
        return LibraryIdentity {
            minor: None,
            debug: false,
            free_threaded: false,
        };
    }
    let after_version = &rest[digits.len()..];
    let variant: String = after_version
        .chars()
        .take_while(|c| matches!(c, 'd' | 't' | '_'))
        .collect();
    LibraryIdentity {
        minor: digits.parse().ok(),
        debug: variant.contains('d'),
        free_threaded: variant.contains('t'),
    }
}

/// Ranks Python shared library names so that the newest interpreter wins.
///
/// The stable ABI library (`python3.dll`, `libpython3.so`) encodes no version
/// and ranks below every version-specific one; among the version-specific ones a
/// debug or free-threaded variant ranks below the default of the same version.
fn library_version_rank(lowercase_name: &str) -> i64 {
    let identity = library_identity(lowercase_name);
    match identity.minor {
        None => -1,
        Some(minor) => {
            i64::from(minor) * 100
                - i64::from(identity.debug) * 10
                - i64::from(identity.free_threaded) * 5
        }
    }
}

/// Looks `name` up, loading the library first if necessary.
///
/// # Panics
///
/// Panics when the symbol cannot be resolved, because there is no way to report
/// an error through the C API shape of the generated wrappers.
unsafe fn lookup(name: &'static str) -> *mut c_void {
    if let Some(symbol) = symbol(name) {
        return symbol;
    }
    if let Err(err) = load_default() {
        panic!(
            "pyo3: the Python C API symbol `{name}` was requested but no Python \
             shared library is loaded ({err})"
        );
    }
    match symbol(name) {
        Some(symbol) => symbol,
        None => panic!(
            "pyo3: the loaded Python shared library ({:?}) does not export `{name}`",
            loaded_path()
        ),
    }
}

/// Resolves a function symbol, caching it in `cache`.
///
/// # Safety
///
/// `T` must be the function pointer type matching the symbol's real signature.
#[inline]
pub unsafe fn resolve_fn<T: Copy>(cache: &AtomicPtr<c_void>, name: &'static str) -> T {
    let mut pointer = cache.load(Ordering::Acquire);
    if pointer.is_null() {
        // SAFETY: `lookup` only reads and updates the loader's own state.
        pointer = unsafe { lookup(name) };
        cache.store(pointer, Ordering::Release);
    }
    // SAFETY: the caller guarantees `T` matches the symbol;
    // function pointers are the same size as data pointers on all supported targets.
    unsafe { core::mem::transmute_copy(&pointer) }
}

/// Resolves the address of a data symbol, caching it in `cache`.
///
/// # Safety
///
/// The returned pointer is only valid while the library is loaded.
#[inline]
pub unsafe fn resolve_data<T>(cache: &AtomicPtr<c_void>, name: &'static str) -> *mut T {
    let mut pointer = cache.load(Ordering::Acquire);
    if pointer.is_null() {
        // SAFETY: `lookup` only reads and updates the loader's own state.
        pointer = unsafe { lookup(name) };
        cache.store(pointer, Ordering::Release);
    }
    pointer.cast::<T>()
}

#[allow(
    dead_code,
    reason = "keep the pointer type import used on all platforms"
)]
const _: *mut c_void = ptr::null_mut();

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A temporary directory which is removed when it goes out of scope.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(std::format!(
                "pyo3-dynamic-loading-test-{}-{name}",
                std::process::id()
            ));
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn touch(&self, file: &str) -> PathBuf {
            let path = self.0.join(file);
            std::fs::write(&path, b"").unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    /// The interpreter named by `PYO3_PYTHON` names a specific library, and the
    /// interpreter executable is never one of the candidates.
    ///
    /// The executable used to be offered as a library, which Windows accepts and
    /// opens, so `load_default` returned success and the failure was reported at
    /// the first symbol lookup instead.
    #[test]
    fn the_interpreter_library_is_preferred_and_its_executable_is_not_a_candidate() {
        let (interpreter, library, built_name, built_file) = if cfg!(windows) {
            ("python.exe", "python314.dll", "python314", "python314.dll")
        } else if cfg!(target_os = "macos") {
            (
                "python3.14",
                "libpython3.14.dylib",
                "python3.14",
                "libpython3.14.dylib",
            )
        } else {
            (
                "python3.14",
                "libpython3.14.so.1.0",
                "python3.14",
                "libpython3.14.so.1.0",
            )
        };
        let dir = TempDir::new("interpreter");
        let from_dir = dir.touch(library).to_string_lossy().into_owned();
        let interpreter = dir.0.join(interpreter);

        let candidates = interpreter_candidates(interpreter.as_os_str(), Some(built_name));

        let from_interpreter = candidates
            .iter()
            .position(|candidate| candidate == &from_dir)
            .expect("the interpreter's library should be a candidate");
        let from_build = candidates
            .iter()
            .position(|candidate| candidate == built_file)
            .expect("the configuration's library should be a candidate");
        assert!(
            from_interpreter < from_build,
            "expected the interpreter to be preferred: {candidates:?}"
        );

        let executable = interpreter
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            !candidates.iter().any(|candidate| Path::new(candidate)
                .file_name()
                .is_some_and(|name| name.to_string_lossy() == executable)),
            "the interpreter executable must not be a candidate: {candidates:?}"
        );
    }

    /// A library of another version or variant is not a candidate for a build
    /// which pins one: the compiled code's assumptions about the objects come
    /// from the interpreter the crate was configured with.
    #[cfg(not(Py_LIMITED_API))]
    #[test]
    fn a_different_version_or_variant_is_not_a_candidate() {
        let (wanted, others): (&str, &[&str]) = if cfg!(windows) {
            (
                "python314.dll",
                &[
                    "python39.dll",
                    "python315.dll",
                    "python314_d.dll",
                    "python314t.dll",
                    "python3.dll",
                ],
            )
        } else if cfg!(target_os = "macos") {
            (
                "libpython3.14.dylib",
                &[
                    "libpython3.9.dylib",
                    "libpython3.15.dylib",
                    "libpython3.14d.dylib",
                ],
            )
        } else {
            (
                "libpython3.14.so.1.0",
                &[
                    "libpython3.9.so.1.0",
                    "libpython3.15.so.1.0",
                    "libpython3.14d.so.1.0",
                    "libpython3.14t.so.1.0",
                    "libpython3.so",
                ],
            )
        };
        let dir = TempDir::new("versions");
        dir.touch(wanted);
        for other in others {
            dir.touch(other);
        }

        let configured = Some(library_identity("python314"));
        let found = library_names_in(&dir.0, configured);
        assert_eq!(
            found.len(),
            1,
            "only the configured version and variant may be offered: {found:?}"
        );
        assert!(found[0].ends_with(wanted));

        // Without a configuration to compare against, every Python library in
        // the directory is offered, newest first.
        let all = library_names_in(&dir.0, None);
        assert!(all.len() > 1, "{all:?}");
        assert!(
            all[0].ends_with("python315.dll") || all[0].contains("3.15"),
            "{all:?}"
        );
    }

    /// The version and variant parsing behind the candidates above.
    #[test]
    fn library_names_are_read_for_their_version_and_variant() {
        let cases: &[(&str, Option<i32>, bool, bool)] = if cfg!(windows) {
            &[
                ("python314.dll", Some(14), false, false),
                ("python314_d.dll", Some(14), true, false),
                ("python314t_d.dll", Some(14), true, true),
                ("python3.dll", None, false, false),
                ("python3t.dll", None, false, true),
                ("python-native.dll", None, false, false),
            ]
        } else if cfg!(target_os = "macos") {
            &[
                ("libpython3.14.dylib", Some(14), false, false),
                ("libpython3.14d.dylib", Some(14), true, false),
                ("libpython3.dylib", None, false, false),
            ]
        } else {
            &[
                ("libpython3.14.so.1.0", Some(14), false, false),
                ("libpython3.14d.so.1.0", Some(14), true, false),
                ("libpython3.14t.so.1.0", Some(14), false, true),
                ("libpython3.so", None, false, false),
            ]
        };
        for (name, minor, debug, free_threaded) in cases {
            let identity = library_identity(name);
            assert_eq!(
                (identity.minor, identity.debug, identity.free_threaded),
                (*minor, *debug, *free_threaded),
                "{name}"
            );
        }
    }

    /// A module which opens but is not Python has to be reported as such: an
    /// executable, for instance, which `LoadLibraryExW` accepts.
    #[test]
    fn a_module_which_is_not_python_is_rejected() {
        let Some(handle) = open_unrelated_library() else {
            return;
        };
        let error = ensure_python_library(&handle, "unrelated").unwrap_err();
        assert!(
            error.message().contains(SENTINEL_SYMBOL),
            "unexpected error: {}",
            error.message()
        );
    }

    /// Opens a library which is certainly not Python.
    fn open_unrelated_library() -> Option<sys::Handle> {
        let names: &[&str] = if cfg!(windows) {
            &["kernel32.dll", "ntdll.dll"]
        } else if cfg!(target_os = "macos") {
            &["libSystem.B.dylib"]
        } else {
            &["libc.so.6", "libc.so"]
        };
        names
            .iter()
            // SAFETY: opening one of these system libraries reads no memory, and
            // the handle is closed when it goes out of scope.
            .find_map(|name| unsafe { sys::Handle::open(name) }.ok())
    }
}
