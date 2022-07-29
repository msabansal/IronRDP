mod gfx;
mod display;

use std::collections::HashMap;
use std::{cmp, io};

use futures_channel::mpsc;
use ironrdp::dvc::FieldType;
use ironrdp::dvc::gfx::ServerPdu;
use ironrdp::rdp::vc::{self, dvc};
use ironrdp::rdp::{ErrorInfo, ProtocolIndependentCode, ServerSetErrorInfoPdu};
use ironrdp::{Data, ShareDataPdu};
use log::{debug, error};

use crate::transport::{
    Decoder, DynamicVirtualChannelTransport, Encoder, SendDataContextTransport, ShareControlHeaderTransport,
    ShareDataHeaderTransport, StaticVirtualChannelTransport, ChannelIdentificators,
};
use crate::RdpError;

pub const RDP8_GRAPHICS_PIPELINE_NAME: &str = "Microsoft::Windows::RDS::Graphics";
pub const RDP8_DISPLAY_PIPELINE_NAME: &str = "Microsoft::Windows::RDS::DisplayControl";

pub struct Processor<'a> {
    static_channels: HashMap<u16, String>,
    channel_map: HashMap<String, u32>,
    dynamic_channels: HashMap<u32, DynamicChannel>,
    global_channel_name: &'a str,
    gfx_handler: Option<mpsc::UnboundedSender<ServerPdu>>,
    user_id: u16,
    drdynvc_channel_id: u16,
}

impl<'a> Processor<'a> {
    pub fn new(
        static_channels: HashMap<u16, String>,
        global_channel_name: &'a str,
        user_id: u16,
        gfx_handler: Option<mpsc::UnboundedSender<ServerPdu>>,
    ) -> Self {
        Self {
            static_channels,
            dynamic_channels: HashMap::new(),
            channel_map: HashMap::new(),
            global_channel_name,
            user_id,
            drdynvc_channel_id: 0,
            gfx_handler,
        }
    }

    pub fn process(&mut self, mut stream: impl io::BufRead + io::Write, data: Data) -> Result<(), RdpError> {
        let mut transport = SendDataContextTransport::default();
        transport.mcs_transport.0.set_decoded_context(data.data_length);

        let channel_ids = transport.decode(&mut stream)?;
        transport.set_decoded_context(channel_ids);

        let channel_id = channel_ids.channel_id;
        let initiator_id = channel_ids.initiator_id;
        match self
            .static_channels
            .get(&channel_id)
            .map(String::as_str)
        {
            Some(vc::DRDYNVC_CHANNEL_NAME) => {
                let transport = DynamicVirtualChannelTransport::new(
                    StaticVirtualChannelTransport::new(transport),
                    channel_id,
                );
                self.drdynvc_channel_id = channel_id;
                self.process_dvc_message(&mut stream, channel_id, transport)
            }
            Some(name) if name == self.global_channel_name => {
                let transport = ShareDataHeaderTransport::new(ShareControlHeaderTransport::new(
                    transport,
                    initiator_id,
                    channel_id,
                ));

                process_global_channel_pdu(&mut stream, transport)
            }
            Some(_) => Err(RdpError::UnexpectedChannel(channel_id)),
            None => panic!("Channel with {} ID must be added", channel_id),
        }
    }

    pub fn send(
        &mut self,
        mut stream: impl io::Write,
        channel_name: &str,
        message: Vec<u8>,
    ) -> Result<(), RdpError> {
        let channel_id = self
            .channel_map.get(channel_name)
            .ok_or(RdpError::AccessToNonExistingChannelName(channel_name.to_string()))?;
        let channel = self
                .dynamic_channels
                .get_mut(channel_id)
                .ok_or(RdpError::AccessToNonExistingChannel(*channel_id))?;

        let transport = SendDataContextTransport::default();
        let mut transport = StaticVirtualChannelTransport::new(transport);
        transport.set_channel_ids(ChannelIdentificators{
            initiator_id: self.user_id,
            channel_id: self.drdynvc_channel_id,
        });
        
        let client_data = dvc::ClientPdu::Data(dvc::DataPdu {
            channel_id_type: channel.channel_id_type,
            channel_id: channel.channel_id,
            data_size: message.len(),
        });
        
        transport.encode(
            DynamicVirtualChannelTransport::prepare_data_to_encode(
                client_data,
                Some(message),
            )?,
            &mut stream,
        )?;
        Ok(())
    }

