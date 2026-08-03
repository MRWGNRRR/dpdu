use crate::{
    CopCtrlData, EcuUniqueRespData, ErrorData, EthSwitchState, EventItem, ExpRespData, ExtraInfo,
    FlagData, InfoData, IoByteArrayData, IoEntityAddressData, IoEntityStatusData,
    IoEventQueuePropertyData, IoFilter, IoFilterData, IoProgVoltageData, IpAddrInfo, ModuleData,
    ModuleItem, ParamByteFieldData, ParamItem, ParamLongFieldData, ParamStructAccessTiming,
    ParamStructFieldData, ParamStructSessionTiming, PduCpst, PduDataItem, PduIt, PduItem, PduPt,
    PduStatus, PinData, ResultData, RscConflictData, RscConflictItem, RscData, RscIdItem,
    RscIdItemData, RscStatusData, RscStatusItem, UniqueRespIdTableItem, VehicleIdRequest,
    VersionData,
};
use parking_lot::RwLock;
use std::ffi::{CStr, c_char, c_void};
use std::fmt::{Debug, Display, Formatter};
use std::sync::OnceLock;
use std::{fmt, slice};

/// Global configuration for debug formatting.
#[allow(missing_copy_implementations)]
#[derive(Debug, Clone)]
pub struct DebugOptions {
    /// Maximum number of elements displayed when formatting pointer-backed arrays.
    ///
    /// This limit applies to arrays whose length is provided separately from the
    /// pointer value.
    ///
    /// `None` disables the limit.
    pub max_collection_items: Option<usize>,

    /// Maximum number of bytes displayed when formatting pointer-backed byte buffers.
    ///
    /// This limit applies to raw byte arrays referenced by a pointer and size pair.
    ///
    /// `None` disables the limit.
    pub max_byte_items: Option<usize>,

    /// Maximum number of bytes displayed from C strings.
    ///
    /// `None` disables the limit.
    pub max_str_len: Option<usize>,
}

impl Default for DebugOptions {
    fn default() -> Self {
        Self {
            max_collection_items: Some(32),
            max_byte_items: Some(32),
            max_str_len: Some(32),
        }
    }
}

static DEBUG_OPTIONS: OnceLock<RwLock<DebugOptions>> = OnceLock::new();

#[allow(missing_docs)]
fn debug_options() -> &'static RwLock<DebugOptions> {
    DEBUG_OPTIONS.get_or_init(|| RwLock::new(DebugOptions::default()))
}

/// Updates global debug formatting options and returns old options.
pub fn set_debug_options(options: DebugOptions) -> DebugOptions {
    let mut opts = debug_options().write();
    let old = opts.clone();

    *opts = options;

    old
}

/// Returns current debug formatting options.
pub fn get_debug_options() -> DebugOptions {
    debug_options().read().clone()
}

/// Provides a debug representation of a value without modifying its original type.
///
/// This trait is intended for types that require a custom or safer representation
/// for debugging purposes, such as FFI structures containing raw pointers.
///
/// Implementations should return a lightweight view that borrows the original
/// value and implements [`Debug`].
pub trait DebugView {
    /// The type used as a debug representation.
    type Output<'a>: Debug
    where
        Self: 'a;

    /// Creates a debug view of this value.
    ///
    /// The returned value borrows the original object and is intended only for
    /// inspection and logging purposes.
    fn debug_view(&self) -> Self::Output<'_>;
}

/// Helper type for formatting raw pointers in logs.
///
/// Displays a pointer address in hexadecimal format (`0x...`)
/// or `<nullptr>` for null pointers.
///
/// This type stores only the pointer address and does not retain
/// ownership or provide access to the underlying memory.
#[derive(Clone, Copy)]
pub(crate) struct PtrRepr(usize);

impl<T> From<*const T> for PtrRepr {
    /// Creates a pointer representation from a constant raw pointer.
    fn from(ptr: *const T) -> Self {
        Self(ptr as usize)
    }
}

impl<T> From<*mut T> for PtrRepr {
    /// Creates a pointer representation from a mutable raw pointer.
    fn from(ptr: *mut T) -> Self {
        Self(ptr as usize)
    }
}

impl Debug for PtrRepr {
    /// Debugs the pointer as a hexadecimal address.
    ///
    /// Null pointers are represented as `<nullptr>`.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for PtrRepr {
    /// Formats the pointer as a hexadecimal address.
    ///
    /// Null pointers are represented as `<nullptr>`.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            write!(f, "<nullptr>")
        } else {
            write!(
                f,
                "{:p}",
                self.0 as *const (),
            )
        }
    }
}

/// Wrapper for formatting a slice using [`DebugView`] implementations.
///
/// This type is intended for debugging FFI structures containing arrays of
/// complex types that have a custom debug representation.
///
/// Each element of the wrapped slice is converted into its debug view before
/// being formatted. This allows avoiding direct [`Debug`] implementations
/// for FFI types containing raw pointers or other unsafe data.
///
/// # Examples
///
/// ```ignore
/// debug.field(
///     "expected_responses",
///     &DebugSlice::new(responses),
/// );
/// ```
pub struct DebugStructSlice<'a, T>(&'a [T]);

impl<'a, T> DebugStructSlice<'a, T> {
    /// Creates a new debug wrapper for the provided slice.
    pub fn new(value: &'a [T]) -> Self {
        Self(value)
    }
}

