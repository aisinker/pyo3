//! Implementation of the C variadic functions of the Python API.
//!
//! A trampoline which resolves a symbol at run time cannot forward an argument
//! list it does not understand, so the wrappers emitted by `extern_libpython!`
//! for the variadic declarations hand their [`VaList`] to one of the functions
//! here. Those either forward it to the `va_list`-taking twin of the variadic
//! function (done directly by the macro), read an argument list which describes
//! itself, or reimplement the function on top of public API, following what
//! CPython does internally.
//!
//! The module is only compiled for the `dynamic-loading` feature on Windows and
//! Unix-like platforms, where the wrappers are emitted instead of the variadic
//! foreign declarations. Defining C variadic functions - and the [`VaList`] this
//! module reads them with - needs Rust 1.99, which is why `pyo3-ffi`'s build
//! script refuses the feature on older compilers instead of raising the MSRV of
//! the crate as a whole.
//!
//! Every function here is a thin wrapper around the Python C API, whose safety
//! contract is the one documented by CPython for the function being wrapped.
#![allow(clippy::undocumented_unsafe_blocks)]
// The crate's MSRV covers builds without `dynamic-loading`; this module is only
// compiled once the build script has accepted a compiler which supports it.
#![allow(
    clippy::incompatible_msrv,
    reason = "only compiled with `dynamic-loading`, which the build script restricts to Rust 1.99"
)]

use core::ffi::{CStr, VaList, c_char, c_int, c_void};
use core::ptr;
use core::sync::atomic::AtomicPtr;

use crate::impl_::dynamic_loading::resolve_fn;
use crate::{Py_ssize_t, PyObject};

/// Declares the cache remembering the function pointer resolved for a symbol.
macro_rules! cache {
    () => {
        static CACHE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
    };
}

/// Formats with `PyUnicode_FromFormatV`, which is what the `PySys_Format*` and
/// `PyErr_Warn*Format` families use.
unsafe fn unicode_from_format(format: *const c_char, varargs: VaList<'_>) -> *mut PyObject {
    cache!();
    type FromFormatV = for<'f> unsafe extern "C" fn(*const c_char, VaList<'f>) -> *mut PyObject;
    let from_format: FromFormatV = unsafe { resolve_fn(&CACHE, "PyUnicode_FromFormatV") };
    unsafe { from_format(format, varargs) }
}

/// The UTF-8 form of `message` as a borrowed C string.
///
/// CPython hands the unicode object to the warning machinery directly; the
/// wrappers have to go through a `const char *`, because that is what the stable
/// ABI offers (`PyErr_WarnEx`). `PyUnicode_AsUTF8String` and `PyBytes_AsString`
/// have been part of it since 3.2, so the round trip through a bytes object works
/// for every `abi3` version - and, unlike `PyUnicode_AsUTF8`, `python3.dll`
/// really exports them.
///
/// A message which UTF-8 cannot represent (a lone surrogate, which
/// `PyUnicode_FromFormatV` happily produces) would fail that round trip with a
/// `UnicodeEncodeError`, turning a warning into an exception for the caller.
/// Those messages are escaped with `backslashreplace` instead, which is what
/// printing such a string does; the text differs from CPython's, but the warning
/// is still reported.
///
/// Returns the new reference which keeps `message` alive next to the borrowed
/// C string, or `None` with an exception set.
unsafe fn message_bytes(message: *mut PyObject) -> Option<(*mut PyObject, *mut c_char)> {
    const ESCAPED: &core::ffi::CStr = c"backslashreplace";

    let utf8 = unsafe { crate::PyUnicode_AsUTF8String(message) };
    let utf8 = if utf8.is_null() {
        unsafe { crate::PyErr_Clear() };
        let escaped = unsafe {
            crate::PyUnicode_AsEncodedString(message, c"utf-8".as_ptr(), ESCAPED.as_ptr())
        };
        if escaped.is_null() {
            return None;
        }
        escaped
    } else {
        utf8
    };
    let text = unsafe { crate::PyBytes_AsString(utf8) };
    if text.is_null() {
        unsafe { crate::Py_DECREF(utf8) };
        return None;
    }
    Some((utf8, text))
}

