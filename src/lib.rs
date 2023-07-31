use pagetop::prelude::*;
use pagetop_minimal::component::*;

pub mod util;

new_handle!(MODULE_MDBOOK);

static_files!(mdbook);

pub struct MdBook;

impl ModuleTrait for MdBook {
    fn handle(&self) -> Handle {
        MODULE_MDBOOK
    }

    fn dependencies(&self) -> Vec<ModuleRef> {
        vec![&pagetop_minimal::Minimal]
    }

    fn configure_service(&self, scfg: &mut service::web::ServiceConfig) {
        serve_static_files!(scfg, "/mdbook", mdbook);
    }
}

impl MdBook {
    pub fn configure_service_for_mdbook(
        scfg: &mut service::web::ServiceConfig,
        mdbook_path: &'static str,
        mdbook_map: &'static HashMapResources,
    ) {
        let path = mdbook_path.trim_end_matches('/');
        scfg.service(
            service::web::scope(path)
                .route(
                    "{tail:.*html$}",
                    service::web::get().to(move |request: service::HttpRequest| {
                        mdbook_page(request, path, mdbook_map)
                    }),
                )
                .route(
                    "{tail:.*$}",
                    service::web::get().to(move |request: service::HttpRequest| {
                        mdbook_resource(request, path, mdbook_map)
                    }),
                ),
        );
    }
}

async fn mdbook_page(
    request: service::HttpRequest,
    mdbook_path: &'static str,
    mdbook_map: &'static HashMapResources,
) -> ResultPage<Markup, FatalError> {
    let path_len = mdbook_path.len() + 1;
    if let Some(content) = mdbook_map.get(&request.path()[path_len..]) {
        if let Ok(html) = std::str::from_utf8(content.data) {
            let lang = langid_for(extract("Lang", html).unwrap_or(""));
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
                .with_title(L10n::n(title))
                .with_metadata("theme-color", "#ffffff")
                .with_context(ContextOp::LangId(lang))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::at(
                    "/mdbook/css/variables.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::at(
                    "/mdbook/css/general.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::at(
                    "/mdbook/css/chrome.css",
                )))
                .with_context(ContextOp::AddStyleSheet(
                    StyleSheet::at("/mdbook/css/print.css").for_media(TargetMedia::Print),
                ))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::at(
                    "/mdbook/FontAwesome/css/font-awesome.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::at(
                    "/mdbook/fonts/fonts.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::at(
                    "/mdbook/highlight.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::at(
                    "/mdbook/tomorrow-night.css",
                )))
                .with_context(ContextOp::AddStyleSheet(StyleSheet::at(
                    "/mdbook/ayu-highlight.css",
                )))
                .with_in(
                    "content",
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
    request: service::HttpRequest,
    mdbook_path: &'static str,
    mdbook_map: &'static HashMapResources,
) -> service::HttpResponse {
    let path_len = mdbook_path.len() + 1;
    // From https://github.com/kilork/actix-web-static-files/blob/master/src/resource_files.rs, see
    // functions respond_to(), any_match() and none_match().
    if let Some(file) = &mdbook_map.get(&request.path()[path_len..]) {
        let etag = Some(service::http::header::EntityTag::new_strong(format!(
            "{:x}:{:x}",
            file.data.len(),
            file.modified
        )));

        let precondition_failed = !any_match(etag.as_ref(), &request);

        let not_modified = !none_match(etag.as_ref(), &request);

        let mut resp = service::HttpResponse::build(service::http::StatusCode::OK);
        resp.insert_header((service::http::header::CONTENT_TYPE, file.mime_type));

        if let Some(etag) = etag {
            resp.insert_header(service::http::header::ETag(etag));
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
    etag: Option<&service::http::header::EntityTag>,
    request: &service::HttpRequest,
) -> bool {
    match request.get_header::<service::http::header::IfMatch>() {
        None | Some(service::http::header::IfMatch::Any) => true,
        Some(service::http::header::IfMatch::Items(ref items)) => {
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
    etag: Option<&service::http::header::EntityTag>,
    request: &service::HttpRequest,
) -> bool {
    match request.get_header::<service::http::header::IfNoneMatch>() {
        Some(service::http::header::IfNoneMatch::Any) => false,
        Some(service::http::header::IfNoneMatch::Items(ref items)) => {
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
