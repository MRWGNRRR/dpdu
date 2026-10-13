use crate::types::pdu_com_param::single::bus_type::{
    CpBaudrate, CpBitSamplePoint, CpCanBaudrateRecord, CpCanFdBaudrate, CpCanFdBitSamplePoint,
    CpCanFdSyncJumpWidth, CpListenOnly, CpSamplesPerBit, CpSyncJumpWidth, CpTerminationType,
};
use crate::types::pdu_com_param::single::com::{
    CpBlockSize, CpBlockSizeOverride, CpCanDataSizeOffset, CpCanFdTxMaxDataLength, CpCanFillerByte,
    CpCanFillerByteHandling, CpCanFirstConsecutiveFrameValue, CpCanFuncReqExtAddr,
    CpCanFuncReqFormat, CpCanFuncReqId, CpCanMaxNumWaitFrames, CpChangeSpeedCtrl,
    CpChangeSpeedMessage, CpChangeSpeedRate, CpChangeSpeedResCtrl, CpEnablePerformanceTest,
    CpLoopback, CpMaxFirstFrameDataLength, CpRequestAddrMode, CpSendRemoteFrame,
    CpStartMsgIndEnable, CpSwCanHighVoltage, CpTransmitIndEnable,
};
use crate::types::pdu_com_param::single::err_hdl::{
    CpRc21Completiontimeout, CpRc21Handling, CpRc21RequestTime, CpRc23Completiontimeout,
    CpRc23Handling, CpRc23RequestTime, CpRc78Completiontimeout, CpRc78Handling, CpRcByteOffset,
    CpRepeatReqCountApp, CpRepeatReqCountTrans, CpSuspendQueueOnError,
};
use crate::types::pdu_com_param::single::tester_present::{
    CpTesterPresentAddrMode, CpTesterPresentExpNegResp, CpTesterPresentExpPosResp,
    CpTesterPresentHandling, CpTesterPresentMessage, CpTesterPresentReqRsp,
    CpTesterPresentSendType, CpTesterPresentTime,
};
use crate::types::pdu_com_param::single::timing::{
    CpAr, CpAs, CpBr, CpBs, CpCanTransmissionTime, CpChangeSpeedTxDelay, CpCr, CpCs,
    CpCyclicRespTimeout, CpModifyTiming, CpP2Max, CpP2Min, CpP2Star, CpP3Func, CpP3Phys,
    CpSessionTimingOverride, CpStMin, CpStMinOverride,
};
use crate::types::pdu_com_param::single::unique::{
    CpCanPhysReqExtAddr, CpCanPhysReqFormat, CpCanPhysReqId, CpCanRespUsdtExtAddr,
    CpCanRespUsdtFormat, CpCanRespUsdtId, CpCanRespUudtExtAddr, CpCanRespUudtFormat,
    CpCanRespUudtId, CpEcuLayerShortName,
};
use crate::types::pdu_com_param::stack::ComParamDefinitionStack;
use crate::types::pdu_com_param::stack::application::UdsStack;
use crate::types::pdu_com_param::stack::physical::DwCanStack;
use crate::types::pdu_com_param::stack::transport::IsoTpStack;
use crate::types::pdu_com_param::table::{
    ComParamDefinition, ComParamDefinitionSet, ComParamDefinitionTable,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Layers:
///   - Application: ISO 15765-3 (UDS)
///   - Transport (Bus): ISO 15765-2 (ISO-TP)
///   - Physical (Can): ISO 11898-2 (DW CAN)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdsOnIsoTpOnDwCanStack {
    #[serde(flatten)]
    pub app_stack: UdsStack,

    #[serde(flatten)]
    pub transport_stack: IsoTpStack,

    #[serde(flatten)]
    pub physical_stack: DwCanStack,
}

