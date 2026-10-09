#![cfg(all(feature = "dynamic-loading", any(windows, unix)))]

//! End to end check that PyO3 can be built without linking to libpython and
//! still run Python, by opening the shared library at runtime.
//!
//! Building this test also verifies the linking side: the resulting binary has
//! no import of the Python shared library.

use std::path::Path;

use pyo3::prelude::*;

/// The variadic functions of the C API are provided by `pyo3-ffi` as real
/// c-variadic wrappers. Exercise one function per forwarding technique.
#[test]
fn variadic_c_functions_are_available() {
    pyo3::ffi::dynamic_loading::load_default().expect("Python should be discoverable");
    pyo3::Python::initialize();

    Python::attach(|py| {
        use core::ffi::{c_int, c_void};
        use pyo3::ffi;
        use pyo3::types::{PyList, PyString, PyTuple};

        // SAFETY: the format strings and arguments match the C API the wrapper
        // forwards them to, and every pointer is either a static Python object
        // accessor or a borrowed reference which stays alive for the call.
        unsafe {
            // Forwarded to Py_VaBuildValue.
            let built = ffi::Py_BuildValue(c"(ii)".as_ptr(), 1 as c_int, 2 as c_int);
            assert!(!built.is_null());
            let built = Bound::from_owned_ptr(py, built)
                .cast_into::<PyTuple>()
                .unwrap();
            assert_eq!(built.len(), 2);
            assert_eq!(built.get_item(1).unwrap().extract::<i32>().unwrap(), 2);

            // Forwarded to PyUnicode_FromFormatV.
            let text = ffi::PyUnicode_FromFormat(c"%d-%s".as_ptr(), 7 as c_int, c"x".as_ptr());
            assert!(!text.is_null());
            let text = Bound::from_owned_ptr(py, text);
            assert_eq!(text.extract::<String>().unwrap(), "7-x");

            // Forwarded to PyErr_FormatV.
            let message = c"boom".as_ptr();
            let error = ffi::PyErr_Format(
                ffi::python_static_value!(ffi::PyExc_ValueError),
                c"%s %d".as_ptr(),
                message,
                42 as c_int,
            );
            assert!(error.is_null());
            let error = PyErr::fetch(py);
            assert!(error.is_instance_of::<pyo3::exceptions::PyValueError>(py));
            assert_eq!(error.value(py).to_string(), "boom 42");

            // Reimplemented on top of Py_VaBuildValue.
            let callable = py.eval(c"lambda a, b: a * b", None, None).unwrap();
            let result = ffi::PyObject_CallFunction(
                callable.as_ptr(),
                c"ii".as_ptr(),
                6 as c_int,
                7 as c_int,
            );
            assert!(!result.is_null());
            assert_eq!(
                Bound::from_owned_ptr(py, result).extract::<i32>().unwrap(),
                42
            );

            // A null (or empty) format means "no arguments", which CPython's
            // `PyObject_CallFunction` and `PyObject_CallMethod` implement
            // themselves: handing the null format to `Py_VaBuildValue` would
            // dereference it.
            let no_args = py.eval(c"lambda: 'called'", None, None).unwrap();
            let empty_list = PyList::empty(py);
            for format in [core::ptr::null(), c"".as_ptr()] {
                let result = ffi::PyObject_CallFunction(no_args.as_ptr(), format);
                assert!(!result.is_null());
                assert_eq!(
                    Bound::from_owned_ptr(py, result)
                        .extract::<String>()
                        .unwrap(),
                    "called"
                );

                let result =
                    ffi::PyObject_CallMethod(empty_list.as_ptr(), c"copy".as_ptr(), format);
                assert!(!result.is_null());
                assert_eq!(
                    Bound::from_owned_ptr(py, result)
                        .cast_into::<PyList>()
                        .unwrap()
                        .len(),
                    0
                );

                // `PyEval_CallFunction` and `PyEval_CallMethod` are aliases of
                // the above, but were removed from the C API in Python 3.13.
                #[cfg(not(Py_3_13))]
                #[allow(
                    deprecated,
                    reason = "covering the deprecated PyEval_* aliases is the point of this block"
                )]
                {
                    let result = ffi::PyEval_CallFunction(no_args.as_ptr(), format);
                    assert!(!result.is_null());
                    assert_eq!(
                        Bound::from_owned_ptr(py, result)
                            .extract::<String>()
                            .unwrap(),
                        "called"
                    );

                    let result =
                        ffi::PyEval_CallMethod(empty_list.as_ptr(), c"copy".as_ptr(), format);
                    assert!(!result.is_null());
                    Bound::from_owned_ptr(py, result);
                }
            }

            // ... and the method-call flavour of the same technique.
            let list = PyList::empty(py);
            let result = ffi::PyObject_CallMethod(
                list.as_ptr(),
                c"append".as_ptr(),
                c"i".as_ptr(),
                5 as c_int,
            );
            assert!(!result.is_null());
            Bound::from_owned_ptr(py, result);
            assert_eq!(list.len(), 1);

            // NULL-terminated argument lists, built with `VaList::next_arg`.
            let join = py
                .eval(c"lambda *args: '+'.join(str(a) for a in args)", None, None)
                .unwrap();
            let one = PyString::new(py, "a");
            let two = PyString::new(py, "b");
            let result = ffi::PyObject_CallFunctionObjArgs(
                join.as_ptr(),
                one.as_ptr(),
                two.as_ptr(),
                core::ptr::null_mut::<c_void>(),
            );
            assert!(!result.is_null());
            assert_eq!(
                Bound::from_owned_ptr(py, result)
                    .extract::<String>()
                    .unwrap(),
                "a+b"
            );

            let result = ffi::PyObject_CallMethodObjArgs(
                list.as_ptr(),
                PyString::new(py, "append").as_ptr(),
                one.as_ptr(),
                core::ptr::null_mut::<c_void>(),
            );
            assert!(!result.is_null());
            Bound::from_owned_ptr(py, result);
            assert_eq!(list.len(), 2);

            // Counted argument list.
            let tuple = ffi::PyTuple_Pack(2, one.as_ptr(), two.as_ptr());
            assert!(!tuple.is_null());
            let tuple = Bound::from_owned_ptr(py, tuple)
                .cast_into::<PyTuple>()
                .unwrap();
            assert_eq!(tuple.len(), 2);

            // PyArg_Parse / PyArg_ParseTuple / ...AndKeywords.
            let args = PyTuple::new(py, [3, 4]).unwrap();
            let mut a: c_int = 0;
            let mut b: c_int = 0;
            let ok = ffi::PyArg_ParseTuple(
                args.as_ptr(),
                c"ii".as_ptr(),
                &mut a as *mut c_int,
                &mut b as *mut c_int,
            );
            assert_eq!(ok, 1);
            assert_eq!((a, b), (3, 4));

            #[cfg(Py_3_13)]
            {
                let mut keywords: [*const core::ffi::c_char; 3] =
                    [c"first".as_ptr(), c"second".as_ptr(), core::ptr::null()];
                let mut c: c_int = 0;
                let mut d: c_int = 0;
                let ok = ffi::PyArg_ParseTupleAndKeywords(
                    args.as_ptr(),
                    core::ptr::null_mut(),
                    c"i|i".as_ptr(),
                    keywords.as_mut_ptr(),
                    &mut c as *mut c_int,
                    &mut d as *mut c_int,
                );
                assert_eq!(ok, 1);
                assert_eq!((c, d), (3, 4));
            }

            // PyArg_UnpackTuple fills the given variables with new references.
            let args = PyTuple::new(py, [1, 2, 3]).unwrap();
            let mut first: *mut ffi::PyObject = core::ptr::null_mut();
            let mut second: *mut ffi::PyObject = core::ptr::null_mut();
            let mut third: *mut ffi::PyObject = core::ptr::null_mut();
            let ok = ffi::PyArg_UnpackTuple(
                args.as_ptr(),
                c"name".as_ptr(),
                2,
                3,
                &mut first as *mut *mut ffi::PyObject,
                &mut second as *mut *mut ffi::PyObject,
                &mut third as *mut *mut ffi::PyObject,
            );
            assert_eq!(ok, 1);
            assert_eq!(
                [
                    Bound::from_owned_ptr(py, first).extract::<i32>().unwrap(),
                    Bound::from_owned_ptr(py, second).extract::<i32>().unwrap(),
                    Bound::from_owned_ptr(py, third).extract::<i32>().unwrap(),
                ],
                [1, 2, 3]
            );

            // Its errors match CPython's, message for message.
            let ok = ffi::PyArg_UnpackTuple(
                args.as_ptr(),
                c"name".as_ptr(),
                4,
                4,
                &mut first as *mut *mut ffi::PyObject,
            );
            assert_eq!(ok, 0);
            assert_eq!(
                PyErr::fetch(py).value(py).to_string(),
                "name expected 4 arguments, got 3"
            );

            let ok = ffi::PyArg_UnpackTuple(
                args.as_ptr(),
                c"name".as_ptr(),
                1,
                2,
                &mut first as *mut *mut ffi::PyObject,
            );
            assert_eq!(ok, 0);
            assert_eq!(
                PyErr::fetch(py).value(py).to_string(),
                "name expected at most 2 arguments, got 3"
            );

            // With a null name CPython reports the mismatch without naming the
            // caller, rather than formatting the null pointer with `%s`.
            let ok = ffi::PyArg_UnpackTuple(
                args.as_ptr(),
                core::ptr::null(),
                4,
                4,
                &mut first as *mut *mut ffi::PyObject,
            );
            assert_eq!(ok, 0);
            assert_eq!(
                PyErr::fetch(py).value(py).to_string(),
                "unpacked tuple should have 4 elements, but has 3"
            );

            let ok = ffi::PyArg_UnpackTuple(
                args.as_ptr(),
                core::ptr::null(),
                4,
                5,
                &mut first as *mut *mut ffi::PyObject,
            );
            assert_eq!(ok, 0);
            assert_eq!(
                PyErr::fetch(py).value(py).to_string(),
                "unpacked tuple should have at least 4 elements, but has 3"
            );

            let ok = ffi::PyArg_UnpackTuple(
                args.as_ptr(),
                core::ptr::null(),
                1,
                2,
                &mut first as *mut *mut ffi::PyObject,
            );
            assert_eq!(ok, 0);
            assert_eq!(
                PyErr::fetch(py).value(py).to_string(),
                "unpacked tuple should have at most 2 elements, but has 3"
            );

            let single = PyTuple::new(py, [7]).unwrap();
            let ok = ffi::PyArg_UnpackTuple(
                single.as_ptr(),
                core::ptr::null(),
                1,
                1,
                &mut first as *mut *mut ffi::PyObject,
            );
            assert_eq!(ok, 1);
            Bound::from_owned_ptr(py, first);

            let ok = ffi::PyArg_UnpackTuple(
                list.as_ptr(),
                c"name".as_ptr(),
                0,
                1,
                &mut first as *mut *mut ffi::PyObject,
            );
            assert_eq!(ok, 0);
            assert!(PyErr::fetch(py).is_instance_of::<pyo3::exceptions::PySystemError>(py));

            // PyTuple_Pack reports a null item instead of dereferencing it.
            let broken = ffi::PyTuple_Pack(1, core::ptr::null_mut::<ffi::PyObject>());
            assert!(broken.is_null());
            let error = PyErr::fetch(py);
            assert!(error.is_instance_of::<pyo3::exceptions::PySystemError>(py));
            assert_eq!(
                error.value(py).to_string(),
                "PyTuple_Pack() received a NULL argument"
            );

            // PySys_WriteStdout / PySys_FormatStdout write to sys.stdout.
            let io = py.import("io").unwrap();
            let sys = py.import("sys").unwrap();
            let buffer = io.call_method0("StringIO").unwrap();
            let saved = sys.getattr("stdout").unwrap();
            sys.setattr("stdout", &buffer).unwrap();
            ffi::PySys_WriteStdout(c"w:%d;".as_ptr(), 1 as c_int);
            ffi::PySys_FormatStdout(c"f:%d;".as_ptr(), 2 as c_int);
            sys.setattr("stdout", saved).unwrap();
            let written: String = buffer.call_method0("getvalue").unwrap().extract().unwrap();
            assert_eq!(written, "w:1;f:2;");

            // PyErr_WarnFormat goes through the warning machinery.
            let warnings = py.import("warnings").unwrap();
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("record", true).unwrap();
            let catcher = warnings
                .call_method("catch_warnings", (), Some(&kwargs))
                .unwrap();
            // `__enter__` returns the list which records the warnings.
            let recorded = catcher.call_method0("__enter__").unwrap();
            warnings.call_method1("simplefilter", ("always",)).unwrap();
            let result = ffi::PyErr_WarnFormat(
                pyo3::ffi::python_static_value!(ffi::PyExc_UserWarning),
                1 as ffi::Py_ssize_t,
                c"warned %s".as_ptr(),
                c"here".as_ptr(),
            );
            assert_eq!(result, 0);

            // PyErr_ResourceWarning warns from the given source object.
            let source = PyList::empty(py);
            let result = ffi::PyErr_ResourceWarning(
                source.as_ptr(),
                1 as ffi::Py_ssize_t,
                c"resource %d".as_ptr(),
                3 as c_int,
            );
            assert_eq!(
                result,
                0,
                "PyErr_ResourceWarning failed: {}",
                PyErr::take(py).map(|e| e.to_string()).unwrap_or_default()
            );

            catcher
                .call_method1("__exit__", (py.None(), py.None(), py.None()))
                .unwrap();
            let recorded = recorded.cast_into::<PyList>().unwrap();
            assert_eq!(recorded.len(), 2);
            let messages: Vec<String> = recorded
                .iter()
                .map(|item| {
                    (
                        item.getattr("category").unwrap().to_string(),
                        item.getattr("message").unwrap().to_string(),
                    )
                })
                .map(|(category, message)| format!("{category}: {message}"))
                .collect();
            assert_eq!(
                messages,
                vec![
                    "<class 'UserWarning'>: warned here",
                    "<class 'ResourceWarning'>: resource 3",
                ]
            );
        }
    });
}

