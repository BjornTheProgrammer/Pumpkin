use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChunkReadingError {
    #[error("Io error: {0}")]
    IoError(std::io::ErrorKind),
    #[error("Invalid header")]
    InvalidHeader,
    #[error("Region is invalid")]
    RegionIsInvalid,
    #[error("Compression error {0}")]
    Compression(CompressionError),
    #[error("Tried to read chunk which does not exist")]
    ChunkNotExist,
    #[error("Failed to parse chunk from bytes: {0}")]
    ParsingError(ChunkParsingError),
}

#[derive(Error, Debug)]
pub enum ChunkWritingError {
    #[error("Io error: {0}")]
    IoError(std::io::ErrorKind),
    #[error("Compression error {0}")]
    Compression(CompressionError),
    #[error("Chunk serializing error: {0}")]
    ChunkSerializingError(String),
}

#[derive(Error, Debug)]
pub enum CompressionError {
    #[error("Compression scheme not recognised")]
    UnknownCompression,
    #[error("Error while working with zlib compression: {0}")]
    ZlibError(std::io::Error),
    #[error("Error while working with Gzip compression: {0}")]
    GZipError(std::io::Error),
    #[error("Error while working with LZ4 compression: {0}")]
    LZ4Error(std::io::Error),
    #[error("Error while working with zstd compression: {0}")]
    ZstdError(std::io::Error),
}

#[derive(Error, Debug)]
pub enum ChunkParsingError {
    #[error("Failed reading chunk status {0}")]
    FailedReadStatus(pumpkin_nbt::Error),
    #[error("The chunk isn't generated yet")]
    ChunkNotGenerated,
    #[error("Error deserializing chunk: {0}")]
    ErrorDeserializingChunk(String),
}

#[derive(Error, Debug)]
pub enum ChunkSerializingError {
    #[error("Error serializing chunk: {0}")]
    ErrorSerializingChunk(pumpkin_nbt::Error),
}
