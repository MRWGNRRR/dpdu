use crate::api::{ApiResult, PduApi};
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use crate::types::{PduCllHandle, PduModuleHandle};

impl PduApi {
    pub fn vt_io_ctl_set_buffer_size(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        size: u32,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_SET_BUFFER_SIZE"),
            Some(&PduIoCtlData::U32(size)),
        )?;
        Ok(())
    }
}
