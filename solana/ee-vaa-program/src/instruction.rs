//! Instruction deserialization/handling code

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use std::{
    convert::TryInto,
    io::{self, Cursor, Read, Write},
};

use crate::error::{Error, Error::*};

/// Present at the beginning of every EE-VAA instruction
pub const EE_VAA_MAGIC: &'static [u8] = b"WHEV"; // Wormhole EE VAA

/// Top-level instruction data type
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Instruction {
    /// Pass an EE-VAA to the bridge
    PostEEVAA(EEVAA),
}

impl Instruction {
    /// QoL wrapper for `deserialize_from_reader`
    #[inline]
    pub fn deserialize(buf: &[u8]) -> Result<Self, Error> {
        Self::deserialize_from_reader(Cursor::new(buf))
    }

    /// Deserialize the custom Instruction format and underlying data
    pub fn deserialize_from_reader<R: Read>(mut r: R) -> Result<Self, Error> {
        let mut magic = vec![0; EE_VAA_MAGIC.len()];

        r.read_exact(&mut magic)
            .map_err(|_| UnexpectedEndOfBuffer)?;

        if magic != EE_VAA_MAGIC {
            return Err(Error::InvalidMagic);
        }

        let kind_byte = r.read_u8().map_err(|_| UnexpectedEndOfBuffer)?;

        let i = match kind_byte {
            n if n == InstructionKind::PostEEVAA as u8 => {
                Self::PostEEVAA(EEVAA::deserialize_from_reader(r)?)
            }
            _other => return Err(InvalidInstructionKind),
        };

        Ok(i)
    }

    /// Turns this instruction into bytes.
    ///
    /// Format:
    /// Magic (EE_VAA_MAGIC.len() bytes, must match exactly)
    /// InstructionKind (1 byte)
    /// Instruction data (may vary, see serialize() of each inner struct)
    pub fn serialize(&self) -> Result<Vec<u8>, io::Error> {
        // Start with a copy of the magic
        let mut buf = EE_VAA_MAGIC.to_owned();

        match self {
            Instruction::PostEEVAA(ee_vaa) => {
                buf.push(InstructionKind::PostEEVAA as u8);
                buf.append(&mut ee_vaa.serialize()?);
            }
        }

        Ok(buf)
    }
}

/// An enum used to distinguish between instructions in the serialization format
#[repr(u8)]
pub enum InstructionKind {
    PostEEVAA = 1,
}

/// EE VAA representation
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EEVAA {
    /// The data to pass along the guardian set
    pub payload: Vec<u8>,
}

impl EEVAA {
    /// QoL Deserialization method
    #[inline]
    pub fn deserialize(bytes: &[u8]) ->  Result<Self, Error> {
	Self::deserialize_from_reader(Cursor::new(bytes))
    }

    /// Deserialize this EE-VAA
    pub fn deserialize_from_reader(mut r: impl Read) -> Result<Self, Error> {
        let payload_len = r
            .read_u16::<BigEndian>()
            .map_err(|_| UnexpectedEndOfBuffer)?;

        let mut payload = vec![0; payload_len as usize];

        r.read_exact(payload.as_mut_slice())
            .map_err(|_| UnexpectedEndOfBuffer)?;

        Ok(Self { payload })
    }

    /// Turns this EE VAA into bytes.
    ///
    /// Format:
    /// Length (2 bytes)
    /// Data (Length bytes)
    pub fn serialize(&self) -> Result<Vec<u8>, io::Error> {
        let mut c = Cursor::new(Vec::new());

        c.write_u16::<BigEndian>(
            self.payload
                .len()
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Could not write payload len"))?,
        )?;

        c.write_all(self.payload.as_slice())?;

        Ok(c.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub type ErrBox = Box<dyn std::error::Error>;

    #[test]
    fn test_serde_eevaa_basic() -> Result<(), ErrBox> {
        let a = EEVAA {
            payload: vec![0x42],
        };

        let buf = a.serialize()?;

        let b = EEVAA::deserialize_from_reader(Cursor::new(buf))?;

        assert_eq!(a, b);

        Ok(())
    }

    #[test]
    fn test_serde_instruction_basic() -> Result<(), ErrBox> {
        let ee_vaa = EEVAA {
            payload: vec![0x42],
        };
        let i_a = Instruction::PostEEVAA(ee_vaa);

        let buf = i_a.serialize()?;

        let i_b = Instruction::deserialize(buf.as_slice())?;

        assert_eq!(i_a, i_b);

        Ok(())
    }
}
