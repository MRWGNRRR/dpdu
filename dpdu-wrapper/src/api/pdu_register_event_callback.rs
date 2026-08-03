use crate::api::{ApiResult, PduApi};
use crate::types::pdu_event::PduEventTarget;
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{EventCallbackFn, PDU_HANDLE_UNDEF, PduError};
use tracing::trace;

impl PduApi {
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
}