impl<T> Debug for DebugStructSlice<'_, T>
where
    T: DebugView,
    for<'a> T::Output<'a>: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let options = get_debug_options();

        let total_len = self.0.len();

        let len = options
            .max_collection_items
            .map(|max| total_len.min(max))
            .unwrap_or(total_len);

        let mut debug_list = f.debug_list();

        for item in &self.0[..len] {
            debug_list.entry(&item.debug_view());
        }

        if len < total_len {
            debug_list.entry(&"...");
        }

        debug_list.finish()
    }
}

/// Debug wrapper for formatting byte slices.
///
/// This type formats a byte slice as a hexadecimal byte sequence and optionally
/// limits the number of displayed bytes.
pub struct DebugByteSlice<'a>(&'a [u8]);

impl<'a> DebugByteSlice<'a> {
    /// Creates a new byte slice debug wrapper.
    pub fn new(data: &'a [u8]) -> Self {
        Self(data)
    }
}

impl Debug for DebugByteSlice<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let options = get_debug_options();

        let len = options
            .max_byte_items
            .map(|max| self.0.len().min(max))
            .unwrap_or(self.0.len());

        let bytes = &self.0[..len];

        for (index, byte) in bytes.iter().enumerate() {
            if index != 0 {
                write!(f, " ")?;
            }

            write!(f, "{byte:02X}")?;
        }

        if len < self.0.len() {
            write!(f, " ...")?;
        }

        Ok(())
    }
}

/// Debug wrapper for formatting DWORD slices.
///
/// This type formats a `u32` slice as a hexadecimal DWORD sequence and
/// optionally limits the number of displayed elements.
pub struct DebugDwordSlice<'a>(&'a [u32]);

impl<'a> DebugDwordSlice<'a> {
    /// Creates a new DWORD slice debug wrapper.
    pub fn new(data: &'a [u32]) -> Self {
        Self(data)
    }
}

impl Debug for DebugDwordSlice<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let options = get_debug_options();

        let len = options
            .max_collection_items
            .map(|max| self.0.len().min(max))
            .unwrap_or(self.0.len());

        let values = &self.0[..len];

        for (index, value) in values.iter().enumerate() {
            if index != 0 {
                write!(f, " ")?;
            }

            write!(f, "{value:#010X}")?;
        }

        if len < self.0.len() {
            write!(f, " ...")?;
        }

        Ok(())
    }
}

/// Debug representation of a null-terminated C string.
///
/// The pointer is expected to reference a valid null-terminated ASCII string.
pub struct CStrDebug(*const c_char);

impl CStrDebug {
    /// Creates a debug representation from a raw C string pointer.
    pub fn new(ptr: *const u8) -> Self {
        Self(ptr as *const c_char)
    }
}

impl Debug for CStrDebug {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.0.is_null() {
            return f.write_str("<nullptr>");
        }

        let options = get_debug_options();

        unsafe {
            let value = CStr::from_ptr(self.0);

            let bytes = value.to_bytes();

            let len = options
                .max_str_len
                .map(|max| bytes.len().min(max))
                .unwrap_or(bytes.len());

            let bytes = &bytes[..len];

            match str::from_utf8(bytes) {
                Ok(value) => {
                    if len < value.len() {
                        write!(f, "{value:?}...")
                    } else {
                        Debug::fmt(&value, f)
                    }
                }

                Err(_) => DebugByteSlice::new(bytes).fmt(f),
            }
        }
    }
}

#[allow(missing_docs)]
pub struct PduItemDebug<'a>(&'a PduItem);

impl DebugView for PduItem {
    type Output<'a>
        = PduItemDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        PduItemDebug(self)
    }
}

impl<'a> Debug for PduItemDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PduItem")
            .field("item_type", &self.0.item_type)
            .finish()
    }
}

#[allow(missing_docs)]
pub struct EventItemDebug<'a>(&'a EventItem);

impl DebugView for EventItem {
    type Output<'a>
        = EventItemDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        EventItemDebug(self)
    }
}

impl<'a> Debug for EventItemDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let item = self.0;

        let mut debug = f.debug_struct("EventItem");

        debug
            .field("item_type", &item.item_type)
            .field(
                "item_type_value",
                &format_args!("{:#010X}", item.item_type as u32),
            )
            .field("h_cop", &item.h_cop)
            .field("p_cop_tag", &PtrRepr::from(item.p_cop_tag))
            .field("timestamp", &item.timestamp)
            .field("p_data", &PtrRepr::from(item.p_data));

        debug_pdu_item_data(&mut debug, item.item_type, item.p_data);

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct PduDataItemDebug<'a>(&'a PduDataItem);

impl DebugView for PduDataItem {
    type Output<'a>
        = PduDataItemDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        PduDataItemDebug(self)
    }
}

impl<'a> Debug for PduDataItemDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let item = self.0;

        let mut debug = f.debug_struct("PduDataItem");

        debug
            .field("item_type", &item.item_type)
            .field(
                "item_type_value",
                &format_args!("{:#010X}", item.item_type as u32),
            )
            .field("p_data", &PtrRepr::from(item.p_data));

        debug_pdu_item_data(&mut debug, item.item_type, item.p_data);

        debug.finish()
    }
}

