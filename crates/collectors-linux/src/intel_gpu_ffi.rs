//! Minimal Level Zero Sysman ABI mirror for the read-only Intel GPU collector.
//!
//! The declarations below mirror the official Level Zero 1.20.41 `ze_api.h` and
//! `zes_api.h` definitions. This is the sole unsafe boundary: function pointers
//! are resolved from a retained `libze_loader` handle, all out-structures are
//! initialized with their documented structure type, and returned handles are
//! passed only to the matching Sysman functions.

use super::intel_gpu::{EnergyCounter, EngineCounter, IntelSourceSnapshot, RawGpuSample};
use libloading::{Library, Symbol};
use std::ffi::{c_char, c_void};
use std::ptr;

const ZE_SUCCESS: i32 = 0;
const ZE_DEVICE_TYPE_GPU: i32 = 1;
const INTEL_PCI_VENDOR_ID: u32 = 0x8086;
const MAX_ENUMERATED_HANDLES: u32 = 256;

type Handle = *mut c_void;
type ZeInit = unsafe extern "C" fn(u32) -> i32;
type ZesInit = unsafe extern "C" fn(u32) -> i32;
type ZeDriverGet = unsafe extern "C" fn(*mut u32, *mut Handle) -> i32;
type ZeDeviceGet = unsafe extern "C" fn(Handle, *mut u32, *mut Handle) -> i32;
type ZeDeviceGetProperties = unsafe extern "C" fn(Handle, *mut ZeDeviceProperties) -> i32;
type ZesDevicePciGetProperties = unsafe extern "C" fn(Handle, *mut ZesPciProperties) -> i32;
type ZesDeviceEnumEngineGroups = unsafe extern "C" fn(Handle, *mut u32, *mut Handle) -> i32;
type ZesEngineGetProperties = unsafe extern "C" fn(Handle, *mut ZesEngineProperties) -> i32;
type ZesEngineGetActivity = unsafe extern "C" fn(Handle, *mut ZesEngineStats) -> i32;
type ZesDeviceEnumMemoryModules = unsafe extern "C" fn(Handle, *mut u32, *mut Handle) -> i32;
type ZesMemoryGetProperties = unsafe extern "C" fn(Handle, *mut ZesMemoryProperties) -> i32;
type ZesMemoryGetState = unsafe extern "C" fn(Handle, *mut ZesMemoryState) -> i32;
type ZesDeviceEnumTemperatureSensors = unsafe extern "C" fn(Handle, *mut u32, *mut Handle) -> i32;
type ZesTemperatureGetProperties =
    unsafe extern "C" fn(Handle, *mut ZesTemperatureProperties) -> i32;
type ZesTemperatureGetState = unsafe extern "C" fn(Handle, *mut f64) -> i32;
type ZesDeviceEnumFrequencyDomains = unsafe extern "C" fn(Handle, *mut u32, *mut Handle) -> i32;
type ZesFrequencyGetProperties = unsafe extern "C" fn(Handle, *mut ZesFrequencyProperties) -> i32;
type ZesFrequencyGetState = unsafe extern "C" fn(Handle, *mut ZesFrequencyState) -> i32;
type ZesDeviceEnumPowerDomains = unsafe extern "C" fn(Handle, *mut u32, *mut Handle) -> i32;
type ZesPowerGetProperties = unsafe extern "C" fn(Handle, *mut ZesPowerProperties) -> i32;
type ZesPowerGetEnergyCounter = unsafe extern "C" fn(Handle, *mut ZesPowerEnergyCounter) -> i32;

pub(super) struct LevelZeroApi {
    _library: Library,
    ze_driver_get: ZeDriverGet,
    ze_device_get: ZeDeviceGet,
    ze_device_get_properties: ZeDeviceGetProperties,
    zes_device_pci_get_properties: Option<ZesDevicePciGetProperties>,
    zes_device_enum_engine_groups: Option<ZesDeviceEnumEngineGroups>,
    zes_engine_get_properties: Option<ZesEngineGetProperties>,
    zes_engine_get_activity: Option<ZesEngineGetActivity>,
    zes_device_enum_memory_modules: Option<ZesDeviceEnumMemoryModules>,
    zes_memory_get_properties: Option<ZesMemoryGetProperties>,
    zes_memory_get_state: Option<ZesMemoryGetState>,
    zes_device_enum_temperature_sensors: Option<ZesDeviceEnumTemperatureSensors>,
    zes_temperature_get_properties: Option<ZesTemperatureGetProperties>,
    zes_temperature_get_state: Option<ZesTemperatureGetState>,
    zes_device_enum_frequency_domains: Option<ZesDeviceEnumFrequencyDomains>,
    zes_frequency_get_properties: Option<ZesFrequencyGetProperties>,
    zes_frequency_get_state: Option<ZesFrequencyGetState>,
    zes_device_enum_power_domains: Option<ZesDeviceEnumPowerDomains>,
    zes_power_get_properties: Option<ZesPowerGetProperties>,
    zes_power_get_energy_counter: Option<ZesPowerGetEnergyCounter>,
}

