mod rpc;

use crate::api::PduApi;
use crate::error::{GeneralError, GeneralResult};
use flume::{Selector, TrySendError};
pub use rpc::Query;
pub use rpc::Response;
use std::sync::{Arc, OnceLock, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::spawn;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tracing::{error, info, warn};

pub type WorkerResult<T> = ::std::result::Result<T, WorkerError>;

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

    pub(crate) query_tx: flume::Sender<(Query, Option<oneshot::Sender<Response>>)>,

    pub(crate) destruct_on_drop: Arc<AtomicBool>,

    pub(crate) dropped: Arc<OnceLock<()>>
}

impl PduAsyncWorker {
    pub fn new(api: Arc<PduApi>) -> Arc<Self> {
        let (cmd_tx, cmd_rx) = flume::unbounded();
        let need_shutdown = Arc::new(OnceLock::new());
        let destruct_on_drop = Arc::new(AtomicBool::new(true));

        cmd_tx.send((Query::PduConstruct, None)).unwrap();

        let worker = Arc::new_cyclic(|weak| PduAsyncWorker {
            me: weak.clone(),
            api: api.clone(),
            query_tx: cmd_tx,
            destruct_on_drop: destruct_on_drop.clone(),
            dropped: need_shutdown.clone(),
        });

        // command supervisor
        spawn(move || {
            loop {
                let api = api.clone();
                let need_shutdown = need_shutdown.clone();
                let cmd_rx = cmd_rx.clone();
                let destruct_on_drop = destruct_on_drop.clone();

                let thread = move || PduAsyncWorker::command_thread(
                    api,
                    need_shutdown,
                    destruct_on_drop,
                    cmd_rx
                );

                if spawn(thread).join().is_ok() {
                    break; // normal thread termination
                } else {
                    warn!("D-PDU command worker panicked; restarting worker thread");
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

    pub fn set_destruct_on_drop(&self, status: bool) {
        self.destruct_on_drop.store(status, Ordering::Release)
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
        need_shutdown: Arc<OnceLock<()>>,
        destruct_on_drop: Arc<AtomicBool>,
        cmd: flume::Receiver<(Query, Option<oneshot::Sender<Response>>)>,
    ) {
        use rpc::Query as Q;
        use rpc::Response as R;

        macro_rules! me {
            ($expr:expr) => {
                $expr.map_err(GeneralError::from)
            };
        }

        loop {
            if need_shutdown.get().is_some() && cmd.is_empty() {
                if destruct_on_drop.load(Ordering::Acquire) {
                    let _ = api.pdu_destruct();
                }
                break;
            }

            let (query, resp_tx) = match cmd.recv_timeout(Duration::from_millis(50)) {
                Ok((query, resp_tx)) => (query, resp_tx),
                Err(_) => {
                    // If there is an error here, it means that the destructor
                    // of PduAsyncWorker has been called.
                    continue;
                }
            };

            let response = match query {
                // Virtual functions.
                Q::VtIoCtlReset => {
                    R::VtCllDestructor(me!(api.vt_io_ctl_reset()))
                }
                Q::VtIoCtlClearTxQueue(h_mod, h_cll) => {
                    R::VtIoCtlClearTxQueue(me!(api.vt_io_ctl_clear_tx_queue(h_mod, h_cll)))
                }
                Q::VtIoCtlSuspendTxQueue(h_mod, h_cll) => {
                    R::VtIoCtlSuspendTxQueue(
                        me!(api.vt_io_ctl_suspend_tx_queue(h_mod, h_cll))
                    )
                }
                Q::VtIoCtlResumeTxQueue(h_mod, h_cll) => {
                    R::VtIoCtlResumeTxQueue(
                        me!(api.vt_io_ctl_resume_tx_queue(h_mod, h_cll))
                    )
                }
                Q::VtIoCtlClearRxQueue(h_mod, h_cll) => {
                    R::VtIoCtlClearRxQueue(me!(api.vt_io_ctl_clear_rx_queue(h_mod, h_cll)))
                }
                Q::VtIoCtlReadVbatt(h_mod) => {
                    R::VtIoCtlReadVbatt(me!(api.vt_io_ctl_read_vbatt(h_mod)))
                }
                Q::VtIoCtlSetProgVoltage(h_mod, voltage, pin) => {
                    R::VtIoCtlSetProgVoltage(
                        me!(api.vt_io_ctl_set_prog_voltage(h_mod, voltage, pin))
                    )
                }
                Q::VtIoCtlReagProgVoltage(h_mod) => {
                    R::VtIoCtlReagProgVoltage(me!(api.vt_io_ctl_read_prog_voltage(h_mod)))
                }
                Q::VtIoCtlGeneric(h_mod, data) => {
                    R::VtIoCtlGeneric(me!(api.vt_io_ctl_generic(h_mod, &data)))
                }
                Q::VtIoCtlSetBufferSize(h_mod, h_cll, size) => {
                    R::VtIoCtlSetBufferSize(
                        me!(api.vt_io_ctl_set_buffer_size(h_mod, h_cll, size))
                    )
                }
                Q::VtIoCtlStartMsgFilter(h_mod, h_cll, data) => {
                    R::VtIoCtlStartMsgFilter(
                        me!(api.vt_io_ctl_start_msg_filter(h_mod, h_cll, data))
                    )
                }
                Q::VtIoCtlStopMsgFilter(h_mod, h_cll, number) => {
                    R::VtIoCtlStopMsgFilter(
                        me!(api.vt_io_ctl_stop_msg_filter(h_mod, h_cll, number))
                    )
                }
                Q::VtIoCtlClearMsgFilter(h_mod, h_cll) => {
                    R::VtIoCtlClearMsgFilter(
                        me!(api.vt_io_ctl_clear_msg_filter(h_mod, h_cll))
                    )
                }
                Q::VtIoCtlSetEventQueueProperties(h_mod, h_cll, size, mode) => {
                    R::VtIoCtlSetEventQueueProperties(me!(
                                api.vt_io_ctl_set_event_queue_properties(
                                    h_mod,
                                    h_cll,
                                    size,
                                    mode
                                )
                            ))
                },
                Q::VtIoCtlGetCableId(h_mod) => {
                    R::VtIoCtlGetCableId(me!(api.vt_io_ctl_get_cable_id(h_mod)))
                }
                Q::VtIoCtlSendBreak(h_mod, h_cll) => {
                    R::VtIoCtlSendBreak(me!(api.vt_io_ctl_send_break(h_mod, h_cll)))
                }
                Q::VtIoCtlReadIgnitionSenseState(h_mod, pin) => {
                    R::VtIoCtlReadIgnitionSenseState(me!(
                                api.vt_io_ctl_read_ignition_sense_state(h_mod, pin)
                            ))
                }
                Q::VtModuleDestructor(h_mod) => {
                    R::VtModuleDestructor(me!(api.vt_module_destructor(h_mod)))
                }
                Q::VtCllDestructor(h_mod, h_cll) => {
                    R::VtCllDestructor(me!(api.vt_cll_destructor(h_mod, h_cll)))
                }
                Q::VtCopDestructor(h_mod, h_cll, h_cop) => {
                    R::VtCopDestructor(me!(api.vt_cop_destructor(h_mod, h_cll, h_cop)))
                }

                // Real D-PDU API queries.
                Q::PduCancelComPrimitive(h_mod, h_cll, h_cop) => {
                    R::PduCancelComPrimitive(me!(
                                api.pdu_cancel_com_primitive(h_mod, h_cll, h_cop)
                            ))
                }
                Q::PduConnect(h_mod, h_cll) => {
                    R::PduConnect(me!(api.pdu_connect(h_mod, h_cll)))
                }
                Q::PduConstruct => {
                    R::PduConstruct(me!(api.pdu_construct()))
                },
                Q::PduCreateComLogicalLink(h_mod, create_type, create_flags, tag) => {
                    R::PduCreateComLogicalLink(me!(
                                api.pdu_create_com_logical_link(
                                    h_mod,
                                    &create_type,
                                    &create_flags,
                                    tag
                                )
                            ))
                }
                Q::PduDestruct => {
                    R::PduDestruct(me!(api.pdu_destruct()))
                }
                Q::PduDestroyComLogicalLink(h_mod, h_cll) => {
                    R::PduDestroyComLogicalLink(me!(
                                api.pdu_destroy_com_logical_link(h_mod, h_cll)
                            ))
                }
                Q::PduDestroyItem(ptr) => {
                    R::PduDestroyItem(me!(api.pdu_destroy_item(ptr.as_mut_ptr())))
                }
                Q::PduDisconnect(h_mod, h_cll) => {
                    R::PduDisconnect(me!(api.pdu_disconnect(h_mod, h_cll)))
                }
                Q::PduGetComParam(h_mod, h_cll, object_id) => {
                    R::PduGetComParam(me!(api.pdu_get_com_param(h_mod, h_cll, object_id)))
                }
                Q::PduGetConflictingResources(resource_id, modules) => {
                    R::PduGetConflictingResources(me!(
                                api.pdu_get_conflicting_resources(resource_id, &modules)
                            ))
                }
                Q::PduGetEventItem(target) => {
                    R::PduGetEventItem(me!(api.pdu_get_event_item(&target)))
                }
                Q::PduGetLastError(target) => {
                    R::PduGetLastError(me!(api.pdu_get_last_error(&target)))
                }
                Q::PduGetModuleIds => {
                    R::PduGetModuleIds(me!(api.pdu_get_module_ids()))
                }
                Q::PduGetObjectId(object, short_name) => {
                    R::PduGetObjectId(me!(api.pdu_get_object_id(object, &short_name)))
                }
                Q::PduGetResourceIds(h_mod, bus, protocol, pins) => {
                    R::PduGetResourceIds(me!(
                                api.pdu_get_resource_ids(h_mod, &bus, &protocol, &pins)
                            ))
                }
                Q::PduGetResourceStatus(resources) => {
                    R::PduGetResourceStatus(me!(api.pdu_get_resource_status(&resources)))
                }
                Q::PduGetStatus(target) => {
                    R::PduGetStatus(me!(api.pdu_get_status(&target)))
                }
                Q::PduGetTimestamp(h_mod) => {
                    R::PduGetTimestamp(me!(api.pdu_get_timestamp(h_mod)))
                }
                Q::PduGetUniqueRespIdTable(h_mod, h_cll) => {
                    R::PduGetUniqueRespIdTable(me!(
                                api.pdu_get_unique_resp_id_table(h_mod, h_cll)
                            ))
                }
                Q::PduGetVersion(h_mod) => {
                    R::PduGetVersion(me!(api.pdu_get_version(h_mod)))
                }
                Q::PduIoCtl(target, command, data) => {
                    R::PduIoCtl(me!(api.pdu_io_ctl(&target, &command, data.as_ref())))
                }
                Q::PduLockResource(h_mod, h_cll, mask) => {
                    R::PduLockResource(me!(api.pdu_lock_resource(h_mod, h_cll, mask)))
                }
                Q::PduModuleConnect(h_mod) => {
                    R::PduModuleConnect(me!(api.pdu_module_connect(h_mod)))
                }
                Q::PduModuleDisconnect(h_mod) => {
                    R::PduModuleDisconnect(me!(api.pdu_module_disconnect(h_mod)))
                }
                Q::PduRegisterEventCallback(target, callback) => {
                    R::PduRegisterEventCallback(me!(
                                api.pdu_register_event_callback(&target, callback)
                            ))
                }
                Q::PduSetComParam(h_mod, h_cll, cp) => {
                    R::PduSetComParam(me!(api.pdu_set_com_param(h_mod, h_cll, &cp)))
                }
                Q::PduSetUniqueRespIdTable(h_mod, h_cll, table) => {
                    R::PduSetUniqueRespIdTable(me!(api.pdu_set_unique_resp_id_table(h_mod, h_cll, &table)))
                }
                Q::PduStartComPrimitive(h_mod, h_cll, primitive_type, tag) => {
                    R::PduStartComPrimitive(me!(
                                api.pdu_start_com_primitive(
                                    h_mod,
                                    h_cll,
                                    &primitive_type,
                                    tag
                                )
                            ))
                }
                Q::PduUnlockResource(h_mod, h_cll, mask) => {
                    R::PduUnlockResource(me!(api.pdu_unlock_resource(h_mod, h_cll, mask)))
                }
            };

            if let Some(tx) = resp_tx {
                let _ = tx.send(response);
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
        self.dropped.set(()).expect(
            "internal error: shutdown flag was already set"
        ); // infallible
    }
}
