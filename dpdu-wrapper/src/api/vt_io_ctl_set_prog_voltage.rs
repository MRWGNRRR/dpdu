use crate::api::{ApiResult, PduApi};
use crate::types::PduModuleHandle;
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use dpdu_api_types::IoProgVoltageData;

impl PduApi {
    pub fn vt_io_ctl_set_prog_voltage(
        &self,
        h_mod: PduModuleHandle,
        voltage: f32,
        pin: u32,
    ) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::Module(h_mod),
            &PduIoCtlCommand::from("PDU_IOCTL_SET_PROG_VOLTAGE"),
            Some(&PduIoCtlData::ProgVoltage(IoProgVoltageData {
                prog_voltage_mv: (voltage * 1000.0) as u32,
                pin_on_dlc: pin,
            })),
        )?;
        Ok(())
    }
}
