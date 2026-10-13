use crate::types::pdu_com_param::single::com::{
    CpChangeSpeedCtrl, CpChangeSpeedMessage, CpChangeSpeedRate, CpChangeSpeedResCtrl,
    CpEnablePerformanceTest, CpLoopback, CpStartMsgIndEnable, CpSwCanHighVoltage,
    CpTransmitIndEnable,
};
use crate::types::pdu_com_param::single::err_hdl::{
    CpRc21Completiontimeout, CpRc21Handling, CpRc21RequestTime, CpRc23Completiontimeout,
    CpRc23Handling, CpRc23RequestTime, CpRc78Completiontimeout, CpRc78Handling, CpRcByteOffset,
    CpRepeatReqCountApp, CpSuspendQueueOnError,
};
use crate::types::pdu_com_param::single::tester_present::{
    CpTesterPresentAddrMode, CpTesterPresentExpNegResp, CpTesterPresentExpPosResp,
    CpTesterPresentHandling, CpTesterPresentMessage, CpTesterPresentReqRsp,
    CpTesterPresentSendType, CpTesterPresentTime,
};
use crate::types::pdu_com_param::single::timing::{
    CpCanTransmissionTime, CpChangeSpeedTxDelay, CpCyclicRespTimeout, CpModifyTiming, CpP2Max,
    CpP2Min, CpP2Star, CpP3Func, CpP3Phys, CpSessionTimingOverride,
};
use crate::types::pdu_com_param::stack::ComParamDefinitionStack;
use crate::types::pdu_com_param::table::{
    ComParamDefinition, ComParamDefinitionSet, ComParamDefinitionTable,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use crate::{com_param_definition_set, com_param_set};

/// UDS application stack (ISO 14229-3).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UdsStack {
    #[serde(rename = "CP_CanTransmissionTime")]
    pub can_transmission_time: CpCanTransmissionTime,

    #[serde(rename = "CP_ChangeSpeedCtrl")]
    pub change_speed_ctrl: CpChangeSpeedCtrl,

    #[serde(rename = "CP_ChangeSpeedMessage")]
    pub change_speed_message: CpChangeSpeedMessage,

    #[serde(rename = "CP_ChangeSpeedRate")]
    pub change_speed_rate: CpChangeSpeedRate,

    #[serde(rename = "CP_ChangeSpeedResCtrl")]
    pub change_speed_res_ctrl: CpChangeSpeedResCtrl,

    #[serde(rename = "CP_ChangeSpeedTxDelay")]
    pub change_speed_tx_delay: CpChangeSpeedTxDelay,

    #[serde(rename = "CP_CyclicRespTimeout")]
    pub cyclic_resp_timeout: CpCyclicRespTimeout,

    #[serde(rename = "CP_EnablePerformanceTest")]
    pub enable_performance_test: CpEnablePerformanceTest,

    #[serde(rename = "CP_Loopback")]
    pub loopback: CpLoopback,

    #[serde(rename = "CP_ModifyTiming")]
    pub modify_timing: CpModifyTiming,

    #[serde(rename = "CP_P2Max")]
    pub p2_max: CpP2Max,

    #[serde(rename = "CP_P2Min")]
    pub p2_min: CpP2Min,

    #[serde(rename = "CP_P2Star")]
    pub p2_star: CpP2Star,

    #[serde(rename = "CP_P3Func")]
    pub p3_func: CpP3Func,

    #[serde(rename = "CP_P3Phys")]
    pub p3_phys: CpP3Phys,

    #[serde(rename = "CP_RC21CompletionTimeout")]
    pub rc21_completion_timeout: CpRc21Completiontimeout,

    #[serde(rename = "CP_RC21Handling")]
    pub rc21_handling: CpRc21Handling,

    #[serde(rename = "CP_RC21RequestTime")]
    pub rc21_request_time: CpRc21RequestTime,

    #[serde(rename = "CP_RC23CompletionTimeout")]
    pub rc23_completion_timeout: CpRc23Completiontimeout,

    #[serde(rename = "CP_RC23Handling")]
    pub rc23_handling: CpRc23Handling,

    #[serde(rename = "CP_RC23RequestTime")]
    pub rc23_request_time: CpRc23RequestTime,

    #[serde(rename = "CP_RC78CompletionTimeout")]
    pub rc78_completion_timeout: CpRc78Completiontimeout,

    #[serde(rename = "CP_RC78Handling")]
    pub rc78_handling: CpRc78Handling,

    #[serde(rename = "CP_RCByteOffset")]
    pub rc_byte_offset: CpRcByteOffset,

    #[serde(rename = "CP_RepeatReqCountApp")]
    pub repeat_req_count_app: CpRepeatReqCountApp,

    #[serde(rename = "CP_SessionTimingOverride")]
    pub session_timing_override: CpSessionTimingOverride,

    #[serde(rename = "CP_StartMsgIndEnable")]
    pub start_msg_ind_enable: CpStartMsgIndEnable,

    #[serde(rename = "CP_SuspendQueueOnError")]
    pub suspend_queue_on_error: CpSuspendQueueOnError,

    #[serde(rename = "CP_SwCan_HighVoltage")]
    pub sw_can_high_voltage: CpSwCanHighVoltage,

    #[serde(rename = "CP_TesterPresentAddrMode")]
    pub tester_present_addr_mode: CpTesterPresentAddrMode,

    #[serde(rename = "CP_TesterPresentExpNegResp")]
    pub tester_present_exp_neg_resp: CpTesterPresentExpNegResp,

    #[serde(rename = "CP_TesterPresentExpPosResp")]
    pub tester_present_exp_pos_resp: CpTesterPresentExpPosResp,

    #[serde(rename = "CP_TesterPresentHandling")]
    pub tester_present_handling: CpTesterPresentHandling,

    #[serde(rename = "CP_TesterPresentMessage")]
    pub tester_present_message: CpTesterPresentMessage,

    #[serde(rename = "CP_TesterPresentReqRsp")]
    pub tester_present_req_rsp: CpTesterPresentReqRsp,

    #[serde(rename = "CP_TesterPresentSendType")]
    pub tester_present_send_type: CpTesterPresentSendType,

    #[serde(rename = "CP_TesterPresentTime")]
    pub tester_present_time: CpTesterPresentTime,

    #[serde(rename = "CP_TransmitIndEnable")]
    pub transmit_ind_enable: CpTransmitIndEnable,
}

