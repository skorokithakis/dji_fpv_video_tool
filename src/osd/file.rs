
use std::error::Error;
use std::fmt::Display;
use std::fs::File;
use std::io::{Error as IOError, Read};
use std::iter::Enumerate;
use std::ops::Index;
use std::path::Path;

use byte_struct::ByteStruct;
use byte_struct::*;

use getset::Getters;
use hd_fpv_osd_font_tool::osd::tile::Dimensions as TileDimensions;
use derive_more::Deref;

const SIGNATURE: &str = "MSPOSD\x00";

#[derive(Debug)]
pub enum OpenError {
    IOError(IOError),
    InvalidSignature
}

impl Error for OpenError {}

impl From<IOError> for OpenError {
    fn from(error: IOError) -> Self {
        Self::IOError(error)
    }
}

impl Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use OpenError::*;
        match self {
            IOError(error) => error.fmt(f),
            InvalidSignature => f.write_str("invalid header"),
        }
    }
}

#[derive(Debug)]
pub enum ReadError {
    IOError(IOError),
    UnexpectedEOF
}

impl Error for ReadError {}

impl From<IOError> for ReadError {
    fn from(error: IOError) -> Self {
        Self::IOError(error)
    }
}

impl Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ReadError::*;
        match self {
            IOError(error) => error.fmt(f),
            UnexpectedEOF => f.write_str("unexpected end of file"),
        }
    }
}

#[derive(ByteStruct, Debug)]
#[byte_struct_le]
struct FileHeaderRaw {
    file_version: u16,
    width_tiles: u8,
    height_tiles: u8,
    tile_width: u8,
    tile_height: u8,
    x_offset: u16,
    y_offset: u16,
    font_variant: u8
}

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct Offset {
    x: u16,
    y: u16
}


#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct DimensionsTiles {
    width: u8,
    height: u8
}

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct FileHeader {
    file_version: u16,
    dimensions_tiles: DimensionsTiles,
    tile_dimensions: TileDimensions,
    offset: Offset,
    font_variant: u8
}

impl From<FileHeaderRaw> for FileHeader {
    fn from(fhr: FileHeaderRaw) -> Self {
        Self {
            file_version: fhr.file_version,
            dimensions_tiles: DimensionsTiles { width: fhr.width_tiles, height: fhr.height_tiles },
            tile_dimensions: TileDimensions { width: fhr.tile_width as u32, height: fhr.tile_height as u32 },
            offset: Offset { x: fhr.x_offset, y: fhr.y_offset },
            font_variant: fhr.font_variant
        }
    }
}

#[derive(ByteStruct, Debug)]
#[byte_struct_le]
struct FrameHeader {
    frame_index: u32,
    data_len: u32
}

pub type TileIndex = u16;
pub type ScreenCoordinate = u8;

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct Dimensions<T> {
    width: T,
    height: T
}

pub const TILE_INDICES_DIMENSIONS_TILES: Dimensions<ScreenCoordinate> = Dimensions { width: 60, height: 22 };

#[derive(Debug, Deref)]
pub struct TileIndices(Vec<TileIndex>);

impl TileIndices {

    fn screen_coordinates_to_index(x: ScreenCoordinate, y: ScreenCoordinate) -> usize {
        y as usize + x as usize * TILE_INDICES_DIMENSIONS_TILES.height as usize
    }

    fn index_to_screen_coordinates(index: usize) -> (ScreenCoordinate, ScreenCoordinate) {
        (
            (index / TILE_INDICES_DIMENSIONS_TILES.height as usize) as ScreenCoordinate,
            (index % TILE_INDICES_DIMENSIONS_TILES.height as usize) as ScreenCoordinate
        )
    }

    pub fn enumerate(&self) -> TileIndicesEnumeratorIter {
        TileIndicesEnumeratorIter(self.iter().enumerate())
    }

}

impl Index<(ScreenCoordinate, ScreenCoordinate)> for TileIndices {
    type Output = TileIndex;

    fn index(&self, index: (ScreenCoordinate, ScreenCoordinate)) -> &Self::Output {
        &self.0[Self::screen_coordinates_to_index(index.0, index.1)]
    }
}

pub struct TileIndicesEnumeratorIter<'a>(Enumerate<std::slice::Iter<'a, u16>>);

impl<'a> Iterator for TileIndicesEnumeratorIter<'a> {
    type Item = (ScreenCoordinate, ScreenCoordinate, TileIndex);

    fn next(&mut self) -> Option<Self::Item> {
        for (tile_index_index, tile_index) in &mut self.0 {
            if *tile_index > 0 {
                let (screen_x, screen_y) = TileIndices::index_to_screen_coordinates(tile_index_index);
                return Some((screen_x, screen_y, *tile_index))
            }
        }
        None
    }
}

#[derive(Debug, Getters, Deref)]
#[getset(get = "pub")]
pub struct Frame {
    index: u32,
    #[deref] tile_indices: TileIndices
}

impl Frame {
    pub fn enumerate_tile_indices(&self) -> TileIndicesEnumeratorIter {
        self.tile_indices().enumerate()
    }
}

pub struct Reader {
    file: File,
    header: FileHeader
}

impl Reader {

    fn check_signature(file: &mut File) -> Result<(), OpenError> {
        let mut signature = [0; SIGNATURE.len()];
        file.read_exact(&mut signature)?;
        if signature != SIGNATURE.as_bytes() {
            return Err(OpenError::InvalidSignature)
        }
        Ok(())
    }

    fn read_header(file: &mut File) -> Result<FileHeaderRaw, IOError> {
        let mut header_bytes = [0; FileHeaderRaw::BYTE_LEN];
        file.read_exact(&mut header_bytes)?;
        let header = FileHeaderRaw::read_bytes(&header_bytes);
        Ok(header)
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, OpenError> {
        let mut file = File::open(&path)?;
        Self::check_signature(&mut file)?;
        let header = Self::read_header(&mut file).unwrap().into();

        Ok(Self { file, header })
    }

    pub fn header(&self) -> &FileHeader {
        &self.header
    }

    fn read_frame_header(&mut self) -> Result<Option<FrameHeader>, ReadError> {
        let mut frame_header_bytes = [0; FrameHeader::BYTE_LEN];
        match self.file.read(&mut frame_header_bytes)? {
            0 => Ok(None),
            FrameHeader::BYTE_LEN => Ok(Some(FrameHeader::read_bytes(&frame_header_bytes))),
            _ => Err(ReadError::UnexpectedEOF)
        }
    }

    pub fn read_frame(&mut self) -> Result<Option<Frame>, ReadError> {
        let header = match self.read_frame_header()? {
            Some(header) => header,
            None => return Ok(None),
        };
        let mut data_bytes= vec![0; header.data_len as usize * 2];
        self.file.read_exact(&mut data_bytes)?;
        let tile_indices = TileIndices(data_bytes.chunks_exact(u16::BYTE_LEN)
            .map(|bytes| u16::from_le_bytes(bytes.try_into().unwrap())).collect());
        Ok(Some(Frame { index: header.frame_index, tile_indices }))
    }

    pub fn frames(self) -> Result<Vec<Frame>, ReadError> {
        let mut frames = vec![];
        for frame_read_result in self {
            match frame_read_result {
                Ok(frame) => frames.push(frame),
                Err(error) => return Err(error),
            }
        }
        Ok(frames)
    }

}

pub struct IntoIter {
    reader: Reader
}

impl Iterator for IntoIter {
    type Item = Result<Frame, ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_frame().transpose()
    }
}

impl IntoIterator for Reader {
    type Item = Result<Frame, ReadError>;

    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter { reader: self }
    }
}