/// Hands the UTF-8 form of `message` to `PyErr_WarnEx`.
unsafe fn warn_ex_utf8(
    category: *mut PyObject,
    message: *mut PyObject,
    stack_level: Py_ssize_t,
) -> c_int {
    let Some((utf8, text)) = (unsafe { message_bytes(message) }) else {
        return -1;
    };
    let result = unsafe { crate::PyErr_WarnEx(category, text, stack_level) };
    unsafe { crate::Py_DECREF(utf8) };
    result
}

/// Builds the argument tuple for a format-based call, the way CPython's
/// `PyObject_CallFunction` does: `Py_VaBuildValue` produces a tuple for formats
/// with several items and the single object otherwise, which is then wrapped.
///
/// A null (or empty) format means "no arguments" for `PyObject_CallFunction`
/// and `PyObject_CallMethod`. CPython implements those two by building the empty
/// tuple themselves in that case, which is what this does, rather than handing
/// the null format to `Py_VaBuildValue`.
unsafe fn build_call_args(format: *const c_char, varargs: VaList<'_>) -> *mut PyObject {
    if format.is_null() || unsafe { *format } == 0 {
        return unsafe { crate::PyTuple_New(0) };
    }

    cache!();
    type VaBuildValue = for<'f> unsafe extern "C" fn(*const c_char, VaList<'f>) -> *mut PyObject;
    let build: VaBuildValue = unsafe { resolve_fn(&CACHE, "Py_VaBuildValue") };
    let built = unsafe { build(format, varargs) };
    if built.is_null() {
        return ptr::null_mut();
    }
    if unsafe { crate::PyTuple_Check(built) } != 0 {
        return built;
    }

    let tuple = unsafe { crate::PyTuple_New(1) };
    if tuple.is_null() {
        unsafe { crate::Py_DECREF(built) };
        return ptr::null_mut();
    }
    unsafe {
        crate::Py_INCREF(built);
        crate::PyTuple_SetItem(tuple, 0, built);
        crate::Py_DECREF(built);
    }
    tuple
}

/// Reports a null argument the way CPython's `PyObject_Call*` helpers do, and
/// returns the `NULL` they return.
///
/// CPython uses the public `PyErr_BadInternalCall` there, whose "null argument
/// to internal routine" message cannot be reproduced by
/// `pyo3_ffi::PyErr_BadInternalCall` (that one adds a source location through
/// the private `_PyErr_BadInternalCall`), and setting the string keeps the
/// wrapper inside the stable ABI.
unsafe fn null_argument_error() -> *mut PyObject {
    unsafe {
        crate::PyErr_SetString(
            python_static_value!(crate::PyExc_SystemError),
            c"null argument to internal routine".as_ptr(),
        );
    }
    ptr::null_mut()
}

/// Implements `PyObject_CallFunction` and `PyEval_CallFunction`.
pub(crate) unsafe fn call_with_format(
    callable: *mut PyObject,
    format: *const c_char,
    varargs: VaList<'_>,
) -> *mut PyObject {
    if callable.is_null() {
        return unsafe { null_argument_error() };
    }
    let args = unsafe { build_call_args(format, varargs) };
    if args.is_null() {
        return ptr::null_mut();
    }
    let result = unsafe { crate::PyObject_CallObject(callable, args) };
    unsafe { crate::Py_DECREF(args) };
    result
}