/// `PyTuple_Pack` stores its items with `PyTuple_SetItem`, which steals a
/// reference, so it has to take a reference of its own first. A tuple which
/// stored the caller's borrowed references would drop them when it is freed,
/// leaving the caller with a dangling reference.
#[test]
fn pytuple_pack_keeps_a_reference_to_its_items() {
    pyo3::ffi::dynamic_loading::load_default().expect("Python should be discoverable");
    pyo3::Python::initialize();

    Python::attach(|py| {
        use pyo3::ffi;
        use pyo3::types::PyList;

        // A single character `str` is immortal in CPython 3.12+ and would hide a
        // missing reference, so use a list.
        let item = PyList::empty(py);
        let refcount = |item: &Bound<'_, PyList>| unsafe {
            // SAFETY: `item` is a live Python object and the GIL is held by `py`.
            ffi::Py_REFCNT(item.as_ptr())
        };
        let before = refcount(&item);

        // Leak the tuple on purpose: without the reference taken by
        // `PyTuple_Pack`, freeing it here would drop a reference which the
        // caller still owns.
        let packed = unsafe {
            // SAFETY: the size argument matches the one item passed, which is a
            // live Python object.
            ffi::PyTuple_Pack(1, item.as_ptr())
        };
        assert!(!packed.is_null());

        assert_eq!(
            refcount(&item),
            before + 1,
            "PyTuple_Pack must keep a reference of its own to each item"
        );
    });
}

