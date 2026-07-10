use tiberius::{Client, ColumnData, ProcedureItem, ProcedureParameter, ProcedureType};

#[test]
fn output_parameter_retains_type_and_by_ref_status() {
    let parameter = ProcedureParameter::output("@Count", ColumnData::I32(None), ProcedureType::Int);

    assert_eq!(parameter.name(), "@Count");
    assert!(parameter.is_output());
    assert_eq!(parameter.procedure_type(), Some(ProcedureType::Int));
}

#[test]
fn input_parameter_has_no_output_status_or_explicit_type() {
    let parameter = ProcedureParameter::input("@HouseId", ColumnData::I32(Some(42)));

    assert_eq!(parameter.name(), "@HouseId");
    assert!(!parameter.is_output());
    assert_eq!(parameter.procedure_type(), None);
}

async fn assert_procedure_method_is_public<S>(client: &mut Client<S>)
where
    S: futures_util::io::AsyncRead + futures_util::io::AsyncWrite + Unpin + Send,
{
    let _result = client.execute_procedure("dbo.Test", vec![]).await;
}

#[test]
fn procedure_execution_api_is_public() {
    let _assertion = assert_procedure_method_is_public::<futures_util::io::Cursor<Vec<u8>>>;
    let _item: Option<ProcedureItem> = None;
}
