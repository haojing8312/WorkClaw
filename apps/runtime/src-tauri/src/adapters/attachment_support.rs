#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterAttachmentSupport {
    pub native_image: bool,
    pub native_file: bool,
    pub document_fallback: bool,
    pub audio_fallback: bool,
    pub video_fallback: bool,
}

pub fn openai_responses_attachment_support() -> AdapterAttachmentSupport {
    AdapterAttachmentSupport {
        native_image: true,
        native_file: false,
        document_fallback: true,
        audio_fallback: true,
        video_fallback: true,
    }
}
