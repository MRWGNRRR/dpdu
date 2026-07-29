#![allow(missing_docs)]

use crate::PduError;
use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct PduErrorFlag: u64 {
        const STATUS_NO_ERROR                             = 1 << 0;
        const FCT_FAILED                                  = 1 << 1;
        const RESERVED1                                   = 1 << 2;
        const COMM_PC_TO_VCI_FAILED                       = 1 << 3;
        const PDU_API_NOT_CONSTRUCTED                     = 1 << 4;
        const SHARING_VIOLATION                           = 1 << 5;
        const RESOURCE_BUSY                               = 1 << 6;
        const RESOURCE_TABLE_CHANGED                      = 1 << 7;
        const RESOURCE_ERROR                              = 1 << 8;
        const CLL_NOT_CONNECTED                           = 1 << 9;
        const CLL_NOT_STARTED                             = 1 << 10;
        const INVALID_PARAMETERS                          = 1 << 11;
        const INVALID_HANDLE                              = 1 << 12;
        const VALUE_NOT_SUPPORTED                         = 1 << 13;
        const ID_NOT_SUPPORTED                            = 1 << 14;
        const COM_PARAM_NOT_SUPPORTED                     = 1 << 15;
        const COM_PARAM_LOCKED                            = 1 << 16;
        const TX_QUEUE_FULL                               = 1 << 17;
        const EVENT_QUEUE_EMPTY                           = 1 << 18;
        const VOLTAGE_NOT_SUPPORTED                       = 1 << 19;
        const MUX_RSC_NOT_SUPPORTED                       = 1 << 20;
        const CABLE_UNKNOWN                               = 1 << 21;
        const NO_CABLE_DETECTED                           = 1 << 22;
        const CLL_CONNECTED                               = 1 << 23;
        const TEMP_PARAM_NOT_ALLOWED                      = 1 << 24;
        const RSC_LOCKED                                  = 1 << 25;
        const RSC_LOCKED_BY_ANOTHER_CLL                   = 1 << 26;
        const RSC_NOT_LOCKED                              = 1 << 27;
        const MODULE_NOT_CONNECTED                        = 1 << 28;
        const API_SW_OUT_OF_DATE                          = 1 << 29;
        const MODULE_FW_OUT_OF_DATE                       = 1 << 30;
        const PIN_NOT_CONNECTED                           = 1 << 31;
        const IP_PROTOCOL_NOT_SUPPORTED                   = 1 << 32;
        const DOIP_ROUTING_ACTIVATION_FAILED              = 1 << 33;
        const DOIP_ROUTING_ACTIVATION_AUTH_FAILED         = 1 << 34;
        const DOIP_AMBIGUOUS_LOGICAL_ADDRESS              = 1 << 35;
        const DOIP_ROUTINE_ACTIVATION_INVALID_SRC_ADDRESS = 1 << 36;
        const DOIP_ROUTING_ACTIVATION_NO_DATA_SOCKET      = 1 << 37;
        const DOIP_ROUTINE_SOURCE_ADDRESS_CHANGED         = 1 << 38;
        const DOIP_ROUTING_SOURCE_ADDRESS_IN_USE          = 1 << 39;
        const DOIP_ROUTINE_CONFIRMATION_REJECTED          = 1 << 40;
        const DOIP_ROUTINE_TYPE_UNSUPPORTED               = 1 << 41;
        const DOIP_ROUTINE_RESPONSE_UNKNOWN               = 1 << 42;
        const DOIP_ROUTING_RESPONSE_TIMEOUT               = 1 << 43;
        const DOIP_RESPONSE_TIMEOUT                       = 1 << 44;
    }
}