/// Implements `PyObject_CallMethod` and `PyEval_CallMethod`.
pub(crate) unsafe fn call_method_with_format(
    obj: *mut PyObject,
    method: *const c_char,
    format: *const c_char,
    varargs: VaList<'_>,
) -> *mut PyObject {
    if obj.is_null() || method.is_null() {
        return unsafe { null_argument_error() };
    }
    let args = unsafe { build_call_args(format, varargs) };
    if args.is_null() {
        return ptr::null_mut();
    }
    let name = unsafe { crate::PyUnicode_FromString(method) };
    if name.is_null() {
        unsafe { crate::Py_DECREF(args) };
        return ptr::null_mut();
    }
    let callable = unsafe { crate::PyObject_GetAttr(obj, name) };
    unsafe { crate::Py_DECREF(name) };
    if callable.is_null() {
        unsafe { crate::Py_DECREF(args) };
        return ptr::null_mut();
    }
    let result = unsafe { crate::PyObject_CallObject(callable, args) };
    unsafe {
        crate::Py_DECREF(callable);
        crate::Py_DECREF(args);
    }
    result
}

/// Collects a `NULL`-terminated argument list into a new tuple.
unsafe fn tuple_from_null_terminated(varargs: VaList<'_>) -> *mut PyObject {
    // A `VaList` may be cloned, which behaves like C's `va_copy`, so the list can
    // be walked twice: once to count the arguments, and once to store them.
    let mut counter = varargs.clone();
    let mut count: Py_ssize_t = 0;
    while !unsafe { counter.next_arg::<*mut PyObject>() }.is_null() {
        count += 1;
    }

    let tuple = unsafe { crate::PyTuple_New(count) };
    if tuple.is_null() {
        return ptr::null_mut();
    }

    let mut varargs = varargs;
    for index in 0..count {
        let item = unsafe { varargs.next_arg::<*mut PyObject>() };
        // The arguments are borrowed from the caller, so the tuple needs its own
        // references; `PyTuple_SetItem` steals the one added here.
        unsafe {
            crate::Py_INCREF(item);
            crate::PyTuple_SetItem(tuple, index, item);
        }
    }
    tuple
}

/// Implements `PyObject_CallFunctionObjArgs`.
pub(crate) unsafe fn call_function_obj_args(
    callable: *mut PyObject,
    varargs: VaList<'_>,
) -> *mut PyObject {
    if callable.is_null() {
        return unsafe { null_argument_error() };
    }
    let args = unsafe { tuple_from_null_terminated(varargs) };
    if args.is_null() {
        return ptr::null_mut();
    }
    let result = unsafe { crate::PyObject_CallObject(callable, args) };
    unsafe { crate::Py_DECREF(args) };
    result
}

/// Implements `PyObject_CallMethodObjArgs`.
pub(crate) unsafe fn call_method_obj_args(
    obj: *mut PyObject,
    method: *mut PyObject,
    varargs: VaList<'_>,
) -> *mut PyObject {
    if obj.is_null() || method.is_null() {
        return unsafe { null_argument_error() };
    }
    let args = unsafe { tuple_from_null_terminated(varargs) };
    if args.is_null() {
        return ptr::null_mut();
    }
    let callable = unsafe { crate::PyObject_GetAttr(obj, method) };
    if callable.is_null() {
        unsafe { crate::Py_DECREF(args) };
        return ptr::null_mut();
    }
    let result = unsafe { crate::PyObject_CallObject(callable, args) };
    unsafe {
        crate::Py_DECREF(callable);
        crate::Py_DECREF(args);
    }
    result
}

/// Implements `PyTuple_Pack`.
///
/// Unlike CPython, a `NULL` item is reported as a `SystemError` instead of being
/// dereferenced.
pub(crate) unsafe fn tuple_pack(size: Py_ssize_t, varargs: VaList<'_>) -> *mut PyObject {
    if size == 0 {
        return unsafe { crate::PyTuple_New(0) };
    }
    if size < 0 {
        unsafe { crate::PyErr_BadInternalCall() };
        return ptr::null_mut();
    }

    let tuple = unsafe { crate::PyTuple_New(size) };
    if tuple.is_null() {
        return ptr::null_mut();
    }

    let mut varargs = varargs;
    for index in 0..size {
        let item = unsafe { varargs.next_arg::<*mut PyObject>() };
        if item.is_null() {
            unsafe {
                crate::Py_DECREF(tuple);
                crate::PyErr_SetString(
                    python_static_value!(crate::PyExc_SystemError),
                    c"PyTuple_Pack() received a NULL argument".as_ptr(),
                );
            }
            return ptr::null_mut();
        }
        // The arguments are borrowed from the caller, so the tuple needs a
        // reference of its own: `PyTuple_SetItem` steals the one taken here,
        // exactly as CPython's `PyTuple_Pack` does with `Py_INCREF` followed by
        // `PyTuple_SET_ITEM`.
        unsafe {
            crate::Py_INCREF(item);
            if crate::PyTuple_SetItem(tuple, index, item) != 0 {
                // The item was not stored, so give back the reference above.
                crate::Py_DECREF(item);
                crate::Py_DECREF(tuple);
                return ptr::null_mut();
            }
        }
    }
    tuple
}

