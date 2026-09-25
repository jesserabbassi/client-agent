//! Native Windows collectors. All handles stay on the collector thread.
use crate::monitoring::models::telemetry::Gpu;
use std::collections::{HashMap, HashSet};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM},
        Graphics::Dxgi::*,
        System::Performance::*,
        UI::WindowsAndMessaging::*,
    },
    core::{BOOL, PCWSTR},
};

pub(super) fn visible_app_pids() -> Option<HashSet<u32>> {
    unsafe extern "system" fn visit(hwnd: HWND, context: LPARAM) -> BOOL {
        // SAFETY: EnumWindows calls synchronously; context points to our live set.
        unsafe {
            if IsWindowVisible(hwnd).as_bool() && GetWindow(hwnd, GW_OWNER).is_err() {
                let mut pid = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if pid != 0 {
                    (&mut *(context.0 as *mut HashSet<u32>)).insert(pid);
                }
            }
        }
        BOOL(1)
    }
    let mut result = HashSet::new();
    // SAFETY: result remains valid for the synchronous enumeration callback.
    unsafe {
        EnumWindows(Some(visit), LPARAM(&mut result as *mut _ as isize)).ok()?;
    }
    Some(result)
}

pub(super) struct WindowsGpus {
    query: PDH_HQUERY,
    engine: PDH_HCOUNTER,
    dedicated: PDH_HCOUNTER,
    shared: PDH_HCOUNTER,
}

impl WindowsGpus {
    pub fn new() -> Self {
        let mut result = Self {
            query: PDH_HQUERY::default(),
            engine: PDH_HCOUNTER::default(),
            dedicated: PDH_HCOUNTER::default(),
            shared: PDH_HCOUNTER::default(),
        };
        // SAFETY: PDH writes to live handle slots; the query is closed in Drop.
        unsafe {
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut result.query) == 0 {
                for (path, counter) in [
                    (r"\GPU Engine(*)\Utilization Percentage", &mut result.engine),
                    (
                        r"\GPU Adapter Memory(*)\Dedicated Usage",
                        &mut result.dedicated,
                    ),
                    (r"\GPU Adapter Memory(*)\Shared Usage", &mut result.shared),
                ] {
                    let path: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
                    PdhAddEnglishCounterW(result.query, PCWSTR(path.as_ptr()), 0, counter);
                }
                PdhCollectQueryData(result.query);
            }
        }
        result
    }

    pub fn collect(&mut self) -> Vec<Gpu> {
        // SAFETY: handles are owned by this collector and never used concurrently.
        unsafe {
            let counters_ok = !self.query.is_invalid() && PdhCollectQueryData(self.query) == 0;
            let engines = if counters_ok {
                counter_values(self.engine)
            } else {
                Vec::new()
            };
            let dedicated = if counters_ok {
                counter_values(self.dedicated)
            } else {
                Vec::new()
            };
            let shared = if counters_ok {
                counter_values(self.shared)
            } else {
                Vec::new()
            };
            let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else {
                return Vec::new();
            };
            let mut result = Vec::new();
            for index in 0..64 {
                let Ok(adapter) = factory.EnumAdapters1(index) else {
                    break;
                };
                let Ok(desc) = adapter.GetDesc1() else {
                    continue;
                };
                if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                    continue;
                }
                let id = format!(
                    "luid_0x{:08x}_0x{:08x}",
                    desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart
                );
                // Sum process contributions to each physical engine, then take the
                // busiest engine. Summing all engines can incorrectly exceed 100%.
                let mut engine_totals = HashMap::<String, f64>::new();
                for (name, value) in &engines {
                    if let Some((_, suffix)) = name.split_once(&format!("{id}_")) {
                        *engine_totals.entry(suffix.to_owned()).or_default() += value;
                    }
                }
                let utilization = engine_totals
                    .values()
                    .copied()
                    .reduce(f64::max)
                    .map(|n| n.clamp(0.0, 100.0).round() as u32);
                let memory_value = |values: &[(String, f64)]| {
                    let values: Vec<_> = values
                        .iter()
                        .filter(|(name, _)| name.starts_with(&format!("{id}_")))
                        .collect();
                    (!values.is_empty()).then(|| values.iter().map(|(_, v)| *v as u64).sum::<u64>())
                };
                let total = desc.DedicatedVideoMemory as u64;
                let used = memory_value(&dedicated);
                let shared_used = memory_value(&shared);
                let name_end = desc
                    .Description
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(desc.Description.len());
                result.push(Gpu {
                    id,
                    name: String::from_utf16_lossy(&desc.Description[..name_end]),
                    vendor: match desc.VendorId {
                        0x10de => "NVIDIA",
                        0x1002 => "AMD",
                        0x8086 => "Intel",
                        _ => "Other",
                    }
                    .into(),
                    source: "dxgi-pdh".into(),
                    utilization_percent: utilization,
                    memory_total_bytes: Some(total),
                    memory_used_bytes: used,
                    memory_free_bytes: used.map(|n| total.saturating_sub(n)),
                    shared_memory_used_bytes: shared_used,
                    ..Gpu::default()
                });
            }
            result
        }
    }
}

