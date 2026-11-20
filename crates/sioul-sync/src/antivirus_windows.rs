// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The calls to Windows' antivirus, as thin as they can be (docs/client.md,
//! "Antivirus"): each turns what Windows answers into plain numbers and hands
//! them to `antivirus`'s readings (`amsi_answer`, `service_answer`,
//! `security_center`, `through`), which every system compiles and tests.
//!
//! Nothing here is linked when Sioul starts. AMSI (`amsi.dll`, Windows 10
//! onwards) and the Security Center's health (`wscapi.dll`, Windows Vista
//! onwards, no server) are looked up in System32 by their full path when
//! asked: a Windows without them answers "closed" or "unknown", where a
//! function linked the usual way would keep the program from starting
//! (Windows ends a process whose libraries it cannot find). The attachment
//! service and the Security Center's list are COM classes, created when
//! asked: one that is not registered is an error to read, not a failure to
//! start. Each COM call runs on a thread of its own, its apartment opened
//! and closed there, so that it never meets the window's.

use super::{Answer, Door, Presence, Verdict, WINDOWS_DOORS, amsi_answer, security_center, service_answer, through, unread};
use std::ffi::c_void;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{ERROR_SUCCESS, HMODULE, S_OK};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{HKEY, HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryInfoKeyW};
use windows::Win32::System::SecurityCenter::{IWSCProductList, WSC_SECURITY_PROVIDER_ANTIVIRUS, WSCProductList};
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::Win32::UI::Shell::{AttachmentServices, IAttachmentExecute};
use windows::core::{GUID, HRESULT, HSTRING, PCWSTR, s, w};

/// Sioul, as the attachment service knows its client (`SetClientGuid`).
const CLIENT: GUID = GUID::from_u128(0xff04ac7e_3163_436b_aa8f_400954e7cbc0);

/// Checks a file through Windows' doors, newest first, as far as needed.
pub(super) fn scan(file: &Path) -> Verdict {
    let bytes = match std::fs::read(file) {
        Ok(bytes) => bytes,
        Err(e) => return unread(&e),
    };
    let presence = presence();
    let answers = WINDOWS_DOORS.iter().map(|door| match door {
        Door::Amsi => amsi(&bytes, file),
        Door::AttachmentService => attachment_service(&bytes, file),
    });
    through(&presence, answers)
}

/// What Windows Security Center said last, and when.
static KNOWN: Mutex<Option<(Instant, Presence)>> = Mutex::new(None);

/// How long what the Security Center said is kept.
const KEPT: Duration = Duration::from_secs(60);

/// What Windows Security Center says, kept a minute: it may be asked for
/// each attachment and by the window's words.
pub(super) fn presence() -> Presence {
    if let Ok(known) = KNOWN.lock()
        && let Some((when, presence)) = known.as_ref()
        && when.elapsed() < KEPT
    {
        return presence.clone();
    }
    let products = std::thread::spawn(products).join().ok().flatten();
    let health = if products.is_none() { provider_health() } else { None };
    let presence = security_center(products.as_deref(), health);
    if let Ok(mut known) = KNOWN.lock() {
        *known = Some((Instant::now(), presence.clone()));
    }
    presence
}

/// What the Security Center said last, at once, for the window's thread,
/// which COM must not hold up: asked again on a thread of its own when it
/// is a minute old or was never asked, and "unknown" until it answers.
pub(super) fn presence_now() -> Presence {
    let known = KNOWN.lock().ok().and_then(|known| known.clone());
    match known {
        Some((when, presence)) if when.elapsed() < KEPT => presence,
        stale => {
            std::thread::spawn(presence);
            stale.map_or(Presence::Unknown, |(_, presence)| presence)
        }
    }
}

