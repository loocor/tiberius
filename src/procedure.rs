use std::borrow::Cow;

use enumflags2::BitFlags;

use crate::{ColumnData, RpcParam, RpcStatus, TypeInfo, VarLenContext, VarLenType};

/// SQL Server parameter type used to encode an RPC output value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcedureType {
    /// `tinyint`.
    TinyInt,
    /// `smallint`.
    SmallInt,
    /// `int`.
    Int,
    /// `bigint`.
    BigInt,
    /// `bit`.
    Bit,
    /// `real`.
    Real,
    /// `float`.
    Float,
    /// `money`.
    Money,
    /// `smallmoney`.
    SmallMoney,
    /// `datetime`.
    DateTime,
    /// `smalldatetime`.
    SmallDateTime,
    /// `uniqueidentifier`.
    Guid,
    /// `decimal` or `numeric` with precision and scale.
    Decimal {
        /// Total number of decimal digits.
        precision: u8,
        /// Number of digits after the decimal point.
        scale: u8,
    },
    /// `nvarchar`, where length is measured in bytes and `u16::MAX` means MAX.
    NVarChar {
        /// Maximum encoded length in bytes.
        max_length: u16,
    },
    /// `varchar`, where length is measured in bytes and `u16::MAX` means MAX.
    VarChar {
        /// Maximum encoded length in bytes.
        max_length: u16,
    },
    /// `varbinary`, where `u16::MAX` means MAX.
    VarBinary {
        /// Maximum length in bytes.
        max_length: u16,
    },
}

/// A named input, output, or input/output stored-procedure parameter.
#[derive(Clone, Debug)]
pub struct ProcedureParameter<'a> {
    name: Cow<'a, str>,
    status: BitFlags<RpcStatus>,
    value: ColumnData<'a>,
    type_info: Option<TypeInfo>,
    procedure_type: Option<ProcedureType>,
}

impl<'a> ProcedureParameter<'a> {
    /// Creates an input parameter whose TDS type is derived from its value.
    pub fn input(name: impl Into<Cow<'a, str>>, value: ColumnData<'a>) -> Self {
        Self {
            name: name.into(),
            status: BitFlags::empty(),
            value,
            type_info: None,
            procedure_type: None,
        }
    }

    /// Creates an input parameter with explicit SQL type information.
    pub fn typed_input(
        name: impl Into<Cow<'a, str>>,
        value: ColumnData<'a>,
        procedure_type: ProcedureType,
    ) -> Self {
        Self {
            name: name.into(),
            status: BitFlags::empty(),
            value,
            type_info: Some(procedure_type.type_info()),
            procedure_type: Some(procedure_type),
        }
    }

    /// Creates an output or input/output parameter with explicit SQL type.
    pub fn output(
        name: impl Into<Cow<'a, str>>,
        value: ColumnData<'a>,
        procedure_type: ProcedureType,
    ) -> Self {
        Self {
            name: name.into(),
            status: RpcStatus::ByRefValue.into(),
            value,
            type_info: Some(procedure_type.type_info()),
            procedure_type: Some(procedure_type),
        }
    }

    /// Returns the SQL parameter name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns true for output and input/output parameters.
    pub fn is_output(&self) -> bool {
        self.status.contains(RpcStatus::ByRefValue)
    }

    /// Returns the explicit SQL type used for an output parameter.
    pub fn procedure_type(&self) -> Option<ProcedureType> {
        self.procedure_type
    }

    /// Returns the encoded value.
    pub fn value(&self) -> &ColumnData<'a> {
        &self.value
    }

    pub(crate) fn into_rpc_param(self) -> RpcParam<'a> {
        RpcParam {
            name: self.name,
            flags: self.status,
            value: self.value,
            type_info: self.type_info,
        }
    }
}

impl ProcedureType {
    /// SQL declaration used for a parameter in `sp_executesql`.
    pub fn sql_declaration(self) -> Cow<'static, str> {
        match self {
            Self::TinyInt => "tinyint".into(),
            Self::SmallInt => "smallint".into(),
            Self::Int => "int".into(),
            Self::BigInt => "bigint".into(),
            Self::Bit => "bit".into(),
            Self::Real => "real".into(),
            Self::Float => "float".into(),
            Self::Money => "money".into(),
            Self::SmallMoney => "smallmoney".into(),
            Self::DateTime => "datetime".into(),
            Self::SmallDateTime => "smalldatetime".into(),
            Self::Guid => "uniqueidentifier".into(),
            Self::Decimal { precision, scale } => format!("decimal({precision},{scale})").into(),
            Self::NVarChar { max_length } if max_length == u16::MAX => "nvarchar(max)".into(),
            Self::NVarChar { max_length } => format!("nvarchar({})", max_length.div_ceil(2)).into(),
            Self::VarChar { max_length } if max_length == u16::MAX => "varchar(max)".into(),
            Self::VarChar { max_length } => format!("varchar({max_length})").into(),
            Self::VarBinary { max_length } if max_length == u16::MAX => "varbinary(max)".into(),
            Self::VarBinary { max_length } => format!("varbinary({max_length})").into(),
        }
    }

    pub(crate) fn type_info(self) -> TypeInfo {
        match self {
            Self::TinyInt => var_len(VarLenType::Intn, 1),
            Self::SmallInt => var_len(VarLenType::Intn, 2),
            Self::Int => var_len(VarLenType::Intn, 4),
            Self::BigInt => var_len(VarLenType::Intn, 8),
            Self::Bit => var_len(VarLenType::Bitn, 1),
            Self::Real => var_len(VarLenType::Floatn, 4),
            Self::Float => var_len(VarLenType::Floatn, 8),
            Self::Money => var_len(VarLenType::Money, 8),
            Self::SmallMoney => var_len(VarLenType::Money, 4),
            Self::DateTime => var_len(VarLenType::Datetimen, 8),
            Self::SmallDateTime => var_len(VarLenType::Datetimen, 4),
            Self::Guid => var_len(VarLenType::Guid, 16),
            Self::Decimal { precision, scale } => TypeInfo::VarLenSizedPrecision {
                ty: VarLenType::Numericn,
                size: decimal_size(precision),
                precision,
                scale,
            },
            Self::NVarChar { max_length } => var_len(VarLenType::NVarchar, usize::from(max_length)),
            Self::VarChar { max_length } => {
                var_len(VarLenType::BigVarChar, usize::from(max_length))
            }
            Self::VarBinary { max_length } => {
                var_len(VarLenType::BigVarBin, usize::from(max_length))
            }
        }
    }
}

fn var_len(r#type: VarLenType, length: usize) -> TypeInfo {
    TypeInfo::VarLenSized(VarLenContext::new(r#type, length, None))
}

fn decimal_size(precision: u8) -> usize {
    match precision {
        1..=9 => 5,
        10..=19 => 9,
        20..=28 => 13,
        _ => 17,
    }
}
