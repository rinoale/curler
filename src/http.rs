use std::{io::Read, time::Duration};

use crate::{
    request::{BodyMode, Header, RequestDraft},
    state::ProjectState,
};

#[cfg_attr(test, allow(dead_code))]
const MAX_RESPONSE_BODY_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<Header>,
    pub body: String,
    pub truncated: bool,
}

impl HttpResponse {
    pub fn summary(&self, method: &str, url: &str) -> String {
        format!("{method} {url} -> {} {}", self.status, self.status_text)
    }
}

#[cfg_attr(test, allow(dead_code))]
pub fn send(request: &RequestDraft, state: &ProjectState) -> Result<HttpResponse, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build();
    let headers = effective_headers(request, state);
    let mut outbound = agent.request(request.method.as_str(), &request.url);

    for header in &headers {
        if !header.name.trim().is_empty() {
            outbound = outbound.set(header.name.trim(), header.value.as_str());
        }
    }

    let response = if request.body.is_empty() {
        outbound.call()
    } else {
        outbound.send_string(&request.body)
    };

    match response {
        Ok(response) => response_from_ureq(response),
        Err(ureq::Error::Status(_, response)) => response_from_ureq(response),
        Err(ureq::Error::Transport(error)) => Err(error.to_string()),
    }
}

#[cfg_attr(test, allow(dead_code))]
fn response_from_ureq(response: ureq::Response) -> Result<HttpResponse, String> {
    let status = response.status();
    let status_text = response.status_text().to_string();
    let headers = response
        .headers_names()
        .into_iter()
        .filter_map(|name| {
            let value = response.header(&name)?;
            Some(Header::new(name, value))
        })
        .collect::<Vec<_>>();
    let (body, truncated) = read_body(response)?;

    Ok(HttpResponse {
        status,
        status_text,
        headers,
        body,
        truncated,
    })
}

#[cfg_attr(test, allow(dead_code))]
fn read_body(response: ureq::Response) -> Result<(String, bool), String> {
    let mut bytes = Vec::new();
    let mut reader = response
        .into_reader()
        .take(MAX_RESPONSE_BODY_BYTES.saturating_add(1));
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let truncated = bytes.len() > MAX_RESPONSE_BODY_BYTES as usize;

    if truncated {
        bytes.truncate(MAX_RESPONSE_BODY_BYTES as usize);
    }

    let body = String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).to_string());

    Ok((body, truncated))
}

fn effective_headers(request: &RequestDraft, state: &ProjectState) -> Vec<Header> {
    let mut headers = Vec::new();

    for header in &state.shared_headers {
        upsert_header(
            &mut headers,
            Header::new(header.name.trim(), state.resolve_value(&header.value)),
        );
    }

    for header in &request.headers {
        upsert_header(
            &mut headers,
            Header::new(header.name.trim(), state.resolve_value(&header.value)),
        );
    }

    if request.body_mode == BodyMode::UrlEncoded
        && !request.body.is_empty()
        && !has_header(&headers, "content-type")
    {
        headers.push(Header::new(
            "Content-Type",
            "application/x-www-form-urlencoded",
        ));
    }

    let cookie_header = state
        .cookies
        .iter()
        .chain(request.cookies.iter())
        .filter(|cookie| !cookie.name.trim().is_empty())
        .map(|cookie| format!("{}={}", cookie.name.trim(), cookie.value.trim()))
        .collect::<Vec<_>>()
        .join("; ");

    if !cookie_header.is_empty() {
        append_header_value(&mut headers, "Cookie", &cookie_header);
    }

    headers
}

fn upsert_header(headers: &mut Vec<Header>, header: Header) {
    if header.name.trim().is_empty() {
        return;
    }

    if let Some(existing) = headers
        .iter_mut()
        .find(|existing| existing.name.eq_ignore_ascii_case(&header.name))
    {
        *existing = header;
    } else {
        headers.push(header);
    }
}

fn append_header_value(headers: &mut Vec<Header>, name: &str, value: &str) {
    if let Some(existing) = headers
        .iter_mut()
        .find(|existing| existing.name.eq_ignore_ascii_case(name))
    {
        if existing.value.trim().is_empty() {
            existing.value = value.to_string();
        } else {
            existing.value = format!("{}; {value}", existing.value.trim());
        }
    } else {
        headers.push(Header::new(name, value));
    }
}

fn has_header(headers: &[Header], name: &str) -> bool {
    headers
        .iter()
        .any(|header| header.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::Cookie;

    #[test]
    fn local_headers_override_shared_headers_and_values_are_resolved() {
        let mut state = ProjectState::default();
        state
            .variables
            .insert("token".to_string(), "abc".to_string());
        state
            .shared_headers
            .push(Header::new("Authorization", "Bearer {{token}}"));
        state
            .shared_headers
            .push(Header::new("Accept", "application/json"));

        let mut request = RequestDraft::from_url("https://example.com").unwrap();
        request.headers = vec![Header::new("Accept", "text/html")];

        let headers = effective_headers(&request, &state);

        assert!(headers.contains(&Header::new("Authorization", "Bearer abc")));
        assert!(headers.contains(&Header::new("Accept", "text/html")));
        assert!(!headers.contains(&Header::new("Accept", "application/json")));
    }

    #[test]
    fn cookies_are_joined_into_cookie_header() {
        let mut request = RequestDraft::from_url("https://example.com").unwrap();
        request.headers.clear();
        request.cookies = vec![Cookie {
            name: "session".to_string(),
            value: "local".to_string(),
        }];
        let state = ProjectState::default();

        let headers = effective_headers(&request, &state);

        assert_eq!(headers, vec![Header::new("Cookie", "session=local")]);
    }
}
