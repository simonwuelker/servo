/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
//@rustc-env:RUSTC_BOOTSTRAP=1

#[crown::unrooted_must_root_lint::must_root]
struct MustBeRooted;

impl MustBeRooted {
    fn do_something_that_can_gc(&self) {
        // If GC happens here and `self` is not rooted then we get a dangling reference.
    }
}


fn main() {
    MustBeRooted.do_something_that_can_gc();
    //~^ ERROR: 17:5: 17:17: Expression of type MustBeRooted must be rooted. [crown::unrooted_must_root]
}
