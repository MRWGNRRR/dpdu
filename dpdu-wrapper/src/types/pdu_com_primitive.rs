use crate::api::PduApi;
use crate::error::{GeneralError, GeneralResult};
use crate::types::pdu_event::{
    ErrorEventStore, PduErrorEvent, PduEvent, PduEventData, PduResultEvent, StopReceive,
};
use crate::types::pdu_status::{PduStatusData, PduStatusTarget};
use crate::types::{PduCllHandle, PduCopHandle, PduModuleHandle, PduUniqueCopTag};
use crate::utils::NonClonable;
use crate::worker::{PduAsyncWorker, Query};
use bytes::Bytes;
use dpdu_api_types::{PduCopt, PduErrorEvt, PduStatus};
use parking_lot::Mutex;
use std::sync::{Arc, Once, OnceLock, Weak};
use std::thread::spawn;
use tokio::sync::{broadcast, mpsc};
use tokio::task::spawn_blocking;
use tracing::{debug, error};

pub type PrimitiveResult<T> = std::result::Result<T, PrimitiveError>;

/// Errors that can occur during primitive execution.
///
/// These errors are divided into:
///
/// - communication errors reported by the underlying D-PDU API;
/// - internal errors caused by invalid primitive state or library logic.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PrimitiveError {
    /// Communication error reported by the D-PDU API.
    #[error("{}", .0.code.as_str())]
    CommunicationError(PduErrorEvent),

    /// The primitive has reached the end of its lifetime and is no longer
    /// available through the D-PDU API.
    #[error("primitive is destroyed")]
    DestroyedError,

    /// The primitive has been created but has not been started yet.
    #[error("primitive has not been started yet")]
    NotStartedError,

    /// The primitive has already been started and cannot be started again.
    #[error("primitive has already been started")]
    AlreadyStartedError,
}

/// High-level wrapper around a D-PDU communication primitive.
///
/// Manages the primitive lifetime, event handling, and interaction with
/// the underlying D-PDU API.
#[derive(Debug)]
pub struct PduPrimitive {
    /// Weak self-reference used to obtain an [`Arc`] to this primitive without
    /// creating a reference cycle.
    pub(crate) me: Weak<Self>,

    /// D-PDU API wrapper through which this primitive was created.
    pub(crate) api: Arc<PduApi>,

    /// Asynchronous worker through which this primitive was created.
    pub(crate) worker: OnceLock<Arc<PduAsyncWorker>>,

    /// Unique tag used by the D-PDU API to identify this primitive.
    pub(crate) unique_tag: PduUniqueCopTag,

    /// D-PDU API handle of the parent module.
    pub(crate) h_mod: PduModuleHandle,

    /// D-PDU API handle of the parent communication logical link.
    pub(crate) h_cll: PduCllHandle,

    /// Contains the primitive type and its handle.
    ///
    /// See the [`PduCopData`] structure.
    pub(crate) h_cop: OnceLock<PduCopHandle>,

    /// A way to create communication primitives.
    pub(crate) primitive_type: PrimitiveType,

    /// Event channel sender owned by [`PduPrimitive`].
    ///
    /// The sender is intentionally dropped when [`PduPrimitive`] is dropped,
    /// allowing the event listener to detect shutdown.
    ///
    /// A weak reference is stored in [`PduHandleManager`] to keep track of the
    /// channel lifetime and to automatically stop event listening in
    /// [`PduPrimitive::listen_events`] and
    /// [`PduPrimitive::blocking_listen_events`].
    ///
    /// # Safety
    ///
    /// This sender must not be cloned. Cloning it would extend the channel
    /// lifetime beyond [`PduPrimitive`] and prevent listeners from being
    /// automatically stopped.
    ///
    /// [`PduHandleManager`]: crate::handle_manager::PduHandleManager
    pub(crate) pdu_event_tx: NonClonable<mpsc::UnboundedSender<PduEvent>>,

    /// A primitive error returned by the D-PDU API.
    pub(crate) error_store: Arc<ErrorEventStore>,

    /// Sender used to create additional receivers after the initial receiver
    /// has been taken.
    pub(crate) primitive_event_tx: broadcast::Sender<PrimitiveEvent>,

    /// The initial receiver returned by the first [`get_primitive_event_receiver`] call.
    ///
    /// After the receiver is taken, subsequent calls create new receivers from
    /// [`primitive_event_tx`].
    pub(crate) primitive_event_rx: Mutex<Option<broadcast::Receiver<PrimitiveEvent>>>,

    /// Shared storage containing the current high-level lifecycle status of the primitive.
    ///
    /// The status is updated internally during primitive execution and can be read
    /// without performing a D-PDU API request.
    pub(crate) status_store: Arc<PrimitiveStatusStore>,

    /// One-time notification triggered when this primitive reaches the end of its lifetime.
    pub(crate) dead_flag: Arc<Once>,

    /// Ensures that D-PDU API calls are executed sequentially.
    pub(crate) pdu_sync: Mutex<()>,
}

impl PartialEq for PduPrimitive {
    fn eq(&self, other: &Self) -> bool {
        self.api.unique_tag == other.api.unique_tag && self.unique_tag == other.unique_tag
    }
}

