use crate::types::pdu_event::PduResultEvent;
use crate::utils::can::{ExtendedId, FrameCreateError, Id, StandardId};

/// A CAN Frame
pub trait CanFrame: Sized {
    /// Creates a new frame.
    ///
    /// This will return `None` if the data slice is too long.
    fn new(id: impl Into<Id>, data: &[u8]) -> Option<Self>;

    /// Creates a new remote frame (RTR bit set).
    ///
    /// This will return `None` if the data length code (DLC) is not valid.
    fn new_remote(id: impl Into<Id>, dlc: usize) -> Option<Self>;

    /// Returns true if this frame is an extended frame.
    fn is_extended(&self) -> bool;

    /// Returns true if this frame is a standard frame.
    fn is_standard(&self) -> bool {
        !self.is_extended()
    }

    /// Returns true if this frame is a remote frame.
    fn is_remote_frame(&self) -> bool;

    /// Returns true if this frame is a data frame.
    fn is_data_frame(&self) -> bool {
        !self.is_remote_frame()
    }

    /// Returns the frame identifier.
    fn id(&self) -> Id;

    /// Returns the data length code (DLC) which is in the range 0..8.
    ///
    /// For data frames the DLC value always matches the length of the data.
    /// Remote frames do not carry any data, yet the DLC can be greater than 0.
    fn dlc(&self) -> usize;

    /// Returns the frame data (0..8 bytes in length).
    fn data(&self) -> &[u8];
}

#[derive(Debug, Clone)]
pub struct Header {
    id: Id,
    len: u8,
    flags: u8,
}

impl Header {
    const FLAG_RTR: u8 = 1 << 0;
    const FLAG_FDCAN: u8 = 1 << 1;
    const FLAG_BRS: u8 = 1<< 2;

    /// Create new CAN Header
    pub fn new(id: Id, len: u8, rtr: bool) -> Header {
        let mut flags = 0u8;
        if rtr { flags |= Self::FLAG_RTR; }
        Header { id, len, flags }
    }

    /// Create new CAN FD Header
    pub fn new_fd(id: Id, len: u8, rtr: bool, brs: bool) -> Header {
        let mut flags = Self::FLAG_FDCAN;
        if rtr { flags |= Self::FLAG_RTR; }
        if brs { flags |= Self::FLAG_BRS; }
        Header { id, len, flags }
    }

    /// Return ID
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// Get mutable reference to ID
    pub fn id_mut(&mut self) -> &mut Id {
        &mut self.id
    }

    /// Return length as u8
    pub fn len(&self) -> u8 {
        self.len
    }

    /// Is remote frame
    pub fn rtr(&self) -> bool {
        (self.flags & Self::FLAG_RTR) != 0
    }

    /// Request/is FDCAN frame
    pub fn fdcan(&self) -> bool {
        (self.flags & Self::FLAG_FDCAN) != 0
    }

    /// Request/is Flexible Data Rate
    pub fn bit_rate_switching(&self) -> bool {
        (self.flags & Self::FLAG_BRS) != 0
    }

    /// Get priority of frame
    pub(crate) fn priority(&self) -> u32 {
        match self.id() {
            Id::Standard(id) => (id.as_raw() as u32) << 18,
            Id::Extended(id) => id.as_raw(),
        }
    }
}

/// Payload of a CAN data frame.
#[derive(Debug, Clone)]
pub struct ClassicData(pub [u8; 8]);

impl ClassicData {
    /// Creates a data payload from a raw byte slice.
    pub fn new(data: &[u8]) -> Result<Self, FrameCreateError> {
        if data.len() > 8 {
            return Err(FrameCreateError::InvalidDataLength);
        }

        let mut bytes = [0; 8];
        bytes[..data.len()].copy_from_slice(data);

        Ok(Self(bytes))
    }

    /// Raw read access to data.
    pub fn raw(&self) -> &[u8] {
        &self.0
    }