fn debug_pdu_item_data(
    debug: &mut fmt::DebugStruct<'_, '_>,
    item_type: PduIt,
    p_data: *mut c_void,
) {
    unsafe {
        if p_data.is_null() {
            return;
        }

        match item_type {
            PduIt::IoUnum32 => {
                debug.field("data", &*(p_data as *const u32));
            }

            PduIt::IoProgVoltage => {
                debug.field("data", &*(p_data as *const u32));
            }

            PduIt::IoByteArray => {
                let value = &*(p_data as *const IoByteArrayData);

                debug.field("data", &value.debug_view());
            }

            PduIt::IoFilter => {
                let value = &*(p_data as *const IoFilter);

                debug.field("data", &value.debug_view());
            }

            PduIt::IoEventQueueProperty => {
                let value = &*(p_data as *const IoEventQueuePropertyData);

                debug.field("data", &value.debug_view());
            }

            PduIt::RscStatus => {
                let value = &*(p_data as *const RscStatusItem);

                debug.field("data", &value.debug_view());
            }

            PduIt::Param => {
                let value = &*(p_data as *const ParamItem);

                debug.field("data", &value.debug_view());
            }

            PduIt::Result => {
                let value = &*(p_data as *const ResultData);

                debug.field("data", &value.debug_view());
            }

            PduIt::Status => {
                let value = *(p_data as *const PduStatus);

                debug.field("data", &value);
            }

            PduIt::Error => {
                let value = &*(p_data as *const ErrorData);

                debug.field("data", &value.debug_view());
            }

            PduIt::Info => {
                let value = &*(p_data as *const InfoData);

                debug.field("data", &value.debug_view());
            }

            PduIt::RscId => {
                let value = &*(p_data as *const RscIdItem);

                debug.field("data", &value.debug_view());
            }

            PduIt::RscConflict => {
                let value = &*(p_data as *const RscConflictItem);

                debug.field("data", &value.debug_view());
            }

            PduIt::ModuleId => {
                let value = &*(p_data as *const ModuleItem);

                debug.field("data", &value.debug_view());
            }

            PduIt::UniqueRespIdTable => {
                let value = &*(p_data as *const UniqueRespIdTableItem);

                debug.field("data", &value.debug_view());
            }

            PduIt::IoVehicleIdRequest => {
                let value = &*(p_data as *const VehicleIdRequest);

                debug.field("data", &value.debug_view());
            }

            PduIt::EthSwitchState => {
                let value = &*(p_data as *const EthSwitchState);

                debug.field("data", &value.debug_view());
            }

            PduIt::EntityAddress => {
                let value = &*(p_data as *const IoEntityAddressData);

                debug.field("data", &value.debug_view());
            }

            PduIt::EntityStatus => {
                let value = &*(p_data as *const IoEntityStatusData);

                debug.field("data", &value.debug_view());
            }
        }
    }
}

#[allow(missing_docs)]
pub struct IoProgVoltageDataDebug<'a>(&'a IoProgVoltageData);

impl DebugView for IoProgVoltageData {
    type Output<'a>
        = IoProgVoltageDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        IoProgVoltageDataDebug(self)
    }
}

impl<'a> Debug for IoProgVoltageDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("IoProgVoltageData")
            .field("prog_voltage_mv", &self.0.prog_voltage_mv)
            .field("pin_on_dlc", &self.0.pin_on_dlc)
            .finish()
    }
}

#[allow(missing_docs)]
pub struct IoByteArrayDataDebug<'a>(&'a IoByteArrayData);

impl DebugView for IoByteArrayData {
    type Output<'a>
        = IoByteArrayDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        IoByteArrayDataDebug(self)
    }
}

impl<'a> Debug for IoByteArrayDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("IoByteArrayData");

        debug
            .field("data_size", &data.data_size)
            .field("p_data", &PtrRepr::from(data.p_data));

        unsafe {
            if !data.p_data.is_null() && data.data_size > 0 {
                let bytes = slice::from_raw_parts(data.p_data, data.data_size as usize);

                debug.field("data", &DebugByteSlice::new(bytes));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct IoFilterDebug<'a>(&'a IoFilter);

impl DebugView for IoFilter {
    type Output<'a>
        = IoFilterDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        IoFilterDebug(self)
    }
}

impl<'a> Debug for IoFilterDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("IoFilter");

        debug
            .field("num_filter_entries", &data.num_filter_entries)
            .field("p_filter_data", &PtrRepr::from(data.p_filter_data));

        unsafe {
            if !data.p_filter_data.is_null() && data.num_filter_entries > 0 {
                let filters =
                    slice::from_raw_parts(data.p_filter_data, data.num_filter_entries as usize);

                debug.field("filter_data", &DebugStructSlice::new(filters));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct IoFilterDataDebug<'a>(&'a IoFilterData);

impl DebugView for IoFilterData {
    type Output<'a>
        = IoFilterDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        IoFilterDataDebug(self)
    }
}

impl<'a> Debug for IoFilterDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("IoFilterData");

        debug
            .field("filter_type", &data.filter_type)
            .field("filter_type_value", &(data.filter_type as u32))
            .field("filter_number", &data.filter_number)
            .field("filter_compare_size", &data.filter_compare_size)
            .field(
                "filter_mask_msg",
                &DebugByteSlice::new(&data.filter_mask_msg),
            )
            .field(
                "filter_pattern_msg",
                &DebugByteSlice::new(&data.filter_pattern_msg),
            );

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct IoEventQueuePropertyDataDebug<'a>(&'a IoEventQueuePropertyData);

impl DebugView for IoEventQueuePropertyData {
    type Output<'a>
        = IoEventQueuePropertyDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        IoEventQueuePropertyDataDebug(self)
    }
}

