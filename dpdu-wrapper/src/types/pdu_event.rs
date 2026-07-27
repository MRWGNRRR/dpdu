use crate::types::{PduCllHandle, PduCopHandle, PduModuleHandle, PduUniqueCopTag};
use dpdu_api_types::{PDU_HANDLE_UNDEF, PduErrorEvt, PduInfo, PduStatus};
use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Once, OnceLock};
use tokio::sync::{mpsc, oneshot, watch, Notify};

/// Flag indicating whether event reception from the D-PDU API should be stopped.
///
/// Used by `PduVci`, `PduLogicalLink`, and `PduPrimitive` event handlers to
/// signal that the receiving loop should terminate.
pub type StopReceive = bool;

#[derive(Debug, Clone)]
pub struct PduEvent {
    pub target: PduEventTarget,

    pub h_cop: Option<PduCopHandle>,

    pub cop_tag: Option<PduUniqueCopTag>,

    pub timestamp: u32,

    /// Желательно создавать через типаж [`Into<PduEventData>`].
    pub data: PduEventData,
}

#[derive(Debug, Clone, strum::AsRefStr, strum::Display)]
pub enum PduEventTarget {
    System,
    Module(PduModuleHandle),
    LogicalLink(PduModuleHandle, PduCllHandle),
}

impl PduEventTarget {
    pub(crate) fn from_callback(h_mod: PduModuleHandle, h_cll: PduCllHandle) -> Self {
        let h_mod_opt = (h_mod != PDU_HANDLE_UNDEF).then(|| h_mod);
        let h_cll_opt = (h_cll != PDU_HANDLE_UNDEF).then(|| h_cll);

        match (h_mod_opt, h_cll_opt) {
            (None, None) => PduEventTarget::System,
            (Some(h_mod), None) => PduEventTarget::Module(h_mod),
            (Some(h_mod), Some(h_cll)) => PduEventTarget::LogicalLink(h_mod, h_cll),
            _ => {
                unreachable!("internal error: CLL handle cannot exist without a module handle");
            }
        }
    }

    pub fn is_system(&self) -> bool {
        matches!(self, PduEventTarget::System)
    }

    pub fn is_module(&self) -> bool {
        matches!(self, PduEventTarget::Module(..))
    }

    pub fn is_logical_link(&self) -> bool {
        matches!(self, PduEventTarget::LogicalLink(..))
    }

    pub fn get_module_handle(&self) -> Option<PduModuleHandle> {
        match self {
            PduEventTarget::Module(h_mod) => Some(h_mod.clone()),
            PduEventTarget::LogicalLink(h_mod, ..) => Some(h_mod.clone()),
            _ => None,
        }
    }

