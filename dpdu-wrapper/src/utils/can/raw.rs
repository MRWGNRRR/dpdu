use crate::utils::can::CanFrame;

pub trait RawCanPrimitiveBuilderExt {
    /// Use case.
    fn monitor() -> Self;

    /// Use case.
    fn send_only_raw_can(frame: impl CanFrame) -> Self;

    /// Use case.
    ///
    /// For this to work, you need to set the following communication parameters:
    /// - CP_CanRespUUDTFormat: NORMAL_UNSEGMENTED_11_BIT
    /// - CP_CanRespUUDTId: <your expected frame ID>
    /// - CP_CanRespUUDTExtAddr: 0
    fn send_recv_raw_can(frame: impl CanFrame) -> Self;
}
