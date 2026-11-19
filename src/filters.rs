use sea_orm::{entity::prelude::*, Condition, EntityTrait, QueryFilter};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use crate::sorting::SortDirection;

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct ListParams {
    /// Search term applied to the endpoint allow-list of text columns.
    #[serde(default)]
    pub search: Option<String>,
    /// Sort field; must be present in the endpoint allow-list.
    #[serde(default)]
    pub sort: Option<String>,
    /// Sort direction (`asc` or `desc`).
    #[serde(default)]
    pub order: SortDirection,
    /// Page number (1-based).
    #[serde(default)]
    #[param(example = 1)]
    pub page: Option<u64>,
    /// Number of items per page.
    #[serde(default)]
    #[param(example = 30)]
    pub page_size: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum FilterValue {
    Int(i32),
    Text(String),
    Bool(bool),
}

pub fn apply_filters<T>(
    mut query_builder: Select<T>,
    filters: Vec<(T::Column, FilterValue)>,
) -> Select<T>
where
    T: EntityTrait,
{
    for (column, value) in filters {
        query_builder = match value {
            FilterValue::Int(value) => query_builder.filter(column.eq(value)),
            FilterValue::Text(value) => query_builder.filter(column.eq(value)),
            FilterValue::Bool(value) => query_builder.filter(column.eq(value)),
        };
    }
    query_builder
}

pub fn apply_date_range<T>(
    mut query_builder: Select<T>,
    column: T::Column,
    from: Option<DateTimeUtc>,
    to: Option<DateTimeUtc>,
) -> Select<T>
where
    T: EntityTrait,
{
    if let Some(from) = from {
        query_builder = query_builder.filter(column.gte(from));
    }
    if let Some(to) = to {
        query_builder = query_builder.filter(column.lte(to));
    }
    query_builder
}

pub fn add_search_condition<T, C>(
    query_builder: Select<T>,
    search_term: &str,
    fields: &[C],
) -> Select<T>
where
    T: EntityTrait,
    C: ColumnTrait,
{
    if !search_term.trim().is_empty() {
        let search_pattern = format!("%{}%", search_term.trim());
        let mut condition = Condition::any();

        for field in fields {
            condition = condition.add(field.contains(&search_pattern));
        }

        query_builder.filter(condition)
    } else {
        query_builder
    }
}

pub fn add_optional_search_condition<T, C>(
    query_builder: Select<T>,
    search_term: Option<String>,
    fields: &[C],
) -> Select<T>
where
    T: EntityTrait,
    C: ColumnTrait,
{
    if let Some(term) = search_term {
        add_search_condition(query_builder, &term, fields)
    } else {
        query_builder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::web;
    use sea_orm::{DbBackend, QueryTrait};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "test_items")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub title: String,
        pub url: String,
        pub active: bool,
        pub created_at: DateTimeUtc,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    #[test]
    fn search_condition_is_applied_to_each_filtered_field() {
        let query = add_search_condition(
            Entity::find(),
            "Smoke",
            &[Column::Title, Column::Url],
        );
        let statement = query.build(DbBackend::Postgres);

        assert!(statement.sql.contains("LIKE"));
        assert!(statement.sql.contains("title"));
        assert!(statement.sql.contains("url"));
        assert!(format!("{:?}", statement.values).contains("%Smoke%"));
    }

    #[test]
    fn empty_search_does_not_add_a_condition() {
        let query = add_search_condition(Entity::find(), "   ", &[Column::Title]);
        let statement = query.build(DbBackend::Postgres);

        assert!(!statement.sql.contains("LIKE"));
    }

    #[test]
    fn optional_search_ignores_a_missing_term() {
        let query = add_optional_search_condition(Entity::find(), None, &[Column::Title]);
        let statement = query.build(DbBackend::Postgres);

        assert!(!statement.sql.contains("LIKE"));
    }

    #[test]
    fn filters_apply_int_text_and_bool_values() {
        let query = apply_filters(
            Entity::find(),
            vec![
                (Column::Id, FilterValue::Int(7)),
                (Column::Title, FilterValue::Text("book".to_owned())),
                (Column::Active, FilterValue::Bool(true)),
            ],
        );
        let statement = query.build(DbBackend::Postgres);

        assert!(statement.sql.contains("id"));
        assert!(statement.sql.contains("title"));
        assert!(statement.sql.contains("active"));
    }

    #[test]
    fn date_range_adds_both_bounds() {
        use std::time::{Duration, SystemTime};

        let from = DateTimeUtc::from(SystemTime::UNIX_EPOCH);
        let to = DateTimeUtc::from(SystemTime::UNIX_EPOCH + Duration::from_secs(86_400));
        let query = apply_date_range(Entity::find(), Column::CreatedAt, Some(from), Some(to));
        let statement = query.build(DbBackend::Postgres);

        assert!(statement.sql.matches("created_at").count() >= 2);
    }

    #[test]
    fn list_params_extracts_the_query_string() {
        let query = web::Query::<ListParams>::from_query(
            "search=book&sort=title&order=asc&page=2&page_size=10",
        )
        .expect("query must parse");

        assert_eq!(query.search.as_deref(), Some("book"));
        assert_eq!(query.sort.as_deref(), Some("title"));
        assert_eq!(query.order, SortDirection::Asc);
        assert_eq!(query.page, Some(2));
        assert_eq!(query.page_size, Some(10));
    }

    #[test]
    fn list_params_uses_defaults_for_missing_values() {
        let query = web::Query::<ListParams>::from_query("").expect("query must parse");

        assert!(query.search.is_none());
        assert!(query.sort.is_none());
        assert_eq!(query.order, SortDirection::Desc);
        assert!(query.page.is_none());
        assert!(query.page_size.is_none());
    }
}
