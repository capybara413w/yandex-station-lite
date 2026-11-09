// Based on YTerMusic by ccgauche (https://github.com/ccgauche/ytermusic)
// Adapted for station_daemon (Yandex Station Lite) under GNU GPL v3.0

use std::str::FromStr;

#[allow(dead_code)]
pub trait StringUtils {
    fn after(&self, needle: &str) -> Option<&str>;
    fn before(&self, needle: &str) -> Option<&str>;
    fn between(&self, start: &str, end: &str) -> Option<&str>;
    fn to_owned_(&self) -> Option<String>;
    fn parse_<T: FromStr>(&self) -> Option<T>;
    fn trim_(&self) -> Option<&str>;
}

impl StringUtils for &str {
    fn after(&self, needle: &str) -> Option<&str> {
        let idx = self.find(needle)?;
        Some(&self[idx + needle.len()..])
    }

    fn before(&self, needle: &str) -> Option<&str> {
        let idx = self.find(needle)?;
        Some(&self[..idx])
    }

    fn between(&self, start: &str, end: &str) -> Option<&str> {
        let start_pos = self.find(start)? + start.len();
        let slice = &self[start_pos..];
        let end_pos = slice.find(end)?;
        Some(&slice[..end_pos])
    }

    fn to_owned_(&self) -> Option<String> {
        Some(self.to_string())
    }

    fn parse_<T: FromStr>(&self) -> Option<T> {
        T::from_str(self).ok()
    }

    fn trim_(&self) -> Option<&str> {
        Some(str::trim(self))
    }
}

impl StringUtils for String {
    fn after(&self, needle: &str) -> Option<&str> {
        let idx = self.find(needle)?;
        Some(&self[idx + needle.len()..])
    }

    fn before(&self, needle: &str) -> Option<&str> {
        let idx = self.find(needle)?;
        Some(&self[..idx])
    }

    fn between(&self, start: &str, end: &str) -> Option<&str> {
        let start_pos = self.find(start)? + start.len();
        let slice = &self[start_pos..];
        let end_pos = slice.find(end)?;
        Some(&slice[..end_pos])
    }

    fn to_owned_(&self) -> Option<String> {
        Some(self.clone())
    }

    fn parse_<T: FromStr>(&self) -> Option<T> {
        T::from_str(self).ok()
    }

    fn trim_(&self) -> Option<&str> {
        Some(self.trim())
    }
}

impl<T: StringUtils> StringUtils for Option<T> {
    fn after(&self, needle: &str) -> Option<&str> {
        self.as_ref().and_then(|s| s.after(needle))
    }

    fn before(&self, needle: &str) -> Option<&str> {
        self.as_ref().and_then(|s| s.before(needle))
    }

    fn between(&self, start: &str, end: &str) -> Option<&str> {
        self.as_ref().and_then(|s| s.between(start, end))
    }

    fn to_owned_(&self) -> Option<String> {
        self.as_ref().and_then(|s| s.to_owned_())
    }

    fn parse_<E: FromStr>(&self) -> Option<E> {
        self.as_ref().and_then(|s| s.parse_::<E>())
    }

    fn trim_(&self) -> Option<&str> {
        self.as_ref().and_then(|s| s.trim_())
    }
}
