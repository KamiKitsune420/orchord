//! Sends the whole keyboard to the Mac while it is "captured".
//!
//! A low-level keyboard hook sees every key before Windows, the focused window or a screen
//! reader acts on it. While captured, each key is swallowed and forwarded to the Mac as a
//! scancode, so the Windows key (Command) and screen-reader keys such as Insert and Caps Lock
//! reach macOS and VoiceOver. Windows itself never passes Control+Alt+Delete or Windows+L
//! to a hook.
//!
//! The keyboard is released by pressing Control and Alt together and letting go without
//! touching another key. Control+Alt with another key is left alone, because Control+Option
//! is VoiceOver's modifier.

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::Sender;

use orchord_core::VmCommand;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

const VK_LCONTROL: u32 = 0xA2;
const VK_RCONTROL: u32 = 0xA3;
const VK_LMENU: u32 = 0xA4;
const VK_RMENU: u32 = 0xA5;

/// Scancodes of every modifier, sent as key-ups on release so none stays stuck down in the Mac:
/// left and right Control, Alt, Shift and Windows.
const MODIFIER_SCANCODES: [u8; 8] = [0x1D, 0x9D, 0x38, 0xB8, 0x2A, 0x36, 0xDB, 0xDC];

static HOOK: AtomicIsize = AtomicIsize::new(0);
static OWNER_WINDOW: AtomicIsize = AtomicIsize::new(0);
static TARGET: Mutex<Option<Sender<VmCommand>>> = Mutex::new(None);
static CONTROL_DOWN: AtomicBool = AtomicBool::new(false);
static ALT_DOWN: AtomicBool = AtomicBool::new(false);
/// True from the moment Control and Alt are both down until any other key is pressed.
static RELEASE_ARMED: AtomicBool = AtomicBool::new(false);
static RELEASE_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn is_captured() -> bool {
    HOOK.load(Ordering::SeqCst) != 0
}

/// Starts sending the keyboard to the Mac behind `target`. `owner_window` is the native handle
/// of the window that must stay in front; if another window comes forward the keyboard is let go.
pub fn capture(owner_window: *mut c_void, target: Sender<VmCommand>) -> bool {
    if is_captured() {
        return true;
    }
    *TARGET.lock().unwrap() = Some(target);
    OWNER_WINDOW.store(owner_window as isize, Ordering::SeqCst);
    CONTROL_DOWN.store(false, Ordering::SeqCst);
    ALT_DOWN.store(false, Ordering::SeqCst);
    RELEASE_ARMED.store(false, Ordering::SeqCst);
    RELEASE_REQUESTED.store(false, Ordering::SeqCst);
    // SAFETY: the hook procedure is a plain function that lives for the whole program, and
    // the hook is removed in `release` before the program's message loop ends.
    let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_procedure), GetModuleHandleW(std::ptr::null()), 0) };
    HOOK.store(hook as isize, Ordering::SeqCst);
    !hook.is_null()
}

/// Gives the keyboard back to Windows.
pub fn release() {
    let hook = HOOK.swap(0, Ordering::SeqCst);
    if hook != 0 {
        // SAFETY: `hook` is the handle SetWindowsHookExW returned in `capture`.
        unsafe { UnhookWindowsHookEx(hook as *mut c_void) };
    }
    if let Some(target) = TARGET.lock().unwrap().take() {
        for scancode in MODIFIER_SCANCODES {
            let _ = target.send(VmCommand::Key { scancode, down: false });
        }
    }
}

/// True once if the user made the release gesture, or another window took the foreground,
/// since the last call. The window polls this and then calls `release`.
pub fn take_release_request() -> bool {
    RELEASE_REQUESTED.swap(false, Ordering::SeqCst)
}

fn forward(scancode: u8, down: bool) {
    if let Ok(target) = TARGET.lock() {
        if let Some(target) = target.as_ref() {
            let _ = target.send(VmCommand::Key { scancode, down });
        }
    }
}

unsafe extern "system" fn hook_procedure(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let pass_on = || unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) };
    if code != HC_ACTION as i32 || RELEASE_REQUESTED.load(Ordering::SeqCst) {
        return pass_on();
    }
    // SAFETY: for HC_ACTION, Windows passes a valid KBDLLHOOKSTRUCT in lparam.
    let key = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
    if unsafe { GetForegroundWindow() } as isize != OWNER_WINDOW.load(Ordering::SeqCst) || key.scanCode == 0 || key.scanCode > 0x7F {
        if key.scanCode != 0 && key.scanCode <= 0x7F {
            RELEASE_REQUESTED.store(true, Ordering::SeqCst);
        }
        return pass_on();
    }

    let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
    let is_control = matches!(key.vkCode, VK_LCONTROL | VK_RCONTROL);
    let is_alt = matches!(key.vkCode, VK_LMENU | VK_RMENU);
    let extended = key.flags & LLKHF_EXTENDED != 0;
    let scancode = key.scanCode as u8 | if extended { 0x80 } else { 0 };

    if is_control || is_alt {
        let was_armed = RELEASE_ARMED.load(Ordering::SeqCst);
        if is_control {
            CONTROL_DOWN.store(down, Ordering::SeqCst);
        } else {
            ALT_DOWN.store(down, Ordering::SeqCst);
        }
        if down {
            if CONTROL_DOWN.load(Ordering::SeqCst) && ALT_DOWN.load(Ordering::SeqCst) && !was_armed {
                RELEASE_ARMED.store(true, Ordering::SeqCst);
            }
        } else if was_armed {
            // Control and Alt were pressed and let go by themselves: that is the release gesture.
            RELEASE_ARMED.store(false, Ordering::SeqCst);
            forward(scancode, false);
            RELEASE_REQUESTED.store(true, Ordering::SeqCst);
            return 1;
        }
    } else if down {
        // Another key was used with Control+Alt, so this is a VoiceOver command, not a release.
        RELEASE_ARMED.store(false, Ordering::SeqCst);
    }

    forward(scancode, down);
    1
}
