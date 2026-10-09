// On x86 Windows, `raw-dylib` with `import_name_type = "undecorated"` removes the
// leading cdecl underscore from function names. This is expected behavior for
// `import_name_type = "undecorated"` (not a rustc bug): it strips the cdecl `_`
// prefix, which collides with symbols whose real names start with `_Py`.
// See https://doc.rust-lang.org/reference/items/external-blocks.html#the-import_name_type-key
//
// That matches ordinary `Py_*` exports, but it breaks CPython's internal `_Py*`
// function exports whose real DLL names already start with an underscore. For
// those functions, ask rustc for one extra underscore so that x86 undecoration
// lands back on CPython's export.
//
// Variables are intentionally excluded here: `import_name_type` does not affect
// variable imports, so `_Py_*` statics continue to work without any rewriting.
#[allow(unused_macros, reason = "used indirectly by extern_libpython_item!")]
macro_rules! extern_libpython_cpython_private_fn {
    ($(#[$attrs:meta])* $vis:vis $name:ident($($args:tt)*) $(-> $ret:ty)?) => {
        #[cfg_attr(
            all(windows, pyo3_use_raw_dylib, target_arch = "x86"),
            link_name = concat!("_", stringify!($name))
        )]
        $(#[$attrs])*
        $vis fn $name($($args)*) $(-> $ret)?;
    };
}

