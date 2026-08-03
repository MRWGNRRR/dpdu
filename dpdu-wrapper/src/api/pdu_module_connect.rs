use crate::api::{ApiResult, PduApi};
use crate::types::PduModuleHandle;
use crate::vendor_specific::wrap_pdu_call;
use tracing::trace;

impl PduApi {
    pub fn pdu_module_connect(&self, h_mod: PduModuleHandle) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, module_connect);

        const FUNC: &'static str = "PDUModuleConnect";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, "D-PDU API Call Args");

        let module_connect_fn = self.symbols.module_connect;
        let result = wrap_pdu_call(FUNC, || module_connect_fn(h_mod));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, module_connect),
            );
            return Err(result)?;
        }

        Ok(())
    }
}
