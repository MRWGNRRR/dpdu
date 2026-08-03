use crate::api::{ApiError, ApiResult, PduApi};
use crate::constants::COP_EVENTS_QUEUE_SIZE;
use crate::error::{GeneralError, GeneralResult};
use crate::handle_manager::PduHandleManager;
use crate::types::pdu_com_param::table::{IntoPduComParam, MapTarget, PduComParamTable, SetTarget};
use crate::types::pdu_com_primitive::{
    ComParamBuffer, ExpectedResponse, MaskData, PduPrimitive, PrimitiveParams,
    PrimitiveStatusStore, PrimitiveType, ReceiveCycles, ResponseType, SendCycles, TransmitFlags,
};
use crate::types::pdu_event::{ErrorEventStore, PduEvent, StopReceive};
use crate::types::pdu_resource::{BusSource, ProtocolSource, TargetPin};
use crate::types::pdu_status::{PduStatusData, PduStatusTarget};
use crate::types::{PduCllHandle, PduModuleHandle, PduObjectId, PduUniqueCllTag, PduUniqueCopTag};
use crate::utils::can::{CanFrame, RawCanPrimitiveBuilderExt};
use crate::utils::{NonClonable, random_non_zero_usize};
use crate::worker::{PduAsyncWorker, Query};
use bytes::Bytes;
use dpdu_api_types::{PduError, PduStatus};
use parking_lot::Mutex;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::{Arc, Once, OnceLock, Weak};
use std::thread::spawn;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::task::spawn_blocking;
use tracing::{debug, error};

#[derive(Debug)]
pub struct PduLogicalLink {
    pub(crate) me: Weak<PduLogicalLink>,

    pub(crate) api: Arc<PduApi>,

    pub(crate) worker: OnceLock<Arc<PduAsyncWorker>>,

    pub(crate) unique_tag: PduUniqueCllTag,

    pub(crate) cll_data: PduCllData,

    /// Event channel sender owned by [`PduLogicalLink`].
    ///
    /// The sender is intentionally dropped when [`PduLogicalLink`] is dropped,
    /// allowing the event listener to detect shutdown.
    ///
    /// A weak reference is stored in [`PduHandleManager`] to keep track of the
    /// channel lifetime and to automatically stop event listening in
    /// [`PduLogicalLink::listen_events`] and
    /// [`PduLogicalLink::blocking_listen_events`].
    ///
    /// # Safety
    ///
    /// This sender must not be cloned. Cloning it would extend the channel
    /// lifetime beyond [`PduLogicalLink`] and prevent listeners from being
    /// automatically stopped.
    ///
    /// [`PduHandleManager`]: crate::handle_manager::PduHandleManager
    pub(crate) pdu_event_tx: NonClonable<mpsc::UnboundedSender<PduEvent>>,

    /// Sender used to create additional receivers after the initial receiver
    /// has been taken.
    pub(crate) logical_link_event_tx: broadcast::Sender<()>,

    /// The initial receiver returned by the first [`get_logical_link_event_receiver`] call.
    ///
    /// After the receiver is taken, subsequent calls create new receivers from
    /// [`logical_link_event_tx`].
    pub(crate) logical_link_event_rx: Mutex<Option<broadcast::Receiver<()>>>,

    pub(crate) sync: Mutex<()>,
}

impl PartialEq for PduLogicalLink {
    fn eq(&self, other: &Self) -> bool {
        self.api.unique_tag == other.api.unique_tag && self.unique_tag == other.unique_tag
    }
}

impl PduLogicalLink {
    pub fn get_module_handle(&self) -> PduModuleHandle {
        self.cll_data.h_mod
    }

    pub fn get_cll_handle(&self) -> PduCllHandle {
        self.cll_data.h_cll
    }

    pub fn get_create_flags(&self) -> &CllCreateFlags {
        &self.cll_data.create_flags
    }

    pub fn get_create_type(&self) -> &CllCreateType {
        &self.cll_data.create_type
    }

    pub fn get_unique_tag(&self) -> PduUniqueCllTag {
        self.unique_tag
    }

    fn take_me_expect(&self) -> Arc<PduLogicalLink> {
        self.me
            .upgrade()
            .expect("internal error: PduLogicalLink self-reference is no longer valid")
    }