/// CPython's `PyObject_Call*` helpers reject a null argument with a
/// `SystemError`; the wrappers have to report the same, because the functions
/// they forward to would dereference the null pointer.
#[test]
fn call_helpers_report_null_arguments_like_cpython() {
    pyo3::ffi::dynamic_loading::load_default().expect("Python should be discoverable");
    pyo3::Python::initialize();

    Python::attach(|py| {
        use pyo3::ffi;
        use pyo3::types::{PyList, PyString};

        let list = PyList::empty(py);
        let method = PyString::new(py, "copy");

        // Each call has to be checked before the next one, because setting an
        // exception replaces the one which is currently set.
        let check = |result: *mut ffi::PyObject| {
            assert!(result.is_null());
            let error = PyErr::fetch(py);
            assert!(error.is_instance_of::<pyo3::exceptions::PySystemError>(py));
            assert_eq!(
                error.value(py).to_string(),
                "null argument to internal routine"
            );
        };

        // SAFETY: each call passes exactly the null argument under test, and
        // every other pointer is a live Python object.
        unsafe {
            check(ffi::PyObject_CallFunction(
                core::ptr::null_mut(),
                c"".as_ptr(),
            ));
            check(ffi::PyObject_CallFunctionObjArgs(
                core::ptr::null_mut(),
                core::ptr::null_mut::<ffi::PyObject>(),
            ));
            check(ffi::PyObject_CallMethod(
                core::ptr::null_mut(),
                c"name".as_ptr(),
                core::ptr::null(),
            ));
            check(ffi::PyObject_CallMethod(
                list.as_ptr(),
                core::ptr::null(),
                core::ptr::null(),
            ));
            check(ffi::PyObject_CallMethodObjArgs(
                core::ptr::null_mut(),
                method.as_ptr(),
                core::ptr::null_mut::<ffi::PyObject>(),
            ));
            check(ffi::PyObject_CallMethodObjArgs(
                list.as_ptr(),
                core::ptr::null_mut(),
                core::ptr::null_mut::<ffi::PyObject>(),
            ));
        }
    });
}

