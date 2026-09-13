#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    // Push constants
    OpFalse = 0x00,
    OpTrue = 0x51,

    // Flow control & verification
    OpNop = 0x61,
    OpVerify = 0x69,
    OpReturn = 0x6a,

    // Stack manipulation
    OpDup = 0x73,
    OpDrop = 0x75,
    OpSwap = 0x78,

    // Comparison & Logic
    OpEqual = 0x87,
    OpEqualVerify = 0x88,

    // Arithmetic
    OpAdd = 0x93,
    OpSub = 0x94,

    // Cryptographic hashing & signature verification
    OpHash256 = 0xaa,
    OpCheckSig = 0xac,
    OpCheckSigVerify = 0xad,
}

impl OpCode {
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::OpFalse),
            0x51 => Some(Self::OpTrue),
            0x61 => Some(Self::OpNop),
            0x69 => Some(Self::OpVerify),
            0x6a => Some(Self::OpReturn),
            0x73 => Some(Self::OpDup),
            0x75 => Some(Self::OpDrop),
            0x78 => Some(Self::OpSwap),
            0x87 => Some(Self::OpEqual),
            0x88 => Some(Self::OpEqualVerify),
            0x93 => Some(Self::OpAdd),
            0x94 => Some(Self::OpSub),
            0xaa => Some(Self::OpHash256),
            0xac => Some(Self::OpCheckSig),
            0xad => Some(Self::OpCheckSigVerify),
            _ => None,
        }
    }
}