impl LevelZeroApi {
    pub(super) fn load() -> Result<Self, ()> {
        let library = unsafe { Library::new("libze_loader.so.1") }
            .ok()
            .or_else(|| unsafe { Library::new("libze_loader.so") }.ok())
            .ok_or(())?;

        let ze_init = required_symbol::<ZeInit>(&library, b"zeInit\0")?;
        let zes_init = required_symbol::<ZesInit>(&library, b"zesInit\0")?;
        let ze_driver_get = required_symbol::<ZeDriverGet>(&library, b"zeDriverGet\0")?;
        let ze_device_get = required_symbol::<ZeDeviceGet>(&library, b"zeDeviceGet\0")?;
        let ze_device_get_properties =
            required_symbol::<ZeDeviceGetProperties>(&library, b"zeDeviceGetProperties\0")?;

        // Level Zero initialization is process-wide and documented as thread-safe/idempotent.
        if unsafe { ze_init(0) } != ZE_SUCCESS || unsafe { zes_init(0) } != ZE_SUCCESS {
            return Err(());
        }

        Ok(Self {
            zes_device_pci_get_properties: optional_symbol(
                &library,
                b"zesDevicePciGetProperties\0",
            ),
            zes_device_enum_engine_groups: optional_symbol(
                &library,
                b"zesDeviceEnumEngineGroups\0",
            ),
            zes_engine_get_properties: optional_symbol(&library, b"zesEngineGetProperties\0"),
            zes_engine_get_activity: optional_symbol(&library, b"zesEngineGetActivity\0"),
            zes_device_enum_memory_modules: optional_symbol(
                &library,
                b"zesDeviceEnumMemoryModules\0",
            ),
            zes_memory_get_properties: optional_symbol(&library, b"zesMemoryGetProperties\0"),
            zes_memory_get_state: optional_symbol(&library, b"zesMemoryGetState\0"),
            zes_device_enum_temperature_sensors: optional_symbol(
                &library,
                b"zesDeviceEnumTemperatureSensors\0",
            ),
            zes_temperature_get_properties: optional_symbol(
                &library,
                b"zesTemperatureGetProperties\0",
            ),
            zes_temperature_get_state: optional_symbol(&library, b"zesTemperatureGetState\0"),
            zes_device_enum_frequency_domains: optional_symbol(
                &library,
                b"zesDeviceEnumFrequencyDomains\0",
            ),
            zes_frequency_get_properties: optional_symbol(&library, b"zesFrequencyGetProperties\0"),
            zes_frequency_get_state: optional_symbol(&library, b"zesFrequencyGetState\0"),
            zes_device_enum_power_domains: optional_symbol(
                &library,
                b"zesDeviceEnumPowerDomains\0",
            ),
            zes_power_get_properties: optional_symbol(&library, b"zesPowerGetProperties\0"),
            zes_power_get_energy_counter: optional_symbol(&library, b"zesPowerGetEnergyCounter\0"),
            _library: library,
            ze_driver_get,
            ze_device_get,
            ze_device_get_properties,
        })
    }

    pub(super) fn collect_devices(&self) -> Result<IntelSourceSnapshot, ()> {
        let drivers =
            enumerate_handles(|count, handles| unsafe { (self.ze_driver_get)(count, handles) })?;
        let mut snapshot = IntelSourceSnapshot::default();

        for driver in drivers {
            let devices = match enumerate_handles(|count, handles| unsafe {
                (self.ze_device_get)(driver, count, handles)
            }) {
                Ok(devices) => devices,
                Err(()) => {
                    snapshot.had_device_error = true;
                    continue;
                }
            };

            for device in devices {
                let mut properties = ZeDeviceProperties::new();
                if unsafe { (self.ze_device_get_properties)(device, &mut properties) } != ZE_SUCCESS
                {
                    snapshot.had_device_error = true;
                    continue;
                }
                if properties.device_type != ZE_DEVICE_TYPE_GPU
                    || properties.vendor_id != INTEL_PCI_VENDOR_ID
                {
                    continue;
                }

                match self.read_gpu(device, &properties) {
                    Some(gpu) => snapshot.devices.push(gpu),
                    None => snapshot.had_device_error = true,
                }
            }
        }

        Ok(snapshot)
    }

