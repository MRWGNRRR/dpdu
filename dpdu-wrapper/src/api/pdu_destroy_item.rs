use crate::api::{ApiResult, PduApi};
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::PduItem;
use tracing::trace;

impl PduApi {
    pub fn pdu_destroy_item(&self, item_ptr: *mut PduItem) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, destroy_item);

        const FUNC: &'static str = "PDUDestroyItem";
        self.log_api_call(FUNC);

        trace!(
            func = FUNC,
            p_item = format!("0x{:x}", item_ptr as usize),
            "D-PDU API Call Args"
        );

        if item_ptr.is_null() {
            return Ok(());
        }

        let destroy_item_fn = self.symbols.destroy_item;
        let result = wrap_pdu_call(FUNC, || destroy_item_fn(item_ptr));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, destroy_item),
            );
            return Err(result)?;
        }

        Ok(())
    }
}
