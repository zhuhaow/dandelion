use rune::{runtime::Shared, FromValue, Value};
use specht_core::Result;

pub fn value_to_result<T: FromValue>(value: Shared<Result<Value, Value>>) -> Result<T> {
    value
        .take()?
        .map_err(|v| match anyhow::Error::from_value(v) {
            Ok(err) => err,
            Err(err) => anyhow::anyhow!(err),
        })
        .and_then(|v| Ok(T::from_value(v)?))
}
