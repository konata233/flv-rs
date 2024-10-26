use crate::flv::header::FlvHeader;
use crate::flv::meta::RawMetaData;
use crate::fmpeg::encoder::{DEFAULT_AUDIO_TRACK_ID, DEFAULT_VIDEO_TRACK_ID};
use crate::fmpeg::mp4head::avc1_utils::AvcCBoxLike;
use crate::fmpeg::parser::{AudioParseResult, Avc1ParseResult, Channel, VideoParseResult};

pub enum TrackType {
    Audio,
    Video,
}

pub struct TrackContext {
    pub track_id: u32,
    pub sequence_number: u32,

    pub track_type: TrackType,
}

impl TrackContext {
    pub fn new(track_id: u32, track_type: TrackType) -> Self {
        Self {
            track_id,
            sequence_number: 1,
            track_type,
        }
    }
}

pub struct SampleContext {
    pub is_leading: bool,
    pub is_non_sync: bool,
    pub is_keyframe: bool,
    pub has_redundancy: bool,

    pub decode_time: u32,
    pub composition_time_offset: u32, // most of the time this can be set to 0.
    // dts   +  cts    =   pts
    // decode   offset     presentation
    pub sample_duration: u32,
    pub sample_size: u32,
}

pub const TIME_SCALE: u32 = 1000;

pub struct RemuxContext {
    pub fps: f64,
    pub fps_num: u32,

    pub duration_ms: u32,

    pub width: f64,
    pub height: f64,

    pub has_audio: bool,
    pub has_video: bool,

    pub audio_codec_id: u8,
    pub audio_codec_type: AudioCodecType,
    pub audio_data_rate: u32,

    // --- must be initialized using audio tag data ---
    pub audio_sample_rate: u32,
    pub audio_channels: u8,
    pub audio_channels_extended: u8,
    pub audio_aac_info: Vec<u8>,
    // ------------------------------------------------

    pub video_codec_id: u8,
    pub video_codec_type: VideoCodecType,

    // --- must be initialized using video tag data ---
    pub video_data_rate: u32,
    pub video_avcc_info: AvcCBoxLike,
    // ------------------------------------------------

    pub major_brand: String,
    pub minor_version: String,
    pub compatible_brands: Vec<String>,

    pub audio_track: TrackContext,
    pub video_track: TrackContext,

    header_sent: bool,
    flv_header_configured: bool,
    metadata_configured: bool,
    video_metadata_configured: bool,
    audio_metadata_configured: bool,
}

pub enum VideoCodecType {
    Avc1,
    None
}

impl From<u8> for VideoCodecType {
    fn from(value: u8) -> Self {
        match value {
            7 => VideoCodecType::Avc1,
            _ => VideoCodecType::None
        }
    }
}

pub enum AudioCodecType {
    Aac,
    Mp3,
    None,
}

impl From<u8> for AudioCodecType {
    fn from(value: u8) -> Self {
        match value {
            10 => AudioCodecType::Aac,
            2 => AudioCodecType::Mp3,
            _ => AudioCodecType::None
        }
    }
}

impl RemuxContext {
    pub fn new() -> Self {
        Self {
            fps: 0.0,
            fps_num: 0,
            duration_ms: 0,

            width: 0.0,
            height: 0.0,

            has_audio: false,
            has_video: false,

            audio_codec_id: 0,
            audio_data_rate: 0,
            audio_sample_rate: 0,
            audio_channels: 0,
            audio_channels_extended: 0,
            audio_aac_info: vec![],

            video_codec_id: 0,
            video_data_rate: 0,
            video_avcc_info: AvcCBoxLike::AvcCBoxLike(vec![]),

            major_brand: String::from("isom"),
            minor_version: String::from("512"),
            compatible_brands: vec![String::from("isom"), String::from("iso2"), String::from("avc1"), String::from("mp41")],

            video_codec_type: VideoCodecType::None,
            audio_codec_type: AudioCodecType::None,

            audio_track: TrackContext::new(DEFAULT_AUDIO_TRACK_ID, TrackType::Audio),
            video_track: TrackContext::new(DEFAULT_VIDEO_TRACK_ID, TrackType::Video),

            header_sent: false,
            flv_header_configured: false,
            metadata_configured: false,
            video_metadata_configured: false,
            audio_metadata_configured: false,
        }
    }

    pub fn parse_flv_header(&mut self, header: &FlvHeader) {
        self.has_audio = header.type_flags_audio;
        self.has_video = header.type_flags_video;
        self.flv_header_configured = true;
    }