impl<'a> Debug for IoEventQueuePropertyDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("IoEventQueuePropertyData");

        debug
            .field("queue_size", &data.queue_size)
            .field("queue_mode", &data.queue_mode)
            .field(
                "queue_mode_value",
                &format_args!("{:#010X}", data.queue_mode as u32),
            );

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct VehicleIdRequestDebug<'a>(&'a VehicleIdRequest);

impl DebugView for VehicleIdRequest {
    type Output<'a>
        = VehicleIdRequestDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        VehicleIdRequestDebug(self)
    }
}

impl<'a> Debug for VehicleIdRequestDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("VehicleIdRequest");

        debug
            .field("preselection_mode", &data.preselection_mode)
            .field(
                "preselection_mode_value",
                &format_args!("{:#04X}", data.preselection_mode as u8),
            )
            .field(
                "preselection_value",
                &CStrDebug::new(data.preselection_value),
            )
            .field("combination_mode", &data.combination_mode)
            .field(
                "combination_mode_value",
                &format_args!("{:#04X}", data.combination_mode as u8),
            )
            .field("vehicle_discovery_time", &data.vehicle_discovery_time)
            .field("num_destination_addresses", &data.num_destination_addresses)
            .field(
                "destination_addresses",
                &PtrRepr::from(data.destination_addresses),
            );

        unsafe {
            if !data.destination_addresses.is_null() && data.num_destination_addresses > 0 {
                let addresses = slice::from_raw_parts(
                    data.destination_addresses,
                    data.num_destination_addresses as usize,
                );

                debug.field(
                    "destination_address_data",
                    &DebugStructSlice::new(addresses),
                );
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct IpAddrInfoDebug<'a>(&'a IpAddrInfo);

impl DebugView for IpAddrInfo {
    type Output<'a>
        = IpAddrInfoDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        IpAddrInfoDebug(self)
    }
}

impl<'a> Debug for IpAddrInfoDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("IpAddrInfo");

        debug
            .field("ip_version", &data.ip_version)
            .field("p_address", &PtrRepr::from(data.p_address));

        unsafe {
            if !data.p_address.is_null() {
                let size = match data.ip_version {
                    4 => 4,
                    6 => 16,
                    _ => 0,
                };

                if size > 0 {
                    let address = slice::from_raw_parts(data.p_address, size);

                    debug.field("address", &DebugByteSlice::new(address));
                }
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct EthSwitchStateDebug<'a>(&'a EthSwitchState);

impl DebugView for EthSwitchState {
    type Output<'a>
        = EthSwitchStateDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        EthSwitchStateDebug(self)
    }
}

impl<'a> Debug for EthSwitchStateDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("EthSwitchState");

        debug
            .field("eth_sense_state", &data.eth_sense_state)
            .field("eth_act_pin_num", &data.eth_act_pin_num);

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct RscStatusDataDebug<'a>(&'a RscStatusData);

impl DebugView for RscStatusData {
    type Output<'a>
        = RscStatusDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        RscStatusDataDebug(self)
    }
}

impl<'a> Debug for RscStatusDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("RscStatusData");

        debug
            .field("h_mod", &data.h_mod)
            .field("resource_id", &data.resource_id)
            .field("resource_status", &data.resource_status);

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct RscStatusItemDebug<'a>(&'a RscStatusItem);

impl DebugView for RscStatusItem {
    type Output<'a>
        = RscStatusItemDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        RscStatusItemDebug(self)
    }
}

impl<'a> Debug for RscStatusItemDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("RscStatusItem");

        debug
            .field("item_type", &data.item_type)
            .field(
                "item_type_value",
                &format_args!("{:#010X}", data.item_type as u32),
            )
            .field("num_entries", &data.num_entries)
            .field(
                "p_resource_status_data",
                &PtrRepr::from(data.p_resource_status_data),
            );

        unsafe {
            if !data.p_resource_status_data.is_null() && data.num_entries > 0 {
                let entries =
                    slice::from_raw_parts(data.p_resource_status_data, data.num_entries as usize);

                debug.field("resource_status_data", &DebugStructSlice::new(entries));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ParamItemDebug<'a>(&'a ParamItem);

impl DebugView for ParamItem {
    type Output<'a>
        = ParamItemDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ParamItemDebug(self)
    }
}

impl<'a> Debug for ParamItemDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ParamItem");

        debug
            .field("item_type", &data.item_type)
            .field(
                "item_type_value",
                &format_args!("{:#010X}", data.item_type as u32),
            )
            .field("com_param_id", &data.com_param_id)
            .field("com_param_data_type", &data.com_param_data_type)
            .field(
                "com_param_data_type_value",
                &format_args!("{:#010X}", data.com_param_data_type as u32),
            )
            .field("com_param_class", &data.com_param_class)
            .field(
                "com_param_class_value",
                &format_args!("{:#010X}", data.com_param_class as u32),
            )
            .field("p_com_param_data", &PtrRepr::from(data.p_com_param_data));

        unsafe {
            if !data.p_com_param_data.is_null() {
                match data.com_param_data_type {
                    PduPt::Unum8 => {
                        debug.field("data", &*(data.p_com_param_data as *const u8));
                    }

                    PduPt::Snum8 => {
                        debug.field("data", &*(data.p_com_param_data as *const i8));
                    }

                    PduPt::Unum16 => {
                        debug.field("data", &*(data.p_com_param_data as *const u16));
                    }

                    PduPt::Snum16 => {
                        debug.field("data", &*(data.p_com_param_data as *const i16));
                    }

                    PduPt::Unum32 => {
                        debug.field("data", &*(data.p_com_param_data as *const u32));
                    }

                    PduPt::Snum32 => {
                        debug.field("data", &*(data.p_com_param_data as *const i32));
                    }

                    PduPt::ByteField => {
                        let value = &*(data.p_com_param_data as *const ParamByteFieldData);

                        debug.field("data", &ParamByteFieldDataDebug(value));
                    }

                    PduPt::LongField => {
                        let value = &*(data.p_com_param_data as *const ParamLongFieldData);

                        debug.field("data", &ParamLongFieldDataDebug(value));
                    }

                    PduPt::StructField => {
                        let value = &*(data.p_com_param_data as *const ParamStructFieldData);

                        debug.field("data", &ParamStructFieldDataDebug(value));
                    }
                }
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ParamByteFieldDataDebug<'a>(&'a ParamByteFieldData);

impl DebugView for ParamByteFieldData {
    type Output<'a>
        = ParamByteFieldDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ParamByteFieldDataDebug(self)
    }
}

