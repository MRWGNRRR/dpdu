use crate::api::{ApiResult, PduApi};
use crate::types::pdu_com_param::table::PduComParamTable;
use crate::types::pdu_com_param::{CpVariant, PduComParam};
use crate::types::{PduCllHandle, PduModuleHandle};
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{
    ParamByteFieldData, ParamLongFieldData, ParamStructAccessTiming, ParamStructFieldData,
    ParamStructSessionTiming, PduCpst, PduError, PduIt, PduPt,
};
use std::ptr::NonNull;
use std::{ptr, slice};
use tracing::{error, trace};

impl PduApi {
    pub fn pdu_get_unique_resp_id_table(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> ApiResult<PduComParamTable> {
        impl_defer_clear_suppress_options!(self, get_unique_resp_id_table);

        const FUNC: &'static str = "PDUGetUniqueRespIdTable";
        self.log_api_call(FUNC);

        let mut table_item_ptr = ptr::null_mut();

        trace!(
            func = FUNC,
            h_mod,
            h_cll,
            item_ptr = format!("{:#x}", &table_item_ptr as *const _ as usize),
            "D-PDU API Call Return"
        );

        let get_timestamp_fn = self.symbols.get_unique_resp_id_table;
        let result = wrap_pdu_call(FUNC, || get_timestamp_fn(h_mod, h_cll, &mut table_item_ptr));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_unique_resp_id_table),
            );
            return Err(result)?;
        }

        trace!(
            func = FUNC,
            item_ptr = format!("0x{:x}", table_item_ptr as usize),
            item_type = ?NonNull::new(table_item_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
            "D-PDU API Call Return"
        );

        if table_item_ptr.is_null() {
            error!(
                func = FUNC,
                "Item data pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let table_item = unsafe { &*table_item_ptr };

        if !matches!(table_item.item_type, PduIt::RscConflict) {
            error!(
                func = FUNC,
                "Invalid item type received: PduIt::{}. Emulation of PduError::FctFailed...",
                table_item.item_type.as_str(),
            );

            self.pdu_destroy_item(table_item_ptr as _)?;
            return Err(PduError::FctFailed)?;
        }

        let table_ptr = table_item.p_unique_data;
        let table_len = table_item.num_entries as usize;

        let table = if table_ptr.is_null() || table_len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(table_ptr, table_len) }
        };

        let mut map = PduComParamTable::with_capacity(table.len());

        trace!(
            func = FUNC,
            table_len = table.len(),
            "D-PDU API Call Return"
        );

        for row in table {
            let unique_id = row.unique_resp_identifier;

            let com_params_ptr = row.p_params;
            let com_params_len = row.num_param_items as usize;

            let com_params = if com_params_ptr.is_null() || com_params_len == 0 {
                &[]
            } else {
                unsafe { slice::from_raw_parts(com_params_ptr, com_params_len) }
            };

            trace!(
                func = FUNC,
                table_item_unique_id = unique_id,
                table_item_cp_len = com_params.len(),
                "D-PDU API Call Return"
            );

            for cp in com_params {
                if !matches!(cp.item_type, PduIt::Param) {
                    error!(
                        func = FUNC,
                        "Invalid ComParam type received: PduIt::{}. Emulation of PduError::FctFailed...",
                        cp.item_type.as_str(),
                    );

                    self.pdu_destroy_item(table_item_ptr as _)?;
                    return Err(PduError::FctFailed)?;
                }

                trace!(
                    func = FUNC,
                    table_item_cp_id = cp.com_param_id,
                    table_item_cp_type = cp.com_param_data_type.as_str(),
                    table_item_cp_class = cp.com_param_class.as_str(),
                    "D-PDU API Call Return"
                );

                let variant: CpVariant = unsafe {
                    use ptr::read;

                    match cp.com_param_data_type {
                        PduPt::Unum8 => read::<u8>(cp.p_com_param_data as _).into(),
                        PduPt::Snum8 => read::<i8>(cp.p_com_param_data as _).into(),
                        PduPt::Unum16 => read::<u16>(cp.p_com_param_data as _).into(),
                        PduPt::Snum16 => read::<i16>(cp.p_com_param_data as _).into(),
                        PduPt::Unum32 => read::<u32>(cp.p_com_param_data as _).into(),
                        PduPt::Snum32 => read::<i32>(cp.p_com_param_data as _).into(),
                        PduPt::ByteField => {
                            let data = read::<ParamByteFieldData>(cp.p_com_param_data as _);
                            let ptr = data.p_data_array;
                            let len = data.param_act_len as usize;
                            let bytes = if ptr.is_null() || len == 0 {
                                &[]
                            } else {
                                slice::from_raw_parts(ptr, len)
                            };
                            (bytes.to_vec(), data.param_max_len as usize).into()
                        }
                        PduPt::LongField => {
                            let data = read::<ParamLongFieldData>(cp.p_com_param_data as _);
                            let ptr = data.p_data_array;
                            let len = data.param_act_len as usize;
                            let nums = if ptr.is_null() || len == 0 {
                                &[]
                            } else {
                                slice::from_raw_parts(ptr, len)
                            };
                            (nums.to_vec(), data.param_max_len as usize).into()
                        }
                        PduPt::StructField => {
                            let data = read::<ParamStructFieldData>(cp.p_com_param_data as _);
                            let ptr = data.p_struct_array;
                            let len = data.param_act_entries as usize;
                            match data.com_param_struct_type {
                                PduCpst::AccessTiming => {
                                    let structs: &[ParamStructAccessTiming] =
                                        if ptr.is_null() || len == 0 {
                                            &[]
                                        } else {
                                            slice::from_raw_parts(ptr as _, len)
                                        };
                                    (structs.to_vec(), data.param_max_entries as usize).into()
                                }
                                PduCpst::SessionTiming => {
                                    let structs: &[ParamStructSessionTiming] =
                                        if ptr.is_null() || len == 0 {
                                            &[]
                                        } else {
                                            slice::from_raw_parts(ptr as _, len)
                                        };
                                    (structs.to_vec(), data.param_max_entries as usize).into()
                                }
                            }
                        }
                    }
                };

                let com_param = PduComParam::from_id(cp.com_param_id, cp.com_param_class, variant);

                com_param.try_init_short_name(self);

                map.add(unique_id, com_param);
            }
        }

        self.pdu_destroy_item(table_item_ptr as _)?;

        Ok(map)
    }
}
