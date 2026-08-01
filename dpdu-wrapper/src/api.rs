use crate::constants::API_EVENTS_QUEUE_SIZE;
use crate::error::GeneralError;
use crate::handle_manager::PduHandleManager;
use crate::types::pdu_com_logical_link::{CllCreateFlags, CllCreateType, PduCllData};
use crate::types::pdu_com_param::table::PduComParamTable;
use crate::types::pdu_com_param::{
    ByteFieldComParam, CpVariant, LongFieldComParam, PduComParam, StructComParam,
    StructFieldComParam,
};
use crate::types::pdu_com_primitive::PduPrimitiveParams;
use crate::types::pdu_error::{PduErrorData, PduLastErrorTarget};
use crate::types::pdu_event::{
    PduErrorEvent, PduEvent, PduEventData, PduEventTarget, PduInfoEvent, PduResultEvent,
    PduStatusEvent, StopReceive,
};
use crate::types::pdu_io_ctl::{IoCtlByteArray, PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use crate::types::pdu_lock_resource::PduLockResourceMask;
use crate::types::pdu_module::{
    PduConflictingModules, PduModuleData, PduModuleList, PduModulesResourcesIds,
};
use crate::types::pdu_object::PduObjectIdSource;
use crate::types::pdu_resource::{
    BusSource, PduResource, PduResourceStatus, PinSource, ProtocolSource, ResourceStatus, TargetPin,
};
use crate::types::pdu_status::{PduStatusData, PduStatusTarget};
use crate::types::pdu_version::PduVersionData;
use crate::types::{
    PduCllHandle, PduCopHandle, PduLibraryPath, PduModuleHandle, PduObjectId, PduOptions,
    PduUniqueApiTag, PduUniqueCllTag, PduUniqueCopTag,
};
use crate::utils::module_description::{PduModuleDescription, PduModuleDescriptionError};
use crate::utils::root_file::Mvci;
use crate::utils::{NonClonable, PhantomRef, c_str, random_non_zero_usize, take_slice_ptr};
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::bitflags::PduErrorFlag;
use dpdu_api_types::{
    CopCtrlData, EcuUniqueRespData, ErrorData, EventCallbackFn, EventItem, ExpRespData, FlagData,
    InfoData, IoByteArrayData, IoEventQueuePropertyData, IoFilterData, IoProgVoltageData,
    ModuleData, ModuleItem, PDU_HANDLE_UNDEF, PDU_ID_UNDEF, ParamByteFieldData, ParamItem,
    ParamLongFieldData, ParamStructAccessTiming, ParamStructFieldData, ParamStructSessionTiming,
    PduCancelComPrimitiveFn, PduConnectFn, PduConstructFn, PduCopt, PduCpst,
    PduCreateComLogicalLinkFn, PduDestroyComLogicalLinkFn, PduDestroyItemFn, PduDestructFn,
    PduDisconnectFn, PduError, PduErrorEvt, PduGetComParamFn, PduGetConflictingResourcesFn,
    PduGetEventItemFn, PduGetLastErrorFn, PduGetModuleIdsFn, PduGetObjectIdFn, PduGetResourceIdsFn,
    PduGetResourceStatusFn, PduGetStatusFn, PduGetTimestampFn, PduGetUniqueRespIdTableFn,
    PduGetVersionFn, PduIoctlFn, PduIt, PduItem, PduLockResourceFn, PduModuleConnectFn,
    PduModuleDisconnectFn, PduObjt, PduPc, PduPt, PduQueueMode, PduRegisterCallbackFn,
    PduSetComParamFn, PduSetUniqueRespIdTableFn, PduStartComPrimitiveFn, PduStatus,
    PduUnlockResourceFn, PinData, ResultData, RscData, RscStatusData, RscStatusItem,
    UniqueRespIdTableItem, VersionData,
};
use parking_lot::{Mutex, RwLock};
use scopeguard::defer;
use std::any::type_name;
use std::cell::OnceCell;
use std::collections::HashMap;
use std::ffi::{CString, c_void};
use std::fmt::{Debug, Formatter};
use std::mem::{ManuallyDrop, MaybeUninit};
use std::num::NonZeroUsize;
use std::ptr::NonNull;
use std::sync::{Arc, OnceLock, Weak};
use std::thread::{sleep, spawn};
use std::{ptr, slice};
use std::time::Duration;
use thread_local::ThreadLocal;
use tokio::sync::{broadcast, mpsc};
use tracing::{Level, debug, error, info, trace, warn};

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ApiError {
    #[error("ffi error: {0}")]
    FfiError(String),

    #[error("pdu error: {0}")]
    PduError(#[from] PduError),

    #[error("module description error: {0}")]
    MdfError(#[from] PduModuleDescriptionError),
}

impl From<libloading::Error> for ApiError {
    fn from(value: libloading::Error) -> Self {
        Self::FfiError(value.to_string())
    }
}

impl From<PduError> for GeneralError {
    fn from(value: PduError) -> Self {
        GeneralError::ApiError(ApiError::PduError(value))
    }
}

impl From<libloading::Error> for GeneralError {
    fn from(value: libloading::Error) -> Self {
        GeneralError::ApiError(ApiError::FfiError(value.to_string()))
    }
}

impl From<PduModuleDescriptionError> for GeneralError {
    fn from(value: PduModuleDescriptionError) -> Self {
        GeneralError::ApiError(ApiError::MdfError(value))
    }
}

#[derive(Debug)]
struct ApiSymbols {
    cancel_primitive: PduCancelComPrimitiveFn,
    connect: PduConnectFn,
    construct: PduConstructFn,
    create_logical_link: PduCreateComLogicalLinkFn,
    destruct: PduDestructFn,
    destroy_logical_link: PduDestroyComLogicalLinkFn,
    destroy_item: PduDestroyItemFn,
    disconnect: PduDisconnectFn,
    get_com_param: PduGetComParamFn,
    get_conflicting_resources: PduGetConflictingResourcesFn,
    get_event_item: PduGetEventItemFn,
    get_last_error: PduGetLastErrorFn,
    get_module_ids: PduGetModuleIdsFn,
    get_object_id: PduGetObjectIdFn,
    get_resource_ids: PduGetResourceIdsFn,
    get_resource_status: PduGetResourceStatusFn,
    get_status: PduGetStatusFn,
    get_timestamp: PduGetTimestampFn,
    get_unique_resp_id_table: PduGetUniqueRespIdTableFn,
    get_version: PduGetVersionFn,
    io_ctl: PduIoctlFn,
    lock_resource: PduLockResourceFn,
    module_connect: PduModuleConnectFn,
    module_disconnect: PduModuleDisconnectFn,
    register_event_callback: PduRegisterCallbackFn,
    set_com_param: PduSetComParamFn,
    set_unique_resp_id_table: PduSetUniqueRespIdTableFn,
    start_primitive: PduStartComPrimitiveFn,
    unlock_resource: PduUnlockResourceFn,
}

/// The internal structure that must be placed in TLS to suppress logging errors that occur
/// in the D-PDU API.
///
/// For internal use only.
#[derive(Debug, Clone, Default)]
pub struct SuppressLogOptions {
    pub(crate) cancel_primitive: PduErrorFlag,
    pub(crate) connect: PduErrorFlag,
    pub(crate) construct: PduErrorFlag,
    pub(crate) create_logical_link: PduErrorFlag,
    pub(crate) destruct: PduErrorFlag,
    pub(crate) destroy_logical_link: PduErrorFlag,
    pub(crate) destroy_item: PduErrorFlag,
    pub(crate) disconnect: PduErrorFlag,
    pub(crate) get_com_param: PduErrorFlag,
    pub(crate) get_conflicting_resources: PduErrorFlag,
    pub(crate) get_event_item: PduErrorFlag,
    pub(crate) get_last_error: PduErrorFlag,
    pub(crate) get_module_ids: PduErrorFlag,
    pub(crate) get_object_id: PduErrorFlag,
    pub(crate) get_resource_ids: PduErrorFlag,
    pub(crate) get_resource_status: PduErrorFlag,
    pub(crate) get_status: PduErrorFlag,
    pub(crate) get_timestamp: PduErrorFlag,
    pub(crate) get_unique_resp_id_table: PduErrorFlag,
    pub(crate) get_version: PduErrorFlag,
    pub(crate) io_ctl: PduErrorFlag,
    pub(crate) lock_resource: PduErrorFlag,
    pub(crate) module_connect: PduErrorFlag,
    pub(crate) module_disconnect: PduErrorFlag,
    pub(crate) register_event_callback: PduErrorFlag,
    pub(crate) set_com_param: PduErrorFlag,
    pub(crate) set_unique_resp_id_table: PduErrorFlag,
    pub(crate) start_primitive: PduErrorFlag,
    pub(crate) unlock_resource: PduErrorFlag,
}

macro_rules! impl_defer_clear_suppress_options {
    ($self:expr, $func:ident) => {
        let suppress_log_options = $self.suppress_log_options.get_or_default().clone();

        defer! {
            let mut options = suppress_log_options.write();
            options.$func = PduErrorFlag::empty();
        }
    };
}

macro_rules! resolve_level_of_log_api_call_fail {
    ($self:expr, $result:expr, $func:ident) => {{
        let suppress_log_options = $self.suppress_log_options.get_or_default().read();
        suppress_log_options
            .$func
            .contains($result.flag())
            .then_some(Level::DEBUG)
    }};
}

pub struct PduApi {
    pub(crate) me: Weak<PduApi>,

    pdu_options: PduOptions,

    pub(crate) unique_tag: PduUniqueApiTag,

    library: libloading::Library,

    library_file: Option<PduLibraryPath>,

    pub module_description: Option<PduModuleDescription>,

    mvci: Option<Mvci>,

    symbols: ApiSymbols,

    suppress_log_options: ThreadLocal<Arc<RwLock<SuppressLogOptions>>>,

    /// Event channel sender owned by [`PduApi`].
    ///
    /// The sender is intentionally dropped when [`PduApi`] is dropped,
    /// allowing the event listener to detect shutdown.
    ///
    /// A weak reference is stored in [`PduHandleManager`] to keep track of the
    /// channel lifetime and to automatically stop event listening in
    /// [`PduApi::listen_events`] and
    /// [`PduApi::blocking_listen_events`].
    ///
    /// # Safety
    ///
    /// This sender must not be cloned. Cloning it would extend the channel
    /// lifetime beyond [`PduApi`] and prevent listeners from being
    /// automatically stopped.
    ///
    /// [`PduHandleManager`]: crate::handle_manager::PduHandleManager
    pub(crate) pdu_event_tx: NonClonable<mpsc::UnboundedSender<PduEvent>>,

    /// Sender used to create additional receivers after the initial receiver
    /// has been taken.
    pub(crate) api_event_tx: broadcast::Sender<()>,

    /// The initial receiver returned by the first [`get_api_event_receiver`] call.
    ///
    /// After the receiver is taken, subsequent calls create new receivers from
    /// [`api_event_tx`].
    pub(crate) api_event_rx: Mutex<Option<broadcast::Receiver<()>>>,
}

impl PartialEq for PduApi {
    fn eq(&self, other: &Self) -> bool {
        self.unique_tag == other.unique_tag
    }
}

impl Debug for PduApi {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("me", &self.me)
            .field("pdu_options", &self.pdu_options)
            .field("unique_tag", &self.unique_tag)
            .field("library", &self.library)
            .field("library_file", &self.library_file)
            .field("module_description", &self.module_description)
            .field("pdu_event_tx", &self.pdu_event_tx)
            .field("api_event_tx", &self.api_event_tx)
            .field("api_event_rx", &self.api_event_rx)
            .field("mvci", &self.mvci)
            .field("symbols", &self.symbols)
            .finish()
    }
}

impl PduApi {
    pub fn new(
        options: PduOptions,
        library: libloading::Library,
        library_file: Option<PduLibraryPath>,
        module_description: Option<PduModuleDescription>,
        mvci: Option<Mvci>,
    ) -> ApiResult<Arc<Self>> {
        let symbols = unsafe {
            ApiSymbols {
                cancel_primitive: *library.get(b"PDUCancelComPrimitive")?,
                connect: *library.get(b"PDUConnect")?,
                construct: *library.get(b"PDUConstruct")?,
                create_logical_link: *library.get(b"PDUCreateComLogicalLink")?,
                destruct: *library.get(b"PDUDestruct")?,
                destroy_logical_link: *library.get(b"PDUDestroyComLogicalLink")?,
                destroy_item: *library.get(b"PDUDestroyItem")?,
                disconnect: *library.get(b"PDUDisconnect")?,
                get_com_param: *library.get(b"PDUGetComParam")?,
                get_conflicting_resources: *library.get(b"PDUGetConflictingResources")?,
                get_event_item: *library.get(b"PDUGetEventItem")?,
                get_last_error: *library.get(b"PDUGetLastError")?,
                get_module_ids: *library.get(b"PDUGetModuleIds")?,
                get_object_id: *library.get(b"PDUGetObjectId")?,
                get_resource_ids: *library.get(b"PDUGetResourceIds")?,
                get_resource_status: *library.get(b"PDUGetResourceStatus")?,
                get_status: *library.get(b"PDUGetStatus")?,
                get_timestamp: *library.get(b"PDUGetTimestamp")?,
                get_unique_resp_id_table: *library.get(b"PDUGetUniqueRespIdTable")?,
                get_version: *library.get(b"PDUGetVersion")?,
                io_ctl: *library.get(b"PDUIoCtl")?,
                lock_resource: *library.get(b"PDULockResource")?,
                module_connect: *library.get(b"PDUModuleConnect")?,
                module_disconnect: *library.get(b"PDUModuleDisconnect")?,
                register_event_callback: *library.get(b"PDURegisterEventCallback")?,
                set_com_param: *library.get(b"PDUSetComParam")?,
                set_unique_resp_id_table: *library.get(b"PDUSetUniqueRespIdTable")?,
                start_primitive: *library.get(b"PDUStartComPrimitive")?,
                unlock_resource: *library.get(b"PDUUnlockResource")?,
            }
        };

        let (pdu_event_tx, pdu_event_rx) = mpsc::unbounded_channel();

        let (api_event_tx, api_event_rx) = broadcast::channel(API_EVENTS_QUEUE_SIZE);

        // This thread is necessary for receiving events from the D-PDU API via the built-in
        // event callback mechanism, in order to generate the high-level events that
        // are actually required when using PduApi.
        spawn({
            let api_event_tx = api_event_tx.clone();
            move || PduApi::blocking_listen_events(pdu_event_rx, api_event_tx)
        });

        let result = Arc::new_cyclic(|me| Self {
            me: me.clone(),
            pdu_options: options,
            unique_tag: random_non_zero_usize(),
            library,
            library_file,
            module_description,
            mvci,
            symbols,
            suppress_log_options: ThreadLocal::default(),
            pdu_event_tx: NonClonable(pdu_event_tx.clone()),
            api_event_tx,
            api_event_rx: Mutex::new(Some(api_event_rx)),
        });

        PduHandleManager::register_api(&result, pdu_event_tx.downgrade());

        Ok(result)
    }

    pub fn from_mvci(mvci: &Mvci, options: PduOptions) -> ApiResult<Arc<Self>> {
        let library = unsafe { libloading::Library::new(&mvci.library_file)? };
        let mdf = mvci
            .module_description_file
            .as_ref()
            .map(|v| PduModuleDescription::parse_from_xml_file(v))
            .transpose()?;

        PduApi::new(
            options,
            library,
            Some(mvci.library_file.clone()),
            mdf,
            Some(mvci.clone()),
        )
    }

    pub fn from_library_path(
        library_file: impl Into<PduLibraryPath>,
        options: PduOptions,
        module_description: Option<PduModuleDescription>,
    ) -> ApiResult<Arc<Self>> {
        let library_file = library_file.into();
        let library = unsafe { libloading::Library::new(&library_file)? };

        PduApi::new(
            options,
            library,
            Some(library_file),
            module_description,
            None,
        )
    }

    pub fn from_library(
        library: libloading::Library,
        options: PduOptions,
        module_description: Option<PduModuleDescription>,
    ) -> ApiResult<Arc<Self>> {
        PduApi::new(options, library, None, module_description, None)
    }

    /// It allows you to suppress the logging level of an unsuccessful D-PDU API call
    /// from ERROR to DEBUG, so as not to mislead the user or themselves.
    ///
    /// Suppression is applied only to the current thread and only for the duration
    /// of the required function call.
    ///
    /// # Safety
    /// Changing these options may affect diagnostic behavior and should only be
    /// done when the caller understands the consequences.
    pub fn modify_suppress_log_options<F>(&self, f: F)
    where
        F: Fn(&mut SuppressLogOptions),
    {
        let options = self.suppress_log_options.get_or_default();
        f(&mut *options.write());
    }

    pub fn get_unique_tag(&self) -> PduUniqueApiTag {
        self.unique_tag
    }

    fn log_api_call(&self, func: &str) {
        debug!(func, "D-PDU API Call");
    }

    pub(crate) fn log_api_call_fail(
        &self,
        func: &str,
        result: PduError,
        desc: Option<String>,
        level: Option<Level>,
    ) {
        let level = level.unwrap_or(Level::ERROR);
        let desc = desc.map(|v| format!(": {v}")).unwrap_or_default();
        match level {
            Level::TRACE => {
                trace!(
                    func,
                    result_str = %result,
                    result_int = format!("{:#x}", result as usize),
                    "D-PDU API Call failed{desc}"
                )
            }
            Level::DEBUG => {
                debug!(
                    func,
                    result_str = %result,
                    result_int = format!("{:#x}", result as usize),
                    "D-PDU API Call failed{desc}"
                )
            }
            Level::INFO => {
                info!(
                    func,
                    result_str = %result,
                    result_int = format!("{:#x}", result as usize),
                    "D-PDU API Call failed{desc}"
                )
            }
            Level::WARN => {
                warn!(
                    func,
                    result_str = %result,
                    result_int = format!("{:#x}", result as usize),
                    "D-PDU API Call failed{desc}"
                )
            }
            Level::ERROR => {
                error!(
                    func,
                    result_str = %result,
                    result_int = format!("{:#x}", result as usize),
                    "D-PDU API Call failed{desc}"
                )
            }
        }
    }

    pub(crate) fn clone_arc(&self) -> Arc<Self> {
        self.me
            .upgrade()
            .expect("internal error: unable to upgrade the Weak<PduApi> pointer") // infallible
    }

    pub fn pdu_construct(&self) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, construct);

        const FUNC: &'static str = "PDUConstruct";
        self.log_api_call(FUNC);

        let options_str = {
            // 9.4.2.4 Parameters
            // OptionStr String containing a list of attributes and their values. An attribute and its corresponding value
            // are to be separated by an >=< sign. The value needs to be put inside two >'< signs. Between
            // pairs of attribute and value shall be at least one space character. Attributes and values are
            // specific to a D-PDU API implementation.
            // When no option is to be set, the OptionStr can either be an empty string or NULL.
            //
            // 9.4.2.5 Example
            // OptionStr = "UseCaching='TRUE' InterfaceCheck='FALSE'"
            self.pdu_options
                .iter()
                .map(|(k, v)| format!("{k}='{v}'"))
                .collect::<Vec<String>>()
                .join(" ")
        };

        trace!(
            func = FUNC,
            options_str,
            unique_tag = ?self.unique_tag.get(),
            "D-PDU API Call Args"
        );

        let options_str = CString::new(options_str).expect("CString::new() failed");
        let construct_fn = self.symbols.construct;
        let result = wrap_pdu_call(FUNC, || {
            construct_fn(
                options_str.as_ptr() as _,
                self.unique_tag.get() as *const PduUniqueApiTag as _,
            )
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, construct),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub fn pdu_destruct(&self) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, destruct);

        const FUNC: &'static str = "PDUDestruct";
        self.log_api_call(FUNC);

        let destruct_fn = self.symbols.destruct;

        match destruct_fn() {
            PduError::StatusNoError | PduError::PduApiNotConstructed => Ok(()),
            v => {
                self.log_api_call_fail(
                    FUNC,
                    v,
                    None,
                    resolve_level_of_log_api_call_fail!(self, v, destruct),
                );
                Err(v)?
            }
        }
    }

    pub fn pdu_destroy_item(&self, item_ptr: *mut PduItem) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, destroy_item);

        const FUNC: &'static str = "PDUDestroyItem";
        self.log_api_call(FUNC);

        trace!(
            func = FUNC,
            p_item = format!("0x{:x}", item_ptr as usize),
            "D-PDU API Call Args"
        );

        if item_ptr.is_null() {
            return Ok(());
        }

        let destroy_item_fn = self.symbols.destroy_item;
        let result = wrap_pdu_call(FUNC, || destroy_item_fn(item_ptr));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, destroy_item),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub fn pdu_get_event_item(&self, target: &PduEventTarget) -> ApiResult<Option<PduEvent>> {
        impl_defer_clear_suppress_options!(self, get_event_item);

        const FUNC: &'static str = "PDUGetEventItem";
        self.log_api_call(FUNC);

        let h_mod = target.get_module_handle().unwrap_or(PDU_HANDLE_UNDEF);
        let h_cll = target.get_cll_handle().unwrap_or(PDU_HANDLE_UNDEF);

        trace!(func = FUNC, h_mod, h_cll, "D-PDU API Call Args");

        let mut item_ptr: *mut EventItem = ptr::null_mut();

        let get_event_item_fn = self.symbols.get_event_item;
        let result = wrap_pdu_call(FUNC, || get_event_item_fn(h_mod, h_cll, &mut item_ptr));
        let debug_ret = || {
            trace!(
                func = FUNC,
                item_ptr = format!("0x{:x}", item_ptr as usize),
                item_type = ?NonNull::new(item_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
                "D-PDU API Call Return"
            );
        };

        match result {
            PduError::StatusNoError => {},
            PduError::EventQueueEmpty => {
                trace!("event queue is empty");
                debug_ret();
                return Ok(None);
            },
            v => {
                self.log_api_call_fail(
                    FUNC,
                    result,
                    None,
                    resolve_level_of_log_api_call_fail!(self, result, get_event_item),
                );
                return Err(v)?;
            }
        }

        debug_ret();

        if item_ptr.is_null() {
            return Ok(None);
        }

        let item = unsafe { &*item_ptr };

        if item.p_data.is_null() {
            error!(
                func = FUNC,
                "Item data pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let data: PduEventData = match item.item_type {
            PduIt::Status => PduStatusEvent(unsafe { *(item.p_data as *const PduStatus) }).into(),
            PduIt::Result => {
                let data = unsafe { &*(item.p_data as *const ResultData) };

                let mut extra_header = OnceCell::new();
                let mut extra_footer = OnceCell::new();

                if !data.p_extra_info.is_null() {
                    let extra_info = unsafe { &*data.p_extra_info };
                    if !extra_info.p_header_bytes.is_null() {
                        let ptr = extra_info.p_header_bytes;
                        let len = extra_info.num_header_bytes;
                        if !ptr.is_null() && len > 0 {
                            extra_header
                                .set(unsafe { slice::from_raw_parts(ptr, len as _) }.to_vec())
                                .unwrap();
                        }
                    }
                    if !extra_info.p_footer_bytes.is_null() {
                        let ptr = extra_info.p_footer_bytes;
                        let len = extra_info.num_footer_bytes;
                        if !ptr.is_null() && len > 0 {
                            extra_footer
                                .set(unsafe { slice::from_raw_parts(ptr, len as _) }.to_vec())
                                .unwrap();
                        }
                    }
                }

                PduResultEvent {
                    rx_flags: unsafe {
                        let ptr = data.rx_flag.p_flag_data;
                        let len = data.rx_flag.num_flag_bytes as usize;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        slice.to_vec().into()
                    },
                    unique_resp_identifier: data.unique_resp_identifier,
                    acceptance_id: data.acceptance_id,
                    timestamp_flags: unsafe {
                        let ptr = data.timestamp_flags.p_flag_data;
                        let len = data.timestamp_flags.num_flag_bytes as usize;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        slice.to_vec().into()
                    },
                    tx_msg_done_timestamp: data.tx_msg_done_timestamp,
                    start_msg_timestamp: data.start_msg_timestamp,
                    data: unsafe {
                        let ptr = data.p_data_bytes;
                        let len = data.num_data_bytes as usize;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        slice.to_vec().into()
                    },
                    extra_info_header: extra_header.take(),
                    extra_info_footer: extra_footer.take(),
                }
                .into()
            }
            PduIt::Error => {
                let data = unsafe { &*(item.p_data as *const ErrorData) };
                PduErrorEvent {
                    code: data.error_code_id,
                    extra_code: data.extra_error_info_id,
                }
                .into()
            }
            PduIt::Info => {
                let data = unsafe { &*(item.p_data as *const InfoData) };
                PduInfoEvent {
                    code: data.info_code,
                    extra_code: data.extra_info_data,
                }
                .into()
            }
            typ => {
                self.pdu_destroy_item(item_ptr as _)?;
                error!(
                    func = FUNC,
                    "Unexpected PduEventItemType = {}. Emulation of PduError::FctFailed...",
                    typ.as_str()
                );
                return Err(PduError::FctFailed)?;
            }
        };

        let h_cop = (item.h_cop != PDU_HANDLE_UNDEF).then(|| item.h_cop);
        let cop_tag = NonZeroUsize::new(item.p_cop_tag as _);
        let timestamp = item.timestamp;

        self.pdu_destroy_item(item_ptr as _)?;

        Ok(Some(PduEvent {
            target: target.clone(),
            h_cop,
            cop_tag,
            timestamp,
            data,
        }))
    }

    pub fn pdu_get_version(&self, h_mod: PduModuleHandle) -> ApiResult<PduVersionData> {
        impl_defer_clear_suppress_options!(self, get_version);

        const FUNC: &'static str = "PDUGetVersion";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, "D-PDU API Call Args");

        let mut version_data = VersionData::default();

        let get_version_fn = self.symbols.get_version;
        let result = wrap_pdu_call(FUNC, || get_version_fn(h_mod, &mut version_data));

        trace!(
            func = FUNC,
            version_data_ptr = format!("0x{:x}", &version_data as *const VersionData as usize),
            "D-PDU API Call Return"
        );

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_version),
            );
            return Err(result)?;
        }

        let version_data = PduVersionData {
            mvci_part1_standard_version: version_data.mvci_part1_standard_version.into(),
            mvci_part2_standard_version: version_data.mvci_part2_standard_version.into(),
            hw_serial_number: version_data.hw_serial_number,
            hw_name: c_str(version_data.hw_name.as_ptr() as _),
            hw_version: version_data.hw_version.into(),
            hw_date: version_data.hw_date.into(),
            hw_interface: version_data.hw_interface,
            fw_name: c_str(version_data.fw_name.as_ptr() as _),
            fw_version: version_data.fw_version.into(),
            fw_date: version_data.fw_date.into(),
            vendor_name: c_str(version_data.vendor_name.as_ptr() as _),
            pdu_api_sw_name: c_str(version_data.pdu_api_sw_name.as_ptr() as _),
            pdu_api_sw_version: version_data.pdu_api_sw_version.into(),
            pdu_api_sw_date: version_data.pdu_api_sw_date.into(),
        };

        Ok(version_data)
    }

    pub fn pdu_get_object_id(
        &self,
        object: PduObjt,
        short_name: &str,
    ) -> ApiResult<Option<PduObjectId>> {
        impl_defer_clear_suppress_options!(self, get_object_id);

        const FUNC: &'static str = "PDUGetObjectId";
        self.log_api_call(FUNC);

        trace!(
            func = FUNC,
            object = object.as_str(),
            short_name,
            "D-PDU API Call Args"
        );

        if let Some(desc) = &self.module_description {
            // First, we will try to obtain the required object ID from the module description
            // file supplied with the D-PDU API driver in order to reduce
            // the number of D-PDU API calls.
            let object_id = match object {
                PduObjt::IoCtrl => desc.io_controls.get_by_short_name(short_name).map(|v| v.id),
                PduObjt::Resource => desc.resources.get_by_short_name(short_name).map(|v| v.id),
                PduObjt::Protocol => desc.protocols.get_by_short_name(short_name).map(|v| v.id),
                PduObjt::BusType => desc.bus_types.get_by_short_name(short_name).map(|v| v.id),
                PduObjt::PinType => desc.pin_types.get_by_short_name(short_name).map(|v| v.id),
                PduObjt::ComParam => desc.com_params.get_by_short_name(short_name).map(|v| v.id),
            };

            if let Some(id) = object_id {
                trace!(func = FUNC, id, "D-PDU API Call Return [virtual]");
                return Ok(Some(id));
            }
        }

        let short_name = CString::new(short_name).expect("CString::new() failed");
        let mut object_id: MaybeUninit<u32> = MaybeUninit::uninit();

        let get_object_id_fn = self.symbols.get_object_id;
        let result = wrap_pdu_call(FUNC, || {
            get_object_id_fn(object, short_name.as_ptr() as _, object_id.as_mut_ptr())
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_object_id),
            );
            return Err(result)?;
        }

        // SAFETY:
        // PDUGetObjectId guarantees that `object_id` is initialized on success.
        let object_id = unsafe { object_id.assume_init() };

        trace!(func = FUNC, object_id, "D-PDU API Call Return");

        if object_id != PDU_ID_UNDEF {
            Ok(Some(object_id))
        } else {
            Ok(None)
        }
    }

    pub fn pdu_get_com_param(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        object_id: PduObjectIdSource,
    ) -> ApiResult<PduComParam> {
        impl_defer_clear_suppress_options!(self, get_com_param);

        const FUNC: &'static str = "PDUGetComParam";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, %object_id, "D-PDU API Call Args");

        let id = match &object_id {
            PduObjectIdSource::Id(v) => *v,
            PduObjectIdSource::ShortName(v) => {
                let Some(id) = self.pdu_get_object_id(PduObjt::ComParam, &v)? else {
                    let result = PduError::ComParamNotSupported;

                    // This is not a critical error.
                    // Therefore, we will not log it separately.
                    self.log_api_call_fail(
                        FUNC,
                        result,
                        Some(format!("unsupported com param: {v}")),
                        Some(Level::WARN),
                    );

                    return Err(result)?;
                };
                id
            }
        };

        let mut item_ptr: *mut ParamItem = ptr::null_mut();
        let get_com_param_fn = self.symbols.get_com_param;
        let result = wrap_pdu_call(FUNC, || get_com_param_fn(h_mod, h_cll, id, &mut item_ptr));

        trace!(
            func = FUNC,
            item_ptr = format!("0x{:x}", item_ptr as usize),
            item_type = ?NonNull::new(item_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
            "D-PDU API Call Return"
        );

        if !result.is_success() {
            return match result {
                PduError::ComParamNotSupported | PduError::InvalidParameters => {
                    // This is not a critical error.
                    // Therefore, we will not log it separately.
                    self.log_api_call_fail(
                        FUNC,
                        result,
                        Some(format!("unsupported com param: {object_id}")),
                        Some(Level::WARN),
                    );
                    Err(PduError::ComParamNotSupported)?
                }
                _ => {
                    self.log_api_call_fail(
                        FUNC,
                        result,
                        None,
                        resolve_level_of_log_api_call_fail!(self, result, get_com_param),
                    );
                    Err(result)?
                }
            };
        }

        if item_ptr.is_null() {
            error!(
                func = FUNC,
                "Item pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let cp = unsafe {
            use ptr::read;

            let item = &*item_ptr;
            let data_ptr = item.p_com_param_data;
            let short_name = OnceLock::new();

            match &object_id {
                PduObjectIdSource::ShortName(v) => {
                    let _ = short_name.set(v.clone());
                }
                _ => {
                    let sn_opt = self
                        .module_description
                        .as_ref()
                        .and_then(|mdf_desc| mdf_desc.com_params.get_by_id(id))
                        .and_then(|mdf_cp| mdf_cp.short_name.clone());

                    if let Some(sn) = sn_opt {
                        let _ = short_name.set(sn);
                    }
                }
            };

            PduComParam {
                short_name,
                id,
                class: item.com_param_class,
                variant: match item.com_param_data_type {
                    PduPt::Unum8 => CpVariant::Unum8(read(data_ptr as _)),
                    PduPt::Snum8 => CpVariant::Snum8(read(data_ptr as _)),
                    PduPt::Unum16 => CpVariant::Unum16(read(data_ptr as _)),
                    PduPt::Snum16 => CpVariant::Snum16(read(data_ptr as _)),
                    PduPt::Unum32 => CpVariant::Unum32(read(data_ptr as _)),
                    PduPt::Snum32 => CpVariant::Snum32(read(data_ptr as _)),
                    PduPt::ByteField => CpVariant::ByteField({
                        let data = &*(data_ptr as *const ParamByteFieldData);
                        let ptr = data.p_data_array;
                        let len = data.param_act_len as _;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        ByteFieldComParam::new(slice.to_vec(), Some(data.param_max_len as _))
                    }),
                    PduPt::StructField => CpVariant::StructField({
                        let data = &*(data_ptr as *const ParamStructFieldData);
                        let ptr = data.p_struct_array as *mut StructComParam;
                        let len = data.param_act_entries as _;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        StructFieldComParam::new(
                            data.com_param_struct_type,
                            slice.to_vec(),
                            Some(data.param_max_entries as _),
                        )
                    }),
                    PduPt::LongField => CpVariant::LongField({
                        let data = &*(data_ptr as *const ParamLongFieldData);
                        let ptr = data.p_data_array;
                        let len = data.param_act_len as _;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        LongFieldComParam::new(slice.to_vec(), Some(data.param_max_len as _))
                    }),
                },
            }
        };

        self.pdu_destroy_item(item_ptr as _)?;

        Ok(cp)
    }

    pub fn pdu_set_com_param(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        cp: &PduComParam,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, set_com_param);

        const FUNC: &'static str = "PDUSetComParam";
        self.log_api_call(FUNC);

        if matches!(cp.class, PduPc::UniqueId) {
            // Chapter 9.4.27.1:
            //
            // NOTE ComParams that are of type PDU_PC_UNIQUE_ID can only be used with
            // the Unique Response ID Table.
            // They cannot be used in the functions PDUGetComParam() or PDUSetComParam().
            //
            // Therefore, to reduce the number of calls to the D-PDU API, we proactively
            // return the PduError::InvalidParameters error on our side.
            let result = PduError::InvalidParameters;
            self.log_api_call_fail(
                FUNC,
                result,
                Some("PDUSetComParam accepts only UniqueId classes".to_string()),
                None,
            );
            return Err(result)?;
        }

        cp.try_init_short_name(self);

        #[repr(C)]
        union TempUnion<'a> {
            u8: u8,
            s8: i8,
            u16: u16,
            s16: i16,
            u32: u32,
            s32: i32,
            long_field: ManuallyDrop<PhantomRef<'a, ParamLongFieldData>>,
            struct_field: ManuallyDrop<PhantomRef<'a, ParamStructFieldData>>,
            byte_field: ManuallyDrop<PhantomRef<'a, ParamByteFieldData>>,
        }

        struct AutoDrop<'a> {
            temp: TempUnion<'a>,
            pdu_type: PduPt,
        }

        impl<'a> Drop for AutoDrop<'a> {
            fn drop(&mut self) {
                match self.pdu_type {
                    PduPt::LongField => unsafe { ManuallyDrop::drop(&mut self.temp.long_field) },
                    PduPt::StructField => unsafe {
                        ManuallyDrop::drop(&mut self.temp.struct_field)
                    },
                    PduPt::ByteField => unsafe { ManuallyDrop::drop(&mut self.temp.byte_field) },
                    _ => {}
                }
            }
        }

        let data = AutoDrop {
            temp: match &cp.variant {
                CpVariant::Unum8(v) => TempUnion { u8: v.to_owned() },
                CpVariant::Snum8(v) => TempUnion { s8: v.to_owned() },
                CpVariant::Unum16(v) => TempUnion { u16: v.to_owned() },
                CpVariant::Snum16(v) => TempUnion { s16: v.to_owned() },
                CpVariant::Unum32(v) => TempUnion { u32: v.to_owned() },
                CpVariant::Snum32(v) => TempUnion { s32: v.to_owned() },
                CpVariant::LongField(v) => TempUnion {
                    long_field: ManuallyDrop::new(v.get_pdu_data()),
                },
                CpVariant::StructField(v) => TempUnion {
                    struct_field: ManuallyDrop::new(v.get_pdu_data()),
                },
                CpVariant::ByteField(v) => TempUnion {
                    byte_field: ManuallyDrop::new(v.get_pdu_data()),
                },
            },
            pdu_type: cp.variant.get_pdu_type(),
        };

        let item = ParamItem {
            item_type: PduIt::Param,
            com_param_id: cp.id,
            com_param_data_type: cp.variant.get_pdu_type(),
            com_param_class: cp.class,
            p_com_param_data: &data.temp as *const _ as _,
        };

        trace!(
            func = FUNC,
            h_mod,
            h_cll,
            com_param = cp.get_debug_name(),
            com_param_ptr = format!("0x{:x}", &item as *const _ as usize),
            "D-PDU API Call Args"
        );

        let set_com_param_fn = self.symbols.set_com_param;
        let result = wrap_pdu_call(FUNC, || {
            set_com_param_fn(h_mod, h_cll, &item as *const _ as _)
        });

        if !result.is_success() {
            return match result {
                PduError::ComParamNotSupported | PduError::InvalidParameters => {
                    // This is not a critical error.
                    // Therefore, we will not log it separately.
                    self.log_api_call_fail(
                        FUNC,
                        result,
                        Some(format!("unsupported com param: {cp}")),
                        Some(Level::WARN),
                    );
                    Err(PduError::ComParamNotSupported)?
                }
                _ => {
                    self.log_api_call_fail(
                        FUNC,
                        result,
                        None,
                        resolve_level_of_log_api_call_fail!(self, result, get_com_param),
                    );
                    Err(result)?
                }
            };
        }

        Ok(())
    }

    pub fn pdu_set_unique_resp_id_table(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        table: &PduComParamTable,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, set_unique_resp_id_table);

        const FUNC: &'static str = "PDUSetUniqueRespIdTable";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, "D-PDU API Call Args");

        type EphemeralGroupKey = usize;

        let mut temp_com_param_groups: HashMap<EphemeralGroupKey, Vec<ParamItem>> =
            HashMap::with_capacity(table.len());

        let mut temp_unique_groups: Vec<EcuUniqueRespData> = Vec::with_capacity(table.len());

        for (key, (unique_resp_identifier, group_set)) in table.iter().enumerate() {
            let mut com_param_set = Vec::with_capacity(group_set.len());

            trace!(
                func = FUNC,
                group = key,
                unique_resp_identifier,
                "D-PDU API Call Args"
            );

            for cp in group_set.iter() {
                if !matches!(cp.class, PduPc::UniqueId) {
                    error!(
                        com_param = cp.get_debug_name(),
                        class = cp.class.as_str(),
                        "Invalid class of the PduComParam stored in PduComParamTable"
                    );
                    let result = PduError::InvalidParameters;
                    self.log_api_call_fail(FUNC, result, None, None);
                    return Err(result)?;
                }

                cp.try_init_short_name(self);

                let item = ParamItem {
                    item_type: PduIt::Param,
                    com_param_id: cp.id,
                    com_param_data_type: cp.variant.get_pdu_type(),
                    com_param_class: PduPc::UniqueId,
                    p_com_param_data: cp.variant.get_pdu_ptr().as_ptr() as _,
                };

                trace!(
                    func = FUNC,
                    group = key,
                    com_param = cp.get_debug_name(),
                    "D-PDU API Call Args"
                );

                com_param_set.push(item);
            }

            temp_com_param_groups.insert(key, com_param_set);

            let temp_com_param_group = temp_com_param_groups.get(&key).expect("inserted above");

            temp_unique_groups.push(EcuUniqueRespData {
                unique_resp_identifier: *unique_resp_identifier,
                num_param_items: temp_com_param_group.len() as _,
                p_params: take_slice_ptr(temp_com_param_group.as_slice()),
            });
        }

        let table = UniqueRespIdTableItem {
            item_type: PduIt::UniqueRespIdTable,
            num_entries: temp_unique_groups.len() as _,
            p_unique_data: take_slice_ptr(temp_unique_groups.as_slice()),
        };

        trace!(
            func = FUNC,
            table_num_entries = table.num_entries,
            table_ptr = format!("{:#x}", &table as *const _ as usize),
            "D-PDU API Call Args"
        );

        let set_unique_resp_id_table_fn = self.symbols.set_unique_resp_id_table;
        let result = wrap_pdu_call(FUNC, || {
            set_unique_resp_id_table_fn(h_mod, h_cll, &table as *const _ as _)
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, set_unique_resp_id_table),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub fn pdu_register_event_callback(
        &self,
        target: &PduEventTarget,
        callback: Option<EventCallbackFn>,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, register_event_callback);

        const FUNC: &'static str = "PDURegisterEventCallback";
        self.log_api_call(FUNC);

        trace!(func = FUNC, %target, "D-PDU API Call Args");

        let (h_mod, h_cll) = match target {
            PduEventTarget::Module(h_mod) => {
                if h_mod == &PDU_HANDLE_UNDEF {
                    let result = PduError::InvalidHandle;
                    self.log_api_call_fail(FUNC, result, Some("module handle of the PduEventCallbackTarget cannot be PDU_HANDLE_UNDEF".to_string()), None);
                    return Err(result)?;
                }

                (h_mod.to_owned(), PDU_HANDLE_UNDEF)
            }
            PduEventTarget::LogicalLink(h_mod, h_cll) => {
                if h_mod == &PDU_HANDLE_UNDEF {
                    let result = PduError::InvalidHandle;
                    self.log_api_call_fail(FUNC, result, Some("module handle of the PduEventCallbackTarget cannot be PDU_HANDLE_UNDEF".to_string()), None);
                    return Err(result)?;
                } else if h_cll == &PDU_HANDLE_UNDEF {
                    let result = PduError::InvalidHandle;
                    self.log_api_call_fail(FUNC, result, Some("logical link handle of the PduEventCallbackTarget cannot be PDU_HANDLE_UNDEF".to_string()), None);
                    return Err(result)?;
                }

                (h_mod.to_owned(), h_cll.to_owned())
            }
            PduEventTarget::System => (PDU_HANDLE_UNDEF, PDU_HANDLE_UNDEF),
        };

        trace!(func = FUNC, h_mod, h_cll, "D-PDU API Call Args");

        let register_event_callback_fn = self.symbols.register_event_callback;
        let result = wrap_pdu_call(FUNC, || {
            register_event_callback_fn(h_mod, h_cll, unsafe { std::mem::transmute(callback) })
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, register_event_callback),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub fn pdu_start_com_primitive(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        cop_type: PduCopt,
        data: &[u8],
        params: Option<&PduPrimitiveParams>,
        tag: Option<PduUniqueCopTag>,
    ) -> ApiResult<PduCopHandle> {
        impl_defer_clear_suppress_options!(self, start_primitive);

        const FUNC: &'static str = "PDUStartComPrimitive";
        self.log_api_call(FUNC);

        let tag = tag.map(|v| v.get()).unwrap_or_default();

        trace!(
            func = FUNC,
            h_mod,
            h_cll,
            cop_type = cop_type.as_str(),
            tag = format!("{tag:#x}"),
            "D-PDU API Call Args"
        );

        let mut cop_handle: MaybeUninit<PduCopHandle> = MaybeUninit::uninit();
        let start_com_primitive_fn = self.symbols.start_primitive;

        let result = match cop_type {
            PduCopt::UpdateParam | PduCopt::RestoreParam => {
                if data.len() != 0 {
                    warn!(
                        func = FUNC,
                        "when PduCopt = UpdateParam or RestoreParam, data is not required"
                    );
                }
                if params.is_some() {
                    warn!(
                        func = FUNC,
                        "when PduCopt = UpdateParam or RestoreParam, PduComPrimitiveParams is not required"
                    );
                }

                trace!(
                    func = FUNC,
                    data_len = 0,
                    data_ptr = "<nullptr>",
                    cop_ctrl_data_ptr = "<nullptr>",
                    tag,
                    cop_handle_ptr = format!("{:#x}", cop_handle.as_ptr() as usize),
                    "D-PDU API Call Args"
                );

                wrap_pdu_call(FUNC, || {
                    start_com_primitive_fn(
                        h_mod,
                        h_cll,
                        cop_type,
                        0,               // data len
                        ptr::null_mut(), // data ptr
                        ptr::null_mut(), // cop ctrl data
                        tag as *mut _,   // tag
                        cop_handle.as_mut_ptr(),
                    )
                })
            }
            _ => {
                let params = params.expect(&format!(
                    "when PduCopt = {}, PduComPrimitiveParams is required",
                    cop_type.as_str()
                ));

                let flags = params.tx_flag.get_pdu_flag_data();

                trace!(
                    func = FUNC,

                    cop_delay_ms = params.time,
                    send_cycles = params.send_cycles.to_i32(),
                    receive_cycles = params.receive_cycles.to_i32(),
                    buffer = params.temp_param_update.as_str(),

                    flags_ptr = format!("{:#x}", flags.as_ptr() as usize),
                    flags = ?flags,

                    expected_responses_len = params.expected_responses.len(),

                    "D-PDU API Call Args"
                );

                let expected_responses = params.expected_responses
                    .iter()
                    .map(|v| {
                        trace!(
                            func = FUNC,

                            response_type = v.response_type.as_str(),
                            acceptance_id = v.acceptance_id,

                            mask_data_ptr = format!("{:#x}", take_slice_ptr(v.mask_data.get_mask()) as usize),
                            mask_data_len = v.mask_data.mask.len(),
                            mask_data = ?v.mask_data.mask,

                            mask_pattern_ptr = format!("{:#x}", take_slice_ptr(v.mask_data.get_pattern()) as usize),
                            mask_pattern_len = v.mask_data.pattern.len(),
                            mask_pattern = ?v.mask_data.pattern,

                            unique_response_ids_ptr = format!("{:#x}", take_slice_ptr(v.unique_response_ids.as_slice()) as usize),
                            unique_response_ids_len = v.unique_response_ids.len(),
                            unique_response_ids = ?v.unique_response_ids,

                            "D-PDU API Call Args"
                        );

                        ExpRespData {
                            response_type: v.response_type as _,
                            acceptance_id: v.acceptance_id,
                            num_mask_pattern_bytes: v.mask_data.len() as _,
                            p_mask_data: take_slice_ptr(v.mask_data.get_mask()),
                            p_pattern_data: take_slice_ptr(v.mask_data.get_pattern()),
                            num_unique_resp_ids: v.unique_response_ids.len() as _, // TODO : heap corruption only under cargo run
                            p_unique_resp_ids: take_slice_ptr(v.unique_response_ids.as_slice()),
                        }
                    })
                    .collect::<Vec<_>>();

                let cop_ctrl_data = CopCtrlData {
                    time: params.time,
                    num_send_cycles: params.send_cycles.to_i32(),
                    num_receive_cycles: params.receive_cycles.to_i32(),
                    temp_param_update: params.temp_param_update as _,
                    tx_flag: FlagData {
                        num_flag_bytes: flags.len() as _,
                        p_flag_data: take_slice_ptr(flags.as_slice()),
                    },
                    num_possible_expected_responses: expected_responses.len() as _,
                    expected_response_array: take_slice_ptr(expected_responses.as_slice()),
                };

                let data_ptr = take_slice_ptr(data);

                trace!(
                    func = FUNC,
                    data_len = data.len(),
                    data_ptr = format!("{:#x}", data_ptr as usize),
                    cop_ctrl_data_ptr = format!("{:#x}", &cop_ctrl_data as *const _ as usize),
                    tag,
                    cop_handle_ptr = format!("{:#x}", cop_handle.as_ptr() as usize),
                    "D-PDU API Call Args"
                );

                wrap_pdu_call(FUNC, || {
                    start_com_primitive_fn(
                        h_mod,
                        h_cll,
                        cop_type,
                        data.len() as _,
                        data_ptr,
                        &cop_ctrl_data as *const _ as _,
                        tag as *mut _, // tag
                        cop_handle.as_mut_ptr(),
                    )
                })
            }
        };

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, start_primitive),
            );
            return Err(result)?;
        }

        // SAFETY:
        // PDUStartComPrimitive guarantees that `phCoP` is initialized on success.
        let cop_handle = unsafe { cop_handle.assume_init() };

        trace!(func = FUNC, cop_handle, "D-PDU API Call Return");

        Ok(cop_handle)
    }

    ///| IOCTL Short Name                      | Target | Input Data Type               | Output Data Type            | Purpose
    ///|---------------------------------------|--------|-------------------------------|-----------------------------|--------------------------------------------------------------------------------
    ///| PDU_IOCTL_RESET                       | M      | —                             | —                           | Reset specific MVCI protocol module.
    ///| PDU_IOCTL_CLEAR_TX_QUEUE              | L      | —                             | —                           | Clear transmit queue of specific ComLogicalLink.
    ///| PDU_IOCTL_SUSPEND_TX_QUEUE            | L      | —                             | —                           | Suspend transmit queue of specific ComLogicalLink.
    ///| PDU_IOCTL_RESUME_TX_QUEUE             | L      | —                             | —                           | Resume transmit queue of specific ComLogicalLink.
    ///| PDU_IOCTL_CLEAR_RX_QUEUE              | L      | —                             | —                           | Clear event queue of specific ComLogicalLink.
    ///| PDU_IOCTL_READ_VBATT                  | M      | —                             | PDU_IT_IO_UNUM32            | Read voltage on pin 16 of MVCI protocol module.
    ///| PDU_IOCTL_SET_PROG_VOLTAGE            | M      | PDU_IT_IO_PROG_VOLTAGE        | —                           | Set programmable voltage on DLC connector pin/resource.
    ///| PDU_IOCTL_READ_PROG_VOLTAGE           | M      | —                             | PDU_IT_IO_UNUM32            | Read feedback of programmable voltage.
    ///| PDU_IOCTL_GENERIC                     | M      | PDU_IT_IO_BYTE_ARRAY          | —                           | Send a generic message to MVCI protocol module drivers.
    ///| PDU_IOCTL_SET_BUFFER_SIZE             | L      | PDU_IT_IO_UNUM32              | —                           | Set buffer size limit of item.
    ///| PDU_IOCTL_START_MSG_FILTER            | L      | PDU_IT_IO_FILTER              | —                           | Start filtering incoming messages for specified ComLogicalLink.
    ///| PDU_IOCTL_CLEAR_MSG_FILTER            | L      | —                             | —                           | Clear all message filters for the ComLogicalLink.
    ///| PDU_IOCTL_STOP_MSG_FILTER             | L      | PDU_IT_IO_UNUM32              | —                           | Stop specified filter based on filter number.
    ///| PDU_IOCTL_SET_EVENT_QUEUE_PROPERTIES  | L      | PDU_IT_IO_EVENT_QUEUE_PROPERTY| —                           | Set size and mode of ComLogicalLink event queue.
    ///| PDU_IOCTL_GET_CABLE_ID                | M      | —                             | PDU_IT_IO_UNUM32            | Get cable ID connected to MVCI protocol module.
    ///| PDU_IOCTL_SEND_BREAK                  | L      | —                             | —                           | Send UART Break Signal on ComLogicalLink.
    ///| PDU_IOCTL_READ_IGNITION_SENSE_STATE   | M      | PDU_IT_IO_UNUM32              | PDU_IT_IO_UNUM32            | Read ignition sense state from vehicle connector pin.
    ///| PDU_IOCTL_VEHICLE_ID_REQUEST          | S, M   | PDU_IT_IO_VEHICLE_ID_REQUEST  | —                           | Send vehicle identification request (DoIP).
    ///| PDU_IOCTL_SET_ETH_SWITCH_STATE        | M      | PDU_IT_IO_ETH_SWITCH_STATE    | —                           | Switch Ethernet activation PIN on DLC.
    ///| PDU_IOCTL_GET_ENTITY_STATUS           | M      | PDU_IT_IO_ENTITY_ADDRESS      | PDU_IT_IO_ENTITY_STATUS     | Retrieve status of a DoIP entity.
    ///| PDU_IOCTL_GET_DIAGNOSTIC_POWER_MODE   | M      | PDU_IT_IO_ENTITY_ADDRESS      | PDU_IT_IO_UNUM32            | Retrieve diagnostic power mode of a DoIP entity.
    ///| PDU_IOCTL_GET_ETH_PIN_OPTION          | M      | PDU_IT_IO_UNUM32              | PDU_IT_IO_UNUM32            | Determine Ethernet pinout option from Ethernet activation PIN (DLC).
    ///| PDU_IOCTL_TLS_SET_CERTIFICATE         | M      | PDU_IT_IO_TLS_CERTIFICATE     | —                           | Set X.509 certificate(s) used for ECU verification during TLS handshake.
    ///| PDU_IOCTL_TLS_GET_CURRENT_SESSION_MODE| L      | —                             | PDU_IT_IO_UNUM32            | Get current DoIP connection mode (unsecure or secured via TLS).
    ///| PDU_IOCTL_ISOBUS_GET_DETECTED_CFS     | L      | —                             | PDU_IT_IO_BYTEARRAY         | Get list of ISOBUS CF-NAMEs detected on CAN bus (8-byte NAME + 1-byte address).
    pub fn pdu_io_ctl(
        &self,
        target: &PduIoCtlTarget,
        command: &PduIoCtlCommand,
        data: Option<&PduIoCtlData>,
    ) -> ApiResult<Option<PduIoCtlData>> {
        impl_defer_clear_suppress_options!(self, io_ctl);

        const FUNC: &'static str = "PDUIoCtl";
        self.log_api_call(FUNC);

        let h_mod = target.get_module_handle().unwrap_or(PDU_HANDLE_UNDEF);
        let h_cll = target.get_cll_handle().unwrap_or(PDU_HANDLE_UNDEF);

        trace!(
            func = FUNC,
            ?h_mod,
            ?h_cll,
            %command,
            "D-PDU API Call Args"
        );

        data.inspect(|data| match data {
            PduIoCtlData::U32(v) => trace!(
                func = FUNC,
                data_type = data.as_str(),
                data_u32 = v,
                "D-PDU API Call Args"
            ),
            PduIoCtlData::ProgVoltage(v) => trace!(
                func = FUNC,
                data_type = data.as_str(),
                data_prog_voltage_mv = v.prog_voltage_mv,
                data_pin_on_dlc = v.pin_on_dlc,
                "D-PDU API Call Args"
            ),
            PduIoCtlData::ByteArray(v) => trace!(
                func = FUNC,
                data_type = data.as_str(),
                data_len = v.len(),
                data_value = ?v,
                "D-PDU API Call Args"
            ),
            PduIoCtlData::Filter(v) => trace!(
                func = FUNC,
                data_type = data.as_str(),
                data_filter_type = v.filter_type.as_str(),
                data_filter_number = v.filter_number,
                data_filter_compare_size = v.filter_compare_size,
                data_filter_mask_msg = ?v.filter_mask_msg,
                data_filter_pattern_msg = ?v.filter_pattern_msg,
                "D-PDU API Call Args"
            ),
            PduIoCtlData::EventQueueProperty(v) => trace!(
                func = FUNC,
                data_type = data.as_str(),
                data_queue_size = v.queue_size,
                data_queue_mode = v.queue_mode.as_str(),
                "D-PDU API Call Args"
            ),
        });

        let object_id = match command {
            PduIoCtlCommand::Id(v) => v.to_owned(),
            PduIoCtlCommand::Name(v) => match self.pdu_get_object_id(PduObjt::IoCtrl, &v)? {
                Some(id) => id,
                None => {
                    let result = PduError::FctFailed;

                    self.log_api_call_fail(
                        FUNC,
                        result,
                        Some(format!("unable to lookup IO_CTRL id by name: {v}")),
                        None,
                    );

                    return Err(PduError::FctFailed)?;
                }
            },
        };

        let input_data_ptr: *const c_void = data
            .as_ref()
            .map(|v| v.to_pdu_data_item().p_data as _)
            .unwrap_or(ptr::null());

        let mut output_data_ptr = ptr::null_mut();

        trace!(
            func = FUNC,
            input_data_ptr = format!("{:#x}", input_data_ptr as usize),
            output_data_ptr = format!("{:#x}", &output_data_ptr as *const _ as usize),
            "D-PDU API Call Args"
        );

        let io_ctl_fn = self.symbols.io_ctl;
        let result = wrap_pdu_call(FUNC, || {
            io_ctl_fn(
                h_mod,
                h_cll,
                object_id,
                input_data_ptr as _,
                &mut output_data_ptr,
            )
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, io_ctl),
            );
            return Err(result)?;
        }

        if !output_data_ptr.is_null() {
            let data = unsafe { &*output_data_ptr };
            let io_ctl_data: ApiResult<Option<PduIoCtlData>> = unsafe {
                match data.item_type {
                    PduIt::IoUnum32 => Ok(Some(data.p_data.cast::<u32>().read().into())),
                    PduIt::IoProgVoltage => {
                        Ok(Some(data.p_data.cast::<IoProgVoltageData>().read().into()))
                    }
                    PduIt::IoByteArray => {
                        let byte_array = &*data.p_data.cast::<IoByteArrayData>();
                        if byte_array.p_data.is_null() {
                            error!(
                                func = FUNC,
                                data_type = PduIt::IoByteArray.as_str(),
                                "Byte array pointer is null. Emulation of PduError::FctFailed..."
                            );
                            return Err(PduError::FctFailed)?;
                        } else {
                            let ptr = byte_array.p_data;
                            let len = byte_array.data_size as _;
                            let slice = if ptr.is_null() || len == 0 {
                                &[]
                            } else {
                                slice::from_raw_parts(ptr, len)
                            };
                            Ok(Some(IoCtlByteArray(slice.to_vec()).into()))
                        }
                    }
                    PduIt::IoFilter => Ok(Some(data.p_data.cast::<IoFilterData>().read().into())),
                    PduIt::IoEventQueueProperty => Ok(Some(
                        data.p_data.cast::<IoEventQueuePropertyData>().read().into(),
                    )),
                    v => {
                        error!(
                            func = FUNC,
                            data_type = v.as_str(),
                            "Unexpected output data type. Emulation of PduError::FctFailed..."
                        );
                        return Err(PduError::FctFailed)?;
                    }
                }
            };

            self.pdu_destroy_item(output_data_ptr as _)?;

            io_ctl_data
        } else {
            Ok(None)
        }
    }

    pub fn pdu_get_module_ids(&self) -> ApiResult<PduModuleList> {
        impl_defer_clear_suppress_options!(self, get_module_ids);

        const FUNC: &'static str = "PDUGetModuleIds";
        self.log_api_call(FUNC);

        let mut module_list_item_ptr = ptr::null_mut();

        let get_module_ids_fn = self.symbols.get_module_ids;
        let result = wrap_pdu_call(FUNC, || get_module_ids_fn(&mut module_list_item_ptr));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_module_ids),
            );
            if !module_list_item_ptr.is_null() {
                self.pdu_destroy_item(module_list_item_ptr as _)?;
            }
            return Err(result)?;
        }

        trace!(
            func = FUNC,
            item_ptr = format!("0x{:x}", module_list_item_ptr as usize),
            item_type = ?NonNull::new(module_list_item_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
            "D-PDU API Call Return"
        );

        if module_list_item_ptr.is_null() {
            error!(
                func = FUNC,
                "Module list pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let module_list_item = unsafe { &*module_list_item_ptr };

        let ptr = module_list_item.p_module_data;
        let len = module_list_item.num_entries as _;

        trace!(
            func = FUNC,
            modules_ptr = format!("{:#x}", ptr as usize),
            modules_len = len,
            "D-PDU API Call Return"
        );

        let modules = if ptr.is_null() || len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(ptr, len) }
        };

        let module_list = modules
            .into_iter()
            .map(|v| {
                let vendor_module_name = c_str(v.vendor_module_name as _);
                let vendor_additional_info = c_str(v.vendor_additional_info as _);

                trace!(
                    func = FUNC,
                    module_handle = v.h_mod,
                    module_type_id = v.module_type_id,
                    module_name = vendor_module_name,
                    module_add_info = vendor_additional_info,
                    "D-PDU API Call Return"
                );

                PduModuleData {
                    h_mod: v.h_mod,
                    module_type_id: v.module_type_id,
                    vendor_module_name,
                    vendor_additional_info,
                    status: v.status,
                }
            })
            .collect::<Vec<_>>();

        self.pdu_destroy_item(module_list_item_ptr as _)?;

        Ok(module_list)
    }

    pub(crate) fn pdu_get_status(&self, target: &PduStatusTarget) -> ApiResult<PduStatusData> {
        impl_defer_clear_suppress_options!(self, get_status);

        const FUNC: &'static str = "PDUGetStatus";
        self.log_api_call(FUNC);

        let h_mod = target.get_module_handle().unwrap_or(PDU_HANDLE_UNDEF);
        let h_cll = target.get_cll_handle().unwrap_or(PDU_HANDLE_UNDEF);
        let h_cop = target.get_cop_handle().unwrap_or(PDU_HANDLE_UNDEF);

        trace!(func = FUNC, h_mod, h_cll, h_cop, "D-PDU API Call Args");

        let mut status_code: MaybeUninit<u32> = MaybeUninit::uninit();
        let mut timestamp: MaybeUninit<u32> = MaybeUninit::uninit();
        let mut extra_info: MaybeUninit<u32> = MaybeUninit::uninit();

        let get_status_fn = self.symbols.get_status;
        let result = wrap_pdu_call(FUNC, || {
            get_status_fn(
                h_mod,
                h_cll,
                h_cop,
                status_code.as_mut_ptr() as _,
                timestamp.as_mut_ptr(),
                extra_info.as_mut_ptr(),
            )
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_status),
            );
            return Err(result)?;
        }

        let status_code = unsafe { status_code.assume_init() };
        let timestamp = unsafe { timestamp.assume_init() };
        let extra_info = unsafe { extra_info.assume_init() };

        trace!(
            func = FUNC,
            status_code, timestamp, extra_info, "D-PDU API Call Return"
        );

        let status_code = match PduStatus::try_from(status_code) {
            Ok(v) => v,
            Err(_) => {
                error!(
                    func = FUNC,
                    "Received out-of-bounds PduStatus value: {:#x}. Emulation of PduError::FctFailed...",
                    status_code,
                );
                return Err(PduError::FctFailed)?;
            }
        };

        Ok(PduStatusData {
            target: target.clone(),
            status_code,
            timestamp,
            extra_info,
        })
    }

    pub fn pdu_create_com_logical_link(
        &self,
        h_mod: PduModuleHandle,
        create_type: &CllCreateType,
        create_flags: &CllCreateFlags,
        tag: Option<PduUniqueCllTag>,
    ) -> ApiResult<PduCllData> {
        impl_defer_clear_suppress_options!(self, create_logical_link);

        const FUNC: &'static str = "PDUCreateComLogicalLink";
        self.log_api_call(FUNC);

        let tag = tag.map(|v| v.get()).unwrap_or_default();

        trace!(
            func = FUNC,
            h_mod,
            tag = format!("{tag:#x}"),
            "D-PDU API Call Args"
        );

        let flag_bytes = create_flags.get_pdu_flag_data();
        let flag_data = FlagData {
            num_flag_bytes: flag_bytes.len() as _,
            p_flag_data: take_slice_ptr(&flag_bytes),
        };

        let mut cll_handle: MaybeUninit<PduCllHandle> = MaybeUninit::uninit();

        let create_com_logical_link_fn = self.symbols.create_logical_link;
        let result = match &create_type {
            CllCreateType::ResourceId(v) => {
                trace!(func = FUNC, resource_id = v, "D-PDU API Call Args");
                wrap_pdu_call(FUNC, || {
                    create_com_logical_link_fn(
                        h_mod,
                        ptr::null_mut(),
                        v.clone(),
                        tag as *mut _,
                        cll_handle.as_mut_ptr(),
                        &flag_data as *const FlagData as _,
                    )
                })
            }
            CllCreateType::ResourceData {
                bus,
                protocol,
                pins,
            } => {
                trace!(func = FUNC, %bus, %protocol, "D-PDU API Call Args");

                let bus_type_id = bus.resolve_bus_id(FUNC, self)?;
                let protocol_id = protocol.resolve_protocol_id(FUNC, self)?;

                let pin_data = target_pins_to_pin_data(self, FUNC, &pins)?;

                let rsc_data = RscData {
                    bus_type_id,
                    protocol_id,
                    num_pin_data: pin_data.len() as _,
                    p_dlc_pin_data: take_slice_ptr(&pin_data),
                };

                trace!(
                    func = FUNC,
                    rsc_data_ptr = format!("{:#x}", &rsc_data as *const _ as usize),
                    bus_type_id,
                    protocol_id,
                    pin_len = pin_data.len(),
                    "D-PDU API Call Args"
                );

                wrap_pdu_call(FUNC, || {
                    create_com_logical_link_fn(
                        h_mod,
                        &rsc_data as *const RscData as _,
                        PDU_ID_UNDEF,
                        tag as *mut _,
                        cll_handle.as_mut_ptr(),
                        &flag_data as *const FlagData as _,
                    )
                })
            }
        };

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, create_logical_link),
            );
            return Err(result)?;
        }

        let h_cll = unsafe { cll_handle.assume_init() };

        trace!(func = FUNC, h_cll, "D-PDU API Call Return");

        Ok(PduCllData {
            h_mod,
            h_cll: unsafe { cll_handle.assume_init() },
            create_type: create_type.clone(),
            create_flags: create_flags.clone(),
        })
    }

    pub fn pdu_destroy_com_logical_link(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, destroy_logical_link);

        const FUNC: &'static str = "PDUDestroyComLogicalLink";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, "D-PDU API Call Args");

        let destroy_fn = self.symbols.destroy_logical_link;
        let result = wrap_pdu_call(FUNC, || destroy_fn(h_mod, h_cll));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, destroy_logical_link),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub fn pdu_get_last_error(&self, target: &PduLastErrorTarget) -> ApiResult<PduErrorData> {
        impl_defer_clear_suppress_options!(self, get_last_error);

        const FUNC: &'static str = "PDUGetLastError";
        self.log_api_call(FUNC);

        let h_mod = target.get_module_handle().unwrap_or(PDU_HANDLE_UNDEF);
        let h_cll = target.get_cll_handle().unwrap_or(PDU_HANDLE_UNDEF);

        let mut error_code: MaybeUninit<u32> = MaybeUninit::uninit(); // will transform to PduErrorEvt
        let mut h_cop: MaybeUninit<PduCopHandle> = MaybeUninit::uninit(); // maybe undef
        let mut timestamp: MaybeUninit<u32> = MaybeUninit::uninit();
        let mut extra_info_code: MaybeUninit<u32> = MaybeUninit::uninit(); // maybe ID_UNDEF?

        trace!(
            func = FUNC,
            h_mod,
            h_cll,
            error_code_ptr = format!("{:#x}", error_code.as_ptr() as usize),
            h_cop_ptr = format!("{:#x}", h_cop.as_ptr() as usize),
            timestamp_ptr = format!("{:#x}", timestamp.as_ptr() as usize),
            extra_info_code_ptr = format!("{:#x}", extra_info_code.as_ptr() as usize),
            "D-PDU API Call Args"
        );

        let get_last_error_fn = self.symbols.get_last_error;
        let result = wrap_pdu_call(FUNC, || {
            get_last_error_fn(
                h_mod,
                h_cll,
                error_code.as_mut_ptr() as _,
                h_cop.as_mut_ptr(),
                timestamp.as_mut_ptr(),
                extra_info_code.as_mut_ptr(),
            )
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_last_error),
            );
            return Err(result)?;
        }

        let error_code = unsafe { error_code.assume_init() };
        let h_cop = unsafe { h_cop.assume_init() };
        let timestamp = unsafe { timestamp.assume_init() };
        let extra_info_code = unsafe { extra_info_code.assume_init() };

        trace!(
            func = FUNC,
            error_code, h_cop, timestamp, extra_info_code, "D-PDU API Call Return"
        );

        let error_event = match PduErrorEvt::try_from(error_code) {
            Ok(v) => v,
            Err(_) => {
                error!(
                    func = FUNC,
                    "Received out-of-bounds PduErrorEvt value: {:#x}. Emulation of PduError::FctFailed...",
                    error_code,
                );
                return Err(PduError::FctFailed)?;
            }
        };

        Ok(PduErrorData {
            target: target.clone(),
            error_event,
            h_cop: (h_cop != PDU_HANDLE_UNDEF).then(|| h_cop),
            timestamp,
            extra_info_code: (extra_info_code != PDU_ID_UNDEF).then(|| extra_info_code),
        })
    }

    pub fn pdu_get_resource_status(
        &self,
        resources: &[PduResource],
    ) -> ApiResult<PduResourceStatus> {
        impl_defer_clear_suppress_options!(self, get_resource_status);

        const FUNC: &'static str = "PDUGetResourceStatus";
        self.log_api_call(FUNC);

        let mut map = HashMap::new();

        if resources.len() == 0 {
            warn!(func = FUNC, "Resources is empty");
            return Ok(map);
        }

        let raw_resources = resources
            .iter()
            .map(|v| {
                trace!(
                    func = FUNC,
                    resource_h_mod = v.h_mod,
                    resource_id = v.resource_id,
                    "D-PDU API Call Args"
                );

                RscStatusData {
                    h_mod: v.h_mod,
                    resource_id: v.resource_id,
                    resource_status: 0,
                }
            })
            .collect::<Vec<_>>();

        let mut item = RscStatusItem {
            item_type: PduIt::RscStatus,
            num_entries: raw_resources.len() as _,
            p_resource_status_data: take_slice_ptr(&raw_resources),
        };

        trace!(
            func = FUNC,
            item_ptr = format!("{:#x}", &item as *const _ as usize),
            item_len = resources.len(),
            resources_ptr = format!("{:#x}", raw_resources.as_ptr() as usize),
            "D-PDU API Call Args"
        );

        let get_resource_status_fn = self.symbols.get_resource_status;
        let result = wrap_pdu_call(FUNC, || get_resource_status_fn(&mut item));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_resource_status),
            );
            return Err(result)?;
        }

        for resource in resources {
            'il: for raw in raw_resources.iter() {
                if resource.h_mod != raw.h_mod || resource.resource_id != raw.resource_id {
                    continue 'il;
                }

                let status = raw.resource_status;

                let busy = ((status >> 0) & 1).try_into().unwrap(); // SAFE
                let available = ((status >> 1) & 1).try_into().unwrap(); // SAFE
                let transmit_queue_lock = ((status >> 2) & 1).try_into().unwrap(); // SAFE
                let physical_com_param_lock = ((status >> 3) & 1).try_into().unwrap(); // SAFE

                trace!(
                    func = FUNC,
                    resource_h_mod = raw.h_mod,
                    resource_id = raw.resource_id,
                    resource_status = status,
                    busy,
                    available,
                    transmit_queue_lock,
                    physical_com_param_lock,
                    "D-PDU API Call Args"
                );

                map.insert(
                    resource.clone(),
                    ResourceStatus {
                        raw_status: status,
                        busy,
                        available,
                        transmit_queue_lock,
                        physical_com_param_lock,
                    },
                );

                break 'il;
            }
        }

        Ok(map)
    }

    pub fn pdu_connect(&self, h_mod: PduModuleHandle, h_cll: PduCllHandle) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, connect);

        const FUNC: &'static str = "PDUConnect";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, "D-PDU API Call Args");

        let connect_fn = self.symbols.connect;
        let result = wrap_pdu_call(FUNC, || connect_fn(h_mod, h_cll));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, connect),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub fn pdu_disconnect(&self, h_mod: PduModuleHandle, h_cll: PduCllHandle) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, disconnect);

        const FUNC: &'static str = "PDUDisconnect";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, "D-PDU API Call Args");

        let disconnect_fn = self.symbols.disconnect;
        let result = wrap_pdu_call(FUNC, || disconnect_fn(h_mod, h_cll));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, disconnect),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub fn pdu_lock_resource(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        mask: PduLockResourceMask,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, lock_resource);

        const FUNC: &'static str = "PDULockResource";
        self.log_api_call(FUNC);

        let mask_data = mask.get_pdu_data();

        trace!(
            func = FUNC,
            h_mod,
            h_cll,
            mask = format!("0x{mask_data:#x}"),
            lock_physical_com_params = mask.lock_physical_com_params,
            lock_physical_transmit_queue = mask.lock_physical_transmit_queue,
            "D-PDU API Call Args"
        );

        let lock_resource_fn = self.symbols.lock_resource;
        let result = wrap_pdu_call(FUNC, || lock_resource_fn(h_mod, h_cll, mask_data));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, lock_resource),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub fn pdu_unlock_resource(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        mask: PduLockResourceMask,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, unlock_resource);

        const FUNC: &'static str = "PDUUnlockResource";
        self.log_api_call(FUNC);

        let mask_data = mask.get_pdu_data();

        trace!(
            func = FUNC,
            h_mod,
            h_cll,
            mask = format!("0x{mask_data:#x}"),
            lock_physical_com_params = mask.lock_physical_com_params,
            lock_physical_transmit_queue = mask.lock_physical_transmit_queue,
            "D-PDU API Call Args"
        );

        let lock_resource_fn = self.symbols.unlock_resource;
        let result = wrap_pdu_call(FUNC, || lock_resource_fn(h_mod, h_cll, mask_data));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, unlock_resource),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub fn pdu_module_connect(&self, h_mod: PduModuleHandle) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, module_connect);

        const FUNC: &'static str = "PDUModuleConnect";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, "D-PDU API Call Args");

        let module_connect_fn = self.symbols.module_connect;
        let result = wrap_pdu_call(FUNC, || module_connect_fn(h_mod));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, module_connect),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub fn pdu_module_disconnect(&self, h_mod: Option<PduModuleHandle>) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, module_disconnect);

        const FUNC: &'static str = "PDUModuleDisconnect";
        self.log_api_call(FUNC);

        let h_mod = h_mod.unwrap_or(PDU_HANDLE_UNDEF);

        trace!(func = FUNC, h_mod, "D-PDU API Call Args");

        let module_disconnect_fn = self.symbols.module_disconnect;
        let result = wrap_pdu_call(FUNC, || module_disconnect_fn(h_mod));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, module_disconnect),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub(crate) fn pdu_cancel_com_primitive(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        h_cop: PduCopHandle,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, cancel_primitive);

        const FUNC: &'static str = "PDUCancelComPrimitive";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, h_cop, "D-PDU API Call Args");

        let cancel_com_primitive_fn = self.symbols.cancel_primitive;
        let result = wrap_pdu_call(FUNC, || cancel_com_primitive_fn(h_mod, h_cll, h_cop));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, cancel_primitive),
            );
            return Err(result)?;
        }

        Ok(())
    }

    pub fn pdu_get_conflicting_resources(
        &self,
        resource_id: PduObjectId,
        modules: &[PduModuleData],
    ) -> ApiResult<PduConflictingModules> {
        impl_defer_clear_suppress_options!(self, get_conflicting_resources);

        const FUNC: &'static str = "PDUGetConflictingResources";
        self.log_api_call(FUNC);

        trace!(func = FUNC, resource_id, "D-PDU API Call Args");

        let mut module_names: Vec<CString> = vec![];
        let mut module_infos: Vec<CString> = vec![];

        let module_items = modules
            .iter()
            .map(|m| {
                trace!(
                    func = FUNC,
                    h_mod = m.h_mod,
                    module_type_id = m.module_type_id,
                    "D-PDU API Call Args"
                );

                let module_name_idx = module_names.len();
                let module_info_idx = module_infos.len();

                module_names.push(
                    CString::new(m.vendor_module_name.clone().unwrap_or_else(String::new))
                        .expect("CString::new()"), // infallible
                );

                module_infos.push(
                    CString::new(m.vendor_additional_info.clone().unwrap_or_else(String::new))
                        .expect("CString::new()"), // infallible
                );

                ModuleData {
                    module_type_id: m.module_type_id,
                    h_mod: m.h_mod,
                    vendor_module_name: module_names[module_name_idx].as_ptr() as _,
                    vendor_additional_info: module_infos[module_info_idx].as_ptr() as _,
                    status: m.status,
                }
            })
            .collect::<Vec<_>>();

        let module_data = ModuleItem {
            item_type: PduIt::ModuleId,
            num_entries: module_items.len() as _,
            p_module_data: take_slice_ptr(&module_items),
        };

        let mut conflict_data_ptr = ptr::null_mut();

        trace!(
            func = FUNC,
            input_module_list_ptr = format!("{:#x}", &module_data as *const _ as usize),
            output_conflict_list_ptr = format!("{:#x}", &conflict_data_ptr as *const _ as usize),
            "D-PDU API Call Args"
        );

        let get_conflicting_resources_fn = self.symbols.get_conflicting_resources;
        let result = wrap_pdu_call(FUNC, || {
            get_conflicting_resources_fn(
                resource_id,
                &module_data as *const _ as _,
                &mut conflict_data_ptr,
            )
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_conflicting_resources),
            );
            return Err(result)?;
        }

        trace!(
            func = FUNC,
            item_ptr = format!("0x{:x}", conflict_data_ptr as usize),
            item_type = ?NonNull::new(conflict_data_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
            "D-PDU API Call Return"
        );

        if conflict_data_ptr.is_null() {
            error!(
                func = FUNC,
                "Item data pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let conflict_data = unsafe { &*conflict_data_ptr };

        if !matches!(conflict_data.item_type, PduIt::RscConflict) {
            error!(
                func = FUNC,
                "Invalid item type received: PduIt::{}. Emulation of PduError::FctFailed...",
                conflict_data.item_type.as_str(),
            );

            self.pdu_destroy_item(conflict_data_ptr as _)?;
            return Err(PduError::FctFailed)?;
        }

        let ptr = conflict_data.p_rsc_conflict_data;
        let len = conflict_data.num_entries as usize;

        let conflict_items = if ptr.is_null() || len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(ptr, len) }
        };

        let map = conflict_items
            .iter()
            .map(|i| {
                trace!(
                    func = FUNC,
                    conflicting_h_mod = i.h_mod,
                    conflicting_resource_id = i.resource_id,
                    "D-PDU API Call Return"
                );
                (i.h_mod, i.resource_id)
            })
            .collect();

        self.pdu_destroy_item(conflict_data_ptr as _)?;

        Ok(map)
    }

    pub fn pdu_get_resource_ids(
        &self,
        h_mod: Option<PduModuleHandle>,
        bus: &BusSource,
        protocol: &ProtocolSource,
        pins: &[TargetPin],
    ) -> ApiResult<PduModulesResourcesIds> {
        impl_defer_clear_suppress_options!(self, get_resource_ids);

        const FUNC: &'static str = "PDUGetResourceIds";
        self.log_api_call(FUNC);

        let h_mod = h_mod.unwrap_or(PDU_HANDLE_UNDEF);

        trace!(
            func = FUNC,
            h_mod,
            %bus,
            %protocol,
            "D-PDU API Call Args",
        );

        let bus_id = bus.resolve_bus_id(FUNC, self)?;
        let protocol_id = protocol.resolve_protocol_id(FUNC, self)?;
        let pin_data = target_pins_to_pin_data(self, FUNC, pins)?;

        let resource_data = RscData {
            bus_type_id: bus_id,
            protocol_id,
            num_pin_data: pin_data.len() as _,
            p_dlc_pin_data: take_slice_ptr(&pin_data),
        };

        let mut rsc_data_ptr = ptr::null_mut();

        let get_resource_ids_fn = self.symbols.get_resource_ids;
        let result = wrap_pdu_call(FUNC, || {
            get_resource_ids_fn(h_mod, &resource_data as *const _ as _, &mut rsc_data_ptr)
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_resource_ids),
            );
            return Err(result)?;
        }

        trace!(
            func = FUNC,
            item_ptr = format!("0x{:x}", rsc_data_ptr as usize),
            item_type = ?NonNull::new(rsc_data_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
            "D-PDU API Call Return"
        );

        if rsc_data_ptr.is_null() {
            error!(
                func = FUNC,
                "Item data pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let rsc_data = unsafe { &*rsc_data_ptr };

        if !matches!(rsc_data.item_type, PduIt::RscConflict) {
            error!(
                func = FUNC,
                "Invalid item type received: PduIt::{}. Emulation of PduError::FctFailed...",
                rsc_data.item_type.as_str(),
            );

            self.pdu_destroy_item(rsc_data_ptr as _)?;
            return Err(PduError::FctFailed)?;
        }

        let mut map = PduModulesResourcesIds::with_capacity(rsc_data.num_modules as _);

        let rsc_items_ptr = rsc_data.p_id_item_data;
        let rsc_items_len = rsc_data.num_modules as usize;

        let rsc_items = if rsc_items_ptr.is_null() || rsc_items_len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(rsc_items_ptr, rsc_items_len) }
        };

        for rsc_item in rsc_items {
            let rsc_ids_ptr = rsc_item.p_resource_id_array;
            let rsc_ids_len = rsc_item.num_ids as usize;

            let resource_ids = if rsc_ids_ptr.is_null() || rsc_ids_len == 0 {
                &[]
            } else {
                unsafe { slice::from_raw_parts(rsc_ids_ptr, rsc_ids_len) }
            };

            trace!(
                func = FUNC,
                rsc_item_h_mod = rsc_item.h_mod,
                rsc_item_resource_ids = ?resource_ids,
                "D-PDU API Call Return"
            );

            map.insert(rsc_item.h_mod, resource_ids.to_vec());
        }

        Ok(map)
    }

    pub fn pdu_get_timestamp(&self, h_mod: PduModuleHandle) -> ApiResult<u32> {
        impl_defer_clear_suppress_options!(self, get_timestamp);

        const FUNC: &'static str = "PDUGetTimestamp";
        self.log_api_call(FUNC);

        let mut timestamp = MaybeUninit::uninit();

        trace!(
            func = FUNC,
            h_mod,
            timestamp_ptr = format!("{:#x}", timestamp.as_ptr() as usize),
            "D-PDU API Call Args"
        );

        let get_timestamp_fn = self.symbols.get_timestamp;
        let result = wrap_pdu_call(FUNC, || get_timestamp_fn(h_mod, timestamp.as_mut_ptr()));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_timestamp),
            );
            return Err(result)?;
        }

        let timestamp = unsafe { timestamp.assume_init() };

        trace!(func = FUNC, timestamp, "D-PDU API Call Return");

        Ok(timestamp)
    }

    pub fn pdu_get_unique_resp_id_table(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<PduComParamTable> {
        impl_defer_clear_suppress_options!(self, get_unique_resp_id_table);

        const FUNC: &'static str = "PDUGetUniqueRespIdTable";
        self.log_api_call(FUNC);

        let mut table_item_ptr = ptr::null_mut();

        trace!(
            func = FUNC,
            h_mod,
            h_cll,
            item_ptr = format!("{:#x}", &table_item_ptr as *const _ as usize),
            "D-PDU API Call Return"
        );

        let get_timestamp_fn = self.symbols.get_unique_resp_id_table;
        let result = wrap_pdu_call(FUNC, || get_timestamp_fn(h_mod, h_cll, &mut table_item_ptr));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_unique_resp_id_table),
            );
            return Err(result)?;
        }

        trace!(
            func = FUNC,
            item_ptr = format!("0x{:x}", table_item_ptr as usize),
            item_type = ?NonNull::new(table_item_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
            "D-PDU API Call Return"
        );

        if table_item_ptr.is_null() {
            error!(
                func = FUNC,
                "Item data pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let table_item = unsafe { &*table_item_ptr };

        if !matches!(table_item.item_type, PduIt::RscConflict) {
            error!(
                func = FUNC,
                "Invalid item type received: PduIt::{}. Emulation of PduError::FctFailed...",
                table_item.item_type.as_str(),
            );

            self.pdu_destroy_item(table_item_ptr as _)?;
            return Err(PduError::FctFailed)?;
        }

        let table_ptr = table_item.p_unique_data;
        let table_len = table_item.num_entries as usize;

        let table = if table_ptr.is_null() || table_len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(table_ptr, table_len) }
        };

        let mut map = PduComParamTable::with_capacity(table.len());

        trace!(
            func = FUNC,
            table_len = table.len(),
            "D-PDU API Call Return"
        );

        for row in table {
            let unique_id = row.unique_resp_identifier;

            let com_params_ptr = row.p_params;
            let com_params_len = row.num_param_items as usize;

            let com_params = if com_params_ptr.is_null() || com_params_len == 0 {
                &[]
            } else {
                unsafe { slice::from_raw_parts(com_params_ptr, com_params_len) }
            };

            trace!(
                func = FUNC,
                table_item_unique_id = unique_id,
                table_item_cp_len = com_params.len(),
                "D-PDU API Call Return"
            );

            for cp in com_params {
                if !matches!(cp.item_type, PduIt::Param) {
                    error!(
                        func = FUNC,
                        "Invalid ComParam type received: PduIt::{}. Emulation of PduError::FctFailed...",
                        cp.item_type.as_str(),
                    );

                    self.pdu_destroy_item(table_item_ptr as _)?;
                    return Err(PduError::FctFailed)?;
                }

                trace!(
                    func = FUNC,
                    table_item_cp_id = cp.com_param_id,
                    table_item_cp_type = cp.com_param_data_type.as_str(),
                    table_item_cp_class = cp.com_param_class.as_str(),
                    "D-PDU API Call Return"
                );

                let variant: CpVariant = unsafe {
                    use ptr::read;

                    match cp.com_param_data_type {
                        PduPt::Unum8 => read::<u8>(cp.p_com_param_data as _).into(),
                        PduPt::Snum8 => read::<i8>(cp.p_com_param_data as _).into(),
                        PduPt::Unum16 => read::<u16>(cp.p_com_param_data as _).into(),
                        PduPt::Snum16 => read::<i16>(cp.p_com_param_data as _).into(),
                        PduPt::Unum32 => read::<u32>(cp.p_com_param_data as _).into(),
                        PduPt::Snum32 => read::<i32>(cp.p_com_param_data as _).into(),
                        PduPt::ByteField => {
                            let data = read::<ParamByteFieldData>(cp.p_com_param_data as _);
                            let ptr = data.p_data_array;
                            let len = data.param_act_len as usize;
                            let bytes = if ptr.is_null() || len == 0 {
                                &[]
                            } else {
                                slice::from_raw_parts(ptr, len)
                            };
                            (bytes.to_vec(), data.param_max_len as usize).into()
                        }
                        PduPt::LongField => {
                            let data = read::<ParamLongFieldData>(cp.p_com_param_data as _);
                            let ptr = data.p_data_array;
                            let len = data.param_act_len as usize;
                            let nums = if ptr.is_null() || len == 0 {
                                &[]
                            } else {
                                slice::from_raw_parts(ptr, len)
                            };
                            (nums.to_vec(), data.param_max_len as usize).into()
                        }
                        PduPt::StructField => {
                            let data = read::<ParamStructFieldData>(cp.p_com_param_data as _);
                            let ptr = data.p_struct_array;
                            let len = data.param_act_entries as usize;
                            match data.com_param_struct_type {
                                PduCpst::AccessTiming => {
                                    let structs: &[ParamStructAccessTiming] =
                                        if ptr.is_null() || len == 0 {
                                            &[]
                                        } else {
                                            slice::from_raw_parts(ptr as _, len)
                                        };
                                    (structs.to_vec(), data.param_max_entries as usize).into()
                                }
                                PduCpst::SessionTiming => {
                                    let structs: &[ParamStructSessionTiming] =
                                        if ptr.is_null() || len == 0 {
                                            &[]
                                        } else {
                                            slice::from_raw_parts(ptr as _, len)
                                        };
                                    (structs.to_vec(), data.param_max_entries as usize).into()
                                }
                            }
                        }
                    }
                };

                let com_param = PduComParam::from_id(cp.com_param_id, cp.com_param_class, variant);

                com_param.try_init_short_name(self);

                map.add(unique_id, com_param);
            }
        }

        self.pdu_destroy_item(table_item_ptr as _)?;

        Ok(map)
    }

    pub(crate) fn vt_module_destructor(&self, h_mod: PduModuleHandle) -> ApiResult<()> {
        self.modify_suppress_log_options(|options| {
            options.get_status = PduErrorFlag::INVALID_HANDLE;
        });

        let target = PduStatusTarget::Module(h_mod);
        let data = self.pdu_get_status(&target)?;

        match data.status_code {
            PduStatus::ModstReady | PduStatus::ModstNotReady => {
                let _ = self.pdu_module_disconnect(Some(h_mod));
            }
            _ => {}
        }

        self.modify_suppress_log_options(|options| {
            options.register_event_callback = PduErrorFlag::MODULE_NOT_CONNECTED;
        });

        let _ = self.pdu_register_event_callback(&PduEventTarget::Module(h_mod), None)?;

        Ok(())
    }

    pub(crate) fn vt_cll_destructor(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        self.modify_suppress_log_options(|options| {
            options.get_status = PduErrorFlag::INVALID_HANDLE;
        });

        let target = PduStatusTarget::LogicalLink(h_mod, h_cll);
        let status = self
            .pdu_get_status(&target)
            .map(|v| v.status_code)
            .unwrap_or(PduStatus::CllstOffline); // for bad drivers
        
        match status {
            PduStatus::CllstOnline | PduStatus::CllstCommStarted => {
                let _ = self.pdu_disconnect(h_mod, h_cll);
            }
            _ => { /* same as above */ }
        }

        let _ = self.pdu_register_event_callback(&PduEventTarget::LogicalLink(h_mod, h_cll), None);
        let _ = self.pdu_destroy_com_logical_link(h_mod, h_cll);

        Ok(())
    }

    pub(crate) fn vt_cop_destructor(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        h_cop: PduCopHandle,
    ) -> ApiResult<()> {
        self.modify_suppress_log_options(|options| {
            options.get_status = PduErrorFlag::INVALID_HANDLE;
        });

        let target = PduStatusTarget::Primitive(h_mod, h_cll, h_cop);
        let data = self.pdu_get_status(&target)?;

        match data.status_code {
            PduStatus::CopstWaiting | PduStatus::CopstIdle | PduStatus::CopstExecuting => {
                self.modify_suppress_log_options(|options| {
                    options.cancel_primitive = PduErrorFlag::INVALID_HANDLE;
                });
                self.pdu_cancel_com_primitive(h_mod, h_cll, h_cop)?;
            }
            _ => { /* same as above */ }
        }

        Ok(())
    }

    /// Reset specific MVCI protocol module.
    pub fn vt_io_ctl_reset(&self) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::System,
            &PduIoCtlCommand::from("PDU_IOCTL_RESET"),
            None,
        )?;

        Ok(())
    }

    /// Clear transmit queue of specific ComLogicalLink.
    pub fn vt_io_ctl_clear_tx_queue(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_CLEAR_TX_QUEUE"),
            None,
        )?;
        Ok(())
    }

    /// Suspend transmit queue of specific ComLogicalLink. The queue processing will be halted
    /// upon this command. This can be used to fill up a ComLogicalLink's queue with
    /// ComPrimitives to achieve a steady processing of ComPrimitives after resuming
    /// the queue (e.g. for fast flash programming operation).
    pub fn vt_io_ctl_suspend_tx_queue(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_SUSPEND_TX_QUEUE"),
            None,
        )?;
        Ok(())
    }

    /// Resume transmit queue of specific ComLogicalLink. The queue processing
    /// will be started upon this command.
    pub fn vt_io_ctl_resume_tx_queue(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_RESUME_TX_QUEUE"),
            None,
        )?;
        Ok(())
    }

    /// Clear event queue of specific ComLogicalLink.
    pub fn vt_io_ctl_clear_rx_queue(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_CLEAR_RX_QUEUE"),
            None,
        )?;
        Ok(())
    }

    /// Reads the vehicle battery voltage using the `PDU_IOCTL_READ_VBATT` IO control command.
    ///
    /// The returned voltage is converted from millivolts to volts.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(voltage))` - Battery voltage in volts.
    /// - `Ok(None)` - The module does not support the `PDU_IOCTL_READ_VBATT` command.
    /// - `Err(...)` - An error occurred while executing the IOCTL request.
    pub fn vt_io_ctl_read_vbatt(&self, h_mod: PduModuleHandle) -> ApiResult<Option<f32>> {
        let ioctl_target = PduIoCtlTarget::Module(h_mod);
        let ioctl_id = match self.pdu_get_object_id(PduObjt::IoCtrl, "PDU_IOCTL_READ_VBATT")? {
            Some(v) => PduIoCtlCommand::Id(v),
            None => return Ok(None)
        };

        match self.pdu_io_ctl(&ioctl_target, &ioctl_id, None) {
            Ok(Some(PduIoCtlData::U32(v))) => Ok(Some(v as f32 / 1000.0)),
            Ok(Some(_)) => {
                error!(
                    h_mod,
                    "IoCtl output data is wrong. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            },
            Ok(None) => {
                error!(
                    h_mod,
                    "IoCtl output data is null. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
            Err(ApiError::PduError(PduError::IdNotSupported)) => return Ok(None),
            Err(err) => Err(err)
        }
    }

    pub fn vt_io_ctl_set_prog_voltage(
        &self,
        h_mod: PduModuleHandle,
        voltage: f32,
        pin: u32,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::Module(h_mod),
            &PduIoCtlCommand::from("PDU_IOCTL_SET_PROG_VOLTAGE"),
            Some(&PduIoCtlData::ProgVoltage(IoProgVoltageData {
                prog_voltage_mv: (voltage * 1000.0) as u32,
                pin_on_dlc: pin,
            })),
        )?;
        Ok(())
    }

    pub fn vt_io_ctl_read_prog_voltage(&self, h_mod: PduModuleHandle) -> ApiResult<f32> {
        match self.pdu_io_ctl(
            &PduIoCtlTarget::Module(h_mod),
            &PduIoCtlCommand::from("PDU_IOCTL_READ_PROG_VOLTAGE"),
            None,
        )? {
            Some(PduIoCtlData::ProgVoltage(v)) => Ok(v.prog_voltage_mv as f32 / 1000.0),
            Some(_) => {
                error!(
                    h_mod,
                    "IoCtl output data is wrong. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
            None => {
                error!(
                    h_mod,
                    "IoCtl output data is null. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
        }
    }

    pub fn vt_io_ctl_generic(&self, h_mod: PduModuleHandle, data: &[u8]) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::Module(h_mod),
            &PduIoCtlCommand::from("PDU_IOCTL_GENERIC"),
            Some(&PduIoCtlData::ByteArray(IoCtlByteArray(data.to_vec()))),
        )?;
        Ok(())
    }

    pub fn vt_io_ctl_set_buffer_size(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        size: u32,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_SET_BUFFER_SIZE"),
            Some(&PduIoCtlData::U32(size)),
        )?;
        Ok(())
    }

    pub fn vt_io_ctl_start_msg_filter(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        filter: IoFilterData,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_START_MSG_FILTER"),
            Some(&PduIoCtlData::Filter(filter)),
        )?;
        Ok(())
    }

    pub fn vt_io_ctl_stop_msg_filter(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        number: u32,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_STOP_MSG_FILTER"),
            Some(&PduIoCtlData::U32(number)),
        )?;
        Ok(())
    }

    pub fn vt_io_ctl_clear_msg_filter(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_CLEAR_MSG_FILTER"),
            None,
        )?;
        Ok(())
    }

    pub fn vt_io_ctl_set_event_queue_properties(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        size: u32,
        mode: PduQueueMode,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_SET_EVENT_QUEUE_PROPERTIES"),
            Some(&PduIoCtlData::EventQueueProperty(
                IoEventQueuePropertyData {
                    queue_size: size,
                    queue_mode: mode,
                },
            )),
        )?;
        Ok(())
    }

    pub fn vt_io_ctl_get_cable_id(&self, h_mod: PduModuleHandle) -> ApiResult<Option<PduObjectId>> {
        match self.pdu_io_ctl(
            &PduIoCtlTarget::Module(h_mod),
            &PduIoCtlCommand::from("PDU_IOCTL_GET_CABLE_ID"),
            None,
        )? {
            Some(PduIoCtlData::U32(v)) if v == PDU_ID_UNDEF => Ok(None),
            Some(PduIoCtlData::U32(v)) => Ok(Some(v)),
            Some(_) => {
                error!(
                    h_mod,
                    "IoCtl output data is wrong. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
            None => {
                error!(
                    h_mod,
                    "IoCtl output data is null. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
        }
    }

    pub fn vt_io_ctl_send_break(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_SEND_BREAK"),
            None,
        )?;
        Ok(())
    }

    pub fn vt_io_ctl_read_ignition_sense_state(
        &self,
        h_mod: PduModuleHandle,
        pin: Option<u32>,
    ) -> ApiResult<bool> {
        match self.pdu_io_ctl(
            &PduIoCtlTarget::Module(h_mod),
            &PduIoCtlCommand::from("PDU_IOCTL_READ_IGNITION_SENSE_STATE"),
            Some(&PduIoCtlData::U32(pin.unwrap_or(0))),
        )? {
            Some(PduIoCtlData::U32(v)) => Ok(if v > 0 { true } else { false }),
            Some(_) => {
                error!(
                    h_mod,
                    "IoCtl output data is wrong. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
            None => {
                error!(
                    h_mod,
                    "IoCtl output data is null. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
        }
    }

    pub(crate) fn blocking_listen_events(
        mut pdu_event_rx: mpsc::UnboundedReceiver<PduEvent>,
        mut api_event_tx: broadcast::Sender<()>,
    ) {
        loop {
            let event = match pdu_event_rx.blocking_recv() {
                Some(value) => value,
                None => {
                    // The channel will be closed when `drop()` is called for the `PduApi`.
                    break;
                }
            };

            if Self::handle_event(event, &mut api_event_tx) {
                break;
            }
        }
    }

    pub(crate) fn handle_event(
        _event: PduEvent,
        _api_event_tx: &mut broadcast::Sender<()>,
    ) -> StopReceive {
        false
    }
}

fn target_pins_to_pin_data(
    api: &PduApi,
    func_name: &str,
    pins: &[TargetPin],
) -> ApiResult<Vec<PinData>> {
    let mut vec = Vec::with_capacity(pins.len());

    for pin in pins.iter() {
        trace!(
            func = func_name,
            pin_num = pin.num_on_vci,
            pin_type = %pin.pin_type,
            "D-PDU API Call Args"
        );

        let pin_id = match &pin.pin_type {
            PinSource::Id(id) => id.clone(),
            PinSource::Name(name) => match api.pdu_get_object_id(PduObjt::PinType, name)? {
                Some(id) => id,
                None => {
                    api.log_api_call_fail(
                        func_name,
                        PduError::FctFailed,
                        Some(format!("unable to lookup pin type by name: {name}")),
                        None,
                    );
                    return Err(PduError::FctFailed)?;
                }
            },
        };

        vec.push(PinData {
            dlc_pin_number: pin.num_on_vci,
            dlc_pin_type_id: pin_id,
        });
    }

    Ok(vec)
}
