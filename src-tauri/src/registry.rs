use crate::model::*;
use serde_json::Value;
use std::{collections::BTreeMap, io::Read, path::PathBuf};

pub fn mcp_detail(name: &str, version: &str) -> Result<Value, String> {
    if name.is_empty() || version.is_empty() {
        return Err("Server name and version are required".into());
    }
    let mut url = reqwest::Url::parse("https://registry.modelcontextprotocol.io/v0.1/servers/")
        .map_err(|e| e.to_string())?;
    url.path_segments_mut()
        .map_err(|_| "Invalid registry URL")?
        .pop_if_empty()
        .push(name)
        .push("versions")
        .push(version);
    let client = reqwest::blocking::Client::builder()
        .user_agent("agent-component-manager/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let value: Value = client
        .get(url)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    let server = value.get("server").unwrap_or(&value);
    if server["name"] != name || server["version"].as_str().unwrap_or("").is_empty() {
        return Err("Registry returned an invalid server identity".into());
    }
    let remote = server["remotes"]
        .as_array()
        .and_then(|r| r.iter().find(|r| r["type"] == "streamable-http"));
    let config = remote
        .and_then(|r| r["url"].as_str())
        .map(|url| serde_json::json!({"url":url}));
    Ok(
        serde_json::json!({"name":name,"version":server["version"],"description":server["description"],"config":config,"source":"MCP Registry","fetchedAt":now(),"warnings":["Review transport and required credentials in registry metadata before generating a preview. Package-only servers require an explicit configuration; no runtime is installed automatically."],"metadata":server}),
    )
}

// Resolve the requested revision once, then download every payload at that commit.
pub fn github_package(
    repository: &str,
    revision: &str,
    subdirectory: &str,
) -> Result<(String, BTreeMap<PathBuf, Vec<u8>>), String> {
    let parts: Vec<_> = repository.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|p| {
            p.is_empty()
                || !p
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        })
    {
        return Err("Use a GitHub owner/repository identifier".into());
    }
    if revision.is_empty()
        || !revision
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    {
        return Err("Use a commit SHA, tag, or simple branch name".into());
    }
    let subdirectory = subdirectory.trim_matches('/');
    if subdirectory
        .split('/')
        .any(|p| p == ".." || p == "." || p.contains('\\') || p.contains(':'))
    {
        return Err("Invalid package subdirectory".into());
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent("agent-component-manager/0.1")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let commit: Value = client
        .get(format!(
            "https://api.github.com/repos/{repository}/commits/{revision}"
        ))
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    let sha = commit["sha"]
        .as_str()
        .filter(|s| s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or("Invalid resolved commit")?
        .to_string();
    let tree: Value = client
        .get(format!(
            "https://api.github.com/repos/{repository}/git/trees/{sha}"
        ))
        .query(&[("recursive", "1")])
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;
    if tree["truncated"].as_bool() == Some(true) {
        return Err("Repository tree truncated; import a local checkout instead".into());
    }
    let prefix = if subdirectory.is_empty() {
        String::new()
    } else {
        format!("{subdirectory}/")
    };
    let mut files = BTreeMap::new();
    let mut total = 0;
    for entry in tree["tree"].as_array().ok_or("Missing GitHub tree")? {
        let path = entry["path"].as_str().ok_or("Missing tree path")?;
        let Some(relative) = path.strip_prefix(&prefix) else {
            continue;
        };
        if entry["type"] == "tree" {
            continue;
        }
        if entry["type"] != "blob" || entry["mode"] == "120000" {
            return Err("Linked files and submodules cannot be imported".into());
        }
        if relative.is_empty()
            || relative.split('/').any(|p| {
                p.is_empty()
                    || p == ".."
                    || p == "."
                    || p.contains('\\')
                    || p.contains(':')
                    || p.ends_with('.')
                    || p.ends_with(' ')
            })
        {
            return Err("Unsafe source path".into());
        }
        if entry["size"].as_u64().unwrap_or(u64::MAX) > 16 * 1024 * 1024 || files.len() >= 1000 {
            return Err("Remote package limit exceeded".into());
        }
        let mut url = reqwest::Url::parse(&format!(
            "https://raw.githubusercontent.com/{repository}/{sha}/"
        ))
        .map_err(|e| e.to_string())?;
        url.path_segments_mut()
            .map_err(|_| "Invalid raw source URL")?
            .pop_if_empty()
            .extend(path.split('/'));
        let response = client
            .get(url)
            .send()
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let mut bytes = vec![];
        response
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        total += bytes.len();
        if bytes.len() > 16 * 1024 * 1024 || total > 64 * 1024 * 1024 {
            return Err("Remote package download limit exceeded".into());
        }
        files.insert(PathBuf::from(relative), bytes);
    }
    let manifest = files
        .get(&PathBuf::from("SKILL.md"))
        .ok_or("Select a skill subdirectory containing SKILL.md")?;
    crate::planner::validate_skill(
        std::str::from_utf8(manifest).map_err(|_| "SKILL.md must be UTF-8")?,
    )?;
    Ok((sha, files))
}
const PAGE_SIZE: usize = 30;
#[cfg(test)]
mod live_checks {
    use super::*;
    fn check(source: &str, query: &str) {
        let first = search(source, query, None).expect("Live registry first page");
        assert!(!first.items.is_empty());
        assert!(first.items.len() <= PAGE_SIZE);
        println!("{source}: {} first-page items", first.items.len());
        if let Some(cursor) = first.next_cursor {
            let second = search(source, query, Some(&cursor)).expect("Live registry next page");
            assert!(!second.items.is_empty());
            assert!(second.items.len() <= PAGE_SIZE);
            assert_ne!(
                first.items.iter().map(|i| &i.name).collect::<Vec<_>>(),
                second.items.iter().map(|i| &i.name).collect::<Vec<_>>()
            );
            println!("{source}: {} second-page items", second.items.len());
        }
    }
    #[test]
    #[ignore = "Requires live public registry network access"]
    fn github() {
        check("GitHub", "agent skills");
    }
    #[test]
    #[ignore = "Requires live public registry network access"]
    fn mcp_registry() {
        check("MCP Registry", "");
    }
    #[test]
    #[ignore = "Requires live public registry network access"]
    fn claude_marketplace() {
        check("Claude Marketplace", "");
    }
}
pub fn cache_key(source: &str, query: &str, cursor: Option<&str>) -> String {
    // Versioned tuple avoids delimiter collisions and never reads legacy unpaged arrays.
    format!("market:v2:{}", serde_json::json!([source, query, cursor]))
}
pub fn cache_page(
    db: &crate::storage::Storage,
    key: &str,
    result: Result<MarketResult, String>,
) -> Result<MarketResult, String> {
    match result {
        Ok(mut page) => {
            if let Err(e) = db.put(key, &page) {
                page.message = Some(format!(
                    "{} Cache could not be saved: {e}",
                    page.message.unwrap_or_default()
                ));
            }
            Ok(page)
        }
        Err(e) => match db.get::<MarketResult>(key) {
            Some(mut page) => {
                page.stale = true;
                page.message = Some(e);
                Ok(page)
            }
            None => Err(e),
        },
    }
}
fn page_number(cursor: Option<&str>, max: usize) -> Result<usize, String> {
    let page = cursor
        .unwrap_or("1")
        .parse::<usize>()
        .map_err(|_| "Invalid market page")?;
    if page == 0 || page > max {
        return Err("Market page limit exceeded".into());
    }
    Ok(page)
}
pub fn search(source: &str, query: &str, cursor: Option<&str>) -> Result<MarketResult, String> {
    if query.len() > 512 || cursor.is_some_and(|c| c.len() > 4096 || c.is_empty()) {
        return Err("Market query or cursor limit exceeded".into());
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent("agent-component-manager/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let request = match source {
        "GitHub" => client
            .get("https://api.github.com/search/repositories")
            .query(&[
                ("q", query.to_string()),
                ("per_page", "30".into()),
                ("page", page_number(cursor, 34)?.to_string()),
            ]),
        "MCP Registry" => {
            let request = client
                .get("https://registry.modelcontextprotocol.io/v0.1/servers")
                .query(&[("search", query), ("limit", "30"), ("version", "latest")]);
            if let Some(cursor) = cursor {
                request.query(&[("cursor", cursor)])
            } else {
                request
            }
        }
        "Claude Marketplace" => {
            page_number(cursor, 334)?;
            client.get("https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json")
        }
        _ => return Err("Unknown registry".into()),
    };
    let response = request.send().map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(status_error(
            source,
            response.status().as_u16(),
            response.headers(),
        ));
    }
    let mut bytes = vec![];
    response
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("Registry response exceeds 4 MiB limit".into());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    normalize_page(source, query, cursor, &value)
}
fn status_error(source: &str, status: u16, headers: &reqwest::header::HeaderMap) -> String {
    if status == 429
        || (status == 403
            && (headers.contains_key("retry-after")
                || headers
                    .get("x-ratelimit-remaining")
                    .is_some_and(|v| v == "0")))
    {
        let retry = headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unavailable");
        let reset = headers
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unavailable");
        format!("{source} rate limit reached (HTTP {status}); Retry-After: {retry}; reset Unix time: {reset}")
    } else {
        format!("{source} request failed (HTTP {status})")
    }
}
fn normalize_page(
    source: &str,
    query: &str,
    cursor: Option<&str>,
    value: &Value,
) -> Result<MarketResult, String> {
    let mut items = normalize(source, query, value)?;
    let mut message = None;
    let next_cursor = match source {
        "GitHub" => {
            let page = page_number(cursor, 34)?;
            let total = value["total_count"]
                .as_u64()
                .ok_or("GitHub response missing total_count")?
                .min(1000) as usize;
            if value["incomplete_results"] == true {
                message =
                    Some("GitHub returned incomplete results; narrow the search or retry.".into());
            }
            items.truncate(
                1000usize
                    .saturating_sub((page - 1) * PAGE_SIZE)
                    .min(PAGE_SIZE),
            );
            (page * PAGE_SIZE < total && items.len() == PAGE_SIZE).then(|| (page + 1).to_string())
        }
        "MCP Registry" => {
            let next = value["metadata"]["nextCursor"]
                .as_str()
                .filter(|s| !s.is_empty());
            if next.is_some_and(|s| s.len() > 4096 || Some(s) == cursor) {
                return Err("Registry returned invalid or repeated cursor".into());
            }
            if items.len() > PAGE_SIZE {
                return Err("Registry exceeded requested page size".into());
            }
            next.map(str::to_owned)
        }
        "Claude Marketplace" => {
            let page = page_number(cursor, 334)?;
            if items.len() > 10_000 {
                return Err("Catalog entry limit exceeded".into());
            }
            let total = items.len();
            items = items
                .into_iter()
                .skip((page - 1) * PAGE_SIZE)
                .take(PAGE_SIZE)
                .collect();
            (page * PAGE_SIZE < total).then(|| (page + 1).to_string())
        }
        _ => return Err("Unknown registry".into()),
    };
    Ok(MarketResult {
        items,
        stale: false,
        message,
        next_cursor,
    })
}
pub fn normalize(source: &str, query: &str, v: &Value) -> Result<Vec<MarketItem>, String> {
    let key = match source {
        "GitHub" => "items",
        "MCP Registry" => "servers",
        "Claude Marketplace" => "plugins",
        _ => return Err("Unknown registry".into()),
    };
    let values = v[key]
        .as_array()
        .ok_or("Registry response has no expected result array")?;
    let mut items = vec![];
    for entry in values {
        let server = if source == "MCP Registry" {
            &entry["server"]
        } else {
            entry
        };
        let name = server[if source == "GitHub" {
            "full_name"
        } else {
            "name"
        }]
        .as_str()
        .ok_or("Registry entry missing name")?;
        let description = server["description"].as_str().unwrap_or("");
        if source == "Claude Marketplace"
            && !format!("{name} {description}")
                .to_lowercase()
                .contains(&query.to_lowercase())
        {
            continue;
        }
        let url = if source == "GitHub" {
            server["html_url"].as_str().unwrap_or("").to_string()
        } else if source == "MCP Registry" {
            server["repository"]["url"]
                .as_str()
                .unwrap_or("https://registry.modelcontextprotocol.io")
                .to_string()
        } else {
            if let Some(repo) = server["source"]["repo"].as_str() {
                format!("https://github.com/{repo}")
            } else if let Some(relative) = server["source"].as_str() {
                format!(
                    "https://github.com/anthropics/claude-plugins-official/tree/main/{}",
                    relative.trim_start_matches("./")
                )
            } else {
                server["source"]["url"]
                    .as_str()
                    .unwrap_or("https://github.com/anthropics/claude-plugins-official")
                    .to_string()
            }
        };
        items.push(MarketItem {
            name: name.into(),
            description: description.into(),
            source: source.into(),
            url,
            version: server["version"].as_str().map(String::from),
            stars: server["stargazers_count"].as_u64(),
            fetched_at: now(),
        });
    }
    Ok(items)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_fallback_is_exact_and_preserves_page_time() {
        let temp = tempfile::tempdir().unwrap();
        let db = crate::storage::Storage::open(&temp.path().join("cache.sqlite")).unwrap();
        let key = cache_key("GitHub", "skill", None);
        let page = MarketResult {
            items: vec![MarketItem {
                name: "fixture".into(),
                description: "".into(),
                source: "GitHub".into(),
                url: "".into(),
                version: None,
                stars: None,
                fetched_at: 42,
            }],
            stale: false,
            message: None,
            next_cursor: Some("2".into()),
        };
        cache_page(&db, &key, Ok(page)).unwrap();
        let cached = cache_page(&db, &key, Err("rate limited".into())).unwrap();
        assert!(cached.stale);
        assert_eq!(cached.items[0].fetched_at, 42);
        assert_eq!(cached.next_cursor.as_deref(), Some("2"));
        assert_eq!(cached.message.as_deref(), Some("rate limited"));
        assert!(cache_page(
            &db,
            &cache_key("GitHub", "skill", Some("2")),
            Err("offline".into())
        )
        .is_err());
        assert!(cache_page(
            &db,
            &cache_key("MCP Registry", "skill", None),
            Err("offline".into())
        )
        .is_err());
        assert!(cache_page(
            &db,
            &cache_key("GitHub", "different", None),
            Err("offline".into())
        )
        .is_err());
        db.put("market:GitHub:legacy", &Vec::<MarketItem>::new())
            .unwrap();
        assert!(cache_page(
            &db,
            &cache_key("GitHub", "legacy", None),
            Err("offline".into())
        )
        .is_err());
    }
    #[test]
    fn pages_are_bounded_and_keep_provider_cursors() {
        let github = serde_json::json!({"items":(0..30).map(|n| serde_json::json!({"full_name":format!("repo/{n}")})).collect::<Vec<_>>(),"total_count":1200});
        assert_eq!(
            normalize_page("GitHub", "", None, &github)
                .unwrap()
                .next_cursor
                .as_deref(),
            Some("2")
        );
        let last = normalize_page("GitHub", "", Some("34"), &github).unwrap();
        assert_eq!(last.items.len(), 10);
        assert!(last.next_cursor.is_none());
        assert!(normalize_page("GitHub", "", Some("35"), &github).is_err());
        let mcp = serde_json::json!({"servers":[],"metadata":{"nextCursor":"opaque+/="}});
        assert_eq!(
            normalize_page("MCP Registry", "", None, &mcp)
                .unwrap()
                .next_cursor
                .as_deref(),
            Some("opaque+/=")
        );
        assert!(normalize_page("MCP Registry", "", Some("opaque+/="), &mcp).is_err());
    }
    #[test]
    fn catalog_filters_before_paging_and_cache_keys_do_not_collide() {
        let plugins = serde_json::json!({"plugins":(0..65).map(|n| serde_json::json!({"name":format!("plugin-{n}"),"description":if n < 31 {"wanted"} else {"other"}})).collect::<Vec<_>>()});
        let first = normalize_page("Claude Marketplace", "wanted", None, &plugins).unwrap();
        let second = normalize_page("Claude Marketplace", "wanted", Some("2"), &plugins).unwrap();
        assert_eq!(first.items.len(), 30);
        assert_eq!(first.next_cursor.as_deref(), Some("2"));
        assert_eq!(second.items.len(), 1);
        assert!(second.next_cursor.is_none());
        assert_ne!(cache_key("a:b", "c", None), cache_key("a", "b:c", None));
        assert_ne!(
            cache_key("GitHub", "skill", None),
            cache_key("GitHub", "skill", Some("2"))
        );
        let legacy: MarketResult =
            serde_json::from_value(serde_json::json!({"items":[],"stale":false,"message":null}))
                .unwrap();
        assert!(legacy.next_cursor.is_none());
    }
    #[test]
    fn rate_limit_errors_include_retry_information() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-ratelimit-remaining", "0".parse().unwrap());
        headers.insert("retry-after", "60".parse().unwrap());
        assert!(status_error("GitHub", 403, &headers).contains("rate limit reached"));
        assert!(status_error("MCP Registry", 429, &headers).contains("Retry-After: 60"));
        assert!(status_error("GitHub", 500, &headers).contains("HTTP 500"));
    }
    #[test]
    fn official_registry_envelope() {
        let v = serde_json::json!({"servers":[{"server":{"name":"io.example/test","description":"Example","version":"1.0"},"_meta":{}}]});
        let result = normalize("MCP Registry", "", &v).unwrap();
        assert_eq!(result[0].version.as_deref(), Some("1.0"));
        assert_eq!(result[0].stars, None);
    }
    #[test]
    fn malformed_response_fails() {
        assert!(normalize("GitHub", "", &serde_json::json!({"message":"rate limited"})).is_err())
    }
}
