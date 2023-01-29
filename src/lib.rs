use pagetop::prelude::*;

pub mod util;

pub_handle!(MODULE_MDBOOK);

include!(concat!(env!("OUT_DIR"), "/mdbook.rs"));

pub struct MdBook;

impl ModuleTrait for MdBook {
    fn handle(&self) -> Handle {
        MODULE_MDBOOK
    }
}

impl MdBook {
    pub fn configure_service_for_common_resources(cfg: &mut server::web::ServiceConfig) {
        serve_static_files!(cfg, "/mdbook/static", bundle_mdbook);
    }

    pub fn configure_service_for_mdbook(
        cfg: &mut server::web::ServiceConfig,
        mdbook_path: &'static str,
        mdbook_map: &'static HashMapResources,
    ) {
        let path = mdbook_path.trim_end_matches('/');
        cfg.service(
            server::web::scope(path)
                .route(
                    "{tail:.*html$}",
                    server::web::get().to(move |request: server::HttpRequest| {
                        mdbook_page(request, path, mdbook_map)
                    }),
                )
                .route(
                    "{tail:.*$}",
                    server::web::get().to(move |request: server::HttpRequest| {
                        mdbook_resource(request, path, mdbook_map)
                    }),
                ),
        );
    }
}

async fn mdbook_page(
    request: server::HttpRequest,
    mdbook_path: &'static str,
    mdbook_map: &'static HashMapResources,
) -> ResultPage<Markup, FatalError> {
    let path_len = mdbook_path.len() + 1;
    if let Some(content) = mdbook_map.get(&request.path()[path_len..]) {
        if let Ok(html) = std::str::from_utf8(content.data) {
            let _lang = extract("Lang", html);
            let title = match extract("Title", html) {
                Some(title) => title,
                _ => "Documentación",
            };
            let _print = matches!(extract("Print", html), Some("enabled"));
            let _mathjax = matches!(extract("MathJax", html), Some("supported"));
            let beginning = {
                let separator = "<!-- mdBook -->";
                match html.find(separator) {
                    Some(pos) => pos + separator.len(),
                    _ => 0,
                }
            };

            Page::new(request)
                .with_title(title)
                .with_metadata("theme-color", "#ffffff")
                .with_context(ContextOp::AddStyleSheet(StyleSheet::located(
                    "/mdbook/static/css/variables.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::located(
                    "/mdbook/static/css/general.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::located(
                    "/mdbook/static/css/chrome.css",
                )))
                .with_context(ContextOp::AddStyleSheet(
                    StyleSheet::located("/mdbook/static/css/print.css")
                        .for_media(TargetMedia::Print),
                ))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::located(
                    "/mdbook/static/FontAwesome/css/font-awesome.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::located(
                    "/mdbook/static/fonts/fonts.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::located(
                    "/mdbook/static/highlight.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::located(
                    "/mdbook/static/tomorrow-night.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::located(
                    "/mdbook/static/ayu-highlight.css",
                )))
                .with_this_in(
                    "region-content",
                    Container::new()
                        .with_id("mdbook")
                        .with_component(Html::with(html! { (PreEscaped(&html[beginning..])) })),
                )
                .render()
        } else {
            Err(FatalError::NotFound(request))
        }
    } else {
        Err(FatalError::NotFound(request))
    }
}

async fn mdbook_resource(
    request: server::HttpRequest,
    mdbook_path: &'static str,
    mdbook_map: &'static HashMapResources,
) -> server::HttpResponse {
    let path_len = mdbook_path.len() + 1;
    // From https://github.com/kilork/actix-web-static-files/blob/master/src/resource_files.rs, see
    // functions respond_to(), any_match() and none_match().
    if let Some(file) = &mdbook_map.get(&request.path()[path_len..]) {
        let etag = Some(server::http::header::EntityTag::new_strong(format!(
            "{:x}:{:x}",
            file.data.len(),
            file.modified
        )));

        let precondition_failed = !any_match(etag.as_ref(), &request);

        let not_modified = !none_match(etag.as_ref(), &request);

        let mut resp = server::HttpResponse::build(server::http::StatusCode::OK);
        resp.insert_header((server::http::header::CONTENT_TYPE, file.mime_type));

        if let Some(etag) = etag {
            resp.insert_header(server::http::header::ETag(etag));
        }

        if precondition_failed {
            return FatalError::PreconditionFailed(request).error_response();
        } else if not_modified {
            return FatalError::NotModified(request).error_response();
        }

        resp.body(file.data)
    } else {
        FatalError::NotFound(request).error_response()
    }
}

/// Returns true if `request` has no `If-Match` header or one which matches `etag`.
fn any_match(
    etag: Option<&server::http::header::EntityTag>,
    request: &server::HttpRequest,
) -> bool {
    match request.get_header::<server::http::header::IfMatch>() {
        None | Some(server::http::header::IfMatch::Any) => true,
        Some(server::http::header::IfMatch::Items(ref items)) => {
            if let Some(some_etag) = etag {
                for item in items {
                    if item.strong_eq(some_etag) {
                        return true;
                    }
                }
            }
            false
        }
    }
}

/// Returns true if `request` doesn't have an `If-None-Match` header matching `req`.
fn none_match(
    etag: Option<&server::http::header::EntityTag>,
    request: &server::HttpRequest,
) -> bool {
    match request.get_header::<server::http::header::IfNoneMatch>() {
        Some(server::http::header::IfNoneMatch::Any) => false,
        Some(server::http::header::IfNoneMatch::Items(ref items)) => {
            if let Some(some_etag) = etag {
                for item in items {
                    if item.weak_eq(some_etag) {
                        return false;
                    }
                }
            }
            true
        }
        None => true,
    }
}

fn extract(attr: &'static str, from: &'static str) -> Option<&'static str> {
    let search = concat_string!("<!-- ", attr, ":");
    if let Some(ini) = from.find(&search) {
        let ini = ini + search.len() + 1;
        if let Some(end) = from[ini..].find("-->").map(|i| i + ini) {
            let end = end - 1;
            return Some(&from[ini..end]);
        }
    }
    None
}
