use crate::api::{ApiResult, PduApi};
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use crate::types::{PduCllHandle, PduModuleHandle};
use dpdu_api_types::{IoEventQueuePropertyData, PduQueueMode};

impl PduApi {
    pub fn vt_io_ctl_set_event_queue_properties(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        size: u32,
        mode: PduQueueMode,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::LogicalLink(h_mod, h_cll),
            &PduIoCtlCommand::from("PDU_IOCTL_SET_EVENT_QUEUE_PROPERTIES"),
            Some(&PduIoCtlData::EventQueueProperty(
                IoEventQueuePropertyData {
                    queue_size: size,
                    queue_mode: mode,
                },
            )),
        )?;
        Ok(())
    }
}
