use crate::api::{ApiResult, PduApi};
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use crate::types::{PduCllHandle, PduModuleHandle};

impl PduApi {
    pub fn vt_io_ctl_stop_msg_filter(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        number: u32,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_STOP_MSG_FILTER"),
            Some(&PduIoCtlData::U32(number)),
        )?;
        Ok(())
    }
}
