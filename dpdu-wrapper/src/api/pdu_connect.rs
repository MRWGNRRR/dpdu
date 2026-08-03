use crate::api::{ApiResult, PduApi};
use crate::types::{PduCllHandle, PduModuleHandle};
use crate::vendor_specific::wrap_pdu_call;
use tracing::trace;

impl PduApi {
    pub fn pdu_connect(&self, h_mod: PduModuleHandle, h_cll: PduCllHandle) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, connect);

        const FUNC: &'static str = "PDUConnect";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, "D-PDU API Call Args");

        let connect_fn = self.symbols.connect;
        let result = wrap_pdu_call(FUNC, || connect_fn(h_mod, h_cll));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, connect),
            );
            return Err(result)?;
        }

        Ok(())
    }
}