// Keep this list in sync with `_Py*` function imports declared through
// `extern_libpython!`. The x86 workaround only needs to apply to functions:
// statics keep their original names even when `import_name_type` is set. Match
// by name only here so the function signature stays in a single generic arm.
//
// TODO: reduce the number of `_Py*` exports from pyo3-ffi over time — the fewer
// CPython-private functions we expose, the smaller this workaround list becomes.
#[allow(unused_macros, reason = "used indirectly by extern_libpython_item!")]
macro_rules! extern_libpython_maybe_private_fn {
    (
        [_PyObject_CallFunction_SizeT]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyObject_MakeTpCall]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_Py_CheckFunctionResult]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyBytes_Resize]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyLong_AsByteArray]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyLong_FromByteArray]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyUnicode_Ready]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyUnicode_ToDecimalDigit]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyThreadState_UncheckedGet]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyInterpreterState_GetEvalFrameFunc]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyInterpreterState_SetEvalFrameFunc]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyObject_GC_New]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyObject_GC_NewVar]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyObject_GC_Resize]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyObject_New]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyObject_NewVar]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_Py_HashBytes]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_Py_DECREF_DecRefTotal]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_Py_Dealloc]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_Py_DecRef]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_Py_INCREF_IncRefTotal]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_Py_IncRef]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_Py_NegativeRefcount]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [_PyErr_BadInternalCall]
        $(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        extern_libpython_cpython_private_fn! { $(#[$attrs])* $vis $name($($args)*) $(-> $ret)? }
    };
    (
        [$name:ident]
        $(#[$attrs:meta])* $vis:vis fn $fn_name:ident($($args:tt)*) $(-> $ret:ty)?
    ) => {
        $(#[$attrs])*
        $vis fn $fn_name($($args)*) $(-> $ret)?;
    };
}

#[allow(
    unused_macros,
    reason = "only used by the linked (non dynamic-loading) expansion"
)]
macro_rules! extern_libpython_item {
    ($(#[$attrs:meta])* $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?) => {
        extern_libpython_maybe_private_fn! {
            [$name]
            $(#[$attrs])*
            $vis fn $name($($args)*) $(-> $ret)?
        }
    };
    ($(#[$attrs:meta])* $vis:vis static mut $name:ident: $ty:ty) => {
        $(#[$attrs])*
        $vis static mut $name: $ty;
    };
    ($(#[$attrs:meta])* $vis:vis static $name:ident: $ty:ty) => {
        $(#[$attrs])*
        $vis static $name: $ty;
    };
}

#[allow(
    unused_macros,
    reason = "only used by the linked (non dynamic-loading) expansion"
)]
macro_rules! extern_libpython_items {
    () => {};
    (
        $(#[$attrs:meta])*
        $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        extern_libpython_item! {
            $(#[$attrs])*
            $vis fn $name($($args)*) $(-> $ret)?
        }
        extern_libpython_items! { $($rest)* }
    };
    (
        $(#[$attrs:meta])*
        $vis:vis static mut $name:ident: $ty:ty;
        $($rest:tt)*
    ) => {
        extern_libpython_item! {
            $(#[$attrs])*
            $vis static mut $name: $ty
        }
        extern_libpython_items! { $($rest)* }
    };
    (
        $(#[$attrs:meta])*
        $vis:vis static $name:ident: $ty:ty;
        $($rest:tt)*
    ) => {
        extern_libpython_item! {
            $(#[$attrs])*
            $vis static $name: $ty
        }
        extern_libpython_items! { $($rest)* }
    };
}

/// Helper macro to declare `extern` blocks that link against libpython on Windows
/// using `raw-dylib`, eliminating the need for import libraries.
///
/// The build script sets a `pyo3_dll` cfg value to the target DLL name (e.g. `python312`),
/// and this macro expands to the appropriate `#[link(name = "...", kind = "raw-dylib")]`
/// attribute for that DLL.
///
/// # Usage
///
/// ```rust,ignore
/// // Default ABI "C" (most common):
/// extern_libpython! {
///     pub fn PyObject_Call(
///         callable: *mut PyObject,
///         args: *mut PyObject,
///         kwargs: *mut PyObject,
///     ) -> *mut PyObject;
/// }
///
/// // Explicit ABI:
/// extern_libpython! { "C-unwind" {
///     pub fn PyGILState_Ensure() -> PyGILState_STATE;
/// }}
/// ```
macro_rules! extern_libpython {
    // Explicit ABI
    ($abi:literal { $($body:tt)* }) => {
        #[cfg(all(feature = "dynamic-loading", any(windows, unix)))]
        extern_libpython_dynamic_items! { $abi $($body)* }

        #[cfg(not(all(feature = "dynamic-loading", any(windows, unix))))]
        extern_libpython!(@impl $abi { $($body)* }
            // abi3
            "python3", "python3_d",
            // abi3t
            "python3t", "python3t_d",
            // Python 3.9 - 3.15
            "python39", "python39_d",
            "python310", "python310_d",
            "python311", "python311_d",
            "python312", "python312_d",
            "python313", "python313_d",
            "python314", "python314_d",
            "python315", "python315_d",
            "python316", "python316_d",
            // free-threaded builds (3.13+)
            "python313t", "python313t_d",
            "python314t", "python314t_d",
            "python315t", "python315t_d",
            "python316t", "python316t_d",
            // PyPy (DLL is libpypy3.X-c.dll, not pythonXY.dll)
            "libpypy3.11-c",
            "libpypy3.12-c",
            // GraalPy DLL
            "python-native",
        );
    };
    // Internal: generate cfg_attr for each DLL name. One of these will be selected
    // by `pyo3-ffi`'s `build.rs`.
    //
    // On x86 Windows, Python DLLs export undecorated symbol names (no leading
    // underscore), but the default for raw-dylib on x86 is fully-decorated
    // (cdecl adds a `_` prefix). We use `import_name_type = "undecorated"` to
    // match. The `import_name_type` key is only valid on x86, so we need
    // separate cfg_attr arms per architecture.
    (@impl $abi:literal { $($body:tt)* } $($dll:literal),* $(,)?) => {
        $(
            #[cfg_attr(all(windows, pyo3_use_raw_dylib, target_arch = "x86", pyo3_dll = $dll),
                link(name = $dll, kind = "raw-dylib", import_name_type = "undecorated"))]
            #[cfg_attr(all(windows, pyo3_use_raw_dylib, not(target_arch = "x86"), pyo3_dll = $dll),
                link(name = $dll, kind = "raw-dylib"))]
        )*
        #[cfg_attr(all(windows, not(pyo3_use_raw_dylib)), link(name = "pythonXY"))]
        unsafe extern $abi {
            extern_libpython_items! { $($body)* }
        }
    };
    // Default ABI: "C"
    ($($body:tt)*) => {
        extern_libpython!("C" { $($body)* });
    };
}

/// The symbol to resolve for a declaration: its `#[link_name]` when it carries
/// one, and its Rust name otherwise.
///
/// `dynamic-loading` looks symbols up by name, so a declaration which is exported
/// under a different name than its Rust name has to say so in a plain
/// `#[link_name]` attribute, which this macro reads. `cfg_attr` cannot be used
/// for that: the attribute it would produce is not visible here, and would be
/// ignored (with a warning) rather than selecting a symbol.
#[allow(unused_macros, reason = "only used with the dynamic-loading feature")]
macro_rules! extern_libpython_dynamic_symbol {
    ($name:ident) => {
        stringify!($name)
    };
    ($name:ident #[link_name = $link:literal] $($rest:tt)*) => {
        $link
    };
    ($name:ident #[$other:meta] $($rest:tt)*) => {
        extern_libpython_dynamic_symbol! { $name $($rest)* }
    };
}

/// Walks the items of an `extern_libpython!` block and emits runtime-loaded
/// wrappers for each of them.
///
/// Used instead of `extern_libpython_items!` when the `dynamic-loading` feature
/// is enabled: instead of declaring foreign items, every function becomes a
/// trampoline which resolves the symbol on first use, and every data symbol
/// becomes an accessor function of the same name.
///
/// Note that the `link_name` attributes which exist for the PyPy/GraalPy C API
/// name mangling are deliberately ignored (they are `cfg_attr`s, and this mode
/// targets CPython); a plain `#[link_name]` is honoured, see
/// `extern_libpython_dynamic_symbol!`.
#[allow(unused_macros, reason = "only used with the dynamic-loading feature")]
macro_rules! extern_libpython_dynamic_items {
    ($abi:literal) => {};
    // Variadic functions cannot be forwarded through a resolved function
    // pointer, so they are handed to `extern_libpython_dynamic_variadic!`, which
    // forwards their `VaList` to the `va_list` twin of the function or
    // reimplements them on top of the public API.
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis fn $name:ident($($args:tt)*) $(-> $ret:ty)?;
        $($rest:tt)*
    ) => {
        extern_libpython_dynamic_fn! {
            $abi $(#[$attrs])* $vis fn $name($($args)*) $(-> $ret)?
        }
        extern_libpython_dynamic_items! { $abi $($rest)* }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis static mut $name:ident : PyTypeObject;
        $($rest:tt)*
    ) => {
        extern_libpython_dynamic_static_object! { $(#[$attrs])* $vis $name : PyTypeObject }
        extern_libpython_dynamic_items! { $abi $($rest)* }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis static mut $name:ident : crate::PyTypeObject;
        $($rest:tt)*
    ) => {
        extern_libpython_dynamic_static_object! { $(#[$attrs])* $vis $name : crate::PyTypeObject }
        extern_libpython_dynamic_items! { $abi $($rest)* }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis static mut $name:ident : PyLongObject;
        $($rest:tt)*
    ) => {
        extern_libpython_dynamic_static_object! { $(#[$attrs])* $vis $name : PyLongObject }
        extern_libpython_dynamic_items! { $abi $($rest)* }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis static mut $name:ident : PyObject;
        $($rest:tt)*
    ) => {
        extern_libpython_dynamic_static_object! { $(#[$attrs])* $vis $name : PyObject }
        extern_libpython_dynamic_items! { $abi $($rest)* }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis static $name:ident : PyTypeObject;
        $($rest:tt)*
    ) => {
        extern_libpython_dynamic_static_object! { $(#[$attrs])* $vis $name : PyTypeObject }
        extern_libpython_dynamic_items! { $abi $($rest)* }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis static $name:ident : crate::PyTypeObject;
        $($rest:tt)*
    ) => {
        extern_libpython_dynamic_static_object! { $(#[$attrs])* $vis $name : crate::PyTypeObject }
        extern_libpython_dynamic_items! { $abi $($rest)* }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis static $name:ident : PyLongObject;
        $($rest:tt)*
    ) => {
        extern_libpython_dynamic_static_object! { $(#[$attrs])* $vis $name : PyLongObject }
        extern_libpython_dynamic_items! { $abi $($rest)* }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis static $name:ident : PyObject;
        $($rest:tt)*
    ) => {
        extern_libpython_dynamic_static_object! { $(#[$attrs])* $vis $name : PyObject }
        extern_libpython_dynamic_items! { $abi $($rest)* }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis static mut $name:ident : $ty:ty;
        $($rest:tt)*
    ) => {
        extern_libpython_dynamic_static_value! { $(#[$attrs])* $vis $name : $ty }
        extern_libpython_dynamic_items! { $abi $($rest)* }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis static $name:ident : $ty:ty;
        $($rest:tt)*
    ) => {
        extern_libpython_dynamic_static_value! { $(#[$attrs])* $vis $name : $ty }
        extern_libpython_dynamic_items! { $abi $($rest)* }
    };
}

/// Emits the wrapper for a variadic function of the Python API, or keeps it
/// declared-only when there is nothing better to do.
///
/// The wrapper is emitted in place of the declaration, so that it inherits the
/// attributes of the declaration (such as `#[cfg]`) and its exact parameter list:
/// a resolved symbol cannot be described by a single signature which works for
/// every variadic function, so each one is either forwarded to its
/// `va_list`-taking twin or handed to a helper which knows its argument list.
#[allow(unused_macros, reason = "only used with the dynamic-loading feature")]
macro_rules! extern_libpython_dynamic_variadic {
    ([$name:ident] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_spec! { [$name] $($rest)* }
    };
}

/// The list of variadic functions which can be wrapped, and how.
#[allow(unused_macros, reason = "only used with the dynamic-loading feature")]
macro_rules! extern_libpython_dynamic_variadic_spec {
    ([PyErr_Format] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_forward! { $($rest)* => "PyErr_FormatV" }
    };
    ([Py_BuildValue] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_forward! { $($rest)* => "Py_VaBuildValue" }
    };
    ([PyUnicode_FromFormat] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_forward! { $($rest)* => "PyUnicode_FromFormatV" }
    };
    ([PyBytes_FromFormat] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_forward! { $($rest)* => "PyBytes_FromFormatV" }
    };
    ([PyArg_Parse] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_forward! { $($rest)* => "PyArg_VaParse" }
    };
    ([PyArg_ParseTuple] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_forward! { $($rest)* => "PyArg_VaParse" }
    };
    ([PyArg_ParseTupleAndKeywords] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_forward! { $($rest)* => "PyArg_VaParseTupleAndKeywords" }
    };
    ([PyObject_CallFunction] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::call_with_format
        }
    };
    ([PyEval_CallFunction] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::call_with_format
        }
    };
    ([PyObject_CallMethod] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::call_method_with_format
        }
    };
    ([PyEval_CallMethod] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::call_method_with_format
        }
    };
    ([PyObject_CallFunctionObjArgs] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::call_function_obj_args
        }
    };
    ([PyObject_CallMethodObjArgs] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::call_method_obj_args
        }
    };
    ([PyTuple_Pack] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::tuple_pack
        }
    };
    ([PyArg_UnpackTuple] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::unpack_tuple
        }
    };
    ([PyErr_WarnFormat] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::warn_format
        }
    };
    ([PyErr_ResourceWarning] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::resource_warning
        }
    };
    ([PySys_WriteStdout] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::sys_write_stdout
        }
    };
    ([PySys_WriteStderr] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::sys_write_stderr
        }
    };
    ([PySys_FormatStdout] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::sys_format_stdout
        }
    };
    ([PySys_FormatStderr] $($rest:tt)*) => {
        extern_libpython_dynamic_variadic_helper! {
            $($rest)* => $crate::impl_::dynamic_variadics::sys_format_stderr
        }
    };
    // A variadic function which is not listed above cannot be forwarded through
    // a function pointer which is resolved at run time: the trampoline cannot
    // read an argument list it does not understand. Declaring it anyway would
    // silently restore the link-time dependency on libpython which this feature
    // exists to remove, so a declaration which is not wrapped above is reported
    // when it is added rather than when someone calls it.
    (
        [$name:ident] $abi:literal $(#[$attrs:meta])* $vis:vis fn $fn_name:ident(
            $($(#[$arg_attrs:meta])* $arg:ident : $ty:ty),+
        ) $(-> $ret:ty)?
    ) => {
        // The attributes have to be repeated here: they are what keeps a
        // declaration which does not exist for this interpreter (a PyPy-only
        // `_Py*` entry, for example) from being reported.
        $(#[$attrs])*
        compile_error!(concat!(
            "`dynamic-loading` has no wrapper for the variadic function `",
            stringify!($fn_name),
            "`: add it to `extern_libpython_dynamic_variadic_spec!`, forwarding its `va_list` \
             to the matching `va_list` function of the Python API or to a helper in \
             `impl_::dynamic_variadics`"
        ));
    };
}

/// Emits a wrapper which forwards its `VaList` to the `va_list`-taking twin of
/// the variadic function.
#[allow(unused_macros, reason = "only used with the dynamic-loading feature")]
macro_rules! extern_libpython_dynamic_variadic_forward {
    (
        $abi:literal $(#[$attrs:meta])* $vis:vis fn $name:ident(
            $($(#[$arg_attrs:meta])* $arg:ident : $ty:ty),+
        ) $(-> $ret:ty)? => $target:literal
    ) => {
        $(#[$attrs])*
        // `VaList`, and defining C variadic functions, is stable since Rust 1.99;
        // `pyo3-ffi`'s build script rejects the feature on older compilers.
        #[allow(
            unused_attributes,
            improper_ctypes_definitions,
            clippy::incompatible_msrv,
            clippy::missing_safety_doc,
            clippy::too_many_arguments
        )]
        $vis unsafe extern $abi fn $name($($(#[$arg_attrs])* $arg : $ty,)+ varargs: ...)
            $(-> $ret)?
        {
            static CACHE: ::core::sync::atomic::AtomicPtr<::core::ffi::c_void> =
                ::core::sync::atomic::AtomicPtr::new(::core::ptr::null_mut());
            // The parameter attributes matter here: a declaration may name the
            // same parameter twice behind mutually exclusive cfgs.
            type Function =
                for<'f> unsafe extern $abi fn(
                    $($(#[$arg_attrs])* $ty,)* ::core::ffi::VaList<'f>
                ) $(-> $ret)?;
            // SAFETY: the signature is copied from the foreign declaration in
            // this same block, so the pointer type matches the real symbol.
            let function: Function = unsafe {
                $crate::impl_::dynamic_loading::resolve_fn(&CACHE, $target)
            };
            // SAFETY: the caller provided arguments matching `$target`'s format,
            // exactly as if it had called the variadic function directly.
            unsafe { function($($(#[$arg_attrs])* $arg,)* varargs) }
        }
    };
}

/// Emits a wrapper which hands its `VaList` to a helper implementing the
/// function on top of the public API (see `impl_::dynamic_variadics`).
#[allow(unused_macros, reason = "only used with the dynamic-loading feature")]
macro_rules! extern_libpython_dynamic_variadic_helper {
    (
        $abi:literal $(#[$attrs:meta])* $vis:vis fn $name:ident(
            $($(#[$arg_attrs:meta])* $arg:ident : $ty:ty),+
        ) $(-> $ret:ty)? => $helper:path
    ) => {
        $(#[$attrs])*
        // `VaList`, and defining C variadic functions, is stable since Rust 1.99;
        // `pyo3-ffi`'s build script rejects the feature on older compilers.
        #[allow(
            unused_attributes,
            improper_ctypes_definitions,
            clippy::incompatible_msrv,
            clippy::missing_safety_doc,
            clippy::too_many_arguments
        )]
        $vis unsafe extern $abi fn $name($($(#[$arg_attrs])* $arg : $ty,)+ varargs: ...)
            $(-> $ret)?
        {
            // SAFETY: as with the C API: the caller must have passed arguments
            // matching the function's format or argument list.
            unsafe { $helper($($(#[$arg_attrs])* $arg,)* varargs) }
        }
    };
}

/// Emits the runtime-loaded trampoline for a single Python C API function.
///
/// Variadic functions are handled by `extern_libpython_dynamic_variadic!`, which
/// either wraps them or leaves them declared (see `impl_::dynamic_variadics`).
#[allow(unused_macros, reason = "only used with the dynamic-loading feature")]
macro_rules! extern_libpython_dynamic_fn {
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis fn $name:ident($($(#[$arg_attrs:meta])* $arg:ident : $ty:ty),+ , ...)
        $(-> $ret:ty)?
    ) => {
        // Defining C variadic functions needs Rust 1.99, which `pyo3-ffi`'s build
        // script enforces when the `dynamic-loading` feature is enabled, so every
        // variadic function is either wrapped by
        // `extern_libpython_dynamic_variadic!` or stays declared-only.
        extern_libpython_dynamic_variadic! {
            [$name] $abi $(#[$attrs])* $vis fn $name($($(#[$arg_attrs])* $arg : $ty),+) $(-> $ret)?
        }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis fn $name:ident($($(#[$arg_attrs:meta])* $arg:ident : $ty:ty),* $(,)?) -> $ret:ty
    ) => {
        $(#[$attrs])*
        #[allow(
            unused_attributes,
            improper_ctypes_definitions,
            clippy::missing_safety_doc,
            clippy::too_many_arguments,
            clippy::missing_transmute_annotations
        )]
        $vis unsafe extern $abi fn $name($($(#[$arg_attrs])* $arg: $ty),*) -> $ret {
            static CACHE: ::core::sync::atomic::AtomicPtr<::core::ffi::c_void> =
                ::core::sync::atomic::AtomicPtr::new(::core::ptr::null_mut());
            let symbol: &'static str =
                extern_libpython_dynamic_symbol! { $name $(#[$attrs])* };
            // SAFETY: the signature is copied from the foreign declaration in
            // this same block, so the pointer type matches the real symbol.
            // The parameter attributes matter here: a declaration may name the
            // same parameter twice behind mutually exclusive cfgs.
            let function: unsafe extern $abi fn($($(#[$arg_attrs])* $ty),*) -> $ret = unsafe {
                $crate::impl_::dynamic_loading::resolve_fn(&CACHE, symbol)
            };
            // SAFETY: upheld by the caller, as with any Python C API function.
            unsafe { function($($(#[$arg_attrs])* $arg),*) }
        }
    };
    (
        $abi:literal
        $(#[$attrs:meta])*
        $vis:vis fn $name:ident($($(#[$arg_attrs:meta])* $arg:ident : $ty:ty),* $(,)?)
    ) => {
        $(#[$attrs])*
        #[allow(
            unused_attributes,
            improper_ctypes_definitions,
            clippy::missing_safety_doc,
            clippy::too_many_arguments,
            clippy::missing_transmute_annotations
        )]
        $vis unsafe extern $abi fn $name($($(#[$arg_attrs])* $arg: $ty),*) {
            static CACHE: ::core::sync::atomic::AtomicPtr<::core::ffi::c_void> =
                ::core::sync::atomic::AtomicPtr::new(::core::ptr::null_mut());
            let symbol: &'static str =
                extern_libpython_dynamic_symbol! { $name $(#[$attrs])* };
            // SAFETY: as above.
            let function: unsafe extern $abi fn($($(#[$arg_attrs])* $ty),*) = unsafe {
                $crate::impl_::dynamic_loading::resolve_fn(&CACHE, symbol)
            };
            // SAFETY: upheld by the caller, as with any Python C API function.
            unsafe { function($($(#[$arg_attrs])* $arg),*) }
        }
    };
}

/// Emits the accessor for a Python data symbol which is a *variable* holding a
/// value (a pointer such as `PyExc_ValueError`, an integer such as
/// `Py_VerboseFlag`, or an array).
///
/// Symbols whose address *is* an object (`PyDict_Type`, `_Py_NoneStruct`, ...)
/// use [`extern_libpython_dynamic_static_object!`] instead; which of the two
/// applies is decided while matching the raw declaration tokens, because a
/// `ty` fragment cannot be matched against a literal type name.
#[allow(unused_macros, reason = "only used with the dynamic-loading feature")]
macro_rules! extern_libpython_dynamic_static_value {
    ($(#[$attrs:meta])* $vis:vis $name:ident : $ty:ty) => {
        $(#[$attrs])*
        #[allow(unused_attributes, clippy::missing_safety_doc)]
        $vis unsafe fn $name() -> $ty {
            static CACHE: ::core::sync::atomic::AtomicPtr<::core::ffi::c_void> =
                ::core::sync::atomic::AtomicPtr::new(::core::ptr::null_mut());
            let symbol: &'static str =
                extern_libpython_dynamic_symbol! { $name $(#[$attrs])* };
            // SAFETY: `$name` is declared as a variable of type `$ty` in the C API.
            unsafe { *$crate::impl_::dynamic_loading::resolve_data::<$ty>(&CACHE, symbol) }
        }
    };
}

/// Emits the accessor for a Python data symbol whose address *is* the object.
#[allow(unused_macros, reason = "only used with the dynamic-loading feature")]
macro_rules! extern_libpython_dynamic_static_object {
    ($(#[$attrs:meta])* $vis:vis $name:ident : $ty:ty) => {
        $(#[$attrs])*
        #[allow(unused_attributes, clippy::missing_safety_doc)]
        $vis unsafe fn $name() -> *mut $ty {
            static CACHE: ::core::sync::atomic::AtomicPtr<::core::ffi::c_void> =
                ::core::sync::atomic::AtomicPtr::new(::core::ptr::null_mut());
            let symbol: &'static str =
                extern_libpython_dynamic_symbol! { $name $(#[$attrs])* };
            // SAFETY: the symbol *is* the Python object.
            unsafe { $crate::impl_::dynamic_loading::resolve_data::<$ty>(&CACHE, symbol) }
        }
    };
}

/// Reference to a Python object data symbol (`PyDict_Type`, `_Py_NoneStruct`,
/// ...) which works both when
/// libpython is linked (the static itself) and when it is loaded at runtime (the
/// accessor function generated by `extern_libpython_dynamic_static!`).
///
/// Yields a pointer to the object (`*mut PyTypeObject`, `*mut PyObject`, ...).
#[macro_export]
#[cfg(not(all(feature = "dynamic-loading", any(windows, unix))))]
macro_rules! python_static_object {
    ($($path:tt)+) => { &raw mut $($path)+ };
}

#[macro_export]
#[cfg(all(feature = "dynamic-loading", any(windows, unix)))]
macro_rules! python_static_object {
    ($($path:tt)+) => { $($path)+() };
}

/// Reference to any other Python data symbol, working in both link modes.
///
/// Yields the value of the symbol (`*mut PyObject` for `PyExc_*`, `c_int` for
/// `Py_VerboseFlag`, ...).
#[macro_export]
#[cfg(not(all(feature = "dynamic-loading", any(windows, unix))))]
macro_rules! python_static_value {
    ($($path:tt)+) => { $($path)+ };
}

#[macro_export]
#[cfg(all(feature = "dynamic-loading", any(windows, unix)))]
macro_rules! python_static_value {
    ($($path:tt)+) => { $($path)+() };
}

#[cfg(test)]
mod tests {
    /// A declaration which is exported under another name has to say so with a
    /// plain `#[link_name]`, which is what the `dynamic-loading` wrappers read:
    /// the `cfg_attr` the linked build uses cannot be evaluated while a wrapper
    /// is generated, and would be silently ignored (the attribute is unused on a
    /// Rust function). `PyUnstable_Code_GetExtra` is the case in point: before
    /// Python 3.12 the shared library only exports `_PyCode_GetExtra`.
    #[test]
    fn a_link_name_attribute_selects_the_symbol() {
        assert_eq!(
            extern_libpython_dynamic_symbol! {
                PyUnstable_Code_GetExtra
                #[cfg(not(Py_3_12))]
                #[link_name = "_PyCode_GetExtra"]
            },
            "_PyCode_GetExtra"
        );
        assert_eq!(
            extern_libpython_dynamic_symbol! { PyUnstable_Code_GetExtra },
            "PyUnstable_Code_GetExtra"
        );
    }
}
