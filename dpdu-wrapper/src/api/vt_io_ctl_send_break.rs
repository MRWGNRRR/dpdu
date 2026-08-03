use crate::api::{ApiResult, PduApi};
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlTarget};
use crate::types::{PduCllHandle, PduModuleHandle};

impl PduApi {
    pub fn vt_io_ctl_send_break(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_SEND_BREAK"),
            None,
        )?;
        Ok(())
    }
}