    /// Raw mutable read access to data.
    pub fn raw_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }

    /// Checks if the length can be encoded in FDCAN DLC field.
    pub const fn is_valid_len(len: usize) -> bool {
        match len {
            0..=8 => true,
            _ => false,
        }
    }

    /// Creates an empty data payload containing 0 bytes.
    pub const fn empty() -> Self {
        Self([0; 8])
    }
}

/// Frame with up to 8 bytes of data payload as per Classic(non-FD) CAN
/// For CAN-FD support use FdFrame
#[derive(Debug, Clone)]
pub struct ClassicFrame {
    pub header: Header,
    pub data: ClassicData
}

impl ClassicFrame {
    /// Create a new CAN classic Frame
    pub fn new(header: Header, raw_data: &[u8]) -> Result<Self, FrameCreateError> {
        let data = ClassicData::new(raw_data)?;
        Ok(ClassicFrame { header, data })
    }

    /// Creates a new data frame.
    pub fn new_data(id: impl Into<Id>, data: &[u8]) -> Result<Self, FrameCreateError> {
        let id = id.into();
        let header = Header::new(id, data.len() as u8, false);
        Self::new(header, data)
    }

    /// Create new extended frame
    pub fn new_extended(raw_id: u32, raw_data: &[u8]) -> Result<Self, FrameCreateError> {
        if let Some(id) = ExtendedId::new(raw_id) {
            Self::new(Header::new(id.into(), raw_data.len() as u8, false), raw_data)
        } else {
            Err(FrameCreateError::InvalidCanId)
        }
    }

    /// Create new standard frame
    pub fn new_standard(raw_id: u16, raw_data: &[u8]) -> Result<Self, FrameCreateError> {
        if let Some(id) = StandardId::new(raw_id) {
            Self::new(Header::new(id.into(), raw_data.len() as u8, false), raw_data)
        } else {
            Err(FrameCreateError::InvalidCanId)
        }
    }

    /// Create new remote frame
    pub fn new_remote(id: impl Into<Id>, len: usize) -> Result<Self, FrameCreateError> {
        if len <= 8usize {
            Self::new(Header::new(id.into(), len as u8, true), &[0; 8])
        } else {
            Err(FrameCreateError::InvalidDataLength)
        }
    }

    /// Get reference to data
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Return ID
    pub fn id(&self) -> &Id {
        &self.header.id
    }

    /// Get mutable reference to ID
    pub fn id_mut(&mut self) -> &mut Id {
        &mut self.header.id
    }

    /// Get reference to data
    pub fn data(&self) -> &[u8] {
        &self.data.raw()[..self.header.len as usize]
    }

    /// Get reference to underlying 8-byte raw data buffer, some bytes on the tail might be undefined.
    pub fn raw_data(&self) -> &[u8] {
        self.data.raw()
    }

    /// Get mutable reference to data
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data.raw_mut()[..self.header.len as usize]
    }

    /// Get priority of frame
    pub fn priority(&self) -> u32 {
        self.header().priority()
    }
}

/// Payload of a (FD)CAN data frame.
///
/// Contains 0 to 64 Bytes of data.
#[derive(Debug, Clone)]
pub struct FdData(pub [u8; 64]);

impl FdData {
    /// Creates a data payload from a raw byte slice.
    ///
    /// Returns `None` if `data` is more than 64 bytes (which is the maximum) or
    /// cannot be represented with an FDCAN DLC.
    pub fn new(data: &[u8]) -> Result<Self, FrameCreateError> {
        if !FdData::is_valid_len(data.len()) {
            return Err(FrameCreateError::InvalidDataLength);
        }

        let mut bytes = [0; 64];
        bytes[..data.len()].copy_from_slice(data);

        Ok(Self(bytes))
    }

    /// Raw read access to data.
    pub fn raw(&self) -> &[u8] {
        &self.0
    }