    fn read_gpu(&self, device: Handle, properties: &ZeDeviceProperties) -> Option<RawGpuSample> {
        let bdf = self.read_pci_bdf(device);
        let id = uuid_string(&properties.uuid)
            .map(|uuid| format!("intel-{uuid}"))
            .or_else(|| bdf.map(|address| format!("intel-{address}")))?;
        let name = device_name(&properties.name);
        let memory = self.read_device_memory(device);

        Some(RawGpuSample {
            id,
            name: if name.is_empty() {
                "Intel GPU".to_owned()
            } else {
                name
            },
            engine: self.read_engine_activity(device),
            memory_used_bytes: memory.map(|memory| memory.0),
            memory_total_bytes: memory.map(|memory| memory.1),
            temperature_celsius: self.read_gpu_temperature(device),
            graphics_clock_mhz: self.read_graphics_clock(device),
            power: self.read_gpu_energy(device),
        })
    }

    fn read_pci_bdf(&self, device: Handle) -> Option<String> {
        let function = self.zes_device_pci_get_properties?;
        let mut properties = ZesPciProperties::new();
        if unsafe { function(device, &mut properties) } != ZE_SUCCESS {
            return None;
        }
        if properties.address.domain > u16::MAX as u32
            || properties.address.bus > u8::MAX as u32
            || properties.address.device > 31
            || properties.address.function > 7
        {
            return None;
        }
        Some(format!(
            "{:04x}:{:02x}:{:02x}.{}",
            properties.address.domain,
            properties.address.bus,
            properties.address.device,
            properties.address.function
        ))
    }

    fn read_engine_activity(&self, device: Handle) -> Option<EngineCounter> {
        let enumerate = self.zes_device_enum_engine_groups?;
        let get_properties = self.zes_engine_get_properties?;
        let get_activity = self.zes_engine_get_activity?;
        let handles =
            enumerate_handles(|count, output| unsafe { enumerate(device, count, output) }).ok()?;
        let mut all_groups = Vec::new();

        for handle in handles {
            let mut properties = ZesEngineProperties::new();
            if unsafe { get_properties(handle, &mut properties) } != ZE_SUCCESS {
                return None;
            }
            if properties.group_type == ZES_ENGINE_GROUP_ALL && properties.on_subdevice == 0 {
                all_groups.push(handle);
            }
        }
        if all_groups.len() != 1 {
            return None;
        }

        let handle = all_groups[0];
        let mut stats = ZesEngineStats::new();
        if unsafe { get_activity(handle, &mut stats) } != ZE_SUCCESS {
            return None;
        }
        Some(EngineCounter {
            handle: handle as usize,
            active_time: stats.active_time,
            timestamp: stats.timestamp,
        })
    }

    fn read_device_memory(&self, device: Handle) -> Option<(u64, u64)> {
        let enumerate = self.zes_device_enum_memory_modules?;
        let get_properties = self.zes_memory_get_properties?;
        let get_state = self.zes_memory_get_state?;
        let handles =
            enumerate_handles(|count, output| unsafe { enumerate(device, count, output) }).ok()?;
        let mut local_memory = Vec::new();

        for handle in handles {
            let mut properties = ZesMemoryProperties::new();
            if unsafe { get_properties(handle, &mut properties) } != ZE_SUCCESS {
                return None;
            }
            if properties.location == ZES_MEM_LOC_DEVICE && properties.on_subdevice == 0 {
                local_memory.push((handle, properties.physical_size));
            }
        }
        if local_memory.len() != 1 {
            return None;
        }

        let (handle, physical_size) = local_memory[0];
        if physical_size == 0 {
            return None;
        }
        let mut state = ZesMemoryState::new();
        if unsafe { get_state(handle, &mut state) } != ZE_SUCCESS || state.free > physical_size {
            return None;
        }
        Some((physical_size - state.free, physical_size))
    }