    fn process_dvc_message(
        &mut self,
        mut stream: impl io::BufRead + io::Write,
        _channel_id: u16,
        mut transport: DynamicVirtualChannelTransport,
    ) -> Result<(), RdpError> {
        match transport.decode(&mut stream)? {
            dvc::ServerPdu::CapabilitiesRequest(caps_request) => {
                debug!("Got DVC Capabilities Request PDU: {:?}", caps_request);
                let caps_response = dvc::ClientPdu::CapabilitiesResponse(dvc::CapabilitiesResponsePdu {
                    version: dvc::CapsVersion::V1,
                });

                debug!("Send DVC Capabilities Response PDU: {:?}", caps_response);
                transport.encode(
                    DynamicVirtualChannelTransport::prepare_data_to_encode(caps_response, None)?,
                    &mut stream,
                )?;
            }
            dvc::ServerPdu::CreateRequest(create_request) => {
                debug!("Got DVC Create Request PDU: {:?}", create_request);

                let creation_status = if let Some(dyncamic_channel) = create_dvc(
                    create_request.channel_name.as_str(),
                    create_request.channel_id,
                    create_request.channel_id_type,
                    &self.gfx_handler
                ) {
                    self.dynamic_channels
                        .insert(create_request.channel_id, dyncamic_channel);
                    self.channel_map.insert(create_request.channel_name.clone(), create_request.channel_id);

                    dvc::DVC_CREATION_STATUS_OK
                } else {
                    dvc::DVC_CREATION_STATUS_NO_LISTENER
                };

                let create_response = dvc::ClientPdu::CreateResponse(dvc::CreateResponsePdu {
                    channel_id_type: create_request.channel_id_type,
                    channel_id: create_request.channel_id,
                    creation_status,
                });

                debug!("Send DVC Create Response PDU: {:?}", create_response);
                transport.encode(
                    DynamicVirtualChannelTransport::prepare_data_to_encode(create_response, None)?,
                    &mut stream,
                )?;

                negotiate_dvc(&create_request, transport, &mut stream)?;
            }
            dvc::ServerPdu::CloseRequest(close_request) => {
                debug!("Got DVC Close Request PDU: {:?}", close_request);

                let close_response = dvc::ClientPdu::CloseResponse(dvc::ClosePdu {
                    channel_id_type: close_request.channel_id_type,
                    channel_id: close_request.channel_id,
                });

                debug!("Send DVC Close Response PDU: {:?}", close_response);
                transport.encode(
                    DynamicVirtualChannelTransport::prepare_data_to_encode(close_response, None)?,
                    &mut stream,
                )?;

                self.dynamic_channels.remove(&close_request.channel_id);
            }
            dvc::ServerPdu::DataFirst(data) => {
                let channel_id_type = data.channel_id_type;
                let channel_id = data.channel_id;
                let mut data_buff = vec![0; data.data_size];
                stream.read_exact(&mut data_buff)?;

                if let Some(dvc_data) = self
                    .dynamic_channels
                    .get_mut(&data.channel_id)
                    .ok_or(RdpError::AccessToNonExistingChannel(data.channel_id))?
                    .process_data_first_pdu(data.total_data_size as usize, data_buff)?
                {
                    let client_data = dvc::ClientPdu::Data(dvc::DataPdu {
                        channel_id_type,
                        channel_id,
                        data_size: dvc_data.len(),
                    });

                    transport.encode(
                        DynamicVirtualChannelTransport::prepare_data_to_encode(client_data, Some(dvc_data))?,
                        &mut stream,
                    )?;
                }
            }
            dvc::ServerPdu::Data(data) => {
                let channel_id_type = data.channel_id_type;
                let channel_id = data.channel_id;
                let mut data_buff = vec![0; data.data_size];
                stream.read_exact(&mut data_buff)?;

                if let Some(dvc_data) = self
                    .dynamic_channels
                    .get_mut(&data.channel_id)
                    .ok_or(RdpError::AccessToNonExistingChannel(data.channel_id))?
                    .process_data_pdu(data_buff)?
                {
                    let client_data = dvc::ClientPdu::Data(dvc::DataPdu {
                        channel_id_type,
                        channel_id,
                        data_size: dvc_data.len(),
                    });

                    transport.encode(
                        DynamicVirtualChannelTransport::prepare_data_to_encode(client_data, Some(dvc_data))?,
                        &mut stream,
                    )?;
                }
            }
        }

        Ok(())
    }
}

fn process_global_channel_pdu(
    mut stream: impl io::BufRead + io::Write,
    mut transport: ShareDataHeaderTransport,
) -> Result<(), RdpError> {
    let share_data_pdu = transport.decode(&mut stream)?;

    match share_data_pdu {
        ShareDataPdu::SaveSessionInfo(session_info) => {
            debug!("Got Session Save Info PDU: {:?}", session_info);

            Ok(())
        }
        ShareDataPdu::ServerSetErrorInfo(ServerSetErrorInfoPdu(ErrorInfo::ProtocolIndependentCode(
            ProtocolIndependentCode::None,
        ))) => {
            debug!("Received None server error");

            Ok(())
        }
        ShareDataPdu::ServerSetErrorInfo(ServerSetErrorInfoPdu(e)) => Err(RdpError::ServerError(e.description())),
        _ => Err(RdpError::UnexpectedPdu(format!(
            "Expected Session Save Info PDU, got: {:?}",
            share_data_pdu.as_short_name()
        ))),
    }
}