impl UdsStack {
    pub fn set_can_transmission_time(
        &mut self,
        value: impl Into<CpCanTransmissionTime>,
    ) -> &mut Self {
        self.can_transmission_time = value.into();
        self
    }

    pub fn set_change_speed_ctrl(&mut self, value: impl Into<CpChangeSpeedCtrl>) -> &mut Self {
        self.change_speed_ctrl = value.into();
        self
    }

    pub fn set_change_speed_message(
        &mut self,
        value: impl Into<CpChangeSpeedMessage>,
    ) -> &mut Self {
        self.change_speed_message = value.into();
        self
    }

    pub fn set_change_speed_rate(&mut self, value: impl Into<CpChangeSpeedRate>) -> &mut Self {
        self.change_speed_rate = value.into();
        self
    }

    pub fn set_change_speed_res_ctrl(
        &mut self,
        value: impl Into<CpChangeSpeedResCtrl>,
    ) -> &mut Self {
        self.change_speed_res_ctrl = value.into();
        self
    }

    pub fn set_change_speed_tx_delay(
        &mut self,
        value: impl Into<CpChangeSpeedTxDelay>,
    ) -> &mut Self {
        self.change_speed_tx_delay = value.into();
        self
    }

    pub fn set_cyclic_resp_timeout(&mut self, value: impl Into<CpCyclicRespTimeout>) -> &mut Self {
        self.cyclic_resp_timeout = value.into();
        self
    }

