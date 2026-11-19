use actix_web::HttpResponse;
use sea_orm::{DatabaseConnection, EntityTrait, ModelTrait, PrimaryKeyTrait};

use crate::error::ErrorResponse;

/// Universal function for retrieving an entity by ID.
///
/// # Arguments
/// * `id` - entity identifier
/// * `db` - database connection
/// * `resource` - human-readable resource name for the 404 response
/// * `mapper` - function for converting model to DTO
pub async fn get_by_id<T, M, D, F>(
    id: i32,
    db: &DatabaseConnection,
    resource: &str,
    mapper: F,
) -> Result<HttpResponse, ErrorResponse>
where
    T: EntityTrait<Model = M>,
    M: ModelTrait + Sized + Send + Sync,
    F: FnOnce(&M) -> D,
    D: serde::Serialize,
    <<T as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType: From<i32>,
{
    let model = T::find_by_id(id)
        .one(db)
        .await
        .map_err(|error| {
            ErrorResponse::internal(format!("database error in get_by_id: {error}"))
        })?
        .ok_or_else(|| ErrorResponse::not_found(resource, id))?;

    let dto = mapper(&model);
    Ok(HttpResponse::Ok().json(dto))
}