impl<'a> Debug for ParamByteFieldDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ParamByteFieldData");

        debug
            .field("param_max_len", &data.param_max_len)
            .field("param_act_len", &data.param_act_len)
            .field("p_data_array", &PtrRepr::from(data.p_data_array));

        unsafe {
            if !data.p_data_array.is_null() && data.param_act_len > 0 {
                let bytes = slice::from_raw_parts(data.p_data_array, data.param_act_len as usize);

                debug.field("data", &DebugByteSlice::new(bytes));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ParamLongFieldDataDebug<'a>(&'a ParamLongFieldData);

impl DebugView for ParamLongFieldData {
    type Output<'a>
        = ParamLongFieldDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ParamLongFieldDataDebug(self)
    }
}

impl<'a> Debug for ParamLongFieldDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ParamLongFieldData");

        debug
            .field("param_max_len", &data.param_max_len)
            .field("param_act_len", &data.param_act_len)
            .field("p_data_array", &PtrRepr::from(data.p_data_array));

        unsafe {
            if !data.p_data_array.is_null() && data.param_act_len > 0 {
                let values = slice::from_raw_parts(data.p_data_array, data.param_act_len as usize);

                debug.field("data", &DebugDwordSlice::new(values));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ParamStructFieldDataDebug<'a>(&'a ParamStructFieldData);

impl DebugView for ParamStructFieldData {
    type Output<'a>
        = ParamStructFieldDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ParamStructFieldDataDebug(self)
    }
}

impl<'a> Debug for ParamStructFieldDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ParamStructFieldData");

        debug
            .field("com_param_struct_type", &data.com_param_struct_type)
            .field(
                "com_param_struct_type_value",
                &format_args!("{:#010X}", data.com_param_struct_type as u32),
            )
            .field("param_max_entries", &data.param_max_entries)
            .field("param_act_entries", &data.param_act_entries)
            .field("p_struct_array", &PtrRepr::from(data.p_struct_array));

        unsafe {
            if !data.p_struct_array.is_null() && data.param_act_entries > 0 {
                let total_len = data.param_act_entries as usize;

                match data.com_param_struct_type {
                    PduCpst::SessionTiming => {
                        let values = slice::from_raw_parts(
                            data.p_struct_array as *const ParamStructSessionTiming,
                            total_len,
                        );

                        debug.field("structures", &DebugStructSlice::new(values));
                    }

                    PduCpst::AccessTiming => {
                        let values = slice::from_raw_parts(
                            data.p_struct_array as *const ParamStructAccessTiming,
                            total_len,
                        );

                        debug.field("structures", &DebugStructSlice::new(values));
                    }
                }
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ParamStructSessionTimingDebug<'a>(&'a ParamStructSessionTiming);

impl DebugView for ParamStructSessionTiming {
    type Output<'a>
        = ParamStructSessionTimingDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ParamStructSessionTimingDebug(self)
    }
}

impl<'a> Debug for ParamStructSessionTimingDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ParamStructSessionTiming");

        debug
            .field("session", &data.session)
            .field("p2_max_high", &data.p2_max_high)
            .field("p2_max_low", &data.p2_max_low)
            .field("p2_star_high", &data.p2_star_high)
            .field("p2_star_low", &data.p2_star_low);

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ParamStructAccessTimingDebug<'a>(&'a ParamStructAccessTiming);

impl DebugView for ParamStructAccessTiming {
    type Output<'a>
        = ParamStructAccessTimingDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ParamStructAccessTimingDebug(self)
    }
}

impl<'a> Debug for ParamStructAccessTimingDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ParamStructAccessTiming");

        debug
            .field("p2_min", &data.p2_min)
            .field("p2_max", &data.p2_max)
            .field("p3_min", &data.p3_min)
            .field("p3_max", &data.p3_max)
            .field("p4_min", &data.p4_min)
            .field("timing_set", &data.timing_set)
            .field(
                "timing_set_value",
                &format_args!("{:#04X}", data.timing_set as u8),
            );

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ModuleItemDebug<'a>(&'a ModuleItem);

impl DebugView for ModuleItem {
    type Output<'a>
        = ModuleItemDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ModuleItemDebug(self)
    }
}

