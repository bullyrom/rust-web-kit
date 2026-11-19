# rust-web-kit

Reusable building blocks for Actix Web + SeaORM + utoipa services:

- a single `ErrorResponse` envelope with 4xx/5xx builders and field errors;
- Django REST Framework-compatible pagination (`count`, `next`, `previous`, `results`);
- list query parameters, filters, date ranges, search and allow-listed sorting;
- generic helpers: get-by-id, text cleaning, URL canonicalization, content hashing,
  HTTP URL validation and RFC3339 parsing;
- Actix JSON/query/path error handlers returning the same error envelope.

The crate intentionally contains no domain models, no migrations and no
application-specific endpoints.

## Installation

```toml
[dependencies]
rust-web-kit = { git = "https://<git-host>/<org>/rust-web-kit.git", rev = "<commit-sha>" }
```

`rev` should be a full commit SHA. The crate uses the same versions as the
consumer service (Actix Web 4, SeaORM 1, utoipa 5, serde 1, url 2).

```rust
use rust_web_kit::{
    ErrorResponse, FieldError, ListParams, PaginatedResponse, SortDirection,
};
```

## Errors

All handlers can return `ErrorResponse` directly because it implements
`ResponseError`:

```rust
use rust_web_kit::{ErrorResponse, FieldError};

fn validate_title(title: &str) -> Result<(), ErrorResponse> {
    if title.trim().is_empty() {
        return Err(ErrorResponse::validation(vec![FieldError::new(
            "title",
            "required",
            "Title must not be empty",
        )]));
    }
    Ok(())
}

let not_found = ErrorResponse::not_found("Book", 42);
let internal = ErrorResponse::internal("database is unavailable");
```

Builders: `bad_request`, `validation`, `not_found`, `unprocessable`,
`internal`, `bad_gateway`, `upstream_rate_limited`, plus the generic
`client_error` and `server_error` for any status.

## Pagination

```rust
use actix_web::{web, HttpRequest};
use sea_orm::DatabaseConnection;
use rust_web_kit::{paginate_query, PaginatedResponse};

async fn list_books(
    db: &DatabaseConnection,
    request: &HttpRequest,
) -> Result<web::Json<PaginatedResponse<BookDto>>, rust_web_kit::ErrorResponse> {
    let query = Book::find();

    paginate_query(
        query,
        db,
        request,
        Some(1),
        Some(30),
        30,
        BookDto::from_model,
    )
    .await
}
```

The response shape matches DRF:

```json
{
  "count": 5,
  "next": "https://api.example/books?page=2&page_size=2",
  "previous": null,
  "results": []
}
```

`next` and `previous` preserve the current query string and are absolute URLs.
`page` is 1-based; `page_size` defaults to the value passed by the endpoint.

## List query, filters and sorting

```rust
use rust_web_kit::filters::{
    add_optional_search_condition, apply_date_range, apply_filters, FilterValue, ListParams,
};
use rust_web_kit::sorting::{apply_sort, resolve_sort_column, SortDirection};

const BOOK_SORT_COLUMNS: [(&str, book::Column); 2] = [
    ("title", book::Column::Title),
    ("created_at", book::Column::CreatedAt),
];

let mut query = book::Entity::find();
query = add_optional_search_condition(
    query,
    params.search.clone(),
    &[book::Column::Title],
);

query = apply_filters(
    query,
    vec![(book::Column::Available, FilterValue::Bool(true))],
);

query = apply_date_range(
    query,
    book::Column::CreatedAt,
    params_from,
    params_to,
);

query = match resolve_sort_column(params.sort.as_deref(), &BOOK_SORT_COLUMNS)? {
    Some(column) => apply_sort(query, column, params.order),
    None => apply_sort(query, book::Column::CreatedAt, SortDirection::Desc),
};
```

`ListParams` is a ready query extractor with `search`, `sort`, `order`,
`page` and `page_size`.

## Helpers

```rust
use rust_web_kit::text::{clean_optional, clean_required};
use rust_web_kit::url::{canonicalize_url, content_hash};
use rust_web_kit::validation::{parse_optional_rfc3339, validate_http_url};

let name = clean_optional(Some("  Book  ".to_owned()));
let title = clean_required(Some("  Chapter  "));
let url = canonicalize_url("https://Example.COM/items/?utm_source=x")?;
let hash = content_hash(&[&url, "Title"]);
validate_http_url("https://example.com")?;
let from = parse_optional_rfc3339(Some("2026-01-01T00:00:00Z"), "created_from")?;
```

## Actix extractor configuration

```rust
use actix_web::{web, App, HttpServer};
use rust_web_kit::web::{json_config, path_config, query_config};

HttpServer::new(|| {
    App::new()
        .app_data(json_config())
        .app_data(query_config())
        .app_data(path_config())
})
```

Invalid JSON bodies, query strings and path parameters then return the same
`ErrorResponse` envelope as application errors.

## Tests

```sh
cargo test
```