impl UdsOnIsoTpOnDwCanStack {
    /// Reference: https://www.scribd.com/document/945641241/Documentation-PDU-API-ISO-15765-3-on-ISO-15765-2
    pub fn default_app_stack() -> UdsStack {
        UdsStack {
            can_transmission_time: CpCanTransmissionTime::ZERO,
            change_speed_ctrl: CpChangeSpeedCtrl::DISABLE,
            change_speed_message: CpChangeSpeedMessage::empty(),
            change_speed_rate: CpChangeSpeedRate::ZERO,
            change_speed_res_ctrl: CpChangeSpeedResCtrl::NOT_USED,
            change_speed_tx_delay: CpChangeSpeedTxDelay::ZERO,
            cyclic_resp_timeout: CpCyclicRespTimeout::ZERO,
            enable_performance_test: CpEnablePerformanceTest::DISABLE,
            loopback: CpLoopback::DISABLE,
            modify_timing: CpModifyTiming::DISABLE,

            p2_max: CpP2Max::Millis(150),
            p2_min: CpP2Min::Millis(25),
            p2_star: CpP2Star::Millis(5050),
            p3_func: CpP3Func::Millis(150),
            p3_phys: CpP3Phys::Millis(150),

            rc21_completion_timeout: CpRc21Completiontimeout::Millis(1300),
            rc21_handling: CpRc21Handling::DISABLE,
            rc21_request_time: CpRc21RequestTime::Millis(10),
            rc23_completion_timeout: CpRc23Completiontimeout::ZERO,
            rc23_handling: CpRc23Handling::DISABLE,
            rc23_request_time: CpRc23RequestTime::ZERO,
            rc78_completion_timeout: CpRc78Completiontimeout::Secs(25),
            rc78_handling: CpRc78Handling::CONTINUE_UNLIMITED,
            rc_byte_offset: CpRcByteOffset::LAST_BYTE,

            repeat_req_count_app: CpRepeatReqCountApp::ZERO,
            session_timing_override: CpSessionTimingOverride::empty(),
            start_msg_ind_enable: CpStartMsgIndEnable::DISABLE,
            suspend_queue_on_error: CpSuspendQueueOnError::DISABLE,
            sw_can_high_voltage: CpSwCanHighVoltage::DISABLE,

            tester_present_addr_mode: CpTesterPresentAddrMode::PHYSICAL, // patch to avoid conflicts
            tester_present_exp_neg_resp: CpTesterPresentExpNegResp::empty(),
            tester_present_exp_pos_resp: CpTesterPresentExpPosResp::empty(),
            tester_present_handling: CpTesterPresentHandling::ENABLE,
            tester_present_message: CpTesterPresentMessage(vec![0x3E, 0x80]),
            tester_present_req_rsp: CpTesterPresentReqRsp::NO_RESPONSE,
            tester_present_send_type: CpTesterPresentSendType::ON_IDLE, // patched to avoid conflicts
            tester_present_time: CpTesterPresentTime::Secs(2),

            transmit_ind_enable: CpTransmitIndEnable::DISABLE,
        }
    }

    /// Reference: https://www.scribd.com/document/945641241/Documentation-PDU-API-ISO-15765-3-on-ISO-15765-2
    pub fn default_transport_stack() -> IsoTpStack {
        IsoTpStack {
            ar: CpAr::Secs(10),
            r#as: CpAs::Secs(10),
            block_size: CpBlockSize::ZERO,
            block_size_override: CpBlockSizeOverride(65535),
            br: CpBr::ZERO,
            bs: CpBs::Secs(10),
            can_data_size_offset: CpCanDataSizeOffset(10),
            can_filler_byte: CpCanFillerByte(0x55),
            can_filler_byte_handling: CpCanFillerByteHandling::ENABLE,
            can_first_consecutive_frame_value: CpCanFirstConsecutiveFrameValue(1),
            canfd_tx_max_data_length: CpCanFdTxMaxDataLength(0),
            can_func_req_ext_addr: CpCanFuncReqExtAddr(0),
            can_func_req_format: CpCanFuncReqFormat::NORMAL_SEGMENTED_11_BIT_WITH_FC,
            can_func_req_id: CpCanFuncReqId(2015),
            can_max_num_wait_frames: CpCanMaxNumWaitFrames(255),
            can_phys_req_ext_addr: CpCanPhysReqExtAddr::ZERO,
            can_phys_req_format: CpCanPhysReqFormat::NORMAL_SEGMENTED_11_BIT_WITH_FC,
            can_phys_req_id: CpCanPhysReqId(2016),
            can_resp_usdt_ext_addr: CpCanRespUsdtExtAddr::ZERO,
            can_resp_usdt_format: CpCanRespUsdtFormat::NORMAL_UNSEGMENTED_11_BIT_WITH_FC,
            can_resp_usdt_id: CpCanRespUsdtId(2024),
            can_resp_uudt_ext_addr: CpCanRespUudtExtAddr::ZERO,
            can_resp_uudt_format: CpCanRespUudtFormat::NORMAL_UNSEGMENTED_11_BIT,
            can_resp_uudt_id: CpCanRespUudtId::NOT_USED,
            cr: CpCr::Secs(10),
            cs: CpCs::ZERO,
            ecu_layer_short_name: CpEcuLayerShortName::empty(),
            max_first_frame_data_length: CpMaxFirstFrameDataLength(4095),
            repeat_req_count_trans: CpRepeatReqCountTrans::ZERO,
            request_addr_mode: CpRequestAddrMode::FUNCTIONAL,
            send_remote_frame: CpSendRemoteFrame::DISABLE,
            st_min: CpStMin::ZERO,
            st_min_override: CpStMinOverride::NOT_USED,
        }
    }

