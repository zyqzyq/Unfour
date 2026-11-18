use super::*;

pub(crate) struct TrustPrompt {
    pub(in crate::ssh::temporary) queue: std::sync::Weak<Mutex<Queue>>,
    pub(in crate::ssh::temporary) handle: String,
    initial_fingerprint: Option<String>,
    key_verified: bool,
}

impl TrustPrompt {
    pub(in crate::ssh::temporary) fn new(
        queue: std::sync::Weak<Mutex<Queue>>,
        handle: String,
    ) -> Self {
        Self {
            queue,
            handle,
            initial_fingerprint: None,
            key_verified: false,
        }
    }

    pub(crate) fn finish_key_exchange(&mut self, workspace: &str) -> Result<(), russh::Error> {
        // The patched client invokes verify before this callback on EVERY
        // exchange. Consume that evidence, so missing callbacks fail closed.
        let verified = std::mem::take(&mut self.key_verified);
        let queue = self.queue.upgrade().ok_or(russh::Error::UnknownKey)?;
        let mut queue = queue.lock().map_err(|_| russh::Error::UnknownKey)?;
        let entry = queue
            .owned(workspace, &self.handle)
            .map_err(|_| russh::Error::UnknownKey)?;
        if !Self::active(entry) {
            return Err(russh::Error::UnknownKey);
        }
        if !verified
            || self.initial_fingerprint.is_none()
            || entry.trusted_fingerprint != self.initial_fingerprint
        {
            entry.summary.error_code = Some("TEMPORARY_REKEY_REQUIRES_RELAUNCH".into());
            if let Some(cancel) = &entry.cancel {
                cancel.send_replace(true);
            }
            return Err(russh::Error::UnknownKey);
        }
        Ok(())
    }

    fn active(entry: &Entry) -> bool {
        matches!(entry.summary.status.as_str(), "connecting" | "connected")
            && entry.summary.error_code.is_none()
            && !entry.cancel.as_ref().is_some_and(|cancel| *cancel.borrow())
    }

    pub(crate) async fn verify(
        &mut self,
        store: &HostKeyStore,
        workspace: &str,
        host: &str,
        port: u16,
        fingerprint: &str,
    ) -> bool {
        self.key_verified = false;
        // Durable pins always take precedence. Temporary associations remain
        // in memory, and rekeys must also match this transport's initial pin.
        let stored = match store
            .matches_known_fingerprint(workspace, host, port, fingerprint)
            .await
        {
            Ok(stored) => stored,
            Err(_) => return false,
        };
        let Some(queue) = self.queue.upgrade() else {
            return false;
        };
        let Ok(mut queue) = queue.lock() else {
            return false;
        };
        // A database lookup may yield while the tab/workspace is closing.
        let Ok(entry) = queue.owned(workspace, &self.handle) else {
            return false;
        };
        if !Self::active(entry)
            || (entry.summary.status == "connected" && self.initial_fingerprint.is_none())
        {
            return false;
        }
        if stored == Some(false)
            || self
                .initial_fingerprint
                .as_deref()
                .is_some_and(|pin| pin != fingerprint)
            || entry
                .trusted_fingerprint
                .as_deref()
                .is_some_and(|pin| pin != fingerprint)
            || (self.initial_fingerprint.is_some() && entry.trusted_fingerprint.is_none())
        {
            entry.summary.error_code = Some("TEMPORARY_HOST_KEY_MISMATCH".into());
            return false;
        }
        let key = (workspace.to_owned(), host.to_ascii_lowercase(), port);
        if let Some(expected) = queue.trusted_hosts.get(&key) {
            if expected != fingerprint {
                queue
                    .owned(workspace, &self.handle)
                    .expect("owned while locked")
                    .summary
                    .error_code = Some("TEMPORARY_HOST_KEY_MISMATCH".into());
                return false;
            }
        } else {
            // Never restore a missing rekey pin or evict another trusted host.
            if self.initial_fingerprint.is_some() || queue.trusted_hosts.len() >= MAX_TRUSTED_HOSTS
            {
                queue
                    .owned(workspace, &self.handle)
                    .expect("owned while locked")
                    .summary
                    .error_code = Some("TEMPORARY_TRUST_CAPACITY".into());
                return false;
            }
            queue.trusted_hosts.insert(key, fingerprint.to_owned());
        }
        queue
            .owned(workspace, &self.handle)
            .expect("owned while locked")
            .trusted_fingerprint = Some(fingerprint.to_owned());
        self.initial_fingerprint
            .get_or_insert_with(|| fingerprint.to_owned());
        self.key_verified = true;
        true
    }
}
