use chrono::{DateTime, Utc};
use rocket::{
    data::ToByteUnit,
    form::{DataField, Error, FromFormField, Result, ValueField},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTimeUtcFormField {
    pub date_time: DateTime<Utc>,
}

#[rocket::async_trait]
impl<'v> FromFormField<'v> for DateTimeUtcFormField {
    fn from_value(field: ValueField<'v>) -> Result<'v, Self> {
        let date_time = DateTime::parse_from_rfc3339(field.value)
            .map_err(|e| Error::validation(format!("invalid rfc3339 date time: {}", e)))?;

        Ok(Self {
            date_time: date_time.into(),
        })
    }

    async fn from_data<'i>(field: DataField<'v, 'i>) -> Result<'v, Self> {
        let data = field.data.open(27.bytes()).into_string().await?;
        let date_time = DateTime::parse_from_rfc3339(&data)
            .map_err(|e| Error::validation(format!("invalid rfc3339 date time: {}", e)))?;

        Ok(Self {
            date_time: date_time.into(),
        })
    }

    fn default() -> Option<Self> {
        None
    }
}
