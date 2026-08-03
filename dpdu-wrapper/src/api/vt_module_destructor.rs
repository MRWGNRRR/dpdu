use crate::api::{ApiResult, PduApi};
use crate::types::PduModuleHandle;
use crate::types::pdu_event::PduEventTarget;
use crate::types::pdu_status::PduStatusTarget;
use dpdu_api_types::PduStatus;
use dpdu_api_types::bitflags::PduErrorFlag;

impl PduApi {
    pub(crate) fn vt_module_destructor(&self, h_mod: PduModuleHandle) -> ApiResult<()> {
        self.modify_suppress_log_options(|options| {
            options.get_status = PduErrorFlag::INVALID_HANDLE;
        });

        let target = PduStatusTarget::Module(h_mod);
        let data = self.pdu_get_status(&target)?;

        match data.status_code {
            PduStatus::ModstReady | PduStatus::ModstNotReady => {
                let _ = self.pdu_module_disconnect(Some(h_mod));
            }
            _ => {}
        }

        self.modify_suppress_log_options(|options| {
            options.register_event_callback = PduErrorFlag::MODULE_NOT_CONNECTED;
        });

        let _ = self.pdu_register_event_callback(&PduEventTarget::Module(h_mod), None)?;

        Ok(())
    }
}