    pub fn blocking_get_status(&self) -> ApiResult<CllStatus> {
        let _sync_guard = self.sync.lock();
        let target = PduStatusTarget::LogicalLink(self.get_module_handle(), self.get_cll_handle());
        let result = self.api.pdu_get_status(&target)?;
        Ok(CllStatus(result))
    }

    pub async fn get_status(&self) -> GeneralResult<CllStatus> {
        match self.worker.get() {
            Some(worker) => {
                let target =
                    PduStatusTarget::LogicalLink(self.get_module_handle(), self.get_cll_handle());
                let result = worker.pdu_get_status(target).await?;
                Ok(CllStatus(result))
            }
            None => {
                let me = self.take_me_expect();
                let result = spawn_blocking(move || me.blocking_get_status())
                    .await
                    .expect(
                        "internal error: PduLogicalLink::blocking_get_status() task panicked",
                    )?;

                Ok(result)
            }
        }
    }

    pub fn blocking_connect(&self) -> ApiResult<bool> {
        let status = self.blocking_get_status()?;
        if !status.is_offline() {
            return Ok(false);
        }

        let _sync_guard = self.sync.lock();
        self.api
            .pdu_connect(self.get_module_handle(), self.get_cll_handle())?;
        Ok(true)
    }

    pub async fn connect(&self) -> GeneralResult<bool> {
        match self.worker.get() {
            Some(worker) => {
                let status = self.get_status().await?;
                if !status.is_offline() {
                    return Ok(false);
                }
                worker
                    .pdu_connect(self.get_module_handle(), self.get_cll_handle())
                    .await?;
                Ok(true)
            }
            None => {
                let me = self.take_me_expect();
                let result = spawn_blocking(move || me.blocking_connect())
                    .await
                    .expect("internal error: PduLogicalLink::blocking_connect() task panicked")?;

                Ok(result)
            }
        }
    }

    pub fn blocking_disconnect(&self) -> GeneralResult<bool> {
        let status = self.blocking_get_status()?;
        if status.is_offline() {
            return Ok(false);
        }

        let _sync_guard = self.sync.lock();
        self.api
            .pdu_disconnect(self.get_module_handle(), self.get_cll_handle())?;
        Ok(true)
    }

    pub async fn disconnect(&self) -> GeneralResult<bool> {
        match self.worker.get() {
            Some(worker) => {
                let status = self.get_status().await?;
                if status.is_offline() {
                    return Ok(false);
                }
                worker
                    .pdu_disconnect(self.get_module_handle(), self.get_cll_handle())
                    .await?;
                Ok(true)
            }
            None => {
                let me = self.take_me_expect();
                let result = spawn_blocking(move || me.blocking_connect()).await.expect(
                    "internal error: PduLogicalLink::blocking_disconnect() task panicked",
                )?;

                Ok(result)
            }
        }
    }