    pub fn get_cll_handle(&self) -> Option<PduCllHandle> {
        match self {
            PduEventTarget::LogicalLink(_, h_cll) => Some(h_cll.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, strum::AsRefStr)]
pub enum PduEventData {
    Status(PduStatusEvent),

    Result(PduResultEvent),

    Error(PduErrorEvent),

    Info(PduInfoEvent),
}

impl From<PduStatusEvent> for PduEventData {
    fn from(value: PduStatusEvent) -> Self {
        PduEventData::Status(value)
    }
}

impl From<PduResultEvent> for PduEventData {
    fn from(value: PduResultEvent) -> Self {
        PduEventData::Result(value)
    }
}

impl From<PduErrorEvent> for PduEventData {
    fn from(value: PduErrorEvent) -> Self {
        PduEventData::Error(value)
    }
}

impl From<PduInfoEvent> for PduEventData {
    fn from(value: PduInfoEvent) -> Self {
        PduEventData::Info(value)
    }
}

impl PduEventData {
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }

    pub fn as_status(&self) -> Option<&PduStatusEvent> {
        match self {
            PduEventData::Status(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_result(&self) -> Option<&PduResultEvent> {
        match self {
            PduEventData::Result(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_error(&self) -> Option<&PduErrorEvent> {
        match self {
            PduEventData::Error(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_info(&self) -> Option<&PduInfoEvent> {
        match self {
            PduEventData::Info(v) => Some(v),
            _ => None,
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct PduStatusEvent(pub PduStatus);

impl Deref for PduStatusEvent {
    type Target = PduStatus;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PduStatusEvent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Result notification event containing received data.
///
/// Generated for the `PDU_IT_RESULT` item.
#[derive(Debug, Clone)]
pub struct PduResultEvent {
    /// Receive message status flags.
    pub rx_flags: PduResultEventRxFlags,

    /// Unique identifier of the ECU response.
    pub unique_resp_identifier: u32,

    /// Acceptance ID from the matched Expected Response entry.
    ///
    /// When multiple Expected Response entries match the received data,
    /// the first matching entry in the Expected Response array is selected.
    /// Entries are evaluated in array order, where lower indices have higher priority.
    pub acceptance_id: u32,

    /// Bit-oriented timestamp validity flags.
    ///
    /// If no timestamp flags are set, timestamp values are invalid.
    pub timestamp_flags: PduResultEventTimestampFlags,

    /// Timestamp in microseconds when transmission of the tester request completed.
    pub tx_msg_done_timestamp: u32,

    /// Timestamp in microseconds when the ECU response started.
    ///
    /// Indicates the start of the received response message.
    pub start_msg_timestamp: u32,

    /// Received PDU data.
    ///
    /// In RawMode, contains protocol frame data including:
    /// header bytes, checksum, payload and optional extra data.
    ///
    /// For ISO 11898, ISO 15765 and SAE J1939,
    /// the first four bytes contain the CAN identifier,
    /// followed by the optional extended address byte.
    pub data: Vec<u8>,

    /// Response PDU header bytes.
    pub extra_info_header: Option<Vec<u8>>,

    /// Response PDU footer bytes.
    ///
    /// May contain extra protocol-specific data, such as:
    /// - IFR data for SAE J1850 PWM;
    /// - checksum bytes for ISO 14230.
    ///
    /// Empty when no footer data is present.
    pub extra_info_footer: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct PduResultEventRxFlags(Vec<u8>);

impl From<Vec<u8>> for PduResultEventRxFlags {
    fn from(value: Vec<u8>) -> Self {
        PduResultEventRxFlags(value)
    }
}

impl Deref for PduResultEventRxFlags {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PduResultEventRxFlags {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl PduResultEventRxFlags {
    /// Returns `true` if a CAN Remote Frame was detected.
    ///
    /// A Remote Frame does not contain data bytes.
    ///
    /// The first byte of the D-PDU data contains the Data Length Code (DLC).
    pub fn is_remote_frame(&self) -> bool {
        if let Some(byte) = self.get(0) {
            return (*byte & 0x80) != 0;
        }

        false
    }

    /// Returns `true` if the communication bus transitioned to a new speed rate.
    pub fn is_speed_change_event(&self) -> bool {
        if let Some(byte) = self.get(1) {
            return (*byte & 0x04) != 0;
        }
        false
    }

    /// Returns `true` if the ECU timing communication parameters were modified.
    ///
    /// This flag is only available when the `CP_ModifyTiming`
    /// ComParam is enabled.
    pub fn is_ecu_timing_changed(&self) -> bool {
        if let Some(byte) = self.get(1) {
            return (*byte & 0x02) != 0;
        }
        false
    }

    /// <summary>
    ///     Indicates that the Single Wire CAN message received was a High-Voltage Message
    ///     false = Normal Message
    ///     true = High-Voltage Message
    /// </summary>
    pub fn is_sw_can_high_voltage_msg(&self) -> bool {
        if let Some(byte) = self.get(1) {
            return (*byte & 0x01) != 0;
        }
        false
    }

    /// Returns true if the CAN frame uses a 29-bit extended identifier.
    pub fn is_can_29_bit_id(&self) -> bool {
        if let Some(byte) = self.get(2) {
            return (*byte & 0x01) != 0;
        }
        false
    }

    /// Returns `true` if ISO 15765-2 transport segmentation was detected.
    ///
    /// This flag is only valid in `RawMode`
    /// when enabled through [`CllCreateFlags`].
    ///
    /// If segmentation handling was performed,
    /// transport protocol segment information was removed
    /// from the PDU data.
    ///
    /// [`CllCreateFlags`]: crate::types::pdu_com_logical_link::CllCreateFlags
    pub fn is_can_segmentation(&self) -> bool {
        if let Some(byte) = self.get(3) {
            return (*byte & 0x40) != 0;
        }
        false
    }

    /// Returns `true` if an ISO 15765 padding error was detected.
    ///
    /// This flag is only valid in `RawMode`
    /// when enabled through [`CllCreateFlags`].
    ///
    /// Indicates that a received CAN frame contained fewer than
    /// 8 data bytes while padding was expected.
    ///
    /// [`CllCreateFlags`]: crate::types::pdu_com_logical_link::CllCreateFlags
    pub fn is_iso_15765_padding_error(&self) -> bool {
        if let Some(byte) = self.get(3) {
            return (*byte & 0x10) != 0;
        }
        false
    }

    /// Returns `true` if a TxDone indication is present.
    pub fn get_tx_status(&self) -> bool {
        if let Some(byte) = self.get(3) {
            return (*byte & 0x08) != 0;
        }
        false
    }

    /// Returns `true` if a SAE J2610 or SAE J1850 VPW break indication
    /// was received.
    pub fn get_rx_break_status(&self) -> bool {
        if let Some(byte) = self.get(3) {
            return (*byte & 0x04) != 0;
        }
        false
    }

    /// Returns `true` if the event indicates the start of message reception.
    ///
    /// This indicates:
    /// - the first byte of an ISO 9141 or ISO 14230 message;
    /// - the first frame of an ISO 15765 multi-frame message.
    pub fn is_start_of_message(&self) -> bool {
        if let Some(byte) = self.get(3) {
            return (*byte & 0x02) != 0;
        }
        false
    }


    /// Returns `true` if the message is a Transmit Loopback message.
    ///
    /// A Transmit Loopback message is an echo of a message
    /// transmitted by the communication device itself.
    pub fn get_tx_msg_type(&self) -> bool {
        if let Some(byte) = self.get(3) {
            return (*byte & 0x01) != 0;
        }
        false
    }

    /// Returns `true` if ISO 15765 extended addressing is used.
    ///
    /// This flag is only valid in `RawMode`
    /// when enabled through [`CllCreateFlags`].
    ///
    /// When extended addressing is used,
    /// the extended address byte follows the CAN identifier
    /// in the PDU data.
    ///
    /// [`CllCreateFlags`]: crate::types::pdu_com_logical_link::CllCreateFlags
    pub fn is_iso_15765_addr_type(&self) -> bool {
        if let Some(byte) = self.get(3) {
            return (*byte & 0x80) != 0;
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct PduResultEventTimestampFlags(Vec<u8>);

impl From<Vec<u8>> for PduResultEventTimestampFlags {
    fn from(value: Vec<u8>) -> Self {
        PduResultEventTimestampFlags(value)
    }
}

impl Deref for PduResultEventTimestampFlags {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PduResultEventTimestampFlags {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl PduResultEventTimestampFlags {
    /// Указывает, что значение Transmit Done Timestamp в структуре
    /// PDU_RESULT_DATA является действительным.
    pub fn is_tx_msg_done_timestamp_indicator(&self) -> bool {
        if let Some(byte) = self.get(0) {
            return (*byte & 0x80) != 0;
        }
        false
    }

    /// Указывает, что значение Start Message Timestamp в структуре
    /// PDU_RESULT_DATA является действительным.
    pub fn is_start_msg_timestamp_indicator(&self) -> bool {
        if let Some(byte) = self.get(0) {
            return (*byte & 0x40) != 0;
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct PduErrorEvent {
    pub code: PduErrorEvt,
    pub extra_code: u32,
}

impl Display for PduErrorEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PduErrorEvent: code={}, extra_code={}",
            self.code.as_ref(),
            self.extra_code
        )
    }
}

#[derive(Debug, Clone)]
pub struct PduInfoEvent {
    pub code: PduInfo,
    pub extra_code: u32,
}

impl Display for PduInfoEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PduInfoEvent: code={}, extra_code={}",
            self.code.as_ref(),
            self.extra_code
        )
    }
}

/// Stores a single error event produced by a D-PDU primitive.
///
/// The event can be written only once and remains available for the lifetime
/// of the store.
#[derive(Debug)]
pub struct ErrorEventStore(OnceLock<PduErrorEvent>);

impl ErrorEventStore {
    /// Creates a new empty error event store.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self(OnceLock::default()))
    }

    /// Stores an error event.
    ///
    /// Returns the provided event back if an error has already been stored.
    pub(crate) fn set(&self, event: PduErrorEvent) -> Result<(), PduErrorEvent> {
        self.0.set(event)
    }

    /// Returns the stored error event, if any.
    pub fn get(&self) -> Option<&PduErrorEvent> {
        self.0.get()
    }
}