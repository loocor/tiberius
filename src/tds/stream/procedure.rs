use crate::tds::stream::{QueryItem, ReceivedToken};
use crate::{row::ColumnType, Column, ColumnData, Row};
use futures_util::{
    ready,
    stream::{BoxStream, Stream, StreamExt},
};
use std::{
    fmt::Debug,
    pin::Pin,
    sync::Arc,
    task::{self, Poll},
};

/// A value returned for an RPC output parameter.
#[derive(Debug)]
pub struct OutputParameter {
    /// Parameter ordinal from the RPC response.
    pub ordinal: u16,
    /// Parameter name from the RPC response.
    pub name: String,
    /// Whether the value belongs to a user-defined function return value.
    pub udf: bool,
    /// Decoded TDS value.
    pub value: ColumnData<'static>,
}

/// An item emitted while consuming a stored-procedure RPC response.
#[derive(Debug)]
pub enum ProcedureItem {
    /// Result-set metadata or row.
    Result(QueryItem),
    /// Stored-procedure integer return status.
    ReturnStatus(u32),
    /// Output or input/output parameter value.
    OutputParameter(OutputParameter),
    /// Row count reported by a DONE token.
    RowsAffected(u64),
}

/// Streaming stored-procedure response preserving result and RPC tokens.
pub struct ProcedureStream<'a> {
    token_stream: BoxStream<'a, crate::Result<ReceivedToken>>,
    columns: Option<Arc<Vec<Column>>>,
    result_set_index: Option<usize>,
}

impl Debug for ProcedureStream<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcedureStream").finish_non_exhaustive()
    }
}

impl<'a> ProcedureStream<'a> {
    pub(crate) fn new(token_stream: BoxStream<'a, crate::Result<ReceivedToken>>) -> Self {
        Self {
            token_stream,
            columns: None,
            result_set_index: None,
        }
    }
}

impl Stream for ProcedureStream<'_> {
    type Item = crate::Result<ProcedureItem>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            let token = match ready!(this.token_stream.poll_next_unpin(cx)) {
                Some(result) => result?,
                None => return Poll::Ready(None),
            };

            let item = match token {
                ReceivedToken::NewResultset(metadata) => {
                    let columns = Arc::new(
                        metadata
                            .columns
                            .iter()
                            .map(|column| Column {
                                name: column.col_name.to_string(),
                                column_type: ColumnType::from(&column.base.ty),
                                flags: column.base.flags,
                            })
                            .collect::<Vec<_>>(),
                    );
                    this.columns = Some(columns.clone());
                    this.result_set_index = this.result_set_index.map(|index| index + 1);
                    ProcedureItem::Result(QueryItem::metadata(
                        columns,
                        *this.result_set_index.get_or_insert(0),
                    ))
                }
                ReceivedToken::Row(data) => {
                    let Some(columns) = this.columns.as_ref() else {
                        return Poll::Ready(Some(Err(crate::Error::Protocol(
                            "procedure row received before column metadata".into(),
                        ))));
                    };
                    let Some(result_index) = this.result_set_index else {
                        return Poll::Ready(Some(Err(crate::Error::Protocol(
                            "procedure row received before result-set index".into(),
                        ))));
                    };
                    ProcedureItem::Result(QueryItem::Row(Row {
                        columns: columns.clone(),
                        data,
                        result_index,
                    }))
                }
                ReceivedToken::ReturnStatus(status) => ProcedureItem::ReturnStatus(status),
                ReceivedToken::ReturnValue(value) => {
                    ProcedureItem::OutputParameter(OutputParameter {
                        ordinal: value.param_ordinal,
                        name: value.param_name,
                        udf: value.udf,
                        value: value.value,
                    })
                }
                ReceivedToken::DoneInProc(done) => ProcedureItem::RowsAffected(done.rows()),
                ReceivedToken::DoneProc(done) if !done.is_final() => {
                    ProcedureItem::RowsAffected(done.rows())
                }
                ReceivedToken::Done(done) if done.rows() > 0 => {
                    ProcedureItem::RowsAffected(done.rows())
                }
                _ => continue,
            };

            return Poll::Ready(Some(Ok(item)));
        }
    }
}