    /// Raw mutable read access to data.
    pub fn raw_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }

    /// Checks if the length can be encoded in FDCAN DLC field.
    pub const fn is_valid_len(len: usize) -> bool {
        match len {
            0..=8 => true,
            12 => true,
            16 => true,
            20 => true,
            24 => true,
            32 => true,
            48 => true,
            64 => true,
            _ => false,
        }
    }

    /// Creates an empty data payload containing 0 bytes.
    #[inline]
    pub const fn empty() -> Self {
        Self([0; 64])
    }
}

/// Frame with up to 8 bytes of data payload as per Fd CAN
#[derive(Debug, Clone)]
pub struct FdFrame {
    header: Header,
    data: FdData,
}

impl FdFrame {
    /// Create a new CAN classic Frame
    pub fn new(header: Header, raw_data: &[u8]) -> Result<Self, FrameCreateError> {
        let data = FdData::new(raw_data)?;
        Ok(FdFrame { header, data })
    }

    /// Create new extended frame
    pub fn new_extended(raw_id: u32, raw_data: &[u8]) -> Result<Self, FrameCreateError> {
        if let Some(id) = ExtendedId::new(raw_id) {
            Self::new(Header::new_fd(id.into(), raw_data.len() as u8, false, true), raw_data)
        } else {
            Err(FrameCreateError::InvalidCanId)
        }
    }

    /// Create new standard frame
    pub fn new_standard(raw_id: u16, raw_data: &[u8]) -> Result<Self, FrameCreateError> {
        if let Some(id) = StandardId::new(raw_id) {
            Self::new(Header::new_fd(id.into(), raw_data.len() as u8, false, true), raw_data)
        } else {
            Err(FrameCreateError::InvalidCanId)
        }
    }

    /// Create new remote frame
    pub fn new_remote(id: impl Into<Id>, len: usize) -> Result<Self, FrameCreateError> {
        if len <= 8 {
            Self::new(Header::new_fd(id.into(), len as u8, true, true), &[0; 8])
        } else {
            Err(FrameCreateError::InvalidDataLength)
        }
    }

    /// Get reference to data
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Return ID
    pub fn id(&self) -> &Id {
        &self.header.id
    }

    /// Get reference to data
    pub fn data(&self) -> &[u8] {
        &self.data.raw()[..self.header.len as usize]
    }

    /// Get mutable reference to data
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data.raw_mut()[..self.header.len as usize]
    }
}

impl CanFrame for ClassicFrame {
    fn new(id: impl Into<Id>, data: &[u8]) -> Option<Self> {
        let frameopt = ClassicFrame::new(Header::new(id.into(), data.len() as u8, false), data);
        match frameopt {
            Ok(frame) => Some(frame),
            Err(_) => None,
        }
    }