    pub fn create_primitive(
        &self,
        primitive_type: PrimitiveType,
        events_queue_size: Option<usize>,
    ) -> Arc<PduPrimitive> {
        let events_queue_size = events_queue_size.unwrap_or(COP_EVENTS_QUEUE_SIZE);

        let unique_tag: PduUniqueCopTag = random_non_zero_usize();

        // The transmitter of this channel will be sent to the PduHandlerManager so that
        // the D-PDU API event handler can send events to this channel.
        //
        // The receiver will be used in an asynchronous event processing task from
        // the D-PDU API via PduPrimitive::blocking_handle_events().
        let (pdu_event_tx, pdu_event_rx) = mpsc::unbounded_channel();

        // High-level event channel of the D-PDU API for PduPrimitive.
        //
        // When an event is received in an asynchronous event processing task via
        // the PduPrimitive::blocking_handle_events(), it will be sent to the receiver of
        // this channel, provided that the event type is Result.
        //
        // The receiver is used in the get_result_receiver() method of
        // the PduPrimitive structure.
        let (primitive_event_tx, primitive_event_rx) = broadcast::channel(events_queue_size);

        // The flag of a dead primitive.
        //
        // It is set if:
        //   - an Error event has been registered in the asynchronous event
        //     processing task of the D-PDU API via the PduPrimitive::blocking_handle_event().
        //   - a Result event with the type CopstFinished or CopstExecuting was received.
        let primitive_dead_flag = Arc::new(Once::new());

        // An "Error" event store for PduPrimitive.
        //
        // Used in an asynchronous event-handling task in the D-PDU API via
        // the PduPrimitive::blocking_handle_event().
        // And also in `PduPrimitive` when checking the primitive’s lifetime.
        let primitive_error_store = ErrorEventStore::new();

        let primitive_status_store = PrimitiveStatusStore::new();

        // Register event tx for unique tag.
        PduHandleManager::register_cop(
            self.api.unique_tag,
            unique_tag,
            Some(pdu_event_tx.downgrade()),
            None,
        );

        // This thread is necessary for receiving events from the D-PDU API via the built-in
        // event callback mechanism, in order to generate the high-level events that
        // are actually required when using PduPrimitive.
        spawn({
            let dead_flag = primitive_dead_flag.clone();
            let error_store = primitive_error_store.clone();
            let status_store = primitive_status_store.clone();
            let primitive_event_tx = primitive_event_tx.clone();
            move || {
                PduPrimitive::blocking_listen_events(
                    pdu_event_rx,
                    primitive_event_tx,
                    error_store,
                    status_store,
                    dead_flag,
                )
            }
        });

        let h_mod = self.get_module_handle();
        let h_cll = self.get_cll_handle();

        let cop = Arc::new_cyclic(|weak| PduPrimitive {
            me: weak.clone(),
            api: self.api.clone(),
            worker: self.worker.clone(),
            unique_tag,
            h_mod,
            h_cll,
            h_cop: OnceLock::new(),
            primitive_type,
            pdu_event_tx: NonClonable(pdu_event_tx),
            error_store: primitive_error_store,
            primitive_event_tx,
            primitive_event_rx: Mutex::new(Some(primitive_event_rx)),
            status_store: primitive_status_store,
            dead_flag: primitive_dead_flag,
            pdu_sync: Mutex::default(),
        });

        // Register cop reference for unique tag.
        PduHandleManager::register_cop(
            self.api.unique_tag,
            unique_tag,
            None,
            Some(Arc::downgrade(&cop)),
        );

        cop
    }

    pub fn create_start_comm_primitive(&self, builder: StartComm) -> Arc<PduPrimitive> {
        let data = Bytes::copy_from_slice(&builder.data);
        let params = builder.build();

        self.create_primitive(
            PrimitiveType::StartComm { data, params },
            builder.events_queue_size,
        )
    }

    pub fn create_stop_comm_primitive(&self, builder: StopComm) -> Arc<PduPrimitive> {
        let data = Bytes::copy_from_slice(&builder.data);
        let params = builder.build();

        self.create_primitive(
            PrimitiveType::StopComm { data, params },
            builder.events_queue_size,
        )
    }

    pub fn create_send_recv_primitive(&self, builder: SendRecv) -> Arc<PduPrimitive> {
        let data = Bytes::copy_from_slice(&builder.data);
        let params = builder.build();

        self.create_primitive(
            PrimitiveType::SendRecv { data, params },
            builder.events_queue_size,
        )
    }

    pub fn create_update_param_primitive(&self) -> Arc<PduPrimitive> {
        self.create_primitive(PrimitiveType::UpdateParam, None)
    }

    pub fn create_restore_param_primitive(&self) -> Arc<PduPrimitive> {
        self.create_primitive(PrimitiveType::RestoreParam, None)
    }

