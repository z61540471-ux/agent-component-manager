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
pub fn search(source: &str, query: &str) -> Result<Vec<MarketItem>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("agent-component-manager/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let request=match source{
  "GitHub"=>client.get("https://api.github.com/search/repositories").query(&[("q",query),("per_page","30")]),
  "MCP Registry"=>client.get("https://registry.modelcontextprotocol.io/v0.1/servers").query(&[("search",query),("limit","30"),("version","latest")]),
  "Claude Marketplace"=>client.get("https://raw.githubusercontent.com/anthropics/claude-plugins-official/main/.claude-plugin/marketplace.json"),
  _=>return Err("Unknown registry".into())};
    let response = request
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let value: Value = response.json().map_err(|e| e.to_string())?;
    normalize(source, query, &value)
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
