#[cfg(all(Py_GIL_DISABLED, not(Py_LIMITED_API)))]
mod atomic_c_ulong {
    pub struct GetAtomicCULong<const WIDTH: usize>();

    pub trait AtomicCULongType {
        type Type;
    }
    impl AtomicCULongType for GetAtomicCULong<32> {
        type Type = core::sync::atomic::AtomicU32;
    }
    impl AtomicCULongType for GetAtomicCULong<64> {
        type Type = core::sync::atomic::AtomicU64;
    }

    pub type TYPE =
        <GetAtomicCULong<{ core::mem::size_of::<core::ffi::c_ulong>() * 8 }> as AtomicCULongType>::Type;
}

/// Explicit runtime loading of the Python shared library.
#[cfg(all(feature = "dynamic-loading", any(windows, unix)))]
pub mod dynamic_loading;

/// Implementation of the C variadic functions of the Python API.
#[cfg(all(feature = "dynamic-loading", any(windows, unix)))]
pub(crate) mod dynamic_variadics;

/// Typedef for an atomic integer to match the platform-dependent c_ulong type.
#[cfg(all(Py_GIL_DISABLED, not(Py_LIMITED_API)))]
#[doc(hidden)]
pub type AtomicCULong = atomic_c_ulong::TYPE;

/// Guard to hang the current thread indefinitely when dropped.
#[cfg(not(any(Py_3_14, target_arch = "wasm32")))]
pub struct HangThread;

#[cfg(not(any(Py_3_14, target_arch = "wasm32")))]
impl Drop for HangThread {
    fn drop(&mut self) {
        loop {
            std::thread::park(); // Block forever.
        }
    }
}
