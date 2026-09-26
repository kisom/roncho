use serde_json::{Map, Value};

/// A file to turn into messages. `metadata` and `configuration` are sent as JSON strings.
pub struct FileUpload<'a> {
    pub peer_id: &'a str,
    pub filename: &'a str,
    pub bytes: &'a [u8],
    pub content_type: Option<&'a str>,
    pub metadata: Option<&'a Map<String, Value>>,
    pub configuration: Option<&'a Map<String, Value>>,
    pub created_at: Option<&'a str>,
}

pub(crate) fn prepare(upload: &FileUpload<'_>) -> Result<(Vec<u8>, String), String> {
    if upload.peer_id.is_empty() {
        return Err("upload peer_id is required".into());
    }
    if upload.bytes.is_empty() {
        return Err("upload file is empty".into());
    }
    let content_type = upload
        .content_type
        .unwrap_or_else(|| guess_content_type(upload.filename));
    if !supported_upload_type(content_type) {
        return Err(format!(
            "unsupported upload type {content_type}; use application/pdf, application/json, or text/*"
        ));
    }
    Ok(multipart_file(upload, content_type))
}

fn guess_content_type(filename: &str) -> &'static str {
    let lower = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match lower.as_str() {
        "pdf" => "application/pdf",
        "json" => "application/json",
        _ => "text/plain",
    }
}

fn supported_upload_type(content_type: &str) -> bool {
    let base = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim();
    base == "application/pdf" || base == "application/json" || base.starts_with("text/")
}

fn form_field(body: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
    );
}

fn multipart_file(upload: &FileUpload<'_>, content_type: &str) -> (Vec<u8>, String) {
    let boundary = "roncho-upload-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: {content_type}\r\n\r\n",
            upload.filename
        )
        .as_bytes(),
    );
    body.extend_from_slice(upload.bytes);
    body.extend_from_slice(b"\r\n");
    form_field(&mut body, boundary, "peer_id", upload.peer_id);
    if let Some(metadata) = upload.metadata {
        form_field(
            &mut body,
            boundary,
            "metadata",
            &Value::Object(metadata.clone()).to_string(),
        );
    }
    if let Some(configuration) = upload.configuration {
        form_field(
            &mut body,
            boundary,
            "configuration",
            &Value::Object(configuration.clone()).to_string(),
        );
    }
    if let Some(created_at) = upload.created_at {
        form_field(&mut body, boundary, "created_at", created_at);
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}
