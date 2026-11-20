//! Serve `/ui/` and inject the ACowork theme/openFile bridge into HTML.
//!
//! The upstream `figcraft-web` `index.html` is not edited. The adapter rewrites
//! the response so the sandboxed iframe can listen for `{channel:"acowork"}`
//! postMessage (ADR-118 / design doc 35).

use axum::body::{Body, to_bytes};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

/// Marker attribute so a page is only rewritten once.
const BRIDGE_MARK: &str = "data-acowork-bridge";

/// Breaks `color-scheme` inheritance from the ACowork shell (`html.dark` would
/// otherwise make the iframe report `prefers-color-scheme: dark` and mint a
/// black Untitled page before the theme message arrives).
const BRIDGE_STYLE: &str = r#"<style data-acowork-bridge="1">html,body{color-scheme:light;background:#f5f5f5;}</style>"#;

/// Injected into served HTML. Enables `?control` (so `window.figcraft` exists)
/// then maps ACowork theme / openFile messages onto the page command API.
const BRIDGE_SCRIPT: &str = r#"<script data-acowork-bridge="1">
(function(){
  var u=new URL(location.href);
  if(!u.searchParams.has("control")){
    u.searchParams.set("control","1");
    location.replace(u.pathname+u.search+u.hash);
    return;
  }
  function setScheme(dark){
    var s=dark?"dark":"light";
    document.documentElement.style.colorScheme=s;
    document.body.style.background=dark?"#2c2c2c":"#f5f5f5";
  }
  var lastTheme=null, themeTimer=null, lastCmd=null;
  function applyTheme(d){
    if(d&&d.channel==="acowork"&&d.type==="theme") lastTheme=d;
    if(!lastTheme)return;
    var dark=lastTheme.theme==="dark";
    setScheme(dark);
    function tryApply(){
      var f=window.figcraft;
      if(!f||typeof f.execute!=="function")return false;
      var cmd=dark?"view.themeDark":"view.themeLight";
      if(cmd===lastCmd)return true;
      try{f.execute(cmd,{}); lastCmd=cmd;}catch(e){}
      return true;
    }
    if(tryApply()){
      if(themeTimer){clearInterval(themeTimer);themeTimer=null;}
      return;
    }
    if(!themeTimer) themeTimer=setInterval(function(){
      if(tryApply()&&themeTimer){clearInterval(themeTimer);themeTimer=null;}
    },200);
  }
  function applyOpen(d){
    if(!d||d.channel!=="acowork"||d.type!=="openFile")return;
    var t=setInterval(function(){
      var f=window.figcraft;
      if(!f||typeof f.openFile!=="function")return;
      clearInterval(t);
      var b=d.bytes;
      if(!(b instanceof Uint8Array)) b=new Uint8Array(b||[]);
      try{f.openFile(d.name||"document.figcraft",b);}catch(e){}
    },200);
  }
  window.addEventListener("message",function(e){applyTheme(e.data);applyOpen(e.data);});
  try{window.parent.postMessage({channel:"acowork",type:"ready"},"*");}catch(e){}
})();
</script>"#;

/// Insert the ACowork bridge script into an HTML document.
pub fn inject_acowork_bridge(html: &str) -> String {
    if html.contains(BRIDGE_MARK) {
        return html.to_string();
    }
    let snippet = format!("{BRIDGE_STYLE}{BRIDGE_SCRIPT}");
    if let Some(i) = find_ci(html, "</head>") {
        return format!("{}{}{}", &html[..i], snippet, &html[i..]);
    }
    if let Some(i) = find_ci(html, "<body")
        && let Some(gt) = html[i..].find('>')
    {
        let at = i + gt + 1;
        return format!("{}{}{}", &html[..at], snippet, &html[at..]);
    }
    format!("{html}{snippet}")
}

fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    hay.to_ascii_lowercase().find(&needle.to_ascii_lowercase())
}

pub fn should_inject_html(path: &str, content_type: Option<&str>) -> bool {
    let path_l = path.to_ascii_lowercase();
    if path_l.ends_with(".js")
        || path_l.ends_with(".mjs")
        || path_l.ends_with(".wasm")
        || path_l.ends_with(".css")
        || path_l.ends_with(".png")
        || path_l.ends_with(".svg")
    {
        return false;
    }
    if path_l.ends_with(".html") || path_l.ends_with('/') || path_l.ends_with("/ui") {
        return true;
    }
    content_type.is_some_and(|ct| ct.to_ascii_lowercase().contains("text/html"))
}

/// After ServeDir: CORS / wasm types, and HTML bridge injection.
pub async fn ui_response_layer(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let path = req.uri().path().to_string();
    let mut response = next.run(req).await;
    crate::apply_asset_headers(&path, &mut response);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    if !should_inject_html(&path, content_type.as_deref()) {
        return response;
    }
    inject_html_response(response).await
}

async fn inject_html_response(response: Response) -> Response {
    let status = response.status();
    let mut headers = response.headers().clone();
    let Ok(bytes) = to_bytes(response.into_body(), 2 * 1024 * 1024).await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let original = String::from_utf8_lossy(&bytes);
    let injected = inject_acowork_bridge(&original);
    headers.remove(header::CONTENT_LENGTH);
    if headers.get(header::CONTENT_TYPE).is_none()
        && let Ok(v) = HeaderValue::from_str("text/html; charset=utf-8")
    {
        headers.insert(header::CONTENT_TYPE, v);
    }
    let mut rebuilt = Response::new(Body::from(injected));
    *rebuilt.status_mut() = status;
    *rebuilt.headers_mut() = headers;
    rebuilt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_once_before_head_close() {
        let html = "<html><head><title>x</title></head><body></body></html>";
        let out = inject_acowork_bridge(html);
        assert!(out.contains(BRIDGE_MARK));
        assert!(out.contains("view.themeDark"));
        assert!(out.contains("openFile"));
        assert!(out.contains("color-scheme:light"));
        let again = inject_acowork_bridge(&out);
        assert_eq!(
            out.matches(BRIDGE_MARK).count(),
            again.matches(BRIDGE_MARK).count()
        );
    }

    #[test]
    fn skips_scripts_and_wasm() {
        assert!(!should_inject_html("/ui/app.js", Some("text/javascript")));
        assert!(!should_inject_html("/ui/app.wasm", Some("application/wasm")));
        assert!(should_inject_html("/ui/", Some("text/html")));
        assert!(should_inject_html("/ui/index.html", None));
    }
}
