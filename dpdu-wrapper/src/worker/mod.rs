mod rpc;

use crate::api::PduApi;
use crate::error::{GeneralError, GeneralResult};
use crossbeam_channel::select;
pub use rpc::Query;
pub use rpc::Response;
use std::sync::{Arc, Weak};
use std::thread::spawn;
use tokio::sync::oneshot;
use tracing::{error, info, warn};

pub type WorkerResult<T> = std::result::Result<T, WorkerError>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum WorkerError {
    #[error("channel error: {0}")]
    ChannelError(String),

    #[error("worker stopped")]
    WorkerStopped,
}

#[derive(Debug, Clone)]
pub struct PduAsyncWorker {
    pub(crate) me: Weak<PduAsyncWorker>,

    pub(crate) api: Arc<PduApi>,

    pub(crate) shutdown_tx: crossbeam_channel::Sender<()>,

    pub(crate) query_tx: crossbeam_channel::Sender<(Query, Option<oneshot::Sender<Response>>)>,
}

impl PduAsyncWorker {
    pub fn new(api: Arc<PduApi>) -> Arc<Self> {
        let (shdn_tx, shdn_rx) = crossbeam_channel::bounded(1);
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();

        cmd_tx.send((Query::PduConstruct, None)).unwrap();

        let worker = Arc::new_cyclic(|weak| PduAsyncWorker {
            me: weak.clone(),
            api: api.clone(),
            shutdown_tx: shdn_tx,
            query_tx: cmd_tx,
        });

        // command supervisor
        spawn({
            let api = api.clone();
            let shdn_rx = shdn_rx.clone();
            let cmd_rx = cmd_rx.clone();
            move || {
                loop {
                    let api = api.clone();
                    let shdn_rx = shdn_rx.clone();
                    let cmd_rx = cmd_rx.clone();

                    let thread = move || {
                        PduAsyncWorker::command_thread(api.clone(), shdn_rx.clone(), cmd_rx.clone())
                    };

                    if spawn(thread).join().is_ok() {
                        break; // normal thread termination
                    } else {
                        warn!("D-PDU command worker panicked; restarting worker thread");
                    }
                }
            }
        });

        worker
    }

    pub(crate) fn clone_arc(&self) -> Arc<Self> {
        self.me
            .upgrade()
            .expect("internal error: unable to upgrade the Weak<PduAsyncWorker> pointer") // infallible
    }

    pub fn get_api(&self) -> &PduApi {
        &self.api
    }

    pub(crate) fn request(
        &self,
        query: Query,
        tx: Option<oneshot::Sender<Response>>,
    ) -> WorkerResult<()> {
        self.query_tx
            .try_send((query, tx))
            .map_err(|err| WorkerError::ChannelError(err.to_string()))?;

        Ok(())
    }

