use crate::api::{ApiResult, PduApi};
use crate::types::PduModuleHandle;
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::PDU_HANDLE_UNDEF;
use tracing::trace;

impl PduApi {
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
}