    fn read_gpu_temperature(&self, device: Handle) -> Option<u32> {
        let enumerate = self.zes_device_enum_temperature_sensors?;
        let get_properties = self.zes_temperature_get_properties?;
        let get_state = self.zes_temperature_get_state?;
        let handles =
            enumerate_handles(|count, output| unsafe { enumerate(device, count, output) }).ok()?;
        let mut gpu_sensors = Vec::new();

        for handle in handles {
            let mut properties = ZesTemperatureProperties::new();
            if unsafe { get_properties(handle, &mut properties) } != ZE_SUCCESS {
                return None;
            }
            if properties.sensor_type == ZES_TEMP_SENSORS_GPU && properties.on_subdevice == 0 {
                gpu_sensors.push(handle);
            }
        }
        if gpu_sensors.len() != 1 {
            return None;
        }

        let mut temperature_celsius = 0.0;
        if unsafe { get_state(gpu_sensors[0], &mut temperature_celsius) } != ZE_SUCCESS
            || !temperature_celsius.is_finite()
            || temperature_celsius < 0.0
            || temperature_celsius > f64::from(u32::MAX)
        {
            return None;
        }
        Some(temperature_celsius.round() as u32)
    }

    fn read_graphics_clock(&self, device: Handle) -> Option<u32> {
        let enumerate = self.zes_device_enum_frequency_domains?;
        let get_properties = self.zes_frequency_get_properties?;
        let get_state = self.zes_frequency_get_state?;
        let handles =
            enumerate_handles(|count, output| unsafe { enumerate(device, count, output) }).ok()?;
        let mut gpu_domains = Vec::new();

        for handle in handles {
            let mut properties = ZesFrequencyProperties::new();
            if unsafe { get_properties(handle, &mut properties) } != ZE_SUCCESS {
                return None;
            }
            if properties.domain_type == ZES_FREQ_DOMAIN_GPU && properties.on_subdevice == 0 {
                gpu_domains.push(handle);
            }
        }
        if gpu_domains.len() != 1 {
            return None;
        }

        let mut state = ZesFrequencyState::new();
        if unsafe { get_state(gpu_domains[0], &mut state) } != ZE_SUCCESS
            || !state.actual_mhz.is_finite()
            || state.actual_mhz < 0.0
            || state.actual_mhz > f64::from(u32::MAX)
        {
            return None;
        }
        Some(state.actual_mhz.round() as u32)
    }

    fn read_gpu_energy(&self, device: Handle) -> Option<EnergyCounter> {
        let enumerate = self.zes_device_enum_power_domains?;
        let get_properties = self.zes_power_get_properties?;
        let get_energy = self.zes_power_get_energy_counter?;
        let handles =
            enumerate_handles(|count, output| unsafe { enumerate(device, count, output) }).ok()?;
        let mut gpu_domains = Vec::new();

        for handle in handles {
            let mut domain_properties = ZesPowerDomainExpProperties::new();
            let mut properties = ZesPowerProperties::new(&mut domain_properties);
            if unsafe { get_properties(handle, &mut properties) } != ZE_SUCCESS {
                return None;
            }
            if properties.on_subdevice == 0
                && domain_properties.power_domain == ZES_POWER_DOMAIN_GPU
            {
                gpu_domains.push(handle);
            }
        }
        if gpu_domains.len() != 1 {
            return None;
        }

        let handle = gpu_domains[0];
        let mut counter = ZesPowerEnergyCounter::new();
        if unsafe { get_energy(handle, &mut counter) } != ZE_SUCCESS {
            return None;
        }
        Some(EnergyCounter {
            handle: handle as usize,
            energy_microjoules: counter.energy,
            timestamp_microseconds: counter.timestamp,
        })
    }
}

fn enumerate_handles(
    mut enumerate: impl FnMut(*mut u32, *mut Handle) -> i32,
) -> Result<Vec<Handle>, ()> {
    let mut count = 0;
    if enumerate(&mut count, ptr::null_mut()) != ZE_SUCCESS || count > MAX_ENUMERATED_HANDLES {
        return Err(());
    }
    if count == 0 {
        return Ok(Vec::new());
    }

    let mut handles = vec![ptr::null_mut(); count as usize];
    if enumerate(&mut count, handles.as_mut_ptr()) != ZE_SUCCESS || count as usize > handles.len() {
        return Err(());
    }
    handles.truncate(count as usize);
    handles.retain(|handle| !handle.is_null());
    Ok(handles)
}

fn required_symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, ()> {
    let symbol: Symbol<'_, T> = unsafe { library.get(name).map_err(|_| ())? };
    Ok(*symbol)
}

fn optional_symbol<T: Copy>(library: &Library, name: &[u8]) -> Option<T> {
    let symbol: Symbol<'_, T> = unsafe { library.get(name).ok()? };
    Some(*symbol)
}

fn uuid_string(uuid: &[u8; 16]) -> Option<String> {
    if uuid.iter().all(|byte| *byte == 0) {
        return None;
    }
    let mut value = String::with_capacity(32);
    for byte in uuid {
        value.push_str(&format!("{byte:02x}"));
    }
    Some(value)
}

