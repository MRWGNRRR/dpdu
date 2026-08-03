use crate::api::{ApiResult, PduApi};
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use crate::types::{PduCllHandle, PduModuleHandle};
use dpdu_api_types::IoFilterData;

impl PduApi {
    pub fn vt_io_ctl_start_msg_filter(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        filter: IoFilterData,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_START_MSG_FILTER"),
            Some(&PduIoCtlData::Filter(filter)),
        )?;
        Ok(())
    }
}
