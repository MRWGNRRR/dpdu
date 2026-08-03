use crate::api::{ApiResult, PduApi};
use crate::types::PduUniqueApiTag;
use crate::vendor_specific::wrap_pdu_call;
use std::ffi::CString;
use tracing::trace;

impl PduApi {
    pub fn pdu_construct(&self) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, construct);

        const FUNC: &'static str = "PDUConstruct";
        self.log_api_call(FUNC);

        let options_str = {
            // 9.4.2.4 Parameters
            // OptionStr String containing a list of attributes and their values. An attribute and its corresponding value
            // are to be separated by an >=< sign. The value needs to be put inside two >'< signs. Between
            // pairs of attribute and value shall be at least one space character. Attributes and values are
            // specific to a D-PDU API implementation.
            // When no option is to be set, the OptionStr can either be an empty string or NULL.
            //
            // 9.4.2.5 Example
            // OptionStr = "UseCaching='TRUE' InterfaceCheck='FALSE'"
            self.pdu_options
                .iter()
                .map(|(k, v)| format!("{k}='{v}'"))
                .collect::<Vec<String>>()
                .join(" ")
        };

        trace!(
            func = FUNC,
            options_str,
            unique_tag = ?self.unique_tag.get(),
            "D-PDU API Call Args"
        );

        let options_str = CString::new(options_str).expect("CString::new() failed");
        let construct_fn = self.symbols.construct;
        let result = wrap_pdu_call(FUNC, || {
            construct_fn(
                options_str.as_ptr() as _,
                self.unique_tag.get() as *const PduUniqueApiTag as _,
            )
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, construct),
            );
            return Err(result)?;
        }

        Ok(())
    }
}
