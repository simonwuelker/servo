/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://w3c.github.io/media-source/#dom-sourcebuffer

// TODO: Expose in dedicated worker
[Exposed=Window]
interface SourceBuffer : EventTarget {
//   attribute AppendMode mode;
//   readonly  attribute boolean updating;
//   readonly  attribute TimeRanges buffered;
//   attribute double timestampOffset;
  readonly  attribute AudioTrackList audioTracks;
//   readonly  attribute VideoTrackList videoTracks;
//   readonly  attribute TextTrackList textTracks;
//   attribute double appendWindowStart;
//   attribute unrestricted double appendWindowEnd;

//   attribute EventHandler onupdatestart;
//   attribute EventHandler onupdate;
//   attribute EventHandler onupdateend;
//   attribute EventHandler onerror;
//   attribute EventHandler onabort;

//   undefined appendBuffer(BufferSource data);
//   undefined abort();
//   undefined changeType(DOMString type);
//   undefined remove(double start, unrestricted double end);
};