    pub fn blocking_set_com_params(&self, set_target: impl Into<SetTarget>) -> GeneralResult<()> {
        let h_mod = self.get_module_handle();
        let h_cll = self.get_cll_handle();

        match set_target.into() {
            SetTarget::Definitions(v) => {
                for def in v.iter() {
                    let cp = match def.blocking_build(&self.api) {
                        Ok(v) => v,
                        Err(GeneralError::ApiError(ApiError::PduError(
                            PduError::ComParamNotSupported,
                        ))) => {
                            continue;
                        }
                        Err(err) => {
                            return Err(err)?;
                        }
                    };
                    match self.api.pdu_set_com_param(h_mod, h_cll, &cp) {
                        Ok(()) => {}
                        Err(ApiError::PduError(PduError::ComParamNotSupported)) => {
                            continue;
                        }
                        Err(err) => Err(err)?,
                    }
                }
            }
            SetTarget::ComParams(v) => {
                for cp in v.iter() {
                    match self.api.pdu_set_com_param(h_mod, h_cll, cp) {
                        Ok(()) => {}
                        Err(ApiError::PduError(PduError::ComParamNotSupported)) => {}
                        Err(err) => Err(err)?,
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn set_com_params(&self, set_target: impl Into<SetTarget>) -> GeneralResult<()> {
        let h_mod = self.get_module_handle();
        let h_cll = self.get_cll_handle();

        match self.worker.get() {
            Some(worker) => match set_target.into() {
                SetTarget::Definitions(v) => {
                    for def in v.iter() {
                        let cp = match def.build(worker.as_ref()).await {
                            Ok(v) => v,
                            Err(GeneralError::ApiError(ApiError::PduError(
                                PduError::ComParamNotSupported,
                            ))) => {
                                continue;
                            }
                            Err(err) => Err(err)?,
                        };

                        match worker.pdu_set_com_param(h_mod, h_cll, cp).await {
                            Ok(()) => {}
                            Err(GeneralError::ApiError(ApiError::PduError(
                                PduError::ComParamNotSupported,
                            ))) => {
                                continue;
                            }
                            Err(err) => Err(err)?,
                        }
                    }
                }
                SetTarget::ComParams(v) => {
                    for cp in v.iter() {
                        match worker.pdu_set_com_param(h_mod, h_cll, cp.clone()).await {
                            Ok(()) => {}
                            Err(GeneralError::ApiError(ApiError::PduError(
                                PduError::ComParamNotSupported,
                            ))) => {}
                            Err(err) => Err(err)?,
                        }
                    }
                }
            },
            None => {
                let me = self.take_me_expect();

                let set_target = set_target.into();
                let thread = move || me.blocking_set_com_params(set_target);

                spawn_blocking(thread).await.expect(
                    "internal error: PduLogicalLink::blocking_set_com_params() task panicked",
                )?;
            }
        }

        Ok(())
    }

    pub fn blocking_set_unique_com_params_table(
        &self,
        map_target: impl Into<MapTarget>,
    ) -> GeneralResult<()> {
        let h_mod = self.get_module_handle();
        let h_cll = self.get_cll_handle();

        match map_target.into() {
            MapTarget::Definitions(v) => {
                let mut table = PduComParamTable::with_capacity(v.len());

                for (unique_id, set) in v.iter() {
                    for def in set.iter() {
                        let cp = def.blocking_build(&self.api)?;
                        table.add(unique_id.to_owned(), cp);
                    }
                }

                self.api
                    .pdu_set_unique_resp_id_table(h_mod, h_cll, &table)?;
            }
            MapTarget::ComParams(v) => {
                self.api.pdu_set_unique_resp_id_table(h_mod, h_cll, &v)?;
            }
        }

        Ok(())
    }

    pub async fn set_unique_com_params_table(
        &self,
        map_target: impl Into<MapTarget>,
    ) -> GeneralResult<()> {
        let h_mod = self.get_module_handle();
        let h_cll = self.get_cll_handle();

        match self.worker.get() {
            Some(worker) => match map_target.into() {
                MapTarget::Definitions(v) => {
                    let mut table = PduComParamTable::with_capacity(v.len());

                    for (unique_id, set) in v.iter() {
                        for def in set.iter() {
                            let cp = def.build(worker.as_ref()).await?;
                            table.add(unique_id.to_owned(), cp);
                        }
                    }

                    worker
                        .pdu_set_unique_resp_id_table(h_mod, h_cll, table)
                        .await?;
                }
                MapTarget::ComParams(v) => {
                    worker.pdu_set_unique_resp_id_table(h_mod, h_cll, v).await?;
                }
            },
            None => {
                let me = self.take_me_expect();

                let map_target = map_target.into();
                let thread = move || me.blocking_set_unique_com_params_table(map_target);

                spawn_blocking(thread)
                    .await
                    .expect("internal error: PduLogicalLink::blocking_set_unique_com_params_table() task panicked")?;
            }
        }

        Ok(())
    }

    pub(crate) fn blocking_listen_events(
        mut pdu_event_rx: mpsc::UnboundedReceiver<PduEvent>,
        mut logical_link_event_tx: broadcast::Sender<()>,
    ) {
        loop {
            let event = match pdu_event_rx.blocking_recv() {
                Some(value) => value,
                None => {
                    // The channel will be closed when `drop()` is called for the `PduLogicalLink`.
                    break;
                }
            };

            if Self::handle_event(event, &mut logical_link_event_tx) {
                break;
            }
        }
    }

    pub(crate) async fn listen_events(
        mut pdu_event_rx: mpsc::UnboundedReceiver<PduEvent>,
        mut logical_link_event_tx: broadcast::Sender<()>,
    ) {
        loop {
            let event = match pdu_event_rx.recv().await {
                Some(value) => value,
                None => {
                    // The channel will be closed when `drop()` is called for the `PduLogicalLink`.
                    break;
                }
            };

            if Self::handle_event(event, &mut logical_link_event_tx) {
                break;
            }
        }
    }

    pub(crate) fn handle_event(
        _event: PduEvent,
        _module_event_tx: &mut broadcast::Sender<()>,
    ) -> StopReceive {
        false
    }
}

impl Drop for PduLogicalLink {
    fn drop(&mut self) {
        debug!(
            h_mod = self.get_module_handle(),
            h_cll = self.get_cll_handle(),
            "Disconnecting the PduLogicalLink via destructor..."
        );

        match self.worker.get() {
            Some(worker) => {
                let query = Query::VtCllDestructor(self.get_module_handle(), self.get_cll_handle());
                match worker.request(query, None) {
                    Ok(_) => {}
                    Err(err) => {
                        error!(
                            h_mod = self.get_module_handle(),
                            h_cll = self.get_cll_handle(),
                            "Error when disconnecting the PduComLogicalLink via destructor: {err}"
                        );
                    }
                }
            }
            None => {
                let api = self.api.clone();
                let h_mod = self.get_module_handle();
                let h_cll = self.get_cll_handle();
                spawn(move || api.vt_cll_destructor(h_mod, h_cll));
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CllStatus(PduStatusData);

impl Deref for CllStatus {
    type Target = PduStatusData;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CllStatus {
    pub fn is_offline(&self) -> bool {
        matches!(self.0.status_code, PduStatus::CllstOffline)
    }

    pub fn is_online(&self) -> bool {
        matches!(self.0.status_code, PduStatus::CllstOnline)
    }

    pub fn is_communication_started(&self) -> bool {
        matches!(self.0.status_code, PduStatus::CllstCommStarted)
    }
}

#[derive(Debug, Clone)]
pub struct PduCllData {
    pub(crate) h_mod: PduModuleHandle,

    pub(crate) h_cll: PduCllHandle,

    pub(crate) create_type: CllCreateType,

    pub(crate) create_flags: CllCreateFlags,
}

impl PduCllData {
    pub fn get_module_handle(&self) -> PduModuleHandle {
        self.h_mod.clone()
    }

    pub fn get_cll_handle(&self) -> PduCllHandle {
        self.h_cll.clone()
    }

    pub fn get_create_type(&self) -> &CllCreateType {
        &self.create_type
    }

    pub fn get_create_flags(&self) -> &CllCreateFlags {
        &self.create_flags
    }
}

#[derive(Debug, Clone)]
pub enum CllCreateType {
    /// ComLogicalLink will be created by resource ID.
    ///
    /// Not recommended.
    ResourceId(PduObjectId),

    /// ComLogicalLink will be created by
    ///  - bus type ID
    ///  - protocol ID
    ///  - information about the pins on VCI (can't be empty).
    ///
    /// Recommended.
    ResourceData {
        bus: BusSource,
        protocol: ProtocolSource,
        pins: Vec<TargetPin>,
    },
}

impl CllCreateType {
    pub fn raw_dw_can_on_obd() -> Self {
        CllCreateType::ResourceData {
            bus: BusSource::dual_wire_can(),
            protocol: ProtocolSource::iso_11898_raw(),
            pins: TargetPin::obd_dual_wire_can(),
        }
    }

    pub fn uds_on_iso_tp_on_dw_can() -> Self {
        CllCreateType::ResourceData {
            bus: BusSource::dual_wire_can(),
            protocol: ProtocolSource::uds_on_iso_tp(),
            pins: TargetPin::obd_dual_wire_can(),
        }
    }

    pub fn kwp_on_iso_tp_on_dw_can() -> Self {
        CllCreateType::ResourceData {
            bus: BusSource::dual_wire_can(),
            protocol: ProtocolSource::kwp_on_iso_tp(),
            pins: TargetPin::obd_dual_wire_can(),
        }
    }
}

/// Flags used by [`PduApi`] when creating a Communication Logical Link.
///
/// Corresponds to ISO 22900-2, section D.2.3, table D.6.
///
/// [`PduApi`]: crate::api::PduApi
#[derive(Debug, Clone)]
pub struct CllCreateFlags {
    /// Byte 0, bit 7.
    ///
    /// Enables transparent transmission and reception of protocol messages
    /// without removing protocol-specific bytes.
    ///
    /// The exact behavior depends on the selected communication protocol.
    ///
    /// `false`:
    /// - The D-PDU API removes protocol header bytes and checksums before
    ///   returning received data.
    ///
    /// - Additional header and footer information can be requested using
    ///   the `TxFlag::ENABLE_EXTRA_INFO` flag.
    ///
    /// `true`:
    /// - Protocol header bytes and checksum bytes are preserved in the
    ///   returned result data.
    pub raw_mode: bool,

    /// Byte 0, bit 6.
    ///
    /// Enables checksum generation by the D-PDU API for transmitted messages.
    ///
    /// This flag is ignored when [`raw_mode`] is `false`.
    pub checksum_mode: bool,

    /// Byte 3, bit 0.
    ///
    /// Softing D-PDU-API specific extension.
    ///
    /// This flag is not defined by ISO 22900-2.
    ///
    /// When enabled, `PDUCreateComLogicalLink` creates a monitor logical
    /// link instead of a regular communication logical link.
    ///
    /// Supported only for:
    /// - `ISO_11898_RAW`
    /// - `ISO_14230_3_on_ISO_14230_2`
    ///
    /// This flag is effective only when [`raw_mode`] is `false`.
    pub monitor_mode: bool,
}

impl Display for CllCreateFlags {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "raw_mode={:?}, checksum_mode={:?}, monitor_mode={:?}",
            self.raw_mode, self.checksum_mode, self.monitor_mode
        )
    }
}

impl CllCreateFlags {
    pub fn raw() -> Self {
        Self {
            raw_mode: true,
            monitor_mode: false,
            checksum_mode: false,
        }
    }

    pub fn raw_with_monitor() -> Self {
        Self {
            raw_mode: true,
            monitor_mode: true,
            checksum_mode: false,
        }
    }

    pub fn recommended() -> Self {
        Self {
            raw_mode: false,
            checksum_mode: true,
            monitor_mode: false,
        }
    }

    pub(crate) fn zb(&self) -> u8 {
        let mut b = 0;

        // Chapter D.2.3.

        // byte pos 0, bit pos 7: RawMode.
        if self.raw_mode {
            b |= 0x80; // 0 - OFF; 1 - ON
        }

        // bye pos 0, bit pos 6: ChecksumMode
        if self.checksum_mode {
            b |= 0x40; // 0 - OFF; 1 - ON
        }

        b
    }

    pub(crate) fn tb(&self) -> u8 {
        let mut b = 0;

        if self.monitor_mode {
            b |= 0x01;
        }

        b
    }

    /// Рассчитывает байтовый массив с учётом используемых режимов.
    pub(crate) fn get_pdu_flag_data(&self) -> [u8; 4] {
        [self.zb(), 0, 0, self.tb()]
    }
}

mod sealed {
    pub trait Sealed {}
}

trait CopParamsBuilder: sealed::Sealed {
    fn build(&self) -> PrimitiveParams;
}

#[derive(Debug, Default)]
pub struct StartComm {
    pub data: Vec<u8>,

    pub tx_flags: TransmitFlags,

    pub receive_cycles: ReceiveCycles,

    pub param_buffer: ComParamBuffer,

    pub filters: Vec<ExpectedResponse>,

    pub events_queue_size: Option<usize>,
}

impl StartComm {
    /// Use case.
    ///
    /// В основном для работы с CAN. Для первоначального Tester Present.
    pub fn initial() -> Self {
        StartComm::default()
    }

    pub fn with_events_queue_size(mut self, size: usize) -> Self {
        self.events_queue_size = Some(size);
        self
    }

    pub fn with_data(mut self, data: &[u8]) -> Self {
        self.data = data.to_vec();
        self
    }

    pub fn with_tx_flags(mut self, flags: TransmitFlags) -> Self {
        self.tx_flags = flags;
        self
    }

    pub fn with_receive_cycles(mut self, cycles: ReceiveCycles) -> Self {
        self.receive_cycles = cycles;
        self
    }

    pub fn with_param_buffer(mut self, param_buffer: ComParamBuffer) -> Self {
        self.param_buffer = param_buffer;
        self
    }

    pub fn with_filters(mut self, vec: Vec<ExpectedResponse>) -> Self {
        self.filters = vec;
        self
    }
}

impl sealed::Sealed for StartComm {}
impl CopParamsBuilder for StartComm {
    fn build(&self) -> PrimitiveParams {
        let mut params = PrimitiveParams::default();

        params.send_cycles = SendCycles::Normal(if self.data.len() > 0 { 1 } else { 0 });
        params.receive_cycles = self.receive_cycles.clone();
        params.temp_param_update = self.param_buffer;
        params.tx_flag = self.tx_flags.clone();
        params.expected_responses = self.filters.clone();

        params
    }
}

#[derive(Debug, Default)]
pub struct StopComm {
    pub data: Vec<u8>,

    pub tx_flags: TransmitFlags,

    pub receive: bool,

    pub param_buffer: ComParamBuffer,

    pub filters: Vec<ExpectedResponse>,

    pub events_queue_size: Option<usize>,
}

impl StopComm {
    /// Use case.
    pub fn now() -> Self {
        StopComm::default()
    }

    /// Use case.
    pub fn now_with_send(data: &[u8]) -> Self {
        Self::now().with_data(data)
    }

    pub fn with_events_queue_size(mut self, size: usize) -> Self {
        self.events_queue_size = Some(size);
        self
    }

    pub fn with_data(mut self, data: &[u8]) -> Self {
        self.data = data.to_vec();
        self
    }

    pub fn with_tx_flags(mut self, flags: TransmitFlags) -> Self {
        self.tx_flags = flags;
        self
    }

    pub fn with_receive(mut self, status: bool) -> Self {
        self.receive = status;
        self
    }

    pub fn with_param_buffer(mut self, param_buffer: ComParamBuffer) -> Self {
        self.param_buffer = param_buffer;
        self
    }

    pub fn with_filters(mut self, vec: Vec<ExpectedResponse>) -> Self {
        self.filters = vec;
        self
    }
}

impl sealed::Sealed for StopComm {}
impl CopParamsBuilder for StopComm {
    fn build(&self) -> PrimitiveParams {
        let mut params = PrimitiveParams::default();

        params.send_cycles = SendCycles::Normal(if self.data.len() > 0 { 1 } else { 0 });
        params.receive_cycles = ReceiveCycles::Normal(if self.receive { 1 } else { 0 });
        params.temp_param_update = self.param_buffer;
        params.tx_flag = self.tx_flags.clone();
        params.expected_responses = self.filters.clone();

        params
    }
}

#[derive(Debug)]
pub struct SendRecv {
    pub data: Vec<u8>,

    pub tx_flags: TransmitFlags,

    pub send_cycles: SendCycles,

    pub receive_cycles: ReceiveCycles,

    pub param_buffer: ComParamBuffer,

    pub filters: Vec<ExpectedResponse>,

    pub delay: Duration,

    pub events_queue_size: Option<usize>,
}

impl SendRecv {
    pub fn new(data: Option<&[u8]>) -> Self {
        SendRecv {
            data: data.map(|v| v.to_vec()).unwrap_or_default(),
            tx_flags: TransmitFlags::default(),
            send_cycles: SendCycles::Normal(1),
            receive_cycles: ReceiveCycles::Normal(1),
            param_buffer: ComParamBuffer::default(),
            filters: vec![ExpectedResponse {
                response_type: ResponseType::Positive,
                acceptance_id: 1,
                mask_data: MaskData::empty(),
                unique_response_ids: vec![],
            }],
            delay: Duration::from_millis(0),
            events_queue_size: None,
        }
    }

    /// Use case.
    pub fn send_only(data: &[u8]) -> Self {
        SendRecv::new(Some(data)).with_no_receive()
    }

    pub fn with_no_receive(self) -> Self {
        self.with_receive_cycles(ReceiveCycles::Normal(0))
    }

    pub fn with_no_send(self) -> Self {
        self.with_send_cycles(SendCycles::Normal(0))
    }

    pub fn with_data(mut self, data: &[u8]) -> Self {
        self.data = data.to_vec();
        self
    }

    pub fn add_data(mut self, data: &[u8]) -> Self {
        self.data.extend_from_slice(data);
        self
    }

    pub fn with_events_queue_size(mut self, size: usize) -> Self {
        self.events_queue_size = Some(size);
        self
    }

    pub fn with_tx_flags(mut self, flags: TransmitFlags) -> Self {
        self.tx_flags = flags;
        self
    }

    pub fn with_tx_flags_mut<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut TransmitFlags),
    {
        let flags = &mut self.tx_flags;
        f(flags);
        self
    }

    pub fn with_send_cycles(mut self, cycles: SendCycles) -> Self {
        self.send_cycles = cycles;
        self
    }

    pub fn with_receive_cycles(mut self, cycles: ReceiveCycles) -> Self {
        //if cycles.to_i32() == 0 {
        //    panic!("internal error: when PduCopt = SendRecv, receive cycles must not be zero");
        //}
        self.receive_cycles = cycles;
        self
    }

    pub fn with_param_buffer(mut self, param_buffer: ComParamBuffer) -> Self {
        self.param_buffer = param_buffer;
        self
    }

    pub fn with_filters(mut self, vec: Vec<ExpectedResponse>) -> Self {
        self.filters = vec;
        self
    }

    pub fn with_delay(mut self, duration: Duration) -> Self {
        self.delay = duration;
        self
    }
}

impl RawCanPrimitiveBuilderExt for SendRecv {
    fn monitor() -> Self {
        Self::new(None)
            .with_no_send()
            .with_receive_cycles(ReceiveCycles::Infinite)
            .with_filters(vec![ExpectedResponse {
                response_type: ResponseType::Positive,
                acceptance_id: 1,
                mask_data: MaskData::empty(),
                unique_response_ids: vec![],
            }])
    }

    fn send_only_raw_can(frame: impl CanFrame) -> Self {
        let mut data = frame.data().to_vec();

        data.splice(0..0, frame.id().as_raw_unchecked().to_be_bytes());

        Self::new(Some(&data))
            .with_no_receive()
            .with_tx_flags_mut(|flags| {
                flags.can_29_bit = frame.is_extended();
            })
    }

    fn send_recv_raw_can(frame: impl CanFrame) -> Self {
        Self::send_only_raw_can(frame)
            .with_send_cycles(SendCycles::Normal(1))
            .with_receive_cycles(ReceiveCycles::Normal(1))
            .with_filters(vec![ExpectedResponse {
                response_type: ResponseType::Positive,
                acceptance_id: 1,
                mask_data: MaskData::empty(),
                unique_response_ids: vec![],
            }])
    }
}

impl sealed::Sealed for SendRecv {}
impl CopParamsBuilder for SendRecv {
    fn build(&self) -> PrimitiveParams {
        let mut params = PrimitiveParams::default();

        params.send_cycles = self.send_cycles.clone();
        params.receive_cycles = self.receive_cycles.clone();
        params.temp_param_update = self.param_buffer;
        params.tx_flag = self.tx_flags.clone();
        params.expected_responses = self.filters.clone();
        params.time = self.delay.as_millis() as _;

        params
    }
}