/// The antivirus products Windows Security Center lists, by name and state
/// (Windows 8 onwards); none when the list cannot be read (a server, Windows 7).
fn products() -> Option<Vec<(String, i32)>> {
    // SAFETY: COM opened and closed on this thread, around objects dropped before it closes.
    unsafe {
        let apartment = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let listed = (|| {
            let list: IWSCProductList = CoCreateInstance(&WSCProductList, None, CLSCTX_INPROC_SERVER).ok()?;
            list.Initialize(WSC_SECURITY_PROVIDER_ANTIVIRUS).ok()?;
            let count = u32::try_from(list.Count().ok()?).ok()?;
            let mut products = Vec::new();
            for index in 0..count {
                let product = list.get_Item(index).ok()?;
                let name = product.ProductName().map(|name| name.to_string()).unwrap_or_default();
                // A state not read is not "on".
                let state = product.ProductState().map_or(-1, |state| state.0);
                products.push((name, state));
            }
            Some(products)
        })();
        if apartment.is_ok() {
            CoUninitialize();
        }
        listed
    }
}

/// `WscGetSecurityProviderHealth`, as `wscapi.dll` exports it.
type ProviderHealth = unsafe extern "system" fn(providers: u32, health: *mut i32) -> HRESULT;

/// The health of the Security Center's antivirus category (Windows Vista
/// onwards); none when the function is not there, fails, or the service
/// does not run (S_FALSE, then always "poor").
fn provider_health() -> Option<i32> {
    // SAFETY: the function as Microsoft declares it, from Windows' own library, with a variable that outlives the call.
    unsafe {
        let library = system_library("wscapi.dll")?;
        let found = GetProcAddress(library, s!("WscGetSecurityProviderHealth"))?;
        let health_of = std::mem::transmute::<unsafe extern "system" fn() -> isize, ProviderHealth>(found);
        let mut health = -1;
        let answered = health_of(WSC_SECURITY_PROVIDER_ANTIVIRUS.0.cast_unsigned(), &mut health);
        (answered == S_OK).then_some(health)
    }
}

/// AMSI's functions, as `amsi.dll` exports them.
type AmsiInitialize = unsafe extern "system" fn(app: PCWSTR, context: *mut *mut c_void) -> HRESULT;
type AmsiScanBuffer = unsafe extern "system" fn(context: *mut c_void, buffer: *const c_void, length: u32, name: PCWSTR, session: *mut c_void, result: *mut i32) -> HRESULT;
type AmsiUninitialize = unsafe extern "system" fn(context: *mut c_void);

/// AMSI's three functions this Windows has.
struct Amsi {
    initialize: AmsiInitialize,
    scan_buffer: AmsiScanBuffer,
    uninitialize: AmsiUninitialize,
}

/// AMSI, looked up once, when first asked; none before Windows 10.
fn amsi_here() -> Option<&'static Amsi> {
    static AMSI: OnceLock<Option<Amsi>> = OnceLock::new();
    AMSI.get_or_init(|| {
        // SAFETY: each function cast to the type Microsoft declares for it, from Windows' own library, kept loaded.
        unsafe {
            let library = system_library("amsi.dll")?;
            let initialize = GetProcAddress(library, s!("AmsiInitialize"))?;
            let scan_buffer = GetProcAddress(library, s!("AmsiScanBuffer"))?;
            let uninitialize = GetProcAddress(library, s!("AmsiUninitialize"))?;
            Some(Amsi {
                initialize: std::mem::transmute::<unsafe extern "system" fn() -> isize, AmsiInitialize>(initialize),
                scan_buffer: std::mem::transmute::<unsafe extern "system" fn() -> isize, AmsiScanBuffer>(scan_buffer),
                uninitialize: std::mem::transmute::<unsafe extern "system" fn() -> isize, AmsiUninitialize>(uninitialize),
            })
        }
    })
    .as_ref()
}

