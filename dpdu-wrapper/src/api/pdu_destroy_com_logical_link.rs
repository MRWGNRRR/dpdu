use crate::api::{ApiResult, PduApi};
use crate::types::{PduCllHandle, PduModuleHandle};
use crate::vendor_specific::wrap_pdu_call;
use tracing::trace;

impl PduApi {
    pub fn pdu_destroy_com_logical_link(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, destroy_logical_link);

        const FUNC: &'static str = "PDUDestroyComLogicalLink";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, "D-PDU API Call Args");

        let destroy_fn = self.symbols.destroy_logical_link;
        let result = wrap_pdu_call(FUNC, || destroy_fn(h_mod, h_cll));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, destroy_logical_link),
            );
            return Err(result)?;
        }

        Ok(())
    }
}
