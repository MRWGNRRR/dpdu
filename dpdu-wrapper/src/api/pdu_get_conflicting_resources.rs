use crate::api::{ApiResult, PduApi};
use crate::types::PduObjectId;
use crate::types::pdu_module::{PduConflictingModules, PduModuleData};
use crate::utils::take_slice_ptr;
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{ModuleData, ModuleItem, PduError, PduIt};
use std::ffi::CString;
use std::ptr::NonNull;
use std::{ptr, slice};
use tracing::{error, trace};

impl PduApi {
    pub fn pdu_get_conflicting_resources(
        &self,
        resource_id: PduObjectId,
        modules: &[PduModuleData],
    ) -> ApiResult<PduConflictingModules> {
        impl_defer_clear_suppress_options!(self, get_conflicting_resources);

        const FUNC: &'static str = "PDUGetConflictingResources";
        self.log_api_call(FUNC);

        trace!(func = FUNC, resource_id, "D-PDU API Call Args");

        let mut module_names: Vec<CString> = vec![];
        let mut module_infos: Vec<CString> = vec![];

        let module_items = modules
            .iter()
            .map(|m| {
                trace!(
                    func = FUNC,
                    h_mod = m.h_mod,
                    module_type_id = m.module_type_id,
                    "D-PDU API Call Args"
                );

                let module_name_idx = module_names.len();
                let module_info_idx = module_infos.len();

                module_names.push(
                    CString::new(m.vendor_module_name.clone().unwrap_or_else(String::new))
                        .expect("CString::new()"), // infallible
                );

                module_infos.push(
                    CString::new(m.vendor_additional_info.clone().unwrap_or_else(String::new))
                        .expect("CString::new()"), // infallible
                );

                ModuleData {
                    module_type_id: m.module_type_id,
                    h_mod: m.h_mod,
                    vendor_module_name: module_names[module_name_idx].as_ptr() as _,
                    vendor_additional_info: module_infos[module_info_idx].as_ptr() as _,
                    status: m.status,
                }
            })
            .collect::<Vec<_>>();

        let module_data = ModuleItem {
            item_type: PduIt::ModuleId,
            num_entries: module_items.len() as _,
            p_module_data: take_slice_ptr(&module_items),
        };

        let mut conflict_data_ptr = ptr::null_mut();

        trace!(
            func = FUNC,
            input_module_list_ptr = format!("{:#x}", &module_data as *const _ as usize),
            output_conflict_list_ptr = format!("{:#x}", &conflict_data_ptr as *const _ as usize),
            "D-PDU API Call Args"
        );

        let get_conflicting_resources_fn = self.symbols.get_conflicting_resources;
        let result = wrap_pdu_call(FUNC, || {
            get_conflicting_resources_fn(
                resource_id,
                &module_data as *const _ as _,
                &mut conflict_data_ptr,
            )
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_conflicting_resources),
            );
            return Err(result)?;
        }

        trace!(
            func = FUNC,
            item_ptr = format!("0x{:x}", conflict_data_ptr as usize),
            item_type = ?NonNull::new(conflict_data_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
            "D-PDU API Call Return"
        );

        if conflict_data_ptr.is_null() {
            error!(
                func = FUNC,
                "Item data pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let conflict_data = unsafe { &*conflict_data_ptr };

        if !matches!(conflict_data.item_type, PduIt::RscConflict) {
            error!(
                func = FUNC,
                "Invalid item type received: PduIt::{}. Emulation of PduError::FctFailed...",
                conflict_data.item_type.as_str(),
            );

            self.pdu_destroy_item(conflict_data_ptr as _)?;
            return Err(PduError::FctFailed)?;
        }

        let ptr = conflict_data.p_rsc_conflict_data;
        let len = conflict_data.num_entries as usize;

        let conflict_items = if ptr.is_null() || len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(ptr, len) }
        };

        let map = conflict_items
            .iter()
            .map(|i| {
                trace!(
                    func = FUNC,
                    conflicting_h_mod = i.h_mod,
                    conflicting_resource_id = i.resource_id,
                    "D-PDU API Call Return"
                );
                (i.h_mod, i.resource_id)
            })
            .collect();

        self.pdu_destroy_item(conflict_data_ptr as _)?;

        Ok(map)
    }
}
