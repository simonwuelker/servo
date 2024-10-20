/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://w3c.github.io/media-source/#mediasource

// TODO expose in dedicated worker
[Exposed=Window]
interface MediaSource : EventTarget {
    constructor();

    // [SameObject, Exposed=DedicatedWorker]
    [SameObject]
    readonly  attribute MediaSourceHandle handle;
    readonly  attribute SourceBufferList sourceBuffers;
    // readonly  attribute SourceBufferList activeSourceBuffers;
    // readonly  attribute ReadyState readyState;

    // attribute unrestricted double duration;
    // attribute EventHandler onsourceopen;
    // attribute EventHandler onsourceended;
    // attribute EventHandler onsourceclose;

    // static readonly attribute boolean canConstructInDedicatedWorker;

    [Throws] SourceBuffer addSourceBuffer(DOMString type);
    // undefined removeSourceBuffer(SourceBuffer sourceBuffer);
    // undefined endOfStream(optional EndOfStreamError error);
    // undefined setLiveSeekableRange(double start, double end);
    // undefined clearLiveSeekableRange();
    static boolean isTypeSupported(DOMString type);
};
