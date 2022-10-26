mod codecs;
mod fast_path;
mod x224;

use std::sync::{Arc, Mutex};

use ironrdp::codecs::rfx::image_processing::PixelFormat;
use ironrdp::fast_path::FastPathError;
use ironrdp::{async_read_complete_pdu, RdpPdu};
use log::warn;
use tokio::io::AsyncWriteExt;

use crate::connection_sequence::ConnectionSequenceResult;
use crate::transport::{Decoder, RdpTransport};
use crate::{utils, InputConfig, RdpError, TlsStreamType};

const DESTINATION_PIXEL_FORMAT: PixelFormat = PixelFormat::RgbA32;

pub async fn process_active_stage(
    mut tls_stream: TlsStreamType,
    config: InputConfig,
    connection_sequence_result: ConnectionSequenceResult,
) -> Result<(), RdpError> {
    let decoded_image = Arc::new(Mutex::new(DecodedImage::new(
        u32::from(connection_sequence_result.desktop_sizes.width),
        u32::from(connection_sequence_result.desktop_sizes.height),
        DESTINATION_PIXEL_FORMAT,
    )));
    let mut x224_processor = x224::Processor::new(
        utils::swap_hashmap_kv(connection_sequence_result.joined_static_channels),
        config.global_channel_name.as_str(),
        config.graphics_config,
    );
    let mut fast_path_processor = fast_path::ProcessorBuilder {
        decoded_image,
        global_channel_id: connection_sequence_result.global_channel_id,
        initiator_id: connection_sequence_result.initiator_id,
    }
    .build();
    let mut rdp_transport = RdpTransport::default();
    let mut output_buffer = Vec::new();
    loop {
        output_buffer.clear();
        let stream = async_read_complete_pdu(&mut tls_stream).await?;
        let mut stream = stream.as_slice();
        match rdp_transport.decode(&mut stream) {
            Ok(RdpPdu::X224(data)) => {
                if let Err(error) = x224_processor.process(&mut stream, &mut output_buffer, data) {
                    match error {
                        RdpError::UnexpectedDisconnection(message) => {
                            warn!("User-Initiated disconnection on Server: {}", message);
                            break;
                        }
                        RdpError::UnexpectedChannel(channel_id) => {
                            warn!("Got message on a channel with {} ID", channel_id);
                            break;
                        }
                        err => {
                            return Err(err);
                        }
                    }
                }
            }
            Ok(RdpPdu::FastPath(header)) => {
                // skip header bytes in such way because here is possible
                // that data length was written in the not right way,
                // so we should skip only what has been actually read

                fast_path_processor.process(&header, &mut stream, &mut output_buffer)?;
            }
            Err(RdpError::FastPathError(FastPathError::NullLength { bytes_read: _ })) => {
                warn!("Received null-length Fast-Path packet, dropping it");
            }
            Err(e) => return Err(e),
        }

        if !output_buffer.is_empty() {
            tls_stream.write_all(&output_buffer).await?;
        }
    }

    Ok(())
}

pub struct DecodedImage {
    data: Vec<u8>,
}

impl DecodedImage {
    fn new(width: u32, height: u32, pixel_format: PixelFormat) -> Self {
        Self {
            data: vec![0; (width * height * u32::from(pixel_format.bytes_per_pixel())) as usize],
        }
    }

    fn get_mut(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
    }
}
