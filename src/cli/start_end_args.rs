use clap::Args;
use getset::CopyGetters;
use thiserror::Error;

use crate::video::timestamp::{Timestamp, TimestampFormatError};


#[derive(Args, CopyGetters)]
#[getset(get_copy = "pub")]
pub struct StartEndArgs {

    /// start timestamp
    #[clap(long, value_parser = timestamp_value_parser, value_name = "[HH:]MM:SS", conflicts_with("fix_audio"), conflicts_with("fix_audio_sync"))]
    start: Option<Timestamp>,

    /// end timestamp
    #[clap(long, value_parser = timestamp_value_parser, value_name = "[HH:]MM:SS")]
    end: Option<Timestamp>,

}

fn timestamp_value_parser(timestamp_str: &str) -> Result<Timestamp, TimestampFormatError> {
    Timestamp::try_from(timestamp_str)
}

#[derive(Debug, Error)]
#[error("`--start` timestamp >= `--end` timestamp")]
pub struct StartGreaterThanEndError;

impl StartEndArgs {

    pub fn are_valid(&self) -> bool {
        if let (Some(start), Some(end)) = (self.start, self.end) {
            return start < end;
        }
        true
    }

    pub fn check_valid(&self) -> Result<(), StartGreaterThanEndError> {
        if ! self.are_valid() {
            return Err(StartGreaterThanEndError);
        }
        Ok(())
    }

}