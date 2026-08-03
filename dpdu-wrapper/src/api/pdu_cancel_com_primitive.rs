use crate::api::{ApiResult, PduApi};
use crate::types::{PduCllHandle, PduCopHandle, PduModuleHandle};
use crate::vendor_specific::wrap_pdu_call;
use tracing::trace;

impl PduApi {
    pub fn pdu_cancel_com_primitive(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        h_cop: PduCopHandle,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, cancel_primitive);

        const FUNC: &'static str = "PDUCancelComPrimitive";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, h_cop, "D-PDU API Call Args");

        let cancel_com_primitive_fn = self.symbols.cancel_primitive;
        let result = wrap_pdu_call(FUNC, || cancel_com_primitive_fn(h_mod, h_cll, h_cop));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, cancel_primitive),
            );
            return Err(result)?;
        }

        Ok(())
    }
}
