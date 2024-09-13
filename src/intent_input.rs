//! A libafl [Input] representing a single intent.

use std::fmt;
use std::{fmt::Write, hash::Hasher};
use strum_macros::EnumIter;

use libafl::prelude::{BytesInput, HasBytesVec, Input};
use serde::{Deserialize, Serialize};

use fasthash::{farm::Hasher128, FastHasher, HasherExt};

use crate::util::encode_hex;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IntentInput {
    // The stuff up here usually doesn't get mutated because it is needed for
    // the intent to even match and hit the intent receiver.
    /// The type of the receiver (i.e., activity, service, or broadcast receiver)
    pub receiver_type: ReceiverType,
    /// The component that receives the intent, e.g
    /// `com.example.app/.ExampleActivity`
    pub component_package: String,
    pub component_class: String,
    /// The action of the intent.
    pub action: String,
    /// The category of the intent.
    pub category: String,

    // These fields get mutated!
    /// The `data` uri component of the Input, raw UTF-8 bytes.
    pub data: Option<URIInput>,
    // The `type`, a mime type for the data.
    pub mime_type: MimeType,
    // The `flags` for the intent.
    pub flags: u32,
    // The `extras` for the intent.
    pub extras: Vec<ExtraInput>,
}

impl IntentInput {
    /// Command to send this intent via adb shell.
    pub fn shell_command(&self) -> String {
        // The way adb shell handles commands is documented here:
        //   https://developer.android.com/studio/command-line/adb#shellcommands
        // but basically we need to generate the command we want to run as
        // a single string fit for use on the android shell.
        let am_command = match self.receiver_type {
            // Activity
            ReceiverType::Activity => "start",
            // Broadcast Receiver
            ReceiverType::BroadcastReceiver => "broadcast",
            // Service is not yet implemented
            _ => panic!("Unsupported receiver type"),
        };

        let mut command = format!(
            "am {} -n '{}' -a '{}' -t '{}' --grant-read-uri-permission ",
            am_command,
            self.component(),
            self.action,
            self.mime_type
        );

        // Append data to the shell_command if it exists.
        if let Some(data) = &self.data {
            write!(&mut command, " -d '{}'", data.identifier(0)).unwrap();
        }

        // Append category to the shell_command if it exists.
        if !self.category.is_empty() {
            write!(&mut command, " -c {}", self.category).unwrap();
        }

        // Append extras to the shell_command.
        let extras_command = self
            .extras
            .iter()
            .enumerate()
            .filter_map(|(index, extra)| extra.command_args(index + 1))
            .collect::<Vec<_>>()
            .join(" ");

        write!(&mut command, " ").unwrap();
        command.push_str(&extras_command);

        command
    }

    /// Creates a unique hash of this input.
    pub fn hash(&self) -> String {
        let mut hasher = Hasher128::new();

        hasher.write(self.component().as_bytes());
        hasher.write(self.action.as_bytes());
        hasher.write(self.category.as_bytes());
        hasher.write(&serde_json::to_vec(&self.data).unwrap());
        hasher.write(self.mime_type.to_string().as_bytes());
        hasher.write(&self.flags.to_le_bytes());

        for extra in &self.extras {
            hasher.write(extra.key.as_bytes());
            hasher.write(&serde_json::to_vec(&extra.value).unwrap());
        }

        format!("{:032x}", hasher.finish_ext())
    }

    /// The component that receives the intent, e.g
    /// `com.example.app/.ExampleActivity`
    pub fn component(&self) -> String {
        format!("{}/{}", self.component_package, self.component_class)
    }
}

impl Input for IntentInput {
    /// Generate a name for this input
    #[must_use]
    fn generate_name(&self, idx: usize) -> String {
        format!("id_{idx}_{hash}", idx = idx, hash = self.hash())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, EnumIter, Copy, PartialEq)]
