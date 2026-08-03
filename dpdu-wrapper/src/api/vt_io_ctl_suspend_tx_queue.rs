use crate::api::{ApiResult, PduApi};
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlTarget};
use crate::types::{PduCllHandle, PduModuleHandle};

impl PduApi {
    /// Suspend transmit queue of specific ComLogicalLink. The queue processing will be halted
    /// upon this command. This can be used to fill up a ComLogicalLink's queue with
    /// ComPrimitives to achieve a steady processing of ComPrimitives after resuming
    /// the queue (e.g. for fast flash programming operation).
    pub fn vt_io_ctl_suspend_tx_queue(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_SUSPEND_TX_QUEUE"),
            None,
        )?;
        Ok(())
    }
}