    pub fn parse_metadata(&mut self, metadata: &RawMetaData) {
        if let Some(duration) = metadata.try_get_number("duration") {
            self.duration_ms = (duration * TIME_SCALE as f64) as u32;
        }

        if let Some(width) = metadata.try_get_number("width") {
            self.width = width;
        }

        if let Some(height) = metadata.try_get_number("height") {
            self.height = height;
        }

        if let Some(frame_rate) = metadata.try_get_number("framerate") {
            self.fps = frame_rate;
            self.fps_num = (frame_rate * TIME_SCALE as f64) as u32;
        }

        if let Some(audio_codec_id) = metadata.try_get_number("audiocodecid") {
            self.audio_codec_id = audio_codec_id as u8;
            self.audio_codec_type = AudioCodecType::from(self.audio_codec_id);
        }

        if let Some(audio_data_rate) = metadata.try_get_number("audiodatarate") {
            self.audio_data_rate = audio_data_rate as u32;
        }

        if let Some(video_codec_id) = metadata.try_get_number("videocodecid") {
            self.video_codec_id = video_codec_id as u8;
            self.video_codec_type = VideoCodecType::from(self.video_codec_id);
        }

        if let Some(video_data_rate) = metadata.try_get_number("videodatarate") {
            self.video_data_rate = video_data_rate as u32;
        }

        if let Some(major_brand) = metadata.try_get_string("major_brand") {
            self.major_brand = major_brand;
        } else {
            self.major_brand = String::from("isom");
        }

        if let Some(minor_version) = metadata.try_get_string("minor_version") {
            self.minor_version = minor_version;
        } else {
            self.minor_version = String::from("512");
        }

        if let Some(mut compatible_brands) = metadata.try_get_string("compatible_brands") {
            self.compatible_brands.push(String::from_iter(compatible_brands.drain(0..4)));
            self.compatible_brands.push(String::from_iter(compatible_brands.drain(0..4)));
            self.compatible_brands.push(String::from_iter(compatible_brands.drain(0..4)));
            self.compatible_brands.push(String::from_iter(compatible_brands.drain(0..4)));
        } else {
            self.compatible_brands.push(String::from("isom"));
            self.compatible_brands.push(String::from("iso2"));
            self.compatible_brands.push(String::from("avc1"));
            self.compatible_brands.push(String::from("mp41"));
        }

        self.metadata_configured = true;
    }

    const AAC_SAMPLE_RATES: [u32; 13] = [
        96000, 88200, 64000, 48000,
        44100, 32000, 24000, 22050,
        16000, 12000, 11025, 8000,
        7350
    ];
    pub fn configure_audio_metadata(&mut self, audio_metadata: &AudioParseResult) {
        match audio_metadata {
            AudioParseResult::AacSequenceHeader(aac_info) => {
                if self.audio_codec_id != 10 {
                    panic!("audio type mismatch: expected aac.");
                }

                self.audio_channels = aac_info.channel_configuration;
                if aac_info.sampling_frequency_index > 12 {
                    panic!("invalid aac sample rate index");
                }
                self.audio_sample_rate = Self::AAC_SAMPLE_RATES[aac_info.sampling_frequency_index as usize];
                self.audio_aac_info = Vec::from(aac_info.raw.clone());
            }
            AudioParseResult::Mp3(mp3_info) => {
                if self.audio_codec_id != 2 {
                    panic!("audio type mismatch: expected mp3.");
                }

                self.audio_channels = match mp3_info.channel {
                    Channel::Mono => {
                        1
                    },
                    Channel::Dual => {
                        2
                    }
                    Channel::Stereo => {
                        2
                    }
                    Channel::JointStereo => {
                        self.audio_channels_extended = mp3_info.channel_extended;
                        2
                    }
                };
                self.audio_sample_rate = mp3_info.sample_rate;
            }
            _ => {
                // raw data, do nothing.
                return;
            }
        }

        self.audio_metadata_configured = true;
        // todo: test this.
    }

    pub fn configure_video_metadata(&mut self, video_metadata: &VideoParseResult) {
        match video_metadata {
            VideoParseResult::Avc1(h264_info) => {
                match h264_info {
                    Avc1ParseResult::AvcSequenceHeader(header) => {
                        self.video_avcc_info = AvcCBoxLike::AvcCBoxLike(Vec::from(header.clone()));
                    }
                    Avc1ParseResult::AvcEndOfSequence => {
                        // todo: handle this.
                    }
                    _ => {
                        // raw data, do nothing.
                        return;
                    }
                }
            }
            _ => {}
        }
        self.video_metadata_configured = true;
    }

    pub fn is_configured(&self) -> bool {
        self.flv_header_configured &&
            self.metadata_configured &&
            self.video_metadata_configured &&
            self.audio_metadata_configured
    }

    /// for testing only!!
    pub fn _set_configured(&mut self, flag: bool) {
        self.metadata_configured = flag;
        self.flv_header_configured = flag;
        self.video_metadata_configured = flag;
        self.audio_metadata_configured = flag;
    }

    pub fn is_header_sent(&self) -> bool {
        self.header_sent
    }

    pub fn set_header_sent(&mut self, flag: bool) {
        self.header_sent = flag;
    }
}