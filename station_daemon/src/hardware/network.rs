use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use tracing::info;

// ---------------------------------------------------------------------------
// Wi-Fi Types & Structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiStatus {
    pub interface: String,
    pub connected: bool,
    pub ssid: String,
    pub ip_address: String,
    pub mac_address: String,
    pub bssid: String,
    pub frequency_mhz: u32,
    pub signal_level_dbm: i32,
    pub wpa_state: String,
    pub key_mgmt: String,
    pub interface_up: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiApInfo {
    pub ssid: String,
    pub bssid: String,
    pub frequency_mhz: u32,
    pub signal_level_dbm: i32,
    pub flags: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedWifiNetwork {
    pub id: u32,
    pub ssid: String,
    pub bssid: String,
    pub flags: String,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiPowerSaveParam {
    pub name: String,
    pub value: String,
    pub recommended_value: String,
    pub is_critical: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiPowerSaveConfig {
    pub module_present: bool,
    pub rtw_power_mgnt: String,
    pub rtw_ips_mode: String,
    pub rtw_smart_ps: String,
    pub rtw_lps_level: String,
    pub is_optimized: bool,
    pub critical_explanation: String,
    pub params: Vec<WifiPowerSaveParam>,
}

// ---------------------------------------------------------------------------
// Bluetooth Types & Structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BtDeviceInfo {
    pub address: String,
    pub name: String,
    pub connected: bool,
    pub paired: bool,
    #[serde(default)]
    pub trusted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothStatus {
    pub available: bool,
    pub adapter: String,
    pub address: String,
    pub name: String,
    pub powered: bool,
    pub discoverable: bool,
    pub connected_device: Option<BtDeviceInfo>,
    pub devices: Vec<BtDeviceInfo>,
    pub paired_devices: Vec<BtDeviceInfo>,
}

// ---------------------------------------------------------------------------
// Network & System Hardware Controller
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct NetworkHardware;

impl NetworkHardware {
    pub fn new() -> Self {
        Self
    }

    // -----------------------------------------------------------------------
    // Wi-Fi Management
    // -----------------------------------------------------------------------

    /// Retrieves current Wi-Fi status using wpa_cli, /sys/class/net, and ifconfig
    pub fn get_wifi_status(&self) -> WifiStatus {
        let mut status = WifiStatus {
            interface: "wlan0".to_string(),
            connected: false,
            ssid: String::new(),
            ip_address: String::new(),
            mac_address: String::new(),
            bssid: String::new(),
            frequency_mhz: 0,
            signal_level_dbm: 0,
            wpa_state: "DISCONNECTED".to_string(),
            key_mgmt: String::new(),
            interface_up: false,
        };

        // 1. MAC address from sysfs
        if let Ok(mac) = fs::read_to_string("/sys/class/net/wlan0/address") {
            status.mac_address = mac.trim().to_uppercase();
        }

        // 2. Operstate
        if let Ok(state) = fs::read_to_string("/sys/class/net/wlan0/operstate") {
            status.interface_up = state.trim() == "up";
        }

        // 3. wpa_cli -i wlan0 status
        if let Ok(output) = Command::new("wpa_cli").args(["-i", "wlan0", "status"]).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let mut parts = line.splitn(2, '=');
                    let k = parts.next().unwrap_or("").trim();
                    let v = parts.next().unwrap_or("").trim();
                    match k {
                        "bssid" => status.bssid = v.to_uppercase(),
                        "ssid" => status.ssid = v.to_string(),
                        "ip_address" => status.ip_address = v.to_string(),
                        "wpa_state" => {
                            status.wpa_state = v.to_string();
                            if v == "COMPLETED" {
                                status.connected = true;
                                status.interface_up = true;
                            }
                        }
                        "key_mgmt" => status.key_mgmt = v.to_string(),
                        "freq" => status.frequency_mhz = v.parse().unwrap_or(0),
                        "address" if status.mac_address.is_empty() => {
                            status.mac_address = v.to_uppercase();
                        }
                        _ => {}
                    }
                }
            }
        }

        // 4. Fallback IP detection if wpa_cli didn't report it
        if status.ip_address.is_empty() {
            if let Ok(output) = Command::new("ifconfig").arg("wlan0").output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("inet addr:") {
                        if let Some(pos) = line.find("inet addr:") {
                            let rest = &line[pos + 10..];
                            let ip = rest.split_whitespace().next().unwrap_or("");
                            status.ip_address = ip.to_string();
                            break;
                        }
                    } else if line.trim().starts_with("inet ") {
                        let parts: Vec<&str> = line.trim().split_whitespace().collect();
                        if parts.len() >= 2 {
                            status.ip_address = parts[1].split('/').next().unwrap_or("").to_string();
                            break;
                        }
                    }
                }
            }
        }

        // 5. Signal level from wpa_cli signal_poll or /proc/net/wireless
        if let Ok(output) = Command::new("wpa_cli").args(["-i", "wlan0", "signal_poll"]).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.starts_with("RSSI=") {
                        if let Ok(val) = line[5..].trim().parse::<i32>() {
                            status.signal_level_dbm = val;
                        }
                    }
                }
            }
        }

        status
    }

    /// Scans for Wi-Fi access points via wpa_cli
    pub fn scan_wifi(&self) -> Result<Vec<WifiApInfo>, String> {
        let _ = Command::new("wpa_cli").args(["-i", "wlan0", "scan"]).output();
        std::thread::sleep(std::time::Duration::from_millis(1500));

        let output = Command::new("wpa_cli")
            .args(["-i", "wlan0", "scan_results"])
            .output()
            .map_err(|e| format!("Failed to execute wpa_cli scan_results: {}", e))?;

        if !output.status.success() {
            return Err("wpa_cli scan_results returned non-zero exit code".to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();
        // Skip header: "bssid / frequency / signal level / flags / ssid"
        for line in stdout.lines().skip(1) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() >= 5 {
                let bssid = cols[0].trim().to_uppercase();
                let freq = cols[1].trim().parse::<u32>().unwrap_or(0);
                let signal = cols[2].trim().parse::<i32>().unwrap_or(-100);
                let flags = cols[3].trim().to_string();
                let ssid = cols[4].trim().to_string();

                if !ssid.is_empty() {
                    results.push(WifiApInfo {
                        ssid,
                        bssid,
                        frequency_mhz: freq,
                        signal_level_dbm: signal,
                        flags,
                    });
                }
            }
        }

        // Sort strongest signal first
        results.sort_by(|a, b| b.signal_level_dbm.cmp(&a.signal_level_dbm));
        Ok(results)
    }

    /// Connects to a Wi-Fi network using wpa_cli and saves config
    pub fn connect_wifi(&self, ssid: &str, psk: &str) -> Result<String, String> {
        info!("Connecting Wi-Fi to SSID: '{}'", ssid);

        // 1. Add new network ID
        let add_out = Command::new("wpa_cli")
            .args(["-i", "wlan0", "add_network"])
            .output()
            .map_err(|e| format!("wpa_cli add_network error: {}", e))?;
        let net_id_str = String::from_utf8_lossy(&add_out.stdout).trim().to_string();
        let net_id = net_id_str
            .parse::<u32>()
            .map_err(|_| format!("Invalid network ID from wpa_cli: '{}'", net_id_str))?;

        // 2. Set SSID: quoted string
        let ssid_arg = format!("\"{}\"", ssid);
        let _ = Command::new("wpa_cli")
            .args(["-i", "wlan0", "set_network", &net_id.to_string(), "ssid", &ssid_arg])
            .output();

        // 3. Set PSK or Key Management
        if psk.is_empty() {
            let _ = Command::new("wpa_cli")
                .args(["-i", "wlan0", "set_network", &net_id.to_string(), "key_mgmt", "NONE"])
                .output();
        } else {
            let psk_arg = format!("\"{}\"", psk);
            let _ = Command::new("wpa_cli")
                .args(["-i", "wlan0", "set_network", &net_id.to_string(), "psk", &psk_arg])
                .output();
        }

        // 4. Enable and Select
        let _ = Command::new("wpa_cli")
            .args(["-i", "wlan0", "enable_network", &net_id.to_string()])
            .output();
        let _ = Command::new("wpa_cli")
            .args(["-i", "wlan0", "select_network", &net_id.to_string()])
            .output();
        let _ = Command::new("wpa_cli")
            .args(["-i", "wlan0", "save_config"])
            .output();

        // 5. Trigger DHCP in background if necessary
        let _ = Command::new("sh").args(["-c", "dhcpcd wlan0 >/dev/null 2>&1 &"]).output();

        Ok(format!("Network #{} configured for '{}'. Connecting...", net_id, ssid))
    }

    /// Restores Wi-Fi interface and restarts wpa_supplicant / dhcpcd
    pub fn restart_wifi(&self) -> Result<String, String> {
        info!("Executing Wi-Fi restart and connection refresh");
        if Path::new("/data/kill_and_wifi.sh").exists() {
            let _ = Command::new("sh").arg("/data/kill_and_wifi.sh").output();
        } else {
            let _ = Command::new("ifconfig").args(["wlan0", "up"]).output();
            let _ = Command::new("sh")
                .args([
                    "-c",
                    "if ! pidof wpa_supplicant > /dev/null; then wpa_supplicant -B -i wlan0 -c /etc/wpa_supplicant.conf 2>/dev/null; fi; if ! pidof dhcpcd > /dev/null; then dhcpcd wlan0 2>/dev/null & fi",
                ])
                .output();
        }
        Ok("Wi-Fi restart sequence initiated".to_string())
    }

    /// Lists saved Wi-Fi networks in wpa_supplicant config
    pub fn get_saved_wifi_networks(&self) -> Result<Vec<SavedWifiNetwork>, String> {
        let output = Command::new("wpa_cli")
            .args(["-i", "wlan0", "list_networks"])
            .output()
            .map_err(|e| format!("Failed to execute wpa_cli list_networks: {}", e))?;

        if !output.status.success() {
            return Err("wpa_cli list_networks returned error".to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        // format:
        // network id / ssid / bssid / flags
        // 0	AliceBridge	any	[DISABLED]
        // 4	TP-Link_4107	any	[CURRENT]
        for line in stdout.lines().skip(1) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() >= 4 {
                if let Ok(id) = cols[0].trim().parse::<u32>() {
                    let ssid = cols[1].trim().to_string();
                    let bssid = cols[2].trim().to_string();
                    let flags = cols[3].trim().to_string();
                    let is_current = flags.contains("[CURRENT]");
                    // Filter out unprintable corrupted ssid if needed
                    if !ssid.is_empty() {
                        results.push(SavedWifiNetwork {
                            id,
                            ssid,
                            bssid,
                            flags,
                            is_current,
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Switches connection to a previously configured network ID
    pub fn select_saved_wifi_network(&self, id: u32) -> Result<String, String> {
        info!("Selecting saved Wi-Fi network id #{}", id);
        let id_str = id.to_string();

        let _ = Command::new("wpa_cli")
            .args(["-i", "wlan0", "enable_network", &id_str])
            .output();

        let out = Command::new("wpa_cli")
            .args(["-i", "wlan0", "select_network", &id_str])
            .output()
            .map_err(|e| format!("wpa_cli select_network error: {}", e))?;

        if !out.status.success() {
            return Err(format!("wpa_cli failed to select network #{}", id));
        }

        let _ = Command::new("wpa_cli")
            .args(["-i", "wlan0", "save_config"])
            .output();

        let _ = Command::new("sh")
            .args(["-c", "dhcpcd wlan0 >/dev/null 2>&1 &"])
            .output();

        Ok(format!("Switched to network #{}", id))
    }

    /// Forgets / removes a saved Wi-Fi network from wpa_supplicant config
    pub fn remove_saved_wifi_network(&self, id: u32) -> Result<String, String> {
        info!("Removing saved Wi-Fi network id #{}", id);
        let id_str = id.to_string();

        let out = Command::new("wpa_cli")
            .args(["-i", "wlan0", "remove_network", &id_str])
            .output()
            .map_err(|e| format!("wpa_cli remove_network error: {}", e))?;

        if !out.status.success() {
            return Err(format!("wpa_cli failed to remove network #{}", id));
        }

        let _ = Command::new("wpa_cli")
            .args(["-i", "wlan0", "save_config"])
            .output();

        Ok(format!("Network #{} removed from wpa_supplicant", id))
    }

    // -----------------------------------------------------------------------
    // Wi-Fi 88x2cs Power Saving (/sys/module/88x2cs/parameters/)
    // -----------------------------------------------------------------------

    /// Reads all power saving parameters of Realtek 88x2cs module.
    /// Crucial for preventing Wi-Fi from dropping when Quasar is disabled!
    pub fn get_powersave_config(&self) -> WifiPowerSaveConfig {
        let base = Path::new("/sys/module/88x2cs/parameters");
        let module_present = base.exists();

        let read_param = |name: &str| -> String {
            let p = base.join(name);
            if p.exists() {
                fs::read_to_string(p).unwrap_or_default().trim().to_string()
            } else {
                "-1".to_string()
            }
        };

        let rtw_power_mgnt = read_param("rtw_power_mgnt");
        let rtw_ips_mode = read_param("rtw_ips_mode");
        let rtw_smart_ps = read_param("rtw_smart_ps");
        let rtw_lps_level = read_param("rtw_lps_level");

        let is_optimized = rtw_power_mgnt == "0" && rtw_ips_mode == "0" && (rtw_smart_ps == "0" || rtw_smart_ps == "-1");

        let mut params = Vec::new();

        params.push(WifiPowerSaveParam {
            name: "rtw_power_mgnt".to_string(),
            value: rtw_power_mgnt.clone(),
            recommended_value: "0".to_string(),
            is_critical: true,
            description: "Главный переключатель энергосбережения чипа: 0 = Выключить полностью (Wi-Fi активен всегда), 1 = Min PS, 2 = Max PS. При выключенном Квазаре значение 1 или 2 переводит чип в глубокий сон и Wi-Fi падает!".to_string(),
        });

        params.push(WifiPowerSaveParam {
            name: "rtw_ips_mode".to_string(),
            value: rtw_ips_mode.clone(),
            recommended_value: "0".to_string(),
            is_critical: true,
            description: "Inactive Power Saving (IPS): 0 = Выключить, 1 = Включить. Отключает уход Wi-Fi интерфейса в сон при отсутствии сетевого трафика.".to_string(),
        });

        params.push(WifiPowerSaveParam {
            name: "rtw_smart_ps".to_string(),
            value: rtw_smart_ps.clone(),
            recommended_value: "0".to_string(),
            is_critical: true,
            description: "Smart Power Saving: 0 = Выключить, 1/2 = Включить. Предотвращает задержки и потерю пинга при стриминге музыки и вызовах ассистента.".to_string(),
        });

        if base.join("rtw_lps_level").exists() {
            params.push(WifiPowerSaveParam {
                name: "rtw_lps_level".to_string(),
                value: rtw_lps_level.clone(),
                recommended_value: "0".to_string(),
                is_critical: false,
                description: "Leisure Power Saving Level: глубина спящего режима между маяками роутера.".to_string(),
            });
        }

        if base.join("rtw_low_power").exists() {
            params.push(WifiPowerSaveParam {
                name: "rtw_low_power".to_string(),
                value: read_param("rtw_low_power"),
                recommended_value: "0".to_string(),
                is_critical: false,
                description: "Общий флаг низкого энергопотребления драйвера (0 = Выключен).".to_string(),
            });
        }

        if base.join("rtw_drv_log_level").exists() {
            params.push(WifiPowerSaveParam {
                name: "rtw_drv_log_level".to_string(),
                value: read_param("rtw_drv_log_level"),
                recommended_value: "0".to_string(),
                is_critical: false,
                description: "Уровень логирования драйвера в dmesg (0 = минимум шума).".to_string(),
            });
        }

        WifiPowerSaveConfig {
            module_present,
            rtw_power_mgnt,
            rtw_ips_mode,
            rtw_smart_ps,
            rtw_lps_level,
            is_optimized,
            critical_explanation: "ВНИМАНИЕ: Чтобы Wi-Fi соединение не падало и не засыпало после отключения сервисов Квазара (maind/quasar_launcher2), обязательно установите в 0 параметры: rtw_power_mgnt, rtw_ips_mode и rtw_smart_ps!".to_string(),
            params,
        }
    }

    /// Writes a specific parameter value to /sys/module/88x2cs/parameters/
    pub fn set_powersave_param(&self, name: &str, value: &str) -> Result<(), String> {
        // Sanitize name to prevent path traversal
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err("Invalid parameter name".to_string());
        }

        let p = Path::new("/sys/module/88x2cs/parameters").join(name);
        if !p.exists() {
            return Err(format!("Parameter {} does not exist in /sys/module/88x2cs/parameters/", name));
        }

        fs::write(&p, value.trim()).map_err(|e| format!("Failed to write to {}: {}", p.display(), e))?;
        info!("Successfully updated 88x2cs sysfs param {} -> {}", name, value.trim());
        Ok(())
    }

    /// Optimizes power saving for headless station operation:
    /// Sets rtw_power_mgnt=0, rtw_ips_mode=0, rtw_smart_ps=0, rtw_lps_mode=0
    pub fn optimize_powersave(&self) -> Result<(), String> {
        let base = Path::new("/sys/module/88x2cs/parameters");
        if !base.exists() {
            return Err("Module 88x2cs sysfs parameters directory not found".to_string());
        }

        let targets = [
            ("rtw_power_mgnt", "0"),
            ("rtw_ips_mode", "0"),
            ("rtw_smart_ps", "0"),
            ("rtw_lps_mode", "0"),
            ("rtw_low_power", "0"),
        ];

        for (name, val) in targets {
            let p = base.join(name);
            if p.exists() {
                let _ = fs::write(p, val);
            }
        }

        info!("Applied recommended Wi-Fi power-saving overrides (all power saving disabled)");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Bluetooth Management (hciconfig / bluetoothctl)
    // -----------------------------------------------------------------------

    /// Retrieves Bluetooth adapter state and known devices
    pub fn get_bluetooth_status(&self) -> BluetoothStatus {
        let mut status = BluetoothStatus {
            available: false,
            adapter: "hci0".to_string(),
            address: String::new(),
            name: "Яндекс Лайт".to_string(),
            powered: false,
            discoverable: false,
            connected_device: None,
            devices: Vec::new(),
            paired_devices: Vec::new(),
        };

        // 1. Check via hciconfig
        if let Ok(output) = Command::new("hciconfig").arg("hci0").output() {
            if output.status.success() {
                status.available = true;
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("BD Address:") {
                        if let Some(pos) = line.find("BD Address:") {
                            let rest = &line[pos + 11..];
                            let addr = rest.split_whitespace().next().unwrap_or("");
                            status.address = addr.to_uppercase();
                        }
                    }
                    if line.contains("UP RUNNING") || line.contains("UP") {
                        status.powered = true;
                    }
                    if line.contains("PSCAN") && line.contains("ISCAN") {
                        status.discoverable = true;
                    }
                }
            }
        }

        // 2. Query name and discoverable state via bluetoothctl show
        if let Ok(output) = Command::new("bluetoothctl").arg("show").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("Alias:") {
                        status.name = trimmed[6..].trim().to_string();
                    } else if trimmed.starts_with("Name:") && status.name.is_empty() {
                        status.name = trimmed[5..].trim().to_string();
                    } else if trimmed.starts_with("Powered:") {
                        status.powered = trimmed[8..].trim() == "yes";
                    } else if trimmed.starts_with("Discoverable:") {
                        if trimmed[13..].trim() == "yes" {
                            status.discoverable = true;
                        }
                    }
                }
            }
        }

        // 3. Find connected devices
        let mut connected_addrs = HashSet::new();
        if let Ok(output) = Command::new("bluetoothctl").args(["devices", "Connected"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("Device ") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let addr = parts[1].to_uppercase();
                        let name = if parts.len() >= 3 {
                            parts[2..].join(" ")
                        } else {
                            addr.clone()
                        };
                        connected_addrs.insert(addr.clone());
                        if status.connected_device.is_none() {
                            status.connected_device = Some(BtDeviceInfo {
                                address: addr,
                                name,
                                connected: true,
                                paired: true,
                                trusted: true,
                            });
                        }
                    }
                }
            }
        }

        // 4. Find paired devices
        let mut paired_addrs = HashSet::new();
        if let Ok(output) = Command::new("bluetoothctl").args(["devices", "Paired"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("Device ") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let addr = parts[1].to_uppercase();
                        let name = if parts.len() >= 3 {
                            parts[2..].join(" ")
                        } else {
                            addr.clone()
                        };
                        paired_addrs.insert(addr.clone());
                        let is_conn = connected_addrs.contains(&addr);
                        status.paired_devices.push(BtDeviceInfo {
                            address: addr,
                            name,
                            connected: is_conn,
                            paired: true,
                            trusted: true,
                        });
                    }
                }
            }
        }

        // 5. Query all devices (cached / paired)
        if let Ok(output) = Command::new("bluetoothctl").arg("devices").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("Device ") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let addr = parts[1].to_uppercase();
                        let name = if parts.len() >= 3 {
                            parts[2..].join(" ")
                        } else {
                            addr.clone()
                        };
                        let is_conn = connected_addrs.contains(&addr);
                        let is_paired = paired_addrs.contains(&addr);
                        status.devices.push(BtDeviceInfo {
                            address: addr,
                            name,
                            connected: is_conn,
                            paired: is_paired,
                            trusted: is_paired,
                        });
                    }
                }
            }
        }

        status
    }

    /// Toggles Bluetooth power state (on/off)
    pub fn toggle_bluetooth_power(&self, power_on: bool) -> Result<(), String> {
        info!("Setting Bluetooth power -> {}", power_on);
        let arg = if power_on { "up" } else { "down" };
        let out = Command::new("hciconfig")
            .args(["hci0", arg])
            .output()
            .map_err(|e| format!("hciconfig error: {}", e))?;

        if !out.status.success() {
            let btctl_arg = if power_on { "on" } else { "off" };
            let _ = Command::new("bluetoothctl").args(["power", btctl_arg]).output();
        }

        Ok(())
    }

    /// Toggles Bluetooth discoverable (pairing / speaker sink mode for phones)
    pub fn set_bluetooth_discoverable(&self, discoverable: bool) -> Result<(), String> {
        info!("Setting Bluetooth discoverable mode -> {}", discoverable);
        // Ensure hci0 is up
        let _ = Command::new("hciconfig").args(["hci0", "up"]).output();

        if discoverable {
            let _ = Command::new("hciconfig").args(["hci0", "piscan"]).output();
            let _ = Command::new("bluetoothctl").args(["discoverable", "on"]).output();
            let _ = Command::new("bluetoothctl").args(["pairable", "on"]).output();
        } else {
            let _ = Command::new("hciconfig").args(["hci0", "noscan"]).output();
            let _ = Command::new("bluetoothctl").args(["discoverable", "off"]).output();
        }

        Ok(())
    }

    /// Scans for nearby Bluetooth devices via bluetoothctl
    pub fn scan_bluetooth(&self) -> Result<Vec<BtDeviceInfo>, String> {
        info!("Starting Bluetooth scan (bluetoothctl --timeout 5 scan on)...");
        // Ensure hci0 is up
        let _ = Command::new("hciconfig").args(["hci0", "up"]).output();

        // 1. Run 5-second scan natively supported by bluetoothctl 5.70
        let _ = Command::new("bluetoothctl")
            .args(["--timeout", "5", "scan", "on"])
            .output();

        // 2. Query sets of connected and paired devices
        let mut connected_set = HashSet::new();
        if let Ok(out) = Command::new("bluetoothctl").args(["devices", "Connected"]).output() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.len() >= 2 && parts[0] == "Device" {
                    connected_set.insert(parts[1].to_uppercase());
                }
            }
        }

        let mut paired_set = HashSet::new();
        if let Ok(out) = Command::new("bluetoothctl").args(["devices", "Paired"]).output() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.len() >= 2 && parts[0] == "Device" {
                    paired_set.insert(parts[1].to_uppercase());
                }
            }
        }

        // 3. Query all discovered devices
        let output = Command::new("bluetoothctl")
            .arg("devices")
            .output()
            .map_err(|e| format!("bluetoothctl devices error: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Device ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    let addr = parts[1].to_uppercase();
                    let name = if parts.len() >= 3 {
                        parts[2..].join(" ")
                    } else {
                        addr.clone()
                    };
                    let is_conn = connected_set.contains(&addr);
                    let is_pair = paired_set.contains(&addr);
                    results.push(BtDeviceInfo {
                        address: addr,
                        name,
                        connected: is_conn,
                        paired: is_pair,
                        trusted: is_pair,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Pairs and trusts a Bluetooth device by MAC address
    pub fn pair_bluetooth(&self, mac: &str) -> Result<String, String> {
        info!("Pairing and trusting Bluetooth device {}", mac);
        let _ = Command::new("bluetoothctl").args(["pair", mac]).output();
        let out = Command::new("bluetoothctl")
            .args(["trust", mac])
            .output()
            .map_err(|e| format!("bluetoothctl trust error: {}", e))?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        Ok(stdout)
    }

    /// Removes / forgets a Bluetooth device by MAC address
    pub fn remove_bluetooth(&self, mac: &str) -> Result<String, String> {
        info!("Removing Bluetooth device {}", mac);
        let out = Command::new("bluetoothctl")
            .args(["remove", mac])
            .output()
            .map_err(|e| format!("bluetoothctl remove error: {}", e))?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        Ok(stdout)
    }

    /// Connects to a Bluetooth device by MAC address
    pub fn connect_bluetooth(&self, mac: &str) -> Result<String, String> {
        info!("Connecting Bluetooth to {}", mac);
        let out = Command::new("bluetoothctl")
            .args(["connect", mac])
            .output()
            .map_err(|e| format!("bluetoothctl connect error: {}", e))?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        Ok(stdout)
    }

    /// Disconnects a Bluetooth device by MAC address
    pub fn disconnect_bluetooth(&self, mac: &str) -> Result<String, String> {
        info!("Disconnecting Bluetooth from {}", mac);
        let out = Command::new("bluetoothctl")
            .args(["disconnect", mac])
            .output()
            .map_err(|e| format!("bluetoothctl disconnect error: {}", e))?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        Ok(stdout)
    }
}
