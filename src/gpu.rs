/// gpu.rs — Windows GPU enumeration via DXGI
/// Replaces the Linux /sys/class/drm sysfs approach entirely.
/// Uses DirectX Graphics Infrastructure (DXGI) to enumerate adapters.

use std::error::Error;

#[cfg(target_os = "windows")]
use windows::{
    Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_DESC1, DXGI_ERROR_NOT_FOUND,
    },
    core::Interface,
};

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum GPUVendor {
    UNKNOWN = 0x0000,
    INTEL   = 0x8086,
    NVIDIA  = 0x10DE,
    AMD     = 0x1002,
}

impl From<u32> for GPUVendor {
    fn from(value: u32) -> Self {
        match value {
            0x8086 => GPUVendor::INTEL,
            0x10DE => GPUVendor::NVIDIA,
            0x1002 => GPUVendor::AMD,
            _      => GPUVendor::UNKNOWN,
        }
    }
}

impl From<&str> for GPUVendor {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "intel"  => GPUVendor::INTEL,
            "nvidia" => GPUVendor::NVIDIA,
            "amd"    => GPUVendor::AMD,
            _        => GPUVendor::UNKNOWN,
        }
    }
}

impl From<String> for GPUVendor {
    fn from(value: String) -> Self {
        GPUVendor::from(value.as_str())
    }
}

impl GPUVendor {
    pub fn as_str(&self) -> &str {
        match self {
            GPUVendor::INTEL   => "Intel",
            GPUVendor::NVIDIA  => "NVIDIA",
            GPUVendor::AMD     => "AMD",
            GPUVendor::UNKNOWN => "Unknown",
        }
    }
}

impl std::fmt::Display for GPUVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Windows GPU info — mirrors the original GPUInfo structure
/// but uses Windows-native identifiers instead of Linux DRI paths.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct GPUInfo {
    pub vendor:      GPUVendor,
    /// DXGI adapter index (0 = primary)
    pub adapter_idx: u32,
    /// Human-readable device name from DXGI
    pub device_name: String,
    /// PCI vendor ID as hex string
    pub vendor_id:   String,
    /// PCI device ID as hex string  
    pub device_id:   String,
    /// Dedicated VRAM in MB
    pub vram_mb:     u64,
}

impl GPUInfo {
    pub fn vendor(&self) -> &GPUVendor     { &self.vendor }
    pub fn adapter_idx(&self) -> u32       { self.adapter_idx }
    pub fn device_name(&self) -> &str      { &self.device_name }
    pub fn vram_mb(&self) -> u64           { self.vram_mb }

    /// Returns a GStreamer-compatible device selector string for nvh264enc etc.
    /// NVENC uses adapter index; MF encoders typically use default device.
    pub fn gst_device_index(&self) -> Option<u32> {
        match self.vendor {
            GPUVendor::NVIDIA => Some(self.adapter_idx),
            _                 => None,
        }
    }

    pub fn as_str(&self) -> String {
        format!(
            "{} (Vendor: {}, Adapter Index: {}, VRAM: {} MB, VendorID: 0x{}, DeviceID: 0x{})",
            self.device_name, self.vendor, self.adapter_idx,
            self.vram_mb, self.vendor_id, self.device_id
        )
    }
}

impl std::fmt::Display for GPUInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Enumerate all GPUs on the system using DXGI.
/// Soft-skips Microsoft Basic Render Driver (software fallback adapter).
#[cfg(target_os = "windows")]
pub fn get_gpus() -> Result<Vec<GPUInfo>, Box<dyn Error>> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    tracing::info!("Enumerating GPUs via DXGI...");

    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
    let mut gpus = Vec::new();
    let mut adapter_idx: u32 = 0;

    loop {
        let adapter = unsafe {
            match factory.EnumAdapters1(adapter_idx) {
                Ok(a)                                         => a,
                Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(e)                                       => return Err(Box::new(e)),
            }
        };

        let desc: DXGI_ADAPTER_DESC1 = unsafe { adapter.GetDesc1()? };

        // Skip Microsoft Basic Render Driver (VendorId == 0x1414, DeviceId == 0x8c)
        if desc.VendorId == 0x1414 && desc.DeviceId == 0x8c {
            adapter_idx += 1;
            continue;
        }

        // Convert wide string description to Rust String
        let name_wide: Vec<u16> = desc.Description.iter()
            .take_while(|&&c| c != 0)
            .copied()
            .collect();
        let device_name = OsString::from_wide(&name_wide)
            .to_string_lossy()
            .into_owned();

        let vram_mb = desc.DedicatedVideoMemory as u64 / (1024 * 1024);
        let vendor   = GPUVendor::from(desc.VendorId);
        let vendor_id = format!("{:04X}", desc.VendorId);
        let device_id = format!("{:04X}", desc.DeviceId);

        tracing::info!(
            "> [GPU:{}] {} | Vendor: {} | VRAM: {} MB",
            adapter_idx, device_name, vendor, vram_mb
        );

        gpus.push(GPUInfo {
            vendor,
            adapter_idx,
            device_name,
            vendor_id,
            device_id,
            vram_mb,
        });

        adapter_idx += 1;
    }

    Ok(gpus)
}

/// Non-Windows stub (compile guard — should never be called on Linux)
#[cfg(not(target_os = "windows"))]
pub fn get_gpus() -> Result<Vec<GPUInfo>, Box<dyn Error>> {
    Err("get_gpus() is only supported on Windows in this build".into())
}

// ─── Filter helpers (mirrors original API) ───────────────────────────────────

pub fn get_gpus_by_vendor(gpus: &[GPUInfo], vendor: GPUVendor) -> Vec<GPUInfo> {
    gpus.iter()
        .filter(|g| *g.vendor() == vendor)
        .cloned()
        .collect()
}

pub fn get_gpu_by_adapter_idx(gpus: &[GPUInfo], idx: u32) -> Option<GPUInfo> {
    gpus.iter().find(|g| g.adapter_idx == idx).cloned()
}

pub fn get_gpus_by_device_name(gpus: &[GPUInfo], name: &str) -> Vec<GPUInfo> {
    let name_lower = name.to_lowercase();
    gpus.iter()
        .filter(|g| g.device_name.to_lowercase().contains(&name_lower))
        .cloned()
        .collect()
}

pub fn get_primary_gpu(gpus: &[GPUInfo]) -> Option<&GPUInfo> {
    // Prefer NVIDIA RTX > AMD > Intel > Unknown
    gpus.iter()
        .min_by_key(|g| match g.vendor {
            GPUVendor::NVIDIA  => 0,
            GPUVendor::AMD     => 1,
            GPUVendor::INTEL   => 2,
            GPUVendor::UNKNOWN => 3,
        })
}
