use crate::types::pdu_com_param::single::bus_type::{
    CpBaudrate, CpBitSamplePoint, CpCanBaudrateRecord, CpCanFdBaudrate, CpCanFdBitSamplePoint,
    CpCanFdSyncJumpWidth, CpListenOnly, CpSamplesPerBit, CpSyncJumpWidth, CpTerminationType,
};
use crate::types::pdu_com_param::single::com::{
    CpCanFillerByte, CpCanFillerByteHandling, CpChangeSpeedCtrl, CpChangeSpeedMessage,
    CpChangeSpeedRate, CpChangeSpeedResCtrl, CpEnablePerformanceTest, CpLoopback,
    CpSendRemoteFrame, CpSwCanHighVoltage, CpTransmitIndEnable,
};
use crate::types::pdu_com_param::single::err_hdl::{CpRepeatReqCountApp, CpRepeatReqCountTrans};
use crate::types::pdu_com_param::single::timing::{
    CpChangeSpeedTxDelay, CpCyclicRespTimeout, CpP2Max, CpP2Min, CpP3Func, CpP3Phys,
};
use crate::types::pdu_com_param::single::unique::{
    CpCanPhysReqExtAddr, CpCanPhysReqFormat, CpCanPhysReqId, CpCanRespUudtExtAddr,
    CpCanRespUudtFormat, CpCanRespUudtId, CpEcuLayerShortName,
};
use crate::types::pdu_com_param::stack::ComParamDefinitionStack;
use crate::types::pdu_com_param::stack::application::RawCanApplicationStack;
use crate::types::pdu_com_param::stack::physical::DwCanStack;
use crate::types::pdu_com_param::stack::transport::RawCanTransportStack;
use crate::types::pdu_com_param::table::{
    ComParamDefinition, ComParamDefinitionSet, ComParamDefinitionTable,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ISO 11898 RAW.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCanStack {
    #[serde(flatten)]
    pub app_stack: RawCanApplicationStack,

    #[serde(flatten)]
    pub transport_stack: RawCanTransportStack,

    #[serde(flatten)]
    pub physical_stack: DwCanStack,
}

impl RawCanStack {
    /// Reference: https://www.scribd.com/document/945641234/Documentation-Pdu-API-Iso-11898-Raw
    pub fn default_app_stack() -> RawCanApplicationStack {
        RawCanApplicationStack {
            change_speed_ctrl: CpChangeSpeedCtrl::DISABLE,
            change_speed_message: CpChangeSpeedMessage::empty(),
            change_speed_rate: CpChangeSpeedRate::ZERO,
            change_speed_res_ctrl: CpChangeSpeedResCtrl::NOT_USED,
            change_speed_tx_delay: CpChangeSpeedTxDelay::ZERO,
            cyclic_resp_timeout: CpCyclicRespTimeout::ZERO,
            enable_performance_test: CpEnablePerformanceTest::DISABLE,
            loopback: CpLoopback::DISABLE,
            p2_max: CpP2Max::Millis(50),
            p2_min: CpP2Min::ZERO,
            p3_func: CpP3Func::ZERO,
            p3_phys: CpP3Phys::ZERO,
            repeat_req_count_app: CpRepeatReqCountApp::ZERO,
            sw_can_high_voltage: CpSwCanHighVoltage::DISABLE,
            transmit_ind_enable: CpTransmitIndEnable::DISABLE,
        }
    }

    /// Reference: https://www.scribd.com/document/945641234/Documentation-Pdu-API-Iso-11898-Raw
    pub fn default_transport_stack() -> RawCanTransportStack {
        RawCanTransportStack {
            can_filler_byte: CpCanFillerByte::ZERO,
            can_filler_byte_handling: CpCanFillerByteHandling::DISABLE,
            can_phys_req_ext_addr: CpCanPhysReqExtAddr::ZERO,
            can_phys_req_format: CpCanPhysReqFormat::NORMAL_SEGMENTED_11_BIT_WITH_FC,
            can_phys_req_id: CpCanPhysReqId(2016),
            can_resp_uudt_ext_addr: CpCanRespUudtExtAddr::ZERO,
            can_resp_uudt_format: CpCanRespUudtFormat::NORMAL_UNSEGMENTED_11_BIT,
            can_resp_uudt_id: CpCanRespUudtId(2024),
            repeat_req_count_trans: CpRepeatReqCountTrans::ZERO,
            send_remote_frame: CpSendRemoteFrame::DISABLE,
            ecu_layer_short_name: CpEcuLayerShortName::empty(),
        }
    }

    /// Reference: https://www.scribd.com/document/945641234/Documentation-Pdu-API-Iso-11898-Raw
    pub fn default_physical_stack() -> DwCanStack {
        DwCanStack {
            baudrate: CpBaudrate(500_000),
            canfd_baudrate: CpCanFdBaudrate::ZERO,
            canfd_bit_sample_point: CpCanFdBitSamplePoint(80),
            canfd_sync_jump_width: CpCanFdSyncJumpWidth(15),
            bit_sample_point: CpBitSamplePoint(80),
            can_baudrate_record: CpCanBaudrateRecord::empty(),
            listen_only: CpListenOnly::DISABLE,
            samples_per_bit: CpSamplesPerBit(0),
            sync_jump_width: CpSyncJumpWidth(15),
            termination_type: CpTerminationType::NO,
        }
    }
}

impl Default for RawCanStack {
    fn default() -> Self {
        Self {
            app_stack: Self::default_app_stack(),
            transport_stack: Self::default_transport_stack(),
            physical_stack: Self::default_physical_stack(),
        }
    }
}

impl ComParamDefinitionStack for RawCanStack {
    fn configure_from_serde_json_map(&mut self, map: &HashMap<String, serde_json::Value>) {
        let mut value =
            serde_json::to_value(&self).expect("internal error: cannot serialize RawCanStack"); // infallible

        let obj = value
            .as_object_mut()
            .expect("internal error: cannot represent RawCanStack as map"); // infallible

        for (k, v) in map {
            if !obj.contains_key(k) {
                continue;
            }
            obj.insert(k.clone(), v.clone());
        }

        let new_self: RawCanStack =
            serde_json::from_value(value).expect("internal error: cannot deserialize RawCanStack"); // infallible

        *self = new_self;
    }

    fn build_set(&self) -> ComParamDefinitionSet<ComParamDefinition> {
        let set = self.app_stack.build_set();

        set
            .merge(self.transport_stack.build_set())
            .merge(self.physical_stack.build_set());

        set
    }

    fn build_table(&self) -> ComParamDefinitionTable<ComParamDefinition> {
        let table = self.app_stack.build_table();

        table
            .merge(self.transport_stack.build_table())
            .merge(self.physical_stack.build_table());

        table
    }
}