    pub fn set_enable_performance_test(
        &mut self,
        value: impl Into<CpEnablePerformanceTest>,
    ) -> &mut Self {
        self.enable_performance_test = value.into();
        self
    }

    pub fn set_loopback(&mut self, value: impl Into<CpLoopback>) -> &mut Self {
        self.loopback = value.into();
        self
    }

    pub fn set_modify_timing(&mut self, value: impl Into<CpModifyTiming>) -> &mut Self {
        self.modify_timing = value.into();
        self
    }

    pub fn set_p2_max(&mut self, value: impl Into<CpP2Max>) -> &mut Self {
        self.p2_max = value.into();
        self
    }

    pub fn set_p2_min(&mut self, value: impl Into<CpP2Min>) -> &mut Self {
        self.p2_min = value.into();
        self
    }

    pub fn set_p2_star(&mut self, value: impl Into<CpP2Star>) -> &mut Self {
        self.p2_star = value.into();
        self
    }

    pub fn set_p3_func(&mut self, value: impl Into<CpP3Func>) -> &mut Self {
        self.p3_func = value.into();
        self
    }

    pub fn set_p3_phys(&mut self, value: impl Into<CpP3Phys>) -> &mut Self {
        self.p3_phys = value.into();
        self
    }

    pub fn set_rc21_completion_timeout(
        &mut self,
        value: impl Into<CpRc21Completiontimeout>,
    ) -> &mut Self {
        self.rc21_completion_timeout = value.into();
        self
    }

    pub fn set_rc21_handling(&mut self, value: impl Into<CpRc21Handling>) -> &mut Self {
        self.rc21_handling = value.into();
        self
    }

    pub fn set_rc21_request_time(&mut self, value: impl Into<CpRc21RequestTime>) -> &mut Self {
        self.rc21_request_time = value.into();
        self
    }

    pub fn set_rc23_completion_timeout(
        &mut self,
        value: impl Into<CpRc23Completiontimeout>,
    ) -> &mut Self {
        self.rc23_completion_timeout = value.into();
        self
    }

    pub fn set_rc23_handling(&mut self, value: impl Into<CpRc23Handling>) -> &mut Self {
        self.rc23_handling = value.into();
        self
    }

    pub fn set_rc23_request_time(&mut self, value: impl Into<CpRc23RequestTime>) -> &mut Self {
        self.rc23_request_time = value.into();
        self
    }

    pub fn set_rc78_completion_timeout(
        &mut self,
        value: impl Into<CpRc78Completiontimeout>,
    ) -> &mut Self {
        self.rc78_completion_timeout = value.into();
        self
    }

    pub fn set_rc78_handling(&mut self, value: impl Into<CpRc78Handling>) -> &mut Self {
        self.rc78_handling = value.into();
        self
    }

    pub fn set_rc_byte_offset(&mut self, value: impl Into<CpRcByteOffset>) -> &mut Self {
        self.rc_byte_offset = value.into();
        self
    }

    pub fn set_repeat_req_count_app(&mut self, value: impl Into<CpRepeatReqCountApp>) -> &mut Self {
        self.repeat_req_count_app = value.into();
        self
    }

    pub fn set_start_msg_ind_enable(&mut self, value: impl Into<CpStartMsgIndEnable>) -> &mut Self {
        self.start_msg_ind_enable = value.into();
        self
    }

    pub fn set_session_timing_override(
        &mut self,
        value: impl Into<CpSessionTimingOverride>,
    ) -> &mut Self {
        self.session_timing_override = value.into();
        self
    }

    pub fn set_suspend_queue_on_error(
        &mut self,
        value: impl Into<CpSuspendQueueOnError>,
    ) -> &mut Self {
        self.suspend_queue_on_error = value.into();
        self
    }

    pub fn set_sw_can_high_voltage(&mut self, value: impl Into<CpSwCanHighVoltage>) -> &mut Self {
        self.sw_can_high_voltage = value.into();
        self
    }

    pub fn set_tester_present_addr_mode(
        &mut self,
        value: impl Into<CpTesterPresentAddrMode>,
    ) -> &mut Self {
        self.tester_present_addr_mode = value.into();
        self
    }