fn create_dvc(
    channel_name: &str,
    channel_id: u32,
    channel_id_type: FieldType,
    gfx_handler: &Option<mpsc::UnboundedSender<ServerPdu>>,
) -> Option<DynamicChannel> {
    match channel_name {
        RDP8_GRAPHICS_PIPELINE_NAME => Some(DynamicChannel::new(Box::new(gfx::Handler::new(gfx_handler.clone())), channel_id, channel_id_type)),
        RDP8_DISPLAY_PIPELINE_NAME => Some(DynamicChannel::new(Box::new(display::Handler::new()),
         channel_id, channel_id_type)),
        _ => {
            error!("Unknown channel name: {}", channel_name);
            None
        },
    }
}

fn negotiate_dvc(
    create_request: &dvc::CreateRequestPdu,
    mut transport: DynamicVirtualChannelTransport,
    mut stream: impl io::Write,
) -> Result<(), RdpError> {
    if create_request.channel_name == RDP8_GRAPHICS_PIPELINE_NAME {
        let dvc_data = gfx::create_capabilities_advertise()?;
        let client_data = dvc::ClientPdu::Data(dvc::DataPdu {
            channel_id_type: create_request.channel_id_type,
            channel_id: create_request.channel_id,
            data_size: dvc_data.len(),
        });

        debug!("Send GFX Capabilities Advertise PDU");
        transport.encode(
            DynamicVirtualChannelTransport::prepare_data_to_encode(client_data, Some(dvc_data))?,
            &mut stream,
        )?;
    }

    Ok(())
}

trait DynamicChannelDataHandler {
    fn process_complete_data(&mut self, complete_data: Vec<u8>) -> Result<Option<Vec<u8>>, RdpError>;
}

pub struct DynamicChannel {
    data: CompleteData,
    channel_id_type: FieldType,
    channel_id: u32,
    handler: Box<dyn DynamicChannelDataHandler>,
}

impl DynamicChannel {
    fn new(handler: Box<dyn DynamicChannelDataHandler>,
        channel_id: u32,
        channel_id_type: FieldType,
    ) -> Self {
        Self {
            data: CompleteData::new(),
            handler,
            channel_id_type,
            channel_id,
        }
    }

    fn process_data_first_pdu(&mut self, total_data_size: usize, data: Vec<u8>) -> Result<Option<Vec<u8>>, RdpError> {
        if let Some(complete_data) = self.data.process_data_first_pdu(total_data_size, data) {
            self.handler.process_complete_data(complete_data)
        } else {
            Ok(None)
        }
    }

    fn process_data_pdu(&mut self, data: Vec<u8>) -> Result<Option<Vec<u8>>, RdpError> {
        if let Some(complete_data) = self.data.process_data_pdu(data) {
            self.handler.process_complete_data(complete_data)
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, PartialEq)]
struct CompleteData {
    total_size: usize,
    data: Vec<u8>,
}

impl CompleteData {
    fn new() -> Self {
        Self {
            total_size: 0,
            data: Vec::new(),
        }
    }

    fn process_data_first_pdu(&mut self, total_data_size: usize, data: Vec<u8>) -> Option<Vec<u8>> {
        if self.total_size != 0 || !self.data.is_empty() {
            error!("Incomplete DVC message, it will be skipped");

            self.data.clear();
        }

        if total_data_size == data.len() {
            Some(data)
        } else {
            self.total_size = total_data_size;
            self.data = data;

            None
        }
    }

    fn process_data_pdu(&mut self, mut data: Vec<u8>) -> Option<Vec<u8>> {
        if self.total_size == 0 && self.data.is_empty() {
            // message is not fragmented
            Some(data)
        } else {
            // message is fragmented so need to reassemble it
            let actual_data_length = self.data.len() + data.len();

            match actual_data_length.cmp(&(self.total_size)) {
                cmp::Ordering::Less => {
                    // this is one of the fragmented messages, just append it
                    self.data.append(&mut data);
                    None
                }
                cmp::Ordering::Equal => {
                    // this is the last fragmented message, need to return the whole reassembled message
                    self.total_size = 0;
                    self.data.append(&mut data);
                    Some(self.data.drain(..).collect())
                }
                cmp::Ordering::Greater => {
                    error!("Actual DVC message size is grater than expected total DVC message size");
                    self.total_size = 0;
                    self.data.clear();

                    None
                }
            }
        }
    }
}
