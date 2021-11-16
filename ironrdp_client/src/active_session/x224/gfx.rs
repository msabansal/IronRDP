use futures_channel::mpsc;
use ironrdp::{
    dvc::gfx::{
        zgfx, CapabilitiesAdvertisePdu, CapabilitiesV104Flags, CapabilitiesV10Flags,
        CapabilitiesV81Flags, CapabilitiesV8Flags, CapabilitySet, ClientPdu, FrameAcknowledgePdu,
        QueueDepth, ServerPdu,
    },
    PduParsing,
};
use log::debug;

use super::DynamicChannelDataHandler;
use crate::RdpError;

pub struct Handler {
    decompressor: zgfx::Decompressor,
    decompressed_buffer: Vec<u8>,
    frames_decoded: u32,
    handler: Option<mpsc::UnboundedSender<ServerPdu>>,
}

impl Handler {
    pub fn new(handler: Option<mpsc::UnboundedSender<ServerPdu>>) -> Self {
        Self {
            decompressor: zgfx::Decompressor::new(),
            decompressed_buffer: Vec::with_capacity(1024 * 16),
            frames_decoded: 0,
            handler,
        }
    }
}

impl DynamicChannelDataHandler for Handler {
    fn process_complete_data(&mut self, complete_data: Vec<u8>) -> Result<Option<Vec<u8>>, RdpError> {
        let mut client_pdu_buffer: Vec<u8> = vec![];
        self.decompressed_buffer.clear();
        self.decompressor
            .decompress(complete_data.as_slice(), &mut self.decompressed_buffer)?;
        let mut slice = &mut self.decompressed_buffer.as_slice();
        while !slice.is_empty() {
            let gfx_pdu = ServerPdu::from_buffer(&mut slice)?;
            debug!("Got GFX PDU: {:?}", gfx_pdu);

            if let ServerPdu::EndFrame(end_frame_pdu) = gfx_pdu {
                self.frames_decoded += 1;
                let client_pdu = ClientPdu::FrameAcknowledge(FrameAcknowledgePdu {
                    queue_depth: QueueDepth::Suspend,
                    frame_id: end_frame_pdu.frame_id,
                    total_frames_decoded: self.frames_decoded,
                });
                debug!("Sending GFX PDU: {:?}", client_pdu);
                client_pdu_buffer.reserve(client_pdu_buffer.len() + client_pdu.buffer_length());
                client_pdu.to_buffer(&mut client_pdu_buffer)?;
            } else if let Some(handler) = &self.handler {
                handler.unbounded_send(gfx_pdu).unwrap();
            }
        }

        if client_pdu_buffer.len() > 0 {
            return Ok(Some(client_pdu_buffer));
        }

        return Ok(None);
    }
}

pub fn create_capabilities_advertise() -> Result<Vec<u8>, RdpError> {
    let capabilities_advertise = ClientPdu::CapabilitiesAdvertise(CapabilitiesAdvertisePdu(vec![
        CapabilitySet::V8 {
            flags: CapabilitiesV8Flags::empty(),
        },
        CapabilitySet::V8_1 {
            flags: CapabilitiesV81Flags::AVC420_ENABLED,
        },
        CapabilitySet::V10 {
            flags: CapabilitiesV10Flags::empty(),
        },
        CapabilitySet::V10_6 {
            flags: CapabilitiesV104Flags::SMALL_CACHE | CapabilitiesV104Flags::AVC_THIN_CLIENT,
        },
    ]));
    let mut capabilities_advertise_buffer =
        Vec::with_capacity(capabilities_advertise.buffer_length());
    capabilities_advertise.to_buffer(&mut capabilities_advertise_buffer)?;

    Ok(capabilities_advertise_buffer)
}
