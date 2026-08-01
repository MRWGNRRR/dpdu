use crate::AsyncRuntimeTarget;
use crate::api::PduApi;
use crate::constants::{CLL_EVENTS_QUEUE_SIZE, MODULE_EVENTS_QUEUE_SIZE};
use crate::error::GeneralResult;
use crate::event_callback::event_callback;
use crate::handle_manager::PduHandleManager;
use crate::types::pdu_com_logical_link::{CllCreateFlags, CllCreateType, PduLogicalLink};
use crate::types::pdu_event::{PduEvent, PduEventTarget, StopReceive};
use crate::types::pdu_module::PduModuleData;
use crate::types::pdu_status::{PduStatusData, PduStatusTarget};
use crate::types::{PduModuleHandle, PduUniqueCllTag};
use crate::utils::{NonClonable, random_non_zero_usize};
use crate::worker::{PduAsyncWorker, Query};
use dpdu_api_types::PduStatus;
use parking_lot::Mutex;
use regex::Regex;
use std::ops::Deref;
use std::sync::{Arc, LazyLock, OnceLock, Weak};
use std::thread::spawn;
use tokio::sync::{broadcast, mpsc};
use tokio::task::spawn_blocking;
use tracing::{debug, error};

pub type VciList = Vec<Arc<PduVci>>;

#[derive(Debug)]
pub struct PduVci {
    pub(crate) me: Weak<PduVci>,

    pub(crate) api: Arc<PduApi>,

    pub(crate) worker: OnceLock<Arc<PduAsyncWorker>>,

    pub(crate) module_data: PduModuleData,

    /// Event channel sender owned by [`PduVci`].
    ///
    /// The sender is intentionally dropped when [`PduVci`] is dropped,
    /// allowing the event listener to detect shutdown.
    ///
    /// A weak reference is stored in [`PduHandleManager`] to keep track of the
    /// channel lifetime and to automatically stop event listening in
    /// [`PduVci::listen_events`] and
    /// [`PduVci::blocking_listen_events`].
    ///
    /// # Safety
    ///
    /// This sender must not be cloned. Cloning it would extend the channel
    /// lifetime beyond [`PduVci`] and prevent listeners from being
    /// automatically stopped.
    ///
    /// [`PduHandleManager`]: crate::handle_manager::PduHandleManager
    pub(crate) pdu_event_tx: NonClonable<mpsc::UnboundedSender<PduEvent>>,

    /// Sender used to create additional receivers after the initial receiver
    /// has been taken.
    pub(crate) module_event_tx: broadcast::Sender<()>,

    /// The initial receiver returned by the first [`get_vci_event_receiver`] call.
    ///
    /// After the receiver is taken, subsequent calls create new receivers from
    /// [`module_event_tx`].
    pub(crate) module_event_rx: Mutex<Option<broadcast::Receiver<()>>>,

    pub(crate) pdu_sync: Mutex<()>,
}

impl PartialEq for PduVci {
    fn eq(&self, other: &Self) -> bool {
        self.api.unique_tag == other.api.unique_tag
            && self.get_module_handle() == other.get_module_handle()
    }
}

impl PduVci {
    pub(crate) fn set_worker(&self, worker: Arc<PduAsyncWorker>) {
        let _ = self.worker.set(worker);
    }

    pub fn get_module_handle(&self) -> PduModuleHandle {
        self.module_data.h_mod
    }

    pub fn get_name(&self) -> Option<&String> {
        self.module_data.vendor_module_name.as_ref()
    }

    pub fn get_additional_info(&self) -> Option<&String> {
        self.module_data.vendor_additional_info.as_ref()
    }

    pub fn get_normalized_name(&self) -> Option<String> {
        static EDIC_RGX: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r#"(?U)ModuleName='(?<name>.+)'"#).unwrap());
        static ACTIA_RGX: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r#"(?U)MVCIFriendlyName='(?<name>.+)'"#).unwrap());

        let module_name = self
            .module_data
            .vendor_module_name
            .clone()
            .unwrap_or_else(|| "VCI".to_string());

        let normalize = |name: &str| {
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_digit()) {
                format!("VCI S/N: {name}")
            } else {
                name.to_owned()
            }
        };

        for regex in [&*EDIC_RGX, &*ACTIA_RGX] {
            if let Some(caps) = regex.captures(&module_name) {
                return Some(normalize(caps.name("name").unwrap().as_str()));
            }
        }

