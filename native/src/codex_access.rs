use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use url::{Host, Url};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexAccess {
    pub allow_all: bool,
    pub allowlist: Vec<String>,
    pub blocklist: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Change {
    AllowAll {
        enabled: bool,
    },
    Site {
        list: SiteList,
        site: String,
        add: bool,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SiteList {
    Allowlist,
    Blocklist,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum SiteAccess {
    Blocked,
    Automatic,
    Manual,
}

impl CodexAccess {
    pub fn normalize(&mut self) {
        for list in [&mut self.allowlist, &mut self.blocklist] {
            let mut seen = HashSet::new();
            list.retain_mut(|site| {
                let Some(domain) = site_domain(site) else {
                    return false;
                };
                *site = domain;
                seen.insert(site.clone())
            });
        }
    }

    pub fn apply(&mut self, change: &Change) -> Result<(), String> {
        match change {
            Change::AllowAll { enabled } => self.allow_all = *enabled,
            Change::Site { list, site, add } => {
                let site = site_domain(site).ok_or("请输入有效的域名或 HTTP(S) 网址。")?;
                let sites = match list {
                    SiteList::Allowlist => &mut self.allowlist,
                    SiteList::Blocklist => &mut self.blocklist,
                };
                if *add {
                    if !sites.contains(&site) {
                        sites.push(site);
                    }
                } else {
                    sites.retain(|saved| saved != &site);
                }
            }
        }
        Ok(())
    }

    pub fn has_automatic_access(&self) -> bool {
        self.allow_all || !self.allowlist.is_empty()
    }

    pub fn for_url(&self, url: &str) -> SiteAccess {
        let Some(url) = Url::parse(url).ok().filter(is_web) else {
            return SiteAccess::Manual;
        };
        let Some(host) = url.host_str() else {
            return SiteAccess::Manual;
        };
        let host = host.strip_suffix('.').unwrap_or(host);
        let matches = |sites: &[String]| {
            sites.iter().any(|site| {
                host == site
                    || (matches!(url.host(), Some(Host::Domain(_)))
                        && host
                            .strip_suffix(site)
                            .is_some_and(|prefix| prefix.ends_with('.')))
            })
        };
        if matches(&self.blocklist) {
            SiteAccess::Blocked
        } else if self.allow_all || matches(&self.allowlist) {
            SiteAccess::Automatic
        } else {
            SiteAccess::Manual
        }
    }
}

fn is_web(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
}

fn site_domain(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty()
        || input.len() > 8192
        || input.contains('\\')
        || input.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return None;
    }
    let url = if input.contains("://") {
        Url::parse(input).ok()?
    } else if !input.contains(['/', '?', '#', '@']) {
        Url::parse(&format!("https://{input}")).ok()?
    } else {
        return None;
    };
    if !is_web(&url) {
        return None;
    }
    let host = url.host_str()?;
    let host = host.strip_suffix('.').unwrap_or(host);
    if matches!(url.host(), Some(Host::Domain(_)))
        && (host.len() > 253
            || !host.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            }))
    {
        return None;
    }
    Some(host.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_normalize_and_match_only_the_domain_and_its_subdomains() {
        let mut access = CodexAccess {
            allowlist: vec![
                " https://EXAMPLE.COM.:444/path ".into(),
                "example.com".into(),
                "*.test".into(),
            ],
            blocklist: vec!["private.example.com".into(), "https://[::1]:444/".into()],
            ..Default::default()
        };
        access.normalize();
        assert_eq!(access.allowlist, ["example.com"]);
        assert!(access.for_url("http://www.example.com.:8000/") == SiteAccess::Automatic);
        assert!(access.for_url("https://notexample.com/") == SiteAccess::Manual);
        assert!(access.for_url("https://example.com.other.test/") == SiteAccess::Manual);
        assert!(access.for_url("https://deep.private.example.com/") == SiteAccess::Blocked);
        access.allow_all = true;
        assert!(access.for_url("https://notexample.com/") == SiteAccess::Automatic);
        assert!(access.for_url("http://[::1]/") == SiteAccess::Blocked);
        assert!(access.for_url("rove://settings/ai-agent") == SiteAccess::Manual);
        assert!(access.for_url("https://private.example.com/") == SiteAccess::Blocked);
    }
}