    /// Reference: https://www.scribd.com/document/945641241/Documentation-PDU-API-ISO-15765-3-on-ISO-15765-2
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

    /// Reference: CPC_NG.smr-d.
    pub fn mercedes_benz_optimized(phys_req_id: Option<u32>, resp_usdt_id: Option<u32>) -> Self {
        Self {
            app_stack: UdsStack {
                can_transmission_time: CpCanTransmissionTime::ZERO,
                change_speed_ctrl: CpChangeSpeedCtrl::DISABLE,
                change_speed_message: CpChangeSpeedMessage::empty(),
                change_speed_rate: CpChangeSpeedRate::ZERO,
                change_speed_res_ctrl: CpChangeSpeedResCtrl::NOT_USED,
                change_speed_tx_delay: CpChangeSpeedTxDelay::ZERO,
                cyclic_resp_timeout: CpCyclicRespTimeout::ZERO,
                enable_performance_test: CpEnablePerformanceTest::DISABLE,
                loopback: CpLoopback::DISABLE,
                modify_timing: CpModifyTiming::DISABLE,

                p2_max: CpP2Max::Millis(2100),
                p2_min: CpP2Min::Millis(0),
                p2_star: CpP2Star::Millis(4500),
                p3_func: CpP3Func::Millis(50),
                p3_phys: CpP3Phys::Millis(50),

                rc21_completion_timeout: CpRc21Completiontimeout::Secs(6),
                rc21_handling: CpRc21Handling::CONTINUE_UNTIL_RC21_TIMEOUT,
                rc21_request_time: CpRc21RequestTime::Millis(200),
                rc23_completion_timeout: CpRc23Completiontimeout::ZERO,
                rc23_handling: CpRc23Handling::DISABLE,
                rc23_request_time: CpRc23RequestTime::Millis(200),
                rc78_completion_timeout: CpRc78Completiontimeout::Secs(60),
                rc78_handling: CpRc78Handling::CONTINUE_UNLIMITED,
                rc_byte_offset: CpRcByteOffset::LAST_BYTE,

                repeat_req_count_app: CpRepeatReqCountApp(3),
                session_timing_override: CpSessionTimingOverride::empty(),
                start_msg_ind_enable: CpStartMsgIndEnable::DISABLE,
                suspend_queue_on_error: CpSuspendQueueOnError::DISABLE,
                sw_can_high_voltage: CpSwCanHighVoltage::DISABLE,

                tester_present_addr_mode: CpTesterPresentAddrMode::PHYSICAL,
                tester_present_exp_neg_resp: CpTesterPresentExpNegResp(vec![0x7F, 0x3E]),
                tester_present_exp_pos_resp: CpTesterPresentExpPosResp(vec![0x7E, 0x00]),
                tester_present_handling: CpTesterPresentHandling::ENABLE,
                tester_present_message: CpTesterPresentMessage(vec![0x3E, 0x00]),
                tester_present_req_rsp: CpTesterPresentReqRsp::RESPONSE_EXPECTED,
                tester_present_send_type: CpTesterPresentSendType::ON_IDLE,
                tester_present_time: CpTesterPresentTime::Secs(2),

                transmit_ind_enable: CpTransmitIndEnable::DISABLE,
            },
            transport_stack: IsoTpStack {
                ar: CpAr::Secs(1),
                r#as: CpAs::Secs(1),
                block_size: CpBlockSize(8),
                block_size_override: CpBlockSizeOverride(65535),
                br: CpBr::ZERO,
                bs: CpBs::Secs(2),
                can_data_size_offset: CpCanDataSizeOffset(0),
                can_filler_byte: CpCanFillerByte(0x55),
                can_filler_byte_handling: CpCanFillerByteHandling::ENABLE,
                can_first_consecutive_frame_value: CpCanFirstConsecutiveFrameValue(1),
                canfd_tx_max_data_length: CpCanFdTxMaxDataLength(0),
                can_func_req_ext_addr: CpCanFuncReqExtAddr(0),
                can_func_req_format: CpCanFuncReqFormat::NORMAL_SEGMENTED_11_BIT_WITH_FC,
                can_func_req_id: CpCanFuncReqId(1089),
                can_max_num_wait_frames: CpCanMaxNumWaitFrames(255),
                can_phys_req_ext_addr: CpCanPhysReqExtAddr::ZERO,
                can_phys_req_format: CpCanPhysReqFormat::NORMAL_SEGMENTED_11_BIT_WITH_FC,
                can_phys_req_id: CpCanPhysReqId(phys_req_id.unwrap_or(2021)),
                can_resp_usdt_ext_addr: CpCanRespUsdtExtAddr::ZERO,
                can_resp_usdt_format: CpCanRespUsdtFormat::NORMAL_UNSEGMENTED_11_BIT_WITH_FC,
                can_resp_usdt_id: CpCanRespUsdtId(resp_usdt_id.unwrap_or(2029)),
                can_resp_uudt_ext_addr: CpCanRespUudtExtAddr::ZERO,
                can_resp_uudt_format: CpCanRespUudtFormat::NORMAL_UNSEGMENTED_11_BIT,
                can_resp_uudt_id: CpCanRespUudtId::NOT_USED,
                cr: CpCr::Secs(2),
                cs: CpCs::ZERO,
                ecu_layer_short_name: CpEcuLayerShortName::empty(),
                max_first_frame_data_length: CpMaxFirstFrameDataLength(4095),
                repeat_req_count_trans: CpRepeatReqCountTrans::ZERO,
                request_addr_mode: CpRequestAddrMode::PHYSICAL,
                send_remote_frame: CpSendRemoteFrame::DISABLE,
                st_min: CpStMin::ZERO,
                st_min_override: CpStMinOverride::NOT_USED,
            },
            physical_stack: DwCanStack {
                baudrate: CpBaudrate(500_000),
                canfd_baudrate: CpCanFdBaudrate::ZERO,
                canfd_bit_sample_point: CpCanFdBitSamplePoint(80),
                canfd_sync_jump_width: CpCanFdSyncJumpWidth(15),
                bit_sample_point: CpBitSamplePoint(80),
                can_baudrate_record: CpCanBaudrateRecord(vec![500_000, 250_000]),
                listen_only: CpListenOnly::DISABLE,
                samples_per_bit: CpSamplesPerBit::ONE_SAMPLE,
                sync_jump_width: CpSyncJumpWidth(15),
                termination_type: CpTerminationType::NO,
            },
        }
    }
}

impl Default for UdsOnIsoTpOnDwCanStack {
    fn default() -> Self {
        Self {
            app_stack: Self::default_app_stack(),
            transport_stack: Self::default_transport_stack(),
            physical_stack: Self::default_physical_stack(),
        }
    }
}

impl ComParamDefinitionStack for UdsOnIsoTpOnDwCanStack {
    fn configure_from_serde_json_map(&mut self, map: &HashMap<String, serde_json::Value>) {
        let mut value = serde_json::to_value(&self)
            .expect("internal error: cannot serialize UdsOnIsoTpOnDwCanStack"); // infallible

        let obj = value
            .as_object_mut()
            .expect("internal error: cannot represent UdsOnIsoTpOnDwCanStack as map"); // infallible

        for (k, v) in map {
            if !obj.contains_key(k) {
                continue;
            }
            obj.insert(k.clone(), v.clone());
        }

        let new_self: UdsOnIsoTpOnDwCanStack = serde_json::from_value(value)
            .expect("internal error: cannot deserialize UdsOnIsoTpOnDwCanStack"); // infallible

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
