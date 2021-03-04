use crate::encoding;
use crate::Result;

/// Blobs represent an arbitrary chunk of binary data, usually a file.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Blob {
    pub payload: encoding::Digest,
    pub size: u64,
}

impl Blob {
    pub fn new(payload: encoding::Digest, size: u64) -> Self {
        Self {
            payload: payload,
            size: size,
        }
    }

    pub fn digest(&self) -> encoding::Digest {
        return self.payload;
    }

    /// Return the child object of this one in the object DG.
    pub fn child_objects(&self) -> Vec<encoding::Digest> {
        Vec::new()
    }
}

impl encoding::Encodable for Blob {
    fn digest(&self) -> Result<encoding::Digest> {
        Ok(self.digest())
    }
    fn encode(&self, mut writer: &mut impl std::io::Write) -> Result<()> {
        encoding::write_digest(&mut writer, &self.payload)?;
        encoding::write_uint(writer, self.size)
    }
}
impl encoding::Decodable for Blob {
    fn decode(mut reader: &mut impl std::io::Read) -> Result<Self> {
        Ok(Blob {
            payload: encoding::read_digest(&mut reader)?,
            size: encoding::read_uint(reader)?,
        })
    }
}
