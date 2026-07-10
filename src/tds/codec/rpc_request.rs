use super::{AllHeaderTy, Encode, TypeInfo, ALL_HEADERS_LEN_TX};
use crate::tds::Collation;
use crate::{tds::codec::ColumnData, BytesMutWithTypeInfo, Error, Result};
use bytes::{BufMut, BytesMut};
use enumflags2::{bitflags, BitFlags};
use std::borrow::BorrowMut;
use std::borrow::Cow;

#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RpcStatus {
    ByRefValue = 1 << 0,
    DefaultValue = 1 << 1,
    // reserved
    Encrypted = 1 << 3,
}

#[bitflags]
#[repr(u16)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RpcOption {
    WithRecomp = 1 << 0,
    NoMeta = 1 << 1,
    ReuseMeta = 1 << 2,
}

#[derive(Debug)]
pub struct TokenRpcRequest<'a> {
    proc_id: RpcProcIdValue<'a>,
    flags: BitFlags<RpcOption>,
    params: Vec<RpcParam<'a>>,
    transaction_desc: [u8; 8],
}

impl<'a> TokenRpcRequest<'a> {
    pub fn new<I>(proc_id: I, params: Vec<RpcParam<'a>>, transaction_desc: [u8; 8]) -> Self
    where
        I: Into<RpcProcIdValue<'a>>,
    {
        Self {
            proc_id: proc_id.into(),
            flags: BitFlags::empty(),
            params,
            transaction_desc,
        }
    }
}

#[derive(Debug)]
pub struct RpcParam<'a> {
    pub name: Cow<'a, str>,
    pub flags: BitFlags<RpcStatus>,
    pub value: ColumnData<'a>,
    pub type_info: Option<TypeInfo>,
}

impl RpcParam<'_> {
    pub(crate) fn apply_default_collation(&mut self, collation: Option<Collation>) {
        if let Some(type_info) = &mut self.type_info {
            type_info.apply_default_collation(collation);
        }
    }
}