        Some(normalize(&module_name))
    }

    fn take_me_expect(&self) -> Arc<PduVci> {
        self.me
            .upgrade()
            .expect("internal error: Vci self-reference is no longer valid")
    }

    pub fn blocking_get_status(&self) -> GeneralResult<VciStatus> {
        let _sync_guard = self.pdu_sync.lock();
        let target = PduStatusTarget::Module(self.module_data.h_mod);
        let result = self.api.pdu_get_status(&target)?;
        Ok(VciStatus(result))
    }

    pub async fn get_status(&self) -> GeneralResult<VciStatus> {
        match self.worker.get() {
            Some(worker) => {
                let target = PduStatusTarget::Module(self.module_data.h_mod);
                let result = worker.pdu_get_status(target).await?;
                Ok(VciStatus(result))
            }
            None => {
                let me = self.take_me_expect();
                let result = spawn_blocking(move || me.blocking_get_status())
                    .await
                    .expect("internal error: PduVci::blocking_get_status() task panicked")?;
                Ok(result)
            }
        }
    }

    pub fn blocking_connect(&self) -> GeneralResult<bool> {
        let status = self.blocking_get_status()?;
        if !status.is_available_for_connection() {
            return Ok(false);
        }

        let _sync_guard = self.pdu_sync.lock();
        self.api.pdu_module_connect(self.module_data.h_mod)?;
        Ok(true)
    }

    pub async fn connect(&self) -> GeneralResult<bool> {
        match self.worker.get() {
            Some(worker) => {
                let status = self.get_status().await?;
                if !status.is_available_for_connection() {
                    return Ok(false);
                }
                worker.pdu_module_connect(self.module_data.h_mod).await?;
                Ok(true)
            }
            None => {
                let me = self.take_me_expect();
                let result = spawn_blocking(move || me.blocking_connect())
                    .await
                    .expect("internal error: PduVci::blocking_connect() task panicked")?;
                Ok(result)
            }
        }
    }

    pub fn blocking_disconnect(&self) -> GeneralResult<bool> {
        let status = self.blocking_get_status()?;
        if !status.is_connected() {
            return Ok(false);
        }

        let _sync_guard = self.pdu_sync.lock();
        self.api
            .pdu_module_disconnect(Some(self.module_data.h_mod))?;
        Ok(true)
    }

    pub async fn disconnect(&self) -> GeneralResult<bool> {
        match self.worker.get() {
            Some(worker) => {
                let status = self.get_status().await?;
                if !status.is_connected() {
                    return Ok(false);
                }
                worker
                    .pdu_module_disconnect(Some(self.module_data.h_mod))
                    .await?;
                Ok(true)
            }
            None => {
                let me = self.take_me_expect();
                let result = spawn_blocking(move || me.blocking_disconnect())
                    .await
                    .expect("internal error: PduVci::blocking_disconnect() task panicked")?;
                Ok(result)
            }
        }
    }

    pub fn blocking_list(
        api: &Arc<PduApi>,
        events_queue_size: Option<usize>,
    ) -> GeneralResult<VciList> {
        let modules = api.pdu_get_module_ids().inspect_err(|err| {
            error!("Failed to retrieve the list of communication modules: {err}");
        })?;

        let events_queue_size = events_queue_size.unwrap_or(MODULE_EVENTS_QUEUE_SIZE);
        let mut list = Vec::with_capacity(modules.len());

        for module in modules.iter() {
            let (pdu_event_tx, pdu_event_rx) = mpsc::unbounded_channel();

            let (module_event_tx, module_event_rx) = broadcast::channel(events_queue_size);

            // This thread is necessary for receiving events from the D-PDU API via the built-in
            // event callback mechanism, in order to generate the high-level events that
            // are actually required when using PduVci.
            spawn({
                let module_event_tx = module_event_tx.clone();
                move || PduVci::blocking_listen_events(pdu_event_rx, module_event_tx)
            });

            let vci = Arc::new_cyclic(|weak| PduVci {
                me: weak.clone(),
                api: api.clone(),
                worker: OnceLock::default(),
                module_data: module.clone(),
                pdu_event_tx: NonClonable(pdu_event_tx.clone()),
                module_event_tx,
                module_event_rx: Mutex::new(Some(module_event_rx)),
                pdu_sync: Mutex::default(),
            });

            /* TODO: Register after connect
            match api.pdu_register_event_callback(
                &PduEventTarget::Module(module.h_mod),
                Some(event_callback)
            ) {
                Ok(_) => {},
                Err(ApiError::PduError(PduError::ModuleNotConnected)) => {
                    continue;
                },
                Err(e) => {
                    return Err(e)?;
                }
            }*/

            PduHandleManager::register_module(
                api.unique_tag,
                module.h_mod,
                pdu_event_tx.downgrade(),
                Arc::downgrade(&vci),
            );

            list.push(vci);
        }

        Ok(list)
    }

    pub async fn list<'a>(
        runtime: impl Into<AsyncRuntimeTarget<'a>>,
        events_queue_size: Option<usize>,
    ) -> GeneralResult<VciList> {
        let events_queue_size = events_queue_size.unwrap_or(MODULE_EVENTS_QUEUE_SIZE);

        match runtime.into() {
            AsyncRuntimeTarget::Api(api) => {
                let api = api.clone_arc();
                let result =
                    spawn_blocking(move || PduVci::blocking_list(&api, Some(events_queue_size)))
                        .await
                        .expect("internal error: PduVci::blocking_list() task panicked");
                Ok(result?)
            }
            AsyncRuntimeTarget::Worker(worker) => {
                let modules = worker.pdu_get_module_ids().await.inspect_err(|err| {
                    error!("Failed to retrieve the list of communication modules: {err}");
                })?;

                let mut list = Vec::with_capacity(modules.len());

                for module in modules.iter() {
                    let (pdu_event_tx, pdu_event_rx) = mpsc::unbounded_channel();

                    let (module_event_tx, module_event_rx) = broadcast::channel(events_queue_size);

                    // This task is necessary for receiving events from the D-PDU API via the built-in
                    // event callback mechanism, in order to generate the high-level events that
                    // are actually required when using PduVci.
                    tokio::spawn({
                        let module_event_tx = module_event_tx.clone();
                        PduVci::listen_events(pdu_event_rx, module_event_tx)
                    });

                    let vci = Arc::new_cyclic(|weak| PduVci {
                        me: weak.clone(),
                        api: worker.api.clone(),
                        worker: OnceLock::default(),
                        module_data: module.clone(),
                        pdu_event_tx: NonClonable(pdu_event_tx),
                        module_event_tx,
                        module_event_rx: Mutex::new(Some(module_event_rx)),
                        pdu_sync: Mutex::default(),
                    });

                    /* TODO: Register after connect
                    match worker.pdu_register_event_callback(
                        PduEventTarget::Module(module.h_mod),
                        Some(event_callback)
                    ).await {
                        Ok(_) => {},
                        Err(GeneralError::ApiError(ApiError::PduError(PduError::ModuleNotConnected))) => {
                            continue;
                        },
                        Err(e) => {
                            return Err(e)?;
                        }
                    }*/

                    PduHandleManager::register_module(
                        worker.api.unique_tag,
                        module.h_mod,
                        vci.pdu_event_tx.get_ref().downgrade(),
                        Arc::downgrade(&vci),
                    );

                    list.push(vci);
                }

                Ok(list)
            }
        }
    }

    pub fn blocking_create_logical_link(
        &self,
        create_type: &CllCreateType,
        create_flags: &CllCreateFlags,
        events_queue_size: Option<usize>,
    ) -> GeneralResult<Arc<PduLogicalLink>> {
        let _sync_guard = self.pdu_sync.lock();

        let events_queue_size = events_queue_size.unwrap_or(CLL_EVENTS_QUEUE_SIZE);
        let unique_tag: PduUniqueCllTag = random_non_zero_usize();

        let (pdu_event_tx, pdu_event_rx) = mpsc::unbounded_channel();

        let (logical_link_event_tx, logical_link_event_rx) = broadcast::channel(events_queue_size);

        // This thread is necessary for receiving events from the D-PDU API via the built-in
        // event callback mechanism, in order to generate the high-level events that
        // are actually required when using PduLogicalLink.
        spawn({
            let logical_link_event_tx = logical_link_event_tx.clone();
            move || PduLogicalLink::blocking_listen_events(pdu_event_rx, logical_link_event_tx)
        });

        // Register event tx for unique tag.
        PduHandleManager::register_cll(
            self.api.unique_tag,
            unique_tag,
            Some(pdu_event_tx.downgrade()),
            None,
        );

        let cll_data = self.api.pdu_create_com_logical_link(
            self.get_module_handle(),
            create_type,
            create_flags,
            Some(unique_tag),
        )?;

        let event_target = PduEventTarget::LogicalLink(self.get_module_handle(), cll_data.h_cll);
        let register_result = self
            .api
            .pdu_register_event_callback(&event_target, Some(event_callback));

        if let Err(err) = register_result {
            let _ = self
                .api
                .pdu_destroy_com_logical_link(self.get_module_handle(), cll_data.h_cll);
            return Err(err)?;
        }

        let cll = Arc::new_cyclic(|weak| PduLogicalLink {
            me: weak.clone(),
            api: self.api.clone(),
            worker: OnceLock::default(),
            unique_tag,
            cll_data: cll_data.into(),
            pdu_event_tx: NonClonable(pdu_event_tx),
            logical_link_event_tx,
            logical_link_event_rx: Mutex::new(Some(logical_link_event_rx)),
            sync: Mutex::default(),
        });

        // Register cll reference for unique tag.
        PduHandleManager::register_cll(
            self.api.unique_tag,
            unique_tag,
            None,
            Some(Arc::downgrade(&cll)),
        );

        Ok(cll)
    }

    pub async fn create_logical_link(
        &self,
        create_type: &CllCreateType,
        create_flags: &CllCreateFlags,
        events_queue_size: Option<usize>,
    ) -> GeneralResult<Arc<PduLogicalLink>> {
        let events_queue_size = events_queue_size.unwrap_or(CLL_EVENTS_QUEUE_SIZE);
        match self.worker.get() {
            Some(worker) => {
                let unique_tag: PduUniqueCllTag = random_non_zero_usize();

                let (pdu_event_tx, pdu_event_rx) = mpsc::unbounded_channel();

                let (logical_link_event_tx, logical_link_event_rx) =
                    broadcast::channel(events_queue_size);

                // This task is necessary for receiving events from the D-PDU API via the built-in
                // event callback mechanism, in order to generate the high-level events that
                // are actually required when using PduLogicalLink.
                tokio::spawn({
                    let logical_link_event_tx = logical_link_event_tx.clone();
                    PduLogicalLink::listen_events(pdu_event_rx, logical_link_event_tx)
                });

                // Register event tx for unique tag.
                PduHandleManager::register_cll(
                    self.api.unique_tag,
                    unique_tag,
                    Some(pdu_event_tx.downgrade()),
                    None,
                );

                let cll_data = worker
                    .pdu_create_com_logical_link(
                        self.get_module_handle(),
                        create_type.to_owned(),
                        create_flags.to_owned(),
                        Some(unique_tag),
                    )
                    .await?;

                let event_target =
                    PduEventTarget::LogicalLink(self.get_module_handle(), cll_data.h_cll);
                let register_result = worker
                    .pdu_register_event_callback(event_target.clone(), Some(event_callback))
                    .await;

                if let Err(err) = register_result {
                    let _ = worker
                        .pdu_destroy_com_logical_link(self.get_module_handle(), cll_data.h_cll);
                    return Err(err)?;
                }

                let cll = Arc::new_cyclic(|weak| PduLogicalLink {
                    me: weak.clone(),
                    api: self.api.clone(),
                    worker: OnceLock::from(worker.clone()),
                    unique_tag,
                    cll_data,
                    pdu_event_tx: NonClonable(pdu_event_tx),
                    logical_link_event_tx,
                    logical_link_event_rx: Mutex::new(Some(logical_link_event_rx)),
                    sync: Mutex::default(),
                });

                // Register cll reference for unique tag.
                PduHandleManager::register_cll(
                    self.api.unique_tag,
                    unique_tag,
                    None,
                    Some(Arc::downgrade(&cll)),
                );

                Ok(cll)
            }
            None => {
                let me = self.take_me_expect();

                let create_type = create_type.to_owned();
                let create_flags = create_flags.to_owned();

                let thread = move || {
                    me.blocking_create_logical_link(
                        &create_type,
                        &create_flags,
                        Some(events_queue_size),
                    )
                };

                let cll = spawn_blocking(thread).await.expect(
                    "internal error: PduVci::blocking_create_com_logical_link task panicked",
                )?;

                Ok(cll)
            }
        }
    }

    /// Reads the current vehicle battery voltage.
    ///
    /// This method performs a synchronous `VT_IOCTL_READ_VBATT` request and blocks
    /// the calling thread until the VCI returns the measured battery voltage.
    ///
    /// # Returns
    ///
    /// See [`PduApi::vt_io_ctl_read_vbatt`].
    ///
    /// # See also
    ///
    /// - [`PduVci::get_battery_voltage`] for the asynchronous equivalent.
    pub fn blocking_get_battery_voltage(&self) -> GeneralResult<Option<f32>> {
        Ok(self.api.vt_io_ctl_read_vbatt(self.module_data.h_mod)?)
    }

    /// Asynchronously reads the current vehicle battery voltage.
    ///
    /// If a dedicated worker thread is available, the request is executed on that
    /// worker. Otherwise, the blocking D-PDU API call is executed using
    /// `tokio::task::spawn_blocking` to avoid blocking the async runtime.
    ///
    /// # Returns
    ///
    /// See [`PduApi::vt_io_ctl_read_vbatt`].
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying D-PDU API call fails.
    pub async fn get_battery_voltage(&self) -> GeneralResult<Option<f32>> {
        let h_mod = self.module_data.h_mod;

        let me = self.take_me_expect();
        let thread = move || me.api.vt_io_ctl_read_vbatt(h_mod);

        // Run on a dedicated blocking thread because this I/O control may take
        // tens of milliseconds. Executing it on the worker queue would serialize
        // unrelated D-PDU requests and increase their latency.
        let result = spawn_blocking(thread)
            .await
            .expect("internal error: PduVci::blocking_get_battery_voltage task panicked")?;

        Ok(result)
    }

    pub(crate) fn blocking_listen_events(
        mut pdu_event_rx: mpsc::UnboundedReceiver<PduEvent>,
        mut module_event_tx: broadcast::Sender<()>,
    ) {
        loop {
            let event = match pdu_event_rx.blocking_recv() {
                Some(value) => value,
                None => {
                    // The channel will be closed when `drop()` is called for the `PduVci`.
                    break;
                }
            };

            if Self::handle_event(event, &mut module_event_tx) {
                break;
            }
        }
    }

    pub(crate) async fn listen_events(
        mut pdu_event_rx: mpsc::UnboundedReceiver<PduEvent>,
        mut module_event_tx: broadcast::Sender<()>,
    ) {
        loop {
            let event = match pdu_event_rx.recv().await {
                Some(value) => value,
                None => {
                    // The channel will be closed when `drop()` is called for the `PduVci`.
                    break;
                }
            };

            if Self::handle_event(event, &mut module_event_tx) {
                break;
            }
        }
    }

    pub(crate) fn handle_event(
        _event: PduEvent,
        _module_event_tx: &mut broadcast::Sender<()>,
    ) -> StopReceive {
        // TODO
        false
    }
}

