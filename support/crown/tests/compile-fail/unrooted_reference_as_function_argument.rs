
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
//@rustc-env:RUSTC_BOOTSTRAP=1

#[crown::unrooted_must_root_lint::must_root]
struct MustBeRooted;

// This function definition is OK. We get a reference, so clearly the value must be rooted
// elsewhere.
fn do_something_that_can_gc(_: &MustBeRooted) {
    // If GC happens here and `self` is not rooted then we get a dangling reference.
}


fn main() {
    do_something_that_can_gc(&MustBeRooted)
    //~^ ERROR: 18:31: 18:43: Expression of type MustBeRooted must be rooted. [crown::unrooted_must_root]
}
