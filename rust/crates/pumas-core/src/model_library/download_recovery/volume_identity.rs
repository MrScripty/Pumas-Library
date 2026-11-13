//! A volume identity that survives reboots.
//!
//! Persisted custody (copied-import receipts, download admissions) records
//! which volume a file lives on. A Unix device number is not stable across
//! boots: block devices are numbered in probe order, so a library on one of
//! two NVMe drives moved from 259:6 to 259:8 after a reboot, and every receipt
//! written before it stopped matching.
//!
//! On Linux the volume is the filesystem UUID, found through the udev
//! `/dev/disk/by-uuid` links whose block device carries the file's device
//! number. Stable volumes have [`STABLE_VOLUME`] set, which raw device numbers
//! never use. A filesystem without a UUID link (tmpfs, overlay, multi-device
//! btrfs) keeps its device number, which was the behaviour before.
//!
//! Records written before this change hold raw device numbers; reading one
//! translates it through the same lookup, so records written during the
//! current boot keep matching after an upgrade.

#[cfg(unix)]
use std::collections::HashMap;
#[cfg(unix)]
use std::sync::{Mutex, OnceLock};

/// Set on every UUID-derived volume; Linux device numbers stay far below it.
pub(crate) const STABLE_VOLUME: u64 = 1 << 63;

/// Only Unix device numbers need a stable volume; Windows volume serial
/// numbers are already stable.
///
/// The stable volume for a device number, or the device number itself when
/// its filesystem has no UUID to name it by.
#[cfg(unix)]
pub(crate) fn stable_volume(device: u64) -> u64 {
    if device & STABLE_VOLUME != 0 {
        return device;
    }
    static CACHE: OnceLock<Mutex<HashMap<u64, u64>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(volume) = cache
        .lock()
        .ok()
        .and_then(|cache| cache.get(&device).copied())
    {
        return volume;
    }
    let volume = filesystem_uuid(device)
        .map(|uuid| volume_from_uuid(&uuid))
        .unwrap_or(device);
    if let Ok(mut cache) = cache.lock() {
        cache.insert(device, volume);
    }
    volume
}

/// FNV-1a over the UUID's case-folded text: deterministic across processes,
/// releases and platforms.
#[cfg(unix)]
pub(super) fn volume_from_uuid(uuid: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in uuid.trim().to_ascii_lowercase().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash | STABLE_VOLUME
}

#[cfg(target_os = "linux")]
fn filesystem_uuid(device: u64) -> Option<String> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    // Read-only discovery of udev's naming links; nothing here is trusted
    // beyond naming the volume a held file already lives on.
    for entry in std::fs::read_dir("/dev/disk/by-uuid").ok()?.flatten() {
        let Ok(metadata) = std::fs::metadata(entry.path()) else {
            continue;
        };
        if metadata.file_type().is_block_device() && metadata.rdev() == device {
            return entry.file_name().into_string().ok();
        }
    }
    None
}

#[cfg(all(unix, not(target_os = "linux")))]
fn filesystem_uuid(_device: u64) -> Option<String> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn uuid_volumes_are_marked_deterministic_and_case_insensitive() {
        let volume = volume_from_uuid("b7b7341e-ec16-4335-96d1-54ecad0f4e2b");
        assert_ne!(volume & STABLE_VOLUME, 0);
        assert_eq!(
            volume,
            volume_from_uuid("B7B7341E-EC16-4335-96D1-54ECAD0F4E2B")
        );
        assert_ne!(
            volume,
            volume_from_uuid("17f18f18-5127-482b-bff5-89e3d250e03e")
        );
        // Pinned: a persisted volume must never change between releases.
        assert_eq!(volume, 0x9475_bba6_8cce_893d);
    }

    #[test]
    fn a_stable_volume_is_never_translated_again() {
        let volume = volume_from_uuid("18E2-6365");
        assert_eq!(stable_volume(volume), volume);
    }

    #[test]
    fn a_device_without_a_uuid_link_keeps_its_number() {
        // No block device carries this number.
        let device = (0xfff_u64 << 32) | 0xff_fff;
        assert_eq!(stable_volume(device), device);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_file_on_a_uuid_filesystem_gets_a_stable_volume() {
        use std::os::unix::fs::MetadataExt;
        let temp = tempfile::tempdir().unwrap();
        let device = std::fs::metadata(temp.path()).unwrap().dev();
        let volume = stable_volume(device);
        // Either the filesystem has a UUID link (stable) or it keeps its
        // device number; it is never some other device's identity.
        match filesystem_uuid(device) {
            Some(uuid) => assert_eq!(volume, volume_from_uuid(&uuid)),
            None => assert_eq!(volume, device),
        }
    }
}