fn device_name(name: &[c_char; 256]) -> String {
    let bytes = name
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| *byte as u8)
        .collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).trim().to_owned()
}

const ZE_STRUCTURE_TYPE_DEVICE_PROPERTIES: i32 = 0x3;
const ZES_STRUCTURE_TYPE_PCI_PROPERTIES: i32 = 0x2;
const ZES_STRUCTURE_TYPE_ENGINE_PROPERTIES: i32 = 0x5;
const ZES_STRUCTURE_TYPE_FREQ_PROPERTIES: i32 = 0x9;
const ZES_STRUCTURE_TYPE_MEM_PROPERTIES: i32 = 0xb;
const ZES_STRUCTURE_TYPE_POWER_PROPERTIES: i32 = 0xd;
const ZES_STRUCTURE_TYPE_TEMP_PROPERTIES: i32 = 0x14;
const ZES_STRUCTURE_TYPE_FREQ_STATE: i32 = 0x1b;
const ZES_STRUCTURE_TYPE_MEM_STATE: i32 = 0x1e;
const ZES_STRUCTURE_TYPE_POWER_DOMAIN_EXP_PROPERTIES: i32 = 0x0002_0001;
const ZES_ENGINE_GROUP_ALL: i32 = 0;
const ZES_MEM_LOC_DEVICE: i32 = 1;
const ZES_TEMP_SENSORS_GPU: i32 = 1;
const ZES_FREQ_DOMAIN_GPU: i32 = 0;
const ZES_POWER_DOMAIN_GPU: i32 = 5;

#[repr(C)]
struct ZeDeviceProperties {
    stype: i32,
    p_next: *mut c_void,
    device_type: i32,
    vendor_id: u32,
    device_id: u32,
    flags: u32,
    subdevice_id: u32,
    core_clock_rate: u32,
    max_mem_alloc_size: u64,
    max_hardware_contexts: u32,
    max_command_queue_priority: u32,
    num_threads_per_eu: u32,
    physical_eu_simd_width: u32,
    num_eus_per_subslice: u32,
    num_subslices_per_slice: u32,
    num_slices: u32,
    timer_resolution: u64,
    timestamp_valid_bits: u32,
    kernel_timestamp_valid_bits: u32,
    uuid: [u8; 16],
    name: [c_char; 256],
}

impl ZeDeviceProperties {
    fn new() -> Self {
        Self {
            stype: ZE_STRUCTURE_TYPE_DEVICE_PROPERTIES,
            p_next: ptr::null_mut(),
            device_type: 0,
            vendor_id: 0,
            device_id: 0,
            flags: 0,
            subdevice_id: 0,
            core_clock_rate: 0,
            max_mem_alloc_size: 0,
            max_hardware_contexts: 0,
            max_command_queue_priority: 0,
            num_threads_per_eu: 0,
            physical_eu_simd_width: 0,
            num_eus_per_subslice: 0,
            num_subslices_per_slice: 0,
            num_slices: 0,
            timer_resolution: 0,
            timestamp_valid_bits: 0,
            kernel_timestamp_valid_bits: 0,
            uuid: [0; 16],
            name: [0; 256],
        }
    }
}

#[repr(C)]
struct ZesPciAddress {
    domain: u32,
    bus: u32,
    device: u32,
    function: u32,
}

#[repr(C)]
struct ZesPciSpeed {
    generation: i32,
    width: i32,
    max_bandwidth: i64,
}

#[repr(C)]
struct ZesPciProperties {
    stype: i32,
    p_next: *mut c_void,
    address: ZesPciAddress,
    max_speed: ZesPciSpeed,
    have_bandwidth_counters: u8,
    have_packet_counters: u8,
    have_replay_counters: u8,
}

impl ZesPciProperties {
    fn new() -> Self {
        Self {
            stype: ZES_STRUCTURE_TYPE_PCI_PROPERTIES,
            p_next: ptr::null_mut(),
            address: ZesPciAddress {
                domain: 0,
                bus: 0,
                device: 0,
                function: 0,
            },
            max_speed: ZesPciSpeed {
                generation: 0,
                width: 0,
                max_bandwidth: 0,
            },
            have_bandwidth_counters: 0,
            have_packet_counters: 0,
            have_replay_counters: 0,
        }
    }
}

#[repr(C)]
struct ZesEngineProperties {
    stype: i32,
    p_next: *mut c_void,
    group_type: i32,
    on_subdevice: u8,
    subdevice_id: u32,
}

