use net_traits::servo_url::{BlobResolver, BlobTokenCommunicator, ServoUrl};
use url::Url;

use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::document::Document;
use crate::dom::globalscope::GlobalScope;

pub(crate) enum RelativeTo<'a> {
    Document(&'a Document),
    Global(&'a GlobalScope),
}

/// This should be called immediately after parsing the url.
pub(crate) fn lock_blob(url: Url, relative_to: RelativeTo) -> ServoUrl {
    let (base, resource_threads) = match relative_to {
        RelativeTo::Document(document) => {
            let global = document.global();
            (document.base_url(), global.resource_threads())
        },
        RelativeTo::Global(global) => (global.api_base_url(), global.resource_threads()),
    };

    let token = BlobResolver {
        origin: base.origin(),
        resource_threads: &resource_threads,
    }
    .acquire_blob_token_for(&url);

    ServoUrl::from_url_with_token(url, token)
}

pub(crate) fn parse_url_and_lock_blob(
    input: &str,
    relative_to: RelativeTo,
) -> Result<ServoUrl, url::ParseError> {
    let url = input.parse()?;
    let locked_url = lock_blob(url, relative_to);

    Ok(locked_url)
}
