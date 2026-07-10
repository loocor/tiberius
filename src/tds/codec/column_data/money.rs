use crate::{error::Error, sql_read_bytes::SqlReadBytes, tds::Numeric, ColumnData};
use bytes::BufMut;

#[cfg(test)]
mod tests {
    use super::{money_value, smallmoney_value};
    use crate::{tds::Numeric, ColumnData};

    #[test]
    fn smallmoney_decodes_without_floating_point_loss() {
        let value = smallmoney_value(12345);

        assert_eq!(
            value,
            ColumnData::Numeric(Some(Numeric::new_with_scale(12345, 4)))
        );
    }

    #[test]
    fn money_preserves_the_full_scaled_integer() {
        let raw = 9_223_372_036_854_775_i64;
        let value = money_value((raw >> 32) as i32, raw as u32);

        assert_eq!(
            value,
            ColumnData::Numeric(Some(Numeric::new_with_scale(i128::from(raw), 4)))
        );
    }
}

fn smallmoney_value(raw: i32) -> ColumnData<'static> {
    ColumnData::Numeric(Some(Numeric::new_with_scale(i128::from(raw), 4)))
}

fn money_value(high: i32, low: u32) -> ColumnData<'static> {
    let raw = (i64::from(high) << 32) + i64::from(low);
    ColumnData::Numeric(Some(Numeric::new_with_scale(i128::from(raw), 4)))
}

pub(crate) fn encode<B>(dst: &mut B, value: Option<Numeric>, len: usize) -> crate::Result<()>
where
    B: BufMut,
{
    let Some(value) = value else {
        dst.put_u8(0);
        return Ok(());
    };
    if value.scale() != 4 {
        return Err(Error::BulkInput(
            format!("money value must use scale 4, got {}", value.scale()).into(),
        ));
    }

    let raw = i64::try_from(value.value()).map_err(|_| {
        Error::BulkInput(format!("money value {} exceeds 64-bit range", value).into())
    })?;
    match len {
        4 => {
            let raw = i32::try_from(raw).map_err(|_| {
                Error::BulkInput(format!("smallmoney value {} is out of range", value).into())
            })?;
            dst.put_u8(4);
            dst.put_i32_le(raw);
        }
        8 => {
            dst.put_u8(8);
            dst.put_i32_le((raw >> 32) as i32);
            dst.put_u32_le(raw as u32);
        }
        _ => {
            return Err(Error::BulkInput(
                format!("money type length must be 4 or 8, got {len}").into(),
            ));
        }
    }
    Ok(())
}

pub(crate) async fn decode<R>(src: &mut R, len: u8) -> crate::Result<ColumnData<'static>>
where
    R: SqlReadBytes + Unpin,
{
    let res = match len {
        0 => ColumnData::Numeric(None),
        4 => smallmoney_value(src.read_i32_le().await?),
        8 => money_value(src.read_i32_le().await?, src.read_u32_le().await?),
        _ => {
            return Err(Error::Protocol(
                format!("money: length of {} is invalid", len).into(),
            ))
        }
    };

    Ok(res)
}
