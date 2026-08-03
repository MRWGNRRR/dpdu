use crate::api::{ApiResult, PduApi};
use dpdu_api_types::PduError;

impl PduApi {
    pub fn pdu_destruct(&self) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, destruct);

        const FUNC: &'static str = "PDUDestruct";
        self.log_api_call(FUNC);

        let destruct_fn = self.symbols.destruct;

        match destruct_fn() {
            PduError::StatusNoError | PduError::PduApiNotConstructed => Ok(()),
            v => {
                self.log_api_call_fail(
                    FUNC,
                    v,
                    None,
                    resolve_level_of_log_api_call_fail!(self, v, destruct),
                );
                Err(v)?
            }
        }
    }
}
