use crate::api::{ApiResult, PduApi};
use crate::types::pdu_status::{PduStatusData, PduStatusTarget};
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{PDU_HANDLE_UNDEF, PduError, PduStatus};
use std::mem::MaybeUninit;
use tracing::{error, trace};

impl PduApi {
    pub(crate) fn pdu_get_status(&self, target: &PduStatusTarget) -> ApiResult<PduStatusData> {
        impl_defer_clear_suppress_options!(self, get_status);

        const FUNC: &'static str = "PDUGetStatus";
        self.log_api_call(FUNC);

        let h_mod = target.get_module_handle().unwrap_or(PDU_HANDLE_UNDEF);
        let h_cll = target.get_cll_handle().unwrap_or(PDU_HANDLE_UNDEF);
        let h_cop = target.get_cop_handle().unwrap_or(PDU_HANDLE_UNDEF);

        trace!(func = FUNC, h_mod, h_cll, h_cop, "D-PDU API Call Args");

        let mut status_code: MaybeUninit<u32> = MaybeUninit::uninit();
        let mut timestamp: MaybeUninit<u32> = MaybeUninit::uninit();
        let mut extra_info: MaybeUninit<u32> = MaybeUninit::uninit();

        let get_status_fn = self.symbols.get_status;
        let result = wrap_pdu_call(FUNC, || {
            get_status_fn(
                h_mod,
                h_cll,
                h_cop,
                status_code.as_mut_ptr() as _,
                timestamp.as_mut_ptr(),
                extra_info.as_mut_ptr(),
            )
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_status),
            );
            return Err(result)?;
        }

        let status_code = unsafe { status_code.assume_init() };
        let timestamp = unsafe { timestamp.assume_init() };
        let extra_info = unsafe { extra_info.assume_init() };

        trace!(
            func = FUNC,
            status_code, timestamp, extra_info, "D-PDU API Call Return"
        );

        let status_code = match PduStatus::try_from(status_code) {
            Ok(v) => v,
            Err(_) => {
                error!(
                    func = FUNC,
                    "Received out-of-bounds PduStatus value: {:#x}. Emulation of PduError::FctFailed...",
                    status_code,
                );
                return Err(PduError::FctFailed)?;
            }
        };

        Ok(PduStatusData {
            target: target.clone(),
            status_code,
            timestamp,
            extra_info,
        })
    }
}
