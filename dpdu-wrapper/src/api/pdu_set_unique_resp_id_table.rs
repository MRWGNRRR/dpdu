use crate::api::{ApiResult, PduApi};
use crate::types::pdu_com_param::table::PduComParamTable;
use crate::types::{PduCllHandle, PduModuleHandle};
use crate::utils::take_slice_ptr;
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{EcuUniqueRespData, ParamItem, PduError, PduIt, PduPc, UniqueRespIdTableItem};
use std::collections::HashMap;
use tracing::{error, trace};

impl PduApi {
    pub fn pdu_set_unique_resp_id_table(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        table: &PduComParamTable,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, set_unique_resp_id_table);

        const FUNC: &'static str = "PDUSetUniqueRespIdTable";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, "D-PDU API Call Args");

        type EphemeralGroupKey = usize;

        let mut temp_com_param_groups: HashMap<EphemeralGroupKey, Vec<ParamItem>> =
            HashMap::with_capacity(table.len());

        let mut temp_unique_groups: Vec<EcuUniqueRespData> = Vec::with_capacity(table.len());

        for (key, (unique_resp_identifier, group_set)) in table.iter().enumerate() {
            let mut com_param_set = Vec::with_capacity(group_set.len());

            trace!(
                func = FUNC,
                group = key,
                unique_resp_identifier,
                "D-PDU API Call Args"
            );

            for cp in group_set.iter() {
                if !matches!(cp.class, PduPc::UniqueId) {
                    error!(
                        com_param = cp.get_debug_name(),
                        class = cp.class.as_str(),
                        "Invalid class of the PduComParam stored in PduComParamTable"
                    );
                    let result = PduError::InvalidParameters;
                    self.log_api_call_fail(FUNC, result, None, None);
                    return Err(result)?;
                }

                cp.try_init_short_name(self);

                let item = ParamItem {
                    item_type: PduIt::Param,
                    com_param_id: cp.id,
                    com_param_data_type: cp.variant.get_pdu_type(),
                    com_param_class: PduPc::UniqueId,
                    p_com_param_data: cp.variant.get_pdu_ptr().as_ptr() as _,
                };

                trace!(
                    func = FUNC,
                    group = key,
                    com_param = cp.get_debug_name(),
                    "D-PDU API Call Args"
                );

                com_param_set.push(item);
            }

            temp_com_param_groups.insert(key, com_param_set);

            let temp_com_param_group = temp_com_param_groups.get(&key).expect("inserted above");

            temp_unique_groups.push(EcuUniqueRespData {
                unique_resp_identifier: *unique_resp_identifier,
                num_param_items: temp_com_param_group.len() as _,
                p_params: take_slice_ptr(temp_com_param_group.as_slice()),
            });
        }

        let table = UniqueRespIdTableItem {
            item_type: PduIt::UniqueRespIdTable,
            num_entries: temp_unique_groups.len() as _,
            p_unique_data: take_slice_ptr(temp_unique_groups.as_slice()),
        };

        trace!(
            func = FUNC,
            table_num_entries = table.num_entries,
            table_ptr = format!("{:#x}", &table as *const _ as usize),
            "D-PDU API Call Args"
        );

        let set_unique_resp_id_table_fn = self.symbols.set_unique_resp_id_table;
        let result = wrap_pdu_call(FUNC, || {
            set_unique_resp_id_table_fn(h_mod, h_cll, &table as *const _ as _)
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, set_unique_resp_id_table),
            );
            return Err(result)?;
        }

        Ok(())
    }
}