impl<'a> Debug for ModuleItemDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ModuleItem");

        debug
            .field("item_type", &data.item_type)
            .field(
                "item_type_value",
                &format_args!("{:#010X}", data.item_type as u32),
            )
            .field("num_entries", &data.num_entries)
            .field("p_module_data", &PtrRepr::from(data.p_module_data));

        unsafe {
            if !data.p_module_data.is_null() && data.num_entries > 0 {
                let total_len = data.num_entries as usize;

                let modules = slice::from_raw_parts(data.p_module_data, total_len);

                debug.field("module_data", &DebugStructSlice::new(modules));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ModuleDataDebug<'a>(&'a ModuleData);

impl DebugView for ModuleData {
    type Output<'a>
        = ModuleDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ModuleDataDebug(self)
    }
}

impl<'a> Debug for ModuleDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ModuleData");

        debug
            .field("module_type_id", &data.module_type_id)
            .field("h_mod", &data.h_mod)
            .field(
                "vendor_module_name",
                &CStrDebug::new(data.vendor_module_name),
            )
            .field(
                "vendor_additional_info",
                &CStrDebug::new(data.vendor_additional_info),
            )
            .field("status", &data.status)
            .field(
                "status_value",
                &format_args!("{:#010X}", data.status as u32),
            );

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct RscIdItemDebug<'a>(&'a RscIdItem);

impl DebugView for RscIdItem {
    type Output<'a>
        = RscIdItemDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        RscIdItemDebug(self)
    }
}

impl<'a> Debug for RscIdItemDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("RscIdItem");

        debug
            .field("item_type", &data.item_type)
            .field(
                "item_type_value",
                &format_args!("{:#010X}", data.item_type as u32),
            )
            .field("num_modules", &data.num_modules)
            .field("p_id_item_data", &PtrRepr::from(data.p_id_item_data));

        unsafe {
            if !data.p_id_item_data.is_null() && data.num_modules > 0 {
                let total_len = data.num_modules as usize;

                let modules = slice::from_raw_parts(data.p_id_item_data, total_len);

                debug.field("id_item_data", &DebugStructSlice::new(modules));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct RscIdItemDataDebug<'a>(&'a RscIdItemData);

impl DebugView for RscIdItemData {
    type Output<'a>
        = RscIdItemDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        RscIdItemDataDebug(self)
    }
}

impl<'a> Debug for RscIdItemDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("RscIdItemData");

        debug
            .field("h_mod", &data.h_mod)
            .field("num_ids", &data.num_ids)
            .field(
                "p_resource_id_array",
                &PtrRepr::from(data.p_resource_id_array),
            );

        unsafe {
            if !data.p_resource_id_array.is_null() && data.num_ids > 0 {
                let total_len = data.num_ids as usize;

                let ids = slice::from_raw_parts(data.p_resource_id_array, total_len);

                debug.field("resource_ids", &DebugDwordSlice::new(ids));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct RscDataDebug<'a>(&'a RscData);

impl DebugView for RscData {
    type Output<'a>
        = RscDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        RscDataDebug(self)
    }
}

impl<'a> Debug for RscDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("RscData");

        debug
            .field("bus_type_id", &data.bus_type_id)
            .field("protocol_id", &data.protocol_id)
            .field("num_pin_data", &data.num_pin_data)
            .field("p_dlc_pin_data", &PtrRepr::from(data.p_dlc_pin_data));

        unsafe {
            if !data.p_dlc_pin_data.is_null() && data.num_pin_data > 0 {
                let total_len = data.num_pin_data as usize;

                let pins = slice::from_raw_parts(data.p_dlc_pin_data, total_len);

                debug.field("pin_data", &DebugStructSlice::new(pins));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct PinDataDebug<'a>(&'a PinData);

impl DebugView for PinData {
    type Output<'a>
        = PinDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        PinDataDebug(self)
    }
}

impl<'a> Debug for PinDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("PinData");

        debug
            .field("dlc_pin_number", &data.dlc_pin_number)
            .field("dlc_pin_type_id", &data.dlc_pin_type_id);

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct RscConflictItemDebug<'a>(&'a RscConflictItem);

impl DebugView for RscConflictItem {
    type Output<'a>
        = RscConflictItemDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        RscConflictItemDebug(self)
    }
}

impl<'a> Debug for RscConflictItemDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("RscConflictItem");

        debug
            .field("item_type", &data.item_type)
            .field(
                "item_type_value",
                &format_args!("{:#010X}", data.item_type as u32),
            )
            .field("num_entries", &data.num_entries)
            .field(
                "p_rsc_conflict_data",
                &PtrRepr::from(data.p_rsc_conflict_data),
            );

        unsafe {
            if !data.p_rsc_conflict_data.is_null() && data.num_entries > 0 {
                let total_len = data.num_entries as usize;

                let conflicts = slice::from_raw_parts(data.p_rsc_conflict_data, total_len);

                debug.field("rsc_conflict_data", &DebugStructSlice::new(conflicts));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct RscConflictDataDebug<'a>(&'a RscConflictData);

impl DebugView for RscConflictData {
    type Output<'a>
        = RscConflictDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        RscConflictDataDebug(self)
    }
}

impl<'a> Debug for RscConflictDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("RscConflictData");

        debug
            .field("h_mod", &data.h_mod)
            .field("resource_id", &data.resource_id);

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct UniqueRespIdTableItemDebug<'a>(&'a UniqueRespIdTableItem);

impl DebugView for UniqueRespIdTableItem {
    type Output<'a>
        = UniqueRespIdTableItemDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        UniqueRespIdTableItemDebug(self)
    }
}

