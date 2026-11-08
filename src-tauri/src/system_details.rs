//! Read-only disk and network inventory based on sysinfo 0.33.
use crate::now_ms;
use serde::Serialize;
use sysinfo::{Disks, Networks};

#[derive(Debug, Serialize)]
pub struct DiskSummary {
    label: String,
    mount: String,
    total_bytes: u64,
    used_bytes: u64,
    available_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct NetworkSummary {
    name: String,
    received_bytes: u64,
    transmitted_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct SystemDetails {
    disks: Vec<DiskSummary>,
    networks: Vec<NetworkSummary>,
    sampled_at_ms: u128,
}

fn used_bytes(total: u64, available: u64) -> u64 {
    total.saturating_sub(available)
}

#[tauri::command]
pub fn get_system_details() -> SystemDetails {
    // No elevation, network requests, wifi scanning, or process history.
    let disks = Disks::new_with_refreshed_list();
    let mut disk_details: Vec<DiskSummary> = disks
        .list()
        .iter()
        .filter(|disk| disk.total_space() > 0)
        .take(24)
        .map(|disk| DiskSummary {
            label: disk.name().to_string_lossy().to_string(),
            mount: disk.mount_point().display().to_string(),
            total_bytes: disk.total_space(),
            used_bytes: used_bytes(disk.total_space(), disk.available_space()),
            available_bytes: disk.available_space(),
        })
        .collect();
    disk_details.sort_by(|a, b| a.mount.cmp(&b.mount));

    let networks = Networks::new_with_refreshed_list();
    let mut network_details: Vec<NetworkSummary> = networks
        .iter()
        .take(32)
        .map(|(name, network)| NetworkSummary {
            name: name.to_owned(),
            received_bytes: network.total_received(),
            transmitted_bytes: network.total_transmitted(),
        })
        .collect();
    network_details.sort_by(|a, b| a.name.cmp(&b.name));

    SystemDetails {
        disks: disk_details,
        networks: network_details,
        sampled_at_ms: now_ms(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn used_space_saturates_on_invalid_available_bytes() {
        assert_eq!(used_bytes(100, 40), 60);
        assert_eq!(used_bytes(100, 150), 0);
        assert_eq!(used_bytes(0, 0), 0);
    }
}
