use crate::api::{ApiResult, PduApi};
use crate::types::{PduCllHandle, PduModuleHandle};
use crate::vendor_specific::wrap_pdu_call;
use tracing::trace;

impl PduApi {
    pub fn pdu_disconnect(&self, h_mod: PduModuleHandle, h_cll: PduCllHandle) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, disconnect);

        const FUNC: &'static str = "PDUDisconnect";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, "D-PDU API Call Args");

        let disconnect_fn = self.symbols.disconnect;
        let result = wrap_pdu_call(FUNC, || disconnect_fn(h_mod, h_cll));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, disconnect),
            );
            return Err(result)?;
        }

        Ok(())
    }
}