impl PduPrimitive {
    /// Returns the D-PDU module handle associated with this primitive.
    pub fn get_module_handle(&self) -> PduModuleHandle {
        self.h_mod
    }

    /// Returns the D-PDU communication logical link handle associated with this primitive.
    pub fn get_cll_handle(&self) -> PduCllHandle {
        self.h_cll
    }

    /// Returns the D-PDU communication primitive handle.
    pub fn get_cop_handle(&self) -> PrimitiveResult<PduCopHandle> {
        self.assert_started()?;
        Ok(self
            .h_cop
            .get()
            .expect("internal error: primitive was reported as started but has no primitive handle")
            .to_owned())
    }

    /// Returns the parameters used to create this primitive, if available.
    ///
    /// The parameters are not present when the primitive was created without
    /// explicitly provided configuration.
    pub fn get_primitive_type(&self) -> &PrimitiveType {
        &self.primitive_type
    }

    /// Returns the unique tag assigned to this communication primitive by the D-PDU API.
    pub fn get_unique_tag(&self) -> PduUniqueCopTag {
        self.unique_tag
    }

    /// Returns an `Arc` reference to this primitive.
    ///
    /// This method upgrades the internal weak self-reference, allowing the
    /// primitive to keep itself alive during asynchronous or blocking operations.
    fn clone_arc(&self) -> Arc<PduPrimitive> {
        self.me
            .upgrade()
            .expect("internal error: PduPrimitive self-reference is no longer valid") // infallible
    }

    /// Retrieves the current execution status of the D-PDU communication primitive.
    ///
    /// This method performs a synchronous D-PDU API call and blocks the current
    /// thread until the status is received.
    ///
    /// # Errors
    ///
    /// Returns an error if the primitive has already reached its terminal state
    /// and has been destroyed, or if the underlying D-PDU API call fails.
    pub fn blocking_get_pdu_status(&self) -> GeneralResult<CopStatus> {
        let _sync_guard = self.pdu_sync.lock();

        let target = PduStatusTarget::Primitive(
            self.get_module_handle(),
            self.get_cll_handle(),
            self.get_cop_handle()?,
        );

        Ok(CopStatus(self.api.pdu_get_status(&target)?))
    }

    /// Retrieves the current execution status of the D-PDU communication primitive.
    ///
    /// Uses the asynchronous D-PDU worker when available. If no worker is
    /// configured, the call is executed on a blocking thread to avoid blocking
    /// the async executor.
    ///
    /// # Errors
    ///
    /// Returns an error if the primitive has already reached its terminal state
    /// and has been destroyed, or if the underlying D-PDU API call fails.
    pub async fn get_pdu_status(&self) -> GeneralResult<CopStatus> {
        let h_mod = self.get_module_handle();
        let h_cll = self.get_cll_handle();
        let h_cop = self.get_cop_handle()?;

        match self.worker.get() {
            Some(worker) => {
                let target = PduStatusTarget::Primitive(h_mod, h_cll, h_cop);
                let result = worker.pdu_get_status(target).await?;
                Ok(CopStatus(result))
            }
            None => {
                let me = self.clone_arc();
                let result = spawn_blocking(move || me.blocking_get_pdu_status())
                    .await
                    .expect("internal error: PduPrimitive::blocking_get_status() task panicked")?;

                Ok(result)
            }
        }
    }

    /// Returns the current high-level lifecycle status of the primitive.
    ///
    /// This status is maintained internally and represents the last known state
    /// of the primitive lifecycle. It does not perform a D-PDU API request.
    pub fn get_status(&self) -> PrimitiveStatus {
        self.status_store.get()
    }

    /// Returns the error event produced by the primitive, if any.
    ///
    /// The returned value is a snapshot of the stored error event.
    pub fn get_error(&self) -> Option<PduErrorEvent> {
        self.error_store.get().cloned()
    }

    /// Starts execution of the D-PDU communication primitive.
    ///
    /// This method initializes the primitive execution in the D-PDU API and
    /// transitions the primitive from the not-started state into active execution.
    ///
    /// # Errors
    ///
    /// Returns an error if the primitive has already been started.
    pub fn blocking_start(&self) -> GeneralResult<()> {
        self.assert_not_started()?;

        let _sync_guard = self.pdu_sync.lock();

        let h_cop = match self.api.pdu_start_com_primitive(
            self.h_mod,
            self.h_cll,
            &self.primitive_type,
            Some(self.unique_tag),
        ) {
            Ok(v) => v,
            Err(err) => {
                let _ = self.primitive_event_tx.send(PrimitiveEvent::StartFailed(
                    GeneralError::ApiError(err.clone()),
                ));
                return Err(err)?;
            }
        };

        self
            .h_cop
            .set(h_cop)
            .expect("internal error: primitive was reported as not started but already has primitive handle");

        Ok(())
    }

