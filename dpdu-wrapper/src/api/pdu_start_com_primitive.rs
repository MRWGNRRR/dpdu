use crate::api::{ApiResult, PduApi};
use crate::types::pdu_com_primitive::{PrimitiveParams, PrimitiveType};
use crate::types::{PduCllHandle, PduCopHandle, PduModuleHandle, PduUniqueCopTag};
use crate::utils::{PtrRepr, take_slice_ptr};
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::debug::DebugView;
use dpdu_api_types::{CopCtrlData, ExpRespData, FlagData};
use std::cell::OnceCell;
use std::mem::MaybeUninit;
use std::ptr;
use tracing::trace;

impl PduApi {
    pub fn pdu_start_com_primitive(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        primitive_type: &PrimitiveType,
        tag: Option<PduUniqueCopTag>,
    ) -> ApiResult<PduCopHandle> {
        impl_defer_clear_suppress_options!(self, start_primitive);

        const FUNC: &'static str = "PDUStartComPrimitive";
        self.log_api_call(FUNC);

        let tag = tag.map(|v| v.get()).unwrap_or_default();
        let cop_type = primitive_type.to_native_type();

        let mut cop_handle: MaybeUninit<PduCopHandle> = MaybeUninit::uninit();
        let start_com_primitive_fn = self.symbols.start_primitive;

        let cop_ctrl_data_flags = OnceCell::<[u8; 4]>::new();
        let cop_ctrl_data_exp_resps = OnceCell::<Vec<ExpRespData>>::new();

        let make_cop_ctrl_data = |params: &PrimitiveParams| -> CopCtrlData {
            let flags = {
                cop_ctrl_data_flags
                    .set(params.tx_flag.get_pdu_flag_data())
                    .expect("internal error: cop_ctrl_data_flags were initialized more than once"); // infallible

                cop_ctrl_data_flags
                    .get()
                    .expect("internal error: cop_ctrl_data_flags were not initialized") // infallible
            };

            let exp_resps = {
                let vec = params
                    .expected_responses
                    .iter()
                    .map(|v| ExpRespData {
                        response_type: v.response_type as _,
                        acceptance_id: v.acceptance_id,
                        num_mask_pattern_bytes: v.mask_data.len() as _,
                        p_mask_data: take_slice_ptr(v.mask_data.get_mask()),
                        p_pattern_data: take_slice_ptr(v.mask_data.get_pattern()),
                        num_unique_resp_ids: v.unique_response_ids.len() as _, // TODO : heap corruption only under cargo run
                        p_unique_resp_ids: take_slice_ptr(v.unique_response_ids.as_slice()),
                    })
                    .collect::<Vec<_>>();

                cop_ctrl_data_exp_resps.set(vec).expect(
                    "internal error: cop_ctrl_data_exp_resps were initialized more than once",
                ); // infallible

                cop_ctrl_data_exp_resps
                    .get()
                    .expect("internal error: cop_ctrl_data_exp_resps were not initialized") // infallible
            };

            CopCtrlData {
                time: params.time,
                num_send_cycles: params.send_cycles.to_i32(),
                num_receive_cycles: params.receive_cycles.to_i32(),
                temp_param_update: params.temp_param_update as _,
                tx_flag: FlagData {
                    num_flag_bytes: flags.len() as _,
                    p_flag_data: flags.as_ptr() as _,
                },
                num_possible_expected_responses: exp_resps.len() as _,
                expected_response_array: take_slice_ptr(&exp_resps),
            }
        };

        let mut call = |data: &[u8], cop_ctrl_data: Option<&CopCtrlData>| {
            let data_ptr = take_slice_ptr(data);
            let cop_handle_ptr = cop_handle.as_mut_ptr();
            let cop_ctrl_data_ptr = cop_ctrl_data
                .map(|v| v as *const _ as *mut CopCtrlData)
                .unwrap_or_else(ptr::null_mut);

            trace!(
                func = FUNC,
                h_mod,
                h_cll,
                cop_type = cop_type.as_str(),
                data_len = data.len(),
                data_ptr = %PtrRepr::from(data_ptr),
                cop_ctrl_data_ptr = %PtrRepr::from(cop_ctrl_data_ptr),
                cop_ctrl_data = ?cop_ctrl_data.map(|v| v.debug_view()),
                tag,
                cop_handle_ptr = %PtrRepr::from(cop_handle_ptr),
                "D-PDU API Call Args"
            );

            wrap_pdu_call(FUNC, || {
                start_com_primitive_fn(
                    h_mod,
                    h_cll,
                    cop_type,
                    data.len() as _,
                    data_ptr,
                    cop_ctrl_data_ptr,
                    tag as *mut _,
                    cop_handle_ptr,
                )
            })
        };

        let result = match &primitive_type {
            PrimitiveType::StartComm { data, params } => {
                let cop_ctrl_data = make_cop_ctrl_data(params);
                call(data.as_ref(), Some(&cop_ctrl_data))
            }
            PrimitiveType::SendRecv { data, params } => {
                let cop_ctrl_data = make_cop_ctrl_data(params);
                call(data.as_ref(), Some(&cop_ctrl_data))
            }
            PrimitiveType::StopComm { data, params } => {
                let cop_ctrl_data = make_cop_ctrl_data(params);
                call(data.as_ref(), Some(&cop_ctrl_data))
            }
            PrimitiveType::UpdateParam | PrimitiveType::RestoreParam => call(&[], None),
            PrimitiveType::Delay { time } => {
                let mut params = PrimitiveParams::default();
                params.time = time.to_owned();

                let cop_ctrl_data = make_cop_ctrl_data(&params);

                call(&[], Some(&cop_ctrl_data))
            }
        };

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, start_primitive),
            );
            return Err(result)?;
        }

        // SAFETY:
        // PDUStartComPrimitive guarantees that `phCoP` is initialized on success.
        let cop_handle = unsafe { cop_handle.assume_init() };

        trace!(func = FUNC, cop_handle, "D-PDU API Call Return");

        Ok(cop_handle)
    }
}