pub enum ReceiverType {
    Activity,
    Service,
    BroadcastReceiver,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExtraInput {
    // The `key` of the extra input.
    pub key: String,
    // The type of the extra input (for example, s, z, i, f).
    pub value: ExtraType,
}

impl ExtraInput {
    /// The command line arguments for this extra input.
    pub fn command_args(&self, index: usize) -> Option<String> {
        let arg_string = match &self.value {
            ExtraType::URI(uri_input) => Some(uri_input.identifier(index)),
            ExtraType::String(d_input) => Some(encode_hex(d_input.buffer.bytes())),
            ExtraType::Boolean(d_input) => {
                if d_input.buffer.bytes().get(0) == Some(&0) {
                    Some("false".to_string())
                } else {
                    Some("true".to_string())
                }
            }
            ExtraType::Int(d_input) => {
                Some(i32::from_le_bytes(d_input.buffer.bytes().try_into().ok()?).to_string())
            }
            ExtraType::Long(d_input) => {
                Some(i64::from_le_bytes(d_input.buffer.bytes().try_into().ok()?).to_string())
            }
            ExtraType::Float(d_input) => {
                let value_f32 = f32::from_le_bytes(d_input.buffer.bytes().try_into().ok()?);
                if value_f32.is_infinite() {
                    Some(if value_f32.is_sign_positive() {
                        "Infinity".to_string()
                    } else {
                        "-Infinity".to_string()
                    })
                } else if value_f32.is_nan() {
                    Some("NaN".to_string())
                } else {
                    Some(value_f32.to_string())
                }
            }
            ExtraType::IntArray(d_input) | ExtraType::IntArrayList(d_input) => {
                let values: Vec<i32> = d_input
                    .buffer
                    .bytes()
                    .chunks(4)
                    .map(|chunk| {
                        let mut bytes = [0u8; 4];
                        bytes[..chunk.len()].copy_from_slice(chunk);
                        i32::from_le_bytes(bytes)
                    })
                    .collect();

                let output = values
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");

                Some(output).filter(|output| !output.is_empty())
            }
            ExtraType::LongArray(d_input) | ExtraType::LongArrayList(d_input) => {
                let values: Vec<i64> = d_input
                    .buffer
                    .bytes()
                    .chunks(8)
                    .map(|chunk| {
                        let mut bytes = [0u8; 8];
                        bytes[..chunk.len()].copy_from_slice(chunk);
                        i64::from_le_bytes(bytes)
                    })
                    .collect();

                let output = values
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");

                Some(output).filter(|output| !output.is_empty())
            }
            ExtraType::FloatArray(d_input) | ExtraType::FloatArrayList(d_input) => {
                let values: Vec<f32> = d_input
                    .buffer
                    .bytes()
                    .chunks(4)
                    .map(|chunk| {
                        let mut bytes = [0u8; 4];
                        bytes[..chunk.len()].copy_from_slice(chunk);
                        f32::from_le_bytes(bytes)
                    })
                    .collect();

                let output = values
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");

                Some(output).filter(|output| !output.is_empty())
            }
            ExtraType::StringArray(d_input) | ExtraType::StringArrayList(d_input) => {
                let result = d_input
                    .buffer
                    .bytes()
                    .iter()
                    .map(|byte| if *byte == 0 { b',' } else { *byte })
                    .collect::<Vec<u8>>();
                Some(encode_hex(&result)).filter(|output| !output.is_empty())
            }
            _ => None,
        };

        arg_string.map(|v| format!(" --e{} '{}' $'{}'", self.value, self.key, v))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct URIInput {
    // The `scheme` of the URI input (for example, content, file).
    pub scheme: URIScheme,
    // The suffix of the URI input.
    pub suffix: URISuffix,
    // The content of the URI input.
    pub content: BytesInput,
}

impl URIInput {
    pub fn identifier(&self, id: usize) -> String {
        match &self.scheme {
            URIScheme::Other => encode_hex(self.content.bytes()),
            _ => {
                let path = match &self.scheme {
                    URIScheme::Content => {
                        "org.gts3.jnifuzz.contentprovider.provider/external_files"
                    }
                    URIScheme::File => "/data/local/tmp",
                    _ => unreachable!(),
                };

                format!(
                    "{}://{}/extra_input_{}{}",
                    self.scheme, path, id, self.suffix
                )
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DirectInput {
    // The value of the primitive input.
    pub buffer: BytesInput,
}

// Enum for the different types of URI schemes.
#[derive(Serialize, Deserialize, Clone, Debug, EnumIter)]
pub enum URIScheme {
    Content,
    File,
    Other,
}

impl fmt::Display for URIScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            URIScheme::Content => write!(f, "content"),
            URIScheme::File => write!(f, "file"),
            URIScheme::Other => Ok(()),
        }
    }
}

// Enum for the different suffixes of URI inputs.
#[derive(Serialize, Deserialize, Clone, Debug, EnumIter)]
pub enum URISuffix {
    AAC,
    APK,
    GIF,
    HTML,
    JPG,
    MIDI,
    MP3,
    MP4,
    OGG,
    PDF,
    PNG,
    TXT,
    WAV,
    WMA,
    WMV,
    XML,
}

impl fmt::Display for URISuffix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            URISuffix::AAC => write!(f, ".aac"),
            URISuffix::APK => write!(f, ".apk"),
            URISuffix::GIF => write!(f, ".gif"),
            URISuffix::HTML => write!(f, ".html"),
            URISuffix::JPG => write!(f, ".jpg"),
            URISuffix::MIDI => write!(f, ".midi"),
            URISuffix::MP3 => write!(f, ".mp3"),
            URISuffix::MP4 => write!(f, ".mp4"),
            URISuffix::OGG => write!(f, ".ogg"),
            URISuffix::PDF => write!(f, ".pdf"),
            URISuffix::PNG => write!(f, ".png"),
            URISuffix::TXT => write!(f, ".txt"),
            URISuffix::WAV => write!(f, ".wav"),
            URISuffix::WMA => write!(f, ".wma"),
            URISuffix::WMV => write!(f, ".wmv"),
            URISuffix::XML => write!(f, ".xml"),
        }
    }
}

// Enum for the different types of extras.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ExtraType {
    String(DirectInput),
    Boolean(DirectInput),
    Int(DirectInput),
    Long(DirectInput),
    Float(DirectInput),
    URI(URIInput),
    ComponentName(DirectInput),
    IntArray(DirectInput),
    IntArrayList(DirectInput),
    LongArray(DirectInput),
    LongArrayList(DirectInput),
    FloatArray(DirectInput),
    FloatArrayList(DirectInput),
    StringArray(DirectInput),
    StringArrayList(DirectInput),
}

