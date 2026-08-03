macro_rules! impl_defer_clear_suppress_options {
    ($self:expr, $func:ident) => {
        let suppress_log_options = $self.suppress_log_options.get_or_default().clone();

        ::scopeguard::defer! {
            let mut options = suppress_log_options.write();
            options.$func = ::dpdu_api_types::bitflags::PduErrorFlag::empty();
        }
    };
}

macro_rules! resolve_level_of_log_api_call_fail {
    ($self:expr, $result:expr, $func:ident) => {{
        let suppress_log_options = $self.suppress_log_options.get_or_default().read();
        suppress_log_options
            .$func
            .contains($result.flag())
            .then_some(::tracing::Level::DEBUG)
    }};
}

pub mod pdu_cancel_com_primitive;
pub mod pdu_connect;
pub mod pdu_construct;
pub mod pdu_create_com_logical_link;
pub mod pdu_destroy_com_logical_link;
pub mod pdu_destroy_item;
pub mod pdu_destruct;
pub mod pdu_disconnect;
pub mod pdu_get_com_param;
pub mod pdu_get_conflicting_resources;
pub mod pdu_get_event_item;
pub mod pdu_get_last_error;
pub mod pdu_get_module_ids;
pub mod pdu_get_object_id;
pub mod pdu_get_resource_ids;
pub mod pdu_get_resource_status;
pub mod pdu_get_status;
pub mod pdu_get_timestamp;
pub mod pdu_get_unique_resp_id_table;
pub mod pdu_get_version;
pub mod pdu_io_ctl;
pub mod pdu_lock_resource;
pub mod pdu_module_connect;
pub mod pdu_module_disconnect;
pub mod pdu_register_event_callback;
pub mod pdu_set_com_param;
pub mod pdu_set_unique_resp_id_table;
pub mod pdu_start_com_primitive;
pub mod pdu_unlock_resource;
pub(crate) mod vt_cll_destructor;
pub(crate) mod vt_cop_destructor;
pub mod vt_io_ctl_clear_msg_filter;
pub mod vt_io_ctl_clear_rx_queue;
pub mod vt_io_ctl_clear_tx_queue;
pub mod vt_io_ctl_generic;
pub mod vt_io_ctl_get_cable_id;
pub mod vt_io_ctl_read_ignition_sense_state;
pub mod vt_io_ctl_read_prog_voltage;
pub mod vt_io_ctl_read_vbatt;
pub mod vt_io_ctl_reset;
pub mod vt_io_ctl_resume_tx_queue;
pub mod vt_io_ctl_send_break;
pub mod vt_io_ctl_set_buffer_size;
pub mod vt_io_ctl_set_event_queue_properties;
pub mod vt_io_ctl_set_prog_voltage;
pub mod vt_io_ctl_start_msg_filter;
pub mod vt_io_ctl_stop_msg_filter;
pub mod vt_io_ctl_suspend_tx_queue;
pub(crate) mod vt_module_destructor;

use crate::constants::API_EVENTS_QUEUE_SIZE;
use crate::error::GeneralError;
use crate::handle_manager::PduHandleManager;
use crate::types::pdu_event::{PduEvent, StopReceive};
use crate::types::pdu_resource::{PinSource, TargetPin};
use crate::types::{PduLibraryPath, PduOptions, PduUniqueApiTag};
use crate::utils::module_description::{PduModuleDescription, PduModuleDescriptionError};
use crate::utils::root_file::Mvci;
use crate::utils::{NonClonable, random_non_zero_usize};
use dpdu_api_types::bitflags::PduErrorFlag;
use dpdu_api_types::{
    PduCancelComPrimitiveFn, PduConnectFn, PduConstructFn, PduCreateComLogicalLinkFn,
    PduDestroyComLogicalLinkFn, PduDestroyItemFn, PduDestructFn, PduDisconnectFn, PduError,
    PduGetComParamFn, PduGetConflictingResourcesFn, PduGetEventItemFn, PduGetLastErrorFn,
    PduGetModuleIdsFn, PduGetObjectIdFn, PduGetResourceIdsFn, PduGetResourceStatusFn,
    PduGetStatusFn, PduGetTimestampFn, PduGetUniqueRespIdTableFn, PduGetVersionFn, PduIoctlFn,
    PduLockResourceFn, PduModuleConnectFn, PduModuleDisconnectFn, PduObjt, PduRegisterCallbackFn,
    PduSetComParamFn, PduSetUniqueRespIdTableFn, PduStartComPrimitiveFn, PduUnlockResourceFn,
    PinData,
};
use parking_lot::{Mutex, RwLock};
use std::any::type_name;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Weak};
use std::thread::spawn;
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
