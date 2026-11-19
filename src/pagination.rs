use actix_web::{web, HttpRequest};
use sea_orm::{DatabaseConnection, EntityTrait, FromQueryResult, PaginatorTrait, Select};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::ErrorResponse;

const FIRST_PAGE: u64 = 1;
const MIN_PAGE_SIZE: u64 = 1;
const PAGE_QUERY_PARAM: &str = "page";

#[derive(Serialize, ToSchema)]
pub struct PaginatedResponse<T: ToSchema> {
    /// Total number of items across all pages.
    pub count: u64,
    /// Absolute URL of the next page, or null on the last page.
    pub next: Option<String>,
    /// Absolute URL of the previous page, or null on the first page.
    pub previous: Option<String>,
    /// Items on the current page.
    pub results: Vec<T>,
}

fn normalize_page(page: Option<u64>) -> u64 {
    page.unwrap_or(FIRST_PAGE).max(FIRST_PAGE)
}

fn normalize_page_size(page_size: Option<u64>, default_page_size: u64) -> u64 {
    page_size.unwrap_or(default_page_size).max(MIN_PAGE_SIZE)
}

fn page_url(request: &HttpRequest, page: u64) -> String {
    let query_pairs: Vec<(String, String)> =
        url::form_urlencoded::parse(request.query_string().as_bytes())
            .into_owned()
            .filter(|(key, _)| key != PAGE_QUERY_PARAM)
            .collect();

    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(query_pairs)
        .append_pair(PAGE_QUERY_PARAM, &page.to_string())
        .finish();

    let connection_info = request.connection_info();
    format!(
        "{}://{}{}?{}",
        connection_info.scheme(),
        connection_info.host(),
        request.path(),
        query
    )
}

pub async fn paginate_query<T, M, D, F>(
    query_builder: Select<T>,
    db: &DatabaseConnection,
    request: &HttpRequest,
    page: Option<u64>,
    page_size: Option<u64>,
    default_page_size: u64,
    mapper: F,
) -> Result<web::Json<PaginatedResponse<D>>, ErrorResponse>
where
    T: EntityTrait<Model = M>,
    M: FromQueryResult + Sized + Send + Sync,
    D: Serialize + ToSchema,
    F: Fn(&M) -> D,
{
    let page = normalize_page(page);
    let page_size = normalize_page_size(page_size, default_page_size);

    let paginator = query_builder.paginate(db, page_size);

    let count = match paginator.num_items().await {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Error getting number of items: {error}");
            return Err(ErrorResponse::internal(format!(
                "database error while counting items: {error}"
            )));
        }
    };

    let items = match paginator.fetch_page(page - FIRST_PAGE).await {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Error fetching page: {error}");
            return Err(ErrorResponse::internal(format!(
                "database error while fetching page: {error}"
            )));
        }
    };

    let results: Vec<D> = items.iter().map(&mapper).collect();

    Ok(web::Json(build_response(
        count, page, page_size, request, results,
    )))
}

fn build_response<D>(
    count: u64,
    page: u64,
    page_size: u64,
    request: &HttpRequest,
    results: Vec<D>,
) -> PaginatedResponse<D>
where
    D: Serialize + ToSchema,
{
    let total_pages = count.div_ceil(page_size);
    let next = (page < total_pages).then(|| page_url(request, page + 1));
    let previous = (page > FIRST_PAGE).then(|| page_url(request, page - 1));

    PaginatedResponse {
        count,
        next,
        previous,
        results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;
    use serde_json::Value as JsonValue;

    const DEFAULT_TEST_PAGE_SIZE: u64 = 30;

    fn request(uri: &str) -> HttpRequest {
        TestRequest::get()
            .uri(uri)
            .insert_header(("Host", "api.example.test"))
            .to_http_request()
    }

    #[derive(Serialize, ToSchema)]
    struct Item {
        id: i32,
        title: String,
    }

    fn item(id: i32) -> Item {
        Item {
            id,
            title: format!("Item {id}"),
        }
    }

    fn query_pairs(url: &str) -> Vec<(String, String)> {
        url::Url::parse(url)
            .expect("page URL must be absolute")
            .query_pairs()
            .into_owned()
            .collect()
    }

    #[test]
    fn page_defaults_to_first_page() {
        assert_eq!(normalize_page(None), FIRST_PAGE);
        assert_eq!(normalize_page(Some(0)), FIRST_PAGE);
        assert_eq!(normalize_page(Some(2)), 2);
    }

    #[test]
    fn page_size_falls_back_to_default_and_rejects_zero() {
        assert_eq!(
            normalize_page_size(None, DEFAULT_TEST_PAGE_SIZE),
            DEFAULT_TEST_PAGE_SIZE
        );
        assert_eq!(
            normalize_page_size(Some(0), DEFAULT_TEST_PAGE_SIZE),
            MIN_PAGE_SIZE
        );
        assert_eq!(normalize_page_size(Some(10), DEFAULT_TEST_PAGE_SIZE), 10);
    }

    #[test]
    fn list_returns_drf_envelope() {
        let response = build_response(
            2,
            FIRST_PAGE,
            DEFAULT_TEST_PAGE_SIZE,
            &request("/items"),
            vec![item(1), item(2)],
        );
        let json: JsonValue = serde_json::to_value(response).expect("response must serialize");

        assert_eq!(json["count"].as_u64(), Some(2));
        assert!(json["next"].is_null());
        assert!(json["previous"].is_null());
        assert_eq!(json["results"].as_array().map(Vec::len), Some(2));
        assert!(json.get("data").is_none());
        assert!(json.get("total_pages").is_none());
    }

    #[test]
    fn page_size_is_kept_in_next_link() {
        let response = build_response(
            5,
            FIRST_PAGE,
            2,
            &request("/items?page_size=2"),
            vec![item(1), item(2)],
        );
        let json: JsonValue = serde_json::to_value(response).expect("response must serialize");

        let next = json["next"].as_str().expect("next page link is required");
        let pairs = query_pairs(next);
        assert!(pairs.contains(&("page".to_owned(), "2".to_owned())));
        assert!(pairs.contains(&("page_size".to_owned(), "2".to_owned())));
        assert!(json["previous"].is_null());
    }

    #[test]
    fn second_page_has_previous_and_next_links() {
        let response = build_response(
            5,
            2,
            2,
            &request("/items?page=2&page_size=2"),
            vec![item(3), item(4)],
        );
        let json: JsonValue = serde_json::to_value(response).expect("response must serialize");

        let previous = json["previous"]
            .as_str()
            .expect("previous page link is required");
        let previous_pairs = query_pairs(previous);
        assert!(previous_pairs.contains(&("page".to_owned(), "1".to_owned())));
        assert!(previous_pairs.contains(&("page_size".to_owned(), "2".to_owned())));

        let next = json["next"].as_str().expect("next page link is required");
        let next_pairs = query_pairs(next);
        assert!(next_pairs.contains(&("page".to_owned(), "3".to_owned())));
        assert!(next_pairs.contains(&("page_size".to_owned(), "2".to_owned())));
    }
}