/// 2.2.6.6 RPC Request
#[allow(dead_code)]
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum RpcProcId {
    CursorOpen = 2,
    CursorFetch = 7,
    CursorClose = 9,
    ExecuteSQL = 10,
    Prepare = 11,
    Execute = 12,
    PrepExec = 13,
    Unprepare = 15,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum RpcProcIdValue<'a> {
    Name(Cow<'a, str>),
    Id(RpcProcId),
}

impl<'a, S> From<S> for RpcProcIdValue<'a>
where
    S: Into<Cow<'a, str>>,
{
    fn from(s: S) -> Self {
        Self::Name(s.into())
    }
}

impl<'a> From<RpcProcId> for RpcProcIdValue<'a> {
    fn from(id: RpcProcId) -> Self {
        Self::Id(id)
    }
}

impl<'a> Encode<BytesMut> for TokenRpcRequest<'a> {
    fn encode(self, dst: &mut BytesMut) -> Result<()> {
        dst.put_u32_le(ALL_HEADERS_LEN_TX as u32);
        dst.put_u32_le(ALL_HEADERS_LEN_TX as u32 - 4);
        dst.put_u16_le(AllHeaderTy::TransactionDescriptor as u16);
        dst.put_slice(&self.transaction_desc);
        dst.put_u32_le(1);

        match self.proc_id {
            RpcProcIdValue::Id(ref id) => {
                let val = (0xffff_u32) | ((*id as u16) as u32) << 16;
                dst.put_u32_le(val);
            }
            RpcProcIdValue::Name(ref name) => {
                let encoded: Vec<u16> = name.encode_utf16().collect();
                let length = u16::try_from(encoded.len()).map_err(|_| {
                    Error::Protocol("RPC procedure name exceeds 65535 UTF-16 code units".into())
                })?;

                dst.put_u16_le(length);
                for codepoint in encoded {
                    dst.put_u16_le(codepoint);
                }
            }
        }

        dst.put_u16_le(self.flags.bits());

        for param in self.params.into_iter() {
            param.encode(dst)?;
        }

        Ok(())
    }
}

impl<'a> Encode<BytesMut> for RpcParam<'a> {
    fn encode(self, dst: &mut BytesMut) -> Result<()> {
        let len_pos = dst.len();
        let mut length = 0u8;

        dst.put_u8(length);

        for codepoint in self.name.encode_utf16() {
            length += 1;
            dst.put_u16_le(codepoint);
        }

        dst.put_u8(self.flags.bits());

        if let Some(type_info) = self.type_info {
            type_info.clone().encode(dst)?;
            let mut dst_fi = BytesMutWithTypeInfo::new(dst).with_type_info(&type_info);
            self.value.encode(&mut dst_fi)?;
        } else {
            let mut dst_fi = BytesMutWithTypeInfo::new(dst);
            self.value.encode(&mut dst_fi)?;
        }

        let dst: &mut [u8] = dst.borrow_mut();
        dst[len_pos] = length;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{RpcParam, RpcStatus, TokenRpcRequest};
    use crate::tds::codec::{ColumnData, Encode, TypeInfo, VarLenContext, VarLenType};
    use bytes::BytesMut;
    use enumflags2::BitFlags;
    use std::borrow::Cow;

    #[test]
    fn encodes_named_procedure_as_utf16() {
        let request = TokenRpcRequest::new("dbo.FindHouse", vec![], [0; 8]);
        let mut encoded = BytesMut::new();

        request.encode(&mut encoded).expect("encode RPC request");

        let procedure_offset = 22;
        let expected_name: Vec<u8> = [
            13, 0, b'd', 0, b'b', 0, b'o', 0, b'.', 0, b'F', 0, b'i', 0, b'n', 0, b'd', 0, b'H', 0,
            b'o', 0, b'u', 0, b's', 0, b'e', 0,
        ]
        .to_vec();

        assert_eq!(
            &encoded[procedure_offset..procedure_offset + expected_name.len()],
            expected_name
        );
    }

    #[test]
    fn encodes_output_parameter_with_explicit_type_info() {
        let parameter = RpcParam {
            name: Cow::Borrowed("@Count"),
            flags: RpcStatus::ByRefValue.into(),
            value: ColumnData::I32(None),
            type_info: Some(TypeInfo::VarLenSized(VarLenContext::new(
                VarLenType::Intn,
                4,
                None,
            ))),
        };
        let request = TokenRpcRequest::new("dbo.CountHouse", vec![parameter], [0; 8]);
        let mut encoded = BytesMut::new();

        request.encode(&mut encoded).expect("encode RPC request");

        let parameter_offset = 22 + 2 + (14 * 2) + 2;
        assert_eq!(encoded[parameter_offset], 6);
        assert_eq!(
            encoded[parameter_offset + 13],
            BitFlags::from(RpcStatus::ByRefValue).bits()
        );
        assert_eq!(encoded[parameter_offset + 14], VarLenType::Intn as u8);
        assert_eq!(encoded[parameter_offset + 15], 4);
        assert_eq!(encoded[parameter_offset + 16], 0);
    }

    #[test]
    fn encodes_null_nvarchar_output_with_collation_bytes() {
        let parameter = RpcParam {
            name: Cow::Borrowed("@Text"),
            flags: RpcStatus::ByRefValue.into(),
            value: ColumnData::String(None),
            type_info: Some(TypeInfo::VarLenSized(VarLenContext::new(
                VarLenType::NVarchar,
                200,
                None,
            ))),
        };
        let request = TokenRpcRequest::new("dbo.OutputText", vec![parameter], [0; 8]);
        let mut encoded = BytesMut::new();

        request.encode(&mut encoded).expect("encode RPC request");

        let parameter_offset = 22 + 2 + (14 * 2) + 2;
        let type_offset = parameter_offset + 1 + (5 * 2) + 1;
        assert_eq!(
            &encoded[type_offset..type_offset + 10],
            &[
                VarLenType::NVarchar as u8,
                200,
                0,
                0,
                0,
                0,
                0,
                0,
                0xff,
                0xff
            ]
        );
    }
}
