#[derive(Debug)]
pub enum Failure {
    IoFailed(std::io::Error),
    ProcessingFailed(mupdf::error::Error),
    OfficeProcessingFailed(libreofficekit::OfficeError),
    FeatureMissing(String),
    InvalidInput(String),
    RuntimeError(String),
    ConfigurationError(toml::ser::Error),
}

impl std::error::Error for Failure {}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::ConfigurationError(err) => {
                write!(f,"{}", err)
            },
            Failure::IoFailed(err) => {
                write!(f,"{}", err)
            },
            Failure::ProcessingFailed(err) => {
                write!(f, "{}", err)
            },
            Failure::OfficeProcessingFailed(err) => {
                write!(f, "{}", err)
            },
            Failure::InvalidInput(details) | Failure::RuntimeError(details) | Failure::FeatureMissing(details) => {
                write!(f, "{}", details)
            },
        }
    }
}

impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Self {
        Failure::IoFailed(error)
    }
}

impl From<toml::ser::Error> for Failure {
    fn from(error: toml::ser::Error) -> Self {
        Failure::ConfigurationError(error)
    }
}

impl From<mupdf::error::Error> for Failure {
    fn from(error: mupdf::error::Error) -> Self {
        Failure::ProcessingFailed(error)
    }
}

impl From<libreofficekit::OfficeError> for Failure {
    fn from(error: libreofficekit::OfficeError) -> Self {
        Failure::OfficeProcessingFailed(error)
    }
}