impl PduError {
    pub fn flag(self) -> PduErrorFlag {
        match self {
            PduError::StatusNoError => PduErrorFlag::STATUS_NO_ERROR,
            PduError::FctFailed => PduErrorFlag::FCT_FAILED,
            PduError::Reserved1 => PduErrorFlag::RESERVED1,
            PduError::CommPcToVciFailed => PduErrorFlag::COMM_PC_TO_VCI_FAILED,
            PduError::PduApiNotConstructed => PduErrorFlag::PDU_API_NOT_CONSTRUCTED,
            PduError::SharingViolation => PduErrorFlag::SHARING_VIOLATION,
            PduError::ResourceBusy => PduErrorFlag::RESOURCE_BUSY,
            PduError::ResourceTableChanged => PduErrorFlag::RESOURCE_TABLE_CHANGED,
            PduError::ResourceError => PduErrorFlag::RESOURCE_ERROR,
            PduError::CllNotConnected => PduErrorFlag::CLL_NOT_CONNECTED,
            PduError::CllNotStarted => PduErrorFlag::CLL_NOT_STARTED,
            PduError::InvalidParameters => PduErrorFlag::INVALID_PARAMETERS,
            PduError::InvalidHandle => PduErrorFlag::INVALID_HANDLE,
            PduError::ValueNotSupported => PduErrorFlag::VALUE_NOT_SUPPORTED,
            PduError::IdNotSupported => PduErrorFlag::ID_NOT_SUPPORTED,
            PduError::ComParamNotSupported => PduErrorFlag::COM_PARAM_NOT_SUPPORTED,
            PduError::ComParamLocked => PduErrorFlag::COM_PARAM_LOCKED,
            PduError::TxQueueFull => PduErrorFlag::TX_QUEUE_FULL,
            PduError::EventQueueEmpty => PduErrorFlag::EVENT_QUEUE_EMPTY,
            PduError::VoltageNotSupported => PduErrorFlag::VOLTAGE_NOT_SUPPORTED,
            PduError::MuxRscNotSupported => PduErrorFlag::MUX_RSC_NOT_SUPPORTED,
            PduError::CableUnknown => PduErrorFlag::CABLE_UNKNOWN,
            PduError::NoCableDetected => PduErrorFlag::NO_CABLE_DETECTED,
            PduError::CllConnected => PduErrorFlag::CLL_CONNECTED,
            PduError::TempParamNotAllowed => PduErrorFlag::TEMP_PARAM_NOT_ALLOWED,
            PduError::RscLocked => PduErrorFlag::RSC_LOCKED,
            PduError::RscLockedByAnotherCll => PduErrorFlag::RSC_LOCKED_BY_ANOTHER_CLL,
            PduError::RscNotLocked => PduErrorFlag::RSC_NOT_LOCKED,
            PduError::ModuleNotConnected => PduErrorFlag::MODULE_NOT_CONNECTED,
            PduError::ApiSwOutOfDate => PduErrorFlag::API_SW_OUT_OF_DATE,
            PduError::ModuleFwOutOfDate => PduErrorFlag::MODULE_FW_OUT_OF_DATE,
            PduError::PinNotConnected => PduErrorFlag::PIN_NOT_CONNECTED,
            PduError::IpProtocolNotSupported => PduErrorFlag::IP_PROTOCOL_NOT_SUPPORTED,
            PduError::DoIpRoutingActivationFailed => PduErrorFlag::DOIP_ROUTING_ACTIVATION_FAILED,
            PduError::DoIpRoutingActivationAuthFailed => {
                PduErrorFlag::DOIP_ROUTING_ACTIVATION_AUTH_FAILED
            }
            PduError::DoIpAmbiguousLogicalAddress => PduErrorFlag::DOIP_AMBIGUOUS_LOGICAL_ADDRESS,
            PduError::DoIpRoutineActivationInvalidSrcAddress => {
                PduErrorFlag::DOIP_ROUTINE_ACTIVATION_INVALID_SRC_ADDRESS
            }
            PduError::DoIpRoutingActivationNoDataSocketAvailable => {
                PduErrorFlag::DOIP_ROUTING_ACTIVATION_NO_DATA_SOCKET
            }
            PduError::DoIpRoutineActivationSourceAddressChanged => {
                PduErrorFlag::DOIP_ROUTINE_SOURCE_ADDRESS_CHANGED
            }
            PduError::DoIpRoutingActivationSourceAddressInUse => {
                PduErrorFlag::DOIP_ROUTING_SOURCE_ADDRESS_IN_USE
            }
            PduError::DoIpRoutineActivationConfirmationRejected => {
                PduErrorFlag::DOIP_ROUTINE_CONFIRMATION_REJECTED
            }
            PduError::DoIpRoutineActivationTypeUnsupported => {
                PduErrorFlag::DOIP_ROUTINE_TYPE_UNSUPPORTED
            }
            PduError::DoIpRoutineActivationResponseCodeUnknown => {
                PduErrorFlag::DOIP_ROUTINE_RESPONSE_UNKNOWN
            }
            PduError::DoIpRoutingActivationResponseTimeout => {
                PduErrorFlag::DOIP_ROUTING_RESPONSE_TIMEOUT
            }
            PduError::DoIpResponseTimeout => PduErrorFlag::DOIP_RESPONSE_TIMEOUT,
        }
    }
}
