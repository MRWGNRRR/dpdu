use crate::api::{ApiResult, PduApi};
use crate::types::pdu_lock_resource::PduLockResourceMask;
use crate::types::{PduCllHandle, PduModuleHandle};
use crate::vendor_specific::wrap_pdu_call;
use tracing::trace;

impl PduApi {
    pub fn pdu_lock_resource(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        mask: PduLockResourceMask,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, lock_resource);

        const FUNC: &'static str = "PDULockResource";
        self.log_api_call(FUNC);

        let mask_data = mask.get_pdu_data();

        trace!(
            func = FUNC,
            h_mod,
            h_cll,
            mask = format!("0x{mask_data:#x}"),
            lock_physical_com_params = mask.lock_physical_com_params,
            lock_physical_transmit_queue = mask.lock_physical_transmit_queue,
            "D-PDU API Call Args"
        );

        let lock_resource_fn = self.symbols.lock_resource;
        let result = wrap_pdu_call(FUNC, || lock_resource_fn(h_mod, h_cll, mask_data));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, lock_resource),
            );
            return Err(result)?;
        }

        Ok(())
    }
}
