use crate::utils::{HKEY_LM_REG_KEY, get_winreg_arch_flags};
use std::fs::copy;
use std::path::PathBuf;
use std::sync::Once;
use tracing::{debug, warn};
use winreg::enums;

/// Creates the missing protocol.dll file required for ISO 11898 RAW protocol support
/// in Softing EDIC.
///
/// Some Softing D-PDU API installers do not create protocol.dll, even though it is
/// required for using the ISO 11898 RAW protocol.
///
/// The protocol.dll file is identical to vcifman_pdu.dll, so this function restores
/// it by copying the existing vcifman_pdu.dll installation.
pub fn restore_protocol_dll() {
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        // Softing D-PDU API installer registry branch.
        try_restore_protocol_dll("SOFTWARE\\Softing\\D-PDU API", "InstallLocation");

        // Softing D-PDU API_Module installer registry branch.
        try_restore_protocol_dll("SOFTWARE\\Softing\\D-PDU API_Module", "PDUAPIInstallDir");
    });
}

/// Attempts to restore a missing protocol.dll file using the specified registry branch.
///
/// The function searches installed Softing D-PDU API versions under the given registry path,
/// retrieves the installation directory from the specified registry value, and creates
/// protocol.dll by copying the existing vcifman_pdu.dll file if needed.
///
/// No action is taken if the required directories or files are missing, or if protocol.dll
/// already exists.
fn try_restore_protocol_dll(winreg_path: &str, install_dir_key: &str) {
    let Ok(path) = HKEY_LM_REG_KEY
        .open_subkey_with_flags(winreg_path, get_winreg_arch_flags())
        .inspect_err(|err| {
            debug!(
                path = winreg_path,
                "Registry path cannot be opened: {err:?}"
            )
        })
    else {
        return;
    };

    for version_path in path.enum_keys().flatten() {
        let full_winreg_path = format!("{winreg_path}\\{version_path}\\Installer");

        let Ok(installer) = path
            .open_subkey_with_flags(format!("{version_path}\\Installer"), enums::KEY_READ)
            .inspect_err(|err| {
                debug!(
                    path = full_winreg_path,
                    "Registry path cannot be opened: {err:?}"
                )
            })
        else {
            continue;
        };

        let Ok(install_directory) = installer
            .get_value::<String, _>(install_dir_key)
            .inspect_err(|err| {
                debug!(
                    path = full_winreg_path,
                    key = install_dir_key,
                    "Registry value cannot be read: {err:?}"
                )
            })
            .map(PathBuf::from)
        else {
            continue;
        };

        let vecom_directory = install_directory.join("VeCom");
        if !vecom_directory.is_dir() {
            // VeCom directory does not exist or is not a directory.
            continue;
        }

        let protocol_dll_path = vecom_directory.join("protocol.dll");
        if protocol_dll_path.exists() {
            // protocol.dll already exists, no action required.
            continue;
        }

        let vcifman_pdu_dll_path = vecom_directory.join("vcifman_pdu.dll");
        if !vcifman_pdu_dll_path.is_file() {
            // vcifman_pdu.dll does not exist.
            continue;
        }

        if let Err(err) = copy(&vcifman_pdu_dll_path, &protocol_dll_path) {
            warn!(
                edic_version = version_path,
                source = ?vcifman_pdu_dll_path,
                dest = ?protocol_dll_path,
                "Failed to create protocol.dll from vcifman_pdu.dll: {err}"
            );
        }
    }
}
