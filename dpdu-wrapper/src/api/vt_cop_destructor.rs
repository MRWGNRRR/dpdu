use crate::api::{ApiResult, PduApi};
use crate::types::pdu_status::PduStatusTarget;
use crate::types::{PduCllHandle, PduCopHandle, PduModuleHandle};
use dpdu_api_types::PduStatus;
use dpdu_api_types::bitflags::PduErrorFlag;

impl PduApi {
    pub fn vt_cop_destructor(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        h_cop: PduCopHandle,
    ) -> ApiResult<()> {
        self.modify_suppress_log_options(|options| {
            options.get_status = PduErrorFlag::INVALID_HANDLE;
        });

        let target = PduStatusTarget::Primitive(h_mod, h_cll, h_cop);
        let data = self.pdu_get_status(&target)?;

        match data.status_code {
            PduStatus::CopstWaiting | PduStatus::CopstIdle | PduStatus::CopstExecuting => {
                self.modify_suppress_log_options(|options| {
                    options.cancel_primitive = PduErrorFlag::INVALID_HANDLE;
                });
                self.pdu_cancel_com_primitive(h_mod, h_cll, h_cop)?;
            }
            _ => { /* same as above */ }
        }

        Ok(())
    }
}
