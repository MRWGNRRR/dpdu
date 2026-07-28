<div align="center">
    <img src="https://cdn.einvest.pro/miniapp-project-avatars/photo_2026-07-17_03-30-13.png" width="400">

  
  ## D-PDU API wrapper - ISO 22900-2

  This project provides a high-level wrapper around the D-PDU API, simplifying common development tasks while maintaining full access to the underlying low-level API. When needed, users can interact directly with D-PDU drivers, combining the convenience of high-level abstractions with the     flexibility of low-level control.
</div>

## Example 1. RAW CAN: send + recv

```rust
use dpdu_wrapper::api::{PduApi};
use dpdu_wrapper::types::PduOptions;
use dpdu_wrapper::types::pdu_com_logical_link::{CllCreateFlags, CllCreateType, PduLogicalLink, SendRecv};
use dpdu_wrapper::types::pdu_com_param::single::timing::{CpP2Max, CpP2Min};
use dpdu_wrapper::types::pdu_com_param::stack::composite::RawCanStack;
use dpdu_wrapper::types::pdu_vci::PduVci;
use dpdu_wrapper::utils::can::{RawCanPrimitiveBuilderExt, ClassicFrame, AbstractCanFrame};
use dpdu_wrapper::utils::root_file::PduRootFile;
use dpdu_wrapper::worker::PduAsyncWorker;

#[tokio::main]
async fn main() {
  let root_file = PduRootFile::guess_and_parse().unwrap();
  let edic_mvci = root_file.get_mvci_by_short_name("EDIC_D_PDU_API_1_20_042").unwrap();
  
  let api = PduApi::from_mvci(edic_mvci, PduOptions::default()).unwrap();
  let worker = PduAsyncWorker::new(api);
  
  let vci_list = PduVci::list(&worker, None).await.unwrap();
  let vci = vci_list.into_iter().next().unwrap();
  
  vci.connect().await.unwrap();
  
  let cll = {
    let create_type = CllCreateType::raw_dw_can_on_obd();
    let create_flags = CllCreateFlags::raw();

    let cll = vci.create_logical_link(&create_type, &create_flags, None)
            .await
            .unwrap();
    
    let raw_can_stack = RawCanStack::default();
    
    stack.app_stack.p2_min = CpP2Min::ZERO;
    stack.app_stack.p2_max = CpP2Max::Millis(50);

    stack.transport_stack.configure_for_receive_11_bit(0x5BB); // receive the 0x5BB frame
    
    cll.set_com_params(stack.build_set()).await.unwrap();
    cll.set_unique_com_params_table(stack.build_table()).await.unwrap();
    cll.connect().await?;
    
    cll
  };

  // send the 0x63A frame and wait the 0x5BB frame
  let primitive = cll.create_send_recv_primitive(SendRecv::send_recv_raw_can(
    ClassicFrame::new_standard(0x63A, &[0x02, 0x3E, 0x00, 0x55, 0x55, 0x55, 0x55, 0x55]).unwrap()
  ));

  primitive.start().await.unwrap();
  
  let raw_response = primitive.get_result().await.unwrap();
  let frame_response = AbstractCanFrame::from_pdu_result_event(&raw_response);
  
  println!("ecu response: {frame_response:?}");
}
```

## Example 2. RAW CAN: monitor

```rust
use dpdu_wrapper::api::{PduApi};
use dpdu_wrapper::types::PduOptions;
use dpdu_wrapper::types::pdu_com_logical_link::{CllCreateFlags, CllCreateType, PduLogicalLink, SendRecv};
use dpdu_wrapper::types::pdu_com_param::single::timing::{CpP2Max, CpP2Min};
use dpdu_wrapper::types::pdu_com_param::stack::composite::RawCanStack;
use dpdu_wrapper::types::pdu_com_primitive::PrimitiveEvent;
use dpdu_wrapper::types::pdu_vci::PduVci;
use dpdu_wrapper::utils::can::{RawCanPrimitiveBuilderExt, ClassicFrame, AbstractCanFrame};
use dpdu_wrapper::utils::root_file::PduRootFile;
use dpdu_wrapper::worker::PduAsyncWorker;
use std::time::Duration;

#[tokio::main]
async fn main() {
  let root_file = PduRootFile::guess_and_parse().unwrap();
  let edic_mvci = root_file.get_mvci_by_short_name("EDIC_D_PDU_API_1_20_042").unwrap();
  
  let api = PduApi::from_mvci(edic_mvci, PduOptions::default()).unwrap();
  let worker = PduAsyncWorker::new(api);
  
  let vci_list = PduVci::list(&worker, None).await.unwrap();
  let vci = vci_list.into_iter().next().unwrap();
  
  vci.connect().await.unwrap();
  
  let cll = {
    let create_type = CllCreateType::raw_dw_can_on_obd();
    let create_flags = CllCreateFlags::raw();

    let cll = vci.create_logical_link(&create_type, &create_flags, None)
            .await
            .unwrap();
    
    let raw_can_stack = RawCanStack::default();
    
    cll.set_com_params(stack.build_set()).await.unwrap();
    cll.connect().await?;
    
    cll
  };
  
  let primitive = cll.create_send_recv_primitive(SendRecv::monitor());
  let mut primitive_event_rx = primitive.get_primitive_event_receiver().unwrap();
  
  tokio::spawn(async move {
    while let Ok(event) = primitive_event_rx.recv().await {
      match event {
        PrimitiveEvent::Status(status) => {
          if !status.is_alive() {
            break; // primitive is dead
          }
        },
        PrimitiveEvent::Result(result) => {
          let frame = AbstractCanFrame::from_pdu_result_event(&result).unwrap();
          info!("received frame: {frame:?}");
        },
        PrimitiveEvent::StartFailed(err) => {
          break; // unable to start
        },
        _ => {}
      }
    }  
  });
  
  primitive.start().await.unwrap();
  
  tokio::time::sleep(Duration::from_secs(100));
}
```

