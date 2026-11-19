use sea_orm::{EntityTrait, QueryOrder, Select};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::error::{ErrorResponse, FieldError};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
}

/// Resolve a requested sort field against an allow-list of entity columns.
pub fn resolve_sort_column<C: Copy>(
    requested: Option<&str>,
    mapping: &[(&str, C)],
) -> Result<Option<C>, ErrorResponse> {
    let Some(field) = requested else {
        return Ok(None);
    };

    mapping
        .iter()
        .find(|(name, _)| *name == field)
        .map(|(_, column)| Some(*column))
        .ok_or_else(|| {
            ErrorResponse::validation(vec![FieldError::new(
                "sort",
                "invalid",
                format!("Unknown sort field '{field}'"),
            )])
        })
}

pub fn apply_sort<T>(query: Select<T>, column: T::Column, direction: SortDirection) -> Select<T>
where
    T: EntityTrait,
{
    match direction {
        SortDirection::Asc => query.order_by_asc(column),
        SortDirection::Desc => query.order_by_desc(column),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "test_items")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub title: String,
        pub created_at: DateTimeUtc,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    const SORT_MAPPING: [(&str, Column); 2] = [
        ("created_at", Column::CreatedAt),
        ("title", Column::Title),
    ];

    #[test]
    fn resolves_a_known_sort_field() {
        let column =
            resolve_sort_column(Some("title"), &SORT_MAPPING).expect("known sort field must resolve");

        assert!(matches!(column, Some(Column::Title)));
    }

    #[test]
    fn no_sort_field_uses_the_default() {
        let column = resolve_sort_column(None, &SORT_MAPPING).expect("default is valid");

        assert!(column.is_none());
    }

    #[test]
    fn rejects_an_unknown_sort_field() {
        let error = resolve_sort_column(Some("secret"), &SORT_MAPPING)
            .expect_err("unknown sort field must fail");

        assert_eq!(error.status_code, 400);
        assert_eq!(error.fields[0].field, "sort");
        assert_eq!(error.fields[0].code, "invalid");
    }

    #[test]
    fn sort_direction_deserializes_from_lowercase() {
        let direction: SortDirection = serde_json::from_str("\"asc\"").expect("asc is valid");
        assert_eq!(direction, SortDirection::Asc);

        let default: SortDirection = serde_json::from_str("null")
            .or_else(|_| serde_json::from_str("\"desc\""))
            .expect("desc is valid");
        assert_eq!(default, SortDirection::Desc);
    }
}
