use crate::api::{ApiResult, PduApi};
use crate::types::pdu_io_ctl::{IoCtlByteArray, PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{
    IoByteArrayData, IoEventQueuePropertyData, IoFilterData, IoProgVoltageData, PDU_HANDLE_UNDEF,
    PduError, PduIt, PduObjt,
};
use std::ffi::c_void;
use std::{ptr, slice};
use tracing::{error, trace};

impl PduApi {
    ///| IOCTL Short Name                      | Target | Input Data Type               | Output Data Type            | Purpose
    ///|---------------------------------------|--------|-------------------------------|-----------------------------|--------------------------------------------------------------------------------
    ///| PDU_IOCTL_RESET                       | M      | —                             | —                           | Reset specific MVCI protocol module.
    ///| PDU_IOCTL_CLEAR_TX_QUEUE              | L      | —                             | —                           | Clear transmit queue of specific ComLogicalLink.
    ///| PDU_IOCTL_SUSPEND_TX_QUEUE            | L      | —                             | —                           | Suspend transmit queue of specific ComLogicalLink.
    ///| PDU_IOCTL_RESUME_TX_QUEUE             | L      | —                             | —                           | Resume transmit queue of specific ComLogicalLink.
    ///| PDU_IOCTL_CLEAR_RX_QUEUE              | L      | —                             | —                           | Clear event queue of specific ComLogicalLink.
    ///| PDU_IOCTL_READ_VBATT                  | M      | —                             | PDU_IT_IO_UNUM32            | Read voltage on pin 16 of MVCI protocol module.
    ///| PDU_IOCTL_SET_PROG_VOLTAGE            | M      | PDU_IT_IO_PROG_VOLTAGE        | —                           | Set programmable voltage on DLC connector pin/resource.
    ///| PDU_IOCTL_READ_PROG_VOLTAGE           | M      | —                             | PDU_IT_IO_UNUM32            | Read feedback of programmable voltage.
    ///| PDU_IOCTL_GENERIC                     | M      | PDU_IT_IO_BYTE_ARRAY          | —                           | Send a generic message to MVCI protocol module drivers.
    ///| PDU_IOCTL_SET_BUFFER_SIZE             | L      | PDU_IT_IO_UNUM32              | —                           | Set buffer size limit of item.
    ///| PDU_IOCTL_START_MSG_FILTER            | L      | PDU_IT_IO_FILTER              | —                           | Start filtering incoming messages for specified ComLogicalLink.
    ///| PDU_IOCTL_CLEAR_MSG_FILTER            | L      | —                             | —                           | Clear all message filters for the ComLogicalLink.
    ///| PDU_IOCTL_STOP_MSG_FILTER             | L      | PDU_IT_IO_UNUM32              | —                           | Stop specified filter based on filter number.
    ///| PDU_IOCTL_SET_EVENT_QUEUE_PROPERTIES  | L      | PDU_IT_IO_EVENT_QUEUE_PROPERTY| —                           | Set size and mode of ComLogicalLink event queue.
    ///| PDU_IOCTL_GET_CABLE_ID                | M      | —                             | PDU_IT_IO_UNUM32            | Get cable ID connected to MVCI protocol module.
    ///| PDU_IOCTL_SEND_BREAK                  | L      | —                             | —                           | Send UART Break Signal on ComLogicalLink.
    ///| PDU_IOCTL_READ_IGNITION_SENSE_STATE   | M      | PDU_IT_IO_UNUM32              | PDU_IT_IO_UNUM32            | Read ignition sense state from vehicle connector pin.
    ///| PDU_IOCTL_VEHICLE_ID_REQUEST          | S, M   | PDU_IT_IO_VEHICLE_ID_REQUEST  | —                           | Send vehicle identification request (DoIP).
    ///| PDU_IOCTL_SET_ETH_SWITCH_STATE        | M      | PDU_IT_IO_ETH_SWITCH_STATE    | —                           | Switch Ethernet activation PIN on DLC.
    ///| PDU_IOCTL_GET_ENTITY_STATUS           | M      | PDU_IT_IO_ENTITY_ADDRESS      | PDU_IT_IO_ENTITY_STATUS     | Retrieve status of a DoIP entity.
    ///| PDU_IOCTL_GET_DIAGNOSTIC_POWER_MODE   | M      | PDU_IT_IO_ENTITY_ADDRESS      | PDU_IT_IO_UNUM32            | Retrieve diagnostic power mode of a DoIP entity.
    ///| PDU_IOCTL_GET_ETH_PIN_OPTION          | M      | PDU_IT_IO_UNUM32              | PDU_IT_IO_UNUM32            | Determine Ethernet pinout option from Ethernet activation PIN (DLC).
    ///| PDU_IOCTL_TLS_SET_CERTIFICATE         | M      | PDU_IT_IO_TLS_CERTIFICATE     | —                           | Set X.509 certificate(s) used for ECU verification during TLS handshake.
    ///| PDU_IOCTL_TLS_GET_CURRENT_SESSION_MODE| L      | —                             | PDU_IT_IO_UNUM32            | Get current DoIP connection mode (unsecure or secured via TLS).
    ///| PDU_IOCTL_ISOBUS_GET_DETECTED_CFS     | L      | —                             | PDU_IT_IO_BYTEARRAY         | Get list of ISOBUS CF-NAMEs detected on CAN bus (8-byte NAME + 1-byte address).
    pub fn pdu_io_ctl(
        &self,
        target: &PduIoCtlTarget,
        command: &PduIoCtlCommand,
        data: Option<&PduIoCtlData>,
    ) -> ApiResult<Option<PduIoCtlData>> {
        impl_defer_clear_suppress_options!(self, io_ctl);

        const FUNC: &'static str = "PDUIoCtl";
        self.log_api_call(FUNC);

        let h_mod = target.get_module_handle().unwrap_or(PDU_HANDLE_UNDEF);
        let h_cll = target.get_cll_handle().unwrap_or(PDU_HANDLE_UNDEF);

        trace!(
            func = FUNC,
            ?h_mod,
            ?h_cll,
            %command,
            "D-PDU API Call Args"
        );

        data.inspect(|data| match data {
            PduIoCtlData::U32(v) => trace!(
                func = FUNC,
                data_type = data.as_str(),
                data_u32 = v,
                "D-PDU API Call Args"
            ),
            PduIoCtlData::ProgVoltage(v) => trace!(
                func = FUNC,
                data_type = data.as_str(),
                data_prog_voltage_mv = v.prog_voltage_mv,
                data_pin_on_dlc = v.pin_on_dlc,
                "D-PDU API Call Args"
            ),
            PduIoCtlData::ByteArray(v) => trace!(
                func = FUNC,
                data_type = data.as_str(),
                data_len = v.len(),
                data_value = ?v,
                "D-PDU API Call Args"
            ),
            PduIoCtlData::Filter(v) => trace!(
                func = FUNC,
                data_type = data.as_str(),
                data_filter_type = v.filter_type.as_str(),
                data_filter_number = v.filter_number,
                data_filter_compare_size = v.filter_compare_size,
                data_filter_mask_msg = ?v.filter_mask_msg,
                data_filter_pattern_msg = ?v.filter_pattern_msg,
                "D-PDU API Call Args"
            ),
            PduIoCtlData::EventQueueProperty(v) => trace!(
                func = FUNC,
                data_type = data.as_str(),
                data_queue_size = v.queue_size,
                data_queue_mode = v.queue_mode.as_str(),
                "D-PDU API Call Args"
            ),
        });

        let object_id = match command {
            PduIoCtlCommand::Id(v) => v.to_owned(),
            PduIoCtlCommand::Name(v) => match self.pdu_get_object_id(PduObjt::IoCtrl, &v)? {
                Some(id) => id,
                None => {
                    let result = PduError::FctFailed;

                    self.log_api_call_fail(
                        FUNC,
                        result,
                        Some(format!("unable to lookup IO_CTRL id by name: {v}")),
                        None,
                    );

                    return Err(PduError::FctFailed)?;
                }
            },
        };

        let input_data_ptr: *const c_void = data
            .as_ref()
            .map(|v| v.to_pdu_data_item().p_data as _)
            .unwrap_or(ptr::null());

        let mut output_data_ptr = ptr::null_mut();

        trace!(
            func = FUNC,
            input_data_ptr = format!("{:#x}", input_data_ptr as usize),
            output_data_ptr = format!("{:#x}", &output_data_ptr as *const _ as usize),
            "D-PDU API Call Args"
        );

        let io_ctl_fn = self.symbols.io_ctl;
        let result = wrap_pdu_call(FUNC, || {
            io_ctl_fn(
                h_mod,
                h_cll,
                object_id,
                input_data_ptr as _,
                &mut output_data_ptr,
            )
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, io_ctl),
            );
            return Err(result)?;
        }

        if !output_data_ptr.is_null() {
            let data = unsafe { &*output_data_ptr };
            let io_ctl_data: ApiResult<Option<PduIoCtlData>> = unsafe {
                match data.item_type {
                    PduIt::IoUnum32 => Ok(Some(data.p_data.cast::<u32>().read().into())),
                    PduIt::IoProgVoltage => {
                        Ok(Some(data.p_data.cast::<IoProgVoltageData>().read().into()))
                    }
                    PduIt::IoByteArray => {
                        let byte_array = &*data.p_data.cast::<IoByteArrayData>();
                        if byte_array.p_data.is_null() {
                            error!(
                                func = FUNC,
                                data_type = PduIt::IoByteArray.as_str(),
                                "Byte array pointer is null. Emulation of PduError::FctFailed..."
                            );
                            return Err(PduError::FctFailed)?;
                        } else {
                            let ptr = byte_array.p_data;
                            let len = byte_array.data_size as _;
                            let slice = if ptr.is_null() || len == 0 {
                                &[]
                            } else {
                                slice::from_raw_parts(ptr, len)
                            };
                            Ok(Some(IoCtlByteArray(slice.to_vec()).into()))
                        }
                    }
                    PduIt::IoFilter => Ok(Some(data.p_data.cast::<IoFilterData>().read().into())),
                    PduIt::IoEventQueueProperty => Ok(Some(
                        data.p_data.cast::<IoEventQueuePropertyData>().read().into(),
                    )),
                    v => {
                        error!(
                            func = FUNC,
                            data_type = v.as_str(),
                            "Unexpected output data type. Emulation of PduError::FctFailed..."
                        );
                        return Err(PduError::FctFailed)?;
                    }
                }
            };

            self.pdu_destroy_item(output_data_ptr as _)?;

            io_ctl_data
        } else {
            Ok(None)
        }
    }
}