## Example 3. UDS on ISO TP on DW CAN.

```rust
use dpdu_wrapper::api::{PduApi};
use dpdu_wrapper::types::PduOptions;
use dpdu_wrapper::types::pdu_com_logical_link::{CllCreateFlags, CllCreateType, PduLogicalLink, SendRecv, StartComm};
use dpdu_wrapper::types::pdu_com_param::stack::composite::UdsOnIsoTpOnDwCanStack;
use dpdu_wrapper::types::pdu_com_primitive::PrimitiveEvent;
use dpdu_wrapper::types::pdu_vci::PduVci;
use dpdu_wrapper::utils::root_file::PduRootFile;
use dpdu_wrapper::worker::PduAsyncWorker;
use std::time::Duration;

#[tokio::main]
async fn main() {
  let root_file = PduRootFile::guess_and_parse().unwrap();
  let edic_mvci = root_file.get_mvci_by_short_name("EDIC_D_PDU_API_1_20_042").unwrap();
  
  let api = PduApi::from_mvci(edic_mvci, PduOptions::default()).unwrap();
  let worker = PduAsyncWorker::new(api);
  
  let vci_list = PduVci::list(&worker, None).await.unwrap();
  let vci = vci_list.into_iter().next().unwrap();
  
  vci.connect().await.unwrap();
  
  let cll = {
    let create_type = CllCreateType::uds_on_iso_tp_on_dw_can();
    let create_flags = CllCreateFlags::recommended();

    let cll = vci.create_logical_link(&create_type, &create_flags, None)
            .await
            .unwrap();
    
    let stack = UdsOnIsoTpOnDwCanStack::mercedes_benz_optimized(Some(1594), Some(1466)); // the CTRLC205 control unit

    cll.set_com_params(stack.build_set()).await?;
    cll.set_unique_com_params_table(stack.build_table()).await?;
    cll.connect().await?;
    
    cll
  };
  
  // Tester present handling.
  let start_comm = cll.create_start_comm_primitive(StartComm::initial());
  start_comm.start().await.unwrap();
  
  // MB ECU Identifcation - Software Logical Block Part Number(s) Read
  let primitive = cll.create_send_recv_primitive(SendRecv::new(Some(&[0x22, 0xF1, 0x21])));
  primitive.start().await.unwrap();

  let result = primitive.get_result().await.unwrap();
  println!("{:?}", result.data);
  
  // PduResultEvent {
  //   rx_flags: PduResultEventRxFlags([0, 0, 0, 0]),
  //   unique_resp_identifier: 0,
  //   acceptance_id: 1,
  //   timestamp_flags: PduResultEventTimestampFlags([0, 0, 0, 0]),
  //   tx_msg_done_timestamp: 0,
  //   start_msg_timestamp: 1308622953,
  //   data: [98, 241, 33, 50, 49, 51, 57, 48, 50, 50, 48, 48, 57],
  //     // SID-PR: 0x62
  //     // RecordDataIdentifier: 0xF1, 0x21
  //     // Software Logical Block #0 Part Number: 2139022009
  //   extra_info_header: None,
  //   extra_info_footer: None
  // }
}
```

## 🤝 Contributions
Contributions of any kind are welcome!
If you have ideas for new features, improvements, or bug fixes, feel free to open an issue or submit a pull request. Whether it's fixing a typo, improving the documentation, reporting a bug, or implementing a new feature, every contribution is appreciated.
Constructive feedback and suggestions are always welcome — they help make the project better for everyone.