    /// Asynchronously starts execution of the D-PDU communication primitive.
    ///
    /// Uses the asynchronous D-PDU worker when available. Otherwise, the blocking
    /// start operation is executed in a blocking context.
    ///
    /// This method transitions the primitive from the not-started state into active
    /// execution.
    ///
    /// # Errors
    ///
    /// Returns an error if the primitive has already been started.
    pub async fn start(&self) -> GeneralResult<()> {
        self.assert_not_started()?;

        match self.worker.get() {
            Some(worker) => {
                let h_cop = match worker
                    .pdu_start_com_primitive(
                        self.h_mod,
                        self.h_cll,
                        self.primitive_type.clone(),
                        Some(self.unique_tag),
                    )
                    .await
                {
                    Ok(v) => v,
                    Err(err) => {
                        let _ = self
                            .primitive_event_tx
                            .send(PrimitiveEvent::StartFailed(err.clone()));

                        return Err(err);
                    }
                };

                self
                    .h_cop
                    .set(h_cop)
                    .expect("internal error: primitive was reported as not started but already has primitive handle");

                Ok(())
            }
            None => {
                let me = self.clone_arc();

                spawn_blocking(move || me.blocking_start())
                    .await
                    .expect("internal error: PduPrimitive::blocking_start() task panicked")?;

                Ok(())
            }
        }
    }

    /// Waits for the primitive result and blocks the current thread.
    ///
    /// Returns the received [`PduResultEvent`] or an error if the primitive fails
    /// or finishes without producing a result.
    pub fn blocking_get_result(&self) -> PrimitiveResult<PduResultEvent> {
        self.assert_started()?;

        let mut receiver = self.get_primitive_event_receiver()?;

        while let Ok(event) = receiver.blocking_recv() {
            match event {
                PrimitiveEvent::Status(status) => {
                    if !status.is_alive() {
                        break;
                    }
                }
                PrimitiveEvent::Result(result) => {
                    return Ok(result);
                }
                PrimitiveEvent::Error(error) => {
                    return Err(PrimitiveError::CommunicationError(error));
                }
                _ => {}
            }
        }

        Err(PrimitiveError::CommunicationError(PduErrorEvent {
            code: PduErrorEvt::RxTimeout,
            extra_code: 0,
        }))?
    }

    /// Asynchronously waits for the primitive result.
    ///
    /// Returns the received [`PduResultEvent`] or an error if the primitive fails
    /// or finishes without producing a result.
    pub async fn get_result(&self) -> PrimitiveResult<PduResultEvent> {
        self.assert_started()?;

        let mut receiver = self.get_primitive_event_receiver()?;

        while let Ok(event) = receiver.recv().await {
            match event {
                PrimitiveEvent::Status(status) => {
                    if !status.is_alive() {
                        break;
                    }
                }
                PrimitiveEvent::Result(result) => {
                    return Ok(result);
                }
                PrimitiveEvent::Error(error) => {
                    return Err(PrimitiveError::CommunicationError(error));
                }
                _ => {}
            }
        }

        Err(PrimitiveError::CommunicationError(PduErrorEvent {
            code: PduErrorEvt::RxTimeout,
            extra_code: 0,
        }))?
    }

