// SPDX-FileCopyrightText: 2023 Greenbone AG
//
// SPDX-License-Identifier: GPL-2.0-or-later

use std::sync::Arc;

use crate::feed::FeedIdentifier;

use super::context::Context;

pub async fn fetch<S, DB>(ctx: Arc<Context<S, DB>>)
where
    S: super::Scanner + 'static + std::marker::Send + std::marker::Sync,
    DB: crate::storage::Storage + 'static + std::marker::Send + std::marker::Sync,
{
    tracing::debug!("Starting VTS synchronization loop");
    if let Some(cfg) = &ctx.feed_config {
        let interval = cfg.verify_interval;
        let signature_check = cfg.signature_check;
        loop {
            let path = cfg.path.clone();
            if *ctx.abort.read().unwrap() {
                tracing::trace!("aborting");
                break;
            };
            let last_hash = ctx.db.feed_hash().await;
            if signature_check {
                if let Err(err) = feed::verify::check_signature(&path) {
                    tracing::warn!(
                        "Signature of {} is not corredct, skipping: {}",
                        path.display(),
                        err
                    );
                }
            }

            let hash =
                tokio::task::spawn_blocking(move || match FeedIdentifier::sumfile_hash(path) {
                    Ok(h) => h,
                    Err(e) => {
                        tracing::warn!("Failed to compute sumfile hash: {e:?}");
                        "".to_string()
                    }
                })
                .await
                .unwrap();
            if last_hash.is_empty() || last_hash != hash {
                match ctx.db.synchronize_feeds(hash).await {
                    Ok(_) => {}
                    Err(e) => tracing::warn!("Unable to sync feed: {e}"),
                }
            }
            tokio::time::sleep(interval).await;
        }
    }
}
