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
pub(crate) fn lock_blob(url: Url, relative_to: RelativeTo) -> ServoUrl  {
    let (base, resource_threads) = match relative_to {
        RelativeTo::Document(document) => {
            let global = document.global();
            (document.base_url(), global.resource_threads())
        },
        RelativeTo::Global(global) => (global.api_base_url(), global.resource_threads()),
    };

    let token = BlobResolver {
        origin: base.origin(),
        resource_threads: &resource_threads
    }.acquire_blob_token_for(&url);

    ServoUrl::from_url_with_token(url, token)
}

// pub(crate) fn resolve_blob_url(input: &str, relative_to: RelativeTo) -> Result<ServoUrl, url::ParseError> {
//     let global;
//     let (base, resource_sender) = match relative_to {
//         RelativeTo::Document(doc) => {
//             global = doc.global();
//             (doc.base_url(), global.resource_threads())
//         },
//         RelativeTo::Global(global) => (global.api_base_url(), global.resource_threads()),
//     };
//     let parsed = Url::options()
//         .base_url(Some(base.as_url()))
//         .parse(input)?;
//     ServoUrl::parse_with_base_and_blob_store(
//         Some(&base),
//         input,
//         &BlobResolver(&resource_sender.core_thread),
//     )
// }