    /// Blocks the current thread until the primitive execution is finished.
    ///
    /// This method waits for primitive events and returns when the primitive
    /// reaches a terminal state ([`PrimitiveStatus::is_alive()`] returns `false`).
    ///
    /// If an error event is received during execution, the method returns
    /// [`PrimitiveError::CommunicationError`].
    pub fn blocking_wait_finish(&self) -> PrimitiveResult<()> {
        self.assert_started()?;

        let mut receiver = self.get_primitive_event_receiver()?;
        while let Ok(event) = receiver.blocking_recv() {
            match event {
                PrimitiveEvent::Status(status) => {
                    if !status.is_alive() {
                        break;
                    }
                }
                PrimitiveEvent::Error(error) => {
                    return Err(PrimitiveError::CommunicationError(error));
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Asynchronously waits until the primitive execution is finished.
    ///
    /// This method waits for primitive events and returns when the primitive
    /// reaches a terminal state ([`PrimitiveStatus::is_alive()`] returns `false`).
    ///
    /// If an error event is received during execution, the method returns
    /// [`PrimitiveError::CommunicationError`].
    pub async fn wait_finish(&self) -> PrimitiveResult<()> {
        self.assert_started()?;

        let mut receiver = self.get_primitive_event_receiver()?;
        while let Ok(event) = receiver.recv().await {
            match event {
                PrimitiveEvent::Status(status) => {
                    if !status.is_alive() {
                        break;
                    }
                }
                PrimitiveEvent::Error(error) => {
                    return Err(PrimitiveError::CommunicationError(error));
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Cancels the execution of the D-PDU communication primitive.
    ///
    /// This method performs a synchronous D-PDU API call and blocks the current
    /// thread until the cancellation request is processed.
    ///
    /// The primitive may not enter the cancelled state immediately after this
    /// method returns. The final lifecycle state transition is reported separately
    /// through primitive status events.
    ///
    /// # Errors
    ///
    /// Returns an error if the cancellation request cannot be sent to the D-PDU API
    /// or if the primitive is no longer valid.
    pub fn blocking_cancel(&self) -> GeneralResult<()> {
        self.assert_started()?;
        self.assert_dead()?;

        let _sync_guard = self.pdu_sync.lock();
        let h_cop = self.get_cop_handle()?;

        self.api
            .pdu_cancel_com_primitive(self.h_mod, self.h_cll, h_cop)?;

        Ok(())
    }

    /// Cancels the execution of the D-PDU communication primitive.
    ///
    /// Uses the asynchronous D-PDU worker when available. If no worker is
    /// configured, the blocking cancellation call is executed on a dedicated
    /// blocking thread.
    ///
    /// The primitive may not enter the cancelled state immediately after this
    /// method returns. The final lifecycle state transition is reported separately
    /// through primitive status events.
    ///
    /// # Errors
    ///
    /// Returns an error if the cancellation request cannot be sent to the D-PDU API
    /// or if the primitive is no longer valid.
    pub async fn cancel(&self) -> GeneralResult<()> {
        self.assert_started()?;
        self.assert_dead()?;

        let h_mod = self.get_module_handle();
        let h_cll = self.get_cll_handle();
        let h_cop = self.get_cop_handle()?;

        match self.worker.get() {
            Some(worker) => {
                worker.pdu_cancel_com_primitive(h_mod, h_cll, h_cop).await?;
                Ok(())
            }
            None => {
                let me = self.clone_arc();

                spawn_blocking(move || me.blocking_cancel())
                    .await
                    .expect("internal error: PduPrimitive::blocking_cancel() task panicked")?;

                Ok(())
            }
        }
    }

    /// Returns whether the primitive has reached the end of its lifetime.
    ///
    /// A dead primitive can no longer be used for D-PDU operations.
    pub fn is_dead(&self) -> bool {
        self.dead_flag.is_completed()
    }

    /// Ensures that the primitive is still alive.
    ///
    /// Returns an error if the primitive has already been destroyed and cannot
    /// accept further operations.
    fn assert_dead(&self) -> PrimitiveResult<()> {
        if self.is_dead() {
            return Err(PrimitiveError::DestroyedError)?;
        }
        Ok(())
    }

    /// Returns an error if the primitive has encountered a communication error.
    ///
    /// This checks the stored error event reported by the D-PDU API and converts it
    /// into a high-level [`PrimitiveError`].
    fn assert_error(&self) -> PrimitiveResult<()> {
        if let Some(error) = self.get_error() {
            return Err(PrimitiveError::CommunicationError(error));
        }
        Ok(())
    }

    /// Ensures that the primitive has been initialized in the D-PDU API.
    ///
    /// Returns an error if the primitive has not been created and does not have
    /// an associated D-PDU ComPrimitive handle yet.
    fn assert_started(&self) -> PrimitiveResult<()> {
        if self.h_cop.get().is_none() {
            return Err(PrimitiveError::NotStartedError);
        }
        Ok(())
    }

    /// Ensures that the primitive has not been initialized in the D-PDU API yet.
    ///
    /// Returns an error if the primitive already owns D-PDU ComPrimitive data.
    fn assert_not_started(&self) -> PrimitiveResult<()> {
        if self.h_cop.get().is_some() {
            return Err(PrimitiveError::AlreadyStartedError);
        }
        Ok(())
    }

    /// Returns a receiver for primitive events.
    ///
    /// The first call returns the internal receiver, preserving events that were
    /// emitted before the receiver was requested. After the initial receiver is
    /// taken, subsequent calls create new subscribers from the internal sender and
    /// only receive events emitted after subscription.
    ///
    /// Returns an error if the primitive is in an invalid state or has failed.
    pub fn get_primitive_event_receiver(
        &self,
    ) -> PrimitiveResult<broadcast::Receiver<PrimitiveEvent>> {
        self.assert_error()?;
        self.assert_dead()?;

        let receiver = self
            .primitive_event_rx
            .lock()
            .take()
            .unwrap_or_else(|| self.primitive_event_tx.subscribe());

        Ok(receiver)
    }

    /// Processes low-level D-PDU events in a blocking context.
    ///
    /// This function receives events from the D-PDU event channel, updates the
    /// primitive state, stores terminal errors, and emits high-level primitive
    /// events for consumers.
    ///
    /// The event loop terminates when the event channel is closed or when the
    /// primitive reaches a terminal state.
    pub(crate) fn blocking_listen_events(
        mut pdu_event_rx: mpsc::UnboundedReceiver<PduEvent>,
        mut primitive_event_tx: broadcast::Sender<PrimitiveEvent>,
        error_event_store: Arc<ErrorEventStore>,
        primitive_status_store: Arc<PrimitiveStatusStore>,
        dead_flag: Arc<Once>,
    ) {
        loop {
            let event = match pdu_event_rx.blocking_recv() {
                Some(value) => value,
                None => {
                    // The channel will be closed when `drop()` is called for the `PduPrimitive`.
                    break;
                }
            };

            if Self::handle_event(
                event,
                &dead_flag,
                &mut primitive_event_tx,
                &error_event_store,
                &primitive_status_store,
            ) {
                break;
            }
        }
    }

    /// Processes low-level D-PDU events asynchronously.
    ///
    /// This function receives events from the D-PDU event channel, updates the
    /// primitive state, stores terminal errors, and emits high-level primitive
    /// events for consumers.
    ///
    /// The event loop terminates when the event channel is closed or when the
    /// primitive reaches a terminal state.
    pub(crate) async fn listen_events(
        mut pdu_event_rx: mpsc::UnboundedReceiver<PduEvent>,
        mut primitive_event_tx: broadcast::Sender<PrimitiveEvent>,
        error_event_store: Arc<ErrorEventStore>,
        primitive_status_store: Arc<PrimitiveStatusStore>,
        dead_flag: Arc<Once>,
    ) {
        loop {
            let event = match pdu_event_rx.recv().await {
                Some(value) => value,
                None => {
                    // The channel will be closed when `drop()` is called for the `PduPrimitive`.
                    break;
                }
            };

            if Self::handle_event(
                event,
                &dead_flag,
                &mut primitive_event_tx,
                &error_event_store,
                &primitive_status_store,
            ) {
                break;
            }
        }
    }

    /// Handles a single low-level D-PDU event.
    ///
    /// Converts D-PDU API events into high-level primitive events, updates the
    /// primitive lifecycle status, and stores error information when required.
    ///
    /// Returns `true` when event processing should stop, which happens after the
    /// primitive reaches a terminal state.
    pub(crate) fn handle_event(
        event: PduEvent,
        dead_flag: &Arc<Once>,
        primitive_event_tx: &mut broadcast::Sender<PrimitiveEvent>,
        error_event_store: &ErrorEventStore,
        primitive_status_store: &PrimitiveStatusStore,
    ) -> StopReceive {
        let send_event = |event: PrimitiveEvent| {
            // After the initial `primitive_event_rx` has been taken by
            // `get_primitive_event_receiver()`, all subsequent receivers are created
            // by subscribing to `primitive_event_tx`.
            //
            // From that point on, the channel lifetime depends entirely on the receivers
            // created by callers. Therefore, the absence of active listeners is expected
            // and a failed send is not treated as an error.
            let _ = primitive_event_tx.send(event);
        };

        let mut stop = false;

        match event.data {
            PduEventData::Status(status) => {
                let is_finished = matches!(status.0, PduStatus::CopstFinished);
                let is_cancelled = matches!(status.0, PduStatus::CopstCancelled);

                if is_finished || is_cancelled {
                    dead_flag.call_once(|| {});
                    stop = true;
                }

                let status = PrimitiveStatus::try_from(status.0)
                    .unwrap_or_else(|err| panic!(
                        "internal error: D-PDU API returned a non-primitive status for a primitive: {err}"
                    ));

                primitive_status_store.set(status);
                send_event(PrimitiveEvent::Status(status));
            }
            PduEventData::Error(error) => {
                error_event_store
                    .set(error.clone())
                    .expect("internal error: error event has been already stored");

                send_event(PrimitiveEvent::Error(error));
            }
            PduEventData::Result(result) => {
                send_event(PrimitiveEvent::Result(result));
            }
            PduEventData::Info(info) => {
                // This indicates an internal error. `PduInfoEvent`s are never expected
                // for communication primitives.
                error!("Invalid PduPrimitive event type received: {info}");
            }
        }

        stop
    }
}

impl Drop for PduPrimitive {
    fn drop(&mut self) {
        let Some(h_cop) = self.h_cop.get().map(|v| v.to_owned()) else {
            return;
        };

        if self.is_dead() {
            return;
        }

        let h_mod = self.get_module_handle();
        let h_cll = self.get_cll_handle();

        debug!(
            h_mod,
            h_cll, h_cop, "Cancelling the PduPrimitive via destructor..."
        );

        match self.worker.get() {
            Some(worker) => {
                let query = Query::VtCopDestructor(h_mod, h_cll, h_cop);
                match worker.request(query, None) {
                    Ok(_) => {}
                    Err(err) => {
                        error!(
                            h_mod,
                            h_cll,
                            h_cop,
                            "Error when cancelling the PduPrimitive via destructor: {err}"
                        );
                    }
                }
            }
            None => {
                let api = self.api.clone();
                spawn(move || api.vt_cop_destructor(h_mod, h_cll, h_cop));
            }
        }
    }
}

/// Current execution status of a D-PDU communication primitive.
#[derive(Debug, Clone)]
pub struct CopStatus(PduStatusData);

impl CopStatus {
    /// Returns `true` if the primitive is idle.
    pub fn is_idle(&self) -> bool {
        matches!(self.0.status_code, PduStatus::CopstIdle)
    }

    /// Returns `true` if the primitive is currently executing.
    pub fn is_executing(&self) -> bool {
        matches!(self.0.status_code, PduStatus::CopstExecuting)
    }

    /// Returns `true` if the primitive has finished execution.
    pub fn is_finished(&self) -> bool {
        matches!(self.0.status_code, PduStatus::CopstFinished)
    }

    /// Returns `true` if the primitive was cancelled.
    pub fn is_cancelled(&self) -> bool {
        matches!(self.0.status_code, PduStatus::CopstCancelled)
    }

    /// Returns `true` if the primitive is waiting for execution.
    pub fn is_waiting(&self) -> bool {
        matches!(self.0.status_code, PduStatus::CopstWaiting)
    }
}

/// Parameters used to configure a D-PDU communication primitive.
#[derive(Debug, Clone)]
pub struct PrimitiveParams {
    /// Cycle time in milliseconds for cyclic transmission.
    pub time: u32,

    /// Number of transmission cycles.
    pub send_cycles: SendCycles,

    /// Number of receive cycles.
    pub receive_cycles: ReceiveCycles,

    /// ComParam buffer attached to the primitive.
    ///
    /// See [`ComParamBuffer`] for details.
    pub temp_param_update: ComParamBuffer,

    /// Transmission flags.
    pub tx_flag: TransmitFlags,

    /// Expected responses used to match incoming data.
    pub expected_responses: Vec<ExpectedResponse>,
}

impl Default for PrimitiveParams {
    fn default() -> Self {
        Self {
            time: 0,
            send_cycles: SendCycles::Normal(0),
            receive_cycles: ReceiveCycles::Normal(0),
            temp_param_update: ComParamBuffer::default(),
            tx_flag: TransmitFlags::default(),
            expected_responses: vec![],
        }
    }
}

/// Flags controlling the behavior of a transmit ComPrimitive.
///
/// These flags correspond to the D-PDU `PDU_FLAG_DATA` field and control
/// response handling, additional result information, and ISO transport
/// layer options.
///
/// Flags encoded into the D-PDU transmit flag data field.
#[derive(Debug, Clone)]
pub struct TransmitFlags {
    /// Suppresses positive responses when supported by the protocol.
    ///
    /// Used for ISO 15765-3 / ISO 14229-3.
    pub suppress_positive_response: bool,

    /// Enables additional information in result data for debugging purposes.
    pub enable_extra_info: bool,

    /// Reduces response wait time when only a single response is expected.
    ///
    /// Applies only when the ComLogicalLink was created in raw mode.
    pub wait_p3_min_only: bool,

    /// Uses 29-bit CAN identifiers instead of 11-bit identifiers.
    ///
    /// Applies only when the ComLogicalLink was created in raw mode.
    pub can_29_bit: bool,

    /// Enables ISO 15765-2 extended addressing.
    ///
    /// Applies only when the ComLogicalLink was created in raw mode.
    pub iso_15765_addr_type: bool,

    /// Enables ISO 15765-2 CAN frame padding.
    ///
    /// Padding uses the `CP_CanFillerByte` ComParam value.
    ///
    /// Applies only when the ComLogicalLink was created in raw mode.
    pub iso_15765_frame_pad: bool,
}

impl Default for TransmitFlags {
    fn default() -> Self {
        Self {
            suppress_positive_response: false,
            enable_extra_info: false,
            wait_p3_min_only: false,
            can_29_bit: false,
            iso_15765_addr_type: false,
            iso_15765_frame_pad: false,
        }
    }
}

impl TransmitFlags {
    /// Encodes the zero byte of the D-PDU transmit flag data.
    pub(crate) fn zb(&self) -> u8 {
        let mut value = 0;

        if self.suppress_positive_response {
            value |= 0x40;
        }

        if self.enable_extra_info {
            value |= 0x20;
        }

        value
    }

    /// Encodes the second byte of the D-PDU transmit flag data.
    pub(crate) fn sb(&self) -> u8 {
        let mut value = 0;

        if self.wait_p3_min_only {
            value |= 0x02;
        }

        if self.can_29_bit {
            value |= 0x01;
        }

        value
    }

    /// Encodes the third byte of the D-PDU transmit flag data.
    pub(crate) fn tb(&self) -> u8 {
        let mut value = 0;

        if self.iso_15765_addr_type {
            value |= 0x80;
        }

        if self.iso_15765_frame_pad {
            value |= 0x40;
        }

        value
    }

    /// Encodes this flag set as a `PDU_FLAG_DATA` value.
    pub(crate) fn get_pdu_flag_data(&self) -> [u8; 4] {
        [self.zb(), 0, self.sb(), self.tb()]
    }
}

/// Defines an expected response used for matching received response data.
#[derive(Debug, Clone)]
pub struct ExpectedResponse {
    /// Expected response type.
    pub response_type: ResponseType,

    /// Application-defined ID returned in `PDU_RESULT_DATA` to identify the
    /// matched expected response.
    pub acceptance_id: u32,

    /// Mask and pattern data used to match the response payload.
    pub mask_data: MaskData,

    /// Optional list of unique response IDs allowed for this expected response.
    ///
    /// If empty, responses with any unique response ID are considered during
    /// matching.
    pub unique_response_ids: Vec<u32>,
}

/// Type of a D-PDU response.
#[repr(u32)]
#[derive(Debug, Copy, Clone, strum::AsRefStr)]
pub enum ResponseType {
    /// Positive response.
    Positive = 0,

    /// Negative response.
    Negative = 1,
}

impl ResponseType {
    /// Returns the string representation of this response type.
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }
}

/// Receive filtering mask and pattern data.
///
/// Used by the D-PDU API to match received response data against an expected
/// response. Each byte of the pattern is compared using the corresponding mask
/// byte.
#[derive(Debug, Clone, Default)]
pub struct MaskData {
    pub(crate) mask: Vec<u8>,
    pub(crate) pattern: Vec<u8>,
}

impl MaskData {
    /// Creates a new mask and pattern pair.
    ///
    /// Returns `None` if the mask and pattern lengths do not match.
    pub fn new(mask: impl AsRef<[u8]>, pattern: impl AsRef<[u8]>) -> Option<Self> {
        let mask = mask.as_ref();
        let pattern = pattern.as_ref();

        if mask.len() != pattern.len() {
            return None;
        }

        Some(Self {
            mask: mask.to_vec(),
            pattern: pattern.to_vec(),
        })
    }

    pub fn empty() -> Self {
        Self {
            mask: vec![],
            pattern: vec![],
        }
    }

    /// Returns the number of mask and pattern bytes.
    pub fn len(&self) -> usize {
        assert_eq!(self.mask.len(), self.pattern.len());
        self.mask.len()
    }

    /// Returns the mask bytes.
    pub fn get_mask(&self) -> &[u8] {
        assert_eq!(self.mask.len(), self.pattern.len());
        &self.mask
    }

    /// Returns the pattern bytes.
    pub fn get_pattern(&self) -> &[u8] {
        assert_eq!(self.mask.len(), self.pattern.len());
        &self.pattern
    }
}

/// Defines the number of send cycles for a ComPrimitive.
#[derive(Debug, Clone)]
pub enum SendCycles {
    /// Send a fixed number of times.
    Normal(u32),

    /// Continue sending until the primitive is completed.
    Infinite,
}

impl Default for SendCycles {
    fn default() -> Self {
        SendCycles::Normal(0)
    }
}

impl SendCycles {
    /// Converts this value to the D-PDU API representation.
    ///
    /// Returns `-1` for infinite send cycles and a positive value for a
    /// fixed number of cycles.
    pub fn to_i32(&self) -> i32 {
        match self {
            SendCycles::Normal(v) => i32::try_from(*v)
                .unwrap_or_else(|_| panic!("SendCycles value is too large for i32: {v}")),
            SendCycles::Infinite => -1,
        }
    }
}

/// Defines the number of receive cycles for a ComPrimitive.
#[derive(Debug, Clone)]
pub enum ReceiveCycles {
    /// Receive a fixed number of responses.
    Normal(u32),

    /// Continue receiving responses until the primitive is completed.
    Infinite,

    /// Receive multiple responses and match them using Unique Response IDs.
    Multiple,
}

impl Default for ReceiveCycles {
    fn default() -> Self {
        ReceiveCycles::Normal(0)
    }
}

impl ReceiveCycles {
    /// Converts this value to the D-PDU API representation.
    ///
    /// Returns:
    /// - a positive value for a fixed number of cycles,
    /// - `-1` for infinite receive cycles,
    /// - `-2` for multiple response mode.
    pub fn to_i32(&self) -> i32 {
        match self {
            ReceiveCycles::Normal(v) => i32::try_from(*v)
                .unwrap_or_else(|_| panic!("ReceiveCycles value is too large for i32: {v}")),
            ReceiveCycles::Infinite => -1,
            ReceiveCycles::Multiple => -2,
        }
    }
}

/// Specifies the ComParam buffer to use for a ComPrimitive.
///
/// Controls whether the primitive uses the active ComParam buffer or a
/// temporary working buffer that remains unchanged during its execution.
#[repr(u32)]
#[derive(Debug, Copy, Clone, Default, strum::AsRefStr)]
pub enum ComParamBuffer {
    /// Use the active ComParam buffer.
    ///
    /// The primitive keeps the ComParams assigned at creation time, even if
    /// the active buffer is modified by subsequent primitives.
    #[default]
    Active = 0,

    /// Use the working ComParam buffer.
    ///
    /// The primitive uses a temporary ComParam configuration that remains
    /// unchanged until the primitive is completed.
    Working = 1,
}

impl ComParamBuffer {
    /// Returns the string representation of this buffer type.
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }
}

/// High-level events generated by a D-PDU communication primitive.
///
/// These events abstract the underlying D-PDU API notifications and provide
/// information about primitive execution results and errors.
#[derive(Debug, Clone)]
pub enum PrimitiveEvent {
    /// Result data produced by the primitive execution.
    Result(PduResultEvent),

    /// Error reported during primitive execution.
    Error(PduErrorEvent),

    /// Status update reported during primitive execution.
    Status(PrimitiveStatus),

    /// Start error reported by the D-PDU layer.
    ///
    /// This event contains detailed information about failures that are not
    /// tied to a specific primitive result or execution state.
    StartFailed(GeneralError),
}

/// High-level lifecycle status of a D-PDU communication primitive.
#[derive(Debug, Copy, Clone, Default)]
pub enum PrimitiveStatus {
    /// The primitive exists only as a local object and has not been started
    /// through the D-PDU API yet.
    ///
    /// No D-PDU primitive execution is associated with this state.
    #[default]
    Created,

    /// The primitive has been created but execution has not started yet.
    ///
    /// Corresponds to `COPST_IDLE`.
    Idle,

    /// The primitive is currently being executed.
    ///
    /// Corresponds to `COPST_EXECUTING`.
    Executing,

    /// The primitive is waiting for an external condition or event before it can continue.
    ///
    /// Corresponds to `COPST_WAITING`.
    Waiting,

    /// The primitive has reached its final state after successful execution.
    ///
    /// Corresponds to `COPST_FINISHED`.
    Finished,

    /// The primitive execution has been cancelled.
    ///
    /// Corresponds to `COPST_CANCELLED`.
    Cancelled,
}

impl PrimitiveStatus {
    /// Returns whether the primitive is still active.
    ///
    /// Active states are `Idle`, `Executing`, and `Waiting`.
    /// Terminal states (`Finished` and `Cancelled`) return `false`.
    pub fn is_alive(&self) -> bool {
        match self {
            Self::Idle | Self::Executing | Self::Waiting | Self::Created => true,
            Self::Finished | Self::Cancelled => false,
        }
    }
}

/// Thread-safe storage for the current high-level primitive status.
#[derive(Debug, Default)]
pub(crate) struct PrimitiveStatusStore {
    store: Mutex<PrimitiveStatus>,
}

impl PrimitiveStatusStore {
    /// Creates a new status store with the default primitive status.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Updates the current primitive status.
    ///
    /// The previously stored status is replaced.
    pub(crate) fn set(&self, status: PrimitiveStatus) {
        *self.store.lock() = status;
    }

    /// Returns the current primitive status.
    ///
    /// The returned value is a snapshot of the status at the time of the call.
    pub(crate) fn get(&self) -> PrimitiveStatus {
        self.store.lock().clone()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unsupported primitive status: {}", .0.as_str())]
pub struct InvalidPrimitiveStatusError(PduStatus);

impl TryFrom<PduStatus> for PrimitiveStatus {
    type Error = InvalidPrimitiveStatusError;

    fn try_from(value: PduStatus) -> Result<Self, Self::Error> {
        match value {
            PduStatus::CopstIdle => Ok(Self::Idle),
            PduStatus::CopstExecuting => Ok(Self::Executing),
            PduStatus::CopstWaiting => Ok(Self::Waiting),
            PduStatus::CopstCancelled => Ok(Self::Cancelled),
            PduStatus::CopstFinished => Ok(Self::Finished),
            status => Err(InvalidPrimitiveStatusError(status)),
        }
    }
}

/// Variant for creating a [`PduPrimitive`].
#[derive(Debug, Clone)]
pub enum PrimitiveType {
    /// `PDU_COPT_START_COMM`
    StartComm {
        /// Payload data to send.
        data: Bytes,

        /// Primitive parameters.
        params: PrimitiveParams,
    },

    /// `PDU_COPT_SEND_RECV`
    SendRecv {
        /// Payload data to send.
        data: Bytes,

        /// Primitive parameters.
        params: PrimitiveParams,
    },

    /// `PDU_COPT_STOP_COMM`
    StopComm {
        /// Payload data to send.
        data: Bytes,

        /// Primitive parameters.
        params: PrimitiveParams,
    },

    /// `PDU_COPT_UPDATE_PARAM`
    UpdateParam,

    /// `PDU_COPT_RESTORE_PARAM`
    RestoreParam,

    /// `PDU_COPT_DELAY`
    Delay {
        /// The delay duration for a `PDU_COPT_DELAY` primitive in milliseconds.
        time: u32,
    },
}

impl PrimitiveType {
    /// Converts this type into a [`PduCopt`].
    pub fn to_native_type(&self) -> PduCopt {
        match self {
            Self::StartComm { .. } => PduCopt::StartComm,
            Self::SendRecv { .. } => PduCopt::SendRecv,
            Self::StopComm { .. } => PduCopt::StopComm,
            Self::UpdateParam => PduCopt::UpdateParam,
            Self::RestoreParam => PduCopt::RestoreParam,
            Self::Delay { .. } => PduCopt::Delay,
        }
    }

    /// Returns the data buffer associated with this primitive operation.
    ///
    /// Data is available only for primitives that transfer payload data:
    /// - [`StartComm`](PrimitiveType::StartComm),
    /// - [`SendRecv`](PrimitiveType::SendRecv),
    /// - [`StopComm`](PrimitiveType::StopComm).
    ///
    /// Returns [`None`] for primitives that do not contain a data payload.
    pub fn get_data(&self) -> Option<&[u8]> {
        match self {
            Self::StartComm { data, .. } => Some(data.as_ref()),
            Self::SendRecv { data, .. } => Some(data.as_ref()),
            Self::StopComm { data, .. } => Some(data.as_ref()),
            _ => None,
        }
    }

    /// Returns the communication parameters associated with this primitive.
    ///
    /// Parameters are available only for communication primitives:
    /// - [`StartComm`](PrimitiveType::StartComm),
    /// - [`SendRecv`](PrimitiveType::SendRecv),
    /// - [`StopComm`](PrimitiveType::StopComm).
    ///
    /// Returns [`None`] for primitives that do not require communication
    /// parameters.
    pub fn get_params(&self) -> Option<&PrimitiveParams> {
        match self {
            Self::StartComm { params, .. } => Some(params),
            Self::SendRecv { params, .. } => Some(params),
            Self::StopComm { params, .. } => Some(params),
            _ => None,
        }
    }
}