impl ExtraType {
    pub fn content_buffer(&mut self) -> &mut BytesInput {
        match self {
            ExtraType::URI(uri_input) => &mut uri_input.content,
            ExtraType::String(d_input) => &mut d_input.buffer,
            ExtraType::Boolean(d_input) => &mut d_input.buffer,
            ExtraType::Int(d_input) => &mut d_input.buffer,
            ExtraType::Long(d_input) => &mut d_input.buffer,
            ExtraType::Float(d_input) => &mut d_input.buffer,
            ExtraType::ComponentName(d_input) => &mut d_input.buffer,
            ExtraType::IntArray(d_input) => &mut d_input.buffer,
            ExtraType::IntArrayList(d_input) => &mut d_input.buffer,
            ExtraType::LongArray(d_input) => &mut d_input.buffer,
            ExtraType::LongArrayList(d_input) => &mut d_input.buffer,
            ExtraType::FloatArray(d_input) => &mut d_input.buffer,
            ExtraType::FloatArrayList(d_input) => &mut d_input.buffer,
            ExtraType::StringArray(d_input) => &mut d_input.buffer,
            ExtraType::StringArrayList(d_input) => &mut d_input.buffer,
        }
    }
}

impl fmt::Display for ExtraType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtraType::String(_) => write!(f, "s"),
            ExtraType::Boolean(_) => write!(f, "z"),
            ExtraType::Int(_) => write!(f, "i"),
            ExtraType::Long(_) => write!(f, "l"),
            ExtraType::Float(_) => write!(f, "f"),
            ExtraType::URI(_) => write!(f, "u"),
            ExtraType::ComponentName(_) => write!(f, "cn"),
            ExtraType::IntArray(_) => write!(f, "ia"),
            ExtraType::IntArrayList(_) => write!(f, "ial"),
            ExtraType::LongArray(_) => write!(f, "la"),
            ExtraType::LongArrayList(_) => write!(f, "lal"),
            ExtraType::FloatArray(_) => write!(f, "fa"),
            ExtraType::FloatArrayList(_) => write!(f, "fal"),
            ExtraType::StringArray(_) => write!(f, "sa"),
            ExtraType::StringArrayList(_) => write!(f, "sal"),
        }
    }
}

// Enum for the following mime types:
#[derive(Serialize, Deserialize, Clone, Debug, EnumIter, Copy)]
pub enum MimeType {
    ApplicationPdf,
    ApplicationVndAndroidPackageArchive,
    AudioAac,
    AudioMidi,
    AudioMpeg,
    AudioMpeg4Generic,
    AudioOgg,
    AudioWav,
    AudioXMsWma,
    ImageGif,
    ImageJpeg,
    ImagePng,
    TextHtml,
    TextPlain,
    TextXml,
    VideoMp4,
    VideoXMsVideo,
    VideoXMsWmv,
}

impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MimeType::ApplicationPdf => write!(f, "application/pdf"),
            MimeType::ApplicationVndAndroidPackageArchive => {
                write!(f, "application/vnd.android.package-archive")
            }
            MimeType::AudioAac => write!(f, "audio/aac"),
            MimeType::AudioMidi => write!(f, "audio/midi"),
            MimeType::AudioMpeg => write!(f, "audio/mpeg"),
            MimeType::AudioMpeg4Generic => write!(f, "audio/mpeg4-generic"),
            MimeType::AudioOgg => write!(f, "audio/ogg"),
            MimeType::AudioWav => write!(f, "audio/wav"),
            MimeType::AudioXMsWma => write!(f, "audio/x-ms-wma"),
            MimeType::ImageGif => write!(f, "image/gif"),
            MimeType::ImageJpeg => write!(f, "image/jpeg"),
            MimeType::ImagePng => write!(f, "image/png"),
            MimeType::TextHtml => write!(f, "text/html"),
            MimeType::TextPlain => write!(f, "text/plain"),
            MimeType::TextXml => write!(f, "text/xml"),
            MimeType::VideoMp4 => write!(f, "video/mp4"),
            MimeType::VideoXMsVideo => write!(f, "video/x-msvideo"),
            MimeType::VideoXMsWmv => write!(f, "video/x-ms-wmv"),
        }
    }
}
