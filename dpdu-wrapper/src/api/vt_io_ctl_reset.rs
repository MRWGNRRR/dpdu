use crate::api::{ApiResult, PduApi};
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlTarget};

impl PduApi {
    /// Reset specific MVCI protocol module.
    pub fn vt_io_ctl_reset(&self) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::System,
            &PduIoCtlCommand::from("PDU_IOCTL_RESET"),
            None,
        )?;

        Ok(())
    }
}
