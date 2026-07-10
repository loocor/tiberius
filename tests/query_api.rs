use futures_util::io::Cursor;
use tiberius::numeric::Numeric;
use tiberius::{Client, Column, ColumnData, ColumnType, FromSqlOwned, ProcedureType, Query};

#[test]
fn query_accepts_preencoded_column_data() {
    let mut query = Query::new("SELECT @P1");

    query.bind_value(ColumnData::I32(Some(42)));
}

#[test]
fn query_accepts_manifest_derived_sql_types() {
    let mut query = Query::new("SELECT @P1");

    query.bind_typed_value(
        ColumnData::String(Some("legacy".into())),
        ProcedureType::VarChar { max_length: 20 },
    );
}

#[test]
fn procedure_types_generate_safe_sp_executesql_declarations() {
    assert_eq!(
        ProcedureType::VarChar { max_length: 20 }.sql_declaration(),
        "varchar(20)"
    );
    assert_eq!(
        ProcedureType::NVarChar { max_length: 200 }.sql_declaration(),
        "nvarchar(100)"
    );
    assert_eq!(
        ProcedureType::Decimal {
            precision: 19,
            scale: 4
        }
        .sql_declaration(),
        "decimal(19,4)"
    );
    assert_eq!(ProcedureType::Money.sql_declaration(), "money");
}

#[test]
fn manually_constructed_columns_report_unknown_nullability() {
    let column = Column::new("Value".to_owned(), ColumnType::Int4);

    assert_eq!(column.nullable(), None);
    assert!(!column.is_identity());
    assert!(!column.is_computed());
}

#[test]
fn client_exposes_idle_state_for_connection_pools() {
    let _idle_state = Client::<Cursor<Vec<u8>>>::is_connection_idle;
}

#[test]
fn floating_point_reads_remain_compatible_with_lossless_money_values() {
    let value = ColumnData::Numeric(Some(Numeric::new_with_scale(12345, 4)));

    let decoded = f64::from_sql_owned(value).expect("decode numeric as f64");

    assert_eq!(decoded, Some(1.2345));
}
