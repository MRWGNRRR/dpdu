use neohook::detour_helper;
use parking_lot::RwLock;
use std::collections::HashSet;
use std::ffi::{CStr, c_void};
use std::sync::{LazyLock, OnceLock};
use tracing::error;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows::core::{PCSTR, s};

static CALLBACKS: LazyLock<RwLock<HashSet<MessageBoxACallback>>> =
    LazyLock::new(|| RwLock::default());

type MsgBoxAFn =
    unsafe extern "system" fn(hwnd: HWND, text: PCSTR, caption: PCSTR, style: u32) -> i32;

type MessageBoxACallback = fn(text: &str) -> bool;

unsafe extern "system" fn hooked_msg_box_a(
    hwnd: HWND,
    text: PCSTR,
    caption: PCSTR,
    style: u32,
) -> i32 {
    let trampoline = TRAMPOLINE
        .get()
        .expect("internal error: MessageBoxA detour is not initialized");

    let call_trampoline = || -> i32 { unsafe { trampoline(hwnd, text, caption, style) } };

    if !text.is_null() {
        let Ok(text) = unsafe { CStr::from_ptr(text.as_ptr() as _) }.to_str() else {
            return call_trampoline();
        };

        let callbacks = CALLBACKS.read();
        for callback in callbacks.iter() {
            if callback(text) {
                return 1;
            }
        }
    }

    call_trampoline()
}

static TRAMPOLINE: OnceLock<MsgBoxAFn> = OnceLock::new();

static DETOUR: LazyLock<Result<(), String>> = LazyLock::new(|| unsafe {
    let user32dll = LoadLibraryA(s!("user32.dll"))
        .map_err(|e| format!("LoadLibraryA(user32.dll) error: {}", e.message()))?;

    let fn_ptr = GetProcAddress(user32dll, s!("MessageBoxA"))
        .map(|v| v as *const c_void)
        .ok_or_else(|| "unable to take a pointer to the MessageBoxA() function".to_string())?;

    let hook_result = detour_helper!(TRAMPOLINE, fn_ptr, hooked_msg_box_a, MsgBoxAFn)
        .map_err(|e| format!("detour_helper!(...): {}", e.to_string()))?;

    Box::leak(Box::new(hook_result));

    Ok(())
});

pub(crate) fn hook_message_box_a() -> bool {
    match DETOUR.as_ref() {
        Ok(_) => true,
        Err(err) => {
            error!("MessageBoxA hook error: {err}");
            false
        }
    }
}

pub(crate) fn register_callback(callback: MessageBoxACallback) {
    let mut callbacks = CALLBACKS.write();
    callbacks.insert(callback);
}