    fn new_remote(id: impl Into<Id>, dlc: usize) -> Option<Self> {
        if dlc <= 8 {
            let frameopt = ClassicFrame::new(Header::new(id.into(), dlc as u8, true), &[0; 8]);
            match frameopt {
                Ok(frame) => Some(frame),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    fn is_extended(&self) -> bool {
        match self.header.id {
            Id::Extended(_) => true,
            Id::Standard(_) => false,
        }
    }

    fn is_remote_frame(&self) -> bool {
        self.header.rtr()
    }

    fn id(&self) -> Id {
        self.header.id
    }

    fn dlc(&self) -> usize {
        self.header.len as usize
    }

    fn data(&self) -> &[u8] {
        &self.data()
    }
}

impl CanFrame for FdFrame {
    fn new(id: impl Into<Id>, raw_data: &[u8]) -> Option<Self> {
        match FdFrame::new(Header::new_fd(id.into(), raw_data.len() as u8, false, true), raw_data) {
            Ok(frame) => Some(frame),
            Err(_) => None,
        }
    }

    fn new_remote(id: impl Into<Id>, len: usize) -> Option<Self> {
        if len <= 8 {
            match FdFrame::new(Header::new_fd(id.into(), len as u8, true, true), &[0; 64]) {
                Ok(frame) => Some(frame),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    fn is_extended(&self) -> bool {
        match self.header.id {
            Id::Extended(_) => true,
            Id::Standard(_) => false,
        }
    }

    fn is_remote_frame(&self) -> bool {
        self.header.rtr()
    }

    fn id(&self) -> Id {
        self.header.id
    }

    // Returns length in bytes even for CANFD packets which embedded-can does not really mention.
    fn dlc(&self) -> usize {
        self.header.len as usize
    }

    fn data(&self) -> &[u8] {
        &self.data()
    }
}

#[derive(Debug, Clone)]
pub enum AbstractCanFrame {
    Classic(ClassicFrame),
    Fd(FdFrame)
}

impl AbstractCanFrame {
    pub fn as_classic(&self) -> Option<&ClassicFrame> {
        match self {
            Self::Classic(frame) => Some(frame),
            _ => None
        }
    }

    pub fn as_fd(&self) -> Option<&FdFrame> {
        match self {
            Self::Fd(frame) => Some(frame),
            _ => None
        }
    }

    pub fn is_fd(&self) -> bool {
        matches!(self, Self::Fd(_))
    }

    pub fn is_classic(&self) -> bool {
        matches!(self, Self::Classic(_))
    }

    pub fn from_pdu_result_event(event: &PduResultEvent) -> Option<Self> {
        let raw_data = event.data.as_slice();

        let (id, rtr_dlc, data) = if event.rx_flags.is_remote_frame() {
            if raw_data.len() < 5 {
                return None;
            }

            let dlc = raw_data[0];

            (
                u32::from_be_bytes([raw_data[1], raw_data[2], raw_data[3], raw_data[4]]),
                Some(dlc),
                &raw_data[5..]
            )
        } else {
            if raw_data.len() < 4 {
                return None;
            }

            (
                u32::from_be_bytes([raw_data[0], raw_data[1], raw_data[2], raw_data[3]]),
                None,
                &raw_data[4..]
            )
        };

        let id: Id = if event.rx_flags.is_can_29_bit_id() {
            ExtendedId::new(id & 0x1FFFFFFF)?.into()
        } else {
            StandardId::new((id & 0x7FF) as _)?.into()
        };

        if let Some(dlc) = rtr_dlc {
            return Some(Self::Classic(
                ClassicFrame::new_remote(id, dlc as _)
                    .expect("internal error: invalid DLC of RTR frame"))
            );
        }

        ClassicFrame::new_data(id, data).ok().map(|v| {
            // So far, I do not know how to work with FDCAN in the D-PDU API,
            // so now it always returns a ClassicFrame.
            Self::Classic(v)
        })
    }
}

impl CanFrame for AbstractCanFrame {
    fn new(_id: impl Into<Id>, _data: &[u8]) -> Option<Self> {
        None
    }

    fn new_remote(_id: impl Into<Id>, _dlc: usize) -> Option<Self> {
        None
    }

    fn is_extended(&self) -> bool {
        match self {
            Self::Classic(frame) => frame.is_extended(),
            Self::Fd(frame) => frame.is_extended()
        }
    }

    fn is_remote_frame(&self) -> bool {
        match self {
            Self::Classic(frame) => frame.is_remote_frame(),
            Self::Fd(frame) => frame.is_remote_frame()
        }
    }

    fn id(&self) -> Id {
        match self {
            Self::Classic(frame) => frame.header.id,
            Self::Fd(frame) => frame.header.id
        }
    }

    fn dlc(&self) -> usize {
        match self {
            Self::Classic(frame) => frame.header.len as _,
            Self::Fd(frame) => frame.header.len as _
        }
    }

    fn data(&self) -> &[u8] {
        match self {
            Self::Classic(frame) => frame.data(),
            Self::Fd(frame) => frame.data()
        }
    }
}