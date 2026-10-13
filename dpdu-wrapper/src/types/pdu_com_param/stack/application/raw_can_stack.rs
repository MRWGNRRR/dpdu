use crate::com_param_definition_set;
use crate::types::pdu_com_param::single::com::{
    CpChangeSpeedCtrl, CpChangeSpeedMessage, CpChangeSpeedRate, CpChangeSpeedResCtrl,
    CpEnablePerformanceTest, CpLoopback, CpSwCanHighVoltage, CpTransmitIndEnable,
};
use crate::types::pdu_com_param::single::err_hdl::CpRepeatReqCountApp;
use crate::types::pdu_com_param::single::timing::{
    CpChangeSpeedTxDelay, CpCyclicRespTimeout, CpP2Max, CpP2Min, CpP3Func, CpP3Phys,
};
use crate::types::pdu_com_param::stack::ComParamDefinitionStack;
use crate::types::pdu_com_param::table::{
    ComParamDefinition, ComParamDefinitionSet, ComParamDefinitionTable,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Raw CAN application stack (ISO 11898-3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCanApplicationStack {
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

    #[serde(rename = "CP_P2Max")]
    pub p2_max: CpP2Max,

    #[serde(rename = "CP_P2Min")]
    pub p2_min: CpP2Min,

    #[serde(rename = "CP_P3Func")]
    pub p3_func: CpP3Func,

    #[serde(rename = "CP_P3Phys")]
    pub p3_phys: CpP3Phys,

    #[serde(rename = "CP_RepeatReqCountApp")]
    pub repeat_req_count_app: CpRepeatReqCountApp,

    #[serde(rename = "CP_SwCan_HighVoltage")]
    pub sw_can_high_voltage: CpSwCanHighVoltage,

    #[serde(rename = "CP_TransmitIndEnable")]
    pub transmit_ind_enable: CpTransmitIndEnable,
}

impl RawCanApplicationStack {
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

    pub fn set_p2_max(&mut self, value: impl Into<CpP2Max>) -> &mut Self {
        self.p2_max = value.into();
        self
    }

    pub fn set_p2_min(&mut self, value: impl Into<CpP2Min>) -> &mut Self {
        self.p2_min = value.into();
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

    pub fn set_repeat_req_count_app(&mut self, value: impl Into<CpRepeatReqCountApp>) -> &mut Self {
        self.repeat_req_count_app = value.into();
        self
    }

    pub fn set_sw_can_high_voltage(&mut self, value: impl Into<CpSwCanHighVoltage>) -> &mut Self {
        self.sw_can_high_voltage = value.into();
        self
    }

    pub fn set_transmit_ind_enable(&mut self, value: impl Into<CpTransmitIndEnable>) -> &mut Self {
        self.transmit_ind_enable = value.into();
        self
    }
}

impl ComParamDefinitionStack for RawCanApplicationStack {
    fn configure_from_serde_json_map(&mut self, map: &HashMap<String, Value>) {
        let mut value = serde_json::to_value(&self)
            .expect("internal error: cannot serialize RawCanApplicationStack"); // infallible

        let obj = value
            .as_object_mut()
            .expect("internal error: cannot represent RawCanApplicationStack as map"); // infallible

        for (k, v) in map {
            if !obj.contains_key(k) {
                continue;
            }
            obj.insert(k.clone(), v.clone());
        }

        let new_self: RawCanApplicationStack = serde_json::from_value(value)
            .expect("internal error: cannot deserialize RawCanApplicationStack"); // infallible

        *self = new_self;
    }

    fn build_set(&self) -> ComParamDefinitionSet<ComParamDefinition> {
        com_param_definition_set! {
            // Timings.
            self.change_speed_tx_delay,
            self.cyclic_resp_timeout,
            self.p2_max,
            self.p2_min,
            self.p3_func,
            self.p3_phys,

            // Error handling.
            self.repeat_req_count_app,

            // Com.
            self.change_speed_ctrl,
            self.change_speed_message.clone(),
            self.change_speed_rate,
            self.change_speed_res_ctrl,
            self.enable_performance_test,
            self.loopback,
            self.sw_can_high_voltage,
            self.transmit_ind_enable,
        }
    }

    fn build_table(&self) -> ComParamDefinitionTable<ComParamDefinition> {
        ComParamDefinitionTable::new()
    }
}
