use crate::api::{ApiResult, PduApi};
use crate::types::PduCopHandle;
use crate::types::pdu_error::{PduErrorData, PduLastErrorTarget};
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{PDU_HANDLE_UNDEF, PDU_ID_UNDEF, PduError, PduErrorEvt};
use std::mem::MaybeUninit;
use tracing::{error, trace};

impl PduApi {
    pub fn pdu_get_last_error(&self, target: &PduLastErrorTarget) -> ApiResult<PduErrorData> {
        impl_defer_clear_suppress_options!(self, get_last_error);

        const FUNC: &'static str = "PDUGetLastError";
        self.log_api_call(FUNC);

        let h_mod = target.get_module_handle().unwrap_or(PDU_HANDLE_UNDEF);
        let h_cll = target.get_cll_handle().unwrap_or(PDU_HANDLE_UNDEF);

        let mut error_code: MaybeUninit<u32> = MaybeUninit::uninit(); // will transform to PduErrorEvt
        let mut h_cop: MaybeUninit<PduCopHandle> = MaybeUninit::uninit(); // maybe undef
        let mut timestamp: MaybeUninit<u32> = MaybeUninit::uninit();
        let mut extra_info_code: MaybeUninit<u32> = MaybeUninit::uninit(); // maybe ID_UNDEF?

        trace!(
            func = FUNC,
            h_mod,
            h_cll,
            error_code_ptr = format!("{:#x}", error_code.as_ptr() as usize),
            h_cop_ptr = format!("{:#x}", h_cop.as_ptr() as usize),
            timestamp_ptr = format!("{:#x}", timestamp.as_ptr() as usize),
            extra_info_code_ptr = format!("{:#x}", extra_info_code.as_ptr() as usize),
            "D-PDU API Call Args"
        );

        let get_last_error_fn = self.symbols.get_last_error;
        let result = wrap_pdu_call(FUNC, || {
            get_last_error_fn(
                h_mod,
                h_cll,
                error_code.as_mut_ptr() as _,
                h_cop.as_mut_ptr(),
                timestamp.as_mut_ptr(),
                extra_info_code.as_mut_ptr(),
            )
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_last_error),
            );
            return Err(result)?;
        }

        let error_code = unsafe { error_code.assume_init() };
        let h_cop = unsafe { h_cop.assume_init() };
        let timestamp = unsafe { timestamp.assume_init() };
        let extra_info_code = unsafe { extra_info_code.assume_init() };

        trace!(
            func = FUNC,
            error_code, h_cop, timestamp, extra_info_code, "D-PDU API Call Return"
        );

        let error_event = match PduErrorEvt::try_from(error_code) {
            Ok(v) => v,
            Err(_) => {
                error!(
                    func = FUNC,
                    "Received out-of-bounds PduErrorEvt value: {:#x}. Emulation of PduError::FctFailed...",
                    error_code,
                );
                return Err(PduError::FctFailed)?;
            }
        };

        Ok(PduErrorData {
            target: target.clone(),
            error_event,
            h_cop: (h_cop != PDU_HANDLE_UNDEF).then(|| h_cop),
            timestamp,
            extra_info_code: (extra_info_code != PDU_ID_UNDEF).then(|| extra_info_code),
        })
    }
}