/// Whether an antivirus is registered with AMSI: a key under
/// `HKLM\SOFTWARE\Microsoft\AMSI\Providers`, where Microsoft tells an
/// antivirus to enrol. Without one, AMSI's "nothing found" would come from
/// nobody.
fn amsi_providers() -> bool {
    // SAFETY: a key opened, counted and closed, with variables that outlive the calls.
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, w!("SOFTWARE\\Microsoft\\AMSI\\Providers"), None, KEY_READ, &mut key) != ERROR_SUCCESS {
            return false;
        }
        let mut count = 0u32;
        let counted = RegQueryInfoKeyW(key, None, None, None, Some(&mut count), None, None, None, None, None, None, None);
        let _ = RegCloseKey(key);
        counted == ERROR_SUCCESS && count > 0
    }
}

/// AMSI's door: the file's bytes, with its name, handed to the antivirus
/// registered with it, in one scan without a session.
fn amsi(bytes: &[u8], file: &Path) -> Answer {
    let Some(amsi) = amsi_here() else {
        return Answer::Closed("no AMSI on this Windows".into());
    };
    if !amsi_providers() {
        return Answer::Closed("no antivirus registered with AMSI".into());
    }
    let Ok(length) = u32::try_from(bytes.len()) else { return Answer::TooBig };
    let app = HSTRING::from("Sioul");
    let name = HSTRING::from(file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    // SAFETY: AMSI's documented sequence on this thread: a context, one scan of
    // a buffer that outlives the call, the context closed.
    unsafe {
        let mut context: *mut c_void = std::ptr::null_mut();
        let started = (amsi.initialize)(PCWSTR(app.as_ptr()), &mut context);
        if started.is_err() || context.is_null() {
            return amsi_answer(started.0.min(-1), 0);
        }
        let mut result = 0i32;
        let scanned = (amsi.scan_buffer)(context, bytes.as_ptr().cast(), length, PCWSTR(name.as_ptr()), std::ptr::null_mut(), &mut result);
        (amsi.uninitialize)(context);
        amsi_answer(scanned.0, result)
    }
}

/// The attachment service's door: `Save` on the file where it is, as a mail
/// program does, then whether the file is still there with the same bytes.
fn attachment_service(bytes: &[u8], file: &Path) -> Answer {
    let path = file.to_path_buf();
    let saved = std::thread::spawn(move || {
        // SAFETY: COM opened and closed on this thread, around an object dropped before it closes.
        unsafe {
            let apartment = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let saved = (|| {
                let service: IAttachmentExecute = CoCreateInstance(&AttachmentServices, None, CLSCTX_INPROC_SERVER).map_err(|e| format!("no attachment service (0x{:08X})", e.code().0.cast_unsigned()))?;
                let _ = service.SetClientGuid(&CLIENT);
                service.SetLocalPath(&HSTRING::from(path.as_path())).map_err(|e| format!("attachment service 0x{:08X}", e.code().0.cast_unsigned()))?;
                Ok::<i32, String>(service.Save().map_or_else(|e| e.code().0, |()| 0))
            })();
            if apartment.is_ok() {
                CoUninitialize();
            }
            saved
        }
    })
    .join();
    match saved {
        Ok(Ok(code)) => service_answer(code, std::fs::read(file).is_ok_and(|now| now == bytes)),
        Ok(Err(why)) => Answer::Closed(why),
        Err(_) => Answer::Closed("the attachment service stopped".into()),
    }
}

/// A library of Windows' own, by its full path in System32 (never one a
/// folder searched first could plant); none when this Windows lacks it.
/// Kept loaded: its functions are kept.
unsafe fn system_library(name: &str) -> Option<HMODULE> {
    let mut folder = [0u16; 261];
    // SAFETY: the buffer and its length, as the function takes them.
    let length = unsafe { GetSystemDirectoryW(Some(&mut folder)) } as usize;
    if length == 0 || length >= folder.len() {
        return None;
    }
    let path = HSTRING::from(format!("{}\\{name}", String::from_utf16_lossy(&folder[..length])));
    // SAFETY: a path that outlives the call.
    unsafe { LoadLibraryW(&path) }.ok()
}
