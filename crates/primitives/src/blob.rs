//! EIP4844 Blob Type

use alloc::vec;
use alloy_eips::eip4844::{Blob, BYTES_PER_BLOB, VERSIONED_HASH_VERSION_KZG};
use alloy_primitives::Bytes;
use thiserror::Error;

/// The blob encoding version
pub(crate) const BLOB_ENCODING_VERSION: u8 = 0;

/// Maximum blob data size
pub(crate) const BLOB_MAX_DATA_SIZE: usize = (4 * 31 + 3) * 1024 - 4; // 130044

/// Blob Encoding/Decoding Rounds
pub(crate) const BLOB_ENCODING_ROUNDS: usize = 1024;

/// Blob Decuding Error
#[derive(Error, Debug, PartialEq, Eq)]
pub enum BlobDecodingError {
    /// Invalid field element
    #[error("Invalid field element")]
    InvalidFieldElement,
    /// Invalid encoding version
    #[error("Invalid encoding version")]
    InvalidEncodingVersion,
    /// Invalid length
    #[error("Invalid length")]
    InvalidLength,
    /// Missing Data
    #[error("Missing data")]
    MissingData,
}

/// The Blob Data
#[derive(Default, Clone, Debug)]
pub struct BlobData {
    /// The blob data
    pub data: Option<Bytes>,
    /// The calldata
    pub calldata: Option<Bytes>,
}

impl BlobData {
    /// Decodes the blob into raw byte data.
    /// Returns a [BlobDecodingError] if the blob is invalid.
    pub fn decode(&self) -> Result<Bytes, BlobDecodingError> {
        let data = self.data.as_ref().ok_or(BlobDecodingError::MissingData)?;

        // Validate the blob encoding version
        if data[VERSIONED_HASH_VERSION_KZG as usize] != BLOB_ENCODING_VERSION {
            return Err(BlobDecodingError::InvalidEncodingVersion);
        }

        // Decode the 3 byte big endian length value into a 4 byte integer
        let length = u32::from_be_bytes([0, data[2], data[3], data[4]]) as usize;

        // Validate the length
        if length > BLOB_MAX_DATA_SIZE {
            return Err(BlobDecodingError::InvalidLength);
        }

        // Round 0 copies the remaining 27 bytes of the first field element
        let mut output = vec![0u8; BLOB_MAX_DATA_SIZE];
        output[0..27].copy_from_slice(&data[5..32]);

        // Process the remaining 3 field elements to complete round 0
        let mut output_pos = 28;
        let mut input_pos = 32;
        let mut encoded_byte = [0u8; 4];
        encoded_byte[0] = data[0];

        for b in encoded_byte.iter_mut().skip(1) {
            let (enc, opos, ipos) =
                self.decode_field_element(output_pos, input_pos, &mut output)?;
            *b = enc;
            output_pos = opos;
            input_pos = ipos;
        }

        // Reassemble the 4 by 6 bit encoded chunks into 3 bytes of output
        output_pos = self.reassemble_bytes(output_pos, &encoded_byte, &mut output);

        // In each remaining round, decode 4 field elements (128 bytes) of the
        // input into 127 bytes of output
        for _ in 1..BLOB_ENCODING_ROUNDS {
            // Break early if the output position is greater than the length
            if output_pos >= length {
                break;
            }

            for d in &mut encoded_byte {
                let (enc, opos, ipos) =
                    self.decode_field_element(output_pos, input_pos, &mut output)?;
                *d = enc;
                output_pos = opos;
                input_pos = ipos;
            }
            output_pos = self.reassemble_bytes(output_pos, &encoded_byte, &mut output);
        }

        // Validate the remaining bytes
        for o in output.iter().skip(length) {
            if *o != 0u8 {
                return Err(BlobDecodingError::InvalidFieldElement);
            }
        }

        // Validate the remaining bytes
        output.truncate(length);
        for i in input_pos..BYTES_PER_BLOB {
            if data[i] != 0 {
                return Err(BlobDecodingError::InvalidFieldElement);
            }
        }

        Ok(Bytes::from(output))
    }

    /// Decodes the next input field element by writing its lower 31 bytes into its
    /// appropriate place in the output and checking the high order byte is valid.
    /// Returns a [BlobDecodingError] if a field element is seen with either of its
    /// two high order bits set.
    pub fn decode_field_element(
        &self,
        output_pos: usize,
        input_pos: usize,
        output: &mut [u8],
    ) -> Result<(u8, usize, usize), BlobDecodingError> {
        let Some(data) = self.data.as_ref() else {
            return Err(BlobDecodingError::MissingData);
        };

        // two highest order bits of the first byte of each field element should always be 0
        if data[input_pos] & 0b1100_0000 != 0 {
            return Err(BlobDecodingError::InvalidFieldElement);
        }
        output[output_pos..output_pos + 31].copy_from_slice(&data[input_pos + 1..input_pos + 32]);
        Ok((data[input_pos], output_pos + 32, input_pos + 32))
    }

