use thiserror::Error;

/// Unified Error Type for PixelDrive operations across ROM loading, core execution, rendering, and audio.
#[derive(Error, Debug)]
#[allow(dead_code, clippy::enum_variant_names)]
pub enum PixelDriveError {
    #[error("ROM Loading Error: {0}")]
    RomLoadError(String),

    #[error("Core Loading Error: {0}")]
    CoreLoadError(String),

    #[error("Save / State Error: {0}")]
    SaveStateError(String),

    #[error("Audio Error: {0}")]
    AudioError(String),

    #[error("Render Error: {0}")]
    RenderError(String),

    #[error("I/O Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("ZIP Archive Error: {0}")]
    ZipError(#[from] zip::result::ZipError),

    #[error("Serialization Error: {0}")]
    BincodeError(#[from] bincode::Error),
}

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, PixelDriveError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_formatting() {
        let err = PixelDriveError::RomLoadError("Invalid header checksum".to_string());
        assert_eq!(
            format!("{}", err),
            "ROM Loading Error: Invalid header checksum"
        );

        let core_err = PixelDriveError::CoreLoadError("Failed to load libretro core".to_string());
        assert_eq!(
            format!("{}", core_err),
            "Core Loading Error: Failed to load libretro core"
        );

        let save_err = PixelDriveError::SaveStateError("Corrupt snapshot file".to_string());
        assert_eq!(
            format!("{}", save_err),
            "Save / State Error: Corrupt snapshot file"
        );

        let audio_err = PixelDriveError::AudioError("No audio output device found".to_string());
        assert_eq!(
            format!("{}", audio_err),
            "Audio Error: No audio output device found"
        );

        let render_err = PixelDriveError::RenderError("Surface lost".to_string());
        assert_eq!(format!("{}", render_err), "Render Error: Surface lost");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let pd_err: PixelDriveError = io_err.into();
        assert!(matches!(pd_err, PixelDriveError::IoError(_)));
    }
}
