use crate::api::{ApiResult, PduApi};
use crate::types::PduModuleHandle;
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use dpdu_api_types::PduError;
use tracing::error;

impl PduApi {
    pub fn vt_io_ctl_read_ignition_sense_state(
        &self,
        h_mod: PduModuleHandle,
        pin: Option<u32>,
    ) -> ApiResult<bool> {
        match self.pdu_io_ctl(
            &PduIoCtlTarget::Module(h_mod),
            &PduIoCtlCommand::from("PDU_IOCTL_READ_IGNITION_SENSE_STATE"),
            Some(&PduIoCtlData::U32(pin.unwrap_or(0))),
        )? {
            Some(PduIoCtlData::U32(v)) => Ok(if v > 0 { true } else { false }),
            Some(_) => {
                error!(
                    h_mod,
                    "IoCtl output data is wrong. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
            None => {
                error!(
                    h_mod,
                    "IoCtl output data is null. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
        }
    }
}
