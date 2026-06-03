use super::config::WindowConfig;
use tauri::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchRequestSource {
    CommandLine,
    OpenedEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchTarget {
    ExistingWindow,
    NewWindow,
}

fn is_allowed_scheme(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

fn root_domain(hostname: &str) -> &str {
    let mut parts = hostname.rsplitn(3, '.');
    let last = parts.next();
    let second_last = parts.next();
    match (second_last, last) {
        (Some(second_last), Some(last)) => {
            let suffix = second_last.len() + last.len() + 1;
            &hostname[hostname.len() - suffix..]
        }
        _ => hostname,
    }
}

fn is_same_domain(candidate_url: &Url, current_url: &Url) -> bool {
    match (candidate_url.host_str(), current_url.host_str()) {
        (Some(candidate_host), Some(current_host)) => {
            candidate_host == current_host
                || root_domain(candidate_host) == root_domain(current_host)
        }
        _ => false,
    }
}

pub fn validate_launch_url(candidate: &str, window_config: &WindowConfig) -> Option<Url> {
    if window_config.url_type != "web" {
        return None;
    }

    let candidate_url = Url::parse(candidate).ok()?;
    if !is_allowed_scheme(&candidate_url) {
        return None;
    }

    if window_config.force_internal_navigation {
        return Some(candidate_url);
    }

    if !window_config.internal_url_regex.is_empty() {
        match regex::Regex::new(&window_config.internal_url_regex) {
            Ok(pattern) => {
                if pattern.is_match(candidate) {
                    return Some(candidate_url);
                }
                return None;
            }
            Err(_) => {
                let current_url = Url::parse(&window_config.url).ok()?;
                if is_same_domain(&candidate_url, &current_url) {
                    return Some(candidate_url);
                }
                return None;
            }
        }
    }

    let current_url = Url::parse(&window_config.url).ok()?;
    if is_same_domain(&candidate_url, &current_url) {
        return Some(candidate_url);
    }

    None
}

pub fn parse_launch_url_args(args: &[String], window_config: &WindowConfig) -> Option<Url> {
    args.iter()
        .find_map(|arg| validate_launch_url(arg, window_config))
}

pub fn contains_url_arg(args: &[String]) -> bool {
    args.iter().any(|arg| Url::parse(arg).is_ok())
}

pub fn decide_launch_target(source: LaunchRequestSource, multi_window: bool) -> LaunchTarget {
    match source {
        LaunchRequestSource::CommandLine if multi_window => LaunchTarget::NewWindow,
        _ => LaunchTarget::ExistingWindow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_window_config() -> WindowConfig {
        WindowConfig {
            url: "https://en.wikipedia.org/wiki/Main_Page".to_string(),
            hide_title_bar: false,
            fullscreen: false,
            maximize: false,
            width: 1200.0,
            height: 780.0,
            resizable: true,
            url_type: "web".to_string(),
            always_on_top: false,
            dark_mode: false,
            disabled_web_shortcuts: false,
            activation_shortcut: String::new(),
            hide_on_close: true,
            incognito: false,
            title: None,
            enable_wasm: false,
            enable_drag_drop: false,
            new_window: false,
            start_to_tray: false,
            force_internal_navigation: false,
            internal_url_regex: String::new(),
            enable_find: false,
            zoom: 100,
            min_width: 0.0,
            min_height: 0.0,
            ignore_certificate_errors: false,
        }
    }

    #[test]
    fn validate_launch_url_allows_same_root_domain_by_default() {
        let config = make_window_config();
        let result = validate_launch_url("https://fr.wikipedia.org/wiki/Genghis_Khan", &config);
        assert!(result.is_some());
    }

    #[test]
    fn validate_launch_url_rejects_other_domains_by_default() {
        let config = make_window_config();
        let result = validate_launch_url("https://example.com", &config);
        assert!(result.is_none());
    }

    #[test]
    fn validate_launch_url_prefers_internal_regex_when_present() {
        let mut config = make_window_config();
        config.internal_url_regex = "^https://en\\.wikipedia\\.org/wiki/".to_string();

        assert!(validate_launch_url("https://en.wikipedia.org/wiki/Rust", &config).is_some());
        assert!(validate_launch_url("https://fr.wikipedia.org/wiki/Rust", &config).is_none());
    }

    #[test]
    fn validate_launch_url_allows_any_https_url_when_force_internal_navigation_is_enabled() {
        let mut config = make_window_config();
        config.force_internal_navigation = true;

        assert!(validate_launch_url("https://example.com/path", &config).is_some());
    }

    #[test]
    fn validate_launch_url_rejects_non_http_urls_even_when_force_internal_navigation_is_enabled() {
        let mut config = make_window_config();
        config.force_internal_navigation = true;

        assert!(validate_launch_url("mailto:test@example.com", &config).is_none());
    }

    #[test]
    fn validate_launch_url_rejects_local_file_apps() {
        let mut config = make_window_config();
        config.url_type = "local".to_string();

        assert!(validate_launch_url("https://example.com/path", &config).is_none());
    }

    #[test]
    fn validate_launch_url_falls_back_to_same_domain_when_regex_is_invalid() {
        let mut config = make_window_config();
        config.internal_url_regex = "(".to_string();

        assert!(validate_launch_url("https://fr.wikipedia.org/wiki/Rust", &config).is_some());
        assert!(validate_launch_url("https://example.com/path", &config).is_none());
    }

    #[test]
    fn command_line_launch_uses_new_window_only_when_multi_window_is_enabled() {
        assert_eq!(
            decide_launch_target(LaunchRequestSource::CommandLine, false),
            LaunchTarget::ExistingWindow
        );
        assert_eq!(
            decide_launch_target(LaunchRequestSource::CommandLine, true),
            LaunchTarget::NewWindow
        );
    }

    #[test]
    fn opened_event_always_reuses_existing_window() {
        assert_eq!(
            decide_launch_target(LaunchRequestSource::OpenedEvent, false),
            LaunchTarget::ExistingWindow
        );
        assert_eq!(
            decide_launch_target(LaunchRequestSource::OpenedEvent, true),
            LaunchTarget::ExistingWindow
        );
    }

    #[test]
    fn contains_url_arg_detects_any_parseable_url() {
        let args = vec!["--flag".to_string(), "https://example.com/path".to_string()];
        assert!(contains_url_arg(&args));
    }

    #[test]
    fn contains_url_arg_ignores_non_urls() {
        let args = vec!["--flag".to_string(), "not-a-url".to_string()];
        assert!(!contains_url_arg(&args));
    }
}
