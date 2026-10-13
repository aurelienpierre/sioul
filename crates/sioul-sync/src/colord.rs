// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! colord's word on the screens, for the sites in the screen's own colours
//! (sioul-app's colour.rs and cpp/screencolours.cpp; docs/colour.md, "Which
//! atom is which screen"): the checksums of the profiles colord keeps for an
//! output. Read only, over the system bus, on a connection made by
//! `crate::bus` (Qt's own reached the bus through libdbus's registration and
//! its process-wide locks: review of battery part C, G1), each question
//! waited for 400 ms at most, as before.

use dbus::Path;
use dbus::blocking::LocalConnection;
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use std::collections::HashMap;
use std::time::Duration;

/// colord, on the system bus.
const SERVICE: &str = "org.freedesktop.ColorManager";
const MANAGER: &str = "/org/freedesktop/ColorManager";
/// How long one question may take.
const ASK: Duration = Duration::from_millis(400);
/// How long the connection's setup may take: the window waits for it.
const SETUP: Duration = Duration::from_secs(1);

/// colord's checksums (MD5, lower case) of the profiles it keeps for the
/// output X11 names `output` ("eDP-1"): its `ProfileId` (`"icc-<MD5>"`) and its
/// metadata's `FILE_checksum`, profile by profile; none when colord does not
/// know the output; nothing at all (`None`) when colord does not answer, or
/// there is no system bus.
pub fn checksums(output: &str) -> Option<Vec<String>> {
    checksums_on(&crate::bus::system_address(), output)
}

