use super::*;
use serde_json::json;

const SCHEMA: &str = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";

pub(super) fn decode(
    value: &Value,
    warnings: &mut Vec<String>,
    variables: &mut Vec<String>,
) -> AppResult<NormalizedCollection> {
    warnings.push("postmanUnsupported".into());
    if value.get("auth").is_some() || value.get("event").is_some() {
        warnings.push("inheritedSettingsFlattened".into());
    }
    let mut parsed = NormalizedCollection {
        name: value["info"]["name"]
            .as_str()
            .unwrap_or("Imported API")
            .into(),
        description: value["info"]["description"].as_str().map(Into::into),
        folders: vec![],
        requests: vec![],
    };
    if let Some(items) = value["variable"].as_array() {
        variables.extend(
            items
                .iter()
                .filter_map(|v| v["key"].as_str().map(Into::into)),
        );
        if !items.is_empty() {
            warnings.push("variablesNotImported".into());
        }
    }
    walk(
        value,
        None,
        &Value::Null,
        &[],
        &[],
        &mut parsed,
        warnings,
        0,
    )?;
    Ok(parsed)
}

fn events(value: &Value, event: &str) -> Vec<String> {
    value["event"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["listen"] == event && e["disabled"] != true)
        .filter_map(|e| {
            let exec = &e["script"]["exec"];
            exec.as_str().map(Into::into).or_else(|| {
                exec.as_array().map(|lines| {
                    lines
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("\n")
                })
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn walk(
    value: &Value,
    parent: Option<String>,
    inherited_auth: &Value,
    inherited_pre: &[String],
    inherited_post: &[String],
    parsed: &mut NormalizedCollection,
    warnings: &mut Vec<String>,
    depth: usize,
) -> AppResult<()> {
    if depth > 64 || parsed.folders.len() + parsed.requests.len() > MAX_IMPORT_ITEMS {
        return Err(import_validation("Postman collection exceeds limits"));
    }
    let auth = value
        .get("auth")
        .filter(|v| !v.is_null())
        .unwrap_or(inherited_auth);
    let mut pre = inherited_pre.to_vec();
    pre.extend(events(value, "prerequest"));
    let mut post = inherited_post.to_vec();
    post.extend(events(value, "test"));
    for (index, item) in value["item"]
        .as_array()
        .ok_or_else(|| import_validation("Postman item must be an array"))?
        .iter()
        .enumerate()
    {
        if item.get("item").is_some() {
            let id = format!("folder:{}", parsed.folders.len());
            parsed.folders.push(NormalizedFolder {
                source_id: id.clone(),
                parent_source_id: parent.clone(),
                name: item["name"].as_str().unwrap_or("Group").into(),
                sort_order: index as i64,
            });
            if item.get("event").is_some() || item.get("auth").is_some() {
                warnings.push("inheritedSettingsFlattened".into());
            }
            walk(
                item,
                Some(id),
                auth,
                &pre,
                &post,
                parsed,
                warnings,
                depth + 1,
            )?;
        } else {
            let request = &item["request"];
            if !request.is_object() && !request.is_string() {
                return Err(import_validation("invalid Postman request"));
            }
            let raw_url = request
                .as_str()
                .or_else(|| request["url"].as_str())
                .or_else(|| request["url"]["raw"].as_str());
            let mut url: String = raw_url
                .map(Into::into)
                .unwrap_or_else(|| structured_url(&request["url"]));
            let query = pairs(&request["url"]["query"]);
            if request["url"].get("query").is_some() && request.get("x-unfour-url").is_none() {
                if let Some((base, _)) = url.split_once('?') {
                    url = base.into();
                }
            }
            if let Some(original) = request["x-unfour-url"].as_str() {
                url = original.into();
            }
            if url.trim().is_empty() {
                return Err(import_validation("missing Postman request URL"));
            }
            let mut request_pre = pre.clone();
            request_pre.extend(events(item, "prerequest"));
            let mut request_post = post.clone();
            request_post.extend(events(item, "test"));
            let (body, mut body_kind) = body(&request["body"], warnings)?;
            if let Some(kind) = request["x-unfour-body-kind"].as_str() {
                body_kind = kind.into();
            }
            if item.get("response").is_some()
                || request.get("description").is_some()
                || request.get("protocolProfileBehavior").is_some()
                || request["url"].get("variable").is_some()
            {
                warnings.push("postmanUnsupported".into());
            }
            parsed.requests.push(NormalizedRequest {
                parent_source_id: parent.clone(),
                name: item["name"].as_str().unwrap_or("Request").into(),
                method: request["method"]
                    .as_str()
                    .unwrap_or("GET")
                    .to_ascii_uppercase(),
                url,
                headers: pairs(&request["header"]),
                query,
                body,
                body_kind,
                auth_json: auth_json(
                    request.get("auth").filter(|v| !v.is_null()).unwrap_or(auth),
                    warnings,
                ),
                settings_json: request
                    .get("x-unfour-settings")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "{\"timeoutMs\":null}".into()),
                pre_request_script: (!request_pre.is_empty()).then(|| request_pre.join("\n")),
                post_response_script: (!request_post.is_empty()).then(|| request_post.join("\n")),
                script_schema_version: 1,
                sort_order: index as i64,
            });
        }
    }
    Ok(())
}

fn structured_url(value: &Value) -> String {
    let join = |v: &Value, separator: &str| {
        v.as_str().map(Into::into).unwrap_or_else(|| {
            v.as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(separator)
        })
    };
    let protocol = value["protocol"].as_str().unwrap_or("https");
    let host = join(&value["host"], ".");
    if host.is_empty() {
        return String::new();
    }
    let port = value["port"]
        .as_str()
        .map(|p| format!(":{p}"))
        .unwrap_or_default();
    format!("{protocol}://{host}{port}/{}", join(&value["path"], "/"))
}

fn pairs(value: &Value) -> Vec<KeyValue> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| KeyValue {
            key: p["key"].as_str().unwrap_or_default().into(),
            value: p["value"].as_str().unwrap_or_default().into(),
            enabled: !p["disabled"].as_bool().unwrap_or(false),
        })
        .collect()
}

fn auth_json(value: &Value, warnings: &mut Vec<String>) -> String {
    let kind = value["type"].as_str().unwrap_or("noauth");
    let entries = pairs(&value[kind]);
    let field = |key: &str| {
        entries
            .iter()
            .find(|p| p.key == key)
            .map(|p| p.value.clone())
            .unwrap_or_default()
    };
    match kind {
        "noauth" => json!({"type":"none"}),
        "bearer" => json!({"type":"bearer","token":field("token")}),
        "basic" => json!({"type":"basic","username":field("username"),"password":field("password")}),
        "apikey" => json!({"type":"api-key","key":field("key"),"value":field("value"),"addTo":if field("in")=="query" {"query"} else {"header"}}),
        _ => { warnings.push("unsupportedAuth".into()); json!({"type":"none"}) }
    }.to_string()
}

fn body(value: &Value, warnings: &mut Vec<String>) -> AppResult<(Option<String>, String)> {
    Ok(match value["mode"].as_str().unwrap_or("none") {
        "none" => (None, "none".into()),
        "raw" => (
            Some(value["raw"].as_str().unwrap_or_default().into()),
            if value["options"]["raw"]["language"] == "json" {
                "json"
            } else {
                "text"
            }
            .into(),
        ),
        "urlencoded" => (
            Some(serde_json::to_string(&pairs(&value["urlencoded"]))?),
            "form-urlencoded".into(),
        ),
        "formdata" => {
            let parts = value["formdata"].as_array().into_iter().flatten().enumerate().map(|(i,p)| {
                let file = p["type"] == "file";
                let mut part = json!({"id":format!("part:{i}"),"type":if file {"file"} else {"text"},"key":p["key"].as_str().unwrap_or_default(),"enabled":!p["disabled"].as_bool().unwrap_or(false)});
                if file { part["fileName"] = Value::Null; warnings.push("reselectFiles".into()); }
                else { part["value"] = json!(p["value"].as_str().unwrap_or_default()); }
                part
            }).collect::<Vec<_>>();
            (
                Some(serde_json::to_string(&parts)?),
                "multipart-form-data".into(),
            )
        }
        _ => {
            warnings.push("unsupportedBody".into());
            (None, "none".into())
        }
    })
}

pub(super) fn encode(parsed: &NormalizedCollection) -> AppResult<Value> {
    Ok(
        json!({"info":{"name":parsed.name,"description":parsed.description,"schema":SCHEMA},"item":encode_items(parsed, None)?}),
    )
}

fn encode_items(parsed: &NormalizedCollection, parent: Option<&str>) -> AppResult<Vec<Value>> {
    let mut items = Vec::new();
    for f in parsed
        .folders
        .iter()
        .filter(|f| f.parent_source_id.as_deref() == parent)
    {
        items.push((
            f.sort_order,
            json!({"name":f.name,"item":encode_items(parsed, Some(&f.source_id))?}),
        ));
    }
    for r in parsed
        .requests
        .iter()
        .filter(|r| r.parent_source_id.as_deref() == parent)
    {
        let pairs = |pairs: &[KeyValue]| {
            pairs
                .iter()
                .map(|p| json!({"key":p.key,"value":p.value,"disabled":!p.enabled}))
                .collect::<Vec<_>>()
        };
        let auth: Value = serde_json::from_str(&r.auth_json)?;
        let kind = auth["type"].as_str().unwrap_or("none");
        let (postman_kind, keys) = match kind {
            "bearer" => ("bearer", vec![("token", "token")]),
            "basic" => (
                "basic",
                vec![("username", "username"), ("password", "password")],
            ),
            "api-key" => (
                "apikey",
                vec![("key", "key"), ("value", "value"), ("in", "addTo")],
            ),
            _ => ("noauth", vec![]),
        };
        let mut postman_auth = json!({"type":postman_kind});
        if !keys.is_empty() {
            postman_auth[postman_kind] = json!(keys
                .iter()
                .map(|(dest, src)| json!({"key":dest,"value":auth[*src],"type":"string"}))
                .collect::<Vec<_>>());
        }
        let mut event = vec![];
        for (listen, script) in [
            ("prerequest", &r.pre_request_script),
            ("test", &r.post_response_script),
        ] {
            if let Some(script) = script {
                event.push(json!({"listen":listen,"script":{"type":"text/javascript","exec":script.split('\n').collect::<Vec<_>>()}}));
            }
        }
        let body = match r.body_kind.as_str() {
            "none" => Value::Null,
            "form-urlencoded" => {
                let p: Vec<KeyValue> = serde_json::from_str(r.body.as_deref().unwrap_or("[]"))?;
                json!({"mode":"urlencoded","urlencoded":pairs(&p)})
            }
            "multipart-form-data" => {
                let p: Vec<Value> = serde_json::from_str(r.body.as_deref().unwrap_or("[]"))?;
                json!({"mode":"formdata","formdata":p.iter().map(|p| if p["type"]=="file" {json!({"key":p["key"],"type":"file","src":null,"disabled":p["enabled"]==false})} else {json!({"key":p["key"],"type":"text","value":p["value"],"disabled":p["enabled"]==false})}).collect::<Vec<_>>()})
            }
            _ => {
                json!({"mode":"raw","raw":r.body,"options":{"raw":{"language":if r.body_kind=="json" {"json"} else {"text"}}}})
            }
        };
        let mut url = json!({"raw":r.url});
        if !r.query.is_empty() {
            url["query"] = json!(pairs(&r.query));
        }
        items.push((r.sort_order,json!({"name":r.name,"event":event,"request":{"method":r.method,"url":url,"header":pairs(&r.headers),"auth":postman_auth,"body":body,"x-unfour-url":r.url,"x-unfour-body-kind":r.body_kind,"x-unfour-settings":serde_json::from_str::<Value>(&r.settings_json)?}})));
    }
    items.sort_by_key(|(order, _)| *order);
    Ok(items.into_iter().map(|(_, item)| item).collect())
}
