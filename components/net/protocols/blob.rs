/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::future::{Future, ready};
use std::pin::Pin;

use headers::{HeaderMapExt, Range};
use http::Method;
use log::debug;
use net_traits::blob_url_store::{BlobURLStoreError, parse_blob_url};
use net_traits::http_status::HttpStatus;
use net_traits::request::Request;
use net_traits::response::{Response, ResponseBody};
use net_traits::{NetworkError, ResourceFetchTiming};
use tokio::sync::mpsc::unbounded_channel;

use crate::fetch::methods::{Data, DoneChannel, FetchContext};
use crate::protocols::{ProtocolHandler, partial_content, range_not_satisfiable_error};

#[derive(Default)]
pub struct BlobProtocolHander {}

impl ProtocolHandler for BlobProtocolHander {
    fn load(
        &self,
        request: &mut Request,
        done_chan: &mut DoneChannel,
        context: &FetchContext,
    ) -> Pin<Box<dyn Future<Output = Response> + Send>> {
        // Part of https://fetch.spec.whatwg.org/#scheme-fetch
        let url = request.current_url();
        debug!("Loading blob {}", url.as_str());

        // Step 1. Let blobURLEntry be request’s current URL’s blob URL entry.
        let blob_url_entry = url.blob_url_entry();

        // Step 2. If request’s method is not `GET` or blobURLEntry is null, then return a network error. [FILEAPI]
        if request.method != Method::GET {
            return Box::pin(ready(Response::network_error(NetworkError::Internal(
                "Unexpected method for blob".into(),
            ))));
        }
        let Some(blob_url_entry) = blob_url_entry else {
            return Box::pin(ready(Response::network_error(NetworkError::Internal(
                "Unresolved blob url".into(),
            ))));
        };

        // TODO: Steps 3-7: Check blob authorization

        // Step 8. If blob is not a Blob object, then return a network error.
        // NOTE: Impossible.

        // Step 9. Let response be a new response.
        let mut response = Response::new(url, ResourceFetchTiming::new(request.timing_type()));

        // Step 10. Let fullLength be blob’s size.
        let full_length = blob.data.len();

        let range_header = request.headers.typed_get::<Range>();
        let is_range_request = range_header.is_some();

        match range_header {
            // Step 13. If request’s header list does not contain `Range`:
            None => {
                // Step 13.1 Let bodyWithType be the result of safely extracting blob.
                // NOTE: This is redundant because the blob url entry is not the actual blob

                // Step 13.2 Set response’s status message to `OK`.

                // Step 13.3 Set response’s body to bodyWithType’s body.

                // Step 13.4 Set response’s header list to « (`Content-Length`, serializedFullLength),
                // (`Content-Type`, type) ».

                response.status = HttpStatus::default();
            },
            // Step 14. Otherwise:
            Some(header) => {
                // Step 14.1 Set response’s range-requested flag.
                response.range_requested = true;

                // Step 14.2 Let rangeHeader be the result of getting `Range` from request’s header list.
                // NOTE: we already have the header

                // Steps 14.3 - 14.12 happen later in Filemanager::fetch_file

                // Step 14.13 Set response’s status to 206.
                // Step 14.14 Set response’s status message to `Partial Content`.
                response.status = StatusCode::PARTIAL_CONTENT.into();

                // Step 14.15 Set response’s header list to « (`Content-Length`, serializedSlicedLength),
                // (`Content-Type`, type), (`Content-Range`, contentRange) ».
            },
        }

        let (mut done_sender, done_receiver) = unbounded_channel();
        *done_chan = Some((done_sender.clone(), done_receiver));
        *response.body.lock().unwrap() = ResponseBody::Receiving(vec![]);

        if let Err(err) = context.filemanager.lock().unwrap().fetch_file(
            &mut done_sender,
            context.cancellation_listener.clone(),
            id,
            &context.file_token,
            origin,
            &mut response,
            range_header,
        ) {
            let _ = done_sender.send(Data::Done);
            let err = match err {
                BlobURLStoreError::InvalidRange => {
                    range_not_satisfiable_error(&mut response);
                    return Box::pin(ready(response));
                },
                _ => format!("{:?}", err),
            };
            return Box::pin(ready(Response::network_error(NetworkError::Internal(err))));
        };

        Box::pin(ready(response))
    }
}