/// Implements `PyArg_UnpackTuple`: reads `PyObject **` output variables from the
/// argument list and fills them with new references.
pub(crate) unsafe fn unpack_tuple(
    args: *mut PyObject,
    name: *const c_char,
    min: Py_ssize_t,
    max: Py_ssize_t,
    varargs: VaList<'_>,
) -> c_int {
    if unsafe { crate::PyTuple_Check(args) } == 0 {
        unsafe {
            crate::PyErr_SetString(
                python_static_value!(crate::PyExc_SystemError),
                c"PyArg_UnpackTuple() argument list is not a tuple".as_ptr(),
            );
        }
        return 0;
    }

    let length = unsafe { crate::PyTuple_Size(args) };
    if length < min {
        // A `min` equal to `max` means an exact argument count is expected.
        let qualifier = if min == max { c"" } else { c"at least " };
        let plural = if min == 1 { c"" } else { c"s" };
        unsafe {
            // A null `name` cannot be formatted with `%s`; CPython reports the
            // mismatch without naming the caller in that case.
            if name.is_null() {
                crate::PyErr_Format(
                    python_static_value!(crate::PyExc_TypeError),
                    c"unpacked tuple should have %s%zd element%s, but has %zd".as_ptr(),
                    qualifier.as_ptr(),
                    min,
                    plural.as_ptr(),
                    length,
                );
            } else {
                crate::PyErr_Format(
                    python_static_value!(crate::PyExc_TypeError),
                    c"%s expected %s%zd argument%s, got %zd".as_ptr(),
                    name,
                    qualifier.as_ptr(),
                    min,
                    plural.as_ptr(),
                    length,
                );
            }
        }
        return 0;
    }
    if length > max {
        let plural = if max == 1 { c"" } else { c"s" };
        unsafe {
            if name.is_null() {
                crate::PyErr_Format(
                    python_static_value!(crate::PyExc_TypeError),
                    c"unpacked tuple should have at most %zd element%s, but has %zd".as_ptr(),
                    max,
                    plural.as_ptr(),
                    length,
                );
            } else {
                crate::PyErr_Format(
                    python_static_value!(crate::PyExc_TypeError),
                    c"%s expected at most %zd argument%s, got %zd".as_ptr(),
                    name,
                    max,
                    plural.as_ptr(),
                    length,
                );
            }
        }
        return 0;
    }
    if length == 0 {
        return 1;
    }

    let mut varargs = varargs;
    for index in 0..length {
        let item = unsafe { crate::PyTuple_GetItem(args, index) };
        if item.is_null() {
            return 0;
        }
        let slot = unsafe { varargs.next_arg::<*mut *mut PyObject>() };
        unsafe {
            crate::Py_INCREF(item);
            *slot = item;
        }
    }
    1
}

/// Implements `PyErr_WarnFormat`.
pub(crate) unsafe fn warn_format(
    category: *mut PyObject,
    stack_level: Py_ssize_t,
    format: *const c_char,
    varargs: VaList<'_>,
) -> c_int {
    let message = unsafe { unicode_from_format(format, varargs) };
    if message.is_null() {
        return -1;
    }
    let result = unsafe { warn_ex_utf8(category, message, stack_level) };
    unsafe { crate::Py_DECREF(message) };
    result
}