    /// Reassemble 4 by 6 bit encoded chunks into 3 bytes of output and place them in their
    /// appropriate output positions.
    pub fn reassemble_bytes(
        &self,
        mut output_pos: usize,
        encoded_byte: &[u8],
        output: &mut [u8],
    ) -> usize {
        output_pos -= 1;
        let x = (encoded_byte[0] & 0b0011_1111) | ((encoded_byte[1] & 0b0011_0000) << 2);
        let y = (encoded_byte[1] & 0b0000_1111) | ((encoded_byte[3] & 0b0000_1111) << 4);
        let z = (encoded_byte[2] & 0b0011_1111) | ((encoded_byte[3] & 0b0011_0000) << 2);
        output[output_pos - 32] = z;
        output[output_pos - (32 * 2)] = y;
        output[output_pos - (32 * 3)] = x;
        output_pos
    }

    /// Fills in the pointers to the fetched blob bodies.
    /// There should be exactly one placeholder blobOrCalldata
    /// element for each blob, otherwise an error is returned.
    pub fn fill(&mut self, blobs: &[Blob], index: usize) -> Result<(), BlobDecodingError> {
        // Do not fill if there is no calldata to fill
        if self.calldata.as_ref().map_or(false, |data| data.is_empty()) {
            return Ok(());
        }

        if index >= blobs.len() {
            return Err(BlobDecodingError::InvalidLength);
        }

        if blobs[index].is_empty() {
            return Err(BlobDecodingError::MissingData);
        }

        self.data = Some(Bytes::from(blobs[index]));
        Ok(())
    }

    /// Returns if a blob is empty.
    pub const fn is_empty(&self) -> bool {
        self.data.is_none() && self.calldata.is_none()
    }

    /// Turns the blob into its inner data.
    pub fn inner(&self) -> anyhow::Result<Bytes> {
        if let Some(data) = &self.calldata {
            return Ok(data.clone());
        }
        if let Some(data) = &self.data {
            return Ok(data.clone());
        }
        Err(anyhow::anyhow!("No data found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_empty() {
        let blob_data = BlobData::default();
        assert!(blob_data.is_empty());
    }

    #[test]
    fn test_reassemble_bytes() {
        let blob_data = BlobData::default();
        let mut output = vec![0u8; 128];
        let encoded_byte = [0x00, 0x00, 0x00, 0x00];
        let output_pos = blob_data.reassemble_bytes(127, &encoded_byte, &mut output);
        assert_eq!(output_pos, 126);
        assert_eq!(output, vec![0u8; 128]);
    }

    #[test]
    fn test_cannot_fill_empty_calldata() {
        let mut blob_data = BlobData { calldata: Some(Bytes::new()), ..Default::default() };
        let blobs = vec![Blob::with_last_byte(1u8)];
        assert_eq!(blob_data.fill(&blobs, 0), Ok(()));
    }

    #[test]
    fn test_fill_oob_index() {
        let mut blob_data = BlobData::default();
        let blobs = vec![Blob::with_last_byte(1u8)];
        assert_eq!(blob_data.fill(&blobs, 1), Err(BlobDecodingError::InvalidLength));
    }

    #[test]
    #[ignore]
    fn test_fill_empty_blob() {
        let mut blob_data = BlobData::default();
        let blobs = vec![Blob::ZERO];
        assert_eq!(blob_data.fill(&blobs, 0), Err(BlobDecodingError::MissingData));
    }

    #[test]
    fn test_fill_blob() {
        let mut blob_data = BlobData::default();
        let blobs = vec![Blob::with_last_byte(1u8)];
        assert_eq!(blob_data.fill(&blobs, 0), Ok(()));
        let expected = Bytes::from([&[0u8; 131071][..], &[1u8]].concat());
        assert_eq!(blob_data.data, Some(expected));
    }

    #[test]
    fn test_not_empty() {
        let mut blob_data =
            BlobData { data: Some(Bytes::from(vec![1u8; 32])), ..Default::default() };
        assert!(!blob_data.is_empty());
        blob_data.data = None;
        blob_data.calldata = Some(Bytes::from(vec![1u8; 32]));
        assert!(!blob_data.is_empty());
    }

    #[test]
    fn test_inner() {
        let blob_data = BlobData { data: Some(Bytes::from(vec![1u8; 32])), ..Default::default() };
        assert_eq!(blob_data.inner().unwrap(), Bytes::from(vec![1u8; 32]));
        let blob_data =
            BlobData { calldata: Some(Bytes::from(vec![1u8; 32])), ..Default::default() };
        assert_eq!(blob_data.inner().unwrap(), Bytes::from(vec![1u8; 32]));
        let blob_data = BlobData::default();
        assert!(blob_data.inner().is_err());
    }

    #[test]
    fn test_blob_data_decode_missing_data() {
        let blob_data = BlobData::default();
        assert_eq!(blob_data.decode(), Err(BlobDecodingError::MissingData));
    }

    #[test]
    fn test_blob_data_decode_invalid_encoding_version() {
        let blob_data = BlobData { data: Some(Bytes::from(vec![1u8; 32])), ..Default::default() };
        assert_eq!(blob_data.decode(), Err(BlobDecodingError::InvalidEncodingVersion));
    }

    #[test]
    fn test_blob_data_decode_invalid_length() {
        let mut data = vec![0u8; 32];
        data[VERSIONED_HASH_VERSION_KZG as usize] = BLOB_ENCODING_VERSION;
        data[2] = 0xFF;
        data[3] = 0xFF;
        data[4] = 0xFF;
        let blob_data = BlobData { data: Some(Bytes::from(data)), ..Default::default() };
        assert_eq!(blob_data.decode(), Err(BlobDecodingError::InvalidLength));
    }
}