impl Drop for PduVci {
    fn drop(&mut self) {
        debug!(
            h_mod = self.get_module_handle(),
            "Disconnecting the PduVci via destructor..."
        );

        match self.worker.get() {
            Some(worker) => {
                let query = Query::VtModuleDestructor(self.get_module_handle());
                match worker.request(query, None) {
                    Ok(_) => {}
                    Err(err) => {
                        error!(
                            h_mod = self.get_module_handle(),
                            "Error when disconnecting the PduVci via destructor: {err}"
                        );
                    }
                }
            }
            None => {
                let api = self.api.clone();
                let h_mod = self.get_module_handle();
                spawn(move || api.vt_module_destructor(h_mod));
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct VciStatus(PduStatusData);

impl Deref for VciStatus {
    type Target = PduStatusData;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl VciStatus {
    pub fn is_available_for_connection(&self) -> bool {
        self.is_status_avail()
    }

    pub fn is_connected(&self) -> bool {
        self.is_status_ready() || self.is_status_not_ready()
    }

    pub fn is_status_avail(&self) -> bool {
        matches!(self.status_code, PduStatus::ModstAvail)
    }

    pub fn is_status_not_avail(&self) -> bool {
        matches!(self.status_code, PduStatus::ModstNotAvail)
    }

    pub fn is_status_ready(&self) -> bool {
        matches!(self.status_code, PduStatus::ModstReady)
    }

    pub fn is_status_not_ready(&self) -> bool {
        matches!(self.status_code, PduStatus::ModstNotReady)
    }
}