    fn command_thread(
        api: Arc<PduApi>,
        shdn: crossbeam_channel::Receiver<()>,
        cmd: crossbeam_channel::Receiver<(Query, Option<oneshot::Sender<Response>>)>,
    ) {
        use rpc::Query as Q;
        use rpc::Response as R;

        macro_rules! map {
            ($expr:expr) => {
                $expr.map_err(GeneralError::from)
            };
        }

        loop {
            select! {
                recv(shdn) -> _ => {
                    info!("Shutdown received");
                    return;
                },

                recv(cmd) -> msg => {
                    let (query, resp_tx) = match msg {
                        Ok((query, resp_tx)) => (query, resp_tx),
                        Err(err) => {
                            unreachable!("Unexpected command receive errror: {}", err);
                        }
                    };

                    let response = match query {
                        // Virtual functions.
                        Q::VtIoCtlReset => R::VtCllDestructor(map!(api.vt_io_ctl_reset())),
                        Q::VtIoCtlClearTxQueue(h_mod, h_cll) => R::VtIoCtlClearTxQueue(map!(api.vt_io_ctl_clear_tx_queue(h_mod, h_cll))),
                        Q::VtIoCtlSuspendTxQueue(h_mod, h_cll) => R::VtIoCtlSuspendTxQueue(map!(api.vt_io_ctl_suspend_tx_queue(h_mod, h_cll))),
                        Q::VtIoCtlResumeTxQueue(h_mod, h_cll) => R::VtIoCtlResumeTxQueue(map!(api.vt_io_ctl_resume_tx_queue(h_mod, h_cll))),
                        Q::VtIoCtlClearRxQueue(h_mod, h_cll) => R::VtIoCtlClearRxQueue(map!(api.vt_io_ctl_clear_rx_queue(h_mod, h_cll))),
                        Q::VtIoCtlReadVbatt(h_mod) => R::VtIoCtlReadVbatt(map!(api.vt_io_ctl_read_vbatt(h_mod))),
                        Q::VtIoCtlSetProgVoltage(h_mod, voltage, pin) => R::VtIoCtlSetProgVoltage(map!(api.vt_io_ctl_set_prog_voltage(h_mod, voltage, pin))),
                        Q::VtIoCtlReagProgVoltage(h_mod) => R::VtIoCtlReagProgVoltage(map!(api.vt_io_ctl_read_prog_voltage(h_mod))),
                        Q::VtIoCtlGeneric(h_mod, data) => R::VtIoCtlGeneric(map!(api.vt_io_ctl_generic(h_mod, &data))),
                        Q::VtIoCtlSetBufferSize(h_mod, h_cll, size) => R::VtIoCtlSetBufferSize(map!(api.vt_io_ctl_set_buffer_size(h_mod, h_cll, size))),
                        Q::VtIoCtlStartMsgFilter(h_mod, h_cll, data) => R::VtIoCtlStartMsgFilter(map!(api.vt_io_ctl_start_msg_filter(h_mod, h_cll, data))),
                        Q::VtIoCtlStopMsgFilter(h_mod, h_cll, number) => R::VtIoCtlStopMsgFilter(map!(api.vt_io_ctl_stop_msg_filter(h_mod, h_cll, number))),
                        Q::VtIoCtlClearMsgFilter(h_mod, h_cll) => R::VtIoCtlClearMsgFilter(map!(api.vt_io_ctl_clear_msg_filter(h_mod, h_cll))),
                        Q::VtIoCtlSetEventQueueProperties(h_mod, h_cll, size, mode) => R::VtIoCtlSetEventQueueProperties(map!(api.vt_io_ctl_set_event_queue_properties(h_mod, h_cll, size, mode))),
                        Q::VtIoCtlGetCableId(h_mod) => R::VtIoCtlGetCableId(map!(api.vt_io_ctl_get_cable_id(h_mod))),
                        Q::VtIoCtlSendBreak(h_mod, h_cll) => R::VtIoCtlSendBreak(map!(api.vt_io_ctl_send_break(h_mod, h_cll))),
                        Q::VtIoCtlReadIgnitionSenseState(h_mod, pin) => R::VtIoCtlReadIgnitionSenseState(map!(api.vt_io_ctl_read_ignition_sense_state(h_mod, pin))),

                        Q::VtModuleDestructor(h_mod) => R::VtModuleDestructor(map!(api.vt_module_destructor(h_mod))),
                        Q::VtCllDestructor(h_mod, h_cll) => R::VtCllDestructor(map!(api.vt_cll_destructor(h_mod, h_cll))),
                        Q::VtCopDestructor(h_mod, h_cll, h_cop) => R::VtCopDestructor(map!(api.vt_cop_destructor(h_mod, h_cll, h_cop))),

                        // Real D-PDU API queries.
                        Q::PduCancelComPrimitive(h_mod, h_cll, h_cop) => R::PduCancelComPrimitive(map!(api.pdu_cancel_com_primitive(h_mod, h_cll, h_cop))),
                        Q::PduConnect(h_mod, h_cll) => R::PduConnect(map!(api.pdu_connect(h_mod, h_cll))),
                        Q::PduConstruct => R::PduConstruct(map!(api.pdu_construct())),
                        Q::PduCreateComLogicalLink(h_mod, create_type, create_flags, tag) => R::PduCreateComLogicalLink(map!(api.pdu_create_com_logical_link(h_mod, &create_type, &create_flags, tag))),
                        Q::PduDestruct => R::PduDestruct(map!(api.pdu_destruct())),
                        Q::PduDestroyComLogicalLink(h_mod, h_cll) => R::PduDestroyComLogicalLink(map!(api.pdu_destroy_com_logical_link(h_mod, h_cll))),
                        Q::PduDestroyItem(ptr) => R::PduDestroyItem(map!(api.pdu_destroy_item(ptr.as_mut_ptr()))),
                        Q::PduDisconnect(h_mod, h_cll) => R::PduDisconnect(map!(api.pdu_disconnect(h_mod, h_cll))),
                        Q::PduGetComParam(h_mod, h_cll, object_id) => R::PduGetComParam(map!(api.pdu_get_com_param(h_mod, h_cll, object_id))),
                        Q::PduGetConflictingResources(resource_id, modules) => R::PduGetConflictingResources(map!(api.pdu_get_conflicting_resources(resource_id, &modules))),
                        Q::PduGetEventItem(target) => R::PduGetEventItem(map!(api.pdu_get_event_item(&target))),
                        Q::PduGetLastError(target) => R::PduGetLastError(map!(api.pdu_get_last_error(&target))),
                        Q::PduGetModuleIds => R::PduGetModuleIds(map!(api.pdu_get_module_ids())),
                        Q::PduGetObjectId(object, short_name) => R::PduGetObjectId(map!(api.pdu_get_object_id(object, &short_name))),
                        Q::PduGetResourceIds(h_mod, bus, protocol, pins) => R::PduGetResourceIds(map!(api.pdu_get_resource_ids(h_mod, &bus, &protocol, &pins))),
                        Q::PduGetResourceStatus(resources) => R::PduGetResourceStatus(map!(api.pdu_get_resource_status(&resources))),
                        Q::PduGetStatus(target) => R::PduGetStatus(map!(api.pdu_get_status(&target))),
                        Q::PduGetTimestamp(h_mod) => R::PduGetTimestamp(map!(api.pdu_get_timestamp(h_mod))),
                        Q::PduGetUniqueRespIdTable(h_mod, h_cll) => R::PduGetUniqueRespIdTable(map!(api.pdu_get_unique_resp_id_table(h_mod, h_cll))),
                        Q::PduGetVersion(h_mod) => R::PduGetVersion(map!(api.pdu_get_version(h_mod))),
                        Q::PduIoCtl(target, command, data) => R::PduIoCtl(map!(api.pdu_io_ctl(&target, &command, data.as_ref()))),
                        Q::PduLockResource(h_mod, h_cll, mask) => R::PduLockResource(map!(api.pdu_lock_resource(h_mod, h_cll, mask))),
                        Q::PduModuleConnect(h_mod) => R::PduModuleConnect(map!(api.pdu_module_connect(h_mod))),
                        Q::PduModuleDisconnect(h_mod) => R::PduModuleDisconnect(map!(api.pdu_module_disconnect(h_mod))),
                        Q::PduRegisterEventCallback(target, callback) => R::PduRegisterEventCallback(map!(api.pdu_register_event_callback(&target, callback))),
                        Q::PduSetComParam(h_mod, h_cll, cp) => R::PduSetComParam(map!(api.pdu_set_com_param(h_mod, h_cll, &cp))),
                        Q::PduSetUniqueRespIdTable(h_mod, h_cll, table) => R::PduSetUniqueRespIdTable(map!(api.pdu_set_unique_resp_id_table(h_mod, h_cll, &table))),
                        Q::PduStartComPrimitive(h_mod, h_cll, cop_type, data, params, tag) => R::PduStartComPrimitive(map!(api.pdu_start_com_primitive(h_mod, h_cll, cop_type, &data, params.as_ref(), tag))),
                        Q::PduUnlockResource(h_mod, h_cll, mask) => R::PduUnlockResource(map!(api.pdu_unlock_resource(h_mod, h_cll, mask)))
                    };

                    if let Some(tx) = resp_tx {
                        let _ = tx.send(response);
                    }
                }
            }
        }
    }

    pub(crate) async fn receive_query_response_callback(
        &self,
        query: Query,
    ) -> GeneralResult<Response> {
        let (tx, rx) = oneshot::channel();

        self.request(query, Some(tx))?;

        Ok(rx
            .await
            .map_err(|e| WorkerError::ChannelError(e.to_string()))?)
    }
}

impl Drop for PduAsyncWorker {
    fn drop(&mut self) {
        use crossbeam_channel::TrySendError;
        use tokio::spawn;

        let query_tx = self.query_tx.clone();
        let shutdown_tx = self.shutdown_tx.clone();

        spawn(async move {
            let (tx, rx) = oneshot::channel();

            match query_tx.try_send((Query::PduDestruct, Some(tx))) {
                Ok(_) => match rx.await {
                    Ok(_) => {}
                    Err(err) => {
                        error!("Query response channel has been unexpectedly closed: {err}");
                    }
                },
                Err(err) => {
                    error!("Unable to send destruct query to the PdyAsyncWorker: {err}");
                }
            };

            match shutdown_tx.try_send(()) {
                Err(TrySendError::Disconnected(_)) => {
                    error!("Unexpected closure of the shutdown channel in PduAsyncWorker");
                }
                _ => {}
            }
        });
    }
}
