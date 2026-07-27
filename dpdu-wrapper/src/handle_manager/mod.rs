use crate::api::PduApi;
use crate::types::pdu_com_logical_link::PduLogicalLink;
use crate::types::pdu_com_primitive::PduPrimitive;
use crate::types::pdu_event::PduEvent;
use crate::types::pdu_vci::PduVci;
use crate::types::{PduModuleHandle, PduUniqueApiTag, PduUniqueCllTag, PduUniqueCopTag};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, OnceLock, Weak};
use std::thread::{sleep, spawn};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc};
use tracing::warn;

static MGR: LazyLock<Arc<PduHandleManager>> = LazyLock::new(|| PduHandleManager::new());
static CONSTRUCTED: AtomicBool = AtomicBool::new(false);

/// Singleton PDU handle manager.
#[derive(Debug)]
pub struct PduHandleManager {
    apis: RwLock<HashMap<PduUniqueApiTag, HandleContainer<PduApi>>>,
    mods: RwLock<HashMap<(PduUniqueApiTag, PduModuleHandle), HandleContainer<PduVci>>>,
    clls: RwLock<HashMap<(PduUniqueApiTag, PduUniqueCllTag), HandleContainer<PduLogicalLink>>>,
    cops: RwLock<HashMap<(PduUniqueApiTag, PduUniqueCopTag), HandleContainer<PduPrimitive>>>,
}

impl PduHandleManager {
    fn assert_not_constructed() {
        let already_constructed = CONSTRUCTED
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err();

        if already_constructed {
            panic!("PduHandleManager instance has already been constructed");
        }
    }

    fn new() -> Arc<Self> {
        PduHandleManager::assert_not_constructed();

        let me = Arc::new(Self {
            apis: RwLock::default(),
            mods: Default::default(),
            clls: RwLock::default(),
            cops: RwLock::default(),
        });

        CONSTRUCTED.store(true, Ordering::Relaxed);

        // supervisor
        spawn({
            let me = me.clone();
            move || {
                loop {
                    let me = me.clone();
                    let thread_result = spawn(move || PduHandleManager::thread(me)).join();

                    if thread_result.is_ok() {
                        break;
                    } else {
                        warn!("D-PDU handle manager worker panicked; restarting worker thread");
                    }
                }
            }
        });

        me
    }
    
    pub(crate) fn register_api(api: &Arc<PduApi>, tx: mpsc::WeakUnboundedSender<PduEvent>) {
        let mut apis = MGR.apis.write();
        apis.insert(
            api.unique_tag,
            HandleContainer {
                reference: OnceLock::from(Arc::downgrade(api)),
                event_tx: OnceLock::from(tx),
                created_at: Instant::now(),
            },
        );
    }

    pub(crate) fn lookup_api_reference(unique_id: PduUniqueApiTag) -> Option<Arc<PduApi>> {
        let apis = MGR.apis.read();
        apis.get(&unique_id)?
            .reference
            .get()
            .and_then(Weak::upgrade)
    }

    pub(crate) fn lookup_api_event_tx(
        unique_id: PduUniqueApiTag,
    ) -> Option<mpsc::UnboundedSender<PduEvent>> {
        let apis = MGR.apis.read();
        apis.get(&unique_id)?.event_tx.get().and_then(mpsc::WeakUnboundedSender::upgrade)
    }

    /// Returns the only one D-PDU API that is registered.
    /// If several are registered, it returns None.
    /// This is necessary for those D-PDU drivers that cannot work with the API tag.
    pub(crate) fn get_single_api() -> Option<Arc<PduApi>> {
        let apis = MGR.apis.read();
        if apis.len() != 1 {
            return None;
        }
        apis.iter()
            .next()
            .and_then(|(_, api)| api.reference.get())
            .and_then(Weak::upgrade)
    }

    pub(crate) fn register_module(
        api_tag: PduUniqueApiTag,
        h_mod: PduModuleHandle,
        event_tx: mpsc::WeakUnboundedSender<PduEvent>,
        vci: Weak<PduVci>,
    ) {
        let mut mods = MGR.mods.write();

        let module = mods
            .entry((api_tag, h_mod))
            .or_insert(HandleContainer {
                reference: Default::default(),
                event_tx: Default::default(),
                created_at: Instant::now(),
            });

        module
            .reference
            .set(vci)
            .expect(&format!("internal error: reference already registered for module handle {h_mod}"));

        module
            .event_tx
            .set(event_tx)
            .expect(&format!("internal error: event_tx already register for module handle {h_mod}"));
    }

    pub(crate) fn lookup_module_reference(
        api_tag: PduUniqueApiTag,
        h_mod: PduModuleHandle,
    ) -> Option<Arc<PduVci>> {
        let mods = MGR.mods.read();
        mods.get(&(api_tag, h_mod))?
            .reference
            .get()
            .and_then(Weak::upgrade)
    }

    pub(crate) fn lookup_module_event_tx(
        api_tag: PduUniqueApiTag,
        h_mod: PduModuleHandle,
    ) -> Option<mpsc::UnboundedSender<PduEvent>> {
        let mods = MGR.mods.read();
        mods.get(&(api_tag, h_mod))?
            .event_tx
            .get()
            .and_then(mpsc::WeakUnboundedSender::upgrade)
    }

