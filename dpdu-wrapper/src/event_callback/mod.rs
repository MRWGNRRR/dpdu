use crate::handle_manager::PduHandleManager;
use crate::types::pdu_event::{PduEvent, PduEventTarget};
use crate::types::{PduCllHandle, PduModuleHandle, PduUniqueApiTag, PduUniqueCllTag};
use dpdu_api_types::PduEvtData;
use dpdu_api_types::bitflags::PduErrorFlag;
use std::ffi::c_void;
use tracing::{debug, error, trace, warn};

pub(crate) unsafe extern "system-unwind" fn event_callback(
    _event_type: PduEvtData,
    h_mod: PduModuleHandle,
    h_cll: PduCllHandle,
    p_cll_tag: *mut c_void,
    p_api_tag: *mut c_void,
) {
    let api = match PduUniqueApiTag::new(p_api_tag as _) {
        Some(api_tag) => PduHandleManager::lookup_api_reference(api_tag),
        None => PduHandleManager::get_single_api(),
    };

    let Some(api) = api else {
        error!(
            api_tag = p_api_tag as usize,
            h_mod, h_cll, "PDUEventCallback: there were no suitable APIs for the event"
        );
        return;
    };

    let event_target = PduEventTarget::from_callback(h_mod, h_cll);

    let mut events: Vec<PduEvent> = vec![];

    loop {
        api.modify_suppress_log_options(|options| {
            options.get_event_item = PduErrorFlag::INVALID_HANDLE;
        });

        match api.pdu_get_event_item(&event_target) {
            Ok(Some(v)) => events.push(v),
            Ok(None) => {
                break;
            }
            Err(err) => {
                if event_target.is_logical_link() {
                    // I'm suppressing this because the ComLogicalLink handle can be destroyed,
                    // which will give difficult triggers.
                    //
                    // This may not be the best solution and I should have a list of deleted
                    // ComLogicalLinks, but I'll leave it that way.
                } else {
                    error!(
                        api_tag = api.get_unique_tag(),
                        h_mod, h_cll, "PDUEventCallback: error reading an event: {err}"
                    );
                }

                break;
            }
        }
    }

    for event in events {
        trace!("PDUEventCallback: {event:?}");

        match event.target {
            PduEventTarget::System => {
                // System event.
                let Some(api_event_tx) =
                    PduHandleManager::lookup_api_event_tx(api.get_unique_tag())
                else {
                    warn!(
                        h_mod,
                        "PDUEventCallback: unable to lookup event_tx for the PduApi"
                    );
                    continue;
                };

                if api_event_tx.send(event).is_err() {
                    error!(
                        h_mod,
                        "PDUEventCallback: unable to send D-PDU event to the PduApi"
                    );
                }
            }
            PduEventTarget::Module(h_mod) => {
                // Module event.
                let Some(module_event_tx) =
                    PduHandleManager::lookup_module_event_tx(api.get_unique_tag(), h_mod)
                else {
                    warn!(
                        h_mod,
                        "PDUEventCallback: unable to lookup event_tx for the PduVci"
                    );
                    continue;
                };

                if module_event_tx.send(event).is_err() {
                    error!(
                        h_mod,
                        "PDUEventCallback: unable to send D-PDU event to the PduVci"
                    );
                }
            }
            PduEventTarget::LogicalLink(h_mod, h_cll) => {
                let Some(cll_tag) = PduUniqueCllTag::new(p_cll_tag as _) else {
                    warn!(
                        "PDUEventCallback: abnormally CLL creation: cll_tag is required when PduEventTarget = ComLogicalLink"
                    );
                    continue;
                };

                if let Some(h_cop) = event.h_cop {
                    // ComPrimitive event.
                    let Some(cop_tag) = event.cop_tag else {
                        warn!(
                            h_cop,
                            "PDUEventCallback: API does not provide a primitive tag"
                        );
                        continue;
                    };

                    let Some(cop_event_tx) =
                        PduHandleManager::lookup_cop_event_tx(api.get_unique_tag(), cop_tag)
                    else {
                        // I'm decreasing log level because the ComPrimitive handle can
                        // be destroyed, which will give difficult triggers.
                        //
                        // This may not be the best solution and I should have a list of deleted
                        // ComPrimitives, but I'll leave it that way.
                        debug!(
                            h_cll,
                            tag = cll_tag,
                            "PDUEventCallback: unable to lookup event_tx for the PduComPrimitive"
                        );
                        continue;
                    };

                    if cop_event_tx.send(event).is_err() {
                        error!(
                            h_mod,
                            h_cll,
                            h_cop,
                            "PDUEventCallback: unable to send D-PDU event to the PduComPrimitive"
                        );
                    }
                } else {
                    // ComLogicalLink event.
                    let Some(cll_event_tx) =
                        PduHandleManager::lookup_cll_event_tx(api.get_unique_tag(), cll_tag)
                    else {
                        // I'm decreasing log level because the ComLogicalLink handle can
                        // be destroyed, which will give difficult triggers.
                        //
                        // This may not be the best solution and I should have a list of deleted
                        // ComLogicalLinks, but I'll leave it that way.
                        debug!(
                            h_cll,
                            tag = cll_tag,
                            "PDUEventCallback: unable to lookup event_tx for the PduLogicalLink"
                        );
                        continue;
                    };

                    if cll_event_tx.send(event).is_err() {
                        error!(
                            h_mod,
                            h_cll,
                            "PDUEventCallback: unable to send D-PDU event to the PduLogicalLink"
                        );
                    }
                }
            }
        }
    }
}
