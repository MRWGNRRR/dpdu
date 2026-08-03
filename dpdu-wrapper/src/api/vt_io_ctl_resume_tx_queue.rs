use crate::api::{ApiResult, PduApi};
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlTarget};
use crate::types::{PduCllHandle, PduModuleHandle};

impl PduApi {
    /// Resume transmit queue of specific ComLogicalLink. The queue processing
    /// will be started upon this command.
    pub fn vt_io_ctl_resume_tx_queue(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_RESUME_TX_QUEUE"),
            None,
        )?;
        Ok(())
    }
}
