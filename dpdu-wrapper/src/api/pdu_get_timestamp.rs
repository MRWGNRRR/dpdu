use crate::api::{ApiResult, PduApi};
use crate::types::PduModuleHandle;
use crate::vendor_specific::wrap_pdu_call;
use std::mem::MaybeUninit;
use tracing::trace;

impl PduApi {
    pub fn pdu_get_timestamp(&self, h_mod: PduModuleHandle) -> ApiResult<u32> {
        impl_defer_clear_suppress_options!(self, get_timestamp);

        const FUNC: &'static str = "PDUGetTimestamp";
        self.log_api_call(FUNC);

        let mut timestamp = MaybeUninit::uninit();

        trace!(
            func = FUNC,
            h_mod,
            timestamp_ptr = format!("{:#x}", timestamp.as_ptr() as usize),
            "D-PDU API Call Args"
        );

        let get_timestamp_fn = self.symbols.get_timestamp;
        let result = wrap_pdu_call(FUNC, || get_timestamp_fn(h_mod, timestamp.as_mut_ptr()));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_timestamp),
            );
            return Err(result)?;
        }

        let timestamp = unsafe { timestamp.assume_init() };

        trace!(func = FUNC, timestamp, "D-PDU API Call Return");

        Ok(timestamp)
    }
}
