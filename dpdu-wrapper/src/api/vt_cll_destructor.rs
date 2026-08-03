use crate::api::{ApiResult, PduApi};
use crate::types::pdu_event::PduEventTarget;
use crate::types::pdu_status::PduStatusTarget;
use crate::types::{PduCllHandle, PduModuleHandle};
use dpdu_api_types::PduStatus;
use dpdu_api_types::bitflags::PduErrorFlag;

impl PduApi {
    pub fn vt_cll_destructor(&self, h_mod: PduModuleHandle, h_cll: PduCllHandle) -> ApiResult<()> {
        self.modify_suppress_log_options(|options| {
            options.get_status = PduErrorFlag::INVALID_HANDLE;
        });

        let target = PduStatusTarget::LogicalLink(h_mod, h_cll);
        let status = self
            .pdu_get_status(&target)
            .map(|v| v.status_code)
            .unwrap_or(PduStatus::CllstOffline); // for bad drivers

        match status {
            PduStatus::CllstOnline | PduStatus::CllstCommStarted => {
                let _ = self.pdu_disconnect(h_mod, h_cll);
            }
            _ => { /* same as above */ }
        }

        let _ = self.pdu_register_event_callback(&PduEventTarget::LogicalLink(h_mod, h_cll), None);
        let _ = self.pdu_destroy_com_logical_link(h_mod, h_cll);

        Ok(())
    }
}
