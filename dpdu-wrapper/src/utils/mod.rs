pub mod can;
pub mod module_description;
pub mod root_file;

use crate::types::PduUniqueRespIdentifier;
use cfg_if::cfg_if;
use rand::RngExt;
use std::ffi::{CStr, c_char, c_void};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Cursor, Read, Seek};
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::ptr;
use std::ptr::NonNull;
use winreg::RegKey;
use winreg::enums::HKEY_LOCAL_MACHINE;

/// Converts a nullable C string to `Option<String>`.
///
/// Returns `None` for null pointers and empty strings.
pub(crate) fn c_str(ptr: *const c_char) -> Option<String> {
    NonNull::new(ptr as _)
        .map(|wrapped_ptr| unsafe {
            CStr::from_ptr(wrapped_ptr.as_ptr())
                .to_string_lossy()
                .into_owned()
        })
        .filter(|s| !s.is_empty())
}

/// FileReader that skips the BOM header.
pub(crate) fn get_bomless_file_reader(path: &Path) -> Result<BufReader<File>, std::io::Error> {
    let file = OpenOptions::new().read(true).open(path)?;
    let mut reader = BufReader::new(file);

    let mut bom_header = [0u8; 3];
    reader.read_exact(&mut bom_header)?;

    if !bom_header.starts_with(&[239, 187, 191]) {
        reader.rewind()?;
    }

    Ok(reader)
}

/// A zero-sized marker type that ties a value to a lifetime `'a`
/// without holding an actual reference.
///
/// `PhantomRef` is used to express that `T` is logically bound to
/// a lifetime, typically for FFI or raw pointer wrappers, while not
/// storing any real reference.
///
/// This does **not** provide any runtime guarantees about validity
/// of pointers or memory safety. It only enforces constraints at
/// compile time.
#[repr(C)]
pub(crate) struct PhantomRef<'a, T> {
    pub data: T,
    _marker: PhantomData<&'a ()>,
}

impl<'a, T> PhantomRef<'a, T> {
    pub fn new(data: T) -> PhantomRef<'a, T> {
        Self {
            data,
            _marker: PhantomData,
        }
    }

    pub fn as_ptr(&self) -> *const T {
        &self.data
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        &mut self.data
    }
}

impl<'a, T> Deref for PhantomRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[derive(Debug)]
pub struct UnsafePtr<T>(pub *const T);

unsafe impl<T> Send for UnsafePtr<T> {}
unsafe impl<T> Sync for UnsafePtr<T> {}

/// A lifetime-bound opaque pointer (`*const c_void`).
///
/// Does not own the pointed-to value.
#[derive(Debug)]
pub(crate) struct PhantomPtr<'a> {
    pub ptr: *const c_void,
    _marker: PhantomData<&'a ()>,
}

impl<'a> PhantomPtr<'a> {
    pub fn new(ptr: *const c_void) -> Self {
        Self {
            ptr,
            _marker: PhantomData,
        }
    }

    /// Returns the pointer as `*const c_void`.
    pub fn as_ptr(&self) -> *const c_void {
        self.ptr
    }

    /// Returns the pointer as `*mut c_void`.
    ///
    /// This only casts the pointer type and does not guarantee mutability
    /// of the underlying data.
    pub fn as_mut_ptr(&self) -> *mut c_void {
        self.ptr as _
    }
}

/// Marks the wrapped value as `Send` and `Sync`.
///
/// # Safety
///
/// The caller must ensure that sharing or transferring `T` across threads
/// is sound. Wrapping a non-thread-safe type in `SendSync` can cause
/// undefined behavior.
#[derive(Debug, Clone)]
pub(crate) struct SendSync<T>(pub T);

unsafe impl<T> Send for SendSync<T> {}
unsafe impl<T> Sync for SendSync<T> {}

impl<T> Deref for SendSync<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for SendSync<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// When calling [`PDUSetUniqueRespIdTable`], the caller must provide a unique 32-bit identifier
/// that is used to identify subsequent requests to and responses from the ECU.
///
/// This function converts block names (for example, `EZS213`) into a 32-bit identifier.
/// The conversion is performed by hashing the string using the MurmurHash3 algorithm,
/// which provides a low probability of collisions compared to many other hashing algorithms.
pub fn ecu_name_to_unique_resp_id<S>(name: S) -> PduUniqueRespIdentifier
where
    S: AsRef<str>,
{
    murmur3::murmur3_32(&mut Cursor::new(name.as_ref()), 0).expect("murmur failed")
}

pub(crate) fn random_non_zero_usize() -> NonZeroUsize {
    NonZeroUsize::new(rand::rng().random_range(1..=usize::MAX))
        .expect("internal error: random_range(1..=usize::MAX) cannot return zero")
}

pub fn take_slice_ptr<T>(slice: &[T]) -> *mut T {
    if slice.is_empty() {
        ptr::null_mut()
    } else {
        slice.as_ptr() as _
    }
}

/// Wrapper that prevents cloning of the contained value.
///
/// The inner value can be extracted or accessed, but `NonClonable<T>` itself
/// does not implement [`Clone`], even if `T` does.
#[derive(Debug)]
pub struct NonClonable<T>(pub(crate) T);

impl<T> NonClonable<T> {
    /// Creates a new non-clonable wrapper.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Returns the wrapped value.
    pub fn into_inner(self) -> T {
        self.0
    }

    /// Returns a shared reference to the wrapped value.
    pub const fn get_ref(&self) -> &T {
        &self.0
    }

    /// Returns a mutable reference to the wrapped value.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

pub const HKEY_LM_REG_KEY: RegKey = RegKey::predef(HKEY_LOCAL_MACHINE);

pub const fn get_winreg_arch_flags() -> u32 {
    use winreg::enums;

    cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            enums::KEY_READ
        } else if #[cfg(target_arch = "x86")] {
            enums::KEY_READ | enums::KEY_WOW64_32KEY
        } else {
            compile_error!("Unsupported target architecture");
        }
    }
}