    pub fn set_tester_present_exp_neg_resp(
        &mut self,
        value: impl Into<CpTesterPresentExpNegResp>,
    ) -> &mut Self {
        self.tester_present_exp_neg_resp = value.into();
        self
    }

    pub fn set_tester_present_exp_pos_resp(
        &mut self,
        value: impl Into<CpTesterPresentExpPosResp>,
    ) -> &mut Self {
        self.tester_present_exp_pos_resp = value.into();
        self
    }

    pub fn set_tester_present_handling(
        &mut self,
        value: impl Into<CpTesterPresentHandling>,
    ) -> &mut Self {
        self.tester_present_handling = value.into();
        self
    }

    pub fn set_tester_present_message(
        &mut self,
        value: impl Into<CpTesterPresentMessage>,
    ) -> &mut Self {
        self.tester_present_message = value.into();
        self
    }

    pub fn set_tester_present_req_rsp(
        &mut self,
        value: impl Into<CpTesterPresentReqRsp>,
    ) -> &mut Self {
        self.tester_present_req_rsp = value.into();
        self
    }

    pub fn set_tester_present_send_type(
        &mut self,
        value: impl Into<CpTesterPresentSendType>,
    ) -> &mut Self {
        self.tester_present_send_type = value.into();
        self
    }

    pub fn set_tester_present_time(&mut self, value: impl Into<CpTesterPresentTime>) -> &mut Self {
        self.tester_present_time = value.into();
        self
    }

    pub fn set_transmit_ind_enable(&mut self, value: impl Into<CpTransmitIndEnable>) -> &mut Self {
        self.transmit_ind_enable = value.into();
        self
    }
}

impl ComParamDefinitionStack for UdsStack {
    fn configure_from_serde_json_map(&mut self, map: &HashMap<String, Value>) {
        let mut value =
            serde_json::to_value(&self).expect("internal error: cannot serialize UdsStack"); // infallible

        let obj = value
            .as_object_mut()
            .expect("internal error: cannot represent UdsStack as map"); // infallible

        for (k, v) in map {
            if !obj.contains_key(k) {
                continue;
            }
            obj.insert(k.clone(), v.clone());
        }

        let new_self: UdsStack =
            serde_json::from_value(value).expect("internal error: cannot deserialize UdsStack"); // infallible

        *self = new_self;
    }

    fn build_set(&self) -> ComParamDefinitionSet<ComParamDefinition> { 
        com_param_definition_set! {
            // Timings.
            self.can_transmission_time,
            self.cyclic_resp_timeout,
            self.modify_timing,
            self.p2_max,
            self.p2_min,
            self.p2_star,
            self.p3_func,
            self.p3_phys,
            self.change_speed_tx_delay,
            self.session_timing_override.clone(),

            // Error handling.
            self.rc21_completion_timeout,
            self.rc21_handling,
            self.rc21_request_time,
            self.rc23_completion_timeout,
            self.rc23_handling,
            self.rc23_request_time,
            self.rc78_completion_timeout,
            self.rc78_handling,
            self.rc_byte_offset,
            self.repeat_req_count_app,
            self.suspend_queue_on_error,

            // Com.
            self.change_speed_ctrl,
            self.change_speed_message.clone(),
            self.change_speed_rate,
            self.start_msg_ind_enable,
            self.sw_can_high_voltage,
            self.transmit_ind_enable,
            self.change_speed_res_ctrl,
            self.enable_performance_test,

            // Tester present.
            self.tester_present_addr_mode,
            self.tester_present_exp_neg_resp.clone(),
            self.tester_present_exp_pos_resp.clone(),
            self.tester_present_handling,
            self.tester_present_message.clone(),
            self.tester_present_req_rsp,
            self.tester_present_send_type,
            self.tester_present_time,
        }
    }

    fn build_table(&self) -> ComParamDefinitionTable<ComParamDefinition> {
        ComParamDefinitionTable::new()
    }
}
