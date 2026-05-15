use std::{collections::BTreeMap, fs, io, path::Path};

use serde::{Deserialize, Serialize};

use crate::domain::request::{Cookie, Header, RequestDraft};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectState {
    pub shared_headers: Vec<Header>,
    pub cookies: Vec<Cookie>,
    pub variables: BTreeMap<String, String>,
    pub response_bindings: Vec<ResponseBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResponseBinding {
    pub variable: String,
    pub json_path: String,
}

impl ProjectState {
    pub fn load(path: &Path) -> io::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(path)?;

        serde_json::from_str::<Self>(&contents).map_err(invalid_data)
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let contents = serde_json::to_string_pretty(self).map_err(invalid_data)?;

        fs::write(path, contents)
    }

    pub fn merge_from_request(&mut self, request: &RequestDraft) {
        for header in &request.headers {
            self.collect_placeholders(&header.value);

            if header.name.eq_ignore_ascii_case("authorization")
                || header.name.eq_ignore_ascii_case("cookie")
            {
                upsert_header(&mut self.shared_headers, header.clone());
            }
        }

        self.collect_placeholders(&request.body);

        for cookie in &request.cookies {
            upsert_cookie(&mut self.cookies, cookie.clone());
        }
    }

    #[allow(dead_code)]
    pub fn apply_response_body(&mut self, response_body: &str) -> Result<(), serde_json::Error> {
        let response = serde_json::from_str::<serde_json::Value>(response_body)?;

        for binding in self.response_bindings.clone() {
            if let Some(value) = select_json_path(&response, &binding.json_path) {
                self.variables
                    .insert(binding.variable, json_value_to_string(value));
            }
        }

        Ok(())
    }

    pub fn resolve_value(&self, value: &str) -> String {
        let mut resolved = String::new();
        let mut rest = value;

        while let Some(start) = rest.find("{{") {
            resolved.push_str(&rest[..start]);
            rest = &rest[start + 2..];

            let Some(end) = rest.find("}}") else {
                resolved.push_str("{{");
                resolved.push_str(rest);
                return resolved;
            };

            let name = rest[..end].trim();

            match self.variables.get(name).filter(|value| !value.is_empty()) {
                Some(value) => resolved.push_str(value),
                None => {
                    resolved.push_str("{{");
                    resolved.push_str(&rest[..end]);
                    resolved.push_str("}}");
                }
            }

            rest = &rest[end + 2..];
        }

        resolved.push_str(rest);
        resolved
    }

    fn collect_placeholders(&mut self, value: &str) {
        for variable in placeholder_names(value) {
            self.variables.entry(variable.clone()).or_default();

            if !self
                .response_bindings
                .iter()
                .any(|binding| binding.variable == variable)
            {
                self.response_bindings.push(ResponseBinding {
                    json_path: format!("$.{variable}"),
                    variable,
                });
            }
        }
    }
}

fn upsert_header(headers: &mut Vec<Header>, header: Header) {
    if let Some(existing) = headers
        .iter_mut()
        .find(|existing| existing.name.eq_ignore_ascii_case(&header.name))
    {
        *existing = header;
    } else {
        headers.push(header);
    }
}

fn upsert_cookie(cookies: &mut Vec<Cookie>, cookie: Cookie) {
    if let Some(existing) = cookies
        .iter_mut()
        .find(|existing| existing.name == cookie.name)
    {
        *existing = cookie;
    } else {
        cookies.push(cookie);
    }
}

fn placeholder_names(value: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = value;

    while let Some(start) = rest.find("{{") {
        rest = &rest[start + 2..];

        let Some(end) = rest.find("}}") else {
            break;
        };

        let name = rest[..end].trim();

        if !name.is_empty()
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
            && !names.iter().any(|existing| existing == name)
        {
            names.push(name.to_string());
        }

        rest = &rest[end + 2..];
    }

    names
}

fn select_json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let path = path.strip_prefix("$.")?;
    let mut current = value;

    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }

        current = current.get(segment)?;
    }

    Some(current)
}

fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Null => String::new(),
        _ => value.to_string(),
    }
}

fn invalid_data(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_token_placeholder_from_authorization_header() {
        let mut state = ProjectState::default();
        let mut request = RequestDraft::from_url("https://api.example.com/me").unwrap();
        request.headers = vec![Header::new("Authorization", "Bearer {{access_token}}")];

        state.merge_from_request(&request);

        assert!(state.variables.contains_key("access_token"));
        assert_eq!(
            state.response_bindings,
            vec![ResponseBinding {
                variable: "access_token".to_string(),
                json_path: "$.access_token".to_string()
            }]
        );
    }

    #[test]
    fn applies_response_binding_and_resolves_header_template() {
        let mut state = ProjectState {
            response_bindings: vec![ResponseBinding {
                variable: "access_token".to_string(),
                json_path: "$.access_token".to_string(),
            }],
            ..ProjectState::default()
        };

        state
            .apply_response_body(r#"{"access_token":"token-123"}"#)
            .expect("response parses");

        assert_eq!(
            state.resolve_value("Bearer {{access_token}}"),
            "Bearer token-123"
        );
    }
}
