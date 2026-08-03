use crate::api::{ApiResult, PduApi};
use crate::types::pdu_resource::{PduResource, PduResourceStatus, ResourceStatus};
use crate::utils::take_slice_ptr;
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{PduIt, RscStatusData, RscStatusItem};
use std::collections::HashMap;
use tracing::{trace, warn};

impl PduApi {
    pub fn pdu_get_resource_status(
        &self,
        resources: &[PduResource],
    ) -> ApiResult<PduResourceStatus> {
        impl_defer_clear_suppress_options!(self, get_resource_status);

        const FUNC: &'static str = "PDUGetResourceStatus";
        self.log_api_call(FUNC);

        let mut map = HashMap::new();

        if resources.len() == 0 {
            warn!(func = FUNC, "Resources is empty");
            return Ok(map);
        }

        let raw_resources = resources
            .iter()
            .map(|v| {
                trace!(
                    func = FUNC,
                    resource_h_mod = v.h_mod,
                    resource_id = v.resource_id,
                    "D-PDU API Call Args"
                );

                RscStatusData {
                    h_mod: v.h_mod,
                    resource_id: v.resource_id,
                    resource_status: 0,
                }
            })
            .collect::<Vec<_>>();

        let mut item = RscStatusItem {
            item_type: PduIt::RscStatus,
            num_entries: raw_resources.len() as _,
            p_resource_status_data: take_slice_ptr(&raw_resources),
        };

        trace!(
            func = FUNC,
            item_ptr = format!("{:#x}", &item as *const _ as usize),
            item_len = resources.len(),
            resources_ptr = format!("{:#x}", raw_resources.as_ptr() as usize),
            "D-PDU API Call Args"
        );

        let get_resource_status_fn = self.symbols.get_resource_status;
        let result = wrap_pdu_call(FUNC, || get_resource_status_fn(&mut item));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_resource_status),
            );
            return Err(result)?;
        }

        for resource in resources {
            'il: for raw in raw_resources.iter() {
                if resource.h_mod != raw.h_mod || resource.resource_id != raw.resource_id {
                    continue 'il;
                }

                let status = raw.resource_status;

                let busy = ((status >> 0) & 1).try_into().unwrap(); // SAFE
                let available = ((status >> 1) & 1).try_into().unwrap(); // SAFE
                let transmit_queue_lock = ((status >> 2) & 1).try_into().unwrap(); // SAFE
                let physical_com_param_lock = ((status >> 3) & 1).try_into().unwrap(); // SAFE

                trace!(
                    func = FUNC,
                    resource_h_mod = raw.h_mod,
                    resource_id = raw.resource_id,
                    resource_status = status,
                    busy,
                    available,
                    transmit_queue_lock,
                    physical_com_param_lock,
                    "D-PDU API Call Args"
                );

                map.insert(
                    resource.clone(),
                    ResourceStatus {
                        raw_status: status,
                        busy,
                        available,
                        transmit_queue_lock,
                        physical_com_param_lock,
                    },
                );

                break 'il;
            }
        }

        Ok(map)
    }
}