#[test]
fn python_is_loaded_explicitly_at_runtime() {
    // Nothing has touched the Python C API before this call, so this is the
    // point at which the library is opened.
    pyo3::ffi::dynamic_loading::load_default()
        .expect("the Python shared library should be discoverable");

    let loaded = pyo3::ffi::dynamic_loading::loaded_path()
        .expect("a library should have been loaded")
        .to_owned();
    assert!(
        Path::new(&loaded).exists(),
        "loaded library {loaded:?} should exist"
    );

    // An explicit path which cannot be loaded is an error: silently falling back
    // to the search path would defeat choosing the interpreter at runtime.
    let missing = if cfg!(windows) {
        r"C:\pyo3-does-not-exist-9f3a\pyo3-not-a-python-9f3a.dll"
    } else {
        "/pyo3-does-not-exist-9f3a/libpyo3-not-a-python-9f3a.so"
    };
    let error = pyo3::ffi::dynamic_loading::load(missing)
        .expect_err("loading a path which does not exist should fail");
    assert!(
        error.message().contains("9f3a"),
        "unexpected error: {}",
        error.message()
    );
    assert_eq!(
        pyo3::ffi::dynamic_loading::loaded_path().as_deref(),
        Some(loaded.as_str()),
        "a failed load must not change the loaded library"
    );

    // On Windows a module of the same file name which is already loaded is still
    // accepted (that is how a stale path reaches the interpreter which is
    // running this process), and the path which is reported is the real one.
    #[cfg(windows)]
    {
        let name = Path::new(&loaded)
            .file_name()
            .expect("the loaded library should be a file")
            .to_string_lossy()
            .into_owned();
        let stale = format!(r"C:\pyo3-does-not-exist-9f3a\{name}");
        pyo3::ffi::dynamic_loading::load(&stale)
            .expect("the already loaded module should be reused");
        let resolved = pyo3::ffi::dynamic_loading::loaded_path().unwrap();
        assert_eq!(resolved, loaded, "the real path should be reported");
        assert!(Path::new(&resolved).exists());
    }

    pyo3::Python::initialize();

    Python::attach(|py| {
        let answer = py.eval(c"6 * 7", None, None).unwrap();
        assert_eq!(answer.extract::<i32>().unwrap(), 42);

        // Exercise symbols with different shapes: a function
        // (`PyImport_ImportModule`), a type object (`PyDict_Type` through
        // downcasting) and the exception statics (`PyExc_ValueError`).
        let sys = py.import("sys").unwrap();
        let version: String = sys.getattr("version").unwrap().extract().unwrap();
        assert!(version.starts_with("3."), "unexpected version {version:?}");

        let dict = pyo3::types::PyDict::new(py);
        dict.set_item("key", "value").unwrap();
        let as_any: &Bound<'_, PyAny> = dict.as_any();
        let cast = as_any.cast::<pyo3::types::PyDict>().unwrap();
        assert_eq!(cast.len(), 1);
        assert!(cast.is_exact_instance_of::<pyo3::types::PyDict>());

        let error = py.eval(c"1 / 0", None, None).unwrap_err();
        assert!(error.is_instance_of::<pyo3::exceptions::PyZeroDivisionError>(py));
    });
}

