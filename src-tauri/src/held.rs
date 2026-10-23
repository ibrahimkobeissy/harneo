//! Values the backend keeps for the window: plans, previews and picked files wait here under a
//! random token. The window shows them and sends back only the token and the user's choices, so
//! what is applied or imported is always what the backend produced, never what the webview says.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long an entry waits before it is dropped.
const TTL: Duration = Duration::from_secs(30 * 60);
/// The most entries kept; a new one beyond it pushes out the oldest.
const MAX_ENTRIES: usize = 32;

const GONE: &str = "that step expired or was already used; start it again";

/// Entries by token, each with its age.
pub struct Held<T> {
    entries: Mutex<HashMap<String, (T, Instant)>>,
    ttl: Duration,
    max: usize,
}

impl<T> Default for Held<T> {
    fn default() -> Self {
        Self::with_limits(TTL, MAX_ENTRIES)
    }
}

/// 128 random bits as hex.
fn new_token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| format!("no random source: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

impl<T> Held<T> {
    pub fn with_limits(ttl: Duration, max: usize) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            ttl,
            max: max.max(1),
        }
    }
}

impl<T: Clone> Held<T> {
    fn locked<R>(
        &self,
        f: impl FnOnce(&mut HashMap<String, (T, Instant)>) -> Result<R, String>,
    ) -> Result<R, String> {
        let mut guard = self.entries.lock().map_err(|e| e.to_string())?;
        let ttl = self.ttl;
        guard.retain(|_, (_, at)| at.elapsed() < ttl);
        f(&mut guard)
    }

    fn insert(&self, token: String, value: T) -> Result<(), String> {
        let max = self.max;
        self.locked(|map| {
            while map.len() >= max {
                let oldest = map
                    .iter()
                    .min_by_key(|(_, (_, at))| *at)
                    .map(|(key, _)| key.clone());
                match oldest {
                    Some(key) => map.remove(&key),
                    None => break,
                };
            }
            map.insert(token, (value, Instant::now()));
            Ok(())
        })
    }

    /// Keeps `value` and returns the token that names it.
    pub fn put(&self, value: T) -> Result<String, String> {
        let token = new_token()?;
        self.insert(token.clone(), value)?;
        Ok(token)
    }

    /// A copy of the held value; it stays held.
    pub fn get(&self, token: &str) -> Result<T, String> {
        self.locked(|map| {
            map.get(token)
                .map(|(value, _)| value.clone())
                .ok_or_else(|| GONE.to_owned())
        })
    }

    /// Removes and returns the held value, so it can be used once.
    pub fn take(&self, token: &str) -> Result<T, String> {
        self.locked(|map| {
            map.remove(token)
                .map(|(value, _)| value)
                .ok_or_else(|| GONE.to_owned())
        })
    }

    /// Puts a taken value back under its token (a failed apply can be tried again).
    pub fn restore(&self, token: &str, value: T) {
        let _ = self.insert(token.to_owned(), value);
    }

    /// Forgets a value that is no longer needed.
    pub fn discard(&self, token: &str) {
        let _ = self.locked(|map| Ok(map.remove(token)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_names_one_value_and_take_uses_it_once() -> Result<(), String> {
        let held = Held::<String>::default();
        let token = held.put("plan".into())?;
        assert_eq!(token.len(), 32);
        assert!(token.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(token, held.put("other".into())?);
        assert_eq!(held.get(&token)?, "plan");
        assert_eq!(held.take(&token)?, "plan");
        assert!(held.take(&token).is_err());
        held.restore(&token, "plan".into());
        assert_eq!(held.get(&token)?, "plan");
        held.discard(&token);
        assert!(held.get(&token).is_err());
        assert!(held.get("not-a-token").is_err());
        Ok(())
    }

    #[test]
    fn entries_expire_and_the_oldest_makes_room() -> Result<(), String> {
        let short = Held::<u8>::with_limits(Duration::ZERO, 4);
        let gone = short.put(1)?;
        assert!(short.get(&gone).is_err());

        let small = Held::<u8>::with_limits(Duration::from_secs(60), 2);
        let first = small.put(1)?;
        std::thread::sleep(Duration::from_millis(2));
        let second = small.put(2)?;
        std::thread::sleep(Duration::from_millis(2));
        let third = small.put(3)?;
        assert!(small.get(&first).is_err());
        assert_eq!(small.get(&second)?, 2);
        assert_eq!(small.get(&third)?, 3);
        Ok(())
    }
}
