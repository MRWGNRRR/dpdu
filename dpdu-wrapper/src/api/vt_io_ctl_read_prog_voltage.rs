use crate::api::{ApiResult, PduApi};
use crate::types::PduModuleHandle;
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use dpdu_api_types::PduError;
use tracing::error;

impl PduApi {
    pub fn vt_io_ctl_read_prog_voltage(&self, h_mod: PduModuleHandle) -> ApiResult<f32> {
        match self.pdu_io_ctl(
            &PduIoCtlTarget::Module(h_mod),
            &PduIoCtlCommand::from("PDU_IOCTL_READ_PROG_VOLTAGE"),
            None,
        )? {
            Some(PduIoCtlData::ProgVoltage(v)) => Ok(v.prog_voltage_mv as f32 / 1000.0),
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
