use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BodyMode {
    #[default]
    Raw,
    FormData,
    UrlEncoded,
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequestDraft {
    pub method: String,
    pub url: String,
    pub origin: String,
    pub path: String,
    pub query: Option<String>,
    pub headers: Vec<Header>,
    pub cookies: Vec<Cookie>,
    #[serde(default)]
    pub body_mode: BodyMode,
    pub body: String,
    pub raw_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UrlParts {
    url: String,
    origin: String,
    path: String,
    query: Option<String>,
}

impl Default for RequestDraft {
    fn default() -> Self {
        Self::from_url("https://api.example.com").expect("default URL is valid")
    }
}

impl RequestDraft {
    pub fn from_url(url: &str) -> Result<Self, String> {
        let parts = parse_url(url)?;

        Ok(Self {
            method: "GET".to_string(),
            url: parts.url,
            origin: parts.origin,
            path: parts.path,
            query: parts.query,
            headers: vec![
                Header::new("Accept", "application/json"),
                Header::new("User-Agent", "curler/0.1"),
            ],
            cookies: Vec::new(),
            body_mode: BodyMode::Raw,
            body: String::new(),
            raw_args: Vec::new(),
        })
    }

    pub fn from_curl_args(args: &[String]) -> Result<Self, String> {
        parse_curl_args(args)
    }

    pub fn set_url(&mut self, url: &str) -> Result<(), String> {
        let parts = parse_url(url)?;

        self.url = parts.url;
        self.origin = parts.origin;
        self.path = parts.path;
        self.query = parts.query;

        Ok(())
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = if query.is_empty() {
            None
        } else {
            Some(query.to_string())
        };
        self.url = match &self.query {
            Some(query) => format!("{}{}?{}", self.origin, self.path, query),
            None => format!("{}{}", self.origin, self.path),
        };
    }

    pub fn set_body_mode(&mut self, mode: BodyMode) {
        self.body_mode = mode;
    }

    pub fn display_path(&self) -> String {
        let path = self.path.trim_start_matches('/');

        if path.is_empty() {
            "/".to_string()
        } else {
            path.to_string()
        }
    }

    pub fn fingerprint(&self) -> String {
        let mut headers = self
            .headers
            .iter()
            .map(|header| format!("{}:{}", header.name.to_lowercase(), header.value))
            .collect::<Vec<_>>();
        headers.sort();

        let mut cookies = self
            .cookies
            .iter()
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect::<Vec<_>>();
        cookies.sort();

        stable_hash(&[
            self.method.as_str(),
            self.origin.as_str(),
            self.path.as_str(),
            self.query.as_deref().unwrap_or_default(),
            self.body_mode.key(),
            self.body.as_str(),
            headers.join("\n").as_str(),
            cookies.join("; ").as_str(),
        ])
    }

    pub fn summary(&self) -> String {
        format!("{} {} {}", self.method, self.origin, self.display_path())
    }

    pub fn variant_label(&self) -> String {
        let mut parts = Vec::new();

        if let Some(query) = &self.query {
            parts.push(format!("qs:{}", compact_value(query)));
        }

        if !self.body.is_empty() {
            if self.body_mode != BodyMode::Raw {
                parts.push(format!("mode:{}", self.body_mode.key()));
            }
            parts.push(format!("body:{}", &stable_hash(&[self.body.as_str()])[..6]));
        }

        if !self.headers.is_empty() {
            let mut headers = self
                .headers
                .iter()
                .map(|header| format!("{}:{}", header.name.to_lowercase(), header.value))
                .collect::<Vec<_>>();
            headers.sort();
            parts.push(format!(
                "hdr:{}",
                &stable_hash(&[headers.join("\n").as_str()])[..6]
            ));
        }

        if !self.cookies.is_empty() {
            let mut cookies = self
                .cookies
                .iter()
                .map(|cookie| format!("{}={}", cookie.name, cookie.value))
                .collect::<Vec<_>>();
            cookies.sort();
            parts.push(format!(
                "ck:{}",
                &stable_hash(&[cookies.join("; ").as_str()])[..6]
            ));
        }

        if parts.is_empty() {
            "default".to_string()
        } else {
            parts.join(" ")
        }
    }
}

fn compact_value(value: &str) -> String {
    const MAX_VALUE_LENGTH: usize = 16;

    if value.chars().count() <= MAX_VALUE_LENGTH {
        value.to_string()
    } else {
        stable_hash(&[value])[..6].to_string()
    }
}

impl Header {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

impl Cookie {
    fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

impl BodyMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Raw => "Raw",
            Self::FormData => "Form Data",
            Self::UrlEncoded => "URL Encoded",
            Self::Binary => "Binary",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::FormData => "form_data",
            Self::UrlEncoded => "urlencoded",
            Self::Binary => "binary",
        }
    }
}

fn parse_curl_args(args: &[String]) -> Result<RequestDraft, String> {
    let args = trim_curl_binary(args);

    if args.is_empty() {
        return Err("no curl arguments were provided".to_string());
    }

    let mut method = None;
    let mut url = None;
    let mut headers = Vec::new();
    let mut cookies = Vec::new();
    let mut body_parts = Vec::new();
    let mut body_mode = BodyMode::Raw;
    let mut json_mode = false;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();

        if arg == "--" {
            if let Some(value) = args.get(index + 1) {
                url.get_or_insert_with(|| value.clone());
            }
            break;
        }

        if arg == "-X" || arg == "--request" {
            method = Some(required_value(args, index, arg)?.to_uppercase());
            index += 2;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--request=") {
            method = Some(value.to_uppercase());
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("-X")
            && !value.is_empty()
        {
            method = Some(value.to_uppercase());
            index += 1;
            continue;
        }

        if arg == "-I" || arg == "--head" {
            method = Some("HEAD".to_string());
            index += 1;
            continue;
        }

        if arg == "--url" {
            url = Some(required_value(args, index, arg)?);
            index += 2;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--url=") {
            url = Some(value.to_string());
            index += 1;
            continue;
        }

        if arg == "-H" || arg == "--header" {
            add_header(
                &mut headers,
                &mut cookies,
                &required_value(args, index, arg)?,
            );
            index += 2;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--header=") {
            add_header(&mut headers, &mut cookies, value);
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("-H")
            && !value.is_empty()
        {
            add_header(&mut headers, &mut cookies, value);
            index += 1;
            continue;
        }

        if arg == "-b" || arg == "--cookie" {
            cookies.extend(parse_cookie_header(&required_value(args, index, arg)?));
            index += 2;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--cookie=") {
            cookies.extend(parse_cookie_header(value));
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("-b")
            && !value.is_empty()
        {
            cookies.extend(parse_cookie_header(value));
            index += 1;
            continue;
        }

        if arg == "--json" {
            body_parts.push(required_value(args, index, arg)?);
            body_mode = BodyMode::Raw;
            json_mode = true;
            index += 2;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--json=") {
            body_parts.push(value.to_string());
            body_mode = BodyMode::Raw;
            json_mode = true;
            index += 1;
            continue;
        }

        if is_data_flag(arg) {
            body_parts.push(required_value(args, index, arg)?);
            body_mode = body_mode_for_data_flag(arg);
            index += 2;
            continue;
        }

        if let Some(value) = data_value_from_inline_flag(arg) {
            body_parts.push(value.to_string());
            body_mode = body_mode_for_data_flag(arg);
            index += 1;
            continue;
        }

        if arg == "-A" || arg == "--user-agent" {
            headers.push(Header::new("User-Agent", required_value(args, index, arg)?));
            index += 2;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--user-agent=") {
            headers.push(Header::new("User-Agent", value));
            index += 1;
            continue;
        }

        if arg == "-e" || arg == "--referer" {
            headers.push(Header::new("Referer", required_value(args, index, arg)?));
            index += 2;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--referer=") {
            headers.push(Header::new("Referer", value));
            index += 1;
            continue;
        }

        if arg.starts_with('-') {
            index += 1 + usize::from(option_takes_value(arg) && args.get(index + 1).is_some());
            continue;
        }

        url.get_or_insert_with(|| arg.to_string());
        index += 1;
    }

    if json_mode {
        ensure_header(&mut headers, "Content-Type", "application/json");
        ensure_header(&mut headers, "Accept", "application/json");
    }

    let body = body_parts.join("&");
    let method = method.unwrap_or_else(|| {
        if body.is_empty() {
            "GET".to_string()
        } else {
            "POST".to_string()
        }
    });
    let url = url.ok_or_else(|| "no URL found in curl arguments".to_string())?;
    let parts = parse_url(&url)?;

    Ok(RequestDraft {
        method,
        url: parts.url,
        origin: parts.origin,
        path: parts.path,
        query: parts.query,
        headers,
        cookies,
        body_mode,
        body,
        raw_args: args.to_vec(),
    })
}

fn trim_curl_binary(args: &[String]) -> &[String] {
    if args
        .first()
        .and_then(|arg| arg.rsplit('/').next())
        .is_some_and(|arg| arg == "curl")
    {
        &args[1..]
    } else {
        args
    }
}

fn required_value(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn add_header(headers: &mut Vec<Header>, cookies: &mut Vec<Cookie>, raw: &str) {
    if let Some(header) = parse_header(raw) {
        if header.name.eq_ignore_ascii_case("cookie") {
            cookies.extend(parse_cookie_header(&header.value));
        }

        headers.push(header);
    }
}

fn parse_header(raw: &str) -> Option<Header> {
    let (name, value) = raw.split_once(':')?;
    let name = name.trim();

    if name.is_empty() {
        return None;
    }

    Some(Header::new(name, value.trim()))
}

fn parse_cookie_header(raw: &str) -> Vec<Cookie> {
    raw.split(';')
        .filter_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            let name = name.trim();

            if name.is_empty() {
                return None;
            }

            Some(Cookie::new(name, value.trim()))
        })
        .collect()
}

fn ensure_header(headers: &mut Vec<Header>, name: &str, value: &str) {
    if !headers
        .iter()
        .any(|header| header.name.eq_ignore_ascii_case(name))
    {
        headers.push(Header::new(name, value));
    }
}

fn is_data_flag(arg: &str) -> bool {
    matches!(
        arg,
        "-d" | "--data"
            | "--data-raw"
            | "--data-binary"
            | "--data-ascii"
            | "--data-urlencode"
            | "-F"
            | "--form"
            | "--form-string"
    )
}

fn data_value_from_inline_flag(arg: &str) -> Option<&str> {
    for prefix in [
        "--data=",
        "--data-raw=",
        "--data-binary=",
        "--data-ascii=",
        "--data-urlencode=",
        "--form=",
        "--form-string=",
    ] {
        if let Some(value) = arg.strip_prefix(prefix) {
            return Some(value);
        }
    }

    if let Some(value) = arg.strip_prefix("-d")
        && !value.is_empty()
    {
        return Some(value);
    }

    if let Some(value) = arg.strip_prefix("-F")
        && !value.is_empty()
    {
        return Some(value);
    }

    None
}

fn body_mode_for_data_flag(arg: &str) -> BodyMode {
    if arg == "-F"
        || arg == "--form"
        || arg == "--form-string"
        || arg.starts_with("-F")
        || arg.starts_with("--form=")
        || arg.starts_with("--form-string=")
    {
        BodyMode::FormData
    } else {
        BodyMode::UrlEncoded
    }
}

fn option_takes_value(arg: &str) -> bool {
    matches!(
        arg,
        "-o" | "--output"
            | "-w"
            | "--write-out"
            | "-u"
            | "--user"
            | "--connect-timeout"
            | "--max-time"
            | "--proxy"
            | "--resolve"
            | "--cacert"
            | "--cert"
            | "--key"
            | "--cert-type"
            | "--key-type"
            | "--interface"
            | "--limit-rate"
            | "--retry"
            | "--retry-delay"
            | "--retry-max-time"
    )
}

fn parse_url(raw: &str) -> Result<UrlParts, String> {
    let raw = raw.trim();

    if raw.is_empty() {
        return Err("URL is empty".to_string());
    }

    let (scheme, rest) = if let Some(index) = raw.find("://") {
        (&raw[..index], &raw[index + 3..])
    } else {
        ("https", raw)
    };

    if scheme.is_empty() {
        return Err(format!("invalid URL: {raw}"));
    }

    let authority_end = rest
        .find(|character| matches!(character, '/' | '?' | '#'))
        .unwrap_or(rest.len());
    let authority = &rest[..authority_end];

    if authority.is_empty() {
        return Err(format!("URL has no host: {raw}"));
    }

    let tail = &rest[authority_end..];
    let tail = tail.split_once('#').map_or(tail, |(before, _)| before);
    let (path, query) = match tail.split_once('?') {
        Some((path, query)) => (path, Some(query.to_string())),
        None => (tail, None),
    };
    let path = if path.is_empty() { "/" } else { path };
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    let origin = format!("{}://{}", scheme.to_lowercase(), authority);
    let url = if let Some(query) = &query {
        format!("{origin}{path}?{query}")
    } else {
        format!("{origin}{path}")
    };

    Ok(UrlParts {
        url,
        origin,
        path,
        query,
    })
}

pub fn stable_hash(parts: &[&str]) -> String {
    let mut hash = 0xcbf29ce484222325u64;

    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }

        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_get_url_into_origin_and_path() {
        let request = RequestDraft::from_curl_args(&strings(&["https://google.com/search"]))
            .expect("request parses");

        assert_eq!(request.method, "GET");
        assert_eq!(request.origin, "https://google.com");
        assert_eq!(request.path, "/search");
        assert_eq!(request.display_path(), "search");
    }

    #[test]
    fn parses_method_headers_body_and_cookies() {
        let request = RequestDraft::from_curl_args(&strings(&[
            "-X",
            "PUT",
            "https://api.example.com/users?id=1",
            "-H",
            "Authorization: Bearer {{access_token}}",
            "-H",
            "Cookie: sid=abc; theme=dark",
            "-d",
            "{\"name\":\"Ada\"}",
        ]))
        .expect("request parses");

        assert_eq!(request.method, "PUT");
        assert_eq!(request.query.as_deref(), Some("id=1"));
        assert_eq!(request.headers.len(), 2);
        assert_eq!(request.cookies.len(), 2);
        assert_eq!(request.body_mode, BodyMode::UrlEncoded);
        assert_eq!(request.body, "{\"name\":\"Ada\"}");
    }

    #[test]
    fn infers_post_for_data_without_explicit_method() {
        let request = RequestDraft::from_curl_args(&strings(&[
            "https://api.example.com/messages",
            "--data-raw",
            "hello=true",
        ]))
        .expect("request parses");

        assert_eq!(request.method, "POST");
        assert_eq!(request.body_mode, BodyMode::UrlEncoded);
    }

    #[test]
    fn form_flag_sets_form_data_body_mode() {
        let request = RequestDraft::from_curl_args(&strings(&[
            "https://api.example.com/upload",
            "-F",
            "name=Ada",
        ]))
        .expect("request parses");

        assert_eq!(request.body_mode, BodyMode::FormData);
    }

    #[test]
    fn fingerprint_separates_body_modes() {
        let mut raw = RequestDraft::from_url("https://google.com/search").expect("valid URL");
        raw.body = "q=rust".to_string();

        let mut urlencoded = raw.clone();
        urlencoded.set_body_mode(BodyMode::UrlEncoded);

        assert_ne!(raw.fingerprint(), urlencoded.fingerprint());
        assert!(urlencoded.variant_label().contains("mode:urlencoded"));
    }

    #[test]
    fn fingerprint_separates_querystring_and_body_variants() {
        let query_one =
            RequestDraft::from_curl_args(&strings(&["https://google.com/search?q=rust"]))
                .expect("request parses");
        let query_two =
            RequestDraft::from_curl_args(&strings(&["https://google.com/search?q=ratatui"]))
                .expect("request parses");
        let body_one = RequestDraft::from_curl_args(&strings(&[
            "https://google.com/search",
            "--data-raw",
            "q=rust",
        ]))
        .expect("request parses");
        let body_two = RequestDraft::from_curl_args(&strings(&[
            "https://google.com/search",
            "--data-raw",
            "q=ratatui",
        ]))
        .expect("request parses");

        assert_ne!(query_one.fingerprint(), query_two.fingerprint());
        assert_ne!(body_one.fingerprint(), body_two.fingerprint());
    }

    #[test]
    fn variant_label_surfaces_cookie_difference() {
        let with_cookie = RequestDraft::from_curl_args(&strings(&[
            "https://google.com/search",
            "-H",
            "Authorization: Bearer {{access_token}}",
            "-b",
            "sid=abc; theme=dark",
            "--data-raw",
            "q=curler",
        ]))
        .expect("request parses");
        let without_cookie = RequestDraft::from_curl_args(&strings(&[
            "https://google.com/search",
            "-H",
            "Authorization: Bearer {{access_token}}",
            "--data-raw",
            "q=curler",
        ]))
        .expect("request parses");

        assert_ne!(with_cookie.variant_label(), without_cookie.variant_label());
        assert!(with_cookie.variant_label().contains("ck:"));
        assert!(!without_cookie.variant_label().contains("ck:"));
    }

    #[test]
    fn setters_update_url_parts_and_query() {
        let mut request = RequestDraft::from_url("https://api.example.com").expect("valid URL");

        request
            .set_url("https://google.com/search")
            .expect("valid URL");
        request.set_query("q=rust");

        assert_eq!(request.origin, "https://google.com");
        assert_eq!(request.path, "/search");
        assert_eq!(request.query.as_deref(), Some("q=rust"));
        assert_eq!(request.url, "https://google.com/search?q=rust");
    }
}