impl<'a> Debug for UniqueRespIdTableItemDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("UniqueRespIdTableItem");

        debug
            .field("item_type", &data.item_type)
            .field(
                "item_type_value",
                &format_args!("{:#010X}", data.item_type as u32),
            )
            .field("num_entries", &data.num_entries)
            .field("p_unique_data", &PtrRepr::from(data.p_unique_data));

        unsafe {
            if !data.p_unique_data.is_null() && data.num_entries > 0 {
                let total_len = data.num_entries as usize;

                let unique_data = slice::from_raw_parts(data.p_unique_data, total_len);

                debug.field("unique_data", &DebugStructSlice::new(unique_data));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct EcuUniqueRespDataDebug<'a>(&'a EcuUniqueRespData);

impl DebugView for EcuUniqueRespData {
    type Output<'a>
        = EcuUniqueRespDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        EcuUniqueRespDataDebug(self)
    }
}

impl<'a> Debug for EcuUniqueRespDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("EcuUniqueRespData");

        debug
            .field("unique_resp_identifier", &data.unique_resp_identifier)
            .field("num_param_items", &data.num_param_items)
            .field("p_params", &PtrRepr::from(data.p_params));

        unsafe {
            if !data.p_params.is_null() && data.num_param_items > 0 {
                let total_len = data.num_param_items as usize;

                let params = slice::from_raw_parts(data.p_params, total_len);

                debug.field("params", &DebugStructSlice::new(params));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct InfoDataDebug<'a>(&'a InfoData);

impl DebugView for InfoData {
    type Output<'a>
        = InfoDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        InfoDataDebug(self)
    }
}

impl<'a> Debug for InfoDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("InfoData");

        debug
            .field("info_code", &data.info_code)
            .field(
                "info_code_value",
                &format_args!("{:#010X}", data.info_code as u32),
            )
            .field("extra_info_data", &data.extra_info_data);

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ErrorDataDebug<'a>(&'a ErrorData);

impl DebugView for ErrorData {
    type Output<'a>
        = ErrorDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ErrorDataDebug(self)
    }
}

impl<'a> Debug for ErrorDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ErrorData");

        debug
            .field("error_code_id", &data.error_code_id)
            .field(
                "error_code_value",
                &format_args!("{:#010X}", data.error_code_id as u32),
            )
            .field("extra_error_info_id", &data.extra_error_info_id);

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct FlagDataDebug<'a>(&'a FlagData);

impl DebugView for FlagData {
    type Output<'a>
        = FlagDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        FlagDataDebug(self)
    }
}

impl<'a> Debug for FlagDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("FlagData");

        debug
            .field("num_flag_bytes", &data.num_flag_bytes)
            .field("p_flag_data", &PtrRepr::from(data.p_flag_data));

        unsafe {
            if !data.p_flag_data.is_null() && data.num_flag_bytes > 0 {
                let bytes = slice::from_raw_parts(data.p_flag_data, data.num_flag_bytes as usize);

                debug.field("flags", &DebugByteSlice::new(bytes));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ExtraInfoDebug<'a>(&'a ExtraInfo);

impl DebugView for ExtraInfo {
    type Output<'a>
        = ExtraInfoDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ExtraInfoDebug(self)
    }
}

impl<'a> Debug for ExtraInfoDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ExtraInfo");

        debug
            .field("num_header_bytes", &data.num_header_bytes)
            .field("num_footer_bytes", &data.num_footer_bytes)
            .field("p_header_bytes", &PtrRepr::from(data.p_header_bytes))
            .field("p_footer_bytes", &PtrRepr::from(data.p_footer_bytes));

        unsafe {
            if !data.p_header_bytes.is_null() && data.num_header_bytes > 0 {
                let header =
                    slice::from_raw_parts(data.p_header_bytes, data.num_header_bytes as usize);

                debug.field("header", &DebugByteSlice::new(header));
            }

            if !data.p_footer_bytes.is_null() && data.num_footer_bytes > 0 {
                let footer =
                    slice::from_raw_parts(data.p_footer_bytes, data.num_footer_bytes as usize);

                debug.field("footer", &DebugByteSlice::new(footer));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ResultDataDebug<'a>(&'a ResultData);

impl DebugView for ResultData {
    type Output<'a>
        = ResultDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ResultDataDebug(self)
    }
}

impl<'a> Debug for ResultDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ResultData");

        debug
            .field("rx_flag", &data.rx_flag.debug_view())
            .field("unique_resp_identifier", &data.unique_resp_identifier)
            .field("acceptance_id", &data.acceptance_id)
            .field("timestamp_flags", &data.timestamp_flags.debug_view())
            .field("tx_msg_done_timestamp", &data.tx_msg_done_timestamp)
            .field("start_msg_timestamp", &data.start_msg_timestamp)
            .field("p_extra_info", &PtrRepr::from(data.p_extra_info))
            .field("num_data_bytes", &data.num_data_bytes)
            .field("p_data_bytes", &PtrRepr::from(data.p_data_bytes));

        unsafe {
            if !data.p_extra_info.is_null() {
                debug.field("extra_info", &(*data.p_extra_info).debug_view());
            }

            if !data.p_data_bytes.is_null() && data.num_data_bytes > 0 {
                let bytes = slice::from_raw_parts(data.p_data_bytes, data.num_data_bytes as usize);

                debug.field("data", &DebugByteSlice::new(bytes));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct VersionDataDebug<'a>(&'a VersionData);

impl DebugView for VersionData {
    type Output<'a>
        = VersionDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        VersionDataDebug(self)
    }
}