/// Implements `PyErr_ResourceWarning`.
///
/// `source` is dropped: CPython passes it to the warning machinery as the source
/// of the warning and records it on the warning, but the stable ABI has no entry
/// point which accepts it - `PyErr_WarnEx` is the only warning function the
/// wrappers can use - so this produces what `PyErr_WarnFormat` does for the
/// `ResourceWarning` category. The category, the message and where the warning is
/// attributed are the same; only the recorded source is missing, which the guide
/// lists among the differences of this mode.
pub(crate) unsafe fn resource_warning(
    source: *mut PyObject,
    stack_level: Py_ssize_t,
    format: *const c_char,
    varargs: VaList<'_>,
) -> c_int {
    let _ = source;
    unsafe {
        warn_format(
            python_static_value!(crate::PyExc_ResourceWarning),
            stack_level,
            format,
            varargs,
        )
    }
}

/// Implements the `PySys_WriteStdout` / `PySys_WriteStderr` family: format with
/// `printf` semantics (truncated to 1000 bytes, like CPython) and write the
/// result to `sys.stdout` / `sys.stderr`.
pub(crate) unsafe fn sys_write_stdout(format: *const c_char, varargs: VaList<'_>) {
    unsafe {
        write_to_stream(c"stdout", format, varargs, /* unicode = */ false)
    }
}

/// See [`sys_write_stdout`].
pub(crate) unsafe fn sys_write_stderr(format: *const c_char, varargs: VaList<'_>) {
    unsafe {
        write_to_stream(c"stderr", format, varargs, /* unicode = */ false)
    }
}

/// Implements the `PySys_FormatStdout` / `PySys_FormatStderr` Family: format with
/// `PyUnicode_FromFormatV` (no truncation) and write the result to `sys.stdout` /
/// `sys.stderr`.
pub(crate) unsafe fn sys_format_stdout(format: *const c_char, varargs: VaList<'_>) {
    unsafe {
        write_to_stream(c"stdout", format, varargs, /* unicode = */ true)
    }
}

/// See [`sys_format_stdout`].
pub(crate) unsafe fn sys_format_stderr(format: *const c_char, varargs: VaList<'_>) {
    unsafe {
        write_to_stream(c"stderr", format, varargs, /* unicode = */ true)
    }
}

/// Shared implementation of the `PySys_*` family.
///
/// These functions never raise: errors are cleared, matching CPython, which uses
/// them to report diagnostics from places where an exception cannot be raised.
unsafe fn write_to_stream(name: &CStr, format: *const c_char, varargs: VaList<'_>, unicode: bool) {
    let stream = unsafe { crate::PySys_GetObject(name.as_ptr()) };
    if stream.is_null() || stream == unsafe { crate::Py_None() } {
        unsafe { crate::PyErr_Clear() };
        return;
    }

    if unicode {
        let text = unsafe { unicode_from_format(format, varargs) };
        if text.is_null() {
            unsafe { crate::PyErr_Clear() };
            return;
        }
        let written = unsafe { crate::PyFile_WriteObject(text, stream, crate::Py_PRINT_RAW) };
        unsafe { crate::Py_DECREF(text) };
        if written != 0 {
            unsafe { crate::PyErr_Clear() };
        }
    } else {
        cache!();
        type Vsnprintf =
            for<'f> unsafe extern "C" fn(*mut c_char, usize, *const c_char, VaList<'f>) -> c_int;
        let vsnprintf: Vsnprintf = unsafe { resolve_fn(&CACHE, "PyOS_vsnprintf") };
        let mut buffer = [0 as c_char; 1000];
        let written = unsafe { vsnprintf(buffer.as_mut_ptr(), buffer.len(), format, varargs) };
        if written < 0 {
            unsafe { crate::PyErr_Clear() };
            return;
        }
        if unsafe { crate::PyFile_WriteString(buffer.as_ptr(), stream) } != 0 {
            unsafe { crate::PyErr_Clear() };
        }
    }
}