fn counter_values(counter: PDH_HCOUNTER) -> Vec<(String, f64)> {
    if counter.is_invalid() {
        return Vec::new();
    }
    // SAFETY: query stays alive; buffer is aligned for PDH items and remains alive
    // while copying the strings PDH places inside it. Retry a growing instance list.
    unsafe {
        for _ in 0..3 {
            let (mut bytes, mut count) = (0, 0);
            if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut bytes, &mut count, None)
                != PDH_MORE_DATA
                || bytes == 0
                || bytes > 4 * 1024 * 1024
            {
                return Vec::new();
            }
            let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
            let ptr = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            let status = PdhGetFormattedCounterArrayW(
                counter,
                PDH_FMT_DOUBLE,
                &mut bytes,
                &mut count,
                Some(ptr),
            );
            if status == PDH_MORE_DATA {
                continue;
            }
            if status != 0
                || count as usize
                    > buffer.len() * 8 / std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>()
            {
                return Vec::new();
            }
            return std::slice::from_raw_parts(ptr, count as usize)
                .iter()
                .filter_map(|item| {
                    if !matches!(
                        item.FmtValue.CStatus,
                        PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA
                    ) || item.szName.is_null()
                    {
                        return None;
                    }
                    let value = item.FmtValue.Anonymous.doubleValue;
                    if !value.is_finite() || value < 0.0 {
                        return None;
                    }
                    Some((item.szName.to_string().ok()?, value))
                })
                .collect();
        }
    }
    Vec::new()
}

impl Drop for WindowsGpus {
    fn drop(&mut self) {
        if !self.query.is_invalid() {
            // SAFETY: we exclusively own this query and all of its counters.
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }
}

/// Enumerate connected keyboard/mouse/HID devices without reading any input.
pub(crate) fn connected_peripherals()
-> Option<Vec<crate::monitoring::models::peripheral_status::PeripheralStatus>> {
    use crate::monitoring::models::peripheral_status::PeripheralStatus;
    use windows::Win32::UI::Input::*;
    // SAFETY: every API receives a correctly sized, initialized buffer. A device
    // change between size-query and read causes a retry on the next sample.
    unsafe {
        let mut count = 0;
        let size = std::mem::size_of::<RAWINPUTDEVICELIST>() as u32;
        if GetRawInputDeviceList(None, &mut count, size) == u32::MAX || count > 256 {
            return None;
        }
        if count == 0 {
            return Some(Vec::new());
        }
        let mut devices = vec![RAWINPUTDEVICELIST::default(); count as usize];
        let read = GetRawInputDeviceList(Some(devices.as_mut_ptr()), &mut count, size);
        if read == u32::MAX || read as usize > devices.len() {
            return None;
        }
        let mut result = Vec::new();
        for device in devices.iter().take(read as usize) {
            let mut length = 0;
            if GetRawInputDeviceInfoW(Some(device.hDevice), RIDI_DEVICENAME, None, &mut length)
                == u32::MAX
                || length == 0
                || length > 4096
            {
                return None;
            }
            let mut name = vec![0u16; length as usize + 1];
            if GetRawInputDeviceInfoW(
                Some(device.hDevice),
                RIDI_DEVICENAME,
                Some(name.as_mut_ptr().cast()),
                &mut length,
            ) == u32::MAX
            {
                return None;
            }
            let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
            let kind = match device.dwType {
                RIM_TYPEKEYBOARD => "keyboard",
                RIM_TYPEMOUSE => "mouse",
                _ => "hid",
            };
            result.push(PeripheralStatus {
                device_id: String::from_utf16_lossy(&name[..end]),
                device_name: match kind {
                    "keyboard" => "Keyboard",
                    "mouse" => "Mouse",
                    _ => "HID device",
                }
                .into(),
                device_type: kind.into(),
                connected: true,
            });
        }
        result.sort_by(|a, b| a.device_id.cmp(&b.device_id));
        result.dedup_by(|a, b| a.device_id == b.device_id);
        Some(result)
    }
}