    pub(crate) fn register_cll(
        api_tag: PduUniqueApiTag,
        cll_tag: PduUniqueCllTag,
        tx: Option<mpsc::WeakUnboundedSender<PduEvent>>,
        cll: Option<Weak<PduLogicalLink>>,
    ) {
        let mut clls = MGR.clls.write();

        let container = clls
            .entry((api_tag, cll_tag))
            .or_insert(HandleContainer {
                reference: Default::default(),
                event_tx: Default::default(),
                created_at: Instant::now(),
            });

        if let Some(tx) = tx {
            container.event_tx.set(tx).expect(&format!(
                "internal error: event_tx already registered for cll tag {cll_tag}"
            ));
        }

        if let Some(cll) = cll {
            container.reference.set(cll).expect(&format!(
                "internal error: reference already registered for cll tag {cll_tag}"
            ));
        }
    }

    pub(crate) fn lookup_cll_reference(
        api_tag: PduUniqueApiTag,
        cll_tag: PduUniqueCllTag,
    ) -> Option<Arc<PduLogicalLink>> {
        let clls = MGR.clls.read();
        clls.get(&(api_tag, cll_tag))?
            .reference
            .get()
            .and_then(Weak::upgrade)
    }

    pub(crate) fn lookup_cll_event_tx(
        api_tag: PduUniqueApiTag,
        cll_tag: PduUniqueCllTag,
    ) -> Option<mpsc::UnboundedSender<PduEvent>> {
        let clls = MGR.clls.read();
        clls.get(&(api_tag, cll_tag))?
            .event_tx
            .get()
            .and_then(mpsc::WeakUnboundedSender::upgrade)
    }

    pub(crate) fn register_cop(
        api_tag: PduUniqueApiTag,
        cop_tag: PduUniqueCopTag,
        tx: Option<mpsc::WeakUnboundedSender<PduEvent>>,
        cop: Option<Weak<PduPrimitive>>,
    ) {
        let mut cops = MGR.cops.write();

        let container = cops.entry((api_tag, cop_tag)).or_insert(HandleContainer {
            reference: Default::default(),
            event_tx: Default::default(),
            created_at: Instant::now(),
        });

        if let Some(tx) = tx {
            container.event_tx.set(tx).expect(&format!(
                "internal error: event_tx already registered for cop tag {cop_tag}"
            ));
        }

        if let Some(cop) = cop {
            container.reference.set(cop).expect(&format!(
                "internal error: reference already registered for cop tag {cop_tag}"
            ));
        }
    }

    pub(crate) fn lookup_cop_reference(
        api_tag: PduUniqueApiTag,
        cop_tag: PduUniqueCopTag,
    ) -> Option<Arc<PduPrimitive>> {
        let cops = MGR.cops.read();
        cops.get(&(api_tag, cop_tag))?
            .reference
            .get()
            .and_then(Weak::upgrade)
    }

    pub(crate) fn lookup_cop_event_tx(
        api_tag: PduUniqueApiTag,
        cop_tag: PduUniqueCopTag,
    ) -> Option<mpsc::UnboundedSender<PduEvent>> {
        let cops = MGR.cops.read();
        cops.get(&(api_tag, cop_tag))?
            .event_tx
            .get()
            .and_then(mpsc::WeakUnboundedSender::upgrade)
    }

    /// A function for clearing containers of outdated entities in the D-PDU API.
    fn retain_handle_containers<T>(now: &Instant, container: &mut HandleContainer<T>) -> bool {
        const REFERENCE_TIMEOUT: Duration = Duration::from_secs(10);

        container
            .event_tx
            .get()
            .map(|weak| weak.strong_count() > 0)
            .unwrap_or_else(|| {
                // There's a small window between registering the channel and creating the weak
                // reference during which the weak reference doesn't exist yet. We must ensure that
                // the reference has been registered within the allotted time before considering
                // the HandleContainer invalid.
                now.duration_since(container.created_at) <= REFERENCE_TIMEOUT
            })
    }

    /// Garbage collector thread.
    fn thread(me: Arc<PduHandleManager>) {
        let need_shrink = |cap: usize, len: usize| cap > (len * 2);

        loop {
            let now = Instant::now();

            {
                let mut apis = me.apis.write();
                apis.retain(|_, handle| Self::retain_handle_containers(&now, handle));
                if need_shrink(apis.capacity(), apis.len()) {
                    // Release resources back to the system.
                    apis.shrink_to_fit();
                }
            }
            {
                let mut mods = me.mods.write();
                mods.retain(|_, handle| Self::retain_handle_containers(&now, handle));
                if need_shrink(mods.capacity(), mods.len()) {
                    // Release resources back to the system.
                    mods.shrink_to_fit();
                }
            }
            {
                let mut clls = me.clls.write();
                clls.retain(|_, handle| Self::retain_handle_containers(&now, handle));
                if need_shrink(clls.capacity(), clls.len()) {
                    // Release resources back to the system.
                    clls.shrink_to_fit();
                }
            }
            {
                let mut cops = me.cops.write();
                cops.retain(|_, handle| Self::retain_handle_containers(&now, handle));
                if need_shrink(cops.capacity(), cops.len()) {
                    // Release resources back to the system.
                    cops.shrink_to_fit();
                }
            }

            sleep(Duration::from_secs(10));
        }
    }
}

/// Container for the module, logical link, and primitive.
///
/// I specifically separated T and its sender, since, for example, the creation and execution
/// of primitives may happen before the primitive creation function returns PduPrimitive and
/// begins routing events to its sender.
#[derive(Debug)]
pub(crate) struct HandleContainer<T> {
    reference: OnceLock<Weak<T>>,
    
    event_tx: OnceLock<mpsc::WeakUnboundedSender<PduEvent>>,

    created_at: Instant,
}