/// `checksums`, from the colord on the bus at `address`.
pub fn checksums_on(address: &str, output: &str) -> Option<Vec<String>> {
    let bus = LocalConnection::from(crate::bus::connect_within(address, SETUP).ok()?);
    let found: Result<(Path<'static>,), dbus::Error> = bus.with_proxy(SERVICE, MANAGER, ASK).method_call(SERVICE, "FindDeviceByProperty", ("XRANDR_name", output));
    let device = match found {
        Ok((device,)) => device,
        // colord answered that it does not know this output; anything else: no colord.
        Err(e) if e.name().is_some_and(|name| name.starts_with(SERVICE)) => return Some(Vec::new()),
        Err(_) => return None,
    };
    let profiles: Vec<Path<'static>> = bus.with_proxy(SERVICE, device, ASK).get("org.freedesktop.ColorManager.Device", "Profiles").unwrap_or_default();
    let mut sums = Vec::new();
    for profile in profiles {
        let profile = bus.with_proxy(SERVICE, profile, ASK);
        if let Ok(id) = profile.get::<String>("org.freedesktop.ColorManager.Profile", "ProfileId")
            && let Some(sum) = id.strip_prefix("icc-")
        {
            sums.push(sum.to_lowercase());
        }
        if let Ok(metadata) = profile.get::<HashMap<String, String>>("org.freedesktop.ColorManager.Profile", "Metadata")
            && let Some(sum) = metadata.get("FILE_checksum").filter(|sum| !sum.is_empty())
        {
            sums.push(sum.to_lowercase());
        }
    }
    Some(sums)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// colord read as before (cpp/screencolours.cpp's own reading until
    /// 13 October 2026), against a stand-in colord on the test's own bus (run
    /// under dbus-run-session; skipped on a bus with a notification server,
    /// the desktop's: nothing here may reach it): an output it knows, its
    /// profiles' checksums in order, lower case; one it does not know, none;
    /// no colord at all, nothing.
    #[test]
    fn colord_says_which_profiles_an_output_has() {
        use dbus::arg::{RefArg, Variant};
        use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
        use dbus::channel::{MatchingReceiver, Sender as _};
        use dbus::message::MatchRule;
        let _alone = crate::bus::NOTIFICATION_SERVER.lock().unwrap_or_else(|e| e.into_inner());
        let Some(address) = crate::bus::session_address() else {
            eprintln!("No bus of the test's own (dbus-run-session): skipped.");
            return;
        };
        let look = LocalConnection::from(crate::bus::connect(&address).unwrap());
        let owned = |name: &str| -> bool { look.with_proxy("org.freedesktop.DBus", "/org/freedesktop/DBus", ASK).method_call("org.freedesktop.DBus", "NameHasOwner", (name,)).is_ok_and(|(owned,): (bool,)| owned) };
        if owned("org.freedesktop.Notifications") || owned(SERVICE) {
            eprintln!("A desktop's bus, or a colord on it already: skipped.");
            return;
        }
        // No colord yet: nothing said.
        assert_eq!(checksums_on(&address, "eDP-1"), None);
        let (ready, started) = std::sync::mpsc::channel::<bool>();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopped = std::sync::Arc::clone(&stop);
        let served = address.clone();
        std::thread::spawn(move || {
            let Some(bus) = crate::bus::connect(&served).ok().map(LocalConnection::from).filter(|bus| matches!(bus.request_name(SERVICE, false, false, true), Ok(RequestNameReply::PrimaryOwner))) else {
                let _ = ready.send(false);
                return;
            };
            bus.start_receive(
                MatchRule::new_method_call(),
                Box::new(|call: dbus::Message, bus: &LocalConnection| {
                    let path = call.path().map(|p| p.to_string()).unwrap_or_default();
                    let reply = match (call.member().as_deref(), path.as_str()) {
                        (Some("FindDeviceByProperty"), MANAGER) => {
                            let (key, output): (String, String) = call.read2().unwrap();
                            if key == "XRANDR_name" && output == "eDP-1" {
                                call.method_return().append1(Path::from("/org/freedesktop/ColorManager/devices/xrandr_eDP_1"))
                            } else {
                                call.error(&"org.freedesktop.ColorManager.NotFound".into(), c"no such device")
                            }
                        }
                        (Some("Get"), path) => {
                            let (_interface, name): (String, String) = call.read2().unwrap();
                            let value: Box<dyn RefArg> = match (path, name.as_str()) {
                                ("/org/freedesktop/ColorManager/devices/xrandr_eDP_1", "Profiles") => Box::new(vec![Path::from("/org/freedesktop/ColorManager/profiles/a"), Path::from("/org/freedesktop/ColorManager/profiles/b")]),
                                ("/org/freedesktop/ColorManager/profiles/a", "ProfileId") => Box::new("icc-0123456789ABCDEF0123456789ABCDEF".to_string()),
                                ("/org/freedesktop/ColorManager/profiles/a", "Metadata") => Box::new(HashMap::from([("FILE_checksum".to_string(), "FEDCBA9876543210FEDCBA9876543210".to_string())])),
                                ("/org/freedesktop/ColorManager/profiles/b", "ProfileId") => Box::new("custom-made".to_string()),
                                ("/org/freedesktop/ColorManager/profiles/b", "Metadata") => Box::new(HashMap::<String, String>::new()),
                                _ => return true,
                            };
                            call.method_return().append1(Variant(value))
                        }
                        _ => return true,
                    };
                    let _ = bus.send(reply);
                    true
                }),
            );
            let _ = ready.send(true);
            let until = std::time::Instant::now() + Duration::from_secs(20);
            while !stopped.load(std::sync::atomic::Ordering::Relaxed) && std::time::Instant::now() < until {
                let _ = bus.process(Duration::from_millis(50));
            }
        });
        if !started.recv_timeout(Duration::from_secs(5)).unwrap_or(false) {
            eprintln!("colord's name taken on this bus: skipped.");
            return;
        }
        assert_eq!(checksums_on(&address, "eDP-1"), Some(vec!["0123456789abcdef0123456789abcdef".to_string(), "fedcba9876543210fedcba9876543210".to_string()]));
        assert_eq!(checksums_on(&address, "HDMI-1"), Some(Vec::new()), "an output colord does not know: none");
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
