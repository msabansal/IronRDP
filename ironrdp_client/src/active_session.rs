mod codecs;
mod fast_path;
mod x224;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use std::io;

use ironrdp::codecs::rfx::image_processing::PixelFormat;

use ironrdp::fast_path::FastPathError;
use ironrdp::RdpPdu;
use log::warn;

use crate::connection_sequence::ConnectionSequenceResult;
use crate::transport::{Decoder, RdpTransport};
use crate::{utils, InputConfig, RdpError};
#[cfg(feature = "gui")]
mod gui;

const DESTINATION_PIXEL_FORMAT: PixelFormat = PixelFormat::RgbA32;

pub fn process_active_stage(
    stream: impl io::Read + io::Write + Send + 'static,
    config: InputConfig,
    connection_sequence_result: ConnectionSequenceResult,
) -> Result<(), RdpError> {
    let decoded_image = Arc::new(Mutex::new(DecodedImage::new(
        u32::from(connection_sequence_result.desktop_sizes.width),
        u32::from(connection_sequence_result.desktop_sizes.height),
        DESTINATION_PIXEL_FORMAT,
    )));

    let fast_path_processor = fast_path::ProcessorBuilder {
        decoded_image,
        global_channel_id: connection_sequence_result.global_channel_id,
        initiator_id: connection_sequence_result.initiator_id,
    }
    .build();

    #[cfg(not(feature = "gui"))]
    {
        let x224_processor = x224::Processor::new(
            utils::swap_hashmap_kv(connection_sequence_result.joined_static_channels),
            config.global_channel_name.clone(),
            config.graphics_config,
            None,
        );
        let x224_processor = Arc::new(Mutex::new(x224_processor));
        process_inner(stream, x224_processor, fast_path_processor)
    }

    #[cfg(feature = "gui")]
    {
        use crate::active_session::gui::launch_gui;
        use crate::utils::cloneable_stream::CloneableStream;
        use std::sync::mpsc::sync_channel;
        use std::thread;

        let (sender, receiver) = sync_channel(1);
        let stream = CloneableStream::new(stream);
        let stream2 = stream.clone();
        let config2 = config.clone();
        let x224_processor = x224::Processor::new(
            utils::swap_hashmap_kv(connection_sequence_result.joined_static_channels),
            config.global_channel_name.clone(),
            config.graphics_config,
            Some(sender),
        );
        let x224_processor = Arc::new(Mutex::new(x224_processor));
        let processor2 = x224_processor.clone();
        thread::spawn(move || {
            let result = process_inner(stream2, x224_processor, fast_path_processor);
            log::info!("Result: {:?}", result);
        });

        launch_gui(
            config2.width,
            config2.height,
            config2.gfx_dump_file,
            receiver,
            Some(stream),
            Some(processor2),
        )
    }
}

fn process_inner(
    mut stream: impl Read + Write,
    x224_processor: Arc<Mutex<x224::Processor>>,
    mut fast_path_processor: fast_path::Processor,
) -> Result<(), RdpError> {
    let mut rdp_transport = RdpTransport::default();
    let mut output_buffer = Vec::<u8>::new();
    loop {
        output_buffer.clear();
        match rdp_transport.decode(&mut stream) {
            Ok(RdpPdu::X224(data)) => {
                let mut x224_processor = x224_processor.lock().map_err(|_| RdpError::LockPoisonedError)?;
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
                // let bytes_read = input_buffer_length - input_buffer.len();
                // stream.consume(bytes_read);
                output_buffer.clear();
                fast_path_processor.process(&header, &mut stream, &mut output_buffer)?;
            }
            Err(RdpError::FastPathError(FastPathError::NullLength { bytes_read })) => {
                warn!("Received null-length Fast-Path packet, dropping it");
                let mut data = vec![0u8; bytes_read];
                stream.read_exact(data.as_mut_slice())?;
            }
            Err(e) => return Err(e),
        }
        if !output_buffer.is_empty() {
            stream.write_all(output_buffer.as_slice())?;
            stream.flush()?;
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