impl ZesEngineProperties {
    fn new() -> Self {
        Self {
            stype: ZES_STRUCTURE_TYPE_ENGINE_PROPERTIES,
            p_next: ptr::null_mut(),
            group_type: 0,
            on_subdevice: 0,
            subdevice_id: 0,
        }
    }
}

#[repr(C)]
struct ZesEngineStats {
    active_time: u64,
    timestamp: u64,
}

impl ZesEngineStats {
    fn new() -> Self {
        Self {
            active_time: 0,
            timestamp: 0,
        }
    }
}

#[repr(C)]
struct ZesMemoryProperties {
    stype: i32,
    p_next: *mut c_void,
    memory_type: i32,
    on_subdevice: u8,
    subdevice_id: u32,
    location: i32,
    physical_size: u64,
    bus_width: i32,
    num_channels: i32,
}

impl ZesMemoryProperties {
    fn new() -> Self {
        Self {
            stype: ZES_STRUCTURE_TYPE_MEM_PROPERTIES,
            p_next: ptr::null_mut(),
            memory_type: 0,
            on_subdevice: 0,
            subdevice_id: 0,
            location: 0,
            physical_size: 0,
            bus_width: 0,
            num_channels: 0,
        }
    }
}

#[repr(C)]
struct ZesMemoryState {
    stype: i32,
    p_next: *const c_void,
    health: i32,
    free: u64,
    size: u64,
}

impl ZesMemoryState {
    fn new() -> Self {
        Self {
            stype: ZES_STRUCTURE_TYPE_MEM_STATE,
            p_next: ptr::null(),
            health: 0,
            free: 0,
            size: 0,
        }
    }
}

#[repr(C)]
struct ZesTemperatureProperties {
    stype: i32,
    p_next: *mut c_void,
    sensor_type: i32,
    on_subdevice: u8,
    subdevice_id: u32,
    max_temperature: f64,
    is_critical_temp_supported: u8,
    is_threshold1_supported: u8,
    is_threshold2_supported: u8,
}

impl ZesTemperatureProperties {
    fn new() -> Self {
        Self {
            stype: ZES_STRUCTURE_TYPE_TEMP_PROPERTIES,
            p_next: ptr::null_mut(),
            sensor_type: 0,
            on_subdevice: 0,
            subdevice_id: 0,
            max_temperature: 0.0,
            is_critical_temp_supported: 0,
            is_threshold1_supported: 0,
            is_threshold2_supported: 0,
        }
    }
}

#[repr(C)]
struct ZesFrequencyProperties {
    stype: i32,
    p_next: *mut c_void,
    domain_type: i32,
    on_subdevice: u8,
    subdevice_id: u32,
    can_control: u8,
    is_throttle_event_supported: u8,
    min_mhz: f64,
    max_mhz: f64,
}

impl ZesFrequencyProperties {
    fn new() -> Self {
        Self {
            stype: ZES_STRUCTURE_TYPE_FREQ_PROPERTIES,
            p_next: ptr::null_mut(),
            domain_type: 0,
            on_subdevice: 0,
            subdevice_id: 0,
            can_control: 0,
            is_throttle_event_supported: 0,
            min_mhz: 0.0,
            max_mhz: 0.0,
        }
    }
}

#[repr(C)]
struct ZesFrequencyState {
    stype: i32,
    p_next: *const c_void,
    current_voltage: f64,
    request_mhz: f64,
    tdp_mhz: f64,
    efficient_mhz: f64,
    actual_mhz: f64,
    throttle_reasons: u32,
}

impl ZesFrequencyState {
    fn new() -> Self {
        Self {
            stype: ZES_STRUCTURE_TYPE_FREQ_STATE,
            p_next: ptr::null(),
            current_voltage: 0.0,
            request_mhz: 0.0,
            tdp_mhz: 0.0,
            efficient_mhz: 0.0,
            actual_mhz: 0.0,
            throttle_reasons: 0,
        }
    }
}

#[repr(C)]
struct ZesPowerProperties {
    stype: i32,
    p_next: *mut c_void,
    on_subdevice: u8,
    subdevice_id: u32,
    can_control: u8,
    is_energy_threshold_supported: u8,
    default_limit: i32,
    min_limit: i32,
    max_limit: i32,
}

impl ZesPowerProperties {
    fn new(extension: &mut ZesPowerDomainExpProperties) -> Self {
        Self {
            stype: ZES_STRUCTURE_TYPE_POWER_PROPERTIES,
            p_next: (extension as *mut ZesPowerDomainExpProperties).cast::<c_void>(),
            on_subdevice: 0,
            subdevice_id: 0,
            can_control: 0,
            is_energy_threshold_supported: 0,
            default_limit: 0,
            min_limit: 0,
            max_limit: 0,
        }
    }
}

