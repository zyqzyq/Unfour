use reqwest::multipart::{Form, Part};
use unfour_core::models::{parse_multipart_definition, ApiMultipartPart, ApiRequestInput};
use unfour_core::{AppError, AppResult};

pub(super) async fn build_form(input: &ApiRequestInput) -> AppResult<Form> {
    let parts = parse_multipart_definition(input.body.as_deref())?;
    let mut form = Form::new();
    for part in parts.into_iter().filter(ApiMultipartPart::enabled) {
        if part.key().trim().is_empty() {
            return Err(AppError::Validation(
                "Multipart field key cannot be empty".into(),
            ));
        }
        match part {
            ApiMultipartPart::Text { key, value, .. } => {
                form = form.text(key, value);
            }
            ApiMultipartPart::File {
                id, key, file_name, ..
            } => {
                let binding = input
                    .multipart_parts
                    .iter()
                    .find(|binding| binding.id == id)
                    .filter(|binding| !binding.file_path.is_empty())
                    .ok_or_else(|| {
                        AppError::Validation(format!(
                            "Multipart file field \"{key}\" requires a local file selection."
                        ))
                    })?;
                // Open asynchronously and stream; errors never include the local path.
                let file = tokio::fs::File::open(&binding.file_path)
                    .await
                    .map_err(|_| file_error())?;
                let metadata = file.metadata().await.map_err(|_| file_error())?;
                if !metadata.is_file() {
                    return Err(file_error());
                }
                let part = Part::stream_with_length(reqwest::Body::from(file), metadata.len())
                    .file_name(file_name.unwrap_or_else(|| "upload".into()));
                form = form.part(key, part);
            }
        }
    }
    Ok(form)
}

fn file_error() -> AppError {
    AppError::Validation(
        "Selected multipart file is missing or unreadable; select the file again.".into(),
    )
}
