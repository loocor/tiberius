use crate::{Error, FeatureLevel, SqlReadBytes};
use std::convert::TryFrom;

#[allow(dead_code)] // we might want to debug the values
#[derive(Debug)]
pub struct TokenLoginAck {
    /// The type of interface with which the server will accept client requests
    /// 0: SQL_DFLT (server confirms that whatever is sent by the client is acceptable. If the client
    ///    requested SQL_DFLT, SQL_TSQL will be used)
    /// 1: SQL_TSQL (TSQL is accepted)
    pub(crate) interface: u8,
    pub(crate) tds_version: FeatureLevel,
    pub(crate) prog_name: String,
    /// major.minor.buildhigh.buildlow
    pub(crate) version: u32,
}

impl TokenLoginAck {
    pub(crate) async fn decode<R>(src: &mut R) -> crate::Result<Self>
    where
        R: SqlReadBytes + Unpin,
    {
        let _length = src.read_u16_le().await?;

        let interface = src.read_u8().await?;

        let tds_version = FeatureLevel::try_from(src.read_u32().await?)
            .map_err(|_| Error::Protocol("Login ACK: Invalid TDS version".into()))?;
        src.context_mut().set_version(tds_version);

        let prog_name = src.read_b_varchar().await?;
        let version = src.read_u32_le().await?;

        Ok(TokenLoginAck {
            interface,
            tds_version,
            prog_name,
            version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::TokenLoginAck;
    use crate::sql_read_bytes::test_utils::IntoSqlReadBytes;
    use crate::{FeatureLevel, SqlReadBytes};
    use bytes::{BufMut, BytesMut};

    #[tokio::test]
    async fn negotiated_tds_version_updates_connection_context() {
        let mut bytes = BytesMut::new();
        bytes.put_u16_le(10);
        bytes.put_u8(1);
        bytes.put_u32(FeatureLevel::SqlServer2005 as u32);
        bytes.put_u8(0);
        bytes.put_u32_le(0);
        let mut reader = bytes.into_sql_read_bytes();

        TokenLoginAck::decode(&mut reader)
            .await
            .expect("decode login acknowledgement");

        assert_eq!(reader.context().version(), FeatureLevel::SqlServer2005);
    }
}
