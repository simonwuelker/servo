/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://w3c.github.io/media-source/#dom-sourcebufferlist

// TODO expose this on dedicated worker
[Exposed=Window]
interface SourceBufferList : EventTarget {
  readonly attribute unsigned long length;

//   attribute EventHandler onaddsourcebuffer;
//   attribute EventHandler onremovesourcebuffer;

//   getter SourceBuffer (unsigned long index);
};