/// `load` accepts the library which is already loaded, but it cannot switch to a
/// different one: the symbols which have been resolved keep pointing into the
/// first library, so loading a second one would leave the process with two
/// Pythons.
#[test]
fn load_cannot_replace_the_loaded_library() {
    pyo3::ffi::dynamic_loading::load_default().expect("Python should be discoverable");
    let loaded = pyo3::ffi::dynamic_loading::loaded_path().expect("a library should be loaded");

    // A copy under another file name is a different library as far as the loader
    // is concerned - the file name of a module already in the process is what
    // Windows would reuse, and `dlopen` keys by path - so this must be refused.
    let directory = std::env::temp_dir().join(format!(
        "pyo3-dynamic-loading-replace-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let name = Path::new(&loaded).file_name().unwrap().to_string_lossy();
    let copy = directory.join(format!("copy-{name}"));
    std::fs::copy(&loaded, &copy).unwrap();

    let error = pyo3::ffi::dynamic_loading::load(copy.to_str().unwrap())
        .expect_err("a different library must not replace the loaded one");
    assert!(
        error.message().contains("already loaded"),
        "unexpected error: {}",
        error.message()
    );
    assert_eq!(
        pyo3::ffi::dynamic_loading::loaded_path().as_deref(),
        Some(loaded.as_str()),
        "a refused load must not change the loaded library"
    );

    std::fs::remove_dir_all(&directory).ok();
}

/// A warning message which UTF-8 cannot encode is escaped rather than turned
/// into a `UnicodeEncodeError`: `PyErr_WarnFormat` is documented to warn, and
/// CPython warns for such a message too.
#[test]
fn warn_format_escapes_a_message_utf8_cannot_encode() {
    pyo3::ffi::dynamic_loading::load_default().expect("Python should be discoverable");
    pyo3::Python::initialize();

    Python::attach(|py| {
        use pyo3::ffi;
        use pyo3::types::{PyAnyMethods, PyDict, PyList, PyListMethods};

        let warnings = py.import("warnings").unwrap();
        let kwargs = PyDict::new(py);
        kwargs.set_item("record", true).unwrap();
        let catcher = warnings
            .call_method("catch_warnings", (), Some(&kwargs))
            .unwrap();
        let recorded = catcher.call_method0("__enter__").unwrap();
        warnings.call_method1("simplefilter", ("always",)).unwrap();

        // A lone surrogate, which `PyUnicode_FromFormatV` produces happily and
        // UTF-8 cannot represent.
        let surrogate = py.eval(c"chr(0xdcff)", None, None).unwrap();
        // SAFETY: the format and the argument match the C API, and the argument
        // is a live Python object.
        let result = unsafe {
            ffi::PyErr_WarnFormat(
                ffi::python_static_value!(ffi::PyExc_UserWarning),
                1,
                c"%U".as_ptr(),
                surrogate.as_ptr(),
            )
        };
        assert_eq!(
            result,
            0,
            "PyErr_WarnFormat failed: {}",
            PyErr::take(py).map(|e| e.to_string()).unwrap_or_default()
        );

        catcher
            .call_method1("__exit__", (py.None(), py.None(), py.None()))
            .unwrap();
        let recorded = recorded.cast_into::<PyList>().unwrap();
        assert_eq!(recorded.len(), 1);
        let message = recorded.get_item(0).unwrap().getattr("message").unwrap();
        assert_eq!(
            message.to_string(),
            "\\udcff",
            "the message should be the backslashreplace form"
        );
    });
}

/// `PyErr_ResourceWarning` has no way to pass its `source` through the stable
/// ABI, so the recorded warning has no source - unlike CPython's own function,
/// which records the object it was given. The guide lists this among the
/// differences of the mode; this test keeps the deviation (and its mention
/// there) honest.
#[test]
fn resource_warning_cannot_record_its_source() {
    pyo3::ffi::dynamic_loading::load_default().expect("Python should be discoverable");
    pyo3::Python::initialize();

    Python::attach(|py| {
        use pyo3::ffi;
        use pyo3::types::{PyAnyMethods, PyDict, PyList, PyListMethods};

        let warnings = py.import("warnings").unwrap();
        let kwargs = PyDict::new(py);
        kwargs.set_item("record", true).unwrap();
        let catcher = warnings
            .call_method("catch_warnings", (), Some(&kwargs))
            .unwrap();
        let recorded = catcher.call_method0("__enter__").unwrap();
        warnings.call_method1("simplefilter", ("always",)).unwrap();

        let source = PyList::empty(py);
        // SAFETY: the format and the argument match the C API, and `source` is a
        // live Python object.
        let result = unsafe {
            ffi::PyErr_ResourceWarning(source.as_ptr(), 1, c"resource %d".as_ptr(), 3_i32)
        };
        assert_eq!(result, 0);

        catcher
            .call_method1("__exit__", (py.None(), py.None(), py.None()))
            .unwrap();
        let recorded = recorded.cast_into::<PyList>().unwrap();
        assert_eq!(recorded.len(), 1);
        let warning = recorded.get_item(0).unwrap();
        assert_eq!(
            warning.getattr("message").unwrap().to_string(),
            "resource 3"
        );
        assert!(
            warning.getattr("source").unwrap().is_none(),
            "the source is not passed through the stable ABI"
        );
    });
}