#[repr(C)]
struct ZesPowerDomainExpProperties {
    stype: i32,
    p_next: *mut c_void,
    power_domain: i32,
}

impl ZesPowerDomainExpProperties {
    fn new() -> Self {
        Self {
            stype: ZES_STRUCTURE_TYPE_POWER_DOMAIN_EXP_PROPERTIES,
            p_next: ptr::null_mut(),
            power_domain: 0,
        }
    }
}

#[repr(C)]
struct ZesPowerEnergyCounter {
    energy: u64,
    timestamp: u64,
}

impl ZesPowerEnergyCounter {
    fn new() -> Self {
        Self {
            energy: 0,
            timestamp: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Handle, INTEL_PCI_VENDOR_ID, IntelSourceSnapshot, LevelZeroApi, MAX_ENUMERATED_HANDLES,
        ZE_DEVICE_TYPE_GPU, ZeDeviceGet, ZeDeviceGetProperties, ZeDeviceProperties, ZeDriverGet,
        device_name, enumerate_handles, uuid_string,
    };
    use libloading::Library;
    use std::ffi::{c_char, c_void};
    use std::mem::{offset_of, size_of};

    #[test]
    fn handle_enumeration_is_bounded_and_preserves_reported_handles() {
        let mut calls = 0;
        let handles = enumerate_handles(|count, output| {
            calls += 1;
            if output.is_null() {
                unsafe { *count = 2 };
            } else {
                unsafe {
                    *count = 2;
                    let handle = std::ptr::dangling_mut::<c_void>();
                    *output = handle;
                    *output.add(1) = handle;
                }
            }
            0
        })
        .expect("bounded fake enumeration succeeds");

        assert_eq!(calls, 2);
        assert_eq!(handles.len(), 2);
        assert!(handles.iter().all(|handle| !handle.is_null()));

        assert!(
            enumerate_handles(|count, _| {
                unsafe { *count = MAX_ENUMERATED_HANDLES + 1 };
                0
            })
            .is_err()
        );
    }

    #[test]
    fn uuid_and_device_name_conversion_reject_empty_identity_and_trim_text() {
        assert_eq!(uuid_string(&[0; 16]), None);
        assert_eq!(
            uuid_string(&[0x01, 0x02, 0x0a, 0x0f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
            Some("01020a0f000000000000000000000001".to_owned())
        );
        let mut name = [0_i8; 256];
        name[0] = b' ' as i8;
        name[1] = b'G' as i8;
        name[2] = b'P' as i8;
        name[3] = b'U' as i8;
        name[4] = b' ' as i8;
        assert_eq!(device_name(&name), "GPU");
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn mirrored_c_struct_layout_matches_the_official_64_bit_headers() {
        assert_eq!(size_of::<ZeDeviceProperties>(), 368);
        assert_eq!(offset_of!(ZeDeviceProperties, vendor_id), 20);
        assert_eq!(offset_of!(ZeDeviceProperties, uuid), 96);
        assert_eq!(offset_of!(ZeDeviceProperties, name), 112);
        assert_eq!(size_of::<super::ZesPciProperties>(), 56);
        assert_eq!(offset_of!(super::ZesPciProperties, address), 16);
        assert_eq!(size_of::<super::ZesEngineProperties>(), 32);
        assert_eq!(offset_of!(super::ZesEngineProperties, group_type), 16);
        assert_eq!(size_of::<super::ZesEngineStats>(), 16);
        assert_eq!(size_of::<super::ZesMemoryProperties>(), 48);
        assert_eq!(offset_of!(super::ZesMemoryProperties, physical_size), 32);
        assert_eq!(size_of::<super::ZesMemoryState>(), 40);
        assert_eq!(offset_of!(super::ZesMemoryState, free), 24);
        assert_eq!(size_of::<super::ZesTemperatureProperties>(), 48);
        assert_eq!(size_of::<super::ZesFrequencyProperties>(), 48);
        assert_eq!(size_of::<super::ZesFrequencyState>(), 64);
        assert_eq!(offset_of!(super::ZesFrequencyState, actual_mhz), 48);
        assert_eq!(size_of::<super::ZesPowerProperties>(), 40);
        assert_eq!(size_of::<super::ZesPowerDomainExpProperties>(), 24);
        assert_eq!(size_of::<super::ZesPowerEnergyCounter>(), 16);
    }

    static DRIVER: u8 = 0;
    static INTEL_GPU_A: u8 = 0;
    static NON_INTEL_GPU: u8 = 0;
    static INTEL_NPU: u8 = 0;
    static INTEL_GPU_B: u8 = 0;
    static FAILED_DEVICE: u8 = 0;

    unsafe extern "C" fn fake_driver_get(count: *mut u32, drivers: *mut Handle) -> i32 {
        if drivers.is_null() {
            unsafe { *count = 1 };
        } else {
            unsafe {
                *count = 1;
                *drivers = std::ptr::addr_of!(DRIVER).cast_mut().cast::<c_void>();
            }
        }
        0
    }

    unsafe extern "C" fn fake_device_get(
        _driver: Handle,
        count: *mut u32,
        devices: *mut Handle,
    ) -> i32 {
        if devices.is_null() {
            unsafe { *count = 5 };
        } else {
            unsafe {
                *count = 5;
                *devices.add(0) = std::ptr::addr_of!(INTEL_GPU_A).cast_mut().cast::<c_void>();
                *devices.add(1) = std::ptr::addr_of!(NON_INTEL_GPU)
                    .cast_mut()
                    .cast::<c_void>();
                *devices.add(2) = std::ptr::addr_of!(INTEL_NPU).cast_mut().cast::<c_void>();
                *devices.add(3) = std::ptr::addr_of!(INTEL_GPU_B).cast_mut().cast::<c_void>();
                *devices.add(4) = std::ptr::addr_of!(FAILED_DEVICE)
                    .cast_mut()
                    .cast::<c_void>();
            }
        }
        0
    }

    unsafe extern "C" fn fake_device_get_properties(
        device: Handle,
        output: *mut ZeDeviceProperties,
    ) -> i32 {
        if device
            == std::ptr::addr_of!(FAILED_DEVICE)
                .cast_mut()
                .cast::<c_void>()
        {
            return -1;
        }

        let mut properties = ZeDeviceProperties::new();
        properties.device_type = if device == std::ptr::addr_of!(INTEL_NPU).cast_mut().cast() {
            5
        } else {
            ZE_DEVICE_TYPE_GPU
        };
        properties.vendor_id = if device == std::ptr::addr_of!(NON_INTEL_GPU).cast_mut().cast() {
            0x10de
        } else {
            INTEL_PCI_VENDOR_ID
        };
        if device == std::ptr::addr_of!(INTEL_GPU_A).cast_mut().cast() {
            properties.uuid[0] = 0xa1;
            properties.name[0] = b'A' as c_char;
        } else {
            properties.uuid[0] = 0xb2;
            properties.name[0] = b'B' as c_char;
        }
        unsafe { output.write(properties) };
        0
    }

    #[test]
    fn discovery_filters_by_gpu_type_and_intel_vendor_and_preserves_other_valid_devices() {
        let library = unsafe { Library::new("libc.so.6") }.expect("load libc for API fake");
        let api = LevelZeroApi {
            _library: library,
            ze_driver_get: fake_driver_get as ZeDriverGet,
            ze_device_get: fake_device_get as ZeDeviceGet,
            ze_device_get_properties: fake_device_get_properties as ZeDeviceGetProperties,
            zes_device_pci_get_properties: None,
            zes_device_enum_engine_groups: None,
            zes_engine_get_properties: None,
            zes_engine_get_activity: None,
            zes_device_enum_memory_modules: None,
            zes_memory_get_properties: None,
            zes_memory_get_state: None,
            zes_device_enum_temperature_sensors: None,
            zes_temperature_get_properties: None,
            zes_temperature_get_state: None,
            zes_device_enum_frequency_domains: None,
            zes_frequency_get_properties: None,
            zes_frequency_get_state: None,
            zes_device_enum_power_domains: None,
            zes_power_get_properties: None,
            zes_power_get_energy_counter: None,
        };

        let snapshot: IntelSourceSnapshot = api.collect_devices().expect("fake enumeration");

        assert_eq!(snapshot.devices.len(), 2);
        assert_ne!(snapshot.devices[0].id, snapshot.devices[1].id);
        assert_eq!(snapshot.devices[0].name, "A");
        assert_eq!(snapshot.devices[1].name, "B");
        assert!(snapshot.had_device_error);
        assert!(
            snapshot
                .devices
                .iter()
                .all(|device| device.engine.is_none())
        );
        assert!(
            snapshot
                .devices
                .iter()
                .all(|device| device.id.starts_with("intel-"))
        );
    }
}