impl<'a> Debug for VersionDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("VersionData");

        debug
            .field(
                "mvci_part1_standard_version",
                &format_args!("{:#010X}", data.mvci_part1_standard_version),
            )
            .field(
                "mvci_part2_standard_version",
                &format_args!("{:#010X}", data.mvci_part2_standard_version),
            )
            .field("hw_serial_number", &data.hw_serial_number)
            .field("hw_name", &CStrDebug::new(data.hw_name.as_ptr()))
            .field("hw_version", &format_args!("{:#010X}", data.hw_version))
            .field("hw_date", &format_args!("{:#010X}", data.hw_date))
            .field("hw_interface", &data.hw_interface)
            .field("fw_name", &CStrDebug::new(data.fw_name.as_ptr()))
            .field("fw_version", &format_args!("{:#010X}", data.fw_version))
            .field("fw_date", &format_args!("{:#010X}", data.fw_date))
            .field("vendor_name", &CStrDebug::new(data.vendor_name.as_ptr()))
            .field(
                "pdu_api_sw_name",
                &CStrDebug::new(data.pdu_api_sw_name.as_ptr()),
            )
            .field(
                "pdu_api_sw_version",
                &format_args!("{:#010X}", data.pdu_api_sw_version),
            )
            .field(
                "pdu_api_sw_date",
                &format_args!("{:#010X}", data.pdu_api_sw_date),
            );

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct CopCtrlDataDebug<'a>(&'a CopCtrlData);

impl DebugView for CopCtrlData {
    type Output<'a>
        = CopCtrlDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        CopCtrlDataDebug(self)
    }
}

impl<'a> Debug for CopCtrlDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("CopCtrlData");

        debug
            .field("time", &data.time)
            .field("num_send_cycles", &data.num_send_cycles)
            .field("num_receive_cycles", &data.num_receive_cycles)
            .field("temp_param_update", &data.temp_param_update)
            .field("tx_flag", &data.tx_flag)
            .field(
                "num_possible_expected_responses",
                &data.num_possible_expected_responses,
            )
            .field(
                "expected_response_array",
                &PtrRepr::from(data.expected_response_array),
            );

        unsafe {
            if !data.expected_response_array.is_null() && data.num_possible_expected_responses > 0 {
                let total_count = data.num_possible_expected_responses as usize;

                let responses = slice::from_raw_parts(data.expected_response_array, total_count);

                debug.field("expected_responses", &DebugStructSlice::new(responses));
            }
        }

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct IoEntityAddressDataDebug<'a>(&'a IoEntityAddressData);

impl DebugView for IoEntityAddressData {
    type Output<'a>
        = IoEntityAddressDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        IoEntityAddressDataDebug(self)
    }
}

impl<'a> Debug for IoEntityAddressDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("IoEntityAddressData");

        debug
            .field("logical_address", &data.logical_address)
            .field("doip_ctrl_timeout", &data.doip_ctrl_timeout);

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct IoEntityStatusDataDebug<'a>(&'a IoEntityStatusData);

impl DebugView for IoEntityStatusData {
    type Output<'a>
        = IoEntityStatusDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        IoEntityStatusDataDebug(self)
    }
}

impl<'a> Debug for IoEntityStatusDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("IoEntityStatusData");

        debug
            .field("entity_type", &data.entity_type)
            .field("tcp_clients_max", &data.tcp_clients_max)
            .field("tcp_clients", &data.tcp_clients)
            .field("max_data_size", &data.max_data_size);

        debug.finish()
    }
}

#[allow(missing_docs)]
pub struct ExpRespDataDebug<'a>(&'a ExpRespData);

impl DebugView for ExpRespData {
    type Output<'a>
        = ExpRespDataDebug<'a>
    where
        Self: 'a;

    fn debug_view(&self) -> Self::Output<'_> {
        ExpRespDataDebug(self)
    }
}

impl<'a> Debug for ExpRespDataDebug<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let data = self.0;

        let mut debug = f.debug_struct("ExpRespData");

        debug
            .field("response_type", &data.response_type)
            .field("acceptance_id", &data.acceptance_id)
            .field("num_mask_pattern_bytes", &data.num_mask_pattern_bytes)
            .field("p_mask_data", &PtrRepr::from(data.p_mask_data))
            .field("p_pattern_data", &PtrRepr::from(data.p_pattern_data))
            .field("num_unique_resp_ids", &data.num_unique_resp_ids)
            .field("p_unique_resp_ids", &PtrRepr::from(data.p_unique_resp_ids));

        unsafe {
            if !data.p_mask_data.is_null() && data.num_mask_pattern_bytes > 0 {
                let bytes =
                    slice::from_raw_parts(data.p_mask_data, data.num_mask_pattern_bytes as usize);

                debug.field("mask_data", &DebugByteSlice::new(bytes));
            }

            if !data.p_pattern_data.is_null() && data.num_mask_pattern_bytes > 0 {
                let bytes = slice::from_raw_parts(
                    data.p_pattern_data,
                    data.num_mask_pattern_bytes as usize,
                );

                debug.field("pattern_data", &DebugByteSlice::new(bytes));
            }

            if !data.p_unique_resp_ids.is_null() && data.num_unique_resp_ids > 0 {
                let total_len = data.num_unique_resp_ids as usize;

                let ids = slice::from_raw_parts(data.p_unique_resp_ids, total_len);

                debug.field("unique_resp_ids", &DebugDwordSlice::new(ids));
            }
        }

        debug.finish()
    }
}
