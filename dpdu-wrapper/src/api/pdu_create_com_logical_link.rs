use crate::api::{ApiResult, PduApi, target_pins_to_pin_data};
use crate::types::pdu_com_logical_link::{CllCreateFlags, CllCreateType, PduCllData};
use crate::types::{PduCllHandle, PduModuleHandle, PduUniqueCllTag};
use crate::utils::take_slice_ptr;
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{FlagData, PDU_ID_UNDEF, RscData};
use std::mem::MaybeUninit;
use std::ptr;
use tracing::trace;

impl PduApi {
    pub fn pdu_create_com_logical_link(
        &self,
        h_mod: PduModuleHandle,
        create_type: &CllCreateType,
        create_flags: &CllCreateFlags,
        tag: Option<PduUniqueCllTag>,
    ) -> ApiResult<PduCllData> {
        impl_defer_clear_suppress_options!(self, create_logical_link);

        const FUNC: &'static str = "PDUCreateComLogicalLink";
        self.log_api_call(FUNC);

        let tag = tag.map(|v| v.get()).unwrap_or_default();

        trace!(
            func = FUNC,
            h_mod,
            tag = format!("{tag:#x}"),
            "D-PDU API Call Args"
        );

        let flag_bytes = create_flags.get_pdu_flag_data();
        let flag_data = FlagData {
            num_flag_bytes: flag_bytes.len() as _,
            p_flag_data: take_slice_ptr(&flag_bytes),
        };

        let mut cll_handle: MaybeUninit<PduCllHandle> = MaybeUninit::uninit();

        let create_com_logical_link_fn = self.symbols.create_logical_link;
        let result = match &create_type {
            CllCreateType::ResourceId(v) => {
                trace!(func = FUNC, resource_id = v, "D-PDU API Call Args");
                wrap_pdu_call(FUNC, || {
                    create_com_logical_link_fn(
                        h_mod,
                        ptr::null_mut(),
                        v.clone(),
                        tag as *mut _,
                        cll_handle.as_mut_ptr(),
                        &flag_data as *const FlagData as _,
                    )
                })
            }
            CllCreateType::ResourceData {
                bus,
                protocol,
                pins,
            } => {
                trace!(func = FUNC, %bus, %protocol, "D-PDU API Call Args");

                let bus_type_id = bus.resolve_bus_id(FUNC, self)?;
                let protocol_id = protocol.resolve_protocol_id(FUNC, self)?;

                let pin_data = target_pins_to_pin_data(self, FUNC, &pins)?;

                let rsc_data = RscData {
                    bus_type_id,
                    protocol_id,
                    num_pin_data: pin_data.len() as _,
                    p_dlc_pin_data: take_slice_ptr(&pin_data),
                };

                trace!(
                    func = FUNC,
                    rsc_data_ptr = format!("{:#x}", &rsc_data as *const _ as usize),
                    bus_type_id,
                    protocol_id,
                    pin_len = pin_data.len(),
                    "D-PDU API Call Args"
                );

                wrap_pdu_call(FUNC, || {
                    create_com_logical_link_fn(
                        h_mod,
                        &rsc_data as *const RscData as _,
                        PDU_ID_UNDEF,
                        tag as *mut _,
                        cll_handle.as_mut_ptr(),
                        &flag_data as *const FlagData as _,
                    )
                })
            }
        };

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, create_logical_link),
            );
            return Err(result)?;
        }

        let h_cll = unsafe { cll_handle.assume_init() };

        trace!(func = FUNC, h_cll, "D-PDU API Call Return");

        Ok(PduCllData {
            h_mod,
            h_cll: unsafe { cll_handle.assume_init() },
            create_type: create_type.clone(),
            create_flags: create_flags.clone(),
        })
    }
}
